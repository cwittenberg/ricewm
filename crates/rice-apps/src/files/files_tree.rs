//! Paged detail expansion and real folder-tree child discovery.

use std::collections::{BTreeSet, VecDeque};

use rice_files::{EntryKind, EntryMetadata, GuestPath, PaneSlot, ViewMode};
use rice_native_app_protocol::{DirectoryEntry, Message, RequestId, VfsResponse, VfsResult};

use super::{FilesActorError, RiceFiles};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingTreeDirectory {
    request: RequestId,
    target: GuestPath,
    cursor: u64,
    entries: Vec<DirectoryEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingFolderTreeProbe {
    request: RequestId,
    target: GuestPath,
    cursor: u64,
    entries: Vec<DirectoryEntry>,
    queue: VecDeque<GuestPath>,
}

impl PendingFolderTreeProbe {
    pub(super) const fn request_id(&self) -> RequestId {
        self.request
    }
}

impl PendingTreeDirectory {
    pub(super) const fn request_id(&self) -> RequestId {
        self.request
    }
}

impl RiceFiles {
    pub(super) fn cancel_folder_tree_probe(&mut self) {
        if let Some(pending) = self.pending_folder_tree_probe.take()
            && !self.retired_directory_requests.contains(&pending.request)
        {
            self.retired_directory_requests.push(pending.request);
            if self.retired_directory_requests.len() > 16 {
                self.retired_directory_requests.remove(0);
            }
        }
    }

    pub(super) fn begin_folder_tree_probes(
        &mut self,
        candidates: impl IntoIterator<Item = GuestPath>,
        request: RequestId,
    ) -> Vec<Message> {
        if !self.folder_tree || self.pending_folder_tree_probe.is_some() {
            return Vec::new();
        }
        let mut seen = BTreeSet::new();
        let mut queue = candidates
            .into_iter()
            .filter(|path| !self.tree_directory_cache.contains_key(path))
            .filter(|path| seen.insert(path.clone()))
            .collect::<VecDeque<_>>();
        let Some(target) = queue.pop_front() else {
            return Vec::new();
        };
        self.pending_folder_tree_probe = Some(PendingFolderTreeProbe {
            request,
            target: target.clone(),
            cursor: 0,
            entries: Vec::new(),
            queue,
        });
        vec![self.directory_request(PaneSlot::Primary, request, &target, 0)]
    }

    pub(super) fn probe_folder_tree_children(
        &mut self,
        path: &GuestPath,
        request: RequestId,
    ) -> Vec<Message> {
        let candidates = self
            .tree_directory_cache
            .get(path)
            .into_iter()
            .flatten()
            .filter(|entry| entry.kind == EntryKind::Directory)
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        self.begin_folder_tree_probes(candidates, request)
    }

    pub(super) fn handle_folder_tree_probe_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let mut pending = self
            .pending_folder_tree_probe
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if pending.request != response.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
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
                    self.pending_folder_tree_probe = Some(pending);
                    return Ok(vec![self.directory_request(
                        PaneSlot::Primary,
                        response.request,
                        &target,
                        *next,
                    )]);
                }
                let mut converted = super::convert_entries(&pending.target, &pending.entries)?;
                converted.retain(|entry| entry.kind == EntryKind::Directory);
                converted.sort_by(|left, right| left.path.cmp(&right.path));
                self.tree_directory_cache
                    .insert(pending.target.clone(), converted);
                let mut messages = vec![self.emit_full()?];
                while let Some(target) = pending.queue.pop_front() {
                    if self.tree_directory_cache.contains_key(&target) {
                        continue;
                    }
                    self.pending_folder_tree_probe = Some(PendingFolderTreeProbe {
                        request: response.request,
                        target: target.clone(),
                        cursor: 0,
                        entries: Vec::new(),
                        queue: pending.queue,
                    });
                    messages.push(self.directory_request(
                        PaneSlot::Primary,
                        response.request,
                        &target,
                        0,
                    ));
                    break;
                }
                Ok(messages)
            }
            VfsResult::Error(_) => {
                self.tree_directory_cache
                    .insert(pending.target.clone(), Vec::new());
                let mut messages = vec![self.emit_full()?];
                if let Some(target) = pending.queue.pop_front() {
                    self.pending_folder_tree_probe = Some(PendingFolderTreeProbe {
                        request: response.request,
                        target: target.clone(),
                        cursor: 0,
                        entries: Vec::new(),
                        queue: pending.queue,
                    });
                    messages.push(self.directory_request(
                        PaneSlot::Primary,
                        response.request,
                        &target,
                        0,
                    ));
                }
                Ok(messages)
            }
            _ => Err(FilesActorError::UnexpectedVfsResponse),
        }
    }

    pub(super) fn visible_entries(&self) -> Vec<EntryMetadata> {
        self.visible_entries_at(self.active_slot().unwrap_or(PaneSlot::Primary))
    }

    pub(super) fn visible_entries_at(&self, slot: PaneSlot) -> Vec<EntryMetadata> {
        let mut visible = Vec::new();
        if let Ok(runtime) = self.runtime_at(self.active_tab_id().unwrap_or(1), slot) {
            append_visible(
                &runtime.entries,
                &runtime.expanded_directories,
                &mut visible,
                self.options.show_hidden,
            );
        }
        visible.truncate(rice_native_app_protocol::MAX_DIRECTORY_ENTRIES);
        visible
    }

    pub(super) fn toggle_detail_directory(
        &mut self,
        entry: EntryMetadata,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pane()?.view != ViewMode::Details || entry.kind != EntryKind::Directory {
            return self.open_selected(request);
        }
        let tab = self.active_tab_id()?;
        let slot = self.active_slot()?;
        if self
            .runtime_at_mut(tab, slot)?
            .expanded_directories
            .remove(&entry.path)
            .is_some()
        {
            self.runtime_at_mut(tab, slot)?.content_notice = format!("Collapsed {}", entry.path);
            return self.emit_full().map(|message| vec![message]);
        }
        if self.runtime()?.pending_tree_directory.is_some()
            || self.runtime()?.pending_directory.is_some()
            || self.pending_mutation.is_some()
        {
            return self.emit_notice(rice_i18n::t("Wait for the current filesystem request to finish"));
        }
        self.runtime_mut()?.pending_tree_directory = Some(PendingTreeDirectory {
            request,
            target: entry.path.clone(),
            cursor: 0,
            entries: Vec::new(),
        });
        self.runtime_mut()?.content_notice = format!("Expanding {}…", entry.path);
        Ok(vec![
            self.emit_full()?,
            self.directory_request(slot, request, &entry.path, 0),
        ])
    }

    pub(super) fn handle_tree_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let slot = self
            .slot_for_tree_request(response.request)
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        let tab = self.active_tab_id()?;
        let mut pending = self
            .runtime_at_mut(tab, slot)?
            .pending_tree_directory
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if pending.request != response.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
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
                    self.runtime_at_mut(tab, slot)?.pending_tree_directory = Some(pending);
                    return Ok(vec![
                        self.emit_full()?,
                        self.directory_request(slot, response.request, &target, *next),
                    ]);
                }
                let mut converted = super::convert_entries(&pending.target, &pending.entries)?;
                let runtime = self.runtime_at_mut(tab, slot)?;
                rice_files::sort_entries(&mut converted, runtime.sort);
                runtime
                    .expanded_directories
                    .insert(pending.target.clone(), converted);
                runtime.content_notice = String::new();
                self.emit_full().map(|message| vec![message])
            }
            VfsResult::Error(error) => {
                let notice = format!("Cannot expand {}: {}", pending.target, error.message);
                self.runtime_at_mut(tab, slot)?.content_notice.clear();
                Ok(vec![
                    self.emit_full()?,
                    self.system_notification("error", &notice),
                ])
            }
            _ => Err(FilesActorError::UnexpectedVfsResponse),
        }
    }
}

fn append_visible(
    entries: &[EntryMetadata],
    expanded: &std::collections::BTreeMap<GuestPath, Vec<EntryMetadata>>,
    output: &mut Vec<EntryMetadata>,
    show_hidden: bool,
) {
    for entry in entries {
        if !show_hidden && entry.name.starts_with('.') { continue; }
        if output.len() == rice_native_app_protocol::MAX_DIRECTORY_ENTRIES {
            return;
        }
        output.push(entry.clone());
        if let Some(children) = expanded.get(&entry.path) {
            append_visible(children, expanded, output, show_hidden);
        }
    }
}
