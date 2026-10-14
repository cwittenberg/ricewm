//! Shared semantic presentation for native controls.
//!
//! Applications describe intent through bounded `WidgetState` flags. Backends
//! remain responsible for translating those intents through the active theme;
//! application code never embeds colors, pixel sizes, or fonts.

use rice_ui::{ChromeProfile, Color, Theme, Widget, WidgetKind, WidgetState};

use super::super::TextAlign;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Appearance {
    Plain,
    Flat,
    Control,
    Panel,
    Bar,
    SplitLeading,
    SplitTrailing,
}

#[derive(Clone, Copy)]
pub(super) struct VisualState {
    pub(super) state: WidgetState,
    pub(super) appearance: Appearance,
    host_selected: bool,
    host_hovered: bool,
    host_pressed: bool,
}

impl VisualState {
    pub(super) fn of(
        widget: &Widget,
        parent_kind: Option<WidgetKind>,
        status_bar: bool,
        host_selected: bool,
        host_hovered: bool,
        host_pressed: bool,
        split_panel: Option<usize>,
        flat_table_item: bool,
    ) -> Self {
        let flat = matches!(
            (widget.kind, parent_kind),
            (
                WidgetKind::Button,
                Some(WidgetKind::IconGrid | WidgetKind::List | WidgetKind::ToolBar)
            ) | (
                WidgetKind::TreeItem,
                Some(WidgetKind::Tree | WidgetKind::TreeItem)
            ) | (WidgetKind::Menu, Some(WidgetKind::MenuBar))
                | (
                    WidgetKind::MenuItem | WidgetKind::CheckBox | WidgetKind::RadioButton,
                    Some(WidgetKind::Menu)
                )
        ) || widget.kind == WidgetKind::MenuItem || flat_table_item
            || (widget.kind == WidgetKind::Row
                && super::super::layout_tokens::control_for(widget.text.as_deref()).is_some());
        Self {
            state: widget.state,
            host_selected,
            host_hovered,
            host_pressed,
            appearance: if flat {
                Appearance::Flat
            } else if status_bar || matches!(widget.kind, WidgetKind::MenuBar | WidgetKind::ToolBar)
            {
                Appearance::Bar
            } else if split_panel == Some(0) {
                Appearance::SplitLeading
            } else if split_panel.is_some() {
                Appearance::SplitTrailing
            } else if matches!(
                widget.kind,
                WidgetKind::Button
                    | WidgetKind::Accordion
                    | WidgetKind::Toggle
                    | WidgetKind::CheckBox
                    | WidgetKind::RadioButton
                    | WidgetKind::TextField
                    | WidgetKind::PasswordField
                    | WidgetKind::TextEditor
                    | WidgetKind::ProgressBar
                    | WidgetKind::ScrollBar
                    | WidgetKind::Slider
                    | WidgetKind::Menu
            ) {
                Appearance::Control
            } else if matches!(
                widget.kind,
                WidgetKind::Container
                    | WidgetKind::GroupBox
                    | WidgetKind::Accordion
                    | WidgetKind::Dialog
                    | WidgetKind::Popover
                    | WidgetKind::Tooltip
                    | WidgetKind::Notification
            ) {
                Appearance::Panel
            } else {
                Appearance::Plain
            },
        }
    }

    pub(super) fn enabled(self) -> bool {
        self.state.contains(WidgetState::ENABLED)
    }

    pub(super) fn selected(self) -> bool {
        self.host_selected || self.state.contains(WidgetState::SELECTED) || self.pressed()
    }

    pub(super) fn pressed(self) -> bool {
        self.host_pressed || self.state.contains(WidgetState::PRESSED)
    }

    pub(super) fn foreground_selected(self, theme: &Theme) -> bool {
        self.selected()
            && !(theme.chrome_profile == ChromeProfile::YellowJacket
                && (self.pressed()
                    // Tabs and open menu headings use a raised, light surface in
                    // Yellow Jacket. Their descendants inherit the host's
                    // selected state, but not its blue selection surface, so a
                    // white foreground becomes unreadable there.
                    || (self.host_selected && !self.state.contains(WidgetState::SELECTED))))
    }

