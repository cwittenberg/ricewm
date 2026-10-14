//! Native checkbox, radio-button, and switch presentation.

use rice_ui::{
    ChromeProfile, Color, Insets, Point, Rect, Size, Theme, Widget, WidgetKind, WidgetState,
};

use super::super::{DrawCommand, NativeUiError, TextAlign, TextRole};
use super::helpers::{bounded_radius, push_command, push_redmond_bevel};

fn push_redmond_checkmark(
    commands: &mut Vec<DrawCommand>,
    indicator: Rect,
) -> Result<(), NativeUiError> {
    // The original control uses a tiny raster tick.  Do not send a Unicode
    // check glyph through the selected theme font: several authentic bitmap
    // fonts do not contain it, which left a logically checked Redmond control
    // looking empty.  These five two-pixel strokes are the stable 7x7 Win98
    // mark and remain inside the painted 13x13 indicator.
    for (x, y, width, height) in [
        (3_u32, 6_u32, 2_u32, 2_u32),
        (4, 7, 2, 2),
        (5, 8, 2, 2),
        (6, 6, 2, 3),
        (7, 4, 2, 3),
    ] {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: indicator.origin.x.saturating_add_unsigned(x),
                        y: indicator.origin.y.saturating_add_unsigned(y),
                    },
                    size: Size { width, height },
                },
                color: Color::opaque(0, 0, 0),
                radius: 0,
            },
        )?;
    }
    Ok(())
}

fn push_mixed_mark(commands: &mut Vec<DrawCommand>, indicator: Rect, color: Color) -> Result<(), NativeUiError> {
    let width = indicator.size.width.saturating_sub(6);
    let height = indicator.size.height.min(2);
    if width == 0 || height == 0 { return Ok(()); }
    push_command(commands, DrawCommand::Fill {
        rect: Rect { origin: Point {
            x: indicator.origin.x.saturating_add_unsigned(3),
            y: indicator.origin.y.saturating_add_unsigned(indicator.size.height.saturating_sub(height) / 2),
        }, size: Size { width, height } }, color, radius: 0,
    })
}

pub(super) fn content_inset(
    widget: &Widget,
    inset: Rect,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<Rect, NativeUiError> {
    if !matches!(widget.kind, WidgetKind::CheckBox | WidgetKind::RadioButton) {
        return Ok(inset);
    }
    let size = if theme.chrome_profile == ChromeProfile::Redmond98 {
        13
    } else {
        u32::from(theme.metrics.icon_small)
    }
    .min(inset.size.height);
    let indicator = Rect {
        origin: Point {
            x: inset.origin.x,
            y: inset
                .origin
                .y
                .saturating_add_unsigned(inset.size.height.saturating_sub(size) / 2),
        },
        size: Size {
            width: size,
            height: size,
        },
    };
    let checked = widget.state.contains(WidgetState::CHECKED);
    let mixed = widget.kind == WidgetKind::CheckBox && widget.state.contains(WidgetState::INDETERMINATE);
    if theme.chrome_profile == ChromeProfile::Redmond98 {
        if widget.kind == WidgetKind::RadioButton {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: indicator,
                    color: Color::opaque(255, 255, 255),
                    radius: u16::try_from(size / 2).unwrap_or(u16::MAX),
                },
            )?;
            push_command(
                commands,
                DrawCommand::Border {
                    rect: indicator,
                    color: Color::opaque(0, 0, 0),
                    width: 1,
                    radius: u16::try_from(size / 2).unwrap_or(u16::MAX),
                },
            )?;
            if checked {
                let dot = indicator.inset(Insets::all(4));
                push_command(
                    commands,
                    DrawCommand::Fill {
                        rect: dot,
                        color: Color::opaque(0, 0, 0),
                        radius: u16::try_from(dot.size.width / 2).unwrap_or(u16::MAX),
                    },
                )?;
            }
        } else {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: indicator,
                    color: Color::opaque(255, 255, 255),
                    radius: 0,
                },
            )?;
            push_redmond_bevel(commands, indicator, true)?;
            if mixed { push_mixed_mark(commands, indicator, Color::opaque(0, 0, 0))?; }
            else if checked { push_redmond_checkmark(commands, indicator)?; }
        }
        return Ok(inset.inset(Insets {
            top: 0,
            right: 0,
            bottom: 0,
            left: size + 4,
        }));
    }
    if checked || mixed {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: indicator,
                color: theme.palette.accent,
                radius: indicator_radius(widget.kind, indicator, theme),
            },
        )?;
    }
    push_command(
        commands,
        DrawCommand::Border {
            rect: indicator,
            color: if checked || mixed {
                theme.palette.accent
            } else {
                theme.palette.text_muted
            },
            width: theme.metrics.border_width.max(1),
            radius: indicator_radius(widget.kind, indicator, theme),
        },
    )?;
    if mixed {
        push_mixed_mark(commands, indicator, theme.palette.accent_text)?;
    } else if checked {
        if widget.kind == WidgetKind::RadioButton {
            let dot_size = size.saturating_sub(8).max(1).min(size);
            let dot_offset = size.saturating_sub(dot_size) / 2;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: indicator.origin.x.saturating_add_unsigned(dot_offset),
                            y: indicator.origin.y.saturating_add_unsigned(dot_offset),
                        },
                        size: Size {
                            width: dot_size,
                            height: dot_size,
                        },
                    },
                    color: theme.palette.accent_text,
                    radius: u16::try_from(dot_size / 2).unwrap_or(u16::MAX),
                },
            )?;
        } else {
            push_command(
                commands,
                DrawCommand::Text {
                    rect: indicator,
                    text: String::from("✓"),
                    color: theme.palette.accent_text,
                    size_px: theme.typography.body_px,
                    align: TextAlign::Center,
                    role: TextRole::Label,
                    caret: false,
                    caret_offset: None,
                    brand: false,
                    gradient: false,
                },
            )?;
        }
    }
    Ok(inset.inset(Insets {
        top: 0,
        right: 0,
        bottom: 0,
        left: size.saturating_add(u32::from(theme.spacing.unit)),
    }))
}

