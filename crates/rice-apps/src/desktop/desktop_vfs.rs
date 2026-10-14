//! Bounded VFS enumeration and mutation state for `RiceDesktop`.

use super::DesktopNotice;
use super::desktop_path::{error_message, valid_file_name, validate_entries};
use super::{
    AppId, BTreeMap, DesktopActorError, DesktopCommand, DesktopItem, DesktopItemId,
    DesktopItemKind, Message, RiceDesktop, replace_item_patches,
};
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, RequestId, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse,
    VfsResult,
};
use std::collections::BTreeSet;

pub(super) const DESKTOP_PATH: &str = "/root/Desktop";
pub(super) const TRASH_FILES_PATH: &str = "/root/.local/share/Trash/files";
const PAGE_LIMIT: u16 = 128;
const MAX_DESKTOP_ITEMS: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingDirectory {
    pub request: RequestId,
    entries: Vec<DirectoryEntry>,
    completion_notice: Option<String>,
}

impl PendingDirectory {
    pub(super) fn new(request: RequestId, completion_notice: Option<String>) -> Self {
        Self {
            request,
            entries: Vec::new(),
            completion_notice,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MutationKind { Rename, DeletePermanently }
impl MutationKind {
    // Only used for existing ignored completion strings; visible failures use full messages.
    fn completion_label(&self) -> &'static str {
        match self { Self::Rename => "Rename", Self::DeletePermanently => "Delete permanently" }
    }
    fn failure(&self, error: &str) -> DesktopNotice {
        let detail = DesktopNotice::error_detail(error);
        match self {
            Self::Rename => DesktopNotice::template("desktop.rename.failed", vec![("error", detail)]),
            Self::DeletePermanently => DesktopNotice::template("desktop.delete-permanently.failed", vec![("error", detail)]),
        }
    }
    fn invalid_response(&self) -> DesktopNotice {
        match self {
            Self::Rename => DesktopNotice::key("desktop.rename.invalid-response"),
            Self::DeletePermanently => DesktopNotice::key("desktop.delete-permanently.invalid-response"),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingMutation {
    pub request: RequestId,
    kind: MutationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingRename {
    request: RequestId,
    from: String,
    to: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingTrash {
    pub request: RequestId,
    items: Vec<PendingTrashItem>,
    names_seen: BTreeSet<String>,
    entries_seen: usize,
    moving_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingTrashStatus {
    pub request: RequestId,
    cursor: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingTrashItem {
    from: String,
    to: String,
    name: String,
}

impl RiceDesktop {
    pub(super) fn begin_trash_status(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_trash_status.is_some() {
            return Ok(Vec::new());
        }
        self.pending_trash_status = Some(PendingTrashStatus { request, cursor: 0 });
        Ok(vec![trash_status_request(self.app, request, 0)])
    }

    pub(super) fn initial_directory_request(&mut self, request: RequestId) -> Message {
        self.pending_directory = Some(PendingDirectory::new(request, None));
        directory_request(self.app, request, 0)
    }

    pub(super) fn begin_refresh(
        &mut self,
        request: RequestId,
        completion_notice: Option<String>,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_directory.is_some()
            || self.pending_transfer.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_create.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
        {
            return Ok(Vec::new());
        }
        self.pending_intents.clear();
        self.pending_directory = Some(PendingDirectory::new(request, completion_notice));
        Ok(vec![directory_request(self.app, request, 0)])
    }

    pub(super) fn begin_rename(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(from) = self.rename_source_path.clone() else {
            return self.notice_message(DesktopNotice::key("desktop.notice.33c617d542f4"));
        };
        if !self
            .items
            .iter()
            .any(|item| item.path.as_deref() == Some(from.as_str()) && item.is_renameable())
        {
            return self.notice_message(DesktopNotice::key("desktop.notice.33c617d542f4"));
        }
        let name = self.rename_text.trim();
        if !valid_file_name(name) {
            return self.notice_message(DesktopNotice::key("desktop.notice.ef20be863ab8"));
        }
        let to = format!("{DESKTOP_PATH}/{name}");
        if from == to {
            // An unchanged submission is a harmless cancel, not an error.
            // Closing the editor also clears any stale information balloon
            // left by older builds instead of pinning it to the desktop.
            return self.close_rename_editor();
        }
        if self.pending_directory.is_some() {
            self.pending_rename = Some(PendingRename { request, from, to });
            self.watch_refresh_pending = false;
            return self.close_rename_editor();
        }
        let mut messages =
            self.begin_mutation(request, MutationKind::Rename, VfsOperation::Rename { from, to })?;
        if self
            .pending_mutation
            .as_ref()
            .is_some_and(|pending| pending.request == request)
        {
            let mut closed = self.close_rename_editor()?;
            closed.append(&mut messages);
            Ok(closed)
        } else {
            Ok(messages)
        }
    }

    pub(super) fn begin_move_to_trash(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let selected = self.model.selection().selected_vec();
        if selected.is_empty() {
            return self.notice_message(DesktopNotice::key("desktop.notice.9639deae6708"));
        }
        let mut items = Vec::with_capacity(selected.len());
        for selected in selected {
            let Some(item) = self.model.item(selected) else {
                return self.notice_message(DesktopNotice::key("desktop.notice.caecdc48adf3"));
            };
            if !item.can_delete() {
                return self.notice_message(DesktopNotice::key("desktop.notice.0be4c8ab20b4"));
            }
            let Some(path) = item.path.as_deref() else {
                return self.notice_message(DesktopNotice::key("desktop.notice.7ef96ba6f6de"));
            };
            let Some(name) = path.strip_prefix(&format!("{DESKTOP_PATH}/")) else {
                return self.notice_message(DesktopNotice::key("desktop.notice.833663669663"));
            };
            if !valid_file_name(name) {
                return self.notice_message(DesktopNotice::key("desktop.notice.1c58d592132c"));
            }
            items.push(PendingTrashItem {
                from: path.to_owned(),
                to: format!("{TRASH_FILES_PATH}/{name}"),
                name: name.to_owned(),
            });
        }
        if self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_create.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
        {
            return self.notice_message(DesktopNotice::key("desktop.notice.3082e580a36b"));
        }
        self.pending_intents.clear();
        self.pending_trash = Some(PendingTrash {
            request,
            items,
            names_seen: BTreeSet::new(),
            entries_seen: 0,
            moving_index: None,
        });
        Ok(vec![trash_directory_request(self.app, request, 0)])
    }

    pub(super) fn begin_permanent_delete(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut paths = Vec::new();
        for selected in self.model.selection().selected_vec() {
            let Some(item) = self.model.item(selected) else {
                return self.notice_message(DesktopNotice::key("desktop.notice.caecdc48adf3"));
            };
            let Some(path) = item.path.as_deref() else {
                return self.notice_message(DesktopNotice::key("desktop.notice.61c897b19a78"));
            };
            if !item.can_delete() || !path.starts_with(&format!("{DESKTOP_PATH}/")) {
                return self
                    .notice_message(DesktopNotice::key("desktop.notice.180146d2a43c"));
            }
            paths.push(path.to_owned());
        }
        if paths.is_empty() {
            return self.notice_message(DesktopNotice::key("desktop.notice.c43fe3dc7658"));
        }
        paths.sort();
        if self.permanent_delete_confirmation != paths {
            self.permanent_delete_confirmation = paths;
            return self
                .notice_message(DesktopNotice::key("desktop.notice.3e358519ab7f"));
        }
        if self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_create.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
        {
            return self.notice_message(DesktopNotice::key("desktop.notice.3082e580a36b"));
        }
        self.permanent_delete_confirmation.clear();
        self.permanent_delete_total = paths.len();
        self.permanent_delete_queue = paths;
        let first = self.permanent_delete_queue.remove(0);
        self.begin_mutation(
            request,
            MutationKind::DeletePermanently,
            VfsOperation::Remove {
                path: first,
                recursive: true,
            },
        )
    }

    fn begin_mutation(
        &mut self,
        request: RequestId,
        kind: MutationKind,
        operation: VfsOperation,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_create.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
        {
            return self.notice_message(DesktopNotice::key("desktop.notice.3082e580a36b"));
        }
        self.pending_intents.clear();
        self.pending_mutation = Some(PendingMutation { request, kind });
        Ok(vec![Message::VfsRequest(VfsRequest {
            request,
            app: self.app,
            operation,
        })])
    }

    fn start_pending_rename(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let Some(pending) = self.pending_rename.take() else {
            return Ok(Vec::new());
        };
        if !self
            .items
            .iter()
            .any(|item| item.path.as_deref() == Some(pending.from.as_str()) && item.is_renameable())
        {
            return self.notice_message(DesktopNotice::key("desktop.notice.33c617d542f4"));
        }
        self.pending_intents.clear();
        self.pending_mutation = Some(PendingMutation {
            request: pending.request,
            kind: MutationKind::Rename,
        });
        Ok(vec![Message::VfsRequest(VfsRequest {
            request: pending.request,
            app: self.app,
            operation: VfsOperation::Rename {
                from: pending.from,
                to: pending.to,
            },
        })])
    }

    pub(crate) fn handle_vfs_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.owns_link_chooser_request(response.request) {
            return self.handle_link_chooser_response(response);
        }
        if self
            .pending_link_activation
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_link_activation_response(response);
        }
        if self.owns_link_icon_request(response.request) {
            return self.handle_link_icon_response(response);
        }
        if self
            .pending_trash_status
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_trash_status_response(response);
        }
        if self
            .pending_properties
            .as_ref()
            .is_some_and(|pending| pending.awaits(response.request))
        {
            return self.handle_properties_response(response);
        }
        if self
            .pending_empty_trash
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_empty_trash_response(response);
        }
        if self
            .pending_trash_menu
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_trash_menu_response(response);
        }
        if self
            .pending_transfer
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_transfer_response(response);
        }
        if self
            .pending_create
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_create_response(response);
        }
        if self
            .pending_desktop_launch
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_desktop_launch_response(response);
        }
        if self
            .pending_trash
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_trash_response(response);
        }
        if self
            .pending_mutation
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_mutation_response(response);
        }
        if self
            .pending_directory
            .as_ref()
            .is_some_and(|pending| pending.request == response.request)
        {
            return self.handle_directory_response(response);
        }
        Err(DesktopActorError::UnexpectedVfsResponse)
    }

    fn handle_trash_status_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_trash_status
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        let nonempty = match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                validate_entries(entries)?;
                if !entries.is_empty() {
                    true
                } else if let Some(cursor) = next_cursor {
                    if *cursor == pending.cursor {
                        return Err(DesktopActorError::UnexpectedVfsResponse);
                    }
                    pending.cursor = *cursor;
                    self.pending_trash_status = Some(pending);
                    return Ok(vec![trash_status_request(
                        self.app,
                        response.request,
                        *cursor,
                    )]);
                } else {
                    false
                }
            }
            VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => false,
            VfsResult::Error(_) => return Ok(Vec::new()),
            _ => return Err(DesktopActorError::UnexpectedVfsResponse),
        };
        self.set_trash_nonempty(nonempty)
    }

    pub(super) fn set_trash_nonempty(
        &mut self,
        nonempty: bool,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.trash_nonempty = nonempty;
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
        else {
            return Ok(Vec::new());
        };
        if item.kind == (DesktopItemKind::Trash { empty: !nonempty }) {
            return Ok(Vec::new());
        }
        item.kind = DesktopItemKind::Trash { empty: !nonempty };
        item.icon_id = if nonempty { "trash-full" } else { "trash" }.to_owned();
        if let Some(model_item) = self.model.item_mut(item.id) {
            model_item.kind = item.kind.clone();
            model_item.icon_id.clone_from(&item.icon_id);
        }
        let patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        self.emit(&patches).map(|message| vec![message])
    }

    fn handle_trash_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_trash
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if let Some(index) = pending.moving_index {
            return match &response.result {
                VfsResult::Done => {
                    let next = index.saturating_add(1);
                    if let Some(item) = pending.items.get(next) {
                        let operation = VfsOperation::Rename {
                            from: item.from.clone(),
                            to: item.to.clone(),
                        };
                        pending.moving_index = Some(next);
                        self.pending_trash = Some(pending);
                        Ok(vec![Message::VfsRequest(VfsRequest {
                            request: response.request,
                            app: self.app,
                            operation,
                        })])
                    } else {
                        let count = pending.items.len();
                        self.pending_directory = Some(PendingDirectory::new(
                            response.request,
                            Some(format!(
                                "Moved {count} item{} to Trash",
                                if count == 1 { "" } else { "s" }
                            )),
                        ));
                        let mut messages = self.set_trash_nonempty(true)?;
                        messages.push(directory_request(self.app, response.request, 0));
                        Ok(messages)
                    }
                }
                VfsResult::Error(error) => {
                    let moved = index;
                    if moved == 0 {
                        self.notice_message(DesktopNotice::template("desktop.trash.failed",
                            vec![("error", DesktopNotice::error_detail(&error.message))]))
                    } else {
                        self.pending_directory = Some(PendingDirectory::new(
                            response.request,
                            Some(format!(
                                "Moved {moved} item{}; the remaining move failed: {}",
                                if moved == 1 { "" } else { "s" },
                                error_message(&error.message)
                            )),
                        ));
                        Ok(vec![directory_request(self.app, response.request, 0)])
                    }
                }
                _ => self.notice_message(DesktopNotice::key("desktop.notice.7e4ff9600eb6")),
            };
        }
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                validate_entries(entries)?;
                pending
                    .names_seen
                    .extend(entries.iter().map(|entry| entry.name.clone()));
                if let Some(collision) = pending
                    .items
                    .iter()
                    .find(|item| pending.names_seen.contains(&item.name))
                {
                    return self.notice_message(DesktopNotice::template("desktop.trash.collision",
                        vec![("name", DesktopNotice::literal(collision.name.clone()))]));
                }
                pending.entries_seen = pending.entries_seen.saturating_add(entries.len());
                if pending.entries_seen > MAX_DESKTOP_ITEMS {
                    return self.notice_message(DesktopNotice::key("desktop.notice.dc15de6e34a9"));
                }
                if let Some(cursor) = next_cursor {
                    self.pending_trash = Some(pending);
                    return Ok(vec![trash_directory_request(
                        self.app,
                        response.request,
                        *cursor,
                    )]);
                }
                let first = pending.items.first().expect("non-empty trash batch");
                let operation = VfsOperation::Rename {
                    from: first.from.clone(),
                    to: first.to.clone(),
                };
                pending.moving_index = Some(0);
                self.pending_trash = Some(pending);
                Ok(vec![Message::VfsRequest(VfsRequest {
                    request: response.request,
                    app: self.app,
                    operation,
                })])
            }
            VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => {
                self.notice_message(DesktopNotice::key("desktop.notice.6b90eaef8658"))
            }
            VfsResult::Error(error) => self.notice_message(DesktopNotice::template("desktop.trash.stopped",
                vec![("error", DesktopNotice::error_detail(&error.message))])),
            _ => self.notice_message(DesktopNotice::key("files.mutation.trash-invalid")),
        }
    }

    fn handle_mutation_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let pending = self
            .pending_mutation
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        match &response.result {
            VfsResult::Done => {
                if !self.permanent_delete_queue.is_empty() {
                    let path = self.permanent_delete_queue.remove(0);
                    self.pending_mutation = Some(pending);
                    return Ok(vec![Message::VfsRequest(VfsRequest {
                        request: response.request,
                        app: self.app,
                        operation: VfsOperation::Remove {
                            path,
                            recursive: true,
                        },
                    })]);
                }
                let completed = if self.permanent_delete_total > 0 {
                    format!(
                        "Permanently deleted {} item{}",
                        self.permanent_delete_total,
                        if self.permanent_delete_total == 1 {
                            ""
                        } else {
                            "s"
                        },
                    )
                } else {
                    format!("{} completed", pending.kind.completion_label())
                };
                self.permanent_delete_total = 0;
                self.rename_text.clear();
                self.pending_directory = Some(PendingDirectory::new(
                    response.request,
                    Some(completed.clone()),
                ));
                Ok(vec![directory_request(self.app, response.request, 0)])
            }
            VfsResult::Error(error) => self
                .notice_message(pending.kind.failure(&error.message))
                .inspect(|_| {
                    self.permanent_delete_queue.clear();
                    self.permanent_delete_total = 0;
                }),
            _ => {
                self.permanent_delete_queue.clear();
                self.permanent_delete_total = 0;
                self.notice_message(pending.kind.invalid_response())
            }
        }
    }

    fn handle_directory_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_directory
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                validate_entries(entries)?;
                if pending.entries.len().saturating_add(entries.len()) > MAX_DESKTOP_ITEMS {
                    return Err(DesktopActorError::EntryLimit);
                }
                pending.entries.extend(entries.iter().cloned());
                if let Some(cursor) = next_cursor {
                    self.pending_directory = Some(pending);
                    return Ok(vec![directory_request(self.app, response.request, *cursor)]);
                }
                let messages = self.apply_directory(pending)?;
                let mut messages = self.finish_observed_refresh(response.request, messages)?;
                if self.pending_directory.is_none() {
                    messages.extend(self.start_pending_rename()?);
                }
                // Browser-backed copies notify the Desktop watch while this
                // actor is still completing its own transfer refresh. The
                // watch is coalesced into one more directory read using the
                // same VFS request ID. Starting thumbnails from this interim
                // snapshot would register that ID as media twice when the
                // final snapshot arrives, faulting the runtime and stranding
                // Desktop input. Decode only after the last coalesced read.
                if self.pending_directory.is_none()
                    && self.pending_mutation.is_none()
                    && let Some(thumbnail) = self.reset_thumbnail_batch(response.request)
                {
                    messages.push(thumbnail);
                }
                if self.pending_directory.is_none()
                    && self.pending_mutation.is_none()
                    && let Some(link) = self.start_link_icon_resolution(response.request)
                {
                    messages.push(link);
                }
                Ok(messages)
            }
            VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => {
                let messages = self.apply_items(
                    Vec::new(),
                    &format!(
                        "Desktop folder /root/Desktop is unavailable: {}",
                        error_message(&error.message)
                    ),
                )?;
                let mut messages = self.finish_observed_refresh(response.request, messages)?;
                if self.pending_directory.is_none() {
                    messages.extend(self.start_pending_rename()?);
                }
                if self.pending_directory.is_none() {
                    self.reset_thumbnail_batch(response.request);
                }
                Ok(messages)
            }
            VfsResult::Error(error) => {
                let mut messages = self.notice_message(DesktopNotice::template("desktop.refresh.failed",
                    vec![("error", DesktopNotice::error_detail(&error.message))]))?;
                messages.extend(self.start_pending_rename()?);
                Ok(messages)
            }
            _ => {
                let mut messages =
                    self.notice_message(DesktopNotice::key("desktop.notice.b4aa5f428033"))?;
                messages.extend(self.start_pending_rename()?);
                Ok(messages)
            }
        }
    }

    fn apply_directory(
        &mut self,
        pending: PendingDirectory,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let item_count = pending.entries.len();
        let items = self.items_from_entries(pending.entries)?;
        let status = pending.completion_notice.unwrap_or_else(|| {
            if item_count == 0 {
                String::from("Desktop folder is empty")
            } else {
                format!(
                    "Showing {item_count} item{}",
                    if item_count == 1 { "" } else { "s" }
                )
            }
        });
        self.apply_items(items, &status)
    }

    fn apply_items(
        &mut self,
        mut items: Vec<DesktopItem>,
        _status: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let virtual_items = [
            (
                DesktopItemId(901),
                "Home",
                "/root",
                DesktopItemKind::Home,
                "home",
            ),
            (
                DesktopItemId(902),
                "Documents",
                "/root/Documents",
                DesktopItemKind::Documents,
                "folder",
            ),
            (
                DesktopItemId(903),
                "Pictures",
                "/root/Pictures",
                DesktopItemKind::Pictures,
                "folder",
            ),
        ];
        if self.show_builtin_icons {
            for (id, label, path, kind, icon_id) in virtual_items.into_iter().rev() {
                if !items
                    .iter()
                    .any(|item| item.label == label || item.path.as_deref() == Some(path))
                {
                    items.insert(
                        0,
                        DesktopItem {
                            id,
                            label: label.to_owned(),
                            icon_id: icon_id.to_owned(),
                            path: Some(path.to_owned()),
                            kind,
                            read_only: false,
                            executable: false,
                            available: true,
                        },
                    );
                }
            }
        }
        if self.show_builtin_icons
            && self.show_trash
            && !items
                .iter()
                .any(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
        {
            items.push(DesktopItem {
                id: DesktopItemId(900),
                label: String::from("Trash"),
                icon_id: String::from("trash"),
                path: None,
                kind: DesktopItemKind::Trash {
                    empty: !self.trash_nonempty,
                },
                read_only: false,
                executable: false,
                available: true,
            });
        }
        items.sort_by(|left, right| {
            left.label
                .to_lowercase()
                .cmp(&right.label.to_lowercase())
                .then_with(|| left.label.cmp(&right.label))
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut config = self.model.grid().config();
        let required_rows = items
            .len()
            .div_ceil(usize::from(config.columns.max(1)))
            .max(1);
        if required_rows > usize::from(config.rows) {
            config.rows =
                u16::try_from(required_rows).map_err(|_| DesktopActorError::EntryLimit)?;
            self.model.execute(DesktopCommand::ResizeGrid(config))?;
        }
        let persisted = self.positions.cells_for(self.model.workspace(), &items);
        self.model.execute(DesktopCommand::ReplaceItems {
            items: items.clone(),
            persisted,
        })?;
        if self.auto_arrange {
            self.model.execute(DesktopCommand::ArrangeIcons)?;
        }
        let rename_item = self.rename_after_refresh.take().and_then(|path| {
            items
                .iter()
                .find(|item| item.path.as_deref() == Some(path.as_str()))
                .map(|item| (item.id, item.label.clone(), path))
        });
        if let Some((item, _, _)) = rename_item.as_ref() {
            self.model.execute(DesktopCommand::Click {
                item: *item,
                mode: super::ClickMode::Replace,
            })?;
        }
        let ordered = self.model.grid().ordered_items();
        self.items = items;
        self.rendered_items = ordered;
        let mut patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        if let Some((_, label, path)) = rename_item {
            self.rename_text = label;
            self.rename_source_path = Some(path);
            patches.extend(super::switch_context_patches(
                self.context_kind,
                super::ContextKind::Rename,
                &self.rename_text,
                &self.context_action_ids,
                &[],
            ));
            self.context_kind = super::ContextKind::Rename;
            self.context_action_ids = vec![super::RENAME_FIELD, super::RENAME];
        }
        self.emit(&patches).map(|message| vec![message])
    }

    fn items_from_entries(
        &mut self,
        mut entries: Vec<DirectoryEntry>,
    ) -> Result<Vec<DesktopItem>, DesktopActorError> {
        entries.retain(|entry| !is_retired_launcher(&entry.name));
        // A non-executable .desktop file is metadata, not a usable launcher.
        // Do not expose a second raw-file icon beside the real application
        // icon; executable launchers still go through the bounded parser.
        entries.retain(|entry| {
            !entry.name.to_ascii_lowercase().ends_with(".desktop")
                || entry.metadata.mode & 0o111 != 0
        });
        if !self.show_mounts {
            entries.retain(|entry| {
                !entry.name.starts_with("ricewm-local-") || !entry.name.ends_with(".desktop")
            });
        }
        entries.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name))
        });
        let prior = self
            .items
            .iter()
            .filter_map(|item| item.path.as_ref().map(|path| (path.clone(), item.id)))
            .collect::<BTreeMap<_, _>>();
        entries
            .into_iter()
            .map(|entry| self.item_from_entry(entry, &prior))
            .collect()
    }

    fn item_from_entry(
        &mut self,
        entry: DirectoryEntry,
        prior: &BTreeMap<String, DesktopItemId>,
    ) -> Result<DesktopItem, DesktopActorError> {
        let path = format!("{DESKTOP_PATH}/{}", entry.name);
        let id = if let Some(id) = prior.get(&path) {
            *id
        } else {
            let id = DesktopItemId(self.next_item_id);
            self.next_item_id = self
                .next_item_id
                .checked_add(1)
                .ok_or(DesktopActorError::EntryLimit)?;
            id
        };
        let kind = match entry.metadata.kind {
            FileKind::Directory => DesktopItemKind::Folder,
            FileKind::File
                if entry.name.to_ascii_lowercase().ends_with(".desktop")
                    && entry.metadata.mode & 0o111 != 0 =>
            {
                DesktopItemKind::Application {
                    desktop_id: entry.name.clone(),
                }
            }
            _ => DesktopItemKind::File,
        };
        let icon_id = if entry.metadata.kind == FileKind::Symlink {
            self.link_icon_ids
                .get(&path)
                .map(String::as_str)
                .unwrap_or("link")
        } else {
            match kind {
                DesktopItemKind::Folder => "folder",
                DesktopItemKind::Application { .. } => "application",
                _ => "document",
            }
        };
        let label = if matches!(kind, DesktopItemKind::Application { .. }) {
            desktop_launcher_label(&entry.name)
        } else {
            entry.name.clone()
        };
        Ok(DesktopItem {
            id,
            label,
            icon_id: icon_id.to_owned(),
            path: Some(path),
            kind,
            read_only: entry.metadata.mode & 0o200 == 0,
            executable: entry.metadata.mode & 0o111 != 0,
            available: true,
        })
    }

    pub(super) fn selected_item(&self) -> Option<&DesktopItem> {
        let selected = self.model.selection().single()?;
        self.items.iter().find(|item| item.id == selected)
    }

    pub(super) fn unsupported_application_notice(
        &mut self,
        path: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.notice_message(DesktopNotice::template("desktop.unsupported-application",
            vec![("path", DesktopNotice::literal(path))]))
    }

    pub(super) fn notice(&mut self, text: &str) -> Result<Vec<Message>, DesktopActorError> {
        self.notice_message(DesktopNotice::literal(text))
    }

    pub(super) fn notice_message(&mut self, notice: DesktopNotice) -> Result<Vec<Message>, DesktopActorError> {
        let text = notice.render();
        self.retained_notice = Some(notice);
        let id = super::widget_id(super::NOTICE);
        let patch = if self.tree.get(id).is_some() {
            super::update_text(super::NOTICE, &text, Some(rice_ui::WidgetState::ENABLED))
        } else {
            let index = self
                .tree
                .get(super::widget_id(super::ROOT))
                .map_or(0, |root| root.children().len());
            rice_ui::Patch::Create {
                widget: rice_ui::WidgetSpec {
                    id,
                    kind: rice_ui::WidgetKind::Notification,
                    state: rice_ui::WidgetState::ENABLED,
                    text: Some(text.to_owned()),
                    icon: Some(rice_ui::IconId::Information),
                },
                parent: Some(super::widget_id(super::ROOT)),
                index,
            }
        };
        self.emit(&[patch]).map(|message| vec![message])
    }
}

