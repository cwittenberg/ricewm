//! Centered modal layout and modal hit-test isolation.

use std::collections::BTreeMap;

use rice_native_app_protocol::WidgetId;
use rice_ui::{Point, Rect, RetainedTree, Size, Theme, WidgetKind, WidgetState};

use super::layout::{layout_widget, preferred_size};

const MIN_DIALOG_WIDTH_CONTROLS: u32 = 12;
const MAX_DIALOG_WIDTH_CONTROLS: u32 = 22;
const VIEWPORT_MARGIN_UNITS: u32 = 2;

pub(super) fn is_dialog(tree: &RetainedTree, id: WidgetId) -> bool {
    tree.get(id).is_some_and(|widget| {
        (widget.kind == WidgetKind::Dialog && !widget.state.contains(WidgetState::FLAT))
            || (widget.kind == WidgetKind::Popover && super::layout_tokens::anchored_popup(widget.text.as_deref()).is_some())
    })
}

pub(super) fn is_tooltip(tree: &RetainedTree, id: WidgetId) -> bool {
    tree.get(id)
        .is_some_and(|widget| widget.kind == WidgetKind::Tooltip)
}

pub(super) fn is_notification(tree: &RetainedTree, id: WidgetId) -> bool {
    tree.get(id)
        .is_some_and(|widget| widget.kind == WidgetKind::Notification)
}

pub(super) fn tooltip_for_hover(tree: &RetainedTree, hovered: WidgetId, geometry: &BTreeMap<WidgetId, Rect>, theme: &Theme) -> Option<WidgetId> {
    let anchor = tree.get(hovered)?;
    // A tooltip must add information. Repeating a label the control already
    // paints is noise, including labels nested inside a composite row. Toolbar
    // buttons still need their own hidden label because only the icon is
    // displayed.
    // A clipped label's full text adds information even when its source string
    // matches the tooltip. Compare the actual allocation with its desired size.
    let clipped_label = matches!(anchor.kind, WidgetKind::Label | WidgetKind::Button | WidgetKind::Menu) && geometry.get(&hovered)
        .is_some_and(|rect| preferred_size(tree, hovered, theme).width > rect.size.width);
    let displayed_label = anchor.text.as_deref().filter(|_| !clipped_label).filter(|_| {
        !(anchor.kind == WidgetKind::Button
            && anchor
                .parent
                .and_then(|parent| tree.get(parent))
                .is_some_and(|parent| parent.kind == WidgetKind::ToolBar))
    });
    anchor.children().iter().copied().find(|child| {
        tree.get(*child).is_some_and(|tooltip| {
            tooltip.kind == WidgetKind::Tooltip
                && tooltip.text.as_deref().is_some_and(|tooltip_text| {
                    displayed_label.is_none_or(|label| !same_visible_copy(label, tooltip_text))
                        && !descendant_label_matches(tree, anchor, tooltip_text)
                })
        })
    })
}

/// Resolve a tooltip and its anchor for the currently hovered control.
///
/// Tooltips are passive overlays: they never become the hovered target
/// themselves. This keeps controls beneath the painted balloon clickable.
pub(super) fn tooltip_overlay_for_hover(
    tree: &RetainedTree,
    hovered: WidgetId,
    geometry: &BTreeMap<WidgetId, Rect>,
    theme: &Theme,
) -> Option<(WidgetId, WidgetId)> {
    tooltip_for_hover(tree, hovered, geometry, theme).map(|tooltip| (tooltip, hovered))
}

fn same_visible_copy(left: &str, right: &str) -> bool {
    left.split_whitespace().eq(right.split_whitespace())
}

fn descendant_label_matches(tree: &RetainedTree, anchor: &rice_ui::Widget, tooltip: &str) -> bool {
    let mut pending = anchor.children().to_vec();
    while let Some(id) = pending.pop() {
        let Some(widget) = tree.get(id) else {
            continue;
        };
        if widget.kind == WidgetKind::Tooltip {
            continue;
        }
        if matches!(widget.kind, WidgetKind::Label | WidgetKind::Markdown)
            && widget
                .text
                .as_deref()
                .is_some_and(|label| same_visible_copy(label, tooltip))
        {
            return true;
        }
        pending.extend(widget.children().iter().copied());
    }
    false
}

