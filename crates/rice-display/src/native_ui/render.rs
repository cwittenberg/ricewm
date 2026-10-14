use std::collections::BTreeMap;

use rice_native_app_protocol::WidgetId;
use rice_ui::{
    ButtonShape, ChromeProfile, EditorText, Insets, Point, Rect, RetainedTree, Size, Theme, Widget,
    WidgetKind, WidgetState,
};

use super::{DrawCommand, HitTarget, NativeUiError, TextAlign, TextEditState, TextRole};

mod choice;
pub(super) mod color_control;
mod helpers;
mod grid;
mod icon;
mod media;
mod menu;
mod scrollbar;
mod semantic;
mod slider;
mod tabs;

use helpers::{
    bounded_radius, interaction, progress_width, push_command, push_target, text_geometry,
};
pub(super) use menu::{menu_popup_rect, render_menu_popup};
use semantic::{Appearance, VisualState};

#[cfg(test)]
pub(super) fn progress_width_for_test(text: Option<&str>, total: u32) -> u32 {
    progress_width(text, total)
}

pub(super) fn is_slim_progress(text: Option<&str>) -> bool {
    helpers::is_slim_progress(text)
}

const FORCED_DARK_EDITOR_SURFACE: rice_ui::Color = rice_ui::Color::opaque(18, 20, 24);
const FORCED_DARK_EDITOR_BORDER: rice_ui::Color = rice_ui::Color::opaque(65, 72, 84);
const FORCED_DARK_EDITOR_TEXT: rice_ui::Color = rice_ui::Color::opaque(235, 238, 244);

fn forced_dark_editor(widget: &Widget) -> bool {
    widget.kind == WidgetKind::TextEditor
        && widget
            .text
            .as_deref()
            .and_then(EditorText::parse)
            .is_some_and(|presentation| presentation.force_dark_mode)
}

fn dotted_tree_segment(commands: &mut Vec<DrawCommand>, rect: Rect) -> Result<(), NativeUiError> {
    if rect.size.width == 0 || rect.size.height == 0 {
        return Ok(());
    }
    push_command(
        commands,
        DrawCommand::DottedLine {
            rect,
            color: rice_ui::Color::opaque(128, 128, 128),
        },
    )
}

fn push_dotted_focus_rect(
    commands: &mut Vec<DrawCommand>,
    rect: Rect,
    color: rice_ui::Color,
) -> Result<(), NativeUiError> {
    if rect.size.width < 2 || rect.size.height < 2 {
        return Ok(());
    }
    let right = rect
        .origin
        .x
        .saturating_add_unsigned(rect.size.width.saturating_sub(1));
    let bottom = rect
        .origin
        .y
        .saturating_add_unsigned(rect.size.height.saturating_sub(1));
    for edge in [
        Rect {
            origin: rect.origin,
            size: Size {
                width: rect.size.width,
                height: 1,
            },
        },
        Rect {
            origin: Point {
                x: rect.origin.x,
                y: bottom,
            },
            size: Size {
                width: rect.size.width,
                height: 1,
            },
        },
        Rect {
            origin: rect.origin,
            size: Size {
                width: 1,
                height: rect.size.height,
            },
        },
        Rect {
            origin: Point {
                x: right,
                y: rect.origin.y,
            },
            size: Size {
                width: 1,
                height: rect.size.height,
            },
        },
    ] {
        push_command(commands, DrawCommand::DottedLine { rect: edge, color })?;
    }
    Ok(())
}

fn render_redmond_tree_connectors(
    tree: &RetainedTree,
    widget: &Widget,
    rect: Rect,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if theme.chrome_profile != ChromeProfile::Redmond98 || widget.kind != WidgetKind::TreeItem {
        return Ok(());
    }
    let Some(parent_id) = widget.parent else {
        return Ok(());
    };
    let Some(parent) = tree.get(parent_id) else {
        return Ok(());
    };
    let row_center = rect.origin.y.saturating_add_unsigned(rect.size.height / 2);
    let node_offset = u32::from(theme.spacing.control_x).saturating_add(4);
    let indent = u32::from(theme.metrics.icon_small);
    let own_axis = rect.origin.x.saturating_add_unsigned(node_offset);
    let icon_x = rect
        .origin
        .x
        .saturating_add_unsigned(u32::from(theme.spacing.control_x))
        .saturating_add_unsigned(indent);
    let siblings = parent
        .children()
        .iter()
        .filter_map(|id| {
            tree.get(*id)
                .is_some_and(|candidate| candidate.kind == WidgetKind::TreeItem)
                .then_some(*id)
        })
        .collect::<Vec<_>>();
    let position = siblings.iter().position(|id| *id == widget.id);

    if parent.kind == WidgetKind::Tree {
        if position.is_some_and(|position| position > 0) {
            dotted_tree_segment(
                commands,
                Rect {
                    origin: Point {
                        x: own_axis,
                        y: rect.origin.y,
                    },
                    size: Size {
                        width: 1,
                        height: rect.size.height / 2 + 1,
                    },
                },
            )?;
        }
        if position.is_some_and(|position| position + 1 < siblings.len()) {
            dotted_tree_segment(
                commands,
                Rect {
                    origin: Point {
                        x: own_axis,
                        y: row_center,
                    },
                    size: Size {
                        width: 1,
                        height: rect.size.height.saturating_sub(rect.size.height / 2),
                    },
                },
            )?;
        }
        dotted_tree_segment(
            commands,
            Rect {
                origin: Point {
                    x: own_axis,
                    y: row_center,
                },
                size: Size {
                    width: icon_x.saturating_sub(own_axis).max(0).cast_unsigned(),
                    height: 1,
                },
            },
        )?;
    } else if parent.kind == WidgetKind::TreeItem {
        let parent_axis = geometry
            .get(&parent_id)
            .map_or(own_axis.saturating_sub_unsigned(indent), |parent_rect| {
                parent_rect.origin.x.saturating_add_unsigned(node_offset)
            });
        dotted_tree_segment(
            commands,
            Rect {
                origin: Point {
                    x: parent_axis,
                    y: rect.origin.y,
                },
                size: Size {
                    width: 1,
                    height: rect.size.height / 2 + 1,
                },
            },
        )?;
        if position.is_some_and(|position| position + 1 < siblings.len()) {
            dotted_tree_segment(
                commands,
                Rect {
                    origin: Point {
                        x: parent_axis,
                        y: row_center,
                    },
                    size: Size {
                        width: 1,
                        height: rect.size.height.saturating_sub(rect.size.height / 2),
                    },
                },
            )?;
        }
        dotted_tree_segment(
            commands,
            Rect {
                origin: Point {
                    x: parent_axis,
                    y: row_center,
                },
                size: Size {
                    width: icon_x.saturating_sub(parent_axis).max(0).cast_unsigned(),
                    height: 1,
                },
            },
        )?;

        let mut descendant = parent_id;
        let mut ancestor_id = parent.parent;
        while let Some(current_ancestor_id) = ancestor_id {
            let Some(ancestor) = tree.get(current_ancestor_id) else {
                break;
            };
            if ancestor.kind != WidgetKind::TreeItem {
                break;
            }
            let branch_has_later_sibling = ancestor.children().iter().any(|candidate| {
                *candidate != descendant
                    && tree
                        .get(*candidate)
                        .is_some_and(|candidate| candidate.kind == WidgetKind::TreeItem)
                    && ancestor
                        .children()
                        .iter()
                        .position(|candidate| *candidate == descendant)
                        < ancestor
                            .children()
                            .iter()
                            .position(|item| item == candidate)
            });
            if branch_has_later_sibling
                && let Some(ancestor_rect) = geometry.get(&current_ancestor_id)
            {
                dotted_tree_segment(
                    commands,
                    Rect {
                        origin: Point {
                            x: ancestor_rect.origin.x.saturating_add_unsigned(node_offset),
                            y: rect.origin.y,
                        },
                        size: Size {
                            width: 1,
                            height: rect.size.height,
                        },
                    },
                )?;
            }
            descendant = current_ancestor_id;
            ancestor_id = ancestor.parent;
        }
    }

    let has_visible_children = widget.state.contains(WidgetState::CHECKED)
        && widget.children().iter().any(|child| {
            tree.get(*child)
                .is_some_and(|child| child.kind == WidgetKind::TreeItem)
        });
    if has_visible_children {
        dotted_tree_segment(
            commands,
            Rect {
                origin: Point {
                    x: own_axis,
                    y: row_center,
                },
                size: Size {
                    width: 1,
                    height: rect.size.height.saturating_sub(rect.size.height / 2),
                },
            },
        )?;
    }
    Ok(())
}

const DIALOG_SURFACE_MIN_ALPHA: u8 = 235;
const TEXT_SELECTION_MAX_ALPHA: u8 = 144;

pub(super) const fn widget_interaction(kind: WidgetKind, enabled: bool) -> (bool, bool) {
    interaction(kind, enabled)
}

/// Text selection is a glyph-run treatment, not a selected-control state.
///
/// Theme selection colors are intentionally strong because they also paint
/// selected rows and tiles. Desktop text controls use the same hue at a quieter
/// opacity, matching the compact text-only highlight used by established
/// browser chrome.
pub(super) fn text_selection_color(theme: &Theme) -> rice_ui::Color {
    let alpha = theme.palette.selection.channels()[3].min(TEXT_SELECTION_MAX_ALPHA);
    theme.palette.selection.with_alpha(alpha)
}

pub(super) fn render_modal_scrim(
    viewport: Size,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    push_command(
        commands,
        DrawCommand::Fill {
            rect: Rect {
                origin: Point { x: 0, y: 0 },
                size: viewport,
            },
            color: rice_ui::Color::rgba(0, 0, 0, 104),
            radius: 0,
        },
    )
}

