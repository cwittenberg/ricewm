//! Theme-native slider track, fill, and thumb rendering.

use rice_ui::{ChromeProfile, Color, Point, Rect, Size, Theme, Widget, WidgetState};

use super::super::{DrawCommand, NativeUiError, scroll};
use super::helpers::{bounded_radius, push_command, push_redmond_bevel};

pub(super) fn render(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    hovered: bool,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let enabled = widget.state.contains(WidgetState::ENABLED);
    let (position, total) = scroll::fraction(widget.text.as_deref());
    let fraction = if total == 0 {
        0
    } else {
        u64::from(position).saturating_mul(1000) / u64::from(total)
    };
    let fraction = u32::try_from(fraction).unwrap_or(1000).min(1000);

    if theme.chrome_profile == ChromeProfile::Redmond98 {
        return render_redmond_slider(rect, fraction, commands);
    }

    let thumb_size = u32::from(theme.metrics.control_height) * 2 / 3;
    let margin = (thumb_size / 2).saturating_add(1).max(6);
    // Orchard's window radius must not turn a slider into a thick capsule.
    // A fine blue track remains visible through the larger clear thumb.
    let orchard = theme.chrome_profile == ChromeProfile::Orchard;
    let track_height = if orchard {
        6
    } else {
        u32::from(theme.metrics.corner_radius)
            .saturating_mul(2)
            .max(6)
    };
    if rect.size.width <= margin.saturating_mul(2) || rect.size.height < 8 {
        return Ok(());
    }
    let track = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add_unsigned(margin),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height / 2)
                .saturating_sub_unsigned(track_height / 2),
        },
        size: Size {
            width: rect.size.width.saturating_sub(margin.saturating_mul(2)),
            height: track_height,
        },
    };
    if theme.signal_controls {
        let center_y = rect.origin.y.saturating_add_unsigned(rect.size.height / 2);
        let fill_width = (u64::from(track.size.width) * u64::from(fraction) / 1000) as u32;
        for (x, y, width, height, color) in [
            (track.origin.x, center_y, track.size.width, 2, theme.palette.border),
            (track.origin.x, center_y, fill_width, 2, if enabled { theme.palette.accent } else { theme.palette.text_muted }),
            (track.origin.x.saturating_add_unsigned(fill_width).saturating_sub(2), center_y.saturating_sub(6), 4, 14, if enabled { theme.palette.text } else { theme.palette.text_muted }),
        ] {
            push_command(commands, DrawCommand::Fill { rect: Rect { origin: Point { x, y }, size: Size { width, height } }, color, radius: 0 })?;
        }
        for index in 0..=10 {
            push_command(commands, DrawCommand::Fill {
                rect: Rect { origin: Point { x: track.origin.x.saturating_add_unsigned(track.size.width * index / 10), y: center_y.saturating_sub(12) }, size: Size { width: 1, height: if index % 5 == 0 { 5 } else { 3 } } },
                color: theme.palette.text_muted, radius: 0,
            })?;
        }
        return Ok(());
    }
    let radius = bounded_radius(track, theme.metrics.corner_radius);
    if theme.frosted_widgets.sliders {
        push_command(
            commands,
            DrawCommand::Glass {
                rect: track,
                tone: 0,
                tint: if enabled {
                    theme.palette.surface_raised
                } else {
                    theme.palette.surface
                },
                radius,
                hovered,
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
                rect: track,
                color: if enabled {
                    theme.palette.surface_raised
                } else {
                    theme.palette.surface
                },
                radius,
            },
        )?;
    }
    let ticks = scroll::ticks(widget.text.as_deref());
    if ticks >= 2 {
        for index in 0..ticks {
            let offset = u32::try_from(
                u64::from(track.size.width).saturating_mul(u64::from(index)) / u64::from(ticks - 1),
            )
            .unwrap_or_default();
            let x = track.origin.x.saturating_add_unsigned(offset);
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: x.saturating_sub(1),
                            y: track.origin.y.saturating_sub_unsigned(3),
                        },
                        size: Size {
                            width: 2,
                            height: track.size.height + 6,
                        },
                    },
                    color: theme.palette.border,
                    radius: 1,
                },
            )?;
        }
    }
    let fill_width =
        u32::try_from(u64::from(track.size.width).saturating_mul(u64::from(fraction)) / 1000)
            .unwrap_or(0)
            .min(track.size.width);
    if fill_width > 0 {
        let fill = Rect {
            origin: track.origin,
            size: Size {
                width: fill_width,
                height: track.size.height,
            },
        };
        if theme.frosted_widgets.sliders {
            push_command(
                commands,
                DrawCommand::Glass {
                    rect: fill,
                    tone: if enabled { 3 } else { 0 },
                    tint: if enabled {
                        theme.palette.accent
                    } else {
                        theme.palette.surface_raised
                    },
                    radius: bounded_radius(fill, radius),
                    hovered,
                    control: true,
                    active: enabled,
                    structural: false,
                    popup: false,
                },
            )?;
        } else {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: fill,
                    color: if enabled {
                        theme.palette.accent
                    } else {
                        theme.palette.surface_raised
                    },
                    radius,
                },
            )?;
        }
    }
    // The thumb's centre travels the full track; keep the thumb itself inside
    // the widget rect by clamping against the reserved margin.
    let center_x = track.origin.x.saturating_add_unsigned(fill_width).clamp(
        track.origin.x,
        track.origin.x.saturating_add_unsigned(track.size.width),
    );
    let thumb = Rect {
        origin: Point {
            x: center_x.saturating_sub_unsigned(thumb_size / 2),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height / 2)
                .saturating_sub_unsigned(thumb_size / 2),
        },
        size: Size {
            width: thumb_size,
            height: thumb_size,
        },
    };
    if theme.frosted_widgets.sliders {
        push_command(
            commands,
            DrawCommand::Glass {
                rect: thumb,
                tone: 0,
                tint: theme.palette.surface,
                radius: bounded_radius(thumb, (thumb_size / 2).try_into().unwrap_or(u16::MAX)),
                hovered,
                control: true,
                active: true,
                structural: false,
                popup: false,
            },
        )?;
    } else {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: thumb,
                color: theme.palette.surface,
                radius: bounded_radius(thumb, (thumb_size / 2).try_into().unwrap_or(u16::MAX)),
            },
        )?;
    }
    let inner = thumb.inset(rice_ui::Insets {
        top: 2,
        right: 2,
        bottom: 2,
        left: 2,
    });
    if !orchard && inner.size.width > 0 && inner.size.height > 0 {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: inner,
                color: if enabled {
                    theme.palette.accent
                } else {
                    theme.palette.surface_raised
                },
                radius: bounded_radius(
                    inner,
                    (inner.size.width / 2).try_into().unwrap_or(u16::MAX),
                ),
            },
        )?;
    }
    Ok(())
}

fn render_redmond_slider(
    rect: Rect,
    fraction: u32,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if rect.size.width < 16 || rect.size.height < 8 {
        return Ok(());
    }
    let margin = 7;
    let track = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add_unsigned(margin),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height / 2)
                .saturating_sub_unsigned(2),
        },
        size: Size {
            width: rect.size.width.saturating_sub(margin * 2),
            height: 4,
        },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect: track,
            color: Color::opaque(255, 255, 255),
            radius: 0,
        },
    )?;
    push_redmond_bevel(commands, track, true)?;
    let travel = track.size.width;
    let offset = u32::try_from(u64::from(travel) * u64::from(fraction) / 1000).unwrap_or_default();
    let thumb = Rect {
        origin: Point {
            x: track
                .origin
                .x
                .saturating_add_unsigned(offset)
                .saturating_sub_unsigned(5),
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height.saturating_sub(18) / 2),
        },
        size: Size {
            width: 11,
            height: 18_u32.min(rect.size.height),
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
