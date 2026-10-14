//! Deterministic desktop icon placement.

use std::collections::{BTreeMap, BTreeSet};

use crate::{DesktopItemId, Rect};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Cell {
    pub column: u16,
    pub row: u16,
}

impl Cell {
    #[must_use]
    pub const fn new(column: u16, row: u16) -> Self {
        Self { column, row }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridConfig {
    pub columns: u16,
    pub rows: u16,
    pub cell_width: u32,
    pub cell_height: u32,
    pub horizontal_gap: u32,
    pub vertical_gap: u32,
    pub origin_x: i32,
    pub origin_y: i32,
}

impl GridConfig {
    #[must_use]
    pub const fn capacity(self) -> usize {
        self.columns as usize * self.rows as usize
    }

    #[must_use]
    pub const fn contains(self, cell: Cell) -> bool {
        cell.column < self.columns && cell.row < self.rows
    }

    #[must_use]
    pub fn rect(self, cell: Cell) -> Rect {
        let stride_x = self.cell_width.saturating_add(self.horizontal_gap);
        let stride_y = self.cell_height.saturating_add(self.vertical_gap);
        let x = i64::from(self.origin_x) + i64::from(cell.column) * i64::from(stride_x);
        let y = i64::from(self.origin_y) + i64::from(cell.row) * i64::from(stride_y);
        Rect::new(
            saturating_i32(x),
            saturating_i32(y),
            self.cell_width,
            self.cell_height,
        )
    }
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            cell_width: 96,
            cell_height: 96,
            horizontal_gap: 4,
            vertical_gap: 4,
            origin_x: 8,
            origin_y: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GridPlacement {
    pub item: DesktopItemId,
    pub cell: Cell,
    pub bounds: Rect,
}

/// Strategy seam for alternate ordering, such as locale- or accessibility-led
/// placement. The default remains stable column-major desktop ordering.
pub trait GridPlacementStrategy {
    fn cells(&self, config: GridConfig) -> Vec<Cell>;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnMajorStrategy;

impl GridPlacementStrategy for ColumnMajorStrategy {
    fn cells(&self, config: GridConfig) -> Vec<Cell> {
        let mut cells = Vec::with_capacity(config.capacity());
        for column in 0..config.columns {
            for row in 0..config.rows {
                cells.push(Cell::new(column, row));
            }
        }
        cells
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridResizeReport {
    pub moved: Vec<(DesktopItemId, Cell, Cell)>,
    pub newly_placed: Vec<(DesktopItemId, Cell)>,
    pub unplaced: Vec<DesktopItemId>,
}

impl GridResizeReport {
    fn empty() -> Self {
        Self {
            moved: Vec::new(),
            newly_placed: Vec::new(),
            unplaced: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GridLayout {
    config: GridConfig,
    positions: BTreeMap<DesktopItemId, Cell>,
    unplaced: BTreeSet<DesktopItemId>,
}

impl GridLayout {
    #[must_use]
    pub fn new(config: GridConfig) -> Self {
        Self {
            config,
            positions: BTreeMap::new(),
            unplaced: BTreeSet::new(),
        }
    }

    #[must_use]
    pub const fn config(&self) -> GridConfig {
        self.config
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    #[must_use]
    pub fn cell_of(&self, item: DesktopItemId) -> Option<Cell> {
        self.positions.get(&item).copied()
    }

    #[must_use]
    pub fn contains(&self, item: DesktopItemId) -> bool {
        self.positions.contains_key(&item) || self.unplaced.contains(&item)
    }

    pub fn unplaced(&self) -> impl Iterator<Item = DesktopItemId> + '_ {
        self.unplaced.iter().copied()
    }

    #[must_use]
    pub fn placements(&self) -> Vec<GridPlacement> {
        let mut output: Vec<_> = self
            .positions
            .iter()
            .map(|(&item, &cell)| GridPlacement {
                item,
                cell,
                bounds: self.config.rect(cell),
            })
            .collect();
        output.sort_by_key(|entry| (entry.cell.column, entry.cell.row, entry.item));
        output
    }

    #[must_use]
    pub fn ordered_items(&self) -> Vec<DesktopItemId> {
        self.placements()
            .into_iter()
            .map(|entry| entry.item)
            .collect()
    }

    pub fn remove(&mut self, item: DesktopItemId) -> bool {
        self.unplaced.remove(&item) || self.positions.remove(&item).is_some()
    }

    /// Reflows every known item into the first column-major cells.
    pub fn arrange(&mut self) {
        let items = self
            .positions
            .keys()
            .chain(&self.unplaced)
            .copied()
            .collect::<BTreeSet<_>>();
        self.arrange_in_order(items);
    }

    /// Reflows every known item using the caller's stable semantic order.
    pub fn arrange_in_order(&mut self, items: impl IntoIterator<Item = DesktopItemId>) {
        let known = self
            .positions
            .keys()
            .chain(&self.unplaced)
            .copied()
            .collect::<BTreeSet<_>>();
        self.positions.clear();
        self.unplaced.clear();
        for item in items {
            if known.contains(&item) && !self.contains(item) {
                self.add(item, None);
            }
        }
    }

    /// Adds one item, respecting a valid persisted cell when possible.
    pub fn add(&mut self, item: DesktopItemId, persisted: Option<Cell>) -> Option<GridPlacement> {
        if let Some(cell) = self.positions.get(&item).copied() {
            return Some(GridPlacement {
                item,
                cell,
                bounds: self.config.rect(cell),
            });
        }
        self.unplaced.remove(&item);
        let occupied: BTreeSet<_> = self.positions.values().copied().collect();
        let target = persisted
            .filter(|cell| self.config.contains(*cell) && !occupied.contains(cell))
            .or_else(|| self.first_available(&occupied));
        if let Some(cell) = target {
            self.positions.insert(item, cell);
            Some(GridPlacement {
                item,
                cell,
                bounds: self.config.rect(cell),
            })
        } else {
            self.unplaced.insert(item);
            None
        }
    }

    /// Atomically places a complete item set. Persisted collisions use item
    /// identifiers as a stable tie-breaker; otherwise caller order controls
    /// the initial placement of new icons.
    pub fn reconcile(
        &mut self,
        items: impl IntoIterator<Item = DesktopItemId>,
        persisted: &BTreeMap<DesktopItemId, Cell>,
    ) {
        let ordered = items.into_iter().collect::<Vec<_>>();
        let wanted = ordered.iter().copied().collect::<BTreeSet<_>>();
        self.positions.retain(|item, _| wanted.contains(item));
        self.unplaced.retain(|item| wanted.contains(item));

        for item in wanted
            .iter()
            .copied()
            .filter(|item| persisted.contains_key(item))
        {
            if !self.contains(item) {
                self.add(item, persisted.get(&item).copied());
            }
        }
        for item in ordered {
            if !self.contains(item) {
                self.add(item, None);
            }
        }
    }

    /// Applies an explicit user move. An occupied target swaps positions,
    /// preserving both items and making the command reversible by repetition.
    pub fn move_to(&mut self, item: DesktopItemId, target: Cell) -> bool {
        if !self.config.contains(target) || !self.contains(item) {
            return false;
        }
        let previous = self.positions.get(&item).copied();
        let occupant = self
            .positions
            .iter()
            .find_map(|(&other, &cell)| (other != item && cell == target).then_some(other));
        if let Some(other) = occupant {
            if let Some(previous) = previous {
                self.positions.insert(other, previous);
            } else {
                self.positions.remove(&other);
                self.unplaced.insert(other);
            }
        }
        self.unplaced.remove(&item);
        self.positions.insert(item, target);
        true
    }

    /// Atomically moves a selected group while preserving its shape.
    ///
    /// The requested anchor delta is clamped at the grid edges. A group move
    /// never displaces an unselected icon; dropping onto one is rejected so a
    /// multi-selection cannot be silently scrambled.
    pub fn move_group(
        &mut self,
        items: &[DesktopItemId],
        anchor: DesktopItemId,
        target: Cell,
    ) -> bool {
        let selected = items.iter().copied().collect::<BTreeSet<_>>();
        if selected.is_empty()
            || !selected.contains(&anchor)
            || selected
                .iter()
                .any(|item| !self.positions.contains_key(item))
        {
            return false;
        }
        let Some(anchor_cell) = self.positions.get(&anchor).copied() else {
            return false;
        };
        let requested_column = i32::from(target.column) - i32::from(anchor_cell.column);
        let requested_row = i32::from(target.row) - i32::from(anchor_cell.row);
        let (minimum_column, maximum_column, minimum_row, maximum_row) = selected
            .iter()
            .filter_map(|item| self.positions.get(item).copied())
            .fold(
                (u16::MAX, 0_u16, u16::MAX, 0_u16),
                |(min_column, max_column, min_row, max_row), cell| {
                    (
                        min_column.min(cell.column),
                        max_column.max(cell.column),
                        min_row.min(cell.row),
                        max_row.max(cell.row),
                    )
                },
            );
        let column_delta = requested_column.clamp(
            -i32::from(minimum_column),
            i32::from(
                self.config
                    .columns
                    .saturating_sub(1)
                    .saturating_sub(maximum_column),
            ),
        );
        let row_delta = requested_row.clamp(
            -i32::from(minimum_row),
            i32::from(
                self.config
                    .rows
                    .saturating_sub(1)
                    .saturating_sub(maximum_row),
            ),
        );
        if column_delta == 0 && row_delta == 0 {
            return false;
        }
        let destinations = selected
            .iter()
            .filter_map(|item| {
                let cell = self.positions.get(item)?;
                let column = u16::try_from(i32::from(cell.column) + column_delta).ok()?;
                let row = u16::try_from(i32::from(cell.row) + row_delta).ok()?;
                Some((*item, Cell::new(column, row)))
            })
            .collect::<BTreeMap<_, _>>();
        if destinations.len() != selected.len()
            || destinations.values().any(|destination| {
                self.positions
                    .iter()
                    .any(|(item, cell)| !selected.contains(item) && cell == destination)
            })
        {
            return false;
        }
        for (item, destination) in destinations {
            self.positions.insert(item, destination);
        }
        true
    }

    /// Preserves valid unique cells and deterministically reflows collisions,
    /// clipped cells, and previously unplaced items.
    pub fn resize(&mut self, config: GridConfig) -> GridResizeReport {
        let old_positions = self.positions.clone();
        let mut report = GridResizeReport::empty();
        let mut retained = BTreeMap::new();
        let mut occupied = BTreeSet::new();
        let mut pending = BTreeSet::new();

        for (&item, &cell) in &old_positions {
            if config.contains(cell) && occupied.insert(cell) {
                retained.insert(item, cell);
            } else {
                pending.insert(item);
            }
        }
        pending.append(&mut self.unplaced);
        self.config = config;
        self.positions = retained;

        for item in pending {
            let old = old_positions.get(&item).copied();
            if let Some(placement) = self.add(item, None) {
                if let Some(old) = old {
                    report.moved.push((item, old, placement.cell));
                } else {
                    report.newly_placed.push((item, placement.cell));
                }
            } else {
                report.unplaced.push(item);
            }
        }
        report
    }

    fn first_available(&self, occupied: &BTreeSet<Cell>) -> Option<Cell> {
        ColumnMajorStrategy
            .cells(self.config)
            .into_iter()
            .find(|cell| !occupied.contains(cell))
    }
}

fn saturating_i32(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value.is_negative() {
        i32::MIN
    } else {
        i32::MAX
    })
}
