use super::*;

fn app() -> AppId { AppId::new(97).unwrap() }
fn window() -> WindowId { WindowId::new(1).unwrap() }
fn widget(id: u64) -> WidgetId { WidgetId::new(id).unwrap() }
fn fixture() -> (NativeUiHost, Constraints) {
    use rice_native_app_protocol::UiPatch;
    let mut host = NativeUiHost::default();
    let create = |id, parent: Option<u64>, index, kind: WidgetKind, text: &str| UiPatch::Create {
        id: widget(id), parent: parent.map(widget), index, kind: kind as u16,
        state: WidgetState::ENABLED.bits(), text: Some(text.into()), icon: None,
    };
    let mut patches = vec![create(1, None, 0, WidgetKind::Column, ""),
        create(2, Some(1), 0, WidgetKind::Menu, "Table"),
        create(3, Some(2), 0, WidgetKind::Column, "panel:menu-grid:columns=10:fixed:flush:gap=0")];
    for row in 0..8 {
        patches.push(create(10 + row, Some(3), row as u32, WidgetKind::Row, "panel:fixed:flush:gap=0"));
        for column in 0..10 {
            patches.push(create(100 + row * 10 + column, Some(10 + row), column as u32, WidgetKind::MenuItem,
                "panel:menu-grid-cell:fixed:accessory-height=24:width=24:flush"));
        }
    }
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1, patches }).unwrap();
    let size = Size { width: 640, height: 480 };
    (host, Constraints { min: size, max: size })
}
fn click(host: &mut NativeUiHost, constraints: Constraints, id: u64) {
    let snapshot = host.render(app(), window(), constraints).unwrap();
    let rect = snapshot.hit_targets.iter().find(|target| target.widget == widget(id)).unwrap().rect;
    let point = Point { x: rect.origin.x + 3, y: rect.origin.y + 3 };
    for kind in [PointerEventKind::Down, PointerEventKind::Up] { host.pointer_event(app(), window(), kind, point, 1, 0).unwrap(); }
    host.render(app(), window(), constraints).unwrap();
}

#[test]
fn popup_grid_keyboard_enters_nested_cells_moves_in_two_dimensions_and_commits() {
    let (mut host, constraints) = fixture(); click(&mut host, constraints, 2);
    for (key, expected) in [(116, 100), (114, 101), (116, 111), (113, 110), (111, 100), (110, 100), (115, 179), (114, 179), (116, 179)] {
        let event = host.key_event(app(), window(), true, key, 0, None).unwrap().unwrap();
        assert_eq!(event.target, widget(expected)); assert_eq!(event.event, UiEvent::Focus { focused: true });
    }
    let event = host.key_event(app(), window(), true, 36, 0, None).unwrap().unwrap();
    assert_eq!(event.target, widget(179)); assert_eq!(event.event, UiEvent::Activate);
    assert!(host.windows[&(app(), window())].opened_menu.is_none());
}

#[test]
fn popup_grid_pointer_hover_emits_preview_and_theme_outline_and_escape_never_activates() {
    let (mut host, constraints) = fixture(); click(&mut host, constraints, 2);
    let snapshot = host.render(app(), window(), constraints).unwrap();
    let rect = host.windows[&(app(), window())].geometry[&widget(112)];
    assert_eq!(rect.size, Size { width: 24, height: 24 });
    assert!(snapshot.commands.iter().any(|command| matches!(command, DrawCommand::Border { rect: border, width: 1, .. } if *border == rect)));
    let event = host.pointer_event(app(), window(), PointerEventKind::Move, Point { x: rect.origin.x + 4, y: rect.origin.y + 4 }, 0, 0).unwrap().unwrap();
    assert_eq!(event.target, widget(112)); assert!(matches!(event.event, UiEvent::Pointer { kind: PointerEventKind::Move, .. }));
    assert_eq!(host.windows[&(app(), window())].focused, Some(widget(112)));
    let event = host.key_event(app(), window(), true, 9, 0, None).unwrap().unwrap();
    assert_ne!(event.event, UiEvent::Activate); assert!(host.windows[&(app(), window())].opened_menu.is_none());
}
