use std::collections::BTreeMap;

use rice_native_app_protocol::WidgetId;
use rice_ui::{
    Constraints, EditorText, Insets, MenuText, Point, Rect, RetainedTree, Size, Theme, Widget,
    WidgetKind, WidgetState,
};

use super::layout_adaptive::{
    compact_navigation_list, compact_split_lengths, is_compact_split, layout_compact_navigation,
};
use super::layout_overlay::{is_dialog, is_notification, is_tooltip};
use super::layout_partition::{
    child_flexes, partition_axis, partition_distribute, partition_flex, partition_resource_columns,
    partition_split, partition_weighted, split_spec,
};

// Canvas text is not guaranteed to use the same metrics as the Rust-side
// estimator. Keep enough room that ordinary menu headings do not elide.
const TEXT_WIDTH_PERCENT: u32 = 72;
// Canvas applies a final two-pixel inset on both sides and real WOFF2 glyph
// metrics can exceed the integer estimator. Reserve both costs here.
const CANVAS_TEXT_INSET: u32 = 12;
const MENU_HORIZONTAL_PADDING: u32 = 12;
const MENU_POPUP_PADDING: u32 = 6;
const REDMOND_MENU_HORIZONTAL_PADDING: u32 = 4;
const REDMOND_MENU_POPUP_PADDING: u32 = 3;
const REDMOND_MENU_INDICATOR_GAP: u32 = 4;
const MENU_INDICATOR_GAP: u32 = 10;
const MENU_SHORTCUT_GAP: u32 = 40;
// Icon-and-label controls need a deliberate visual gap even in compact themes
// whose base spacing unit is tuned for borders and grid gutters.
pub(super) const ICON_TEXT_GAP: u32 = 8;
/// Keep ordinary application and context menus fully expanded. Scrolling is a
/// last resort for genuinely long command lists; showing a scrollbar beside a
/// routine context menu is visually noisy and obscures the available actions.
pub(super) const MAX_MENU_POPUP_ROWS: usize = 20;

// Legacy apps use a final root label for status; newer apps mark a panel.
// Decide from the current tree on every layout, so updates restore the footer.
pub(super) fn empty_status_bar(tree: &RetainedTree, id: WidgetId) -> bool {
    let Some(widget) = tree.get(id) else { return false; };
    let legacy_footer = widget.kind == WidgetKind::Label
        && widget.parent.and_then(|id| tree.get(id)).is_some_and(|parent| {
            parent.kind == WidgetKind::Column && parent.parent.is_none()
                && parent.children().len() > 1 && parent.children().last() == Some(&id)
        });
    (super::layout_tokens::is_status_bar(widget.text.as_deref()) || legacy_footer)
        && !status_has_content(tree, widget)
}

fn status_has_content(tree: &RetainedTree, widget: &Widget) -> bool {
    match widget.kind {
        WidgetKind::Label | WidgetKind::Markdown => {
            if widget.text.as_deref().is_some_and(|text| !text.trim().is_empty()) {
                return true;
            }
        }
        WidgetKind::Container | WidgetKind::Row | WidgetKind::Column
        | WidgetKind::Stack | WidgetKind::SplitPane => {}
        // Progress and controls remain useful even without a text caption.
        WidgetKind::Image => {}
        _ => return true,
    }
    widget.children().iter().any(|id| tree.get(*id).is_some_and(|child| status_has_content(tree, child)))
}

fn is_menu_command(tree: &RetainedTree, child: WidgetId) -> bool {
    tree.get(child)
        .and_then(|widget| widget.text.as_deref())
        .is_none_or(|text| !MenuText::parse(text).separator)
}

pub(super) fn menu_has_leading_column(tree: &RetainedTree, menu: &Widget) -> bool {
    // Leading indicators form one column owned by the popup, not independent
    // per-row indents. Plain rows therefore align with icon-bearing siblings,
    // while an entirely plain menu does not carry an empty gutter.
    menu.kind == WidgetKind::Menu
        && menu.children().iter().any(|child| {
            tree.get(*child).is_some_and(|child| {
                child.icon.is_some()
                    || matches!(child.kind, WidgetKind::CheckBox | WidgetKind::RadioButton)
            })
        })
}

/// Passive hover metadata is never a popup row or a scroll position.
pub(super) fn menu_content_children(tree: &RetainedTree, menu: &Widget) -> Vec<WidgetId> {
    menu.children().iter().copied().filter(|id| {
        tree.get(*id).is_some_and(|child| child.kind != WidgetKind::Tooltip)
    }).collect()
}

pub(super) fn maximum_menu_scroll_row(tree: &RetainedTree, menu: WidgetId) -> usize {
    let Some(menu) = tree.get(menu) else {
        return 0;
    };
    let children = menu_content_children(tree, menu);
    let command_count = children
        .iter()
        .filter(|child| is_menu_command(tree, **child))
        .count();
    let commands_to_skip = command_count.saturating_sub(MAX_MENU_POPUP_ROWS);
    if commands_to_skip == 0 {
        return 0;
    }
    children.iter()
        .enumerate()
        .filter(|(_, child)| is_menu_command(tree, **child))
        .nth(commands_to_skip)
        .map_or(0, |(index, _)| index)
}

fn stacked_form(widget: &Widget, width: u32) -> bool {
    widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "form-field")) && width < 460
}

fn lays_out_children_horizontally(widget: &Widget) -> bool {
    // `panel:distribute` shares spare space along a widget's natural axis.
    // Composite controls have historically used it to opt into a row, but an
    // explicit Column must remain vertical.
    matches!(
        widget.kind,
        WidgetKind::Row
            | WidgetKind::MenuBar
            | WidgetKind::ToolBar
            | WidgetKind::Breadcrumb
    ) || (widget.kind == WidgetKind::SplitPane
        && !widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "vertical"))) || (widget.kind == WidgetKind::Tabs
        && !super::layout_tokens::is_vertical_tabs(widget.text.as_deref()))
        || (widget.kind != WidgetKind::Column
            && super::layout_tokens::distributes_children(widget.text.as_deref()))
}

pub(super) fn root_size(tree: &RetainedTree, constraints: Constraints, theme: &Theme) -> Size {
    let gap = u32::from(theme.spacing.unit);
    let mut natural = Size::default();
    for (index, root) in tree.roots().iter().enumerate() {
        let preferred = preferred_size(tree, *root, theme);
        natural.width = natural.width.max(preferred.width);
        natural.height = natural.height.saturating_add(preferred.height);
        if index != 0 {
            natural.height = natural.height.saturating_add(gap);
        }
    }
    constraints.constrain(natural)
}

pub(super) fn preferred_size(tree: &RetainedTree, id: WidgetId, theme: &Theme) -> Size {
    let menu_theme;
    let theme = if theme.metrics.menu_row_height > 0 && tree.get(id).is_some_and(|widget|widget.kind==WidgetKind::MenuItem) {
        menu_theme = {let mut t=*theme;t.metrics.control_height=t.metrics.menu_row_height;t}; &menu_theme
    } else {theme};

    if let Some(size) = super::layout_measurements::size(tree, theme, id) { return size; }
    let size = measure_preferred_size(tree, id, theme);
    super::layout_measurements::save_size(tree, theme, id, size);
    size
}

