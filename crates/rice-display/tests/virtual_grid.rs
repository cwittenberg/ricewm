use rice_display::native_ui::*;
use rice_native_app_protocol::{AppId, UiPatchEnvelope, WidgetId, WindowId};
use rice_native_app_protocol::{PointerEventKind, UiEvent, UiPatch};
use rice_ui::grid_frame::{GridCell, GridFrame, GridSelection, GridStyle};
use rice_ui::virtual_grid::GridAxis;
use rice_ui::{Constraints, Point, Size};
use rice_ui::{WidgetKind, WidgetState};

fn frame() -> GridFrame {
    GridFrame {
        merges:Vec::new(),
        version: 1,
        revision: 9,
        sheet: 3,
        row_count: 1_048_576,
        column_count: 16_384,
        width: 200,
        height: 80,
        row_header: 40,
        column_header: 20,
        zoom_percent: None, canvas: None,
        rows: GridAxis::new(1_048_576, 20)
            .unwrap()
            .viewport(1, 185, 60)
            .unwrap()
            .tracks
            .into_iter()
            .map(Into::into)
            .collect(),
        columns: GridAxis::new(16_384, 80)
            .unwrap()
            .viewport(1, 165, 160)
            .unwrap()
            .tracks
            .into_iter()
            .map(Into::into)
            .collect(),
        strings: vec!["Budget".into(), "125.50".into()],
        styles: vec![
            GridStyle {
                alignment: 0,
                role: 1,
                font: None, paint: None,
            },
            GridStyle {
                alignment: 2,
                role: 0,
                font: None, paint: None,
            },
        ],
        cells: vec![GridCell(0, 0, 0, 0), GridCell(1, 1, 1, 1)],
        selection: Some(GridSelection {
            first_row: 1,
            last_row: 1,
            first_column: 1,
            last_column: 1,
        }),
        active: Some((10, 3)),
        filter_columns: vec![],
        filter_header_row: None,
        fill_endpoint: None,
        references: vec![],
    }
}
fn ids() -> (AppId, WindowId, WidgetId) {
    (
        AppId::new(1).unwrap(),
        WindowId::new(1).unwrap(),
        WidgetId::new(2).unwrap(),
    )
}
fn host(text: String) -> NativeUiHost {
    let (app, window, id) = ids();
    let root = WidgetId::new(1).unwrap();
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: root,
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id,
                parent: Some(root),
                index: 0,
                kind: WidgetKind::VirtualGrid as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(text),
                icon: None,
            },
        ],
    })
    .unwrap();
    host
}
fn constraints() -> Constraints {
    Constraints::new(
        Size {
            width: 300,
            height: 150,
        },
        Size {
            width: 300,
            height: 150,
        },
    )
    .unwrap()
}

#[test]
fn table_filter_arrow_is_painted_and_hit_in_the_actual_data_header() {
    use rice_ui::grid_frame::GridFilterColumn;
    let (app,window,id)=ids();
    let mut frame=frame();
    frame.filter_header_row=Some(frame.rows[1].index);
    frame.filter_columns=vec![GridFilterColumn{column:frame.columns[1].index,active:false}];
    let (button,_)=frame.filter_button(frame.columns[1].index).unwrap();
    assert!(button.origin.y>=frame.column_header as i32);
    let mut host=host(frame.encode().unwrap());
    let output=host.render(app,window,constraints()).unwrap();
    let origin=output.hit_targets[0].rect.origin;
    let point=Point{x:origin.x+button.origin.x+3,y:origin.y+button.origin.y+3};
    assert!(output.commands.iter().any(|command|matches!(command,DrawCommand::Fill{rect,radius:2,..}
        if rect.origin.x==origin.x+button.origin.x && rect.origin.y==origin.y+button.origin.y)));
    let event=host.pointer_event(app,window,PointerEventKind::Down,point,1,0).unwrap().unwrap();
    assert_eq!(event.target,id);
    let UiEvent::Pointer{x,y,..}=event.event else{panic!("native pointer expected")};
    let hit=frame.hit_test(x,y).unwrap();
    assert!(hit.filter);
    assert_eq!(hit.row,frame.filter_header_row);
}

