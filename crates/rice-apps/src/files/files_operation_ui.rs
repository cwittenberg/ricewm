//! Dialog-driven file operations for `RiceFiles`.

use rice_native_app_protocol::{Message, RequestId};

use super::{FileControls, FilesActorError, RiceFiles, files_dialogs::Dialog};

impl RiceFiles {
    pub(super) fn show_create_folder(&mut self) -> Result<Vec<Message>, FilesActorError> {
        self.dialog = Some(Dialog::CreateFolder {
            value: String::from(rice_i18n::t("New Folder")),
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_rename(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.selected_entries().into_iter().next() else {
            return self.emit_notice(rice_i18n::t("Select one item before renaming"));
        };
        if super::files_preferences::is_builtin(entry.path.as_str()) {
            return self.emit_notice(rice_i18n::t("Built-in Places cannot be renamed"));
        }
        self.dialog = Some(Dialog::Rename { value: entry.name });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_move(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if self.selected_entries().is_empty() {
            return self.emit_notice(rice_i18n::t("Select at least one item before moving"));
        }
        if self
            .selected_entries()
            .iter()
            .any(|entry| super::files_preferences::is_builtin(entry.path.as_str()))
        {
            return self.emit_notice(rice_i18n::t("Built-in Places cannot be moved"));
        }
        self.dialog = Some(Dialog::MoveTo {
            value: self.location().as_str().to_owned(),
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_copy(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if self.selected_entries().is_empty() {
            return self.emit_notice(rice_i18n::t("Select at least one item before copying"));
        }
        self.dialog = Some(Dialog::CopyTo {
            value: self.location().as_str().to_owned(),
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_delete_permanently(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if self.selected_entries().is_empty() {
            return self.emit_notice(rice_i18n::t("Select at least one item before deleting"));
        }
        if self
            .selected_entries()
            .iter()
            .any(|entry| super::files_preferences::is_builtin(entry.path.as_str()))
        {
            return self.emit_notice(rice_i18n::t("Built-in Places cannot be deleted"));
        }
        self.dialog = Some(Dialog::DeletePermanently);
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_about(&mut self) -> Result<Vec<Message>, FilesActorError> {
        self.dialog = Some(Dialog::About { section: 0, scroll: 0 });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn close_operation_dialog(&mut self) -> Result<Vec<Message>, FilesActorError> {
        if let Some(Dialog::Properties(state)) = &self.dialog {
            for request in state.pending_requests() {
                if !self.abandoned_property_requests.contains(&request) {
                    self.abandoned_property_requests.push(request);
                }
                if self.abandoned_property_requests.len() > 16 {
                    self.abandoned_property_requests.remove(0);
                }
            }
        }
        if self.dialog.take().is_none() {
            return Ok(Vec::new());
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn update_operation_field(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        match self.dialog.as_mut() {
            Some(Dialog::CreateFolder { value: draft })
            | Some(Dialog::Rename { value: draft })
            | Some(Dialog::MoveTo { value: draft })
            | Some(Dialog::CopyTo { value: draft })
            | Some(Dialog::Conflict { rename: draft, .. }) => draft.clone_from(&value.to_owned()),
            _ => return Ok(Vec::new()),
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn toggle_conflict_apply_all(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Conflict { apply_all, .. }) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        *apply_all = !*apply_all;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn confirm_operation(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(Dialog::Conflict { rename, .. }) = &self.dialog {
            let rename = rename.clone();
            return self.rename_conflict(&rename);
        }
        let Some(dialog) = self.dialog.take() else {
            return Ok(Vec::new());
        };
        let sources = self.selected_entries();
        let label = match &dialog {
            Dialog::CreateFolder { .. } => "Create Folder",
            Dialog::Rename { .. } => "Rename",
            Dialog::MoveTo { .. } => "Move",
            Dialog::CopyTo { .. } => "Copy",
            Dialog::DeletePermanently => "Delete Permanently",
            Dialog::Properties(_) | Dialog::About { .. } | Dialog::Conflict { .. } => {
                return self.emit_full().map(|message| vec![message]);
            }
        };
        let plan = match dialog {
            Dialog::CreateFolder { value } => FileControls::create_directory(
                request.get(),
                &self.home,
                &self.location().clone(),
                &value,
            ),
            Dialog::Rename { value } => FileControls::rename(
                request.get(),
                &self.home,
                sources.into_iter().next(),
                &value,
            ),
            Dialog::MoveTo { value } => FileControls::move_to(
                request.get(),
                &self.home,
                &self.location().clone(),
                sources,
                &value,
            ),
            Dialog::CopyTo { value } => FileControls::copy_to(
                request.get(),
                &self.home,
                &self.location().clone(),
                sources,
                &value,
            ),
            Dialog::DeletePermanently => FileControls::delete(request.get(), &self.home, sources),
            Dialog::Properties(_) | Dialog::About { .. } | Dialog::Conflict { .. } => unreachable!(),
        };
        match plan {
            Ok(plan) => self.begin_mutation(label, &plan, request),
            Err(notice) => self.emit_notice(&notice),
        }
    }
}
