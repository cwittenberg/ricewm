use super::*;


    #[test]
    fn window_icon_menu_and_alt_window_shortcuts_share_task_commands() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0197);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, owner);
        let frame = server.managed_frames[&client.0];
        let frame_state = server.windows[&frame.0].clone();

        server.inject_pointer_button(
            true,
            1,
            frame_state.x.saturating_add(8),
            frame_state.y.saturating_add(8),
            0,
        );
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::TaskContext(WmTaskTarget::X11(client))
        );
        assert!(server.windows[&WM_MENU.0].mapped);
        assert_eq!(
            server.wm_menu_columns()[0],
            vec![
                WmMenuRow::WindowMinimize,
                WmMenuRow::WindowMaximize,
                WmMenuRow::Divider,
                WmMenuRow::WindowClose,
            ],
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.text == b"Alt+F4")
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.text == b"Alt+F9")
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| overlay.text == b"Alt+F10")
        );

        server.inject_key(true, 76, 0x0008);
        assert!(server.maximized_geometries.contains_key(&client.0));
        server.inject_key(true, 76, 0x0008);
        assert!(!server.maximized_geometries.contains_key(&client.0));

        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: 17,
            workspace: 0,
            icon: 1,
            minimized: false,
            maximized: true,
            active: true,
            title: b"RiceFiles".to_vec(),
        }]));
        assert!(server.show_native_task_menu(17, 100, 120));
        assert_eq!(
            server.wm_menu_columns()[0],
            vec![
                WmMenuRow::WindowRestore,
                WmMenuRow::WindowMinimize,
                WmMenuRow::Divider,
                WmMenuRow::WindowClose,
            ],
        );
        while server.drain_wm_action().is_some() {}
        server.inject_key(true, 76, 0x0008);
        server.inject_key(true, 75, 0x0008);
        server.inject_key(true, 70, 0x0008);
        assert_eq!(
            server.wm_actions.into_iter().collect::<Vec<_>>(),
            vec![
                WM_ACTION_TASK_RESTORE_PREFIX | 17,
                WM_ACTION_TASK_MINIMIZE_PREFIX | 17,
                WM_ACTION_TASK_CLOSE_PREFIX | 17,
            ],
        );
    }

    #[test]
    fn native_wm_chrome_has_hover_active_and_incremental_workspace_states() {
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
        let window = ResourceId(0x0020_0087);
        server
            .feed(
                client,
                &create_window_request(order, window.0, STRUCTURE_NOTIFY_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);

        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(330, 10) & 0x00ff_ffff,
            0x00e9_5420,
            "the focused application task must use the active accent",
        );
        server.inject_pointer_motion(18, 584, 0);
        assert_eq!(server.wm_chrome_hover, Some(WmChromeTarget::Start));
        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(100, 10) & 0x00ff_ffff,
            0x00e9_5420,
            "the hovered RiceWM button must use the theme accent",
        );
        server.inject_pointer_motion(143, 574, 0);
        assert_eq!(server.wm_chrome_hover, Some(WmChromeTarget::QuickLaunch(0)),);
        assert!(server.windows[&WM_TOOLTIP.0].mapped);
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| { overlay.window == WM_TOOLTIP && overlay.text == b"Show desktop" })
        );
        assert_eq!(
            server.wm_quick_launchers[1].icon,
            server
                .wm_menu_entries
                .iter()
                .find(|entry| entry.action == WM_ACTION_PCMANFM)
                .unwrap()
                .icon,
            "the Files shortcut and RiceFiles menu entry share one icon",
        );
        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(100, 10) & 0x00ff_ffff,
            0x0050_4654,
            "the RiceWM button returns to its idle theme color",
        );

        let frame = server.managed_frames[&window.0];
        let frame_state = server.windows[&frame.0].clone();
        let maximize_x = frame_state
            .x
            .saturating_add(i16::try_from(frame_state.width).unwrap())
            .saturating_sub(42);
        server.inject_pointer_motion(maximize_x, frame_state.y.saturating_add(2), 0);
        assert_eq!(
            server.wm_chrome_hover,
            Some(WmChromeTarget::FrameControl(frame, 1)),
        );
        assert!(!server.windows[&WM_TOOLTIP.0].mapped);
        assert_eq!(
            server.surfaces[&frame.0].pixel(frame_state.width - 42, 2) & 0x00ff_ffff,
            0x00e9_5420,
            "title controls must visibly adopt the hover accent",
        );

        let _ = drain_scene(&mut server);
        server.switch_workspace(1);
        assert!(!server.windows[&frame.0].mapped);
        let workspace_scene = drain_scene(&mut server);
        assert!(!workspace_scene.is_empty());
        assert!(
            workspace_scene
                .chunks_exact(SCENE_COMMAND_BYTES)
                .all(|command| command[0] != SCENE_RESET),
            "workspace switching must update visibility incrementally",
        );
        server.switch_workspace(0);
        assert!(server.windows[&frame.0].mapped);
        let second = ResourceId(0x0020_0088);
        server
            .feed(
                client,
                &create_window_request(order, second.0, STRUCTURE_NOTIFY_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &second.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let _ = drain_scene(&mut server);
        server.activate_managed_window(window);
        let activation_scene = drain_scene(&mut server);
        assert_incremental_restack(&activation_scene, &[window.0, second.0]);
        let active_frame = server.managed_frames[&window.0];
        assert!(
            activation_scene
                .chunks_exact(SCENE_COMMAND_BYTES)
                .any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == active_frame.0
                        && command[1] & 64 != 0
                }),
            "focused X11 frames are marked for compositor focus effects"
        );
    }

    #[test]
    fn resizing_a_mapped_managed_client_emits_a_full_exposure() {
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
        let window = 0x0020_0081;
        server
            .feed(
                client,
                &create_window_request(order, window, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let mut configure = Vec::new();
        push_u32(order, &mut configure, window);
        push_u16(order, &mut configure, (1 << 2) | (1 << 3));
        push_u16(order, &mut configure, 0);
        push_u32(order, &mut configure, 480);
        push_u32(order, &mut configure, 300);
        server
            .feed(client, &request(order, 12, 0, &configure))
            .unwrap();

        let events = drain_all(&mut server, client);
        let event_types = events
            .chunks_exact(32)
            .map(|event| event[0] & 0x7f)
            .collect::<Vec<_>>();
        assert_eq!(event_types, vec![22, 12]);
        assert_eq!(events[0] & 0x80, 0);
        assert_eq!(
            i16::from_le_bytes(events[16..18].try_into().unwrap()),
            i16::try_from(WM_BORDER_WIDTH).unwrap()
        );
        assert_eq!(i16::from_le_bytes(events[18..20].try_into().unwrap()), 24);
        let expose = &events[32..64];
        assert_eq!(u32::from_le_bytes(expose[4..8].try_into().unwrap()), window);
        assert_eq!(u16::from_le_bytes(expose[12..14].try_into().unwrap()), 480);
        assert_eq!(u16::from_le_bytes(expose[14..16].try_into().unwrap()), 300);
    }

    #[test]
    fn freed_pixmaps_remain_alive_while_picture_and_gc_state_reference_them() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let pixmap = ResourceId(0x0020_0081);
        server.pixmaps.insert(
            pixmap.0,
            Pixmap {
                id: pixmap,
                owner: 1,
            },
        );
        server.surfaces.insert(pixmap.0, Surface::new(8, 8, 1, 1));
        server.pictures.insert(
            0x0020_0082,
            Picture {
                owner: 1,
                source: PictureSource::Drawable(pixmap),
                format: x11::RENDER_FMT_A1,
                repeat: 0,
                component_alpha: false,
                clip_x_origin: 0,
                clip_y_origin: 0,
                clip_mask: Some(pixmap),
                clip_rectangles: None,
                transform: render_identity_transform(),
                filter: PictureFilter::Nearest,
            },
        );
        let mut gc = GraphicsContext::new(1, 1);
        gc.tile = Some(pixmap);
        gc.stipple = Some(pixmap);
        gc.clip_mask = Some(pixmap);
        server.graphics_contexts.insert(0x0020_0083, gc);

        server.release_pixmap_resource(pixmap);

        assert!(!server.pixmaps.contains_key(&pixmap.0));
        assert!(!server.surfaces.contains_key(&pixmap.0));
        let PictureSource::Drawable(retained) = server.pictures[&0x0020_0082].source else {
            panic!("drawable picture was not retained");
        };
        assert!(retained.0 >= 0xf000_0000);
        assert_eq!(server.pictures[&0x0020_0082].clip_mask, Some(retained));
        let retained_gc = &server.graphics_contexts[&0x0020_0083];
        assert_eq!(retained_gc.tile, Some(retained));
        assert_eq!(retained_gc.stipple, Some(retained));
        assert_eq!(retained_gc.clip_mask, Some(retained));
        assert_eq!(server.surfaces[&retained.0].pixel(0, 0), 1);

        server.pictures.clear();
        server.graphics_contexts.clear();
        server.collect_orphan_surfaces();
        assert!(!server.surfaces.contains_key(&retained.0));
    }

    #[test]
    fn gc_pixmap_fill_put_image_and_copy_area_update_retained_pixels() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let _ = drain_scene(&mut server);
        let window = 0x0020_0100;
        let gc = 0x0020_0101;
        let pixmap = 0x0020_0102;

        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        let mut gc_body = Vec::new();
        push_u32(order, &mut gc_body, gc);
        push_u32(order, &mut gc_body, window);
        push_u32(order, &mut gc_body, 1 << 2);
        push_u32(order, &mut gc_body, 0x00ff_0000);
        server
            .feed(client, &request(order, 55, 0, &gc_body))
            .unwrap();

        let mut fill = Vec::new();
        push_u32(order, &mut fill, window);
        push_u32(order, &mut fill, gc);
        push_u16(order, &mut fill, 4);
        push_u16(order, &mut fill, 5);
        push_u16(order, &mut fill, 2);
        push_u16(order, &mut fill, 1);
        server.feed(client, &request(order, 70, 0, &fill)).unwrap();
        let mut rgba = [0; 8];
        assert_eq!(
            server.read_surface_rgba(window, 4, 5, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(rgba, [255, 0, 0, 255, 255, 0, 0, 255]);

        let mut pixmap_body = Vec::new();
        push_u32(order, &mut pixmap_body, pixmap);
        push_u32(order, &mut pixmap_body, window);
        push_u16(order, &mut pixmap_body, 2);
        push_u16(order, &mut pixmap_body, 1);
        server
            .feed(client, &request(order, 53, 24, &pixmap_body))
            .unwrap();

        let mut image = Vec::new();
        push_u32(order, &mut image, pixmap);
        push_u32(order, &mut image, gc);
        push_u16(order, &mut image, 2);
        push_u16(order, &mut image, 1);
        push_u16(order, &mut image, 0);
        push_u16(order, &mut image, 0);
        image.push(0);
        image.push(24);
        image.extend_from_slice(&[0, 0]);
        push_u32(order, &mut image, 0x0000_ff00);
        push_u32(order, &mut image, 0x0000_ff00);
        server.feed(client, &request(order, 72, 2, &image)).unwrap();

        let mut copy = Vec::new();
        push_u32(order, &mut copy, pixmap);
        push_u32(order, &mut copy, window);
        push_u32(order, &mut copy, gc);
        push_u16(order, &mut copy, 0);
        push_u16(order, &mut copy, 0);
        push_u16(order, &mut copy, 10);
        push_u16(order, &mut copy, 10);
        push_u16(order, &mut copy, 2);
        push_u16(order, &mut copy, 1);
        server.feed(client, &request(order, 62, 0, &copy)).unwrap();
        let no_exposure = drain_all(&mut server, client);
        assert_eq!(no_exposure.len(), 32);
        assert_eq!(no_exposure[0], 14);

        assert_eq!(
            server.read_surface_rgba(window, 10, 10, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(rgba, [0, 255, 0, 255, 0, 255, 0, 255]);

        let scene = drain_scene(&mut server);
        let damages: Vec<&[u8]> = scene
            .chunks_exact(SCENE_COMMAND_BYTES)
            .filter(|command| command[0] == SCENE_DAMAGE_WINDOW)
            .collect();
        assert_eq!(damages.len(), 2);
        assert_eq!(
            i32::from_le_bytes(damages[0][12..16].try_into().unwrap()),
            4
        );
        assert_eq!(
            i32::from_le_bytes(damages[1][12..16].try_into().unwrap()),
            10
        );

        server
            .feed(client, &request(order, 54, 0, &pixmap.to_le_bytes()))
            .unwrap();
        server
            .feed(client, &request(order, 60, 0, &gc.to_le_bytes()))
            .unwrap();
        assert!(drain_all(&mut server, client).is_empty());
    }

    #[test]
    fn native_bgra_upload_updates_existing_drawable_and_damage() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let _ = drain_scene(&mut server);
        let window = 0x0020_0150;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        let _ = drain_scene(&mut server);

        let source = [
            0x10, 0x20, 0x40, 0xff, 0x30, 0x60, 0x90, 0xff, 0xaa, 0xbb, 0xcc, 0xff, 0x00, 0x11,
            0x22, 0xff,
        ];
        assert!(server.write_surface_bgra(window, 3, 4, 2, 2, 8, &source));
        let mut rgba = [0; 16];
        assert_eq!(
            server.read_surface_rgba(window, 3, 4, 2, 2, &mut rgba),
            Some(16)
        );
        assert_eq!(
            rgba,
            [
                0x40, 0x20, 0x10, 0xff, 0x90, 0x60, 0x30, 0xff, 0xcc, 0xbb, 0xaa, 0xff, 0x22, 0x11,
                0x00, 0xff,
            ]
        );
        assert!(!server.write_surface_bgra(window, 3, 4, 2, 2, 7, &source));
        assert!(server.write_surface_bgra(window, 319, 4, 2, 2, 8, &source));
        assert!(!server.write_surface_bgra(window, 320, 4, 1, 1, 4, &source));

        let scene = drain_scene(&mut server);
        let damage = scene
            .chunks_exact(SCENE_COMMAND_BYTES)
            .find(|command| command[0] == SCENE_DAMAGE_WINDOW)
            .expect("native upload emits retained damage");
        assert_eq!(u32::from_le_bytes(damage[4..8].try_into().unwrap()), window);
        assert_eq!(i32::from_le_bytes(damage[12..16].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(damage[16..20].try_into().unwrap()), 4);
        assert_eq!(u16::from_le_bytes(damage[20..22].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(damage[22..24].try_into().unwrap()), 2);
    }

    #[test]
    fn put_image_repaints_changed_pixels_but_preserves_xdamage() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let window = 0x0020_0180;
            let gc = 0x0020_0181;
            server.feed(client, &create_window_request(order, window, 0)).unwrap();
            let mut gc_body = Vec::new();
            push_u32(order, &mut gc_body, gc);
            push_u32(order, &mut gc_body, window);
            push_u32(order, &mut gc_body, 0);
            server.feed(client, &request(order, 55, 0, &gc_body)).unwrap();
            let mut image = Vec::new();
            push_u32(order, &mut image, window);
            push_u32(order, &mut image, gc);
            for value in [8, 4, 3, 5] { push_u16(order, &mut image, value); }
            image.extend_from_slice(&[0, 24, 0, 0]);
            for _ in 0..32 { push_u32(order, &mut image, 0x00112233); }
            server.feed(client, &request(order, 72, 2, &image)).unwrap();
            let _ = drain_scene(&mut server);
            server.damage_resources.insert(0x0020_0182, super::DamageResource {
                owner: client.0, drawable: ResourceId(window), level: 0, pending: Vec::new(),
            });
            // A repeat upload must notify XDamage clients without scheduling
            // any desktop repaint.
            server.feed(client, &request(order, 72, 2, &image)).unwrap();
            assert!(drain_scene(&mut server).is_empty());
            let pending = &server.damage_resources[&0x0020_0182].pending;
            assert_eq!((pending[0].x, pending[0].y, pending[0].width, pending[0].height), (3, 5, 8, 4));
            // Change one pixel inside the otherwise identical full image.
            let offset = 20 + (8 + 2) * 4;
            let mut changed = Vec::new();
            push_u32(order, &mut changed, 0x00abcdef);
            image[offset..offset + 4].copy_from_slice(&changed);
            server.feed(client, &request(order, 72, 2, &image)).unwrap();
            let scene = drain_scene(&mut server);
            let damages: Vec<_> = scene.chunks_exact(SCENE_COMMAND_BYTES)
                .filter(|command| command[0] == SCENE_DAMAGE_WINDOW).collect();
            assert_eq!(damages.len(), 1);
            let damage = damages[0];
            assert_eq!(i32::from_le_bytes(damage[12..16].try_into().unwrap()), 5);
            assert_eq!(i32::from_le_bytes(damage[16..20].try_into().unwrap()), 6);
            assert_eq!(u16::from_le_bytes(damage[20..22].try_into().unwrap()), 1);
            assert_eq!(u16::from_le_bytes(damage[22..24].try_into().unwrap()), 1);
            let mut rgba = [0; 4];
            assert_eq!(server.read_surface_rgba(window, 5, 6, 1, 1, &mut rgba), Some(4));
            assert_eq!(rgba, [0xab, 0xcd, 0xef, 0xff]);
        }
    }
