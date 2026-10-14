use super::*;


    #[test]
    fn gnome_50_panel_keeps_taskbar_and_renders_extension_indicator() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let indicator =
            |visible: bool, has_icon: bool, position: u8, label: &[u8]| WmGnomeIndicator {
                uuid: b"legacy".to_vec(),
                visible,
                has_icon,
                position,
                index: 0,
                label: label.to_vec(),
                image: None,
            };
        assert!(server.update_gnome_panel(true, vec![indicator(true, true, 2, b"198.51.100.4")],));
        assert_eq!(server.windows[&WM_GNOME_PANEL.0].y, 0);
        assert!(server.windows[&WM_GNOME_PANEL.0].mapped);
        assert_eq!(
            server.windows[&WM_TASKBAR.0].y,
            600 - WM_TASKBAR_HEIGHT as i16
        );
        assert!(server.windows[&WM_TASKBAR.0].mapped);
        assert_eq!(
            server.wm_workarea(),
            (WM_TASKBAR_HEIGHT, 600 - WM_TASKBAR_HEIGHT * 2)
        );
        assert_eq!(
            server.surfaces[&WM_GNOME_PANEL.0].pixel(400, 10),
            server.wm_themes[usize::from(server.wm_theme)].taskbar[0] | 0xff00_0000
        );
        assert!(server.wm_text_overlays.iter().any(|overlay| {
            overlay.window == WM_GNOME_PANEL && overlay.text == b"198.51.100.4"
        }));
        assert!(server.update_gnome_indicator_image(
            b"legacy".to_vec(),
            2,
            1,
            vec![0xffff_0000, 0xff00_00ff],
        ));
        let (_, indicator_x, _) = server.wm_gnome_indicator_layout()[0];
        assert_eq!(
            server.surfaces[&WM_GNOME_PANEL.0].pixel(
                u16::try_from(indicator_x).unwrap_or(0) + 8,
                WM_TASKBAR_HEIGHT.saturating_sub(1) / 2,
            ),
            0xffff_0000,
        );
        assert!(
            !server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.text == b"Activities")
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| { overlay.window == WM_TASKBAR && overlay.text == b"RiceWM" })
        );
        assert!(
            !server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.window == WM_GNOME_PANEL && overlay.text == b"VOL  NET  PWR")
        );
        let (_, right_x, _) = server.wm_gnome_indicator_layout()[0];
        server.inject_pointer_button(true, 1, right_x + 4, 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_GNOME_INDICATOR_PREFIX | (1 << 8))
        );
        server.inject_pointer_button(true, 2, right_x + 4, 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_GNOME_INDICATOR_PREFIX | (2 << 8))
        );
        server.inject_pointer_button(true, 3, right_x + 4, 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_GNOME_INDICATOR_PREFIX | (3 << 8))
        );

        assert!(server.update_gnome_menu(vec![
            WmGnomeMenuItem {
                index: 0,
                separator: true,
                reactive: false,
                toggle: false,
                icon_kind: 0,
                image: false,
                checked: false,
                label: b"Network Details".to_vec(),
            },
            WmGnomeMenuItem {
                index: 2,
                separator: false,
                reactive: true,
                toggle: true,
                icon_kind: 4,
                image: false,
                checked: true,
                label: b"Settings".to_vec(),
            },
            WmGnomeMenuItem {
                index: 4,
                separator: false,
                reactive: true,
                toggle: false,
                icon_kind: 7,
                image: false,
                checked: false,
                label: b"Netherlands".to_vec(),
            },
            WmGnomeMenuItem {
                index: 3,
                separator: false,
                reactive: true,
                toggle: false,
                icon_kind: 0,
                image: true,
                checked: false,
                label: Vec::new(),
            },
        ]));
        assert!(server.windows[&WM_GNOME_MENU.0].mapped);
        assert!(server.update_gnome_menu_image(
            2,
            2,
            vec![0xffff_0000, 0xff00_ff00, 0xff00_00ff, 0xffff_ffff],
        ));
        assert!(server.update_gnome_menu_icon(4, 2, 1, vec![0xffff_0000, 0xff00_00ff],));
        assert_eq!(server.surfaces[&WM_GNOME_MENU.0].pixel(16, 78), 0xffff_0000);
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| { overlay.window == WM_GNOME_MENU && overlay.text == b"Settings" })
        );
        let menu = server.windows[&WM_GNOME_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 6 + 28 + 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_GNOME_MENU_ITEM_PREFIX | (1 << 8) | 2),
        );
        server.inject_pointer_button(true, 3, menu.x + 20, menu.y + 6 + 28 + 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_GNOME_MENU_ITEM_PREFIX | (3 << 8) | 2),
        );

        assert!(server.update_gnome_panel(true, vec![indicator(true, true, 0, b"IP")],));
        assert!(server.update_gnome_panel(true, vec![indicator(true, true, 1, b"IP")],));
        assert!(
            server.wm_gnome_indicators[0].image.is_some(),
            "a repeated actor snapshot retains separately uploaded icon pixels",
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.window == WM_GNOME_PANEL && overlay.text == b"IP")
        );

        assert!(server.update_gnome_panel(true, Vec::new()));
        assert!(!server.wm_text_overlays.iter().any(|overlay| {
            overlay.window == WM_GNOME_PANEL && overlay.text == b"198.51.100.4"
        }));
        assert!(server.update_gnome_panel(false, Vec::new()));
        assert!(!server.windows[&WM_GNOME_PANEL.0].mapped);
        assert_eq!(
            server.windows[&WM_TASKBAR.0].y,
            600 - WM_TASKBAR_HEIGHT as i16
        );
        assert_eq!(server.wm_workarea(), (0, 600 - WM_TASKBAR_HEIGHT));
    }

    #[test]
    fn repeated_animated_extension_frames_preserve_compact_dock_hover() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let indicator = WmGnomeIndicator {
            uuid: b"starfield@cwittenberg".to_vec(),
            visible: true,
            has_icon: false,
            position: 2,
            index: 0,
            label: b"Starfield".to_vec(),
            image: None,
        };
        assert!(server.configure_gnome_panel(36, false));
        assert!(server.update_gnome_panel(true, vec![indicator.clone()]));
        assert!(server.set_dock_style(true));
        let (dock_x, dock_y, _, dock_height) = server.wm_taskbar_rect();
        let step = server.wm_dock_step();
        let pointer_x = dock_x
            + i16::try_from(WM_COMPACT_DOCK_AXIS_PADDING + step / 2).unwrap();
        let pointer_y = dock_y + i16::try_from(dock_height / 2).unwrap();
        server.inject_pointer_motion(pointer_x, pointer_y, 0);
        let hovered = server.wm_chrome_hover.expect("compact dock hover");
        let hovered_width = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_TASKBAR && overlay.text == b"__dock_icon:logo:0"
            )
            .expect("magnified dock icon")
            .width;
        assert!(hovered_width > WM_COMPACT_DOCK_ICON_SIZE as u16);
        let _ = drain_scene(&mut server);

        for _ in 0..4 {
            assert!(server.configure_gnome_panel(36, false));
            assert!(server.update_gnome_panel(true, vec![indicator.clone()]));
            assert!(server.update_gnome_background(Vec::new(), Vec::new()));
            assert_eq!(server.wm_chrome_hover, Some(hovered));
            assert_eq!(
                server
                    .wm_text_overlays
                    .iter()
                    .find(|overlay| {
                        overlay.window == WM_TASKBAR
                            && overlay.text == b"__dock_icon:logo:0"
                    })
                    .expect("retained magnified dock icon")
                    .width,
                hovered_width,
            );
            assert!(
                drain_scene(&mut server).is_empty(),
                "unchanged animated-extension metadata must not rebuild the scene"
            );
        }
    }

    #[test]
    fn native_system_cluster_is_opt_in_and_uses_exactly_one_configured_shell_edge() {
        let mut server=DisplayServer::new(DisplayConfig{width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        assert_eq!(server.wm_system_controls,0,"browser/shared default remains unchanged");
        assert!(server.update_gnome_panel(true,Vec::new()));
        assert!(server.set_system_controls(0x3f));
        assert!(server.set_mail_indicator(Some(0)));
        assert!(server.update_gnome_panel(true, vec![WmGnomeIndicator {
            uuid: b"spacing".to_vec(),
            visible: true,
            has_icon: true,
            position: 2,
            index: 0,
            label: b"Mail".to_vec(),
            image: None,
        }]));
        let (_, indicator_x, indicator_width) = server.wm_gnome_indicator_layout()[0];
        let cluster_x = i32::from(server.config.width_px)
            - i32::try_from(WM_SYSTEM_CONTROLS_WIDTH + WM_SYSTEM_CONTROLS_RIGHT_MARGIN)
                .unwrap_or(i32::MAX);
        let mail_x = cluster_x
            - i32::try_from(WM_SYSTEM_CONTROLS_EXTENSION_GAP + WM_RICE_BOX_INDICATOR_WIDTH)
                .unwrap_or(i32::MAX);
        assert!(
            i32::from(indicator_x) + i32::from(indicator_width)
                <= mail_x - i32::try_from(WM_SYSTEM_CONTROLS_EXTENSION_GAP + 74).unwrap_or(0),
            "extension indicators reserve search and applications before Rice Box and system controls",
        );
        assert!(!server.wm_tray_layout().iter().any(|(_,item,_,_)|*item==TrayItem::Connectivity));
        assert_eq!(server.wm_chrome_hit(WM_GNOME_PANEL,795,12),Some(WmChromeTarget::SystemControls));
        assert!(server.handle_wm_pointer_press(WM_GNOME_PANEL,795,12));
        assert_eq!(server.drain_wm_action(),Some(WM_ACTION_SYSTEM_CONTROLS));
        assert_ne!(server.wm_chrome_hit(WM_TASKBAR,795,590),Some(WmChromeTarget::SystemControls));
        assert!(server.set_system_controls(0x7f));
        assert_eq!(server.wm_chrome_hit(WM_GNOME_PANEL,795,12),None);
        assert!(server.handle_wm_pointer_press(WM_TASKBAR,795,590));
        assert_eq!(server.drain_wm_action(),Some(WM_ACTION_SYSTEM_CONTROLS));
        assert!(!server.set_system_controls(0x80));
    }

    #[test]
    fn native_rice_box_indicator_survives_panel_repaint_and_reserves_extension_space() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.update_gnome_panel(
            true,
            vec![WmGnomeIndicator {
                uuid: b"extension".to_vec(),
                visible: true,
                has_icon: true,
                position: 2,
                index: 0,
                label: b"Status".to_vec(),
                image: None,
            }],
        ));
        assert_eq!(server.wm_mail_unread, None, "shared/browser default is unchanged");
        let (_, browser_x, browser_width) = server.wm_gnome_indicator_layout()[0];
        assert!(i32::from(browser_x) + i32::from(browser_width)
            <= 800 - 94 - WM_RICE_BOX_INDICATOR_WIDTH as i32 - 74
                - WM_SYSTEM_CONTROLS_EXTENSION_GAP as i32,
            "browser canvas accessories reserve space without retained native mail pixels");
        assert!(server.set_mail_indicator(Some(7)));
        let mail_x = 800usize - 94 - WM_RICE_BOX_INDICATOR_WIDTH;
        assert_eq!(
            server.surfaces[&WM_GNOME_PANEL.0].pixel(
                u16::try_from(mail_x + 24).unwrap(),
                4,
            ),
            0xffe8_4716,
        );
        let (_, extension_x, extension_width) = server.wm_gnome_indicator_layout()[0];
        assert!(
            i32::from(extension_x) + i32::from(extension_width)
                <= i32::try_from(mail_x - WM_SYSTEM_CONTROLS_EXTENSION_GAP).unwrap(),
            "the extension remains visible to the left of Rice Box",
        );

        assert!(server.set_wall_clock(12, 34));
        assert_eq!(
            server.surfaces[&WM_GNOME_PANEL.0].pixel(
                u16::try_from(mail_x + 24).unwrap(),
                4,
            ),
            0xffe8_4716,
            "ordinary panel damage must retain Rice Box",
        );
        assert_eq!(
            server.wm_chrome_hit(
                WM_GNOME_PANEL,
                i16::try_from(mail_x + WM_RICE_BOX_INDICATOR_WIDTH / 2).unwrap(),
                12,
            ),
            Some(WmChromeTarget::RiceBox),
        );
        server.inject_pointer_button(
            true,
            1,
            i16::try_from(mail_x + WM_RICE_BOX_INDICATOR_WIDTH / 2).unwrap(),
            12,
            0,
        );
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RICE_BOX));
        server.inject_pointer_button(
            true,
            3,
            i16::try_from(mail_x + WM_RICE_BOX_INDICATOR_WIDTH / 2).unwrap(),
            12,
            0,
        );
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RICE_BOX_COMPOSE));
        assert!(!server.set_mail_indicator(Some(1000)));
    }

    #[test]
    fn native_system_cluster_clears_extension_pixels_beneath_rounded_corners() {
        let mut pixels=vec![0x00ff_ffff;200*32];
        draw_system_controls_cluster(
            &mut pixels,200,32,40,3,0x3f,None,0x00ff_ffff,0x0080_8080,
            0x0024_2424,0x0044_4444,
        );
        assert_eq!(pixels[3*200+40],0x0024_2424);
    }

    #[test]
    fn native_system_cluster_adds_battery_percentage_only_when_present() {
        let mut server=DisplayServer::new(DisplayConfig{width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        assert!(server.update_gnome_panel(true,Vec::new()));
        assert!(server.set_system_controls(0x3f));
        assert!(server.set_system_battery(Some(82)));
        let battery_cluster_left=800i32-i32::try_from(WM_SYSTEM_CONTROLS_BATTERY_WIDTH+WM_SYSTEM_CONTROLS_RIGHT_MARGIN).unwrap();
        assert_eq!(server.wm_chrome_hit(WM_GNOME_PANEL,battery_cluster_left as i16,12),Some(WmChromeTarget::SystemControls));
        assert!(!server.set_system_battery(Some(101)));
        assert!(server.set_system_battery(None));
        let desktop_cluster_left=800i32-i32::try_from(WM_SYSTEM_CONTROLS_WIDTH+WM_SYSTEM_CONTROLS_RIGHT_MARGIN).unwrap();
        assert_eq!(server.wm_chrome_hit(WM_GNOME_PANEL,(desktop_cluster_left-1) as i16,12),None);
    }

    #[test]
    fn gnome_background_renders_without_intercepting_input_and_unmaps_on_clear() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.update_gnome_background(
            vec![WmGnomeOverlayRect {
                x: 10,
                y: 20,
                width: 30,
                height: 40,
                color: 0x5200_0502,
            }],
            vec![WmGnomeBackgroundText {
                x: 15,
                y: 25,
                width: 16,
                height: 18,
                font_px: 14,
                weight: 4,
                align: 1,
                foreground: 0xff20_ee62,
                text: b"A".to_vec(),
            }],
        ));
        assert!(server.windows[&WM_GNOME_BACKGROUND.0].mapped);
        assert_eq!(
            server.surfaces[&WM_GNOME_BACKGROUND.0].pixel(12, 22),
            0x5200_0502
        );
        assert!(server.wm_text_overlays.iter().any(|overlay| {
            overlay.window == WM_GNOME_BACKGROUND
                && overlay.text == b"A"
                && overlay.foreground == 0xff20_ee62
        }));
        assert_ne!(server.hit_test(16, 26), WM_GNOME_BACKGROUND);
        assert_ne!(server.child_at(ROOT_WINDOW, 16, 26), WM_GNOME_BACKGROUND);

        assert!(server.update_gnome_background(Vec::new(), Vec::new()));
        assert!(!server.windows[&WM_GNOME_BACKGROUND.0].mapped);
        assert_eq!(server.surfaces[&WM_GNOME_BACKGROUND.0].pixel(12, 22), 0);
        assert!(
            !server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.window == WM_GNOME_BACKGROUND)
        );
    }

    #[test]
    fn switcher_retains_minimized_windows_and_filters_workspace_and_parked_clients() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800, height_px: 600, native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server.feed(client, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, client);
        let window = ResourceId(0x0020_0095);
        server.feed(client, &create_window_request(order, window.0, 0)).unwrap();
        server.feed(client, &request(order, 8, 0, &window.0.to_le_bytes())).unwrap();
        let frame = server.managed_frames[&window.0];
        server.minimize_managed_task(window);
        assert_eq!(server.switcher_window(0), Some(frame.0));
        assert_eq!(server.switcher_window(1), None);
        assert!(server.activate_overview_window(frame.0));
        assert!(!server.minimized_windows.contains(&window.0));
        server.window_workspaces.insert(window.0, 1);
        assert_eq!(server.switcher_window(0), None);
        server.window_workspaces.insert(window.0, 0);
        server.parked_managed_windows.insert(window.0);
        assert_eq!(server.switcher_window(0), None);
    }

    #[test]
    fn x11_task_only_minimizes_when_it_is_globally_frontmost() {
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
        let window = ResourceId(0x0020_0095);
        server
            .feed(client, &create_window_request(order, window.0, 0))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
            .unwrap();
        while server.drain_wm_action().is_some() {}
        let frame = server.managed_frames[&window.0];
        let active_frame_color = server.surfaces[&frame.0].pixel(160, 5);

        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: u16::MAX,
            workspace: 0,
            icon: 2,
            minimized: false,
            maximized: false,
            active: true,
            title: b"Rise Terminal".to_vec(),
        }]));
        let inactive_frame_color = server.surfaces[&frame.0].pixel(160, 5);
        assert_ne!(active_frame_color, inactive_frame_color);
        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert!(
            !server.minimized_windows.contains(&window.0),
            "the X11 task must be raised, not minimized, while another task is frontmost",
        );
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_X11_FOREGROUND_PREFIX),
        );

        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: u16::MAX,
            workspace: 0,
            icon: 2,
            minimized: true,
            maximized: false,
            active: false,
            title: b"Rise Terminal".to_vec(),
        }]));
        assert_eq!(server.surfaces[&frame.0].pixel(160, 5), active_frame_color);
        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert!(
            server.minimized_windows.contains(&window.0),
            "a second click minimizes the now-frontmost X11 task",
        );
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_X11_FOREGROUND_PREFIX),
        );
        let _ = drain_scene(&mut server);
        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert!(
            !server.minimized_windows.contains(&window.0),
            "clicking a parked X11 task must restore it",
        );
        let restored = drain_scene(&mut server);
        assert!(
            restored.chunks_exact(SCENE_COMMAND_BYTES).any(|command| {
                command[0] == SCENE_UPSERT_WINDOW
                    && u32::from_le_bytes(command[4..8].try_into().unwrap()) == window.0
                    && command[1] & 1 == 1
            }),
            "restoring a frame must make its retained client surface viewable"
        );
    }
