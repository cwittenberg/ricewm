//! Shared retained-mode UI primitives for RiceWM-native applications.
//!
//! This crate contains data structures and validation only. Rendering and host
//! integration remain in `rice-display`.

mod catalog;
mod editor;
mod editor_font_metrics;
pub use editor_font_metrics::editor_glyph_advance;
mod file_chooser;
mod geometry;
mod menu;
mod protocol_adapter;
mod theme;
mod tree;
mod widgets;
pub mod terminal;
pub mod virtual_grid;
pub mod grid_frame;
pub mod grid_damage;

pub use catalog::{
    COMPONENT_CATALOG, ComponentCategory, ComponentImplementation, ComponentSpec, component_by_id,
    components_in,
};
pub use editor::{
    EditorText, EditorViewport, EditorWrap, editor_text, editor_text_selection, editor_text_selection_highlighted,
    editor_text_windowed_highlighted,
};
pub use file_chooser::{
    ChooserEffect, FileChooser, FileChooserEntry, FileChooserEventResult, FileChooserMode,
    FileChooserViewIds, FileFilter, Place, compose_file_chooser,
};
pub use geometry::{Alignment, Axis, Constraints, Insets, LinearLayout, Point, Rect, Size};
pub use menu::{MENU_SEPARATOR, MenuText, menu_item};
pub use protocol_adapter::PatchConversionError;
pub use rice_native_app_protocol::WidgetId;
pub use theme::{
    ButtonShape, CatalogTheme, CatalogThemeError, ChromeProfile, Color, FRAME_COLOR_COUNT,
    MAX_CORNER_RADIUS, MENU_COLOR_COUNT, MODERN_CORNER_RADIUS, Metrics, Palette, Spacing,
    TASKBAR_COLOR_COUNT, Theme, Typography,
};
pub use tree::{RetainedTree, TreeError};
pub use widgets::{
    IconId, Patch, ValueChange, Widget, WidgetKind, WidgetSpec, WidgetState, file_icon, folder_icon,
};

pub const MAX_WIDGETS: usize = 16_384;
pub const MAX_ROOTS: usize = 256;
pub const MAX_CHILDREN_PER_WIDGET: usize = 4_096;
pub const MAX_WIDGET_TEXT_BYTES: usize = rice_native_app_protocol::MAX_TEXT_BYTES;
/// Bounded image data uses the existing larger widget-patch wire allowance.
pub const MAX_INLINE_IMAGE_TEXT_BYTES: usize = rice_native_app_protocol::MAX_WIDGET_PATCH_TEXT_BYTES;
pub const MAX_PATCH_BATCH: usize = rice_native_app_protocol::MAX_PATCHES;

#[cfg(test)]
mod tests;