fn measure_preferred_size(tree: &RetainedTree, id: WidgetId, theme: &Theme) -> Size {
    if empty_status_bar(tree, id) { return Size::default(); }
    let Some(widget) = tree.get(id) else {
        return Size::default();
    };
    let color_widget = super::render::color_control::normalized(widget);
    let widget = color_widget.as_ref().unwrap_or(widget);
    if widget.kind == WidgetKind::Stack && (super::layout_tokens::positioned_overlay(widget.text.as_deref()) || super::scene_surface::overlay(widget.text.as_deref()).is_some()) {
        return Size::default();
    }
    if widget.text.as_deref() == Some("panel:settings-section-gap") {
        return Size { width: 0, height: 8 };
    }
    if widget.kind == WidgetKind::TerminalRow {
        return Size {
            width: 0,
            height: widget
                .text
                .as_deref()
                .and_then(|s| rice_ui::terminal::TerminalRow::parse(s).ok())
                .map_or(18, |row| {
                    if row.font_size == 0 {
                        18
                    } else {
                        u32::from(row.font_size) + 5
                    }
                }),
        };
    }
    if matches!(widget.kind, WidgetKind::TerminalGrid | WidgetKind::VirtualGrid) {
        return Size {
            width: 800,
            height: 432,
        };
    }
    if widget.kind == WidgetKind::MenuBar && widget.text.as_deref() == Some("context") {
        return Size::default();
    }
    if widget.kind == WidgetKind::ToolBar
        && theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
        && super::layout_tokens::is_redmond98_files_toolbar(widget.text.as_deref())
    {
        return Size {
            width: 0,
            height: 36,
        };
    }
    if super::layout_tokens::is_redmond98_webview_rule(widget.text.as_deref()) {
        return Size {
            width: 0,
            height: 2,
        };
    }
    if widget.kind == WidgetKind::Row && widget.text.as_deref() == Some("panel:separator") {
        return Size {
            width: 0,
            height: u32::from(theme.spacing.section).max(8),
        };
    }
    if widget.kind == WidgetKind::Row && widget.text.as_deref() == Some("panel:tight-separator") {
        return Size {
            width: 0,
            height: 8,
        };
    }
    if widget.kind == WidgetKind::Label && widget.text.as_deref() == Some("panel:section-gap") {
        return Size {
            width: 0,
            height: u32::from(theme.spacing.section),
        };
    }
    if widget.kind == WidgetKind::Label
        && widget.text.as_deref() == Some("panel:settings-section-gap")
    {
        return Size {
            width: 0,
            height: u32::from(theme.spacing.section).saturating_mul(2),
        };
    }
    if widget.kind == WidgetKind::Label && widget.text.as_deref() == Some("panel:hero-wordmark-gap")
    {
        return Size {
            width: 0,
            height: 16,
        };
    }
    if widget.kind == WidgetKind::Label && widget.text.as_deref() == Some("panel:hero-copy-gap") {
        return Size {
            width: 0,
            height: 4,
        };
    }
    if widget.kind == WidgetKind::Label && widget.text.as_deref() == Some("panel:document-gap") {
        return Size {
            width: 0,
            height: u32::from(theme.spacing.section).saturating_mul(3),
        };
    }
    if widget.kind == WidgetKind::Label
        && widget.text.as_deref().is_none_or(str::is_empty)
        && widget.icon.is_none()
    {
        return Size::default();
    }
    let gap = u32::from(theme.spacing.unit);
    // Dialog overlays reserve panel insets on both axes. Account for those
    // exact insets in preferred size; ordinary containers intentionally keep
    // their compact control padding so flexible app surfaces retain space.
    let panel_chrome = matches!(widget.kind, WidgetKind::Dialog | WidgetKind::GroupBox);
    let settings_page = super::layout_tokens::is_settings_page(widget.text.as_deref());
    let settings_card = super::layout_tokens::is_settings_card(widget.text.as_deref());
    let settings_shell = super::layout_tokens::is_settings_shell(widget.text.as_deref());
    let browser_tab_strip = super::layout_tokens::is_browser_tab_strip(widget.text.as_deref());
    let integrated_tabs = widget.kind == WidgetKind::Tabs
        && super::layout_tokens::is_integrated_tabs(widget.text.as_deref());
    let bookmark_bar = super::layout_tokens::is_bookmark_bar(widget.text.as_deref());
    let bookmark_member = widget
        .parent
        .and_then(|parent| tree.get(parent))
        .is_some_and(|parent| super::layout_tokens::is_bookmark_composite(parent.text.as_deref()));
    let office_group = super::layout_tokens::has_token(widget.text.as_deref(), "office-ribbon-group");
    let padding_x = if let Some(padding) = super::layout_tokens::padding(widget.text.as_deref()) {
        padding * 2
    } else if office_group {
        20
    } else if integrated_tabs || widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "flush")) {
        0
    } else if browser_tab_strip {
        u32::from(theme.spacing.control_y).saturating_mul(2)
    } else if bookmark_bar {
        8
    } else if settings_page {
        u32::from(theme.spacing.section).saturating_mul(2)
    } else if settings_card {
        u32::from(theme.spacing.control_y).saturating_mul(2)
    } else if settings_shell {
        u32::from(theme.spacing.control_y).saturating_mul(2)
    } else {
        u32::from(if panel_chrome {
            theme.spacing.panel
        } else {
            theme.spacing.control_x
        })
        .saturating_mul(2)
    };
    let padding_y = if let Some(padding) = super::layout_tokens::padding(widget.text.as_deref()) {
        padding * 2
    } else if office_group {
        12
    } else if integrated_tabs || widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "flush")) {
        0
    } else if browser_tab_strip {
        u32::from(theme.spacing.control_y)
    } else if bookmark_bar {
        4
    } else if settings_page {
        u32::from(theme.spacing.section).saturating_mul(2)
    } else if settings_card {
        u32::from(theme.spacing.control_y).saturating_mul(2)
    } else if settings_shell {
        u32::from(theme.spacing.control_y).saturating_mul(2)
    } else {
        u32::from(if panel_chrome {
            theme.spacing.panel
        } else {
            theme.spacing.control_y
        })
        .saturating_mul(2)
    };
    let toolbar_icon = widget
        .parent
        .and_then(|parent| tree.get(parent))
        .is_some_and(|parent| {
            parent.kind == WidgetKind::ToolBar
                && matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
                && !widget.state.contains(rice_ui::WidgetState::START_ALIGNED)
        });
    let compact_label = is_compact_label(tree, widget);
    let desktop_token = widget.parent.is_none()
        && widget
            .text
            .as_deref()
            .is_some_and(|text| text.starts_with("desktop:"));
    let fixed_panel = widget.kind == WidgetKind::Container
        && widget.text.as_deref().is_some_and(|text| {
            super::layout_tokens::is_fixed_panel(Some(text))
                || text.starts_with("panel:fixed-height:")
        });
    if let Some(height) = super::layout_tokens::positioned_canvas_height(widget.text.as_deref()) {
        return Size { width:620, height };
    }
    if widget.text.as_deref().is_some_and(|text| text.starts_with("drag-handle:")) {
        return Size { width: 28, height: u32::from(theme.metrics.control_height) };
    }
    if widget.text.as_deref().is_some_and(|text| text.starts_with("theme-preview:")) {
        return Size { width: 600, height: 172 };
    }
    if matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
        && widget.text.as_deref().is_some_and(|text| text.starts_with("control-tile:")) {
        return Size { width: 176, height: 68 };
    }
    if widget.text.as_deref().is_some_and(|text| text.starts_with("mail-avatar:")) {
        return Size { width: 64, height: 64 };
    }
    if widget.kind == WidgetKind::Button && widget.text.as_deref().is_some_and(|text| text.starts_with("mail-summary:")) {
        return Size { width: 320, height: 92 };
    }
    if toolbar_icon && widget.state.contains(WidgetState::LARGE_ICON) {
        let height = widget.parent.and_then(|parent| tree.get(parent))
            .and_then(|parent| super::layout_tokens::fixed_height(parent.text.as_deref())).unwrap_or(60);
        return Size { width: estimated_text_width(widget.text.as_deref().unwrap_or(""), theme).saturating_add(20).max(64), height };
    }
    if toolbar_icon && widget.state.contains(WidgetState::DISPLAY) {
        let height = widget.parent.and_then(|parent| tree.get(parent))
            .and_then(|parent| super::layout_tokens::fixed_height(parent.text.as_deref()));
        let edge = height.unwrap_or(44);
        let dropdown = height.is_some() && widget.kind == WidgetKind::Menu
            && widget.icon.is_some_and(|icon| !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown));
        return Size { width: edge.saturating_add(if dropdown { 12 } else { 0 }), height: edge };
    }
    let color_swatch = matches!(widget.kind, WidgetKind::Container | WidgetKind::Button | WidgetKind::MenuItem)
        && widget
            .text
            .as_deref()
            .is_some_and(|text| text.starts_with("color-swatch:#"));
    let color_picker = widget.kind == WidgetKind::ScrollBar
        && widget.text.as_deref().is_some_and(|text| {
            text.starts_with("color-picker:#")
                || text.starts_with("color-plane:#")
                || text.starts_with("color-hue:#")
        });
    let layout_token = widget.text.as_deref().is_some_and(|text| {
        text.starts_with("split:")
            || text.starts_with("theme:")
            || text.starts_with("panel:")
            || text.starts_with("table-columns:")
            || text.starts_with("graph:")
            || text.starts_with("inline-image:")
            || super::layout_tokens::is_file_grid(Some(text))
            || super::layout_tokens::is_app_grid(Some(text))
    });
    let performance_graph = widget.kind == WidgetKind::Container
        && widget
            .text
            .as_deref()
            .is_some_and(|text| text.starts_with("graph:"));
    let menu_parent = widget
        .parent
        .and_then(|parent| tree.get(parent))
        .filter(|parent| parent.kind == WidgetKind::Menu);
    let menu_child = menu_parent.is_some();
    let menu_text = widget.text.as_deref().map(MenuText::parse);
    if menu_child && menu_text.is_some_and(|text| text.separator) {
        return Size {
            width: u32::from(theme.metrics.control_height).saturating_mul(10),
            height: u32::from(theme.spacing.section).max(8),
        };
    }
    let action_width_hint = widget
        .parent
        .and_then(|parent| tree.get(parent))
        .and_then(|parent| match widget.kind {
            WidgetKind::Label => {
                super::layout_tokens::action_width_hint(parent.text.as_deref(), "label-width=")
            }
            WidgetKind::Button => {
                super::layout_tokens::action_width_hint(parent.text.as_deref(), "choice-width=")
            }
            _ => None,
        });
    let text_width = if toolbar_icon
        || desktop_token
        || fixed_panel
        || layout_token
        || color_swatch
        || color_picker
    {
        0
    } else if menu_child {
        menu_text.map_or(0, |text| {
            estimated_text_width(text.label, theme)
                .saturating_add(
                    text.shortcut
                        .map_or(0, |shortcut| estimated_text_width(shortcut, theme)),
                )
                .saturating_add(text.shortcut.map_or(0, |_| MENU_SHORTCUT_GAP))
        })
    } else {
        widget
            .text
            .as_deref()
            .map_or(0, |text| estimated_widget_text_width(widget, text, theme))
            .max(
                action_width_hint
                    .map_or(0, |text| estimated_widget_text_width(widget, text, theme)),
            )
            .saturating_add(CANVAS_TEXT_INSET)
    };
    let redmond_menu_child =
        menu_child && theme.chrome_profile == rice_ui::ChromeProfile::Redmond98;
    let menu_leading_gutter =
        u32::from(menu_parent.is_some_and(|parent| menu_has_leading_column(tree, parent)))
            .saturating_mul(u32::from(theme.metrics.icon_small).saturating_add(
                if redmond_menu_child {
                    REDMOND_MENU_INDICATOR_GAP
                } else {
                    MENU_INDICATOR_GAP
                },
            ));
    let choice_width = u32::from(
        !menu_child && matches!(widget.kind, WidgetKind::CheckBox | WidgetKind::RadioButton),
    )
    .saturating_mul(u32::from(theme.metrics.icon_small).saturating_add(gap));
    let toggle_width = u32::from(
        widget.kind == WidgetKind::Toggle
            && widget.text.as_deref().is_some_and(|text| !text.is_empty()),
    )
    .saturating_mul(36_u32.saturating_add(gap));
    let large_image_extent = u32::from(theme.metrics.icon_medium)
        .saturating_mul(4)
        .saturating_mul(super::layout_tokens::zoom_permille(widget.text.as_deref()))
        / 1_000;
    let large_image =
        widget.kind == WidgetKind::Image && widget.state.contains(rice_ui::WidgetState::LARGE_ICON);
    let display_image_extent = u32::from(theme.metrics.icon_large);
    let display_image =
        widget.kind == WidgetKind::Image && widget.state.contains(rice_ui::WidgetState::DISPLAY);
    let brand_mark = widget.state.contains(rice_ui::WidgetState::BRAND_TYPE)
        && widget.state.contains(rice_ui::WidgetState::DISPLAY)
        && widget.icon.is_some();
    let icon_width = widget
        .icon
        .filter(|_| !menu_child)
        .map_or(0, |_| {
            if large_image {
                large_image_extent
            } else if brand_mark {
                display_image_extent
                    .saturating_add(u32::from(theme.spacing.section).saturating_mul(4))
                    .saturating_add(gap)
            } else if display_image {
                display_image_extent
            } else if widget.parent.and_then(|id| tree.get(id)).is_some_and(|p| p.kind == WidgetKind::ToolBar)
                && matches!(widget.kind, WidgetKind::Button | WidgetKind::Menu)
                && widget.state.contains(WidgetState::START_ALIGNED) {
                u32::from(theme.metrics.icon_medium).saturating_add(gap.max(ICON_TEXT_GAP))
                    .saturating_add(if widget.kind == WidgetKind::Menu && widget.icon.is_some_and(|icon| !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown)) {12} else {0})
            } else {
                u32::from(theme.metrics.icon_small).saturating_add(gap.max(ICON_TEXT_GAP))
            }
        })
        .saturating_add(choice_width)
        .saturating_add(
            if widget.kind == WidgetKind::TreeItem
                && (widget.state.contains(WidgetState::INDETERMINATE)
                    || widget.children().iter().any(|child| {
                        tree.get(*child)
                            .is_some_and(|child| child.kind == WidgetKind::TreeItem)
                    }))
            {
                u32::from(theme.metrics.icon_small).saturating_add(gap)
            } else {
                0
            },
        );
    let text_lines = if widget.kind == WidgetKind::TextEditor {
        widget
            .text
            .as_deref()
            .and_then(EditorText::parse)
            .map_or(1, |editor| {
                u32::try_from(editor.document.lines().count())
                    .unwrap_or(u32::MAX)
                    .max(1)
            })
    } else if is_chat_composer(tree, widget) && widget.text.as_deref().is_some_and(|text|text.contains('\n')) {
        widget.text.as_deref().map_or(1,|text|(text.bytes().filter(|byte|*byte==b'\n').count()+1).min(5) as u32)
    } else if matches!(
        widget.kind,
        WidgetKind::Label | WidgetKind::Markdown | WidgetKind::Tooltip
    ) {
        widget.text.as_deref().map_or(1, |text| {
            u32::try_from(text.lines().count())
                .unwrap_or(u32::MAX)
                .max(1)
        })
    } else {
        1
    };
    let mut leaf = Size {
        width: text_width
            .saturating_add(icon_width)
            .saturating_add(menu_leading_gutter)
            .saturating_add(if menu_child {
                if redmond_menu_child {
                    REDMOND_MENU_HORIZONTAL_PADDING.saturating_mul(2)
                } else {
                    MENU_HORIZONTAL_PADDING.saturating_mul(2)
                }
            } else {
                padding_x
            })
            .saturating_add(toggle_width)
            .max(if color_swatch {
                u32::from(theme.metrics.control_height).saturating_mul(2)
            } else {
                u32::from(theme.metrics.control_height)
            })
            .max(if widget.kind == WidgetKind::Slider {
                160
            } else {
                0
            }),
        height: if large_image {
            large_image_extent
        } else if brand_mark {
            display_image_extent.saturating_add(u32::from(theme.spacing.section).saturating_mul(4))
        } else if display_image {
            display_image_extent
        } else if performance_graph {
            128
        } else if color_picker {
            if widget
                .text
                .as_deref()
                .is_some_and(|s| s.starts_with("color-hue:"))
            {
                24
            } else if widget
                .text
                .as_deref()
                .is_some_and(|s| s.starts_with("color-plane:"))
            {
                160
            } else {
                112
            }
        } else if menu_child {
            if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
                // Native Win98 menus use compact 20px command bands.  The
                // ordinary theme metric deliberately remains untouched.
                20
            } else {
                u32::from(theme.metrics.control_height).saturating_add(gap.saturating_mul(2))
            }
        } else if widget.kind == WidgetKind::Markdown {
            let size = u32::from(super::layout_tokens::preferred_text_size(widget, theme));
            let line_height = (size * 135 + 50) / 100;
            line_height.max(10).saturating_mul(text_lines).saturating_add(padding_y + 4)
        } else if is_chat_composer(tree, widget) && text_lines > 1 {
            // The scoped editor painter consumes a four-pixel inset on each
            // edge and uses ceil(font * 1.35) line boxes. Reserve those in
            // addition to the native field insets so its last line is visible.
            let size = u32::from(super::layout_tokens::preferred_text_size(widget, theme)).max(8);
            let line_height = ((size * 135 + 99) / 100).max(size + 2);
            u32::from(theme.metrics.control_height).max(
                line_height.saturating_mul(text_lines).saturating_add(8).saturating_add(padding_y)
            )
        } else if compact_label {
            let line_height = if is_canvas_multiline_label(tree, widget) {
                compact_label_line_height(widget, theme, text_lines)
            } else {
                super::layout_tokens::preferred_line_height(widget, theme)
            };
            line_height.saturating_mul(text_lines)
        } else {
            u32::from(theme.metrics.control_height).max(
                super::layout_tokens::preferred_line_height(widget, theme)
                    .saturating_mul(text_lines)
                    .saturating_add(padding_y),
            )
        },
    };
    if widget.kind == WidgetKind::ProgressBar
        && super::render::is_slim_progress(widget.text.as_deref())
    {
        leaf.height = 6;
    }
    if toolbar_icon {
        let height = widget.parent.and_then(|parent| tree.get(parent))
            .and_then(|parent| super::layout_tokens::fixed_height(parent.text.as_deref()))
            .unwrap_or(u32::from(theme.metrics.control_height));
        leaf.width = height;
        leaf.height = height;
        if widget.kind == WidgetKind::Menu && widget.icon.is_some_and(|icon| !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown)) {
            leaf.width = leaf.width.saturating_add(12);
        }
    }
    if bookmark_member {
        let compact_height = 22;
        if matches!(widget.kind, WidgetKind::Image | WidgetKind::Label) {
            leaf.width = 16;
            leaf.height = 16;
        } else {
            leaf.height = compact_height;
        }
    }
    if super::layout_tokens::is_window_control(widget.text.as_deref()) {
        leaf.width = 28;
        leaf.height = 28;
    }
    if super::layout_tokens::is_window_drag_region(widget.text.as_deref()) {
        leaf.width = 0;
        leaf.height = u32::from(theme.metrics.control_height);
    }
    if let Some(height) = super::layout_tokens::fixed_height(widget.text.as_deref()) {
        leaf.height = height;
    }
    if let Some(width) = super::layout_tokens::fixed_width(widget.text.as_deref()) {
        leaf.width = width;
    }
    if widget.kind == WidgetKind::Tree {
        let rows = visible_tree_rows(tree, id);
        let indent = if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
            u32::from(theme.metrics.icon_small)
        } else {
            u32::from(theme.metrics.icon_small).saturating_add(u32::from(theme.spacing.unit))
        };
        let width = rows
            .iter()
            .map(|(item, depth)| {
                preferred_size(tree, *item, theme)
                    .width
                    .saturating_add(indent.saturating_mul(u32::from(*depth)))
            })
            .max()
            .unwrap_or_default()
            .saturating_add(padding_x);
        let height = u32::try_from(rows.len())
            .unwrap_or(u32::MAX)
            .saturating_mul(u32::from(theme.metrics.control_height))
            .saturating_add(padding_y);
        return Size {
            width: super::layout_tokens::fixed_width(widget.text.as_deref()).unwrap_or(width),
            height,
        };
    }
    if widget.children().iter().all(|child| is_tooltip(tree, *child))
        || matches!(
            widget.kind,
            WidgetKind::Menu | WidgetKind::TreeItem
        ) || (widget.kind == WidgetKind::MenuItem && !inline_menu_content(tree, widget))
    {
        return leaf;
    }
    let children = widget
        .children()
        .iter()
        .filter(|child| {
            !tree.get(**child).is_some_and(|child| child.kind == WidgetKind::Menu && widget.kind == WidgetKind::MenuItem)
                && !empty_status_bar(tree, **child) && !is_dialog(tree, **child)
                && !is_notification(tree, **child)
                && !is_tooltip(tree, **child)
        })
        .map(|child| preferred_size(tree, *child, theme))
        .collect::<Vec<_>>();
    if widget.text.as_deref().is_none_or(str::is_empty)
        && widget.icon.is_none()
        && children.iter().all(|size| *size == Size::default())
        && matches!(
            widget.kind,
            WidgetKind::Container | WidgetKind::Row | WidgetKind::Column | WidgetKind::Stack
        )
    {
        return Size::default();
    }
    let container_gap = if widget.kind == WidgetKind::MenuBar
        || (widget.kind == WidgetKind::Tabs
            && theme.chrome_profile == rice_ui::ChromeProfile::Redmond98)
    {
        0
    } else {
        super::layout_tokens::child_gap(widget.text.as_deref(), gap)
    };
    let horizontal_children = lays_out_children_horizontally(widget);
    let inner = if horizontal_children {
        Size {
            width: children
                .iter()
                .map(|size| size.width)
                .fold(0_u32, u32::saturating_add)
                .saturating_add(container_gap.saturating_mul(
                    u32::try_from(children.len().saturating_sub(1)).unwrap_or(u32::MAX),
                )),
            height: children.iter().map(|size| size.height).max().unwrap_or(0),
        }
    } else if widget.kind == WidgetKind::Stack {
        Size {
            width: children.iter().map(|size| size.width).max().unwrap_or(0),
            height: children.iter().map(|size| size.height).max().unwrap_or(0),
        }
    } else {
        Size {
            width: children.iter().map(|size| size.width).max().unwrap_or(0),
            height: children
                .iter()
                .map(|size| size.height)
                .fold(0_u32, u32::saturating_add)
                .saturating_add(container_gap.saturating_mul(
                    u32::try_from(children.len().saturating_sub(1)).unwrap_or(u32::MAX),
                )),
        }
    };
    if color_swatch && widget.kind == WidgetKind::Container {
        let padding = u32::from(theme.spacing.panel).saturating_mul(2);
        return Size {
            width: leaf.width.max(inner.width.saturating_add(padding)),
            height: inner.height.saturating_add(padding),
        };
    }
    if widget.kind == WidgetKind::Accordion {
        let panel_padding = u32::from(theme.spacing.panel).saturating_mul(2);
        let width = leaf.width.max(inner.width.saturating_add(panel_padding));
        if !widget.state.contains(rice_ui::WidgetState::CHECKED) {
            return Size {
                width,
                height: leaf.height,
            };
        }
        return Size {
            width,
            height: leaf
                .height
                .saturating_add(gap)
                .saturating_add(inner.height)
                .saturating_add(panel_padding),
        };
    }
    if matches!(widget.kind, WidgetKind::GroupBox | WidgetKind::Popover)
        && widget.text.as_deref().is_some_and(|text| !text.is_empty() && !text.starts_with("panel:"))
    {
        let header = u32::from(theme.metrics.control_height);
        return Size {
            width: leaf.width.max(inner.width.saturating_add(padding_x)),
            height: header
                .saturating_add(u32::from(theme.spacing.unit))
                .saturating_add(inner.height)
                .saturating_add(padding_y),
        };
    }
    let mut size = Size {
        width: leaf.width.max(inner.width.saturating_add(padding_x)),
        height: leaf.height.max(inner.height.saturating_add(padding_y)),
    };
    if super::layout_tokens::is_navigation_row(widget.text.as_deref()) {
        size.height = 56;
    }
    if widget.kind == WidgetKind::Row
        && let Some(height) = super::layout_tokens::dense_row_height(widget.text.as_deref())
    {
        size.height = height;
    }
    if super::layout_tokens::is_status_bar(widget.text.as_deref()) {
        size.height = u32::from(theme.metrics.control_height);
    }
    if integrated_tabs {
        // A classic integrated strip is exactly one control row. Generic
        // control padding otherwise leaves a detached grey band below it.
        size.height = u32::from(theme.metrics.control_height);
    }
    if super::layout_tokens::is_spin_control(widget.text.as_deref()) {
        size.height = u32::from(theme.metrics.control_height).max(36);
    }
    if super::layout_tokens::is_tab_composite(widget.text.as_deref()) {
        size.height = u32::from(theme.metrics.control_height);
    }
    if super::layout_tokens::is_bookmark_composite(widget.text.as_deref()) {
        size.height = 22;
    }
    if super::layout_tokens::is_bookmark_bar(widget.text.as_deref()) {
        size.height = 26;
    }
    if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_form_control_row(widget.text.as_deref())
    {
        size.height = size.height.max(
            u32::from(theme.metrics.control_height)
                .saturating_add(u32::from(theme.spacing.section))
                .saturating_add(u32::from(theme.spacing.unit)),
        );
    }
    if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_filter_strip(widget.text.as_deref())
    {
        size.height = size.height.max(
            u32::from(theme.metrics.control_height)
                .saturating_add(u32::from(theme.spacing.section).saturating_mul(2)),
        );
    }
    if widget.kind == WidgetKind::Dialog
        && (widget
            .text
            .as_deref()
            .is_some_and(|text| !text.starts_with("panel:"))
            || theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
                && super::layout_tokens::is_side_sheet(widget.text.as_deref()))
    {
        size.height = size
            .height
            .saturating_add(u32::from(theme.metrics.control_height))
            .saturating_add(u32::from(theme.spacing.section));
    }
    if widget.kind == WidgetKind::ScrollView
        && let Some(height) = super::layout_tokens::viewport_height(widget.text.as_deref())
    {
        size.height = height;
    }
    if let Some(height) = super::layout_tokens::fixed_height(widget.text.as_deref()) {
        size.height = height;
    }
    if let Some(width) = super::layout_tokens::fixed_width(widget.text.as_deref()) {
        size.width = width;
    }
    size
}

