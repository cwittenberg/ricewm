use rice_ui::{Color, Insets, Point, Rect, Size, Theme, Widget, WidgetKind};

use super::super::{
    DrawCommand, HitTarget, MAX_DRAW_COMMANDS, MAX_HIT_TARGETS, NativeUiError, TextAlign,
};

pub(super) const fn interaction(kind: WidgetKind, enabled: bool) -> (bool, bool) {
    if !enabled {
        return (false, false);
    }
    match kind {
        WidgetKind::Button
        | WidgetKind::Toggle
        | WidgetKind::CheckBox
        | WidgetKind::RadioButton
        | WidgetKind::TreeItem
        | WidgetKind::Accordion
        | WidgetKind::Menu
        | WidgetKind::MenuItem => (true, true),
        WidgetKind::TextField
        | WidgetKind::PasswordField
        | WidgetKind::TextEditor
        | WidgetKind::ProgressBar
        | WidgetKind::ScrollBar
        | WidgetKind::Slider
        | WidgetKind::List
        | WidgetKind::Table
        | WidgetKind::VirtualGrid
        | WidgetKind::Tree
        | WidgetKind::IconGrid
        | WidgetKind::Image
        | WidgetKind::WebSurface
            | WidgetKind::SceneSurface => (true, false),
        WidgetKind::TerminalSurface | WidgetKind::TerminalGrid | WidgetKind::HostSurface => {
            (true, false)
        }
        _ => (false, false),
    }
}

pub(super) fn progress_width(text: Option<&str>, total: u32) -> u32 {
    let text = text.map(|text| text.strip_prefix("slim:").unwrap_or(text));
    if let Some(value) = text.and_then(|text| text.strip_prefix("seek:")) {
        let (position, duration) = super::super::scroll::fraction(Some(value));
        return u32::try_from(
            u64::from(total).saturating_mul(u64::from(position)) / u64::from(duration),
        )
        .unwrap_or(total)
        .min(total);
    }
    if let Some((completed, count)) = text.and_then(|text| text.split_once('/'))
        && let (Ok(completed), Ok(count)) = (completed.parse::<u64>(), count.parse::<u64>())
        && count > 0
    {
        return u32::try_from(u64::from(total).saturating_mul(completed.min(count)) / count)
            .unwrap_or(total)
            .min(total);
    }
    let percent = text
        .and_then(|text| text.trim_end_matches('%').parse::<u32>().ok())
        .unwrap_or(0)
        .min(100);
    u32::try_from(u64::from(total) * u64::from(percent) / 100).unwrap_or(total)
}

pub(super) fn is_slim_progress(text: Option<&str>) -> bool {
    text.is_some_and(|text| text.starts_with("slim:"))
}

pub(super) fn bounded_radius(rect: Rect, requested: u16) -> u16 {
    requested.min(u16::try_from(rect.size.width.min(rect.size.height) / 2).unwrap_or(u16::MAX))
}

/// Fits a file-manager icon to its tile while preserving one readable label
/// line. Generic icon grids retain the compact theme metric.
pub(super) fn grid_icon_extent(inset: Rect, theme: &Theme, file_grid_item: bool) -> u32 {
    if !file_grid_item {
        return u32::from(theme.metrics.icon_medium)
            .min(inset.size.width)
            .min(inset.size.height);
    }
    if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
        // Classic Explorer reserves a 48 px resource box whose copied Win98
        // raster contains the native 32 px artwork. Stretching that box to
        // the whole modern tile made both the icon and its caption gap much
        // larger than Explorer's icon view.
        return 48_u32.min(inset.size.width).min(inset.size.height).max(1);
    }
    let label_height = u32::from(theme.typography.body_px)
        .saturating_mul(u32::from(theme.typography.line_height_percent))
        .div_ceil(100)
        .saturating_add(u32::from(theme.spacing.unit));
    inset
        .size
        .height
        .saturating_sub(label_height)
        .min(inset.size.width)
        .max(1)
}

pub(super) fn push_command(
    commands: &mut Vec<DrawCommand>,
    command: DrawCommand,
) -> Result<(), NativeUiError> {
    if commands.len() >= MAX_DRAW_COMMANDS {
        return Err(NativeUiError::ResourceLimit {
            resource: "draw commands",
            maximum: MAX_DRAW_COMMANDS,
        });
    }
    commands.push(command);
    Ok(())
}

/// Paints the asymmetric two-pixel edge used by Windows 95/98 controls.
/// The outer and inner colors intentionally stay literal system colors so a
/// custom palette cannot turn Redmond furniture into a modern single border.
pub(super) fn push_redmond_bevel(
    commands: &mut Vec<DrawCommand>,
    rect: Rect,
    pressed: bool,
) -> Result<(), NativeUiError> {
    let (outer_tl, inner_tl, inner_br, outer_br) = if pressed {
        (
            Color::opaque(0, 0, 0),
            Color::opaque(128, 128, 128),
            Color::opaque(223, 223, 223),
            Color::opaque(255, 255, 255),
        )
    } else {
        (
            Color::opaque(255, 255, 255),
            Color::opaque(223, 223, 223),
            Color::opaque(128, 128, 128),
            Color::opaque(0, 0, 0),
        )
    };
    for (bevel, top_left, bottom_right) in [
        (rect, outer_tl, outer_br),
        (rect.inset(Insets::all(1)), inner_tl, inner_br),
    ] {
        if bevel.size.width == 0 || bevel.size.height == 0 {
            continue;
        }
        for (edge, color) in [
            (
                Rect {
                    origin: bevel.origin,
                    size: Size {
                        width: bevel.size.width,
                        height: 1,
                    },
                },
                top_left,
            ),
            (
                Rect {
                    origin: bevel.origin,
                    size: Size {
                        width: 1,
                        height: bevel.size.height,
                    },
                },
                top_left,
            ),
            (
                Rect {
                    origin: Point {
                        x: bevel.origin.x,
                        y: bevel
                            .origin
                            .y
                            .saturating_add_unsigned(bevel.size.height.saturating_sub(1)),
                    },
                    size: Size {
                        width: bevel.size.width,
                        height: 1,
                    },
                },
                bottom_right,
            ),
            (
                Rect {
                    origin: Point {
                        x: bevel
                            .origin
                            .x
                            .saturating_add_unsigned(bevel.size.width.saturating_sub(1)),
                        y: bevel.origin.y,
                    },
                    size: Size {
                        width: 1,
                        height: bevel.size.height,
                    },
                },
                bottom_right,
            ),
        ] {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: edge,
                    color,
                    radius: 0,
                },
            )?;
        }
    }
    Ok(())
}

