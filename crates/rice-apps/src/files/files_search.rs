//! Bounded, recursive, cancellable VFS search with incremental results.

use std::collections::{BTreeSet, VecDeque};

use rice_files::{EntryKind, EntryMetadata, GuestPath, infer_media_type, sort_entries};
use rice_native_app_protocol::{Message, RequestId, VfsResponse, VfsResult};

use super::{FilesActorError, RiceFiles};

const MAX_RESULTS: usize = rice_native_app_protocol::MAX_DIRECTORY_ENTRIES;
const MAX_DIRECTORIES: usize = rice_native_app_protocol::MAX_DIRECTORY_ENTRIES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingSearch {
    request: RequestId,
    filters: SearchFilters,
    queue: VecDeque<GuestPath>,
    visited: BTreeSet<GuestPath>,
    cursor: u64,
    results: Vec<EntryMetadata>,
    unreadable: usize,
    cancelled: bool,
}

impl PendingSearch {
    pub(super) const fn request_id(&self) -> RequestId {
        self.request
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct SearchFilters {
    name: String,
    kind: Option<String>,
    minimum_size: Option<u64>,
    maximum_size: Option<u64>,
    after_millis: Option<i64>,
    before_millis: Option<i64>,
}

impl RiceFiles {
    /// Applies the search field to the already loaded folder immediately.
    ///
    /// This is deliberately local: typing in the file surface should feel
    /// instantaneous, while Enter retains the existing bounded recursive
    /// search behavior below.
    pub(super) fn update_quick_filter(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        const MAX_QUICK_FILTER_BYTES: usize = 256;
        let mut bounded = value.to_owned();
        while bounded.len() > MAX_QUICK_FILTER_BYTES {
            bounded.pop();
        }
        if !bounded.is_empty() {
            self.search_expanded = true;
        }
        self.runtime_mut()?.controls.search_text = bounded;
        if let Some(search) = self.runtime_mut()?.pending_search.as_mut() {
            search.cancelled = true;
        }
        self.apply_quick_filter()?;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn append_quick_filter(
        &mut self,
        text: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let mut query = self.runtime()?.controls.search_text.clone();
        query.push_str(text);
        self.update_quick_filter(&query)
    }

    pub(super) fn backspace_quick_filter(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let mut query = self.runtime()?.controls.search_text.clone();
        query.pop();
        self.update_quick_filter(&query)
    }

    fn apply_quick_filter(&mut self) -> Result<(), FilesActorError> {
        self.apply_quick_filter_at(self.active_slot()?, None, true)
    }

    pub(super) fn reapply_quick_filter_after_directory(
        &mut self,
        slot: rice_files::PaneSlot,
        completion_notice: Option<&str>,
    ) -> Result<(), FilesActorError> {
        // A directory refresh keeps the current view. Navigation already resets
        // scroll in commit_navigation; typing a different filter resets it above.
        self.apply_quick_filter_at(slot, completion_notice, false)
    }

    fn apply_quick_filter_at(
        &mut self,
        slot: rice_files::PaneSlot,
        completion_notice: Option<&str>,
        reset_scroll: bool,
    ) -> Result<(), FilesActorError> {
        let tab = self.active_tab_id()?;
        let query = self
            .runtime_at(tab, slot)?
            .controls
            .search_text
            .trim()
            .to_owned();
        let mut entries = if query.is_empty() {
            self.runtime_at(tab, slot)?.directory_entries.clone()
        } else {
            let filters = SearchFilters::parse(&query);
            self.runtime_at(tab, slot)?
                .directory_entries
                .iter()
                .filter(|entry| filters.matches(entry))
                .cloned()
                .collect()
        };
        let sort = self.runtime_at(tab, slot)?.sort;
        sort_entries(&mut entries, sort);
        let count = entries.len();
        self.runtime_at_mut(tab, slot)?.entries = entries;
        let snapshot = self.runtime_at(tab, slot)?.entries.clone();
        self.tab_mut()?.pane_mut(slot).reconcile_entries(&snapshot);
        if reset_scroll {
            self.tab_mut()?
                .pane_mut(slot)
                .set_scroll(rice_files::ScrollPosition::default());
        }
        let filter_notice = if query.is_empty() {
            if count == 0 {
                String::from(rice_i18n::t("This folder is empty"))
            } else {
                String::new()
            }
        } else {
            rice_i18n::format(if count == 1 { "{count} immediate match for “{query}”" } else { "{count} immediate matches for “{query}”" }, &[("count", &count.to_string()), ("query", &query)])
        };
        self.runtime_at_mut(tab, slot)?.content_notice = match completion_notice
            .map(str::trim)
            .filter(|notice| !notice.is_empty())
        {
            Some(notice) if filter_notice.is_empty() => notice.to_owned(),
            Some(notice) => rice_i18n::format("{notice}; {filter_notice}", &[("notice", notice), ("filter_notice", &filter_notice)]),
            None => filter_notice,
        };
        Ok(())
    }

    pub(super) fn start_search(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if let Some(search) = self.runtime_mut()?.pending_search.as_mut() {
            search.cancelled = true;
            self.runtime_mut()?.content_notice =
                String::from(rice_i18n::t("Search cancellation requested; finishing the current folder page"));
            return self.emit_full().map(|message| vec![message]);
        }
        let query = self.runtime()?.controls.search_text.trim().to_owned();
        if query.is_empty() {
            let directory_entries = self.runtime()?.directory_entries.clone();
            self.runtime_mut()?.entries = directory_entries;
            let snapshot = self.runtime()?.entries.clone();
            self.pane_mut()?.reconcile_entries(&snapshot);
            self.runtime_mut()?.content_notice.clear();
            return self.emit_full().map(|message| vec![message]);
        }
        if self.runtime()?.pending_directory.is_some() || self.pending_mutation.is_some() {
            return self.emit_notice(rice_i18n::t("Wait for the current filesystem operation to finish"));
        }
        let filters = SearchFilters::parse(&query);
        let root = self.location().clone();
        let slot = self.active_slot()?;
        let mut queue = VecDeque::new();
        queue.push_back(root.clone());
        let mut visited = BTreeSet::new();
        visited.insert(root.clone());
        self.runtime_mut()?.entries.clear();
        self.pane_mut()?.set_selection([]);
        self.runtime_mut()?.pending_search = Some(PendingSearch {
            request,
            filters,
            queue,
            visited,
            cursor: 0,
            results: Vec::new(),
            unreadable: 0,
            cancelled: false,
        });
        self.runtime_mut()?.content_notice = rice_i18n::format("Searching {root}…", &[("root", root.as_str())]);
        Ok(vec![
            self.emit_full()?,
            self.directory_request(slot, request, &root, 0),
        ])
    }

    pub(super) fn handle_search_response(
        &mut self,
        response: &VfsResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let slot = self
            .slot_for_search_request(response.request)
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        let tab = self.active_tab_id()?;
        let mut search = self
            .runtime_at_mut(tab, slot)?
            .pending_search
            .take()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        if search.request != response.request {
            return Err(FilesActorError::UnexpectedVfsResponse);
        }
        if search.cancelled {
            self.runtime_at_mut(tab, slot)?.pending_search = None;
            if slot == self.active_slot()? {
                self.apply_quick_filter()?;
                let filtered = self.runtime()?.content_notice.clone();
                self.runtime_mut()?.content_notice = if filtered.is_empty() {
                    String::from(rice_i18n::t("Search cancelled"))
                } else {
                    rice_i18n::format("Search cancelled; {filtered}", &[("filtered", &filtered)])
                };
            } else {
                self.runtime_at_mut(tab, slot)?.content_notice = rice_i18n::format("Search cancelled; results: {count}", &[("count", &search.results.len().to_string())]);
                self.runtime_at_mut(tab, slot)?.entries = search.results;
            }
            let notice = self.runtime_at(tab, slot)?.content_notice.clone();
            let mut messages = self.finish_search_thumbnails(next_request)?;
            messages.push(self.system_notification("info", &notice));
            return Ok(messages);
        }
        let current = search
            .queue
            .front()
            .cloned()
            .ok_or(FilesActorError::UnexpectedVfsResponse)?;
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                let converted = super::convert_entries(&current, entries)?;
                for entry in converted {
                    if entry.kind == EntryKind::Directory
                        && search.visited.len() < MAX_DIRECTORIES
                        && search.visited.insert(entry.path.clone())
                    {
                        search.queue.push_back(entry.path.clone());
                    }
                    if search.results.len() < MAX_RESULTS && search.filters.matches(&entry) {
                        search.results.push(entry);
                    }
                }
                if let Some(next) = next_cursor {
                    if *next == search.cursor {
                        return Err(FilesActorError::UnexpectedVfsResponse);
                    }
                    search.cursor = *next;
                    return self.continue_search(slot, search, &current, *next);
                }
                search.queue.pop_front();
                search.cursor = 0;
            }
            VfsResult::Error(_) => {
                search.unreadable = search.unreadable.saturating_add(1);
                search.queue.pop_front();
                search.cursor = 0;
            }
            _ => return Err(FilesActorError::UnexpectedVfsResponse),
        }
        if let Some(next) = search.queue.front().cloned() {
            return self.continue_search(slot, search, &next, 0);
        }
        let sort = self.runtime_at(tab, slot)?.sort;
        sort_entries(&mut search.results, sort);
        self.runtime_at_mut(tab, slot)?.entries = search.results;
        let count = self.runtime_at(tab, slot)?.entries.len();
        let notice = rice_i18n::format("Results: {count}; unreadable folders skipped: {unreadable}", &[("count", &count.to_string()), ("unreadable", &search.unreadable.to_string())]);
        notice.clone_into(&mut self.runtime_at_mut(tab, slot)?.content_notice);
        let mut messages = self.finish_search_thumbnails(next_request)?;
        messages.push(self.system_notification("info", &notice));
        Ok(messages)
    }

    fn finish_search_thumbnails(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let mut messages = vec![self.emit_full()?];
        let mut entries = self.visible_entries_at(rice_files::PaneSlot::Primary);
        if self.tab()?.dual_pane {
            entries.extend(self.visible_entries_at(rice_files::PaneSlot::Secondary));
        }
        // Search results may be outside the current folder. Add bounded
        // results without evicting thumbnails hidden only by the query.
        if let Some(thumbnail) = self.reset_thumbnail_batch(&entries, request, false) {
            messages.push(thumbnail);
        }
        Ok(messages)
    }

    fn continue_search(
        &mut self,
        slot: rice_files::PaneSlot,
        mut search: PendingSearch,
        directory: &GuestPath,
        cursor: u64,
    ) -> Result<Vec<Message>, FilesActorError> {
        let tab = self.active_tab_id()?;
        let sort = self.runtime_at(tab, slot)?.sort;
        sort_entries(&mut search.results, sort);
        self.runtime_at_mut(tab, slot)?.entries = search.results.clone();
        let count = self.runtime_at(tab, slot)?.entries.len();
        self.runtime_at_mut(tab, slot)?.content_notice = rice_i18n::format("Searching… Results: {count}; folders queued: {queued}", &[("count", &count.to_string()), ("queued", &search.queue.len().to_string())]);
        let request = search.request;
        self.runtime_at_mut(tab, slot)?.pending_search = Some(search);
        Ok(vec![
            self.emit_full()?,
            self.directory_request(slot, request, directory, cursor),
        ])
    }
}

impl SearchFilters {
    fn parse(input: &str) -> Self {
        let mut filters = Self::default();
        let mut name = Vec::new();
        for token in input.split_whitespace() {
            let Some((key, value)) = token.split_once(':') else {
                name.push(token);
                continue;
            };
            match key.to_ascii_lowercase().as_str() {
                "type" => filters.kind = Some(value.to_ascii_lowercase()),
                "min" | "min-size" => filters.minimum_size = value.parse().ok(),
                "max" | "max-size" => filters.maximum_size = value.parse().ok(),
                "after" => filters.after_millis = value.parse().ok(),
                "before" => filters.before_millis = value.parse().ok(),
                _ => name.push(token),
            }
        }
        filters.name = name.join(" ").to_ascii_lowercase();
        filters
    }

