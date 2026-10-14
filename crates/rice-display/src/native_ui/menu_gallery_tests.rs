use super::*;

fn fixture() -> (NativeUiHost, Constraints) {
    let mut host = NativeUiHost::default();
    let enabled = WidgetState::ENABLED;
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, enabled, None),
            create(2, Some(1), 0, WidgetKind::ToolBar, enabled, None),
            create(3, Some(2), 0, WidgetKind::Menu, enabled, Some("Shapes")),
            create(4, Some(3), 0, WidgetKind::Column, enabled, None),
            create(5, Some(4), 0, WidgetKind::Row, enabled, None),
            create(6, Some(5), 0, WidgetKind::Button, enabled, Some("More colors")),
            create(7, Some(5), 1, WidgetKind::MenuItem, enabled, Some("color-swatch:#ee1133")),
            create(8, Some(4), 1, WidgetKind::MenuItem, enabled, None),
            create(9, Some(8), 0, WidgetKind::Container, WidgetState::empty(), Some("panel:fixed-height:64:width=64")),
            create(10, Some(9), 0, WidgetKind::SceneSurface, WidgetState::empty(), None),
            create(11, Some(8), 1, WidgetKind::Tooltip, WidgetState::empty(), Some("Long shape label should not determine the cell width")),
        ] }).expect("gallery tree");
    let size = Size { width: 640, height: 480 };
    (host, Constraints { min: size, max: size })
}
fn click(host: &mut NativeUiHost, constraints: Constraints, id: u64) -> Option<UiEventEnvelope> {
    let snapshot = host.render(app(), window(), constraints).expect("snapshot");
    let rect = snapshot.hit_targets.iter().find(|item| item.widget == widget(id)).expect("visible action").rect;
    let point = Point { x: rect.origin.x + 2, y: rect.origin.y + 2 };
    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0).expect("down");
    host.pointer_event(app(), window(), PointerEventKind::Up, point, 1, 0).expect("up")
}
fn visible(host: &mut NativeUiHost, constraints: Constraints, id: u64) -> bool {
    host.render(app(), window(), constraints).expect("snapshot").hit_targets.iter().any(|item| item.widget == widget(id))
}
#[test]
fn nested_popup_draft_controls_remain_open_and_swatch_commits_close() {
    let (mut host, constraints) = fixture();
    click(&mut host, constraints, 3);
    let draft = click(&mut host, constraints, 6).expect("draft activation");
    assert_eq!(draft.target, widget(6));
    assert!(visible(&mut host, constraints, 7));
    let snapshot = host.render(app(), window(), constraints).expect("swatch");
    assert!(snapshot.commands.iter().any(|command| matches!(command,
        DrawCommand::Fill { color, .. } if *color == Color::opaque(238, 17, 51))));
    assert!(!snapshot.commands.iter().any(|command| matches!(command,
        DrawCommand::Text { text, .. } if text.contains("color-swatch:"))));
    let committed = click(&mut host, constraints, 7).expect("swatch activation");
    assert_eq!(committed.target, widget(7));
    assert_eq!(committed.event, UiEvent::Activate);
    assert!(!visible(&mut host, constraints, 7));
}
#[test]
fn inline_menu_action_lays_out_passive_preview_and_keyboard_dismisses() {
    let (mut host, constraints) = fixture();
    click(&mut host, constraints, 3);
    host.render(app(), window(), constraints).expect("opened snapshot");
    let state = host.windows.get(&(app(), window())).expect("state");
    let preview = state.geometry.get(&widget(10)).expect("inline preview geometry");
    assert!(preview.size.width >= 40 && preview.size.height >= 40);
    assert!(preview.size.width < 200, "tooltip is not measured as cell content");
    let rect = state.geometry[&widget(8)];
    host.pointer_event(app(), window(), PointerEventKind::Down,
        Point { x: rect.origin.x + 3, y: rect.origin.y + 3 }, 1, 0).expect("focus action");
    assert!(visible(&mut host, constraints, 8));
    let activated = host.key_event(app(), window(), true, 36, 0, None).expect("Enter").expect("activation");
    assert_eq!(activated.target, widget(8));
    assert_eq!(activated.event, UiEvent::Activate);
    assert!(!visible(&mut host, constraints, 8));
    assert_eq!(host.windows[&(app(), window())].focused, Some(widget(3)));
    click(&mut host, constraints, 3);
    host.key_event(app(), window(), true, 9, 0, None).expect("Escape");
    assert!(!visible(&mut host, constraints, 8));
}

