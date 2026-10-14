//! Collision-safe desktop item creation.

use rice_native_app_protocol::{
    Message, RequestId, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

use super::{DesktopActorError, RiceDesktop};

const MAX_CREATE_ATTEMPTS: u16 = 999;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingCreate {
    pub request: RequestId,
    path: String,
    folder: bool,
    creating: bool,
    attempt: u16,
}

impl RiceDesktop {
    pub(super) fn begin_new_folder(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.begin_create(request, "New Folder", true)
    }

    pub(super) fn begin_new_file(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.begin_create(request, "new file.txt", false)
    }

    fn begin_create(
        &mut self,
        request: RequestId,
        name: &str,
        folder: bool,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_create.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
            || self.pending_desktop_launch.is_some()
            || self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
        {
            return self.notice("Another desktop filesystem request is already in progress");
        }
        let path = format!("{}/{name}", super::desktop_vfs::DESKTOP_PATH);
        self.pending_create = Some(PendingCreate {
            request,
            path: path.clone(),
            folder,
            creating: false,
            attempt: 1,
        });
        Ok(vec![request_message(
            self.app,
            request,
            VfsOperation::Stat { path },
        )])
    }

    pub(super) fn handle_create_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_create
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if !pending.creating {
            return match &response.result {
                VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => {
                    pending.creating = true;
                    let operation = if pending.folder {
                        VfsOperation::CreateDirectory {
                            path: pending.path.clone(),
                        }
                    } else {
                        VfsOperation::Write {
                            path: pending.path.clone(),
                            offset: 0,
                            data: Vec::new(),
                        }
                    };
                    self.pending_create = Some(pending);
                    Ok(vec![request_message(self.app, response.request, operation)])
                }
                VfsResult::Metadata(_) if pending.attempt < MAX_CREATE_ATTEMPTS => {
                    pending.attempt += 1;
                    let name = if pending.folder {
                        format!("New Folder ({})", pending.attempt)
                    } else {
                        format!("new file ({}).txt", pending.attempt)
                    };
                    pending.path = format!("{}/{name}", super::desktop_vfs::DESKTOP_PATH);
                    let path = pending.path.clone();
                    self.pending_create = Some(pending);
                    Ok(vec![request_message(
                        self.app,
                        response.request,
                        VfsOperation::Stat { path },
                    )])
                }
                VfsResult::Metadata(_) => {
                    self.notice("Could not find an available name for the new desktop item")
                }
                VfsResult::Error(error) => self.notice(&format!(
                    "Could not check the new item name: {}",
                    error.message
                )),
                _ => self.notice("Could not check the new item name: invalid VFS response"),
            };
        }
        match &response.result {
            VfsResult::Done | VfsResult::Written(_) => {
                self.rename_after_refresh = Some(pending.path);
                self.begin_refresh(response.request, Some(String::from("Desktop item created")))
            }
            VfsResult::Error(error) => {
                self.notice(&format!("Could not create desktop item: {}", error.message))
            }
            _ => self.notice("Could not create desktop item: invalid VFS response"),
        }
    }
}

fn request_message(
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
