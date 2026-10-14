use rice_ui::grid_frame::*;
use rice_ui::virtual_grid::GridAxis;

#[test]
fn table_filters_follow_absolute_header_row_and_disappear_when_offscreen() {
    let mut frame=fixture();
    frame.filter_columns=vec![GridFilterColumn{column:frame.columns[1].index,active:true}];
    frame.filter_header_row=Some(frame.rows[1].index);
    let (rect,active)=frame.filter_button(frame.columns[1].index).unwrap();
    assert!(active);
    assert!(rect.origin.y >= frame.column_header as i32);
    assert!(frame.hit_test(rect.origin.x+2,rect.origin.y+2).unwrap().filter);
    assert!(!frame.hit_test(rect.origin.x+2,5).unwrap().filter);
    let encoded=frame.encode().unwrap();
    assert_eq!(GridFrame::parse(&encoded).unwrap().filter_header_row,frame.filter_header_row);
    frame.filter_header_row=Some(900);
    assert!(frame.filter_button(frame.columns[1].index).is_none());
    frame.filter_header_row=Some(frame.row_count);
    assert!(frame.encode().is_err());
}

#[test]
fn expanded_grid_budget_preserves_visible_text_and_remains_bounded() {
    let mut frame=fixture();
    frame.strings=(0..40).map(|index|format!("{index}:{}","x".repeat(2048))).collect();
    let encoded=frame.encode().unwrap();
    assert!(encoded.len()>64*1024);
    assert_eq!(GridFrame::parse(&encoded).unwrap().strings[39],frame.strings[39]);
    frame.strings=(0..150).map(|_|"x".repeat(2048)).collect();
    assert!(frame.encode().is_err());
}

fn fixture() -> GridFrame {
    let rows = GridAxis::new(1_048_576, 20)
        .unwrap()
        .viewport(1, 180, 60)
        .unwrap();
    let columns = GridAxis::new(16_384, 80)
        .unwrap()
        .viewport(1, 160, 160)
        .unwrap();
    GridFrame {
        merges:Vec::new(),
        version: 1,
        revision: 7,
        sheet: 2,
        row_count: 1_048_576,
        column_count: 16_384,
        width: 200,
        height: 80,
        row_header: 40,
        column_header: 20,
        zoom_percent: None, canvas: None,
        rows: rows.tracks.into_iter().map(Into::into).collect(),
        columns: columns.tracks.into_iter().map(Into::into).collect(),
        strings: vec!["120.50".into(), "Budget".into()],
        styles: vec![GridStyle {
            alignment: 2,
            role: 0,
            font: None, paint: None,
        }],
        cells: vec![GridCell(0, 0, 0, 0), GridCell(1, 1, 1, 0)],
        references: vec![],
        selection: Some(GridSelection {
            first_row: 0,
            last_row: 1,
            first_column: 0,
            last_column: 1,
        }),
        active: Some((10, 3)),
        filter_columns: vec![],
        filter_header_row: None,
        fill_endpoint: None,
    }
}

#[test]
fn viewport_preserves_frozen_and_distant_logical_cell_identities() {
    let frame = fixture();
    assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(), frame);
    assert_eq!(frame.rows.len(), 3);
    assert_eq!(frame.rows[1].index, 10);
    assert_eq!(frame.columns[1].index, 3);
    assert_eq!(
        frame.hit_test(121, 41),
        Some(GridHit {
            filter: false,
            fill: false,
            revision: 7,
            sheet: 2,
            row: Some(10),
            column: Some(3)
        })
    );
    assert_eq!(frame.hit_test(41, 21).unwrap().row, Some(0));
    assert_eq!(frame.hit_test(5, 41).unwrap().column, None);
    assert_eq!(frame.hit_test(121, 5).unwrap().row, None);
    assert_eq!(frame.hit_test(0, 0).unwrap().column, None);
    assert!(frame.hit_test(-1, 0).is_none());
    assert!(frame.hit_test(200, 0).is_none());
}