pub(super) fn preferred_height_for_width(tree: &RetainedTree, id: WidgetId, width: u32, theme: &Theme) -> u32 {
    let width = if tree.get(id).is_some_and(|widget| widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "reading-column"))) { width.min(760) } else { width };
    if let Some(height) = super::layout_measurements::height(tree, theme, id, width) { return height; }
    let height = measure_preferred_height_for_width(tree, id, width, theme);
    super::layout_measurements::save_height(tree, theme, id, width, height);
    height
}

fn measure_preferred_height_for_width(tree: &RetainedTree, id: WidgetId, width: u32, theme: &Theme) -> u32 {
    if empty_status_bar(tree, id) { return 0; }
    let Some(widget) = tree.get(id) else {
        return 0;
    };
    if widget.kind == WidgetKind::Stack && (super::layout_tokens::positioned_overlay(widget.text.as_deref()) || super::scene_surface::overlay(widget.text.as_deref()).is_some()) { return 0; }
    if widget.text.as_deref() == Some("panel:settings-section-gap") { return 8; }
    if let Some((height, _)) = super::layout_adaptive::toolbar_flow(tree, id, width, theme) { return height; }
    // Explicit compact heights take precedence over semantic defaults such as
    // status bars, matching preferred_size and final layout.
    if let Some(height) = super::layout_tokens::fixed_height(widget.text.as_deref()) { return height; }
    if widget.kind == WidgetKind::Row
        && let Some(height) = super::layout_tokens::dense_row_height(widget.text.as_deref())
    {
        return height;
    }
    // Composite navigation buttons are one compact, atomic row.  Measuring
    // their icon, two-line copy, and caret as a vertical document inflated
    // each material row to several control heights inside responsive pages.
    if super::layout_tokens::is_navigation_row(widget.text.as_deref()) {
        return 56;
    }
    if super::layout_tokens::is_status_bar(widget.text.as_deref()) {
        return u32::from(theme.metrics.control_height);
    }
    if super::layout_tokens::is_spin_control(widget.text.as_deref()) {
        return u32::from(theme.metrics.control_height).max(36);
    }
    if super::layout_tokens::is_tab_composite(widget.text.as_deref()) {
        return u32::from(theme.metrics.control_height);
    }
    if (widget.kind == WidgetKind::MenuBar && widget.text.as_deref() == Some("context"))
        || matches!(
            widget.kind,
            WidgetKind::Menu | WidgetKind::TreeItem
        ) || (widget.kind == WidgetKind::MenuItem && !inline_menu_content(tree, widget))
    {
        return preferred_size(tree, id, theme).height;
    }
    if widget.kind == WidgetKind::Tree {
        return preferred_size(tree, id, theme).height;
    }
    if let Some(height) = super::layout_tokens::welcome_hero_height(widget.text.as_deref(), width) {
        return height;
    }
    if let Some(height) = super::layout_tokens::fixed_height(widget.text.as_deref()) {
        return height;
    }
    if widget.kind == WidgetKind::ScrollView
        && let Some(height) = super::layout_tokens::viewport_height(widget.text.as_deref())
    {
        return height;
    }
    if widget.children().iter().all(|child| is_tooltip(tree, *child)) {
        let wrapping_control = matches!(widget.kind, WidgetKind::Button | WidgetKind::CheckBox | WidgetKind::Toggle | WidgetKind::RadioButton)
            && widget.parent.and_then(|id| tree.get(id)).is_some_and(|parent| super::layout_tokens::has_token(parent.text.as_deref(), "wrap-controls"));
        if !wrapping_control && !matches!(widget.kind, WidgetKind::Label | WidgetKind::Markdown | WidgetKind::Tooltip) {
            return preferred_size(tree, id, theme).height;
        }
        let padding_x = u32::from(theme.spacing.control_x).saturating_mul(2);
        let padding_y = u32::from(theme.spacing.control_y).saturating_mul(2);
        let icon_width = if wrapping_control && matches!(widget.kind, WidgetKind::CheckBox | WidgetKind::Toggle | WidgetKind::RadioButton) {
            u32::from(theme.metrics.icon_small) + ICON_TEXT_GAP
        } else { widget.icon.map_or(0, |_| {
            u32::from(theme.metrics.icon_small)
                .saturating_add(u32::from(theme.spacing.unit).max(ICON_TEXT_GAP))
        }) };
        let available = width
            .saturating_sub(padding_x)
            .saturating_sub(icon_width)
            .max(1);
        if widget.kind == WidgetKind::Markdown {
            // Markdown paints 1.35 line boxes inside a two-pixel inset.
            let size = u32::from(super::layout_tokens::preferred_text_size(widget, theme));
            let line_height = ((size * 135 + 50) / 100).max(10);
            let lines = widget.text.as_deref().map_or(1, |text|
                wrapped_line_count(widget, text, available.saturating_sub(4).max(1), theme));
            return line_height.saturating_mul(lines.max(1)).saturating_add(padding_y + 4);
        }
        let mut line_height = super::layout_tokens::preferred_line_height(widget, theme).max(1);
        let lines = widget
            .text
            .as_deref()
            .map_or(1, |text| wrapped_line_count(widget, text, available, theme));
        // Match the Canvas text renderer's 1.25 line box for every wrapped label.
        // Reserving only the font size clipped the last line of ordinary settings copy.
        line_height = line_height.max(compact_label_line_height(widget, theme, lines)).max(1);
        if is_compact_label(tree, widget) {
            let canvas_line = (u32::from(super::layout_tokens::preferred_text_size(widget, theme)) * 125 + 50) / 100;
            return line_height.max(canvas_line).saturating_mul(lines.max(1));
        }
        return u32::from(theme.metrics.control_height).max(
            line_height
                .saturating_mul(lines.max(1))
                .saturating_add(padding_y),
        );
    }
    if widget.kind == WidgetKind::IconGrid {
        return icon_grid_extent(tree, id, width, theme);
    }
    let panel = matches!(
        widget.kind,
        WidgetKind::Container
            | WidgetKind::GroupBox
            | WidgetKind::Dialog
            | WidgetKind::Popover
            | WidgetKind::Notification
    );
    let inset = if widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "flush")) {
        0
    } else if widget.kind == WidgetKind::Tabs {
        0
    } else if super::layout_tokens::is_settings_page(widget.text.as_deref()) {
        u32::from(theme.spacing.section)
    } else if super::layout_tokens::is_settings_card(widget.text.as_deref()) {
        u32::from(theme.spacing.control_y)
    } else if super::layout_tokens::is_settings_shell(widget.text.as_deref()) {
        u32::from(theme.spacing.control_y)
    } else {
        u32::from(if panel {
            theme.spacing.panel
        } else {
            theme.spacing.control_y
        })
    };
    let inner_width = super::layout_tokens::clamped_content(widget.text.as_deref(), Rect {
        origin: Point { x: 0, y: 0 },
        size: Size { width: width.saturating_sub(inset.saturating_mul(2)), height: 0 },
    }).size.width;
    let children = widget
        .children()
        .iter()
        .copied()
        .filter(|child| {
            !tree.get(*child).is_some_and(|child| child.kind == WidgetKind::Menu && widget.kind == WidgetKind::MenuItem)
                && !empty_status_bar(tree, *child) && !is_dialog(tree, *child) && !is_notification(tree, *child) && !is_tooltip(tree, *child)
        })
        .collect::<Vec<_>>();
    let gap = if widget.kind == WidgetKind::Tabs {
        0
    } else {
        super::layout_tokens::child_gap(widget.text.as_deref(), u32::from(theme.spacing.unit))
    };
    let horizontal = !stacked_form(widget, inner_width) && lays_out_children_horizontally(widget);
    let inner = if horizontal {
        let gaps =
            gap.saturating_mul(u32::try_from(children.len().saturating_sub(1)).unwrap_or(u32::MAX));
        // Measure wrapped text using the same column widths as final layout.
        // Settings split rows give their value less than half the width.
        let widths = if widget.kind == WidgetKind::Row
            && let Some(columns) = widget.parent.and_then(|parent| tree.get(parent))
                .filter(|parent| parent.kind == WidgetKind::Table)
                .and_then(|parent| super::layout_tokens::table_columns(parent.text.as_deref()))
            && columns.len() == children.len()
        {
            // Table column weights override row split hints in final layout.
            partition_weighted(inner_width, gap, &columns)
        } else if widget.text.as_deref().and_then(split_spec).is_some() {
            let preferred = children.iter()
                .map(|child| preferred_size(tree, *child, theme).width)
                .collect::<Vec<_>>();
            partition_split(inner_width, gap, &preferred,
                widget.text.as_deref().and_then(split_spec), true)
        } else if widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| matches!(part, "mail-header" | "wrap-controls"))) {
            let preferred = children.iter().map(|child| preferred_size(tree, *child, theme).width).collect::<Vec<_>>();
            let flex = children.iter().map(|child| child_flexes(tree, *child, true)).collect::<Vec<_>>();
            partition_flex(inner_width, gap, &preferred, &flex)
        } else {
            vec![inner_width.saturating_sub(gaps)
                / u32::try_from(children.len()).unwrap_or(1).max(1); children.len()]
        };
        children
            .iter()
            .zip(widths)
            .map(|(child, child_width)| preferred_height_for_width(tree, *child, child_width, theme))
            .max()
            .unwrap_or(0)
    } else {
        children
            .iter()
            .map(|child| preferred_height_for_width(tree, *child, inner_width, theme))
            .fold(0_u32, u32::saturating_add)
            .saturating_add(gap.saturating_mul(
                u32::try_from(children.len().saturating_sub(1)).unwrap_or(u32::MAX),
            ))
    };
    let height = inner.saturating_add(inset.saturating_mul(2));
    if widget.kind == WidgetKind::Accordion && !widget.state.contains(rice_ui::WidgetState::CHECKED)
    {
        preferred_size(tree, id, theme).height
    } else if super::layout_tokens::is_store_document(widget.text.as_deref()) {
        // The Store grid reflows against the actual viewport width. Its
        // width-aware height is authoritative; generic preferred_size treats
        // IconGrid children as a vertical stack and creates a large blank tail.
        height
    } else {
        height.max(preferred_size(tree, id, theme).height)
    }
}

pub(super) fn is_compact_label(tree: &RetainedTree, widget: &Widget) -> bool {
    widget.kind == WidgetKind::Label
        && widget
            .parent
            .and_then(|parent| tree.get(parent))
            .is_some_and(|parent| {
                super::layout_tokens::is_intrinsic_stack(parent.text.as_deref())
                    || super::layout_tokens::positioned_item(parent.text.as_deref()).is_some()
                    || parent
                        .parent
                        .and_then(|grandparent| tree.get(grandparent))
                        .is_some_and(|grandparent| {
                            grandparent.kind == WidgetKind::Row
                                && super::layout_tokens::compacts_children(
                                    grandparent.text.as_deref(),
                                )
                        })
            })
}

fn compact_label_line_height(widget: &Widget, theme: &Theme, lines: u32) -> u32 {
    let preferred = super::layout_tokens::preferred_line_height(widget, theme);
    if lines <= 1 {
        return preferred;
    }
    // Canvas paints ordinary native labels in a 1.25 line box. Compact
    // stacks may omit general control padding, so explicitly multiline copy
    // must reserve that exact box or its last line is ellipsized at paint time.
    let size = u32::from(super::layout_tokens::preferred_text_size(widget, theme));
    preferred.max(size.saturating_mul(125).saturating_add(50) / 100)
}

pub(super) fn is_chat_composer(tree: &RetainedTree, widget: &Widget) -> bool {
    widget.kind == WidgetKind::TextField
        && widget.parent.and_then(|id| tree.get(id))
            .and_then(|parent| parent.text.as_deref())
            .is_some_and(|text| text.split(':').any(|token| token == "chat-composer"))
}

fn is_canvas_multiline_label(tree: &RetainedTree, widget: &Widget) -> bool {
    if !is_compact_label(tree, widget) {
        return false;
    }
    let has_multiline_token = |text: Option<&str>| {
        text.is_some_and(|text| text.split(':').any(|part| part == "multiline"))
    };
    let parent = widget.parent.and_then(|parent| tree.get(parent));
    parent.is_some_and(|parent| has_multiline_token(parent.text.as_deref()))
        || parent
            .and_then(|parent| parent.parent)
            .and_then(|grandparent| tree.get(grandparent))
            .is_some_and(|grandparent| has_multiline_token(grandparent.text.as_deref()))
}

