use super::*;


    #[test]
    fn fourth_native_instance_becomes_an_exact_task_group_menu() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let tasks = (1..=4)
            .map(|id| WmNativeTask {
                id,
                workspace: 0,
                icon: 1,
                minimized: false,
                maximized: false,
                active: id == 1,
                title: format!("RiceFiles ({id})").into_bytes(),
            })
            .collect();
        assert!(server.replace_native_tasks(tasks));
        assert_eq!(server.wm_task_targets(), vec![WmTaskTarget::Group(0)]);

        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        server.inject_pointer_motion(WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert!(server.windows[&WM_TASK_PREVIEW.0].mapped);
        assert_eq!(server.wm_task_preview_target, Some(WmTaskTarget::Group(0)));

        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert!(server.windows[&WM_MENU.0].mapped);
        assert_eq!(server.wm_menu_level, WmMenuLevel::TaskGroup(0));
        assert_eq!(
            server.wm_menu_columns()[0],
            vec![
                WmMenuRow::Task(WmTaskTarget::Native(1)),
                WmMenuRow::Task(WmTaskTarget::Native(2)),
                WmMenuRow::Task(WmTaskTarget::Native(3)),
                WmMenuRow::Task(WmTaskTarget::Native(4)),
            ],
        );
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_motion(menu.x + 20, menu.y + 2 * 36 + 12, 0);
        let preview = server.windows[&WM_TASK_PREVIEW.0].clone();
        assert!(preview.mapped);
        assert_eq!(server.wm_task_preview_target, Some(WmTaskTarget::Native(3)),);
        assert_eq!(
            server.wm_task_preview_placement,
            WmTaskPreviewPlacement::MenuRight,
        );
        assert!(preview.x >= menu.x.saturating_add_unsigned(menu.width));
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 2 * 36 + 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_NATIVE_TASK_PREFIX | 3),
        );

        server.show_wm_task_menu(WmMenuLevel::TaskGroup(0), 780, 584);
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_motion(menu.x + 20, menu.y + 36 + 12, 0);
        let preview = server.windows[&WM_TASK_PREVIEW.0].clone();
        assert_eq!(server.wm_task_preview_target, Some(WmTaskTarget::Native(2)),);
        assert_eq!(
            server.wm_task_preview_placement,
            WmTaskPreviewPlacement::MenuLeft,
        );
        assert!(preview.x.saturating_add_unsigned(preview.width) <= menu.x);

        assert!(server.set_task_previews_enabled(false));
        assert!(!server.windows[&WM_TASK_PREVIEW.0].mapped);

        server.inject_pointer_button(true, 3, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::TaskContext(WmTaskTarget::Group(0)),
        );
        assert_eq!(
            server.wm_menu_columns()[0],
            vec![
                WmMenuRow::StackTile,
                WmMenuRow::StackCascade,
                WmMenuRow::StackClose,
            ],
        );
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_STACK_TILE_PREFIX | 1),
        );
    }

    #[test]
    fn x11_toolkit_instances_group_by_wm_class_and_keep_exact_identity() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let wm_class = server.atoms.intern("WM_CLASS", false);
        let string = server.atoms.intern("STRING", false);
        let windows = (0..4)
            .map(|index| ResourceId(0x0020_00b0 + index))
            .collect::<Vec<_>>();
        for window in &windows {
            server
                .feed(client, &create_window_request(order, window.0, 0))
                .unwrap();
            server
                .windows
                .get_mut(&window.0)
                .unwrap()
                .properties
                .insert(
                    wm_class,
                    Property {
                        r#type: string,
                        format: 8,
                        data: b"qt-instance\0Org.Rice.Terminal\0".to_vec(),
                    },
                );
            server
                .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
                .unwrap();
        }
        assert_eq!(server.wm_task_targets(), vec![WmTaskTarget::Group(0)]);

        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 2 * 36 + 12, 0);
        assert_eq!(server.focus_window, windows[2]);

        server.inject_pointer_button(true, 3, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::TaskContext(WmTaskTarget::Group(0)),
        );
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 12, 0);
        let positions = windows
            .iter()
            .map(|window| {
                let frame = server.managed_frames[&window.0];
                (server.windows[&frame.0].x, server.windows[&frame.0].y)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            positions.len(),
            4,
            "tile arrangement must move every X11 instance"
        );
    }

    #[test]
    fn core_font_has_distinct_lowercase_and_complete_printable_ascii() {
        let fallback = core_font_glyph(u16::from(b'?'));
        for byte in 0x20u8..=0x7e {
            if byte != b'?' {
                assert_ne!(
                    core_font_glyph(u16::from(byte)),
                    fallback,
                    "printable ASCII byte 0x{byte:02x} used the fallback glyph",
                );
            }
        }
        for lowercase in b'a'..=b'z' {
            assert_ne!(
                core_font_glyph(u16::from(lowercase)),
                core_font_glyph(u16::from(lowercase.to_ascii_uppercase())),
                "lowercase {} was substituted with uppercase",
                char::from(lowercase),
            );
        }
    }
