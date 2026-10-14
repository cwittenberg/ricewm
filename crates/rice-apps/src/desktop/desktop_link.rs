//! Real file/folder target selection for creating Desktop symbolic links.

use rice_native_app_protocol::{
    FileKind, Message, RequestId, UiEvent, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};
use rice_ui::{
    ChooserEffect, FileChooser, FileChooserEventResult, FileChooserMode, FileChooserViewIds,
    FileFilter, IconId, Patch, Place, ValueChange, WidgetKind, WidgetSpec, WidgetState,
    compose_file_chooser,
};

use super::{
    ActivationTarget, DesktopActorError, DesktopItem, DesktopItemKind, ROOT, RiceDesktop,
    activation_target,
    desktop_clipboard::{TransferAction, TransferStep},
    desktop_path::valid_file_name,
    desktop_vfs::DESKTOP_PATH,
    desktop_view::replace_item_patches,
    widget_id,
};

pub(super) const LINK_DIALOG: u64 = 400;
pub(super) const LINK_NAME_FIELD: u64 = 401;
pub(super) const LINK_TARGET_FIELD: u64 = 402;
pub(super) const LINK_FIND_TARGET: u64 = 403;
pub(super) const LINK_CREATE: u64 = 404;
pub(super) const LINK_CANCEL: u64 = 405;
const LINK_FORM: u64 = 406;
const LINK_NAME_ROW: u64 = 407;
const LINK_TARGET_ROW: u64 = 408;
const LINK_ACTIONS: u64 = 409;
const LINK_FEEDBACK: u64 = 410;
const LINK_CHOOSER_DIALOG: u64 = 420;
pub(super) const LINK_CHOOSER_ACCEPT: u64 = 421;
const LINK_CHOOSER_CANCEL: u64 = 422;
const LINK_CHOOSER_IDS: FileChooserViewIds = FileChooserViewIds::new(
    LINK_CHOOSER_DIALOG,
    LINK_CHOOSER_ACCEPT,
    LINK_CHOOSER_CANCEL,
    500,
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LinkDraft {
    name: String,
    target: String,
    feedback: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingLinkActivation {
    pub(super) request: RequestId,
    link_path: String,
    target: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingLinkIcon {
    pub(super) request: RequestId,
    link_path: String,
    target: Option<String>,
}

impl RiceDesktop {
    pub(super) fn begin_link_activation(
        &mut self,
        item: DesktopItem,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.busy_with_vfs() {
            return self.notice("Another desktop filesystem request is already in progress");
        }
        let Some(link_path) = item.path else {
            return self.notice("The selected shortcut has no filesystem path");
        };
        self.pending_link_activation = Some(PendingLinkActivation {
            request,
            link_path: link_path.clone(),
            target: None,
        });
        Ok(vec![Message::VfsRequest(VfsRequest {
            request,
            app: self.app,
            operation: VfsOperation::ReadLink { path: link_path },
        })])
    }

    pub(super) fn handle_link_activation_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_link_activation
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if pending.target.is_none() {
            return match &response.result {
                VfsResult::Data(data) => match String::from_utf8(data.clone()) {
                    Ok(target) if target.starts_with('/') => {
                        pending.target = Some(target.clone());
                        self.pending_link_activation = Some(pending);
                        Ok(vec![Message::VfsRequest(VfsRequest {
                            request: response.request,
                            app: self.app,
                            operation: VfsOperation::Stat { path: target },
                        })])
                    }
                    _ => self.notice("The shortcut target is not a valid absolute guest path"),
                },
                VfsResult::Error(error) => self.notice(&format!(
                    "Could not read shortcut {}: {}",
                    pending.link_path,
                    error.message.trim()
                )),
                _ => self.notice("Could not read shortcut: invalid filesystem response"),
            };
        }
        let target = pending.target.expect("link target stage");
        let metadata = match &response.result {
            VfsResult::Metadata(metadata) => metadata,
            VfsResult::Error(error) => {
                return self.notice(&format!(
                    "Could not open shortcut target: {}",
                    error.message.trim()
                ));
            }
            _ => return self.notice("Could not inspect shortcut target"),
        };
        if metadata.kind == FileKind::Symlink {
            return self.notice("Nested symbolic links are not opened from the desktop");
        }
        let kind = if metadata.kind == FileKind::Directory {
            DesktopItemKind::Folder
        } else if target.to_ascii_lowercase().ends_with(".desktop") {
            DesktopItemKind::Application {
                desktop_id: target.clone(),
            }
        } else {
            DesktopItemKind::File
        };
        let resolved = DesktopItem {
            id: rice_desktop::DesktopItemId(1),
            label: target.rsplit('/').next().unwrap_or(&target).to_owned(),
            icon_id: String::new(),
            path: Some(target),
            kind,
            read_only: metadata.mode & 0o200 == 0,
            executable: metadata.mode & 0o111 != 0,
            available: true,
        };
        match activation_target(&resolved) {
            ActivationTarget::Launch {
                entrypoint,
                arguments,
            } => Ok(vec![Message::Launch {
                app: self.app,
                entrypoint: entrypoint.to_owned(),
                arguments,
            }]),
            ActivationTarget::DesktopEntry(path) => {
                self.begin_desktop_launch(path, response.request)
            }
            ActivationTarget::File(path) => self.begin_file_activation(path, response.request),
            ActivationTarget::Unsupported(path) => {
                self.notice(&format!("No installed RiceWM application handles {path}"))
            }
        }
    }

    pub(super) fn begin_link_dialog(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        if self.link_draft.is_some() || self.link_chooser.is_some() || self.busy_with_vfs() {
            return self.notice("Another desktop filesystem request is already in progress");
        }
        self.link_draft = Some(LinkDraft {
            name: String::from("New Shortcut"),
            target: String::new(),
            feedback: None,
        });
        self.render_link_dialog().map(|message| vec![message])
    }

    pub(super) fn update_link_name(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(draft) = self.link_draft.as_mut() else {
            return Ok(Vec::new());
        };
        draft.name = value.to_owned();
        draft.feedback = validate_link_draft(draft).err().map(str::to_owned);
        self.update_link_dialog_field(LINK_NAME_FIELD)
    }

    pub(super) fn update_link_target(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(draft) = self.link_draft.as_mut() else {
            return Ok(Vec::new());
        };
        draft.target = value.to_owned();
        draft.feedback = validate_link_draft(draft).err().map(str::to_owned);
        self.update_link_dialog_field(LINK_TARGET_FIELD)
    }

    pub(super) fn cancel_link_dialog(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        self.link_draft = None;
        self.link_chooser = None;
        let mut patches = Vec::new();
        for id in [LINK_CHOOSER_DIALOG, LINK_DIALOG] {
            if self.tree.get(widget_id(id)).is_some() {
                patches.push(Patch::Remove { id: widget_id(id) });
            }
        }
        if patches.is_empty() {
            return Ok(Vec::new());
        }
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn create_link(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(draft) = self.link_draft.as_mut() else {
            return Ok(Vec::new());
        };
        if let Err(error) = validate_link_draft(draft) {
            draft.feedback = Some(error.to_owned());
            return self.render_link_dialog().map(|message| vec![message]);
        }
        let name = draft.name.trim().to_owned();
        let target = draft.target.trim().to_owned();
        let destination = format!("{DESKTOP_PATH}/{name}");
        self.link_icon_ids.remove(&destination);
        self.link_draft = None;
        let mut messages = vec![self.emit(&[Patch::Remove {
            id: widget_id(LINK_DIALOG),
        }])?];
        messages.extend(self.start_transfer(
            request,
            vec![TransferStep {
                from: target,
                to: destination,
                action: TransferAction::Link,
            }],
        )?);
        Ok(messages)
    }

    pub(super) fn begin_link_chooser(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self.link_draft.is_none() {
            return Ok(Vec::new());
        }
        if self.link_chooser.is_some() || self.busy_with_vfs() {
            return self.notice("Another desktop filesystem request is already in progress");
        }
        let initial = self
            .link_draft
            .as_ref()
            .map(|draft| draft.target.as_str())
            .filter(|target| target.starts_with('/'))
            .and_then(|target| target.rsplit_once('/').map(|(parent, _)| parent))
            .filter(|parent| !parent.is_empty())
            .unwrap_or("/root/");
        let mut chooser = FileChooser::new(
            FileChooserMode::SelectAny,
            initial,
            vec![FileFilter::all_files("All Files and Folders").localized()],
            vec![
                Place::localized("Home", "/root"),
                Place::localized("Desktop", DESKTOP_PATH),
                Place::localized("Documents", "/root/Documents"),
                Place::localized("Pictures", "/root/Pictures"),
            ],
        );
        let effect = chooser.start(request);
        self.link_chooser = Some(chooser);
        self.apply_link_chooser_effect(effect, request)
    }

    pub(super) fn handle_link_chooser_event(
        &mut self,
        event: &UiEvent,
        target: u64,
        request: RequestId,
    ) -> Result<Option<Vec<Message>>, DesktopActorError> {
        let Some(chooser) = self.link_chooser.as_mut() else {
            return Ok(None);
        };
        match chooser.handle_ui_event(LINK_CHOOSER_IDS, target, event, request) {
            FileChooserEventResult::Unhandled => Ok(None),
            FileChooserEventResult::Cancel => {
                self.link_chooser = None;
                self.emit(&[Patch::Remove {
                    id: widget_id(LINK_CHOOSER_DIALOG),
                }])
                .map(|message| Some(vec![message]))
            }
            FileChooserEventResult::Effect(effect) => {
                self.apply_link_chooser_effect(effect, request).map(Some)
            }
        }
    }

    pub(super) fn owns_link_chooser_request(&self, request: RequestId) -> bool {
        self.link_chooser
            .as_ref()
            .and_then(FileChooser::pending_request)
            == Some(request)
    }

    pub(super) fn handle_link_chooser_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let effect = self
            .link_chooser
            .as_mut()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?
            .handle_response(response);
        self.apply_link_chooser_effect(effect, response.request)
    }

    fn apply_link_chooser_effect(
        &mut self,
        effect: ChooserEffect,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        match effect {
            ChooserEffect::Render => self.render_link_chooser().map(|message| vec![message]),
            ChooserEffect::Request(operation) => Ok(vec![
                self.render_link_chooser()?,
                Message::VfsRequest(VfsRequest {
                    request,
                    app: self.app,
                    operation,
                }),
            ]),
            ChooserEffect::Accepted(target) => {
                self.link_chooser = None;
                let Some(draft) = self.link_draft.as_mut() else {
                    return Ok(Vec::new());
                };
                let suggested = target
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .filter(|name| !name.is_empty())
                    .unwrap_or("Shortcut");
                if draft.name.trim().is_empty() || draft.name == "New Shortcut" {
                    draft.name = format!("{suggested} Shortcut");
                }
                draft.target = target;
                draft.feedback = None;
                Ok(vec![self.render_link_dialog_with(&[Patch::Remove {
                    id: widget_id(LINK_CHOOSER_DIALOG),
                }])?])
            }
        }
    }

    fn render_link_chooser(&mut self) -> Result<Message, DesktopActorError> {
        let chooser = self
            .link_chooser
            .as_ref()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        let mut patches = Vec::new();
        if self.tree.get(widget_id(LINK_CHOOSER_DIALOG)).is_some() {
            patches.push(Patch::Remove {
                id: widget_id(LINK_CHOOSER_DIALOG),
            });
        }
        patches.push(Patch::Create {
            widget: WidgetSpec {
                id: widget_id(LINK_CHOOSER_DIALOG),
                kind: WidgetKind::Dialog,
                state: WidgetState::ENABLED | WidgetState::EXPANSIVE,
                text: Some(String::from("Find Shortcut Target")),
                icon: Some(IconId::Search),
            },
            parent: Some(widget_id(ROOT)),
            index: 2,
        });
        compose_file_chooser(&mut patches, chooser, LINK_CHOOSER_IDS);
        self.emit(&patches)
    }

    fn render_link_dialog(&mut self) -> Result<Message, DesktopActorError> {
        self.render_link_dialog_with(&[])
    }

    /// Keep the active native text editor alive while its value changes.  A
    /// remove/recreate cycle here loses the browser-side editor and is visible
    /// as an unresponsive field in the live RiceWM canvas.
    fn update_link_dialog_field(&mut self, field: u64) -> Result<Vec<Message>, DesktopActorError> {
        let draft = self
            .link_draft
            .as_ref()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        let value = if field == LINK_NAME_FIELD {
            draft.name.clone()
        } else {
            draft.target.clone()
        };
        let mut patches = vec![
            Patch::Update {
                id: widget_id(field),
                state: Some(WidgetState::ENABLED | WidgetState::FOCUSED),
                text: ValueChange::Set(value),
                icon: ValueChange::Keep,
            },
            Patch::Update {
                id: widget_id(LINK_CREATE),
                state: Some(if validate_link_draft(draft).is_ok() {
                    WidgetState::ENABLED | WidgetState::ACCENT
                } else {
                    WidgetState::empty()
                }),
                text: ValueChange::Keep,
                icon: ValueChange::Keep,
            },
        ];
        match (
            self.tree.get(widget_id(LINK_FEEDBACK)).is_some(),
            draft.feedback.as_deref(),
        ) {
            (true, Some(feedback)) => patches.push(Patch::Update {
                id: widget_id(LINK_FEEDBACK),
                state: Some(WidgetState::ENABLED | WidgetState::WARNING),
                text: ValueChange::Set(feedback.to_owned()),
                icon: ValueChange::Keep,
            }),
            (true, None) => patches.push(Patch::Remove {
                id: widget_id(LINK_FEEDBACK),
            }),
            (false, Some(feedback)) => patches.push(Patch::Create {
                widget: WidgetSpec {
                    id: widget_id(LINK_FEEDBACK),
                    kind: WidgetKind::Label,
                    state: WidgetState::ENABLED | WidgetState::WARNING,
                    text: Some(feedback.to_owned()),
                    icon: Some(IconId::Warning),
                },
                parent: Some(widget_id(LINK_FORM)),
                index: 3,
            }),
            (false, None) => {}
        }
        self.emit(&patches).map(|message| vec![message])
    }

    fn render_link_dialog_with(&mut self, leading: &[Patch]) -> Result<Message, DesktopActorError> {
        let draft = self
            .link_draft
            .as_ref()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        let mut patches = leading.to_vec();
        if self.tree.get(widget_id(LINK_DIALOG)).is_some() {
            patches.push(Patch::Remove {
                id: widget_id(LINK_DIALOG),
            });
        }
        patches.push(Patch::Create {
            widget: WidgetSpec {
                id: widget_id(LINK_DIALOG),
                kind: WidgetKind::Dialog,
                state: WidgetState::ENABLED,
                text: Some(String::from("Create Shortcut")),
                icon: Some(IconId::Document.shortcut()),
            },
            parent: Some(widget_id(ROOT)),
            index: 1,
        });
        create(
            &mut patches,
            LINK_FORM,
            Some(LINK_DIALOG),
            WidgetKind::Column,
            None,
            None,
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            LINK_NAME_ROW,
            Some(LINK_FORM),
            WidgetKind::Row,
            Some("split:110"),
            None,
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            LINK_NAME_ROW + 100,
            Some(LINK_NAME_ROW),
            WidgetKind::Label,
            Some(rice_i18n::t("Name")),
            None,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
        );
        create(
            &mut patches,
            LINK_NAME_FIELD,
            Some(LINK_NAME_ROW),
            WidgetKind::TextField,
            Some(&draft.name),
            Some(IconId::EditRename),
            WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::SELECT_ALL_ON_FOCUS,
        );
        create(
            &mut patches,
            LINK_TARGET_ROW,
            Some(LINK_FORM),
            WidgetKind::Row,
            Some("split:110"),
            None,
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            LINK_TARGET_ROW + 100,
            Some(LINK_TARGET_ROW),
            WidgetKind::Label,
            Some(rice_i18n::t("Target")),
            None,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
        );
        create(
            &mut patches,
            LINK_TARGET_FIELD,
            Some(LINK_TARGET_ROW),
            WidgetKind::TextField,
            Some(&draft.target),
            Some(IconId::DocumentOpen),
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            LINK_FIND_TARGET,
            Some(LINK_FORM),
            WidgetKind::Button,
            Some(rice_i18n::t("Find Target…")),
            Some(IconId::Search),
            WidgetState::ENABLED,
        );
        if let Some(feedback) = draft.feedback.as_deref() {
            create(
                &mut patches,
                LINK_FEEDBACK,
                Some(LINK_FORM),
                WidgetKind::Label,
                Some(feedback),
                Some(IconId::Warning),
                WidgetState::ENABLED | WidgetState::WARNING,
            );
        }
        create(
            &mut patches,
            LINK_ACTIONS,
            Some(LINK_FORM),
            WidgetKind::Row,
            Some("panel:actions"),
            None,
            WidgetState::ENABLED,
        );
        create(
            &mut patches,
            LINK_CREATE,
            Some(LINK_ACTIONS),
            WidgetKind::Button,
            Some(rice_i18n::t("Create Shortcut")),
            Some(IconId::Document.shortcut()),
            if validate_link_draft(draft).is_ok() {
                WidgetState::ENABLED | WidgetState::ACCENT
            } else {
                WidgetState::empty()
            },
        );
        create(
            &mut patches,
            LINK_CANCEL,
            Some(LINK_ACTIONS),
            WidgetKind::Button,
            Some(rice_i18n::t("Cancel")),
            Some(IconId::Close),
            WidgetState::ENABLED,
        );
        self.emit(&patches)
    }
}

fn validate_link_draft(draft: &LinkDraft) -> Result<(), &'static str> {
    if !valid_file_name(draft.name.trim()) {
        return Err("Enter a valid shortcut name without '/'");
    }
    let target = draft.target.trim();
    if !target.starts_with('/') {
        return Err("Choose an absolute file or folder target");
    }
    if target.len() > 4_096 || target.bytes().any(|byte| byte == 0 || byte < 0x20) {
        return Err("The shortcut target contains invalid characters");
    }
    Ok(())
}

impl RiceDesktop {
    /// Resolve every visible symlink after a directory refresh.  Directory
    /// enumeration deliberately reports links without following them, so this
    /// bounded queue is what recovers the target's real folder/MIME icon.
    pub(super) fn start_link_icon_resolution(&mut self, request: RequestId) -> Option<Message> {
        if self.pending_link_icon.is_some() {
            return None;
        }
        let visible = self
            .items
            .iter()
            .filter(|item| item.icon_id == "link" || item.icon_id.starts_with("shortcut:"))
            .filter_map(|item| item.path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        self.link_icon_ids.retain(|path, _| visible.contains(path));
        self.link_icon_queue.clear();
        self.link_icon_queue.extend(
            visible
                .into_iter()
                .filter(|path| !self.link_icon_ids.contains_key(path)),
        );
        self.start_next_link_icon(request)
    }

    pub(super) fn owns_link_icon_request(&self, request: RequestId) -> bool {
        self.pending_link_icon
            .as_ref()
            .is_some_and(|pending| pending.request == request)
    }

    pub(super) fn refresh_link_icon(
        &mut self,
        link_path: String,
        request: RequestId,
    ) -> Option<Message> {
        self.link_icon_ids.remove(&link_path);
        self.link_icon_queue.retain(|queued| queued != &link_path);
        self.link_icon_queue.push_front(link_path);
        if self.pending_link_icon.is_none() {
            self.start_next_link_icon(request)
        } else {
            None
        }
    }

    pub(super) fn handle_link_icon_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let mut pending = self
            .pending_link_icon
            .take()
            .ok_or(DesktopActorError::UnexpectedVfsResponse)?;
        if pending.target.is_none() {
            if let VfsResult::Data(data) = &response.result
                && let Ok(target) = String::from_utf8(data.clone())
                && target.starts_with('/')
            {
                pending.target = Some(target.clone());
                self.pending_link_icon = Some(pending);
                return Ok(vec![Message::VfsRequest(VfsRequest {
                    request: response.request,
                    app: self.app,
                    operation: VfsOperation::Stat { path: target },
                })]);
            }
            return Ok(self
                .start_next_link_icon(response.request)
                .into_iter()
                .collect());
        }

        let target = pending.target.expect("link icon target stage");
        let semantic = match &response.result {
            VfsResult::Metadata(metadata) if metadata.kind == FileKind::Directory => {
                format!("shortcut:folder:{target}")
            }
            VfsResult::Metadata(metadata) if metadata.kind == FileKind::File => {
                if target.to_ascii_lowercase().ends_with(".desktop") && metadata.mode & 0o111 != 0 {
                    format!("shortcut:application:{target}")
                } else {
                    format!("shortcut:file:{target}")
                }
            }
            _ => String::from("link"),
        };
        self.link_icon_ids
            .insert(pending.link_path.clone(), semantic.clone());
        let mut changed = false;
        if let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.path.as_deref() == Some(pending.link_path.as_str()))
            && item.icon_id != semantic
        {
            item.icon_id.clone_from(&semantic);
            if let Some(model_item) = self.model.item_mut(item.id) {
                model_item.icon_id.clone_from(&semantic);
            }
            changed = true;
        }
        let mut messages = Vec::new();
        if changed {
            let patches = replace_item_patches(
                &self.items,
                self.model.grid(),
                self.model.selection(),
                &self.thumbnail_cache,
            );
            messages.push(self.emit(&patches)?);
        }
        if let Some(next) = self.start_next_link_icon(response.request) {
            messages.push(next);
        }
        Ok(messages)
    }

    fn start_next_link_icon(&mut self, request: RequestId) -> Option<Message> {
        let link_path = self.link_icon_queue.pop_front()?;
        self.pending_link_icon = Some(PendingLinkIcon {
            request,
            link_path: link_path.clone(),
            target: None,
        });
        Some(Message::VfsRequest(VfsRequest {
            request,
            app: self.app,
            operation: VfsOperation::ReadLink { path: link_path },
        }))
    }
}

fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
    state: WidgetState,
) {
    let parent_id = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| matches!(patch, Patch::Create { parent, .. } if *parent == parent_id))
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state,
            text: text.map(str::to_owned),
            icon,
        },
        parent: parent_id,
        index,
    });
}
