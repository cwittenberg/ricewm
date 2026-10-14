use super::*;


    #[test]
    fn side_taskbar_start_menu_opens_beside_its_button_and_cascades_inward() {
        for edge in [WmTaskbarEdge::Left, WmTaskbarEdge::Right] {
            let mut server = DisplayServer::new(DisplayConfig {
                width_px: 800,
                height_px: 600,
                native_window_manager: true,
                ..DisplayConfig::default()
            });
            assert!(server.update_gnome_panel(true, Vec::new()));
            assert!(server.configure_gnome_panel(36, false));
            assert!(server.set_taskbar_configuration(true, edge.wire() as u8, false));

            let taskbar = server.windows[&WM_TASKBAR.0].clone();
            server.inject_pointer_button(
                true,
                1,
                taskbar.x.saturating_add_unsigned(taskbar.width / 2),
                taskbar.y.saturating_add(12),
                0,
            );
            let root = server.windows[&WM_MENU.0].clone();
            assert!(root.mapped);
            assert_eq!(root.y, taskbar.y);
            assert_eq!(server.wm_menu_column_offsets()[0], 0);

            let taskbar_gap = if server
                .wm_themes
                .get(usize::from(server.wm_theme))
                .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98)
            {
                0
            } else {
                4
            };
            if edge == WmTaskbarEdge::Left {
                assert_eq!(
                    root.x,
                    taskbar
                        .x
                        .saturating_add_unsigned(taskbar.width)
                        .saturating_add(taskbar_gap),
                );
            } else {
                assert_eq!(
                    root.x.saturating_add_unsigned(root.width),
                    taskbar.x.saturating_sub(taskbar_gap),
                );
            }

            server.inject_pointer_button(
                true,
                1,
                taskbar.x.saturating_add_unsigned(taskbar.width / 2),
                taskbar.y.saturating_add(12),
                0,
            );
            assert!(
                !server.windows[&WM_MENU.0].mapped,
                "clicking a vertical Start button twice must close its menu",
            );
            server.inject_pointer_button(
                true,
                1,
                taskbar.x.saturating_add_unsigned(taskbar.width / 2),
                taskbar.y.saturating_add(12),
                0,
            );
            assert!(server.windows[&WM_MENU.0].mapped);

            server.wm_menu_level = WmMenuLevel::Categories;
            server.relayout_wm_menu();
            let cascade = server.windows[&WM_MENU.0].clone();
            let widths = server.wm_menu_column_widths();
            let offsets = server.wm_menu_column_x_offsets();
            if edge == WmTaskbarEdge::Left {
                assert_eq!(offsets[0], 0);
                assert!(offsets[1] >= widths[0]);
            } else {
                assert!(offsets[0] > offsets[1]);
                assert_eq!(
                    cascade
                        .x
                        .saturating_add_unsigned(u16::try_from(offsets[0] + widths[0]).unwrap()),
                    taskbar.x.saturating_sub(taskbar_gap),
                );
            }
        }
    }

    #[test]
    fn menu_layout_can_open_applications_only_and_hide_optional_root_rows() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });

        assert!(server.set_menu_layout(true, true, true));
        assert_eq!(server.wm_root_rows(), vec![WmMenuRow::Applications]);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Categories);
        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 1, 12, taskbar_y, 0);
        assert!(server.windows[&WM_MENU.0].mapped);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Categories);
        let columns = server.wm_menu_columns();
        assert_eq!(columns, vec![server.wm_category_rows()]);
        assert!(
            columns
                .iter()
                .flatten()
                .all(|row| *row != WmMenuRow::Applications),
            "Applications-only mode must not render a redundant Applications parent row",
        );
        let (category_row, category) = server
            .wm_category_rows()
            .into_iter()
            .enumerate()
            .find_map(|(row, entry)| match entry {
                WmMenuRow::Category(category) => Some((row, category)),
                _ => None,
            })
            .expect("built-in application category");
        let menu = server.windows[&WM_MENU.0].clone();
        let category_x = menu.x.saturating_add_unsigned(
            u16::try_from(server.wm_menu_column_x_offsets()[0] + 12).unwrap(),
        );
        let category_y = menu.y.saturating_add(
            i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + category_row * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap(),
        );
        server.inject_pointer_motion(category_x, category_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Applications(category));
        let expanded = server.wm_menu_columns();
        assert_eq!(expanded.len(), 2);
        assert!(
            expanded
                .iter()
                .flatten()
                .all(|row| *row != WmMenuRow::Applications),
        );

        assert!(server.set_menu_layout(false, false, false));
        let rows = server.wm_root_rows();
        assert!(!rows.contains(&WmMenuRow::Themes));
        assert!(!rows.contains(&WmMenuRow::WorkspaceNavigation));
        assert!(rows.contains(&WmMenuRow::Run));
        assert!(rows.contains(&WmMenuRow::Exit));
    }

    #[test]
    fn menu_icon_size_is_bounded_and_expands_rows_without_fallback_tiles() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });

        assert!(!server.set_menu_icon_size(48));
        for size in [16, 24, 32, 64] {
            assert!(server.set_menu_icon_size(size));
            assert_eq!(server.wm_menu_icon_size, size);
        }
        assert_eq!(server.wm_menu_base_row_height(), 76);

        server.paint_wm_chrome(WM_MENU);
        let applications = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_MENU && overlay.text == b"Applications")
            .expect("root Applications overlay");
        assert_ne!(
            applications.align & WM_TEXT_ALIGN_MENU_ICON_FALLBACK_OMITTED,
            0
        );
        assert_eq!((applications.align >> 2) & 0x03, 3);

        server.wm_menu_level = WmMenuLevel::Themes;
        server.paint_wm_chrome(WM_MENU);
        let theme = server
            .wm_text_overlays
            .iter()
            .find(|overlay| {
                overlay.window == WM_MENU
                    && server
                        .wm_themes
                        .iter()
                        .any(|palette| palette.name == overlay.text)
            })
            .expect("dynamic theme overlay");
        assert_ne!(theme.align & WM_TEXT_ALIGN_MENU_ICON_FALLBACK_OMITTED, 0);
        assert_eq!((theme.align >> 4) & 0x03, 1);
    }

    #[test]
    fn blank_taskbar_drag_previews_and_snaps_to_the_nearest_edge() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.inject_pointer_button(true, 1, 400, 584, 0);
        assert!(server.wm_taskbar_drag.is_some());
        server.inject_pointer_motion(3, 300, 0x0100);
        let preview = &server.windows[&WM_SNAP_PREVIEW.0];
        assert_eq!(
            (preview.x, preview.y, preview.width, preview.height),
            (0, 0, 32, 600)
        );
        assert!(preview.mapped);

        server.inject_pointer_button(false, 1, 3, 300, 0x0100);
        assert_eq!(server.wm_taskbar_edge, WmTaskbarEdge::Left);
        assert!(!server.windows[&WM_SNAP_PREVIEW.0].mapped);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_TASKBAR_EDGE_PREFIX | WmTaskbarEdge::Left.wire()),
        );
    }

    #[test]
    fn native_wm_notifications_are_real_responsive_topmost_surfaces() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 640,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(!server.show_wm_notification(1, b"BAD\nTITLE".to_vec(), Vec::new(),));
        assert!(server.show_wm_notification(
            1,
            b"LOCAL STORAGE CONNECTED".to_vec(),
            b"PROJECT IS READY IN PCMANFM".to_vec(),
        ));
        let notification = server.windows[&WM_NOTIFICATION.0].clone();
        assert!(notification.mapped);
        assert_eq!(
            server.windows[&ROOT_WINDOW.0].children.last(),
            Some(&WM_NOTIFICATION),
        );
        let surface = &server.surfaces[&WM_NOTIFICATION.0];
        assert!(
            surface
                .pixels
                .as_ref()
                .is_some_and(|pixels| pixels.contains(&0x0022_c55e)),
        );

        assert!(server.resize_output(640, 480));
        let resized = server.windows[&WM_NOTIFICATION.0].clone();
        assert_eq!(
            resized.x,
            i16::try_from(
                640u16.saturating_sub(resized.width.saturating_add(WM_NOTIFICATION_MARGIN),),
            )
            .unwrap(),
        );
        server.inject_pointer_button(
            true,
            1,
            resized.x.saturating_add(8),
            resized.y.saturating_add(8),
            0,
        );
        assert!(!server.windows[&WM_NOTIFICATION.0].mapped);
    }

    #[test]
    fn native_wm_keeps_ewmh_desktop_windows_unframed_and_below_clients() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let desktop = ResourceId(0x0020_0090);
        server
            .feed(client, &create_window_request(order, desktop.0, 0))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &desktop.0.to_le_bytes()))
            .unwrap();
        assert!(server.managed_frames.contains_key(&desktop.0));

        let window_type = server.atoms.intern("_NET_WM_WINDOW_TYPE", false);
        let desktop_type = server.atoms.intern("_NET_WM_WINDOW_TYPE_DESKTOP", false);
        let atom_type = server.atoms.intern("ATOM", false);
        let mut property = Vec::new();
        push_u32(order, &mut property, desktop.0);
        push_u32(order, &mut property, window_type.0);
        push_u32(order, &mut property, atom_type.0);
        property.push(32);
        property.extend_from_slice(&[0, 0, 0]);
        push_u32(order, &mut property, 1);
        push_u32(order, &mut property, desktop_type.0);
        server
            .feed(client, &request(order, 18, 0, &property))
            .unwrap();

        assert!(!server.managed_frames.contains_key(&desktop.0));
        assert_eq!(server.windows[&desktop.0].parent, ROOT_WINDOW);
        assert_eq!(server.windows[&ROOT_WINDOW.0].children[0], desktop);
        assert_eq!(server.focus_window, ROOT_WINDOW);
        assert_eq!(
            server.pointer_layer(8, 8),
            0,
            "EWMH desktop windows must route to the native desktop layer",
        );
        let taskbar_y =
            i16::try_from(server.config.height_px.saturating_sub(8)).unwrap_or(i16::MAX);
        assert_eq!(
            server.pointer_layer(18, taskbar_y),
            2,
            "RiceWM shell chrome must retain topmost pointer priority",
        );

        let application = ResourceId(0x0020_0091);
        server
            .feed(client, &create_window_request(order, application.0, 0))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &application.0.to_le_bytes()))
            .unwrap();
        let frame = server.managed_frames[&application.0];
        let frame_state = &server.windows[&frame.0];
        assert_eq!(
            server.pointer_layer(
                frame_state.x.saturating_add(4),
                frame_state.y.saturating_add(4),
            ),
            3,
            "compositor-owned application frames must retain a distinct managed-frame route",
        );
        assert_eq!(
            server.pointer_layer(
                frame_state.x.saturating_add(20),
                frame_state
                    .y
                    .saturating_add(i16::try_from(WM_TITLE_HEIGHT + 4).unwrap()),
            ),
            1,
            "ordinary X11 application content must remain interactive",
        );
    }

    #[test]
    fn background_child_remapping_during_resize_preserves_focus() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig{native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let a=0x0020_0070u32;let b=0x0020_0071u32;let child=0x0020_0072u32;
        for id in [a,b] {
            server.feed(owner,&create_window_request(order,id,STRUCTURE_NOTIFY_MASK),
                ).unwrap();
            server.feed(owner,&request(order,8,0,&id.to_le_bytes())).unwrap();
        }
        server.feed(owner,&create_child_window_request(order,child,a,0,0,100,60,0),
            ).unwrap();
        for opcode in [8,10,8] {
            server.feed(owner,&request(order,opcode,0,&child.to_le_bytes())).unwrap();
            assert_eq!(server.focus_window,ResourceId(b));
        }
    }

    #[test]
    fn stale_client_focus_reply_cannot_undo_newer_activation() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig{native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let a=ResourceId(0x0020_0080);let b=ResourceId(0x0020_0081);
        for id in [a,b] {
            server.feed(owner,&create_window_request(order,id.0,STRUCTURE_NOTIFY_MASK),
                ).unwrap();
            server.feed(owner,&request(order,8,0,&id.0.to_le_bytes())).unwrap();
        }
        server.set_focus(a);
        let old_time=server.last_focus_time;
        server.set_focus(b);
        let body=|id:ResourceId,time:u32|[id.0.to_le_bytes(),time.to_le_bytes()].concat();
        server.set_input_focus(&body(a,old_time)).unwrap();
        assert_eq!(server.focus_window,b);
        server.set_input_focus(&body(a,server.timestamp.wrapping_add(100))).unwrap();
        assert_eq!(server.focus_window,b);
        server.set_input_focus(&body(a,0)).unwrap();
        assert_eq!(server.focus_window,a);
        server.timestamp=2;server.last_focus_time=u32::MAX-2;
        server.set_input_focus(&body(b,1)).unwrap();
        assert_eq!(server.focus_window,b,"valid focus survives timestamp wrap");
        server.set_input_focus(&body(a,u32::MAX-3)).unwrap();
        assert_eq!(server.focus_window,b,"stale pre-wrap focus is ignored");
    }

    #[test]
    fn client_focus_reply_cannot_raise_x11_over_an_active_native_task() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig { native_window_manager: true, ..DisplayConfig::default() });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let client = ResourceId(0x0020_0080);
        server.feed(owner, &create_window_request(order, client.0, 0)).unwrap();
        server.feed(owner, &request(order, 8, 0, &client.0.to_le_bytes())).unwrap();
        server.wm_actions.clear();
        let mut pointer = vec![0; 20];
        pointer[..4].copy_from_slice(&client.0.to_le_bytes());
        server.grab_pointer(owner, order, SequenceNumber(1), 0, &pointer).unwrap();
        assert!(server.pointer_grab.is_some());
        server.replace_native_tasks(vec![WmNativeTask {
            id: 12, workspace: 0, icon: 0, minimized: false, maximized: false,
            active: true, title: b"Terminal".to_vec(),
        }]);
        assert!(server.pointer_grab.is_none());
        server.grab_pointer(owner, order, SequenceNumber(1), 0, &pointer).unwrap();
        server.grab_keyboard(owner, order, SequenceNumber(1), 0, &pointer).unwrap();
        let mut xi = vec![0; 20];
        xi[..4].copy_from_slice(&client.0.to_le_bytes());
        for device in [2u16, 3, 4, 5] {
            xi[12..14].copy_from_slice(&device.to_le_bytes());
            let reply = server.xinput_grab_device(owner, order, SequenceNumber(1), &xi).unwrap();
            assert_eq!(reply[8], 1, "native ownership rejects a delayed XI2 grab");
        }
        assert!(server.pointer_grab.is_none());
        assert!(server.keyboard_grab.is_none());
        // Reasserting the background client's focus must not raise its layer.
        for time in [0, server.timestamp] {
            let body = [client.0.to_le_bytes(), time.to_le_bytes()].concat();
            server.set_input_focus(&body).unwrap();
            assert!(server.wm_actions.is_empty());
        }
        server.activate_managed_window(client);
        assert!(server.wm_actions.contains(&WM_ACTION_X11_FOREGROUND_PREFIX));
        server.native_tasks[0].active = false;
        server.grab_pointer(owner, order, SequenceNumber(1), 0, &pointer).unwrap();
        assert!(server.pointer_grab.is_some(), "active X11 applications can still capture input");
    }

    #[test]
    fn native_wm_focuses_sibling_clients_with_nonlinear_detail() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let main_window = 0x0020_0082;
        let dialog_window = 0x0020_0083;
        for window in [main_window, dialog_window] {
            server
                .feed(
                    client,
                    &create_window_request(
                        order,
                        window,
                        STRUCTURE_NOTIFY_MASK | FOCUS_CHANGE_MASK,
                    ),
                )
                .unwrap();
        }
        let protocols = server.atoms.intern("WM_PROTOCOLS", false);
        let take_focus = server.atoms.intern("WM_TAKE_FOCUS", false);
        server.set_wm_property(
            ResourceId(dialog_window),
            "WM_PROTOCOLS",
            "ATOM",
            32,
            take_focus.0.to_le_bytes().to_vec(),
        );
        server
            .feed(client, &request(order, 8, 0, &main_window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        server
            .feed(client, &request(order, 8, 0, &dialog_window.to_le_bytes()))
            .unwrap();
        assert_eq!(
            server.wm_task_targets(),
            vec![
                WmTaskTarget::X11(ResourceId(main_window)),
                WmTaskTarget::X11(ResourceId(dialog_window)),
            ],
            "each normal top-level dialog is independently represented in the taskbar",
        );

        let output = drain_all(&mut server, client);
        let focus_events = output
            .chunks_exact(32)
            .filter(|event| matches!(event[0] & 0x7f, 9 | 10))
            .map(|event| {
                (
                    event[0] & 0x7f,
                    event[1],
                    u32::from_le_bytes(event[4..8].try_into().unwrap()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            focus_events,
            vec![
                (10, NOTIFY_NONLINEAR, main_window),
                (9, NOTIFY_NONLINEAR, dialog_window),
            ]
        );
        let take_focus_event = output
            .chunks_exact(32)
            .find(|event| event[0] & 0x7f == 33)
            .expect("WM_TAKE_FOCUS ClientMessage");
        assert_ne!(take_focus_event[0] & 0x80, 0);
        assert_eq!(take_focus_event[1], 32);
        assert_eq!(
            u32::from_le_bytes(take_focus_event[4..8].try_into().unwrap()),
            dialog_window
        );
        assert_eq!(
            u32::from_le_bytes(take_focus_event[8..12].try_into().unwrap()),
            protocols.0
        );
        assert_eq!(
            u32::from_le_bytes(take_focus_event[12..16].try_into().unwrap()),
            take_focus.0
        );
        assert_ne!(
            u32::from_le_bytes(take_focus_event[16..20].try_into().unwrap()),
            0
        );
    }