fn wrapped_line_count(widget: &Widget, text: &str, available: u32, theme: &Theme) -> u32 {
    let space_width = estimated_widget_text_width(widget, " ", theme);
    text.lines().fold(0_u32, |total, source| {
        if source.trim().is_empty() {
            return total.saturating_add(1);
        }
        let mut lines = 1_u32;
        let mut current_width = 0_u32;
        for word in source.split_whitespace() {
            let word_width = estimated_widget_text_width(widget, word, theme);
            let candidate_width = if current_width == 0 {
                word_width
            } else {
                current_width
                    .saturating_add(space_width)
                    .saturating_add(word_width)
            };
            if current_width != 0 && candidate_width > available {
                lines = lines.saturating_add(1);
                current_width = word_width;
            } else {
                current_width = candidate_width;
            }
        }
        total.saturating_add(lines)
    })
}

fn visible_tree_rows(tree: &RetainedTree, root: WidgetId) -> Vec<(WidgetId, u8)> {
    fn append(tree: &RetainedTree, parent: WidgetId, depth: u8, rows: &mut Vec<(WidgetId, u8)>) {
        let Some(parent) = tree.get(parent) else {
            return;
        };
        for child in parent.children() {
            let Some(item) = tree.get(*child) else {
                continue;
            };
            if item.kind != WidgetKind::TreeItem {
                continue;
            }
            rows.push((*child, depth));
            if item.state.contains(rice_ui::WidgetState::CHECKED) && depth < 31 {
                append(tree, *child, depth.saturating_add(1), rows);
            }
        }
    }

    let mut rows = Vec::new();
    append(tree, root, 0, &mut rows);
    rows
}

fn is_responsive_document(tree: &RetainedTree, id: WidgetId) -> bool {
    let mut current = Some(id);
    for _ in 0..12 {
        let Some(candidate) = current.and_then(|candidate| tree.get(candidate)) else {
            return false;
        };
        if candidate.text.as_deref() == Some("panel:welcome-document")
            || super::layout_tokens::is_intrinsic_flow(candidate.text.as_deref())
            || super::layout_tokens::is_settings_page(candidate.text.as_deref())
            || super::layout_tokens::is_side_sheet(candidate.text.as_deref())
            || super::layout_tokens::is_store_document(candidate.text.as_deref())
        {
            return true;
        }
        current = candidate.parent;
    }
    false
}

fn layout_orchard_files_status(
    tree: &RetainedTree,
    status: WidgetId,
    rect: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    geometry.insert(status, rect);
    let Some(status_widget) = tree.get(status) else {
        return;
    };
    let Some(layout) = status_widget.children().iter().copied().find(|child| {
        tree.get(*child).is_some_and(|child| {
            child
                .text
                .as_deref()
                .is_some_and(|text| text.split(':').any(|token| token == "files-status-layout"))
        })
    }) else {
        return;
    };
    geometry.insert(layout, rect);
    let Some(status_layout_widget) = tree.get(layout) else {
        return;
    };
    let Some(copy) = status_layout_widget
        .children()
        .iter()
        .copied()
        .find(|child| {
            tree.get(*child).is_some_and(|child| {
                child
                    .text
                    .as_deref()
                    .is_some_and(|text| text.split(':').any(|token| token == "files-status-copy"))
            })
        })
    else {
        return;
    };
    geometry.insert(copy, rect);
    let Some(copy_widget) = tree.get(copy) else {
        return;
    };
    let zoom_width = 140.min(rect.size.width / 3);
    let copy_width = rect.size.width.saturating_sub(zoom_width);
    let location_width = copy_width.saturating_mul(2) / 5;
    let breadcrumb = Rect {
        origin: rect.origin,
        size: Size {
            width: location_width,
            height: rect.size.height,
        },
    };
    let summary = Rect {
        origin: Point {
            x: rect.origin.x.saturating_add_unsigned(location_width),
            y: rect.origin.y,
        },
        size: Size {
            width: copy_width.saturating_sub(location_width),
            height: rect.size.height,
        },
    };
    if let Some(location) = copy_widget.children().first() {
        layout_widget(tree, *location, breadcrumb, theme, geometry);
    }
    if let Some(selection) = copy_widget.children().get(1) {
        layout_widget(tree, *selection, summary, theme, geometry);
    }
    if let Some(zoom) = status_layout_widget
        .children()
        .iter()
        .copied()
        .find(|child| *child != copy)
    {
        let zoom_rect = Rect {
            origin: Point {
                x: rect
                    .origin
                    .x
                    .saturating_add_unsigned(rect.size.width.saturating_sub(zoom_width)),
                y: rect.origin.y,
            },
            size: Size {
                width: zoom_width,
                height: rect.size.height,
            },
        };
        layout_widget(tree, zoom, zoom_rect, theme, geometry);
    }
}

fn layout_orchard_files_shell(
    tree: &RetainedTree,
    widget: &Widget,
    children: &[WidgetId],
    content: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    let Some(files_content) = children.iter().copied().find(|child| {
        tree.get(*child)
            .is_some_and(|child| super::layout_tokens::is_files_content(child.text.as_deref()))
    }) else {
        return;
    };
    let Some(content_widget) = tree.get(files_content) else {
        return;
    };
    let sidebar = content_widget.children().first().copied().filter(|child| {
        tree.get(*child)
            .is_some_and(|child| super::layout_tokens::is_sidebar_panel(child.text.as_deref()))
    });
    let Some(main) = content_widget
        .children()
        .get(usize::from(sidebar.is_some()))
        .copied()
    else {
        return;
    };
    let permille = content_widget
        .text
        .as_deref()
        .and_then(|text| text.strip_prefix("split:"))
        .and_then(|text| text.split(':').next())
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(220)
        .clamp(100, 600);
    let drawer = sidebar.is_some() && content.size.width < 600 && super::layout_measurements::compact_form_factor();
    let sidebar_width = sidebar.filter(|_| !drawer).map_or(0, |_| {
        u32::try_from(u64::from(content.size.width).saturating_mul(u64::from(permille)) / 1_000)
            .unwrap_or(content.size.width)
            .clamp(200.min(content.size.width), 420.min(content.size.width))
    });
    let rtl = rice_i18n::direction() == rice_i18n::TextDirection::Rtl;
    // The sidebar spans every header band. Move the complete semantic regions
    // before laying out their descendants, preserving widths and logical IDs.
    let trailing = Rect {
        origin: Point {
            x: content.origin.x.saturating_add_unsigned(if rtl { 0 } else { sidebar_width }),
            y: content.origin.y + if drawer { 44 } else { 0 },
        },
        size: Size {
            width: content.size.width.saturating_sub(sidebar_width),
            height: content.size.height.saturating_sub(if drawer {44} else {0}),
        },
    };
    if let Some(sidebar) = sidebar.filter(|_| drawer) {
        let mut rect = content;
        if !super::layout_measurements::navigation_open(sidebar) { rect.size.height = rect.size.height.min(44); }
        super::layout_adaptive::layout_navigation_drawer(tree, sidebar, rect, theme, geometry);
        if super::layout_measurements::navigation_open(sidebar) {
            geometry.insert(files_content, content);
            return;
        }
    }
    if let Some(sidebar) = sidebar.filter(|_| !drawer) {
        layout_widget(
            tree,
            sidebar,
            Rect {
                origin: Point {
                    x: content.origin.x.saturating_add_unsigned(if rtl {
                        content.size.width.saturating_sub(sidebar_width)
                    } else { 0 }),
                    y: content.origin.y,
                },
                size: Size {
                    width: sidebar_width,
                    height: content.size.height,
                },
            },
            theme,
            geometry,
        );
    }

    let status = children.iter().copied().find(|child| {
        tree.get(*child)
            .is_some_and(|child| super::layout_tokens::is_files_status(child.text.as_deref()))
    });
    let status_height = status.map_or(0, |_| {
        u32::from(theme.metrics.control_height)
            .saturating_add(2)
            .clamp(24, 34)
            .min(trailing.size.height / 3)
    });
    let mut cursor_y = trailing.origin.y;
    for child in children.iter().copied().filter(|child| {
        *child != files_content
            && Some(*child) != status
            && tree.get(*child).is_some_and(|child| {
                !(child.kind == WidgetKind::MenuBar && child.text.as_deref() == Some("context"))
            })
    }) {
        let height = preferred_size(tree, child, theme)
            .height
            .min(trailing.size.height.saturating_sub(status_height));
        let header = Rect {
            origin: Point {
                x: trailing.origin.x,
                y: cursor_y,
            },
            size: Size {
                width: trailing.size.width,
                height,
            },
        };
        layout_widget(tree, child, header, theme, geometry);
        cursor_y = cursor_y.saturating_add_unsigned(height);
    }
    let used_header = cursor_y
        .saturating_sub(trailing.origin.y)
        .max(0)
        .cast_unsigned();
    let main_height = trailing
        .size
        .height
        .saturating_sub(used_header)
        .saturating_sub(status_height);
    let main_rect = Rect {
        origin: Point {
            x: trailing.origin.x,
            y: cursor_y,
        },
        size: Size {
            width: trailing.size.width,
            height: main_height,
        },
    };
    layout_widget(tree, main, main_rect, theme, geometry);
    // A SplitPane needs the complete shell rectangle so its sidebar can span
    // every header band. Without a sidebar CONTENT is an ordinary Container;
    // giving it that same full rectangle paints its surface over the menu,
    // location and toolbar that were laid out before it.
    geometry.insert(
        files_content,
        if sidebar.is_some() {
            content
        } else {
            main_rect
        },
    );
    if let Some(status) = status {
        layout_orchard_files_status(
            tree,
            status,
            Rect {
                origin: Point {
                    x: trailing.origin.x,
                    y: trailing.origin.y.saturating_add_unsigned(
                        trailing.size.height.saturating_sub(status_height),
                    ),
                },
                size: Size {
                    width: trailing.size.width,
                    height: status_height,
                },
            },
            theme,
            geometry,
        );
    }

    // Keep the parameter part of this semantic layout contract explicit: only
    // the root RiceFiles column may enter this branch.
    debug_assert!(
        widget.parent.is_none() || super::layout_tokens::is_tab_page(widget.text.as_deref())
    );
}

