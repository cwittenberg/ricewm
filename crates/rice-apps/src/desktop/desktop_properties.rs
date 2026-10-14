//! Real, VFS-backed properties dialog for desktop items.

use rice_native_app_protocol::{
    ClipboardRequest, ClipboardSelection, FileKind, FileMetadata, Message, RequestId, VfsError,
    VfsOperation, VfsRequest, VfsResponse, VfsResult,
};
use rice_ui::{IconId, Patch, ValueChange, WidgetKind, WidgetSpec, WidgetState};

use super::{
    DesktopActorError, DesktopItem, DesktopItemKind, PROPERTIES_CLOSE, PROPERTIES_DIALOG, ROOT,
    RiceDesktop, desktop_clipboard::PendingClipboard, desktop_path::valid_file_name, widget_id,
};
use crate::time_format::modified_timestamp_label;

const SUMMARY: u64 = 62;
const DETAILS: u64 = 63;
const ACTIONS: u64 = 64;
const ROW_BASE: u64 = 70;
pub(super) const COPY_LOCATION: u64 = 65;
pub(super) const MODE_FIELD: u64 = 66;
pub(super) const APPLY_MODE: u64 = 67;
pub(super) const EXECUTABLE: u64 = 68;
const PERMISSIONS: u64 = 69;
const SUMMARY_NAME: u64 = 200;
const ACCESS_ROW: u64 = 201;
const ACCESS_LABEL: u64 = 202;
const ACCESS_MENU_BAR: u64 = 203;
pub(super) const ACCESS_MENU: u64 = 204;
const MODE_ROW: u64 = 205;
const FEEDBACK: u64 = 206;
const COPY_LOCATION_TOOLTIP: u64 = 207;
pub(super) const ACCESS_READ_WRITE: u64 = 210;
pub(super) const ACCESS_READ_ONLY: u64 = 211;
pub(super) const ACCESS_WRITE_ONLY: u64 = 212;
pub(super) const ACCESS_NONE: u64 = 213;
pub(super) const LINK_TARGET_FIELD: u64 = 214;
pub(super) const APPLY_LINK_TARGET: u64 = 215;
const LINK_TARGET_ROW: u64 = 216;
pub(super) const NAME_FIELD: u64 = 217;
pub(super) const APPLY_NAME: u64 = 218;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DesktopProperties {
    item: DesktopItem,
    request: Option<RequestId>,
    result: PropertyResult,
    metadata: Option<FileMetadata>,
    name_input: String,
    requested_name: Option<String>,
    requested_path: Option<String>,
    mode_input: String,
    requested_mode: Option<u32>,
    link_target: String,
    requested_link_target: Option<String>,
    feedback: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PropertyResult {
    Loading,
    LoadingLink,
    Ready,
    Applying,
    Verifying,
    ApplyingLink,
    VerifyingLink,
    ApplyingName,
    VerifyingName,
    Error(String),
    Virtual,
}

impl DesktopProperties {
    pub(super) fn awaits(&self, request: RequestId) -> bool {
        self.request == Some(request)
    }
}

impl RiceDesktop {
    pub(super) fn show_properties(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(item) = self.selected_item().cloned() else {
            return self.notice("Select one desktop item to inspect");
        };
        let operation = item
            .path
            .as_ref()
            .map(|path| VfsOperation::Stat { path: path.clone() });
        let name_input = item.label.clone();
        self.pending_properties = Some(DesktopProperties {
            item,
            request: operation.as_ref().map(|_| request),
            result: if operation.is_some() {
                PropertyResult::Loading
            } else {
                PropertyResult::Virtual
            },
            metadata: None,
            name_input,
            requested_name: None,
            requested_path: None,
            mode_input: String::new(),
            requested_mode: None,
            link_target: String::new(),
            requested_link_target: None,
            feedback: None,
        });
        let ui = self.replace_properties_dialog()?;
        let Some(operation) = operation else {
            return Ok(vec![ui]);
        };
        Ok(vec![
            ui,
            Message::VfsRequest(VfsRequest {
                request,
                app: self.app,
                operation,
            }),
        ])
    }

    pub(super) fn close_properties(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_properties.take().is_none() {
            return Ok(Vec::new());
        }
        let mut patches = vec![Patch::Remove {
            id: widget_id(PROPERTIES_DIALOG),
        }];
        patches.extend(super::replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        ));
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn handle_properties_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let state = self
            .pending_properties
            .as_mut()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if state.request != Some(response.request) {
            return Err(DesktopActorError::UnexpectedVfsResponse);
        }
        let mut follow_up = None;
        let mut refresh_link_icon = None;
        match state.result {
            PropertyResult::Loading => match &response.result {
                VfsResult::Metadata(metadata) => {
                    update_metadata(state, metadata);
                    if metadata.kind == FileKind::Symlink {
                        state.result = PropertyResult::LoadingLink;
                        follow_up = state
                            .item
                            .path
                            .as_ref()
                            .map(|path| VfsOperation::ReadLink { path: path.clone() });
                    } else {
                        state.result = PropertyResult::Ready;
                        state.request = None;
                    }
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Error(vfs_error(error));
                    state.request = None;
                }
                _ => {
                    state.result = PropertyResult::Error(String::from(
                        "The filesystem returned an invalid metadata response.",
                    ));
                    state.request = None;
                }
            },
            PropertyResult::LoadingLink => match &response.result {
                VfsResult::Data(data) => match String::from_utf8(data.clone()) {
                    Ok(target) => {
                        state.link_target = target;
                        state.result = PropertyResult::Ready;
                        state.request = None;
                    }
                    Err(_) => {
                        state.result = PropertyResult::Error(String::from(
                            "The symbolic-link target is not valid UTF-8.",
                        ));
                        state.request = None;
                    }
                },
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Error(vfs_error(error));
                    state.request = None;
                }
                _ => {
                    state.result = PropertyResult::Error(String::from(
                        "The filesystem returned an invalid link-target response.",
                    ));
                    state.request = None;
                }
            },
            PropertyResult::Applying => match &response.result {
                VfsResult::Done => {
                    state.result = PropertyResult::Verifying;
                    state.feedback = Some(String::from("Verifying permissions…"));
                    follow_up = state
                        .item
                        .path
                        .as_ref()
                        .map(|path| VfsOperation::Stat { path: path.clone() });
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_mode = None;
                    state.feedback = Some(format!(
                        "Permissions were not changed: {}",
                        vfs_error(error)
                    ));
                }
                _ => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_mode = None;
                    state.feedback = Some(String::from(
                        "Permissions were not changed: invalid filesystem response",
                    ));
                }
            },
            PropertyResult::Verifying => match &response.result {
                VfsResult::Metadata(metadata) => {
                    let requested = state.requested_mode.take();
                    update_metadata(state, metadata);
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.feedback = requested.map(|mode| {
                        if metadata.mode & 0o7777 == mode {
                            format!("Permissions changed to {mode:04o}")
                        } else {
                            format!(
                                "Verification failed: requested {mode:04o}, host reported {:04o}",
                                metadata.mode & 0o7777
                            )
                        }
                    });
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_mode = None;
                    state.feedback = Some(format!(
                        "The change was accepted, but verification failed: {}",
                        vfs_error(error)
                    ));
                }
                _ => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_mode = None;
                    state.feedback = Some(String::from(
                        "The change was accepted, but verification returned an invalid response",
                    ));
                }
            },
            PropertyResult::ApplyingLink => match &response.result {
                VfsResult::Done => {
                    state.result = PropertyResult::VerifyingLink;
                    state.feedback = Some(String::from("Verifying link target…"));
                    follow_up = state
                        .item
                        .path
                        .as_ref()
                        .map(|path| VfsOperation::ReadLink { path: path.clone() });
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_link_target = None;
                    state.feedback =
                        Some(format!("Link target was not changed: {}", vfs_error(error)));
                }
                _ => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_link_target = None;
                    state.feedback = Some(String::from(
                        "Link target was not changed: invalid filesystem response",
                    ));
                }
            },
            PropertyResult::VerifyingLink => match &response.result {
                VfsResult::Data(data) => match String::from_utf8(data.clone()) {
                    Ok(target) => {
                        let requested = state.requested_link_target.take();
                        state.link_target = target.clone();
                        state.result = PropertyResult::Ready;
                        state.request = None;
                        state.feedback = Some(if requested.as_deref() == Some(target.as_str()) {
                            String::from("Link target changed")
                        } else {
                            String::from("Link target verification failed")
                        });
                        refresh_link_icon = state.item.path.clone();
                    }
                    Err(_) => {
                        state.result = PropertyResult::Error(String::from(
                            "The verified link target is not valid UTF-8.",
                        ));
                        state.request = None;
                    }
                },
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Error(format!(
                        "The target change was accepted, but verification failed: {}",
                        vfs_error(error)
                    ));
                    state.request = None;
                }
                _ => {
                    state.result = PropertyResult::Error(String::from(
                        "The target change was accepted, but verification was invalid.",
                    ));
                    state.request = None;
                }
            },
            PropertyResult::ApplyingName => match &response.result {
                VfsResult::Done => {
                    let Some(path) = state.requested_path.clone() else {
                        state.result = PropertyResult::Error(String::from(
                            "The rename lost its verified destination path.",
                        ));
                        state.request = None;
                        return self
                            .replace_properties_dialog()
                            .map(|message| vec![message]);
                    };
                    state.result = PropertyResult::VerifyingName;
                    state.feedback = Some(String::from("Verifying renamed item…"));
                    follow_up = Some(VfsOperation::Stat { path });
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_name = None;
                    state.requested_path = None;
                    state.feedback = Some(format!("Name was not changed: {}", vfs_error(error)));
                }
                _ => {
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.requested_name = None;
                    state.requested_path = None;
                    state.feedback = Some(String::from(
                        "Name was not changed: invalid filesystem response",
                    ));
                }
            },
            PropertyResult::VerifyingName => match &response.result {
                VfsResult::Metadata(metadata) => {
                    let name = state
                        .requested_name
                        .take()
                        .unwrap_or_else(|| state.name_input.clone());
                    let path = state
                        .requested_path
                        .take()
                        .unwrap_or_else(|| state.item.path.clone().unwrap_or_default());
                    state.item.label.clone_from(&name);
                    state.item.path = Some(path);
                    state.name_input = name;
                    update_metadata(state, metadata);
                    state.result = PropertyResult::Ready;
                    state.request = None;
                    state.feedback = Some(String::from("Name changed"));
                }
                VfsResult::Error(error) => {
                    state.result = PropertyResult::Error(format!(
                        "The rename completed, but verification failed: {}",
                        vfs_error(error)
                    ));
                    state.request = None;
                }
                _ => {
                    state.result = PropertyResult::Error(String::from(
                        "The rename completed, but verification was invalid.",
                    ));
                    state.request = None;
                }
            },
            PropertyResult::Ready | PropertyResult::Error(_) | PropertyResult::Virtual => {}
        }
        let verified_item =
            matches!(response.result, VfsResult::Metadata(_)).then(|| state.item.clone());
        if let Some(item) = verified_item {
            if let Some(current) = self.items.iter_mut().find(|current| current.id == item.id) {
                current.label.clone_from(&item.label);
                current.path.clone_from(&item.path);
                current.read_only = item.read_only;
                current.executable = item.executable;
            }
            if let Some(current) = self.model.item_mut(item.id) {
                current.label.clone_from(&item.label);
                current.path.clone_from(&item.path);
                current.read_only = item.read_only;
                current.executable = item.executable;
            }
        }
        let ui = self.replace_properties_dialog()?;
        let mut messages = match follow_up {
            Some(operation) => vec![
                ui,
                Message::VfsRequest(VfsRequest {
                    request: response.request,
                    app: self.app,
                    operation,
                }),
            ],
            None => vec![ui],
        };
        if let Some(path) = refresh_link_icon
            && let Some(request) = self.refresh_link_icon(path, response.request)
        {
            messages.push(request);
        }
        Ok(messages)
    }

    pub(super) fn copy_property_location(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.pending_clipboard.is_some() {
            return self.notice("A clipboard operation is already in progress");
        }
        let path = self
            .pending_properties
            .as_ref()
            .and_then(|state| state.item.path.clone());
        let Some(path) = path else {
            return Ok(Vec::new());
        };
        self.pending_clipboard = Some(PendingClipboard::Location { request });
        Ok(vec![Message::ClipboardRequest(ClipboardRequest::Write {
            request,
            selection: ClipboardSelection::Clipboard,
            mime_type: String::from("text/plain;charset=utf-8"),
            data: path.into_bytes(),
        })])
    }

    pub(super) fn update_property_mode(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state.result != PropertyResult::Ready || state.metadata.is_none() {
            return Ok(Vec::new());
        }
        state.mode_input = value.trim().to_owned();
        state.feedback = None;
        let field_state = WidgetState::ENABLED
            | WidgetState::FOCUSED
            | if parse_mode(&state.mode_input).is_err() {
                WidgetState::INVALID
            } else {
                WidgetState::empty()
            };
        let apply_state = if apply_enabled(state) {
            WidgetState::ENABLED | WidgetState::ACCENT
        } else {
            WidgetState::empty()
        };
        let value = state.mode_input.clone();
        self.update_property_editor(MODE_FIELD, value, field_state, APPLY_MODE, apply_state)
    }

    pub(super) fn update_property_link_target(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state.result != PropertyResult::Ready
            || state.metadata.as_ref().map(|metadata| metadata.kind) != Some(FileKind::Symlink)
        {
            return Ok(Vec::new());
        }
        state.link_target = value.to_owned();
        state.feedback = validate_link_target(value).err().map(str::to_owned);
        let apply_state = if validate_link_target(&state.link_target).is_ok() {
            WidgetState::ENABLED
        } else {
            WidgetState::empty()
        };
        let value = state.link_target.clone();
        self.update_property_editor(
            LINK_TARGET_FIELD,
            value,
            WidgetState::ENABLED | WidgetState::FOCUSED,
            APPLY_LINK_TARGET,
            apply_state,
        )
    }

    pub(super) fn update_property_name(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state.result != PropertyResult::Ready || !state.item.is_renameable() {
            return Ok(Vec::new());
        }
        state.name_input = value.to_owned();
        state.feedback = (!valid_file_name(value.trim()))
            .then(|| String::from("Enter a valid file name without '/'"));
        let apply_state = if valid_file_name(state.name_input.trim())
            && state.name_input.trim() != state.item.label
        {
            WidgetState::ENABLED
        } else {
            WidgetState::empty()
        };
        let value = state.name_input.clone();
        self.update_property_editor(
            NAME_FIELD,
            value,
            WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::EMPHASIZED,
            APPLY_NAME,
            apply_state,
        )
    }

    fn update_property_editor(
        &mut self,
        field: u64,
        value: String,
        field_state: WidgetState,
        action: u64,
        action_state: WidgetState,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut patches = vec![
            Patch::Update {
                id: widget_id(field),
                state: Some(field_state),
                text: ValueChange::Set(value),
                icon: ValueChange::Keep,
            },
            Patch::Update {
                id: widget_id(action),
                state: Some(action_state),
                text: ValueChange::Keep,
                icon: ValueChange::Keep,
            },
        ];
        if self.tree.get(widget_id(FEEDBACK)).is_some() {
            patches.push(Patch::Remove {
                id: widget_id(FEEDBACK),
            });
        }
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn apply_property_name(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let app = self.app;
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state.result != PropertyResult::Ready || !state.item.is_renameable() {
            return Ok(Vec::new());
        }
        let name = state.name_input.trim().to_owned();
        if !valid_file_name(&name) {
            state.feedback = Some(String::from("Enter a valid file name without '/'"));
            return self
                .replace_properties_dialog()
                .map(|message| vec![message]);
        }
        let Some(from) = state.item.path.clone() else {
            return Ok(Vec::new());
        };
        let parent = from.rsplit_once('/').map_or("", |(parent, _)| parent);
        let to = format!("{parent}/{name}");
        if from == to {
            state.feedback = Some(String::from("The item already has this name"));
            return self
                .replace_properties_dialog()
                .map(|message| vec![message]);
        }
        state.result = PropertyResult::ApplyingName;
        state.request = Some(request);
        state.requested_name = Some(name);
        state.requested_path = Some(to.clone());
        state.feedback = Some(String::from("Changing name…"));
        Ok(vec![
            self.replace_properties_dialog()?,
            Message::VfsRequest(VfsRequest {
                request,
                app,
                operation: VfsOperation::Rename { from, to },
            }),
        ])
    }

    pub(super) fn apply_property_link_target(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let app = self.app;
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state.result != PropertyResult::Ready
            || state.metadata.as_ref().map(|metadata| metadata.kind) != Some(FileKind::Symlink)
        {
            return Ok(Vec::new());
        }
        let target = state.link_target.trim().to_owned();
        if let Err(error) = validate_link_target(&target) {
            state.feedback = Some(error.to_owned());
            return self
                .replace_properties_dialog()
                .map(|message| vec![message]);
        }
        let Some(path) = state.item.path.clone() else {
            return Ok(Vec::new());
        };
        state.result = PropertyResult::ApplyingLink;
        state.request = Some(request);
        state.requested_link_target = Some(target.clone());
        state.feedback = Some(String::from("Changing link target…"));
        Ok(vec![
            self.replace_properties_dialog()?,
            Message::VfsRequest(VfsRequest {
                request,
                app,
                operation: VfsOperation::ReplaceLink { path, target },
            }),
        ])
    }

    pub(super) fn choose_property_access(
        &mut self,
        owner_read_write: u32,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if !can_edit_mode(state) {
            return Ok(Vec::new());
        }
        let Ok(mode) = parse_mode(&state.mode_input) else {
            state.feedback = Some(String::from(
                "Correct the octal permissions before choosing access",
            ));
            return self
                .replace_properties_dialog()
                .map(|message| vec![message]);
        };
        state.mode_input = format!(
            "{:04o}",
            (mode & !0o700) | owner_read_write | (mode & 0o100)
        );
        state.feedback = None;
        self.replace_properties_dialog()
            .map(|message| vec![message])
    }

    pub(super) fn apply_property_mode(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mode = match self.pending_properties.as_ref() {
            Some(state) if can_edit_mode(state) => match parse_mode(&state.mode_input) {
                Ok(mode) => mode,
                Err(error) => {
                    let state = self.pending_properties.as_mut().expect("property state");
                    state.feedback = Some(error.to_owned());
                    return self
                        .replace_properties_dialog()
                        .map(|message| vec![message]);
                }
            },
            _ => return Ok(Vec::new()),
        };
        self.begin_mode_change(request, mode)
    }

    pub(super) fn toggle_property_executable(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(state) = self.pending_properties.as_ref() else {
            return Ok(Vec::new());
        };
        if !can_edit_mode(state) || !shows_executable(state) {
            return Ok(Vec::new());
        }
        let mode = parse_mode(&state.mode_input).unwrap_or_else(|_| {
            state
                .metadata
                .as_ref()
                .map_or(0, |metadata| metadata.mode & 0o7777)
        });
        self.begin_mode_change(
            request,
            if mode & 0o111 == 0 {
                mode | 0o111
            } else {
                mode & !0o111
            },
        )
    }

    fn begin_mode_change(
        &mut self,
        request: RequestId,
        mode: u32,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let app = self.app;
        let Some(state) = self.pending_properties.as_mut() else {
            return Ok(Vec::new());
        };
        if state
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.mode & 0o7777 == mode)
        {
            return Ok(Vec::new());
        }
        let Some(path) = state.item.path.clone() else {
            return Ok(Vec::new());
        };
        state.result = PropertyResult::Applying;
        state.request = Some(request);
        state.requested_mode = Some(mode);
        state.mode_input = format!("{mode:04o}");
        state.feedback = Some(String::from("Applying permissions…"));
        Ok(vec![
            self.replace_properties_dialog()?,
            Message::VfsRequest(VfsRequest {
                request,
                app,
                operation: VfsOperation::SetMode { path, mode },
            }),
        ])
    }

    fn replace_properties_dialog(&mut self) -> Result<Message, DesktopActorError> {
        let Some(state) = self.pending_properties.as_ref() else {
            return Err(DesktopActorError::UnexpectedVfsResponse);
        };
        let existed = self.tree.get(widget_id(PROPERTIES_DIALOG)).is_some();
        let mut patches = Vec::new();
        if existed {
            patches.push(Patch::Remove {
                id: widget_id(PROPERTIES_DIALOG),
            });
        }
        create(
            &mut patches,
            PROPERTIES_DIALOG,
            Some(ROOT),
            WidgetKind::Dialog,
            Some(rice_i18n::t("Properties")),
            Some(IconId::Information),
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            SUMMARY,
            Some(PROPERTIES_DIALOG),
            WidgetKind::Row,
            Some("panel:actions"),
            Some(icon_for(&state.item)),
            WidgetState::ENABLED | WidgetState::EMPHASIZED,
        );
        create(
            &mut patches,
            if state.item.is_renameable() {
                NAME_FIELD
            } else {
                SUMMARY_NAME
            },
            Some(SUMMARY),
            if state.item.is_renameable() {
                WidgetKind::TextField
            } else {
                WidgetKind::Label
            },
            Some(if state.item.is_renameable() {
                &state.name_input
            } else {
                &state.item.label
            }),
            None,
            if state.result == PropertyResult::Ready {
                WidgetState::ENABLED | WidgetState::EMPHASIZED
            } else {
                WidgetState::BUSY
            },
        );
        if state.item.is_renameable() {
            create(
                &mut patches,
                APPLY_NAME,
                Some(SUMMARY),
                WidgetKind::Button,
                Some(rice_i18n::t("Rename")),
                Some(IconId::EditRedo),
                if state.result == PropertyResult::Ready
                    && valid_file_name(state.name_input.trim())
                    && state.name_input.trim() != state.item.label
                {
                    WidgetState::ENABLED
                } else {
                    WidgetState::empty()
                },
            );
        }
        if state.item.path.is_some() {
            create(
                &mut patches,
                COPY_LOCATION,
                Some(SUMMARY),
                WidgetKind::Button,
                Some(""),
                Some(IconId::EditCopy),
                WidgetState::ENABLED | WidgetState::FLAT,
            );
            create(
                &mut patches,
                COPY_LOCATION_TOOLTIP,
                Some(COPY_LOCATION),
                WidgetKind::Tooltip,
                Some(rice_i18n::t("Copy location")),
                None,
                WidgetState::ENABLED,
            );
        }
        create(
            &mut patches,
            DETAILS,
            Some(PROPERTIES_DIALOG),
            WidgetKind::Column,
            Some(rice_i18n::t("File information")),
            None,
            WidgetState::ENABLED,
        );
        for (index, (label, value)) in property_rows(state).into_iter().enumerate() {
            let row = ROW_BASE + u64::try_from(index).unwrap_or_default() * 3;
            create(
                &mut patches,
                row,
                Some(DETAILS),
                WidgetKind::Row,
                Some("split:300"),
                None,
                WidgetState::ENABLED,
            );
            create(
                &mut patches,
                row + 1,
                Some(row),
                WidgetKind::Label,
                Some(label),
                None,
                WidgetState::ENABLED | WidgetState::MUTED,
            );
            create(
                &mut patches,
                row + 2,
                Some(row),
                WidgetKind::Label,
                Some(&value),
                None,
                WidgetState::ENABLED,
            );
        }
        link_target_patches(&mut patches, state);
        permission_patches(&mut patches, state);
        if let Some(feedback) = validation_feedback(state).or(state.feedback.as_deref()) {
            create(
                &mut patches,
                FEEDBACK,
                Some(PROPERTIES_DIALOG),
                WidgetKind::Label,
                Some(feedback),
                Some(if feedback.starts_with("Permissions changed") {
                    IconId::Information
                } else {
                    IconId::Warning
                }),
                WidgetState::ENABLED,
            );
        }
        create(
            &mut patches,
            ACTIONS,
            Some(PROPERTIES_DIALOG),
            WidgetKind::Row,
            Some("panel:actions"),
            None,
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            PROPERTIES_CLOSE,
            Some(ACTIONS),
            WidgetKind::Button,
            Some(rice_i18n::t("Close")),
            Some(IconId::Close),
            WidgetState::ENABLED,
        );
        self.emit(&patches)
    }
}

