use std::collections::BTreeMap;

use rice_desktop::{
    Cell, ClickMode, DesktopItemId, Direction, GridConfig, GridLayout, Rect, SelectionState,
};

fn config(columns: u16, rows: u16) -> GridConfig {
    GridConfig {
        columns,
        rows,
        cell_width: 80,
        cell_height: 60,
        horizontal_gap: 10,
        vertical_gap: 5,
        origin_x: 7,
        origin_y: 11,
    }
}

fn layout(count: u64) -> GridLayout {
    let mut layout = GridLayout::new(config(3, 3));
    layout.reconcile((1..=count).map(DesktopItemId), &BTreeMap::new());
    layout
}

#[test]
fn persisted_collision_is_resolved_by_stable_item_id() {
    let mut grid = GridLayout::new(config(2, 2));
    let persisted = BTreeMap::from([
        (DesktopItemId(3), Cell::new(1, 0)),
        (DesktopItemId(1), Cell::new(1, 0)),
        (DesktopItemId(2), Cell::new(1, 0)),
    ]);
    grid.reconcile(
        [DesktopItemId(3), DesktopItemId(2), DesktopItemId(1)],
        &persisted,
    );
    assert_eq!(grid.cell_of(DesktopItemId(1)), Some(Cell::new(1, 0)));
    assert_eq!(grid.cell_of(DesktopItemId(2)), Some(Cell::new(0, 0)));
    assert_eq!(grid.cell_of(DesktopItemId(3)), Some(Cell::new(0, 1)));
}

#[test]
fn resize_preserves_valid_cells_and_reports_overflow() {
    let mut grid = GridLayout::new(config(3, 2));
    grid.reconcile((1..=6).map(DesktopItemId), &BTreeMap::new());
    let stable = grid.cell_of(DesktopItemId(1));
    let report = grid.resize(config(2, 2));
    assert_eq!(grid.cell_of(DesktopItemId(1)), stable);
    assert_eq!(grid.len(), 4);
    assert_eq!(report.unplaced, vec![DesktopItemId(5), DesktopItemId(6)]);

    let report = grid.resize(config(3, 2));
    assert!(report.unplaced.is_empty());
    assert_eq!(grid.len(), 6);
    assert_eq!(report.newly_placed.len(), 2);
}

#[test]
fn explicit_move_swaps_instead_of_losing_an_item() {
    let mut grid = layout(3);
    let first = grid.cell_of(DesktopItemId(1)).expect("first");
    let second = grid.cell_of(DesktopItemId(2)).expect("second");
    assert!(grid.move_to(DesktopItemId(1), second));
    assert_eq!(grid.cell_of(DesktopItemId(1)), Some(second));
    assert_eq!(grid.cell_of(DesktopItemId(2)), Some(first));
    assert_eq!(grid.len(), 3);
}

#[test]
fn group_move_is_atomic_preserves_shape_and_rejects_unselected_collisions() {
    let mut grid = layout(4);
    assert!(grid.move_group(
        &[DesktopItemId(1), DesktopItemId(2)],
        DesktopItemId(1),
        Cell::new(2, 0),
    ));
    assert_eq!(grid.cell_of(DesktopItemId(1)), Some(Cell::new(2, 0)));
    assert_eq!(grid.cell_of(DesktopItemId(2)), Some(Cell::new(2, 1)));

    let before = grid.placements();
    assert!(!grid.move_group(
        &[DesktopItemId(1), DesktopItemId(2)],
        DesktopItemId(1),
        Cell::new(1, 0),
    ));
    assert_eq!(grid.placements(), before);
}

#[test]
fn range_and_rubber_band_selection_follow_visual_order() {
    let grid = layout(6);
    let mut selection = SelectionState::default();
    selection.click(DesktopItemId(2), ClickMode::Replace, &grid);
    selection.click(DesktopItemId(5), ClickMode::ExtendRange, &grid);
    assert_eq!(
        selection.selected_vec(),
        vec![
            DesktopItemId(2),
            DesktopItemId(3),
            DesktopItemId(4),
            DesktopItemId(5)
        ]
    );

    selection.rubber_band(Rect::new(0, 0, 88, 140), false, &grid);
    assert_eq!(
        selection.selected_vec(),
        vec![DesktopItemId(1), DesktopItemId(2)]
    );
}

#[test]
fn keyboard_navigation_is_spatial_and_can_extend() {
    let grid = layout(6);
    let mut selection = SelectionState::default();
    selection.navigate(Direction::Home, false, &grid);
    assert_eq!(selection.focus(), Some(DesktopItemId(1)));
    selection.navigate(Direction::Down, false, &grid);
    assert_eq!(selection.focus(), Some(DesktopItemId(2)));
    selection.navigate(Direction::Right, true, &grid);
    assert_eq!(selection.focus(), Some(DesktopItemId(5)));
    assert_eq!(
        selection.selected_vec(),
        vec![
            DesktopItemId(2),
            DesktopItemId(3),
            DesktopItemId(4),
            DesktopItemId(5)
        ]
    );
    selection.navigate(Direction::End, false, &grid);
    assert_eq!(selection.single(), Some(DesktopItemId(6)));
}

#[test]
fn stale_selection_is_pruned_after_refresh() {
    let mut grid = layout(3);
    let mut selection = SelectionState::default();
    selection.select_all(&grid);
    grid.remove(DesktopItemId(2));
    selection.retain_layout_items(&grid);
    assert_eq!(
        selection.selected_vec(),
        vec![DesktopItemId(1), DesktopItemId(3)]
    );
}
