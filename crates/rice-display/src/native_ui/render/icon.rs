//! Semantic icon placement kept separate from surface/text rendering.

use rice_ui::{Point, Rect, Size, Theme, Widget, WidgetKind, WidgetState};

use super::{
    helpers::{grid_icon_extent, push_command},
    semantic::{self, Appearance, VisualState},
};
use crate::native_ui::{DrawCommand, NativeUiError};

pub(super) fn render(
    widget: &Widget,
    inset: Rect,
    theme: &Theme,
    visual: VisualState,
    parent_kind: Option<WidgetKind>,
    desktop_item: bool,
    file_grid_item: bool,
    inherited_content_background: Option<rice_ui::Color>,
    commands: &mut Vec<DrawCommand>,
) -> Result<i32, NativeUiError> {
    let Some(icon) = widget.icon else {
        return Ok(inset.origin.x);
    };
    // A toolbar menu must visibly differ from a command button. Reserve space
    // inside its hit target for a theme-owned disclosure icon.
    let menu_disclosure = widget.kind == WidgetKind::Menu
        && !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown)
        && parent_kind == Some(WidgetKind::ToolBar);
    let disclosure_inset = inset;
    let inset = if menu_disclosure {
        Rect { size: Size { width: inset.size.width.saturating_sub(12), ..inset.size }, ..inset }
    } else { inset };
    let (mut icon, shortcut) = icon.shortcut_base();
    let command_deck_key = theme.chrome_profile == rice_ui::ChromeProfile::CommandDeck
        && (widget.kind == WidgetKind::Button
            && (visual.appearance != Appearance::Flat || visual.hovered())
            || widget.kind == WidgetKind::Menu && parent_kind == Some(WidgetKind::MenuBar));
    if widget.kind == WidgetKind::Accordion
        && matches!(
            icon,
            rice_ui::IconId::ChevronDown | rice_ui::IconId::ChevronUp
        )
    {
        icon = if widget.state.contains(WidgetState::CHECKED) {
            rice_ui::IconId::ChevronUp
        } else {
            rice_ui::IconId::ChevronDown
        };
    }
    let toolbar = matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
        && parent_kind == Some(WidgetKind::ToolBar)
        && !widget.state.contains(WidgetState::START_ALIGNED)
        && theme.chrome_profile != rice_ui::ChromeProfile::Redmond98;
    let icon_only_button = matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu | WidgetKind::MenuItem)
        && widget
            .text
            .as_deref()
            .is_none_or(|text| text.is_empty() || text.starts_with("panel:"));
    let grid = widget.kind == WidgetKind::Button && parent_kind == Some(WidgetKind::IconGrid);
    let image = widget.kind == WidgetKind::Image;
    let trailing = widget.state.contains(WidgetState::TRAILING_ICON);
    let requested = if (image || theme.chrome_profile == rice_ui::ChromeProfile::Redmond98)
        && widget.state.contains(WidgetState::LARGE_ICON)
    {
        large_icon_extent(widget, theme)
    } else if widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::DISPLAY)
    {
        u32::from(theme.metrics.icon_large)
            .saturating_add(u32::from(theme.spacing.section).saturating_mul(4))
    } else if image && widget.state.contains(WidgetState::DISPLAY) {
        u32::from(theme.metrics.icon_large)
    } else if desktop_item {
        if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
            && matches!(
                icon,
                rice_ui::IconId::Trash | rice_ui::IconId::TrashFull | rice_ui::IconId::Link
            )
        {
            // The copied recycle-bin and link rasters fill their entire
            // resource, while the other classic 48px resources retain
            // transparent padding around roughly 32px artwork. Normalize
            // their visible size without changing another theme.
            32_u32
        } else {
            48_u32
        }
    } else if grid {
        grid_icon_extent(inset, theme, file_grid_item)
    } else if toolbar || image || (widget.kind == WidgetKind::MenuItem && icon_only_button)
        || (parent_kind == Some(WidgetKind::ToolBar) && matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)) {
        u32::from(theme.metrics.icon_medium)
    } else {
        u32::from(theme.metrics.icon_small)
    };
    let size = requested.min(inset.size.width).min(inset.size.height);
    let centered_x = inset
        .origin
        .x
        .saturating_add_unsigned(inset.size.width.saturating_sub(size) / 2);
    let labeled_image = image
        && widget.text.as_deref().is_some_and(|text| {
            !text.is_empty()
                && !text.starts_with("media-request:")
                && !text.starts_with("inline-image:")
        });
    let centered_label = widget.kind == WidgetKind::Button && !toolbar && !grid
        && !icon_only_button && !trailing
        && !widget.state.contains(WidgetState::START_ALIGNED)
        && !matches!(parent_kind, Some(WidgetKind::List | WidgetKind::Table | WidgetKind::Tree));
    let gap = u32::from(theme.spacing.unit).max(super::super::layout::ICON_TEXT_GAP);
    let label_width = super::super::layout::estimated_text_width(widget.text.as_deref().unwrap_or(""), theme).saturating_add(6)
        .min(inset.size.width.saturating_sub(size.saturating_add(gap)));
    let icon_x = if centered_label {
        inset.origin.x.saturating_add_unsigned(inset.size.width.saturating_sub(size.saturating_add(gap).saturating_add(label_width)) / 2)
    } else { inset.origin.x };
    let y = icon_y(inset, size, theme, grid, labeled_image);
    let mut color = semantic::icon_color(
        widget,
        theme,
        if super::tabs::is_plain_tab(widget, parent_kind)
            && theme.chrome_profile == rice_ui::ChromeProfile::Familiar
        {
            super::tabs::familiar_ink(theme, visual.enabled())
        } else { semantic::content_color(
            widget,
            theme,
            visual.enabled(),
            visual.content_background(
                widget,
                theme,
                parent_kind,
                desktop_item,
                file_grid_item,
                inherited_content_background,
            ),
            command_deck_key,
        ) },
    );
    // Classic tree selection covers the caption only. The icon remains on
    // the white tree canvas, so it must retain its ordinary foreground.
    if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
        && widget.kind == WidgetKind::TreeItem
        && visual.selected()
    {
        color = theme.palette.text;
        if !visual.enabled() { color = color.with_alpha(110); }
    }
    // Win98 applies a stippled selection mask to the icon while limiting the
    // solid highlight to its caption. Theme raster icons consume this alpha,
    // allowing the teal desktop to show through without drawing a modern
    // full-cell selection rectangle.
    if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
        && (desktop_item || file_grid_item)
        && visual.selected()
    {
        color = color.with_alpha(176);
    }
    push_command(
        commands,
        DrawCommand::Icon {
            rect: Rect {
                origin: Point {
                    x: if trailing {
                        inset
                            .origin
                            .x
                            .saturating_add_unsigned(inset.size.width.saturating_sub(size))
                    } else if toolbar
                        || icon_only_button
                        || grid
                        || (image && !widget.state.contains(WidgetState::START_ALIGNED))
                    {
                        centered_x
                    } else {
                        icon_x
                    },
                    y,
                },
                size: Size {
                    width: size,
                    height: size,
                },
            },
            icon,
            color,
            hovered: visual.hovered(),
            animated: widget.state.contains(WidgetState::AMBIENT_MOTION),
            shortcut,
        },
    )?;
    if menu_disclosure {
        let extent = 8_u32.min(disclosure_inset.size.width).min(disclosure_inset.size.height);
        push_command(commands, DrawCommand::Icon {
            rect: Rect {
                origin: Point {
                    x: disclosure_inset.origin.x.saturating_add_unsigned(disclosure_inset.size.width.saturating_sub(extent)),
                    y: disclosure_inset.origin.y.saturating_add_unsigned(disclosure_inset.size.height.saturating_sub(extent) / 2),
                },
                size: Size { width: extent, height: extent },
            },
            icon: rice_ui::IconId::ChevronDown,
            color,
            hovered: visual.hovered(), animated: false, shortcut: false,
        })?;
    }
    Ok(if toolbar || icon_only_button || grid || trailing {
        inset.origin.x
    } else {
        icon_x.saturating_add_unsigned(
            size.saturating_add(
                u32::from(theme.spacing.unit).max(super::super::layout::ICON_TEXT_GAP),
            ),
        )
    })
}

