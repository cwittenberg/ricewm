use rice_desktop::{DesktopItem, DesktopItemKind};
use rice_files::{EntryKind, EntryMetadata, GuestPath, infer_media_type};
use rice_native_app_protocol::{
    APPLICATION_CATALOG_VERSION, DataProviderOperation, DataProviderRequest, DataProviderResponse,
    DataProviderResult, Message, RequestId,
};

use super::desktop_vfs::TRASH_FILES_PATH;
use super::{DesktopActorError, RiceDesktop};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingDesktopApplication {
    Catalog {
        request: RequestId,
        path: String,
        mime_type: String,
    },
    Launch {
        request: RequestId,
        application_id: String,
    },
}

pub(super) enum ActivationTarget {
    Launch {
        entrypoint: &'static str,
        arguments: Vec<String>,
    },
    File(String),
    DesktopEntry(String),
    Unsupported(String),
}

pub(super) fn activation_target(item: &DesktopItem) -> ActivationTarget {
    let files = |path: &str| ActivationTarget::Launch {
        entrypoint: "rice-files",
        arguments: vec![path.to_owned()],
    };
    match &item.kind {
        DesktopItemKind::Home => files("/root"),
        DesktopItemKind::Documents => files("/root/Documents"),
        DesktopItemKind::Pictures => files("/root/Pictures"),
        DesktopItemKind::Trash { .. } => files(TRASH_FILES_PATH),
        DesktopItemKind::Folder | DesktopItemKind::Mount { .. } => item
            .path
            .as_deref()
            .map_or_else(|| ActivationTarget::Unsupported(item.label.clone()), files),
        DesktopItemKind::File => item.path.as_deref().map_or_else(
            || ActivationTarget::Unsupported(item.label.clone()),
            |path| {
                if path.to_ascii_lowercase().ends_with(".desktop") {
                    ActivationTarget::DesktopEntry(path.to_owned())
                } else if item.executable && is_program_path(path) {
                    ActivationTarget::Launch {
                        entrypoint: "rice-term",
                        arguments: vec![format!("exec {}", shell_quote(path))],
                    }
                } else {
                    ActivationTarget::File(path.to_owned())
                }
            },
        ),
        DesktopItemKind::Application { .. } => item.path.as_deref().map_or_else(
            || ActivationTarget::Unsupported(item.label.clone()),
            |path| ActivationTarget::DesktopEntry(path.to_owned()),
        ),
    }
}

fn is_program_path(path: &str) -> bool {
    matches!(
        path.rsplit_once('.').map(|(_, extension)| extension),
        Some(extension) if extension.eq_ignore_ascii_case("sh")
            || extension.eq_ignore_ascii_case("bin")
            || extension.eq_ignore_ascii_case("run")
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn media_type_for_path(path: &str) -> Option<String> {
    let path = GuestPath::absolute(path).ok()?;
    let entry = EntryMetadata::new(path, EntryKind::File, 0, 0);
    Some(infer_media_type(&entry).to_owned())
}

impl RiceDesktop {
    pub(super) fn begin_file_activation(
        &mut self,
        path: String,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_application.is_some() {
            return self.notice("Another desktop file is already opening");
        }
        if path.to_ascii_lowercase().ends_with(".theme") {
            return Ok(vec![Message::Launch { app: self.app, entrypoint: "rice-files".into(),
                arguments: vec![format!("--select={path}"), "--open-with".into()] }]);
        }
        let Some(mime_type) = media_type_for_path(&path) else {
            return self.notice(&format!("Cannot open invalid desktop path {path}"));
        };
        self.pending_application = Some(PendingDesktopApplication::Catalog {
            request,
            path,
            mime_type: mime_type.clone(),
        });
        Ok(vec![Message::DataProviderRequest(DataProviderRequest {
            request,
            app: self.app,
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
        })])
    }

    pub(crate) fn handle_application_response(
        &mut self,
        response: &DataProviderResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let pending = self
            .pending_application
            .take()
            .ok_or(DesktopActorError::UnexpectedDataProviderResponse)?;
        match (pending, &response.result) {
            (
                PendingDesktopApplication::Catalog {
                    request,
                    path,
                    mime_type,
                },
                DataProviderResult::Applications {
                    schema_version,
                    mime_type: returned_type,
                    entries,
                },
            ) if request == response.request
                && *schema_version == APPLICATION_CATALOG_VERSION
                && *returned_type == mime_type =>
            {
                let application = entries
                    .iter()
                    .find(|entry| entry.is_default && entry.matches_requested_type)
                    .or_else(|| entries.iter().find(|entry| entry.matches_requested_type));
                let Some(application) = application else {
                    return Ok(vec![Message::Launch {
                        app: self.app,
                        entrypoint: "rice-files".into(),
                        arguments: vec![format!("--select={path}"), "--open-with".into()],
                    }]);
                };
                self.pending_application = Some(PendingDesktopApplication::Launch {
                    request: next_request,
                    application_id: application.id.clone(),
                });
                Ok(vec![Message::DataProviderRequest(DataProviderRequest {
                    request: next_request,
                    app: self.app,
                    operation: DataProviderOperation::ApplicationLaunch {
                        application_id: application.id.clone(),
                        mime_type,
                        paths: vec![path],
                        remember_default: false,
                    },
                })])
            }
            (
                PendingDesktopApplication::Launch {
                    request,
                    application_id,
                },
                DataProviderResult::ApplicationLaunched {
                    application_id: launched,
                    ..
                },
            ) if request == response.request && application_id == *launched => Ok(Vec::new()),
            (_, DataProviderResult::Unavailable { reason }) => self.notice(reason),
            _ => Err(DesktopActorError::UnexpectedDataProviderResponse),
        }
    }
}
