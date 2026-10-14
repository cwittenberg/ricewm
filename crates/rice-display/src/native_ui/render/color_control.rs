//! Shared color-command metadata leaves icon lookup and menu interaction native.
use super::{DrawCommand, NativeUiError, push_command};
use rice_ui::{Color, Point, Rect, Size, Theme, Widget, WidgetKind, WidgetState};

fn parts(widget: &Widget) -> Option<(&str, &str)> {
    if !matches!(
        widget.kind,
        WidgetKind::Menu | WidgetKind::Button | WidgetKind::MenuItem
    ) {
        return None;
    }
    let (value, label) = widget
        .text
        .as_deref()?
        .strip_prefix("panel:color-control:")?
        .split_once('|')?;
    if label.is_empty() || label.len() > 160 || label.contains(['\r', '\n']) {
        return None;
    }
    (matches!(value, "none" | "mixed") || color(value).is_some()).then_some((value, label))
}
fn color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let packed = u32::from_str_radix(hex, 16).ok()?;
    Some(Color::opaque(
        ((packed >> 16) & 255) as u8,
        ((packed >> 8) & 255) as u8,
        (packed & 255) as u8,
    ))
}
/// Measure and paint the same ordinary label in expanded and overflow menus.
pub(crate) fn normalized(widget: &Widget) -> Option<Widget> {
    let (_, label) = parts(widget)?;
    let mut copy = widget.clone();
    copy.text = Some(label.into());
    Some(copy)
}
pub(super) fn paint(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    start: usize,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let Some((value, _)) = parts(widget) else {
        return Ok(());
    };
    let Some((icon_index, icon)) =
        commands[start..]
            .iter()
            .enumerate()
            .find_map(|(index, command)| match command {
                DrawCommand::Icon { rect, icon, .. } if Some(*icon) == widget.icon => {
                    Some((start + index, *rect))
                }
                _ => None,
            })
    else {
        return Ok(());
    };
    let width = icon.size.width.min(rect.size.width.saturating_sub(4));
    if width < 4 || rect.size.height < 4 {
        return Ok(());
    }
    let palette_item = widget.kind == WidgetKind::MenuItem;
    let bar = if palette_item {
        // Menu entries use the existing icon slot for an uncluttered color sample.
        // Toolbar menus/buttons retain their icon and current-color underline.
        commands.remove(icon_index);
        Rect {
            size: Size {
                width,
                height: icon.size.height.min(rect.size.height.saturating_sub(4)),
            },
            ..icon
        }
    } else {
        Rect {
            origin: Point {
                x: icon.origin.x,
                y: icon
                    .origin
                    .y
                    .saturating_add_unsigned(icon.size.height + 1)
                    .min(
                        rect.origin
                            .y
                            .saturating_add_unsigned(rect.size.height.saturating_sub(4)),
                    ),
            },
            size: Size { width, height: 4 },
        }
    };
    let fill = color(value).unwrap_or(theme.palette.surface);
    push_command(
        commands,
        DrawCommand::Fill {
            rect: bar,
            color: if widget.state.contains(WidgetState::ENABLED) {
                fill
            } else {
                fill.with_alpha(110)
            },
            radius: 0,
        },
    )?;
    if value == "mixed" {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    size: Size {
                        width: width / 2,
                        height: bar.size.height,
                    },
                    ..bar
                },
                color: theme.palette.text,
                radius: 0,
            },
        )?;
    } else if value == "none" {
        let height = bar.size.height;
        for index in 0..height {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: bar.origin.x.saturating_add_unsigned(index * width / height),
                            y: bar.origin.y.saturating_add_unsigned(height - 1 - index),
                        },
                        size: Size {
                            width: (width / height).max(1),
                            height: 1,
                        },
                    },
                    color: theme.palette.text,
                    radius: 0,
                },
            )?;
        }
    }
    push_command(
        commands,
        DrawCommand::Border {
            rect: bar,
            color: theme.palette.border,
            width: 1,
            radius: 0,
        },
    )
}