/// Paint an HTML `title` hint using the compact rectangular treatment native
/// browsers use for page content. It deliberately has no balloon caret: that
/// richer shape belongs to application-authored native tooltips.
pub(super) fn render_web_title_tooltip(
    viewport: Size,
    pointer: Point,
    title: &str,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let estimated_text_width = u32::try_from(title.chars().count())
        .unwrap_or(u32::MAX)
        .saturating_mul(7);
    let width = estimated_text_width
        .saturating_add(12)
        .clamp(24, viewport.width.max(24));
    let height = 24_u32.min(viewport.height);
    let maximum_x = viewport.width.saturating_sub(width);
    let maximum_y = viewport.height.saturating_sub(height);
    let preferred_x = pointer.x.saturating_add(12).max(0).cast_unsigned();
    let preferred_y = pointer.y.saturating_add(18).max(0).cast_unsigned();
    let x = preferred_x.min(maximum_x);
    let y = if preferred_y <= maximum_y {
        preferred_y
    } else {
        pointer
            .y
            .saturating_sub(28)
            .max(0)
            .cast_unsigned()
            .min(maximum_y)
    };
    let rect = Rect {
        origin: Point {
            x: i32::try_from(x).unwrap_or_default(),
            y: i32::try_from(y).unwrap_or_default(),
        },
        size: Size { width, height },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect,
            color: rice_ui::Color::rgba(24, 24, 24, 255),
            radius: 0,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Border {
            rect,
            color: rice_ui::Color::rgba(220, 220, 220, 255),
            width: 1,
            radius: 0,
        },
    )?;
    push_command(
        commands,
        DrawCommand::Text {
            rect: Rect {
                origin: Point {
                    x: rect.origin.x.saturating_add(6),
                    y: rect.origin.y.saturating_add(2),
                },
                size: Size {
                    width: rect.size.width.saturating_sub(12),
                    height: rect.size.height.saturating_sub(4),
                },
            },
            text: title.to_owned(),
            color: rice_ui::Color::rgba(255, 255, 255, 255),
            size_px: 13,
            align: TextAlign::Start,
            role: TextRole::Label,
            caret: false,
            caret_offset: None,
            brand: false,
            gradient: false,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_widget(
    tree: &RetainedTree,
    id: WidgetId,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
    focused: Option<WidgetId>,
    hovered: Option<WidgetId>,
    pressed: Option<WidgetId>,
    opened_menu: Option<WidgetId>,
    text_selection_all: Option<WidgetId>,
    text_edit: Option<TextEditState>,
    clip: Option<Rect>,
    commands: &mut Vec<DrawCommand>,
    targets: &mut Vec<HitTarget>,
) -> Result<(), NativeUiError> {
    let target_start = targets.len();
    let Some(widget) = tree.get(id) else {
        return Ok(());
    };
    let Some(rect) = geometry.get(&id).copied() else {
        return Ok(());
    };
    if widget.kind == WidgetKind::MenuBar && widget.text.as_deref() == Some("context") {
        return Ok(());
    }
    let clip = if super::scene_surface::overlay(widget.text.as_deref()).is_some() {
        widget.parent.and_then(|id| geometry.get(&id)).map(|parent| clipped_rect(*parent, clip)).or(clip)
    } else { clip };
    let enabled = widget.state.contains(WidgetState::ENABLED);
    let parent = widget.parent.and_then(|parent| tree.get(parent));
    let parent_kind = parent.map(|parent| parent.kind);
    let menu_has_leading_column = parent
        .filter(|parent| parent.kind == WidgetKind::Menu)
        .is_some_and(|parent| super::layout::menu_has_leading_column(tree, parent));
    // A tooltip paints its own surface outside the anchor's selection plane.
    let selected_control = (widget.kind != WidgetKind::Tooltip)
        .then(|| selected_control_ancestor(tree, widget, 4)).flatten();
    let selected_control_content = selected_control.is_some();
    let composite_control_selected = (widget.kind == WidgetKind::Row)
        .then(|| {
            super::layout_tokens::control_for(widget.text.as_deref())
                .and_then(WidgetId::new)
                .and_then(|control| tree.get(control))
                .is_some_and(|control| {
                    control.state.contains(WidgetState::SELECTED)
                        || control.state.contains(WidgetState::PRESSED)
                })
        })
        .unwrap_or(false);
    let inherited_content_background =
        selected_control.map(|control| selected_control_background(control, theme));
    // Only composite controls propagate a held surface into their authored
    // children. Tree items are structurally nested, but pressing a parent row
    // must never paint every visible descendant as selected while the mouse is
    // held (the Win98 tree keeps exactly one row highlight).
    let pressed_control_content =
        pressed
            .and_then(|pressed| tree.get(pressed))
            .is_some_and(|control| {
                (control.kind == WidgetKind::Button
                    || super::layout_tokens::is_tab_composite(control.text.as_deref()))
                    && has_ancestor_id(tree, widget, control.id, 4)
            })
            || pressed_control_ancestor(tree, widget, 4);
    let browser_tab_primary_control = widget.kind == WidgetKind::Button
        && parent.is_some_and(|parent| {
            super::layout_tokens::is_browser_tab(parent.text.as_deref())
                && super::layout_tokens::control_for(parent.text.as_deref()) == Some(id.get())
        });
    let tab_owner = parent
        .filter(|parent| super::layout_tokens::is_tab_composite(parent.text.as_deref()));
    let bookmark_primary_control = widget.kind == WidgetKind::Button
        && parent.is_some_and(|parent| {
            super::layout_tokens::is_bookmark_composite(parent.text.as_deref())
                && super::layout_tokens::control_for(parent.text.as_deref()) == Some(id.get())
        });
    let file_grid_item = file_grid_ancestor(tree, widget).is_some();
    let simplified_files_shell = belongs_to_simplified_files_shell(tree, widget);
    let file_detail_row = file_detail_row_ancestor(tree, widget);
    let file_detail_row_content = file_detail_row.is_some();
    let selected_file_detail_row_content = file_detail_row.is_some_and(|row| {
        super::layout_tokens::control_for(row.text.as_deref())
            .and_then(WidgetId::new)
            .and_then(|control| tree.get(control))
            .is_some_and(|control| {
                control.state.contains(WidgetState::SELECTED)
                    || control.state.contains(WidgetState::PRESSED)
            })
    });
    let selected_file_detail_row = widget.kind == WidgetKind::Row
        && super::layout_tokens::is_file_detail_row(widget.text.as_deref())
        && super::layout_tokens::control_for(widget.text.as_deref())
            .and_then(WidgetId::new)
            .and_then(|control| tree.get(control))
            .is_some_and(|control| {
                control.state.contains(WidgetState::SELECTED)
                    || control.state.contains(WidgetState::PRESSED)
            });
    let desktop_item = widget
        .parent
        .and_then(|parent| tree.get(parent))
        .is_some_and(|parent| {
            parent.kind == WidgetKind::IconGrid
                && parent
                    .text
                    .as_deref()
                    .is_some_and(|text| text == "desktop-grid" || text.starts_with("desktop-grid:"))
        });
    let status_bar = super::layout_tokens::is_status_bar(widget.text.as_deref())
        || parent.is_some_and(|parent| {
            parent.parent.is_none()
                && parent.kind == WidgetKind::Column
                && parent.children().last() == Some(&widget.id)
        });
    let split_panel = parent
        .filter(|parent| parent.kind == WidgetKind::SplitPane)
        .and_then(|parent| {
            parent
                .children()
                .iter()
                .position(|child| *child == widget.id)
        });
    let file_detail_header_item = widget.kind == WidgetKind::Button
        && has_ancestor_token(tree, widget, "file-detail-header", 3);
    let redmond_calculator_key = theme.chrome_profile == ChromeProfile::Redmond98
        && widget.kind == WidgetKind::Button
        && has_ancestor_token(tree, widget, "redmond98-calculator-keypad", 3);
    let flat_table_item = widget.kind == WidgetKind::Button
        && !file_detail_header_item
        && has_ancestor_kind(tree, widget, WidgetKind::Table, 4);
    let seamless_calendar_block = widget.kind == WidgetKind::Button
        && widget.state.contains(WidgetState::SELECTED)
        && parent.is_some_and(|parent| {
            parent.kind == WidgetKind::Stack
                && super::layout_tokens::calendar_day_band(parent.text.as_deref()).is_some()
        });
    let detail_tree_depth = if flat_table_item {
        widget.text.as_deref().map_or(0, |text| {
            text.chars()
                .take_while(|character| *character == ' ')
                .count()
                / 2
        })
    } else {
        0
    };
    let row_separator = widget.kind == WidgetKind::Row
        && !super::layout_tokens::has_spaced_separator(widget.text.as_deref())
        && !matches!(
            widget.text.as_deref(),
            Some("panel:brand-row" | "panel:welcome-features")
        )
        && matches!(parent_kind, Some(WidgetKind::Container | WidgetKind::Table))
        && parent.is_some_and(|parent| parent.children().last() != Some(&widget.id))
        || widget.kind == WidgetKind::Row && widget.text.as_deref() == Some("panel:separator");
    let has_submenu = widget.kind == WidgetKind::MenuItem && widget.children().iter().any(|child| {
        tree.get(*child).is_some_and(|child| child.kind == WidgetKind::Menu)
    });
    let submenu_opens_left = (widget.kind == WidgetKind::MenuItem)
        .then(|| {
            widget.children().iter().copied().find_map(|child| {
                tree.get(child)
                    .is_some_and(|child| child.kind == WidgetKind::Menu)
                    .then(|| menu_popup_rect(tree, child, geometry, theme))
                    .flatten()
            })
        })
        .flatten()
        .is_some_and(|popup| popup.origin.x < rect.origin.x);
    let detail_disclosure = super::layout_tokens::is_file_detail_disclosure(widget.text.as_deref());
    let tree_item_has_children = detail_disclosure
        || widget.kind == WidgetKind::TreeItem
            && (widget.state.contains(WidgetState::INDETERMINATE)
                || widget.children().iter().any(|child| {
                    tree.get(*child)
                        .is_some_and(|child| child.kind == WidgetKind::TreeItem)
                }));
    let tooltip_caret = (widget.kind == WidgetKind::Tooltip)
        .then(|| {
            let anchor = widget
                .parent
                .and_then(|parent| geometry.get(&parent))
                .copied()?;
            let caret_at_top =
                rect.origin.y >= anchor.origin.y.saturating_add_unsigned(anchor.size.height);
            let minimum = rect.origin.x.saturating_add(12);
            let maximum = rect
                .origin
                .x
                .saturating_add_unsigned(rect.size.width)
                .saturating_sub(12)
                .max(minimum);
            let caret_x = anchor
                .origin
                .x
                .saturating_add_unsigned(anchor.size.width / 2)
                .clamp(minimum, maximum);
            Some((caret_x, caret_at_top))
        })
        .flatten();
    let (focusable, activatable) = interaction(widget.kind, enabled);
    let window_drag_region =
        enabled && super::layout_tokens::is_window_drag_region(widget.text.as_deref());
    if widget.kind == WidgetKind::Row
        && let Some(control) =
            super::layout_tokens::control_for(widget.text.as_deref()).and_then(WidgetId::new)
        && let Some(control_widget) = tree.get(control)
        && has_ancestor_id(tree, control_widget, widget.id, 4)
        && control_widget.state.contains(WidgetState::ENABLED)
    {
        let (control_focusable, control_activatable) = interaction(control_widget.kind, true);
        if control_focusable || control_activatable {
            push_target(
                targets,
                HitTarget {
                    widget: control,
                    rect: clipped_rect(rect, clip),
                    focusable: false,
                    activatable: control_activatable,
                },
            )?;
        }
    }
    let tooltip_anchor = widget.children().iter().any(|child| {
        tree.get(*child)
            .is_some_and(|child| child.kind == WidgetKind::Tooltip)
    });
    let scroll_view = super::layout::scroll_view_geometry(tree, id, geometry, theme);
    let interaction_rect = if widget.kind == WidgetKind::Accordion {
        Rect {
            origin: rect.origin,
            size: Size {
                width: rect.size.width,
                height: u32::from(theme.metrics.control_height).min(rect.size.height),
            },
        }
    } else if widget.kind == WidgetKind::SplitPane {
        split_handle_rect(tree, widget, geometry, rect)
    } else if let Some(scroll_view) = scroll_view {
        scroll_view.track
    } else {
        rect
    };
    let target_rect = clipped_rect(interaction_rect, clip);
    if (focusable
        || activatable
        || tooltip_anchor
        || window_drag_region
        || widget.kind == WidgetKind::SplitPane
        || (enabled && scroll_view.is_some()))
        && target_rect.size.width > 0
        && target_rect.size.height > 0
    {
        push_target(
            targets,
            HitTarget {
                widget: id,
                rect: target_rect,
                focusable,
                activatable,
            },
        )?;
    }
    let is_clip = matches!(
        widget.kind,
        WidgetKind::ScrollView
            | WidgetKind::SplitPane
            | WidgetKind::List
            | WidgetKind::Table
            | WidgetKind::Tree
            | WidgetKind::IconGrid
            | WidgetKind::TextEditor
            | WidgetKind::TerminalSurface
            | WidgetKind::TerminalGrid
            | WidgetKind::VirtualGrid
            | WidgetKind::HostSurface
    );
    let surface_clip = if is_clip {
        Some(clipped_rect(rect, clip))
    } else {
        clip
    };
    let child_clip = if widget.kind == WidgetKind::ScrollView {
        Some(clipped_rect(
            super::layout::scroll_view_viewport(widget, rect, theme),
            clip,
        ))
    } else {
        surface_clip
    };
    if let Some(clip) = is_clip.then_some(surface_clip).flatten() {
        push_command(commands, DrawCommand::PushClip { rect: clip })?;
    }
    let tab_page_join = tabs::standalone_page_join(tree, widget, geometry, rect, theme);
    let glass_sidebar_content = theme.glass_sidebars && belongs_to_sidebar_panel(tree, widget);
    let surface_command_start = commands.len();
    render_surface(
        widget,
        rect,
        SurfaceContext {
            document_font: tree.document_font(id),
            document_paint: tree.document_paint(id),
            document_zoom: tree.document_zoom(id),
            compact_label: super::layout::is_compact_label(tree, widget),            theme,
            chat_composer: super::layout::is_chat_composer(tree, widget),
            parent_kind,
            status_bar,
            host_focused: focused == Some(id),
            // Composite tabs paint one continuous surface around their icon,
            // label, and close control. Hovering any descendant therefore
            // highlights the tab rather than only that descendant's box.
            host_hovered: hovered == Some(id)
                || (super::layout_tokens::is_tab_composite(widget.text.as_deref())
                    && hovered
                        .and_then(|hovered| tree.get(hovered))
                        .is_some_and(|hovered| has_ancestor_id(tree, hovered, id, 4))),
            // Text selection is painted from the character-range edit state.
            // Treating select-all as a selected control made the address bar's
            // entire background look selected instead of its URL glyphs.
            host_selected: opened_menu == Some(id)
                || pressed == Some(id)
                || selected_control_content
                || composite_control_selected
                || selected_file_detail_row_content
                || selected_file_detail_row,
            host_pressed: pressed == Some(id) || pressed_control_content,
            suppress_focus_ring: browser_tab_primary_control
                || (tab_owner.is_some() && widget.text.as_deref() != Some("panel:tab-close"))
                || selected_file_detail_row_content
                || (theme.chrome_profile == ChromeProfile::Redmond98
                    && (desktop_item || file_grid_item || widget.kind == WidgetKind::TreeItem)),
            bookmark_primary_control,
            tab_label: tab_owner.is_some() && widget.text.as_deref() != Some("panel:tab-close"),
            text_edit: text_edit.filter(|edit| edit.widget == id),
            desktop_item,
            split_panel,
            flat_table_item,
            seamless_calendar_block,
            row_separator,
            file_grid_item,
            inherited_content_background: inherited_content_background
                .or_else(|| selected_file_detail_row_content.then_some(theme.palette.selection))
                .or_else(|| {
                    (theme.chrome_profile == ChromeProfile::Redmond98
                        && widget.kind == WidgetKind::Button
                        && parent_kind == Some(WidgetKind::List)
                        && widget.state.contains(WidgetState::SELECTED))
                    .then_some(theme.palette.selection)
                }),
            detail_tree_depth,
            submenu_opens_left,
            has_submenu,
            menu_has_leading_column,
            tree_item_has_children,
            tooltip_caret,
            glass_sidebar_content,
            // Detail rows are one composite list-view control. Descendant
            // cells must never grow their own hover bevels; the row owns the
            // selection plane and its children only paint content over it.
            file_detail_row_content,
            file_detail_header_item,
            redmond_calculator_key,
            simplified_files_shell,
            joined_tab_page: tab_page_join.is_some(),
            tab_axis: if super::layout_tokens::vertical_tabs_on_right(widget.text.as_deref()) {
                2
            } else if super::layout_tokens::is_vertical_tabs(widget.text.as_deref()) {
                1
            } else {
                parent.map_or(0, |parent| {
                    if super::layout_tokens::vertical_tabs_on_right(parent.text.as_deref()) {
                        2
                    } else if super::layout_tokens::is_vertical_tabs(parent.text.as_deref()) {
                        1
                    } else {
                        0
                    }
                })
            },
        },
        commands,
    )?;
    let skin_role = if row_separator {Some(6)} else if super::layout_tokens::is_tab_composite(widget.text.as_deref()){Some(1)} else {match widget.kind {
        WidgetKind::Button if tab_owner.is_some() || super::layout_tokens::is_tab_composite(widget.text.as_deref()) => Some(1),
        WidgetKind::Button => Some(0), WidgetKind::Tabs => Some(1), WidgetKind::MenuItem => Some(3),
        WidgetKind::Notification => Some(5), WidgetKind::TextField | WidgetKind::PasswordField | WidgetKind::TextEditor => Some(8),
        WidgetKind::CheckBox => Some(9), WidgetKind::Toggle => Some(10), WidgetKind::Slider => Some(11),
        WidgetKind::ScrollBar => Some(12), _ => None,
    }};
    if let Some(role) = skin_role {
        let state = if !enabled {4} else if pressed == Some(id)||widget.state.contains(WidgetState::PRESSED) {2} else if hovered == Some(id)||widget.state.contains(WidgetState::HOVERED) {1} else if widget.state.contains(WidgetState::SELECTED)||matches!(widget.kind,WidgetKind::CheckBox|WidgetKind::Toggle|WidgetKind::RadioButton)&&widget.state.contains(WidgetState::CHECKED) {5} else if focused == Some(id)||widget.state.contains(WidgetState::FOCUSED) {3} else {0};
        let insertion = commands[surface_command_start..].iter().position(|c| matches!(c,DrawCommand::Text {..}|DrawCommand::Icon {..})).map_or(commands.len(),|i|surface_command_start+i);
        push_command(commands,DrawCommand::ThemeSurface {rect:clipped_rect(rect,clip),role,state})?;
        commands[insertion..].rotate_right(1);
    }
    if let Some(selector)=tree.icon_selector(id) {
        let icons=commands[surface_command_start..].iter().filter_map(|command|if let DrawCommand::Icon {rect,..}=command{Some(*rect)}else{None}).collect::<Vec<_>>();
        for rect in icons {let index=commands[surface_command_start..].iter().position(|command|matches!(command,DrawCommand::Icon {rect:icon_rect,..} if *icon_rect==rect)).map(|i|surface_command_start+i).unwrap();push_command(commands,DrawCommand::IconOverride {rect,selector:selector.to_owned()})?;commands[index..].rotate_right(1);}
    }
    // Composite tab children are flat content, not independent LCARS keys.
    // Their ink must follow the tab face in both selected and resting states.
    if theme.chrome_profile == ChromeProfile::CommandDeck
        && let Some(tab) = tab_owner
    {
        let face = if tab.state.contains(WidgetState::SELECTED) {
            tabs::page_color(
                theme,
                super::layout_tokens::is_browser_tab(tab.text.as_deref()),
            )
        } else {
            tabs::strip_color(theme)
        };
        let ink = semantic::contrasting_ink(face);
        for command in &mut commands[surface_command_start..] {
            match command {
                DrawCommand::Text { color, .. } | DrawCommand::Icon { color, .. } => *color = ink,
                _ => {}
            }
        }
    }
    render_redmond_tree_connectors(tree, widget, rect, geometry, theme, commands)?;
    if glass_sidebar_content {
        let foreground = if widget.state.contains(WidgetState::SELECTED) {
            // `selection_text` is only a theme hint. Glass themes frequently
            // use a translucent or nearly white selected surface, so derive
            // ink from the pixels this row actually paints.
            semantic::contrasting_ink(semantic::composite_over(
                theme.palette.selection,
                theme.palette.surface,
            ))
        } else if enabled {
            // Sidebar glass belongs to the application surface, not the menu
            // popup plane. Glass-menu themes can use pale popup copy over a
            // dark coat while retaining a bright sidebar, so borrowing the
            // menu palette makes Places labels disappear.
            theme.palette.text
        } else {
            theme.palette.text_muted
        };
        for command in &mut commands[surface_command_start..] {
            if let DrawCommand::Text { color, .. } = command {
                *color = foreground;
            }
        }
    }
    if desktop_item {
        // The desktop grid is composited over the live wallpaper rather than
        // `palette.desktop`, so that palette token cannot reliably describe
        // the pixels behind a label. Paint the conventional dark text shadow
        // followed by light copy so captions remain readable over both bright
        // and dark wallpaper, without forcing semantic/color icons to white.
        let tail = commands.split_off(surface_command_start);
        for mut command in tail {
            if matches!(command, DrawCommand::Text { .. }) {
                let mut shadow = command.clone();
                if let DrawCommand::Text {
                    rect: shadow_rect,
                    color: shadow_color,
                    ..
                } = &mut shadow
                {
                    shadow_rect.origin.x = shadow_rect.origin.x.saturating_add(1);
                    shadow_rect.origin.y = shadow_rect.origin.y.saturating_add(1);
                    *shadow_color = rice_ui::Color::opaque(0, 0, 0).with_alpha(190);
                }
                commands.push(shadow);
                if let DrawCommand::Text { color, .. } = &mut command {
                    *color = if enabled {
                        rice_ui::Color::opaque(255, 255, 255)
                    } else {
                        rice_ui::Color::opaque(255, 255, 255).with_alpha(150)
                    };
                }
            }
            commands.push(command);
        }
    }
    let has_inner_child_clip = is_clip && child_clip != surface_clip;
    if let Some(clip) = has_inner_child_clip.then_some(child_clip).flatten() {
        push_command(commands, DrawCommand::PushClip { rect: clip })?;
    }
    if widget.kind != WidgetKind::Menu {
        let render_children = if widget.kind == WidgetKind::Tabs
            && super::layout_tokens::is_integrated_tabs(widget.text.as_deref())
        {
            let mut ordered = widget.children().to_vec();
            ordered.sort_by_key(|child| {
                !tree
                    .get(*child)
                    .is_some_and(|child| super::layout_tokens::is_tab_page(child.text.as_deref()))
            });
            ordered
        } else {
            widget.children().to_vec()
        };
        let selected_stack_child = (widget.kind == WidgetKind::Stack
            && super::layout_tokens::calendar_day_band(widget.text.as_deref()).is_none())
        .then(|| {
            widget.children().iter().copied().find(|child| {
                tree.get(*child)
                    .is_some_and(|child| child.state.contains(WidgetState::SELECTED))
            })
        })
        .flatten();
        for child in &render_children {
            if widget.kind == WidgetKind::Accordion && !widget.state.contains(WidgetState::CHECKED)
            {
                continue;
            }
            if selected_stack_child.is_some_and(|selected| selected != *child) {
                continue;
            }
            if super::layout_overlay::is_dialog(tree, *child)
                || super::layout_overlay::is_tooltip(tree, *child)
                || (widget.kind == WidgetKind::MenuItem
                    && tree
                        .get(*child)
                        .is_some_and(|child| child.kind == WidgetKind::Menu))
            {
                continue;
            }
            render_widget(
                tree,
                *child,
                geometry,
                theme,
                focused,
                hovered,
                pressed,
                opened_menu,
                text_selection_all,
                text_edit,
                child_clip,
                commands,
                targets,
            )?;
        }
    }
    // A real divider must win over child controls that touch its hit strip.
    // A split with fewer than two laid-out children uses its whole rectangle
    // as the fallback interaction rectangle; never re-add that fallback after
    // its child targets or it steals every click in the pane.
    let split_bounds = clipped_rect(rect, clip);
    if widget.kind == WidgetKind::SplitPane
        && target_rect != split_bounds
        && target_rect.size.width > 0
        && target_rect.size.height > 0
    {
        push_target(targets, HitTarget { widget: id, rect: target_rect,
            focusable: false, activatable: false })?;
    }
    // A resizable table owns one unobtrusive hit strip per shared column
    // boundary. These are appended after child targets so the narrow resize
    // affordance wins over a sortable header button only at the divider.
    if widget.kind == WidgetKind::Table
        && super::layout_tokens::resizable_table(widget.text.as_deref())
    {
        for handle in table_column_handle_rects(tree, widget, geometry, rect) {
            let handle = clipped_rect(handle, clip);
            if handle.size.width > 0 && handle.size.height > 0 {
                push_target(
                    targets,
                    HitTarget {
                        widget: id,
                        rect: handle,
                        focusable: false,
                        activatable: false,
                    },
                )?;
            }
        }
    }
    if has_inner_child_clip {
        push_command(commands, DrawCommand::PopClip)?;
    }
    if widget.kind == WidgetKind::ScrollView && !widget.state.contains(WidgetState::FLAT) {
        // The frame belongs to the stationary viewport, never to its moving
        // content. Repaint it after every child so ordinary lists (including
        // Open With) and virtual editors cannot carry or cover the top edge.
        if theme.chrome_profile == ChromeProfile::Redmond98 {
            helpers::push_redmond_bevel(commands, rect, true)?;
        } else {
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
    }
    if super::layout_tokens::is_thumbnail_cell(widget.text.as_deref())
        && widget.state.contains(WidgetState::SELECTED)
        && theme.chrome_profile != ChromeProfile::Redmond98
    {
        push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: theme.palette.selection,
                width: theme.metrics.focus_width.max(2),
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
    }
    if let Some(join) = tab_page_join {
        push_command(
            commands,
            DrawCommand::Fill {
                rect: clipped_rect(join, clip),
                color: tabs::page_color(theme, false),
                radius: 0,
            },
        )?;
    }
    if is_clip {
        push_command(commands, DrawCommand::PopClip)?;
    }
    if let Some(scroll_view) = scroll_view {
        scrollbar::render_scroll_view(
            scroll_view.track,
            scroll_view.position,
            scroll_view.viewport_extent,
            scroll_view.content_extent,
            hovered == Some(id),
            theme,
            commands,
        )?;
    }
    // A disclosed drawer stays in its owner's layout, with a quiet seam on
    // the edge joining the primary content. This paints without stealing space
    // or creating another card/titlebar around the panel.
    if matches!(widget.kind, WidgetKind::Column | WidgetKind::Container)
        && let Some(edge) = widget.text.as_deref().and_then(|text| text.split(':').find_map(|part| part.strip_prefix("drawer-edge=")))
    {
        let seam = match edge {
            "right" => Rect {size: Size {width: 1, height: rect.size.height}, ..rect},
            "left" => Rect {origin: Point {x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(1)), ..rect.origin}, size: Size {width: 1, height: rect.size.height}},
            "bottom" => Rect {size: Size {width: rect.size.width, height: 1}, ..rect},
            "top" => Rect {origin: Point {y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(1)), ..rect.origin}, size: Size {width: rect.size.width, height: 1}},
            _ => Rect::default(),
        };
        if seam.size.width > 0 && seam.size.height > 0 {
            push_command(commands, DrawCommand::Fill {rect: clipped_rect(seam, clip), color: theme.palette.border.with_alpha(160), radius: 0})?;
            // Close the same surface around its footer, so confirmation actions
            // visibly belong to the drawer rather than the following screen.
            let end = match edge {
                "right" => Rect {origin: Point {x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(1)), ..rect.origin}, ..seam},
                "left" => Rect {origin: rect.origin, ..seam},
                "bottom" => Rect {origin: Point {y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(1)), ..rect.origin}, ..seam},
                _ => Rect {origin: rect.origin, ..seam},
            };
            push_command(commands, DrawCommand::Fill {rect: clipped_rect(end, clip), color: theme.palette.border.with_alpha(160), radius: 0})?;
        }
    }
    if widget.kind == WidgetKind::Dialog
        && !widget.text.as_deref().is_some_and(|text| text.starts_with("panel:window-editor:"))
        && enabled
        && let Some(close) = dialog_close_rect(rect, theme)
    {
        // The caption close box is painted above the dialog client and must
        // also win hit testing above every descendant. Appending it after the
        // child walk prevents an accidentally oversized picker or action row
        // from stealing clicks through the visible close box.
        push_target(
            targets,
            HitTarget {
                widget: id,
                rect: clipped_rect(close, clip),
                focusable: false,
                activatable: true,
            },
        )?;
    }
    // A thumbnail and its caption are one file control. The image itself can
    // emit pointer events, so place the shared target above its descendants.
    if enabled
        && super::layout_tokens::is_thumbnail_cell(widget.text.as_deref())
        && let Some(control) =
            super::layout_tokens::control_for(widget.text.as_deref()).and_then(WidgetId::new)
        && let Some(control_widget) = tree.get(control)
        && has_ancestor_id(tree, control_widget, widget.id, 4)
        && control_widget.state.contains(WidgetState::ENABLED)
    {
        let (_, activatable) = interaction(control_widget.kind, true);
        push_target(
            targets,
            HitTarget {
                widget: control,
                rect: clipped_rect(rect, clip),
                focusable: false,
                activatable,
            },
        )?;
    }
    // A tooltip and every control used to compose it are paint-only. Discard
    // any targets produced while walking that subtree so pointer input falls
    // through to the ordinary window content beneath the balloon.
    if widget.kind == WidgetKind::Tooltip {
        targets.truncate(target_start);
    }
    Ok(())
}

