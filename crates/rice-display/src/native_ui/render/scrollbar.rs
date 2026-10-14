//! Theme-native scrollbar track and proportional thumb rendering.

use rice_ui::{ChromeProfile, Color, Point, Rect, Size, Theme, Widget, WidgetState};

use super::super::{DrawCommand, NativeUiError, scroll};
use super::helpers::{bounded_radius, push_command, push_redmond_bevel};

pub(super) fn render(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if let Some(text) = widget.text.as_deref()
        && let Some(digits) = text.strip_prefix("color-plane:#").or_else(|| text.strip_prefix("color-hue:#"))
        && let Some(rgb) = parse_rgb(digits.split('@').next().unwrap_or(digits)) {
        return push_command(commands, DrawCommand::ColorPicker { rect, color: Color::opaque(rgb[0],rgb[1],rgb[2]), hue: text.starts_with("color-hue:"), hue_degrees: digits.split('@').nth(1).and_then(|s|s.parse::<u16>().ok()).unwrap_or(0).min(360) });
    }
    if let Some(selected) = widget
        .text
        .as_deref()
        .and_then(|text| text.strip_prefix("color-picker:#"))
        .and_then(parse_rgb)
    {
        return render_color_picker(rect, selected, theme, commands);
    }
    let rect = if widget.state.contains(WidgetState::FLAT) {
        scroll::compact_track(rect)
    } else {
        rect
    };
    let extent = scroll::thumb_extent(rect, widget.text.as_deref());
    let (position, total) = scroll::fraction(widget.text.as_deref());
    render_fraction(
        rect,
        position,
        total,
        extent,
        widget.state.contains(WidgetState::HOVERED) || widget.state.contains(WidgetState::PRESSED),
        theme,
        commands,
    )
}

pub(super) fn render_scroll_view(
    rect: Rect,
    position: u32,
    viewport_extent: u32,
    content_extent: u32,
    hovered: bool,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let maximum = content_extent.saturating_sub(viewport_extent);
    if maximum == 0 {
        return Ok(());
    }
    let major = rect.size.height;
    let extent = u32::try_from(
        u64::from(major).saturating_mul(u64::from(viewport_extent))
            / u64::from(content_extent.max(1)),
    )
    .unwrap_or(major)
    .max(rect.size.width)
    .min(major);
    render_fraction(
        rect,
        position.min(maximum),
        maximum,
        extent,
        hovered,
        theme,
        commands,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_fraction(
    rect: Rect,
    position: u32,
    total: u32,
    extent: u32,
    active: bool,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if theme.chrome_profile == ChromeProfile::Redmond98 {
        return render_redmond_fraction(rect, position, total, extent, theme, commands);
    }
    if theme.chrome_profile == ChromeProfile::YellowJacket {
        return render_yellow_jacket_fraction(
            rect, position, total, extent, active, theme, commands,
        );
    }
    if theme.frosted_widgets.scrollbars {
        push_command(
            commands,
            DrawCommand::Glass {
                rect,
                tone: 0,
                tint: theme.palette.surface_raised,
                radius: bounded_radius(rect, theme.metrics.corner_radius),
                hovered: active,
                control: true,
                active: false,
                structural: false,
                popup: false,
            },
        )?;
    } else {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: theme.palette.surface_raised,
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
        push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: theme.palette.border,
                width: theme.metrics.border_width.max(1),
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
    }
    let vertical = rect.size.height > rect.size.width;
    let major = if vertical {
        rect.size.height
    } else {
        rect.size.width
    };
    let minor = if vertical {
        rect.size.width
    } else {
        rect.size.height
    };
    let extent = extent.max(minor).min(major);
    let travel = major.saturating_sub(extent);
    let offset = u32::try_from(
        u64::from(travel).saturating_mul(u64::from(position)) / u64::from(total.max(1)),
    )
    .unwrap_or(travel);
    let thumb = Rect {
        origin: Point {
            x: if vertical {
                rect.origin.x
            } else {
                rect.origin.x.saturating_add_unsigned(offset)
            },
            y: if vertical {
                rect.origin.y.saturating_add_unsigned(offset)
            } else {
                rect.origin.y
            },
        },
        size: Size {
            width: if vertical { minor } else { extent },
            height: if vertical { extent } else { minor },
        },
    };
    if theme.frosted_widgets.scrollbars {
        push_command(
            commands,
            DrawCommand::Glass {
                rect: thumb,
                tone: if active { 3 } else { 0 },
                tint: if active {
                    theme.palette.accent
                } else {
                    theme.palette.text_muted
                },
                radius: bounded_radius(thumb, theme.metrics.corner_radius),
                hovered: active,
                control: true,
                active,
                structural: false,
                popup: false,
            },
        )
    } else {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: thumb,
                color: if active {
                    theme.palette.accent
                } else {
                    theme.palette.text_muted
                },
                radius: bounded_radius(thumb, theme.metrics.corner_radius),
            },
        )
    }
}