/// Keep decoded media that represents an icon in the same bounded box as its
/// semantic fallback. Standalone images without a fallback continue to own
/// their full layout rectangle (previews, artwork, and video frames rely on
/// that behavior).
pub(super) fn media_rect(widget: &Widget, inset: Rect, theme: &Theme) -> Rect {
    if widget.kind != WidgetKind::Image || widget.icon.is_none() {
        return inset;
    }
    let requested = if widget.state.contains(WidgetState::LARGE_ICON) {
        large_icon_extent(widget, theme)
    } else if widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::DISPLAY)
    {
        u32::from(theme.metrics.icon_large)
            .saturating_add(u32::from(theme.spacing.section).saturating_mul(4))
    } else if widget.state.contains(WidgetState::DISPLAY) {
        u32::from(theme.metrics.icon_large)
    } else {
        u32::from(theme.metrics.icon_medium)
    };
    let size = requested.min(inset.size.width).min(inset.size.height);
    Rect {
        origin: Point {
            x: inset
                .origin
                .x
                .saturating_add_unsigned(inset.size.width.saturating_sub(size) / 2),
            y: inset
                .origin
                .y
                .saturating_add_unsigned(inset.size.height.saturating_sub(size) / 2),
        },
        size: Size {
            width: size,
            height: size,
        },
    }
}

fn large_icon_extent(widget: &Widget, theme: &Theme) -> u32 {
    u32::from(theme.metrics.icon_medium)
        .saturating_mul(4)
        .saturating_mul(super::super::layout_tokens::zoom_permille(
            widget.text.as_deref(),
        ))
        / 1_000
}

fn icon_y(inset: Rect, size: u32, theme: &Theme, grid: bool, image: bool) -> i32 {
    if grid {
        inset.origin.y
    } else if image {
        inset.origin.y.saturating_add_unsigned(
            inset
                .size
                .height
                .saturating_sub(size.saturating_add(u32::from(theme.metrics.control_height)))
                / 2,
        )
    } else {
        inset
            .origin
            .y
            .saturating_add_unsigned(inset.size.height.saturating_sub(size) / 2)
    }
}