pub(super) fn layout_tooltip_overlay(
    tree: &RetainedTree,
    tooltip: WidgetId,
    anchor: WidgetId,
    viewport: Size,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    let Some(anchor) = geometry.get(&anchor).copied() else {
        return;
    };
    // Leave a quiet seam for the 8 px directional balloon pointer.
    let gap = u32::from(theme.spacing.unit).max(10);
    let preferred = preferred_size(tree, tooltip, theme);
    let width = preferred.width.min(viewport.width).min(560);
    let size = Size {
        width,
        height: super::layout::preferred_height_for_width(tree, tooltip, width, theme)
            .max(preferred.height).min(viewport.height),
    };
    let maximum_x = viewport.width.saturating_sub(size.width);
    let x = anchor.origin.x.max(0).cast_unsigned().min(maximum_x);
    let below = anchor
        .origin
        .y
        .saturating_add_unsigned(anchor.size.height.saturating_add(gap));
    let maximum_y = viewport.height.saturating_sub(size.height);
    let y = if below.max(0).cast_unsigned() <= maximum_y {
        below.max(0).cast_unsigned()
    } else {
        anchor
            .origin
            .y
            .saturating_sub_unsigned(size.height.saturating_add(gap))
            .max(0)
            .cast_unsigned()
            .min(maximum_y)
    };
    layout_widget(
        tree,
        tooltip,
        Rect {
            origin: Point {
                x: i32::try_from(x).unwrap_or_default(),
                y: i32::try_from(y).unwrap_or_default(),
            },
            size,
        },
        theme,
        geometry,
    );
}

pub(super) fn dialog_widgets(tree: &RetainedTree) -> Vec<WidgetId> {
    let mut dialogs = Vec::new();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let Some(widget) = tree.get(id) else {
            continue;
        };
        pending.extend(widget.children().iter().rev().copied());
        if is_dialog(tree, id) {
            dialogs.push(id);
        }
    }
    dialogs
}

pub(super) fn notification_widgets(tree: &RetainedTree) -> Vec<WidgetId> {
    let mut notifications = Vec::new();
    let mut pending = tree.roots().to_vec();
    while let Some(id) = pending.pop() {
        let Some(widget) = tree.get(id) else {
            continue;
        };
        pending.extend(widget.children().iter().rev().copied());
        if widget.kind == WidgetKind::Notification {
            notifications.push(id);
        }
    }
    notifications
}

pub(super) fn layout_notification_overlay(
    tree: &RetainedTree,
    notification: WidgetId,
    viewport: Size,
    theme: &Theme,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    let preferred = preferred_size(tree, notification, theme);
    let margin = u32::from(theme.spacing.section).max(8);
    let maximum_width = 420_u32.min(viewport.width.saturating_sub(margin.saturating_mul(2)));
    let width = preferred.width.clamp(180.min(maximum_width), maximum_width);
    let height = preferred
        .height
        .max(u32::from(theme.metrics.control_height))
        .min(viewport.height.saturating_sub(margin.saturating_mul(2)));
    let rect = Rect {
        origin: Point {
            x: i32::try_from(margin).unwrap_or_default(),
            y: i32::try_from(viewport.height.saturating_sub(height + margin)).unwrap_or_default(),
        },
        size: Size { width, height },
    };
    layout_widget(tree, notification, rect, theme, geometry);
}

