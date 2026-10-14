use super::*;

#[test]
fn icon_drag_saves_workspace_position_through_settings_provider() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("notes.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "notes.txt")
        .expect("notes item")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 8,
                    y: 8,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("drag begins");
    let messages = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: 210,
                    y: 8,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("drag moves");
    assert!(matches!(&messages[0], Message::UiPatch(_)));
    let Message::SettingsRequest(SettingsRequest {
        operation: SettingsOperation::Set { key, value },
        ..
    }) = &messages[1]
    else {
        panic!("drag persists through settings");
    };
    assert_eq!(key, desktop_persistence::POSITIONS_SETTING);
    assert!(value.contains("\t2\t0"));
}

#[test]
fn persisted_cells_restore_by_path_even_when_runtime_item_id_changes() {
    let mut desktop = launch();
    let encoded = [
        "776f726b",
        "2f726f6f742f4465736b746f702f6e6f7465732e747874",
        "2",
        "0",
    ]
    .join("\t");
    desktop
        .handle_settings_response(&SettingsResponse {
            request: request(1),
            result: SettingsResult::Values(vec![SettingEntry {
                key: desktop_persistence::POSITIONS_SETTING.to_owned(),
                value: encoded,
            }]),
        })
        .expect("positions load");
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("notes.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "notes.txt")
        .expect("notes")
        .id;
    assert_eq!(desktop.model().grid().cell_of(item), Some(Cell::new(2, 0)));
}

#[test]
fn persisted_auto_arrange_is_loaded_and_blocks_free_dragging() {
    let mut desktop = launch();
    desktop
        .handle_settings_response(&SettingsResponse {
            request: request(1),
            result: SettingsResult::Values(vec![SettingEntry {
                key: String::from("desktop.icons.auto_arrange"),
                value: String::from("true"),
            }]),
        })
        .expect("auto-arrange setting loads");
    assert!(desktop.auto_arrange);
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("fixed.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "fixed.txt")
        .expect("fixed item")
        .id;
    let before = desktop.model().grid().cell_of(item).expect("initial cell");
    let rect = desktop.model().grid().config().rect(before);
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: rect.x + 10,
                    y: rect.y + 10,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("selection starts");
    let moved = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: rect.x + 310,
                    y: rect.y + 10,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("motion while arranged");
    assert!(moved.is_empty());
    assert_eq!(desktop.model().grid().cell_of(item), Some(before));
}

#[test]
fn desktop_window_geometry_expands_the_free_positioning_canvas() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("sized.txt", FileKind::File)],
    );
    let messages = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::WindowState {
                    window: window_id(WINDOW),
                    content_width: 1_280,
                    content_height: 720,
                    fullscreen: false,
                    maximized: true,
                    focused: true,
                    visible: true,
                },
            },
            request(2),
        )
        .expect("desktop resize");
    assert_eq!(desktop.model().grid().config().columns, 12);
    assert_eq!(desktop.model().grid().config().rows, 7);
    assert_eq!(messages.len(), 2);
    let patch = envelope(messages[0].clone());
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create {
            id,
            text: Some(text),
            ..
        } if *id == widget_id(ICON_GRID) && text == "desktop-grid:12:7:96:96:4:4:8:8"
    )));
}

#[test]
fn filesystem_wake_coalesces_behind_active_enumeration() {
    let mut desktop = launch();
    assert!(
        desktop
            .handle_host_event(
                &HostEvent {
                    app: app(7),
                    event: HostEventKind::Timer {
                        token: WATCH_EVENT_TOKEN,
                    },
                },
                request(2),
            )
            .expect("watch event")
            .is_empty()
    );
    let messages = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![directory_entry("new.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("initial enumeration");
    assert!(matches!(messages[0], Message::UiPatch(_)));
    assert!(matches!(
        messages[1],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { .. },
            ..
        })
    ));
}