fn render_redmond_fraction(
    rect: Rect,
    position: u32,
    total: u32,
    extent: u32,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let vertical = rect.size.height > rect.size.width;
    let major = if vertical {
        rect.size.height
    } else {
        rect.size.width
    };
    let minor = if vertical {
        rect.size.width
    } else {
        rect.size.height
    };
    let arrow = minor.min(major / 2).max(1);
    let track_major = major.saturating_sub(arrow.saturating_mul(2));
    push_command(
        commands,
        DrawCommand::Fill {
            rect,
            color: Color::opaque(223, 223, 223),
            radius: 0,
        },
    )?;
    let leading = Rect {
        origin: rect.origin,
        size: Size {
            width: if vertical { minor } else { arrow },
            height: if vertical { arrow } else { minor },
        },
    };
    let trailing = Rect {
        origin: Point {
            x: if vertical {
                rect.origin.x
            } else {
                rect.origin
                    .x
                    .saturating_add_unsigned(major.saturating_sub(arrow))
            },
            y: if vertical {
                rect.origin
                    .y
                    .saturating_add_unsigned(major.saturating_sub(arrow))
            } else {
                rect.origin.y
            },
        },
        size: leading.size,
    };
    for button in [leading, trailing] {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: button,
                color: Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        push_redmond_bevel(commands, button, false)?;
    }
    render_pixel_arrow(leading, vertical, true, theme.palette.text, commands)?;
    render_pixel_arrow(trailing, vertical, false, theme.palette.text, commands)?;
    if track_major == 0 {
        return Ok(());
    }
    let thumb_extent = u32::try_from(
        u64::from(extent).saturating_mul(u64::from(track_major)) / u64::from(major.max(1)),
    )
    .unwrap_or(track_major)
    .max(minor.min(track_major))
    .min(track_major);
    let travel = track_major.saturating_sub(thumb_extent);
    let offset = u32::try_from(
        u64::from(travel).saturating_mul(u64::from(position)) / u64::from(total.max(1)),
    )
    .unwrap_or(travel);
    let thumb = Rect {
        origin: Point {
            x: if vertical {
                rect.origin.x
            } else {
                rect.origin.x.saturating_add_unsigned(arrow + offset)
            },
            y: if vertical {
                rect.origin.y.saturating_add_unsigned(arrow + offset)
            } else {
                rect.origin.y
            },
        },
        size: Size {
            width: if vertical { minor } else { thumb_extent },
            height: if vertical { thumb_extent } else { minor },
        },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect: thumb,
            color: Color::opaque(192, 192, 192),
            radius: 0,
        },
    )?;
    push_redmond_bevel(commands, thumb, false)
}