fn property_rows(state: &DesktopProperties) -> Vec<(&'static str, String)> {
    let path = state
        .item
        .path
        .clone()
        .unwrap_or_else(|| String::from("Virtual desktop item"));
    let mut rows = vec![("Location", path)];
    match &state.result {
        PropertyResult::Loading => rows.push(("Status", String::from("Reading metadata…"))),
        PropertyResult::LoadingLink => {
            rows.push(("Status", String::from("Reading link target…")));
        }
        PropertyResult::Error(error) => rows.push(("Error", error.clone())),
        PropertyResult::Virtual => {
            rows.push(("Type", desktop_kind(&state.item).to_owned()));
            rows.push((
                "Access",
                if state.item.read_only {
                    String::from("Read-only")
                } else {
                    String::from("Writable")
                },
            ));
        }
        PropertyResult::Ready
        | PropertyResult::Applying
        | PropertyResult::Verifying
        | PropertyResult::ApplyingLink
        | PropertyResult::VerifyingLink
        | PropertyResult::ApplyingName
        | PropertyResult::VerifyingName => {
            let Some(metadata) = state.metadata.as_ref() else {
                return rows;
            };
            rows.push(("Type", file_kind(metadata.kind).to_owned()));
            rows.push(("Size", format_size(metadata.size)));
            rows.push((
                "Modified",
                modified_timestamp_label(metadata.modified_millis),
            ));
        }
    }
    rows
}