pub(super) fn push_target(
    targets: &mut Vec<HitTarget>,
    target: HitTarget,
) -> Result<(), NativeUiError> {
    if targets.len() >= MAX_HIT_TARGETS {
        return Err(NativeUiError::ResourceLimit {
            resource: "hit targets",
            maximum: MAX_HIT_TARGETS,
        });
    }
    targets.push(target);
    Ok(())
}

#[cfg(test)]
mod progress_tests {
    use super::progress_width;

    #[test]
    fn determinate_progress_accepts_the_app_store_percent_contract() {
        assert_eq!(progress_width(Some("2%"), 400), 8);
        assert_eq!(progress_width(Some("43%"), 400), 172);
        assert_eq!(progress_width(Some("100%"), 400), 400);
    }
}

pub(super) fn text_geometry(
    widget: &Widget,
    inset: Rect,
    theme: &Theme,
    content_x: i32,
    parent_kind: Option<WidgetKind>,
    file_grid_item: bool,
) -> (Point, Size, TextAlign) {
    let trailing_icon_width =
        if widget.state.contains(rice_ui::WidgetState::TRAILING_ICON) && widget.icon.is_some() {
            u32::from(theme.metrics.icon_small).saturating_add(u32::from(theme.spacing.unit))
        } else {
            0
        };
    if widget.kind == WidgetKind::Button && parent_kind == Some(WidgetKind::IconGrid) {
        let icon_height = grid_icon_extent(inset, theme, file_grid_item);
        let caption_top =
            if file_grid_item && theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
                // The Win98 rasters have a transparent tail. Let the caption use
                // that tail instead of inserting a second artificial gap.
                icon_height.saturating_sub(6)
            } else {
                icon_height
            };
        return (
            Point {
                x: inset.origin.x,
                y: inset.origin.y.saturating_add_unsigned(caption_top),
            },
            Size {
                width: inset.size.width,
                height: inset.size.height.saturating_sub(caption_top),
            },
            TextAlign::Center,
        );
    }
    if widget.kind == WidgetKind::Button
        && (widget.state.contains(rice_ui::WidgetState::START_ALIGNED)
            || matches!(
                parent_kind,
                Some(WidgetKind::List | WidgetKind::Table | WidgetKind::Tree)
            ))
    {
        return (
            Point {
                x: content_x,
                y: inset.origin.y,
            },
            Size {
                width: inset
                    .origin
                    .x
                    .saturating_add_unsigned(inset.size.width)
                    .saturating_sub(content_x)
                    .try_into()
                    .unwrap_or(0_u32)
                    .saturating_sub(trailing_icon_width),
                height: inset.size.height,
            },
            TextAlign::Start,
        );
    }
    if widget.kind == WidgetKind::Button && widget.icon.is_some()
        && !widget.state.contains(rice_ui::WidgetState::TRAILING_ICON)
        && content_x > inset.origin.x {
        let available = inset.origin.x.saturating_add_unsigned(inset.size.width)
            .saturating_sub(content_x).max(0).cast_unsigned();
        return (Point { x: content_x, y: inset.origin.y }, Size {
            width: super::super::layout::estimated_text_width(widget.text.as_deref().unwrap_or(""), theme).saturating_add(6).min(available),
            height: inset.size.height,
        }, TextAlign::Center);
    }
    if widget.kind == WidgetKind::Button {
        return (
            inset.origin,
            Size {
                width: inset.size.width.saturating_sub(trailing_icon_width),
                height: inset.size.height,
            },
            TextAlign::Center,
        );
    }
    if widget.kind == WidgetKind::Image {
        let icon_height = widget
            .icon
            .map_or(0, |_| u32::from(theme.metrics.icon_medium))
            .min(inset.size.height);
        let label_height = u32::from(theme.metrics.control_height);
        return (
            Point {
                x: inset.origin.x,
                y: inset
                    .origin
                    .y
                    .saturating_add_unsigned(
                        inset
                            .size
                            .height
                            .saturating_sub(icon_height.saturating_add(label_height))
                            / 2,
                    )
                    .saturating_add_unsigned(icon_height),
            },
            Size {
                width: inset.size.width,
                height: label_height.min(inset.size.height),
            },
            TextAlign::Center,
        );
    }
    (
        Point {
            x: content_x,
            y: inset.origin.y,
        },
        Size {
            width: inset
                .origin
                .x
                .saturating_add_unsigned(inset.size.width)
                .saturating_sub(content_x)
                .try_into()
                .unwrap_or(0_u32)
                .saturating_sub(trailing_icon_width),
            height: inset.size.height,
        },
        TextAlign::Start,
    )
}
