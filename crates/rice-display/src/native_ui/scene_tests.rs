use super::*;
use rice_native_app_protocol::UiPatch;

fn app() -> AppId {
    AppId::new(41).unwrap()
}
fn window() -> WindowId {
    WindowId::new(1).unwrap()
}
fn widget(id: u64) -> WidgetId {
    WidgetId::new(id).unwrap()
}
fn fixture() -> NativeUiHost {
    let mut host = NativeUiHost::default();
    let create = |id, parent, kind| UiPatch::Create {
        id: widget(id),
        parent,
        index: 0,
        kind: kind as u16,
        state: WidgetState::ENABLED.bits(),
        text: None,
        icon: None,
    };
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            create(1, None, WidgetKind::Column),
            create(2, Some(widget(1)), WidgetKind::SceneSurface),
        ],
    })
    .unwrap();
    host
}
fn solid() -> Vec<u8> {
    let mut bytes = vec![rice_web_render_protocol::SOLID_RECT, 0, 7, 0, 0, 0, 0, 0];
    for n in [0_f32, 0., 200., 100.] {
        bytes.extend(n.to_le_bytes());
    }
    bytes.extend(0xff00_00ff_u32.to_le_bytes());
    bytes
}
fn frame(epoch: u64) -> WebRenderPatch {
    WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(2),
        epoch,
        viewport_width: 200,
        viewport_height: 100,
        display_list: solid(),
    }
}
fn render(host: &mut NativeUiHost) -> RenderOutput {
    let size = Size {
        width: 600,
        height: 400,
    };
    host.render(app(), window(), Constraints::new(size, size).unwrap())
        .unwrap()
}