#[allow(clippy::too_many_arguments)]
fn render_yellow_jacket_fraction(
    rect: Rect,
    position: u32,
    total: u32,
    extent: u32,
    active: bool,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let vertical = rect.size.height > rect.size.width;
    let major = if vertical {
        rect.size.height
    } else {
        rect.size.width
    };
    let minor = if vertical {
        rect.size.width
    } else {
        rect.size.height
    };
    let arrow_extent = minor.min(major / 2).max(1);
    let track_major = major.saturating_sub(arrow_extent.saturating_mul(2));
    let track = Rect {
        origin: Point {
            x: if vertical {
                rect.origin.x
            } else {
                rect.origin.x.saturating_add_unsigned(arrow_extent)
            },
            y: if vertical {
                rect.origin.y.saturating_add_unsigned(arrow_extent)
            } else {
                rect.origin.y
            },
        },
        size: Size {
            width: if vertical { minor } else { track_major },
            height: if vertical { track_major } else { minor },
        },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect,
            color: theme.palette.surface,
            radius: 0,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect,
            color: theme.palette.border,
            width: 1,
            radius: 0,
        },
    )?;
    let leading = Rect {
        origin: rect.origin,
        size: Size {
            width: if vertical { minor } else { arrow_extent },
            height: if vertical { arrow_extent } else { minor },
        },
    };
    let trailing = Rect {
        origin: Point {
            x: if vertical {
                rect.origin.x
            } else {
                rect.origin.x.saturating_add_unsigned(major - arrow_extent)
            },
            y: if vertical {
                rect.origin.y.saturating_add_unsigned(major - arrow_extent)
            } else {
                rect.origin.y
            },
        },
        size: leading.size,
    };
    for button in [leading, trailing] {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: button,
                color: theme.palette.surface_raised,
                radius: 0,
            },
        )?;
        push_command(
            commands,
            DrawCommand::Border {
                rect: button,
                color: theme.palette.border,
                width: 1,
                radius: 0,
            },
        )?;
    }
    render_pixel_arrow(leading, vertical, true, theme.palette.text, commands)?;
    render_pixel_arrow(trailing, vertical, false, theme.palette.text, commands)?;

    if track_major == 0 {
        return Ok(());
    }
    let thumb_extent = u32::try_from(
        u64::from(extent).saturating_mul(u64::from(track_major)) / u64::from(major.max(1)),
    )
    .unwrap_or(track_major)
    .max(minor.min(track_major))
    .min(track_major);
    let travel = track_major.saturating_sub(thumb_extent);
    let offset = u32::try_from(
        u64::from(travel).saturating_mul(u64::from(position)) / u64::from(total.max(1)),
    )
    .unwrap_or(travel);
    let thumb = Rect {
        origin: Point {
            x: if vertical {
                track.origin.x
            } else {
                track.origin.x.saturating_add_unsigned(offset)
            },
            y: if vertical {
                track.origin.y.saturating_add_unsigned(offset)
            } else {
                track.origin.y
            },
        },
        size: Size {
            width: if vertical { minor } else { thumb_extent },
            height: if vertical { thumb_extent } else { minor },
        },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect: thumb,
            color: if active {
                theme.palette.surface_raised
            } else {
                theme.palette.surface
            },
            radius: 0,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect: thumb,
            color: theme.palette.border,
            width: 1,
            radius: 0,
        },
    )?;
    let thumb_center_x = thumb.origin.x.saturating_add_unsigned(thumb.size.width / 2);
    let thumb_center_y = thumb
        .origin
        .y
        .saturating_add_unsigned(thumb.size.height / 2);
    for offset in [-4_i32, 0, 4] {
        let grip = if vertical {
            Rect {
                origin: Point {
                    x: thumb_center_x.saturating_sub_unsigned(1),
                    y: thumb_center_y.saturating_add(offset),
                },
                size: Size {
                    width: 3,
                    height: 1,
                },
            }
        } else {
            Rect {
                origin: Point {
                    x: thumb_center_x.saturating_add(offset),
                    y: thumb_center_y.saturating_sub_unsigned(1),
                },
                size: Size {
                    width: 1,
                    height: 3,
                },
            }
        };
        push_command(
            commands,
            DrawCommand::Fill {
                rect: grip,
                color: theme.palette.border,
                radius: 0,
            },
        )?;
    }
    Ok(())
}

fn render_pixel_arrow(
    rect: Rect,
    vertical: bool,
    leading: bool,
    color: Color,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let center_x = rect.origin.x.saturating_add_unsigned(rect.size.width / 2);
    let center_y = rect.origin.y.saturating_add_unsigned(rect.size.height / 2);
    for row in 0..5_u32 {
        let breadth = if leading {
            row * 2 + 1
        } else {
            (4 - row) * 2 + 1
        };
        let shift = if leading { row } else { 4 - row };
        let pixel = if vertical {
            Rect {
                origin: Point {
                    x: center_x.saturating_sub_unsigned(breadth / 2),
                    y: center_y
                        .saturating_sub_unsigned(2)
                        .saturating_add_unsigned(shift),
                },
                size: Size {
                    width: breadth,
                    height: 1,
                },
            }
        } else {
            Rect {
                origin: Point {
                    x: center_x
                        .saturating_sub_unsigned(2)
                        .saturating_add_unsigned(shift),
                    y: center_y.saturating_sub_unsigned(breadth / 2),
                },
                size: Size {
                    width: 1,
                    height: breadth,
                },
            }
        };
        push_command(
            commands,
            DrawCommand::Fill {
                rect: pixel,
                color,
                radius: 0,
            },
        )?;
    }
    Ok(())
}

