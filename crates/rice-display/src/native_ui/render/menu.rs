use std::collections::BTreeMap;

use rice_native_app_protocol::WidgetId;
use rice_ui::{
    ChromeProfile, IconId, Insets, MenuText, Point, Rect, RetainedTree, Size, Theme, Widget,
    WidgetKind, WidgetState,
};

use super::super::{DrawCommand, HitTarget, NativeUiError, TextAlign, TextRole};
use super::helpers::{bounded_radius, push_command, push_redmond_bevel};

const POPUP_PADDING: u32 = 6;
const ROW_HORIZONTAL_PADDING: u32 = 12;
const INDICATOR_GAP: u32 = 10;
const REDMOND_POPUP_PADDING: u32 = 3;
const REDMOND_ROW_HORIZONTAL_PADDING: u32 = 4;
const REDMOND_INDICATOR_GAP: u32 = 4;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_menu_popup(
    tree: &RetainedTree,
    menu: WidgetId,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
    focused: Option<WidgetId>,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
    commands: &mut Vec<DrawCommand>,
    targets: &mut Vec<HitTarget>,
    _elevation_depth: u8,
    _joined_edges: (bool, bool),
) -> Result<(), NativeUiError> {
    let Some(menu) = tree.get(menu) else {
        return Ok(());
    };
    if let Some(popup) = menu_popup_rect(tree, menu.id, geometry, theme) {
        let popup_radius = bounded_radius(popup, theme.metrics.corner_radius.saturating_add(4));
        if theme.chrome_profile == ChromeProfile::Redmond98 {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: popup,
                    color: theme.menu_palette.surface,
                    radius: 0,
                },
            )?;
            push_redmond_bevel(commands, popup, false)?;
        } else if theme.glass_menus {
            push_command(
                commands,
                DrawCommand::Glass {
                    rect: popup,
                    tone: 0,
                    tint: theme.menu_palette.surface.with_alpha(178),
                    radius: popup_radius,
                    hovered: false,
                    control: false,
                    active: false,
                    structural: false,
                    popup: true,
                },
            )?;
            // Backdrop brightness must not decide menu legibility. A single
            // dark reading coat retains the blur while keeping Jade's menu
            // labels crisp over white documents and bright wallpapers.
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: popup,
                    color: theme.menu_palette.surface.with_alpha(210),
                    radius: popup_radius,
                },
            )?;
        } else {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: popup,
                    color: theme.menu_palette.surface_raised,
                    radius: popup_radius,
                },
            )?;
        }
        if theme.chrome_profile != ChromeProfile::Redmond98 && !theme.glass_menus {
            push_command(
                commands,
                DrawCommand::Border {
                    rect: popup,
                    color: theme.menu_palette.border,
                    width: theme.metrics.border_width,
                    radius: popup_radius,
                },
            )?;
        }
        if theme.chrome_profile == ChromeProfile::Familiar {
            let inner = popup.inset(Insets {
                top: u32::from(theme.metrics.border_width),
                right: u32::from(theme.metrics.border_width),
                bottom: u32::from(theme.metrics.border_width),
                left: u32::from(theme.metrics.border_width),
            });
            push_command(
                commands,
                DrawCommand::Border {
                    rect: inner,
                    color: theme.menu_palette.surface_raised,
                    width: theme.metrics.border_width.max(1),
                    radius: 0,
                },
            )?;
        }
        push_command(commands,DrawCommand::ThemeSurface {rect:popup,role:2,state:0})?;
        let children = super::super::layout::menu_content_children(tree, menu);
        let visible = children
            .iter()
            .filter(|child| geometry.contains_key(child))
            .copied()
            .collect::<Vec<_>>();
        if visible.len() < children.len()
            && let Some(first) = visible.first()
            && let Some(offset) = children.iter().position(|child| child == first)
        {
            let track = Rect {
                origin: Point {
                    x: popup
                        .origin
                        .x
                        .saturating_add_unsigned(popup.size.width.saturating_sub(7)),
                    y: popup.origin.y.saturating_add(3),
                },
                size: Size {
                    width: 4,
                    height: popup.size.height.saturating_sub(6),
                },
            };
            let total = u32::try_from(children.len())
                .unwrap_or(u32::MAX)
                .max(1);
            let showing = u32::try_from(visible.len()).unwrap_or(u32::MAX).min(total);
            let thumb_height = track
                .size
                .height
                .saturating_mul(showing)
                .checked_div(total)
                .unwrap_or(0)
                .max(18)
                .min(track.size.height);
            let maximum = children.len().saturating_sub(visible.len()).max(1);
            let travel = track.size.height.saturating_sub(thumb_height);
            let thumb_y = travel
                .saturating_mul(u32::try_from(offset).unwrap_or(u32::MAX))
                .checked_div(u32::try_from(maximum).unwrap_or(u32::MAX))
                .unwrap_or(0);
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: track,
                    color: theme.menu_palette.border.with_alpha(92),
                    radius: 2,
                },
            )?;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: track.origin.x,
                            y: track.origin.y.saturating_add_unsigned(thumb_y),
                        },
                        size: Size {
                            width: track.size.width,
                            height: thumb_height,
                        },
                    },
                    color: theme.menu_palette.text_muted.with_alpha(210),
                    radius: 2,
                },
            )?;
        }
    }
    let clip = menu_popup_rect(tree, menu.id, geometry, theme);
    if let Some(rect) = clip {
        push_command(commands, DrawCommand::PushClip { rect })?;
    }
    for child in super::super::layout::menu_content_children(tree, menu) {
        if !geometry.contains_key(&child) {
            continue;
        }
        super::render_widget(
            tree, child, geometry, theme, focused, hovered, pressed, None, None, None, clip,
            commands, targets,
        )?;
    }
    if clip.is_some() {
        push_command(commands, DrawCommand::PopClip)?;
    }
    Ok(())
}

