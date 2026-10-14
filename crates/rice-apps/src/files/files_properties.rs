//! Real RiceFiles metadata inspection and POSIX mode editing.

use std::collections::{BTreeSet, VecDeque};

use rice_files::{EntryKind, EntryMetadata, GuestPath, infer_media_type};
use rice_native_app_protocol::{
    FileKind, FileMetadata, Message, RequestId, VfsError, VfsErrorCode, VfsOperation, VfsRequest,
    VfsResponse, VfsResult,
};
use rice_ui::{IconId, Patch, WidgetKind, WidgetState};

use super::files_dialogs::CONTENT as DIALOG_CONTENT;
use super::{
    FilesActorError, RiceFiles,
    files_details::{size_label, total_size_label},
    files_dialogs::Dialog,
    files_view::{create, create_with_state},
};
use crate::time_format::modified_timestamp_label;

pub(super) const MODE_FIELD: u64 = 300_040;
pub(super) const APPLY_MODE: u64 = 300_041;
pub(super) const COPY_LOCATION: u64 = 300_046;
pub(super) const EXECUTABLE: u64 = 300_047;
pub(super) const ACCESS_READ_WRITE: u64 = 300_048;
pub(super) const ACCESS_READ_ONLY: u64 = 300_049;
pub(super) const ACCESS_WRITE_ONLY: u64 = 300_050;
pub(super) const ACCESS_NONE: u64 = 300_051;
const DETAILS: u64 = 300_043;
const PERMISSIONS: u64 = 300_044;
const FEEDBACK: u64 = 300_045;
const PERMISSION_ACCESS: u64 = 300_053;
const ACCESS_MENU_BAR: u64 = 300_054;
const ACCESS_MENU: u64 = 300_055;
const MODE_ROW: u64 = 300_056;
const COPY_LOCATION_TOOLTIP: u64 = 300_057;
const PERMISSION_SYMBOLIC: u64 = 300_058;
const PERMISSION_HEADING: u64 = 300_059;
const MODE_HEADING: u64 = 300_052;
const ROW_BASE: u64 = 300_060;
pub(super) const NAME_FIELD: u64 = 300_032;
const FILE_HEADER: u64 = 300_034;
pub(super) const HIDDEN_ATTRIBUTE: u64 = 300_037;
const PROPERTY_SEPARATOR: u64 = 300_038;
pub(super) const READ_ONLY_ATTRIBUTE: u64 = 300_039;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Loading,
    Ready,
    Renaming,
    Applying,
    Verifying,
}

