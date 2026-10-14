//! Paged directory enumeration and atomic location commits.

use rice_files::{GuestPath, NavigationPlan, PaneSlot};
use rice_native_app_protocol::{
    DirectoryEntry, Message, RequestId, VfsErrorCode, VfsResponse, VfsResult,
};

use super::{FilesActorError, RiceFiles};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingDirectory {
    pub request: RequestId,
    pub target: GuestPath,
    pub navigation: Option<NavigationPlan>,
    pub completion_notice: Option<String>,
    cursor: u64,
    entries: Vec<DirectoryEntry>,
}

impl PendingDirectory {
    pub(super) fn loaded_count(&self) -> usize {self.entries.len()}
    pub(super) fn new(
        request: RequestId,
        target: GuestPath,
        navigation: Option<NavigationPlan>,
        completion_notice: Option<String>,
    ) -> Self {
        Self {
            request,
            target,
            navigation,
            completion_notice,
            cursor: 0,
            entries: Vec::new(),
        }
    }
}

impl RiceFiles {
    pub(super) fn retire_pending_directory(
        &mut self,
        slot: PaneSlot,
    ) -> Result<(), FilesActorError> {
        let tab = self.active_tab_id()?;
        let Some(pending) = self.runtime_at_mut(tab, slot)?.pending_directory.take() else {
            return Ok(());
        };
        if !self.retired_directory_requests.contains(&pending.request) {
            self.retired_directory_requests.push(pending.request);
            if self.retired_directory_requests.len() > 16 {
                self.retired_directory_requests.remove(0);
            }
        }
        Ok(())
    }

    pub(super) fn handle_directory_response(
        &mut self,
        response: &VfsResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let slot = self.slot_for_directory_request(response.request)?;
        let tab = self.active_tab_id()?;
        let mut pending = self
            .runtime_at_mut(tab, slot)?
            .pending_directory
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        // A failed refresh must not keep claiming that the target is available.
        // Failed navigation to another target still retains the previous folder.
        if self.runtime_at(tab, slot)?.loaded_location.as_ref() == Some(&pending.target) {
            self.runtime_at_mut(tab, slot)?.loaded_location = None;
        }
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                if pending.entries.len().saturating_add(entries.len())
                    > rice_native_app_protocol::MAX_DIRECTORY_ENTRIES
                {
                    return Err(FilesActorError::EntryLimit);
                }
                pending.entries.extend_from_slice(entries);
                if let Some(next) = next_cursor {
                    if *next == pending.cursor {
                        return Err(FilesActorError::UnexpectedVfsResponse);
                    }
                    pending.cursor = *next;
                    let target = pending.target.clone();
                    self.runtime_at_mut(tab, slot)?.content_notice =
                        format!("Loading {target}… {} items received", pending.entries.len());
                    self.runtime_at_mut(tab, slot)?.pending_directory = Some(pending);
                    return Ok(vec![
                        self.emit_full()?,
                        self.directory_request(slot, response.request, &target, *next),
                    ]);
                }
                let entries = core::mem::take(&mut pending.entries);
                self.apply_directory(slot, &pending, &entries, next_request)
            }
            VfsResult::Error(error) => {
                let location = self.tab()?.pane(slot).location().as_str().to_owned();
                let notice = if let Some(completed) = pending.completion_notice {
                    format!(
                        "{completed}, but refresh of {} failed: {}",
                        pending.target,
                        error.message.trim()
                    )
                } else {
                    format!("Cannot open {}: {}", pending.target, error.message.trim())
                };
                let runtime = self.runtime_at_mut(tab, slot)?;
                location.clone_into(&mut runtime.location_text);
                runtime.completion = rice_files::DirectoryCompletion::default();
                runtime.completion_index = None;
                runtime.content_notice.clear();
                let unavailable = (error.code == VfsErrorCode::NotFound)
                    .then(|| pending.target.clone())
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut messages = super::files_preferences::remove_unavailable_paths(
                    self,
                    &unavailable,
                    response.request,
                )?;
                if messages.is_empty() {
                    messages.push(self.emit_full()?);
                }
                messages.push(self.system_notification("error", &notice));
                Ok(messages)
            }
            _ => {
                let notice = "VFS returned an invalid directory response";
                self.runtime_at_mut(tab, slot)?.content_notice.clear();
                Ok(vec![
                    self.emit_full()?,
                    self.system_notification("error", notice),
                ])
            }
        }
    }

    fn apply_directory(
        &mut self,
        slot: PaneSlot,
        pending: &PendingDirectory,
        entries: &[DirectoryEntry],
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let converted = super::convert_entries(&pending.target, entries)?;
        if pending.target.as_str() == "/root/.local/share/Trash/files" {
            self.trash_nonempty = !converted.is_empty();
        }
        self.tree_directory_cache.insert(
            pending.target.clone(),
            converted
                .iter()
                .filter(|entry| entry.kind == rice_files::EntryKind::Directory)
                .cloned()
                .collect(),
        );
        if let Some(plan) = &pending.navigation {
            self.tab_mut()?
                .pane_mut(slot)
                .commit_navigation(plan, pending.target.clone())?;
        }
        if self.redmond98 && !self.redmond_view_explicit {
            self.tab_mut()?
                .pane_mut(slot)
                .set_view(rice_files::ViewMode::Icons);
        }
        let tab = self.active_tab_id()?;
        let runtime = self.runtime_at_mut(tab, slot)?;
        pending
            .target
            .as_str()
            .clone_into(&mut runtime.location_text);
        runtime.completion = rice_files::DirectoryCompletion::default();
        runtime.completion_index = None;
        runtime.directory_entries = converted;
        runtime.loaded_location = Some(pending.target.clone());
        runtime.content_notice.clear();
        runtime.expanded_directories.clear();
        rice_files::sort_entries(&mut runtime.directory_entries, runtime.sort);
        // Mutation completion is already published through the host-owned
        // notification surface. Do not recreate it as RiceFiles' persistent
        // in-pane notice after the follow-up directory refresh.
        self.reapply_quick_filter_after_directory(slot, None)?;
        if slot == PaneSlot::Primary
            && pending.navigation.is_none()
            && let Some(selection) = self.launch_selection.take()
            && self
                .runtime_at(tab, slot)?
                .entries
                .iter()
                .any(|entry| entry.path == selection)
        {
            self.tab_mut()?
                .pane_mut(slot)
                .set_selection([selection.clone()]);
            let runtime = self.runtime_at_mut(tab, slot)?;
            runtime.selection_anchor = Some(selection.clone());
            runtime.selection_caret = Some(selection);
            if core::mem::take(&mut self.launch_open_with) {
                return self.show_open_with(next_request);
            }
        }
        if core::mem::take(&mut self.launch_open_with) {
            return self.emit_notice(rice_i18n::t("The file selected for Open With is no longer available"));
        }
        self.emit_full().map(|message| vec![message])
    }
}