#[test]
fn virtual_grid_renders_real_values_headers_selection_and_balanced_clips() {
    let (app, window, id) = ids();
    let mut host = host(frame().encode().unwrap());
    let output = host.render(app, window, constraints()).unwrap();
    let strings: Vec<_> = output
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    for value in ["Budget", "125.50", "A", "D", "1", "11"] {
        assert!(strings.contains(&value), "missing {value}");
    }
    assert!(
        !strings.iter().any(|s| s.contains("\"version\"")),
        "frame JSON is never rendered as text"
    );
    assert_eq!(
        output.hit_targets.len(),
        1,
        "cells do not become retained widgets or targets"
    );
    assert_eq!(output.hit_targets[0].widget, id);
    let mut depth = 0;
    let mut value_clip = None;
    let mut clips = Vec::new();
    for command in &output.commands {
        match command {
            DrawCommand::PushClip { rect } => {
                depth += 1;
                clips.push(*rect);
            }
            DrawCommand::PopClip => {
                depth -= 1;
                assert!(depth >= 0);
                clips.pop();
            }
            DrawCommand::Text { text, align, .. } if text == "125.50" => {
                assert_eq!(*align, TextAlign::End);
                value_clip = clips.last().copied();
            }
            _ => {}
        }
    }
    assert_eq!(depth, 0);
    let clip = value_clip.expect("cell clipping");
    assert_eq!(
        clip.size,
        Size {
            width: 75,
            height: 15
        },
        "scrolled cell is clipped at frozen pane boundary"
    );
    assert!(output.commands.iter().any(|command| matches!(command,
        DrawCommand::Fill { color, .. } if *color == host.theme().palette.selection)));
    assert!(output.commands.iter().any(|command| matches!(command,
        DrawCommand::Border { width: 2, color, .. } if *color == host.theme().palette.focus)));
}

#[test]
fn virtual_grid_pointer_coordinates_use_actual_widget_origin_and_focus() {
    let (app, window, id) = ids();
    let frame = frame();
    let mut host = host(frame.encode().unwrap());
    let output = host.render(app, window, constraints()).unwrap();
    let rect = output.hit_targets[0].rect;
    let event = host
        .pointer_event(
            app,
            window,
            PointerEventKind::Down,
            Point {
                x: rect.origin.x + 121,
                y: rect.origin.y + 41,
            },
            1,
            0,
        )
        .unwrap()
        .unwrap();
    assert_eq!(event.target, id);
    match event.event {
        UiEvent::Pointer { x, y, .. } => {
            assert_eq!((x, y), (121, 41));
            let hit = frame.hit_test(x, y).unwrap();
            assert_eq!((hit.revision, hit.row, hit.column), (9, Some(10), Some(3)));
        }
        other => panic!("expected pointer, received {other:?}"),
    }
    let key = host
        .key_event(app, window, true, 23, 0, None)
        .unwrap()
        .unwrap();
    assert_eq!(
        key.target, id,
        "Tab is delivered to the grid rather than leaving its cell navigation"
    );
}

#[test]
fn virtual_grid_invalid_frames_fail_without_rendering_fabricated_cells() {
    let (app, window, _) = ids();
    // Deliberately bypass the producer's validating encode method.
    let text = frame()
        .encode()
        .unwrap()
        .replace("[0,0,0,0]", "[0,0,65535,0]");
    let mut host = host(text);
    assert!(matches!(
        host.render(app, window, constraints()),
        Err(NativeUiError::InvalidPresentation(_))
    ));
}

