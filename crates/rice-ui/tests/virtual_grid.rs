use rice_ui::virtual_grid::{GridAxis, GridError, MAX_VISIBLE_TRACKS};

#[test]
fn million_rows_have_constant_default_storage_and_bounded_view() {
    let rows = GridAxis::new(1_048_576, 20).unwrap();
    assert_eq!(rows.run_count(), 1);
    let view = rows.viewport(1, 20_000_007, 600).unwrap();
    assert_eq!(view.tracks.len(), 31);
    assert_eq!(view.tracks[0].index, 0);
    assert_eq!(view.tracks[1].index, 1_000_001);
    assert_eq!(view.tracks[1].start, 13);
    assert_eq!(view.hit_test(19), Some(0));
    assert_eq!(view.hit_test(20), Some(1_000_001));
    assert_eq!(view.hit_test(599), Some(1_000_030));
    assert_eq!(view.hit_test(600), None);
}

#[test]
fn hiding_a_million_tracks_skips_them_and_restores_coalesced_geometry() {
    let mut rows = GridAxis::new(1_048_576, 20).unwrap();
    rows.set_extent(1..1_048_575, 0).unwrap();
    assert_eq!(rows.run_count(), 3);
    assert_eq!(rows.total_extent(), 40);
    assert_eq!(rows.track_at(20), Some(1_048_575));
    let view = rows.viewport(1, u64::MAX, 600).unwrap();
    assert_eq!(view.tracks.iter().map(|track| track.index).collect::<Vec<_>>(), vec![0, 1_048_575]);
    assert_eq!(view.hit_test(40), None);
    rows.set_extent(1..1_048_575, 20).unwrap();
    assert_eq!(rows.run_count(), 1);
}

#[test]
fn resizing_overlapping_ranges_matches_dense_reference() {
    let mut axis = GridAxis::new(100, 10).unwrap();
    let mut dense = vec![10_u32; 100];
    for (range, extent) in [(7..28, 15), (12..50, 0), (20..80, 2), (0..9, 42), (90..100, 0)] {
        axis.set_extent(range.clone(), extent).unwrap();
        for value in &mut dense[range.start as usize..range.end as usize] { *value = extent; }
        let mut offset = 0;
        for (index, &size) in dense.iter().enumerate() {
            assert_eq!(axis.offset(index as u32), Some(offset));
            for pixel in 0..size {
                assert_eq!(axis.track_at(offset + u64::from(pixel)), Some(index as u32));
            }
            offset += u64::from(size);
        }
        assert_eq!(axis.total_extent(), offset);
        assert_eq!(axis.track_at(offset), None);
    }
}

#[test]
fn frozen_tracks_clip_before_scrolling_tracks() {
    let axis = GridAxis::new(100, 40).unwrap();
    let view = axis.viewport(4, 1000, 90).unwrap();
    assert_eq!(view.tracks.len(), 3);
    assert!(view.tracks.iter().all(|track| track.frozen));
    assert_eq!(view.hit_test(89), Some(2));
    assert_eq!(view.hit_test(90), None);
}

#[test]
fn scrolled_view_with_resizes_and_hidden_ranges_matches_independent_pixel_lookup() {
    let mut axis = GridAxis::new(80, 11).unwrap();
    axis.set_extent(4..22, 0).unwrap();
    axis.set_extent(30..35, 42).unwrap();
    for frozen in [0, 2, 18, 33, 80] {
        for scroll in [0, 7, 123, 9000] {
            let view = axis.viewport(frozen, scroll, 100).unwrap();
            for pixel in 0..100 {
                let absolute = if pixel < view.frozen_extent {
                    u64::from(pixel)
                } else {
                    axis.offset(frozen).unwrap() + view.scroll + u64::from(pixel - view.frozen_extent)
                };
                assert_eq!(view.hit_test(pixel), axis.track_at(absolute));
            }
        }
    }
}

#[test]
fn invalid_ranges_are_atomic_and_dense_views_fail_explicitly() {
    let mut axis = GridAxis::new(100, 20).unwrap();
    let original = axis.clone();
    assert_eq!(axis.set_extent(50..101, 1), Err(GridError::InvalidRange));
    assert_eq!(axis, original);
    assert_eq!(axis.viewport(101, 0, 100), Err(GridError::InvalidRange));
    let axis = GridAxis::new(MAX_VISIBLE_TRACKS as u32 + 1, 1).unwrap();
    assert_eq!(axis.viewport(0, 0, MAX_VISIBLE_TRACKS as u32 + 1), Err(GridError::ViewportTooDense));
}

#[test]
fn empty_and_fully_hidden_axes_are_valid() {
    for count in [0, 1_048_576] {
        let axis = GridAxis::new(count, 0).unwrap();
        assert!(axis.viewport(count, u64::MAX, 500).unwrap().tracks.is_empty());
        assert_eq!(axis.track_at(0), None);
    }
}