pub(crate) fn menu_popup_rect(
    tree: &RetainedTree,
    menu: WidgetId,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
) -> Option<Rect> {
    let menu = tree.get(menu)?;
    let rects = super::super::layout::menu_content_children(tree, menu)
        .iter()
        .filter_map(|child| geometry.get(child).copied())
        .collect::<Vec<_>>();
    let popup_padding = if theme.chrome_profile == ChromeProfile::Redmond98 {
        REDMOND_POPUP_PADDING
    } else {
        POPUP_PADDING
    };
    // Content may be a compound container, and child order is not a spatial
    // ordering. Union the allocated popup viewports, never passive tooltips or
    // overflowing scroll descendants.
    let first = rects.first()?;
    let left = rects.iter().fold(first.origin.x, |x, rect| x.min(rect.origin.x));
    let top = rects.iter().fold(first.origin.y, |y, rect| y.min(rect.origin.y));
    let right = rects.iter().map(|rect| rect.origin.x.saturating_add_unsigned(rect.size.width)).max()?;
    let bottom = rects.iter().map(|rect| rect.origin.y.saturating_add_unsigned(rect.size.height)).max()?;
    Some(Rect {
        origin: Point {
            x: left.saturating_sub_unsigned(popup_padding),
            y: top.saturating_sub_unsigned(popup_padding),
        },
        size: Size {
            width: right.saturating_sub(left).max(0).cast_unsigned().saturating_add(popup_padding * 2),
            height: bottom.saturating_sub(top).max(0).cast_unsigned().saturating_add(popup_padding * 2),
        },
    })
}