fn clipped_rect(rect: Rect, clip: Option<Rect>) -> Rect {
    clip.map_or(rect, |clip| {
        rect.intersection(clip).unwrap_or(Rect {
            origin: rect.origin,
            size: Size::default(),
        })
    })
}

pub(super) fn table_column_handle_rects(
    tree: &RetainedTree,
    table: &Widget,
    geometry: &BTreeMap<WidgetId, Rect>,
    fallback: Rect,
) -> Vec<Rect> {
    let Some(row) = table
        .children()
        .iter()
        .filter_map(|child| tree.get(*child))
        .find(|child| child.kind == WidgetKind::Row)
    else {
        return Vec::new();
    };
    let cells = row
        .children()
        .iter()
        .filter_map(|child| geometry.get(child).copied())
        .collect::<Vec<_>>();
    if cells.len() < 2 {
        return Vec::new();
    }
    const HANDLE: u32 = 10;
    cells
        .windows(2)
        .map(|pair| {
            let boundary = pair[0]
                .origin
                .x
                .saturating_add_unsigned(pair[0].size.width)
                .saturating_add(pair[1].origin.x)
                / 2;
            Rect {
                origin: Point {
                    x: boundary.saturating_sub(i32::try_from(HANDLE / 2).unwrap_or_default()),
                    y: fallback.origin.y,
                },
                size: Size {
                    width: HANDLE,
                    height: fallback.size.height,
                },
            }
        })
        .collect()
}

/// Read reversal from the geometry that was actually painted, not a possibly
/// newer locale. This also leaves compact/vertical and custom Files splits alone.
pub(super) fn split_is_reversed(
    tree: &RetainedTree,
    widget: &Widget,
    geometry: &BTreeMap<WidgetId, Rect>,
) -> bool {
    if widget.kind != WidgetKind::SplitPane
        || widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "vertical"))
        || super::layout_tokens::is_simplified_files_shell(widget.text.as_deref())
    {
        return false;
    }
    let mut children = widget.children().iter()
        .filter(|child| !super::layout_overlay::is_dialog(tree, **child))
        .filter_map(|child| geometry.get(child));
    match (children.next(), children.next()) {
        // Orchard Files gives its main pane a header offset while its sidebar
        // spans the full height. Its x geometry still owns the split direction.
        // Ordinary compact splits remain stacked and do not enter this branch.
        (Some(first), Some(second)) => (first.origin.y == second.origin.y
            || super::layout_tokens::is_files_content(widget.text.as_deref()))
            && first.origin.x > second.origin.x,
        _ => false,
    }
}

pub(super) fn split_handle_rect(
    tree: &RetainedTree,
    widget: &Widget,
    geometry: &BTreeMap<WidgetId, Rect>,
    fallback: Rect,
) -> Rect {
    let mut children = widget
        .children()
        .iter()
        .filter(|child| !super::layout_overlay::is_dialog(tree, **child))
        .filter_map(|child| geometry.get(child).copied());
    let (Some(first), Some(second)) = (children.next(), children.next()) else {
        return fallback;
    };
    const HANDLE: u32 = 10;
    if first.origin.x != second.origin.x {
        let (left, right) = if first.origin.x < second.origin.x {
            (first, second)
        } else {
            (second, first)
        };
        let boundary = ((i64::from(left.origin.x) + i64::from(left.size.width)
            + i64::from(right.origin.x)) / 2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        Rect {
            origin: Point {
                x: boundary.saturating_sub(i32::try_from(HANDLE / 2).unwrap_or_default()),
                y: fallback.origin.y,
            },
            size: Size {
                width: HANDLE,
                height: fallback.size.height,
            },
        }
    } else if first.origin.y != second.origin.y {
        let (top, bottom) = if first.origin.y < second.origin.y {
            (first, second)
        } else {
            (second, first)
        };
        let boundary = ((i64::from(top.origin.y) + i64::from(top.size.height)
            + i64::from(bottom.origin.y)) / 2)
            .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        Rect {
            origin: Point {
                x: fallback.origin.x,
                y: boundary.saturating_sub(i32::try_from(HANDLE / 2).unwrap_or_default()),
            },
            size: Size {
                width: fallback.size.width,
                height: HANDLE,
            },
        }
    } else {
        fallback
    }
}

fn has_ancestor_kind(
    tree: &RetainedTree,
    widget: &Widget,
    expected: WidgetKind,
    maximum_depth: usize,
) -> bool {
    let mut parent = widget.parent;
    for _ in 0..maximum_depth {
        let Some(candidate) = parent.and_then(|id| tree.get(id)) else {
            return false;
        };
        if candidate.kind == expected {
            return true;
        }
        parent = candidate.parent;
    }
    false
}

fn has_ancestor_token(
    tree: &RetainedTree,
    widget: &Widget,
    expected: &str,
    maximum_depth: usize,
) -> bool {
    let mut parent = widget.parent;
    for _ in 0..maximum_depth {
        let Some(candidate) = parent.and_then(|id| tree.get(id)) else {
            return false;
        };
        if candidate
            .text
            .as_deref()
            .is_some_and(|text| text.split(':').any(|token| token == expected))
        {
            return true;
        }
        parent = candidate.parent;
    }
    false
}

fn belongs_to_sidebar_panel(tree: &RetainedTree, widget: &Widget) -> bool {
    if super::layout_tokens::is_sidebar_panel(widget.text.as_deref()) {
        return true;
    }
    let mut parent = widget.parent;
    for _ in 0..8 {
        let Some(candidate) = parent.and_then(|id| tree.get(id)) else {
            return false;
        };
        if super::layout_tokens::is_sidebar_panel(candidate.text.as_deref()) {
            return true;
        }
        parent = candidate.parent;
    }
    false
}

fn has_ancestor_id(
    tree: &RetainedTree,
    widget: &Widget,
    expected: WidgetId,
    maximum_depth: usize,
) -> bool {
    let mut parent = widget.parent;
    for _ in 0..maximum_depth {
        let Some(candidate) = parent else {
            return false;
        };
        if candidate == expected {
            return true;
        }
        parent = tree.get(candidate).and_then(|widget| widget.parent);
    }
    false
}

fn selected_control_ancestor<'a>(
    tree: &'a RetainedTree,
    widget: &Widget,
    maximum_depth: usize,
) -> Option<&'a Widget> {
    let mut parent = widget.parent;
    for _ in 0..maximum_depth {
        let Some(candidate) = parent.and_then(|id| tree.get(id)) else {
            return None;
        };
        if (candidate.kind == WidgetKind::Button
            || super::layout_tokens::is_tab_composite(candidate.text.as_deref()))
            && (candidate.state.contains(WidgetState::SELECTED)
                || candidate.state.contains(WidgetState::PRESSED))
        {
            return Some(candidate);
        }
        parent = candidate.parent;
    }
    None
}

fn selected_control_background(control: &Widget, theme: &Theme) -> rice_ui::Color {
    if super::layout_tokens::is_tab_composite(control.text.as_deref()) {
        return tabs::page_color(
            theme,
            super::layout_tokens::is_browser_tab(control.text.as_deref()),
        );
    }
    semantic::composite_over(theme.palette.selection, theme.palette.surface)
}

fn pressed_control_ancestor(tree: &RetainedTree, widget: &Widget, maximum_depth: usize) -> bool {
    let mut parent = widget.parent;
    for _ in 0..maximum_depth {
        let Some(candidate) = parent.and_then(|id| tree.get(id)) else {
            return false;
        };
        if (candidate.kind == WidgetKind::Button
            || super::layout_tokens::is_tab_composite(candidate.text.as_deref()))
            && candidate.state.contains(WidgetState::PRESSED)
        {
            return true;
        }
        parent = candidate.parent;
    }
    false
}

fn file_grid_ancestor<'a>(tree: &'a RetainedTree, widget: &Widget) -> Option<&'a Widget> {
    let mut parent = widget.parent;
    while let Some(candidate) = parent.and_then(|id| tree.get(id)) {
        if candidate.kind == WidgetKind::IconGrid
            && super::layout_tokens::is_file_grid(candidate.text.as_deref())
        {
            return Some(candidate);
        }
        parent = candidate.parent;
    }
    None
}

fn file_detail_row_ancestor<'a>(tree: &'a RetainedTree, widget: &Widget) -> Option<&'a Widget> {
    let mut parent = widget.parent;
    while let Some(candidate) = parent.and_then(|id| tree.get(id)) {
        if candidate.kind == WidgetKind::Row
            && super::layout_tokens::is_file_detail_row(candidate.text.as_deref())
        {
            return Some(candidate);
        }
        parent = candidate.parent;
    }
    None
}

fn belongs_to_simplified_files_shell(tree: &RetainedTree, widget: &Widget) -> bool {
    let mut current = Some(widget);
    while let Some(candidate) = current {
        if super::layout_tokens::is_simplified_files_shell(candidate.text.as_deref()) {
            return true;
        }
        current = candidate.parent.and_then(|parent| tree.get(parent));
    }
    false
}

#[derive(Clone, Copy)]
struct SurfaceContext<'a> {
    document_font: Option<&'a rice_ui::grid_frame::GridFont>,
    document_paint: Option<&'a rice_ui::grid_frame::DocumentPaint>,
    document_zoom: u16,
    compact_label: bool,
    chat_composer: bool,
    theme: &'a Theme,
    parent_kind: Option<WidgetKind>,
    status_bar: bool,
    host_focused: bool,
    host_hovered: bool,
    host_selected: bool,
    host_pressed: bool,
    suppress_focus_ring: bool,
    bookmark_primary_control: bool,
    tab_label: bool,
    text_edit: Option<TextEditState>,
    desktop_item: bool,
    split_panel: Option<usize>,
    flat_table_item: bool,
    seamless_calendar_block: bool,
    row_separator: bool,
    file_grid_item: bool,
    inherited_content_background: Option<rice_ui::Color>,
    detail_tree_depth: usize,
    submenu_opens_left: bool,
    has_submenu: bool,
    menu_has_leading_column: bool,
    tree_item_has_children: bool,
    tooltip_caret: Option<(i32, bool)>,
    glass_sidebar_content: bool,
    file_detail_row_content: bool,
    file_detail_header_item: bool,
    redmond_calculator_key: bool,
    simplified_files_shell: bool,
    tab_axis: u8,
    joined_tab_page: bool,
}

fn dialog_close_rect(rect: Rect, theme: &Theme) -> Option<Rect> {
    if theme.chrome_profile != ChromeProfile::Redmond98 {
        return None;
    }
    let titlebar = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add(3),
            y: rect.origin.y.saturating_add(3),
        },
        size: Size {
            width: rect.size.width.saturating_sub(6),
            height: rect.size.height.min(22).saturating_sub(3),
        },
    };
    Some(Rect {
        origin: Point {
            x: titlebar
                .origin
                .x
                .saturating_add_unsigned(titlebar.size.width.saturating_sub(18)),
            y: titlebar.origin.y.saturating_add(1),
        },
        size: Size {
            width: 17,
            height: titlebar.size.height.saturating_sub(2),
        },
    })
}