#[test]
fn optional_filter_buttons_render_native_arrows_and_route_precise_widget_local_hits() {
    use rice_ui::grid_frame::GridFilterColumn;
    let (app, window, id) = ids();
    let mut frame = frame();
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
    let mut host = host(frame.encode().unwrap());
    let output = host.render(app, window, constraints()).unwrap();
    assert_eq!(
        output.hit_targets.len(),
        1,
        "filter controls remain part of one retained grid"
    );
    let origin = output.hit_targets[0].rect.origin;
    for (column, active) in [(0, false), (3, true)] {
        let (button, actual_active) = frame.filter_button(column).unwrap();
        assert_eq!(actual_active, active);
        let point = Point {
            x: origin.x + button.origin.x + 8,
            y: origin.y + button.origin.y + 8,
        };
        let event = host
            .pointer_event(app, window, PointerEventKind::Down, point, 1, 0)
            .unwrap()
            .unwrap();
        assert_eq!(event.target, id);
        let UiEvent::Pointer { x, y, .. } = event.event else {
            panic!("expected native pointer");
        };
        let hit = frame.hit_test(x, y).unwrap();
        assert!(hit.filter);
        assert_eq!(hit.column, Some(column));
        assert_eq!(hit.revision, 9);
        assert!(output.commands.iter().any(|command| matches!(command,
            DrawCommand::Fill { rect, color, radius: 2 } if rect.origin.x == origin.x + button.origin.x
                && rect.origin.y == origin.y + button.origin.y && rect.size == button.size
                && *color == if active { host.theme().palette.selection } else { host.theme().palette.surface_raised })));
        let bars: &[(i32,u32)] = if active { &[(0,9),(1,7),(2,5),(3,3),(4,3),(5,3),(6,3)] } else { &[(0,7),(1,5),(2,3),(3,1)] };
        for &(offset, width) in bars {
            assert!(output.commands.iter().any(|command| matches!(command,
                DrawCommand::Fill { rect, color, radius: 0 }
                    if rect.origin.x == origin.x + button.origin.x + if active {3+(9-width as i32)/2} else {4+offset}
                    && rect.origin.y == origin.y + button.origin.y + button.size.height as i32 / 2 - if active {3} else {2} + offset
                    && rect.size == Size { width, height: 1 }
                    && *color == if active { host.theme().palette.selection_text } else { host.theme().palette.text_muted })));
        }
    }
    let mut depth = 0;
    for command in &output.commands {
        match command {
            DrawCommand::PushClip { .. } => depth += 1,
            DrawCommand::PopClip => depth -= 1,
            _ => {}
        }
        assert!(depth >= 0);
    }
    assert_eq!(depth, 0);
}

#[test]
fn native_selection_draws_the_same_fill_handle_that_hit_testing_targets() {
    let (app,window,_) = ids(); let frame=frame();
    let (handle,_) = frame.fill_handle().unwrap();
    let mut host=host(frame.encode().unwrap());let output=host.render(app,window,constraints()).unwrap();
    let origin=output.hit_targets[0].rect.origin;
    assert!(output.commands.iter().any(|command| matches!(command, DrawCommand::Fill {rect,color,radius:0}
        if rect.origin.x==origin.x+handle.origin.x && rect.origin.y==origin.y+handle.origin.y
            && rect.size==handle.size && *color==host.theme().palette.focus)));
    assert!(frame.hit_test(handle.origin.x+2,handle.origin.y+2).unwrap().fill);
}

#[test]
fn native_offscreen_logical_endpoint_preserves_highlight_but_draws_no_false_fill_square() {
    let(app,window,_)=ids();let mut frame=frame();frame.fill_endpoint=Some((500,3));
    let mut host=host(frame.encode().unwrap());let output=host.render(app,window,constraints()).unwrap();
    assert!(output.commands.iter().any(|c| matches!(c,DrawCommand::Fill{color,..} if *color==host.theme().palette.selection)));
    assert!(!output.commands.iter().any(|c| matches!(c,DrawCommand::Fill{rect,color,..}
        if rect.size==Size{width:6,height:6} && *color==host.theme().palette.focus)));
}