#[test]
fn malformed_frames_fail_before_rendering() {
    let mut frame = fixture();
    frame.cells[0].2 = 99;
    assert!(frame.encode().is_err());
    frame = fixture();
    frame.cells.reverse();
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.cells.push(frame.cells[1].clone());
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.styles[0].role = 4;
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.rows[1].clip_start = 1;
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.columns[1].index = frame.columns[0].index;
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.version = 2;
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.revision = u64::MAX;
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.strings[0] = "bad\nvalue".into();
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.active = Some((frame.row_count, 0));
    assert!(frame.validate().is_err());
    frame = fixture();
    frame.rows[0].start = i64::MAX;
    assert!(frame.validate().is_err());
    assert!(GridFrame::parse(&" ".repeat(MAX_FRAME_BYTES + 1)).is_err());
    let json = fixture()
        .encode()
        .unwrap()
        .replace("\"version\":1", "\"version\":1,\"html\":\"unsafe\"");
    assert!(GridFrame::parse(&json).is_err());
}

#[test]
fn column_labels_cover_excel_limit_and_larger_model_identity() {
    for (index, expected) in [
        (0, "A"),
        (25, "Z"),
        (26, "AA"),
        (701, "ZZ"),
        (702, "AAA"),
        (16383, "XFD"),
    ] {
        assert_eq!(column_label(index), expected);
    }
    assert!(!column_label(u32::MAX).is_empty());
}

#[test]
fn wire_kind_and_payload_use_one_retained_widget() {
    use rice_ui::{Patch, RetainedTree, WidgetId, WidgetKind, WidgetSpec, WidgetState};
    let patch = Patch::Create {
        widget: WidgetSpec {
            id: WidgetId::new(1).unwrap(),
            kind: WidgetKind::VirtualGrid,
            state: WidgetState::ENABLED,
            text: Some(fixture().encode().unwrap()),
            icon: None,
        },
        parent: None,
        index: 0,
    };
    let encoded = patch.to_protocol().unwrap();
    assert_eq!(Patch::try_from(&encoded).unwrap(), patch);
    let mut tree = RetainedTree::default();
    tree.apply(&[patch]).unwrap();
    assert!(
        tree.get(WidgetId::new(1).unwrap())
            .unwrap()
            .children()
            .is_empty()
    );
}

#[test]
fn sorted_visible_rows_keep_identity_separate_from_geometry_and_selection() {
    let mut frame = fixture();
    frame.rows[0].index = 120;
    frame.rows[1].index = 3;
    frame.rows[2].index = 99;
    frame.selection = Some(GridSelection {
        first_row: 1,
        last_row: 1,
        first_column: 1,
        last_column: 1,
    });
    frame.validate().unwrap();
    assert_eq!(frame.hit_test(121, 41).unwrap().row, Some(3));
    assert!(frame.selection.as_ref().unwrap().contains(1, 1));
    assert!(!frame.selection.as_ref().unwrap().contains(0, 1));
}

