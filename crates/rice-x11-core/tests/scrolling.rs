use super::*;
const AREA: Rect = Rect {
    x: 0,
    y: 30,
    width: 1000,
    height: 660,
};
fn strip(ids: &[u32]) -> ScrollingStrip<u32> {
    let mut s = ScrollingStrip::default();
    s.sync(ids, AREA, &[]);
    // Exercise manually narrowed columns independently of the opening default.
    for &id in ids {
        let column = s.locate(id).unwrap().0;
        let width = s.columns[column].width;
        s.resize(id, Axis::Horizontal, 500 - i32::from(width), AREA, &[]);
        s.columns[column].automatic = false;
    }
    s
}
#[test]
fn opening_keeps_sizes_and_inserts_after_focus() {
    let mut s = strip(&[1, 2, 3]);
    s.focus(Some(2), AREA);
    s.sync(&[1, 2, 3, 4], AREA, &[]);
    assert_eq!(
        s.columns.iter().map(|c| c.windows[0]).collect::<Vec<_>>(),
        vec![1, 2, 4, 3]
    );
    assert!(s
        .rectangles(AREA, &[])
        .iter()
        .all(|(id, r)| r.width == if *id == 4 { 333 } else { 500 }));
    s.focus(Some(4), AREA);
    let cells = s.rectangles(AREA, &[]);
    assert_eq!(cells.iter().find(|(id, _)| *id == 4).unwrap().1.x, 667);
    assert_eq!(cells[0].1.x, -333);
}
#[test]
fn close_to_left_preserves_active_position() {
    let mut s = strip(&[1, 2, 3, 4, 5]);
    s.focus(Some(4), AREA);
    let x = s.rectangles(AREA, &[])[3].1.x;
    s.sync(&[2, 3, 4, 5], AREA, &[]);
    s.focus(Some(4), AREA);
    assert_eq!(s.rectangles(AREA, &[])[2].1.x, x);
}
#[test]
fn pan_clamps_without_focus_snapping_and_edges_do_not_wrap() {
    let mut s = strip(&[1, 2, 3, 4]);
    s.focus(Some(1), AREA);
    assert!(s.pan(250, AREA.width));
    s.sync(&[1, 2, 3, 4], AREA, &[]);
    s.focus(Some(1), AREA);
    assert_eq!(s.offset, 250);
    s.pan(i32::MAX, AREA.width);
    assert_eq!(s.offset, 1000);
    assert!(!s.pan(500, AREA.width));
    assert_eq!(s.neighbor(1, 0), None);
    assert_eq!(s.neighbor(4, 1), None);
    s.focus(Some(2), AREA);
    assert_eq!(s.offset, 500);
}
#[test]
fn stacking_expelling_resizing_and_constraints() {
    let mut s = strip(&[1, 2, 3]);
    assert!(s.insert(2, 1, 3, AREA, &[]));
    assert_eq!(s.neighbor(1, 3), Some(2));
    assert_eq!(s.rectangles(AREA, &[])[0].1.height, 330);
    assert!(s.resize(1, Axis::Horizontal, 100, AREA, &[]));
    assert_eq!(s.rectangles(AREA, &[])[1].1.width, 600);
    assert_eq!(s.rectangles(AREA, &[])[2].1.width, 500);
    assert!(!s.insert(3, 1, 3, AREA, &[(3, 200, 400)]));
    assert!(s.insert(2, 1, 1, AREA, &[]));
    assert_eq!(s.columns.len(), 3);
    assert!(s.swap(1, 3));
    assert_eq!(s.neighbor(3, 1), Some(2));
}
#[test]
fn durable_columns_remap_ids_and_reject_corruption() {
    let mut s = strip(&[1, 2, 3]);
    s.insert(2, 1, 3, AREA, &[]);
    s.resize(1, Axis::Horizontal, 80, AREA, &[]);
    let mut words = Vec::new();
    s.encode(&mut |id| id, &mut words);
    let restored = ScrollingStrip::decode(&words, &mut 0, &|id| Some(id + 10)).unwrap();
    assert_eq!(restored.columns[0].windows, vec![11, 12]);
    assert_eq!(restored.columns[0].width, 580);
    for n in 0..words.len() {
        assert!(ScrollingStrip::<u32>::decode(&words[..n], &mut 0, &Some).is_none());
    }
    let mut invalid = words.clone();
    invalid[1] = 0;
    assert!(ScrollingStrip::<u32>::decode(&invalid, &mut 0, &Some).is_none());
}
#[test]
fn many_columns_remain_signed_and_work_is_bounded() {
    let ids: Vec<_> = (1..=128).collect();
    let mut s = strip(&ids);
    s.focus(Some(128), AREA);
    let cells = s.rectangles(AREA, &[]);
    assert_eq!(cells.len(), 128);
    assert_eq!(cells[0].1.x, -63000);
    assert_eq!(cells[127].1.x, 500);
    assert_eq!(s.neighbor(128, 0), Some(127));
    let started = std::time::Instant::now();
    for _ in 0..1000 {
        s.sync(&ids, AREA, &[]);
        s.focus(Some(128), AREA);
        std::hint::black_box(s.rectangles(AREA, &[]));
    }
    eprintln!(
        "128-column sync + layout: {:?} per update",
        started.elapsed() / 1000
    );
}
#[test]
fn presets_and_centering_do_not_resize_neighbors_or_snap_on_repaint() {
    let mut s = strip(&[1, 2, 3]);
    s.focus(Some(1), AREA);
    assert!(s.preset(1, 4, AREA, &[]));
    assert_eq!(s.columns[0].width, 666);
    assert_eq!(s.columns[1].width, 500);
    assert!(s.preset(1, 5, AREA, &[]));
    assert_eq!(s.columns[0].width, 1000);
    assert!(s.preset(1, 5, AREA, &[]));
    assert_eq!(s.columns[0].width, 666);
    assert!(s.preset(1, 6, AREA, &[]));
    s.sync(&[1, 2, 3], AREA, &[]);
    s.focus(Some(1), AREA);
    assert_eq!(s.rectangles(AREA, &[])[0].1.x, 167);
    s.focus(Some(3), AREA);
    assert!(s.rectangles(AREA, &[])[2].1.x + 500 <= 1000);
}
#[test]
fn row_resizing_respects_minima_and_survives_snapshot() {
    let mut s = strip(&[1, 2, 3]);
    let minima = [(1, 200, 150), (2, 200, 200)];
    assert!(s.insert(2, 1, 3, AREA, &minima));
    let before = s.rectangles(AREA, &minima);
    assert!(s.resize(1, Axis::Vertical, 32, AREA, &minima));
    let after = s.rectangles(AREA, &minima);
    assert_eq!(after[0].1.height, before[0].1.height + 32);
    assert_eq!(after[1].1.height, before[1].1.height - 32);
    let mut words = Vec::new();
    s.encode(&mut |id| id, &mut words);
    let restored = ScrollingStrip::decode(&words, &mut 0, &Some).unwrap();
    assert_eq!(restored.rectangles(AREA, &minima), after);
    s.resize(1, Axis::Vertical, 10000, AREA, &minima);
    assert_eq!(s.rectangles(AREA, &minima)[1].1.height, 200);
}