#[test]
fn grid_document_font_is_a_native_styled_text_command_with_theme_owned_color() {
    let mut frame=frame();
    let font=rice_ui::grid_frame::GridFont{family:"RiceWM Mono".into(),size_pt:rice_ui::grid_frame::GridFontSize::new(18.0).unwrap(),weight:700,italic:true,underline:true};
    frame.styles[1].font=Some(font.clone());
    let mut host=host(frame.encode().unwrap());let (app,window,_)=ids();
    let output=host.render(app,window,constraints()).unwrap();
    let styles:Vec<_>=output.commands.iter().filter_map(|c|if let DrawCommand::StyledText{text,font,..}=c{Some((text,font))}else{None}).collect();
    assert_eq!(styles,vec![(&"125.50".to_string(),&font)]);
    assert_eq!(output.hit_targets.len(),1,"document font does not allocate cell widgets");
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Text{text,..} if text=="Budget")),"plain themed labels remain unchanged");
}

#[test]
fn fractional_grid_fonts_survive_plain_and_painted_rendering_at_view_zoom() {
    use rice_ui::grid_frame::{GridFont,GridFontSize,GridPaint};
    for size in [11.5,14.5,11.25] {
        for zoom in [70,100,150] {
            for painted in [false,true] {
                let mut frame=frame();frame.zoom_percent=Some(zoom);
                let font=GridFont{family:"Arial".into(),size_pt:GridFontSize::new(size).unwrap(),weight:700,italic:true,underline:true};
                frame.styles[1].font=Some(font.clone());
                if painted {frame.styles[1].paint=Some(GridPaint{foreground:Some("#000000".into()),..GridPaint::default()});}
                let mut host=host(frame.encode().unwrap());let(app,window,_)=ids();
                let output=host.render(app,window,constraints()).unwrap();
                let found=output.commands.iter().find_map(|c|match c {
                    DrawCommand::StyledText{text,font,zoom_percent,..} if text=="125.50"=>Some((font,*zoom_percent)),
                    DrawCommand::GridText{text,font:Some(font),zoom_percent,..} if text=="125.50"=>Some((font,*zoom_percent)),
                    _=>None,
                }).unwrap();
                assert_eq!(found,(&font,zoom));
                assert_eq!(found.0.size_pt.points().to_bits(),size.to_bits());
                assert_eq!(output.hit_targets.len(),1);
            }
        }
    }
}

#[test]
fn fill_handle_has_a_theme_contrast_edge_without_changing_its_hit_bounds() {
    let (app,window,_)=ids();
    for theme in [rice_ui::Theme::familiar_light(),rice_ui::Theme::ricewm_dark()] {
        let frame=frame();let (handle,_)=frame.fill_handle().unwrap();
        let mut host=host(frame.encode().unwrap());host.set_theme(theme);
        let output=host.render(app,window,constraints()).unwrap();
        let origin=output.hit_targets[0].rect.origin;
        let rect=rice_ui::Rect{origin:Point{x:origin.x+handle.origin.x,y:origin.y+handle.origin.y},size:handle.size};
        assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Border{rect:r,color,width:1,radius:0} if *r==rect&&*color==theme.palette.selection_text)));
        assert_ne!(theme.palette.selection_text,theme.palette.selection);
        assert!(frame.hit_test(handle.origin.x+3,handle.origin.y+3).unwrap().fill);
    }
}

#[test]
fn active_fill_drag_keeps_crosshair_over_destination_and_clears_with_state() {
    use rice_native_app_protocol::PatchValue;
    let(app,window,id)=ids();let mut host=host(frame().encode().unwrap());
    let output=host.render(app,window,constraints()).unwrap();
    let origin=output.hit_targets[0].rect.origin;let point=Point{x:origin.x+60,y:origin.y+70};
    host.pointer_event(app,window,PointerEventKind::Move,point,0,0).unwrap();
    assert_ne!(host.render(app,window,constraints()).unwrap().cursor,PointerCursor::Crosshair);
    for (sequence,pressed) in [(2,true),(3,false)] {
        host.apply_patch(&UiPatchEnvelope{app,window,sequence,patches:vec![UiPatch::Update{id,
            state:PatchValue::Set((WidgetState::ENABLED|if pressed{WidgetState::PRESSED}else{WidgetState::empty()}).bits()),
            text:PatchValue::Keep,icon:PatchValue::Keep}]}).unwrap();
        let cursor=host.render(app,window,constraints()).unwrap().cursor;
        assert_eq!(cursor==PointerCursor::Crosshair,pressed);
    }
}