fn link_target_patches(patches: &mut Vec<Patch>, state: &DesktopProperties) {
    if state.metadata.as_ref().map(|metadata| metadata.kind) != Some(FileKind::Symlink) {
        return;
    }
    create(
        patches,
        LINK_TARGET_ROW,
        Some(PROPERTIES_DIALOG),
        WidgetKind::Row,
        Some("split:300"),
        None,
        WidgetState::ENABLED,
    );
    create(
        patches,
        LINK_TARGET_FIELD,
        Some(LINK_TARGET_ROW),
        WidgetKind::TextField,
        Some(&state.link_target),
        Some(super::desktop_view::icon_for_item(&state.item)),
        if state.result == PropertyResult::Ready {
            WidgetState::ENABLED
        } else {
            WidgetState::BUSY
        },
    );
    create(
        patches,
        APPLY_LINK_TARGET,
        Some(LINK_TARGET_ROW),
        WidgetKind::Button,
        Some(rice_i18n::t("Change Target")),
        Some(IconId::FolderOpen),
        if state.result == PropertyResult::Ready && validate_link_target(&state.link_target).is_ok()
        {
            WidgetState::ENABLED
        } else {
            WidgetState::empty()
        },
    );
}

fn validate_link_target(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    if !value.starts_with('/') {
        return Err("Link target must be an absolute RiceWM path");
    }
    if value.len() > 4_096 || value.bytes().any(|byte| byte == 0 || byte < 0x20) {
        return Err("Link target contains invalid characters");
    }
    Ok(())
}

