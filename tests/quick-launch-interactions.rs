use super::*;

#[test]
fn menu_drop_pins_without_moving_or_launching_the_application() {
    for edge in [
        WmTaskbarEdge::Bottom,
        WmTaskbarEdge::Top,
        WmTaskbarEdge::Left,
        WmTaskbarEdge::Right,
    ] {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.wm_taskbar_edge = edge;
        assert!(server.replace_wm_menu_entries(vec![(41, 1, 1, b"Editor".to_vec())]));
        server.wm_menu_structured = true;
        let entry = server.wm_menu_entries[0].clone();
        server.wm_menu_drag = Some(WmMenuDrag {
            entry,
            source_index: 0,
            pointer_x: 400,
            pointer_y: 300,
        });
        let (x, y, _, _) = server.wm_taskbar_rect();
        let (drop_x, drop_y) = if edge.vertical() {
            (x + 10, y + server.wm_vertical_quick_launch_y() as i16 + 30)
        } else {
            (server.wm_quick_launch_x() + 30, y + 10)
        };
        server.finish_wm_menu_drag(drop_x, drop_y);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_QUICK_PIN_PREFIX | 1)
        );
        assert_eq!(server.drain_wm_action(), None);
        assert_eq!(server.wm_menu_entries[0].action, 41);
        assert_eq!(server.wm_menu_entries[0].category, 1);
        assert!(!server.windows[&WM_MENU.0].mapped);
        assert_eq!(server.wm_quick_drop_position(400, 300), None);
    }
}

#[test]
fn shortcut_context_removes_only_the_clicked_shortcut() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 800,
        height_px: 600,
        native_window_manager: true,
        ..DisplayConfig::default()
    });
    let index = server
        .wm_quick_launchers
        .iter()
        .position(|entry| entry.action == WM_ACTION_TERMINAL)
        .unwrap();
    let x = server.wm_quick_launch_x() + index as i16 * 28 + 4;
    server.inject_pointer_button(true, 3, x, 584, 0);
    assert_eq!(server.wm_quick_context_index, Some(index as u8));
    let rows = &server.wm_menu_columns()[0];
    let remove = rows
        .iter()
        .position(|row| *row == WmMenuRow::QuickShortcutRemove)
        .unwrap();
    let y = server.windows[&WM_MENU.0].y
        + (wm_menu_row_top(rows, remove) + wm_menu_row_height(&rows[remove]) / 2) as i16;
    server.inject_pointer_button(true, 1, server.windows[&WM_MENU.0].x + 20, y, 0);
    assert_eq!(
        server.drain_wm_action(),
        Some(WM_ACTION_QUICK_REMOVE_PREFIX | index as u32)
    );
    assert_eq!(server.drain_wm_action(), None);
}

#[test]
fn structured_menu_pointer_drag_can_pin_and_escape_cancels() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 800,
        height_px: 600,
        native_window_manager: true,
        ..DisplayConfig::default()
    });
    assert!(server.replace_wm_structured_menu_entries(vec![(
        41,
        1,
        false,
        false,
        false,
        Vec::new(),
        b"Editor".to_vec()
    )]));
    server.wm_menu_level = WmMenuLevel::MenuFolders;
    server.relayout_wm_menu();
    server.windows.get_mut(&WM_MENU.0).unwrap().mapped = true;
    let menu = server.windows[&WM_MENU.0].clone();
    let x = menu.x + server.wm_menu_column_widths()[0] as i16 + 12;
    let y = menu.y + (server.wm_menu_column_offsets()[1] + WM_MENU_ROW_HEIGHT / 2) as i16;
    assert!(server.handle_native_pointer_button(true, 1, WM_MENU, x, y));
    assert!(server.wm_menu_drag.is_some());
    assert_eq!(server.drain_wm_action(), None);
    server.inject_pointer_motion(server.wm_quick_launch_x() + 30, 584, 0x0100);
    server.inject_pointer_button(false, 1, server.wm_quick_launch_x() + 30, 584, 0x0100);
    assert_eq!(
        server.drain_wm_action(),
        Some(WM_ACTION_QUICK_PIN_PREFIX | 1)
    );
    assert!(server.handle_native_pointer_button(true, 1, WM_MENU, x, y));
    server.inject_key(true, 9, 0);
    assert!(server.wm_menu_drag.is_none());
    server.inject_pointer_button(false, 1, server.wm_quick_launch_x() + 30, 584, 0);
    assert_eq!(server.drain_wm_action(), None);
}

