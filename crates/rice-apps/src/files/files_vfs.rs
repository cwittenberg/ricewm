//! VFS request state machines for directory reads and file mutations.

use super::{FilesActorError, PendingDirectory, RiceFiles};
use rice_files::{
    ConflictAction, ConflictDecision, ConflictStrategy, GuestPath, OperationPlan, PaneSlot,
    ProgressEvent, ProgressState,
};
use rice_native_app_protocol::{
    AppId, Message, RequestId, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

use super::files_mutation_recovery::{RecoveryAdvance, ReplacementRecovery};
use super::files_mutation_steps::{MutationStep, StepExpectation, destination_of, steps_from_plan};
use super::files_progress::ProgressView;

#[path = "files_vfs_completion.rs"]
mod completion;
use completion::bounded_error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingMutation {
    request: RequestId,
    label: &'static str,
    steps: Vec<MutationStep>,
    position: usize,
    progress: ProgressState,
    conflicts: ConflictStrategy,
    replacement: Option<ReplacementRecovery>,
    cancel_request: Option<RequestId>,
    cancel_acknowledged: bool,
    cancel_terminal: bool,
    progress_sequence: u32,
    byte_progress: Option<(u64, u64)>,
    unavailable_source_paths: Vec<GuestPath>,
}

pub(super) enum MutationAdvance {
    Next(Message, String),
    Complete(String),
    Cancelled(String),
    Failed(String),
}

impl PendingMutation {
    fn new(
        app: AppId,
        request: RequestId,
        label: &'static str,
        plan: &OperationPlan,
    ) -> Result<(Self, Message), FilesActorError> {
        let steps = steps_from_plan(plan)?;
        if steps.is_empty() {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        let total = u64::try_from(steps.len()).map_err(|_| FilesActorError::EntryLimit)?;
        let mut progress = ProgressState::new(plan.id(), total, None);
        progress
            .apply(ProgressEvent::StartItem(steps[0].path.clone()))
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        let pending = Self {
            request,
            label,
            steps,
            position: 0,
            progress,
            conflicts: ConflictStrategy::default(),
            replacement: None,
            cancel_request: None,
            cancel_acknowledged: false,
            cancel_terminal: false,
            progress_sequence: 0,
            byte_progress: None,
            unavailable_source_paths: Vec::new(),
        };
        let message = pending.current_request(app);
        Ok((pending, message))
    }

    fn advance(
        &mut self,
        app: AppId,
        response: &VfsResponse,
    ) -> Result<MutationAdvance, FilesActorError> {
        if response.request != self.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        if self.replacement.is_some() {
            return self.advance_replacement(app, response);
        }
        if self.steps[self.position].expectation == StepExpectation::MissingDestination {
            return self.advance_missing_destination(app, response);
        }
        match &response.result {
            VfsResult::Done => {
                self.record_unavailable_source();
                self.complete_step(app)
            }
            VfsResult::Error(error) => {
                let detail = bounded_error(error.message.trim());
                self.progress
                    .apply(ProgressEvent::Fail(detail.clone()))
                    .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
                Ok(MutationAdvance::Failed(rice_i18n::format("{operation} stopped after {completed}/{total} items: {detail}", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string()), ("detail", &detail)])))
            }
            _ => {
                let detail = String::from(rice_i18n::t("VFS returned an invalid mutation response"));
                self.progress
                    .apply(ProgressEvent::Fail(detail.clone()))
                    .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
                Ok(MutationAdvance::Failed(rice_i18n::format("{operation} stopped after {completed}/{total} items: {detail}", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string()), ("detail", &detail)])))
            }
        }
    }

    fn advance_replacement(
        &mut self,
        app: AppId,
        response: &VfsResponse,
    ) -> Result<MutationAdvance, FilesActorError> {
        let cancel_requested = self.progress.status == rice_files::ProgressStatus::CancelRequested;
        let advance = self
            .replacement
            .as_mut()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?
            .advance(app, response, cancel_requested);
        match advance {
            RecoveryAdvance::Request(request, notice) => Ok(MutationAdvance::Next(request, notice)),
            RecoveryAdvance::Replaced => {
                self.replacement = None;
                self.record_unavailable_source();
                self.complete_step(app)
            }
            RecoveryAdvance::Cancelled(notice) => {
                self.replacement = None;
                self.progress
                    .apply(ProgressEvent::Cancelled)
                    .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
                Ok(MutationAdvance::Cancelled(notice))
            }
            RecoveryAdvance::Failed(notice) => {
                self.replacement = None;
                self.progress
                    .apply(ProgressEvent::Fail(notice.clone()))
                    .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
                Ok(MutationAdvance::Failed(notice))
            }
        }
    }

    fn advance_missing_destination(
        &mut self,
        app: AppId,
        response: &VfsResponse,
    ) -> Result<MutationAdvance, FilesActorError> {
        match &response.result {
            VfsResult::Error(error) if error.code == VfsErrorCode::NotFound => {
                self.complete_step(app)
            }
            VfsResult::Metadata(_) => Ok(MutationAdvance::Failed(rice_i18n::format("{operation} stopped: Trash already contains {path}", &[("operation", rice_i18n::t(self.label)), ("path", self.steps[self.position].path.as_str())]))),
            VfsResult::Error(error) => Ok(MutationAdvance::Failed(rice_i18n::format("{operation} stopped while checking Trash: {detail}", &[("operation", rice_i18n::t(self.label)), ("detail", &bounded_error(error.message.trim()))]))),
            _ => Ok(MutationAdvance::Failed(String::from(rice_i18n::t("Move to Trash stopped: invalid collision-check response")))),
        }
    }

    fn complete_step(&mut self, app: AppId) -> Result<MutationAdvance, FilesActorError> {
        self.progress
            .apply(ProgressEvent::CompleteItem)
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        self.position += 1;
        if self.position == self.steps.len() {
            self.progress
                .apply(ProgressEvent::Complete)
                .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
            return Ok(MutationAdvance::Complete(rice_i18n::format(if self.steps.len() == 1 { "{operation} completed for {count} item" } else { "{operation} completed for {count} items" }, &[("operation", rice_i18n::t(self.label)), ("count", &self.steps.len().to_string())])));
        }
        self.progress_sequence = 0;
        self.byte_progress = None;
        if self.progress.status == rice_files::ProgressStatus::CancelRequested {
            self.progress
                .apply(ProgressEvent::Cancelled)
                .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
            return Ok(MutationAdvance::Cancelled(rice_i18n::format("{operation} cancelled after {completed}/{total} items", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string())])));
        }
        self.progress
            .apply(ProgressEvent::StartItem(
                self.steps[self.position].path.clone(),
            ))
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        let notice = rice_i18n::format("{operation}: {completed}/{total} complete; processing {path}", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string()), ("path", self.steps[self.position].path.as_str())]);
        Ok(MutationAdvance::Next(self.current_request(app), notice))
    }

    fn record_unavailable_source(&mut self) {
        let source = match &self.steps[self.position].operation {
            VfsOperation::Rename { from, .. } | VfsOperation::Remove { path: from, .. } => {
                GuestPath::absolute(from).ok()
            }
            _ => None,
        };
        if let Some(source) = source
            && !self.unavailable_source_paths.contains(&source)
        {
            self.unavailable_source_paths.push(source);
        }
    }

    fn current_request(&self, app: AppId) -> Message {
        Message::VfsRequest(VfsRequest {
            request: self.request,
            app,
            operation: self.steps[self.position].operation.clone(),
        })
    }

    pub(super) fn view(&self) -> ProgressView<'_> {
        ProgressView {
            label: self.label,
            completed: self.progress.completed_items,
            total: self.progress.total_items,
            current: self
                .progress
                .current_path
                .as_ref()
                .map_or("", GuestPath::as_str),
            cancel_requested: self.progress.status == rice_files::ProgressStatus::CancelRequested,
            byte_progress: self.byte_progress,
        }
    }

    fn request_cancel(
        &mut self,
        app: AppId,
        request: RequestId,
    ) -> Result<Message, FilesActorError> {
        if request == self.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        self.progress
            .request_cancel()
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        self.cancel_request = Some(request);
        Ok(Message::VfsRequest(VfsRequest {
            request,
            app,
            operation: VfsOperation::Cancel {
                target: self.request,
            },
        }))
    }

    fn apply_byte_progress(
        &mut self,
        sequence: u32,
        completed: u64,
        total: u64,
    ) -> Result<(), FilesActorError> {
        if sequence == 0
            || sequence <= self.progress_sequence
            || completed > total
            || self
                .byte_progress
                .is_some_and(|(previous, expected)| completed < previous || total != expected)
        {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        self.progress_sequence = sequence;
        self.byte_progress = Some((completed, total));
        Ok(())
    }

    fn cancel_denied(&mut self) -> Result<(), FilesActorError> {
        self.progress
            .apply(ProgressEvent::CancelDenied)
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        self.cancel_request = None;
        self.cancel_acknowledged = false;
        Ok(())
    }

    fn mark_cancelled(&mut self) -> Result<(), FilesActorError> {
        self.progress
            .apply(ProgressEvent::Cancelled)
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        self.cancel_terminal = true;
        Ok(())
    }

    fn conflict_destination(&self, response: &VfsResponse) -> Option<GuestPath> {
        if !matches!(
            &response.result,
            VfsResult::Error(error) if error.code == VfsErrorCode::AlreadyExists
        ) {
            return None;
        }
        destination_of(&self.steps[self.position].operation)
    }

    pub(super) fn prepare_replace(
        &mut self,
        app: AppId,
        apply_all: bool,
    ) -> Result<Message, FilesActorError> {
        let current = self.steps[self.position].clone();
        let destination =
            destination_of(&current.operation).ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if apply_all {
            self.conflicts
                .decide(
                    &destination,
                    ConflictDecision::ApplyAll(ConflictAction::Replace),
                )
                .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        }
        let recovery =
            ReplacementRecovery::new(self.request, self.position, current.operation, destination)?;
        let request = recovery.initial_request(app);
        self.replacement = Some(recovery);
        Ok(request)
    }

    pub(super) fn rename_conflict(
        &mut self,
        app: AppId,
        destination: &GuestPath,
    ) -> Result<Message, FilesActorError> {
        let current_destination = destination_of(&self.steps[self.position].operation)
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        self.conflicts
            .decide(
                &current_destination,
                ConflictDecision::Rename(destination.clone()),
            )
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        let operation = &mut self.steps[self.position].operation;
        match operation {
            VfsOperation::Rename { to, .. }
            | VfsOperation::Copy { to, .. }
            | VfsOperation::Link { to, .. } => {
                *to = destination.as_str().to_owned();
                Ok(self.current_request(app))
            }
            VfsOperation::Archive {
                destination: to, ..
            } => {
                *to = destination.as_str().to_owned();
                Ok(self.current_request(app))
            }
            _ => Err(FilesActorError::UnexpectedVfsResponse),
        }
    }

    pub(super) fn cancel_conflict(&mut self) -> Result<String, FilesActorError> {
        self.progress
            .request_cancel()
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        self.progress
            .apply(ProgressEvent::Cancelled)
            .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        Ok(rice_i18n::format("{operation} cancelled after {completed}/{total} items", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string())]))
    }

    pub(super) fn skip_conflict(
        &mut self,
        app: AppId,
        apply_all: bool,
    ) -> Result<MutationAdvance, FilesActorError> {
        if apply_all {
            let destination = destination_of(&self.steps[self.position].operation)
                .ok_or(FilesActorError::UnexpectedVfsResponse)?;
            self.conflicts
                .decide(
                    &destination,
                    ConflictDecision::ApplyAll(ConflictAction::Skip),
                )
                .map_err(|_| FilesActorError::UnexpectedVfsResponse)?;
        }
        let advance = self.complete_step(app)?;
        Ok(match advance {
            MutationAdvance::Next(message, _) => MutationAdvance::Next(
                message,
                rice_i18n::format("{operation}: conflict skipped; {completed}/{total} items processed", &[("operation", rice_i18n::t(self.label)), ("completed", &self.progress.completed_items.to_string()), ("total", &self.progress.total_items.to_string())]),
            ),
            MutationAdvance::Complete(_) => MutationAdvance::Complete(rice_i18n::format("{operation} finished; the conflicting item was skipped", &[("operation", rice_i18n::t(self.label))])),
            other => other,
        })
    }

    pub(super) const fn request_id(&self) -> RequestId {
        self.request
    }

    pub(super) fn archive_destination(&self) -> Option<GuestPath> {
        self.steps.iter().find_map(|step| match &step.operation {
            VfsOperation::Archive { destination, .. } => GuestPath::absolute(destination).ok(),
            _ => None,
        })
    }

    pub(super) fn unavailable_source_paths(&self) -> Vec<GuestPath> {
        self.unavailable_source_paths.clone()
    }

    fn automatic_conflict(&self) -> Option<ConflictAction> {
        self.conflicts.automatic().cloned()
    }
}

