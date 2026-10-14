//! Context menu policy adaptation and executable desktop actions.

use super::{
    ARRANGE, COPY, CUT, DELETE, DesktopActorError, DesktopCommand, DesktopIntent, EMPTY_TRASH,
    IconId, LINK, MenuActionSpec, Message, NEW_FILE, NEW_FOLDER, OPEN, OPEN_WITH, PASTE,
    PROPERTIES, RENAME, RENAME_FIELD, RiceDesktop, SELECT_ALL, SETTINGS, activation_target,
    replace_item_patches, switch_context_patches,
};
use super::{ActivationTarget, ContextKind, RequestId};
use rice_desktop::{ContextMenuPolicy, DesktopItemKind, ItemAction, MenuEntry, MenuRequest};

impl RiceDesktop {
    pub(super) fn open_rename_editor(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        // Keep the editor bound to the item that opened it. Pointer activity
        // and directory refreshes may change the desktop selection while the
        // modal dialog is open; submission must not rename that later item.
        self.rename_source_path = self
            .selected_item()
            .filter(|item| item.is_renameable())
            .and_then(|item| item.path.clone());
        let mut patches = switch_context_patches(
            self.context_kind,
            ContextKind::Rename,
            &self.rename_text,
            &self.context_action_ids,
            &[],
        );
        self.retained_notice = None;
        if self.tree.get(super::widget_id(super::NOTICE)).is_some() {
            patches.push(rice_ui::Patch::Remove {
                id: super::widget_id(super::NOTICE),
            });
        }
        self.context_kind = ContextKind::Rename;
        self.context_action_ids = vec![RENAME_FIELD, RENAME];
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn close_rename_editor(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let mut patches = switch_context_patches(
            self.context_kind,
            ContextKind::Hidden,
            "",
            &self.context_action_ids,
            &[],
        );
        self.retained_notice = None;
        if self.tree.get(super::widget_id(super::NOTICE)).is_some() {
            patches.push(rice_ui::Patch::Remove {
                id: super::widget_id(super::NOTICE),
            });
        }
        self.context_kind = ContextKind::Hidden;
        self.context_action_ids.clear();
        self.rename_source_path = None;
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn context_menu_entries(
        &mut self,
        clipboard_has_files: bool,
    ) -> Result<Vec<MenuActionSpec>, DesktopActorError> {
        let update = self.model.execute(DesktopCommand::RequestContextMenu {
            clipboard_has_files,
            desktop_read_only: false,
        })?;
        let entries = update.intents.into_iter().find_map(|intent| match intent {
            DesktopIntent::ShowContextMenu(entries) => Some(entries),
            _ => None,
        });
        let mut actions = self.adapt_menu_entries(entries.unwrap_or_default());
        if self.model.selection().is_empty() {
            let index = actions
                .iter()
                .position(|entry| entry.id == SETTINGS)
                .unwrap_or(actions.len());
            let index = if index > 0 && actions[index - 1].label == rice_ui::MENU_SEPARATOR { index - 1 } else { index };
            actions.insert(
                index,
                MenuActionSpec {
                    id: super::REFRESH,
                    label: String::from(rice_i18n::t("Refresh")),
                    message: "Refresh",
                    icon: IconId::Refresh,
                    checked: None,
                },
            );
        }
        Ok(actions)
    }

    pub(super) fn context_menu_entries_for_trash(
        &self,
        nonempty: bool,
    ) -> Result<Vec<MenuActionSpec>, DesktopActorError> {
        let Some(selected) = self.selected_item() else {
            return Ok(Vec::new());
        };
        let mut item = selected.clone();
        item.kind = DesktopItemKind::Trash { empty: !nonempty };
        Ok(self.adapt_menu_entries(ContextMenuPolicy.entries(MenuRequest::Selection {
            items: &[&item],
            clipboard_has_files: true,
        })))
    }

    // Preserve policy groups after unsupported/disabled actions are removed.
    // Delaying separators avoids empty groups and leading/trailing rules.
    fn adapt_menu_entries(&self, entries: Vec<MenuEntry>) -> Vec<MenuActionSpec> {
        let mut actions = Vec::new();
        let mut pending_separator = false;
        for entry in entries {
            match entry {
                MenuEntry::Separator => pending_separator = !actions.is_empty(),
                MenuEntry::Action { label, action, enabled: true } => {
                    if let Some(action) = self.supported_menu_action(label, action) {
                        if pending_separator {
                            actions.push(MenuActionSpec {
                                id: 900 + actions.len() as u64,
                                label: rice_ui::MENU_SEPARATOR.to_owned(),
                                message: rice_ui::MENU_SEPARATOR,
                                icon: IconId::Document,
                                checked: None,
                            });
                        }
                        pending_separator = false;
                        actions.push(action);
                    }
                }
                _ => {}
            }
        }
        actions
    }

    fn supported_menu_action(
        &self,
        label: &'static str,
        action: ItemAction,
    ) -> Option<MenuActionSpec> {
        let (id, icon) = match action {
            ItemAction::Open | ItemAction::Launch => (OPEN, IconId::FolderOpen),
            ItemAction::OpenWith => {
                let item = self.selected_item()?;
                let path = item.path.as_deref()?;
                if !path.starts_with('/') { return None; }
                (OPEN_WITH, IconId::Application)
            }
            ItemAction::Rename => (RENAME, IconId::EditUndo),
            ItemAction::MoveToTrash => (DELETE, IconId::Trash),
            ItemAction::Cut => (CUT, IconId::EditCut),
            ItemAction::Copy => (COPY, IconId::EditCopy),
            ItemAction::Paste => (PASTE, IconId::EditPaste),
            ItemAction::Link => (LINK, IconId::Document.shortcut()),
            ItemAction::EmptyTrash => (EMPTY_TRASH, IconId::Trash),
            ItemAction::Properties => (PROPERTIES, IconId::Information),
            ItemAction::NewFolder => (NEW_FOLDER, IconId::Folder),
            ItemAction::NewFile => (NEW_FILE, IconId::Document),
            ItemAction::SelectAll => (SELECT_ALL, IconId::ViewList),
            ItemAction::ArrangeIcons => (ARRANGE, IconId::ViewGrid),
            ItemAction::DesktopSettings => (SETTINGS, IconId::Settings),
            _ => return None,
        };
        // Only policy-owned static labels enter this metadata. User item names do not.
        let message = if action == ItemAction::Rename {
            "Rename…"
        } else if action == ItemAction::ArrangeIcons {
            "Auto-arrange icons"
        } else if action == ItemAction::Paste && label == "Paste Into" {
            "desktop.paste-into"
        } else {
            label
        };
        Some(MenuActionSpec {
            id,
            label: rice_i18n::t(message).to_owned(),
            message,
            icon,
            checked: (action == ItemAction::ArrangeIcons).then_some(self.auto_arrange),
        })
    }

    pub(super) fn select_all(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        self.model.execute(DesktopCommand::SelectAll)?;
        let patches = self.selection_patches("All desktop items selected");
        self.emit(&patches).map(|message| vec![message])
    }

    pub(super) fn toggle_auto_arrange(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.auto_arrange = !self.auto_arrange;
        if self.auto_arrange {
            let update = self.model.execute(DesktopCommand::ArrangeIcons)?;
            if let Some(positions) = update.intents.iter().find_map(|intent| match intent {
                DesktopIntent::PersistPositions(positions) => Some(positions.as_slice()),
                _ => None,
            }) {
                self.positions
                    .replace_workspace(self.model.workspace(), &self.items, positions);
            }
        }
        self.rendered_items = self.model.grid().ordered_items();
        let patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        self.auto_arrange_saves.insert(request);
        Ok(vec![
            self.emit(&patches)?,
            Message::SettingsRequest(rice_native_app_protocol::SettingsRequest {
                request,
                app: self.app,
                operation: rice_native_app_protocol::SettingsOperation::Set {
                    key: String::from("desktop.icons.auto_arrange"),
                    value: self.auto_arrange.to_string(),
                },
            }),
        ])
    }

    pub(super) fn launch_settings(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let update = self.model.execute(DesktopCommand::RequestDesktopSettings)?;
        self.pending_intents = update.intents;
        Ok(vec![Message::Launch {
            app: self.app,
            entrypoint: String::from("rice-settings"),
            // "Desktop Settings" should land on the page that contains the
            // real Auto-arrange checkbox and icon visibility controls.  The
            // previous Appearance landing page made the setting effectively
            // undiscoverable from the desktop context menu.
            arguments: vec![String::from("--section=desktop")],
        }])
    }

    pub(super) fn activate_item(
        &mut self,
        item: super::DesktopItemId,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.model.execute(DesktopCommand::Click {
            item,
            mode: super::ClickMode::Replace,
        })?;
        let update = self.model.execute(DesktopCommand::Activate(item))?;
        self.pending_intents = update.intents;
        let entry = self.model.item(item).cloned();
        self.model.execute(DesktopCommand::ClearSelection)?;
        let selected = self.emit(&self.selection_patches(""))?;
        let Some(entry) = entry else {
            return Ok(vec![selected]);
        };
        if entry.icon_id == "link" || entry.icon_id.starts_with("shortcut:") {
            let mut messages = vec![selected];
            messages.extend(self.begin_link_activation(entry, request)?);
            return Ok(messages);
        }
        match activation_target(&entry) {
            ActivationTarget::Launch {
                entrypoint,
                arguments,
            } => Ok(vec![
                selected,
                Message::Launch {
                    app: self.app,
                    entrypoint: entrypoint.to_owned(),
                    arguments,
                },
            ]),
            ActivationTarget::DesktopEntry(path) => {
                let mut messages = vec![selected];
                messages.extend(self.begin_desktop_launch(path, request)?);
                Ok(messages)
            }
            ActivationTarget::File(path) => {
                let mut messages = vec![selected];
                messages.extend(self.begin_file_activation(path, request)?);
                Ok(messages)
            }
            ActivationTarget::Unsupported(path) => {
                let mut messages = vec![selected];
                messages.extend(
                    self.unsupported_application_notice(&path)?,
                );
                Ok(messages)
            }
        }
    }
}
