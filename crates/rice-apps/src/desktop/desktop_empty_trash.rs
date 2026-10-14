//! On-demand Trash status and confirmed bounded emptying.

use rice_native_app_protocol::{
    DirectoryEntry, Message, RequestId, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

use super::{ContextKind, DesktopActorError, DesktopItemId, RiceDesktop, switch_context_patches};

const PAGE_LIMIT: u16 = 128;
const MAX_TRASH_ITEMS: usize = 512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingTrashMenu {
    pub request: RequestId,
    pub item: DesktopItemId,
    entries: Vec<DirectoryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingEmptyTrash {
    pub request: RequestId,
    entries: Vec<DirectoryEntry>,
    position: usize,
    deleting: bool,
}

impl RiceDesktop {
    pub(super) fn begin_trash_menu(
        &mut self,
        item: DesktopItemId,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_trash_menu.is_some() || self.pending_empty_trash.is_some() {
            return self.notice("A Trash operation is already in progress");
        }
        self.pending_trash_menu = Some(PendingTrashMenu {
            request,
            item,
            entries: Vec::new(),
        });
        Ok(vec![trash_request(self.app, request, 0)])
    }

    pub(super) fn handle_trash_menu_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_trash_menu
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                pending.entries.extend(entries.iter().cloned());
                if pending.entries.len() > MAX_TRASH_ITEMS {
                    return self.notice("Trash contains too many items for the Desktop menu");
                }
                if let Some(cursor) = next_cursor {
                    self.pending_trash_menu = Some(pending);
                    return Ok(vec![trash_request(self.app, response.request, *cursor)]);
                }
                let actions = self.context_menu_entries_for_trash(!pending.entries.is_empty())?;
                let patches = switch_context_patches(
                    self.context_kind,
                    ContextKind::Item,
                    "",
                    &self.context_action_ids,
                    &actions,
                );
                self.context_kind = ContextKind::Item;
                self.context_action_ids = actions.iter().map(|entry| entry.id).collect();
                self.context_messages = actions.iter().map(|entry| (entry.id, entry.message)).collect();
                self.trash_confirm_pending = false;
                self.emit(&patches).map(|message| vec![message])
            }
            VfsResult::Error(error) => {
                self.notice(&format!("Could not inspect Trash: {}", error.message))
            }
            _ => self.notice("Could not inspect Trash: invalid VFS response"),
        }
    }

    pub(super) fn confirm_or_empty_trash(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if !self.trash_confirm_pending {
            self.trash_confirm_pending = true;
            return self
                .emit(&[super::update_text(
                    super::EMPTY_TRASH,
                    "Confirm Empty Trash (permanent)",
                    Some(rice_ui::WidgetState::ENABLED | rice_ui::WidgetState::DESTRUCTIVE),
                )])
                .map(|message| vec![message]);
        }
        self.trash_confirm_pending = false;
        self.pending_empty_trash = Some(PendingEmptyTrash {
            request,
            entries: Vec::new(),
            position: 0,
            deleting: false,
        });
        Ok(vec![trash_request(self.app, request, 0)])
    }

    pub(super) fn handle_empty_trash_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_empty_trash
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if !pending.deleting {
            return match &response.result {
                VfsResult::Directory {
                    entries,
                    next_cursor,
                } => {
                    pending.entries.extend(entries.iter().cloned());
                    if pending.entries.len() > MAX_TRASH_ITEMS {
                        return self.notice("Empty Trash stopped at its 512-item safety bound");
                    }
                    if pending
                        .entries
                        .iter()
                        .any(|entry| !super::desktop_path::valid_file_name(&entry.name))
                    {
                        return self.notice(
                            "Empty Trash stopped: the Trash directory returned an unsafe name",
                        );
                    }
                    if let Some(cursor) = next_cursor {
                        self.pending_empty_trash = Some(pending);
                        return Ok(vec![trash_request(self.app, response.request, *cursor)]);
                    }
                    if pending.entries.is_empty() {
                        return self.notice("Trash is already empty");
                    }
                    pending.deleting = true;
                    let operation = remove_operation(&pending.entries[0]);
                    self.pending_empty_trash = Some(pending);
                    Ok(vec![vfs_request(self.app, response.request, operation)])
                }
                VfsResult::Error(error) => self.notice(&format!(
                    "Empty Trash could not enumerate items: {}",
                    error.message
                )),
                _ => self.notice("Empty Trash stopped: invalid directory response"),
            };
        }
        match &response.result {
            VfsResult::Done => {
                pending.position += 1;
                if pending.position == pending.entries.len() {
                    let mut messages = self.set_trash_nonempty(false)?;
                    messages.extend(self.notice("Trash emptied")?);
                    return Ok(messages);
                }
                let operation = remove_operation(&pending.entries[pending.position]);
                self.pending_empty_trash = Some(pending);
                Ok(vec![vfs_request(self.app, response.request, operation)])
            }
            VfsResult::Error(error) => self.notice(&format!(
                "Empty Trash stopped after {} items: {}",
                pending.position, error.message
            )),
            _ => self.notice("Empty Trash stopped: invalid removal response"),
        }
    }
}

fn trash_request(app: rice_native_app_protocol::AppId, request: RequestId, cursor: u64) -> Message {
    vfs_request(
        app,
        request,
        VfsOperation::ReadDirectory {
            path: String::from(super::desktop_vfs::TRASH_FILES_PATH),
            cursor,
            limit: PAGE_LIMIT,
        },
    )
}

fn remove_operation(entry: &DirectoryEntry) -> VfsOperation {
    VfsOperation::Remove {
        path: format!("{}/{}", super::desktop_vfs::TRASH_FILES_PATH, entry.name),
        recursive: entry.metadata.kind == rice_native_app_protocol::FileKind::Directory,
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