    fn matches(&self, entry: &EntryMetadata) -> bool {
        (self.name.is_empty() || entry.name.to_ascii_lowercase().contains(&self.name))
            && self
                .minimum_size
                .is_none_or(|minimum| entry.size >= minimum)
            && self
                .maximum_size
                .is_none_or(|maximum| entry.size <= maximum)
            && self
                .after_millis
                .is_none_or(|after| entry.modified_unix_ms >= after)
            && self
                .before_millis
                .is_none_or(|before| entry.modified_unix_ms <= before)
            && self.kind.as_deref().is_none_or(|kind| match kind {
                "file" => entry.kind == EntryKind::File,
                "directory" | "folder" => entry.kind == EntryKind::Directory,
                "symlink" | "link" => entry.kind == EntryKind::Symlink,
                "image" => infer_media_type(entry).starts_with("image/"),
                "video" | "movie" => infer_media_type(entry).starts_with("video/"),
                "text" => infer_media_type(entry).starts_with("text/"),
                "other" => entry.kind == EntryKind::Other,
                mime => infer_media_type(entry) == mime,
            })
    }
}


#[cfg(test)]
mod translation_tests {
    use super::*;
    use super::super::tests::{loaded_actor, request, directory_response};
    use rice_native_app_protocol::{VfsError, VfsErrorCode};

