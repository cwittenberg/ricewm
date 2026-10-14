//! Desktop selection and keyboard navigation policy.

use std::collections::BTreeSet;

use crate::{Cell, DesktopItemId, GridLayout, Rect};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClickMode {
    Replace,
    Toggle,
    ExtendRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SelectionState {
    selected: BTreeSet<DesktopItemId>,
    anchor: Option<DesktopItemId>,
    focus: Option<DesktopItemId>,
}

impl SelectionState {
    pub fn selected(&self) -> impl Iterator<Item = DesktopItemId> + '_ {
        self.selected.iter().copied()
    }

    #[must_use]
    pub fn selected_vec(&self) -> Vec<DesktopItemId> {
        self.selected.iter().copied().collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.selected.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    #[must_use]
    pub const fn anchor(&self) -> Option<DesktopItemId> {
        self.anchor
    }

    #[must_use]
    pub const fn focus(&self) -> Option<DesktopItemId> {
        self.focus
    }

    #[must_use]
    pub fn contains(&self, item: DesktopItemId) -> bool {
        self.selected.contains(&item)
    }

    pub fn clear(&mut self) {
        self.selected.clear();
        self.anchor = None;
        self.focus = None;
    }

    pub fn select_all(&mut self, layout: &GridLayout) {
        self.selected = layout.ordered_items().into_iter().collect();
        self.anchor = self.selected.first().copied();
        self.focus = self.selected.last().copied();
    }

    pub fn click(&mut self, item: DesktopItemId, mode: ClickMode, layout: &GridLayout) {
        if !layout.contains(item) {
            return;
        }
        match mode {
            ClickMode::Replace => {
                self.selected.clear();
                self.selected.insert(item);
                self.anchor = Some(item);
            }
            ClickMode::Toggle => {
                if !self.selected.remove(&item) {
                    self.selected.insert(item);
                }
                self.anchor = Some(item);
            }
            ClickMode::ExtendRange => self.extend_range(item, layout),
        }
        self.focus = Some(item);
    }

    /// Applies a selection rectangle in desktop coordinates.
    pub fn rubber_band(&mut self, area: Rect, additive: bool, layout: &GridLayout) {
        if !additive {
            self.selected.clear();
        }
        let intersecting: Vec<_> = layout
            .placements()
            .into_iter()
            .filter(|placement| intersects(area, placement.bounds))
            .map(|placement| placement.item)
            .collect();
        self.selected.extend(intersecting.iter().copied());
        if let Some(&last) = intersecting.last() {
            self.focus = Some(last);
            self.anchor.get_or_insert(last);
        } else if !additive {
            self.focus = None;
            self.anchor = None;
        }
    }

    /// Moves focus spatially and applies ordinary or range selection.
    pub fn navigate(&mut self, direction: Direction, extend: bool, layout: &GridLayout) {
        let ordered = layout.ordered_items();
        let Some(next) = navigation_target(self.focus, direction, layout, &ordered) else {
            return;
        };
        if extend {
            self.extend_range(next, layout);
        } else {
            self.selected.clear();
            self.selected.insert(next);
            self.anchor = Some(next);
        }
        self.focus = Some(next);
    }

    /// Removes stale selections after an event-driven refresh.
    pub fn retain_layout_items(&mut self, layout: &GridLayout) {
        self.selected.retain(|item| layout.contains(*item));
        if self.anchor.is_some_and(|item| !layout.contains(item)) {
            self.anchor = None;
        }
        if self.focus.is_some_and(|item| !layout.contains(item)) {
            self.focus = None;
        }
    }

    #[must_use]
    pub fn single(&self) -> Option<DesktopItemId> {
        (self.selected.len() == 1)
            .then(|| self.selected.first().copied())
            .flatten()
    }

    fn extend_range(&mut self, item: DesktopItemId, layout: &GridLayout) {
        let ordered = layout.ordered_items();
        let anchor = self
            .anchor
            .filter(|id| layout.contains(*id))
            .unwrap_or(item);
        let Some(anchor_index) = ordered.iter().position(|id| *id == anchor) else {
            return;
        };
        let Some(item_index) = ordered.iter().position(|id| *id == item) else {
            return;
        };
        let (start, end) = if anchor_index <= item_index {
            (anchor_index, item_index)
        } else {
            (item_index, anchor_index)
        };
        self.selected = ordered[start..=end].iter().copied().collect();
        self.anchor = Some(anchor);
    }
}

fn navigation_target(
    focus: Option<DesktopItemId>,
    direction: Direction,
    layout: &GridLayout,
    ordered: &[DesktopItemId],
) -> Option<DesktopItemId> {
    if matches!(direction, Direction::Home) {
        return ordered.first().copied();
    }
    if matches!(direction, Direction::End) {
        return ordered.last().copied();
    }
    let focus = focus.or_else(|| ordered.first().copied())?;
    let origin = layout.cell_of(focus)?;
    ordered
        .iter()
        .filter_map(|&candidate| {
            let cell = layout.cell_of(candidate)?;
            directional_score(origin, cell, direction).map(|score| (score, candidate))
        })
        .min()
        .map(|(_, item)| item)
        .or(Some(focus))
}

fn directional_score(origin: Cell, candidate: Cell, direction: Direction) -> Option<(u16, u16)> {
    let column_delta = origin.column.abs_diff(candidate.column);
    let row_delta = origin.row.abs_diff(candidate.row);
    match direction {
        Direction::Left if candidate.column < origin.column => Some((column_delta, row_delta)),
        Direction::Right if candidate.column > origin.column => Some((column_delta, row_delta)),
        Direction::Up if candidate.row < origin.row => Some((row_delta, column_delta)),
        Direction::Down if candidate.row > origin.row => Some((row_delta, column_delta)),
        _ => None,
    }
}

fn intersects(left: Rect, right: Rect) -> bool {
    let left_right = i64::from(left.x) + i64::from(left.width);
    let right_right = i64::from(right.x) + i64::from(right.width);
    let left_bottom = i64::from(left.y) + i64::from(left.height);
    let right_bottom = i64::from(right.y) + i64::from(right.height);
    i64::from(left.x) < right_right
        && i64::from(right.x) < left_right
        && i64::from(left.y) < right_bottom
        && i64::from(right.y) < left_bottom
}