fn permission_patches(patches: &mut Vec<Patch>, state: &DesktopProperties) {
    let Some(metadata) = state.metadata.as_ref() else {
        return;
    };
    create(
        patches,
        PERMISSIONS,
        Some(PROPERTIES_DIALOG),
        WidgetKind::Column,
        Some(&format!(
            "Permissions  ·  {}",
            symbolic_mode(metadata.mode, metadata.kind)
        )),
        None,
        WidgetState::ENABLED,
    );
    if metadata.kind == FileKind::Symlink {
        create(
            patches,
            MODE_FIELD,
            Some(PERMISSIONS),
            WidgetKind::Label,
            Some(&format!(
                "{:04o} (read-only symbolic link)",
                metadata.mode & 0o7777
            )),
            None,
            WidgetState::ENABLED | WidgetState::MUTED,
        );
        return;
    }
    let current_mode = parse_mode(&state.mode_input).unwrap_or(metadata.mode & 0o7777);
    create(
        patches,
        ACCESS_ROW,
        Some(PERMISSIONS),
        WidgetKind::Row,
        Some("split:300"),
        None,
        WidgetState::ENABLED,
    );
    create(
        patches,
        ACCESS_LABEL,
        Some(ACCESS_ROW),
        WidgetKind::Label,
        Some(rice_i18n::t("Access")),
        None,
        WidgetState::ENABLED | WidgetState::MUTED,
    );
    create(
        patches,
        ACCESS_MENU_BAR,
        Some(ACCESS_ROW),
        WidgetKind::MenuBar,
        None,
        None,
        WidgetState::ENABLED,
    );
    create(
        patches,
        ACCESS_MENU,
        Some(ACCESS_MENU_BAR),
        WidgetKind::Menu,
        Some(owner_access_label(current_mode)),
        Some(IconId::ChevronDown),
        if state.result == PropertyResult::Ready {
            WidgetState::ENABLED | WidgetState::TRAILING_ICON
        } else {
            WidgetState::empty()
        },
    );
    for (id, label, bits) in ACCESS_CHOICES {
        create(
            patches,
            id,
            Some(ACCESS_MENU),
            WidgetKind::RadioButton,
            Some(label),
            None,
            WidgetState::ENABLED
                | if current_mode & 0o600 == bits {
                    WidgetState::CHECKED
                } else {
                    WidgetState::empty()
                },
        );
    }
    create(
        patches,
        MODE_ROW,
        Some(PERMISSIONS),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
        WidgetState::ENABLED,
    );
    let mut field_state = if state.result == PropertyResult::Ready {
        WidgetState::ENABLED
    } else {
        WidgetState::BUSY
    };
    if state.result == PropertyResult::Ready && parse_mode(&state.mode_input).is_err() {
        field_state = field_state | WidgetState::INVALID;
    }
    create(
        patches,
        MODE_FIELD,
        Some(MODE_ROW),
        WidgetKind::TextField,
        Some(&state.mode_input),
        None,
        field_state,
    );
    create(
        patches,
        APPLY_MODE,
        Some(MODE_ROW),
        WidgetKind::Button,
        Some(rice_i18n::t("Apply")),
        None,
        if apply_enabled(state) {
            WidgetState::ENABLED | WidgetState::ACCENT
        } else {
            WidgetState::empty()
        },
    );
    if shows_executable(state) {
        create(
            patches,
            EXECUTABLE,
            Some(PERMISSIONS),
            WidgetKind::Toggle,
            Some(rice_i18n::t("Executable as Program")),
            Some(IconId::Program),
            if state.result == PropertyResult::Ready {
                WidgetState::ENABLED
                    | if metadata.mode & 0o111 != 0 {
                        WidgetState::CHECKED
                    } else {
                        WidgetState::empty()
                    }
            } else {
                WidgetState::BUSY
            },
        );
    }
}

