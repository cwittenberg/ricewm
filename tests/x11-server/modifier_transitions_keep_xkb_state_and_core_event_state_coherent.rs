use super::*;


    #[test]
    fn modifier_transitions_keep_xkb_state_and_core_event_state_coherent() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0a11;

        server
            .feed(
                client,
                &create_window_request(order, window, KEY_PRESS_MASK | KEY_RELEASE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        server.inject_pointer_motion(20, 30, 0);
        let _ = drain_all(&mut server, client);
        server.inject_pointer_button(true, 1, 20, 30, 0);
        let _ = drain_all(&mut server, client);
        server.inject_pointer_button(false, 1, 20, 30, 0x0100);
        let _ = drain_all(&mut server, client);

        server.inject_key(true, 50, 0);
        let shift_press = drain_all(&mut server, client);
        assert_eq!((shift_press[0], shift_press[1]), (2, 50));
        assert_eq!(read_u16(order, &shift_press[28..30]), 0);

        server.inject_key(true, 55, 0x0001);
        let upper_v_press = drain_all(&mut server, client);
        assert_eq!((upper_v_press[0], upper_v_press[1]), (2, 55));
        assert_eq!(read_u16(order, &upper_v_press[28..30]), 0x0001);

        server.inject_key(false, 55, 0x0001);
        let upper_v_release = drain_all(&mut server, client);
        assert_eq!((upper_v_release[0], upper_v_release[1]), (3, 55));
        assert_eq!(read_u16(order, &upper_v_release[28..30]), 0x0001,);

        server.inject_key(false, 50, 0x0001);
        let shift_release = drain_all(&mut server, client);
        assert_eq!((shift_release[0], shift_release[1]), (3, 50));
        assert_eq!(read_u16(order, &shift_release[28..30]), 0x0001);

        server
            .feed(client, &request(order, XKB_MAJOR_OPCODE, 4, &[0, 1, 0, 0]))
            .unwrap();
        let state = drain_all(&mut server, client);
        assert_eq!((state[8], state[9]), (0, 0));
    }

    #[test]
    fn xinput2_warp_pointer_supports_sdl_recentering_and_source_bounds() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let warp = |source: u32, destination: u32, x: i32, y: i32, device: u16| {
                let mut body = Vec::new();
                for value in [source, destination, 0, 0] {
                    push_u32(order, &mut body, value);
                }
                push_u16(order, &mut body, 40);
                push_u16(order, &mut body, 40);
                push_u32(order, &mut body, (x << 16) as u32);
                push_u32(order, &mut body, (y << 16) as u32);
                push_u16(order, &mut body, device);
                push_u16(order, &mut body, 0);
                request(order, XINPUT_MAJOR_OPCODE, 41, &body)
            };
            server.feed(client, &warp(0, ROOT_WINDOW.0, 120, 90, 2)).unwrap();
            let response = drain_all(&mut server, client);
            assert!(response.is_empty(), "{order:?}: {response:?}");
            assert_eq!((server.pointer_x, server.pointer_y), (120, 90));
            server.feed(client, &warp(ROOT_WINDOW.0, 0, 20, 20, 2)).unwrap();
            assert!(drain_all(&mut server, client).is_empty());
            assert_eq!((server.pointer_x, server.pointer_y), (120, 90));
            server.feed(client, &warp(0, 0, -10, 5, 4)).unwrap();
            assert!(drain_all(&mut server, client).is_empty());
            assert_eq!((server.pointer_x, server.pointer_y), (110, 95));
            server.feed(client, &warp(0, ROOT_WINDOW.0, 1, 1, 3)).unwrap();
            let error = drain_all(&mut server, client);
            assert_eq!(error[1], XINPUT_FIRST_ERROR);
            assert_eq!((server.pointer_x, server.pointer_y), (110, 95));
            server.feed(client, &request(order, XINPUT_MAJOR_OPCODE, 41, &[])).unwrap();
            assert_eq!(drain_all(&mut server, client)[1], x11::error::BAD_LENGTH);
        }
    }

    #[test]
    fn xinput2_key_delivery_suppresses_duplicate_core_event() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0094;
        server.feed(client, &create_window_request(order, window, KEY_PRESS_MASK | KEY_RELEASE_MASK),
            ).unwrap();
        server.feed(client, &request(order, 8, 0, &window.to_le_bytes())).unwrap();
        server.set_focus(ResourceId(window));
        drain_all(&mut server, client);
        server.clients.get_mut(&client.0).unwrap().xi2_masks.insert((window, 3), (1 << 2) | (1 << 3));
        for pressed in [true, false] {
            server.inject_key(pressed, 38, 0);
            let bytes = drain_all(&mut server, client);
            assert_eq!(bytes[0], 35);
            let length = 32 + u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize * 4;
            assert_eq!(bytes.len(), length, "one XI2 event, no trailing core event");
        }
        server.clients.get_mut(&client.0).unwrap().xi2_masks.clear();
        server.inject_key(true, 38, 0);
        let bytes = drain_all(&mut server, client);
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], 2, "core-only applications retain keyboard delivery");
    }

    #[test]
    fn xinput2_select_events_delivers_generic_pointer_motion() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let mut select = Vec::new();
        push_u32(order, &mut select, ROOT_WINDOW.0);
        push_u16(order, &mut select, 1);
        push_u16(order, &mut select, 0);
        push_u16(order, &mut select, 1);
        push_u16(order, &mut select, 1);
        push_u32(order, &mut select, 1 << 6);
        server
            .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 46, &select))
            .unwrap();
        assert_eq!(server.pending_output(client).unwrap(), 0);

        server.inject_pointer_motion(120, 90, 0);
        let response = drain_all(&mut server, client);
        assert_eq!(response[0], 35);
        assert_eq!(response[1], XINPUT_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes(response[8..10].try_into().unwrap()), 6);
        assert_eq!(u16::from_le_bytes(response[10..12].try_into().unwrap()), 2);
    }

    #[test]
    fn xinput2_button_selection_does_not_suppress_core_wheel_events() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        server.clients.get_mut(&client.0).unwrap().event_masks.insert(
            ROOT_WINDOW.0,
            BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK);
        server
            .clients
            .get_mut(&client.0)
            .unwrap()
            .xi2_masks
            .insert(
                (ROOT_WINDOW.0, 2),
                (1 << 4) | (1 << 5));

        server.inject_pointer_button(true, 5, 120, 90, 0);
        let press = drain_all(&mut server, client);
        assert_eq!(press.len(), 32);
        assert_eq!(press[0], 4, "button 5 press uses core X11 wheel delivery");
        assert_eq!(press[1], 5);

        server.inject_pointer_button(false, 5, 120, 90, 0x1000);
        let release = drain_all(&mut server, client);
        assert_eq!(release.len(), 32);
        assert_eq!(release[0], 5);
        assert_eq!(release[1], 5);
    }

    #[test]
    fn xinput2_raw_motion_preserves_physical_deltas_across_cursor_warps() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let mut select = Vec::new();
            push_u32(order, &mut select, ROOT_WINDOW.0);
            push_u16(order, &mut select, 2);
            push_u16(order, &mut select, 0);
            // Overlapping master selectors must not multiply movement.
            for device in [1, 2] {
                push_u16(order, &mut select, device);
                push_u16(order, &mut select, 1);
                push_u32(order, &mut select, 1 << 17);
            }
            server.feed(client, &request(order, XINPUT_MAJOR_OPCODE, 46, &select)).unwrap();
            assert!(!server.relative_pointer_capture_requested(), "raw subscription alone cannot capture the browser");
            server.pointer_grab = Some(ActivePointerGrab {
                owner: client.0, window: ROOT_WINDOW, owner_events: true,
                event_mask: u16::try_from(POINTER_MOTION_MASK).unwrap(), automatic: true,
            });
            assert!(!server.relative_pointer_capture_requested(), "implicit button grabs cannot capture the browser");
            server.pointer_grab.as_mut().unwrap().automatic = false;
            assert!(server.relative_pointer_capture_requested(), "explicit relative grab requests capture");
            server.pointer_grab = None;
            server.inject_pointer_motion(100, 100, 0);
            let baseline = drain_all(&mut server, client);
            assert!(baseline.is_empty(), "first absolute sample establishes a baseline: {order:?} {baseline:?}");
            server.inject_pointer_motion(107, 96, 0);
            let event = drain_all(&mut server, client);
            assert_eq!(event.len(), 72);
            assert_eq!(event[0], 35);
            assert_eq!(read_u16(order, &event[8..10]), 17);
            assert_eq!(read_u16(order, &event[10..12]), 2);
            assert_eq!(read_u16(order, &event[20..22]), 4);
            assert_eq!(read_u32(order, &event[32..36]), 3);
            for offset in [40, 56] { assert_eq!(read_u32(order, &event[offset..offset + 4]) as i32, 7); }
            for offset in [48, 64] { assert_eq!(read_u32(order, &event[offset..offset + 4]) as i32, -4); }

            // Both core and XI2 warps must move the logical pointer silently
            // for raw listeners, without corrupting the next physical delta.
            let mut core_warp = vec![0; 20];
            core_warp[4..8].copy_from_slice(&ROOT_WINDOW.0.to_le_bytes());
            core_warp[16..18].copy_from_slice(&300i16.to_le_bytes());
            core_warp[18..20].copy_from_slice(&200i16.to_le_bytes());
            server.warp_pointer(&core_warp).unwrap();
            let mut xi_warp = vec![0; 32];
            xi_warp[4..8].copy_from_slice(&ROOT_WINDOW.0.to_le_bytes());
            xi_warp[20..24].copy_from_slice(&(150i32 << 16).to_le_bytes());
            xi_warp[24..28].copy_from_slice(&(120i32 << 16).to_le_bytes());
            xi_warp[28..30].copy_from_slice(&2u16.to_le_bytes());
            server.xinput_warp_pointer(&xi_warp).unwrap();
            assert!(drain_all(&mut server, client).is_empty());
            server.inject_pointer_motion(110, 95, 0);
            let event = drain_all(&mut server, client);
            assert_eq!(event.len(), 72);
            assert_eq!(read_u32(order, &event[56..60]) as i32, 3);
            assert_eq!(read_u32(order, &event[64..68]) as i32, -1);

            // Pointer lock can move without changing absolute coordinates.
            server.inject_pointer_motion_with_delta(110, 95, -500, 270, 0);
            let event = drain_all(&mut server, client);
            assert_eq!(event.len(), 72);
            assert_eq!(read_u32(order, &event[56..60]) as i32, -500);
            assert_eq!(read_u32(order, &event[64..68]) as i32, 270);
        }
    }

    #[test]
    fn xinput2_raw_motion_requires_root_selection_and_can_select_slave() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        server.inject_pointer_motion(10, 10, 0);
        server.inject_pointer_motion(15, 15, 0);
        assert!(drain_all(&mut server, client).is_empty());
        let window = 0x0020_0091;
        server.feed(client, &create_window_request(order, window, 0)).unwrap();
        let mut select = Vec::new();
        push_u32(order, &mut select, window);
        push_u16(order, &mut select, 1);
        push_u16(order, &mut select, 0);
        push_u16(order, &mut select, 4);
        push_u16(order, &mut select, 1);
        push_u32(order, &mut select, 1 << 17);
        server.feed(client, &request(order, XINPUT_MAJOR_OPCODE, 46, &select)).unwrap();
        assert!(drain_all(&mut server, client).is_empty());
        server.inject_pointer_motion(16, 17, 0);
        assert!(drain_all(&mut server, client).is_empty(), "child selection must not receive raw root events");
        server.clients.get_mut(&client.0).unwrap().xi2_masks.insert((ROOT_WINDOW.0, 4), 1 << 17);
        server.inject_pointer_motion(16, 18, 0);
        let event = drain_all(&mut server, client);
        assert_eq!(event.len(), 72);
        assert_eq!(read_u16(order, &event[10..12]), 4);
        server.clients.get_mut(&client.0).unwrap().xi2_masks.clear();
        server.inject_pointer_motion(19, 23, 0);
        assert!(drain_all(&mut server, client).is_empty());
    }

    #[test]
    fn xinput2_grab_device_updates_shared_core_grabs() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0091;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        assert!(drain_all(&mut server, client).is_empty());

        let grab_request = |device: u16, mask: u32| {
            let mut body = Vec::new();
            push_u32(order, &mut body, window);
            push_u32(order, &mut body, 0);
            push_u32(order, &mut body, 0);
            push_u16(order, &mut body, device);
            body.extend_from_slice(&[1, 1, 1, 0]);
            push_u16(order, &mut body, 1);
            push_u32(order, &mut body, mask);
            request(order, XINPUT_MAJOR_OPCODE, 51, &body)
        };

        server
            .feed(client, &grab_request(2, (1 << 4) | (1 << 5) | (1 << 6)))
            .unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!((reply[0], reply[1], reply[8]), (1, 51, 0));
        assert!(server.pointer_grab.is_some_and(|grab| {
            grab.owner == client.0
                && grab.window == ResourceId(window)
                && grab.owner_events
                && !grab.automatic
                && u32::from(grab.event_mask)
                    == BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK | POINTER_MOTION_MASK
        }));

        server
            .feed(client, &grab_request(3, (1 << 2) | (1 << 3)))
            .unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!((reply[0], reply[1], reply[8]), (1, 51, 0));
        assert!(server.keyboard_grab.is_some_and(|grab| {
            grab.owner == client.0 && grab.window == ResourceId(window) && grab.owner_events
        }));

        let contender = server.open_client().unwrap();
        let setup = setup_request(order, &[], &[]);
        server.feed(contender, &setup).unwrap();
        let _ = drain_all(&mut server, contender);
        server
            .feed(contender, &grab_request(2, (1 << 4) | (1 << 5) | (1 << 6)))
            .unwrap();
        let reply = drain_all(&mut server, contender);
        assert_eq!((reply[0], reply[1], reply[8]), (1, 51, 1));
        assert_eq!(server.pointer_grab.map(|grab| grab.owner), Some(client.0));

        let ungrab_request = |device: u16| {
            let mut body = Vec::new();
            push_u32(order, &mut body, 0);
            push_u16(order, &mut body, device);
            push_u16(order, &mut body, 0);
            request(order, XINPUT_MAJOR_OPCODE, 52, &body)
        };
        server.feed(client, &ungrab_request(2)).unwrap();
        assert!(drain_all(&mut server, client).is_empty());
        assert!(server.pointer_grab.is_none());
        assert!(server.keyboard_grab.is_some());
        assert!(server.keyboard_grabbed());
        server.feed(client, &ungrab_request(3)).unwrap();
        assert!(drain_all(&mut server, client).is_empty());
        assert!(server.keyboard_grab.is_none());
        assert!(!server.keyboard_grabbed());
    }

    #[test]
    fn xinput2_passive_ungrab_validates_and_accepts_both_byte_orders() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let mut body = Vec::new();
            push_u32(order, &mut body, ROOT_WINDOW.0);
            push_u32(order, &mut body, 0); // Any button/key detail.
            push_u16(order, &mut body, 1); // XIAllMasterDevices.
            push_u16(order, &mut body, 2);
            body.extend_from_slice(&[0, 0, 0, 0]); // Button grab type and padding.
            push_u32(order, &mut body, 0);
            push_u32(order, &mut body, 1 << 31); // AnyModifier.
            assert_eq!(body.len(), 24);
            assert_eq!(read_u16(order, &body[10..12]), 2);
            server
                .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 55, &body))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert!(response.is_empty(), "unexpected XI2 response: {response:?}");

            // The declared modifier list is part of the wire length contract.
            body.truncate(20);
            server
                .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 55, &body))
                .unwrap();
            let error = drain_all(&mut server, client);
            assert_eq!((error[0], error[1]), (0, x11::error::BAD_LENGTH));
        }
    }

    #[test]
    fn xinput2_passive_grab_accepts_sdl_relative_mouse_request() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let mut body = Vec::new();
            push_u32(order, &mut body, 0); // CurrentTime.
            push_u32(order, &mut body, ROOT_WINDOW.0);
            push_u32(order, &mut body, 0); // None cursor.
            push_u32(order, &mut body, 0); // Any button detail.
            push_u16(order, &mut body, 2); // Master pointer.
            push_u16(order, &mut body, 1); // One modifier.
            push_u16(order, &mut body, 1); // One event-mask word.
            body.extend_from_slice(&[0, 2, 1, 1]);
            push_u16(order, &mut body, 0);
            push_u32(order, &mut body, (1 << 4) | (1 << 5) | (1 << 6));
            push_u32(order, &mut body, 1 << 31); // AnyModifier.
            assert_eq!(body.len(), 36);
            server
                .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 54, &body))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!((response[0], response[1]), (1, 54));
            assert_eq!(read_u16(order, &response[8..10]), 0);

            body.truncate(32);
            server
                .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 54, &body))
                .unwrap();
            let error = drain_all(&mut server, client);
            assert_eq!((error[0], error[1]), (0, x11::error::BAD_LENGTH));
        }
    }

    #[test]
    fn xinput2_delivers_crossing_and_focus_for_toolkit_seats() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0090;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        assert!(drain_all(&mut server, client).is_empty());

        let mut select = Vec::new();
        push_u32(order, &mut select, window);
        push_u16(order, &mut select, 1);
        push_u16(order, &mut select, 0);
        push_u16(order, &mut select, 1);
        push_u16(order, &mut select, 1);
        push_u32(order, &mut select, (1 << 7) | (1 << 9));
        server
            .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 46, &select))
            .unwrap();

        server.inject_pointer_motion(20, 30, 0);
        let crossing = drain_all(&mut server, client);
        assert_eq!(crossing.len(), 76);
        assert_eq!(crossing[0], 35);
        assert_eq!(crossing[1], XINPUT_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes(crossing[8..10].try_into().unwrap()), 7);

        server.set_focus(ResourceId(window));
        let focus = drain_all(&mut server, client);
        assert_eq!(focus.len(), 76);
        assert_eq!(focus[0], 35);
        assert_eq!(focus[1], XINPUT_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes(focus[8..10].try_into().unwrap()), 9);
    }

    #[test]
    fn xinput2_query_device_reports_master_and_slave_pairs() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let mut body = Vec::new();
        push_u16(order, &mut body, 0);
        push_u16(order, &mut body, 0);
        server
            .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 48, &body))
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response[0], 1);
        assert_eq!(u16::from_le_bytes(response[8..10].try_into().unwrap()), 4);
        assert!(response.len() > 2_000);
        let first_name_len = usize::from(u16::from_le_bytes(response[40..42].try_into().unwrap()));
        let button_class = 44 + x11::pad4(first_name_len);
        assert_eq!(
            u16::from_le_bytes(response[button_class..button_class + 2].try_into().unwrap(),),
            1,
        );
        assert_eq!(
            u16::from_le_bytes(
                response[button_class + 2..button_class + 4]
                    .try_into()
                    .unwrap(),
            ),
            10,
        );
        assert_eq!(
            u32::from_le_bytes(
                response[button_class + 8..button_class + 12]
                    .try_into()
                    .unwrap(),
            ),
            0,
        );
        let first_valuator = button_class + 40;
        assert_eq!(
            u16::from_le_bytes(
                response[first_valuator..first_valuator + 2]
                    .try_into()
                    .unwrap(),
            ),
            2,
        );
        let class_type =
            |offset: usize| u16::from_le_bytes(response[offset..offset + 2].try_into().unwrap());
        let class_bytes = |offset: usize| {
            usize::from(u16::from_le_bytes(
                response[offset + 2..offset + 4].try_into().unwrap(),
            )) * 4
        };
        let mut offset = button_class;
        let mut types = Vec::new();
        for _ in 0..7 {
            types.push(class_type(offset));
            offset += class_bytes(offset);
        }
        assert_eq!(types, vec![1, 2, 2, 2, 2, 3, 3]);
    }

    #[test]
    fn xinput2_get_missing_device_property_returns_empty_reply() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let property = server
            .atoms
            .intern("libinput Scroll Methods Available", false)
            .0;
        let mut body = Vec::new();
        push_u16(order, &mut body, 2);
        body.extend_from_slice(&[0, 0]);
        push_u32(order, &mut body, property);
        push_u32(order, &mut body, 0);
        push_u32(order, &mut body, 0);
        push_u32(order, &mut body, 1024);
        server
            .feed(client, &request(order, XINPUT_MAJOR_OPCODE, 59, &body))
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response.len(), 32);
        assert_eq!((response[0], response[1]), (1, 59));
        assert_eq!(read_u32(order, &response[4..8]), 0);
        assert_eq!(read_u32(order, &response[8..12]), 0);
        assert_eq!(read_u32(order, &response[12..16]), 0);
        assert_eq!(read_u32(order, &response[16..20]), 0);
        assert_eq!(response[20], 0);
    }

    #[test]
    fn preserves_sequences_across_coalesced_requests() {
        let (mut server, client) = ready_server(ClientByteOrder::LittleEndian);
        let mut requests = request(ClientByteOrder::LittleEndian, 99, 0, &[]);
        requests.extend_from_slice(&request(ClientByteOrder::LittleEndian, 127, 0, &[]));
        requests.extend_from_slice(&request(ClientByteOrder::LittleEndian, 43, 0, &[]));
        server.feed(client, &requests).unwrap();
        let response = drain_all(&mut server, client);
        let extensions_bytes = 32
            + usize::try_from(u32::from_le_bytes(response[4..8].try_into().unwrap())).unwrap() * 4;
        assert_eq!(response.len(), extensions_bytes + 32);
        assert_eq!(
            u16::from_le_bytes(
                response[extensions_bytes + 2..extensions_bytes + 4]
                    .try_into()
                    .unwrap()
            ),
            3
        );
    }