pub(super) fn layout_widget(
    tree: &RetainedTree,
    id: WidgetId,
    rect: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    if empty_status_bar(tree, id) { return; }
    let Some(widget) = tree.get(id) else {
        return;
    };
    if widget.kind == WidgetKind::List
        && super::layout_measurements::navigation_grid_extent(id).is_some()
    {
        super::layout_adaptive::layout_navigation_grid(tree, id, rect, theme, geometry);
        return;
    }
    let mut rect = rect;
    if widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "reading-column")) {
        let width = rect.size.width.min(760);
        rect.origin.x = rect.origin.x.saturating_add_unsigned((rect.size.width - width) / 2);
        rect.size.width = width;
    }
    if let Some(height) = super::layout_tokens::positioned_canvas_height(widget.text.as_deref()) {
        let width = rect.size.width.min(rect.size.height.saturating_mul(620) / height);
        let fitted_height = width.saturating_mul(height) / 620;
        rect.origin.x += ((rect.size.width - width) / 2) as i32;
        rect.origin.y += ((rect.size.height - fitted_height) / 2) as i32;
        rect.size = Size { width, height: fitted_height };
    }
    if widget.kind == WidgetKind::SceneSurface {
        rect = super::scene_surface::page_rect(rect, widget.text.as_deref());
    }
    if widget.kind == WidgetKind::Stack
        && let Some((surface, bounds)) = super::scene_surface::overlay(widget.text.as_deref())
    {
        let source = tree.get(surface).filter(|source| {
            source.kind == WidgetKind::SceneSurface && source.parent == widget.parent
        }).and_then(|_| geometry.get(&surface).copied());
        // A missing/hidden/reordered surface must not turn an anchored editor
        // into a full-viewport input target.
        rect = source.map_or(Rect::default(), |source| super::scene_surface::overlay_rect(source, bounds));
    }
    geometry.insert(id, rect);
    if let Some((_, cells)) = super::layout_adaptive::toolbar_flow(tree, id, rect.size.width, theme) {
        for (child, mut cell) in cells {
            cell.origin.x += rect.origin.x;
            cell.origin.y += rect.origin.y;
            layout_widget(tree, child, cell, theme, geometry);
        }
        return;
    }
    if widget.kind == WidgetKind::TerminalGrid {
        let cell_height = widget
            .text
            .as_deref()
            .and_then(|s| s.split(':').nth(1)?.parse::<u32>().ok())
            .unwrap_or(13)
            .clamp(10, 24)
            + 5;
        for (index, child) in widget.children().iter().enumerate() {
            geometry.insert(
                *child,
                Rect {
                    origin: Point {
                        x: rect.origin.x,
                        y: rect.origin.y + index as i32 * cell_height as i32,
                    },
                    size: Size {
                        width: rect.size.width,
                        height: cell_height,
                    },
                },
            );
        }
        return;
    }

    if widget.kind == WidgetKind::MenuBar && widget.text.as_deref() == Some("context") {
        return;
    }
    if widget.children().is_empty()
        || matches!(
            widget.kind,
            WidgetKind::Menu | WidgetKind::TreeItem
        ) || (widget.kind == WidgetKind::MenuItem && !inline_menu_content(tree, widget))
    {
        return;
    }
    let children = widget
        .children()
        .iter()
        .copied()
        .filter(|child| {
            !empty_status_bar(tree, *child) && !is_dialog(tree, *child) && !is_notification(tree, *child) && !is_tooltip(tree, *child)
                && !tree.get(*child).is_some_and(|widget|
                    widget.kind == WidgetKind::MenuBar && widget.text.as_deref() == Some("context"))
        })
        .collect::<Vec<_>>();
    if children.is_empty() {
        return;
    }
    let feature_card_copy = widget.kind == WidgetKind::Column
        && widget
            .parent
            .and_then(|parent| tree.get(parent))
            .is_some_and(|parent| {
                super::layout_tokens::is_feature_card_content(parent.text.as_deref())
            });
    // Host-rendered application clients already own their complete content
    // rectangle. Insetting the sole HostSurface child exposed the window-body
    // fill as a dark band around Electron applications, most visibly beneath
    // Neon Grid's luminous frame artwork.
    let edge_to_edge_host_surface = children.len() == 1
        && children
            .first()
            .and_then(|child| tree.get(*child))
            .is_some_and(|child| child.kind == WidgetKind::HostSurface);
    let edge_to_edge_sidebar_split = theme.glass_sidebars
        && widget.kind == WidgetKind::SplitPane
        && children
            .first()
            .and_then(|child| tree.get(*child))
            .is_some_and(|child| super::layout_tokens::is_sidebar_panel(child.text.as_deref()));
    let dense_table_content = super::layout_tokens::is_dense_table(widget.text.as_deref())
        || (widget.kind == WidgetKind::ScrollView
            && children.iter().any(|child| {
                tree.get(*child).is_some_and(|child| {
                    child.kind == WidgetKind::Table
                        && super::layout_tokens::is_dense_table(child.text.as_deref())
                })
            }));
    let inset = if let Some(padding) = super::layout_tokens::padding(widget.text.as_deref()) {
        Insets::all(padding)
    } else if super::layout_tokens::has_token(widget.text.as_deref(), "office-ribbon-group") {
        Insets { top: 6, right: 10, bottom: 6, left: 10 }
    } else if widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "flush")) {
        Insets::all(0)
    } else if super::layout_tokens::positioned_canvas_height(widget.text.as_deref()).is_some()
        || super::layout_tokens::positioned_item(widget.text.as_deref()).is_some()
    {
        Insets::all(0)
    } else if widget.kind == WidgetKind::Tabs
        && !super::layout_tokens::is_browser_tab_strip(widget.text.as_deref())
    {
        Insets::all(0)
    } else if super::layout_tokens::is_tab_page(widget.text.as_deref()) {
        Insets::all(
            if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
                4
            } else {
                0
            },
        )
    } else if super::layout_tokens::is_simplified_files_shell(widget.text.as_deref())
        || edge_to_edge_sidebar_split
        || dense_table_content
    {
        Insets::all(0)
    } else if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_dense_row(widget.text.as_deref())
    {
        Insets::all(0)
    } else if super::layout_tokens::is_browser_tab_strip(widget.text.as_deref()) {
        Insets {
            top: u32::from(theme.spacing.control_y),
            right: u32::from(theme.spacing.control_y),
            bottom: 0,
            left: u32::from(theme.spacing.control_y),
        }
    } else if super::layout_tokens::is_bookmark_bar(widget.text.as_deref()) {
        Insets::all(2)
    } else if feature_card_copy
        || edge_to_edge_host_surface
        || super::layout_tokens::is_status_bar(widget.text.as_deref())
        || super::layout_tokens::is_spin_control(widget.text.as_deref())
        || super::layout_tokens::is_spin_buttons(widget.text.as_deref())
        || super::layout_tokens::is_tab_composite(widget.text.as_deref())
        || super::layout_tokens::is_bookmark_composite(widget.text.as_deref())
    {
        Insets::all(0)
    } else if widget.text.as_deref() == Some("panel:welcome-hero") {
        Insets {
            top: u32::from(theme.spacing.panel),
            right: 48,
            bottom: u32::from(theme.spacing.panel),
            left: 48,
        }
    } else if super::layout_tokens::is_settings_page(widget.text.as_deref()) {
        Insets::all(u32::from(theme.spacing.section))
    } else if super::layout_tokens::is_settings_card(widget.text.as_deref()) {
        Insets::all(u32::from(theme.spacing.control_y))
    } else if super::layout_tokens::is_settings_shell(widget.text.as_deref()) {
        Insets::all(u32::from(theme.spacing.control_y))
    } else if super::layout_tokens::distributes_children(widget.text.as_deref())
        || super::layout_tokens::calendar_day_band(widget.text.as_deref()).is_some()
        || (widget.kind == WidgetKind::Row
            && super::layout_tokens::equal_row(widget.text.as_deref()))
        || (widget.kind == WidgetKind::IconGrid
            && super::layout_tokens::is_file_grid(widget.text.as_deref()))
        || widget
            .text
            .as_deref()
            .is_some_and(|text| text.starts_with("desktop:"))
    {
        Insets::all(0)
    } else if super::layout_tokens::is_thumbnail_cell(widget.text.as_deref()) {
        Insets::all(1)
    } else if matches!(
        widget.kind,
        WidgetKind::Container
            | WidgetKind::GroupBox
            | WidgetKind::Dialog
            | WidgetKind::Popover
            | WidgetKind::Notification
    ) {
        Insets::all(u32::from(theme.spacing.panel))
    } else {
        Insets::all(u32::from(theme.spacing.control_y))
    };
    let mut content =
        super::layout_tokens::clamped_content(widget.text.as_deref(), rect.inset(inset));
    if widget.kind == WidgetKind::Accordion {
        if !widget.state.contains(rice_ui::WidgetState::CHECKED) {
            return;
        }
        let header = u32::from(theme.metrics.control_height).min(content.size.height);
        let gap = u32::from(theme.spacing.unit).min(content.size.height.saturating_sub(header));
        let consumed = header.saturating_add(gap).min(content.size.height);
        content.origin.y = content.origin.y.saturating_add_unsigned(consumed);
        content.size.height = content.size.height.saturating_sub(consumed);
    }
    if matches!(widget.kind, WidgetKind::GroupBox | WidgetKind::Popover)
        && widget.text.as_deref().is_some_and(|text| !text.is_empty() && !text.starts_with("panel:"))
    {
        let header =
            u32::from(theme.metrics.control_height).saturating_add(u32::from(theme.spacing.unit));
        let consumed = header.min(content.size.height);
        content.origin.y = content.origin.y.saturating_add_unsigned(consumed);
        content.size.height = content.size.height.saturating_sub(consumed);
    }
    if widget.kind == WidgetKind::Dialog
        && (widget
            .text
            .as_deref()
            .is_some_and(|text| !text.starts_with("panel:"))
            || theme.chrome_profile == rice_ui::ChromeProfile::Redmond98
                && super::layout_tokens::is_side_sheet(widget.text.as_deref()))
    {
        let header = u32::from(theme.metrics.control_height)
            .saturating_add(u32::from(theme.spacing.section));
        content.origin.y = content
            .origin
            .y
            .saturating_add_unsigned(header.min(content.size.height));
        content.size.height = content.size.height.saturating_sub(header);
    }
    if layout_integrated_tabs(tree, widget, &children, content, theme, geometry) {
        return;
    }
    if super::layout_tokens::is_simplified_files_shell(widget.text.as_deref())
        && !children.iter().any(|child| {
            tree.get(*child).is_some_and(|child| {
                child.kind == WidgetKind::Tabs
                    && super::layout_tokens::is_integrated_tabs(child.text.as_deref())
            })
        })
    {
        layout_orchard_files_shell(tree, widget, &children, content, theme, geometry);
        return;
    }
    if super::layout_tokens::positioned_canvas_height(widget.text.as_deref()).is_some()
        || (widget.kind == WidgetKind::Stack && super::layout_tokens::positioned_overlay(widget.text.as_deref())) {
        for child in &children {
            if let Some([x,y,width,height]) = tree.get(*child).and_then(|child| super::layout_tokens::positioned_item(child.text.as_deref())) {
                let rect = Rect {
                    origin: Point { x:content.origin.x.saturating_add_unsigned(content.size.width*x/1000), y:content.origin.y.saturating_add_unsigned(content.size.height*y/1000) },
                    size: Size { width:content.size.width*width/1000, height:content.size.height*height/1000 },
                };
                layout_widget(tree,*child,rect,theme,geometry);
            }
        }
        return;
    }
    if widget.kind == WidgetKind::Stack {
        for child in &children {
            layout_widget(tree, *child, content, theme, geometry);
        }
        return;
    }
    if widget.kind == WidgetKind::IconGrid {
        let centered_empty_state = super::layout_tokens::is_file_grid(widget.text.as_deref())
            && children.len() == 1
            && tree.get(children[0]).is_some_and(|child| {
                child.kind == WidgetKind::Label && child.state.contains(WidgetState::CENTER_ALIGNED)
            });
        if centered_empty_state {
            layout_widget(tree, children[0], content, theme, geometry);
            return;
        }
        if widget
            .text
            .as_deref()
            .is_some_and(|text| text == "desktop-grid" || text.starts_with("desktop-grid:"))
        {
            layout_desktop_grid(
                tree,
                &children,
                content,
                theme,
                widget.text.as_deref(),
                geometry,
            );
        } else {
            layout_icon_grid(
                tree,
                &children,
                content,
                theme,
                super::layout_tokens::zoom_permille(widget.text.as_deref()),
                geometry,
            );
        }
        return;
    }
    if widget.kind == WidgetKind::Tree {
        let row_height = u32::from(theme.metrics.control_height);
        let indent = if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
            u32::from(theme.metrics.icon_small)
        } else {
            u32::from(theme.metrics.icon_small).saturating_add(u32::from(theme.spacing.unit))
        };
        for (index, (child, depth)) in visible_tree_rows(tree, id).into_iter().enumerate() {
            let offset = indent
                .saturating_mul(u32::from(depth))
                .min(content.size.width);
            let rect = Rect {
                origin: Point {
                    x: content.origin.x.saturating_add_unsigned(offset),
                    y: content.origin.y.saturating_add_unsigned(
                        u32::try_from(index)
                            .unwrap_or(u32::MAX)
                            .saturating_mul(row_height),
                    ),
                },
                size: Size {
                    width: content.size.width.saturating_sub(offset),
                    height: row_height,
                },
            };
            layout_widget(tree, child, rect, theme, geometry);
        }
        return;
    }
    if widget.kind == WidgetKind::ScrollView {
        if widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "document-viewport")) {
            for child in &children { layout_widget(tree, *child, content, theme, geometry); }
            return;
        }
        if super::layout_tokens::is_editor_viewport(widget.text.as_deref()) {
            let (_, extent) = scroll_metrics(widget.text.as_deref());
            let width = content.size.width.saturating_sub(if extent.unwrap_or(0) > content.size.height {
                u32::from(theme.metrics.scrollbar_width) + u32::from(theme.spacing.panel)
            } else { 0 });
            for child in &children {
                layout_widget(tree, *child, Rect { origin: content.origin, size: Size { width, height: content.size.height } }, theme, geometry);
            }
            return;
        }
        for child in &children {
            let preferred = preferred_size(tree, *child, theme);
            let (requested_offset, explicit_extent) = scroll_metrics(widget.text.as_deref());
            let virtual_window =
                super::layout_tokens::virtual_window_offset(widget.text.as_deref());
            let mut child_width = content.size.width;
            let mut child_height = if virtual_window.is_some() {
                scroll_child_extent(tree, *child, child_width, preferred.height, None, theme)
                    .max(content.size.height)
            } else {
                scroll_child_extent(
                    tree,
                    *child,
                    child_width,
                    preferred.height,
                    explicit_extent,
                    theme,
                )
                .max(content.size.height)
            };
            let logical_extent = virtual_window
                .and(explicit_extent)
                .unwrap_or(child_height)
                .max(content.size.height);
            if logical_extent > content.size.height {
                child_width = child_width
                    .saturating_sub(u32::from(theme.metrics.scrollbar_width))
                    .saturating_sub(u32::from(theme.spacing.panel));
                child_height = if virtual_window.is_some() {
                    scroll_child_extent(tree, *child, child_width, preferred.height, None, theme)
                        .max(content.size.height)
                } else {
                    scroll_child_extent(
                        tree,
                        *child,
                        child_width,
                        preferred.height,
                        explicit_extent,
                        theme,
                    )
                    .max(content.size.height)
                };
            }
            let logical_extent = virtual_window
                .and(explicit_extent)
                .unwrap_or(child_height)
                .max(content.size.height);
            let maximum_offset = logical_extent.saturating_sub(content.size.height);
            let anchor_offset = super::layout_tokens::scroll_anchor(widget.text.as_deref())
                .and_then(WidgetId::new)
                .and_then(|anchor| {
                    let probe_rect = Rect {
                        origin: content.origin,
                        size: Size {
                            width: child_width,
                            height: child_height,
                        },
                    };
                    let mut probe = BTreeMap::new();
                    layout_widget(tree, *child, probe_rect, theme, &mut probe);
                    probe.get(&anchor).map(|rect| {
                        rect.origin
                            .y
                            .saturating_sub(content.origin.y)
                            .max(0)
                            .cast_unsigned()
                    })
                });
            let offset = anchor_offset
                .unwrap_or(requested_offset)
                .min(maximum_offset);
            let window_offset = virtual_window
                .unwrap_or_default()
                .min(logical_extent.saturating_sub(child_height));
            let child_rect = Rect {
                origin: Point {
                    x: content.origin.x,
                    y: content
                        .origin
                        .y
                        .saturating_add_unsigned(window_offset)
                        .saturating_sub_unsigned(offset),
                },
                size: Size {
                    width: child_width,
                    height: child_height,
                },
            };
            layout_widget(tree, *child, child_rect, theme, geometry);
        }
        return;
    }
    let compact_split = widget.kind == WidgetKind::SplitPane
        && is_compact_split(content.size.width, widget.text.as_deref());
    let is_row = !compact_split && !stacked_form(widget, content.size.width) && lays_out_children_horizontally(widget);
    // Only generic horizontal split panes participate in this first RTL layout
    // boundary. Keep rows, compact/vertical splits and custom Files shells in
    // their existing coordinate systems. Child identity and ratios stay logical.
    let mirror_split = is_row && widget.kind == WidgetKind::SplitPane
        && !super::layout_tokens::is_simplified_files_shell(widget.text.as_deref())
        && rice_i18n::direction() == rice_i18n::TextDirection::Rtl;
    let gap = if widget.parent.is_none()
        && matches!(
            theme.chrome_profile,
            rice_ui::ChromeProfile::Orchard | rice_ui::ChromeProfile::Redmond98
        ) {
        // AppKit and classic Win32 window content join title, toolbar, tab,
        // client and status surfaces without exposing body gutters.
        0
    } else if widget.kind == WidgetKind::Tabs {
        0
    } else if edge_to_edge_sidebar_split {
        0
    } else if feature_card_copy {
        2
    } else if widget.kind == WidgetKind::MenuBar {
        0
    } else {
        super::layout_tokens::child_gap(widget.text.as_deref(), u32::from(theme.spacing.unit))
    };
    let mut preferred = children
        .iter()
        .map(|child| {
            let size = preferred_size(tree, *child, theme);
            if is_row {
                size.width
            } else if let Some(height) = tree.get(*child).and_then(|child| {
                super::layout_tokens::welcome_hero_height(child.text.as_deref(), content.size.width)
            }) {
                height
            } else {
                // Labels and explanatory copy are multiline by default in
                // every application, not only inside settings/gallery pages.
                // Non-text leaves return their ordinary preferred height.
                preferred_height_for_width(tree, *child, content.size.width, theme)
            }
        })
        .collect::<Vec<_>>();
    if matches!(
        widget.kind,
        WidgetKind::List | WidgetKind::Table | WidgetKind::Tree
    ) {
        let zoom = super::layout_tokens::zoom_permille(widget.text.as_deref());
        for length in &mut preferred {
            *length = u32::try_from(u64::from(*length).saturating_mul(u64::from(zoom)) / 1_000)
                .unwrap_or(u32::MAX)
                .max(1);
        }
    }
    let available = if is_row {
        content.size.width
    } else {
        content.size.height
    };
    let distribute = super::layout_tokens::distributes_children(widget.text.as_deref());
    let flex = if (widget.kind == WidgetKind::Row
        && (super::layout_tokens::equal_row(widget.text.as_deref())
            || super::layout_tokens::equal_fixed(widget.text.as_deref())))
        || distribute
    {
        vec![true; children.len()]
    } else {
        children
            .iter()
            .map(|child| child_flexes(tree, *child, is_row))
            .collect::<Vec<_>>()
    };
    let compact_navigation = compact_split
        && children
            .first()
            .is_some_and(|child| compact_navigation_list(tree, *child).is_some());
    let lengths = if is_row && widget.text.as_deref() == Some("panel:status-sections") && children.len() == 4 {
        // Reserve accessories independently of their changing text. The path
        // takes remaining space, so large columns cannot move the zoom group.
        let usable = available.saturating_sub(gap * 3);
        let zoom = 220.min(usable / 2);
        let position = 170.min(usable.saturating_sub(zoom) / 2);
        let format = 310.min(usable.saturating_sub(zoom + position));
        vec![usable.saturating_sub(zoom + position + format), position, format, zoom]
    } else if super::layout_tokens::is_intrinsic_flow(widget.text.as_deref()) {
        preferred.clone()
    } else if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_feature_card_content(widget.text.as_deref())
        && preferred.len() == 2
    {
        let icon = 44.min(available);
        vec![icon, available.saturating_sub(icon).saturating_sub(gap)]
    } else if compact_navigation {
        compact_split_lengths(tree, &children, available, gap, theme)
    } else if super::layout_tokens::is_browser_tab(widget.text.as_deref()) && preferred.len() == 3 {
        let favicon = u32::from(theme.metrics.icon_small)
            .saturating_add(gap.saturating_mul(2))
            .min(available / 4);
        let close = u32::from(theme.metrics.control_height).min(available / 3);
        vec![
            favicon,
            available
                .saturating_sub(favicon)
                .saturating_sub(close)
                .saturating_sub(gap.saturating_mul(2)),
            close,
        ]
    } else if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_form_control_row(widget.text.as_deref())
        && preferred.len() == 2
    {
        let label =
            shared_form_label_width(tree, widget, theme).min(available.saturating_mul(2) / 5);
        vec![label, available.saturating_sub(label).saturating_sub(gap)]
    } else if super::layout_tokens::is_tab_composite(widget.text.as_deref()) && preferred.len() == 2
    {
        let close = u32::from(theme.metrics.control_height).min(available / 2);
        vec![available.saturating_sub(close).saturating_sub(gap), close]
    } else if widget.kind == WidgetKind::Tabs {
        const MAX_TAB_WIDTH: u32 = 220;
        let fixed = children
            .iter()
            .map(|child| {
                tree.get(*child).is_some_and(|child| {
                    child.text.as_deref() == Some("panel:tab-close")
                        || child.text.as_deref() == Some("+")
                        || super::layout_tokens::is_window_control(child.text.as_deref())
                        || super::layout_tokens::is_window_drag_region(child.text.as_deref())
                })
            })
            .collect::<Vec<_>>();
        let tab_count = fixed.iter().filter(|fixed| !**fixed).count();
        for ((length, fixed), child) in preferred.iter_mut().zip(&fixed).zip(&children) {
            if tree.get(*child).is_some_and(|child| {
                super::layout_tokens::is_window_drag_region(child.text.as_deref())
            }) {
                *length = 0;
                continue;
            }
            let browser_tab = tree
                .get(*child)
                .is_some_and(|child| super::layout_tokens::is_browser_tab(child.text.as_deref()));
            if !fixed && (tab_count > 1 || browser_tab) {
                *length = (*length).min(MAX_TAB_WIDTH);
            }
        }
        let gaps = gap
            .saturating_mul(u32::try_from(preferred.len().saturating_sub(1)).unwrap_or(u32::MAX));
        let wanted = preferred
            .iter()
            .copied()
            .fold(0_u32, u32::saturating_add)
            .saturating_add(gaps);
        if wanted <= available {
            if let Some(index) = children.iter().position(|child| {
                tree.get(*child).is_some_and(|child| {
                    super::layout_tokens::is_window_drag_region(child.text.as_deref())
                })
            }) {
                preferred[index] = preferred[index].saturating_add(available - wanted);
            }
            preferred.clone()
        } else {
            partition_distribute(available, gap, &preferred, &fixed)
        }
    } else if widget.kind == WidgetKind::Row
        && super::layout_tokens::equal_row(widget.text.as_deref())
    {
        partition_distribute(available, gap, &preferred, &vec![false; preferred.len()])
    } else if super::layout_tokens::aligns_actions_end(widget.text.as_deref()) {
        let gaps = gap
            .saturating_mul(u32::try_from(preferred.len().saturating_sub(1)).unwrap_or(u32::MAX));
        let wanted = preferred
            .iter()
            .copied()
            .fold(0_u32, u32::saturating_add)
            .saturating_add(gaps);
        if wanted <= available {
            preferred.clone()
        } else {
            partition_axis(available, gap, &preferred)
        }
    } else if super::layout_tokens::is_navigation_row(widget.text.as_deref())
        && preferred.len() == 3
    {
        let accessory = u32::from(theme.metrics.control_height).min(available / 3);
        vec![
            accessory,
            available
                .saturating_sub(accessory.saturating_mul(2))
                .saturating_sub(gap.saturating_mul(2)),
            accessory,
        ]
    } else if distribute {
        let fixed = widget
            .children()
            .iter()
            .filter(|child| {
                !empty_status_bar(tree, **child) && !is_dialog(tree, **child)
                    && !is_notification(tree, **child)
                    && !is_tooltip(tree, **child)
            })
            .map(|child| {
                tree.get(*child)
                    .is_some_and(|child| super::layout_tokens::equal_fixed(child.text.as_deref()))
            })
            .collect::<Vec<_>>();
        partition_distribute(available, gap, &preferred, &fixed)
    } else if widget.kind == WidgetKind::Row
        && let Some(columns) = widget
            .parent
            .and_then(|parent| tree.get(parent))
            .filter(|parent| parent.kind == WidgetKind::Table)
            .and_then(|parent| super::layout_tokens::table_columns(parent.text.as_deref()))
        && columns.len() == preferred.len()
    {
        partition_weighted(available, gap, &columns)
    } else if widget.kind == WidgetKind::Row
        && super::layout_tokens::is_resource_columns(widget.text.as_deref())
        && preferred.len() == 5
    {
        partition_resource_columns(available, gap)
    } else if widget.kind == WidgetKind::SplitPane
        || (is_row && widget.kind == WidgetKind::Row && widget.text.as_deref().and_then(split_spec).is_some())
    {
        partition_split(
            available,
            gap,
            &preferred,
            widget.text.as_deref().and_then(split_spec),
            is_row,
        )
    } else if matches!(
        widget.kind,
        WidgetKind::List | WidgetKind::Table | WidgetKind::Tree
    ) {
        preferred
    } else {
        partition_flex(available, gap, &preferred, &flex)
    };
    let used = lengths
        .iter()
        .copied()
        .fold(0_u32, u32::saturating_add)
        .saturating_add(
            gap.saturating_mul(u32::try_from(lengths.len().saturating_sub(1)).unwrap_or(u32::MAX)),
        );
    let mut cursor = if is_row {
        if super::layout_tokens::aligns_actions_end(widget.text.as_deref()) {
            content
                .origin
                .x
                .saturating_add_unsigned(content.size.width.saturating_sub(used))
        } else if super::layout_tokens::centers_children(widget.text.as_deref()) {
            content
                .origin
                .x
                .saturating_add_unsigned(content.size.width.saturating_sub(used) / 2)
        } else {
            content.origin.x
        }
    } else if super::layout_tokens::centers_vertical(widget.text.as_deref()) {
        content
            .origin
            .y
            .saturating_add_unsigned(content.size.height.saturating_sub(used) / 2)
    } else {
        content.origin.y
    };
    let trailing_action_offset =
        if is_row && super::layout_tokens::aligns_trailing_action(widget.text.as_deref()) {
            available.saturating_sub(used)
        } else {
            0
        };
    let last_child = children.last().copied();
    for (child_index, (child, length)) in children.iter().zip(lengths).enumerate() {
        if last_child == Some(*child) {
            cursor = cursor.saturating_add_unsigned(trailing_action_offset);
        }
        // A standalone tab strip and its following page share an edge.
        // Give the page the reserved gap so later siblings keep their positions.
        let joins_tabs = !is_row
            && child_index > 0
            && tree.get(children[child_index - 1]).is_some_and(|previous| {
                previous.kind == WidgetKind::Tabs
                    && !previous
                        .children()
                        .iter()
                        .filter_map(|id| tree.get(*id))
                        .any(|child| super::layout_tokens::is_tab_page(child.text.as_deref()))
            });
        let length = if joins_tabs {
            cursor = cursor.saturating_sub_unsigned(gap);
            length.saturating_add(gap)
        } else {
            length
        };
        let mut child_rect = if is_row {
            Rect {
                origin: Point {
                    x: cursor,
                    y: content.origin.y,
                },
                size: Size {
                    width: length,
                    height: content.size.height,
                },
            }
        } else {
            let reference_card = super::layout_tokens::is_reference_card(widget.text.as_deref())
                || (widget.kind == WidgetKind::GroupBox
                    && widget
                        .parent
                        .and_then(|parent| tree.get(parent))
                        .is_some_and(|parent| {
                            super::layout_tokens::is_settings_page(parent.text.as_deref())
                        }));
            let explicit_width = tree
                .get(*child)
                .and_then(|candidate| super::layout_tokens::fixed_width(candidate.text.as_deref()))
                .map(|width| width.min(content.size.width));
            let width = if let Some(width) = explicit_width {
                width
            } else if reference_card {
                tree.get(*child).map_or(content.size.width, |candidate| {
                    let preferred = preferred_size(tree, *child, theme).width;
                    match candidate.kind {
                        WidgetKind::Button
                            if super::layout_tokens::is_navigation_row(
                                candidate.text.as_deref(),
                            ) =>
                        {
                            content.size.width
                        }
                        WidgetKind::Button
                        | WidgetKind::CheckBox
                        | WidgetKind::RadioButton
                        | WidgetKind::Toggle
                        | WidgetKind::Menu => preferred.min(content.size.width),
                        WidgetKind::TextField
                        | WidgetKind::PasswordField
                        | WidgetKind::Slider
                        | WidgetKind::ScrollBar
                        | WidgetKind::ProgressBar => preferred
                            .max(u32::from(theme.metrics.control_height).saturating_mul(9))
                            .min(u32::from(theme.metrics.control_height).saturating_mul(15))
                            .min(content.size.width),
                        _ => content.size.width,
                    }
                })
            } else {
                content.size.width
            };
            Rect {
                origin: Point {
                    x: content.origin.x,
                    y: cursor,
                },
                size: Size {
                    width,
                    height: length,
                },
            }
        };
        // A fixed-height accessory keeps its cross-axis contract inside a row,
        // including compact status controls. Center it without changing siblings.
        if is_row && let Some(height) = tree.get(*child)
            .and_then(|child| super::layout_tokens::fixed_height(child.text.as_deref()))
        {
            let height = height.min(child_rect.size.height);
            child_rect.origin.y = child_rect.origin.y.saturating_add_unsigned(child_rect.size.height.saturating_sub(height) / 2);
            child_rect.size.height = height;
        }
        // Ordinary action buttons keep their intrinsic height even beside
        // multiline headings. Only explicitly expansive controls stretch.
        if is_row && widget.kind == WidgetKind::Row && (widget.text.as_deref().and_then(split_spec).is_some() || super::layout_tokens::aligns_actions_end(widget.text.as_deref())) && tree.get(*child).is_some_and(|candidate|
            (candidate.kind == WidgetKind::Menu || (candidate.kind == WidgetKind::Button && candidate.children().is_empty()))
            && !candidate.state.contains(WidgetState::EXPANSIVE)) {
            let height = preferred_size(tree, *child, theme).height.min(child_rect.size.height);
            child_rect.origin.y = child_rect.origin.y.saturating_add_unsigned(child_rect.size.height.saturating_sub(height)/2);
            child_rect.size.height = height;
        }
        if is_row
            && widget.kind == WidgetKind::ToolBar
            && tree.get(*child).is_some_and(|candidate| {
                matches!(candidate.kind, WidgetKind::Button | WidgetKind::Menu)
                    && !candidate.state.contains(WidgetState::START_ALIGNED)
            })
        {
            let preferred = preferred_size(tree, *child, theme);
            let side = preferred
                .width
                .min(preferred.height)
                .min(child_rect.size.width)
                .min(child_rect.size.height);
            child_rect.origin.y = child_rect
                .origin
                .y
                .saturating_add_unsigned(child_rect.size.height.saturating_sub(side) / 2);
            child_rect.size.width = if tree.get(*child).is_some_and(|candidate| candidate.kind == WidgetKind::Menu && candidate.icon.is_some_and(|icon| !matches!(icon, rice_ui::IconId::Menu | rice_ui::IconId::ChevronDown))) {
                preferred.width.min(child_rect.size.width)
            } else { side };
            child_rect.size.height = side;
        }
        if is_row
            && widget.kind == WidgetKind::Tabs
            && tree.get(*child).is_some_and(|candidate| {
                super::layout_tokens::is_window_control(candidate.text.as_deref())
            })
        {
            let side = 28.min(child_rect.size.width).min(rect.size.height);
            child_rect.origin.y = rect.origin.y.saturating_add_unsigned(2);
            child_rect.size.width = side;
            child_rect.size.height = side;
        }
        if is_row
            && super::layout_tokens::centers_setting_choice_control(widget.text.as_deref())
            && tree
                .get(*child)
                .is_some_and(|candidate| candidate.kind == WidgetKind::Button)
        {
            let height = u32::from(theme.metrics.control_height)
                .max(32)
                .min(child_rect.size.height);
            child_rect.origin.y = child_rect
                .origin
                .y
                .saturating_add_unsigned(child_rect.size.height.saturating_sub(height) / 2);
            child_rect.size.height = height;
        }
        // Browser tabs keep a deliberate leading gutter before the favicon.
        // The tab surface itself still begins at `cursor`, so selection and
        // hit testing remain one continuous target around all three children.
        if child_index == 0
            && is_row
            && super::layout_tokens::is_browser_tab(widget.text.as_deref())
        {
            let leading = gap.min(child_rect.size.width);
            child_rect.origin.x = child_rect.origin.x.saturating_add_unsigned(leading);
            child_rect.size.width = child_rect.size.width.saturating_sub(leading);
        }
        if mirror_split {
            // Move the child before recursion; never reflect editor/terminal
            // paint commands or their local pointer coordinates.
            let mirrored = i64::from(content.origin.x) * 2
                + i64::from(content.size.width)
                - i64::from(child_rect.origin.x) - i64::from(child_rect.size.width);
            child_rect.origin.x = mirrored.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        }
        if super::layout_measurements::compact_form_factor() && (child_rect.size.width == 0 || child_rect.size.height == 0) { continue; }
        if compact_navigation && children.first() == Some(child) {
            layout_compact_navigation(tree, *child, child_rect, theme, geometry);
        } else {
            layout_widget(tree, *child, child_rect, theme, geometry);
        }
        cursor = cursor.saturating_add_unsigned(length.saturating_add(gap));
    }
}

