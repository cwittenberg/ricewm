//! Native file clipboard and bounded Desktop transfer state machines.

use rice_native_app_protocol::{
    ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection, DragAction,
    DragEnvelope, DragId, DragPayload, DragPhase, DragSourceEvent, DragSourceResult,
    DragSourceState, Message, RequestId, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse,
    VfsResult,
};

use super::{DesktopActorError, RiceDesktop};

const GNOME_FILES: &str = "x-special/gnome-copied-files";
const URI_LIST: &str = "text/uri-list";
const MAX_CLIPBOARD_FILES: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
enum DesktopDropTarget {
    Background,
    Directory(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingClipboard {
    Write { request: RequestId, cut: bool },
    Paste { request: RequestId, target: String },
    Location { request: RequestId },
}

impl PendingClipboard {
    pub(super) fn request(&self) -> RequestId {
        match self {
            Self::Write { request, .. }
            | Self::Paste { request, .. }
            | Self::Location { request } => *request,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransferAction {
    Copy,
    Move,
    Link,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TransferStep {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) action: TransferAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingTransfer {
    pub request: RequestId,
    steps: Vec<TransferStep>,
    position: usize,
    applying: bool,
}

impl RiceDesktop {
    pub(super) fn begin_source_drag(
        &mut self,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Option<Message>, DesktopActorError> {
        let paths = self.draggable_paths();
        if paths.is_empty() {
            self.source_drag = None;
            return Ok(None);
        }
        let mut data = Vec::new();
        for path in paths {
            data.extend_from_slice(b"file://");
            data.extend_from_slice(encode_uri_path(&path).as_bytes());
            data.extend_from_slice(b"\r\n");
        }
        let drag = DragId::new(request.get()).ok_or(DesktopActorError::InvalidClipboard)?;
        self.source_drag = Some(drag);
        self.source_repositioned = false;
        Ok(Some(Message::DragSource(DragSourceEvent {
            drag,
            app: self.app,
            window: self.window,
            state: DragSourceState::Offer {
                action: source_action(modifiers),
                mime_types: vec![String::from(URI_LIST), String::from(GNOME_FILES)],
                payload: DragPayload {
                    mime_type: String::from(URI_LIST),
                    data,
                },
            },
        })))
    }

    pub(super) fn cancel_source_drag(&self, drag: DragId) -> Message {
        Message::DragSource(DragSourceEvent {
            drag,
            app: self.app,
            window: self.window,
            state: DragSourceState::Cancel,
        })
    }

    pub(crate) fn finish_source_drag(
        &mut self,
        event: &DragSourceEvent,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if event.app != self.app || event.window != self.window {
            return Ok(Vec::new());
        }
        let Some(active) = self.source_drag else {
            return Ok(Vec::new());
        };
        if active != event.drag {
            return Err(DesktopActorError::InvalidClipboard);
        }
        let DragSourceState::Finished(result) = event.state else {
            return Err(DesktopActorError::InvalidClipboard);
        };
        self.source_drag = None;
        self.drag = None;
        let repositioned = core::mem::take(&mut self.source_repositioned);
        if matches!(result, DragSourceResult::Completed(DragAction::Move))
            && !repositioned
            && !self.busy_with_vfs()
        {
            let refresh = RequestId::new(event.drag.get() | (1_u64 << 63))
                .ok_or(DesktopActorError::InvalidClipboard)?;
            return self.begin_refresh(refresh, None);
        }
        let notice = match result {
            DragSourceResult::Completed(DragAction::Copy) => "Files copied to the destination",
            DragSourceResult::Completed(DragAction::Move) if repositioned => {
                "Desktop icons repositioned"
            }
            DragSourceResult::Completed(DragAction::Move) => "Files moved to the destination",
            DragSourceResult::Completed(DragAction::Link) => "Links created at the destination",
            DragSourceResult::Completed(DragAction::Ask) => "The destination completed the drop",
            DragSourceResult::Rejected => "This location does not accept file drops",
            DragSourceResult::Cancelled => "File drag cancelled",
            DragSourceResult::Failed => "The destination did not complete the drop",
        };
        self.notice(notice)
    }

    pub(crate) fn handle_external_drag(
        &mut self,
        drag: &DragEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if drag.app != self.app || drag.window != self.window {
            return Ok(Vec::new());
        }
        match drag.phase {
            DragPhase::Enter | DragPhase::Motion => {
                if self.busy_with_vfs()
                    || !drag
                        .mime_types
                        .iter()
                        .any(|mime| matches!(mime.as_str(), GNOME_FILES | URI_LIST))
                {
                    self.external_drag = None;
                    return Err(DesktopActorError::InvalidClipboard);
                }
                let action = transfer_action(drag.action)?;
                let target = self.drop_target(drag.target.map(|target| target.get()))?;
                if self.source_drag == Some(drag.drag)
                    && matches!(target, DesktopDropTarget::Background)
                    && self.drag.is_none()
                {
                    self.external_drag = None;
                    return Err(DesktopActorError::InvalidClipboard);
                }
                self.external_drag = Some((drag.drag, action));
                Ok(Vec::new())
            }
            DragPhase::Leave | DragPhase::Finished => {
                if self
                    .external_drag
                    .is_some_and(|(active, _)| active == drag.drag)
                {
                    self.external_drag = None;
                }
                Ok(Vec::new())
            }
            DragPhase::Drop => {
                let (_, action) = self
                    .external_drag
                    .take()
                    .filter(|(active, _)| *active == drag.drag)
                    .ok_or(DesktopActorError::InvalidClipboard)?;
                if action != transfer_action(drag.action)? {
                    return Err(DesktopActorError::InvalidClipboard);
                }
                let target = self.drop_target(drag.target.map(|target| target.get()))?;
                if self.source_drag == Some(drag.drag)
                    && matches!(target, DesktopDropTarget::Background)
                {
                    self.source_repositioned = true;
                    let messages = self.drag_icons(drag.x, drag.y, request)?;
                    self.drag = None;
                    return Ok(messages);
                }
                let target = match target {
                    DesktopDropTarget::Directory(path) => path,
                    DesktopDropTarget::Background => String::from(super::desktop_vfs::DESKTOP_PATH),
                };
                let payload = drag
                    .payload
                    .as_ref()
                    .ok_or(DesktopActorError::InvalidClipboard)?;
                let (_, paths) = parse_file_clipboard(&payload.mime_type, &payload.data)?;
                let steps = paths
                    .into_iter()
                    .filter_map(|from| {
                        let name = file_name(&from)?;
                        Some(TransferStep {
                            to: format!("{target}/{name}"),
                            from,
                            action,
                        })
                    })
                    .collect();
                self.start_transfer(request, steps)
            }
        }
    }

    fn drop_target(&self, target: Option<u64>) -> Result<DesktopDropTarget, DesktopActorError> {
        let Some(target) = target else {
            return Ok(DesktopDropTarget::Background);
        };
        if matches!(target, super::ROOT | super::ICON_GRID) || target >= super::CELL_BASE {
            return Ok(DesktopDropTarget::Background);
        }
        if let Some(item) = self.item_for_widget(target)
            && let Some(entry) = self.model.item(item)
            && matches!(entry.kind, rice_desktop::DesktopItemKind::Folder)
            && let Some(path) = &entry.path
        {
            return Ok(DesktopDropTarget::Directory(path.clone()));
        }
        // Dropping over a regular desktop icon conventionally targets the
        // desktop directory surrounding it.
        self.item_for_widget(target)
            .map(|_| DesktopDropTarget::Background)
            .ok_or(DesktopActorError::InvalidClipboard)
    }

    pub(super) fn copy_selection(
        &mut self,
        request: RequestId,
        cut: bool,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_clipboard.is_some() || self.busy_with_vfs() {
            return self.notice("Another Desktop operation is already in progress");
        }
        let paths = self.selected_paths();
        if paths.is_empty() {
            return self.notice("Select at least one filesystem item first");
        }
        let mut text = if cut {
            String::from("cut\n")
        } else {
            String::from("copy\n")
        };
        for path in paths {
            text.push_str("file://");
            text.push_str(&encode_uri_path(&path));
            text.push('\n');
        }
        self.pending_clipboard = Some(PendingClipboard::Write { request, cut });
        Ok(vec![Message::ClipboardRequest(ClipboardRequest::Write {
            request,
            selection: ClipboardSelection::Clipboard,
            mime_type: String::from(GNOME_FILES),
            data: text.into_bytes(),
        })])
    }

    pub(super) fn paste_files(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_clipboard.is_some() || self.busy_with_vfs() {
            return self.notice("Another Desktop operation is already in progress");
        }
        let target = self
            .selected_item()
            .filter(|item| matches!(item.kind, rice_desktop::DesktopItemKind::Folder))
            .and_then(|item| item.path.clone())
            .unwrap_or_else(|| String::from(super::desktop_vfs::DESKTOP_PATH));
        self.pending_clipboard = Some(PendingClipboard::Paste { request, target });
        Ok(vec![Message::ClipboardRequest(ClipboardRequest::Read {
            request,
            selection: ClipboardSelection::Clipboard,
            accepted_mime_types: vec![String::from(GNOME_FILES), String::from(URI_LIST)],
        })])
    }

    pub(crate) fn handle_clipboard_response(
        &mut self,
        response: &ClipboardResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let pending = self
            .pending_clipboard
            .take()
            .ok_or(DesktopActorError::UnexpectedClipboardResponse)?;
        if pending.request() != response.request {
            self.pending_clipboard = Some(pending);
            return Err(DesktopActorError::UnexpectedClipboardResponse);
        }
        match (pending, &response.result) {
            (PendingClipboard::Write { cut, .. }, ClipboardResult::Data { .. }) => {
                self.notice(if cut {
                    "Files cut to the clipboard"
                } else {
                    "Files copied to the clipboard"
                })
            }
            (PendingClipboard::Location { .. }, ClipboardResult::Data { .. }) => {
                self.notice("Location copied to the clipboard")
            }
            (PendingClipboard::Paste { target, .. }, ClipboardResult::Data { mime_type, data })
                if matches!(mime_type.as_str(), GNOME_FILES | URI_LIST) =>
            {
                let (action, paths) = parse_file_clipboard(mime_type, data)?;
                let steps = paths
                    .into_iter()
                    .filter_map(|from| {
                        let name = file_name(&from)?;
                        Some(TransferStep {
                            to: format!("{target}/{name}"),
                            from,
                            action,
                        })
                    })
                    .collect();
                self.start_transfer(response.request, steps)
            }
            (_, ClipboardResult::Empty) => self.notice("The file clipboard is empty"),
            (_, ClipboardResult::Rejected) => {
                self.notice("The clipboard host rejected the file operation")
            }
            _ => self.notice("The clipboard does not contain a supported file list"),
        }
    }

    pub(super) fn handle_transfer_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_transfer
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        let step = &pending.steps[pending.position];
        if !pending.applying {
            return match &response.result {
                VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => {
                    pending.applying = true;
                    let operation = transfer_operation(step);
                    self.pending_transfer = Some(pending);
                    Ok(vec![vfs_request(self.app, response.request, operation)])
                }
                VfsResult::Metadata(_) => {
                    self.notice("File transfer stopped: the destination already exists")
                }
                VfsResult::Error(error) => self.notice(&format!(
                    "File transfer stopped while checking its destination: {}",
                    error.message
                )),
                _ => self.notice("File transfer stopped: invalid collision-check response"),
            };
        }
        match &response.result {
            VfsResult::Progress(_) => {
                // Browser-backed copies stream byte progress before their
                // terminal response. The Desktop has no progress panel, but
                // it must retain the transfer so Done, Error, or Cancelled can
                // finish the same request.
                self.pending_transfer = Some(pending);
                Ok(Vec::new())
            }
            VfsResult::Done => {
                pending.position += 1;
                if pending.position == pending.steps.len() {
                    return self.begin_refresh(
                        response.request,
                        Some(String::from("File transfer completed")),
                    );
                }
                pending.applying = false;
                let destination = pending.steps[pending.position].to.clone();
                self.pending_transfer = Some(pending);
                Ok(vec![vfs_request(
                    self.app,
                    response.request,
                    VfsOperation::Stat { path: destination },
                )])
            }
            VfsResult::Error(error) => {
                self.notice(&format!("File transfer failed: {}", error.message))
            }
            _ => self.notice("File transfer failed: invalid VFS response"),
        }
    }

    pub(super) fn start_transfer(
        &mut self,
        request: RequestId,
        steps: Vec<TransferStep>,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if steps.is_empty() {
            return self.notice("No valid filesystem items were available");
        }
        if steps.len() > MAX_CLIPBOARD_FILES {
            return self.notice("The file clipboard exceeds the Desktop item limit");
        }
        let destination = steps[0].to.clone();
        self.pending_transfer = Some(PendingTransfer {
            request,
            steps,
            position: 0,
            applying: false,
        });
        Ok(vec![vfs_request(
            self.app,
            request,
            VfsOperation::Stat { path: destination },
        )])
    }

    fn selected_paths(&self) -> Vec<String> {
        self.model
            .selection()
            .selected()
            .filter_map(|id| self.model.item(id)?.path.clone())
            .take(MAX_CLIPBOARD_FILES + 1)
            .collect()
    }

    fn draggable_paths(&self) -> Vec<String> {
        self.model
            .selection()
            .selected()
            .filter_map(|id| {
                let item = self.model.item(id)?;
                matches!(
                    item.kind,
                    rice_desktop::DesktopItemKind::File
                        | rice_desktop::DesktopItemKind::Folder
                        | rice_desktop::DesktopItemKind::Application { .. }
                )
                .then(|| item.path.clone())
                .flatten()
            })
            .take(MAX_CLIPBOARD_FILES + 1)
            .collect()
    }

    pub(super) fn busy_with_vfs(&self) -> bool {
        self.link_chooser.is_some()
            || self.pending_link_activation.is_some()
            || self.pending_transfer.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
            || self.pending_create.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
    }
}

fn transfer_operation(step: &TransferStep) -> VfsOperation {
    match step.action {
        TransferAction::Copy => VfsOperation::Copy {
            from: step.from.clone(),
            to: step.to.clone(),
            recursive: true,
        },
        TransferAction::Move => VfsOperation::Rename {
            from: step.from.clone(),
            to: step.to.clone(),
        },
        TransferAction::Link => VfsOperation::Link {
            from: step.from.clone(),
            to: step.to.clone(),
        },
    }
}

fn transfer_action(action: DragAction) -> Result<TransferAction, DesktopActorError> {
    match action {
        DragAction::Copy => Ok(TransferAction::Copy),
        DragAction::Move => Ok(TransferAction::Move),
        DragAction::Link => Ok(TransferAction::Link),
        DragAction::Ask => Err(DesktopActorError::InvalidClipboard),
    }
}

const fn source_action(modifiers: u16) -> DragAction {
    if modifiers & 1 != 0 {
        DragAction::Copy
    } else {
        match modifiers & 12 {
            12 => DragAction::Ask,
            4 => DragAction::Copy,
            8 => DragAction::Link,
            _ => DragAction::Move,
        }
    }
}

fn parse_file_clipboard(
    mime_type: &str,
    bytes: &[u8],
) -> Result<(TransferAction, Vec<String>), DesktopActorError> {
    let text = core::str::from_utf8(bytes).map_err(|_| DesktopActorError::InvalidClipboard)?;
    let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let action = if mime_type == GNOME_FILES {
        match lines.next() {
            Some("cut") => TransferAction::Move,
            Some("copy") => TransferAction::Copy,
            _ => return Err(DesktopActorError::InvalidClipboard),
        }
    } else {
        TransferAction::Copy
    };
    let paths = lines
        .filter(|line| !line.starts_with('#'))
        .map(decode_file_uri)
        .collect::<Result<Vec<_>, _>>()?;
    if paths.is_empty() || paths.len() > MAX_CLIPBOARD_FILES {
        return Err(DesktopActorError::InvalidClipboard);
    }
    Ok((action, paths))
}

fn decode_file_uri(uri: &str) -> Result<String, DesktopActorError> {
    let encoded = uri
        .strip_prefix("file://")
        .ok_or(DesktopActorError::InvalidClipboard)?;
    let path = percent_decode(encoded)?;
    if !path.starts_with('/') || path.split('/').any(|part| part == "..") {
        return Err(DesktopActorError::InvalidClipboard);
    }
    Ok(path)
}

fn percent_decode(value: &str) -> Result<String, DesktopActorError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex(*bytes
                .get(index + 1)
                .ok_or(DesktopActorError::InvalidClipboard)?)?;
            let low = hex(*bytes
                .get(index + 2)
                .ok_or(DesktopActorError::InvalidClipboard)?)?;
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).map_err(|_| DesktopActorError::InvalidClipboard)
}

fn encode_uri_path(path: &str) -> String {
    path.replace('%', "%25")
        .replace(' ', "%20")
        .replace('#', "%23")
}

fn file_name(path: &str) -> Option<&str> {
    path.rsplit('/').find(|part| !part.is_empty())
}

fn hex(value: u8) -> Result<u8, DesktopActorError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(DesktopActorError::InvalidClipboard),
    }
}

fn vfs_request(
    app: rice_native_app_protocol::AppId,
    request: RequestId,
    operation: VfsOperation,
) -> Message {
    Message::VfsRequest(VfsRequest {
        request,
        app,
        operation,
    })
}