#[test]
fn compound_gallery_popup_encloses_viewport_ignores_tooltip_and_clips_small_windows() {
    for size in [Size { width: 640, height: 480 }, Size { width: 300, height: 220 }] {
        let mut host = NativeUiHost::default();
        let enabled = WidgetState::ENABLED;
        let mut patches = vec![
            create(1, None, 0, WidgetKind::Column, enabled, None),
            create(2, Some(1), 0, WidgetKind::ToolBar, enabled, None),
            create(3, Some(2), 0, WidgetKind::Menu, enabled, Some("Shapes")),
            create(4, Some(3), 0, WidgetKind::ScrollView, enabled, Some("viewport:360|0/0")),
            create(5, Some(4), 0, WidgetKind::Column, enabled, Some("panel:fixed:gap=4")),
            create(6, Some(3), 1, WidgetKind::Tooltip, WidgetState::empty(), Some("Shapes")),
        ];
        for row in 0..8_u64 {
            let id = 100 + row * 40;
            patches.push(create(id, Some(5), row as u32, WidgetKind::Row, enabled, Some("panel:fixed:gap=4")));
            for column in 0..8_u64 {
                let item = id + column * 4 + 1;
                patches.push(create(item, Some(id), column as u32, WidgetKind::MenuItem, enabled, None));
                patches.push(create(item + 1, Some(item), 0, WidgetKind::Container, WidgetState::empty(), Some("panel:fixed:accessory-height=40:width=40:flush")));
                patches.push(create(item + 2, Some(item + 1), 0, WidgetKind::SceneSurface, WidgetState::empty(), None));
            }
        }
        host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1, patches }).unwrap();
        let constraints = Constraints { min: size, max: size };
        click(&mut host, constraints, 3);
        let snapshot = host.render(app(), window(), constraints).unwrap();
        assert!(!snapshot.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text.starts_with("viewport:") || text.starts_with("panel:"))),
            "compound popup layout metadata must never be painted as a menu caption");
        let state = &host.windows[&(app(), window())];
        let popup = super::super::render::menu_popup_rect(&state.tree, widget(3), &state.geometry, &host.theme).unwrap();
        let scroll = state.geometry[&widget(4)];
        assert!(popup.origin.x >= 0 && popup.origin.y >= 0);
        assert!(popup.origin.x as u32 + popup.size.width <= size.width);
        assert!(popup.origin.y as u32 + popup.size.height <= size.height);
        assert_eq!(popup.intersection(scroll), Some(scroll), "background contains entire gallery viewport");
        assert!(!state.geometry.contains_key(&widget(6)), "tooltip never consumes popup space");
        assert!(snapshot.commands.iter().any(|command| matches!(command, DrawCommand::Fill { rect, .. } if *rect == popup)));
        for target in snapshot.hit_targets.iter().filter(|target| target.widget.get() >= 100) {
            assert_eq!(target.rect.intersection(scroll), Some(target.rect), "offscreen gallery actions cannot be clicked");
        }
        if size.width == 640 {
            let preview = state.geometry[&widget(103)];
            assert_eq!(preview.size, Size { width: 40, height: 40 }, "explicit flush preview keeps square geometry");
            assert_eq!(scroll.size.height, 360);
        }
        let event = host.pointer_event(app(), window(), PointerEventKind::Wheel,
            Point { x: scroll.origin.x + 16, y: scroll.origin.y + 16 }, 5, 0)
            .expect("gallery wheel").expect("nested scroll update");
        assert_eq!(event.target, widget(4));
        assert!(matches!(event.event, UiEvent::ValueChanged(ref value) if value.starts_with("96/")));
        let preview = host.windows[&(app(), window())].geometry[&widget(103)];
        let event = host.pointer_event(app(), window(), PointerEventKind::Wheel,
            Point { x: preview.origin.x + 4, y: preview.origin.y + 4 }, 5, 0)
            .expect("wheel over passive artwork").expect("artwork delegates wheel to gallery");
        assert_eq!(event.target, widget(4));
        assert!(matches!(event.event, UiEvent::ValueChanged(ref value) if value.starts_with("96/")));
    }
}

#[test]
fn start_aligned_toolbar_menus_keep_font_values_visible() {
    let mut host = NativeUiHost::default();
    let enabled = WidgetState::ENABLED;
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1, patches: vec![
        create(1, None, 0, WidgetKind::ToolBar, enabled, None),
        create_with_icon(2, Some(1), 0, WidgetKind::Menu, enabled | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON, Some("Liberation Sans"), IconId::ChevronDown),
        create_with_icon(3, Some(1), 1, WidgetKind::Menu, enabled | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON, Some("18 pt"), IconId::ChevronDown),
    ] }).unwrap();
    let size = Size { width: 640, height: 100 };
    let snapshot = host.render(app(), window(), Constraints { min: size, max: size }).unwrap();
    for value in ["Liberation Sans", "18 pt"] {
        assert!(snapshot.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == value)));
    }
    assert!(host.windows[&(app(), window())].geometry[&widget(2)].size.width > 100);
}