fn render_surface(
    widget: &Widget,
    rect: Rect,
    context: SurfaceContext<'_>,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let theme = context.theme;
    if widget.kind == WidgetKind::Button && widget.state.contains(WidgetState::FLAT) {
        if let Some(paint) = context.document_paint {
            if context.host_selected || widget.state.contains(WidgetState::SELECTED) || context.host_hovered || context.host_pressed {
                let ink = paint.foreground.as_deref().map_or(theme.palette.text, grid::paint_color);
                push_command(commands, DrawCommand::Fill {rect, color:ink.with_alpha(if context.host_pressed {32} else if context.host_selected || widget.state.contains(WidgetState::SELECTED) {24} else {12}), radius:4})?;
            }
        }
    }
    if matches!(widget.kind, WidgetKind::Column | WidgetKind::Container)
        && super::layout_tokens::has_token(widget.text.as_deref(), "drawer-surface") {
        return push_command(commands, DrawCommand::Fill {rect, color: theme.palette.surface_raised, radius: 0});
    }
    if widget.kind == WidgetKind::ScrollView && widget.state.contains(WidgetState::FLAT) { return Ok(()); }

    let document_editor = widget.kind == WidgetKind::TextEditor
        && context.document_font.is_some()
        && widget.text.as_deref().is_some_and(|text| text.starts_with("editor:document~"));
    let document_overlay = widget.kind == WidgetKind::Stack
        && super::scene_surface::overlay(widget.text.as_deref()).is_some();
    if widget.kind == WidgetKind::Container && widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "document-workspace")) {
        return push_command(commands, DrawCommand::Fill { rect, color: theme.palette.surface, radius: 0 });
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && widget.text.as_deref().is_some_and(|text| {
            text.split(':')
                .any(|token| token == "redmond98-calculator-memory-indicator")
        })
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: theme.palette.surface,
                radius: 0,
            },
        )?;
        return helpers::push_redmond_bevel(commands, rect, true);
    }
    if super::layout_tokens::is_file_detail_disclosure_spacer(widget.text.as_deref()) {
        // This owns the indentation width used by directory disclosure
        // arrows.  For files it is empty geometry, never a visible control.
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::Redmond98 && context.file_detail_header_item {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        helpers::push_redmond_bevel(commands, rect, false)?;
    }
    if matches!(
        widget.kind,
        WidgetKind::Column | WidgetKind::Button | WidgetKind::Stack
    ) && let Some(raised) = super::layout_tokens::calendar_day_band(widget.text.as_deref())
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: if raised {
                    theme.palette.surface_raised
                } else {
                    theme.palette.surface
                },
                radius: 0,
            },
        )?;
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: rect.origin,
                    size: Size {
                        width: u32::from(theme.metrics.border_width).max(1),
                        height: rect.size.height,
                    },
                },
                color: theme.palette.border,
                radius: 0,
            },
        )?;
    }
    if matches!(widget.kind, WidgetKind::Column | WidgetKind::Stack)
        && super::layout_tokens::calendar_workday(widget.text.as_deref())
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: theme.palette.accent.with_alpha(18),
                radius: 0,
            },
        )?;
    }
    if widget.kind == WidgetKind::Container
        && let Some((kind, values)) = widget.text.as_deref().and_then(parse_performance_graph)
    {
        return render_performance_graph(rect, theme, kind, &values, commands);
    }
    if widget.text.as_deref().is_some_and(|text| text.starts_with("drag-handle:")) {
        let center = rect.origin.x + (rect.size.width / 2) as i32;
        let top = rect.origin.y + (rect.size.height.saturating_sub(14) / 2) as i32;
        for x in [-4, 3] { for y in [0, 6, 12] {
            push_command(commands, DrawCommand::Fill {
                rect: Rect { origin: Point { x: center+x, y: top+y }, size: Size{width:3,height:3}},
                color: theme.palette.text.with_alpha(140), radius: 1,
            })?;
        }}
        return Ok(());
    }
    if let Some(spec) = widget.text.as_deref().and_then(|text| text.strip_prefix("theme-preview:")) {
        let values: Vec<u32> = spec.split(',').filter_map(|value| u32::from_str_radix(value,16).ok()).collect();
        if values.len() == 8 {
            let colors: Vec<rice_ui::Color> = values[..6].iter().map(|value| rice_ui::Color::opaque((value>>16) as u8,(value>>8) as u8,*value as u8)).collect();
            let style = values[6];
            let radius = values[7].min(24) as u16;
            let mut paint = |x:u32,y:u32,w:u32,h:u32,color:rice_ui::Color,radius:u16| push_command(commands,DrawCommand::Fill {
                rect:Rect{origin:Point{x:rect.origin.x+x as i32,y:rect.origin.y+y as i32},size:Size{width:w.min(rect.size.width.saturating_sub(x)),height:h.min(rect.size.height.saturating_sub(y))}},color,radius,
            });
            let w=rect.size.width; let h=rect.size.height;
            paint(0,0,w,h,colors[0],radius)?;
            if style == 2 {paint(0,0,22,h,colors[5],0)?;}
            if style == 3 {paint(0,0,w,14,colors[5],0)?;}
            paint(42,24,w.saturating_sub(96),h.saturating_sub(61),colors[1],radius)?;
            paint(42,24,w.saturating_sub(96),16,colors[5],radius)?;
            paint(55,51,70,5,colors[2],2)?;
            paint(55,65,w/3,3,colors[3].with_alpha(150),1)?;
            paint(55,75,w/4,3,colors[3].with_alpha(110),1)?;
            paint(w.saturating_sub(135),94, 60,16,colors[4],radius)?;
            if style <= 1 {paint(w/2-62,h.saturating_sub(28),124,22,colors[5],11)?;}
            for i in 0..5 {let (x,y)=if style==2 {(7,16+i*22)} else {(w/2-49+i*22,h.saturating_sub(22))}; if style!=3 {paint(x,y,10,10,colors[2],3)?;}}
        }
        return Ok(());
    }
    if widget.kind == WidgetKind::MenuItem
        && widget.text.as_deref().is_some_and(|text| text.starts_with("panel:") && text.split(':').any(|token| token == "menu-grid-cell"))
    {
        push_command(commands, DrawCommand::Fill { rect,
            color: if widget.state.contains(WidgetState::SELECTED) { theme.palette.selection } else { theme.palette.surface }, radius: 0 })?;
        return push_command(commands, DrawCommand::Border { rect,
            color: if context.host_focused { theme.palette.text } else { theme.palette.border },
            width: if context.host_focused { 2 } else { 1 }, radius: 0 });
    }
    if matches!(widget.kind, WidgetKind::Container | WidgetKind::Button | WidgetKind::MenuItem)
        && let Some(color) = widget.text.as_deref().and_then(parse_color_swatch)
    {
        let radius = bounded_radius(rect, theme.metrics.corner_radius);
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color,
                radius,
            },
        )?;
        return push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: if widget.state.contains(WidgetState::SELECTED) {theme.palette.text} else {theme.palette.border},
                width: if widget.state.contains(WidgetState::SELECTED) {3} else {1},
                radius,
            },
        );
    }
    if widget.kind == WidgetKind::Container
        && let Some(area) = widget.text.as_deref().and_then(parse_desktop_selection)
    {
        let selection = Rect {
            origin: Point {
                x: rect.origin.x.saturating_add(area.0),
                y: rect.origin.y.saturating_add(area.1),
            },
            size: Size {
                width: area.2.min(rect.size.width),
                height: area.3.min(rect.size.height),
            },
        };
        let [red, green, blue, _] = theme.palette.selection.channels();
        push_command(
            commands,
            DrawCommand::Fill {
                rect: selection,
                color: rice_ui::Color::rgba(red, green, blue, 72),
                radius: 0,
            },
        )?;
        return push_command(
            commands,
            DrawCommand::Border {
                rect: selection,
                color: theme.palette.focus,
                width: 1,
                radius: 0,
            },
        );
    }
    let text_selection = context
        .text_edit
        .map(|edit| (edit.anchor.min(edit.caret), edit.anchor.max(edit.caret)))
        .filter(|(start, end)| start != end)
        .or_else(|| {
            (context.host_selected
                && matches!(
                    widget.kind,
                    WidgetKind::TextField | WidgetKind::PasswordField | WidgetKind::TextEditor
                ))
            .then(|| (0, widget.text.as_deref().map_or(0, str::len)))
        });
    let text_selected = text_selection.is_some();
    let visual = VisualState::of(
        widget,
        context.parent_kind,
        context.status_bar,
        context.host_selected && !text_selected,
        context.host_hovered,
        context.host_pressed,
        context.split_panel,
        context.flat_table_item,
    );
    if !document_editor && !document_overlay && !context.file_detail_row_content && !context.file_detail_header_item && !context.tab_label {
        render_chrome(
            widget,
            rect,
            theme,
            visual,
            context.parent_kind,
            context.seamless_calendar_block,
            context.tooltip_caret,
            context.glass_sidebar_content,
            context.desktop_item,
            context.file_grid_item,
            context.tab_axis,
            context.joined_tab_page,
            commands,
        )?;
    }
    if widget.kind == WidgetKind::Button && !widget.state.contains(WidgetState::FLAT) {
        if let Some(paint) = context.document_paint {
            push_command(commands,DrawCommand::Fill{rect:rect.inset(Insets::all(1)),color:grid::paint_color(&paint.background),radius:0})?;
        }
    }
    if let Some(paint) = context.document_paint.filter(|_| !matches!(widget.kind, WidgetKind::Label | WidgetKind::Button)) {
        // Document overlays paint only document content; ordinary fields
        // retain native control chrome and its one-pixel inset.
        push_command(commands, DrawCommand::Fill {
            rect: if document_editor { rect } else { rect.inset(Insets::all(1)) }, color: grid::paint_color(&paint.background), radius: 0,
        })?;
    }
    if context.row_separator
        && !(context.simplified_files_shell
            && super::layout_tokens::is_file_detail_row(widget.text.as_deref()))
    {
        render_row_separator(rect, theme, commands)?;
    }
    if media::render(widget, rect, theme, commands)? {
        return Ok(());
    }
    if matches!(widget.kind, WidgetKind::WebSurface | WidgetKind::SceneSurface) {
        if widget.kind == WidgetKind::SceneSurface && super::scene_surface::page_size(widget.text.as_deref()).is_some() {
            let outline = Rect { origin: Point { x: rect.origin.x.saturating_sub(1), y: rect.origin.y.saturating_sub(1) },
                size: Size { width: rect.size.width.saturating_add(2), height: rect.size.height.saturating_add(2) } };
            let shadow = Rect { origin: Point { x: outline.origin.x.saturating_add(3), y: outline.origin.y.saturating_add(3) }, ..outline };
            push_command(commands, DrawCommand::Fill { rect: shadow, color: theme.palette.text.with_alpha(28), radius: 0 })?;
            push_command(commands, DrawCommand::Border { rect: outline, color: theme.palette.border, width: 1, radius: 0 })?;
        }
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: if widget.kind == WidgetKind::SceneSurface {
                    super::scene_surface::background(widget.text.as_deref())
                } else {
                    rice_ui::Color::opaque(255, 255, 255)
                },
                radius: 0,
            },
        )?;
        return push_command(
            commands,
            DrawCommand::WebScene {
                surface: widget.id,
                rect,
                epoch: 0,
                viewport_width: rect.size.width,
                viewport_height: rect.size.height,
                display_list: std::sync::Arc::default(),
                damage: None,
                vector_scene: widget.kind == WidgetKind::SceneSurface,
            },
        );
    }
    if widget.kind == WidgetKind::VirtualGrid {
        return grid::render(widget, rect, theme, commands);
    }
    if widget.kind == WidgetKind::TerminalGrid {
        let session = widget
            .text
            .as_deref()
            .and_then(|s| s.split(':').next()?.parse::<u64>().ok())
            .filter(|s| *s > 0)
            .ok_or(NativeUiError::InvalidPresentation("terminal grid session"))?;
        return push_command(commands, DrawCommand::TerminalGrid { rect, session });
    }
    if widget.kind == WidgetKind::TerminalRow {
        let text = widget.text.as_deref().unwrap_or("");
        rice_ui::terminal::TerminalRow::parse(text)
            .map_err(|_| NativeUiError::InvalidPresentation("terminal row"))?;
        return push_command(
            commands,
            DrawCommand::TerminalRow {
                rect,
                text: text.to_owned(),
            },
        );
    }
    if widget.kind == WidgetKind::TerminalSurface {
        let session = widget
            .text
            .as_deref()
            .and_then(|text| text.strip_prefix("terminal-session:"))
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value != 0)
            .ok_or(NativeUiError::InvalidPresentation("terminal session"))?;
        return push_command(commands, DrawCommand::TerminalPortal { rect, session });
    }
    if widget.kind == WidgetKind::HostSurface {
        let identity = widget
            .text
            .as_deref()
            .and_then(|text| text.strip_prefix("host-surface:"))
            .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(NativeUiError::InvalidPresentation("host surface identity"))?;
        return push_command(
            commands,
            DrawCommand::HostPortal {
                rect,
                identity: identity.to_ascii_lowercase(),
            },
        );
    }
    let color_content_start = commands.len();
    render_content(
        context.document_font,
        context.document_paint,
        context.document_zoom,
        context.compact_label,
        context.chat_composer,
        widget,
        rect,
        theme,
        visual,
        context.parent_kind,
        context.bookmark_primary_control,
        context.desktop_item,
        context.file_grid_item,
        context.inherited_content_background,
        context.detail_tree_depth,
        context.submenu_opens_left,
        context.has_submenu,
        context.menu_has_leading_column,
        context.tree_item_has_children,
        context.redmond_calculator_key,
        // FOCUSED is an application request retained until its next patch.
        // Pointer focus can already have moved elsewhere. Only current native
        // focus may advertise caret/IME eligibility to the browser ingress.
        context.host_focused && widget.state.contains(WidgetState::ENABLED),
        text_selection,
        context.text_edit.map(|edit| edit.caret),
        commands,
    )?;
    // Scoped native label/action ink also colors their semantic icons. The
    // control remains native for focus, hover, hit testing and activation.
    if matches!(widget.kind, WidgetKind::Label | WidgetKind::Button) {
        if let Some(ink) = context.document_paint.and_then(|paint| paint.foreground.as_deref()) {
            let ink = grid::paint_color(ink);
            for command in &mut commands[color_content_start..] {
                match command {
                    DrawCommand::Icon {rect,icon,hovered,..} => {let scoped=DrawCommand::ScopedIcon{rect:*rect,icon:*icon,color:ink,hovered:*hovered};*command=scoped;},
                    DrawCommand::Text {color,..} => *color=ink, _=>{}
                }
            }
        }
    }
    color_control::paint(widget, rect, theme, color_content_start, commands)?;
    let keyboard_focused = context.host_focused || widget.state.contains(WidgetState::FOCUSED);
    let redmond_hover_focus = theme.chrome_profile == ChromeProfile::Redmond98
        && widget.kind == WidgetKind::Button
        && context.host_hovered
        && widget.state.contains(WidgetState::ENABLED);
    if (keyboard_focused || redmond_hover_focus)
        && !document_editor && !document_overlay
        && !context.suppress_focus_ring
        && !matches!(
            widget.kind,
            WidgetKind::Menu
                | WidgetKind::IconGrid
                | WidgetKind::List
                | WidgetKind::Table
                | WidgetKind::Tree
        )
    {
        let focus_rect = rect.inset(Insets::all(
            if theme.chrome_profile == ChromeProfile::Redmond98 && widget.kind == WidgetKind::Button
            {
                4
            } else {
                u32::from(theme.metrics.border_width)
            },
        ));
        if theme.chrome_profile == ChromeProfile::Redmond98 && widget.kind == WidgetKind::Button {
            push_dotted_focus_rect(commands, focus_rect, theme.palette.text)?;
        } else if widget.kind == WidgetKind::Button && theme.button_shape != ButtonShape::Default {
            push_command(
                commands,
                DrawCommand::ShapedBorder {
                    rect: focus_rect,
                    color: theme.palette.focus,
                    width: theme.metrics.focus_width,
                    shape: theme.button_shape,
                },
            )?;
        } else {
            push_command(
                commands,
                DrawCommand::Border {
                    rect: focus_rect,
                    color: theme.palette.focus,
                    width: theme.metrics.focus_width,
                    radius: bounded_radius(focus_rect, theme.metrics.corner_radius),
                },
            )?;
        }
    }
    Ok(())
}

fn parse_performance_graph(text: &str) -> Option<(&str, Vec<u8>)> {
    let rest = text.strip_prefix("graph:")?;
    let (kind, points) = rest.split_once(':')?;
    if !matches!(kind, "cpu" | "gpu" | "memory" | "network" | "spectrum") {
        return None;
    }
    let values = points
        .split(',')
        .take(120)
        .map(|point| point.parse::<u8>().ok().map(|value| value.min(100)))
        .collect::<Option<Vec<_>>>()?;
    (!values.is_empty()).then_some((kind, values))
}

fn render_performance_graph(
    rect: Rect,
    theme: &Theme,
    kind: &str,
    values: &[u8],
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    push_command(
        commands,
        DrawCommand::Fill {
            rect,
            color: theme.palette.surface_raised,
            radius: bounded_radius(rect, theme.metrics.corner_radius),
        },
    )?;
    for quarter in 1..if kind == "spectrum" { 1 } else { 4_u32 } {
        let y = rect
            .origin
            .y
            .saturating_add_unsigned(rect.size.height.saturating_mul(quarter) / 4);
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: rect.origin.x,
                        y,
                    },
                    size: Size {
                        width: rect.size.width,
                        height: 1,
                    },
                },
                color: theme.palette.border,
                radius: 0,
            },
        )?;
    }
    let plot_height = rect.size.height.saturating_sub(4);
    let count = u32::try_from(values.len()).unwrap_or(1).max(1);
    let color = match kind {
        "network" => theme.palette.focus,
        "memory" => theme.palette.selection,
        "gpu" => theme.palette.focus,
        _ => theme.palette.accent,
    };
    for (index, value) in values.iter().enumerate() {
        let left = rect.origin.x.saturating_add_unsigned(
            rect.size
                .width
                .saturating_mul(u32::try_from(index).unwrap_or_default())
                / count,
        );
        let right = rect.origin.x.saturating_add_unsigned(
            rect.size
                .width
                .saturating_mul(u32::try_from(index + 1).unwrap_or(count))
                / count,
        );
        let height = plot_height.saturating_mul(u32::from(*value)) / 100;
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: left,
                        y: rect.origin.y.saturating_add_unsigned(
                            rect.size.height.saturating_sub(height).saturating_sub(2),
                        ),
                    },
                    size: Size {
                        width: u32::try_from(right.saturating_sub(left))
                            .unwrap_or_default()
                            .saturating_sub(if kind == "spectrum" { 2 } else { 0 })
                            .max(1),
                        height: height.max(1),
                    },
                },
                color,
                radius: 0,
            },
        )?;
    }
    push_command(
        commands,
        DrawCommand::Border {
            rect,
            color: theme.palette.border,
            width: 1,
            radius: bounded_radius(rect, theme.metrics.corner_radius),
        },
    )
}

fn parse_color_swatch(text: &str) -> Option<rice_ui::Color> {
    let digits = text.strip_prefix("color-swatch:#")?;
    (digits.len() == 6)
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
        .map(|packed| {
            rice_ui::Color::opaque(
                ((packed >> 16) & 0xff) as u8,
                ((packed >> 8) & 0xff) as u8,
                (packed & 0xff) as u8,
            )
        })
}

fn parse_desktop_selection(text: &str) -> Option<(i32, i32, u32, u32)> {
    let values = text.strip_prefix("desktop-selection:")?;
    let mut values = values.split(':');
    let x = values.next()?.parse().ok()?;
    let y = values.next()?.parse().ok()?;
    let width = values.next()?.parse().ok()?;
    let height = values.next()?.parse().ok()?;
    values.next().is_none().then_some((x, y, width, height))
}