#[test]
fn optional_filter_buttons_validate_and_hit_only_the_small_header_region() {
    let mut frame = fixture();
    let legacy = frame.encode().unwrap();
    assert!(!legacy.contains("filter_columns"));
    frame.filter_columns = vec![
        GridFilterColumn {
            column: 0,
            active: false,
        },
        GridFilterColumn {
            column: 3,
            active: true,
        },
    ];
    assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(), frame);
    assert!(frame.hit_test(190, 10).unwrap().filter);
    assert_eq!(frame.hit_test(190, 10).unwrap().column, Some(3));
    assert!(frame.hit_test(110, 10).unwrap().filter);
    for (x, y) in [
        (180, 10),
        (199, 10),
        (190, 0),
        (190, 19),
        (190, 21),
        (20, 10),
    ] {
        assert!(
            !frame.hit_test(x, y).unwrap().filter,
            "unexpected filter hit {x},{y}"
        );
    }
    frame.columns[1].clip_end = 100;
    assert!(frame.filter_button(3).is_none());
    frame.filter_columns.push(GridFilterColumn {
        column: 3,
        active: false,
    });
    assert!(frame.validate().is_err());
    frame.filter_columns = vec![GridFilterColumn {
        column: frame.column_count,
        active: false,
    }];
    assert!(frame.validate().is_err());
    frame.filter_columns = (0..513)
        .map(|column| GridFilterColumn {
            column,
            active: false,
        })
        .collect();
    assert!(frame.validate().is_err());
    for invalid in [
        "null",
        "[{\"column\":0,\"active\":1}]",
        "[{\"column\":0,\"active\":true,\"extra\":1}]",
    ] {
        let json = legacy.replacen('{', &format!("{{\"filter_columns\":{invalid},"), 1);
        assert!(GridFrame::parse(&json).is_err());
    }
}

#[test]
fn fill_handle_preserves_filtered_endpoint_identity_and_suppresses_clipped_edges() {
    let mut frame = fixture();
    let (rect, endpoint) = frame.fill_handle().unwrap();
    assert_eq!(endpoint, (10, 3));
    assert_eq!(rect.size, rice_ui::Size { width: 6, height: 6 });
    assert!(frame.hit_test(rect.origin.x+2, rect.origin.y+2).unwrap().fill);
    assert!(!frame.hit_test(rect.origin.x-1, rect.origin.y+2).unwrap().fill);
    assert!(!frame.hit_test(rect.origin.x+6, rect.origin.y+2).unwrap().fill);
    frame.columns[1].clip_end -= 1;
    assert!(frame.fill_handle().is_none());
}

#[test]
fn logical_fill_endpoint_hides_offscreen_and_filtered_end_without_removing_highlight() {
    let mut frame=fixture();let selection=frame.selection.clone();
    for endpoint in [(99,3),(10,99),(9,3)] {
        frame.fill_endpoint=Some(endpoint);frame.validate().unwrap();
        assert!(frame.fill_handle().is_none());assert_eq!(frame.selection,selection);
        assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(),frame);
    }
    frame.fill_endpoint=Some((10,3));assert!(frame.fill_handle().is_some());
    frame.fill_endpoint=Some((frame.row_count,0));assert!(frame.validate().is_err());
}

#[test]
fn native_fill_endpoint_rejects_explicit_null_like_sdk_contract() {
    let json=fixture().encode().unwrap();
    let invalid=json.replacen('{',"{\"fill_endpoint\":null,",1);
    assert!(GridFrame::parse(&invalid).is_err());
}

#[test]
fn optional_document_fonts_validate_and_survive_frame_roundtrip() {
    let mut frame=fixture();
    let legacy=frame.encode().unwrap();assert!(!legacy.contains("\"font\""));
    assert!(GridFrame::parse(&legacy.replace("\"role\":0", "\"role\":0,\"font\":null")).is_err());
    frame.styles[0].font=Some(GridFont{family:"RiceWM Mono".into(),size_pt:GridFontSize::new(18.0).unwrap(),weight:700,italic:true,underline:true});
    assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(),frame);
    frame.styles[0].font.as_mut().unwrap().weight=701;assert!(frame.validate().is_err());
    frame.styles[0].font.as_mut().unwrap().weight=700;
    frame.styles[0].font.as_mut().unwrap().family="bad\0font".into();assert!(frame.validate().is_err());
}

