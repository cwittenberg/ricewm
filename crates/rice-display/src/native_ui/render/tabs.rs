//! Folder tabs share their open edge and surface with the selected page.
use super::*;
use rice_ui::Color;

pub(super) fn is_plain_tab(widget: &Widget, parent_kind: Option<WidgetKind>) -> bool {
    widget.kind == WidgetKind::Button
        && parent_kind == Some(WidgetKind::Tabs)
        && widget
            .text
            .as_deref()
            .is_some_and(|text| !text.is_empty() && !text.starts_with("panel:"))
        && !super::super::layout_tokens::is_window_control(widget.text.as_deref())
}

// The joined chrome plane must cover the seam even in translucent themes.
pub(super) fn page_color(theme: &Theme, browser: bool) -> Color {
    if theme.chrome_profile == ChromeProfile::Redmond98 {
        Color::opaque(192, 192, 192)
    } else if browser {
        theme.palette.surface_raised.with_alpha(255)
    } else {
        theme.palette.surface.with_alpha(255)
    }
}

pub(super) fn strip_color(theme: &Theme) -> Color {
    if theme.chrome_profile == ChromeProfile::Redmond98 {
        page_color(theme, false)
    } else {
        semantic::composite_over(theme.palette.text.with_alpha(22), theme.palette.surface)
    }
}

/// Familiar's blue system-highlight foreground does not apply to folder tabs:
/// their selected face is the page surface, not the selection surface.
pub(super) fn familiar_ink(theme: &Theme, enabled: bool) -> Color {
    semantic::contrasting_ink(page_color(theme, false)).with_alpha(if enabled { 255 } else { 128 })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_tab(
    rect: Rect,
    stable: bool,
    theme: &Theme,
    selected: bool,
    hovered: bool,
    axis: u8,
    browser: bool,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    // Work toward the page in local coordinates, including vertical strips.
    let (length, depth) = if axis == 0 {
        (rect.size.width, rect.size.height)
    } else {
        (rect.size.height, rect.size.width)
    };
    let local = |u: u32, v: u32, width: u32, height: u32| -> Rect {
        let (x, y, width, height) = match axis {
            1 => (v, u, height, width),
            2 => (
                depth.saturating_sub(v.saturating_add(height)),
                u,
                height,
                width,
            ),
            _ => (u, v, width, height),
        };
        Rect {
            origin: Point {
                x: rect.origin.x.saturating_add_unsigned(x),
                y: rect.origin.y.saturating_add_unsigned(y),
            },
            size: Size { width, height },
        }
    };
    // Plain settings tabs keep their dimensions while selection changes.
    let recess = if selected || stable { 0 } else { 4.min(depth) };
    let tab = local(0, recess, length, depth.saturating_sub(recess));
    let face = if selected {
        page_color(theme, browser)
    } else if hovered {
        semantic::composite_over(theme.palette.focus.with_alpha(28), strip_color(theme))
    } else {
        strip_color(theme)
    };
    let edge = if selected || hovered {
        theme.palette.accent
    } else {
        theme.palette.border
    };
    let border = u32::from(theme.metrics.border_width.max(1))
        .min(length / 2)
        .min(depth / 2);
    let radius = bounded_radius(tab, theme.metrics.corner_radius.min(8));
    // Extend the rounded shape beyond its page edge and clip it there.
    // One continuous stroke gives both shoulders the same antialiased width;
    // no square patches or second crest can leave jagged joins or remnants.
    let extension = u32::from(radius).saturating_add(border).max(2);
    let outline = match axis {
        1 => Rect {
            size: Size {
                width: tab.size.width.saturating_add(extension),
                ..tab.size
            },
            ..tab
        },
        2 => Rect {
            origin: Point {
                x: tab.origin.x.saturating_sub_unsigned(extension),
                ..tab.origin
            },
            size: Size {
                width: tab.size.width.saturating_add(extension),
                ..tab.size
            },
        },
        _ => Rect {
            size: Size {
                height: tab.size.height.saturating_add(extension),
                ..tab.size
            },
            ..tab
        },
    };
    push_command(commands, DrawCommand::PushClip { rect: tab })?;
    push_command(
        commands,
        DrawCommand::Fill {
            rect: outline,
            color: face,
            radius,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect: outline,
            color: edge,
            width: border as u16,
            radius,
        },
    )?;
    if !selected {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: local(0, depth.saturating_sub(border), length, border),
                color: theme.palette.border,
                radius: 0,
            },
        )?;
    }
    push_command(commands, DrawCommand::PopClip)?;
    Ok(())
}

/// Older app trees place a page after the strip instead of inside `Tabs`.
/// Join that page without changing the application protocol or hit targets.
pub(super) fn standalone_page_join(
    tree: &RetainedTree,
    widget: &Widget,
    geometry: &BTreeMap<WidgetId, Rect>,
    rect: Rect,
    theme: &Theme,
) -> Option<Rect> {
    if !matches!(
        widget.kind,
        WidgetKind::Container | WidgetKind::Column | WidgetKind::Row | WidgetKind::Stack
    ) {
        return None;
    }
    let parent = tree.get(widget.parent?)?;
    let index = parent.children().iter().position(|id| *id == widget.id)?;
    let strip = tree.get(*parent.children().get(index.checked_sub(1)?)?)?;
    let strip_rect = geometry.get(&strip.id)?;
    if strip.kind != WidgetKind::Tabs
        || strip
            .children()
            .iter()
            .filter_map(|id| tree.get(*id))
            .any(|child| super::super::layout_tokens::is_tab_page(child.text.as_deref()))
        || super::super::layout_tokens::is_vertical_tabs(strip.text.as_deref())
        || strip_rect
            .origin
            .y
            .saturating_add_unsigned(strip_rect.size.height)
            != rect.origin.y
    {
        return None;
    }
    let active = strip
        .children()
        .iter()
        .filter_map(|id| tree.get(*id))
        .find(|tab| {
            tab.state.contains(WidgetState::SELECTED)
                && (is_plain_tab(tab, Some(WidgetKind::Tabs))
                    || super::super::layout_tokens::is_tab_composite(tab.text.as_deref()))
        })?;
    let active = geometry.get(&active.id)?;
    let border = u32::from(theme.metrics.border_width.max(1)).max(2);
    Rect {
        origin: Point {
            x: active.origin.x.saturating_add_unsigned(border),
            y: rect.origin.y,
        },
        size: Size {
            width: active.size.width.saturating_sub(border * 2),
            height: border,
        },
    }
    .intersection(rect)
}