fn shared_form_label_width(tree: &RetainedTree, widget: &Widget, theme: &Theme) -> u32 {
    let Some(parent) = widget.parent.and_then(|id| tree.get(id)) else {
        return widget
            .children()
            .first()
            .map_or(0, |id| preferred_size(tree, *id, theme).width);
    };
    parent
        .children()
        .iter()
        .filter_map(|row| tree.get(*row))
        .filter(|row| {
            row.kind == WidgetKind::Row
                && super::layout_tokens::is_form_control_row(row.text.as_deref())
        })
        .filter_map(|row| row.children().first())
        .map(|label| preferred_size(tree, *label, theme).width)
        .max()
        .unwrap_or(0)
}

fn layout_integrated_tabs(
    tree: &RetainedTree,
    widget: &Widget,
    children: &[WidgetId],
    content: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) -> bool {
    if widget.kind != WidgetKind::Tabs
        || !super::layout_tokens::is_integrated_tabs(widget.text.as_deref())
    {
        return false;
    }
    let Some(page) = children.iter().copied().find(|child| {
        tree.get(*child)
            .is_some_and(|child| super::layout_tokens::is_tab_page(child.text.as_deref()))
    }) else {
        return false;
    };
    let overlap = u32::from(theme.metrics.border_width.max(1)).max(2);
    let tabs = children
        .iter()
        .copied()
        .filter(|child| *child != page)
        .collect::<Vec<_>>();
    if super::layout_tokens::is_vertical_tabs(widget.text.as_deref()) {
        let strip_width = tabs
            .iter()
            .map(|child| preferred_size(tree, *child, theme).width)
            .max()
            .unwrap_or_else(|| u32::from(theme.metrics.control_height))
            .max(u32::from(theme.metrics.control_height))
            .min(content.size.width / 2);
        let right = super::layout_tokens::vertical_tabs_on_right(widget.text.as_deref());
        let page_width = content
            .size
            .width
            .saturating_sub(strip_width)
            .saturating_add(overlap);
        layout_widget(
            tree,
            page,
            Rect {
                origin: Point {
                    x: if right {
                        content.origin.x
                    } else {
                        content
                            .origin
                            .x
                            .saturating_add_unsigned(strip_width.saturating_sub(overlap))
                    },
                    y: content.origin.y,
                },
                size: Size {
                    width: page_width,
                    height: content.size.height,
                },
            },
            theme,
            geometry,
        );
        let tab_x = if right {
            content
                .origin
                .x
                .saturating_add_unsigned(content.size.width.saturating_sub(strip_width))
        } else {
            content.origin.x
        };
        let mut offset = 0_u32;
        for tab in tabs {
            let height = preferred_size(tree, tab, theme)
                .height
                .max(u32::from(theme.metrics.control_height))
                .min(content.size.height.saturating_sub(offset));
            layout_widget(
                tree,
                tab,
                Rect {
                    origin: Point {
                        x: tab_x,
                        y: content.origin.y.saturating_add_unsigned(offset),
                    },
                    size: Size {
                        width: strip_width,
                        height,
                    },
                },
                theme,
                geometry,
            );
            offset = offset.saturating_add(height);
        }
    } else {
        let strip_height = u32::from(theme.metrics.control_height).min(content.size.height);
        layout_widget(
            tree,
            page,
            Rect {
                origin: Point {
                    x: content.origin.x,
                    y: content
                        .origin
                        .y
                        .saturating_add_unsigned(strip_height.saturating_sub(overlap)),
                },
                size: Size {
                    width: content.size.width,
                    height: content
                        .size
                        .height
                        .saturating_sub(strip_height)
                        .saturating_add(overlap),
                },
            },
            theme,
            geometry,
        );
        let trailing = super::layout_tokens::aligns_trailing_action(widget.text.as_deref());
        let last = tabs.last().copied();
        let mut offset = 0_u32;
        for tab in tabs {
            let icon_only = tree.get(tab).is_some_and(|item| {
                matches!(item.kind, WidgetKind::Button | WidgetKind::Menu)
                    && item.icon.is_some()
                    && item.text.as_deref().is_none_or(str::is_empty)
            });
            let width = if icon_only {
                strip_height
            } else {
                preferred_size(tree, tab, theme).width.min(220)
            }
            .min(content.size.width);
            let tab_x = if trailing && Some(tab) == last {
                content
                    .origin
                    .x
                    .saturating_add_unsigned(content.size.width.saturating_sub(width))
            } else {
                content.origin.x.saturating_add_unsigned(offset)
            };
            let inset = if trailing && Some(tab) == last && icon_only
                && tree.get(tab).is_some_and(|item| item.kind == WidgetKind::Menu)
                && widget.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "compact-actions"))
            { 2_u32.min(width / 4).min(strip_height / 4) } else { 0 };
            layout_widget(
                tree,
                tab,
                Rect {
                    origin: Point {
                        x: tab_x.saturating_add_unsigned(inset),
                        y: content.origin.y.saturating_add_unsigned(inset),
                    },
                    size: Size {
                        width: width.saturating_sub(inset * 2),
                        height: strip_height.saturating_sub(inset * 2),
                    },
                },
                theme,
                geometry,
            );
            if !(trailing && Some(tab) == last) {
                offset = offset.saturating_add(width);
            }
        }
    }
    true
}