#[test]
fn popup_commands_keep_captions_while_direct_editors_keep_native_carets() {
    let mut host = NativeUiHost::default();
    let enabled = WidgetState::ENABLED;
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1, patches: vec![
        create(1, None, 0, WidgetKind::Column, enabled, None),
        create(2, Some(1), 0, WidgetKind::ToolBar, enabled, None),
        create(3, Some(2), 0, WidgetKind::Menu, enabled, Some("Options")),
        create(4, Some(3), 0, WidgetKind::MenuItem, enabled, Some("Commit choice")),
        create(5, Some(3), 1, WidgetKind::Button, enabled, Some("Draft action")),
        create(6, Some(3), 2, WidgetKind::Label, enabled, Some("Section caption")),
        create(7, Some(3), 3, WidgetKind::TextField, enabled, Some("Editable value")),
        create(8, Some(3), 4, WidgetKind::Column, enabled, Some("panel:flush:fixed")),
        create(9, Some(8), 0, WidgetKind::Label, enabled, Some("Nested caption")),
    ] }).unwrap();
    let size = Size { width: 640, height: 480 };
    let constraints = Constraints { min: size, max: size };
    click(&mut host, constraints, 3);
    click(&mut host, constraints, 7);
    let snapshot = host.render(app(), window(), constraints).unwrap();
    for expected in ["Commit choice", "Draft action", "Section caption", "Nested caption"] {
        assert!(snapshot.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text == expected)), "menu caption {expected}");
    }
    assert!(snapshot.commands.iter().any(|command| matches!(command,
        DrawCommand::Text { text, caret: true, .. } if text == "Editable value")),
        "embedded editable control uses its native text/caret rendering path");
    assert!(!snapshot.commands.iter().any(|command| matches!(command,
        DrawCommand::Text { text, .. } if text.starts_with("panel:"))));
    let event = click(&mut host, constraints, 5).unwrap();
    assert_eq!(event.target, widget(5));
    assert!(visible(&mut host, constraints, 4), "draft button retains popup");
    let event = click(&mut host, constraints, 4).unwrap();
    assert_eq!(event.target, widget(4));
    assert!(!visible(&mut host, constraints, 4), "ordinary menu command commits and dismisses");
}

#[test]
fn bounded_scroll_view_accepts_native_descendant_anchors() {
    assert_eq!(super::super::layout_tokens::scroll_anchor(Some("viewport:360|anchor:103")), Some(103));
    assert_eq!(super::super::layout_tokens::scroll_anchor(Some("anchor:103")), Some(103));
    assert_eq!(super::super::layout_tokens::scroll_anchor(Some("viewport:360|0/100")), None);
}