    #[test]
    fn translated_search_notices_preserve_queries_and_real_results() {
        for (locale, expected) in [(1, "Resultados: 1; carpetas ilegibles omitidas: 1"),
                                   (2, "结果数：1；已跳过无法读取的文件夹：1")] {
            rice_i18n::with_locale(locale, || {
                let (mut actor, _) = loaded_actor();
                actor.update_quick_filter("{count}").unwrap();
                assert_eq!(actor.runtime().unwrap().controls.search_text, "{count}");
                assert!(actor.runtime().unwrap().content_notice.contains("{count}"));
                assert!(!actor.runtime().unwrap().content_notice.contains("immediate matches"));
                actor.update_quick_filter("FILE10").unwrap();
                assert_eq!(actor.runtime().unwrap().entries[0].name, "file10.txt");
                actor.start_search(request(20)).unwrap();
                let nested = actor.handle_vfs_response(&directory_response(20)).unwrap();
                assert!(nested.iter().any(|message| matches!(message, Message::VfsRequest(rice_native_app_protocol::VfsRequest {
                    operation: rice_native_app_protocol::VfsOperation::ReadDirectory {path, ..}, ..
                }) if path == "/root/Folder")));
                actor.handle_vfs_response(&VfsResponse {request: request(20), result: VfsResult::Error(VfsError {
                    code: VfsErrorCode::PermissionDenied, message: "host diagnostic".into(),
                })}).unwrap();
                assert_eq!(actor.runtime().unwrap().content_notice, expected);
                assert_eq!(actor.runtime().unwrap().entries.len(), 1);
                assert_eq!(actor.runtime().unwrap().entries[0].path.as_str(), "/root/file10.txt");
                assert_eq!(actor.runtime().unwrap().controls.search_text, "FILE10");
            });
        }
    }
}