pub(super) fn layout_dialog_overlay(
    tree: &RetainedTree,
    dialog: WidgetId,
    viewport: Size,
    theme: &Theme,
    work_area: Option<Rect>,
    geometry: &mut BTreeMap<WidgetId, Rect>,
) {
    // Compact owners can disclose an independent editor without replacing
    // their primary form. The host supplies desktop bounds in owner coordinates.
    if tree.get(dialog).and_then(|widget| widget.text.as_deref())
        .is_some_and(|text| text.starts_with("panel:drawer-dialog:"))
        && let Some(area) = work_area
    {
        let preferred = preferred_size(tree, dialog, theme);
        let width = preferred.width.max(360).min(area.size.width.saturating_sub(24));
        let height = preferred.height.max(240).min(area.size.height.saturating_sub(24));
        let gap = 12;
        let fits = |x: i32, y: i32| x >= area.origin.x + gap && y >= area.origin.y + gap
            && i64::from(x) + i64::from(width) <= i64::from(area.origin.x) + i64::from(area.size.width) - i64::from(gap)
            && i64::from(y) + i64::from(height) <= i64::from(area.origin.y) + i64::from(area.size.height) - i64::from(gap);
        let right = (viewport.width as i32 + gap, 0);
        let left = (-(width as i32) - gap, 0);
        let bottom = (0, viewport.height as i32 + gap);
        let top = (0, -(height as i32) - gap);
        let anchor = tree.get(dialog).and_then(|widget| widget.text.as_deref())
            .and_then(|text| text.strip_prefix("panel:drawer-dialog:"))
            .and_then(|text| text.split(':').next());
        let preferred_position = match anchor { Some("left") => left, Some("top") => top, Some("bottom") => bottom, _ => right };
        let positions = [preferred_position, right, left, bottom, top];
        let (x, y) = positions.into_iter().find(|&(x, y)| fits(x, y)).unwrap_or((
            area.origin.x + (area.size.width.saturating_sub(width) / 2) as i32,
            area.origin.y + (area.size.height.saturating_sub(height) / 2) as i32,
        ));
        layout_widget(tree, dialog, Rect { origin: Point { x, y }, size: Size { width, height } }, theme, geometry);
        return;
    }
    if let Some(widget) = tree.get(dialog)
        && widget.kind == WidgetKind::Popover
        && let Some([x, y, width, height]) = super::layout_tokens::anchored_popup(widget.text.as_deref())
    {
        let parent = widget.parent.and_then(|id| geometry.get(&id)).copied().unwrap_or_default();
        let width = width.min(viewport.width.saturating_sub(8));
        let requested_height = height.min(viewport.height.saturating_sub(8));
        let desired_x = parent.origin.x.saturating_add_unsigned(x).max(4) as u32;
        let below = parent.origin.y.saturating_add_unsigned(y).max(4) as u32;
        let available_below = viewport.height.saturating_sub(below.saturating_add(4));
        // Keep a useful scrolled popup attached to its column header. Flip
        // above only when the area below cannot fit the controls and footer.
        let (top, height) = if available_below >= requested_height.min(320) {
            (below, requested_height.min(available_below))
        } else {
            let height = requested_height.min(below.saturating_sub(4).max(160))
                .min(viewport.height.saturating_sub(8));
            (below.saturating_sub(height).max(4), height)
        };
        let rect = Rect { origin: Point {
            x: desired_x.min(viewport.width.saturating_sub(width + 4)) as i32,
            y: top.min(viewport.height.saturating_sub(height + 4)) as i32,
        }, size: Size { width, height } };
        layout_widget(tree, dialog, rect, theme, geometry);
        return;
    }
    // Window editors use the owner's titlebar and its entire client area.
    if tree.get(dialog).and_then(|widget| widget.text.as_deref())
        .is_some_and(|text| text.starts_with("panel:window-editor:")) {
        layout_widget(tree, dialog, Rect { origin: Point { x: 0, y: 0 }, size: viewport }, theme, geometry);
        return;
    }
    let preferred = preferred_size(tree, dialog, theme);
    let control = u32::from(theme.metrics.control_height);
    let margin = u32::from(theme.spacing.section).saturating_mul(VIEWPORT_MARGIN_UNITS);
    let available = Size {
        width: viewport.width.saturating_sub(margin.saturating_mul(2)),
        height: viewport.height.saturating_sub(margin.saturating_mul(2)),
    };
    let expansive = tree
        .get(dialog)
        .is_some_and(|widget| widget.state.contains(WidgetState::EXPANSIVE));
    let side_sheet = tree
        .get(dialog)
        .is_some_and(|widget| super::layout_tokens::is_side_sheet(widget.text.as_deref()));
    if side_sheet {
        let target_width = viewport.width.saturating_mul(72) / 100;
        let width = target_width.clamp(560, 780).min(available.width);
        let size = Size {
            width,
            height: available.height,
        };
        let rect = Rect {
            origin: Point {
                x: i32::try_from(viewport.width.saturating_sub(width + margin)).unwrap_or_default(),
                y: i32::try_from(margin).unwrap_or_default(),
            },
            size,
        };
        layout_widget(tree, dialog, rect, theme, geometry);
        return;
    }
    let size = if expansive {
        Size {
            // Follow the resizable containing window all the way up to the
            // safe overlay margins. Keeping an arbitrary 960x720 ceiling made
            // large file choosers remain finicky even after users enlarged
            // their RiceWM window. `min(available)` is also essential for
            // compact windows; a lower clamp larger than the viewport used to
            // strand dialog footers below the visible area.
            width: available.width,
            height: available.height,
        }
    } else {
        Size {
            width: preferred
                .width
                .max(control.saturating_mul(MIN_DIALOG_WIDTH_CONTROLS))
                .min(control.saturating_mul(MAX_DIALOG_WIDTH_CONTROLS))
                .min(available.width),
            height: preferred.height.min(available.height),
        }
    };
    let rect = Rect {
        origin: Point {
            x: i32::try_from(viewport.width.saturating_sub(size.width) / 2).unwrap_or_default(),
            y: i32::try_from(viewport.height.saturating_sub(size.height) / 2).unwrap_or_default(),
        },
        size,
    };
    layout_widget(tree, dialog, rect, theme, geometry);
}

pub(super) fn widget_belongs_to(
    tree: &RetainedTree,
    mut candidate: WidgetId,
    dialog: WidgetId,
) -> bool {
    loop {
        if candidate == dialog {
            return true;
        }
        let Some(parent) = tree.get(candidate).and_then(|widget| widget.parent) else {
            return false;
        };
        candidate = parent;
    }
}