/// Keep an English fallback for operation-state checks; render owned text
/// against the current locale so language changes never alter completion state.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Feedback {
    fallback: String,
    message: String,
    arguments: Vec<(&'static str, String)>,
    error: Option<VfsError>,
    path_error: Option<rice_files::NavigationError>,
}
impl From<String> for Feedback {
    fn from(value: String) -> Self {
        Self { fallback: value.clone(), message: value, arguments: Vec::new(), error: None, path_error: None }
    }
}
impl From<&str> for Feedback {
    fn from(value: &str) -> Self { value.to_owned().into() }
}
impl std::ops::Deref for Feedback {
    type Target = str;
    fn deref(&self) -> &str { &self.fallback }
}
impl Feedback {
    fn formatted(message: &str, arguments: Vec<(&'static str, String)>) -> Self {
        let pairs = arguments.iter().map(|(key, value)| (*key, value.as_str())).collect::<Vec<_>>();
        let fallback = rice_i18n::with_locale(0, || rice_i18n::format(message, &pairs));
        Self { fallback, message: message.to_owned(), arguments, error: None, path_error: None }
    }
    fn failure(message: &str, error: &VfsError) -> Self {
        let mut feedback = Self::formatted(message, vec![("error", rice_i18n::with_locale(0, || vfs_error(error)))]);
        feedback.error = Some(error.clone());
        feedback
    }
    fn navigation_failure(error: rice_files::NavigationError) -> Self {
        let mut feedback = Self::formatted("Name is invalid: {error}", vec![
            ("error", rice_i18n::with_locale(0, || navigation_error(error))),
        ]);
        feedback.path_error = Some(error);
        feedback
    }
    fn render(&self) -> String {
        let error = self.path_error.map(navigation_error)
            .or_else(|| self.error.as_ref().map(vfs_error));
        let pairs = self.arguments.iter().map(|(key, value)| (*key,
            if *key == "error" { error.as_deref().unwrap_or(value) } else { value.as_str() }
        )).collect::<Vec<_>>();
        rice_i18n::format(&self.message, &pairs)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PropertiesState {
    entries: Vec<EntryMetadata>,
    metadata: Option<FileMetadata>,
    name_input: String,
    mode_input: String,
    request: Option<RequestId>,
    requested_mode: Option<u32>,
    requested_path: Option<GuestPath>,
    close_after_apply: bool,
    stage: Stage,
    feedback: Option<Feedback>,
    folder_scan: Option<FolderScan>,
    folder_totals: Option<FolderTotals>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FolderTotals {
    files: u64,
    folders: u64,
    bytes: u64,
    unreadable_folders: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FolderScan {
    request: RequestId,
    pending: VecDeque<(GuestPath, u64)>,
    visited: BTreeSet<GuestPath>,
    totals: FolderTotals,
}

impl FolderScan {
    fn new(request: RequestId, root: GuestPath) -> Self {
        let visited = BTreeSet::from([root.clone()]);
        Self {
            request,
            pending: VecDeque::from([(root, 0)]),
            visited,
            totals: FolderTotals::default(),
        }
    }

    fn next_operation(&self) -> Option<VfsOperation> {
        self.pending
            .front()
            .map(|(path, cursor)| VfsOperation::ReadDirectory {
                path: path.as_str().to_owned(),
                cursor: *cursor,
                limit: 128,
            })
    }
}

impl PropertiesState {
    fn new(entries: Vec<EntryMetadata>, request: RequestId) -> Self {
        let single = entries.len() == 1;
        Self {
            name_input: entries
                .first()
                .map_or_else(String::new, |entry| entry.name.clone()),
            mode_input: entries
                .first()
                .map_or_else(String::new, |entry| format!("{:04o}", entry.mode)),
            entries,
            metadata: None,
            request: single.then_some(request),
            requested_mode: None,
            requested_path: None,
            close_after_apply: false,
            stage: if single { Stage::Loading } else { Stage::Ready },
            feedback: None,
            folder_scan: None,
            folder_totals: None,
        }
    }

    pub(super) fn awaits(&self, request: RequestId) -> bool {
        self.request == Some(request)
            || self
                .folder_scan
                .as_ref()
                .is_some_and(|scan| scan.request == request)
    }

    pub(super) fn pending_requests(&self) -> Vec<RequestId> {
        self.request
            .into_iter()
            .chain(self.folder_scan.as_ref().map(|scan| scan.request))
            .collect()
    }

    fn can_edit_mode(&self) -> bool {
        self.entries.len() == 1
            && self.metadata.is_some()
            && self.entries[0].kind != EntryKind::Symlink
            && self.stage == Stage::Ready
    }

    fn parsed_mode(&self) -> Result<u32, &'static str> {
        parse_mode(&self.mode_input)
    }

    fn mode_changed(&self) -> bool {
        self.parsed_mode()
            .ok()
            .zip(self.metadata.as_ref())
            .is_some_and(|(mode, metadata)| mode != metadata.mode)
    }

    fn name_changed(&self) -> bool {
        self.entries.len() == 1 && self.entries[0].name != self.name_input.trim()
    }

    pub(super) fn apply_enabled(&self) -> bool {
        self.stage == Stage::Ready
            && self.entries.len() == 1
            && self.parsed_mode().is_ok()
            && (self.mode_changed() || self.name_changed())
    }

    fn accept(&mut self, response: &VfsResponse) -> Option<VfsOperation> {
        if self
            .folder_scan
            .as_ref()
            .is_some_and(|scan| scan.request == response.request)
        {
            return self.accept_folder_scan(response);
        }
        match self.stage {
            Stage::Loading => self.accept_stat(response, false),
            Stage::Renaming => self.accept_rename(response),
            Stage::Applying => self.accept_apply(response),
            Stage::Verifying => self.accept_stat(response, true),
            Stage::Ready => None,
        }
    }

    fn accept_stat(&mut self, response: &VfsResponse, verifying: bool) -> Option<VfsOperation> {
        match &response.result {
            VfsResult::Metadata(metadata) => {
                self.update_metadata(metadata);
                let requested = self.requested_mode.take();
                self.stage = Stage::Ready;
                self.request = None;
                self.feedback =
                    if verifying {
                        requested.map_or_else(
                        || Some(Feedback::from("Changes applied")),
                        |mode| Some({
                        let reported = self
                            .metadata
                            .as_ref()
                            .map_or(metadata.mode & 0o7777, |value| value.mode);
                        if reported == mode {
                            Feedback::formatted("Permissions changed to {mode}", vec![("mode", format!("{reported:04o}"))])
                        } else {
                            Feedback::formatted("Verification failed: requested {requested}, host reported {reported}", vec![
                                ("requested", format!("{mode:04o}")), ("reported", format!("{reported:04o}")),
                            ])
                        }
                    }),
                    )
                    } else {
                        None
                    };
                if !verifying
                    && metadata.kind == FileKind::Directory
                    && let Some(root) = self.entries.first().map(|entry| entry.path.clone())
                {
                    self.folder_scan = Some(FolderScan::new(response.request, root));
                    return self
                        .folder_scan
                        .as_ref()
                        .and_then(FolderScan::next_operation);
                }
            }
            VfsResult::Error(error) => {
                self.stage = Stage::Ready;
                self.request = None;
                self.close_after_apply = false;
                self.feedback = Some(if verifying {
                    Feedback::failure("The host accepted the permission change, but verification failed: {error}", error)
                } else {
                    Feedback::failure("Metadata could not be read: {error}", error)
                });
            }
            _ => {
                self.stage = Stage::Ready;
                self.request = None;
                self.close_after_apply = false;
                self.feedback = Some(Feedback::from(
                    "Metadata could not be read: the VFS returned an invalid response",
                ));
            }
        }
        None
    }

    fn accept_folder_scan(&mut self, response: &VfsResponse) -> Option<VfsOperation> {
        let Some(scan) = self.folder_scan.as_mut() else {
            return None;
        };
        match &response.result {
            VfsResult::Directory {
                entries,
                next_cursor,
            } => {
                let Some((directory, cursor)) = scan.pending.front().cloned() else {
                    return None;
                };
                for entry in entries {
                    if entry.name.is_empty()
                        || entry.name == "."
                        || entry.name == ".."
                        || entry.name.contains('/')
                    {
                        scan.totals.unreadable_folders =
                            scan.totals.unreadable_folders.saturating_add(1);
                        continue;
                    }
                    if entry.metadata.kind == FileKind::Directory {
                        scan.totals.folders = scan.totals.folders.saturating_add(1);
                        if let Ok(child) = directory.join(&entry.name) {
                            if scan.visited.insert(child.clone()) {
                                scan.pending.push_back((child, 0));
                            }
                        } else {
                            scan.totals.unreadable_folders =
                                scan.totals.unreadable_folders.saturating_add(1);
                        }
                    } else {
                        scan.totals.files = scan.totals.files.saturating_add(1);
                        scan.totals.bytes = scan.totals.bytes.saturating_add(entry.metadata.size);
                    }
                }
                if let Some(next) = next_cursor {
                    if *next == cursor {
                        scan.totals.unreadable_folders =
                            scan.totals.unreadable_folders.saturating_add(1);
                        scan.pending.pop_front();
                    } else if let Some((_, pending_cursor)) = scan.pending.front_mut() {
                        *pending_cursor = *next;
                    }
                } else {
                    scan.pending.pop_front();
                }
            }
            VfsResult::Error(_)
            | VfsResult::Metadata(_)
            | VfsResult::Data(_)
            | VfsResult::Written(_)
            | VfsResult::Done
            | VfsResult::Progress(_)
            | VfsResult::Cancelled => {
                scan.totals.unreadable_folders = scan.totals.unreadable_folders.saturating_add(1);
                scan.pending.pop_front();
            }
        }
        let next = scan.next_operation();
        if next.is_none() {
            self.folder_totals = Some(scan.totals.clone());
            self.folder_scan = None;
        }
        next
    }

    pub(super) fn title(&self) -> String {
        match self.entries.as_slice() {
            [entry] => rice_i18n::format("files.properties.ebc9c6c875a2", &[("name", &entry.name)]),
            entries => rice_i18n::format("files.properties.71e3b039197c", &[("count", &entries.len().to_string())]),
        }
    }

    pub(super) fn redmond_title(&self) -> String {
        match self.entries.as_slice() {
            [entry] => rice_i18n::format("files.properties.a99a7d799c6f", &[("name", &entry.name)]),
            entries => rice_i18n::format("files.properties.da62d2d15867", &[("count", &entries.len().to_string())]),
        }
    }

    fn accept_rename(&mut self, response: &VfsResponse) -> Option<VfsOperation> {
        match &response.result {
            VfsResult::Done => {
                let destination = self.requested_path.take()?;
                if let Some(entry) = self.entries.first_mut() {
                    entry.name = destination
                        .as_str()
                        .rsplit('/')
                        .next()
                        .unwrap_or_default()
                        .to_owned();
                    entry.path = destination.clone();
                }
                if let Some(mode) = self.requested_mode {
                    self.stage = Stage::Applying;
                    self.feedback = Some(Feedback::from("Applying permissions…"));
                    Some(VfsOperation::SetMode {
                        path: destination.as_str().to_owned(),
                        mode,
                    })
                } else {
                    self.stage = Stage::Verifying;
                    self.feedback = Some(Feedback::from("Verifying changes…"));
                    Some(VfsOperation::Stat {
                        path: destination.as_str().to_owned(),
                    })
                }
            }
            VfsResult::Error(error) => {
                self.stage = Stage::Ready;
                self.request = None;
                self.requested_mode = None;
                self.requested_path = None;
                self.close_after_apply = false;
                self.feedback = Some(Feedback::failure("Name was not changed: {error}", error));
                None
            }
            _ => {
                self.stage = Stage::Ready;
                self.request = None;
                self.requested_mode = None;
                self.requested_path = None;
                self.close_after_apply = false;
                self.feedback = Some(Feedback::from(
                    "Name was not changed: the VFS returned an invalid response",
                ));
                None
            }
        }
    }

    fn accept_apply(&mut self, response: &VfsResponse) -> Option<VfsOperation> {
        match &response.result {
            VfsResult::Done => {
                self.stage = Stage::Verifying;
                self.feedback = Some(Feedback::from("Verifying permissions…"));
                self.entries.first().map(|entry| VfsOperation::Stat {
                    path: entry.path.as_str().to_owned(),
                })
            }
            VfsResult::Error(error) => {
                self.stage = Stage::Ready;
                self.request = None;
                self.requested_mode = None;
                self.close_after_apply = false;
                self.feedback = Some(Feedback::failure("Permissions were not changed: {error}", error));
                None
            }
            _ => {
                self.stage = Stage::Ready;
                self.request = None;
                self.requested_mode = None;
                self.close_after_apply = false;
                self.feedback = Some(Feedback::from(
                    "Permissions were not changed: the VFS returned an invalid response",
                ));
                None
            }
        }
    }

    fn update_metadata(&mut self, metadata: &FileMetadata) {
        let mut metadata = metadata.clone();
        metadata.mode &= 0o7777;
        self.metadata = Some(metadata.clone());
        self.mode_input = format!("{:04o}", metadata.mode);
        if let Some(entry) = self.entries.first_mut() {
            entry.kind = entry_kind(metadata.kind);
            entry.size = metadata.size;
            entry.modified_unix_ms = metadata.modified_millis;
            entry.mode = metadata.mode;
        }
    }
}

impl RiceFiles {
    pub(super) fn show_properties(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let entries = self.selected_entries();
        if entries.is_empty() {
            return self.emit_notice(rice_i18n::t("Select at least one item to view properties"));
        }
        let state = PropertiesState::new(entries, request);
        let stat = state
            .entries
            .first()
            .filter(|_| state.entries.len() == 1)
            .map(|entry| {
                Message::VfsRequest(VfsRequest {
                    request,
                    app: self.app,
                    operation: VfsOperation::Stat {
                        path: entry.path.as_str().to_owned(),
                    },
                })
            });
        self.dialog = Some(Dialog::Properties(state));
        let mut messages = vec![self.emit_full()?];
        messages.extend(stat);
        Ok(messages)
    }

    pub(super) fn update_property_mode(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if state.stage != Stage::Ready || state.metadata.is_none() {
            return Ok(Vec::new());
        }
        state.mode_input = value.trim().to_owned();
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn update_property_name(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if state.stage != Stage::Ready || state.entries.len() != 1 {
            return Ok(Vec::new());
        }
        state.name_input = value.to_owned();
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn apply_property_changes(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.apply_property_changes_with_close(request, false)
    }

    pub(super) fn confirm_property_changes(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let has_changes = matches!(
            self.dialog,
            Some(Dialog::Properties(ref state)) if state.apply_enabled()
        );
        if !has_changes {
            return self.close_operation_dialog();
        }
        self.apply_property_changes_with_close(request, true)
    }

    fn apply_property_changes_with_close(
        &mut self,
        request: RequestId,
        close_after_apply: bool,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_mutation.is_some() || self.runtime()?.is_enumerating() {
            return self.property_feedback("Wait for the current filesystem request to finish");
        }
        let app = self.app;
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if state.stage != Stage::Ready || state.entries.len() != 1 {
            return Ok(Vec::new());
        }
        let mode = match state.parsed_mode() {
            Ok(mode) => mode,
            Err(error) => return self.property_feedback(error),
        };
        let entry = state.entries[0].clone();
        let name = state.name_input.trim();
        if name.is_empty() || name == "." || name == ".." || name.contains('/') {
            return self.property_feedback("Enter a valid file name without slashes");
        }
        let destination = match entry.path.parent().join(name) {
            Ok(path) => path,
            Err(error) => return self.property_feedback(Feedback::navigation_failure(error)),
        };
        let name_changed = destination != entry.path;
        let mode_changed = state
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.mode != mode);
        if !name_changed && !mode_changed {
            return Ok(Vec::new());
        }
        state.close_after_apply = close_after_apply;
        if name_changed {
            state.stage = Stage::Renaming;
            state.request = Some(request);
            state.requested_path = Some(destination.clone());
            state.requested_mode = mode_changed.then_some(mode);
            state.feedback = Some(Feedback::from("Changing name…"));
            return Ok(vec![
                self.emit_full()?,
                Message::VfsRequest(VfsRequest {
                    request,
                    app,
                    operation: VfsOperation::Rename {
                        from: entry.path.as_str().to_owned(),
                        to: destination.as_str().to_owned(),
                    },
                }),
            ]);
        }
        self.begin_property_mode_change(request, app, mode)
    }

    pub(super) fn choose_property_access(
        &mut self,
        owner_read_write: u32,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if !state.can_edit_mode() {
            return Ok(Vec::new());
        }
        let Ok(mode) = state.parsed_mode() else {
            return self.property_feedback("Correct the octal permissions before choosing access");
        };
        let owner_execute = mode & 0o100;
        state.mode_input = format!("{:04o}", (mode & !0o700) | owner_read_write | owner_execute);
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn toggle_property_read_only(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if !state.can_edit_mode() {
            return Ok(Vec::new());
        }
        let mode = state.parsed_mode().unwrap_or(0);
        state.mode_input = format!(
            "{:04o}",
            if mode & 0o222 == 0 {
                mode | 0o200
            } else {
                mode & !0o222
            }
        );
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn toggle_property_hidden(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if state.stage != Stage::Ready || state.entries.len() != 1 {
            return Ok(Vec::new());
        }
        let trimmed = state.name_input.trim();
        state.name_input = if let Some(visible) = trimmed.strip_prefix('.') {
            visible.to_owned()
        } else {
            format!(".{trimmed}")
        };
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn toggle_property_executable(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let redmond98 = self.redmond98;
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        if !state.can_edit_mode() || !shows_executable(state, redmond98) {
            return Ok(Vec::new());
        }
        let mode = state
            .parsed_mode()
            .unwrap_or_else(|_| state.metadata.as_ref().map_or(0, |metadata| metadata.mode));
        let mode = if mode & 0o111 == 0 {
            mode | 0o111
        } else {
            mode & !0o111
        };
        state.mode_input = format!("{mode:04o}");
        state.feedback = None;
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn copy_property_location(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let path = match self.dialog.as_ref() {
            Some(Dialog::Properties(state)) => state
                .entries
                .first()
                .filter(|_| state.entries.len() == 1)
                .map(|entry| entry.path.as_str().to_owned()),
            _ => None,
        };
        let Some(path) = path else {
            return Ok(Vec::new());
        };
        super::files_clipboard::write_location(self, request, &path)
    }

    fn begin_property_mode_change(
        &mut self,
        request: RequestId,
        app: rice_native_app_protocol::AppId,
        mode: u32,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        let Some(entry) = state.entries.first() else {
            return Ok(Vec::new());
        };
        if state
            .metadata
            .as_ref()
            .is_some_and(|metadata| metadata.mode == mode)
        {
            return Ok(Vec::new());
        }
        let path = entry.path.as_str().to_owned();
        state.stage = Stage::Applying;
        state.request = Some(request);
        state.requested_mode = Some(mode);
        state.mode_input = format!("{mode:04o}");
        state.feedback = Some(Feedback::from("Applying permissions…"));
        Ok(vec![
            self.emit_full()?,
            Message::VfsRequest(VfsRequest {
                request,
                app,
                operation: VfsOperation::SetMode { path, mode },
            }),
        ])
    }

    pub(super) fn handle_properties_response(
        &mut self,
        response: &VfsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        let (follow_up, verified_entry, renamed_entry, close_dialog) = {
            let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
                return Err(FilesActorError::UnexpectedVfsResponse);
            };
            if !state.awaits(response.request) {
                return Err(FilesActorError::UnexpectedVfsResponse);
            }
            let was_verifying = state.stage == Stage::Verifying;
            let renamed_from = (state.stage == Stage::Renaming
                && matches!(response.result, VfsResult::Done))
            .then(|| state.entries.first().map(|entry| entry.path.clone()))
            .flatten();
            let follow_up = state.accept(response);
            let verified_entry = (was_verifying
                && matches!(response.result, VfsResult::Metadata(_)))
            .then(|| state.entries.first().cloned())
            .flatten();
            let renamed_entry = renamed_from.zip(state.entries.first().cloned());
            let close_dialog = follow_up.is_none()
                && state.close_after_apply
                && state.stage == Stage::Ready
                && state.feedback.as_deref().is_some_and(|feedback| {
                    feedback == "Changes applied" || feedback.starts_with("Permissions changed")
                });
            (follow_up, verified_entry, renamed_entry, close_dialog)
        };
        if let Some((old_path, renamed)) = renamed_entry
            && let Some(entry) = self
                .runtime_mut()?
                .entries
                .iter_mut()
                .find(|entry| entry.path == old_path)
        {
            *entry = renamed;
        }
        if let Some(verified) = verified_entry {
            if let Some(entry) = self
                .runtime_mut()?
                .entries
                .iter_mut()
                .find(|entry| entry.path == verified.path)
            {
                *entry = verified;
            }
        }
        if close_dialog {
            self.dialog = None;
        }
        let patch = self.emit_full()?;
        Ok(match follow_up {
            Some(operation) => vec![
                patch,
                Message::VfsRequest(VfsRequest {
                    request: response.request,
                    app: self.app,
                    operation,
                }),
            ],
            None => vec![patch],
        })
    }

    fn property_feedback(&mut self, message: impl Into<Feedback>) -> Result<Vec<Message>, FilesActorError> {
        let Some(Dialog::Properties(state)) = self.dialog.as_mut() else {
            return Ok(Vec::new());
        };
        state.feedback = Some(message.into());
        self.emit_full().map(|message| vec![message])
    }
}

pub(super) fn patches(patches: &mut Vec<Patch>, state: &PropertiesState, redmond98: bool) {
    if redmond98 {
        if let Some(entry) = state.entries.first() {
            create(
                patches,
                FILE_HEADER,
                Some(DIALOG_CONTENT),
                WidgetKind::Row,
                Some("split:70"),
                None,
            );
            create_with_state(
                patches,
                FILE_HEADER + 100,
                Some(FILE_HEADER),
                WidgetKind::Label,
                None,
                Some(if entry.kind == EntryKind::Directory {
                    IconId::Folder
                } else {
                    IconId::Document
                }),
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
            );
            create_with_state(
                patches,
                NAME_FIELD,
                Some(FILE_HEADER),
                WidgetKind::TextField,
                Some(&state.name_input),
                None,
                if state.stage == Stage::Ready {
                    WidgetState::ENABLED
                } else {
                    WidgetState::BUSY
                },
            );
        }
    }
    create(
        patches,
        DETAILS,
        Some(DIALOG_CONTENT),
        WidgetKind::Column,
        None,
        None,
    );
    let property_rows = if redmond98 {
        redmond_rows(state)
    } else {
        rows(state)
    };
    for (index, (label, value)) in property_rows.into_iter().enumerate() {
        property_row(patches, index, label, &value, redmond98);
    }
    permission_patches(patches, state, redmond98);
    let rendered_feedback = state.feedback.as_ref().map(Feedback::render);
    if let Some(feedback) = validation_feedback(state).or(rendered_feedback.as_deref()) {
        create(
            patches,
            FEEDBACK,
            Some(DIALOG_CONTENT),
            WidgetKind::Label,
            Some(rice_i18n::t(feedback)),
            Some(if validation_feedback(state).is_some() { IconId::Warning } else { feedback_icon(state.feedback.as_deref().unwrap_or(feedback)) }),
        );
    }
}

fn permission_patches(patches: &mut Vec<Patch>, state: &PropertiesState, redmond98: bool) {
    let Some(metadata) = &state.metadata else {
        if state.stage == Stage::Loading {
            create(
                patches,
                PERMISSIONS,
                Some(DIALOG_CONTENT),
                WidgetKind::Label,
                Some(rice_i18n::t("Reading permissions…")),
                None,
            );
        }
        return;
    };
    let symbolic = symbolic_mode(metadata.mode, state.entries[0].kind);
    create(
        patches,
        PERMISSIONS,
        Some(DIALOG_CONTENT),
        WidgetKind::Column,
        None,
        None,
    );
    if redmond98 {
        create(
            patches,
            PROPERTY_SEPARATOR,
            Some(PERMISSIONS),
            WidgetKind::Row,
            Some("panel:separator"),
            None,
        );
        create_with_state(
            patches,
            PERMISSION_HEADING,
            Some(PERMISSIONS),
            WidgetKind::Label,
            Some(rice_i18n::t("Attributes:")),
            None,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
        );
        if state.entries[0].kind != EntryKind::Symlink {
            create_with_state(
                patches,
                READ_ONLY_ATTRIBUTE,
                Some(PERMISSIONS),
                WidgetKind::CheckBox,
                Some(rice_i18n::t("Read-only")),
                None,
                WidgetState::ENABLED
                    | if state.parsed_mode().unwrap_or(metadata.mode) & 0o222 == 0 {
                        WidgetState::CHECKED
                    } else {
                        WidgetState::empty()
                    },
            );
            create_with_state(
                patches,
                HIDDEN_ATTRIBUTE,
                Some(PERMISSIONS),
                WidgetKind::CheckBox,
                Some(rice_i18n::t("Hidden")),
                None,
                WidgetState::ENABLED
                    | if state.name_input.trim().starts_with('.') {
                        WidgetState::CHECKED
                    } else {
                        WidgetState::empty()
                    },
            );
            if shows_executable(state, true) {
                create_with_state(
                    patches,
                    EXECUTABLE,
                    Some(PERMISSIONS),
                    WidgetKind::CheckBox,
                    Some(rice_i18n::t("Execute")),
                    None,
                    WidgetState::ENABLED
                        | if state.parsed_mode().unwrap_or(metadata.mode) & 0o111 != 0 {
                            WidgetState::CHECKED
                        } else {
                            WidgetState::empty()
                        },
                );
            }
        }
        create_with_state(
            patches,
            MODE_HEADING,
            Some(PERMISSIONS),
            WidgetKind::Label,
            Some(rice_i18n::t("Permissions:")),
            None,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
        );
    } else {
        create_with_state(
            patches,
            PERMISSION_HEADING,
            Some(PERMISSIONS),
            WidgetKind::Label,
            Some(rice_i18n::t("Permissions")),
            None,
            WidgetState::ENABLED | WidgetState::EMPHASIZED | WidgetState::START_ALIGNED,
        );
    }
    create_with_state(
        patches,
        PERMISSION_SYMBOLIC,
        Some(PERMISSIONS),
        WidgetKind::Label,
        Some(&symbolic),
        None,
        WidgetState::ENABLED | WidgetState::MUTED | WidgetState::START_ALIGNED,
    );
    if state.entries[0].kind == EntryKind::Symlink {
        create(
            patches,
            MODE_FIELD,
            Some(PERMISSIONS),
            WidgetKind::Label,
            Some(&rice_i18n::format("files.properties.readonly-link", &[("mode", &format!("{:04o}", metadata.mode))])),
            None,
        );
        return;
    }
    create(
        patches,
        PERMISSION_ACCESS,
        Some(PERMISSIONS),
        WidgetKind::Row,
        Some("split:300"),
        None,
    );
    create_with_state(
        patches,
        PERMISSION_ACCESS + 100,
        Some(PERMISSION_ACCESS),
        WidgetKind::Label,
        Some(rice_i18n::t("Access")),
        None,
        WidgetState::ENABLED | WidgetState::MUTED,
    );
    create(
        patches,
        ACCESS_MENU_BAR,
        Some(PERMISSION_ACCESS),
        WidgetKind::MenuBar,
        None,
        None,
    );
    create_with_state(
        patches,
        ACCESS_MENU,
        Some(ACCESS_MENU_BAR),
        WidgetKind::Menu,
        Some(owner_access_label(
            state.parsed_mode().unwrap_or(metadata.mode),
        )),
        Some(IconId::ChevronDown),
        if state.stage == Stage::Ready {
            WidgetState::ENABLED | WidgetState::TRAILING_ICON
        } else {
            WidgetState::empty()
        },
    );
    for (id, label, bits) in ACCESS_CHOICES {
        create_with_state(
            patches,
            id,
            Some(ACCESS_MENU),
            WidgetKind::RadioButton,
            Some(rice_i18n::t(label)),
            None,
            WidgetState::ENABLED
                | if state.parsed_mode().unwrap_or(metadata.mode) & 0o600 == bits {
                    WidgetState::CHECKED
                } else {
                    WidgetState::empty()
                },
        );
    }
    create(
        patches,
        MODE_ROW,
        Some(PERMISSIONS),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    let mut field_state = if state.stage == Stage::Ready {
        WidgetState::ENABLED
    } else {
        WidgetState::BUSY
    };
    if state.stage == Stage::Ready && state.parsed_mode().is_err() {
        field_state = field_state | WidgetState::INVALID;
    }
    create_with_state(
        patches,
        MODE_FIELD,
        Some(MODE_ROW),
        WidgetKind::TextField,
        Some(&state.mode_input),
        None,
        field_state,
    );
    if !redmond98 {
        create_with_state(
            patches,
            APPLY_MODE,
            Some(MODE_ROW),
            WidgetKind::Button,
            Some(rice_i18n::t("Apply")),
            None,
            if state.apply_enabled() {
                WidgetState::ENABLED | WidgetState::ACCENT
            } else {
                WidgetState::empty()
            },
        );
    }
    if !redmond98 && shows_executable(state, false) {
        create_with_state(
            patches,
            EXECUTABLE,
            Some(PERMISSIONS),
            WidgetKind::Toggle,
            Some(rice_i18n::t("Executable as Program")),
            Some(IconId::Program),
            if state.stage == Stage::Ready {
                WidgetState::ENABLED
                    | if metadata.mode & 0o111 != 0 {
                        WidgetState::CHECKED
                    } else {
                        WidgetState::empty()
                    }
            } else {
                WidgetState::BUSY
            },
        );
    }
}

const ACCESS_CHOICES: [(u64, &str, u32); 4] = [
    (ACCESS_READ_WRITE, "Read and write", 0o600),
    (ACCESS_READ_ONLY, "Read only", 0o400),
    (ACCESS_WRITE_ONLY, "Write only", 0o200),
    (ACCESS_NONE, "No access", 0),
];

fn owner_access_label(mode: u32) -> &'static str {
    match mode & 0o600 {
        0o600 => rice_i18n::t("Read and write"),
        0o400 => rice_i18n::t("Read only"),
        0o200 => rice_i18n::t("Write only"),
        _ => rice_i18n::t("No access"),
    }
}

fn shows_executable(state: &PropertiesState, all_files: bool) -> bool {
    let Some(entry) = state.entries.first().filter(|_| state.entries.len() == 1) else {
        return false;
    };
    entry.kind == EntryKind::File
        && (all_files
            || entry.name.rsplit_once('.').is_some_and(|(_, extension)| {
                ["sh", "bin", "run"]
                    .into_iter()
                    .any(|choice| extension.eq_ignore_ascii_case(choice))
            }))
}

fn rows(state: &PropertiesState) -> Vec<(&'static str, String)> {
    if state.entries.len() != 1 {
        let paths = state
            .entries
            .iter()
            .map(|entry| entry.path.as_str().to_owned())
            .collect::<Vec<_>>()
            .join("\n");
        return vec![
            ("Selected", rice_i18n::format("files.count.items", &[("count", &state.entries.len().to_string())])),
            ("Paths", paths),
            (
                "Combined size",
                rice_i18n::format("files.count.bytes", &[("count", &state.entries.iter().map(|entry| entry.size).sum::<u64>().to_string())]),
            ),
        ];
    }
    let entry = &state.entries[0];
    let mut rows = vec![
        ("Selected", rice_i18n::t("files.count.item").to_owned()),
        ("Type", if entry.kind == EntryKind::File { super::files_details::type_label(entry) } else { type_label(entry.kind).to_owned() }),
        ("Path", entry.path.as_str().to_owned()),
        ("MIME type", infer_media_type(entry).to_owned()),
        (
            "Size",
            rice_i18n::format("files.count.size", &[("size", &size_label(entry)), ("bytes", &entry.size.to_string())]),
        ),
        ("Modified", modified_timestamp_label(entry.modified_unix_ms)),
    ];
    if entry.kind == EntryKind::Directory {
        let (contents, size) = folder_rows(state);
        rows.push(("Contents", contents));
        rows.push(("Contents size", size));
    }
    if let Some(target) = &entry.symlink_target {
        rows.push(("Link target", target.as_str().to_owned()));
    }
    rows
}

fn redmond_rows(state: &PropertiesState) -> Vec<(&'static str, String)> {
    if state.entries.len() != 1 {
        return rows(state);
    }
    let entry = &state.entries[0];
    let location = entry
        .path
        .as_str()
        .rsplit_once('/')
        .map_or(
            "/",
            |(parent, _)| if parent.is_empty() { "/" } else { parent },
        );
    let mut values = vec![
        (
            "Type:",
            if entry.kind == EntryKind::Directory {
                rice_i18n::t("File Folder").to_owned()
            } else {
                if entry.kind == EntryKind::File { super::files_details::type_label(entry) } else { type_label(entry.kind).to_owned() }
            },
        ),
        ("Location:", location.to_owned()),
    ];
    if entry.kind == EntryKind::Directory {
        let (contents, size) = folder_rows(state);
        values.push(("Size:", size));
        values.push(("Contains:", contents));
    } else {
        values.push((
            "Size:",
            rice_i18n::format("files.count.size", &[("size", &size_label(entry)), ("bytes", &entry.size.to_string())]),
        ));
    }
    values.push((
        "Modified:",
        modified_timestamp_label(entry.modified_unix_ms),
    ));
    values
}

fn property_row(patches: &mut Vec<Patch>, index: usize, label: &str, value: &str, redmond98: bool) {
    let Ok(index) = u64::try_from(index) else {
        return;
    };
    let row = ROW_BASE + index * 3;
    create(
        patches,
        row,
        Some(DETAILS),
        WidgetKind::Row,
        Some("split:300"),
        None,
    );
    create_with_state(
        patches,
        row + 1,
        Some(row),
        WidgetKind::Label,
        Some(rice_i18n::t(label)),
        None,
        WidgetState::ENABLED
            | if redmond98 {
                WidgetState::empty()
            } else {
                WidgetState::MUTED
            },
    );
    create(
        patches,
        row + 2,
        Some(row),
        WidgetKind::Label,
        Some(value),
        None,
    );
    if label == "Path" || label == "Location:" {
        create_with_state(
            patches,
            COPY_LOCATION,
            Some(row),
            WidgetKind::Button,
            Some(""),
            Some(IconId::EditCopy),
            WidgetState::ENABLED | WidgetState::FLAT,
        );
        create(
            patches,
            COPY_LOCATION_TOOLTIP,
            Some(COPY_LOCATION),
            WidgetKind::Tooltip,
            Some(rice_i18n::t("Copy full path")),
            None,
        );
    }
}

fn folder_rows(state: &PropertiesState) -> (String, String) {
    let totals = state
        .folder_scan
        .as_ref()
        .map(|scan| &scan.totals)
        .or(state.folder_totals.as_ref());
    let Some(totals) = totals else {
        return (rice_i18n::t("Calculating…").to_owned(), rice_i18n::t("Calculating…").to_owned());
    };
    let decorate = |summary: String| {
        let summary = if totals.unreadable_folders == 0 { summary } else {
            rice_i18n::format("files.count.unreadable", &[("summary", &summary)])
        };
        if state.folder_scan.is_some() {
            rice_i18n::format("files.count.progress", &[("summary", &summary)])
        } else { summary }
    };
    (
        decorate(rice_i18n::format("files.count.contents", &[
            ("files", &totals.files.to_string()), ("folders", &totals.folders.to_string()),
        ])),
        decorate(rice_i18n::format("files.count.size", &[
            ("size", &total_size_label(totals.bytes)), ("bytes", &totals.bytes.to_string()),
        ])),
    )
}

fn type_label(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::File => rice_i18n::t("File"),
        EntryKind::Directory => rice_i18n::t("Folder"),
        EntryKind::Symlink => rice_i18n::t("Symbolic link"),
        EntryKind::Other => rice_i18n::t("Other"),
    }
}

fn parse_mode(value: &str) -> Result<u32, &'static str> {
    let value = value.trim().strip_prefix("0o").unwrap_or(value.trim());
    if !(3..=4).contains(&value.len()) || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(rice_i18n::t("Enter three or four octal digits (for example, 0644)"));
    }
    u32::from_str_radix(value, 8)
        .ok()
        .filter(|mode| *mode <= 0o7777)
        .ok_or(rice_i18n::t("Permissions must be between 0000 and 7777"))
}

fn symbolic_mode(mode: u32, kind: EntryKind) -> String {
    let mut text = String::with_capacity(10);
    text.push(match kind {
        EntryKind::Directory => 'd',
        EntryKind::Symlink => 'l',
        EntryKind::File => '-',
        EntryKind::Other => '?',
    });
    for (read, write, execute, special, special_exec, special_plain) in [
        (0o400, 0o200, 0o100, 0o4000, 's', 'S'),
        (0o040, 0o020, 0o010, 0o2000, 's', 'S'),
        (0o004, 0o002, 0o001, 0o1000, 't', 'T'),
    ] {
        text.push(if mode & read != 0 { 'r' } else { '-' });
        text.push(if mode & write != 0 { 'w' } else { '-' });
        text.push(if mode & special != 0 {
            if mode & execute != 0 {
                special_exec
            } else {
                special_plain
            }
        } else if mode & execute != 0 {
            'x'
        } else {
            '-'
        });
    }
    text
}

const fn entry_kind(kind: FileKind) -> EntryKind {
    match kind {
        FileKind::File => EntryKind::File,
        FileKind::Directory => EntryKind::Directory,
        FileKind::Symlink => EntryKind::Symlink,
        FileKind::Other => EntryKind::Other,
    }
}

fn validation_feedback(state: &PropertiesState) -> Option<&'static str> {
    (state.stage == Stage::Ready && state.metadata.is_some())
        .then(|| state.parsed_mode().err())
        .flatten()
}

fn feedback_icon(feedback: &str) -> IconId {
    if feedback == "Changes applied" || feedback.starts_with("Permissions changed") {
        IconId::Information
    } else {
        IconId::Warning
    }
}

fn navigation_error(error: rice_files::NavigationError) -> String {
    use rice_files::NavigationError;
    match error {
        NavigationError::NulByte => rice_i18n::t("files.path.nul-byte").to_owned(),
        NavigationError::PathTooLong(length) => rice_i18n::format("files.path.too-long", &[
            ("length", &length.to_string()), ("maximum", &GuestPath::MAX_BYTES.to_string()),
        ]),
        NavigationError::AbsolutePathRequired => rice_i18n::t("files.path.absolute-required").to_owned(),
        NavigationError::RelativePathRequired => rice_i18n::t("files.path.relative-required").to_owned(),
        NavigationError::NoHistory => rice_i18n::t("files.path.no-history").to_owned(),
        NavigationError::StalePlan => rice_i18n::t("files.path.stale-plan").to_owned(),
    }
}

fn vfs_error(error: &VfsError) -> String {
    let category = match error.code {
        VfsErrorCode::NotFound => rice_i18n::t("not found"),
        VfsErrorCode::PermissionDenied => rice_i18n::t("permission denied"),
        VfsErrorCode::AlreadyExists => rice_i18n::t("already exists"),
        VfsErrorCode::InvalidInput => rice_i18n::t("invalid input"),
        VfsErrorCode::NotSupported => rice_i18n::t("not supported"),
        VfsErrorCode::Busy => rice_i18n::t("busy"),
        VfsErrorCode::Io => rice_i18n::t("I/O error"),
    };
    let message = error.message.trim();
    if message.is_empty() {
        category.to_owned()
    } else {
        rice_i18n::format("files.error.c3b4205ecc48", &[("category", category), ("message", message)])
    }
}

#[cfg(test)]
#[path = "files_properties_tests.rs"]
mod tests;
