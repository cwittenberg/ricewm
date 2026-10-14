//! Narrow-window adaptations shared by split-view applications.

use std::collections::BTreeMap;

use rice_native_app_protocol::WidgetId;
use rice_ui::{Insets, Point, Rect, RetainedTree, Size, Theme};

use super::layout::{layout_widget, preferred_size};
use super::layout_partition::partition_flex;

const COMPACT_SPLIT_WIDTH: u32 = 400;
const COMPACT_NAVIGATION_COLUMNS: u32 = 2;
const COMPACT_NAVIGATION_MAX_PERCENT: u32 = 45;

pub(super) fn is_compact_split(width: u32, text: Option<&str>) -> bool {
    width < super::layout_tokens::compact_split_threshold(text, if super::layout_measurements::compact_form_factor() { 600 } else { COMPACT_SPLIT_WIDTH })
}

pub(super) fn compact_navigation_list(
    tree: &RetainedTree,
    navigation: WidgetId,
) -> Option<WidgetId> {
    let navigation = tree.get(navigation)?;
    if navigation.kind == rice_ui::WidgetKind::List {
        return Some(navigation.id);
    }
    navigation.children().iter().copied().find(|child| {
        tree.get(*child)
            .is_some_and(|child| child.kind == rice_ui::WidgetKind::List)
    })
}

pub(super) fn compact_split_lengths(
    tree: &RetainedTree,
    children: &[WidgetId],
    available: u32,
    gap: u32,
    theme: &Theme,
) -> Vec<u32> {
    if children.len() != 2 {
        let preferred = children
            .iter()
            .map(|child| preferred_size(tree, *child, theme).height)
            .collect::<Vec<_>>();
        return partition_flex(available, gap, &preferred, &vec![true; children.len()]);
    }
    if super::layout_measurements::compact_form_factor() {
        let height = if super::layout_measurements::navigation_open(children[0]) { available } else { available.min(44) };
        return vec![height, available.saturating_sub(height + gap)];
    }
    let navigation = compact_navigation_list(tree, children[0]).map_or(0, |navigation| {
        let list_height = navigation_grid_extent(tree, navigation, theme);
        // A split's navigation child may wrap the actual List with a heading,
        // status, or other fixed chrome. Preserve that chrome when replacing
        // the list's ordinary vertical layout with the compact two-column grid.
        let wrapper_chrome = preferred_size(tree, children[0], theme)
            .height
            .saturating_sub(preferred_size(tree, navigation, theme).height);
        list_height.saturating_add(wrapper_chrome)
    });
    // In the stacked compact form the content pane remains primary: it must
    // retain room for at least one item instead of allowing a long places list
    // to consume the complete split axis. Additional places remain reachable
    // when the window grows.
    let navigation = navigation
        .min(available.saturating_sub(gap))
        .min(available.saturating_mul(COMPACT_NAVIGATION_MAX_PERCENT) / 100);
    vec![
        navigation,
        available.saturating_sub(navigation.saturating_add(gap)),
    ]
}

pub(super) fn layout_compact_navigation(
    tree: &RetainedTree,
    navigation: WidgetId,
    rect: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    if super::layout_measurements::compact_form_factor() {
        layout_navigation_drawer(tree, navigation, rect, theme, geometry);
        return;
    }
    let Some(list) = compact_navigation_list(tree, navigation) else {
        layout_widget(tree, navigation, rect, theme, geometry);
        return;
    };
    // Scroll allocation, hit testing and the scrollbar must use the same
    // two-column document as the cells, rather than the ordinary vertical list.
    super::layout_measurements::record_navigation_grid(list, navigation_grid_extent(tree, list, theme));
    if list == navigation {
        layout_navigation_grid(tree, list, rect, theme, geometry);
        return;
    }
    // Lay out wrapper chrome (for example a "Places" heading) normally, then
    // only replace the nested list's one-column rows with the compact grid.
    layout_widget(tree, navigation, rect, theme, geometry);
}

fn navigation_grid_extent(tree: &RetainedTree, list: WidgetId, theme: &Theme) -> u32 {
    let rows = tree.get(list).map_or(0, |widget| {
        u32::try_from(widget.children().len()).unwrap_or(u32::MAX).div_ceil(COMPACT_NAVIGATION_COLUMNS)
    });
    rows.saturating_mul(navigation_row_height(tree, list, theme))
        .saturating_add(u32::from(theme.spacing.unit).saturating_mul(rows.saturating_sub(1)))
        .saturating_add(u32::from(theme.spacing.control_y).saturating_mul(2))
}

