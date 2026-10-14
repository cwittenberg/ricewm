//! Reusable, bounded guest-VFS file chooser state.

use core::cmp::Ordering;
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, RequestId, VfsErrorCode, VfsOperation, VfsResponse, VfsResult,
};
use std::collections::BTreeMap;

mod actions;
mod collation;
mod view;

pub use view::{FileChooserEventResult, FileChooserViewIds, compose_file_chooser};

const PAGE_SIZE: u16 = 128;
const MAX_ENTRIES: usize = 4_096;
const MAX_SUGGESTIONS: usize = 16;
const MAX_FILENAME_BYTES: usize = 255;
const MAX_FILTERS: usize = 64;
const MAX_PLACES: usize = 128;
const MAX_PLACE_LABEL_CHARS: usize = 80;
const CHOOSER_ROW_HEIGHT: u32 = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChooserSortKey {
    Name,
    Size,
    Type,
    Modified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileChooserMode {
    Open,
    /// Select either a file or a directory. Activating a directory still
    /// navigates into it; the explicit accept button chooses the highlighted
    /// directory, which keeps ordinary double-click navigation intact.
    SelectAny,
    SelectDirectory,
    Save,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileFilter {
    label: String,
    localize_label: bool,
    mime_patterns: Vec<String>,
    extensions: Vec<String>,
    accepts_all: bool,
}

impl FileFilter {
    #[must_use]
    pub fn new(label: impl Into<String>, extensions: &[&str]) -> Self {
        Self {
            label: label.into(),
            localize_label: false,
            mime_patterns: Vec::new(),
            extensions: extensions
                .iter()
                .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
                .filter(|extension| !extension.is_empty())
                .collect(),
            accepts_all: false,
        }
    }

    #[must_use]
    pub fn typed(
        label: impl Into<String>,
        mime_patterns: &[&str],
        fallback_extensions: &[&str],
    ) -> Self {
        Self {
            label: label.into(),
            localize_label: false,
            mime_patterns: mime_patterns
                .iter()
                .map(|pattern| pattern.to_ascii_lowercase())
                .collect(),
            extensions: fallback_extensions
                .iter()
                .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
                .filter(|extension| !extension.is_empty())
                .collect(),
            accepts_all: false,
        }
    }

    #[must_use]
    pub fn all_files(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            localize_label: false,
            mime_patterns: Vec::new(),
            extensions: Vec::new(),
            accepts_all: true,
        }
    }

    #[must_use]
    pub fn label(&self) -> &str {
        if self.localize_label { rice_i18n::t(&self.label) } else { &self.label }
    }

    /// Marks an application-owned label for translation at render time.
    #[must_use]
    pub fn localized(mut self) -> Self {
        self.localize_label = true;
        self
    }

    #[must_use]
    pub fn accepts(&self, name: &str, mime_type: Option<&str>) -> bool {
        if self.accepts_all {
            return true;
        }
        if !self.mime_patterns.is_empty()
            && let Some(mime_type) = mime_type
        {
            return self
                .mime_patterns
                .iter()
                .any(|pattern| mime_matches(pattern, mime_type));
        }
        name.rsplit_once('.').is_some_and(|(_, extension)| {
            self.extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub label: String,
    localize_label: bool,
    pub path: String,
}

impl Place {
    #[must_use]
    pub fn new(label: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            localize_label: false,
            path: path.into(),
        }
    }
    /// A built-in place. User and mount labels should use `new` instead.
    #[must_use]
    pub fn localized(label: impl Into<String>, path: impl Into<String>) -> Self {
        let mut place = Self::new(label, path);
        place.localize_label = true;
        place
    }

    #[must_use]
    pub fn display_label(&self) -> &str {
        if self.localize_label { rice_i18n::t(&self.label) } else { &self.label }
    }

}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChooserEntry {
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_millis: i64,
    pub mime_type: Option<String>,
}

impl From<&DirectoryEntry> for FileChooserEntry {
    fn from(entry: &DirectoryEntry) -> Self {
        Self {
            name: entry.name.clone(),
            kind: entry.metadata.kind,
            size: entry.metadata.size,
            modified_millis: entry.metadata.modified_millis,
            mime_type: entry.mime_type.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HistoryChange {
    Keep,
    Push,
    Move(usize),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingDirectory {
    request: RequestId,
    path: String,
    cursor: u64,
    entries: Vec<FileChooserEntry>,
    history: HistoryChange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingProbe {
    request: RequestId,
    path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Pending {
    Directory(PendingDirectory),
    Probe(PendingProbe),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChooserEffect {
    Render,
    Request(VfsOperation),
    Accepted(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileChooser {
    mode: FileChooserMode,
    directory: String,
    location_text: String,
    filename: String,
    entries: Vec<FileChooserEntry>,
    selection: Option<String>,
    suggestions: Vec<String>,
    filters: Vec<FileFilter>,
    filter_index: usize,
    places: Vec<Place>,
    history: Vec<String>,
    history_index: usize,
    show_hidden: bool,
    show_details: bool,
    sort_key: ChooserSortKey,
    sort_descending: bool,
    pending: Option<Pending>,
    notice: Option<ChooserNotice>,
    overwrite: Option<String>,
    scroll: u32,
    thumbnails: BTreeMap<String, RequestId>,
}

impl FileChooser {
    #[must_use]
    pub fn new(
        mode: FileChooserMode,
        initial_path: &str,
        filters: Vec<FileFilter>,
        places: Vec<Place>,
    ) -> Self {
        let normalized = normalize_absolute(initial_path).unwrap_or_else(|| String::from("/root"));
        let (directory, filename) = match mode {
            FileChooserMode::Open
            | FileChooserMode::SelectAny
            | FileChooserMode::SelectDirectory => {
                let directory = if initial_path.ends_with('/') {
                    normalized
                } else {
                    parent_path(&normalized).to_owned()
                };
                (directory, String::new())
            }
            FileChooserMode::Save => (
                parent_path(&normalized).to_owned(),
                file_name(&normalized).unwrap_or("untitled.txt").to_owned(),
            ),
        };
        Self {
            mode,
            location_text: directory.clone(),
            history: vec![directory.clone()],
            directory,
            filename,
            entries: Vec::new(),
            selection: None,
            suggestions: Vec::new(),
            filters: if filters.is_empty() {
                vec![FileFilter::all_files("All Files").localized()]
            } else {
                filters.into_iter().take(MAX_FILTERS).collect()
            },
            filter_index: 0,
            places: normalized_places(places),
            history_index: 0,
            show_hidden: false,
            show_details: true,
            sort_key: ChooserSortKey::Name,
            sort_descending: false,
            pending: None,
            notice: None,
            overwrite: None,
            scroll: 0,
            thumbnails: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn mode(&self) -> FileChooserMode {
        self.mode
    }

    #[must_use]
    pub const fn show_details(&self) -> bool {
        self.show_details
    }

    pub fn toggle_details(&mut self) {
        self.show_details = !self.show_details;
        self.scroll = 0;
    }

    pub fn set_show_details(&mut self, show: bool) {
        self.show_details = show;
        self.scroll = 0;
    }

    #[must_use]
    pub fn thumbnail(&self, name: &str) -> Option<RequestId> {
        self.thumbnails.get(name).copied()
    }

    pub fn set_thumbnail(&mut self, name: &str, request: RequestId) {
        if self.entries.iter().any(|entry| entry.name == name) {
            self.thumbnails.insert(name.to_owned(), request);
        }
    }

    pub fn clear_thumbnails(&mut self) {
        self.thumbnails.clear();
    }

    /// Bounded image candidates for an embedding application's trusted media
    /// provider. The chooser owns presentation; decoded pixels stay in the
    /// host cache under the returned request IDs.
    #[must_use]
    pub fn preview_candidates(&self) -> Vec<(String, String, String)> {
        self.visible_entries()
            .into_iter()
            .filter(|entry| entry.kind == FileKind::File)
            .filter_map(|entry| {
                let mime = entry
                    .mime_type
                    .clone()
                    .or_else(|| image_mime_for_name(&entry.name))?;
                mime.starts_with("image/").then(|| {
                    (
                        entry.name.clone(),
                        join_path(&self.directory, &entry.name),
                        mime,
                    )
                })
            })
            .take(64)
            .collect()
    }

    #[must_use]
    pub fn directory(&self) -> &str {
        &self.directory
    }

    #[must_use]
    pub fn location_text(&self) -> &str {
        &self.location_text
    }

    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    #[must_use]
    pub fn places(&self) -> &[Place] {
        &self.places
    }

    #[must_use]
    pub fn filters(&self) -> &[FileFilter] {
        &self.filters
    }

    #[must_use]
    pub fn filter_index(&self) -> usize {
        self.filter_index
    }

    #[must_use]
    pub fn suggestions(&self) -> &[String] {
        &self.suggestions
    }

    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|notice| notice.fallback.as_str())
    }

    #[must_use]
    pub fn overwrite_path(&self) -> Option<&str> {
        self.overwrite.as_deref()
    }

    #[must_use]
    pub fn selected_name(&self) -> Option<&str> {
        self.selection.as_deref()
    }

    #[must_use]
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    #[must_use]
    pub fn scroll(&self) -> u32 {
        self.scroll
    }

    /// Replaces provider-backed places without disturbing the current folder.
    pub fn replace_places(&mut self, places: Vec<Place>) {
        self.places = normalized_places(places);
    }

    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.pending.is_some()
    }

    #[must_use]
    pub fn pending_request(&self) -> Option<RequestId> {
        match &self.pending {
            Some(Pending::Directory(pending)) => Some(pending.request),
            Some(Pending::Probe(pending)) => Some(pending.request),
            None => None,
        }
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        self.history_index > 0 && !self.is_busy()
    }

    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        self.history_index + 1 < self.history.len() && !self.is_busy()
    }

    #[must_use]
    pub fn visible_entries(&self) -> Vec<&FileChooserEntry> {
        let filter = &self.filters[self.filter_index];
        let mut entries = self
            .entries
            .iter()
            .filter(|entry| self.show_hidden || !entry.name.starts_with('.'))
            .filter(|entry| {
                entry.kind == FileKind::Directory
                    || filter.accepts(&entry.name, entry.mime_type.as_deref())
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| self.compare_entries(left, right));
        entries
    }

    fn compare_entries(&self, left: &FileChooserEntry, right: &FileChooserEntry) -> Ordering {
        // Folder grouping is stable across every column, matching RiceFiles:
        // changing a metadata sort never scatters navigation targets through
        // ordinary files.
        let kind = u8::from(left.kind != FileKind::Directory)
            .cmp(&u8::from(right.kind != FileKind::Directory));
        if kind != Ordering::Equal {
            return kind;
        }
        let order = match self.sort_key {
            ChooserSortKey::Name => collation::compare(&left.name, &right.name),
            ChooserSortKey::Size => left.size.cmp(&right.size),
            ChooserSortKey::Type => chooser_type_key(left).cmp(&chooser_type_key(right)),
            ChooserSortKey::Modified => left.modified_millis.cmp(&right.modified_millis),
        }
        .then_with(|| collation::compare(&left.name, &right.name));
        if self.sort_descending {
            order.reverse()
        } else {
            order
        }
    }

    fn select_sort(&mut self, key: ChooserSortKey) {
        if self.sort_key == key {
            self.sort_descending = !self.sort_descending;
        } else {
            self.sort_key = key;
            self.sort_descending = false;
        }
        self.scroll = 0;
    }

    fn sort_label(&self, key: ChooserSortKey, label: &str) -> String {
        if self.sort_key != key {
            return label.to_owned();
        }
        format!("{label} {}", if self.sort_descending { '↓' } else { '↑' })
    }
}

fn chooser_type_key(entry: &FileChooserEntry) -> (u8, String) {
    if entry.kind == FileKind::Directory {
        return (0, String::new());
    }
    let extension = entry
        .name
        .rsplit_once('.')
        .map_or("", |(_, extension)| extension)
        .to_ascii_lowercase();
    (1, extension)
}

fn image_mime_for_name(name: &str) -> Option<String> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(
        match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "svg" | "svgz" => "image/svg+xml",
            _ => return None,
        }
        .to_owned(),
    )
}

fn mime_matches(pattern: &str, mime_type: &str) -> bool {
    pattern.eq_ignore_ascii_case(mime_type)
        || pattern.strip_suffix("/*").is_some_and(|prefix| {
            mime_type.len() > prefix.len()
                && mime_type
                    .as_bytes()
                    .get(..prefix.len())
                    .is_some_and(|value| value.eq_ignore_ascii_case(prefix.as_bytes()))
                && mime_type.as_bytes()[prefix.len()] == b'/'
        })
}

fn normalized_places(places: Vec<Place>) -> Vec<Place> {
    let mut normalized = Vec::with_capacity(places.len().min(MAX_PLACES));
    for place in places {
        if normalized.len() >= MAX_PLACES {
            break;
        }
        let Some(path) = normalize_absolute(&place.path) else {
            continue;
        };
        let label = place
            .label
            .trim()
            .chars()
            .take(MAX_PLACE_LABEL_CHARS)
            .collect::<String>();
        if label.is_empty() || normalized.iter().any(|item: &Place| item.path == path) {
            continue;
        }
        normalized.push(Place { label, path, localize_label: place.localize_label });
    }
    normalized
}

fn read_directory(path: String, cursor: u64) -> VfsOperation {
    VfsOperation::ReadDirectory {
        path,
        cursor,
        limit: PAGE_SIZE,
    }
}

fn valid_entry_name(name: &str) -> bool {
    !name.is_empty() && !matches!(name, "." | "..") && !name.contains(['/', '\\', '\0'])
}

fn filename_error(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        Some("Enter a file name")
    } else if matches!(name, "." | "..") {
        Some("The file name cannot be “.” or “..”")
    } else if name.len() > MAX_FILENAME_BYTES {
        Some("The file name is too long")
    } else if name.contains(['/', '\\', '\0']) || name.chars().any(char::is_control) {
        Some("The file name contains an invalid character")
    } else {
        None
    }
}

fn normalize_partial(path: &str) -> Option<String> {
    let trailing = path.ends_with('/');
    let mut normalized = normalize_absolute(path)?;
    if trailing && normalized != "/" {
        normalized.push('/');
    }
    Some(normalized)
}

fn normalize_absolute(path: &str) -> Option<String> {
    if !path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    let normalized = format!("/{}", parts.join("/"));
    (normalized.len() <= rice_native_app_protocol::MAX_PATH_BYTES).then_some(normalized)
}

fn parent_path(path: &str) -> &str {
    let Some((parent, _)) = path.rsplit_once('/') else {
        return "/";
    };
    if parent.is_empty() { "/" } else { parent }
}

fn file_name(path: &str) -> Option<&str> {
    path.rsplit('/').next().filter(|name| !name.is_empty())
}

fn join_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{name}")
    } else {
        format!("{parent}/{name}")
    }
}

fn bounded_error(message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return String::from("VFS operation failed");
    }
    let mut result = message.chars().take(256).collect::<String>();
    if message.chars().count() > 256 {
        result.push('…');
    }
    result
}

#[cfg(test)]
#[path = "file_chooser/tests.rs"]
mod tests;


#[derive(Clone, Debug, Eq, PartialEq)]
struct ChooserNotice {
    message: &'static str,
    arguments: Vec<(&'static str, String)>,
    fallback: String,
    default_error: bool,
}
impl ChooserNotice {
    fn message(message: &'static str) -> Self { Self::formatted(message, Vec::new()) }
    fn formatted(message: &'static str, arguments: Vec<(&'static str, String)>) -> Self {
        let fallback = rice_i18n::with_locale(0, || rice_i18n::format(message, &arguments.iter().map(|(key,value)| (*key,value.as_str())).collect::<Vec<_>>()));
        Self { message, arguments, fallback, default_error: false }
    }
    fn failure(message: &'static str, error: &str) -> Self {
        let mut notice = Self::formatted(message, vec![("error", bounded_error(error))]);
        notice.default_error = error.trim().is_empty();
        notice
    }
    fn render(&self) -> String {
        rice_i18n::format(self.message, &self.arguments.iter().map(|(key,value)|
            (*key, if *key == "error" && self.default_error { rice_i18n::t("VFS operation failed") } else { value.as_str() })
        ).collect::<Vec<_>>())
    }
}