#[test]
fn compact_dock_groups_by_app_identity_and_shift_launches_another_instance() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 1024,
        height_px: 768,
        native_window_manager: true,
        ..DisplayConfig::default()
    });
    server.set_dock_style(true);
    let index = server
        .wm_quick_launchers
        .iter()
        .position(|launcher| launcher.action == WM_ACTION_TERMINAL)
        .unwrap();
    server.set_dock_identity(1, WM_ACTION_TERMINAL, 1234);
    for id in 1..=2 {
        server.set_dock_identity(0, id, 1234);
    }
    server.replace_native_tasks(
        (1..=2)
            .map(|id| WmNativeTask {
                id,
                workspace: 0,
                icon: 2,
                minimized: false,
                maximized: false,
                active: id == 2,
                title: format!("Different document {id}").into_bytes(),
            })
            .collect(),
    );
    assert!(matches!(
        server.wm_dock_launcher_task(index as u8),
        Some(WmTaskTarget::Group(_))
    ));
    let targets = server.wm_dock_targets();
    assert_eq!(targets[0], WmChromeTarget::Start);
    assert_eq!(targets[1], WmChromeTarget::Activities);
    assert!(
        !targets
            .iter()
            .any(|target| matches!(target, WmChromeTarget::Task(_))),
        "running pinned app must occupy one slot"
    );
    let slot = targets
        .iter()
        .position(|target| *target == WmChromeTarget::QuickLaunch(index as u8))
        .unwrap();
    let (x, y, _, _) = server.wm_taskbar_rect();
    let x = x + 8 + (slot * server.wm_dock_step() + server.wm_dock_step() / 2) as i16;
    server.inject_pointer_button(true, 1, x, y + 28, 1);
    server.inject_pointer_button(false, 1, x, y + 28, 1);
    assert_eq!(server.drain_wm_action(), Some(0xffe8_0001));
    server.inject_pointer_button(true, 1, x, y + 28, 0);
    server.inject_pointer_button(false, 1, x, y + 28, 0);
    assert!(matches!(server.wm_menu_level, WmMenuLevel::TaskGroup(_)));
    assert_eq!(server.wm_menu_base_row_height(), 144);
    assert!(server.windows[&WM_MENU.0].mapped);
}

#[test]
fn compact_start_menu_keeps_its_anchor_when_cascades_open() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 1024,
        height_px: 768,
        native_window_manager: true,
        ..DisplayConfig::default()
    });
    server.set_dock_style(true);
    server.wm_menu_level = WmMenuLevel::Root;
    server.relayout_wm_menu();
    let anchor = server.windows[&WM_MENU.0].x;
    server.wm_menu_level = WmMenuLevel::Categories;
    server.relayout_wm_menu();
    assert_eq!(
        server.windows[&WM_MENU.0].x + server.wm_menu_column_x_offsets()[0] as i16,
        anchor
    );
    server.wm_menu_level = WmMenuLevel::Applications(0);
    server.relayout_wm_menu();
    assert_eq!(
        server.windows[&WM_MENU.0].x + server.wm_menu_column_x_offsets()[0] as i16,
        anchor
    );
}