const ACCESS_CHOICES: [(u64, &str, u32); 4] = [
    (ACCESS_READ_WRITE, "Read and write", 0o600),
    (ACCESS_READ_ONLY, "Read only", 0o400),
    (ACCESS_WRITE_ONLY, "Write only", 0o200),
    (ACCESS_NONE, "No access", 0),
];

fn owner_access_label(mode: u32) -> &'static str {
    match mode & 0o600 {
        0o600 => "Read and write",
        0o400 => "Read only",
        0o200 => "Write only",
        _ => "No access",
    }
}

fn parse_mode(value: &str) -> Result<u32, &'static str> {
    let value = value.trim().strip_prefix("0o").unwrap_or(value.trim());
    if !(3..=4).contains(&value.len()) || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err("Enter three or four octal digits (for example, 0644)");
    }
    u32::from_str_radix(value, 8)
        .ok()
        .filter(|mode| *mode <= 0o7777)
        .ok_or("Permissions must be between 0000 and 7777")
}

fn can_edit_mode(state: &DesktopProperties) -> bool {
    state.result == PropertyResult::Ready
        && state
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.kind != FileKind::Symlink)
}

fn apply_enabled(state: &DesktopProperties) -> bool {
    can_edit_mode(state)
        && parse_mode(&state.mode_input)
            .ok()
            .zip(state.metadata.as_ref())
            .is_some_and(|(mode, metadata)| mode != metadata.mode & 0o7777)
}