pub(super) fn layout_navigation_grid(
    tree: &RetainedTree,
    id: WidgetId,
    rect: Rect,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    geometry.insert(id, rect);
    let Some(widget) = tree.get(id) else {
        return;
    };
    let padding = u32::from(theme.spacing.control_y);
    let gap = u32::from(theme.spacing.unit);
    let content = rect.inset(Insets::all(padding));
    let columns = COMPACT_NAVIGATION_COLUMNS.min(
        u32::try_from(widget.children().len())
            .unwrap_or(u32::MAX)
            .max(1),
    );
    let width = content
        .size
        .width
        .saturating_sub(gap.saturating_mul(columns.saturating_sub(1)))
        / columns;
    let height = navigation_row_height(tree, id, theme);
    for (index, child) in widget.children().iter().enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let column = index % columns;
        let row = index / columns;
        let offset = column.saturating_mul(width.saturating_add(gap));
        // Reflect each cell inside this semantic navigation owner, including
        // any remainder pixel. Keep logical order and each child's internals.
        let offset = if rice_i18n::direction() == rice_i18n::TextDirection::Rtl {
            content.size.width.saturating_sub(width).saturating_sub(offset)
        } else {
            offset
        };
        layout_widget(
            tree,
            *child,
            Rect {
                origin: Point {
                    x: content
                        .origin
                        .x
                        .saturating_add_unsigned(offset),
                    y: content
                        .origin
                        .y
                        .saturating_add_unsigned(row.saturating_mul(height.saturating_add(gap))),
                },
                size: Size { width, height },
            },
            theme,
            geometry,
        );
    }
}

// Flow whole controls, preserving their identity and hit geometry. Only constrained
// toolbars participate; grids, documents and positioned surfaces keep their layout.
pub(super) fn toolbar_flow(tree: &RetainedTree, id: WidgetId, width: u32, theme: &Theme) -> Option<(u32, Vec<(WidgetId, Rect)>)> {
    let widget = tree.get(id)?;
    if !super::layout_measurements::compact_form_factor() || widget.kind != rice_ui::WidgetKind::ToolBar || width >= 700 { return None; }
    let padding = u32::from(theme.spacing.control_y);
    let gap = u32::from(theme.spacing.unit);
    let available = width.saturating_sub(padding * 2).max(1);
    let children = widget.children().iter().filter_map(|id| {
        let child = tree.get(*id)?;
        if child.kind == rice_ui::WidgetKind::Tooltip { return None; }
        let mut size = preferred_size(tree, *id, theme);
        if child.kind == rice_ui::WidgetKind::TextField { size.width = size.width.max(180); }
        size.width = size.width.min(available);
        Some((*id, size))
    }).collect::<Vec<_>>();
    let natural = children.iter().map(|(_, size)| size.width).sum::<u32>()
        .saturating_add(gap * children.len().saturating_sub(1) as u32);
    if natural <= available { return None; }
    let (mut x, mut y, mut row_height) = (0_u32, padding, 0_u32);
    let mut cells = Vec::new();
    for (id, size) in children {
        if x > 0 && x.saturating_add(size.width) > available {
            x = 0; y += row_height + gap; row_height = 0;
        }
        cells.push((id, Rect { origin: Point { x: (padding + x) as i32, y: y as i32 }, size }));
        x += size.width + gap;
        row_height = row_height.max(size.height);
    }
    Some((y + row_height + padding, cells))
}

fn navigation_row_height(tree: &RetainedTree, list: WidgetId, theme: &Theme) -> u32 {
    // Composite buttons need their nested content and padding even when a
    // desktop window crosses the compact split breakpoint.
    tree.get(list).into_iter().flat_map(|widget| widget.children()).map(|id| {
        preferred_size(tree, *id, theme).height
    }).max().unwrap_or(0).max(u32::from(theme.metrics.control_height))
}

pub(super) fn layout_navigation_drawer(tree: &RetainedTree, id: WidgetId, rect: Rect, theme: &Theme, geometry: &mut BTreeMap<WidgetId, Rect>) {
    let header = Rect { origin: rect.origin, size: Size { width: rect.size.width, height: rect.size.height.min(44) } };
    super::layout_measurements::record_navigation_header(id, header);
    if super::layout_measurements::navigation_open(id) {
        let mut body = rect;
        body.origin.y += header.size.height as i32;
        body.size.height = body.size.height.saturating_sub(header.size.height);
        let extent = preferred_size(tree, id, theme).height.max(body.size.height);
        let maximum = extent.saturating_sub(body.size.height);
        let offset = super::layout_measurements::navigation_scroll(maximum);
        let document = Rect { origin: Point {x:body.origin.x,y:body.origin.y.saturating_sub_unsigned(offset)},size:Size{width:body.size.width,height:extent} };
        layout_widget(tree, id, document, theme, geometry);
        geometry.insert(id, body);
    } else {
        geometry.insert(id, header);
    }
}
