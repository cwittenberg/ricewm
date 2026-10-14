//! Bounded cross-native URI-list drops with target-side metadata validation.

use rice_files::{DropAction, EntryKind, EntryMetadata, GuestPath};
use rice_native_app_protocol::{
    DragAction, DragEnvelope, DragId, DragPhase, FileKind, Message, RequestId, VfsOperation,
    VfsRequest, VfsResponse, VfsResult,
};

use super::{CONTENT, FileControls, FilesActorError, RiceFiles, files_view};

const URI_LIST: &str = "text/uri-list";
const GNOME_FILES: &str = "x-special/gnome-copied-files";
const MAX_DROP_PATHS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ActiveExternalDrag {
    id: DragId,
    action: DropAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExternalDropTarget {
    Directory(GuestPath),
    Trash,
    Places,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingExternalDrop {
    request: RequestId,
    drag: Option<DragId>,
    action: DropAction,
    target: ExternalDropTarget,
    paths: Vec<GuestPath>,
    entries: Vec<EntryMetadata>,
    cancelled: bool,
}

impl PendingExternalDrop {
    pub(super) const fn request_id(&self) -> RequestId {
        self.request
    }

    fn request(&self, app: rice_native_app_protocol::AppId) -> Message {
        Message::VfsRequest(VfsRequest {
            request: self.request,
            app,
            operation: VfsOperation::Stat {
                path: self.paths[self.entries.len()].as_str().to_owned(),
            },
        })
    }
}

impl RiceFiles {
    pub(crate) fn handle_drag(
        &mut self,
        drag: &DragEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if drag.app != self.app || drag.window != self.window {
            return Ok(Vec::new());
        }
        if let Some(slot) = drag
            .target
            .and_then(|target| files_view::pane_for_widget(target.get()))
        {
            self.focus_pane(slot)?;
        }
        match drag.phase {
            DragPhase::Enter | DragPhase::Motion => self.accept_drag(drag),
            DragPhase::Leave => self.leave_drag(drag),
            DragPhase::Finished => self.finish_external_drag(drag),
            DragPhase::Drop => self.drop_external(drag, request),
        }
    }

    fn accept_drag(&mut self, drag: &DragEnvelope) -> Result<Vec<Message>, FilesActorError> {
        // Hit testing intentionally reports every widget inside RiceFiles so
        // the source can keep receiving pointer motion across the window. A
        // toolbar, menu, or preview widget is not a filesystem destination,
        // however, and must reject the offer before the drag visual claims it
        // can be dropped there.
        if self
            .drop_target(drag.target.map(|target| target.get()))
            .is_err()
        {
            self.active_external_drag = None;
            return Err(FilesActorError::UnexpectedDrag);
        }
        if !drag
            .mime_types
            .iter()
            .any(|mime| matches!(mime.as_str(), URI_LIST | GNOME_FILES))
        {
            self.active_external_drag = None;
            return self.emit_notice(rice_i18n::t("This drag does not contain a supported file list"));
        }
        let Some(action) = drop_action(drag.action) else {
            self.active_external_drag = None;
            return self.emit_notice(rice_i18n::t("Choose Copy, Move, or Link in the source application before dropping"));
        };
        self.active_external_drag = Some(ActiveExternalDrag {
            id: drag.drag,
            action,
        });
        self.runtime_mut()?.content_notice =
            rice_i18n::t(match drag.action {
                DragAction::Copy => "Release to copy the dragged files",
                DragAction::Move => "Release to move the dragged files",
                DragAction::Link => "Release to link the dragged files",
                DragAction::Ask => "Choose Copy, Move, or Link in the source application before dropping",
            }).to_owned();
        self.emit_full().map(|message| vec![message])
    }

    fn leave_drag(&mut self, drag: &DragEnvelope) -> Result<Vec<Message>, FilesActorError> {
        if self
            .active_external_drag
            .as_ref()
            .is_some_and(|active| active.id == drag.drag)
        {
            self.active_external_drag = None;
            self.runtime_mut()?.content_notice = String::from(rice_i18n::t("Drag cancelled before drop"));
            return self.emit_full().map(|message| vec![message]);
        }
        Ok(Vec::new())
    }

    fn finish_external_drag(
        &mut self,
        drag: &DragEnvelope,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self
            .pending_external_drop
            .as_ref()
            .is_some_and(|pending| pending.drag == Some(drag.drag))
        {
            if let Some(pending) = self.pending_external_drop.as_mut() {
                pending.cancelled = true;
            }
            self.runtime_mut()?.content_notice =
                String::from(rice_i18n::t("Cancellation requested; finishing the current metadata check"));
            return self.emit_full().map(|message| vec![message]);
        }
        self.leave_drag(drag)
    }

    fn drop_external(
        &mut self,
        drag: &DragEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let active = self
            .active_external_drag
            .take()
            .filter(|active| active.id == drag.drag)
            .ok_or(FilesActorError::UnexpectedDrag)?;
        if Some(active.action) != drop_action(drag.action) {
            return self.emit_notice(rice_i18n::t("The negotiated drag action changed before drop"));
        }
        let payload = drag
            .payload
            .as_ref()
            .ok_or(FilesActorError::UnexpectedDrag)?;
        let paths = match payload.mime_type.as_str() {
            URI_LIST => parse_uri_list(&payload.data)?,
            GNOME_FILES => parse_gnome_files(&payload.data)?.1,
            _ => return Err(FilesActorError::UnexpectedDrag),
        };
        let target = self.drop_target(drag.target.map(|target| target.get()))?;
        self.start_external_transfer_to(paths, target, active.action, request, Some(drag.drag))
    }

    pub(super) fn start_external_transfer(
        &mut self,
        paths: Vec<GuestPath>,
        target: GuestPath,
        action: DropAction,
        request: RequestId,
        drag: Option<DragId>,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.start_external_transfer_to(
            paths,
            ExternalDropTarget::Directory(target),
            action,
            request,
            drag,
        )
    }

    fn start_external_transfer_to(
        &mut self,
        paths: Vec<GuestPath>,
        target: ExternalDropTarget,
        action: DropAction,
        request: RequestId,
        drag: Option<DragId>,
    ) -> Result<Vec<Message>, FilesActorError> {
        if paths.is_empty()
            || self.runtime()?.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_external_drop.is_some()
        {
            return self.emit_notice(rice_i18n::t("Another filesystem operation is already in progress"));
        }
        let pending = PendingExternalDrop {
            request,
            drag,
            action,
            target,
            paths,
            entries: Vec::new(),
            cancelled: false,
        };
        let stat = pending.request(self.app);
        self.pending_external_drop = Some(pending);
        self.runtime_mut()?.content_notice =
            String::from(rice_i18n::t("Inspecting dragged files before transfer…"));
        Ok(vec![self.emit_full()?, stat])
    }

    pub(super) fn handle_external_drop_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let mut pending = self
            .pending_external_drop
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if pending.request != response.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        if pending.cancelled {
            return self.emit_notice(rice_i18n::t("Cross-application drop cancelled"));
        }
        let VfsResult::Metadata(metadata) = &response.result else {
            let notice = match &response.result {
                VfsResult::Error(error) => {
                    rice_i18n::format("Cannot transfer dragged file: {error}", &[("error", &error.message)])
                }
                _ => String::from(rice_i18n::t("Drag metadata response was invalid")),
            };
            return self.emit_system_notice("error", &notice);
        };
        let path = pending.paths[pending.entries.len()].clone();
        path.file_name()
            .ok_or(FilesActorError::InvalidDirectoryEntry)?;
        pending.entries.push(EntryMetadata::new(
            path,
            entry_kind(metadata.kind),
            metadata.size,
            metadata.modified_millis,
        ));
        if pending.entries.len() < pending.paths.len() {
            let stat = pending.request(self.app);
            self.pending_external_drop = Some(pending);
            return Ok(vec![self.emit_full()?, stat]);
        }
        let (plan, label) = match pending.target {
            ExternalDropTarget::Places => {
                if pending
                    .entries
                    .iter()
                    .any(|entry| entry.kind != EntryKind::Directory)
                {
                    return self.emit_notice(rice_i18n::t("Only folders can be added to Places"));
                }
                let paths = pending
                    .entries
                    .into_iter()
                    .map(|entry| entry.path)
                    .collect();
                return super::files_preferences::add_paths(self, paths, response.request);
            }
            ExternalDropTarget::Directory(target) => {
                let action = effective_drop_action(pending.action, &pending.entries, &target);
                (
                    FileControls::drop(
                        response.request.get(),
                        &self.home,
                        pending.entries,
                        target,
                        action,
                    )
                    .map_err(|_| FilesActorError::UnexpectedDrag)?,
                    action_label(action),
                )
            }
            ExternalDropTarget::Trash => (
                FileControls::trash(response.request.get(), &self.home, pending.entries)
                    .map_err(|_| FilesActorError::UnexpectedDrag)?,
                "Move to Trash",
            ),
        };
        self.begin_mutation(label, &plan, response.request)
    }

    fn drop_target(&self, target: Option<u64>) -> Result<ExternalDropTarget, FilesActorError> {
        let Some(mut target) = target else {
            return Ok(ExternalDropTarget::Directory(self.location().clone()));
        };
        loop {
            if let Ok(destination) = self.drop_target_exact(target) {
                return Ok(destination);
            }
            let Some(parent) = rice_native_app_protocol::WidgetId::new(target)
                .and_then(|id| self.tree.get(id))
                .and_then(|widget| widget.parent)
            else {
                return Err(FilesActorError::UnexpectedDrag);
            };
            target = parent.get();
        }
    }

    fn drop_target_exact(&self, target: u64) -> Result<ExternalDropTarget, FilesActorError> {
        match target {
            CONTENT => Ok(ExternalDropTarget::Directory(self.location().clone())),
            files_view::PLACES
            | files_view::PLACES_LIST
            | files_view::PLACES_HEADING
            | files_view::PLACE_ADD => Ok(ExternalDropTarget::Places),
            files_view::PLACE_HOME => GuestPath::absolute("/root")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_DOCUMENTS => GuestPath::absolute("/root/Documents")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_DESKTOP => GuestPath::absolute("/root/Desktop")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_DOWNLOADS => GuestPath::absolute("/root/Downloads")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_PICTURES => GuestPath::absolute("/root/Pictures")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_VIDEOS => GuestPath::absolute("/root/Videos")
                .map(ExternalDropTarget::Directory)
                .map_err(Into::into),
            files_view::PLACE_TRASH => Ok(ExternalDropTarget::Trash),
            files_view::PLACE_MOUNTS if self.local_files_available => {
                GuestPath::absolute(self.local_mount_root())
                    .map(ExternalDropTarget::Directory)
                    .map_err(Into::into)
            }
            target if files_view::bookmark_index(target).is_some() => {
                files_view::bookmark_index(target)
                    .and_then(|index| self.preferences.bookmarks().get(index))
                    .map(|bookmark| ExternalDropTarget::Directory(bookmark.path.clone()))
                    .ok_or(FilesActorError::UnexpectedDrag)
            }
            target if files_view::entry_for_drop_widget(target).is_some() => {
                let entry = files_view::entry_for_drop_widget(target)
                    .and_then(|(slot, index)| self.visible_entries_at(slot).get(index).cloned())
                    .ok_or(FilesActorError::UnexpectedDrag)?;
                if entry.kind == EntryKind::Directory {
                    Ok(ExternalDropTarget::Directory(entry.path))
                } else {
                    // File-manager convention: dropping over a non-directory
                    // item targets the directory shown by that pane.
                    Ok(ExternalDropTarget::Directory(self.location().clone()))
                }
            }
            target if files_view::pane_for_widget(target).is_some() => {
                Ok(ExternalDropTarget::Directory(self.location().clone()))
            }
            _ => Err(FilesActorError::UnexpectedDrag),
        }
    }
}

fn effective_drop_action(
    requested: DropAction,
    entries: &[EntryMetadata],
    target: &GuestPath,
) -> DropAction {
    if requested != DropAction::Move {
        return requested;
    }
    let target_is_local =
        target.as_str() == "/mnt/local" || target.as_str().starts_with("/mnt/local/");
    let crosses_mount = entries.iter().any(|entry| {
        let source_is_local =
            entry.path.as_str() == "/mnt/local" || entry.path.as_str().starts_with("/mnt/local/");
        source_is_local != target_is_local
    });
    if crosses_mount {
        DropAction::Copy
    } else {
        DropAction::Move
    }
}

pub(super) fn parse_uri_list(data: &[u8]) -> Result<Vec<GuestPath>, FilesActorError> {
    let text = core::str::from_utf8(data).map_err(|_| FilesActorError::UnexpectedDrag)?;
    let mut paths = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if paths.len() == MAX_DROP_PATHS {
            return Err(FilesActorError::EntryLimit);
        }
        let value = line
            .strip_prefix("file://")
            .ok_or(FilesActorError::UnexpectedDrag)?;
        if !value.starts_with('/') {
            return Err(FilesActorError::UnexpectedDrag);
        }
        let decoded = percent_decode(value)?;
        let path = GuestPath::absolute(&decoded)?;
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    Ok(paths)
}

pub(super) fn parse_gnome_files(
    data: &[u8],
) -> Result<(DropAction, Vec<GuestPath>), FilesActorError> {
    let text = core::str::from_utf8(data).map_err(|_| FilesActorError::UnexpectedDrag)?;
    let (verb, files) = text
        .split_once('\n')
        .ok_or(FilesActorError::UnexpectedDrag)?;
    let action = match verb.trim() {
        "copy" => DropAction::Copy,
        "cut" => DropAction::Move,
        _ => return Err(FilesActorError::UnexpectedDrag),
    };
    Ok((action, parse_uri_list(files.as_bytes())?))
}

fn percent_decode(value: &str) -> Result<String, FilesActorError> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or(FilesActorError::UnexpectedDrag)?;
            let text = core::str::from_utf8(pair).map_err(|_| FilesActorError::UnexpectedDrag)?;
            output.push(u8::from_str_radix(text, 16).map_err(|_| FilesActorError::UnexpectedDrag)?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| FilesActorError::UnexpectedDrag)
}

const fn drop_action(action: DragAction) -> Option<DropAction> {
    match action {
        DragAction::Copy => Some(DropAction::Copy),
        DragAction::Move => Some(DropAction::Move),
        DragAction::Link => Some(DropAction::Link),
        DragAction::Ask => None,
    }
}

const fn action_label(action: DropAction) -> &'static str {
    match action {
        DropAction::Copy => "Copy",
        DropAction::Move => "Move",
        DropAction::Link => "Link",
    }
}

const fn entry_kind(kind: FileKind) -> EntryKind {
    match kind {
        FileKind::File => EntryKind::File,
        FileKind::Directory => EntryKind::Directory,
        FileKind::Symlink => EntryKind::Symlink,
        FileKind::Other => EntryKind::Other,
    }
}
