use super::*;


    #[test]
    fn window_search_includes_other_workspaces_and_minimized_but_excludes_preloads() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig { native_window_manager: true, ..DisplayConfig::default() });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0184);
        server.feed(owner, &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK),
            ).unwrap();
        server.feed(owner, &request(order, 8, 0, &client.0.to_le_bytes())).unwrap();
        let frame = server.managed_frames[&client.0];
        assert_eq!(server.overview_client(frame.0),Some(client.0));
        assert_eq!(server.overview_client(ROOT_WINDOW.0),None);
        server.minimized_windows.insert(client.0);
        assert_eq!(server.overview_client(frame.0),None);
        server.switch_workspace(1);
        let snapshot = server.search_window_snapshot();
        assert_eq!(u32::from_le_bytes(snapshot[0..4].try_into().unwrap()), frame.0);
        assert_eq!(u32::from_le_bytes(snapshot[4..8].try_into().unwrap()), 0);
        assert_eq!(u32::from_le_bytes(snapshot[8..12].try_into().unwrap()), 1);
        assert_eq!(&snapshot[16..], b"APPLICATION");
        assert!(!server.activate_overview_window(frame.0));
        server.switch_workspace(0);
        assert!(server.activate_overview_window(frame.0));
        assert!(!server.minimized_windows.contains(&client.0));
        assert_eq!(server.overview_client(frame.0),Some(client.0));
        server.parked_managed_windows.insert(client.0);
        assert_eq!(server.overview_client(frame.0),None);
        assert!(server.search_window_snapshot().is_empty());
        server.parked_managed_windows.remove(&client.0);
        server.feed(owner, &request(order, 4, 0, &client.0.to_le_bytes())).unwrap();
        assert!(server.search_window_snapshot().is_empty());
        assert!(!server.activate_overview_window(frame.0));
    }

    #[test]
    fn parked_managed_window_paints_while_hidden_then_activates_atomically() {
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
        let _ = drain_scene(&mut server);

        assert!(server.request_park_next_managed_window());
        assert!(!server.request_park_next_managed_window());
        let client = ResourceId(0x0020_0184);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();

        assert_eq!(server.parked_managed_window(), Some(client.0));
        assert_eq!(
            server.map_state(client),
            2,
            "GTK must observe a viewable X window"
        );
        assert_eq!(server.focus_window, ROOT_WINDOW);
        assert!(server.wm_task_targets().is_empty());
        let hidden = drain_scene(&mut server);
        let frame = server.managed_frames[&client.0];
        for id in [frame.0, client.0] {
            assert!(
                hidden.chunks_exact(SCENE_COMMAND_BYTES).any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == id
                        && command[1] & 1 == 0
                }),
                "parked managed subtree must not be composited"
            );
        }

        let _ = drain_all(&mut server, owner);
        assert!(server.activate_parked_managed_window(client.0));
        assert!(!server.activate_parked_managed_window(client.0));
        assert_eq!(server.parked_managed_window(), None);
        assert_eq!(server.focus_window, client);
        assert_eq!(server.wm_task_targets(), vec![WmTaskTarget::X11(client)]);
        let visible = drain_scene(&mut server);
        for id in [frame.0, client.0] {
            assert!(
                visible.chunks_exact(SCENE_COMMAND_BYTES).any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == id
                        && command[1] & 1 == 1
                }),
                "activation must publish retained surfaces without rebuilding them"
            );
        }
    }

    #[test]
    fn oversized_dialog_fits_workarea_on_map_and_client_resize() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800, height_px: 600, native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0186);
        server.feed(owner, &create_child_window_request(order, client.0,
            ROOT_WINDOW.0, 100, 100, 1600, 1200,
            STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK)).unwrap();
        let dialog_type = server.atoms.intern("_NET_WM_WINDOW_TYPE_DIALOG", false);
        server.set_wm_property(client, "_NET_WM_WINDOW_TYPE", "ATOM", 32,
            dialog_type.0.to_le_bytes().to_vec());
        server.feed(owner, &request(order, 8, 0, &client.0.to_le_bytes())).unwrap();
        let frame = server.managed_frames[&client.0];
        for resize in [false, true] {
            if resize {
                // A restored toolkit preference can request the oversized
                // allocation again after mapping; it must receive a bounded one.
                let mut body = client.0.to_le_bytes().to_vec();
                body.extend_from_slice(&15u16.to_le_bytes());
                body.extend_from_slice(&0u16.to_le_bytes());
                for value in [700u32, 500, 1800, 1400] {
                    body.extend_from_slice(&value.to_le_bytes());
                }
                server.feed(owner, &request(order, 12, 0, &body)).unwrap();
            }
            let (ax, ay, aw, ah) = server.wm_workarea_rect();
            let f = &server.windows[&frame.0];
            assert!(i32::from(f.x) >= i32::from(ax));
            assert!(i32::from(f.y) >= i32::from(ay));
            assert!(i32::from(f.x) + i32::from(f.width) <= i32::from(ax) + i32::from(aw));
            assert!(i32::from(f.y) + i32::from(f.height) <= i32::from(ay) + i32::from(ah));
            let events = drain_all(&mut server, owner);
            assert!(events.chunks_exact(32).any(|event| event[0] & 0x7f == 22),
                "GTK must receive ConfigureNotify for its actual allocation");
        }
    }

    #[test]
    fn oversized_managed_window_keeps_its_close_control_inside_the_workarea() {
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
        let window = ResourceId(0x0020_0185);
        server
            .feed(
                client,
                &create_child_window_request(
                    order,
                    window.0,
                    ROOT_WINDOW.0,
                    0,
                    0,
                    800,
                    400,
                    STRUCTURE_NOTIFY_MASK,
                ),
            )
            .unwrap();
        let delete = server.atoms.intern("WM_DELETE_WINDOW", false);
        server.set_wm_property(
            window,
            "WM_PROTOCOLS",
            "ATOM",
            32,
            delete.0.to_le_bytes().to_vec(),
        );
        server
            .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let frame = server.windows[&server.managed_frames[&window.0].0].clone();
        assert_eq!(frame.width, 800 + WM_BORDER_WIDTH * 2);
        assert_eq!(frame.x, -i16::try_from(WM_BORDER_WIDTH * 2).unwrap());
        let close_left = frame.x.saturating_add(
            i16::try_from(wm_frame_control_left(
                WmChromeProfile::Standard,
                usize::from(frame.width),
                2,
            ))
            .unwrap(),
        );
        let close_right = close_left.saturating_add(i16::try_from(WM_TITLE_BUTTON_WIDTH).unwrap());
        assert!(close_left >= 0);
        assert!(close_right <= 800);
        server.inject_pointer_button(
            true,
            1,
            close_left.saturating_add(i16::try_from(WM_TITLE_BUTTON_WIDTH / 2).unwrap()),
            frame.y.saturating_add(12),
            0,
        );
        let close_event = drain_all(&mut server, client);
        assert!(close_event.chunks_exact(32).any(|event| {
            event[0] == (33 | 0x80)
                && u32::from_le_bytes(event[4..8].try_into().unwrap()) == window.0
                && u32::from_le_bytes(event[12..16].try_into().unwrap()) == delete.0
        }));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn native_titlebar_buttons_maximize_minimize_and_request_close() {
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
        let window = ResourceId(0x0020_0086);
        server
            .feed(
                client,
                &create_window_request(
                    order,
                    window.0,
                    STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK | PROPERTY_CHANGE_MASK,
                ),
            )
            .unwrap();
        let protocols = server.atoms.intern("WM_PROTOCOLS", false);
        let delete = server.atoms.intern("WM_DELETE_WINDOW", false);
        let attention = server.atoms.intern("_NET_WM_STATE_DEMANDS_ATTENTION", false);
        server.set_wm_property(
            window,
            "_NET_WM_STATE",
            "ATOM",
            32,
            attention.0.to_le_bytes().to_vec(),
        );
        server.set_wm_property(
            window,
            "WM_PROTOCOLS",
            "ATOM",
            32,
            delete.0.to_le_bytes().to_vec(),
        );
        server
            .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let frame_id = server.managed_frames[&window.0];
        let frame = server.windows[&frame_id.0].clone();
        let original_geometry = (frame.x, frame.y, frame.width, frame.height);
        server.pointer_grab = Some(ActivePointerGrab {
            owner: client.0,
            window,
            owner_events: true,
            event_mask: u16::try_from(
                BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK | POINTER_MOTION_MASK,
            )
            .unwrap(),
            automatic: false,
        });
        server.inject_pointer_button(
            true,
            1,
            frame
                .x
                .saturating_add(i16::try_from(frame.width).unwrap())
                .saturating_sub(33),
            frame.y.saturating_add(12),
            0,
        );
        assert!(server.maximized_geometries.contains_key(&window.0));
        let net_state = server.atoms.intern("_NET_WM_STATE", false);
        let max_vert = server.atoms.intern("_NET_WM_STATE_MAXIMIZED_VERT", false);
        let max_horz = server.atoms.intern("_NET_WM_STATE_MAXIMIZED_HORZ", false);
        let state = server.windows[&window.0]
            .properties
            .get(&net_state)
            .expect("maximize must publish the state SDL and GTK read");
        let states: Vec<_> = state.data.chunks_exact(4).map(read_u32_le).collect();
        assert!(states.contains(&max_vert.0) && states.contains(&max_horz.0));
        assert!(states.contains(&attention.0));
        assert!(
            server.pointer_grab.is_some(),
            "using compositor chrome must not silently revoke an application grab",
        );
        server.pointer_grab = None;
        let maximized = server.windows[&frame_id.0].clone();
        assert_eq!((maximized.x, maximized.y), (0, 0));
        assert_eq!(maximized.width, 800);
        assert_eq!(maximized.height, 600 - WM_TASKBAR_HEIGHT);
        let client_state = &server.windows[&window.0];
        assert_eq!(
            client_state.width,
            800u16.saturating_sub(WM_BORDER_WIDTH * 2),
        );
        assert_eq!(
            client_state.height,
            600u16
                .saturating_sub(WM_TASKBAR_HEIGHT)
                .saturating_sub(WM_TITLE_HEIGHT)
                .saturating_sub(WM_BORDER_WIDTH),
        );
        let resize_events = drain_all(&mut server, client);
        let state_notify = resize_events
            .chunks_exact(32)
            .position(|event| event[0] == 28 && read_u32_le(&event[8..12]) == net_state.0)
            .expect("maximize must notify subscribers of the state change");
        let configure = resize_events
            .chunks_exact(32)
            .position(|event| event[0] == 22)
            .expect("maximize must configure the client");
        assert!(state_notify < configure, "state must be observable before resize");
        server.publish_managed_maximized_state(window);
        assert!(drain_all(&mut server, client).is_empty(),
            "unchanged state must not flood the toolkit with property events");
        assert!(
            resize_events.chunks_exact(32).any(|event| event[0] == 22),
            "maximize must send a real client resize event for toolkit relayout",
        );
        assert!(
            resize_events
                .chunks_exact(32)
                .any(|event| event[0] == (22 | 0x80)),
            "moving a reparented client must include an ICCCM synthetic configure",
        );
        assert!(
            resize_events
                .chunks_exact(32)
                .any(|event| event[0] & 0x7f == 12),
            "maximize must expose the resized client for relayout and repaint",
        );

        server.inject_pointer_button(
            true,
            6,
            maximized.x.saturating_add(120),
            maximized.y.saturating_add(12),
            0,
        );
        let restored_by_double_click = server.windows[&frame_id.0].clone();
        assert_eq!(
            server.windows[&window.0].properties[&net_state].data,
            attention.0.to_le_bytes(),
            "restore must remove maximize hints only",
        );
        assert_eq!(
            (
                restored_by_double_click.x,
                restored_by_double_click.y,
                restored_by_double_click.width,
                restored_by_double_click.height,
            ),
            original_geometry,
        );
        server.inject_pointer_button(
            true,
            6,
            restored_by_double_click.x.saturating_add(120),
            restored_by_double_click.y.saturating_add(12),
            0,
        );
        let maximized = server.windows[&frame_id.0].clone();
        assert_eq!((maximized.x, maximized.y), (0, 0));

        server.inject_pointer_button(
            true,
            1,
            maximized
                .x
                .saturating_add(i16::try_from(maximized.width).unwrap())
                .saturating_sub(55),
            maximized.y.saturating_add(12),
            0,
        );
        assert!(server.minimized_windows.contains(&window.0));
        assert!(!server.windows[&frame_id.0].mapped);

        server.activate_managed_window(window);
        let restored = server.windows[&frame_id.0].clone();
        server.inject_pointer_button(
            true,
            1,
            restored
                .x
                .saturating_add(i16::try_from(restored.width).unwrap())
                .saturating_sub(11),
            restored.y.saturating_add(12),
            0,
        );
        let output = drain_all(&mut server, client);
        let close = output
            .chunks_exact(32)
            .find(|event| event[0] & 0x7f == 33)
            .expect("WM_DELETE_WINDOW ClientMessage");
        assert_eq!(
            u32::from_le_bytes(close[4..8].try_into().unwrap()),
            window.0
        );
        assert_eq!(
            u32::from_le_bytes(close[8..12].try_into().unwrap()),
            protocols.0
        );
        assert_eq!(
            u32::from_le_bytes(close[12..16].try_into().unwrap()),
            delete.0
        );
    }

    #[test]
    fn fixed_managed_window_suppresses_compositor_maximize() {
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
        let window = ResourceId(0x0020_0186);
        server
            .feed(
                client,
                &create_window_request(order, window.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        assert!(server.set_managed_window_resizable(window.0, false));
        let frame_id = server.managed_frames[&window.0];
        let original = server.windows[&frame_id.0].clone();
        server.inject_pointer_button(
            true,
            1,
            original
                .x
                .saturating_add(i16::try_from(original.width).unwrap())
                .saturating_sub(33),
            original.y.saturating_add(12),
            0,
        );
        let after = &server.windows[&frame_id.0];
        assert_eq!(
            (after.x, after.y, after.width, after.height),
            (original.x, original.y, original.width, original.height),
        );
        assert!(!server.maximized_geometries.contains_key(&window.0));
        assert!(server.set_managed_window_resizable(window.0, true));
        server.inject_pointer_button(
            true,
            1,
            original
                .x
                .saturating_add(i16::try_from(original.width).unwrap())
                .saturating_sub(33),
            original.y.saturating_add(12),
            0,
        );
        assert!(server.maximized_geometries.contains_key(&window.0));
    }

    #[test]
    fn tiled_title_drag_excludes_controls_edges_content_and_occluded_targets() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig {width_px:800,height_px:600,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        server.begin_auto_tile(true);server.apply_auto_tile();
        let frame=server.windows[&server.managed_frames[&client.0].0].clone();
        let (x,y)=(i32::from(frame.x),i32::from(frame.y));
        assert_eq!(server.tiled_x11_title_at(x+200,y+12),client.0);
        assert_eq!(server.tile_at_field(x+200,y+40,1),client.0);
        for (px,py) in [(x+200,y+2),(x+200,y+40),(x+wm_frame_icon_left(WmChromeProfile::Standard) as i32+8,y+12,
            ),(x+wm_frame_control_left(WmChromeProfile::Standard,usize::from(frame.width),2) as i32+8,y+12,
            ),
        ] {
            assert_eq!(server.tiled_x11_title_at(px,py),0,"non-title controls retain ownership");
        }
        let overlay=ResourceId(0x0020_0191);
        server.feed(owner,&create_window_request(order,overlay.0,0)).unwrap();
        server.feed(owner,&request(order,8,0,&overlay.0.to_le_bytes())).unwrap();
        let overlay_frame=server.managed_frames[&overlay.0];
        server.apply_managed_geometry(overlay,overlay_frame,(x+100) as i16,(y+20) as i16,300,200,
        );
        assert_eq!(server.tile_at_field(x+200,y+80,1),0,"an untiled window obscures the tiled destination");
        let modal=server.atoms.intern("_NET_WM_STATE_MODAL",false);
        server.set_wm_property(overlay,"_NET_WM_STATE","ATOM",32,modal.0.to_le_bytes().to_vec(),
        );
        assert!(server.tile_input_blocked());
        server.begin_auto_tile(true);server.apply_auto_tile();
        assert!(!server.tile_layouts[&0].contains(WmTaskTarget::X11(overlay)),"modal dialogs stay outside the tiling tree");
        assert_eq!(server.tiled_x11_title_at(x+200,y+12),0);
        assert!(!server.move_tile_divider(2,x+300));
        server.minimized_windows.insert(client.0);
        assert_eq!(server.tiled_x11_title_at(x+200,y+12),0);
    }

    #[test]
    fn scrolling_mixed_windows_retain_pixels_without_exposure_or_resize_events() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig{width_px:1000,height_px:700,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0190);
        server.feed(owner,&create_window_request(order,client.0,(1<<15)|(1<<17))).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        server.replace_native_tasks((12..15).map(|id|WmNativeTask{id,workspace:0,icon:0,minimized:false,maximized:false,active:false,title:b"Native".to_vec()}).collect());
        server.activate_managed_window(client);
        let apply=|server:&mut DisplayServer| {
            server.begin_auto_tile(true);server.set_auto_tile_scrolling(true);
            for id in 12..15 {server.add_auto_tile_native(id);}
            server.apply_auto_tile();
        };
        apply(&mut server);let frame=server.managed_frames[&client.0];
        let initial=server.windows[&frame.0].clone();drain_all(&mut server,owner);drain_scene(&mut server);
        assert_eq!(server.focus_direction(1),12);
        assert!(server.scroll_tiles(250));apply(&mut server);
        let moved=&server.windows[&frame.0];
        assert_eq!(moved.x,initial.x-250);assert_eq!(moved.width,initial.width);assert_eq!(moved.height,initial.height);
        let events=drain_all(&mut server,owner);
        assert!(!events.is_empty());
        for event in events.chunks_exact(32) {
            assert_ne!(event[0]&127,12,"panning must never expose app contents");
            if event[0]&127==22 {assert_ne!(event[0]&128,0,"only synthetic position notification, never a toolkit resize");}
        }
        let scene=drain_scene(&mut server);
        assert!(!scene.is_empty());assert!(scene.chunks_exact(32).all(|record|record[0]!=0),"panning must not reset the scene");
        server.native_tasks[2].active=true;apply(&mut server);
        assert_eq!(server.focus_direction(0),13);
        server.begin_auto_tile(false);server.apply_auto_tile();
        assert_eq!(server.windows[&client.0].width,320,"floating geometry is restored");
    }