fn render_redmond_tab(
    rect: Rect,
    selected: bool,
    stable: bool,
    tab_axis: u8,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let face = rice_ui::Color::opaque(192, 192, 192);
    let light = rice_ui::Color::opaque(255, 255, 255);
    let shadow = rice_ui::Color::opaque(128, 128, 128);
    let dark = rice_ui::Color::opaque(0, 0, 0);
    let tab = if selected || stable {
        rect
    } else {
        match tab_axis {
            // A left-hand vertical strip joins its page on the right.
            1 => Rect {
                origin: rect.origin,
                size: Size {
                    width: rect.size.width.saturating_sub(2),
                    height: rect.size.height,
                },
            },
            // A right-hand vertical strip joins its page on the left.
            2 => Rect {
                origin: Point {
                    x: rect.origin.x.saturating_add_unsigned(2),
                    y: rect.origin.y,
                },
                size: Size {
                    width: rect.size.width.saturating_sub(2),
                    height: rect.size.height,
                },
            },
            _ => Rect {
                origin: Point {
                    x: rect.origin.x,
                    y: rect.origin.y.saturating_add_unsigned(2),
                },
                size: Size {
                    width: rect.size.width,
                    height: rect.size.height.saturating_sub(2),
                },
            },
        }
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect: tab,
            color: face,
            radius: 0,
        },
    )?;
    // A Win98 tab is a stepped folder-tab outline, not a raised rectangular
    // push button. Build the one-pixel shoulders explicitly so the active tab
    // reads as part of the page and the inactive tabs sit two pixels lower.
    let mut line = |rect: Rect, color| {
        if rect.size.width == 0 || rect.size.height == 0 {
            return Ok(());
        }
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color,
                radius: 0,
            },
        )
    };
    match tab_axis {
        1 => {
            // Tabs on the left join the page at their right edge.
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x,
                        y: tab.origin.y.saturating_add_unsigned(2),
                    },
                    size: Size {
                        width: 1,
                        height: tab.size.height.saturating_sub(4),
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(1),
                        y: tab.origin.y.saturating_add_unsigned(1),
                    },
                    size: Size {
                        width: tab.size.width.saturating_sub(3),
                        height: 1,
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(1),
                        y: tab
                            .origin
                            .y
                            .saturating_add_unsigned(tab.size.height.saturating_sub(2)),
                    },
                    size: Size {
                        width: tab.size.width.saturating_sub(3),
                        height: 1,
                    },
                },
                dark,
            )?;
        }
        2 => {
            // Tabs on the right join the page at their left edge.
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(1),
                        y: tab.origin.y.saturating_add_unsigned(1),
                    },
                    size: Size {
                        width: tab.size.width.saturating_sub(2),
                        height: 1,
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab
                            .origin
                            .x
                            .saturating_add_unsigned(tab.size.width.saturating_sub(2)),
                        y: tab.origin.y.saturating_add_unsigned(2),
                    },
                    size: Size {
                        width: 1,
                        height: tab.size.height.saturating_sub(4),
                    },
                },
                dark,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(1),
                        y: tab
                            .origin
                            .y
                            .saturating_add_unsigned(tab.size.height.saturating_sub(2)),
                    },
                    size: Size {
                        width: tab.size.width.saturating_sub(2),
                        height: 1,
                    },
                },
                dark,
            )?;
        }
        _ => {
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(2),
                        y: tab.origin.y,
                    },
                    size: Size {
                        width: tab.size.width.saturating_sub(4),
                        height: 1,
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x.saturating_add_unsigned(1),
                        y: tab.origin.y.saturating_add_unsigned(1),
                    },
                    size: Size {
                        width: 1,
                        height: 1,
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab.origin.x,
                        y: tab.origin.y.saturating_add_unsigned(2),
                    },
                    size: Size {
                        width: 1,
                        height: tab.size.height.saturating_sub(2),
                    },
                },
                light,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab
                            .origin
                            .x
                            .saturating_add_unsigned(tab.size.width.saturating_sub(2)),
                        y: tab.origin.y.saturating_add_unsigned(1),
                    },
                    size: Size {
                        width: 1,
                        height: 1,
                    },
                },
                shadow,
            )?;
            line(
                Rect {
                    origin: Point {
                        x: tab
                            .origin
                            .x
                            .saturating_add_unsigned(tab.size.width.saturating_sub(1)),
                        y: tab.origin.y.saturating_add_unsigned(2),
                    },
                    size: Size {
                        width: 1,
                        height: tab.size.height.saturating_sub(2),
                    },
                },
                dark,
            )?;
            if !selected {
                line(
                    Rect {
                        origin: Point {
                            x: tab.origin.x.saturating_add_unsigned(1),
                            y: tab
                                .origin
                                .y
                                .saturating_add_unsigned(tab.size.height.saturating_sub(2)),
                        },
                        size: Size {
                            width: tab.size.width.saturating_sub(2),
                            height: 1,
                        },
                    },
                    shadow,
                )?;
                line(
                    Rect {
                        origin: Point {
                            x: tab.origin.x.saturating_add_unsigned(1),
                            y: tab
                                .origin
                                .y
                                .saturating_add_unsigned(tab.size.height.saturating_sub(1)),
                        },
                        size: Size {
                            width: tab.size.width.saturating_sub(2),
                            height: 1,
                        },
                    },
                    dark,
                )?;
            }
        }
    }
    if !selected {
        return Ok(());
    }
    let seam = match tab_axis {
        1 => Rect {
            origin: Point {
                x: tab
                    .origin
                    .x
                    .saturating_add_unsigned(tab.size.width.saturating_sub(2)),
                y: tab.origin.y.saturating_add_unsigned(2),
            },
            size: Size {
                width: tab.size.width.min(2),
                height: tab.size.height.saturating_sub(4),
            },
        },
        2 => Rect {
            origin: Point {
                x: tab.origin.x,
                y: tab.origin.y.saturating_add_unsigned(2),
            },
            size: Size {
                width: tab.size.width.min(2),
                height: tab.size.height.saturating_sub(4),
            },
        },
        _ => Rect {
            origin: Point {
                x: tab.origin.x.saturating_add_unsigned(2),
                y: tab
                    .origin
                    .y
                    .saturating_add_unsigned(tab.size.height.saturating_sub(2)),
            },
            size: Size {
                width: tab.size.width.saturating_sub(4),
                height: tab.size.height.min(2),
            },
        },
    };
    push_command(
        commands,
        DrawCommand::Fill {
            rect: seam,
            color: face,
            radius: 0,
        },
    )
}