#[test]
fn overlay_surfaces_receive_retained_payloads_after_all_paint_layers() {
    // A menu in a dialog follows a different paint branch from a root menu.
    // Exercise both, plus a direct dialog surface, against the real renderer.
    for (dialog, menu) in [(false, true), (true, false), (true, true)] {
        let mut host = NativeUiHost::default();
        let mut patches = Vec::new();
        let mut create = |id: u64, parent: Option<u64>, kind: WidgetKind| patches.push(UiPatch::Create {
            id: widget(id), parent: parent.map(widget), index: 0,
            kind: kind as u16, state: WidgetState::ENABLED.bits(),
            text: None, icon: None,
        });
        create(1, None, WidgetKind::Column);
        let mut parent = 1;
        if dialog {
            create(3, Some(parent), WidgetKind::Dialog);
            parent = 3;
        }
        if menu {
            create(4, Some(parent), WidgetKind::Menu);
            parent = 4;
        }
        create(2, Some(parent), WidgetKind::SceneSurface);
        host.apply_patch(&UiPatchEnvelope {
            app: app(), window: window(), sequence: 1, patches,
        }).unwrap();
        if menu {
            host.windows.get_mut(&(app(), window())).unwrap().opened_menu = Some(widget(4));
        }
        for epoch in [1, 2] {
            host.apply_web_render(&frame(epoch)).unwrap();
            let output = render(&mut host);
            let (index, command) = output.commands.iter().enumerate().find(|(_, command)| {
                matches!(command, DrawCommand::WebScene { surface, .. } if *surface == widget(2))
            }).expect("overlay surface is painted");
            if menu {
                assert!(index >= output.popup.as_ref().unwrap().command_start);
            }
            match command {
                DrawCommand::WebScene { epoch: actual, viewport_width, viewport_height, display_list, .. } => {
                    assert_eq!(*actual, epoch, "dialog={dialog}, menu={menu}");
                    assert_eq!((*viewport_width, *viewport_height), (200, 100));
                    assert_eq!(display_list.as_ref(), solid().as_slice());
                }
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn scene_surface_retains_owned_vectors_and_marks_snapshot_scale() {
    assert_eq!(WidgetKind::from_wire(65), Some(WidgetKind::SceneSurface));
    assert_eq!(WidgetKind::from_wire(63), Some(WidgetKind::TerminalGrid));
    let mut host = fixture();
    host.apply_web_render(&frame(1)).unwrap();
    let before = render(&mut host);
    let data = before
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::WebScene {
                vector_scene: true,
                display_list,
                epoch: 1,
                ..
            } => Some(display_list.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(&*data, solid());
    let bytes =
        crate::native_ui_snapshot::encode_snapshot(app().get(), window().get(), &before).unwrap();
    let mut cursor = usize::from(crate::native_ui_snapshot::SNAPSHOT_HEADER_BYTES);
    while bytes[cursor] != 7 {
        cursor += usize::from(u16::from_le_bytes(
            bytes[cursor + 2..cursor + 4].try_into().unwrap(),
        ));
    }
    assert_eq!(
        bytes[cursor + 1],
        1,
        "scene header uses vector flag, keeping tag and record size"
    );
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![],
    })
    .unwrap();
    let after = render(&mut host);
    assert!(after.commands.iter().any(|command| matches!(command,
        DrawCommand::WebScene { display_list, .. } if Arc::ptr_eq(display_list, &data))));
}

#[test]
fn scene_ingress_rejects_html_stale_invalid_dimensions_and_wrong_owner_without_mutation() {
    let mut host = fixture();
    host.apply_web_render(&frame(2)).unwrap();
    let mut invalid = frame(3);
    invalid.display_list.clear();
    assert!(host.apply_web_render(&invalid).is_err());
    let mut invalid = frame(3);
    invalid.display_list = rice_web_render_protocol::html_document("<h1>denied</h1>").unwrap();
    assert!(host.apply_web_render(&invalid).is_err());
    for (width, height, epoch) in [
        (0, 100, 3),
        (200, 0, 3),
        (16_385, 100, 3),
        (200, 16_385, 3),
        (200, 100, 0),
    ] {
        let mut invalid = frame(epoch);
        invalid.viewport_width = width;
        invalid.viewport_height = height;
        assert!(host.apply_web_render(&invalid).is_err());
    }
    assert!(matches!(
        host.apply_web_render(&frame(2)),
        Err(NativeUiError::StaleSequence { .. })
    ));
    let mut invalid = frame(3);
    invalid.app = AppId::new(42).unwrap();
    assert!(host.apply_web_render(&invalid).is_err());
    invalid = frame(3);
    invalid.window = WindowId::new(2).unwrap();
    assert!(host.apply_web_render(&invalid).is_err());
    invalid = frame(3);
    invalid.surface = widget(1);
    assert!(host.apply_web_render(&invalid).is_err());
    invalid = frame(3);
    invalid.display_list = vec![0; rice_web_render_protocol::MAX_SCENE_BYTES + 4];
    assert!(host.apply_web_render(&invalid).is_err());
    assert!(
        render(&mut host)
            .commands
            .iter()
            .any(|command| matches!(command,
        DrawCommand::WebScene { epoch: 2, display_list, .. } if display_list.as_ref() == solid()))
    );
}

#[test]
fn scene_pointer_coordinates_use_centered_contain_and_letterbox_does_not_hit() {
    let mut host = fixture();
    let mut patch = frame(1);
    let mut hit = vec![rice_web_render_protocol::HIT_REGION, 0, 9, 0, 0, 0, 0, 0];
    for n in [80_f32, 40., 40., 20.] {
        hit.extend(n.to_le_bytes());
    }
    hit.extend(1_u64.to_le_bytes());
    hit.extend([1, 0, 0, 0]);
    patch.display_list.extend(hit);
    host.apply_web_render(&patch).unwrap();
    let output = render(&mut host);
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .unwrap();
    let point = Point {
        x: target.rect.origin.x + i32::try_from(target.rect.size.width / 2).unwrap(),
        y: target.rect.origin.y + i32::try_from(target.rect.size.height / 2).unwrap(),
    };
    let event = host
        .pointer_event(app(), window(), PointerEventKind::Move, point, 0, 0)
        .unwrap()
        .unwrap();
    assert!(matches!(
        event.event,
        UiEvent::Pointer {
            x: 99..=100,
            y: 49..=50,
            ..
        }
    ));
    assert_eq!(render(&mut host).cursor, PointerCursor::Pointer);
    let outside = Point {
        x: point.x,
        y: target.rect.origin.y + 1,
    };
    let event = host
        .pointer_event(app(), window(), PointerEventKind::Move, outside, 0, 0)
        .unwrap()
        .unwrap();
    assert!(matches!(event.event, UiEvent::Pointer { y, .. } if y < 0));
    assert_eq!(render(&mut host).cursor, PointerCursor::Default);
    let event = host
        .pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .unwrap()
        .unwrap();
    assert!(matches!(
        event.event,
        UiEvent::Pointer {
            x: 99..=100,
            y: 49..=50,
            ..
        }
    ));
}

fn image_frame_bytes(width: u32, height: u32) -> Vec<u8> {
    let pixels = usize::try_from(width * height * 4).unwrap();
    let mut bytes = vec![0; 24 + pixels];
    bytes[0] = rice_web_render_protocol::DEFINE_IMAGE;
    let length=u32::try_from(bytes.len()).unwrap();
    bytes[4..8].copy_from_slice(&length.to_le_bytes());
    for (offset, value) in [(8, 1_u32), (12, width), (16, height), (20, u32::try_from(pixels).unwrap())] {
        bytes[offset..offset+4].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}
#[test]
fn full_hd_scene_and_authoritative_aggregate_budget_reject_transactionally() {
    let mut host = fixture();
    let mut patch = frame(1);
    patch.display_list = image_frame_bytes(1920, 1080);
    host.apply_web_render(&patch).unwrap();
    // A second full-HD surface is allowed; a third exceeds the 16MiB app budget.
    host.apply_patch(&UiPatchEnvelope {app:app(),window:window(),sequence:2,patches:vec![
        UiPatch::Create{id:widget(3),parent:Some(widget(1)),index:1,kind:WidgetKind::SceneSurface as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None},
        UiPatch::Create{id:widget(4),parent:Some(widget(1)),index:2,kind:WidgetKind::SceneSurface as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None},
    ]}).unwrap();
    patch.surface=widget(3);host.apply_web_render(&patch).unwrap();
    patch.surface=widget(4);assert!(host.apply_web_render(&patch).is_err());
    assert!(!host.windows.get(&(app(),window())).unwrap().web_scenes.contains_key(&widget(4)));
    let previous=host.windows.get(&(app(),window())).unwrap().web_scenes[&widget(2)].display_list.clone();
    patch.surface=widget(2);patch.epoch=2;patch.display_list=image_frame_bytes(2048,1024);
    assert!(host.apply_web_render(&patch).is_err());
    let retained=&host.windows.get(&(app(),window())).unwrap().web_scenes[&widget(2)];
    assert_eq!(retained.epoch,1);assert_eq!(retained.display_list,previous);
    // Replacing an existing frame subtracts its old allocation before charging.
    patch.display_list=image_frame_bytes(1920,1080);host.apply_web_render(&patch).unwrap();
}

#[test]
fn native_global_scene_byte_budget_is_shared_across_applications() {
    let mut host = NativeUiHost::default();
    let bytes = image_frame_bytes(1920,1080);
    for owner in 1..=5 {
        let app=AppId::new(owner).unwrap();
        host.apply_patch(&UiPatchEnvelope{app,window:window(),sequence:1,patches:vec![
            UiPatch::Create{id:widget(1),parent:None,index:0,kind:WidgetKind::SceneSurface as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None},
            UiPatch::Create{id:widget(2),parent:None,index:1,kind:WidgetKind::SceneSurface as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None},
        ]}).unwrap();
        for id in 1..=2 {
            let patch=WebRenderPatch{app,window:window(),surface:widget(id),epoch:1,viewport_width:1920,viewport_height:1080,display_list:bytes.clone()};
            if owner==5 {
                assert!(host.apply_web_render(&patch).is_err());
                assert!(host.windows.get(&(app,window())).unwrap().web_scenes.is_empty());
            }else{host.apply_web_render(&patch).unwrap();}
        }
    }
}