#[test]
fn document_paint_renders_blank_fill_borders_layout_and_retains_selection_outline() {
    use rice_ui::grid_frame::{GridPaint,GridBorders,GridBorder};
    use rice_ui::Color;
    let (app,window,_)=ids();let mut frame=frame();
    frame.styles[1].paint=Some(GridPaint{foreground:Some("#123456".into()),background:Some("#abcdef".into()),vertical:Some("bottom".into()),wrap:Some(true),strike:Some(true),borders:Some(GridBorders{bottom:Some(GridBorder{style:"double".into(),color:"#654321".into()}),..Default::default()})});
    let mut host=host(frame.encode().unwrap());let output=host.render(app,window,constraints()).unwrap();
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::GridText{text,color,vertical:2,wrap:true,strike:true,..} if text=="125.50"&&*color==Color::opaque(0x12,0x34,0x56))));
    assert_eq!(output.commands.iter().filter(|c|matches!(c,DrawCommand::StrokeLine{color,width:1,pattern:0,..} if *color==Color::opaque(0x65,0x43,0x21))).count(),2);
    let last_border=output.commands.iter().rposition(|c|matches!(c,DrawCommand::Border{width:2,..})).unwrap();
    let document_border=output.commands.iter().rposition(|c|matches!(c,DrawCommand::StrokeLine{..})).unwrap();assert!(last_border>document_border,"selection stays visible over document borders");
    frame.strings[1]=String::new();frame.selection=None;frame.active=None;
    let mut host=super_host(frame.encode().unwrap());let output=host.render(app,window,constraints()).unwrap();
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..} if *color==Color::opaque(0xab,0xcd,0xef))));
}
fn super_host(text:String)->NativeUiHost{host(text)}

#[test]
fn autofilter_headers_are_bold_and_active_filter_has_a_funnel_stem() {
    use rice_ui::grid_frame::GridFilterColumn;
    let(app,window,_)=ids();let mut frame=frame();frame.filter_header_row=Some(frame.rows[0].index);frame.filter_columns=vec![GridFilterColumn{column:frame.columns[0].index,active:true}];
    let (button,_)=frame.filter_button(frame.columns[0].index).unwrap();
    let mut host=host(frame.encode().unwrap());let output=host.render(app,window,constraints()).unwrap();
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::StyledText{text,font,..} if text=="Budget"&&font.weight==700)));
    let origin=output.hit_targets[0].rect.origin;
    let stems=output.commands.iter().filter(|c|matches!(c,DrawCommand::Fill{rect,radius:0,..} if rect.size.width==3&&rect.size.height==1&&rect.origin.x>=origin.x+button.origin.x&&rect.origin.x<origin.x+button.origin.x+button.size.width as i32&&rect.origin.y>=origin.y+button.origin.y&&rect.origin.y<origin.y+button.origin.y+button.size.height as i32)).count();
    assert!(stems>=4,"active funnel must have a visible stem, found {stems}");
}