#[test]
fn pointer_dividers_resize_stacked_rows_without_changing_other_columns() {
    let mut s = strip(&[1, 2, 3]);
    s.insert(2, 1, 3, AREA, &[]);
    let dividers = s.dividers(AREA, &[]);
    let (index, (_, _, position)) = dividers
        .iter()
        .enumerate()
        .find(|(_, d)| d.0 == Axis::Vertical)
        .unwrap();
    assert!(s.move_divider(((index as u32 + 1) << 1) | 1, *position + 32, AREA, &[]));
    let cells = s.rectangles(AREA, &[]);
    assert_eq!(cells[0].1.height, 362);
    assert_eq!(cells[1].1.height, 298);
    assert_eq!(cells[2].1.height, 660);
}

#[test]
fn fullscreen_reserves_a_screen_without_losing_preferred_width() {
    let mut s = strip(&[1, 2, 3]);
    let initial = s.rectangles(AREA, &[]);
    s.set_fullscreens(&[(2, 1000)]);
    s.focus(Some(2), AREA);
    let full = s.rectangles(AREA, &[]);
    assert_eq!(full[1].1.x, AREA.x);
    assert_eq!(full[1].1.width, 1000);
    assert_eq!(full[2].1.x, AREA.x + 1000);
    s.set_fullscreens(&[]);
    s.focus(Some(2), AREA);
    let restored = s.rectangles(AREA, &[]);
    assert_eq!(
        restored
            .iter()
            .map(|(id, r)| (*id, r.width, r.height))
            .collect::<Vec<_>>(),
        initial
            .iter()
            .map(|(id, r)| (*id, r.width, r.height))
            .collect::<Vec<_>>()
    );
}

