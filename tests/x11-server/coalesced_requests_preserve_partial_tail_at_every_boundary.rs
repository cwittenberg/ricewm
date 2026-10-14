use super::*;


    #[test]
    fn coalesced_requests_preserve_partial_tail_at_every_boundary() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let mut batch = Vec::new();
            for _ in 0..40 {
                batch.extend_from_slice(&request(order, 127, 0, &[0; 1024]));
            }
            batch.extend_from_slice(&request(order, 43, 0, &[]));
            for tail in 1..=7 {
                let (mut server, client) = ready_server(order);
                let split = batch.len() - tail;
                server.feed(client, &batch[..split]).unwrap();
                assert!(drain_all(&mut server, client).is_empty());
                server.feed(client, &batch[split..]).unwrap();
                let response = drain_all(&mut server, client);
                assert_eq!(response.len(), 32);
                assert_eq!(read_u16(order, &response[2..4]), 41);
                assert_eq!(server.clients[&client.0].ingress.len(), 0);
                server.feed(client, &request(order, 43, 0, &[])).unwrap();
                assert_eq!(read_u16(order, &drain_all(&mut server, client)[2..4]), 42);
            }
        }
    }

    #[test]
    fn invalid_core_length_returns_bad_length() {
        let (mut server, client) = ready_server(ClientByteOrder::LittleEndian);
        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 43, 0, &[0; 4]),
            )
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response[1], x11::error::BAD_LENGTH);
    }

    #[test]
    fn enforces_client_and_ingress_limits() {
        let config = DisplayConfig {
            max_clients: 1,
            max_ingress_bytes: MAX_SETUP_BYTES,
            ..DisplayConfig::default()
        };
        let mut server = DisplayServer::new(config);
        let client = server.open_client().unwrap();
        assert_eq!(server.open_client(), Err(OpenError::ClientLimit));
        let too_large = vec![0; MAX_SETUP_BYTES + 1];
        assert_eq!(
            server.feed(client, &too_large),
            Err(ClientError::IngressLimit)
        );
    }

    #[test]
    fn accepts_advertised_argb32_child_below_depth_24_root() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_01f0;
        let mut body = Vec::new();
        push_u32(order, &mut body, window);
        push_u32(order, &mut body, ROOT_WINDOW.0);
        push_u16(order, &mut body, 0);
        push_u16(order, &mut body, 0);
        push_u16(order, &mut body, 320);
        push_u16(order, &mut body, 200);
        push_u16(order, &mut body, 0);
        push_u16(order, &mut body, 1);
        push_u32(order, &mut body, ARGB_VISUAL.0);
        push_u32(order, &mut body, 0);

        server.feed(client, &request(order, 1, 32, &body)).unwrap();

        assert!(drain_all(&mut server, client).is_empty());
        assert_eq!(server.windows[&window].depth, 32);
        assert_eq!(server.windows[&window].visual, ARGB_VISUAL);
        assert_eq!(server.surfaces[&window].depth, 32);
    }

    #[test]
    fn window_lifecycle_updates_queries_events_and_scene() {
        let (mut server, client) = ready_server(ClientByteOrder::LittleEndian);
        let _ = drain_scene(&mut server);
        let window = 0x0020_0200;
        server
            .feed(
                client,
                &create_window_request(
                    ClientByteOrder::LittleEndian,
                    window,
                    STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK,
                ),
            )
            .unwrap();
        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 8, 0, &window.to_le_bytes()),
            )
            .unwrap();
        let events = drain_all(&mut server, client);
        assert_eq!(events.len(), 64);
        assert_eq!(events[0], 19);
        assert_eq!(events[32], 12);

        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 14, 0, &window.to_le_bytes()),
            )
            .unwrap();
        let geometry = drain_all(&mut server, client);
        assert_eq!(geometry[0], 1);
        assert_eq!(
            u16::from_le_bytes(geometry[16..18].try_into().unwrap()),
            320
        );

        server
            .feed(
                client,
                &request(
                    ClientByteOrder::LittleEndian,
                    15,
                    0,
                    &ROOT_WINDOW.0.to_le_bytes(),
                ),
            )
            .unwrap();
        let tree = drain_all(&mut server, client);
        assert_eq!(u16::from_le_bytes(tree[16..18].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(tree[32..36].try_into().unwrap()), window);

        let scene = drain_scene(&mut server);
        let opcodes: Vec<u8> = scene
            .chunks_exact(SCENE_COMMAND_BYTES)
            .map(|command| command[0])
            .collect();
        assert_eq!(
            opcodes,
            [
                SCENE_UPSERT_WINDOW,
                SCENE_UPSERT_WINDOW,
                SCENE_DAMAGE_WINDOW,
            ]
        );

        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 4, 0, &window.to_le_bytes()),
            )
            .unwrap();
        let destroyed = drain_all(&mut server, client);
        assert_eq!(destroyed[0], 17);
        let scene = drain_scene(&mut server);
        assert_eq!(scene[0], SCENE_REMOVE_WINDOW);
    }

    #[test]
    fn atoms_and_properties_round_trip_with_notifications() {
        let (mut server, client) = ready_server(ClientByteOrder::LittleEndian);
        let window = 0x0020_0300;
        server
            .feed(
                client,
                &create_window_request(ClientByteOrder::LittleEndian, window, PROPERTY_CHANGE_MASK),
            )
            .unwrap();
        server
            .feed(
                client,
                &intern_atom_request(ClientByteOrder::LittleEndian, "_RICE_TEST"),
            )
            .unwrap();
        let atom_reply = drain_all(&mut server, client);
        let property = u32::from_le_bytes(atom_reply[8..12].try_into().unwrap());
        assert!(property >= 69);

        let mut change = Vec::new();
        push_u32(ClientByteOrder::LittleEndian, &mut change, window);
        push_u32(ClientByteOrder::LittleEndian, &mut change, property);
        push_u32(ClientByteOrder::LittleEndian, &mut change, 31);
        change.push(8);
        change.extend_from_slice(&[0; 3]);
        push_u32(ClientByteOrder::LittleEndian, &mut change, 5);
        change.extend_from_slice(b"hello");
        change.extend_from_slice(&[0; 3]);
        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 18, 0, &change),
            )
            .unwrap();
        let notify = drain_all(&mut server, client);
        assert_eq!(notify[0], 28);

        let mut get = Vec::new();
        push_u32(ClientByteOrder::LittleEndian, &mut get, window);
        push_u32(ClientByteOrder::LittleEndian, &mut get, property);
        push_u32(ClientByteOrder::LittleEndian, &mut get, 0);
        push_u32(ClientByteOrder::LittleEndian, &mut get, 0);
        push_u32(ClientByteOrder::LittleEndian, &mut get, 16);
        server
            .feed(client, &request(ClientByteOrder::LittleEndian, 20, 0, &get))
            .unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!(reply[1], 8);
        assert_eq!(&reply[32..37], b"hello");

        server
            .feed(
                client,
                &request(ClientByteOrder::LittleEndian, 21, 0, &window.to_le_bytes()),
            )
            .unwrap();
        let listed = drain_all(&mut server, client);
        assert_eq!(
            u32::from_le_bytes(listed[32..36].try_into().unwrap()),
            property
        );
    }

    #[test]
    fn repeated_wm_root_publication_does_not_notify_unchanged_properties() {
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
        server
            .clients
            .get_mut(&client.0)
            .unwrap()
            .event_masks
            .insert(ROOT_WINDOW.0, PROPERTY_CHANGE_MASK);

        server.publish_wm_root_properties();
        server.publish_wm_root_properties();
        assert!(
            drain_all(&mut server, client).is_empty(),
            "periodic publication must not re-notify unchanged EWMH root properties",
        );
    }

    #[test]
    fn repeated_frame_extents_requests_each_emit_property_notify() {
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

        let window = 0x0020_0301;
        server
            .feed(
                client,
                &create_window_request(order, window, PROPERTY_CHANGE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        assert!(server.managed_frames.contains_key(&window));
        assert_eq!(
            server.take_presented_managed_window(),
            Some(ResourceId(window)),
            "a managed, viewable top-level must publish one presentation boundary",
        );
        assert_eq!(server.take_presented_managed_window(), None);

        let request_extents = server.atoms.intern("_NET_REQUEST_FRAME_EXTENTS", false);
        let frame_extents = server.atoms.intern("_NET_FRAME_EXTENTS", false);
        let mut event = [0; 32];
        event[0] = 33;
        event[1] = 32;
        event[4..8].copy_from_slice(&window.to_le_bytes());
        event[8..12].copy_from_slice(&request_extents.0.to_le_bytes());
        let mut send = Vec::new();
        push_u32(order, &mut send, ROOT_WINDOW.0);
        push_u32(order, &mut send, 0);
        send.extend_from_slice(&event);
        let request_extents = request(order, 25, 0, &send);

        for _ in 0..2 {
            server.feed(client, &request_extents).unwrap();
            let notify = drain_all(&mut server, client);
            assert_eq!(notify.len(), 32);
            assert_eq!(notify[0], 28);
            assert_eq!(u32::from_le_bytes(notify[4..8].try_into().unwrap()), window,);
            assert_eq!(
                u32::from_le_bytes(notify[8..12].try_into().unwrap()),
                frame_extents.0,
            );
        }
    }

    #[test]
    fn motif_no_decorations_removes_compositor_titlebar_but_keeps_resize_hit_target() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client_handle = server.open_client().unwrap();
        server
            .feed(client_handle, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client_handle);

        let window = 0x0020_0303;
        server
            .feed(
                client_handle,
                &create_window_request(order, window, PROPERTY_CHANGE_MASK),
            )
            .unwrap();
        server
            .feed(client_handle, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client_handle);

        let client_id = ResourceId(window);
        let frame_id = server.managed_frames[&window];
        let decorated = server.windows[&frame_id.0].clone();
        assert_eq!(decorated.width, 320 + WM_BORDER_WIDTH * 2);
        assert_eq!(decorated.height, 200 + WM_TITLE_HEIGHT + WM_BORDER_WIDTH);

        let motif_hints = server.atoms.intern("_MOTIF_WM_HINTS", false);
        let mut change = Vec::new();
        push_u32(order, &mut change, window);
        push_u32(order, &mut change, motif_hints.0);
        push_u32(order, &mut change, motif_hints.0);
        change.push(32);
        change.extend_from_slice(&[0; 3]);
        push_u32(order, &mut change, 5);
        // MWM_HINTS_DECORATIONS with decorations=0 is the standard hint used
        // by Electron/Chromium frame:false windows.
        for value in [2, 0, 0, 0, 0] {
            push_u32(order, &mut change, value);
        }
        server
            .feed(client_handle, &request(order, 18, 0, &change))
            .unwrap();
        let _ = drain_all(&mut server, client_handle);

        let undecorated = server.windows[&frame_id.0].clone();
        let client = &server.windows[&window];
        assert_eq!((undecorated.width, undecorated.height), (320, 200));
        assert_eq!((client.x, client.y), (0, 0));
        assert_eq!(server.managed_frame_insets(client_id), (0, 0, 0, 0));
        let frame_extents = server.atoms.intern("_NET_FRAME_EXTENTS", true);
        assert_eq!(
            server.windows[&window].properties[&frame_extents].data,
            vec![0; 16],
        );
        let resize_x = undecorated
            .x
            .saturating_add(i16::try_from(undecorated.width).unwrap())
            .saturating_sub(5);
        let resize_y = undecorated
            .y
            .saturating_add(i16::try_from(undecorated.height / 2).unwrap());
        assert_eq!(server.hit_test(resize_x, resize_y), client_id);
        assert_eq!(server.pointer_layer(resize_x, resize_y), 3);
        assert!(
            server
                .managed_resize_frame_at(client_id, resize_x, resize_y)
                .is_some_and(|(frame, edges)| frame == frame_id && edges == WM_RESIZE_RIGHT),
        );
        assert_eq!(server.wm_chrome_hit(frame_id, resize_x, undecorated.y), None);

        // Client titlebar plus WM border and resize handles (Motif 2 | 4).
        change[28..32].copy_from_slice(&6u32.to_le_bytes());
        server.feed(client_handle, &request(order, 18, 0, &change)).unwrap();
        let _ = drain_all(&mut server, client_handle);
        assert_eq!(server.managed_frame_insets(client_id), (1, 1, 1, 1));
        assert_eq!((server.windows[&window].x, server.windows[&window].y), (1, 1));
        assert_eq!((server.windows[&frame_id.0].width, server.windows[&frame_id.0].height), (322, 202));
        assert_eq!(server.wm_chrome_hit(frame_id, resize_x, undecorated.y), None);
        let pixels = server.surfaces[&frame_id.0].pixels.as_ref().unwrap();
        assert_eq!(pixels[0], server.wm_themes[usize::from(server.wm_theme)].frame[2] | 0xff00_0000);

        change[28..32].copy_from_slice(&1u32.to_le_bytes());
        server
            .feed(client_handle, &request(order, 18, 0, &change))
            .unwrap();
        let _ = drain_all(&mut server, client_handle);
        let restored = &server.windows[&frame_id.0];
        assert_eq!(restored.width, 320 + WM_BORDER_WIDTH * 2);
        assert_eq!(restored.height, 200 + WM_TITLE_HEIGHT + WM_BORDER_WIDTH);
        assert_eq!(server.windows[&window].y, WM_TITLE_HEIGHT as i16);
    }

    #[test]
    fn mapped_managed_frame_repaints_when_application_icon_changes() {
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
        let _ = drain_scene(&mut server);

        let window = 0x0020_0302;
        server
            .feed(
                client,
                &create_window_request(order, window, PROPERTY_CHANGE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let _ = drain_scene(&mut server);

        let frame = server.managed_frames[&window];
        let fallback_pixel = server.surfaces[&frame.0].pixel(5, 4);
        let net_wm_icon = server.atoms.intern("_NET_WM_ICON", false);
        let cardinal = server.atoms.intern("CARDINAL", false);
        let mut icon_values = Vec::with_capacity((2 + 16 * 16) * 4);
        push_u32(order, &mut icon_values, 16);
        push_u32(order, &mut icon_values, 16);
        for _ in 0..16 * 16 {
            push_u32(order, &mut icon_values, 0xff12_3456);
        }
        let mut change = Vec::with_capacity(20 + icon_values.len());
        push_u32(order, &mut change, window);
        push_u32(order, &mut change, net_wm_icon.0);
        push_u32(order, &mut change, cardinal.0);
        change.push(32);
        change.extend_from_slice(&[0; 3]);
        push_u32(order, &mut change, 2 + 16 * 16);
        change.extend_from_slice(&icon_values);
        server
            .feed(client, &request(order, 18, 0, &change))
            .unwrap();
        let _ = drain_all(&mut server, client);

        assert_ne!(fallback_pixel & 0x00ff_ffff, 0x0012_3456);
        assert_eq!(
            server.surfaces[&frame.0].pixel(5, 4) & 0x00ff_ffff,
            0x0012_3456,
            "a post-map _NET_WM_ICON update must repaint the existing titlebar",
        );
        assert!(
            drain_scene(&mut server)
                .chunks_exact(SCENE_COMMAND_BYTES)
                .any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == frame.0
                }),
            "the repainted frame must be published in the retained scene"
        );

        let mut delete = Vec::with_capacity(8);
        push_u32(order, &mut delete, window);
        push_u32(order, &mut delete, net_wm_icon.0);
        server
            .feed(client, &request(order, 19, 0, &delete))
            .unwrap();
        let _ = drain_all(&mut server, client);
        assert_ne!(
            server.surfaces[&frame.0].pixel(5, 4) & 0x00ff_ffff,
            0x0012_3456,
            "removing _NET_WM_ICON must repaint the generic fallback",
        );
    }

    #[test]
    fn scene_stream_begins_with_reset_and_root() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let scene = drain_scene(&mut server);
        assert_eq!(scene.len(), SCENE_COMMAND_BYTES * 2);
        assert_eq!(scene[0], SCENE_RESET);
        assert_eq!(scene[SCENE_COMMAND_BYTES], SCENE_UPSERT_WINDOW);
    }

    #[test]
    fn browser_input_is_hit_tested_propagated_and_encoded_as_core_events() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0001;
        let input_mask = KEY_PRESS_MASK
            | KEY_RELEASE_MASK
            | BUTTON_PRESS_MASK
            | BUTTON_RELEASE_MASK
            | POINTER_MOTION_MASK
            | FOCUS_CHANGE_MASK;

        server
            .feed(client, &create_window_request(order, window, input_mask))
            .unwrap();
        let mut map_body = Vec::new();
        push_u32(order, &mut map_body, window);
        server
            .feed(client, &request(order, 8, 0, &map_body))
            .unwrap();
        assert!(drain_all(&mut server, client).is_empty());

        server.inject_pointer_motion(20, 30, 0);
        let motion = drain_all(&mut server, client);
        assert_eq!(motion.len(), 32);
        assert_eq!(motion[0], 6);
        assert_eq!(
            u32::from_le_bytes(motion[12..16].try_into().unwrap()),
            window
        );
        assert_eq!(u32::from_le_bytes(motion[16..20].try_into().unwrap()), 0);
        assert_eq!(i16::from_le_bytes(motion[24..26].try_into().unwrap()), 8);
        assert_eq!(i16::from_le_bytes(motion[26..28].try_into().unwrap()), 12);

        server.inject_pointer_button(true, 1, 20, 30, 0);
        let press = drain_all(&mut server, client);
        assert_eq!(press.len(), 64);
        assert_eq!(press[0], 9);
        assert_eq!(press[32], 4);
        assert_eq!(press[33], 1);
        assert_eq!(
            u32::from_le_bytes(press[44..48].try_into().unwrap()),
            window
        );

        server.inject_key(true, 38, 0x0001);
        let key = drain_all(&mut server, client);
        assert_eq!(key.len(), 32);
        assert_eq!(key[0], 2);
        assert_eq!(key[1], 38);
        assert_eq!(u32::from_le_bytes(key[12..16].try_into().unwrap()), window);
        assert_eq!(u16::from_le_bytes(key[28..30].try_into().unwrap()), 1);
    }