#[test]
fn automatic_foreground_has_contrast_on_explicit_fill_but_document_ink_is_exact() {
    use rice_ui::grid_frame::GridPaint;
    use rice_ui::Color;
    let(app,window,_)=ids();
    for background in ["#ffff00","#000000"] {
        let mut f=frame();f.selection=None;f.active=None;f.styles[1].paint=Some(GridPaint{background:Some(background.into()),..Default::default()});let mut h=host(f.encode().unwrap());let output=h.render(app,window,constraints()).unwrap();
        let color=output.commands.iter().find_map(|c|if let DrawCommand::GridText{text,color,..}=c{(text=="125.50").then_some(*color)}else{None}).unwrap();
        let [r,g,b,_]=color.channels();if background=="#ffff00" {assert!(r<100&&g<100&&b<100);}else{assert!(r>150&&g>150&&b>150);}
        f.styles[1].paint.as_mut().unwrap().foreground=Some("#ff0000".into());let mut h=host(f.encode().unwrap());assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::GridText{text,color,..}if text=="125.50"&&*color==Color::opaque(255,0,0))));
    }
}

#[test]
fn document_canvas_keeps_paper_ink_and_authored_colors_across_desktop_themes() {
    use rice_ui::{Color,Theme};
    use rice_ui::grid_frame::GridPaint;
    let(app,window,_)=ids();
    for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
        for (foreground,background,wanted) in [
            (None,None,Color::opaque(0,0,0)),
            (Some("#000000"),None,Color::opaque(0,0,0)),
            (None,Some("#FFFFFF"),Color::opaque(0,0,0)),
            (None,Some("#000000"),Color::opaque(255,255,255)),
            (Some("#FFFFFF"),Some("#000000"),Color::opaque(255,255,255)),
            (Some("#123456"),Some("#ABCDEF"),Color::opaque(0x12,0x34,0x56)),
        ] {
            let mut f=frame();f.canvas=Some("document".into());f.selection=None;f.active=None;
            f.styles[1].paint=Some(GridPaint{foreground:foreground.map(str::to_owned),background:background.map(str::to_owned),..Default::default()});
            let mut h=host(f.encode().unwrap());h.set_theme(theme);let output=h.render(app,window,constraints()).unwrap();
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::GridText{text,color,..} if text=="125.50"&&*color==wanted)));
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..} if *color==Color::opaque(255,255,255))),"document paper is white even under dark desktop");
            let header=rice_ui::grid_frame::column_label(f.columns[0].index);
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Text{text,color,..} if *text==header&&*color==theme.palette.text_muted)),"header stays native");
            if let Some(rgb)=background {let rgb=u32::from_str_radix(&rgb[1..],16).unwrap();let fill=Color::opaque((rgb>>16)as u8,(rgb>>8)as u8,rgb as u8);assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..}if *color==fill)));}
        }
        let mut legacy=frame();legacy.selection=None;legacy.active=None;
        let mut h=host(legacy.encode().unwrap());h.set_theme(theme);
        assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::Text{text,color,..}if text=="125.50"&&*color==theme.palette.text)),"absence preserves legacy theme ink");
    }
}

#[test]
fn document_selection_tints_fill_without_replacing_document_ink_or_native_focus() {
    use rice_ui::{Color,Theme};use rice_ui::grid_frame::GridPaint;
    let(app,window,_)=ids();
    for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
        for background in [None,Some("#FFF000")] {
            let mut f=frame();f.canvas=Some("document".into());f.styles[1].paint=Some(GridPaint{foreground:Some("#000000".into()),background:background.map(str::to_owned),..Default::default()});
            let mut h=host(f.encode().unwrap());h.set_theme(theme);let output=h.render(app,window,constraints()).unwrap();
            let base=if background.is_some(){[255u16,240,0]}else{[255u16;3]};let accent=theme.palette.selection.channels();let tint=Color::opaque(((base[0]*3+u16::from(accent[0]))/4)as u8,((base[1]*3+u16::from(accent[1]))/4)as u8,((base[2]*3+u16::from(accent[2]))/4)as u8);
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..}if *color==tint)));
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::GridText{text,color,..}if text=="125.50"&&*color==Color::opaque(0,0,0))));
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Border{width:2,color,..}if *color==theme.palette.focus)));
        }
    }
}

