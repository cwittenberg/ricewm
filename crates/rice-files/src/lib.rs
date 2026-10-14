//! Pure file-manager state and commands for `RiceFiles`.
//!
//! This crate never accesses the host or guest filesystem. It validates user
//! commands and emits small adapter intents for the composition root.

mod association;
mod completion;
mod model;
mod navigation;
mod operation;
mod preferences;
mod preview;
mod progress;
mod provider;
mod search;
mod sort;
mod transfer;

pub use completion::{CompletionIntent, DirectoryCompletion, MAX_COMPLETIONS};
pub use model::{
    EntryKind, EntryMetadata, FileManager, ModelError, PaneSlot, PaneState, ScrollPosition,
    TabState, ViewMode, WindowState,
};
pub use navigation::{
    GuestPath, NavigationCommand, NavigationError, NavigationIntent, NavigationPlan,
};
pub use operation::{
    AdapterIntent, ArchiveFormat, CommandId, ConflictAction, ConflictDecision, ConflictError,
    ConflictStrategy, OperationCommand, OperationError, OperationPlan, OperationScope,
};
pub use preferences::{
    Bookmark, BookmarkAvailability, FilePreferences, FolderPreferences, PreferenceError,
    PreferencesIntent,
};
pub use preview::{PreviewCacheKey, PreviewFingerprint, PreviewIntent, PreviewPurpose};
pub use progress::{
    ProgressError, ProgressEvent, ProgressState, ProgressStatus, RecoveryAction, RecoveryRecord,
};
pub use provider::{ConnectIntent, ProviderLocation, ProviderLocationError};
pub use search::{SearchError, SearchIntent, SearchQuery, SearchState, SearchStatus};
pub use sort::{SortDirection, SortKey, SortStrategy, compare_entries, sort_entries};
pub use transfer::{DropAction, DropPlan};

#[cfg(test)]
mod tests;
pub use association::{
    ApplicationAssociation, AssociationError, AssociationRegistry, DesktopEntryError,
    DesktopLaunchIntent, LaunchIntent, MAX_ASSOCIATION_CANDIDATES, MAX_DESKTOP_ENTRY_BYTES,
    infer_media_type,
};

pub use preview::supports_thumbnail;