fn render_chrome(
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    visual: VisualState,
    parent_kind: Option<WidgetKind>,
    seamless_calendar_block: bool,
    tooltip_caret: Option<(i32, bool)>,
    glass_sidebar_content: bool,
    desktop_item: bool,
    file_grid_item: bool,
    tab_axis: u8,
    joined_tab_page: bool,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    if super::layout_tokens::has_token(widget.text.as_deref(), "office-ribbon-heading") {
        return push_command(commands, DrawCommand::Fill { rect, color: tabs::strip_color(theme), radius: 0 });
    }
    if ["office-ribbon-page", "office-ribbon-group", "office-ribbon-tools", "office-ribbon-caption"]
        .iter().any(|token| super::layout_tokens::has_token(widget.text.as_deref(), token))
    {
        push_command(commands, DrawCommand::Fill { rect, color: tabs::page_color(theme, false), radius: 0 })?;
        if super::layout_tokens::has_token(widget.text.as_deref(), "office-ribbon-group") {
            let separator = Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(1)), y: rect.origin.y.saturating_add_unsigned(6) },
                size: Size { width: 1, height: rect.size.height.saturating_sub(12) } };
            push_command(commands, DrawCommand::Fill { rect: separator, color: theme.palette.border, radius: 0 })?;
        }
        if super::layout_tokens::has_token(widget.text.as_deref(), "office-ribbon-page") {
            let seam = Rect { origin: Point { x: rect.origin.x, y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(1)) }, size: Size { width: rect.size.width, height: 1 } };
            push_command(commands, DrawCommand::Fill { rect: seam, color: theme.palette.border, radius: 0 })?;
        }
        return Ok(());
    }
    if widget.kind == WidgetKind::Container
        && widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "content-card"))
    {
        let radius = bounded_radius(rect, theme.metrics.corner_radius.min(18));
        let [red, green, blue, _] = theme.palette.surface_raised.channels();
        let [ink_red, ink_green, ink_blue, _] = theme.palette.text.channels();
        // A quiet, opaque lift keeps cards distinct without a glass effect.
        let lift = |base: u8, ink: u8| ((u16::from(base) * 15 + u16::from(ink)) / 16) as u8;
        let face = rice_ui::Color::opaque(lift(red, ink_red), lift(green, ink_green), lift(blue, ink_blue));
        push_command(commands, DrawCommand::Fill { rect, color: face, radius })?;
        return push_command(commands, DrawCommand::Border {
            rect, color: theme.palette.border.with_alpha(180), width: 1, radius,
        });
    }
    // A dropdown in a form/action row uses the same chrome as adjacent
    // action buttons; its popup behavior remains owned by the Menu widget.
    let mut menu_button;
    let widget = if widget.kind == WidgetKind::Menu && parent_kind == Some(WidgetKind::Row) {
        menu_button = widget.clone();
        menu_button.kind = WidgetKind::Button;
        &menu_button
    } else { widget };
    let rect = if widget.kind == WidgetKind::ProgressBar
        && helpers::is_slim_progress(widget.text.as_deref())
    {
        let height = rect.size.height.min(6);
        Rect {
            origin: Point {
                x: rect.origin.x,
                y: rect
                    .origin
                    .y
                    .saturating_add_unsigned(rect.size.height.saturating_sub(height) / 2),
            },
            size: Size {
                width: rect.size.width,
                height,
            },
        }
    } else {
        rect
    };
    if super::layout_tokens::is_spacer(widget.text.as_deref()) {
        return Ok(());
    }
    if forced_dark_editor(widget) {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: FORCED_DARK_EDITOR_SURFACE,
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
        push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: FORCED_DARK_EDITOR_BORDER,
                width: theme.metrics.border_width.max(1),
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
        return Ok(());
    }
    let command_menu_heading = theme.chrome_profile == ChromeProfile::CommandDeck
        && widget.kind == WidgetKind::Menu
        && parent_kind == Some(WidgetKind::MenuBar);
    if command_menu_heading {
        let pill = rect.inset(Insets {
            top: 3,
            right: 4,
            bottom: 3,
            left: 4,
        });
        let active = visual.selected() || visual.hovered();
        let radius = u16::try_from(pill.size.height / 2).unwrap_or(u16::MAX);
        push_command(
            commands,
            DrawCommand::Fill {
                rect: pill,
                color: if active {
                    theme.palette.selection
                } else if widget.id.get() % 2 == 0 {
                    theme.palette.border
                } else {
                    theme.palette.danger
                },
                radius,
            },
        )?;
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::CommandDeck && parent_kind == Some(WidgetKind::Menu) {
        // Popup rows get their one authoritative treatment in
        // render_menu_row. The generic control pass used to flatten it.
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::Redmond98 && parent_kind == Some(WidgetKind::Menu) {
        if visual.selected() || visual.hovered() {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect,
                    color: theme.menu_palette.selection,
                    radius: 0,
                },
            )?;
        }
        return Ok(());
    }
    if theme.glass_menus && parent_kind == Some(WidgetKind::Menu) {
        if visual.selected() || visual.hovered() {
            return push_command(
                commands,
                DrawCommand::Fill {
                    rect,
                    color: theme.menu_palette.selection.with_alpha(210),
                    radius: theme.metrics.corner_radius.min(6),
                },
            );
        }
        return Ok(());
    }
    if widget.kind == WidgetKind::Tabs {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: tabs::strip_color(theme),
                radius: 0,
            },
        )?;
        let seam = match tab_axis {
            1 => Rect {
                origin: Point {
                    x: rect
                        .origin
                        .x
                        .saturating_add_unsigned(rect.size.width.saturating_sub(1)),
                    y: rect.origin.y,
                },
                size: Size {
                    width: 1,
                    height: rect.size.height,
                },
            },
            2 => Rect {
                origin: rect.origin,
                size: Size {
                    width: 1,
                    height: rect.size.height,
                },
            },
            _ => Rect {
                origin: Point {
                    x: rect.origin.x,
                    y: rect
                        .origin
                        .y
                        .saturating_add_unsigned(rect.size.height.saturating_sub(1)),
                },
                size: Size {
                    width: rect.size.width,
                    height: 1,
                },
            },
        };
        return push_command(
            commands,
            DrawCommand::Fill {
                rect: seam,
                color: theme.palette.border,
                radius: 0,
            },
        );
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && (joined_tab_page || super::layout_tokens::is_tab_page(widget.text.as_deref()))
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        return helpers::push_redmond_bevel(commands, rect, false);
    }
    if joined_tab_page || super::layout_tokens::is_tab_page(widget.text.as_deref()) {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: tabs::page_color(theme, false),
                radius: 0,
            },
        )?;
        return push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: theme.palette.border,
                width: theme.metrics.border_width.max(1),
                radius: 0,
            },
        );
    }
    if super::layout_tokens::is_tab_composite(widget.text.as_deref())
        || tabs::is_plain_tab(widget, parent_kind)
    {
        if tabs::is_plain_tab(widget, parent_kind) && widget.state.contains(WidgetState::FLAT) {
            let selected = visual.selected() || widget.state.contains(WidgetState::CHECKED);
            let face = if selected { theme.palette.accent }
                else if visual.hovered() { semantic::composite_over(theme.palette.focus.with_alpha(28), tabs::strip_color(theme)) }
                else { tabs::strip_color(theme) };
            push_command(commands, DrawCommand::Fill { rect, color: face, radius: 0 })?;
            if selected {
                let underline = Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(10), y: rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(4)) },
                    size: Size { width: rect.size.width.saturating_sub(20), height: 4 } };
                push_command(commands, DrawCommand::Fill { rect: underline, color: theme.palette.accent_text, radius: 0 })?;
            }
            return Ok(());
        }
        let selected = widget.state.contains(WidgetState::SELECTED);
        if theme.chrome_profile == ChromeProfile::Redmond98 {
            return render_redmond_tab(rect, selected, tabs::is_plain_tab(widget, parent_kind), tab_axis, commands);
        }
        return tabs::render_tab(
            rect,
            tabs::is_plain_tab(widget, parent_kind),
            theme,
            selected,
            visual.hovered(),
            tab_axis,
            super::layout_tokens::is_browser_tab(widget.text.as_deref()),
            commands,
        );
    }
    if widget.state.contains(WidgetState::SCENIC_BACKDROP) {
        return push_command(commands, DrawCommand::ScenicBackdrop { rect });
    }
    if widget.text.as_deref() == Some("panel:welcome-journey") {
        return push_command(commands, DrawCommand::WelcomeJourney { rect });
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && widget.kind == WidgetKind::IconGrid
        && super::layout_tokens::is_file_grid(widget.text.as_deref())
    {
        return push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        );
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && matches!(
            widget.kind,
            WidgetKind::List | WidgetKind::Table | WidgetKind::Tree
        )
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        )?;
        helpers::push_redmond_bevel(commands, rect, true)?;
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_redmond98_folder_tree(widget.text.as_deref())
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        )?;
        return push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: rect
                            .origin
                            .x
                            .saturating_add_unsigned(rect.size.width.saturating_sub(1)),
                        y: rect.origin.y,
                    },
                    size: Size {
                        width: 1,
                        height: rect.size.height,
                    },
                },
                color: theme.palette.border,
                radius: 0,
            },
        );
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_redmond98_webview(widget.text.as_deref())
    {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        )?;
        // The Explorer web-view identity wash starts blue at the left and
        // dissolves into the white folder surface toward the right.
        let wash_width = rect.size.width.min(230);
        for band in 0..16_u32 {
            let x = wash_width.saturating_mul(band) / 16;
            let next_x = wash_width.saturating_mul(band + 1) / 16;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: rect.origin.x.saturating_add_unsigned(x),
                            y: rect.origin.y,
                        },
                        size: Size {
                            width: next_x.saturating_sub(x),
                            height: rect.size.height.min(126),
                        },
                    },
                    color: rice_ui::Color::opaque(
                        214_u8.saturating_add(
                            u8::try_from(band.saturating_mul(41) / 15).unwrap_or(41),
                        ),
                        239_u8.saturating_add(
                            u8::try_from(band.saturating_mul(16) / 15).unwrap_or(16),
                        ),
                        249_u8
                            .saturating_add(u8::try_from(band.saturating_mul(6) / 15).unwrap_or(6)),
                    ),
                    radius: 0,
                },
            )?;
        }
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_redmond98_details_view(widget.text.as_deref())
    {
        return push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        );
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_redmond98_webview_rule(widget.text.as_deref())
    {
        return push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(0, 160, 208),
                radius: 0,
            },
        );
    }
    if theme.glass_sidebars && super::layout_tokens::is_sidebar_panel(widget.text.as_deref()) {
        return push_command(
            commands,
            DrawCommand::Glass {
                rect,
                tone: 0,
                tint: theme.palette.surface.with_alpha(112),
                radius: 0,
                hovered: false,
                control: false,
                active: false,
                structural: true,
                popup: false,
            },
        );
    }
    if glass_sidebar_content
        && widget.kind == WidgetKind::Button
        && (visual.selected() || visual.hovered())
    {
        return push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: if visual.selected() {
                    theme.palette.selection
                } else {
                    theme.palette.selection.with_alpha(44)
                },
                radius: bounded_radius(rect, theme.metrics.corner_radius.min(6)),
            },
        );
    }
    if super::layout_tokens::is_file_detail_row(widget.text.as_deref()) {
        if theme.chrome_profile == ChromeProfile::Redmond98 {
            return push_command(
                commands,
                DrawCommand::Fill {
                    rect,
                    color: if visual.selected() {
                        theme.palette.selection
                    } else {
                        rice_ui::Color::opaque(255, 255, 255)
                    },
                    radius: 0,
                },
            );
        }
        if visual.selected() || super::layout_tokens::is_alternate_row(widget.text.as_deref()) {
            return push_command(
                commands,
                DrawCommand::Fill {
                    rect,
                    color: if visual.selected() {
                        theme.palette.selection
                    } else {
                        theme.palette.border.with_alpha(64)
                    },
                    radius: 0,
                },
            );
        }
        return Ok(());
    }
    if widget.state.contains(WidgetState::LIQUID_GLASS) {
        let tone = if widget.state.contains(WidgetState::WARNING) {
            1
        } else if widget.state.contains(WidgetState::SUCCESS) {
            2
        } else if widget.state.contains(WidgetState::INFORMATION) {
            3
        } else {
            0
        };
        return push_command(
            commands,
            DrawCommand::Glass {
                rect,
                tone,
                tint: semantic::control_fill(
                    widget,
                    theme,
                    theme.palette.surface_raised,
                    visual.hovered(),
                ),
                radius: bounded_radius(rect, theme.metrics.corner_radius),
                hovered: visual.hovered(),
                control: false,
                active: visual.selected(),
                structural: false,
                popup: false,
            },
        );
    }
    if super::layout_tokens::is_desktop_thumbnail_cell(widget.text.as_deref()) {
        return Ok(());
    }
    if theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_thumbnail_cell(widget.text.as_deref())
    {
        // Classic Explorer never puts a modern card behind image/media
        // entries.  Its selection belongs to the icon mask and the tightly
        // fitted caption only; the thumbnail cell itself stays transparent.
        return Ok(());
    }
    if widget.kind == WidgetKind::GroupBox {
        let legend_x = rect
            .origin
            .x
            .saturating_add_unsigned(u32::from(theme.spacing.panel))
            .saturating_add_unsigned(u32::from(theme.spacing.control_x));
        return push_command(
            commands,
            DrawCommand::GroupBoxFrame {
                rect,
                legend: widget.text.clone().unwrap_or_default(),
                color: theme.palette.border,
                width: theme.metrics.border_width.max(1),
                size_px: semantic::text_size(widget, theme),
                legend_x,
                legend_y: rect.origin.y.saturating_add_unsigned(
                    u32::from(theme.metrics.control_height).min(rect.size.height) / 2,
                ),
            },
        );
    }
    if theme.chrome_profile == ChromeProfile::Redmond98 && widget.kind == WidgetKind::Dialog {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        helpers::push_redmond_bevel(commands, rect, false)?;
        if widget.text.as_deref().is_some_and(|text| text.starts_with("panel:window-editor:")) { return Ok(()); }
        let titlebar = Rect {
            origin: Point {
                x: rect.origin.x.saturating_add(3),
                y: rect.origin.y.saturating_add(3),
            },
            size: Size {
                width: rect.size.width.saturating_sub(6),
                height: rect.size.height.min(22).saturating_sub(3),
            },
        };
        for band in 0..16_u32 {
            let x = titlebar.size.width.saturating_mul(band) / 16;
            let next_x = titlebar.size.width.saturating_mul(band + 1) / 16;
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: titlebar.origin.x.saturating_add_unsigned(x),
                            y: titlebar.origin.y,
                        },
                        size: Size {
                            width: next_x.saturating_sub(x),
                            height: titlebar.size.height,
                        },
                    },
                    color: rice_ui::Color::opaque(
                        0,
                        u8::try_from(band.saturating_mul(96) / 15).unwrap_or(96),
                        128_u8.saturating_add(
                            u8::try_from(band.saturating_mul(79) / 15).unwrap_or(79),
                        ),
                    ),
                    radius: 0,
                },
            )?;
        }
        let close = dialog_close_rect(rect, theme).expect("Redmond dialogs have a close box");
        push_command(
            commands,
            DrawCommand::Fill {
                rect: close,
                color: rice_ui::Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        helpers::push_redmond_bevel(commands, close, false)?;
        push_command(
            commands,
            DrawCommand::Text {
                rect: close,
                text: String::from("×"),
                color: rice_ui::Color::opaque(0, 0, 0),
                size_px: 11,
                align: super::TextAlign::Center,
                role: TextRole::Label,
                caret: false,
                caret_offset: None,
                brand: false,
                gradient: false,
            },
        )?;
        return Ok(());
    }
    if widget.kind == WidgetKind::Tooltip {
        let (caret_x, caret_at_top) = tooltip_caret.unwrap_or((
            rect.origin.x.saturating_add_unsigned(rect.size.width / 2),
            true,
        ));
        return push_command(
            commands,
            DrawCommand::BalloonFrame {
                rect,
                fill: theme.palette.surface_raised,
                border: theme.palette.border,
                radius: bounded_radius(rect, theme.metrics.corner_radius),
                caret_x,
                caret_at_top,
            },
        );
    }
    if widget.parent.is_none() {
        match widget.text.as_deref() {
            Some("desktop:transparent") => return Ok(()),
            Some("desktop:solid") => {
                return push_command(
                    commands,
                    DrawCommand::Fill {
                        rect,
                        color: theme.palette.desktop,
                        radius: 0,
                    },
                );
            }
            _ => {}
        }
    }
    if matches!(widget.kind, WidgetKind::MenuBar | WidgetKind::ToolBar) {
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: if theme.chrome_profile == ChromeProfile::Orchard {
                    theme.palette.surface
                } else if widget.kind == WidgetKind::ToolBar
                    && theme.chrome_profile != ChromeProfile::Redmond98
                {
                    theme.palette.surface_raised
                } else {
                    theme.palette.surface
                },
                radius: 0,
            },
        )?;
        if theme.chrome_profile == ChromeProfile::Orchard && widget.kind == WidgetKind::MenuBar {
            // Orchard's in-window menu and tool rows read as one continuous
            // translucent header instead of a stack of bordered bands.
            return Ok(());
        }
        return push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: rect.origin.x,
                        y: rect
                            .origin
                            .y
                            .saturating_add_unsigned(rect.size.height.saturating_sub(1)),
                    },
                    size: Size {
                        width: rect.size.width,
                        height: 1,
                    },
                },
                color: theme.palette.border,
                radius: 0,
            },
        );
    }
    if widget.kind == WidgetKind::ScrollBar {
        return scrollbar::render(widget, rect, theme, commands);
    }
    if widget.kind == WidgetKind::Slider {
        return slider::render(widget, rect, theme, visual.hovered(), commands);
    }
    if widget.kind == WidgetKind::Toggle {
        return choice::render_toggle(widget, rect, theme, visual.hovered(), commands);
    }
    if matches!(widget.kind, WidgetKind::CheckBox | WidgetKind::RadioButton)
        && visual.appearance != Appearance::Flat
    {
        return Ok(());
    }
    let feature_card_action = super::layout_tokens::is_feature_card_action(widget.text.as_deref());
    let redmond_icon_item = theme.chrome_profile == ChromeProfile::Redmond98
        && ((widget.kind == WidgetKind::Button && (desktop_item || file_grid_item))
            || widget.kind == WidgetKind::TreeItem);
    // Windows 98 selects icon captions, not the complete desktop/grid cell.
    // The caption highlight is emitted by `render_content`; returning here
    // prevents the generic selected-control fill from painting the whole tile.
    if redmond_icon_item {
        return Ok(());
    }
    let redmond_list_row = theme.chrome_profile == ChromeProfile::Redmond98
        && widget.kind == WidgetKind::Button
        && parent_kind == Some(WidgetKind::List);
    if redmond_list_row && visual.selected() {
        // A Win98 list-box choice is a navy selection row, not a raised gray
        // push button. Keep it inside the parent's two-pixel sunken frame.
        let selection = rect.inset(Insets {
            top: 0,
            right: 2,
            bottom: 0,
            left: 2,
        });
        return push_command(
            commands,
            DrawCommand::Fill {
                rect: selection,
                color: theme.palette.selection,
                radius: 0,
            },
        );
    }
    let button_has_chrome = widget.kind == WidgetKind::Button
        && !redmond_list_row
        && !semantic::has_flat_chrome(widget)
        && (visual.appearance != Appearance::Flat || visual.selected() || visual.hovered());
    if theme.frosted_widgets.buttons
        && button_has_chrome
        && visual.selected()
        && parent_kind == Some(WidgetKind::IconGrid)
    {
        return push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: theme.palette.selection,
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        );
    }
    if theme.frosted_widgets.buttons
        && button_has_chrome
        && theme.button_shape == ButtonShape::Default
    {
        let tone = if widget.state.contains(WidgetState::WARNING)
            || widget.state.contains(WidgetState::DESTRUCTIVE)
        {
            1
        } else if widget.state.contains(WidgetState::SUCCESS) {
            2
        } else if widget.state.contains(WidgetState::INFORMATION)
            || widget.state.contains(WidgetState::ACCENT)
        {
            3
        } else {
            0
        };
        return push_command(
            commands,
            DrawCommand::Glass {
                rect,
                tone,
                tint: semantic::control_fill(
                    widget,
                    theme,
                    theme.palette.surface_raised,
                    visual.hovered(),
                ),
                radius: if seamless_calendar_block {
                    0
                } else {
                    bounded_radius(rect, theme.metrics.corner_radius)
                },
                hovered: visual.hovered(),
                control: true,
                active: visual.selected(),
                structural: false,
                popup: false,
            },
        );
    }
    if visual.appearance != Appearance::Plain || widget.parent.is_none() {
        let menu_heading = widget.kind == WidgetKind::Menu && visual.appearance == Appearance::Flat;
        let menu_row = parent_kind == Some(WidgetKind::Menu);
        let fallback = if theme.chrome_profile == ChromeProfile::YellowJacket
            && widget.kind == WidgetKind::Button
            && visual.pressed()
        {
            semantic::yellow_jacket_pressed_surface(theme)
        } else if theme.chrome_profile == ChromeProfile::CommandDeck
            && button_has_chrome
            && !feature_card_action
        {
            // LCARS controls are opaque color keys rather than dark controls
            // with a colored outline. Keep the states in the same bright
            // family so their automatically-derived dark ink stays legible.
            if visual.pressed() {
                theme.palette.border
            } else if visual.selected() {
                theme.palette.selection
            } else if visual.appearance == Appearance::Flat && visual.hovered() {
                // Collection hover is a separate LCARS state from selection:
                // purple for pointing, yellow for the persistent choice.
                theme.palette.border
            } else if visual.hovered() {
                theme.palette.accent
            } else {
                theme.palette.selection
            }
        } else if menu_row && visual.hovered() {
            theme.palette.selection
        // Classic split panes are gray chrome; their list/tree/scroll
        // controls paint the inset white content area themselves.
        } else if visual.appearance == Appearance::SplitLeading
            && theme.chrome_profile != ChromeProfile::Redmond98 {
            theme.palette.surface_raised
        } else if visual.appearance == Appearance::SplitTrailing {
            theme.palette.surface
        } else if visual.selected() && menu_heading {
            if theme.chrome_profile == ChromeProfile::Redmond98 {
                theme.palette.surface
            } else {
                theme.palette.surface_raised
            }
        } else if visual.selected() {
            theme.palette.selection
        } else if visual.hovered() && feature_card_action {
            // A raised-surface fill is identical to the resting card in
            // several themes; use a quiet semantic wash without moving it.
            theme.palette.selection.with_alpha(40)
        } else if visual.hovered() && visual.appearance == Appearance::Flat {
            // Flat collection rows often sit on a raised panel, so using the
            // raised surface again makes pointer hover invisible. A restrained
            // accent wash stays visible across light, dark, and glass themes.
            theme.palette.selection.with_alpha(36)
        } else if visual.hovered() || visual.appearance == Appearance::Control {
            theme.palette.surface_raised
        } else {
            theme.palette.surface
        };
        let color = if theme.chrome_profile == ChromeProfile::Redmond98 && button_has_chrome {
            theme.palette.surface
        } else if theme.chrome_profile == ChromeProfile::YellowJacket
            && widget.kind == WidgetKind::Button
            && button_has_chrome
            && !widget.state.contains(WidgetState::ACCENT)
            && !widget.state.contains(WidgetState::DESTRUCTIVE)
            && !widget.state.contains(WidgetState::INFORMATION)
            && !widget.state.contains(WidgetState::SUCCESS)
            && !widget.state.contains(WidgetState::WARNING)
        {
            fallback
        } else {
            semantic::control_fill(widget, theme, fallback, visual.hovered())
        };
        let color = if widget.kind == WidgetKind::Button && visual.appearance != Appearance::Flat && widget.state.contains(WidgetState::ENABLED)
            && visual.hovered() && !visual.pressed() && !visual.selected() && !feature_card_action && theme.chrome_profile != ChromeProfile::Redmond98 {
            let [r,g,b,a] = color.channels();
            let light = (u32::from(r)*54 + u32::from(g)*183 + u32::from(b)*19)/256;
            let target = if light > 140 {0} else {255};
            let amount = if visual.pressed() {48} else {28};
            let mix = |channel:u8| ((u32::from(channel)*(255-amount)+target*amount)/255) as u8;
            rice_ui::Color::rgba(mix(r),mix(g),mix(b),a)
        } else {color};
        // Frosted Ice is a material treatment, not an excuse to expose the
        // application through modal content. Dialogs need a stable reading
        // plane; keep only a restrained amount of translucency.
        let color = if widget.kind == WidgetKind::Dialog {
            let alpha = color.channels()[3].max(DIALOG_SURFACE_MIN_ALPHA);
            color.with_alpha(alpha)
        } else {
            color
        };
        if !semantic::has_flat_chrome(widget)
            && (visual.appearance != Appearance::Flat || visual.selected() || visual.hovered())
        {
            if button_has_chrome && theme.button_shape != ButtonShape::Default {
                push_command(
                    commands,
                    DrawCommand::ShapedFill {
                        rect,
                        color,
                        shape: theme.button_shape,
                    },
                )?;
                if theme.chrome_profile != ChromeProfile::CommandDeck {
                    push_command(
                        commands,
                        DrawCommand::ShapedBorder {
                            rect,
                            color: theme.palette.border,
                            width: theme.metrics.border_width.max(1),
                            shape: theme.button_shape,
                        },
                    )?;
                }
            } else {
                push_command(
                    commands,
                    DrawCommand::Fill {
                        rect,
                        color,
                        // The compositor owns the outer window silhouette.
                        // Rounding the root client surface as well creates a
                        // second pair of corners directly below the titlebar,
                        // which reads as a malformed nested window.
                        radius: if widget.parent.is_none()
                            || seamless_calendar_block
                            || matches!(
                                visual.appearance,
                                Appearance::SplitLeading | Appearance::SplitTrailing
                            ) {
                            0
                        } else {
                            bounded_radius(rect, theme.metrics.corner_radius)
                        },
                    },
                )?;
            }
        }
    }
    if widget.kind == WidgetKind::ProgressBar {
        let indeterminate = widget.state.contains(WidgetState::INDETERMINATE);
        if indeterminate {
            push_command(
                commands,
                DrawCommand::Activity {
                    rect,
                    color: theme.palette.accent,
                },
            )?;
            return Ok(());
        }
        let width = progress_width(widget.text.as_deref(), rect.size.width);
        let progress_rect = Rect {
            origin: Point {
                x: rect.origin.x,
                y: rect.origin.y,
            },
            size: Size {
                width,
                height: rect.size.height,
            },
        };
        push_command(
            commands,
            DrawCommand::Fill {
                rect: progress_rect,
                color: theme.palette.accent,
                radius: bounded_radius(progress_rect, theme.metrics.corner_radius),
            },
        )?;
    }
    if theme.signal_controls && button_has_chrome && rect.size.width > 12 && rect.size.height > 12 {
        let color = if widget.state.contains(WidgetState::ENABLED) { theme.palette.accent } else { theme.palette.text_muted };
        for (x, y, right, bottom) in [
            (rect.origin.x + 2, rect.origin.y + 2, false, false),
            (rect.origin.x.saturating_add_unsigned(rect.size.width).saturating_sub(3), rect.origin.y.saturating_add_unsigned(rect.size.height).saturating_sub(3), true, true),
        ] {
            for mark in [
                Rect { origin: Point { x: if right { x - 5 } else { x }, y }, size: Size { width: 6, height: 1 } },
                Rect { origin: Point { x, y: if bottom { y - 5 } else { y } }, size: Size { width: 1, height: 6 } },
            ] { push_command(commands, DrawCommand::Fill { rect: mark, color, radius: 0 })?; }
        }
    }
    let familiar_button = button_has_chrome
        && theme.button_shape == ButtonShape::Default
        && (theme.chrome_profile == ChromeProfile::YellowJacket
            || (theme.metrics.corner_radius == 0
                && theme.metrics.control_height == 24
                && theme.metrics.scrollbar_width == 17));
    if theme.chrome_profile == ChromeProfile::Redmond98
        && widget.kind == WidgetKind::Menu
        && parent_kind == Some(WidgetKind::MenuBar)
        && visual.selected()
    {
        // Win98 keeps the owning menubar heading visibly latched while the
        // pointer traverses its popup. It is a sunken gray menu button, not a
        // white tab and not an unmarked piece of the menubar.
        push_command(
            commands,
            DrawCommand::Fill {
                rect,
                color: rice_ui::Color::opaque(192, 192, 192),
                radius: 0,
            },
        )?;
        helpers::push_redmond_bevel(commands, rect, true)?;
        return Ok(());
    }
    if button_has_chrome && theme.chrome_profile == ChromeProfile::Redmond98 {
        helpers::push_redmond_bevel(commands, rect, visual.pressed())?;
        return Ok(());
    }
    if familiar_button {
        let (top_left, bottom_right) = if visual.selected() {
            (theme.palette.border, theme.palette.surface_raised)
        } else {
            (theme.palette.surface_raised, theme.palette.border)
        };
        let bevel_rects = if theme.chrome_profile == ChromeProfile::YellowJacket {
            vec![
                rect,
                rect.inset(Insets {
                    top: 1,
                    right: 1,
                    bottom: 1,
                    left: 1,
                }),
            ]
        } else {
            vec![rect]
        };
        for bevel in bevel_rects {
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
        return Ok(());
    }
    if matches!(
        visual.appearance,
        Appearance::Control | Appearance::Panel | Appearance::Bar
    ) && !semantic::has_flat_chrome(widget)
        && !(widget.kind == WidgetKind::Button && !visual.invalid())
    {
        let redmond_sunken = theme.chrome_profile == ChromeProfile::Redmond98
            && matches!(
                widget.kind,
                WidgetKind::TextField
                    | WidgetKind::PasswordField
                    | WidgetKind::TextEditor
                    | WidgetKind::List
                    | WidgetKind::Table
                    | WidgetKind::Tree
                    | WidgetKind::IconGrid
                    | WidgetKind::ScrollView
            );
        if redmond_sunken && !visual.invalid() {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect,
                    color: rice_ui::Color::opaque(255, 255, 255),
                    radius: 0,
                },
            )?;
            helpers::push_redmond_bevel(commands, rect, true)?;
            return Ok(());
        }
        let quiet_yellow_jacket_field = theme.chrome_profile == ChromeProfile::YellowJacket
            && matches!(
                widget.kind,
                WidgetKind::TextField | WidgetKind::PasswordField | WidgetKind::TextEditor
            )
            && !visual.invalid();
        push_command(
            commands,
            DrawCommand::Border {
                rect,
                color: if visual.invalid() {
                    theme.palette.danger
                } else if quiet_yellow_jacket_field {
                    theme.palette.border.with_alpha(112)
                } else {
                    theme.palette.border
                },
                width: if quiet_yellow_jacket_field {
                    1
                } else {
                    theme.metrics.border_width
                },
                radius: bounded_radius(rect, theme.metrics.corner_radius),
            },
        )?;
    }
    Ok(())
}