#[test]
fn inline_document_paint_is_validated_atomic_and_clear_restores_theme() {
    use rice_ui::{Color,Theme};use rice_native_app_protocol::PatchValue;
    let(app,window,id)=ids();
    for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
        let mut h=NativeUiHost::default();h.set_theme(theme);
        h.apply_patch(&UiPatchEnvelope{app,window,sequence:1,patches:vec![UiPatch::Create{id,parent:None,index:0,kind:WidgetKind::TextField as u16,state:(WidgetState::ENABLED|WidgetState::FOCUSED).bits(),text:Some("104".into()),icon:None}]}).unwrap();
        let mut sequence=2;
        for (paint,wanted,fill) in [(r##"{"background":"#FFFFFF","foreground":"#000000"}"##,Color::opaque(0,0,0),Color::opaque(255,255,255)),(r##"{"background":"#000000"}"##,Color::opaque(255,255,255),Color::opaque(0,0,0))] {
            let font=format!(r#"{{"family":"Arial","size_pt":10,"weight":400,"italic":false,"underline":false,"zoom_percent":150,"paint":{paint}}}"#);
            h.apply_patch(&UiPatchEnvelope{app,window,sequence,patches:vec![UiPatch::DocumentFont{id,font:Some(font)}]}).unwrap();sequence+=1;
            let output=h.render(app,window,constraints()).unwrap();
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::StyledEdit{text,color,zoom_percent:150,caret:Some(_),..}if text=="104"&&*color==wanted)));
            assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..}if *color==fill)));
        }
        for invalid in ["null","[]",r##"["#FFFFFF"]"##,r##"["#FFFFFF","#000000"]"##,r##"{"foreground":"#000000"}"##,r##"{"background":null}"##,r##"{"background":"#FFF"}"##,r##"{"background":"#FFFFFF","foreground":null}"##,r##"{"background":"#FFFFFF","extra":1}"##] {
            let font=format!(r#"{{"family":"Arial","size_pt":10,"weight":400,"italic":false,"underline":false,"paint":{invalid}}}"#);
            assert!(h.apply_patch(&UiPatchEnvelope{app,window,sequence,patches:vec![UiPatch::Update{id,state:PatchValue::Keep,text:PatchValue::Set("corrupted".into()),icon:PatchValue::Keep},UiPatch::DocumentFont{id,font:Some(font)}]}).is_err());
            assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::StyledEdit{text,color,..}if text=="104"&&*color==Color::opaque(255,255,255))));
        }
        h.apply_patch(&UiPatchEnvelope{app,window,sequence,patches:vec![UiPatch::DocumentFont{id,font:None}]}).unwrap();
        assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::Text{text,color,..}if text=="104"&&*color==theme.palette.text)));
    }
}

#[test]
fn document_reference_inks_follow_paper_not_desktop_and_legacy_stays_themed() {
    use rice_ui::{Color,Theme};use rice_ui::grid_frame::GridReference;
    let(app,window,_)=ids();let paper=Color::opaque(255,255,255);
    for slot in 0..8 {
        let wanted=Theme::familiar_light().formula_reference_ink_on(slot,paper);
        assert_ne!(wanted,Theme::ricewm_dark().formula_reference_ink(slot));
        for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
            assert_eq!(theme.formula_reference_ink_on(slot,paper),wanted);
            for document in [false,true] {
                let mut f=frame();f.canvas=document.then(||"document".into());f.references=vec![GridReference{first_row:f.rows[1].index,last_row:f.rows[1].index,first_column:f.columns[1].index,last_column:f.columns[1].index,color:slot}];
                let mut h=host(f.encode().unwrap());h.set_theme(theme);let output=h.render(app,window,constraints()).unwrap();let ink=if document{wanted}else{theme.formula_reference_ink(slot)};
                assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Fill{color,..}if *color==ink)),"reference slot{slot} document={document}");
            }
        }
    }
}