    pub(super) fn content_background(
        self,
        widget: &Widget,
        theme: &Theme,
        parent_kind: Option<WidgetKind>,
        desktop_item: bool,
        file_grid_item: bool,
        inherited_background: Option<Color>,
    ) -> Option<Color> {
        if super::tabs::is_plain_tab(widget, parent_kind) {
            return Some(super::tabs::page_color(theme, false));
        }
        if !self.foreground_selected(theme) {
            return None;
        }

        if self.host_selected
            && !self.state.contains(WidgetState::SELECTED)
            && !self.state.contains(WidgetState::PRESSED)
            && let Some(background) = inherited_background
        {
            return Some(background);
        }

        // Open menu headings deliberately remain on the raised menubar
        // surface. Desktop icons marked FLAT likewise receive only a focus
        // outline, not a selection fill. Derive ink from the surface that is
        // actually beneath the glyphs instead of assuming every selected
        // control was painted with the selection color.
        if widget.kind == WidgetKind::Menu && parent_kind == Some(WidgetKind::MenuBar) {
            if theme.chrome_profile == ChromeProfile::CommandDeck {
                return Some(composite_over(
                    theme.palette.selection,
                    theme.palette.surface,
                ));
            }
            return Some(theme.palette.surface_raised);
        }
        if desktop_item && has_flat_chrome(widget) {
            return Some(theme.palette.desktop);
        }

        let base = if desktop_item {
            theme.palette.desktop
        } else if file_grid_item || parent_kind == Some(WidgetKind::Menu) {
            theme.palette.surface_raised
        } else {
            theme.palette.surface
        };
        Some(composite_over(theme.palette.selection, base))
    }

    pub(super) fn invalid(self) -> bool {
        self.state.contains(WidgetState::INVALID)
    }

    pub(super) fn hovered(self) -> bool {
        self.host_hovered || self.state.contains(WidgetState::HOVERED)
    }
}

pub(super) fn control_fill(
    widget: &Widget,
    theme: &Theme,
    fallback: Color,
    hovered: bool,
) -> Color {
    if widget.state.contains(WidgetState::DESTRUCTIVE) {
        theme.palette.danger
    } else if widget.state.contains(WidgetState::ACCENT) {
        theme.palette.accent
    } else if widget.state.contains(WidgetState::INFORMATION) {
        theme
            .palette
            .information
            .with_alpha(if hovered { 68 } else { 42 })
    } else if widget.state.contains(WidgetState::SUCCESS) {
        theme
            .palette
            .success
            .with_alpha(if hovered { 68 } else { 42 })
    } else if widget.state.contains(WidgetState::WARNING) {
        theme
            .palette
            .warning
            .with_alpha(if hovered { 72 } else { 46 })
    } else {
        fallback
    }
}

pub(super) fn icon_color(widget: &Widget, theme: &Theme, fallback: Color) -> Color {
    if widget.state.contains(WidgetState::INFORMATION) {
        theme.palette.information
    } else if widget.state.contains(WidgetState::SUCCESS) {
        theme.palette.success
    } else if widget.state.contains(WidgetState::WARNING) {
        theme.palette.warning
    } else if theme.chrome_profile == rice_ui::ChromeProfile::Familiar
        && !widget.state.contains(WidgetState::FLAT)
        && !widget.state.contains(WidgetState::SELECTED)
    {
        // Familiar keeps monochrome glyphs stable on ordinary controls.
        // Selected file rows use their explicit white selection ink; flat
        // desktop items still derive contrast from their wallpaper surface.
        theme.palette.text
    } else {
        fallback
    }
}