impl RiceFiles {
    #[cfg(test)]
    pub(crate) fn handle_vfs_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.handle_vfs_response_with_request(response, response.request)
    }

    pub(crate) fn handle_vfs_response_with_request(
        &mut self,
        response: &VfsResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(index) = self
            .retired_directory_requests
            .iter()
            .position(|request| *request == response.request)
        {
            self.retired_directory_requests.remove(index);
            return Ok(Vec::new());
        }
        if self.owns_text_preview_request(response.request) {
            return self.handle_text_preview_response(response);
        }
        if self.dialog.as_ref().is_some_and(|dialog| {
            matches!(
                dialog,
                super::files_dialogs::Dialog::Properties(state)
                    if state.awaits(response.request)
            )
        }) {
            return self.handle_properties_response(response);
        }
        if let Some(index) = self
            .abandoned_property_requests
            .iter()
            .position(|request| *request == response.request)
        {
            self.abandoned_property_requests.remove(index);
            return Ok(Vec::new());
        }
        if self.retired_mutation_requests.contains(&response.request) {
            return Ok(Vec::new());
        }
        if self
            .pending_folder_tree_probe
            .as_ref()
            .is_some_and(|pending| pending.request_id() == response.request)
        {
            return self.handle_folder_tree_probe_response(response);
        }
        if self.slot_for_search_request(response.request).is_some() {
            return self.handle_search_response(response, next_request);
        }
        if self.slot_for_tree_request(response.request).is_some() {
            return self.handle_tree_response(response);
        }
        if self
            .pending_external_drop
            .as_ref()
            .is_some_and(|pending| pending.request_id() == response.request)
        {
            return self.handle_external_drop_response(response);
        }
        if self.pending_mutation.as_ref().is_some_and(|pending| {
            pending.request == response.request || pending.cancel_request == Some(response.request)
        }) {
            return self.handle_mutation_response(response);
        }
        if let Ok(slot) = self.slot_for_directory_request(response.request) {
            let complete = matches!(
                response.result,
                VfsResult::Directory {
                    next_cursor: None,
                    ..
                }
            );
            let mut messages = self.handle_directory_response(response, next_request)?;
            if complete {
                let tab = self.active_tab_id()?;
                let probe_candidates = self
                    .runtime_at(tab, slot)?
                    .directory_entries
                    .iter()
                    .filter(|entry| entry.kind == rice_files::EntryKind::Directory)
                    .map(|entry| entry.path.clone())
                    .collect::<Vec<_>>();
                messages.extend(self.begin_folder_tree_probes(probe_candidates, next_request));
                let mut entries = self
                    .runtime_at(tab, PaneSlot::Primary)?
                    .directory_entries
                    .clone();
                if self.tab()?.dual_pane {
                    entries.extend(
                        self.runtime_at(tab, PaneSlot::Secondary)?
                            .directory_entries
                            .iter()
                            .cloned(),
                    );
                }
                if let Some(thumbnail) = self.reset_thumbnail_batch(&entries, next_request, true) {
                    messages.push(thumbnail);
                }
            }
            return Ok(messages);
        }
        Err(FilesActorError::UnexpectedVfsResponse)
    }

    fn handle_mutation_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let mut pending = self
            .pending_mutation
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if pending.cancel_request == Some(response.request) {
            return self.handle_cancel_ack(pending, response);
        }
        if let VfsResult::Progress(progress) = response.result {
            pending.apply_byte_progress(
                progress.sequence,
                progress.completed_bytes,
                progress.total_bytes,
            )?;
            self.pending_mutation = Some(pending);
            return self.emit_full().map(|message| vec![message]);
        }
        if matches!(response.result, VfsResult::Cancelled) {
            if pending.cancel_request.is_none() {
                return Err(FilesActorError::UnexpectedVfsResponse);
            }
            pending.mark_cancelled()?;
            if pending.cancel_acknowledged {
                return self.finish_provider_cancellation(pending);
            }
            self.pending_mutation = Some(pending);
            return self.emit_full().map(|message| vec![message]);
        }
        if let Some(destination) = pending.conflict_destination(response) {
            if let Some(action) = pending.automatic_conflict() {
                let request = pending.request_id();
                return match action {
                    ConflictAction::Skip => {
                        self.download_after_mutation = false;
                        let advance = pending.skip_conflict(self.app, false)?;
                        self.apply_mutation_advance(pending, advance, request)
                    }
                    ConflictAction::Replace => {
                        let request_message = pending.prepare_replace(self.app, false)?;
                        self.pending_mutation = Some(pending);
                        Ok(vec![self.emit_full()?, request_message])
                    }
                    ConflictAction::Rename(_) => Err(FilesActorError::UnexpectedVfsResponse),
                };
            }
            self.dialog = Some(super::files_dialogs::Dialog::Conflict {
                destination: destination.as_str().to_owned(),
                rename: destination.as_str().to_owned(),
                apply_all: false,
            });
            self.pending_mutation = Some(pending);
            return self.emit_full().map(|message| vec![message]);
        }
        let advance = pending.advance(self.app, response)?;
        self.apply_mutation_advance(pending, advance, response.request)
    }

    fn handle_cancel_ack(
        &mut self,
        mut pending: PendingMutation,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        match &response.result {
            VfsResult::Done => {
                pending.cancel_acknowledged = true;
                if pending.cancel_terminal {
                    return self.finish_provider_cancellation(pending);
                }
                self.pending_mutation = Some(pending);
                self.emit_full().map(|message| vec![message])
            }
            VfsResult::Error(error) if !pending.cancel_terminal => {
                let reason = bounded_error(error.message.trim());
                pending.cancel_denied()?;
                let _ = reason;
                self.pending_mutation = Some(pending);
                self.emit_full().map(|message| vec![message])
            }
            VfsResult::Error(_) if pending.cancel_terminal => {
                self.finish_provider_cancellation(pending)
            }
            _ => Err(FilesActorError::UnexpectedVfsResponse),
        }
    }

    fn finish_provider_cancellation(
        &mut self,
        pending: PendingMutation,
    ) -> Result<Vec<Message>, FilesActorError> {
        let refresh_request = pending
            .cancel_request
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        self.retire_mutation_request(pending.request);
        let notice = rice_i18n::format("{operation} cancelled after {completed}/{total} items", &[("operation", rice_i18n::t(pending.label)), ("completed", &pending.progress.completed_items.to_string()), ("total", &pending.progress.total_items.to_string())]);
        self.apply_mutation_advance(pending, MutationAdvance::Cancelled(notice), refresh_request)
    }
}