fn render_row_separator(
    rect: Rect,
    theme: &Theme,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let inset = u32::from(theme.spacing.control_x);
    push_command(
        commands,
        DrawCommand::Fill {
            rect: Rect {
                origin: Point {
                    x: rect.origin.x.saturating_add_unsigned(inset),
                    y: rect
                        .origin
                        .y
                        .saturating_add_unsigned(rect.size.height.saturating_sub(1)),
                },
                size: Size {
                    width: rect.size.width.saturating_sub(inset.saturating_mul(2)),
                    height: 1,
                },
            },
            color: theme.palette.border,
            radius: 0,
        },
    )
}

#[allow(clippy::too_many_arguments)]
fn render_content(
    document_font: Option<&rice_ui::grid_frame::GridFont>,
    document_paint: Option<&rice_ui::grid_frame::DocumentPaint>,
    document_zoom: u16,
    compact_label: bool,
    chat_composer: bool,
    widget: &Widget,
    rect: Rect,
    theme: &Theme,
    visual: VisualState,
    parent_kind: Option<WidgetKind>,
    bookmark_primary_control: bool,
    desktop_item: bool,
    file_grid_item: bool,
    inherited_content_background: Option<rice_ui::Color>,
    detail_tree_depth: usize,
    submenu_opens_left: bool,
    has_submenu: bool,
    menu_has_leading_column: bool,
    tree_item_has_children: bool,
    redmond_calculator_key: bool,
    focused: bool,
    text_selection: Option<(usize, usize)>,
    text_caret: Option<usize>,
    commands: &mut Vec<DrawCommand>,
) -> Result<(), NativeUiError> {
    let color_widget = color_control::normalized(widget);
    let widget = color_widget.as_ref().unwrap_or(widget);
    // A large toolbar command shares one native hit target for its icon and
    // caption. Menus keep their native popup and disclosure behavior.
    if parent_kind == Some(WidgetKind::ToolBar)
        && matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
        && widget.state.contains(WidgetState::LARGE_ICON)
    {
        let color = semantic::content_color(widget, theme, visual.enabled(), Some(tabs::page_color(theme, false)), false);
        let edge = 32.min(rect.size.width).min(rect.size.height.saturating_sub(32));
        if let Some(icon) = widget.icon {
            push_command(commands, DrawCommand::Icon { rect: Rect { origin: Point {
                x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(edge) / 2), y: rect.origin.y.saturating_add_unsigned(8) }, size: Size { width: edge, height: edge } },
                icon, color, hovered: visual.hovered(), animated: false, shortcut: false })?;
        }
        let disclosure = if widget.kind == WidgetKind::Menu { 12 } else { 0 };
        push_command(commands, DrawCommand::Text { rect: Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(4), y: rect.origin.y.saturating_add_unsigned(46) },
            size: Size { width: rect.size.width.saturating_sub(8 + disclosure), height: rect.size.height.saturating_sub(48) } },
            text: widget.text.clone().unwrap_or_default(), color, size_px: theme.typography.body_px, align: TextAlign::Center, role: TextRole::Label,
            caret: false, caret_offset: None, brand: false, gradient: false })?;
        if disclosure != 0 {
            push_command(commands, DrawCommand::Icon { rect: Rect { origin: Point { x: rect.origin.x.saturating_add_unsigned(rect.size.width.saturating_sub(14)), y: rect.origin.y.saturating_add_unsigned(52) },
                size: Size { width: 10, height: 10 } }, icon: rice_ui::IconId::ChevronDown, color, hovered: visual.hovered(), animated: false, shortcut: false })?;
        }
        return Ok(());
    }
    if let Some(initials) = widget.text.as_deref().and_then(|text| text.strip_prefix("mail-avatar:")) {
        let circle = Rect { origin: Point { x: rect.origin.x + 4, y: rect.origin.y + 4 }, size: Size { width: 52, height: 52 } };
        push_command(commands, DrawCommand::Fill { rect: circle, color: theme.palette.accent, radius: 26 })?;
        push_command(commands, DrawCommand::Text { rect: circle, text: initials.to_owned(), color: theme.palette.accent_text,
            size_px: 20, align: TextAlign::Center, role: TextRole::Label, caret: false, caret_offset: None, brand: false, gradient: false })?;
        return Ok(());
    }
    if widget.kind == WidgetKind::Button
        && let Some(summary) = widget.text.as_deref().and_then(|text| text.strip_prefix("mail-summary:"))
    {
        if visual.selected() || visual.hovered() {
            push_command(commands, DrawCommand::Fill { rect, color: if visual.selected() { theme.palette.selection } else { theme.palette.accent.with_alpha(36) }, radius: 10 })?;
        }
        if widget.state.contains(WidgetState::EMPHASIZED) {
            push_command(commands, DrawCommand::Fill { rect: Rect { origin: Point { x: rect.origin.x+3, y: rect.origin.y+21 }, size: Size { width:5, height:5 } }, color: theme.palette.accent, radius: 3 })?;
        }
        let color = if widget.state.contains(WidgetState::SELECTED) { theme.palette.accent_text } else { theme.palette.text };
        if widget.icon == Some(rice_ui::IconId::Attachment) {
            push_command(commands, DrawCommand::Icon { rect: Rect { origin: Point { x: rect.origin.x + rect.size.width.saturating_sub(28) as i32, y: rect.origin.y + 61 }, size: Size { width:16, height:16 } }, icon: rice_ui::IconId::Attachment, color, hovered:visual.hovered(), animated:false, shortcut:false })?;
        }
        let date = summary.lines().nth(3).unwrap_or("");
        if !date.is_empty() {
            push_command(commands, DrawCommand::Text {
                rect: Rect { origin: Point { x: rect.origin.x + rect.size.width.saturating_sub(86) as i32, y: rect.origin.y + 12 }, size: Size { width: 72, height: 24 } },
                text: date.to_owned(), color: color.with_alpha(170), size_px: 11, align: TextAlign::End, role: TextRole::Label,
                caret: false, caret_offset: None, brand: false, gradient: false,
            })?;
        }
        for (index, line) in summary.lines().take(3).enumerate() {
            push_command(commands, DrawCommand::Text {
                rect: Rect { origin: Point { x: rect.origin.x + 14, y: rect.origin.y + 12 + index as i32 * 24 },
                    size: Size { width: rect.size.width.saturating_sub(if index == 0 && !date.is_empty() { 110 } else if index == 2 && widget.icon == Some(rice_ui::IconId::Attachment) { 50 } else { 28 }), height: 24 } },
                text: line.to_owned(), color: if index == 2 { color.with_alpha(170) } else { color },
                size_px: if index == 0 { 16 } else { 14 }, align: TextAlign::Start,
                role: TextRole::Label, caret: false, caret_offset: None, brand: false, gradient: false,
            })?;
        }
        return Ok(());
    }
    let command_deck_key = theme.chrome_profile == ChromeProfile::CommandDeck
        && !tabs::is_plain_tab(widget, parent_kind)
        && (widget.kind == WidgetKind::Button
            && !super::layout_tokens::is_feature_card_action(widget.text.as_deref())
            && (visual.appearance != Appearance::Flat || visual.hovered())
            || widget.kind == WidgetKind::Menu && parent_kind == Some(WidgetKind::MenuBar));
    if matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
        && let Some(value)=widget.text.as_deref().and_then(|text|text.strip_prefix("control-tile:")) {
        let (title,status)=value.split_once('|').unwrap_or((value,""));
        let color=if widget.state.contains(WidgetState::SELECTED){theme.palette.accent_text}else{theme.palette.text};
        let color=if visual.enabled(){color}else{color.with_alpha(110)};
        let icon_y=rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(22)/2);
        let copy_y=rect.origin.y.saturating_add_unsigned(rect.size.height.saturating_sub(40)/2);
        if let Some(icon)=widget.icon {
            push_command(commands,DrawCommand::Icon{rect:Rect{origin:Point{x:rect.origin.x+14,y:icon_y},size:Size{width:22,height:22}},icon,color,hovered:visual.hovered(),animated:false,shortcut:false})?;
        }
        for(text,y,size,height)in [(title,0,14,20),(status,21,11,18)] {
            push_command(commands,DrawCommand::Text{rect:Rect{origin:Point{x:rect.origin.x+46,y:copy_y+y},size:Size{width:rect.size.width.saturating_sub(58),height}},text:text.into(),color:if size==11{color.with_alpha(185)}else{color},size_px:size,align:TextAlign::Start,role:TextRole::Label,caret:false,caret_offset:None,brand:false,gradient:false})?;
        }
        return Ok(());
    }
    // A popup can contain compound content or editable controls. Only actual
    // menu commands/captions use MenuText; interpreting a ScrollView's layout
    // metadata here leaked viewport tokens and bypassed native editor painting.
    if parent_kind == Some(WidgetKind::Menu)
        && matches!(widget.kind, WidgetKind::MenuItem | WidgetKind::Menu | WidgetKind::Button
            | WidgetKind::Label | WidgetKind::CheckBox | WidgetKind::RadioButton | WidgetKind::Toggle)
        && menu::render_menu_row(
            widget,
            rect,
            theme,
            visual.enabled(),
            visual.selected() || visual.hovered(),
            submenu_opens_left,
            has_submenu,
            menu_has_leading_column,
            commands,
        )?
    {
        return Ok(());
    }
    if widget.kind == WidgetKind::ScrollBar {
        return Ok(());
    }
    if widget.kind == WidgetKind::ProgressBar {
        return Ok(());
    }
    let content_rect = if widget.kind == WidgetKind::Accordion {
        Rect {
            origin: rect.origin,
            size: Size {
                width: rect.size.width,
                height: u32::from(theme.metrics.control_height).min(rect.size.height),
            },
        }
    } else if widget.kind == WidgetKind::Dialog && theme.chrome_profile == ChromeProfile::Redmond98
    {
        // The dialog widget's own label is its caption. Constrain it to the
        // blue title strip and stop before the close box; using the ordinary
        // panel padding lets the caption descend into the client controls.
        Rect {
            origin: Point {
                x: rect.origin.x.saturating_add_unsigned(5),
                y: rect.origin.y.saturating_add_unsigned(3),
            },
            size: Size {
                width: rect.size.width.saturating_sub(28),
                height: rect.size.height.min(22).saturating_sub(3),
            },
        }
    } else if widget.kind == WidgetKind::Dialog {
        let panel = u32::from(theme.spacing.panel);
        Rect {
            origin: Point {
                x: rect.origin.x.saturating_add_unsigned(panel),
                y: rect.origin.y.saturating_add_unsigned(panel),
            },
            size: Size {
                width: rect.size.width.saturating_sub(panel.saturating_mul(2)),
                height: u32::from(theme.metrics.control_height),
            },
        }
    } else if matches!(widget.kind, WidgetKind::GroupBox | WidgetKind::Popover) {
        Rect {
            origin: Point {
                x: rect
                    .origin
                    .x
                    .saturating_add_unsigned(u32::from(theme.spacing.panel)),
                y: rect.origin.y,
            },
            size: Size {
                width: rect
                    .size
                    .width
                    .saturating_sub(u32::from(theme.spacing.panel).saturating_mul(2)),
                height: u32::from(theme.metrics.control_height).min(rect.size.height),
            },
        }
    } else {
        rect
    };
    let wordmark = widget.kind == WidgetKind::Label
        && widget.state.contains(WidgetState::BRAND_TYPE)
        && widget.state.contains(WidgetState::DISPLAY);
    let window_control = super::layout_tokens::is_window_control(widget.text.as_deref());
    let compact_icon = matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu | WidgetKind::MenuItem)
        && widget.icon.is_some()
        && (widget.text.as_deref().is_none_or(|text| text.is_empty() || text.starts_with("panel:"))
            || parent_kind == Some(WidgetKind::ToolBar)
                && !widget.state.contains(WidgetState::START_ALIGNED));
    // Calendar days and short numeric keys retain their ordinary font in a
    // comfortable target. A two-digit day must not lose most of its 40px box
    // to the padding intended for a full caption.
    let short_numeric_button = widget.kind == WidgetKind::Button && widget.icon.is_none()
        && widget.text.as_deref().is_some_and(|text| !text.is_empty() && text.len() <= 2 && text.bytes().all(|b| b.is_ascii_digit()));
    let horizontal_inset = if short_numeric_button {
        u32::from(theme.spacing.control_x).min(content_rect.size.width.saturating_sub(
            super::layout::estimated_text_width(widget.text.as_deref().unwrap_or(""), theme).saturating_add(4)
        ) / 2)
    } else if compact_icon {
        4
    } else if desktop_item || compact_label {
        0
    } else if wordmark {
        0
    } else if bookmark_primary_control {
        2
    } else if window_control {
        4
    } else {
        u32::from(theme.spacing.control_x)
    };
    let vertical_inset = if desktop_item || compact_label {
        0
    } else if matches!(widget.kind, WidgetKind::TextField | WidgetKind::PasswordField) {
        // Inline editors inherit their cell's compact height. Keep the normal
        // theme font readable instead of consuming most of that height with
        // dialog-field padding and letting the canvas shrink the text to fit.
        let text_height = u32::from(semantic::text_size(widget, theme));
        u32::from(theme.spacing.control_y).min(
            content_rect.size.height.saturating_sub(text_height + 2).saturating_div(2).max(1),
        )
    } else if window_control || compact_icon || (parent_kind == Some(WidgetKind::ToolBar) && matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)) {
        4
    } else {
        u32::from(theme.spacing.control_y)
    };
    // The plain viewport owns its four-pixel inset. Its observed size,
    // browser paint and pointer measurement must use the same rectangle.
    let plain_viewport = widget.kind == WidgetKind::TextEditor
        && widget.text.as_deref().and_then(EditorText::parse).is_some_and(|p| p.viewport.is_some());
    let inset = if (document_font.is_some() || plain_viewport) && widget.kind == WidgetKind::TextEditor { content_rect } else if document_font.is_some() { let scale=|n:u32|(n*u32::from(document_zoom)+50)/100; content_rect.inset(Insets { left: scale(4), right: scale(4), top: scale(1), bottom: scale(1) }) } else { content_rect.inset(Insets {
        top: vertical_inset,
        right: horizontal_inset,
        bottom: vertical_inset,
        left: horizontal_inset,
    }) };
    let toolbar_icon = matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
        && parent_kind == Some(WidgetKind::ToolBar)
        && !widget.state.contains(WidgetState::START_ALIGNED);
    let inset = if widget.kind == WidgetKind::Toggle {
        if widget.state.contains(WidgetState::START_ALIGNED) {
            inset.inset(Insets {
                top: 0,
                right: 0,
                bottom: 0,
                left: 44_u32.min(inset.size.width),
            })
        } else {
            inset.inset(Insets {
                top: 0,
                right: 44_u32.min(inset.size.width),
                bottom: 0,
                left: 0,
            })
        }
    } else {
        choice::content_inset(widget, inset, theme, commands)?
    };
    let mut inset = if detail_tree_depth == 0 {
        inset
    } else {
        let step =
            u32::from(theme.metrics.icon_small).saturating_add(u32::from(theme.spacing.unit));
        let indent = step
            .saturating_mul(u32::try_from(detail_tree_depth).unwrap_or(u32::MAX))
            .min(inset.size.width);
        Rect {
            origin: Point {
                x: inset.origin.x.saturating_add_unsigned(indent),
                y: inset.origin.y,
            },
            size: Size {
                width: inset.size.width.saturating_sub(indent),
                height: inset.size.height,
            },
        }
    };
    let redmond_tree_item =
        theme.chrome_profile == ChromeProfile::Redmond98 && widget.kind == WidgetKind::TreeItem;
    let redmond_detail_disclosure = theme.chrome_profile == ChromeProfile::Redmond98
        && super::layout_tokens::is_file_detail_disclosure(widget.text.as_deref());
    if tree_item_has_children && (redmond_tree_item || redmond_detail_disclosure) {
        let disclosure_size = 9_u32.min(inset.size.width).min(inset.size.height);
        let disclosure = Rect {
            origin: Point {
                x: inset.origin.x,
                y: inset
                    .origin
                    .y
                    .saturating_add_unsigned(inset.size.height.saturating_sub(disclosure_size) / 2),
            },
            size: Size {
                width: disclosure_size,
                height: disclosure_size,
            },
        };
        push_command(
            commands,
            DrawCommand::Fill {
                rect: disclosure,
                color: rice_ui::Color::opaque(255, 255, 255),
                radius: 0,
            },
        )?;
        push_command(
            commands,
            DrawCommand::Border {
                rect: disclosure,
                color: rice_ui::Color::opaque(128, 128, 128),
                width: 1,
                radius: 0,
            },
        )?;
        let center_x = disclosure
            .origin
            .x
            .saturating_add_unsigned(disclosure.size.width / 2);
        let center_y = disclosure
            .origin
            .y
            .saturating_add_unsigned(disclosure.size.height / 2);
        push_command(
            commands,
            DrawCommand::Fill {
                rect: Rect {
                    origin: Point {
                        x: disclosure.origin.x.saturating_add_unsigned(2),
                        y: center_y,
                    },
                    size: Size {
                        width: disclosure.size.width.saturating_sub(4),
                        height: 1,
                    },
                },
                color: rice_ui::Color::opaque(0, 0, 0),
                radius: 0,
            },
        )?;
        if !widget.state.contains(WidgetState::CHECKED) {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: Rect {
                        origin: Point {
                            x: center_x,
                            y: disclosure.origin.y.saturating_add_unsigned(2),
                        },
                        size: Size {
                            width: 1,
                            height: disclosure.size.height.saturating_sub(4),
                        },
                    },
                    color: rice_ui::Color::opaque(0, 0, 0),
                    radius: 0,
                },
            )?;
        }
        let reserved = u32::from(theme.metrics.icon_small);
        inset.origin.x = inset.origin.x.saturating_add_unsigned(reserved);
        inset.size.width = inset.size.width.saturating_sub(reserved);
        if redmond_detail_disclosure {
            // The Win98 disclosure is the boxed plus/minus itself. Do not
            // layer the cross-platform chevron icon over that native glyph.
            return Ok(());
        }
    } else if tree_item_has_children {
        let size = u32::from(theme.metrics.icon_small)
            .min(inset.size.width)
            .min(inset.size.height);
        push_command(
            commands,
            DrawCommand::Icon {
                rect: Rect {
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
                },
                icon: if widget.state.contains(WidgetState::CHECKED) {
                    rice_ui::IconId::ChevronDown
                } else {
                    rice_ui::IconId::GoForward
                },
                color: if theme.chrome_profile == ChromeProfile::Redmond98
                    && widget.kind == WidgetKind::Button
                {
                    theme.palette.text
                } else {
                    semantic::content_color(
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
                    )
                },
                hovered: visual.hovered(),
                animated: false,
                shortcut: false,
            },
        )?;
        let disclosure = size.saturating_add(u32::from(theme.spacing.unit));
        inset.origin.x = inset.origin.x.saturating_add_unsigned(disclosure);
        inset.size.width = inset.size.width.saturating_sub(disclosure);
    } else if redmond_tree_item {
        let reserved = u32::from(theme.metrics.icon_small);
        inset.origin.x = inset.origin.x.saturating_add_unsigned(reserved);
        inset.size.width = inset.size.width.saturating_sub(reserved);
    }
    let content_x = if widget.kind == WidgetKind::Spinner {
        let size = u32::from(theme.metrics.icon_small)
            .min(inset.size.width)
            .min(inset.size.height);
        let spinner = Rect {
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
        push_command(
            commands,
            DrawCommand::Spinner {
                rect: spinner,
                color: theme.palette.accent,
            },
        )?;
        inset
            .origin
            .x
            .saturating_add_unsigned(size.saturating_add(u32::from(theme.spacing.unit)))
    } else {
        icon::render(
            widget,
            inset,
            theme,
            visual,
            parent_kind,
            desktop_item,
            file_grid_item,
            inherited_content_background,
            commands,
        )?
    };
    // Labelled toolbar menus reserve their disclosure independently of text.
    if widget.kind == WidgetKind::Menu && parent_kind == Some(WidgetKind::ToolBar)
        && widget.state.contains(WidgetState::START_ALIGNED)
        && widget.icon.is_some_and(|icon| !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown)) {
        inset.size.width = inset.size.width.saturating_sub(12);
    }
    if let Some(text) = &widget.text
        && widget.kind != WidgetKind::SplitPane
        && !toolbar_icon
        && !text.starts_with("desktop:")
        && (!text.starts_with("panel:")
            || theme.chrome_profile == ChromeProfile::Redmond98
                && widget.kind == WidgetKind::Dialog
                && super::layout_tokens::side_sheet_title(Some(text)).is_some())
        && !text.starts_with("split:")
        && !text.starts_with("theme:")
        && super::layout_tokens::positioned_canvas_height(Some(text)).is_none()
        && super::layout_tokens::positioned_item(Some(text)).is_none()
        && !text.starts_with("table-columns:")
        && !text.starts_with("zoom:")
        && !super::layout_tokens::is_app_grid(Some(text))
        && !super::layout_tokens::is_file_grid(Some(text))
        && widget.kind != WidgetKind::IconGrid
        && widget.kind != WidgetKind::Tabs
        && widget.kind != WidgetKind::ScrollView
        && widget.kind != WidgetKind::Slider
        && widget.kind != WidgetKind::ScrollBar
    {
        let (text_origin, text_size, align) = if desktop_item {
            let icon_height = 48_u32.min(inset.size.height);
            // The copied classic rasters retain a transparent 48 px resource
            // box around their visible artwork. Overlap the transparent tail
            // only; never pull the caption through the icon's resource box.
            let caption_top = if theme.chrome_profile == ChromeProfile::Redmond98 {
                icon_height.saturating_sub(6)
            } else {
                icon_height.saturating_add(4)
            };
            let available_height = inset.size.height.saturating_sub(caption_top);
            let caption_height = if theme.chrome_profile == ChromeProfile::Redmond98 {
                u32::from(theme.typography.body_px)
                    .saturating_add(4)
                    .min(available_height)
            } else {
                available_height
            };
            (
                Point {
                    x: inset.origin.x,
                    y: inset.origin.y.saturating_add_unsigned(caption_top),
                },
                Size {
                    width: inset.size.width,
                    height: caption_height,
                },
                super::TextAlign::Center,
            )
        } else {
            text_geometry(widget, inset, theme, content_x, parent_kind, file_grid_item)
        };
        let rendered_text = if theme.chrome_profile == ChromeProfile::Redmond98
            && widget.kind == WidgetKind::Dialog
            && let Some(title) = super::layout_tokens::side_sheet_title(Some(text))
        {
            title.to_owned()
        } else if widget.kind == WidgetKind::PasswordField {
            "•".repeat(text.chars().count())
        } else {
            text.clone()
        };
        let text_selection = text_selection.and_then(|(start, end)| {
            rendered_selection_range(text, &rendered_text, widget.kind, start, end)
        });
        let multiline_field=chat_composer && text.contains('\n');
        let single_line = matches!(widget.kind,WidgetKind::TextField|WidgetKind::PasswordField)&&!multiline_field;
        let selection_rect = text_selection.filter(|_| !single_line && !multiline_field).map(|(start, end)| {
            let selection_height = u32::from(semantic::text_size(widget, theme))
                .saturating_add(if single_line { 2 } else { 4 })
                .min(text_size.height);
            let prefix_width = super::layout::estimated_text_width(&rendered_text[..start], theme)
                .min(text_size.width);
            let selected_width =
                super::layout::estimated_text_width(&rendered_text[start..end], theme)
                    .min(text_size.width.saturating_sub(prefix_width));
            Rect {
                origin: Point {
                    x: text_origin.x.saturating_add_unsigned(prefix_width),
                    y: text_origin.y.saturating_add_unsigned(
                        text_size.height.saturating_sub(selection_height) / 2,
                    ),
                },
                size: Size {
                    width: selected_width,
                    height: selection_height,
                },
            }
        });
        if let Some(selection) = selection_rect.filter(|rect| rect.size.width > 0) {
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: selection,
                    color: if single_line {
                        text_selection_color(theme)
                    } else {
                        theme.palette.selection
                    },
                    radius: 0,
                },
            )?;
        }
        let text_rect = Rect {
            origin: text_origin,
            size: text_size,
        };
        let redmond_icon_selection = theme.chrome_profile == ChromeProfile::Redmond98
            && ((widget.kind == WidgetKind::Button && (desktop_item || file_grid_item))
                || widget.kind == WidgetKind::TreeItem)
            && visual.selected();
        if redmond_icon_selection {
            let label_width = super::layout::estimated_classic_text_width(
                &rendered_text,
                semantic::text_size(widget, theme),
            )
            .saturating_add(2)
            .min(text_rect.size.width);
            let label_height = u32::from(semantic::text_size(widget, theme))
                .saturating_add(4)
                .min(text_rect.size.height);
            let label_x = if align == super::TextAlign::Center {
                text_rect
                    .origin
                    .x
                    .saturating_add_unsigned(text_rect.size.width.saturating_sub(label_width) / 2)
            } else {
                text_rect.origin.x
            };
            let label_rect = Rect {
                origin: Point {
                    x: label_x,
                    y: text_rect.origin.y.saturating_add_unsigned(
                        text_rect.size.height.saturating_sub(label_height) / 2,
                    ),
                },
                size: Size {
                    width: label_width,
                    height: label_height,
                },
            };
            push_command(
                commands,
                DrawCommand::Fill {
                    rect: label_rect,
                    color: theme.palette.selection,
                    radius: 0,
                },
            )?;
        }
        if let Some(font) = document_font {
            if widget.kind == WidgetKind::TextEditor {
                let parsed = rice_ui::EditorText::parse(text);
                let caret = focused.then(|| parsed.and_then(|value| value.caret).unwrap_or_default() as u32);
                let selection = parsed.and_then(|value| value.anchor.zip(value.caret)).map(|(anchor, caret)| (anchor.min(caret) as u32, anchor.max(caret) as u32));
                let color = document_paint.and_then(|paint| paint.foreground.as_deref()).map_or(theme.palette.text, grid::paint_color);
                return push_command(commands, DrawCommand::StyledEdit { zoom_percent: 100, rect: text_rect, text: rendered_text, color, font: font.clone(), caret, selection, selection_color: text_selection_color(theme), vertical: 1 });
            }
            let caret = (focused && widget.kind == WidgetKind::TextField).then(|| u32::try_from(text[..super::clamp_text_boundary(text, text_caret.unwrap_or(text.len()))].chars().count()).unwrap_or(u32::MAX));
            let selection = text_selection.map(|(start,end)| (rendered_text[..start].chars().count() as u32, rendered_text[..end].chars().count() as u32));
            let color=document_paint.map_or(theme.palette.text,|paint|paint.foreground.as_deref().map(grid::paint_color).unwrap_or_else(||grid::automatic_ink(grid::paint_color(&paint.background),rice_ui::Color::opaque(0,0,0))));
            let vertical=match document_paint.and_then(|paint|paint.vertical.as_deref()) {
                Some("top")=>1, Some("bottom")=>2, _=>0,
            };
            return push_command(commands, DrawCommand::StyledEdit { zoom_percent:document_zoom, rect: text_rect, text: rendered_text, color, font: font.clone(), caret, selection, selection_color: text_selection_color(theme), vertical });
        }
        if single_line && let Some((start, end)) = text_selection {
            push_command(
                commands,
                DrawCommand::TextSelection {
                    rect: text_rect,
                    text: rendered_text.clone(),
                    color: text_selection_color(theme),
                    size_px: semantic::text_size(widget, theme),
                    align: semantic::text_align(widget, align),
                    start: u32::try_from(rendered_text[..start].chars().count())
                        .unwrap_or(u32::MAX),
                    end: u32::try_from(rendered_text[..end].chars().count()).unwrap_or(u32::MAX),
                },
            )?;
        }
        let caret = matches!(
            widget.kind,
            WidgetKind::TextField | WidgetKind::PasswordField | WidgetKind::TextEditor
        ) && focused;
        let caret_offset = (caret && widget.kind != WidgetKind::TextEditor).then(|| {
            let byte_offset = super::clamp_text_boundary(text, text_caret.unwrap_or(text.len()));
            u32::try_from(text[..byte_offset].chars().count()).unwrap_or(u32::MAX)
        });
        // Keep the retained TextField value plain. Its drawing uses the existing
        // editor presentation so selection direction and Unicode offsets reach
        // the shaped multiline painter without extending the command wire.
        let command_text = if multiline_field {
            let caret_byte = super::clamp_text_boundary(text, text_caret.unwrap_or(text.len()));
            let anchor_byte = text_selection.map_or(caret_byte, |(start, end)| {
                if caret_byte == start { end } else { start }
            });
            rice_ui::editor_text_selection("plain", rice_ui::EditorWrap::Word, false,
                text[..caret_byte].chars().count(), text[..anchor_byte].chars().count(), text)
        } else { rendered_text.clone() };
        push_command(
            commands,
            DrawCommand::Text {
                rect: text_rect,
                text: command_text,
                color: if forced_dark_editor(widget) {
                    FORCED_DARK_EDITOR_TEXT
                } else if tabs::is_plain_tab(widget, parent_kind)
                    && widget.state.contains(WidgetState::FLAT)
                    && (visual.selected() || widget.state.contains(WidgetState::CHECKED))
                {
                    theme.palette.accent_text
                } else if tabs::is_plain_tab(widget, parent_kind)
                    && theme.chrome_profile == ChromeProfile::Familiar
                {
                    tabs::familiar_ink(theme, visual.enabled())
                } else if redmond_icon_selection {
                    theme.palette.selection_text
                } else if redmond_calculator_key {
                    if widget.state.contains(WidgetState::DESTRUCTIVE) {
                        rice_ui::Color::opaque(255, 0, 0)
                    } else {
                        rice_ui::Color::opaque(0, 0, 255)
                    }
                } else if theme.chrome_profile == ChromeProfile::Redmond98
                    && widget.kind == WidgetKind::Menu
                    && parent_kind == Some(WidgetKind::MenuBar)
                {
                    theme.palette.text
                } else if theme.chrome_profile == ChromeProfile::Redmond98
                    && widget.kind == WidgetKind::Dialog
                {
                    rice_ui::Color::opaque(255, 255, 255)
                } else if theme.chrome_profile == ChromeProfile::Redmond98
                    && inherited_content_background == Some(theme.palette.selection)
                {
                    // A selected list/detail row owns one navy plane across
                    // every descendant cell. All of its text inherits the
                    // classic highlight foreground, including metadata.
                    theme.palette.selection_text
                } else if theme.chrome_profile == ChromeProfile::Redmond98
                    && widget.kind == WidgetKind::Button
                    && parent_kind == Some(WidgetKind::List)
                    && visual.selected()
                {
                    theme.palette.selection_text
                } else if theme.chrome_profile == ChromeProfile::Redmond98
                    && widget.kind == WidgetKind::Button
                    && inherited_content_background != Some(theme.palette.selection)
                {
                    // Ordinary Win98 push buttons keep black ink while being
                    // pressed. A button embedded in a selected detail row is
                    // list content instead, and must inherit white highlight
                    // ink from the row's navy selection plane.
                    theme.palette.text
                } else {
                    semantic::content_color(
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
                    )
                },
                size_px: semantic::text_size(widget, theme),
                align: semantic::text_align(widget, align),
                role: match widget.kind {
                    WidgetKind::TextField if multiline_field => TextRole::Editor,
                    WidgetKind::TextEditor => TextRole::Editor,
                    WidgetKind::Markdown => TextRole::Markdown,
                    _ => TextRole::Label,
                },
                caret,
                caret_offset,
                brand: widget.state.contains(WidgetState::BRAND_TYPE),
                gradient: widget.state.contains(WidgetState::ACCENT_GRADIENT),
            },
        )?;
        if let Some(selection) = selection_rect.filter(|rect| rect.size.width > 0) {
            push_command(commands, DrawCommand::PushClip { rect: selection })?;
            push_command(
                commands,
                DrawCommand::Text {
                    rect: text_rect,
                    text: rendered_text,
                    color: if forced_dark_editor(widget) {
                        FORCED_DARK_EDITOR_TEXT
                    } else {
                        semantic::content_color(
                            widget,
                            theme,
                            visual.enabled(),
                            Some(semantic::composite_over(
                                theme.palette.selection,
                                theme.palette.surface_raised,
                            )),
                            command_deck_key,
                        )
                    },
                    size_px: semantic::text_size(widget, theme),
                    align: semantic::text_align(widget, align),
                    role: match widget.kind {
                        WidgetKind::TextEditor => TextRole::Editor,
                        WidgetKind::Markdown => TextRole::Markdown,
                        _ => TextRole::Label,
                    },
                    caret: false,
                    caret_offset: None,
                    brand: widget.state.contains(WidgetState::BRAND_TYPE),
                    gradient: widget.state.contains(WidgetState::ACCENT_GRADIENT),
                },
            )?;
            push_command(commands, DrawCommand::PopClip)?;
        }
    }
    Ok(())
}

fn rendered_selection_range(
    source: &str,
    rendered: &str,
    kind: WidgetKind,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let start = super::clamp_text_boundary(source, start);
    let end = super::clamp_text_boundary(source, end);
    if start >= end {
        return None;
    }
    if kind != WidgetKind::PasswordField {
        return Some((start.min(rendered.len()), end.min(rendered.len())));
    }
    let rendered_offset = |characters: usize| {
        rendered
            .char_indices()
            .nth(characters)
            .map_or(rendered.len(), |(offset, _)| offset)
    };
    Some((
        rendered_offset(source[..start].chars().count()),
        rendered_offset(source[..end].chars().count()),
    ))
}