fn is_retired_launcher(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "home.desktop"
            | "documents.desktop"
            | "pictures.desktop"
            | "riceplayer-web.desktop"
            | "riceplayer_web.desktop"
            | "rice-web-player.desktop"
    )
}

fn desktop_launcher_label(name: &str) -> String {
    let stem = name
        .strip_suffix(".desktop")
        .or_else(|| name.strip_suffix(".DESKTOP"))
        .unwrap_or(name);
    let stem = stem.strip_prefix("ricewm-local-").map_or(stem, |mount| {
        mount.rsplit_once('-').map_or(mount, |(label, _)| label)
    });
    let mut output = String::with_capacity(stem.len());
    let mut capitalize = true;
    for character in stem.chars() {
        if matches!(character, '-' | '_') {
            if !output.ends_with(' ') {
                output.push(' ');
            }
            capitalize = true;
        } else if capitalize {
            output.extend(character.to_uppercase());
            capitalize = false;
        } else {
            output.push(character);
        }
    }
    output.trim().to_owned()
}

pub(super) fn directory_request(app: AppId, request: RequestId, cursor: u64) -> Message {
    Message::VfsRequest(VfsRequest {
        request,
        app,
        operation: VfsOperation::ReadDirectory {
            path: DESKTOP_PATH.to_owned(),
            cursor,
            limit: PAGE_LIMIT,
        },
    })
}

fn trash_directory_request(app: AppId, request: RequestId, cursor: u64) -> Message {
    Message::VfsRequest(VfsRequest {
        request,
        app,
        operation: VfsOperation::ReadDirectory {
            path: TRASH_FILES_PATH.to_owned(),
            cursor,
            limit: PAGE_LIMIT,
        },
    })
}

fn trash_status_request(app: AppId, request: RequestId, cursor: u64) -> Message {
    Message::VfsRequest(VfsRequest {
        request,
        app,
        operation: VfsOperation::ReadDirectory {
            path: String::from(TRASH_FILES_PATH),
            cursor,
            limit: 1,
        },
    })
}