fn scroll_metrics(text: Option<&str>) -> (u32, Option<u32>) {
    let Some((position, extent)) = super::layout_tokens::scroll_value(text)
        .unwrap_or_default()
        .split_once('/')
    else {
        return (0, None);
    };
    (
        position.trim().parse().unwrap_or_default(),
        extent.trim().parse().ok(),
    )
}

fn scroll_child_extent(
    tree: &RetainedTree,
    child: WidgetId,
    width: u32,
    preferred: u32,
    explicit: Option<u32>,
    theme: &Theme,
) -> u32 {
    if let Some(extent) = super::layout_measurements::navigation_grid_extent(child) {
        return extent;
    }
    let Some(widget) = tree.get(child) else {
        return preferred.max(explicit.unwrap_or_default());
    };
    if widget.kind == WidgetKind::IconGrid
        && !widget
            .text
            .as_deref()
            .is_some_and(|text| text == "desktop-grid" || text.starts_with("desktop-grid:"))
    {
        return icon_grid_extent(tree, child, width, theme);
    }
    if is_responsive_document(tree, child)
        || matches!(widget.kind, WidgetKind::Table | WidgetKind::List)
    {
        // List/table rows use width-dependent heights during allocation. Their
        // scroll range must include wrapped cells rather than natural heights.
        let responsive = preferred_height_for_width(tree, child, width, theme);
        if super::layout_tokens::is_store_document(widget.text.as_deref()) {
            responsive.max(explicit.unwrap_or_default())
        } else {
            responsive.max(preferred).max(explicit.unwrap_or_default())
        }
    } else {
        preferred.max(explicit.unwrap_or_default())
    }
}

#[derive(Clone, Copy)]
pub(super) struct ScrollViewGeometry {
    pub track: Rect,
    pub position: u32,
    pub viewport_extent: u32,
    pub content_extent: u32,
}

pub(super) fn scroll_view_viewport(widget: &rice_ui::Widget, rect: Rect, theme: &Theme) -> Rect {
    let inset = if let Some(padding) = super::layout_tokens::padding(widget.text.as_deref()) { padding }
        else if super::layout_tokens::has_token(widget.text.as_deref(), "flush") { 0 }
        else { u32::from(theme.spacing.control_y) };
    rect.inset(Insets::all(inset))
}

pub(super) fn scroll_view_geometry(
    tree: &RetainedTree,
    widget: WidgetId,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
) -> Option<ScrollViewGeometry> {
    let retained = tree.get(widget)?;
    if retained.kind != WidgetKind::ScrollView {
        return None;
    }
    if retained.text.as_deref().is_some_and(|text| text.split(':').any(|part| part == "document-viewport")) { return None; }
    let rect = geometry.get(&widget).copied()?;
    let viewport = scroll_view_viewport(retained, rect, theme);
    let content = retained
        .children()
        .first()
        .and_then(|child| geometry.get(child))?;
    if super::layout_tokens::virtual_window_offset(retained.text.as_deref()).is_some()
        || super::layout_tokens::is_editor_viewport(retained.text.as_deref()) {
        let (requested_position, explicit_extent) = scroll_metrics(retained.text.as_deref());
        let content_extent = explicit_extent?.max(viewport.size.height);
        if content_extent <= viewport.size.height {
            return None;
        }
        let width = u32::from(theme.metrics.scrollbar_width).min(viewport.size.width);
        return Some(ScrollViewGeometry {
            track: Rect {
                origin: Point {
                    x: viewport
                        .origin
                        .x
                        .saturating_add_unsigned(viewport.size.width.saturating_sub(width)),
                    y: viewport.origin.y,
                },
                size: Size {
                    width,
                    height: viewport.size.height,
                },
            },
            position: requested_position.min(content_extent.saturating_sub(viewport.size.height)),
            viewport_extent: viewport.size.height,
            content_extent,
        });
    }
    let content_extent = content.size.height;
    if content_extent <= viewport.size.height {
        return None;
    }
    let width = u32::from(theme.metrics.scrollbar_width).min(viewport.size.width);
    Some(ScrollViewGeometry {
        track: Rect {
            origin: Point {
                x: viewport
                    .origin
                    .x
                    .saturating_add_unsigned(viewport.size.width.saturating_sub(width)),
                y: viewport.origin.y,
            },
            size: Size {
                width,
                height: viewport.size.height,
            },
        },
        position: viewport
            .origin
            .y
            .saturating_sub(content.origin.y)
            .max(0)
            .cast_unsigned(),
        viewport_extent: viewport.size.height,
        content_extent,
    })
}

fn inline_menu_content(tree: &RetainedTree, widget: &rice_ui::Widget) -> bool {
    widget.kind == WidgetKind::MenuItem && widget.children().iter().any(|child| {
        tree.get(*child).is_some_and(|child| !matches!(child.kind, WidgetKind::Menu | WidgetKind::Tooltip))
    })
}

pub(super) fn layout_menu_popup(
    tree: &RetainedTree,
    menu: WidgetId,
    viewport: Size,
    popup_anchor: Option<Point>,
    scroll_row: usize,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    let Some(widget) = tree.get(menu) else {
        return;
    };
    let anchor = popup_anchor.map_or_else(
        || geometry.get(&menu).copied(),
        |point| {
            Some(Rect {
                origin: point,
                size: Size::default(),
            })
        },
    );
    let Some(anchor) = anchor else { return };
    let gap = 0;
    let children = menu_content_children(tree, widget);
    let compound_content = children.iter().any(|child| tree.get(*child).is_some_and(|child| {
        matches!(child.kind, WidgetKind::Column | WidgetKind::Row | WidgetKind::Container | WidgetKind::ScrollView)
    }));
    let preferred = children
        .iter()
        .map(|child| preferred_size(tree, *child, theme))
        .collect::<Vec<_>>();
    let content_width = preferred
        .iter()
        .map(|size| size.width)
        .max()
        .unwrap_or(0)
        .max(u32::from(theme.metrics.control_height).saturating_mul(10));
    let choice_control = widget
        .state
        .contains(WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON)
        && anchor.size.width > 0;
    let popup_padding = if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
        REDMOND_MENU_POPUP_PADDING
    } else {
        MENU_POPUP_PADDING
    };
    let width = if choice_control {
        anchor.size.width
    } else {
        content_width.saturating_add(popup_padding.saturating_mul(2))
    }
    .min(viewport.width);
    let menu_bar_dropdown = popup_anchor.is_none()
        && widget
            .parent
            .and_then(|parent| tree.get(parent))
            .is_some_and(|parent| parent.kind == WidgetKind::MenuBar);
    let below = anchor.origin.y.saturating_add_unsigned(anchor.size.height);
    let available_below = viewport.height.saturating_sub(below.max(0).cast_unsigned());
    let compact = super::layout_measurements::compact_form_factor();
    let content_budget = if compact { viewport.height } else { available_below }
        .saturating_sub(popup_padding * 2);
    let mut height_rows = 0;
    let mut used_height = 0_u32;
    for size in &preferred {
        if height_rows > 0 && used_height.saturating_add(size.height) > content_budget {
            break;
        }
        used_height = used_height.saturating_add(size.height);
        height_rows += 1;
    }
    let maximum_scroll_row = maximum_menu_scroll_row(tree, menu).max(if menu_bar_dropdown || compact {
        preferred.len().saturating_sub(height_rows)
    } else {
        0
    });
    let scroll_row = scroll_row.min(maximum_scroll_row);
    let mut visible = Vec::new();
    let mut visible_commands = 0_usize;
    let mut visible_height = 0_u32;
    for (index, size) in preferred.iter().enumerate().skip(scroll_row) {
        if (menu_bar_dropdown || compact)
            && !visible.is_empty()
            && visible_height.saturating_add(size.height) > content_budget
        {
            break;
        }
        visible_height = visible_height.saturating_add(size.height);
        let command = is_menu_command(tree, children[index]);
        if maximum_scroll_row > 0 && command && visible_commands == MAX_MENU_POPUP_ROWS {
            break;
        }
        visible.push((index, size));
        visible_commands = visible_commands.saturating_add(usize::from(command));
    }
    let wanted_content_height = visible
        .iter()
        .map(|(_, size)| size.height)
        .fold(0_u32, u32::saturating_add);
    let wanted_height = wanted_content_height
        .saturating_add(popup_padding.saturating_mul(2))
        .min(viewport.height);
    let x = anchor
        .origin
        .x
        .min(i32::try_from(viewport.width.saturating_sub(width)).unwrap_or(i32::MAX))
        .max(0);
    // Keep dropdowns below their menu-bar buttons; tall menus scroll.
    let popup_y = if (menu_bar_dropdown && !compact) || available_below >= wanted_height {
        below
    } else {
        anchor
            .origin
            .y
            .saturating_sub_unsigned(wanted_height.min(anchor.origin.y.max(0).cast_unsigned()))
    }
    .max(0);
    let popup_y = if compact { popup_y.min(viewport.height.saturating_sub(wanted_height) as i32) } else { popup_y };
    for child in widget.children() {
        geometry.remove(child);
    }
    let mut y = popup_y.saturating_add_unsigned(popup_padding);
    // Compound popup content (palettes and galleries) can be taller than the
    // window. Allocate only the visible viewport; an embedded ScrollView owns
    // its content extent and scrolling rather than overflowing the popup.
    let bottom = viewport.height.saturating_sub(popup_padding);
    for (index, size) in visible {
        let child = children[index];
        let remaining = bottom.saturating_sub(y.max(0).cast_unsigned());
        let length = if compound_content { size.height.min(remaining) } else { size.height };
        if length == 0 { break; }
        let rect = Rect {
            origin: Point {
                x: x.saturating_add_unsigned(popup_padding),
                y,
            },
            size: Size {
                width: width.saturating_sub(popup_padding.saturating_mul(2)),
                height: length,
            },
        };
        layout_widget(tree, child, rect, theme, geometry);
        y = y.saturating_add_unsigned(length.saturating_add(gap));
    }
}

pub(super) fn layout_submenu_popup(
    tree: &RetainedTree,
    menu: WidgetId,
    parent_item: WidgetId,
    viewport: Size,
    scroll_row: usize,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    let Some(widget) = tree.get(menu) else { return };
    let Some(anchor) = geometry.get(&parent_item).copied() else {
        return;
    };
    let children = menu_content_children(tree, widget);
    let preferred = children
        .iter()
        .map(|child| preferred_size(tree, *child, theme))
        .collect::<Vec<_>>();
    let content_width = preferred
        .iter()
        .map(|size| size.width)
        .max()
        .unwrap_or(0)
        .max(u32::from(theme.metrics.control_height).saturating_mul(10));
    let popup_padding = if theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
        REDMOND_MENU_POPUP_PADDING
    } else {
        MENU_POPUP_PADDING
    };
    let width = content_width
        .saturating_add(popup_padding.saturating_mul(2))
        .min(viewport.width);
    let compact = super::layout_measurements::compact_form_factor();
    let scroll_row = if compact { scroll_row.min(children.len().saturating_sub(1)) } else { 0 };
    let mut visible_count = preferred.len();
    if compact {
        visible_count = 0;
        let budget = viewport.height.saturating_sub(popup_padding * 2);
        let mut used = 0_u32;
        for size in preferred.iter().skip(scroll_row) {
            if used.saturating_add(size.height) > budget { break; }
            used += size.height;
            visible_count += 1;
        }
    }
    let height = preferred
        .iter().skip(scroll_row).take(visible_count)
        .map(|size| size.height)
        .fold(0_u32, u32::saturating_add)
        .saturating_add(popup_padding.saturating_mul(2))
        .min(viewport.height);
    let owner_popup = tree
        .get(parent_item)
        .and_then(|item| item.parent)
        .and_then(|menu| super::render::menu_popup_rect(tree, menu, geometry, theme));
    let command_gap = (theme.chrome_profile == rice_ui::ChromeProfile::CommandDeck)
        .then_some(4_u32)
        .unwrap_or_default();
    let anchor_right = if command_gap > 0 {
        owner_popup.map_or_else(
            || anchor.origin.x.saturating_add_unsigned(anchor.size.width),
            |popup| {
                popup
                    .origin
                    .x
                    .saturating_add_unsigned(popup.size.width)
                    .saturating_add_unsigned(command_gap)
            },
        )
    } else {
        anchor.origin.x.saturating_add_unsigned(anchor.size.width)
    };
    let right_space = i32::try_from(viewport.width)
        .unwrap_or(i32::MAX)
        .saturating_sub(anchor_right)
        .max(0)
        .cast_unsigned();
    let left_edge = if command_gap > 0 {
        owner_popup.map_or(anchor.origin.x, |popup| popup.origin.x)
    } else {
        anchor.origin.x
    };
    let left_x = left_edge
        .saturating_sub_unsigned(width.saturating_add(command_gap))
        .max(0);
    let left_space = left_edge.max(0).cast_unsigned();
    let inherited_left =
        tree.get(parent_item)
            .and_then(|item| item.parent)
            .and_then(|owner_menu| {
                let owner_popup =
                    super::render::menu_popup_rect(tree, owner_menu, geometry, theme)?;
                let owner_parent = tree.get(owner_menu)?.parent?;
                let owner_anchor = geometry.get(&owner_parent)?;
                Some(owner_popup.origin.x < owner_anchor.origin.x)
            });
    let opens_left = match inherited_left {
        Some(true) => left_space >= width || right_space < width,
        Some(false) => right_space < width && left_space >= width,
        None => right_space < width && left_space >= width,
    };
    let x = if opens_left { left_x } else { anchor_right }
        .min(i32::try_from(viewport.width.saturating_sub(width)).unwrap_or(i32::MAX))
        .max(0);
    let y = anchor
        .origin
        .y
        .min(i32::try_from(viewport.height.saturating_sub(height)).unwrap_or(i32::MAX))
        .max(0);
    for child in widget.children() { geometry.remove(child); }
    let mut cursor = y.saturating_add_unsigned(popup_padding);
    for (child, size) in children.iter().zip(preferred).skip(scroll_row).take(visible_count) {
        let rect = Rect {
            origin: Point {
                x: x.saturating_add_unsigned(popup_padding),
                y: cursor,
            },
            size: Size {
                width: width.saturating_sub(popup_padding.saturating_mul(2)),
                height: size.height,
            },
        };
        layout_widget(tree, *child, rect, theme, geometry);
        cursor = cursor.saturating_add_unsigned(size.height);
    }
}

