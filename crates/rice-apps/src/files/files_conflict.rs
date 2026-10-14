//! User-driven conflict decisions for pending file mutations.

use rice_files::GuestPath;
use rice_native_app_protocol::Message;

use super::{
    FilesActorError, RiceFiles,
    files_vfs::{MutationAdvance, PendingMutation},
};

impl RiceFiles {
    pub(super) fn skip_conflict(&mut self) -> Result<Vec<Message>, FilesActorError> {
        self.download_after_mutation = false;
        let apply_all = self.conflict_apply_all();
        let mut pending = self.take_conflict()?;
        let request = pending.request_id();
        let advance = pending.skip_conflict(self.app, apply_all)?;
        self.apply_mutation_advance(pending, advance, request)
    }

    pub(super) fn replace_conflict(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let apply_all = self.conflict_apply_all();
        let mut pending = self.take_conflict()?;
        let request = pending.prepare_replace(self.app, apply_all)?;
        self.runtime_mut()?.content_notice =
            String::from("Preserving the conflicting destination before replacement");
        self.pending_mutation = Some(pending);
        Ok(vec![self.emit_full()?, request])
    }

    pub(super) fn rename_conflict(&mut self, value: &str) -> Result<Vec<Message>, FilesActorError> {
        let destination = match GuestPath::absolute(value.trim()) {
            Ok(path) if path.is_within(&self.home) && !path.is_root() => path,
            _ => return self.emit_notice(rice_i18n::t("Choose an absolute replacement path inside /root")),
        };
        let Some(original) = self.conflict_destination() else {
            return Err(FilesActorError::UnexpectedVfsResponse);
        };
        if destination.parent() != original.parent() || destination == original {
            return self
                .emit_notice(rice_i18n::t("Choose a different replacement name in the same destination folder"));
        }
        let mut pending = self.take_conflict()?;
        let request = pending.rename_conflict(self.app, &destination)?;
        self.runtime_mut()?.content_notice = format!("Retrying with {destination}");
        self.pending_mutation = Some(pending);
        Ok(vec![self.emit_full()?, request])
    }

    pub(super) fn cancel_conflict(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let mut pending = self.take_conflict()?;
        let request = pending.request_id();
        let notice = pending.cancel_conflict()?;
        self.apply_mutation_advance(pending, MutationAdvance::Cancelled(notice), request)
    }

    fn take_conflict(&mut self) -> Result<PendingMutation, FilesActorError> {
        self.dialog = None;
        self.pending_mutation
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)
    }

    fn conflict_apply_all(&self) -> bool {
        matches!(
            self.dialog,
            Some(super::files_dialogs::Dialog::Conflict {
                apply_all: true,
                ..
            })
        )
    }

    fn conflict_destination(&self) -> Option<GuestPath> {
        match &self.dialog {
            Some(super::files_dialogs::Dialog::Conflict { destination, .. }) => {
                GuestPath::absolute(destination).ok()
            }
            _ => None,
        }
    }
}
