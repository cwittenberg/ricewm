//! Toolkit-neutral host for retained RiceWM-native application windows.
//!
//! Applications own their models and send versioned [`UiPatchEnvelope`] batches.
//! This module validates and applies those batches, then turns the retained tree
//! into geometry, semantic draw commands, and semantic input events. Text and
//! icons deliberately remain unresolved so browser and native render backends
//! can share this layer without shipping a font or raster stack.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use rice_native_app_protocol::{
    AppId, MAX_TEXT_BYTES, PointerEventKind, UiEvent, UiEventEnvelope, UiPatchEnvelope,
    WebRenderPatch, WidgetId, WindowId,
};
use rice_ui::{
    ChromeProfile, Color, Constraints, IconId, Patch, Point, Rect, RetainedTree, Size, Theme,
    ValueChange, WidgetKind, WidgetState,
};
use rice_web_render_protocol::{Command as WebRenderCommand, DisplayList as WebDisplayList};

mod error;
mod input;
pub(crate) mod editor_measurement;
mod layout;
mod layout_measurements;
mod layout_adaptive;
mod layout_overlay;
mod layout_partition;
mod layout_tokens;
mod render;
mod scroll;
mod text_menu;

pub use error::NativeUiError;
use input::contextual_menu;
use layout::{layout_menu_popup, layout_submenu_popup, layout_widget, preferred_size, root_size};
use layout_overlay::{
    dialog_widgets, layout_dialog_overlay, layout_notification_overlay, layout_tooltip_overlay,
    notification_widgets, tooltip_overlay_for_hover, widget_belongs_to,
};
use layout_partition::partition_axis;
use render::{menu_popup_rect, render_menu_popup, render_widget};

