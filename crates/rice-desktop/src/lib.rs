//! Pure, host-independent `RiceDesktop` domain model.
//!
//! Browser, guest-VFS, image decoding, and window-manager integration live
//! behind [`DesktopIntent`]. This crate owns deterministic desktop policy.

#![forbid(unsafe_code)]

pub mod events;
pub mod grid;
pub mod menu;
pub mod model;
pub mod selection;
pub mod wallpaper;

pub use events::{
    CoalescedEvent, DesktopEventCoalescer, EventPath, MonitorEvent, MountId, MountRecord,
};
pub use grid::{Cell, GridConfig, GridLayout, GridPlacement, GridResizeReport};
pub use menu::{ContextMenuPolicy, MenuEntry, MenuRequest};
pub use model::{
    DesktopCommand, DesktopIntent, DesktopItem, DesktopItemId, DesktopItemKind, DesktopModel,
    ItemAction, ModelError, ModelUpdate,
};
pub use selection::{ClickMode, Direction, SelectionState};
pub use wallpaper::{
    Color, ImagePlacement, Rect, Size, Wallpaper, WallpaperLayout, WallpaperMode,
    WallpaperSettings, WorkspaceId,
};
