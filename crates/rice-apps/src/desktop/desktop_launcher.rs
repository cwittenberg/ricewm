//! Bounded, allowlisted `.desktop` launcher loading.

use rice_files::{AssociationRegistry, DesktopEntryError, MAX_DESKTOP_ENTRY_BYTES};
use rice_native_app_protocol::{
    FileKind, Message, RequestId, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

use super::{DesktopActorError, RiceDesktop};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingDesktopLaunch {
    pub request: RequestId,
    path: String,
    reading: bool,
}

impl RiceDesktop {
    pub(super) fn begin_desktop_launch(
        &mut self,
        path: String,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_desktop_launch.is_some()
            || self.pending_empty_trash.is_some()
            || self.pending_trash_menu.is_some()
            || self.pending_create.is_some()
            || self.pending_directory.is_some()
            || self.pending_mutation.is_some()
            || self.pending_trash.is_some()
        {
            return self.notice("Another desktop filesystem request is already in progress");
        }
        self.pending_desktop_launch = Some(PendingDesktopLaunch {
            request,
            path: path.clone(),
            reading: false,
        });
        Ok(vec![Message::VfsRequest(VfsRequest {
            request,
            app: self.app,
            operation: VfsOperation::Stat { path },
        })])
    }

    pub(super) fn handle_desktop_launch_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_desktop_launch
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if !pending.reading {
            return match &response.result {
                VfsResult::Metadata(metadata)
                    if metadata.kind == FileKind::File
                        && metadata.size <= MAX_DESKTOP_ENTRY_BYTES as u64 =>
                {
                    pending.reading = true;
                    let path = pending.path.clone();
                    self.pending_desktop_launch = Some(pending);
                    Ok(vec![Message::VfsRequest(VfsRequest {
                        request: response.request,
                        app: self.app,
                        operation: VfsOperation::Read {
                            path,
                            offset: 0,
                            length: MAX_DESKTOP_ENTRY_BYTES as u32,
                        },
                    })])
                }
                VfsResult::Metadata(_) => {
                    self.notice("The desktop launcher is not a bounded regular file")
                }
                VfsResult::Error(error) => self.notice(&format!(
                    "Could not inspect desktop launcher: {}",
                    message_or_fallback(&error.message)
                )),
                _ => self.notice("Could not inspect desktop launcher: invalid VFS response"),
            };
        }
        match &response.result {
            VfsResult::Data(data) => {
                match AssociationRegistry::first_party().launch_desktop_entry(data, &[]) {
                    Ok(intent) => Ok(vec![Message::Launch {
                        app: self.app,
                        entrypoint: intent.entrypoint.to_owned(),
                        arguments: intent.arguments,
                    }]),
                    Err(DesktopEntryError::UnregisteredExecutable)
                        if is_safe_desktop_shortcut_path(&pending.path) =>
                    {
                        // Dynamic package applications are validated against
                        // the mounted XDG registry by the browser host. Pass
                        // only the descriptor identity; never pass or execute
                        // its Exec command from this actor.
                        Ok(vec![Message::Launch {
                            app: self.app,
                            entrypoint: "rice-desktop-entry".to_owned(),
                            arguments: vec![pending.path],
                        }])
                    }
                    Err(error) => self.notice(&format!("Could not launch shortcut: {error}")),
                }
            }
            VfsResult::Error(error) => self.notice(&format!(
                "Could not read desktop launcher: {}",
                message_or_fallback(&error.message)
            )),
            _ => self.notice("Could not read desktop launcher: invalid VFS response"),
        }
    }
}

fn is_safe_desktop_shortcut_path(path: &str) -> bool {
    let Some(name) = path.strip_prefix("/root/Desktop/") else {
        return false;
    };
    name.len() >= ".desktop".len() + 1
        && name.len() <= 263
        && name.ends_with(".desktop")
        && !name.contains('/')
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'+' | b'-')
        })
}

fn message_or_fallback(message: &str) -> &str {
    let message = message.trim();
    if message.is_empty() {
        "VFS operation failed"
    } else {
        message
    }
}