#[test]
fn compact_hover_preview_activates_and_closes_only_its_window() {
    for compact in [false, true] {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 768,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.set_dock_style(compact);
        server.replace_native_tasks(vec![WmNativeTask {
            id: 7,
            workspace: 0,
            icon: 116,
            minimized: false,
            maximized: false,
            active: true,
            title: b"Calc".to_vec(),
        }]);
        server.set_wm_task_preview(Some(WmTaskTarget::Native(7)), 512);
        let preview = server.windows[&WM_TASK_PREVIEW.0].clone();
        while server.drain_wm_action().is_some() {}
        server.inject_pointer_button(true, 1, preview.x + 80, preview.y + 80, 0);
        assert_eq!(server.drain_wm_action(), compact.then_some(0xffe6_0007));
        if compact {
            server.set_wm_task_preview(Some(WmTaskTarget::Native(7)), 512);
            let preview = server.windows[&WM_TASK_PREVIEW.0].clone();
            server.inject_pointer_button(
                true,
                1,
                preview.x + preview.width as i16 - 32,
                preview.y + 22,
                0,
            );
            assert_eq!(
                server.drain_wm_action(),
                Some(WM_ACTION_TASK_CLOSE_PREFIX | 7)
            );
        }
    }
}

#[test]
fn compact_group_hover_shows_all_windows_and_leaving_clears_previews() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 1024,
        height_px: 768,
        native_window_manager: true,
        ..DisplayConfig::default()
    });
    server.set_dock_style(true);
    let index = server
        .wm_quick_launchers
        .iter()
        .position(|launcher| launcher.action == WM_ACTION_TERMINAL)
        .unwrap();
    server.set_dock_identity(1, WM_ACTION_TERMINAL, 1234);
    for id in 1..=2 {
        server.set_dock_identity(0, id, 1234);
    }
    server.replace_native_tasks(
        (1..=2)
            .map(|id| WmNativeTask {
                id,
                workspace: 0,
                icon: 2,
                minimized: false,
                maximized: false,
                active: id == 2,
                title: format!("Document {id}").into_bytes(),
            })
            .collect(),
    );
    let slot = server
        .wm_dock_targets()
        .iter()
        .position(|target| *target == WmChromeTarget::QuickLaunch(index as u8))
        .unwrap();
    let (x, y, _, _) = server.wm_taskbar_rect();
    let x = x + 8 + (slot * server.wm_dock_step() + server.wm_dock_step() / 2) as i16;
    server.inject_pointer_motion(x, y + 28, 0);
    assert!(server.windows[&WM_MENU.0].mapped);
    assert!(matches!(server.wm_menu_level, WmMenuLevel::TaskGroup(_)));
    assert_eq!(
        server.wm_menu_columns()[0]
            .iter()
            .filter(|row| matches!(row, WmMenuRow::Task(_)))
            .count(),
        2
    );
    server.inject_pointer_button(true, 3, x, y + 28, 0);
    server.inject_pointer_button(false, 3, x, y + 28, 0);
    server.inject_pointer_motion(x + 1, y + 28, 0);
    assert!(matches!(
        server.wm_menu_level,
        WmMenuLevel::TaskContext(WmTaskTarget::Group(_))
    ));
    assert_eq!(
        server.wm_menu_columns()[0],
        vec![
            WmMenuRow::StackTile,
            WmMenuRow::StackCascade,
            WmMenuRow::StackClose,
            WmMenuRow::Divider,
            WmMenuRow::DockSettings,
        ]
    );
    assert!(
        !server.windows[&WM_TASK_PREVIEW.0].mapped,
        "a hover preview must not cover the group context menu"
    );
    server.dismiss_wm_menu();
    server.inject_pointer_motion(x, y + 28, 0);

    server.inject_pointer_motion(10, 10, 0);
    assert!(!server.windows[&WM_MENU.0].mapped);
    server.inject_pointer_motion(x, y + 28, 0);
    server.clear_dock_hover();
    assert!(!server.windows[&WM_MENU.0].mapped);
    server.set_wm_task_preview(Some(WmTaskTarget::Native(1)), 512);
    server.dismiss_wm_menu();
    assert!(server.wm_task_preview_target.is_none());
}