pub(super) fn render_toggle(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    hovered: bool,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if theme.chrome_profile == ChromeProfile::Redmond98 {
        let size = 13_u32.min(rect.size.width).min(rect.size.height).max(1);
        let leading = widget.state.contains(WidgetState::START_ALIGNED);
        let indicator = Rect {
            origin: Point {
                x: if leading {
                    rect.origin.x
                } else {
                    rect.origin
                        .x
                        .saturating_add_unsigned(rect.size.width.saturating_sub(size))
                },
                y: rect
                    .origin
                    .y
                    .saturating_add_unsigned(rect.size.height.saturating_sub(size) / 2),
            },
            size: Size {
                width: size,
                height: size,
            },
        };
        push_command(
            commands,
            DrawCommand::Fill {
                rect: indicator,
                color: Color::opaque(255, 255, 255),
                radius: 0,
            },
        )?;
        push_redmond_bevel(commands, indicator, true)?;
        if widget.state.contains(WidgetState::CHECKED) {
            push_redmond_checkmark(commands, indicator)?;
        }
        return Ok(());
    }
    let height = 20_u32.min(rect.size.height.max(1));
    let width = 36_u32.min(rect.size.width.max(1));
    let leading = widget.state.contains(WidgetState::START_ALIGNED);
    let track = Rect {
        origin: Point {
            x: if leading {
                rect.origin.x
            } else {
                rect.origin
                    .x
                    .saturating_add_unsigned(rect.size.width.saturating_sub(width))
            },
            y: rect
                .origin
                .y
                .saturating_add_unsigned(rect.size.height.saturating_sub(height) / 2),
        },
        size: Size { width, height },
    };
    let checked = widget.state.contains(WidgetState::CHECKED);
    if theme.frosted_widgets.switches {
        push_command(
            commands,
            DrawCommand::Glass {
                rect: track,
                tone: if checked { 3 } else { 0 },
                tint: if checked {
                    theme.palette.accent
                } else {
                    theme.palette.surface_raised
                },
                radius: bounded_radius(track, u16::try_from(height / 2).unwrap_or(u16::MAX)),
                hovered,
                control: true,
                active: checked,
                structural: false,
                popup: false,
            },
        )?;
    }
    // Switch state is semantic, not optical. Paint a stable state coat after
    // optional glass so pointer-light sampling and scrolling cannot recolor a
    // checked switch from the theme accent to a transient spectral hue.
    push_command(
        commands,
        DrawCommand::Fill {
            rect: track,
            color: if checked {
                theme.palette.accent.with_alpha(245)
            } else {
                theme.palette.surface_raised.with_alpha(245)
            },
            radius: u16::try_from(height / 2).unwrap_or(u16::MAX),
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect: track,
            color: if checked {
                theme.palette.accent
            } else {
                theme.palette.border
            },
            width: theme.metrics.border_width.max(1),
            radius: u16::try_from(height / 2).unwrap_or(u16::MAX),
        },
    )?;
    let inset = 3_u32.min(height / 2).min(width / 2);
    let thumb_size = height
        .saturating_sub(inset.saturating_mul(2))
        .max(1)
        .min(height)
        .min(width);
    let thumb_x = if checked {
        width.saturating_sub(thumb_size).saturating_sub(inset)
    } else {
        inset.min(width.saturating_sub(thumb_size))
    };
    let thumb = Rect {
        origin: Point {
            x: track.origin.x.saturating_add_unsigned(thumb_x),
            y: track
                .origin
                .y
                .saturating_add_unsigned(height.saturating_sub(thumb_size) / 2),
        },
        size: Size {
            width: thumb_size,
            height: thumb_size,
        },
    };
    if theme.chrome_profile == ChromeProfile::Orchard && theme.frosted_widgets.switches {
        return push_command(
            commands,
            DrawCommand::Glass {
                rect: thumb,
                tone: 0,
                tint: Color::opaque(255, 255, 255),
                radius: bounded_radius(thumb, u16::try_from(thumb_size / 2).unwrap_or(u16::MAX)),
                hovered,
                control: true,
                active: checked,
                structural: false,
                popup: false,
            },
        );
    }
    push_command(
        commands,
        DrawCommand::Fill {
            rect: thumb,
            color: if checked {
                theme.palette.accent_text
            } else {
                if theme.frosted_widgets.switches {
                    theme.palette.accent_text.with_alpha(220)
                } else {
                    theme.palette.text_muted
                }
            },
            radius: bounded_radius(thumb, u16::try_from(thumb_size / 2).unwrap_or(u16::MAX)),
        },
    )
}

fn indicator_radius(kind: WidgetKind, rect: Rect, theme: &Theme) -> u16 {
    let requested = if kind == WidgetKind::RadioButton {
        u16::try_from(rect.size.width / 2).unwrap_or(u16::MAX)
    } else {
        theme.metrics.corner_radius.min(4)
    };
    bounded_radius(rect, requested)
}