pub(super) fn render_menu_row(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    enabled: bool,
    selected: bool,
    submenu_opens_left: bool,
    has_submenu: bool,
    has_leading_column: bool,
    commands: &mut Vec<DrawCommand>,
) -> Result<bool, NativeUiError> {
    let Some(raw_text) = widget.text.as_deref() else {
        return Ok(false);
    };
    let text = MenuText::parse(raw_text);
    if text.separator {
        let separator_padding = if theme.chrome_profile == ChromeProfile::Redmond98 {
            REDMOND_ROW_HORIZONTAL_PADDING
        } else {
            ROW_HORIZONTAL_PADDING
        };
        let y = rect.origin.y.saturating_add_unsigned(rect.size.height / 2);
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: rect
                            .origin
                            .x
                            .saturating_add(separator_padding.cast_signed()),
                        y,
                    },
                    size: Size {
                        width: rect
                            .size
                            .width
                            .saturating_sub(separator_padding.saturating_mul(2)),
                        height: u32::from(theme.metrics.border_width).max(1),
                    },
                },
                color: if theme.chrome_profile == ChromeProfile::Redmond98 {
                    rice_ui::Color::opaque(128, 128, 128)
                } else {
                    theme.menu_palette.border
                },
                radius: 0,
            },
        )?;
        if theme.chrome_profile == ChromeProfile::Redmond98 {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: rect.origin.x.saturating_add_unsigned(separator_padding),
                            y: y.saturating_add(1),
                        },
                        size: Size {
                            width: rect
                                .size
                                .width
                                .saturating_sub(separator_padding.saturating_mul(2)),
                            height: 1,
                        },
                    },
                    color: rice_ui::Color::opaque(255, 255, 255),
                    radius: 0,
                },
            )?;
        }
        return Ok(true);
    }

    if selected && theme.chrome_profile == ChromeProfile::CommandDeck {
        let pill = rect.inset(Insets {
            top: 3,
            right: 4,
            bottom: 3,
            left: 4,
        });
        push_command(
            commands,
            DrawCommand::Fill {
                rect: pill,
                color: theme.menu_palette.selection,
                radius: u16::try_from(pill.size.height / 2).unwrap_or(u16::MAX),
            },
        )?;
    }

    let selected_background = super::semantic::composite_over(
        theme.menu_palette.selection,
        theme.menu_palette.surface_raised,
    );
    let color = if !enabled {
        theme.menu_palette.text_muted
    } else if selected && theme.chrome_profile == ChromeProfile::Redmond98 {
        theme.menu_palette.selection_text
    } else if selected {
        super::semantic::contrasting_ink(selected_background)
    } else {
        theme.menu_palette.text
    };
    let redmond98 = theme.chrome_profile == ChromeProfile::Redmond98;
    let row_padding = if redmond98 {
        REDMOND_ROW_HORIZONTAL_PADDING
    } else {
        ROW_HORIZONTAL_PADDING
    };
    let indicator_gap = if redmond98 {
        REDMOND_INDICATOR_GAP
    } else {
        INDICATOR_GAP
    };
    let inset = rect.inset(Insets {
        top: if redmond98 {
            1
        } else {
            u32::from(theme.spacing.control_y)
        },
        right: row_padding,
        bottom: if redmond98 {
            1
        } else {
            u32::from(theme.spacing.control_y)
        },
        left: row_padding,
    });
    let leading_size = u32::from(theme.metrics.icon_small).min(inset.size.height);
    let leading_gutter =
        u32::from(has_leading_column).saturating_mul(leading_size.saturating_add(indicator_gap));
    let label_x = inset.origin.x.saturating_add_unsigned(leading_gutter);
    if matches!(widget.kind, WidgetKind::CheckBox | WidgetKind::RadioButton) {
        let indicator = leading_size;
        let indicator_rect = Rect {
            origin: Point {
                x: inset.origin.x,
                y: inset
                    .origin
                    .y
                    .saturating_add_unsigned(inset.size.height.saturating_sub(indicator) / 2),
            },
            size: Size {
                width: indicator,
                height: indicator,
            },
        };
        if !redmond98 {
            push_command(
                commands,
                DrawCommand::Border {
                    rect: indicator_rect,
                    color: if enabled {
                        if selected {
                            color
                        } else {
                            theme.menu_palette.text_muted
                        }
                    } else {
                        theme.menu_palette.border
                    },
                    width: theme.metrics.border_width.max(1),
                    radius: if widget.kind == WidgetKind::RadioButton {
                        u16::try_from(indicator / 2).unwrap_or(u16::MAX)
                    } else {
                        theme.metrics.corner_radius.min(3)
                    },
                },
            )?;
        }
        if widget.state.contains(WidgetState::CHECKED) {
            if widget.kind == WidgetKind::RadioButton {
                let dot = if redmond98 {
                    5_u32.min(indicator)
                } else {
                    indicator.saturating_sub(8).max(5)
                };
                let dot_rect = Rect {
                    origin: Point {
                        x: indicator_rect
                            .origin
                            .x
                            .saturating_add_unsigned((indicator - dot) / 2),
                        y: indicator_rect
                            .origin
                            .y
                            .saturating_add_unsigned((indicator - dot) / 2),
                    },
                    size: Size {
                        width: dot,
                        height: dot,
                    },
                };
                if redmond98 {
                    // The classic menu radio is a tiny pixel diamond, not a
                    // checkbox and not a font-dependent square glyph.
                    for (row, width) in [(0_u32, 1_u32), (1, 3), (2, 5), (3, 3), (4, 1)] {
                        push_command(
                            commands,
                            DrawCommand::Fill {
                                rect: Rect {
                                    origin: Point {
                                        x: dot_rect
                                            .origin
                                            .x
                                            .saturating_add_unsigned(dot.saturating_sub(width) / 2),
                                        y: dot_rect.origin.y.saturating_add_unsigned(row),
                                    },
                                    size: Size { width, height: 1 },
                                },
                                color,
                                radius: 0,
                            },
                        )?;
                    }
                } else {
                    push_command(
                        commands,
                        DrawCommand::Fill {
                            rect: dot_rect,
                            color: if selected {
                                color
                            } else {
                                theme.menu_palette.selection
                            },
                            radius: u16::try_from(dot / 2).unwrap_or(u16::MAX),
                        },
                    )?;
                }
            } else {
                if redmond98 {
                    // Paint the Win98 check from pixels so its shape does not
                    // depend on a Unicode glyph in the active UI font.
                    let x = indicator_rect.origin.x.saturating_add(2);
                    let y = indicator_rect.origin.y.saturating_add(4);
                    for (dx, dy) in [
                        (0_u32, 3_u32),
                        (1, 4),
                        (2, 5),
                        (3, 4),
                        (4, 3),
                        (5, 2),
                        (6, 1),
                        (7, 0),
                    ] {
                        push_command(
                            commands,
                            DrawCommand::Fill {
                                rect: Rect {
                                    origin: Point {
                                        x: x.saturating_add_unsigned(dx),
                                        y: y.saturating_add_unsigned(dy),
                                    },
                                    size: Size {
                                        width: 2,
                                        height: 2,
                                    },
                                },
                                color,
                                radius: 0,
                            },
                        )?;
                    }
                } else {
                    push_command(
                        commands,
                        DrawCommand::Text {
                            rect: indicator_rect,
                            text: String::from("✓"),
                            color: if selected {
                                color
                            } else {
                                theme.menu_palette.selection
                            },
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
        }
    } else if let Some(icon) = widget.icon {
        let icon_size = leading_size;
        let icon_rect = Rect {
            origin: Point {
                x: inset.origin.x,
                y: inset
                    .origin
                    .y
                    .saturating_add_unsigned(inset.size.height.saturating_sub(icon_size) / 2),
            },
            size: Size {
                width: icon_size,
                height: icon_size,
            },
        };
        push_command(
            commands,
            DrawCommand::Icon {
                rect: icon_rect,
                icon,
                color,
                hovered: selected,
                animated: false,
                shortcut: false,
            },
        )?;
    }
    let trailing_size = u32::from(theme.metrics.icon_small).min(inset.size.height);
    // The retained child menu is the source of truth. Requiring every app to
    // duplicate that relationship with TRAILING_ICON allowed genuine Start
    // menu submenus to lose their right arrow after a patch rebuild.
    let trailing_reserved = if has_submenu || widget.state.contains(WidgetState::TRAILING_ICON) {
        trailing_size.saturating_add(indicator_gap)
    } else {
        0
    };
    let text_width = inset
        .origin
        .x
        .saturating_add_unsigned(inset.size.width)
        .saturating_sub_unsigned(trailing_reserved)
        .saturating_sub(label_x)
        .try_into()
        .unwrap_or(0);
    let text_inset = Rect {
        origin: inset.origin,
        size: Size {
            width: inset.size.width.saturating_sub(trailing_reserved),
            height: inset.size.height,
        },
    };
    let label_rect = Rect {
        origin: Point {
            x: label_x,
            y: inset.origin.y,
        },
        size: Size {
            width: text_width,
            height: inset.size.height,
        },
    };
    if !enabled && theme.chrome_profile == ChromeProfile::Redmond98 {
        push_command(
            commands,
            DrawCommand::Text {
                rect: Rect {
                    origin: Point {
                        x: label_rect.origin.x.saturating_add(1),
                        y: label_rect.origin.y.saturating_add(1),
                    },
                    size: label_rect.size,
                },
                text: text.label.to_owned(),
                color: rice_ui::Color::opaque(255, 255, 255),
                size_px: theme.typography.body_px,
                align: TextAlign::Start,
                role: TextRole::Label,
                caret: false,
                caret_offset: None,
                brand: false,
                gradient: false,
            },
        )?;
    }
    push_command(
        commands,
        DrawCommand::Text {
            rect: label_rect,
            text: text.label.to_owned(),
            color,
            size_px: theme.typography.body_px,
            align: TextAlign::Start,
            role: TextRole::Label,
            caret: false,
            caret_offset: None,
            brand: false,
            gradient: false,
        },
    )?;
    if let Some(shortcut) = text.shortcut {
        push_command(
            commands,
            DrawCommand::Text {
                rect: text_inset,
                text: shortcut.to_owned(),
                color: if enabled {
                    if selected || redmond98 {
                        color
                    } else {
                        theme.menu_palette.text_muted
                    }
                } else {
                    color
                },
                size_px: theme.typography.body_px,
                align: TextAlign::End,
                role: TextRole::Label,
                caret: false,
                caret_offset: None,
                brand: false,
                gradient: false,
            },
        )?;
    }
    if has_submenu || widget.state.contains(WidgetState::TRAILING_ICON) {
        if redmond98 {
            let x = inset
                .origin
                .x
                .saturating_add_unsigned(inset.size.width.saturating_sub(trailing_size / 2 + 2));
            let y = inset
                .origin
                .y
                .saturating_add_unsigned(inset.size.height / 2);
            for column in 0..4_u32 {
                push_command(
                    commands,
                    DrawCommand::Fill {
                        rect: Rect {
                            origin: Point {
                                x: x.saturating_add_unsigned(if submenu_opens_left {
                                    3 - column
                                } else {
                                    column
                                }),
                                y: y.saturating_sub((3 - column).cast_signed()),
                            },
                            size: Size {
                                width: 1,
                                height: 7 - column * 2,
                            },
                        },
                        color,
                        radius: 0,
                    },
                )?;
            }
            return Ok(true);
        }
        push_command(
            commands,
            DrawCommand::Icon {
                rect: Rect {
                    origin: Point {
                        x: inset.origin.x.saturating_add_unsigned(
                            inset.size.width.saturating_sub(trailing_size),
                        ),
                        y: inset.origin.y.saturating_add_unsigned(
                            inset.size.height.saturating_sub(trailing_size) / 2,
                        ),
                    },
                    size: Size {
                        width: trailing_size,
                        height: trailing_size,
                    },
                },
                icon: if submenu_opens_left {
                    IconId::SubmenuLeft
                } else {
                    IconId::SubmenuRight
                },
                color,
                hovered: selected,
                animated: false,
                shortcut: false,
            },
        )?;
    }
    Ok(true)
}
