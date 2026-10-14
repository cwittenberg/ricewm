use super::*;
fn hit(tokens: &str, text: &str, x: i32, y: i32, zoom: Option<u16>) -> Option<usize> {
    let language = format!("rich~o400,600,0,1{tokens}");
    let presentation = EditorText {
        viewport: None,
        language: &language,
        syntax_style: "adaptive",
        wrap: EditorWrap::Word,
        line_numbers: false,
        highlight_current_line: false,
        caret: None,
        anchor: None,
        line_number_start: 1,
        font_size: zoom,
        force_dark_mode: false,
        document: text,
    };
    office_editor_hit(
        &presentation,
        Rect {
            origin: Point { x: 0, y: 0 },
            size: Size {
                width: 416,
                height: 300,
            },
        },
        Point { x: x + 8, y: y + 8 },
    )
}
#[test]
fn office_hits_use_proportional_shaped_positions_and_skip_combining_interior() {
    let stops = "~l0,4,10,20,16~t0,10.0,20,40,1~t1,14.4,20,40,1~t3,23.3,20,40,1~t4,38.9,20,40,0";
    assert_eq!(hit(stops, "ie\u{301}W", 14, 30, None), Some(1));
    assert_eq!(hit(stops, "ie\u{301}W", 20, 30, None), Some(3));
    assert_eq!(hit(stops, "ie\u{301}W", 38, 30, None), Some(4));
    assert_eq!(hit(stops, "ie\u{301}W", 0, 30, None), Some(0));
}
#[test]
fn office_hits_follow_visual_bidi_order_lines_empty_paragraphs_and_zoom() {
    let stops = "~t0,90,20,40,1~t1,75,20,40,1~t2,60,20,40,0~t3,10,50,70,1";
    assert_eq!(hit(stops, "אב\n", 60, 30, None), Some(2));
    assert_eq!(hit(stops, "אב\n", 90, 30, None), Some(0));
    assert_eq!(hit(stops, "אב\n", 500, 60, None), Some(3));
    assert_eq!(hit(stops, "אב\n", 150, 60, Some(24)), Some(1));
}
#[test]
fn malformed_shaped_tokens_are_bounded_and_legacy_office_stays_available() {
    let legacy = "~l0,4,10,20,16";
    assert_eq!(hit(legacy, "abcd", 28, 30, None), Some(2));
    for bad in [
        "~t999,10,20,40,1",
        "~t2,NaN,20,40,1",
        "~t2,20,50,40,1",
        "~t2,20,20,40,9",
        "~t2,20,20,40,1,extra",
    ] {
        assert_eq!(
            hit(&format!("{legacy}{bad}"), "abcd", 28, 30, None),
            Some(2)
        );
    }
    assert_eq!(hit("~t4,10,20,40,1", "", 10, 30, None), None);
}
#[test]
fn ordinary_rich_metadata_does_not_activate_office_shaped_hit() {
    let presentation = EditorText::parse("editor:rich~t0,10,20,40,1:word:nolines\nabc").unwrap();
    assert_eq!(
        office_editor_hit(
            &presentation,
            Rect {
                origin: Point { x: 0, y: 0 },
                size: Size {
                    width: 416,
                    height: 300
                }
            },
            Point { x: 18, y: 30 }
        ),
        None
    );
}

#[test]
fn compact_office_lines_reuse_geometry_and_reject_malformed_row_headers() {
    assert_eq!(hit("~e100,124~d0,96,1~d1,120,1", "x", 119, 110, None), Some(1));
    assert_eq!(hit("~d0,96,1", "x", 96, 110, None), None);
    assert_eq!(hit("~e100,garbage,124~d0,96,1", "x", 96, 110, None), None);
}

#[test]
fn zoomed_out_page_caret_hit_uses_centered_sheet_origin() {
    let presentation = EditorText::parse("editor:rich~o794,1123,0,1~e76,92~d0,76,1~d5,104,0~z6:word:nolines\nChild").unwrap();
    let rect = Rect { origin: Point { x: 10, y: 20 }, size: Size { width: 1000, height: 800 } };
    // Sheet width397, centered left301.5; source104px maps to52px.
    assert_eq!(office_editor_hit(&presentation,rect,Point { x:364,y:70 }),Some(5));
    assert_eq!(office_editor_hit(&presentation,rect,Point { x:350,y:70 }),Some(0));
}