#[test]
fn fractional_document_sizes_are_lossless_numeric_and_orderable() {
    use std::collections::{BTreeSet, HashSet};
    let points=[1.0,11.0,11.25,11.5,14.5,409.0];
    let sizes:Vec<_>=points.into_iter().map(|p|GridFontSize::new(p).unwrap()).collect();
    for (size,expected) in sizes.iter().zip(points) {
        let json=serde_json::to_string(size).unwrap();
        assert!(!json.starts_with('"'));
        let roundtrip:GridFontSize=serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.points().to_bits(),expected.to_bits());
        let mut frame=fixture();
        frame.styles[0].font=Some(GridFont{family:"Arial".into(),size_pt:*size,weight:400,italic:false,underline:false});
        assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(),frame);
    }
    assert_eq!(serde_json::to_string(&sizes[1]).unwrap(),"11","legacy integer JSON stays unchanged");
    assert_eq!(serde_json::from_str::<GridFontSize>("11.0").unwrap(),sizes[1]);
    assert_eq!(sizes.iter().copied().collect::<HashSet<_>>().len(),points.len());
    assert_eq!(sizes.iter().rev().copied().collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),sizes);
    // A representable step smaller than a twip/hundredth must not be rounded.
    let adjacent=GridFontSize::new(f64::from_bits(11.5_f64.to_bits()+1)).unwrap();
    assert_ne!(adjacent,sizes[3]);
    assert_eq!(serde_json::from_str::<GridFontSize>(&serde_json::to_string(&adjacent).unwrap()).unwrap(),adjacent);
    for index in 0..512 {
        let bits=11.0_f64.to_bits()+index*7919;
        let size=GridFontSize::new(f64::from_bits(bits)).unwrap();
        assert_eq!(serde_json::from_str::<GridFontSize>(&serde_json::to_string(&size).unwrap()).unwrap(),size);
    }
}

#[test]
fn invalid_document_font_sizes_are_rejected_before_frame_replacement() {
    for points in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY,-1.0,-0.0,0.0,0.999,409.0001] {
        assert!(GridFontSize::new(points).is_err());
    }
    let mut frame=fixture();
    frame.styles[0].font=Some(GridFont{family:"Arial".into(),size_pt:GridFontSize::new(11.5).unwrap(),weight:400,italic:false,underline:false});
    let original=frame.encode().unwrap();
    for value in ["null","true","\"11.5\"","0","-1","409.5","1e999","NaN"] {
        assert!(GridFrame::parse(&original.replace("\"size_pt\":11.5",&format!("\"size_pt\":{value}"))).is_err(),"accepted {value}");
    }
    assert_eq!(GridFrame::parse(&original).unwrap(),frame);
}

#[test]
fn fractional_font_only_damage_remains_distinct_and_leaves_other_cells_unchanged() {
    use rice_ui::grid_damage::{GridDamage,geometry_key};
    let mut frame=fixture();
    frame.styles[0].font=Some(GridFont{family:"Arial".into(),size_pt:GridFontSize::new(14.0).unwrap(),weight:400,italic:false,underline:false});
    let mut changed=frame.styles[0].clone();changed.font.as_mut().unwrap().size_pt=GridFontSize::new(14.5).unwrap();
    let damage=GridDamage{version:1,sheet:frame.sheet,base_revision:frame.revision,revision:frame.revision+1,base_geometry:geometry_key(&frame),cells:vec![(0,0,Some(frame.strings[0].clone()),Some(changed.clone()))],selection:frame.selection.clone(),active:frame.active,references:None};
    let next=damage.apply(&frame).unwrap();
    assert_eq!(next.styles[next.cells[0].3 as usize],changed);
    assert_eq!(next.styles[next.cells[1].3 as usize],frame.styles[0]);
    assert_ne!(next.cells[0].3,next.cells[1].3);
}

