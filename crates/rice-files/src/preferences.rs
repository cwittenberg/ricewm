//! Persistent file-view and bookmark choices, independent of storage.

use crate::{GuestPath, SortStrategy, ViewMode};
use core::fmt;
use std::collections::BTreeMap;

const MAX_FOLDER_PREFERENCES: usize = 2048;
const MAX_BOOKMARKS: usize = 256;
const MAX_LABEL_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FolderPreferences {
    pub view: ViewMode,
    pub sort: SortStrategy,
    /// `None` inherits the global hidden-file preference.
    pub show_hidden: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BookmarkAvailability {
    Available,
    Missing,
    ProviderUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bookmark {
    pub path: GuestPath,
    pub label: String,
    pub availability: BookmarkAvailability,
    system_root_label: bool,
}

impl Bookmark {
    /// A desktop-owned default, distinct from a user bookmark named "Root".
    #[must_use]
    pub fn system_root() -> Self {
        Self {
            path: GuestPath::root(),
            label: String::from("Root"),
            availability: BookmarkAvailability::Available,
            system_root_label: true,
        }
    }

    /// Public path/label edits discard the display privilege automatically.
    #[must_use]
    pub fn has_system_root_label(&self) -> bool {
        self.system_root_label && self.path.is_root() && self.label == "Root"
    }

    /// # Errors
    ///
    /// Returns an error for an empty or oversized label.
    pub fn new(path: GuestPath, label: impl Into<String>) -> Result<Self, PreferenceError> {
        let label = label.into();
        validate_label(&label)?;
        Ok(Self {
            path,
            label,
            availability: BookmarkAvailability::Available,
            system_root_label: false,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FilePreferences {
    pub show_hidden: bool,
    pub default_folder: FolderPreferences,
    per_folder: BTreeMap<GuestPath, FolderPreferences>,
    bookmarks: Vec<Bookmark>,
}

impl FilePreferences {
    #[must_use]
    pub fn folder(&self, path: &GuestPath) -> FolderPreferences {
        self.per_folder
            .get(path)
            .copied()
            .unwrap_or(self.default_folder)
    }

    #[must_use]
    pub fn shows_hidden_in(&self, path: &GuestPath) -> bool {
        self.folder(path).show_hidden.unwrap_or(self.show_hidden)
    }

    /// # Errors
    ///
    /// Returns [`PreferenceError::FolderLimit`] when the bounded map is full.
    pub fn set_folder(
        &mut self,
        path: GuestPath,
        preferences: FolderPreferences,
    ) -> Result<(), PreferenceError> {
        if !self.per_folder.contains_key(&path) && self.per_folder.len() == MAX_FOLDER_PREFERENCES {
            return Err(PreferenceError::FolderLimit);
        }
        self.per_folder.insert(path, preferences);
        Ok(())
    }

    pub fn remove_folder(&mut self, path: &GuestPath) -> bool {
        self.per_folder.remove(path).is_some()
    }

    #[must_use]
    pub fn bookmarks(&self) -> &[Bookmark] {
        &self.bookmarks
    }

    /// # Errors
    ///
    /// Returns an error for duplicate bookmarks or when the bound is reached.
    pub fn add_bookmark(&mut self, bookmark: Bookmark) -> Result<(), PreferenceError> {
        if self.bookmarks.len() == MAX_BOOKMARKS {
            return Err(PreferenceError::BookmarkLimit);
        }
        if self
            .bookmarks
            .iter()
            .any(|existing| existing.path == bookmark.path)
        {
            return Err(PreferenceError::DuplicateBookmark);
        }
        self.bookmarks.push(bookmark);
        Ok(())
    }

    pub fn remove_bookmark(&mut self, path: &GuestPath) -> bool {
        let Some(index) = self
            .bookmarks
            .iter()
            .position(|bookmark| &bookmark.path == path)
        else {
            return false;
        };
        self.bookmarks.remove(index);
        true
    }

    /// # Errors
    ///
    /// Returns [`PreferenceError::BookmarkIndex`] for an invalid index.
    pub fn move_bookmark(&mut self, from: usize, to: usize) -> Result<(), PreferenceError> {
        if from >= self.bookmarks.len() || to >= self.bookmarks.len() {
            return Err(PreferenceError::BookmarkIndex);
        }
        if from != to {
            let bookmark = self.bookmarks.remove(from);
            self.bookmarks.insert(to, bookmark);
        }
        Ok(())
    }

    /// Keeps deleted/provider-backed targets visible with an explicit state.
    ///
    /// # Errors
    ///
    /// Returns [`PreferenceError::UnknownBookmark`] when no target matches.
    pub fn set_bookmark_availability(
        &mut self,
        path: &GuestPath,
        availability: BookmarkAvailability,
    ) -> Result<(), PreferenceError> {
        let bookmark = self
            .bookmarks
            .iter_mut()
            .find(|bookmark| &bookmark.path == path)
            .ok_or(PreferenceError::UnknownBookmark)?;
        bookmark.availability = availability;
        Ok(())
    }

    #[must_use]
    pub fn persist_intent(&self) -> PreferencesIntent {
        PreferencesIntent::Persist(self.clone())
    }
}

/// Storage adapter action. Encoding and browser persistence live elsewhere.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreferencesIntent {
    Persist(FilePreferences),
}

fn validate_label(label: &str) -> Result<(), PreferenceError> {
    if label.trim().is_empty() {
        Err(PreferenceError::EmptyLabel)
    } else if label.len() > MAX_LABEL_BYTES {
        Err(PreferenceError::LabelTooLong(label.len()))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferenceError {
    EmptyLabel,
    LabelTooLong(usize),
    FolderLimit,
    BookmarkLimit,
    DuplicateBookmark,
    BookmarkIndex,
    UnknownBookmark,
}

impl fmt::Display for PreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PreferenceError {}