fn render_color_picker(
    rect: Rect,
    selected: [u8; 3],
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let mut nearest = (0, 0, u32::MAX);
    for row in 0..scroll::COLOR_ROWS {
        let top = u32::try_from(
            u64::from(rect.size.height).saturating_mul(u64::from(row))
                / u64::from(scroll::COLOR_ROWS),
        )
        .unwrap_or(rect.size.height);
        let bottom = u32::try_from(
            u64::from(rect.size.height).saturating_mul(u64::from(row + 1))
                / u64::from(scroll::COLOR_ROWS),
        )
        .unwrap_or(rect.size.height);
        for column in 0..scroll::COLOR_COLUMNS {
            let left = u32::try_from(
                u64::from(rect.size.width).saturating_mul(u64::from(column))
                    / u64::from(scroll::COLOR_COLUMNS),
            )
            .unwrap_or(rect.size.width);
            let right = u32::try_from(
                u64::from(rect.size.width).saturating_mul(u64::from(column + 1))
                    / u64::from(scroll::COLOR_COLUMNS),
            )
            .unwrap_or(rect.size.width);
            let rgb = scroll::color_picker_rgb(column, row);
            let distance = rgb
                .iter()
                .zip(selected)
                .map(|(candidate, selected)| {
                    let delta = i32::from(*candidate) - i32::from(selected);
                    delta.unsigned_abs().saturating_pow(2)
                })
                .fold(0_u32, u32::saturating_add);
            if distance < nearest.2 {
                nearest = (column, row, distance);
            }
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: rect.origin.x.saturating_add_unsigned(left),
                            y: rect.origin.y.saturating_add_unsigned(top),
                        },
                        size: Size {
                            width: right.saturating_sub(left).max(1),
                            height: bottom.saturating_sub(top).max(1),
                        },
                    },
                    color: Color::opaque(rgb[0], rgb[1], rgb[2]),
                    radius: 0,
                },
            )?;
        }
    }
    push_command(
        commands,
        DrawCommand::Border {
            rect,
            color: theme.palette.border,
            width: 1,
            radius: bounded_radius(rect, theme.metrics.corner_radius),
        },
    )?;
    let marker_x = u32::try_from(
        u64::from(rect.size.width).saturating_mul(u64::from(nearest.0))
            / u64::from(scroll::COLOR_COLUMNS),
    )
    .unwrap_or_default();
    let marker_right = u32::try_from(
        u64::from(rect.size.width).saturating_mul(u64::from(nearest.0 + 1))
            / u64::from(scroll::COLOR_COLUMNS),
    )
    .unwrap_or(rect.size.width);
    let marker_y = u32::try_from(
        u64::from(rect.size.height).saturating_mul(u64::from(nearest.1))
            / u64::from(scroll::COLOR_ROWS),
    )
    .unwrap_or_default();
    let marker_bottom = u32::try_from(
        u64::from(rect.size.height).saturating_mul(u64::from(nearest.1 + 1))
            / u64::from(scroll::COLOR_ROWS),
    )
    .unwrap_or(rect.size.height);
    let marker = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add_unsigned(marker_x),
            y: rect.origin.y.saturating_add_unsigned(marker_y),
        },
        size: Size {
            width: marker_right.saturating_sub(marker_x).max(1),
            height: marker_bottom.saturating_sub(marker_y).max(1),
        },
    };
    push_command(
        commands,
        DrawCommand::Border {
            rect: marker,
            color: contrasting(selected),
            width: 3,
            radius: bounded_radius(marker, theme.metrics.corner_radius),
        },
    )
}

fn parse_rgb(digits: &str) -> Option<[u8; 3]> {
    if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let packed = u32::from_str_radix(digits, 16).ok()?;
    Some([
        u8::try_from((packed >> 16) & 0xff).ok()?,
        u8::try_from((packed >> 8) & 0xff).ok()?,
        u8::try_from(packed & 0xff).ok()?,
    ])
}

fn contrasting([red, green, blue]: [u8; 3]) -> Color {
    let luminance = u32::from(red) * 299 + u32::from(green) * 587 + u32::from(blue) * 114;
    if luminance >= 150_000 {
        Color::opaque(0, 0, 0)
    } else {
        Color::opaque(255, 255, 255)
    }
}
