//! Starting, cancelling, and completing mutation batches.

use super::*;

impl RiceFiles {
    pub(in crate::files) fn begin_mutation(
        &mut self,
        label: &'static str,
        plan: &OperationPlan,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.runtime()?.pending_directory.is_some() || self.pending_mutation.is_some() {
            return self.emit_notice(rice_i18n::t("Another VFS request is already in progress"));
        }
        let slot = self.active_slot()?;
        let (pending, request_message) = PendingMutation::new(self.app, request, label, plan)?;
        // Keep the directory view stable while the dedicated progress panel
        // reports mutation state below it.
        self.runtime_mut()?.content_notice.clear();
        self.pending_mutation = Some(pending);
        self.pending_mutation_pane = Some(slot);
        let patch = self.emit_full()?;
        Ok(vec![patch, request_message])
    }

    pub(in crate::files) fn cancel_mutation(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(pending) = self.pending_mutation.as_mut() else {
            return Ok(Vec::new());
        };
        let cancel = pending.request_cancel(self.app, request)?;
        Ok(vec![self.emit_full()?, cancel])
    }

    pub(in crate::files) fn apply_mutation_advance(
        &mut self,
        pending: PendingMutation,
        advance: MutationAdvance,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let slot = self
            .pending_mutation_pane
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        let tab = self.active_tab_id()?;
        match advance {
            MutationAdvance::Next(request, _notice) => {
                self.pending_mutation = Some(pending);
                Ok(vec![self.emit_full()?, request])
            }
            MutationAdvance::Failed(notice) => {
                self.download_after_mutation = false;
                self.pending_mutation_pane = None;
                self.runtime_at_mut(tab, slot)?.content_notice.clear();
                Ok(vec![
                    self.emit_full()?,
                    self.system_notification("error", &notice),
                ])
            }
            MutationAdvance::Cancelled(notice) => {
                self.download_after_mutation = false;
                self.rice_bus_deferred_refresh[match slot {
                    PaneSlot::Primary => 0,
                    PaneSlot::Secondary => 1,
                }] = false;
                let target = self.tab()?.pane(slot).location().clone();
                self.runtime_at_mut(tab, slot)?.pending_directory = Some(PendingDirectory::new(
                    request,
                    target.clone(),
                    None,
                    Some(notice.clone()),
                ));
                self.pending_mutation_pane = None;
                self.runtime_at_mut(tab, slot)?.content_notice.clear();
                Ok(vec![
                    self.emit_full()?,
                    self.directory_request(slot, request, &target, 0),
                    self.system_notification("info", &notice),
                ])
            }
            MutationAdvance::Complete(notice) => {
                self.rice_bus_deferred_refresh[match slot {
                    PaneSlot::Primary => 0,
                    PaneSlot::Secondary => 1,
                }] = false;
                let unavailable_paths = pending.unavailable_source_paths();
                let download = self
                    .download_after_mutation
                    .then(|| pending.archive_destination())
                    .flatten();
                self.download_after_mutation = false;
                let target = self.tab()?.pane(slot).location().clone();
                self.runtime_at_mut(tab, slot)?.pending_directory = Some(PendingDirectory::new(
                    request,
                    target.clone(),
                    None,
                    Some(notice.clone()),
                ));
                self.pending_mutation_pane = None;
                self.runtime_at_mut(tab, slot)?.content_notice.clear();
                let mut messages = crate::files::files_preferences::remove_unavailable_paths(
                    self,
                    &unavailable_paths,
                    request,
                )?;
                if messages.is_empty() {
                    messages.push(self.emit_full()?);
                }
                messages.push(self.directory_request(slot, request, &target, 0));
                messages.push(self.system_notification("info", &notice));
                if let Some(path) = download {
                    messages.extend(crate::files::files_download::completed(self, path, request));
                }
                Ok(messages)
            }
        }
    }

    pub(super) fn retire_mutation_request(&mut self, request: RequestId) {
        if self.retired_mutation_requests.contains(&request) {
            return;
        }
        self.retired_mutation_requests.push(request);
        if self.retired_mutation_requests.len() > 16 {
            self.retired_mutation_requests.remove(0);
        }
    }
}

pub(super) fn bounded_error(message: &str) -> String {
    let message = if message.is_empty() {
        rice_i18n::t("VFS operation failed")
    } else {
        message
    };
    let mut end = message.len().min(1024);
    while !message.is_char_boundary(end) {
        end -= 1;
    }
    message[..end].to_owned()
}