#[test]
fn document_alignment_defaults_only_document_cells_to_bottom() {
    use rice_ui::{Color,Theme};
    use rice_ui::grid_frame::{GridFont,GridFontSize,GridPaint,GridMerge};
    let(app,window,_)=ids();
    for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
        for merged in [false,true] {
            for font in [None,Some(GridFont{family:"Arial".into(),size_pt:GridFontSize::new(14.5).unwrap(),weight:400,italic:false,underline:false})] {
                for paint in [None,Some(GridPaint::default()),Some(GridPaint{foreground:Some("#123456".into()),..Default::default()}),Some(GridPaint{vertical:Some("top".into()),..Default::default()}),Some(GridPaint{vertical:Some("center".into()),..Default::default()}),Some(GridPaint{vertical:Some("bottom".into()),..Default::default()})] {
                    for document in [false,true] {
                        let mut f=frame();f.canvas=document.then(||"document".into());f.selection=None;f.active=None;f.height=200;
                        f.rows=GridAxis::new(f.row_count,60).unwrap().viewport(0,0,180).unwrap().tracks.into_iter().map(Into::into).collect();
                        f.columns=GridAxis::new(f.column_count,80).unwrap().viewport(0,0,160).unwrap().tracks.into_iter().map(Into::into).collect();
                        f.cells=vec![GridCell(0,0,0,0)];f.styles[0].font=font.clone();f.styles[0].paint=paint.clone();
                        if merged {f.merges=vec![GridMerge{first_row:0,last_row:2,first_column:0,last_column:1,row_slot:0,column_slot:0,x:0,y:0,width:160,height:180}];}
                        let mut h=host(f.encode().unwrap());h.set_theme(theme);let output=h.render(app,window,constraints()).unwrap();
                        let command=output.commands.iter().find(|c|match c{DrawCommand::GridText{text,..}|DrawCommand::StyledText{text,..}|DrawCommand::Text{text,..}=>text=="Budget",_=>false}).unwrap();
                        let expected=match paint.as_ref().and_then(|p|p.vertical.as_deref()){Some("top")=>0,Some("center")=>1,Some("bottom")=>2,_=>if document{2}else{1}};
                        let rect=if document||paint.is_some(){
                            let DrawCommand::GridText{vertical,rect,wrap,font:actual_font,color,..}=command else {panic!("Document cell needs effective alignment")};
                            assert_eq!(*vertical,expected);assert!(!wrap);assert_eq!(*actual_font,font);
                            if paint.as_ref().and_then(|p|p.foreground.as_deref()).is_some(){assert_eq!(*color,Color::opaque(0x12,0x34,0x56));}
                            *rect
                        }else if font.is_some(){let DrawCommand::StyledText{rect,..}=command else{panic!("Legacy document font path changed")};*rect}
                        else{let DrawCommand::Text{rect,..}=command else{panic!("Generic themed grid path changed")};*rect};
                        assert_eq!(rect.size.height,if merged{178}else{58});
                        assert_eq!(rect.size.width,if merged{152}else{72});
                        assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::Text{text,color,..}if text=="A"&&*color==theme.palette.text_muted)),"header remains themed/centered");
                    }
                }
            }
        }
    }
}

#[test]
fn document_alignment_survives_selection_only_damage_and_legacy_restoration() {
    use rice_ui::grid_damage::{GridDamage,geometry_key};
    let(app,window,_)=ids();
    let mut f=frame();f.canvas=Some("document".into());f.styles[1].paint=None;
    let damage=GridDamage{version:1,sheet:f.sheet,base_revision:f.revision,revision:f.revision+1,base_geometry:geometry_key(&f),cells:vec![],selection:None,active:None,references:None};
    let updated=damage.apply(&f).unwrap();assert_eq!(updated.styles,f.styles);assert_eq!(updated.cells,f.cells);
    let mut h=host(updated.encode().unwrap());assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::GridText{text,vertical:2,..}if text=="125.50")));
    f.canvas=None;f.selection=None;f.active=None;
    let mut h=host(f.encode().unwrap());assert!(h.render(app,window,constraints()).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::Text{text,..}if text=="125.50")));
}