pub const MAX_NATIVE_APPS: usize = 64;
pub const MAX_WINDOWS_PER_APP: usize = 32;
pub const MAX_NATIVE_WINDOWS: usize = 256;
pub const MAX_DRAW_COMMANDS: usize = 65_536;
pub const MAX_HIT_TARGETS: usize = rice_ui::MAX_WIDGETS;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DrawCommand {
    IconOverride {rect:Rect,selector:String},
    ThemeSurface { rect: Rect, role: u8, state: u8 },
    Fill {
        rect: Rect,
        color: Color,
        radius: u16,
    },
    Border {
        rect: Rect,
        color: Color,
        width: u16,
        radius: u16,
    },
    /// A one-pixel horizontal or vertical dotted connector, used by classic
    /// retained trees without expanding every dot into its own draw command.
    StrokeLine { rect: Rect, color: Color, width: u16, pattern: u8 },
    DottedLine {
        rect: Rect,
        color: Color,
    },
    ShapedFill {
        rect: Rect,
        color: Color,
        shape: rice_ui::ButtonShape,
    },
    ShapedBorder {
        rect: Rect,
        color: Color,
        width: u16,
        shape: rice_ui::ButtonShape,
    },
    /// A framed group whose legend gap is measured by the final text renderer.
    ///
    /// Rust deliberately does not estimate this gap: the browser owns the
    /// loaded font and therefore the only authoritative glyph advances.
    GroupBoxFrame {
        rect: Rect,
        legend: String,
        color: Color,
        width: u16,
        size_px: u16,
        legend_x: i32,
        /// Absolute center line of the legend text box.
        legend_y: i32,
    },
    /// A tooltip/balloon surface with a directional pointer aimed at its owner.
    BalloonFrame {
        rect: Rect,
        fill: Color,
        border: Color,
        radius: u16,
        caret_x: i32,
        caret_at_top: bool,
    },
    Text {
        rect: Rect,
        text: String,
        color: Color,
        size_px: u16,
        align: TextAlign,
        role: TextRole,
        caret: bool,
        /// Unicode scalar offset of the active caret for single-line editors.
        /// Text editors carry their richer caret in their presentation model.
        caret_offset: Option<u32>,
        brand: bool,
        gradient: bool,
    },
    /// Explicit document font, independent from themed interface text. The
    /// renderer resolves family hints to available fonts and measures underline.
    GridText { zoom_percent:u16, rect: Rect, text: String, color: Color, align: TextAlign, font: Option<rice_ui::grid_frame::GridFont>, size_px: u16, vertical: u8, wrap: bool, strike: bool },
    // Edit alignment wire values: 0 keeps legacy center, 1 top, 2 bottom.
    StyledEdit { zoom_percent:u16, rect: Rect, text: String, color: Color, font: rice_ui::grid_frame::GridFont, caret: Option<u32>, selection: Option<(u32,u32)>, selection_color: Color, vertical: u8 },
    StyledText {
        zoom_percent: u16,
        rect: Rect,
        text: String,
        color: Color,
        align: TextAlign,
        font: rice_ui::grid_frame::GridFont,
    },
    /// Selection paint for a single-line text control.
    ///
    /// The browser/native renderer owns glyph measurement, so this command
    /// carries character offsets instead of an estimated pixel rectangle.
    /// Keeping selection geometry beside text shaping prevents proportional
    /// glyphs (notably punctuation in URLs) from painting past the selected
    /// run.
    TextSelection {
        rect: Rect,
        text: String,
        color: Color,
        size_px: u16,
        align: TextAlign,
        start: u32,
        end: u32,
    },
    Icon {
        rect: Rect,
        icon: IconId,
        color: Color,
        hovered: bool,
        animated: bool,
        /// Paint the conventional shortcut badge over the target icon.
        shortcut: bool,
    },
    ScopedIcon { rect: Rect, icon: IconId, color: Color, hovered: bool },
    Spinner {
        rect: Rect,
        color: Color,
    },
    Activity {
        rect: Rect,
        color: Color,
    },
    Glass {
        rect: Rect,
        tone: u8,
        tint: Color,
        radius: u16,
        hovered: bool,
        control: bool,
        active: bool,
        structural: bool,
        /// A transient menu/context-menu sheet. Popup glass keeps a neutral
        /// elevation shadow and a strict rounded clip instead of decorative
        /// card caustics and luminous rim bloom.
        popup: bool,
    },
    ScenicBackdrop {
        rect: Rect,
    },
    WelcomeJourney {
        rect: Rect,
    },
    PushClip {
        rect: Rect,
    },
    PopClip,
    Media {
        rect: Rect,
        request: u64,
        quarter_turns: u8,
        scale_mode: MediaScaleMode,
        scale_permille: u16,
        pan_x: i16,
        pan_y: i16,
        /// Source crop x/y/width/height, then output width/height after rotation.
        image_edit: Option<[u16; 6]>,
    },
    /// A small, bounded application-provided image such as a page favicon.
    /// The data URL remains below the ordinary widget-text limit and is decoded
    /// by the browser rather than by the Wasm layout engine.
    InlineImage {
        rect: Rect,
        data_url: String,
    },
    WebScene {
        surface: WidgetId,
        rect: Rect,
        epoch: u64,
        viewport_width: u32,
        viewport_height: u32,
        display_list: Arc<[u8]>,
        damage: Option<[u32; 4]>,
        /// Centered aspect-preserving vector viewport; never a browser document.
        vector_scene: bool,
    },
    ColorPicker { rect: Rect, color: Color, hue: bool, hue_degrees: u16 },
    TerminalRow {
        rect: Rect,
        text: String,
    },
    TerminalGrid {
        rect: Rect,
        session: u64,
    },
    TerminalPortal {
        rect: Rect,
        session: u64,
    },
    HostPortal {
        rect: Rect,
        identity: String,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum MediaScaleMode {
    #[default]
    Fit,
    Actual,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum TextRole {
    #[default]
    Label,
    Editor,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HitTarget {
    pub widget: WidgetId,
    pub rect: Rect,
    pub focusable: bool,
    pub activatable: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOutput {
    pub size: Size,
    pub commands: Vec<DrawCommand>,
    pub hit_targets: Vec<HitTarget>,
    pub focused: Option<WidgetId>,
    pub cursor: PointerCursor,
    pub dialog: Option<RenderedDialog>,
    pub popup: Option<RenderedPopup>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u32)]
pub enum PointerCursor {
    #[default]
    Default = 0,
    ColumnResize = 1,
    RowResize = 2,
    Pointer = 3,
    Text = 4,
    Crosshair = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderedDialog {
    pub widget: WidgetId,
    pub rect: Rect,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderedPopup {
    pub rect: Rect,
    pub command_start: usize,
}

fn rect_union(first: Rect, second: Rect) -> Rect {
    let left = first.origin.x.min(second.origin.x);
    let top = first.origin.y.min(second.origin.y);
    let right = first
        .origin
        .x
        .saturating_add_unsigned(first.size.width)
        .max(second.origin.x.saturating_add_unsigned(second.size.width));
    let bottom = first
        .origin
        .y
        .saturating_add_unsigned(first.size.height)
        .max(second.origin.y.saturating_add_unsigned(second.size.height));
    Rect {
        origin: Point { x: left, y: top },
        size: Size {
            width: right.saturating_sub(left).max(0).cast_unsigned(),
            height: bottom.saturating_sub(top).max(0).cast_unsigned(),
        },
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct NativeWindow {
    sequence: u64,
    grid_resync_shadow: Option<RetainedTree>,
    grid_resync_notify: bool,
    tree: RetainedTree,
    tree_expansion: BTreeMap<WidgetId, TreeExpansionOverride>,
    focused: Option<WidgetId>,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
    hover_point: Option<Point>,
    opened_menu: Option<WidgetId>,
    opened_submenus: Vec<WidgetId>,
    menu_scroll_rows: BTreeMap<WidgetId, usize>,
    navigation_open: Option<WidgetId>,
    navigation_offset: u32,
    navigation_maximum: u32,
    navigation_drag: Option<(Point, u32)>,
    navigation_scrolled: bool,
    navigation_headers: BTreeMap<WidgetId, Rect>,
    text_selection_all: Option<WidgetId>,
    text_edit: Option<TextEditState>,
    text_drag: Option<TextDragState>,
    popup_anchor: Option<Point>,
    popup_context_target: Option<WidgetId>,
    scroll_drag: Option<WidgetId>,
    geometry: BTreeMap<WidgetId, Rect>,
    dialog_work_area: Option<Rect>,
    hit_targets: Vec<HitTarget>,
    web_scenes: BTreeMap<WidgetId, WebScene>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PointerVisualState {
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
    focused: Option<WidgetId>,
    hover_point: Option<Point>,
    opened_menu: Option<WidgetId>,
    opened_submenus: Vec<WidgetId>,
    menu_scroll_rows: BTreeMap<WidgetId, usize>,
    navigation_open: Option<WidgetId>,
    navigation_offset: u32,
    navigation_maximum: u32,
    navigation_drag: Option<(Point, u32)>,
    navigation_scrolled: bool,
    navigation_headers: BTreeMap<WidgetId, Rect>,
    text_selection_all: Option<WidgetId>,
    text_edit: Option<TextEditState>,
    text_drag: Option<TextDragState>,
    popup_anchor: Option<Point>,
    scroll_drag: Option<WidgetId>,
    tree_expansion: BTreeMap<WidgetId, TreeExpansionOverride>,
}

impl NativeWindow {
    /// Clone only application-owned UI state while a patch is validated.
    ///
    /// Web scenes are large immutable renderer payloads. Cloning them for every
    /// status-bar or resource-progress patch made patch cost proportional to the
    /// current web page size and exhausted the Wasm heap on complex pages.
    fn clone_for_ui_patch(&self) -> Self {
        Self {
            sequence: self.sequence,
            grid_resync_shadow: None,
            grid_resync_notify: self.grid_resync_notify,
            tree: self.grid_resync_shadow.as_ref().unwrap_or(&self.tree).clone(),
            tree_expansion: self.tree_expansion.clone(),
            focused: self.focused,
            hovered: self.hovered,
            pressed: self.pressed,
            hover_point: self.hover_point,
            opened_menu: self.opened_menu,
            opened_submenus: self.opened_submenus.clone(),
            menu_scroll_rows: self.menu_scroll_rows.clone(),
            navigation_open: self.navigation_open,
            navigation_offset: self.navigation_offset,
            navigation_maximum: self.navigation_maximum,
            navigation_drag: self.navigation_drag,
            navigation_scrolled: self.navigation_scrolled,
            navigation_headers: self.navigation_headers.clone(),
            text_selection_all: self.text_selection_all,
            text_edit: self.text_edit,
            text_drag: self.text_drag,
            popup_anchor: self.popup_anchor,
            popup_context_target: self.popup_context_target,
            scroll_drag: self.scroll_drag,
            geometry: BTreeMap::new(),
            dialog_work_area: self.dialog_work_area,
            hit_targets: Vec::new(),
            web_scenes: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TreeExpansionOverride {
    application_expanded: bool,
    user_expanded: bool,
}

fn reapply_tree_expansion_overrides(
    window: &mut NativeWindow,
    patches: &[Patch],
) -> Result<(), NativeUiError> {
    let mut declared = BTreeMap::new();
    for patch in patches {
        match patch {
            Patch::Create { widget, .. } => {
                declared.insert(widget.id, widget.state.contains(WidgetState::CHECKED));
            }
            Patch::Update {
                id,
                state: Some(state),
                ..
            } => {
                declared.insert(*id, state.contains(WidgetState::CHECKED));
            }
            Patch::Clear => declared.clear(),
            Patch::Update { state: None, .. } | Patch::Move { .. } | Patch::Remove { .. } | Patch::GridDamage { .. } | Patch::ObserveSize { .. } | Patch::EditSelection { .. } | Patch::DocumentFont { .. } | Patch::IconSelector { .. } => {}
        }
    }

    let mut retained = BTreeMap::new();
    let mut updates = Vec::new();
    for (id, expansion) in &window.tree_expansion {
        if !input::tree_item_expandable(&window.tree, *id) {
            continue;
        }
        if declared
            .get(id)
            .is_some_and(|expanded| *expanded != expansion.application_expanded)
        {
            continue;
        }
        retained.insert(*id, *expansion);
        if let Some(widget) = window.tree.get(*id)
            && widget.state.contains(WidgetState::CHECKED) != expansion.user_expanded
        {
            let state =
                WidgetState::from_bits_retain(widget.state.bits() ^ WidgetState::CHECKED.bits());
            updates.push(Patch::Update {
                id: *id,
                state: Some(state),
                text: ValueChange::Keep,
                icon: ValueChange::Keep,
            });
        }
    }
    window.tree_expansion = retained;
    window.tree.apply(&updates)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextEditState {
    widget: WidgetId,
    anchor: usize,
    caret: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextDragState {
    widget: WidgetId,
    anchor: usize,
    moved: bool,
}

fn clamp_text_boundary(value: &str, requested: usize) -> usize {
    let mut boundary = requested.min(value.len());
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebScene {
    epoch: u64,
    viewport_width: u32,
    viewport_height: u32,
    display_list: Arc<[u8]>,
    hover_regions: Vec<WebHoverRegion>,
    damage: Option<[u32; 4]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WebHoverKind {
    Pointer,
    Title(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WebHoverRegion {
    rect_bits: [u32; 4],
    kind: WebHoverKind,
}

impl WebHoverRegion {
    fn rect(&self) -> [f32; 4] {
        self.rect_bits.map(f32::from_bits)
    }
}

fn web_hover_regions(display_list: &[u8]) -> Vec<WebHoverRegion> {
    let Ok(display_list) = WebDisplayList::validate(display_list) else {
        return Vec::new();
    };
    display_list
        .commands()
        .flatten()
        .filter_map(|command| match command {
            WebRenderCommand::InputRegion { rect, flags, .. } if flags & 1 == 0 => {
                Some(WebHoverRegion {
                    rect_bits: rect.map(f32::to_bits),
                    kind: WebHoverKind::Pointer,
                })
            }
            WebRenderCommand::LinkRegion { rect, .. }
            | WebRenderCommand::HitRegion { rect, .. } => Some(WebHoverRegion {
                rect_bits: rect.map(f32::to_bits),
                kind: WebHoverKind::Pointer,
            }),
            WebRenderCommand::TitleRegion { rect, title, .. } => Some(WebHoverRegion {
                rect_bits: rect.map(f32::to_bits),
                kind: WebHoverKind::Title(String::from_utf8_lossy(title).into_owned()),
            }),
            _ => None,
        })
        .collect()
}

fn web_scene_point(scene: &WebScene, surface: Rect, point: Point) -> Option<(f32, f32)> {
    if surface.size.width == 0 || surface.size.height == 0 || !surface.contains(point) {
        return None;
    }
    let x = point.x.saturating_sub(surface.origin.x) as f32 * scene.viewport_width as f32
        / surface.size.width as f32;
    let y = point.y.saturating_sub(surface.origin.y) as f32 * scene.viewport_height as f32
        / surface.size.height as f32;
    Some((x, y))
}

fn web_scene_cursor(scene: &WebScene, surface: Rect, point: Point) -> PointerCursor {
    let Some((x, y)) = web_scene_point(scene, surface, point) else {
        return PointerCursor::Default;
    };
    let contains = |rect: [f32; 4]| {
        rect[2] > 0.0
            && rect[3] > 0.0
            && x >= rect[0]
            && y >= rect[1]
            && x < rect[0] + rect[2]
            && y < rect[1] + rect[3]
    };
    for region in &scene.hover_regions {
        match &region.kind {
            // Every retained semantic control, including editable inputs,
            // follows the same base interaction-cursor contract.
            WebHoverKind::Pointer if contains(region.rect()) => {
                return PointerCursor::Pointer;
            }
            WebHoverKind::Title(_) => {}
            _ => {}
        }
    }
    PointerCursor::Default
}

fn web_scene_title(scene: &WebScene, surface: Rect, point: Point) -> Option<&str> {
    let (x, y) = web_scene_point(scene, surface, point)?;
    scene.hover_regions.iter().rev().find_map(|region| {
        let rect = region.rect();
        let contains = rect[2] > 0.0
            && rect[3] > 0.0
            && x >= rect[0]
            && y >= rect[1]
            && x < rect[0] + rect[2]
            && y < rect[1] + rect[3];
        match &region.kind {
            WebHoverKind::Title(title) if contains && !title.trim().is_empty() => {
                Some(title.as_str())
            }
            _ => None,
        }
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeUiHost {
    pub(crate) compact_form_factor: bool,
    theme: Theme,
    windows: BTreeMap<(AppId, WindowId), NativeWindow>,
}

impl Default for NativeUiHost {
    fn default() -> Self {
        Self::new(Theme::ricewm_default())
    }
}

impl NativeUiHost {
    #[must_use]
    pub fn new(theme: Theme) -> Self {
        Self {
            compact_form_factor: false,
            theme,
            windows: BTreeMap::new(),
        }
    }

    #[must_use]
    pub const fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn set_dialog_work_area(&mut self, app: AppId, window: WindowId, area: Option<Rect>) -> bool {
        let Some(owner) = self.windows.get_mut(&(app, window)) else { return false; };
        owner.dialog_work_area = area;
        true
    }

    pub(crate) fn pointer_visual_state(
        &self,
        app: AppId,
        window: WindowId,
    ) -> Option<PointerVisualState> {
        let native_window = self.windows.get(&(app, window))?;
        let point_sensitive = native_window.hovered.is_some_and(|hovered| {
            native_window
                .tree
                .get(hovered)
                .is_some_and(|widget| matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface))
        }) || native_window.scroll_drag.is_some()
            || native_window.text_drag.is_some();
        Some(PointerVisualState {
            hovered: native_window.hovered,
            pressed: native_window.pressed,
            focused: native_window.focused,
            hover_point: point_sensitive
                .then_some(native_window.hover_point)
                .flatten(),
            opened_menu: native_window.opened_menu,
            opened_submenus: native_window.opened_submenus.clone(),
            menu_scroll_rows: native_window.menu_scroll_rows.clone(),
            navigation_open: native_window.navigation_open,
            navigation_offset: native_window.navigation_offset,
            navigation_maximum: native_window.navigation_maximum,
            navigation_drag: native_window.navigation_drag,
            navigation_scrolled: native_window.navigation_scrolled,
            navigation_headers: native_window.navigation_headers.clone(),
            text_selection_all: native_window.text_selection_all,
            text_edit: native_window.text_edit,
            text_drag: native_window.text_drag,
            popup_anchor: native_window.popup_anchor,
            scroll_drag: native_window.scroll_drag,
            tree_expansion: native_window.tree_expansion.clone(),
        })
    }

    pub(crate) fn pointer_motion_continuous(&self, app: AppId, window: WindowId) -> bool {
        let Some(native_window) = self.windows.get(&(app, window)) else {
            return false;
        };
        native_window.hovered.is_some_and(|hovered| {
            native_window
                .tree
                .get(hovered)
                .is_some_and(|widget| matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface))
        }) || native_window.scroll_drag.is_some()
            || native_window.text_drag.is_some()
    }

    #[must_use]
    pub fn app_count(&self) -> usize {
        self.windows
            .keys()
            .map(|(app, _)| *app)
            .collect::<BTreeSet<_>>()
            .len()
    }

    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    #[must_use]
    pub fn contains_window(&self, app: AppId, window: WindowId) -> bool {
        self.windows.contains_key(&(app, window))
    }

    /// Returns selected UTF-8 text in a focused field or SDK plain/document editor.
    ///
    /// Password fields deliberately never expose their contents. Keeping this
    /// query in the shared UI host means every native application uses the
    /// same pointer, keyboard, and clipboard selection model.
    #[must_use]
    pub fn selected_text(&self, app: AppId, window: WindowId) -> Option<String> {
        let native_window = self.windows.get(&(app, window))?;
        let focused = native_window.focused?;
        let widget = native_window.tree.get(focused)?;
        if widget.kind == WidgetKind::TextEditor {
            // SDK plain and document editors carry scalar selection in their
            // bounded presentation. Rich editors retain actor-owned clipboard
            // behavior; never copy presentation metadata.
            let text = rice_ui::EditorText::parse(widget.text.as_deref()?)?;
            if text.language != "plain" && !text.language.starts_with("document~") {
                return None;
            }
            let (anchor, caret) = text.anchor.zip(text.caret)?;
            let start = anchor.min(caret);
            let end = anchor.max(caret);
            if start == end || end > text.document.chars().count() {
                return None;
            }
            return Some(text.document.chars().skip(start).take(end - start).collect());
        }
        if widget.kind != WidgetKind::TextField {
            return None;
        }
        let value = widget.text.as_deref().unwrap_or_default();
        let edit = native_window
            .text_edit
            .filter(|edit| edit.widget == focused)?;
        let start = clamp_text_boundary(value, edit.anchor.min(edit.caret));
        let end = clamp_text_boundary(value, edit.anchor.max(edit.caret));
        (start < end).then(|| value[start..end].to_owned())
    }

    #[must_use]
    pub fn sequence(&self, app: AppId, window: WindowId) -> Option<u64> {
        self.windows
            .get(&(app, window))
            .map(|native_window| native_window.sequence)
    }

    /// Atomically applies an application patch batch.
    ///
    /// A new window must start at sequence 1. Existing windows accept exactly
    /// the next sequence, which distinguishes replay/stale input from a missing
    /// batch and makes recovery explicit.
    ///
    /// # Errors
    ///
    /// Returns [`NativeUiError`] for sequence, conversion, tree, or host resource
    /// violations. The prior retained tree remains unchanged on every error.
    pub(crate) fn take_grid_resync(&mut self, app: AppId, window: WindowId) -> Option<WidgetId> {
        let value = self.windows.get_mut(&(app, window))?;
        if !value.grid_resync_notify { return None; }
        value.grid_resync_notify = false;
        value.tree.roots().first().copied()
    }

    pub fn apply_patch(&mut self, envelope: &UiPatchEnvelope) -> Result<(), NativeUiError> {
        let key = (envelope.app, envelope.window);
        if let Some(existing) = self.windows.get(&key) {
            let expected = existing
                .sequence
                .checked_add(1)
                .ok_or(NativeUiError::SequenceExhausted)?;
            if envelope.sequence < expected {
                return Err(NativeUiError::StaleSequence {
                    received: envelope.sequence,
                    current: existing.sequence,
                });
            }
            if envelope.sequence > expected {
                return Err(NativeUiError::OutOfOrderSequence {
                    received: envelope.sequence,
                    expected,
                });
            }
        } else {
            if envelope.sequence != 1 {
                return Err(NativeUiError::OutOfOrderSequence {
                    received: envelope.sequence,
                    expected: 1,
                });
            }
            self.validate_new_window_capacity(envelope.app)?;
        }

        let patches = envelope
            .patches
            .iter()
            .map(Patch::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidate = self
            .windows
            .get(&key)
            .map_or_else(NativeWindow::default, NativeWindow::clone_for_ui_patch);
        let semantic_focus_before = first_focused_widget(&candidate.tree);
        let recovering = self.windows.get(&key).is_some_and(|window| window.grid_resync_shadow.is_some());
        let full_snapshot = matches!(patches.first(), Some(Patch::Clear)) && patches.len() > 1
            && patches[1..].iter().all(|patch| matches!(patch,
                Patch::Create { .. } | Patch::ObserveSize { .. } | Patch::EditSelection { .. } | Patch::DocumentFont { .. } | Patch::IconSelector { .. }));
        let base_mismatch = candidate.tree.apply_for_grid_resync(&patches)?;
        if base_mismatch || (recovering && !full_snapshot) {
            let existing = self.windows.get_mut(&key).ok_or(NativeUiError::InvalidPresentation("grid recovery needs existing window"))?;
            if existing.tree.roots().is_empty() { return Err(NativeUiError::InvalidPresentation("grid recovery needs existing root")); }
            existing.sequence = envelope.sequence;
            existing.grid_resync_shadow = Some(candidate.tree);
            if !recovering { existing.grid_resync_notify = true; }
            // Preserve the last valid visible tree and hit geometry until one
            // complete snapshot arrives. Queued updates only advance a private
            // validation shadow and the envelope sequence, never presentation.
            return Ok(());
        }
        if recovering {
            let mut ids = candidate.tree.roots().to_vec();
            while let Some(id) = ids.pop() {
                let widget = candidate.tree.get(id).ok_or(NativeUiError::UnknownWidget(id))?;
                ids.extend_from_slice(widget.children());
                if widget.kind == WidgetKind::VirtualGrid {
                    rice_ui::grid_frame::GridFrame::parse(widget.text.as_deref().unwrap_or(""))
                        .map_err(NativeUiError::InvalidPresentation)?;
                }
            }
        }
        candidate.grid_resync_notify = false;
        reapply_tree_expansion_overrides(&mut candidate, &patches)?;
        candidate.sequence = envelope.sequence;
        candidate.geometry.clear();
        candidate.hit_targets.clear();
        if candidate
            .focused
            .is_some_and(|focused| candidate.tree.get(focused).is_none())
        {
            candidate.focused = None;
        }
        if candidate
            .text_selection_all
            .is_some_and(|selected| candidate.tree.get(selected).is_none())
        {
            candidate.text_selection_all = None;
        }
        if candidate
            .text_edit
            .is_some_and(|edit| candidate.tree.get(edit.widget).is_none())
        {
            candidate.text_edit = None;
        }
        if candidate
            .text_drag
            .is_some_and(|drag| candidate.tree.get(drag.widget).is_none())
        {
            candidate.text_drag = None;
        }
        if let Some(edit) = candidate.text_edit.as_mut()
            && let Some(text) = candidate
                .tree
                .get(edit.widget)
                .and_then(|widget| widget.text.as_deref())
        {
            if candidate.text_selection_all == Some(edit.widget) {
                // The application may replace a compact, unfocused display
                // value with its full editable value in the focus response
                // (for example Bowl's origin-only address text). Preserve the
                // select-all gesture across that semantic value expansion.
                edit.anchor = 0;
                edit.caret = text.len();
            } else {
                edit.anchor = clamp_text_boundary(text, edit.anchor);
                edit.caret = clamp_text_boundary(text, edit.caret);
            }
        }
        if candidate
            .hovered
            .is_some_and(|hovered| candidate.tree.get(hovered).is_none())
        {
            candidate.hovered = None;
        }
        let requested_focus = first_focused_widget(&candidate.tree);
        let semantic_focus_released = semantic_focus_before.is_some()
            && requested_focus.is_none()
            && candidate.focused == semantic_focus_before;
        if semantic_focus_released {
            candidate.focused = None;
            candidate.text_edit = None;
            candidate.text_drag = None;
            candidate.text_selection_all = None;
        }
        // FOCUSED is retained semantic state, not a request to restart the
        // focus gesture on every patch. Re-selecting an already-focused text
        // field here makes each actor echo of ValueChanged select the entire
        // value again, so the next typed character replaces the previous one.
        // An unrelated patch must not undo focus moved by a click or Tab.
        // Recreating a focused widget or explicitly positioning its caret
        // reasserts focus. Formula point selection temporarily focuses the grid;
        // its response restores the editor without recreating that control.
        // Selection requests on fields without FOCUSED never steal focus.
        let adopt_requested_focus = requested_focus.is_some()
            && (requested_focus != semantic_focus_before || candidate.focused.is_none()
                || patches.iter().any(|patch| match patch {
                    Patch::Create { widget, .. } => Some(widget.id) == requested_focus
                        && widget.state.contains(WidgetState::FOCUSED),
                    Patch::EditSelection { id, .. } => Some(*id) == requested_focus,
                    _ => false,
                }));
        let focus_changed = adopt_requested_focus && candidate.focused != requested_focus;
        let select_requested_focus = focus_changed
            && requested_focus.is_some_and(|focused| {
                candidate.tree.get(focused).is_some_and(|widget| {
                    matches!(
                        widget.kind,
                        WidgetKind::TextField | WidgetKind::PasswordField
                    ) && widget.state.contains(WidgetState::SELECT_ALL_ON_FOCUS)
                })
            });
        if select_requested_focus {
            candidate.focused = requested_focus;
            candidate.text_selection_all = requested_focus;
            if let Some(widget) = requested_focus
                && let Some(length) = candidate
                    .tree
                    .get(widget)
                    .and_then(|widget| widget.text.as_deref())
                    .map(str::len)
            {
                candidate.text_edit = Some(TextEditState {
                    widget,
                    anchor: 0,
                    caret: length,
                });
            }
        } else if adopt_requested_focus {
            candidate.focused = requested_focus;
        }
        if candidate
            .scroll_drag
            .is_some_and(|scrollbar| candidate.tree.get(scrollbar).is_none())
        {
            candidate.scroll_drag = None;
        }
        if candidate
            .opened_menu
            .is_some_and(|menu| candidate.tree.get(menu).is_none())
        {
            candidate.opened_menu = None;
            candidate.opened_submenus.clear();
            candidate.popup_anchor = None;
        }
        let mut parent_menu = candidate.opened_menu;
        candidate.opened_submenus.retain(|submenu| {
            let valid = parent_menu.is_some_and(|expected| {
                candidate
                    .tree
                    .get(*submenu)
                    .and_then(|menu| menu.parent)
                    .and_then(|item| candidate.tree.get(item).and_then(|item| item.parent))
                    == Some(expected)
            });
            if valid {
                parent_menu = Some(*submenu);
            }
            valid
        });
        if candidate.opened_menu.is_none() && candidate.popup_anchor.is_some() {
            candidate.opened_menu = contextual_menu(&candidate.tree, candidate.popup_context_target);
        }
        // HoldMenuButton uses a rising SELECTED request. Accept it only for
        // the still-held native command; delayed actors cannot open a popup
        // after release, pointer departure, removal, or disabling the button.
        if let Some(id) = candidate.pressed
            && candidate.tree.get(id).is_some_and(|widget| widget.kind == WidgetKind::Menu
                && widget.state.contains(WidgetState::ENABLED | WidgetState::KEY_NAVIGATION | WidgetState::SELECTED))
            && self.windows.get(&key).and_then(|window| window.tree.get(id))
                .is_some_and(|widget| !widget.state.contains(WidgetState::SELECTED))
        {
            candidate.opened_menu = Some(id);
            candidate.opened_submenus.clear();
            candidate.popup_anchor = None;
            candidate.menu_scroll_rows.insert(id, 0);
            candidate.focused = input::menu_keyboard_items(&candidate.tree, id).first().copied();
        }
        // Selection requests follow text updates in the same atomic patch.
        // The retained tree validates scalar bounds; native editing uses UTF-8
        // offsets internally. Only explicitly subscribed fields emit metadata.
        for patch in &patches {
            if let Patch::EditSelection { id, anchor, caret } = patch {
                if candidate.focused == Some(*id) && let Some(widget) = candidate.tree.get(*id) {
                    let text = widget.text.as_deref().unwrap_or("");
                    let offset = |position: u32| text.char_indices().nth(position as usize).map_or(text.len(), |(byte, _)| byte);
                    candidate.text_edit = Some(TextEditState { widget: *id, anchor: offset(*anchor), caret: offset(*caret) });
                    candidate.text_selection_all = (*anchor != *caret && (*anchor == 0 || *caret == 0)
                        && (*anchor as usize == text.chars().count() || *caret as usize == text.chars().count())).then_some(*id);
                }
            }
        }
        // All fallible validation is complete. Move the large renderer-owned
        // payloads into the candidate instead of copying them during the
        // atomic UI transaction.
        let recreated_scenes = patches.iter().filter_map(|patch| match patch {
            Patch::Create { widget, .. } if widget.kind == WidgetKind::SceneSurface => Some(widget.id),
            _ => None,
        }).collect::<BTreeSet<_>>();
        if let Some(existing) = self.windows.get_mut(&key) {
            candidate.web_scenes = std::mem::take(&mut existing.web_scenes);
            candidate.web_scenes.retain(|surface, _| {
                candidate
                    .tree
                    .get(*surface)
                    .is_some_and(|widget| matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface)
                        && existing.tree.get(*surface).is_some_and(|old| old.kind == widget.kind)
                        // A replacement native control owns a fresh scene/epoch,
                        // even when the SDK preserves its positional widget ID.
                        && !recreated_scenes.contains(surface))
            });
        }
        self.windows.insert(key, candidate);
        Ok(())
    }

    pub(crate) fn has_text_selection_observers(&self, app: AppId, window: WindowId) -> bool {
        self.windows.get(&(app, window)).is_some_and(|window| window.tree.has_text_selection_observers())
    }

    pub(crate) fn accepts_text_selection(&self, app: AppId, window: WindowId, target: WidgetId) -> bool {
        self.windows.get(&(app, window)).is_some_and(|window| window.tree.get(target).is_some_and(|widget|
            widget.kind == WidgetKind::TextEditor || window.tree.observes_text_selection(target)))
    }

    pub(crate) fn text_field_selection(&self, app: AppId, window: WindowId) -> Option<UiEventEnvelope> {
        self.text_field_selection_for_value(app, window, None)
    }

    pub(crate) fn text_field_selection_for_value(&self, app: AppId, window: WindowId, value: Option<&str>) -> Option<UiEventEnvelope> {
        let window_state = self.windows.get(&(app, window))?;
        let target = window_state.focused?;
        if !window_state.tree.observes_text_selection(target) { return None; }
        let widget = window_state.tree.get(target)?;
        // ValueChanged carries the edited value before its actor echo updates
        // the retained tree. Convert the new byte caret against that value.
        let text = value.or(widget.text.as_deref()).unwrap_or("");
        let edit = window_state.text_edit.filter(|edit| edit.widget == target);
        let scalar = |offset| text[..clamp_text_boundary(text, offset)].chars().count() as u32;
        Some(UiEventEnvelope { app, window, target, event: UiEvent::TextSelection {
            anchor: scalar(edit.map_or(text.len(), |edit| edit.anchor)),
            caret: scalar(edit.map_or(text.len(), |edit| edit.caret)),
        } })
    }

    /// Replace the retained scene attached to one browser or vector surface.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown window or surface, a non-surface widget,
    /// an epoch that does not advance, or invalid native vector content.
    pub fn apply_web_render(&mut self, patch: &WebRenderPatch) -> Result<(), NativeUiError> {
        // Resource budgets are authoritative even for callers bypassing the SDK.
        // Account replacement before mutating any scene so a rejected frame
        // preserves both the previous pixels and its accepted epoch.
        let mut owner_bytes = patch.display_list.len();
        let mut total_bytes = patch.display_list.len();
        for ((app, window), state) in &self.windows {
            for (surface, scene) in &state.web_scenes {
                if (*app, *window, *surface) == (patch.app, patch.window, patch.surface) {
                    continue;
                }
                total_bytes = total_bytes.saturating_add(scene.display_list.len());
                if *app == patch.app {
                    owner_bytes = owner_bytes.saturating_add(scene.display_list.len());
                }
            }
        }
        if owner_bytes > 16 * 1024 * 1024 || total_bytes > 64 * 1024 * 1024 {
            return Err(NativeUiError::InvalidPresentation("scene aggregate byte budget"));
        }
        let native_window = self.windows.get_mut(&(patch.app, patch.window)).ok_or(
            NativeUiError::UnknownWindow {
                app: patch.app,
                window: patch.window,
            },
        )?;
        if !native_window
            .tree
            .get(patch.surface)
            .is_some_and(|widget| matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface))
        {
            return Err(NativeUiError::UnknownWidget(patch.surface));
        }
        if native_window.tree.get(patch.surface).is_some_and(|widget| widget.kind == WidgetKind::SceneSurface) {
            if patch.epoch == 0 || !(1..=rice_web_render_protocol::MAX_SCENE_DIMENSION).contains(&patch.viewport_width)
                || !(1..=rice_web_render_protocol::MAX_SCENE_DIMENSION).contains(&patch.viewport_height) {
                return Err(NativeUiError::InvalidPresentation("scene viewport or epoch"));
            }
            rice_web_render_protocol::DisplayList::validate_scene(&patch.display_list)
                .map_err(|_| NativeUiError::InvalidPresentation("vector scene"))?;
        }
        if let Some(current) = native_window.web_scenes.get(&patch.surface)
            && patch.epoch <= current.epoch
        {
            return Err(NativeUiError::StaleSequence {
                received: patch.epoch,
                current: current.epoch,
            });
        }
        // A display list is an immutable frame. Never diff two potentially
        // megabyte-sized command streams on the UI/input turn: doing so made
        // a 100 ms page timer monopolise RiceDisplay and could poison the
        // whole desktop Wasm instance on allocation failure. Producer-owned
        // damage hints can be added without reintroducing that O(frame) work;
        // until then a new web frame invalidates its surface once.
        let damage = None;
        native_window.web_scenes.insert(
            patch.surface,
            WebScene {
                epoch: patch.epoch,
                viewport_width: patch.viewport_width,
                viewport_height: patch.viewport_height,
                display_list: Arc::from(patch.display_list.clone()),
                hover_regions: web_hover_regions(&patch.display_list),
                damage,
            },
        );
        Ok(())
    }

    /// Whether an extant widget is subscribed to native layout feedback.
    pub fn observes_widget_size(&self, app: AppId, window: WindowId, id: WidgetId) -> bool {
        self.windows.get(&(app, window)).is_some_and(|window| window.tree.size_observers().any(|target| target == id))
    }

    /// Current resolved bounds for visible subscribed widgets.
    pub fn observed_widget_sizes(&self, app: AppId, window: WindowId) -> Vec<(WidgetId, Size)> {
        self.windows.get(&(app, window)).map(|window| window.tree.size_observers()
            .filter_map(|id| window.geometry.get(&id).map(|rect| (id, rect.size))).collect()).unwrap_or_default()
    }

    /// Lays out and renders one native window under the supplied constraints.
    ///
    /// # Errors
    ///
    /// Returns [`NativeUiError::UnknownWindow`] or a bounded-output error.
    pub fn render(
        &mut self,
        app: AppId,
        window: WindowId,
        constraints: Constraints,
    ) -> Result<RenderOutput, NativeUiError> {
        let theme = self
            .windows
            .get(&(app, window))
            .map_or(self.theme, |native_window| {
                effective_window_theme(&native_window.tree, self.theme)
            });
        let native_window = self
            .windows
            .get_mut(&(app, window))
            .ok_or(NativeUiError::UnknownWindow { app, window })?;
        let _measurements = layout_measurements::RenderMeasurements::new(&native_window.tree, &theme, self.compact_form_factor);
        layout_measurements::set_navigation_open(native_window.navigation_open);
        layout_measurements::set_navigation_offset(native_window.navigation_offset);
        let size = root_size(&native_window.tree, constraints, &theme);
        native_window.geometry.clear();
        let roots = native_window.tree.roots();
        let gap = u32::from(theme.spacing.unit);
        let root_heights = roots
            .iter()
            .map(|root| preferred_size(&native_window.tree, *root, &theme).height)
            .collect::<Vec<_>>();
        let heights = partition_axis(size.height, gap, &root_heights);
        let mut y = 0_i32;
        for (root, height) in roots.iter().zip(heights) {
            let rect = Rect {
                origin: Point { x: 0, y },
                size: Size {
                    width: size.width,
                    height,
                },
            };
            layout_widget(
                &native_window.tree,
                *root,
                rect,
                &theme,
                &mut native_window.geometry,
            );
            y = y.saturating_add_unsigned(height.saturating_add(gap));
        }
        native_window.navigation_headers = layout_measurements::navigation_headers().into_iter().collect();
        native_window.navigation_maximum = layout_measurements::navigation_maximum();
        if native_window.navigation_open.is_some_and(|id| !native_window.navigation_headers.contains_key(&id)) {
            native_window.navigation_open = None;
            native_window.navigation_drag = None;
            native_window.navigation_offset = 0;
        }
        let dialogs = dialog_widgets(&native_window.tree);
        for dialog in &dialogs {
            layout_dialog_overlay(
                &native_window.tree,
                *dialog,
                size,
                &theme,
                native_window.dialog_work_area,
                &mut native_window.geometry,
            );
        }
        for notification in notification_widgets(&native_window.tree) {
            layout_notification_overlay(
                &native_window.tree,
                notification,
                size,
                &theme,
                &mut native_window.geometry,
            );
        }
        if let Some(dialog) = dialogs.last().copied() {
            if native_window
                .focused
                .is_some_and(|focused| !widget_belongs_to(&native_window.tree, focused, dialog))
            {
                native_window.focused = None;
            }
            if native_window
                .hovered
                .is_some_and(|hovered| !widget_belongs_to(&native_window.tree, hovered, dialog))
            {
                native_window.hovered = None;
            }
            if native_window.focused.is_none() {
                native_window.focused = native_window.geometry.keys().copied().find(|candidate| {
                    widget_belongs_to(&native_window.tree, *candidate, dialog)
                        && native_window
                            .tree
                            .get(*candidate)
                            .is_some_and(|widget| widget.state.contains(WidgetState::FOCUSED))
                });
            }
        }
        if let Some(menu) = native_window.opened_menu {
            layout_menu_popup(
                &native_window.tree,
                menu,
                size,
                native_window.popup_anchor,
                native_window
                    .menu_scroll_rows
                    .get(&menu)
                    .copied()
                    .unwrap_or(0),
                &theme,
                &mut native_window.geometry,
            );
        }
        for submenu in native_window.opened_submenus.iter().copied() {
            if let Some(parent) = native_window.tree.get(submenu).and_then(|menu| menu.parent) {
                layout_submenu_popup(
                    &native_window.tree,
                    submenu,
                    parent,
                    size,
                    native_window.menu_scroll_rows.get(&submenu).copied().unwrap_or(0),
                    &theme,
                    &mut native_window.geometry,
                );
            }
        }
        // A popup owns the pointer interaction, including a held toolbar
        // command whose pointer has not moved yet. Do not paint its old
        // tooltip over the menu's first actionable row. Ordinary hover hints
        // become eligible again when the popup closes.
        let tooltip_hover = native_window.opened_menu.is_none()
            .then_some(native_window.hovered).flatten();
        let tooltip = tooltip_hover
            .and_then(|hovered| tooltip_overlay_for_hover(&native_window.tree, hovered, &native_window.geometry, &theme));
        if let Some((tooltip, anchor)) = tooltip {
            layout_tooltip_overlay(
                &native_window.tree,
                tooltip,
                anchor,
                size,
                &theme,
                &mut native_window.geometry,
            );
        }
        let web_title = tooltip_hover.and_then(|hovered| {
            let widget = native_window.tree.get(hovered)?;
            if widget.kind != WidgetKind::WebSurface {
                return None;
            }
            let point = native_window.hover_point?;
            let surface = native_window.geometry.get(&hovered).copied()?;
            let scene = native_window.web_scenes.get(&hovered)?;
            web_scene_title(scene, surface, point).map(str::to_owned)
        });

        let mut commands = Vec::new();
        let mut hit_targets = Vec::new();
        for root in native_window.tree.roots() {
            render_widget(
                &native_window.tree,
                *root,
                &native_window.geometry,
                &theme,
                native_window.focused,
                native_window.hovered,
                native_window.pressed,
                native_window.opened_menu,
                native_window.text_selection_all,
                native_window.text_edit,
                None,
                &mut commands,
                &mut hit_targets,
            )?;
        }
        for (&id, &rect) in &native_window.navigation_headers {
            hit_targets.retain(|hit| hit.widget != id);
            let title = native_window.tree.get(id).into_iter().flat_map(|widget| widget.children())
                .filter_map(|child| native_window.tree.get(*child))
                .find(|child| child.kind == WidgetKind::Label)
                .and_then(|child| child.text.as_deref()).unwrap_or(rice_i18n::t("Navigation"));
            commands.push(DrawCommand::Fill {rect, color: theme.palette.surface, radius: theme.metrics.corner_radius});
            commands.push(DrawCommand::Border {rect, color: if native_window.focused == Some(id) {theme.palette.focus} else {theme.palette.border}, width:1, radius:theme.metrics.corner_radius});
            commands.push(DrawCommand::Text {rect:rect.inset(rice_ui::Insets::all(10)),text:format!("{}  {}", if native_window.navigation_open == Some(id) {"‹"} else {"☰"}, title),color:theme.palette.text,size_px:theme.typography.body_px,align:TextAlign::Start,role:TextRole::Label,caret:false,caret_offset:None,brand:false,gradient:false});
            hit_targets.push(HitTarget {widget:id,rect,focusable:true,activatable:true});
        }
        let dialog = dialogs.last().copied();
        let popup_above_dialog = dialog.is_some_and(|dialog| {
            native_window
                .opened_menu
                .is_some_and(|menu| widget_belongs_to(&native_window.tree, menu, dialog))
        });
        let mut popup: Option<RenderedPopup> = None;
        if !popup_above_dialog && let Some(menu) = native_window.opened_menu {
            let command_start = commands.len();
            let popup_chain = std::iter::once(menu)
                .chain(native_window.opened_submenus.iter().copied())
                .filter_map(|menu| {
                    menu_popup_rect(&native_window.tree, menu, &native_window.geometry, &theme)
                        .map(|rect| (menu, rect))
                })
                .collect::<Vec<_>>();
            for (depth, (menu, rect)) in popup_chain.iter().copied().enumerate() {
                let mut joined = (false, false);
                if theme.chrome_profile == ChromeProfile::CommandDeck {
                    for neighbor in depth
                        .checked_sub(1)
                        .into_iter()
                        .chain((depth + 1 < popup_chain.len()).then_some(depth + 1))
                    {
                        if popup_chain[neighbor].1.origin.x < rect.origin.x {
                            joined.0 = true;
                        } else {
                            joined.1 = true;
                        }
                    }
                }
                render_menu_popup(
                    &native_window.tree,
                    menu,
                    &native_window.geometry,
                    &theme,
                    native_window.focused,
                    native_window.hovered,
                    native_window.pressed,
                    &mut commands,
                    &mut hit_targets,
                    u8::try_from(depth).unwrap_or(u8::MAX),
                    joined,
                )?;
                popup = Some(RenderedPopup {
                    rect: popup
                        .as_ref()
                        .map_or(rect, |parent| rect_union(parent.rect, rect)),
                    command_start,
                });
            }
        }
        if let Some(dialog) = dialog {
            if theme.chrome_profile != ChromeProfile::Redmond98
                && !native_window.tree.get(dialog).is_some_and(|widget| widget.kind == WidgetKind::Popover
                    || (native_window.dialog_work_area.is_some() && widget.text.as_deref().is_some_and(|text| text.starts_with("panel:drawer-dialog:")))) {
                render::render_modal_scrim(size, &mut commands)?;
            }
            render_widget(
                &native_window.tree,
                dialog,
                &native_window.geometry,
                &theme,
                native_window.focused,
                native_window.hovered,
                native_window.pressed,
                None,
                native_window.text_selection_all,
                native_window.text_edit,
                None,
                &mut commands,
                &mut hit_targets,
            )?;
            hit_targets
                .retain(|target| widget_belongs_to(&native_window.tree, target.widget, dialog));
        }
        if popup_above_dialog && let Some(menu) = native_window.opened_menu {
            let command_start = commands.len();
            let popup_chain = std::iter::once(menu)
                .chain(native_window.opened_submenus.iter().copied())
                .filter_map(|menu| {
                    menu_popup_rect(&native_window.tree, menu, &native_window.geometry, &theme)
                        .map(|rect| (menu, rect))
                })
                .collect::<Vec<_>>();
            for (depth, (menu, rect)) in popup_chain.iter().copied().enumerate() {
                let mut joined = (false, false);
                if theme.chrome_profile == ChromeProfile::CommandDeck {
                    for neighbor in depth
                        .checked_sub(1)
                        .into_iter()
                        .chain((depth + 1 < popup_chain.len()).then_some(depth + 1))
                    {
                        if popup_chain[neighbor].1.origin.x < rect.origin.x {
                            joined.0 = true;
                        } else {
                            joined.1 = true;
                        }
                    }
                }
                render_menu_popup(
                    &native_window.tree,
                    menu,
                    &native_window.geometry,
                    &theme,
                    native_window.focused,
                    native_window.hovered,
                    native_window.pressed,
                    &mut commands,
                    &mut hit_targets,
                    u8::try_from(depth).unwrap_or(u8::MAX),
                    joined,
                )?;
                popup = Some(RenderedPopup {
                    rect: popup
                        .as_ref()
                        .map_or(rect, |parent| rect_union(parent.rect, rect)),
                    command_start,
                });
            }
        }
        if let Some((tooltip, _)) = tooltip {
            render_widget(
                &native_window.tree,
                tooltip,
                &native_window.geometry,
                &theme,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                &mut commands,
                &mut hit_targets,
            )?;
        }
        if let (Some(title), Some(point)) = (web_title.as_deref(), native_window.hover_point) {
            render::render_web_title_tooltip(size, point, title, &mut commands)?;
        }
        // Popups and dialogs append surfaces after the main widget tree. Resolve
        // every retained scene only once all overlay commands have been emitted.
        for command in &mut commands {
            if let DrawCommand::WebScene {
                surface,
                epoch,
                viewport_width,
                viewport_height,
                display_list,
                damage,
                ..
            } = command
                && let Some(scene) = native_window.web_scenes.get(surface)
            {
                *epoch = scene.epoch;
                *viewport_width = scene.viewport_width;
                *viewport_height = scene.viewport_height;
                display_list.clone_from(&scene.display_list);
                *damage = scene.damage;
            }
        }
        native_window.hit_targets.clone_from(&hit_targets);
        let dialog = dialogs.last().and_then(|widget| {
            let retained = native_window.tree.get(*widget)?;
            let rect = native_window.geometry.get(widget).copied()?;
            Some(RenderedDialog {
                widget: *widget,
                rect,
                title: retained
                    .text
                    .clone()
                    .unwrap_or_else(|| String::from("Dialog")),
            })
        });
        let cursor = native_window
            .hovered
            .and_then(|hovered| {
                let widget = native_window.tree.get(hovered)?;
                if widget.kind == WidgetKind::VirtualGrid {
                    // The SDK keeps this state only while a fill drag owns the
                    // pointer. Crossing into destination cells must not lose
                    // the handle's cursor before the operation is committed.
                    if widget.state.contains(WidgetState::PRESSED) {
                        return Some(PointerCursor::Crosshair);
                    }
                    let bounds = native_window.geometry.get(&hovered)?;
                    let point = native_window.hover_point?;
                    let x = point.x.checked_sub(bounds.origin.x)?;
                    let y = point.y.checked_sub(bounds.origin.y)?;
                    let frame = rice_ui::grid_frame::GridFrame::parse(widget.text.as_deref()?).ok()?;
                    if frame.hit_test(x, y).is_some_and(|hit| hit.fill) { return Some(PointerCursor::Crosshair); }
                    if y >= 0 && y < frame.column_header as i32 && x >= frame.row_header as i32
                        && frame.columns.iter().any(|t| i64::from(t.clip_end) == t.start + i64::from(t.extent) && (x - frame.row_header as i32 - t.clip_end as i32).abs() <= 3) {
                        return Some(PointerCursor::ColumnResize);
                    }
                    if x >= 0 && x < frame.row_header as i32 && y >= frame.column_header as i32
                        && frame.rows.iter().any(|t| i64::from(t.clip_end) == t.start + i64::from(t.extent) && (y - frame.column_header as i32 - t.clip_end as i32).abs() <= 3) {
                        return Some(PointerCursor::RowResize);
                    }
                    return None;
                }
                (widget.kind == WidgetKind::SplitPane
                    || (widget.kind == WidgetKind::Table
                        && layout_tokens::resizable_table(widget.text.as_deref())))
                .then(|| {
                    if widget.kind == WidgetKind::Table {
                        return PointerCursor::ColumnResize;
                    }
                    let fallback = native_window
                        .geometry
                        .get(&hovered)
                        .copied()
                        .unwrap_or_default();
                    let handle = render::split_handle_rect(
                        &native_window.tree,
                        widget,
                        &native_window.geometry,
                        fallback,
                    );
                    if handle.size.height >= handle.size.width {
                        PointerCursor::ColumnResize
                    } else {
                        PointerCursor::RowResize
                    }
                })
            })
            .or_else(|| {
                let hovered = native_window.hovered?;
                let widget = native_window.tree.get(hovered)?;
                if !matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface) {
                    return None;
                }
                let point = native_window.hover_point?;
                let surface = native_window.geometry.get(&hovered).copied()?;
                let scene = native_window.web_scenes.get(&hovered)?;
                Some(if widget.kind == WidgetKind::SceneSurface {
                    scene_surface::drawing_cursor(widget.text.as_deref())
                        .unwrap_or_else(|| scene_surface::cursor(scene, surface, point))
                } else { web_scene_cursor(scene, surface, point) })
            })
            .or_else(|| {
                let point = native_window.hover_point?;
                native_window
                    .hit_targets
                    .iter()
                    .rev()
                    // A focusable collection or surface is not itself a
                    // command. Keep the classic default cursor over empty
                    // desktop/list space and reserve the hand for concrete
                    // activation targets and direct manipulation controls.
                    .find(|target| {
                        target.rect.contains(point)
                            && (target.activatable
                                || native_window.tree.get(target.widget).is_some_and(|widget| {
                                    matches!(
                                        widget.kind,
                                        WidgetKind::Slider
                                            | WidgetKind::ScrollBar
                                            | WidgetKind::ScrollView
                                    )
                                }))
                    })
                    .map(|_| PointerCursor::Pointer)
            })
            .unwrap_or_default();
        Ok(RenderOutput {
            size,
            commands,
            hit_targets,
            focused: native_window.focused,
            cursor,
            dialog,
            popup,
        })
    }

    #[must_use]
    pub fn hit_test(&self, app: AppId, window: WindowId, point: Point) -> Option<HitTarget> {
        self.windows
            .get(&(app, window))?
            .hit_targets
            .iter()
            .rev()
            .find(|target| target.rect.contains(point))
            .copied()
    }

    /// Measure a detached, bounded control tree using this host's current theme.
    /// Neither live trees, input queues nor window geometry are changed.
    pub fn measure_detached(&self, envelope: &UiPatchEnvelope, width: u32, owner: Option<(AppId, WindowId)>) -> Result<Size, NativeUiError> {
        let theme = owner.and_then(|key| self.windows.get(&key))
            .map_or(self.theme, |window| effective_window_theme(&window.tree, self.theme));
        let mut scratch = Self::new(theme);
        scratch.apply_patch(envelope)?;
        let window = scratch.windows.get(&(envelope.app,envelope.window)).ok_or(NativeUiError::UnknownWindow{app:envelope.app,window:envelope.window})?;
        let root = *window.tree.roots().first().ok_or(NativeUiError::UnknownWindow{app:envelope.app,window:envelope.window})?;
        let _measurements = layout_measurements::RenderMeasurements::new(&window.tree,&theme,self.compact_form_factor);
        let preferred = layout::preferred_size(&window.tree,root,&theme);
        Ok(Size{width:preferred.width.max(1),height:if width==0 {preferred.height.max(1)} else {layout::preferred_height_for_width(&window.tree,root,width,&theme).max(1)}})
    }

    pub fn remove_window(&mut self, app: AppId, window: WindowId) -> bool {
        self.windows.remove(&(app, window)).is_some()
    }

    pub fn remove_app(&mut self, app: AppId) -> usize {
        let before = self.windows.len();
        self.windows.retain(|(candidate, _), _| *candidate != app);
        before - self.windows.len()
    }

    pub fn clear(&mut self) {
        self.windows.clear();
    }

    fn validate_new_window_capacity(&self, app: AppId) -> Result<(), NativeUiError> {
        if self.windows.len() >= MAX_NATIVE_WINDOWS {
            return Err(NativeUiError::ResourceLimit {
                resource: "native windows",
                maximum: MAX_NATIVE_WINDOWS,
            });
        }
        let app_windows = self
            .windows
            .keys()
            .filter(|(candidate, _)| *candidate == app)
            .count();
        if app_windows >= MAX_WINDOWS_PER_APP {
            return Err(NativeUiError::ResourceLimit {
                resource: "windows per app",
                maximum: MAX_WINDOWS_PER_APP,
            });
        }
        if app_windows == 0 && self.app_count() >= MAX_NATIVE_APPS {
            return Err(NativeUiError::ResourceLimit {
                resource: "native apps",
                maximum: MAX_NATIVE_APPS,
            });
        }
        Ok(())
    }
}

fn effective_window_theme(tree: &RetainedTree, system: Theme) -> Theme {
    let Some(token) = tree
        .roots()
        .first()
        .and_then(|root| tree.get(*root))
        .and_then(|root| root.text.as_deref())
        .filter(|text| text.starts_with("theme:"))
    else {
        return system;
    };
    let mut parts = token.split(':');
    let _prefix = parts.next();
    let second = parts.next().unwrap_or("ricewm");
    let third = parts.next().unwrap_or("system");
    // Retain the original `theme:dark:*` / `theme:light:*` shorthand while
    // also allowing the configured theme family named `dark` to use the
    // explicit `theme:dark:dark|light` form.
    let (family, mode) = if matches!(second, "dark" | "light") && !matches!(third, "dark" | "light")
    {
        ("ricewm", second)
    } else {
        (second, third)
    };
    let dark = mode == "dark";
    let mut theme = match family {
        "familiar" => {
            if dark {
                Theme::familiar_dark()
            } else {
                Theme::familiar_light()
            }
        }
        "dark" => dark_catalog_preview(dark),
        "emerald" => emerald_catalog_preview(dark),
        "frosted-ice" => frosted_ice_catalog_preview(dark),
        "ricewm" => {
            if dark {
                Theme::ricewm_dark()
            } else {
                Theme::ricewm_default()
            }
        }
        _ => system,
    };
    let scale = token
        .split(':')
        .find_map(|part| part.strip_prefix("font-scale="))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(100)
        .clamp(75, 175);
    if token.split(':').any(|part| part == "liquid-frost") {
        theme.palette.surface = theme.palette.surface.with_alpha(190);
        theme.palette.surface_raised = theme.palette.surface_raised.with_alpha(150);
        theme.palette.border = theme.palette.border.with_alpha(190);
        theme.palette.focus = theme.palette.focus.with_alpha(205);
        theme.palette.selection = theme.palette.selection.with_alpha(170);
        theme.palette.accent = theme.palette.accent.with_alpha(205);
        theme.frosted_widgets.buttons = true;
        theme.frosted_widgets.switches = true;
        theme.frosted_widgets.sliders = true;
        theme.frosted_widgets.scrollbars = true;
    }
    theme.typography.body_px = scaled_type(theme.typography.body_px, scale);
    theme.typography.small_px = scaled_type(theme.typography.small_px, scale);
    theme.typography.title_px = scaled_type(theme.typography.title_px, scale);
    theme.typography.display_px = scaled_type(theme.typography.display_px, scale);
    theme
}

fn dark_catalog_preview(dark: bool) -> Theme {
    let mut theme = if dark {
        Theme::ricewm_dark()
    } else {
        Theme::ricewm_default()
    };
    if dark {
        theme.palette.desktop = Color::opaque(16, 21, 28);
        theme.palette.surface = Color::opaque(17, 24, 39);
        theme.palette.surface_raised = Color::opaque(23, 34, 51);
        theme.palette.text = Color::opaque(248, 250, 252);
        theme.palette.text_muted = Color::opaque(166, 177, 194);
        theme.palette.border = Color::opaque(51, 65, 85);
        theme.palette.selection = Color::opaque(72, 39, 48);
    }
    theme.palette.accent = Color::opaque(96, 165, 250);
    theme.palette.focus = Color::opaque(30, 64, 175);
    theme.palette.information = Color::opaque(96, 165, 250);
    theme
}

fn emerald_catalog_preview(dark: bool) -> Theme {
    let mut theme = if dark {
        Theme::ricewm_dark()
    } else {
        Theme::ricewm_default()
    };
    if dark {
        theme.palette.desktop = Color::opaque(5, 8, 7);
        theme.palette.surface = Color::opaque(7, 16, 12);
        theme.palette.surface_raised = Color::opaque(11, 18, 15);
        theme.palette.text = Color::opaque(237, 247, 241);
        theme.palette.text_muted = Color::opaque(147, 166, 155);
        theme.palette.border = Color::opaque(29, 81, 59);
        theme.palette.selection = Color::opaque(11, 59, 42);
    } else {
        theme.palette.desktop = Color::opaque(220, 246, 232);
        theme.palette.surface = Color::opaque(241, 250, 245);
        theme.palette.surface_raised = Color::opaque(255, 255, 255);
        theme.palette.text = Color::opaque(16, 48, 35);
        theme.palette.text_muted = Color::opaque(72, 105, 91);
        theme.palette.border = Color::opaque(151, 210, 184);
        theme.palette.selection = Color::opaque(205, 243, 225);
        theme.palette.selection_text = Color::opaque(16, 72, 49);
        theme.palette.accent_text = Color::opaque(5, 40, 27);
    }
    theme.palette.accent = Color::opaque(52, 211, 153);
    theme.palette.focus = Color::opaque(16, 140, 96);
    theme.palette.information = Color::opaque(33, 150, 143);
    theme.palette.success = Color::opaque(22, 163, 74);
    theme
}

fn frosted_ice_catalog_preview(dark: bool) -> Theme {
    let mut theme = if dark {
        Theme::ricewm_dark()
    } else {
        Theme::ricewm_default()
    };
    if dark {
        theme.palette.desktop = Color::opaque(16, 24, 32);
        theme.palette.surface = Color::opaque(17, 27, 34);
        theme.palette.surface_raised = Color::opaque(23, 36, 44);
        theme.palette.text = Color::opaque(243, 248, 250);
        theme.palette.text_muted = Color::opaque(161, 184, 193);
        theme.palette.border = Color::opaque(49, 80, 90);
        theme.palette.selection = Color::opaque(25, 61, 69);
    } else {
        theme.palette.desktop = Color::opaque(215, 237, 242);
        theme.palette.surface = Color::opaque(239, 248, 250);
        theme.palette.surface_raised = Color::opaque(255, 255, 255);
        theme.palette.text = Color::opaque(20, 52, 62);
        theme.palette.text_muted = Color::opaque(77, 107, 116);
        theme.palette.border = Color::opaque(157, 199, 207);
        theme.palette.selection = Color::opaque(208, 239, 243);
        theme.palette.selection_text = Color::opaque(15, 69, 78);
        theme.palette.accent_text = Color::opaque(10, 53, 59);
    }
    theme.palette.accent = Color::opaque(87, 199, 212);
    theme.palette.focus = Color::opaque(43, 126, 139);
    theme.palette.information = Color::opaque(50, 144, 170);
    theme
}

fn scaled_type(value: u16, scale: u32) -> u16 {
    u16::try_from(u32::from(value).saturating_mul(scale) / 100)
        .unwrap_or(u16::MAX)
        .max(1)
}

fn first_focused_widget(tree: &RetainedTree) -> Option<WidgetId> {
    fn visit(tree: &RetainedTree, id: WidgetId) -> Option<WidgetId> {
        let widget = tree.get(id)?;
        if widget.state.contains(WidgetState::FOCUSED) {
            return Some(id);
        }
        widget
            .children()
            .iter()
            .find_map(|child| visit(tree, *child))
    }

    tree.roots().iter().find_map(|root| visit(tree, *root))
}

#[cfg(test)]
#[path = "native_ui/adaptive_tests.rs"]
mod adaptive_tests;
#[cfg(test)]
#[path = "native_ui/layout_overlay_tests.rs"]
mod layout_overlay_tests;

#[cfg(test)]
#[path = "native_ui/input_tests.rs"]
mod input_tests;

#[cfg(test)]
#[path = "native_ui/menu_tests.rs"]
mod menu_tests;

#[cfg(test)]
#[path = "native_ui/media_tests.rs"]
mod media_tests;

#[cfg(test)]
#[path = "native_ui/semantic_tests.rs"]
mod semantic_tests;

#[cfg(test)]
#[path = "native_ui/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "native_ui/scene_tests.rs"]
mod scene_tests;

#[path = "native_ui/scene_surface.rs"]
mod scene_surface;

#[cfg(test)]
#[path = "native_ui/chooser_scroll_tests.rs"]
mod chooser_scroll_tests;

#[cfg(test)]
#[path = "../../../tests/compact-native-layout.rs"]
mod compact_form_factor_tests;