#[test]
fn font_only_damage_interns_complete_style_and_rejects_invalid_font_atomically() {
    use rice_ui::grid_damage::{GridDamage,geometry_key};
    let frame=fixture();
    let font=GridFont{family:"RiceWM Mono".into(),size_pt:GridFontSize::new(18.0).unwrap(),weight:700,italic:true,underline:true};
    let mut damage=GridDamage{version:1,sheet:frame.sheet,base_revision:frame.revision,revision:frame.revision+1,
        base_geometry:geometry_key(&frame),cells:vec![(0,0,Some(frame.strings[0].clone()),Some(GridStyle{alignment:frame.styles[0].alignment,role:frame.styles[0].role,font:Some(font.clone()),paint:None}))],
        selection:frame.selection.clone(),active:frame.active,references:None};
    let updated=damage.apply(&frame).unwrap();let changed=updated.cells.iter().find(|c|c.0==0&&c.1==0).unwrap();
    assert_eq!(updated.styles[changed.3 as usize].font,Some(font));
    assert_eq!(frame.styles[0].font,None,"original frame is immutable");
    damage.cells[0].3.as_mut().unwrap().font.as_mut().unwrap().weight=701;
    assert!(damage.apply(&frame).is_err());assert_eq!(frame.styles[0].font,None);
}

#[test]
fn document_style_palette_above_legacy_limit_preserves_ids_and_total_byte_budget() {
    let mut frame=fixture();
    frame.styles=(0..100).map(|i|GridStyle{alignment:0,role:0,font:Some(GridFont{
        family:"RiceWM Sans".into(),size_pt:GridFontSize::new(f64::from(i+1)).unwrap(),weight:400,italic:false,underline:false
    }),paint:None}).collect();
    frame.cells[0].3=99;
    let parsed=GridFrame::parse(&frame.encode().unwrap()).unwrap();
    assert_eq!(parsed.styles.len(),100);
    assert_eq!(parsed.styles[parsed.cells[0].3 as usize].font.as_ref().unwrap().size_pt.points(),100.0);
    frame.styles=vec![frame.styles[0].clone();4097];assert!(frame.validate().is_err());
    frame.styles=vec![frame.styles[0].clone();4096];
    assert!(frame.encode().is_err(),"larger style palette does not relax the total frame byte budget");
}

#[test]
fn formula_reference_damage_is_bounded_transient_and_atomic() {
    use rice_ui::grid_damage::{GridDamage, geometry_key};
    let original = fixture();
    let reference = GridReference {first_row:0,last_row:10,first_column:0,last_column:3,color:1};
    let damage = GridDamage {version:1,sheet:original.sheet,base_revision:original.revision,revision:original.revision,
        base_geometry:geometry_key(&original),cells:vec![],selection:original.selection.clone(),active:original.active,references:Some(vec![reference.clone()])};
    let highlighted = damage.apply(&original).unwrap();
    assert_eq!(highlighted.references, vec![reference.clone()]);
    assert_eq!(highlighted.cells, original.cells);
    assert_eq!(highlighted.revision, original.revision);
    assert_eq!(geometry_key(&highlighted), geometry_key(&original));
    assert_eq!(GridFrame::parse(&highlighted.encode().unwrap()).unwrap(),highlighted);
    let mut clear = damage.clone(); clear.references=Some(vec![]);
    assert_eq!(clear.apply(&highlighted).unwrap(),original);
    let mut invalid = damage.clone();invalid.references=Some(vec![reference.clone();17]);
    assert!(invalid.apply(&original).is_err());
    invalid.references=Some(vec![GridReference{last_row:original.row_count,..reference}]);
    assert!(invalid.apply(&original).is_err());
    assert!(original.references.is_empty());
}