fn validation_feedback(state: &DesktopProperties) -> Option<&'static str> {
    (state.result == PropertyResult::Ready && state.metadata.is_some())
        .then(|| parse_mode(&state.mode_input).err())
        .flatten()
}

fn shows_executable(state: &DesktopProperties) -> bool {
    if state.metadata.as_ref().map(|metadata| metadata.kind) != Some(FileKind::File) {
        return false;
    }
    state
        .item
        .path
        .as_deref()
        .unwrap_or(&state.item.label)
        .rsplit_once('.')
        .is_some_and(|(_, extension)| {
            ["sh", "bin", "run"]
                .into_iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn symbolic_mode(mode: u32, kind: FileKind) -> String {
    let mut text = String::with_capacity(10);
    text.push(match kind {
        FileKind::Directory => 'd',
        FileKind::Symlink => 'l',
        FileKind::File => '-',
        FileKind::Other => '?',
    });
    for (read, write, execute) in [
        (0o400, 0o200, 0o100),
        (0o040, 0o020, 0o010),
        (0o004, 0o002, 0o001),
    ] {
        text.push(if mode & read != 0 { 'r' } else { '-' });
        text.push(if mode & write != 0 { 'w' } else { '-' });
        text.push(if mode & execute != 0 { 'x' } else { '-' });
    }
    text
}

fn update_metadata(state: &mut DesktopProperties, metadata: &FileMetadata) {
    let mut metadata = metadata.clone();
    metadata.mode &= 0o7777;
    state.mode_input = format!("{:04o}", metadata.mode);
    state.item.read_only = metadata.mode & 0o200 == 0;
    state.item.executable = metadata.mode & 0o111 != 0;
    state.metadata = Some(metadata);
}

fn desktop_kind(item: &DesktopItem) -> &'static str {
    match item.kind {
        DesktopItemKind::Home => "Home folder",
        DesktopItemKind::Documents => "Documents folder",
        DesktopItemKind::Pictures => "Pictures folder",
        DesktopItemKind::Trash { .. } => "Trash",
        DesktopItemKind::Mount { .. } => "Mounted location",
        DesktopItemKind::Folder => "Folder",
        DesktopItemKind::File => "File",
        DesktopItemKind::Application { .. } => "Application launcher",
    }
}

const fn file_kind(kind: FileKind) -> &'static str {
    match kind {
        FileKind::File => "File",
        FileKind::Directory => "Folder",
        FileKind::Symlink => "Symbolic link",
        FileKind::Other => "Other",
    }
}

fn icon_for(item: &DesktopItem) -> IconId {
    super::desktop_view::icon_for_item(item)
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {} ({bytes} bytes)", UNITS[unit])
    }
}

fn vfs_error(error: &VfsError) -> String {
    if error.message.trim().is_empty() {
        format!("{:?}", error.code)
    } else {
        error.message.clone()
    }
}

#[allow(clippy::too_many_arguments)]
fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
    state: WidgetState,
) {
    let parent = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| matches!(patch, Patch::Create { parent: value, .. } if *value == parent))
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state,
            text: text.map(str::to_owned),
            icon,
        },
        parent,
        index,
    });
}
