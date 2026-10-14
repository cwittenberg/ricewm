//! Transaction-like replacement recovery over the bounded VFS request protocol.

use rice_files::GuestPath;
use rice_native_app_protocol::{
    AppId, Message, RequestId, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplacementStage {
    Preserve,
    Retry,
    RemoveBackup,
    RemovePartial,
    Restore,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReplacementRecovery {
    request: RequestId,
    original: VfsOperation,
    destination: GuestPath,
    backup: GuestPath,
    stage: ReplacementStage,
    failure: Option<VfsResult>,
    cancelling: bool,
}

pub(super) enum RecoveryAdvance {
    Request(Message, String),
    Replaced,
    Cancelled(String),
    Failed(String),
}

impl ReplacementRecovery {
    pub(super) fn new(
        request: RequestId,
        position: usize,
        original: VfsOperation,
        destination: GuestPath,
    ) -> Result<Self, rice_files::NavigationError> {
        let name = destination.file_name().unwrap_or("item");
        let backup = destination.parent().join(&format!(
            ".{name}.ricewm-backup-{}-{position}",
            request.get(),
        ))?;
        Ok(Self {
            request,
            original,
            destination,
            backup,
            stage: ReplacementStage::Preserve,
            failure: None,
            cancelling: false,
        })
    }

    pub(super) fn initial_request(&self, app: AppId) -> Message {
        self.request(
            app,
            VfsOperation::Rename {
                from: self.destination.as_str().to_owned(),
                to: self.backup.as_str().to_owned(),
            },
        )
    }

    pub(super) fn advance(
        &mut self,
        app: AppId,
        response: &VfsResponse,
        cancel_requested: bool,
    ) -> RecoveryAdvance {
        if response.request != self.request {
            return RecoveryAdvance::Failed(String::from(rice_i18n::t(
                "Replacement recovery received a mismatched VFS response",
            )));
        }
        match self.stage {
            ReplacementStage::Preserve => self.preserved(app, response, cancel_requested),
            ReplacementStage::Retry => self.retried(app, response),
            ReplacementStage::RemoveBackup => self.backup_removed(response),
            ReplacementStage::RemovePartial => self.partial_removed(app, response),
            ReplacementStage::Restore => self.restored(response),
        }
    }

    fn preserved(
        &mut self,
        app: AppId,
        response: &VfsResponse,
        cancel_requested: bool,
    ) -> RecoveryAdvance {
        if !matches!(response.result, VfsResult::Done) {
            return RecoveryAdvance::Failed(rice_i18n::format(
                "Replace stopped before changing {path}: {error}",
                &[
                    ("path", self.destination.as_str()),
                    ("error", &response_error(response)),
                ],
            ));
        }
        if cancel_requested {
            self.cancelling = true;
            self.stage = ReplacementStage::Restore;
            return RecoveryAdvance::Request(
                self.restore_request(app),
                rice_i18n::format(
                    "Restoring {path} before cancellation",
                    &[("path", self.destination.as_str())],
                ),
            );
        }
        self.stage = ReplacementStage::Retry;
        RecoveryAdvance::Request(
            self.request(app, self.original.clone()),
            rice_i18n::format(
                "Replacing {path} from its preserved backup",
                &[("path", self.destination.as_str())],
            ),
        )
    }

    fn retried(&mut self, app: AppId, response: &VfsResponse) -> RecoveryAdvance {
        if matches!(response.result, VfsResult::Done) {
            self.stage = ReplacementStage::RemoveBackup;
            return RecoveryAdvance::Request(
                self.remove_request(app, &self.backup),
                rice_i18n::format(
                    "Removing the backup for {path}",
                    &[("path", self.destination.as_str())],
                ),
            );
        }
        self.failure = Some(response.result.clone());
        self.stage = ReplacementStage::RemovePartial;
        RecoveryAdvance::Request(
            self.remove_request(app, &self.destination),
            rice_i18n::format(
                "Removing a partial replacement before restoring {path}",
                &[("path", self.destination.as_str())],
            ),
        )
    }

    fn backup_removed(&self, response: &VfsResponse) -> RecoveryAdvance {
        if done_or_missing(response) {
            RecoveryAdvance::Replaced
        } else {
            RecoveryAdvance::Failed(rice_i18n::format(
                "Replacement completed, but its backup remains at {path}: {error}",
                &[
                    ("path", self.backup.as_str()),
                    ("error", &response_error(response)),
                ],
            ))
        }
    }

    fn partial_removed(&mut self, app: AppId, response: &VfsResponse) -> RecoveryAdvance {
        if !done_or_missing(response) {
            return RecoveryAdvance::Failed(rice_i18n::format(
                "Replacement failed and its preserved original remains at {path}; the partial destination could not be removed: {error}",
                &[
                    ("path", self.backup.as_str()),
                    ("error", &response_error(response)),
                ],
            ));
        }
        self.stage = ReplacementStage::Restore;
        RecoveryAdvance::Request(
            self.restore_request(app),
            rice_i18n::format(
                "Restoring the original {path}",
                &[("path", self.destination.as_str())],
            ),
        )
    }

    fn restored(&self, response: &VfsResponse) -> RecoveryAdvance {
        if !matches!(response.result, VfsResult::Done) {
            return RecoveryAdvance::Failed(rice_i18n::format(
                "The preserved original remains at {path}; restore failed: {error}",
                &[
                    ("path", self.backup.as_str()),
                    ("error", &response_error(response)),
                ],
            ));
        }
        if self.cancelling {
            RecoveryAdvance::Cancelled(rice_i18n::format(
                "Replacement cancelled; restored {path}",
                &[("path", self.destination.as_str())],
            ))
        } else {
            RecoveryAdvance::Failed(rice_i18n::format(
                "Replacement failed for {path}: {error}; the original was restored",
                &[
                    ("path", self.destination.as_str()),
                    (
                        "error",
                        &self
                            .failure
                            .as_ref()
                            .map(result_error)
                            .unwrap_or_else(|| rice_i18n::t("VFS operation failed").to_owned()),
                    ),
                ],
            ))
        }
    }

    fn restore_request(&self, app: AppId) -> Message {
        self.request(
            app,
            VfsOperation::Rename {
                from: self.backup.as_str().to_owned(),
                to: self.destination.as_str().to_owned(),
            },
        )
    }

    fn remove_request(&self, app: AppId, path: &GuestPath) -> Message {
        self.request(
            app,
            VfsOperation::Remove {
                path: path.as_str().to_owned(),
                recursive: true,
            },
        )
    }

    fn request(&self, app: AppId, operation: VfsOperation) -> Message {
        Message::VfsRequest(VfsRequest {
            request: self.request,
            app,
            operation,
        })
    }
}

fn done_or_missing(response: &VfsResponse) -> bool {
    matches!(response.result, VfsResult::Done)
        || matches!(
            &response.result,
            VfsResult::Error(error) if error.code == VfsErrorCode::NotFound
        )
}

fn response_error(response: &VfsResponse) -> String {
    result_error(&response.result)
}

fn result_error(result: &VfsResult) -> String {
    match result {
        VfsResult::Error(error) if !error.message.trim().is_empty() => {
            let message = error.message.trim();
            let mut end = message.len().min(512);
            while !message.is_char_boundary(end) {
                end -= 1;
            }
            message[..end].to_owned()
        }
        VfsResult::Error(error) => rice_i18n::t(match error.code {
            VfsErrorCode::NotFound => "not found",
            VfsErrorCode::PermissionDenied => "permission denied",
            VfsErrorCode::AlreadyExists => "already exists",
            VfsErrorCode::InvalidInput => "invalid input",
            VfsErrorCode::NotSupported => "not supported",
            VfsErrorCode::Busy => "busy",
            VfsErrorCode::Io => "I/O error",
        })
        .to_owned(),
        _ => String::from(rice_i18n::t("invalid VFS response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rice_native_app_protocol::VfsError;

    #[test]
    fn translated_recovery_preserves_paths_and_retranslates_saved_errors() {
        let request = RequestId::new(77).unwrap();
        let app = AppId::new(1).unwrap();
        let path = "/root/{error}.txt";
        let mut recovery = ReplacementRecovery::new(
            request,
            0,
            VfsOperation::Copy {
                from: "/root/source.txt".into(),
                to: path.into(),
                recursive: false,
            },
            GuestPath::absolute(path).unwrap(),
        )
        .unwrap();
        let done = VfsResponse {
            request,
            result: VfsResult::Done,
        };
        rice_i18n::with_locale(2, || {
            let RecoveryAdvance::Request(Message::VfsRequest(next), notice) =
                recovery.advance(app, &done, false)
            else {
                panic!("retry")
            };
            assert_eq!(next.operation, recovery.original);
            assert!(notice.contains(path));
            assert!(notice.starts_with("正在从保留的备份替换"));
            let error = VfsResponse {
                request,
                result: VfsResult::Error(VfsError {
                    code: VfsErrorCode::PermissionDenied,
                    message: String::new(),
                }),
            };
            let RecoveryAdvance::Request(_, notice) = recovery.advance(app, &error, false) else {
                panic!("remove partial")
            };
            assert!(notice.contains(path));
        });
        // Switch languages before restoration: the saved error is structured,
        // so its owned category renders in the new language.
        rice_i18n::with_locale(1, || {
            let RecoveryAdvance::Request(Message::VfsRequest(next), notice) =
                recovery.advance(app, &done, false)
            else {
                panic!("restore")
            };
            assert!(notice.starts_with("Restaurando el original"));
            assert_eq!(
                next.operation,
                VfsOperation::Rename {
                    from: recovery.backup.as_str().into(),
                    to: path.into()
                }
            );
            let RecoveryAdvance::Failed(notice) = recovery.advance(app, &done, false) else {
                panic!("restored")
            };
            assert!(notice.contains(path));
            assert!(
                notice.contains(rice_i18n::t("permission denied")),
                "{notice}"
            );
            assert!(notice.ends_with("se restauró el original"));
        });
        rice_i18n::with_locale(2, || {
            let error = VfsResult::Error(VfsError {
                code: VfsErrorCode::Io,
                message: "host: {path} {error}".into(),
            });
            assert_eq!(result_error(&error), "host: {path} {error}");
            let error = VfsResult::Error(VfsError {
                code: VfsErrorCode::Io,
                message: "界".repeat(200),
            });
            let bounded = result_error(&error);
            assert!(bounded.len() <= 512);
            assert!(bounded.chars().all(|ch| ch == '界'));
        });
    }
}