#[test]
fn document_paint_roundtrips_all_border_styles_and_rejects_invalid_metadata() {
    for style in ["hair","thin","medium","thick","double","dotted","dashed","dashDot","dashDotDot","mediumDashed","mediumDashDot","mediumDashDotDot","slantDashDot"] {
        let mut f=fixture();
        let json=format!(r##"{{"foreground":"#112233","background":"#abcdef","vertical":"bottom","wrap":true,"strike":false,"borders":{{"top":{{"style":"{}","color":"#123456"}}}}}}"##,style);
        f.styles[0].paint=Some(serde_json::from_str(&json).unwrap());
        assert_eq!(GridFrame::parse(&f.encode().unwrap()).unwrap(),f);
    }
    for invalid in ["null","[]",r##"{"foreground":"#123"}"##,r##"{"background":"red"}"##,r#"{"vertical":"middle"}"#,r#"{"wrap":1}"#,r#"{"strike":null}"#,r#"{"unexpected":true}"#,r#"{"borders":{"top":null}}"#,r##"{"borders":{"diagonal":{"style":"thin","color":"#123456"}}}"##,r##"{"borders":{"top":{"style":"other","color":"#123456"}}}"##] {
        let json=fixture().encode().unwrap().replacen("\"role\":0",&format!("\"role\":0,\"paint\":{invalid}"),1);
        assert!(GridFrame::parse(&json).is_err(),"accepted {invalid}");
    }
}

#[test]
fn paint_only_damage_preserves_blank_styled_cells_and_rejects_invalid_color_atomically() {
    use rice_ui::grid_damage::{GridDamage,geometry_key};
    let frame=fixture();
    let mut style=frame.styles[0].clone();
    style.paint=Some(serde_json::from_str(r##"{"background":"#ff0000","borders":{"bottom":{"style":"double","color":"#112233"}}}"##).unwrap());
    let mut damage=GridDamage{version:1,sheet:frame.sheet,base_revision:frame.revision,revision:frame.revision+1,base_geometry:geometry_key(&frame),cells:vec![(0,0,Some(String::new()),Some(style.clone()))],selection:frame.selection.clone(),active:frame.active,references:None};
    let updated=damage.apply(&frame).unwrap();let cell=updated.cells.iter().find(|c|c.0==0&&c.1==0).unwrap();
    assert_eq!(updated.strings[cell.2 as usize],"");assert_eq!(updated.styles[cell.3 as usize],style);assert!(frame.styles[0].paint.is_none());
    damage.cells[0].3.as_mut().unwrap().paint.as_mut().unwrap().background=Some("url(bad)".into());assert!(damage.apply(&frame).is_err());assert!(frame.styles[0].paint.is_none());
}

#[test]
fn view_zoom_roundtrips_and_changes_damage_geometry_without_modifying_fonts() {
    use rice_ui::grid_damage::geometry_key;
    let original=fixture();let original_key=geometry_key(&original);
    for zoom in [25,50,100,125,200,400] {
        let mut frame=original.clone();frame.zoom_percent=Some(zoom);assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap(),frame);assert_ne!(geometry_key(&frame),original_key);assert_eq!(frame.styles,original.styles);
    }
    for zoom in ["null","24","401","125.5","\"125\""] {
        let json=original.encode().unwrap().replacen('{',&format!("{{\"zoom_percent\":{zoom},"),1);assert!(GridFrame::parse(&json).is_err());
    }
}

#[test]
fn document_canvas_is_strict_view_metadata_and_invalidates_legacy_damage() {
    use rice_ui::grid_damage::{GridDamage,geometry_key,GridDamageError};
    let original=fixture();let key=geometry_key(&original);
    let mut document=original.clone();document.canvas=Some("document".into());
    assert_eq!(GridFrame::parse(&document.encode().unwrap()).unwrap(),document);
    assert_ne!(key,geometry_key(&document));assert_eq!(document.cells,original.cells);assert_eq!(document.styles,original.styles);
    let damage=GridDamage{version:1,sheet:original.sheet,base_revision:original.revision,revision:original.revision+1,base_geometry:key,cells:vec![],selection:original.selection.clone(),active:original.active,references:None};
    assert_eq!(damage.apply(&document),Err(GridDamageError::BaseMismatch));
    document.canvas=None;assert_eq!(geometry_key(&document),key);assert_eq!(document.encode().unwrap(),original.encode().unwrap());
    for value in ["null","false","1","{}","[]","\"dark\"","\"Document\""] {
        let json=original.encode().unwrap().replacen('{',&format!("{{\"canvas\":{value},"),1);
        assert!(GridFrame::parse(&json).is_err(),"accepted {value}");
    }
}

#[test]
fn document_canvas_geometry_matches_sdk_fnv_optional_field_order() {
    use rice_ui::grid_damage::geometry_key;
    let mut f=GridFrame::parse(r#"{"version":1,"revision":1,"sheet":0,"row_count":100,"column_count":20,"width":200,"height":80,"row_header":40,"column_header":20,"rows":[{"index":0,"start":0,"extent":20,"clip_start":0,"clip_end":20,"frozen":false}],"columns":[{"index":0,"start":0,"extent":80,"clip_start":0,"clip_end":80,"frozen":false}],"strings":["9"],"styles":[{"alignment":0,"role":0}],"cells":[[0,0,0,0]],"selection":null,"active":[0,0]}"#).unwrap();
    assert_eq!(geometry_key(&f),4287419196);
    f.canvas=Some("document".into());assert_eq!(geometry_key(&f),1033518023);
    f.zoom_percent=Some(150);f.fill_endpoint=Some((0,0));f.filter_header_row=Some(0);assert_eq!(geometry_key(&f),1121918735);
}

#[test]
fn merged_cells_preserve_offscreen_anchor_and_validate_geometry() {
    let mut f=fixture();
    f.rows=GridAxis::new(100,20).unwrap().viewport(0,20,60).unwrap().tracks.into_iter().map(Into::into).collect();
    f.columns=GridAxis::new(100,80).unwrap().viewport(0,80,160).unwrap().tracks.into_iter().map(Into::into).collect();
    f.selection=None;
    f.merges=vec![GridMerge{first_row:0,last_row:2,first_column:0,last_column:2,row_slot:0,column_slot:0,x:-80,y:-20,width:240,height:60}];
    let encoded=f.encode().unwrap();assert_eq!(GridFrame::parse(&encoded).unwrap(),f);
    let hit=f.hit_test(150,45).unwrap();assert_eq!((hit.row,hit.column),(Some(0),Some(0)));
    f.merges.push(f.merges[0].clone());assert!(f.encode().is_err());f.merges.pop();
    f.merges[0].width=81;assert!(f.encode().is_err());
}

#[test]
fn multiline_display_requires_wrapping_on_every_reference_and_damage_style() {
    use rice_ui::grid_damage::{GridDamage, geometry_key};
    let mut frame=fixture();
    frame.strings[0]="First\nSecond".into();
    assert!(frame.validate().is_err());
    frame.styles[0].paint=Some(serde_json::from_str(r#"{"wrap":true}"#).unwrap());
    assert!(frame.validate().is_ok());
    assert_eq!(GridFrame::parse(&frame.encode().unwrap()).unwrap().strings[0],"First\nSecond");
    let mut bad=frame.clone();bad.strings.push("unused\nvalue".into());assert!(bad.validate().is_err());
    let mut bad=frame.clone();let mut unwrapped=frame.styles[0].clone();unwrapped.paint=None;bad.styles.push(unwrapped);bad.cells[1]=GridCell(1,1,0,1);assert!(bad.validate().is_err());
    for control in ['\r','\t','\0','\u{85}'] {let mut bad=frame.clone();bad.strings[0]=format!("a{control}b");assert!(bad.validate().is_err());}
    let mut damage=GridDamage{version:1,sheet:frame.sheet,base_revision:frame.revision,revision:frame.revision+1,base_geometry:geometry_key(&frame),cells:vec![(0,0,Some("New\nLines".into()),Some(frame.styles[0].clone()))],selection:frame.selection.clone(),active:frame.active,references:None};
    assert_eq!(damage.apply(&frame).unwrap().strings[0],"New\nLines");
    damage.cells[0].3.as_mut().unwrap().paint=None;assert!(damage.validate().is_err());
    damage.cells[0].3=Some(frame.styles[0].clone());damage.cells[0].2=Some("New\rLines".into());assert!(damage.validate().is_err());
}