pub(super) fn content_color(
    widget: &Widget,
    theme: &Theme,
    enabled: bool,
    background: Option<Color>,
    command_deck_key: bool,
) -> Color {
    // Foreground tokens are theme hints, but the surface underneath is the
    // final authority. This prevents custom/catalog themes from ever pairing
    // black with a dark selection or white with a bright yellow/neon action.
    if background.is_some()
        && theme.chrome_profile == ChromeProfile::Familiar
        && widget.kind != WidgetKind::Menu
        && !widget.state.contains(WidgetState::FLAT)
    {
        // Familiar's system highlight contract is white-on-blue even when a
        // malformed custom palette supplies a dark selection-text hint.
        Color::opaque(255, 255, 255)
    } else if let Some(background) = background {
        contrasting_ink(background)
    } else if widget.kind == rice_ui::WidgetKind::ProgressBar {
        contrasting_ink(theme.palette.accent)
    } else if widget.state.contains(WidgetState::DESTRUCTIVE) {
        contrasting_ink(theme.palette.danger)
    } else if widget.state.contains(WidgetState::ACCENT)
        && widget.state.contains(WidgetState::MUTED)
    {
        if matches!(widget.kind, WidgetKind::Button | WidgetKind::Accordion) {
            contrasting_ink(theme.palette.accent).with_alpha(184)
        } else {
            theme.palette.accent.with_alpha(184)
        }
    } else if widget.state.contains(WidgetState::ACCENT) {
        if matches!(widget.kind, WidgetKind::Button | WidgetKind::Accordion) {
            contrasting_ink(theme.palette.accent)
        } else {
            theme.palette.accent
        }
    } else if theme.chrome_profile == ChromeProfile::CommandDeck && command_deck_key {
        // Command Deck renders ordinary buttons as solid LCARS color keys.
        // Their foreground follows that bright surface, not the dark window.
        contrasting_ink(theme.palette.selection)
    } else if widget.state.contains(WidgetState::MUTED)
        && theme.chrome_profile == ChromeProfile::Redmond98
        && enabled
    {
        // Windows 98 reserves gray system text for unavailable controls.
        // Explanatory copy remains ordinary black text even when the shared
        // semantic tree marks it as visually secondary in modern themes.
        theme.palette.text
    } else if widget.state.contains(WidgetState::MUTED) {
        theme.palette.text_muted.with_alpha(150)
    } else if !enabled {
        theme.palette.text_muted
    } else {
        theme.palette.text
    }
}

pub(super) fn contrasting_ink(background: Color) -> Color {
    let [red, green, blue, _] = background.channels();
    let linear = |channel: u8| {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = linear(red) * 0.2126 + linear(green) * 0.7152 + linear(blue) * 0.0722;
    // Pick whichever of near-black and white has the higher WCAG contrast
    // ratio. Medium saturated blues such as Jade's #3e93f5 look vivid but are
    // already bright enough to require dark button ink.
    if luminance >= 0.179_129 {
        Color::opaque(10, 8, 13)
    } else {
        Color::opaque(255, 255, 255)
    }
}

pub(super) fn composite_over(foreground: Color, background: Color) -> Color {
    let [foreground_red, foreground_green, foreground_blue, alpha] = foreground.channels();
    let [background_red, background_green, background_blue, _] = background.channels();
    let blend = |foreground: u8, background: u8| {
        let alpha = u32::from(alpha);
        let channel =
            u32::from(foreground) * alpha + u32::from(background) * (u32::from(u8::MAX) - alpha);
        u8::try_from((channel + 127) / u32::from(u8::MAX)).unwrap_or(u8::MAX)
    };
    Color::opaque(
        blend(foreground_red, background_red),
        blend(foreground_green, background_green),
        blend(foreground_blue, background_blue),
    )
}

pub(super) fn yellow_jacket_pressed_surface(theme: &Theme) -> Color {
    let [red, green, blue, alpha] = theme.palette.surface_raised.channels();
    Color::rgba(
        u8::try_from(u16::from(red) * 78 / 100).unwrap_or(u8::MAX),
        u8::try_from(u16::from(green) * 78 / 100).unwrap_or(u8::MAX),
        u8::try_from(u16::from(blue) * 78 / 100).unwrap_or(u8::MAX),
        alpha,
    )
}

pub(super) fn text_size(widget: &Widget, theme: &Theme) -> u16 {
    if widget.kind == WidgetKind::Dialog && !widget.state.contains(WidgetState::PROMINENT) {
        theme.typography.title_px
    } else {
        super::super::layout_tokens::preferred_text_size(widget, theme)
    }
}

pub(super) fn text_align(widget: &Widget, fallback: TextAlign) -> TextAlign {
    if widget.state.contains(WidgetState::END_ALIGNED) {
        TextAlign::End
    } else if widget.state.contains(WidgetState::CENTER_ALIGNED) {
        TextAlign::Center
    } else {
        fallback
    }
}

pub(super) fn has_flat_chrome(widget: &Widget) -> bool {
    widget.state.contains(WidgetState::FLAT)
}