#[test]
fn fullscreen_row_temporarily_detaches_and_restores_stack() {
    let mut s = strip(&[1, 2, 3]);
    assert!(s.insert(2, 1, 3, AREA, &[]));
    let before = s.rectangles(AREA, &[]);
    assert_eq!(before[0].1.x, before[1].1.x);
    s.set_fullscreens(&[(2, 1000)]);
    s.focus(Some(2), AREA);
    let full = s.rectangles(AREA, &[]);
    let row = full.iter().find(|(id, _)| *id == 2).unwrap().1;
    assert_eq!(row.width, 1000);
    assert_eq!(row.height, AREA.height);
    assert!(full
        .iter()
        .filter(|(id, _)| *id != 2)
        .all(|(_, r)| r.x + i32::from(r.width) <= row.x || r.x >= row.x + i32::from(row.width)));
    s.set_fullscreens(&[]);
    s.focus(Some(1), AREA);
    assert_eq!(s.rectangles(AREA, &[]), before);
}

#[test]
fn automatic_columns_fill_then_scroll_and_expand_when_closed() {
    let mut s = ScrollingStrip::default();
    for count in 1..=4 {
        let ids: Vec<_> = (1..=count).collect();
        s.sync(&ids, AREA, &[]);
        s.focus(Some(count), AREA);
        let cells = s.rectangles(AREA, &[]);
        assert!(cells.iter().all(|(_, r)| r.height == AREA.height));
        let total: u32 = cells.iter().map(|(_, r)| u32::from(r.width)).sum();
        if count <= 3 {
            assert_eq!(total, 1000);
            assert_eq!(cells[0].1.x, 0);
            assert!(!s.pan(100, AREA.width));
        } else {
            assert_eq!(total, 1334);
            assert!(cells[0].1.x < 0);
            assert!(s.pan(-100, AREA.width));
        }
    }
    s.sync(&[1, 2], AREA, &[]);
    assert!(s.rectangles(AREA, &[]).iter().all(|(_, r)| r.width == 500));
    s.sync(&[1], AREA, &[]);
    assert_eq!(s.rectangles(AREA, &[]), vec![(1, AREA)]);
}

#[test]
fn automatic_sizing_and_explicit_widths_survive_snapshot_and_resize() {
    let mut s = ScrollingStrip::default();
    s.sync(&[1, 2], AREA, &[]);
    assert!(s.resize(1, Axis::Horizontal, 100, AREA, &[]));
    let mut words = Vec::new();
    s.encode(&mut |id| id, &mut words);
    let mut restored = ScrollingStrip::decode(&words, &mut 0, &Some).unwrap();
    restored.sync(&[1, 2, 3], AREA, &[]);
    assert_eq!(restored.columns[0].width, 600);
    assert_eq!(restored.columns[1].width, 333);
    let larger = Rect {
        width: 1200,
        ..AREA
    };
    restored.sync(&[1, 2, 3], larger, &[]);
    assert_eq!(restored.columns[0].width, 600);
    assert_eq!(restored.columns[1].width, 400);
    words[1] = 0x20001;
    assert!(ScrollingStrip::<u32>::decode(&words, &mut 0, &Some).is_none());
}

#[test]
fn expand_restores_automatic_thirds_and_custom_widths() {
    let mut s = ScrollingStrip::default();
    s.sync(&[1, 2, 3], AREA, &[]);
    let before = s.rectangles(AREA, &[]);
    assert!(s.preset(2, 5, AREA, &[]));
    s.sync(&[1, 2, 3], AREA, &[]);
    assert_eq!(s.columns[1].width, AREA.width);
    assert!(s.preset(2, 5, AREA, &[]));
    s.sync(&[1, 2, 3], AREA, &[]);
    assert_eq!(s.rectangles(AREA, &[]), before);
    assert!(s.columns[1].automatic);
    s.resize(2, Axis::Horizontal, 71, AREA, &[]);
    let previous = s.columns[1].width;
    assert!(s.preset(2, 5, AREA, &[]));
    assert!(s.preset(2, 5, AREA, &[]));
    assert_eq!(s.columns[1].width, previous);
    assert!(!s.columns[1].automatic);
}