pub(super) fn estimated_text_width(text: &str, theme: &Theme) -> u32 {
    estimated_text_width_at_size(text, theme.typography.body_px)
}

pub(super) fn estimated_classic_text_width(text: &str, size: u16) -> u32 {
    text.lines()
        .map(|line| {
            // These advances are the measured 400-weight RiceWM Sans glyph
            // widths at the classic theme's 11 px body size. Keep these in
            // sync with the actual `ricewm-sans.woff2`, not Manrope: using the
            // latter clipped selected tree captions such as "My Computer".
            // Accumulate first and scale once so rounding error cannot make a
            // selected Win98 caption expose white text beyond its blue label.
            let advances = line.chars().fold(0_u32, |width, character| {
                let weight = match character {
                    'A' | 'B' | 'C' | 'D' | 'H' | 'N' | 'R' | 'U' | 'V' | 'X' | 'Z' => 8,
                    'E' | 'K' | 'L' => 7,
                    'F' => 6,
                    'G' | 'M' | 'O' | 'Q' => 9,
                    'I' | 'J' => 3,
                    'P' | 'S' | 'T' | 'Y' => 7,
                    'W' => 11,
                    'a' | 'b' | 'd' | 'e' | 'g' | 'h' | 'n' | 'o' | 'p' | 'q' | 'u' | 'x' | 'y' => {
                        7
                    }
                    'c' | 'k' | 's' | 'z' => 6,
                    'f' | 't' => 4,
                    'i' | 'j' | 'l' => 3,
                    'm' => 11,
                    'r' => 5,
                    'v' => 7,
                    'w' => 9,
                    '0'..='9' => 7,
                    ' ' | '.' | '-' | '!' | '|' => 4,
                    '_' => 6,
                    '…' => 11,
                    value if is_cjk_full_width(value) => 11,
                    _ => 7,
                };
                width.saturating_add(weight)
            });
            advances.saturating_mul(u32::from(size)).saturating_add(5) / 11
        })
        .max()
        .unwrap_or_default()
}

fn estimated_widget_text_width(widget: &Widget, text: &str, theme: &Theme) -> u32 {
    if widget.state.contains(rice_ui::WidgetState::BRAND_TYPE) {
        let size = u32::from(super::layout_tokens::preferred_text_size(widget, theme));
        let width = text
            .lines()
            .map(|line| {
                line.chars().fold(0_u32, |width, character| {
                    let weight = match character {
                        'M' | 'W' | 'm' | 'w' => 90,
                        'I' | 'i' | 'l' | '!' => 30,
                        ' ' => 32,
                        _ => 52,
                    };
                    width.saturating_add(size.saturating_mul(weight) / 100)
                })
            })
            .max()
            .unwrap_or_default();
        if widget.state.contains(rice_ui::WidgetState::DISPLAY) {
            width
        } else {
            width.saturating_mul(7) / 5
        }
    } else {
        estimated_text_width_at_size(
            text,
            super::layout_tokens::preferred_text_size(widget, theme),
        )
    }
}

fn is_cjk_full_width(character: char) -> bool {
    matches!(character as u32, 0x1100..=0x11ff | 0x2e80..=0x9fff | 0xa960..=0xa97f | 0xac00..=0xd7ff | 0xf900..=0xfaff | 0xfe10..=0xfe1f | 0xfe30..=0xfe4f | 0xff01..=0xff60 | 0xffe0..=0xffe6 | 0x20000..=0x3ffff)
}

fn estimated_text_width_at_size(text: &str, size: u16) -> u32 {
    text.lines().map(|line| {
        line.chars().fold(0_u32, |width, character| {
            width.saturating_add(if is_cjk_full_width(character) { 100 } else { TEXT_WIDTH_PERCENT })
        }).saturating_mul(u32::from(size)) / 100
    }).max().unwrap_or(0)
}

#[cfg(test)]
mod locale_measurement_tests {
    use super::*;
    #[test]
    fn chinese_labels_reserve_full_glyph_width_in_both_layouts() {
        assert_eq!(estimated_text_width_at_size("保存", 14), 28);
        assert_eq!(estimated_text_width_at_size("저장", 14), 28);
        assert_eq!(estimated_classic_text_width("저장", 11), 22);
        assert_eq!(estimated_classic_text_width("保存", 11), 22);
        assert_eq!(estimated_text_width_at_size("Save", 14), 40);
    }
}

fn layout_icon_grid(
    tree: &RetainedTree,
    children: &[WidgetId],
    content: Rect,
    theme: &Theme,
    zoom: u32,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    if children.is_empty() {
        return;
    }
    let metrics = icon_grid_metrics(tree, children, content.size.width, theme, zoom);
    for (index, child) in children.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let column = index % metrics.columns;
        let row = index / metrics.columns;
        let rect = Rect {
            origin: Point {
                x: content.origin.x.saturating_add_unsigned(
                    column.saturating_mul(metrics.cell_width.saturating_add(metrics.gap)),
                ),
                y: content.origin.y.saturating_add_unsigned(
                    row.saturating_mul(metrics.cell_height.saturating_add(metrics.gap)),
                ),
            },
            size: Size {
                width: metrics.cell_width,
                height: metrics.cell_height,
            },
        };
        layout_widget(tree, *child, rect, theme, geometry);
    }
}

#[derive(Clone, Copy)]
struct IconGridMetrics {
    gap: u32,
    columns: u32,
    cell_width: u32,
    cell_height: u32,
}

fn icon_grid_metrics(
    tree: &RetainedTree,
    children: &[WidgetId],
    width: u32,
    theme: &Theme,
    zoom: u32,
) -> IconGridMetrics {
    let app_grid = children
        .first()
        .and_then(|child| tree.get(*child))
        .and_then(|child| child.parent)
        .and_then(|parent| tree.get(parent))
        .is_some_and(|parent| super::layout_tokens::is_app_grid(parent.text.as_deref()));
    let file_grid = children
        .first()
        .and_then(|child| tree.get(*child))
        .and_then(|child| child.parent)
        .and_then(|parent| tree.get(parent))
        .is_some_and(|parent| super::layout_tokens::is_file_grid(parent.text.as_deref()));
    if app_grid {
        let grid_text = children.first().and_then(|child| tree.get(*child))
            .and_then(|child| child.parent).and_then(|parent| tree.get(parent))
            .and_then(|parent| parent.text.as_deref());
        let card_minimum = grid_text.and_then(|text| text.split(':').find_map(|part| part.strip_prefix("min-cell=")))
            .and_then(|value| value.parse::<u32>().ok()).filter(|value| (160..=600).contains(value));
        let gap = if card_minimum.is_some() { 16 } else { u32::from(theme.spacing.unit).max(8) };
        let store_grid = children
            .first()
            .and_then(|child| tree.get(*child))
            .and_then(|child| child.parent)
            .and_then(|parent| tree.get(parent))
            .is_some_and(|parent| super::layout_tokens::is_store_grid(parent.text.as_deref()));
        if store_grid {
            // Use all available window space. Tiles grow between column
            // thresholds, then reflow so a wider store shows more applications.
            let columns = (width.saturating_add(gap) / (260 + gap)).max(1);
            let cell_width = width.saturating_sub(gap * (columns - 1)) / columns;
            let control = u32::from(theme.metrics.control_height);
            return IconGridMetrics {
                gap,
                columns,
                cell_width,
                cell_height: control.saturating_mul(super::layout_tokens::app_grid_rows(grid_text).clamp(6, 16)),
            };
        }
        let columns: u32 = card_minimum.map_or(if width >= 680 { 2 } else { 1 },
            |minimum| (width.saturating_add(gap) / minimum.saturating_add(gap)).clamp(1, 2));
        let total_gaps = gap.saturating_mul(columns.saturating_sub(1));
        return IconGridMetrics {
            gap,
            columns,
            cell_width: width.saturating_sub(total_gaps) / columns,
            cell_height: u32::from(theme.metrics.control_height).saturating_mul(
                children
                    .first()
                    .and_then(|child| tree.get(*child))
                    .and_then(|child| child.parent)
                    .and_then(|parent| tree.get(parent))
                    .map_or(4, |parent| {
                        super::layout_tokens::app_grid_rows(parent.text.as_deref())
                    }),
            ),
        };
    }
    if file_grid && theme.chrome_profile == rice_ui::ChromeProfile::Redmond98 {
        // Explorer's icon view uses a fixed grid origin and pitch. Dividing
        // the available width between a content-dependent column count made
        // the first icon visibly jump sideways when navigation changed the
        // longest filename or the number of entries.
        let cell_width = u32::try_from(
            u64::from(120_u32)
                .saturating_mul(u64::from(zoom))
                .div_ceil(1_000),
        )
        .unwrap_or(u32::MAX)
        .max(1)
        .min(width.max(1));
        let columns = (width / cell_width).max(1);
        return IconGridMetrics {
            gap: 0,
            columns,
            cell_width,
            cell_height: u32::from(theme.metrics.control_height)
                .saturating_mul(4)
                .saturating_mul(zoom)
                .div_ceil(1_000)
                .max(1),
        };
    }
    let gap = if file_grid {
        2
    } else {
        u32::from(theme.spacing.unit)
    };
    let scale = |value: u32| {
        u32::try_from(u64::from(value).saturating_mul(u64::from(zoom)) / 1_000)
            .unwrap_or(u32::MAX)
            .max(1)
    };
    let minimum_factor = if file_grid { 4 } else { 3 };
    // Keep the default file-grid density aligned with interactive controls,
    // but grow zoom above 100% on the icon scale. Touch-target increases must
    // not be multiplied fivefold into huge single-column thumbnails.
    let scaled_grid_metric = |factor: u32| {
        let control = u32::from(theme.metrics.control_height).saturating_mul(factor);
        if !file_grid || zoom <= 1_000 {
            return scale(control);
        }
        let growth = u64::from(theme.metrics.icon_medium)
            .saturating_mul(u64::from(factor))
            .saturating_mul(u64::from(zoom.saturating_sub(1_000)))
            / 1_000;
        control.saturating_add(u32::try_from(growth).unwrap_or(u32::MAX))
    };
    let minimum = scaled_grid_metric(minimum_factor);
    let maximum = scaled_grid_metric(5);
    let natural = children
        .iter()
        .map(|child| preferred_size(tree, *child, theme).width)
        .max()
        .unwrap_or(minimum)
        .max(minimum)
        .min(maximum);
    let columns = (width.saturating_add(gap) / natural.saturating_add(gap)).max(1);
    let total_gaps = gap.saturating_mul(columns.saturating_sub(1));
    IconGridMetrics {
        gap,
        columns,
        cell_width: width.saturating_sub(total_gaps) / columns,
        cell_height: scaled_grid_metric(if file_grid { 4 } else { 2 }),
    }
}

fn icon_grid_extent(tree: &RetainedTree, grid: WidgetId, width: u32, theme: &Theme) -> u32 {
    let Some(widget) = tree.get(grid) else {
        return 0;
    };
    if widget.children().is_empty() {
        return 0;
    }
    let metrics = icon_grid_metrics(
        tree,
        widget.children(),
        width,
        theme,
        super::layout_tokens::zoom_permille(widget.text.as_deref()),
    );
    let count = u32::try_from(widget.children().len()).unwrap_or(u32::MAX);
    let rows = count.div_ceil(metrics.columns);
    rows.saturating_mul(metrics.cell_height)
        .saturating_add(metrics.gap.saturating_mul(rows.saturating_sub(1)))
}

fn layout_desktop_grid(
    tree: &RetainedTree,
    children: &[WidgetId],
    content: Rect,
    theme: &Theme,
    token: Option<&str>,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    // Keep rendering in lockstep with RiceDesktop's pure GridConfig. This is
    // what makes rubber-band intersection and persisted drag ordering match
    // the pixels the user sees.
    let configured = token
        .and_then(|text| text.strip_prefix("desktop-grid:"))
        .and_then(|dimensions| {
            let mut fields = dimensions.split(':');
            let columns = fields.next()?.parse::<u32>().ok()?;
            let rows = fields.next()?.parse::<u32>().ok()?;
            let cell_width = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(96);
            let cell_height = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(96);
            let horizontal_gap = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4);
            let vertical_gap = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4);
            let origin_x = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4);
            let origin_y = fields
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(4);
            Some((
                columns,
                rows,
                cell_width,
                cell_height,
                horizontal_gap,
                vertical_gap,
                origin_x,
                origin_y,
            ))
        })
        .filter(|(columns, rows, cell_width, cell_height, ..)| {
            *columns > 0 && *rows > 0 && *cell_width > 0 && *cell_height > 0
        });
    let fallback = || {
        let rows = u32::try_from(children.len().div_ceil(4).max(1)).unwrap_or(u32::MAX);
        (4, rows, 96, 96, 4, 4, 4, 4)
    };
    let (columns, rows, cell_width, cell_height, horizontal_gap, vertical_gap, origin_x, origin_y) =
        configured.unwrap_or_else(fallback);
    let cell_width = cell_width.min(content.size.width.max(1));
    let cell_height = cell_height.min(content.size.height.max(1));
    for (index, child) in children.iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let column = index / rows;
        let row = index % rows;
        if column >= columns {
            break;
        }
        let rect = Rect {
            origin: Point {
                x: content
                    .origin
                    .x
                    .saturating_add_unsigned(origin_x)
                    .saturating_add_unsigned(
                        column.saturating_mul(cell_width.saturating_add(horizontal_gap)),
                    ),
                y: content
                    .origin
                    .y
                    .saturating_add_unsigned(origin_y)
                    .saturating_add_unsigned(
                        row.saturating_mul(cell_height.saturating_add(vertical_gap)),
                    ),
            },
            size: Size {
                width: cell_width,
                height: cell_height,
            },
        };
        layout_widget(tree, *child, rect, theme, geometry);
    }
}