#[test]
fn icon_gallery_cells_are_compact_flat_and_follow_theme_ink() {
    let mut colors = Vec::new();
    let mut redmond = Theme::familiar_light();
    redmond.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    for theme in [Theme::ricewm_default(), Theme::ricewm_dark(), redmond] {
        let mut host = NativeUiHost::default();
        host.set_theme(theme);
        let enabled = WidgetState::ENABLED;
        let mut patches = vec![
            create(1, None, 0, WidgetKind::Column, enabled, None),
            create(2, Some(1), 0, WidgetKind::ToolBar, enabled, None),
            create_with_icon(3, Some(2), 0, WidgetKind::Menu, enabled, None, IconId::Shapes),
            create(4, Some(3), 0, WidgetKind::Column, enabled, Some("panel:fixed:gap=0:flush")),
        ];
        for row in 0..4_u64 {
            patches.push(create(10+row, Some(4), row as u32, WidgetKind::Row, enabled, Some("panel:fixed:gap=0:flush")));
            for column in 0..8_u64 {
                let index=row*8+column;
                patches.push(create_with_icon(100+index, Some(10+row), column as u32, WidgetKind::MenuItem, enabled,
                    Some("panel:fixed:accessory-height=36:width=36:flush"), IconId::from_wire(198+index as u16).unwrap()));
            }
        }
        host.apply_patch(&UiPatchEnvelope { app:app(),window:window(),sequence:1,patches }).unwrap();
        let size=Size { width:800,height:600 };
        let constraints=Constraints { min:size,max:size };
        click(&mut host,constraints,3);
        let snapshot=host.render(app(),window(),constraints).unwrap();
        let popup=snapshot.popup.unwrap();
        for index in 0..32_u64 {
            let rect=host.windows[&(app(),window())].geometry[&widget(100+index)];
            assert_eq!(rect.size,Size{width:36,height:36},"fixed icon cell");
            assert!(!snapshot.commands[popup.command_start..].iter().any(|command| matches!(command,
                DrawCommand::Border { rect: painted,.. }|DrawCommand::Fill { rect:painted,.. } if *painted==rect)),"no resting cell border or card");
            let icon=IconId::from_wire(198+index as u16).unwrap();
            let ink=snapshot.commands.iter().find_map(|command|match command {
                DrawCommand::Icon { icon:actual,rect:art,color,.. } if *actual==icon=>{
                    assert!(art.size.width>=20 && art.size.height>=20,"recognizable artwork"); Some(*color)
                },_=>None,
            }).expect("each canonical shape is painted");
            if index==0 { colors.push(ink); }
        }
        let rect=host.windows[&(app(),window())].geometry[&widget(100)];
        host.pointer_event(app(),window(),PointerEventKind::Move,Point{x:rect.origin.x+18,y:rect.origin.y+18},0,0).unwrap();
        let hovered=host.render(app(),window(),constraints).unwrap();
        assert!(hovered.commands.iter().any(|command|matches!(command,DrawCommand::Fill {rect:painted,..} if *painted==rect)),"hover paints only the active cell");
        assert_eq!(click(&mut host,constraints,100).unwrap().target,widget(100));
        assert!(host.render(app(),window(),constraints).unwrap().popup.is_none());
    }
    assert_ne!(colors[0],colors[1],"light and dark themes supply their own icon contrast");
}

#[test]
fn document_page_fits_intrinsic_ratio_and_keeps_pointer_coordinates_on_resize() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {app:app(),window:window(),sequence:1,patches:vec![
        create(1,None,0,WidgetKind::Container,WidgetState::ENABLED,Some("panel:document-workspace:flush")),
        create(2,Some(1),0,WidgetKind::SceneSurface,WidgetState::ENABLED,Some("scene:page:1600:900:bg:#ffffff")),
    ]}).unwrap();
    let mut display_list=vec![rice_web_render_protocol::SOLID_RECT,0,7,0,0,0,0,0];
    for value in [0_f32,0.,1600.,900.] { display_list.extend(value.to_le_bytes()); }
    display_list.extend(0xffff_ffff_u32.to_le_bytes());
    host.apply_web_render(&rice_native_app_protocol::WebRenderPatch {app:app(),window:window(),surface:widget(2),epoch:1,viewport_width:1600,viewport_height:900,display_list}).unwrap();
    for (width,height) in [(800,600),(400,800),(1200,300)] {
        let size=Size{width,height};
        let snapshot=host.render(app(),window(),Constraints{min:size,max:size}).unwrap();
        let rect=host.windows[&(app(),window())].geometry[&widget(2)];
        assert!(rect.origin.x>=20 && rect.origin.y>=20,"workspace gutters surround the page");
        assert!((i64::from(rect.size.width)*900-i64::from(rect.size.height)*1600).abs()<1600,"page preserves 16:9 ratio");
        assert!((i64::from(rect.origin.x)*2+i64::from(rect.size.width)-i64::from(width)).abs()<=1);
        assert!((i64::from(rect.origin.y)*2+i64::from(rect.size.height)-i64::from(height)).abs()<=1);
        assert!(snapshot.commands.iter().any(|command|matches!(command,DrawCommand::Fill{rect:painted,color,..} if painted.size==size && *color==Theme::ricewm_default().palette.surface)),"themed workspace behind page");
        assert!(snapshot.commands.iter().any(|command|matches!(command,DrawCommand::WebScene{rect:painted,..} if *painted==rect)),"scene paint uses the actual fitted rectangle");
        let point=Point{x:rect.origin.x+(rect.size.width/2) as i32,y:rect.origin.y+(rect.size.height/2) as i32};
        let event=host.pointer_event(app(),window(),PointerEventKind::Down,point,1,0).unwrap().unwrap();
        assert_eq!(event.target,widget(2));
        assert!(matches!(event.event,UiEvent::Pointer{x,y,..} if (x-800).abs()<=4 && (y-450).abs()<=4),"page center maps to document center");
        host.pointer_event(app(),window(),PointerEventKind::Up,point,1,0).unwrap();
    }
}
