//! File chooser controller and VFS response strategy.

#[allow(clippy::wildcard_imports)]
use super::*;

impl FileChooser {
    pub fn start(&mut self, request: RequestId) -> ChooserEffect {
        self.begin_directory(self.directory.clone(), request, HistoryChange::Keep)
    }

    pub fn edit_location(&mut self, value: &str) {
        value.clone_into(&mut self.location_text);
        self.overwrite = None;
        self.notice = None;
        self.refresh_suggestions();
    }

    pub fn edit_filename(&mut self, value: &str) {
        value.clone_into(&mut self.filename);
        self.overwrite = None;
        self.notice = filename_error(value).map(ChooserNotice::message);
    }

    pub fn select_filter(&mut self, index: usize) {
        if index < self.filters.len() {
            // In Save As, selecting a named file type selects its extension as
            // well. Preserve explicitly typed unknown extensions and names in
            // Open dialogs; only replace a suffix owned by these filters.
            if self.mode == FileChooserMode::Save && !self.filename.is_empty() {
                if let Some(extension) = self.filters[index].extensions.first() {
                    match self.filename.rsplit_once('.') {
                        Some((stem, suffix))
                            if !stem.is_empty()
                                && self.filters.iter().any(|filter| {
                                    filter.extensions.iter().any(|candidate| candidate.eq_ignore_ascii_case(suffix))
                                }) => {
                            self.filename = format!("{stem}.{extension}");
                        }
                        None => self.filename = format!("{}.{extension}", self.filename),
                        _ => {}
                    }
                }
            }
            self.filter_index = index;
            self.selection = None;
            self.scroll = 0;
            self.overwrite = None;
            self.refresh_suggestions();
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.set_show_hidden(!self.show_hidden);
    }

    pub(super) fn set_show_hidden(&mut self, show_hidden: bool) {
        self.show_hidden = show_hidden;
        if self
            .selection
            .as_ref()
            .is_some_and(|name| !self.show_hidden && name.starts_with('.'))
        {
            self.selection = None;
        }
        self.refresh_suggestions();
        self.clamp_scroll();
    }

    pub fn select_visible(&mut self, index: usize) {
        let selected = self
            .visible_entries()
            .get(index)
            .map(|entry| (entry.name.clone(), entry.kind));
        if let Some((name, kind)) = selected {
            self.selection = Some(name.clone());
            if self.mode == FileChooserMode::Save && kind != FileKind::Directory {
                self.filename = name;
            }
        }
    }

    pub fn move_selection(&mut self, delta: i8) {
        let names = self
            .visible_entries()
            .iter()
            .map(|entry| entry.name.clone())
            .collect::<Vec<_>>();
        if names.is_empty() {
            self.selection = None;
            return;
        }
        let current = self
            .selection
            .as_ref()
            .and_then(|selected| names.iter().position(|name| name == selected))
            .unwrap_or(0);
        let next = if delta < 0 {
            current.checked_sub(1).unwrap_or(names.len() - 1)
        } else {
            (current + 1) % names.len()
        };
        self.selection = Some(names[next].clone());
        self.scroll = u32::try_from(next)
            .unwrap_or(u32::MAX)
            .saturating_mul(CHOOSER_ROW_HEIGHT)
            .saturating_sub(CHOOSER_ROW_HEIGHT.saturating_mul(2));
        self.clamp_scroll();
    }

    pub fn scroll_wheel(&mut self, button: u8) {
        let step = CHOOSER_ROW_HEIGHT.saturating_mul(3);
        self.scroll = match button {
            4 => self.scroll.saturating_sub(step),
            5 => self.scroll.saturating_add(step),
            _ => self.scroll,
        };
        self.clamp_scroll();
    }

    pub fn set_scroll(&mut self, position: u32) {
        self.scroll = position;
        self.clamp_scroll();
    }

    pub fn activate_visible(&mut self, index: usize, request: RequestId) -> ChooserEffect {
        let selected = self
            .visible_entries()
            .get(index)
            .map(|entry| (entry.name.clone(), entry.kind));
        let Some((name, kind)) = selected else {
            return ChooserEffect::Render;
        };
        self.selection = Some(name.clone());
        if kind == FileKind::Directory {
            self.begin_directory(
                join_path(&self.directory, &name),
                request,
                HistoryChange::Push,
            )
        } else if matches!(
            self.mode,
            FileChooserMode::Open | FileChooserMode::SelectAny
        ) {
            ChooserEffect::Accepted(join_path(&self.directory, &name))
        } else if self.mode == FileChooserMode::SelectDirectory {
            ChooserEffect::Render
        } else {
            self.filename = name;
            self.submit(request)
        }
    }

    pub fn activate_suggestion(&mut self, index: usize, request: RequestId) -> ChooserEffect {
        let Some(path) = self.suggestions.get(index).cloned() else {
            return ChooserEffect::Render;
        };
        self.location_text.clone_from(&path);
        self.suggestions.clear();
        if self
            .entry_for_path(&path)
            .is_some_and(|entry| entry.kind == FileKind::Directory)
        {
            self.begin_directory(path, request, HistoryChange::Push)
        } else {
            self.begin_probe(path, request)
        }
    }

    pub fn submit(&mut self, request: RequestId) -> ChooserEffect {
        if self.is_busy() {
            self.notice = Some(ChooserNotice::message("Wait for the current folder operation to finish"));
            return ChooserEffect::Render;
        }
        self.overwrite = None;
        match self.mode {
            FileChooserMode::Open => self.submit_open(request),
            FileChooserMode::SelectAny => self.submit_any(request),
            FileChooserMode::SelectDirectory => ChooserEffect::Accepted(self.directory.clone()),
            FileChooserMode::Save => self.submit_save(request),
        }
    }

    fn submit_any(&mut self, request: RequestId) -> ChooserEffect {
        if let Some(selected) = self.selection.clone() {
            return ChooserEffect::Accepted(join_path(&self.directory, &selected));
        }
        let Some(path) = normalize_absolute(&self.location_text) else {
            self.notice = Some(ChooserNotice::message("Enter an absolute RiceWM path"));
            return ChooserEffect::Render;
        };
        if path == self.directory {
            return ChooserEffect::Accepted(path);
        }
        self.begin_probe(path, request)
    }

    fn submit_open(&mut self, request: RequestId) -> ChooserEffect {
        if let Some(selected) = self.selection.clone() {
            let path = join_path(&self.directory, &selected);
            if self
                .entries
                .iter()
                .find(|entry| entry.name == selected)
                .is_some_and(|entry| entry.kind == FileKind::Directory)
            {
                return self.begin_directory(path, request, HistoryChange::Push);
            }
            return ChooserEffect::Accepted(path);
        }
        let Some(path) = normalize_absolute(&self.location_text) else {
            self.notice = Some(ChooserNotice::message("Enter an absolute RiceWM path"));
            return ChooserEffect::Render;
        };
        if path == self.directory {
            self.notice = Some(ChooserNotice::message("Select a document to open"));
            return ChooserEffect::Render;
        }
        self.begin_probe(path, request)
    }

    fn submit_save(&mut self, request: RequestId) -> ChooserEffect {
        if let Some(error) = filename_error(&self.filename) {
            self.notice = Some(ChooserNotice::message(error));
            return ChooserEffect::Render;
        }
        if !self.filename.contains('.') {
            if let Some(extension) = self.filters[self.filter_index].extensions.first() {
                self.filename = format!("{}.{extension}", self.filename);
            }
        }
        self.begin_probe(join_path(&self.directory, &self.filename), request)
    }

    pub fn confirm_overwrite(&mut self) -> ChooserEffect {
        self.overwrite
            .take()
            .map_or(ChooserEffect::Render, ChooserEffect::Accepted)
    }

    pub fn navigate_up(&mut self, request: RequestId) -> ChooserEffect {
        self.begin_directory(
            parent_path(&self.directory).to_owned(),
            request,
            HistoryChange::Push,
        )
    }

    pub fn navigate_place(&mut self, index: usize, request: RequestId) -> ChooserEffect {
        let Some(path) = self.places.get(index).map(|place| place.path.clone()) else {
            return ChooserEffect::Render;
        };
        self.begin_directory(path, request, HistoryChange::Push)
    }

    pub fn navigate_back(&mut self, request: RequestId) -> ChooserEffect {
        if !self.can_go_back() {
            return ChooserEffect::Render;
        }
        let index = self.history_index - 1;
        self.begin_directory(
            self.history[index].clone(),
            request,
            HistoryChange::Move(index),
        )
    }

    pub fn navigate_forward(&mut self, request: RequestId) -> ChooserEffect {
        if !self.can_go_forward() {
            return ChooserEffect::Render;
        }
        let index = self.history_index + 1;
        self.begin_directory(
            self.history[index].clone(),
            request,
            HistoryChange::Move(index),
        )
    }

    #[must_use]
    pub fn owns_request(&self, request: RequestId) -> bool {
        match &self.pending {
            Some(Pending::Directory(pending)) => pending.request == request,
            Some(Pending::Probe(pending)) => pending.request == request,
            None => false,
        }
    }

    pub fn handle_response(&mut self, response: &VfsResponse) -> ChooserEffect {
        let Some(pending) = self.pending.take() else {
            self.notice = Some(ChooserNotice::message("Unexpected file chooser response"));
            return ChooserEffect::Render;
        };
        match pending {
            Pending::Directory(pending) if pending.request == response.request => {
                self.handle_directory_response(pending, response)
            }
            Pending::Probe(pending) if pending.request == response.request => {
                self.handle_probe_response(&pending.path, response)
            }
            other => {
                self.pending = Some(other);
                self.notice = Some(ChooserNotice::message("Unexpected file chooser response"));
                ChooserEffect::Render
            }
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn begin_directory(
        &mut self,
        path: String,
        request: RequestId,
        history: HistoryChange,
    ) -> ChooserEffect {
        if self.is_busy() {
            self.notice = Some(ChooserNotice::message("Wait for the current folder operation to finish"));
            return ChooserEffect::Render;
        }
        let Some(path) = normalize_absolute(&path) else {
            self.notice = Some(ChooserNotice::message("Enter an absolute RiceWM folder path"));
            return ChooserEffect::Render;
        };
        self.notice = None;
        self.pending = Some(Pending::Directory(PendingDirectory {
            request,
            path: path.clone(),
            cursor: 0,
            entries: Vec::new(),
            history,
        }));
        ChooserEffect::Request(read_directory(path, 0))
    }

    #[allow(clippy::needless_pass_by_value)]
    fn begin_probe(&mut self, path: String, request: RequestId) -> ChooserEffect {
        if self.is_busy() {
            self.notice = Some(ChooserNotice::message("Wait for the current folder operation to finish"));
            return ChooserEffect::Render;
        }
        let Some(path) = normalize_absolute(&path) else {
            self.notice = Some(ChooserNotice::message("Enter an absolute RiceWM path"));
            return ChooserEffect::Render;
        };
        let known_mime_type = self
            .entry_for_path(&path)
            .and_then(|entry| entry.mime_type.as_deref());
        if self.mode == FileChooserMode::Open
            && !self.filters[self.filter_index]
                .accepts(file_name(&path).unwrap_or(""), known_mime_type)
        {
            self.notice = Some(ChooserNotice::formatted("“{name}” does not match the selected file type", vec![("name", file_name(&path).unwrap_or(&path).to_owned())]));
            return ChooserEffect::Render;
        }
        self.notice = None;
        self.pending = Some(Pending::Probe(PendingProbe {
            request,
            path: path.clone(),
        }));
        ChooserEffect::Request(VfsOperation::Stat { path })
    }

    fn handle_directory_response(
        &mut self,
        mut pending: PendingDirectory,
        response: &VfsResponse,
    ) -> ChooserEffect {
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                for entry in entries {
                    if pending.entries.len() >= MAX_ENTRIES {
                        self.notice = Some(ChooserNotice::formatted("Folder contains more than {maximum} entries; refine the path", vec![("maximum", MAX_ENTRIES.to_string())]));
                        return ChooserEffect::Render;
                    }
                    if valid_entry_name(&entry.name) {
                        pending.entries.push(FileChooserEntry::from(entry));
                    }
                }
                if let Some(cursor) = next_cursor {
                    pending.cursor = *cursor;
                    let operation = read_directory(pending.path.clone(), *cursor);
                    self.pending = Some(Pending::Directory(pending));
                    return ChooserEffect::Request(operation);
                }
                pending.entries.sort_by(|left, right| {
                    (left.kind != FileKind::Directory)
                        .cmp(&(right.kind != FileKind::Directory))
                        .then_with(|| collation::compare(&left.name, &right.name))
                });
                self.apply_history(&pending.path, pending.history);
                self.directory = pending.path;
                self.location_text.clone_from(&self.directory);
                self.entries = pending.entries;
                self.thumbnails.clear();
                self.selection = None;
                self.scroll = 0;
                self.suggestions.clear();
                self.notice = None;
                ChooserEffect::Render
            }
            VfsResult::Error(error) => {
                self.notice = Some(ChooserNotice::failure("Could not open folder: {error}", &error.message));
                ChooserEffect::Render
            }
            _ => {
                self.notice = Some(ChooserNotice::message("Could not open folder: invalid VFS response"));
                ChooserEffect::Render
            }
        }
    }

    fn handle_probe_response(&mut self, path: &str, response: &VfsResponse) -> ChooserEffect {
        match (&self.mode, &response.result) {
            (_, VfsResult::Metadata(metadata)) if metadata.kind == FileKind::Directory => {
                self.begin_directory(path.to_owned(), response.request, HistoryChange::Push)
            }
            (FileChooserMode::Open, VfsResult::Metadata(_)) => {
                ChooserEffect::Accepted(path.to_owned())
            }
            (FileChooserMode::SelectAny, VfsResult::Metadata(_)) => {
                ChooserEffect::Accepted(path.to_owned())
            }
            (FileChooserMode::Open, VfsResult::Error(error)) => {
                self.notice = Some(ChooserNotice::failure("Could not open file: {error}", &error.message));
                ChooserEffect::Render
            }
            (FileChooserMode::SelectAny, VfsResult::Error(error)) => {
                self.notice = Some(ChooserNotice::failure("Could not select item: {error}", &error.message));
                ChooserEffect::Render
            }
            (FileChooserMode::SelectDirectory, VfsResult::Metadata(_)) => {
                self.notice = Some(ChooserNotice::message("Select a folder, not a file"));
                ChooserEffect::Render
            }
            (FileChooserMode::SelectDirectory, VfsResult::Error(error)) => {
                self.notice = Some(ChooserNotice::failure("Could not open folder: {error}", &error.message));
                ChooserEffect::Render
            }
            (FileChooserMode::Save, VfsResult::Metadata(_)) => {
                self.overwrite = Some(path.to_owned());
                self.notice = Some(ChooserNotice::formatted("“{name}” already exists. Replace it?", vec![("name", file_name(path).unwrap_or(path).to_owned())]));
                ChooserEffect::Render
            }
            (FileChooserMode::Save, VfsResult::Error(error))
                if error.code == VfsErrorCode::NotFound =>
            {
                ChooserEffect::Accepted(path.to_owned())
            }
            (FileChooserMode::Save, VfsResult::Error(error)) => {
                self.notice = Some(ChooserNotice::failure("Could not validate destination: {error}", &error.message));
                ChooserEffect::Render
            }
            _ => {
                self.notice = Some(ChooserNotice::message("Invalid response while validating the file"));
                ChooserEffect::Render
            }
        }
    }

    fn apply_history(&mut self, path: &str, change: HistoryChange) {
        match change {
            HistoryChange::Push
                if self
                    .history
                    .get(self.history_index)
                    .is_none_or(|p| p != path) =>
            {
                self.history.truncate(self.history_index + 1);
                self.history.push(path.to_owned());
                self.history_index = self.history.len() - 1;
            }
            HistoryChange::Keep | HistoryChange::Push => {}
            HistoryChange::Move(index) => self.history_index = index,
        }
    }

    fn refresh_suggestions(&mut self) {
        self.suggestions.clear();
        let Some(path) = normalize_partial(&self.location_text) else {
            return;
        };
        let parent = parent_path(&path);
        if parent != self.directory {
            return;
        }
        let prefix = file_name(&path).unwrap_or("");
        let filter = &self.filters[self.filter_index];
        self.suggestions.extend(
            self.entries
                .iter()
                .filter(|entry| self.show_hidden || !entry.name.starts_with('.'))
                .filter(|entry| entry.name.starts_with(prefix))
                .filter(|entry| {
                    entry.kind == FileKind::Directory
                        || filter.accepts(&entry.name, entry.mime_type.as_deref())
                })
                .take(MAX_SUGGESTIONS)
                .map(|entry| join_path(&self.directory, &entry.name)),
        );
    }

    fn entry_for_path(&self, path: &str) -> Option<&FileChooserEntry> {
        (parent_path(path) == self.directory)
            .then(|| file_name(path))
            .flatten()
            .and_then(|name| self.entries.iter().find(|entry| entry.name == name))
    }

    fn clamp_scroll(&mut self) {
        let total = u32::try_from(self.visible_entries().len())
            .unwrap_or(u32::MAX)
            .saturating_mul(CHOOSER_ROW_HEIGHT);
        self.scroll = self.scroll.min(total.saturating_sub(CHOOSER_ROW_HEIGHT));
    }
}
