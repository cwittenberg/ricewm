use super::*;

    #[test]
    fn pointer_wire_timestamps_follow_elapsed_milliseconds_and_wrap() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let client = server.open_client().unwrap();
        server.feed(client, &setup_request(ClientByteOrder::LittleEndian, &[], &[])).unwrap();
        let _ = drain_all(&mut server, client);
        server.clients.get_mut(&client.0).unwrap().event_masks.insert(
            ROOT_WINDOW.0, BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK);
        for (press_time, release_time, elapsed) in [(1000u32, 2200u32, 1200u32),
                                                  (u32::MAX - 5, 3, 9)] {
            server.set_timestamp_millis(press_time);
            server.inject_pointer_button(true, 1, 2, 2, 0);
            server.set_timestamp_millis(release_time);
            server.inject_pointer_button(false, 1, 2, 2, 0x0100);
            let bytes = drain_all(&mut server, client);
            let times: Vec<u32> = bytes.chunks_exact(32)
                .filter(|event| matches!(event[0] & 0x7f, 4 | 5))
                .map(|event| u32::from_le_bytes(event[4..8].try_into().unwrap()))
                .collect();
            assert_eq!(times, [press_time, release_time]);
            assert_eq!(times[1].wrapping_sub(times[0]), elapsed);
        }
    }

    #[test]
    fn browser_owned_shortcut_icon_preserves_the_control_background() {
        let mut pixels = [0x0012_3456; 32 * 32];
        draw_app_icon(&mut pixels, 32, 32, 3, 7, 255, 0x00ff_ffff);
        assert!(pixels.iter().all(|pixel| *pixel == 0x0012_3456));
        draw_app_icon(&mut pixels, 32, 32, 3, 7, 2, 0x00ff_ffff);
        assert!(pixels.iter().any(|pixel| *pixel != 0x0012_3456));
    }

    #[test]
    fn text_overlays_replace_unsupported_regional_flags_without_mojibake() {
        assert_eq!(
            text_overlay_bytes("Location: 🇳🇱 Netherlands".as_bytes(), 255),
            b"Location: [NL] Netherlands"
        );
        assert_eq!(text_overlay_bytes("éé".as_bytes(), 3), "é".as_bytes());
    }

    #[test]
    fn scanline_polygon_rasterizer_matches_point_sample_reference() {
        let bounds = ClippedRectangle {
            x: 2,
            y: 3,
            width: 19,
            height: 15,
        };
        let cases = [
            vec![vec![(2.0, 3.0), (18.0, 5.25), (7.5, 17.0)]],
            vec![
                vec![(3.125, 4.0), (12.75, 4.0), (12.75, 13.875), (3.125, 13.875)],
                vec![(8.5, 7.25), (20.0, 7.25), (20.0, 16.0), (8.5, 16.0)],
            ],
            vec![vec![
                (2.5, 5.5),
                (19.5, 5.5),
                (6.0, 16.5),
                (11.0, 3.5),
                (16.0, 16.5),
            ]],
        ];
        for polygons in cases {
            let mut reference = vec![0u8; usize::from(bounds.width) * usize::from(bounds.height)];
            for row in 0..bounds.height {
                for column in 0..bounds.width {
                    let mut covered = 0u16;
                    for sample_y in [0.125, 0.375, 0.625, 0.875] {
                        for sample_x in [0.125, 0.375, 0.625, 0.875] {
                            let point = (
                                f64::from(bounds.x + column) + sample_x,
                                f64::from(bounds.y + row) + sample_y,
                            );
                            if polygons
                                .iter()
                                .any(|polygon| point_in_polygon(point, polygon))
                            {
                                covered += 1;
                            }
                        }
                    }
                    reference[usize::from(row) * usize::from(bounds.width) + usize::from(column)] =
                        u8::try_from(covered * 255 / 16).unwrap_or(255);
                }
            }
            assert_eq!(rasterize_polygons(&polygons, bounds), reference);
        }
    }

    #[test]
    fn application_cursor_tracks_names_inheritance_and_resource_lifetime() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_01ff;
        server.feed(client, &create_window_request(order, window, 0)).unwrap();
        let cursor_id = 0x0020_0200;
        server.cursors.insert(cursor_id, RiceCursor::new(client.0, 0));
        let mut attributes = Vec::new();
        push_u32(order, &mut attributes, window);
        push_u32(order, &mut attributes, 1 << 14);
        push_u32(order, &mut attributes, cursor_id);
        server.feed(client, &request(order, 2, 0, &attributes)).unwrap();
        server.feed(client, &request(order, 8, 0, &window.to_le_bytes())).unwrap();
        let (x, y) = server.window_origin(ResourceId(window));
        server.inject_pointer_motion((x + 20) as i16, (y + 20) as i16, 0);
        assert_eq!(server.pointer_cursor(), 0);
        for (name, shape) in [(b"hand2".as_slice(), 1), (b"xterm".as_slice(), 2), (b"left_ptr".as_slice(), 0)] {
            let mut body = Vec::new();
            push_u32(order, &mut body, cursor_id);
            push_u16(order, &mut body, name.len() as u16);
            push_u16(order, &mut body, 0);
            body.extend_from_slice(name);
            while body.len() % 4 != 0 { body.push(0); }
            server.xfixes_request(client, order, SequenceNumber(0), x11xfixes::SET_CURSOR_NAME, &body).unwrap();
            assert_eq!(server.pointer_cursor(), shape);
        }
        server.cursors[&cursor_id].set_shape(1);
        server.free_cursor(&cursor_id.to_le_bytes()).unwrap();
        assert_eq!(server.pointer_cursor(), 1, "FreeCursor must preserve a window's reference");
        server.cursors.insert(cursor_id, RiceCursor::new(client.0, 2));
        assert_eq!(server.pointer_cursor(), 1, "reusing an XID must not retarget old references");
        server.window_cursors.insert(ROOT_WINDOW.0, RiceCursor::new(client.0, 3));
        attributes[8..12].copy_from_slice(&0u32.to_le_bytes());
        server.feed(client, &request(order, 2, 0, &attributes)).unwrap();
        assert_eq!(server.pointer_cursor(), 3, "None inherits the ancestor cursor");
        attributes[8..12].copy_from_slice(&0x12345678u32.to_le_bytes());
        assert_eq!(server.change_window_attributes(client, &attributes).unwrap_err().code, x11::error::BAD_CURSOR);
        assert_eq!(server.pointer_cursor(), 3);
    }

    #[test]
    fn application_cursor_accepts_create_window_attribute_and_glyph_shape() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_01ff;
        let cursor_id = 0x0020_0200;
        // A core cursor-font request is used when an Xcursor theme is absent.
        let font = 0x0020_0201;
        let mut open_font = Vec::new();
        push_u32(order, &mut open_font, font);
        push_u16(order, &mut open_font, 6);
        push_u16(order, &mut open_font, 0);
        open_font.extend_from_slice(b"cursor\0\0");
        server.feed(client, &request(order, 45, 0, &open_font)).unwrap();
        let mut glyph = Vec::new();
        for value in [cursor_id, font, font] { push_u32(order, &mut glyph, value); }
        push_u16(order, &mut glyph, 60);
        push_u16(order, &mut glyph, 61);
        glyph.extend_from_slice(&[0; 12]);
        server.feed(client, &request(order, 94, 0, &glyph)).unwrap();
        assert_eq!(server.cursors[&cursor_id].shape(), 1);
        let mut create = create_window_request(order, window, 0);
        let mask = u32::from_le_bytes(create[28..32].try_into().unwrap()) | (1 << 14);
        create[28..32].copy_from_slice(&mask.to_le_bytes());
        create.extend_from_slice(&cursor_id.to_le_bytes());
        let length = (create.len() / 4) as u16;
        create[2..4].copy_from_slice(&length.to_le_bytes());
        server.feed(client, &create).unwrap();
        assert_eq!(server.window_cursors[&window].shape(), 1);
    }

    #[test]
    fn get_window_attributes_reports_core_map_state_on_the_wire() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let window = 0x0020_01ff;
            server
                .feed(client, &create_window_request(order, window, 0))
                .unwrap();

            let mut body = Vec::new();
            push_u32(order, &mut body, window);
            server
                .feed(client, &request(order, 3, 0, &body))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!(response.len(), 44);
            assert_eq!(response[0], 1);
            assert_eq!(response[26], 0, "a newly created window must be IsUnmapped");

            server
                .feed(client, &request(order, 8, 0, &body))
                .unwrap();
            let _ = drain_all(&mut server, client);
            server
                .feed(client, &request(order, 3, 0, &body))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!(response.len(), 44);
            assert_eq!(response[26], 2, "a mapped root child must be IsViewable");
        }
    }

    #[test]
    fn mapping_a_parent_makes_already_mapped_descendants_viewable_in_scene() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let parent = 0x0020_0200;
        let child = 0x0020_0201;
        server
            .feed(client, &create_window_request(order, parent, 0))
            .unwrap();
        server
            .feed(
                client,
                &create_child_window_request(order, child, parent, 8, 12, 80, 24, 0),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &child.to_le_bytes()))
            .unwrap();
        let before_parent = drain_scene(&mut server);
        assert_eq!(server.map_state(ResourceId(child)), 1);
        assert!(
            before_parent
                .chunks_exact(SCENE_COMMAND_BYTES)
                .any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == child
                        && command[1] & 1 == 0
                })
        );

        server
            .feed(client, &request(order, 8, 0, &parent.to_le_bytes()))
            .unwrap();
        assert_eq!(server.map_state(ResourceId(child)), 2);
        let after_parent = drain_scene(&mut server);
        assert!(
            after_parent
                .chunks_exact(SCENE_COMMAND_BYTES)
                .any(|command| {
                    command[0] == SCENE_UPSERT_WINDOW
                        && u32::from_le_bytes(command[4..8].try_into().unwrap()) == child
                        && command[1] & 1 == 1
                }),
            "mapping a parent must publish its mapped child as viewable"
        );
    }

    #[test]
    fn unmapping_not_useful_window_discards_pixels_and_remap_exposes_it() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0202;
        server
            .feed(
                client,
                &create_window_request(order, window, EXPOSURE_MASK))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let surface = server.surfaces.get_mut(&window).unwrap();
        surface.pixels = Some(vec![0x00aa_1144; surface.pixel_count().unwrap()]);

        server
            .feed(client, &request(order, 10, 0, &window.to_le_bytes()))
            .unwrap();
        assert!(
            server.surfaces[&window].pixels.is_none(),
            "BackingStore NotUseful must not preserve stale unmapped pixels",
        );
        let _ = drain_all(&mut server, client);

        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let events = drain_all(&mut server, client);
        assert!(events.chunks_exact(32).any(|event| {
            event[0] & 0x7f == 12
                && u32::from_le_bytes(event[4..8].try_into().unwrap()) == window
                && u16::from_le_bytes(event[12..14].try_into().unwrap()) == 320
                && u16::from_le_bytes(event[14..16].try_into().unwrap()) == 200
        }));
        assert_eq!(server.surfaces[&window].pixel(10, 10), 0x0022_3344);
    }

    #[test]
    fn accepts_a_setup_request_fragmented_at_every_byte() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let client = server.open_client().unwrap();
        let setup = setup_request(ClientByteOrder::LittleEndian, &[], &[]);
        for byte in setup {
            assert_eq!(server.feed(client, &[byte]).unwrap(), 1);
        }
        let response = drain_all(&mut server, client);
        assert_eq!(response[0], 1);
        assert_eq!(
            u32::from_le_bytes(response[12..16].try_into().unwrap()),
            0x0020_0000
        );
    }

    #[test]
    fn emits_a_big_endian_setup_reply() {
        let (mut server, client) = ready_server(ClientByteOrder::BigEndian);
        server
            .feed(client, &request(ClientByteOrder::BigEndian, 43, 0, &[]))
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response.len(), 32);
        assert_eq!(
            u32::from_be_bytes(response[8..12].try_into().unwrap()),
            ROOT_WINDOW.0
        );
    }

    #[test]
    fn rejects_auth_until_the_session_cookie_is_configured() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let client = server.open_client().unwrap();
        let setup = setup_request(
            ClientByteOrder::LittleEndian,
            b"MIT-MAGIC-COOKIE-1",
            &[7; 16],
        );
        server.feed(client, &setup).unwrap();
        assert_eq!(server.status(client).unwrap(), ClientStatus::Rejected);
    }

    #[test]
    fn handles_fragmented_query_extension_and_advertises_implemented_randr() {
        let (mut server, client) = ready_server(ClientByteOrder::LittleEndian);
        let mut body = Vec::new();
        push_u16(ClientByteOrder::LittleEndian, &mut body, 5);
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(b"RANDR");
        body.resize(x11::pad4(body.len()), 0);
        let query = request(ClientByteOrder::LittleEndian, 98, 0, &body);
        server.feed(client, &query[..7]).unwrap();
        assert_eq!(server.pending_output(client).unwrap(), 0);
        server.feed(client, &query[7..]).unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response[8], 1);
        assert_eq!(response[9], RANDR_MAJOR_OPCODE);
        assert_eq!(response[10], RANDR_FIRST_EVENT);
        assert_eq!(response[11], RANDR_FIRST_ERROR);
    }

    #[test]
    fn randr_list_output_properties_accepts_the_protocol_four_byte_request() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            let mut body = Vec::new();
            push_u32(order, &mut body, RANDR_OUTPUT);
            server
                .feed(
                    client,
                    &request(
                        order,
                        RANDR_MAJOR_OPCODE,
                        x11randr::RR_LIST_OUTPUT_PROPERTIES,
                        &body,
                    ),
                )
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!(response.len(), 32);
            assert_eq!(response[0], 1);
            assert_eq!(read_u16(order, &response[8..10]), 0);
        }
    }

    #[test]
    fn advertises_generic_events_and_xinput2_with_negotiated_versions() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        for (name, opcode, first_event, first_error) in [
            ("Generic Event Extension", GENERIC_EVENT_MAJOR_OPCODE, 0, 0),
            (
                "XInputExtension",
                XINPUT_MAJOR_OPCODE,
                XINPUT_FIRST_EVENT,
                XINPUT_FIRST_ERROR,
            ),
        ] {
            server
                .feed(client, &query_extension_request(order, name))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!(
                (response[8], response[9], response[10], response[11]),
                (1, opcode, first_event, first_error),
            );
        }

        let mut ge_version = Vec::new();
        push_u16(order, &mut ge_version, 1);
        push_u16(order, &mut ge_version, 0);
        server
            .feed(
                client,
                &request(order, GENERIC_EVENT_MAJOR_OPCODE, 0, &ge_version),
            )
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(&response[8..12], &[1, 0, 0, 0]);

        let mut xi_version = Vec::new();
        push_u16(order, &mut xi_version, 2);
        push_u16(order, &mut xi_version, 9);
        server
            .feed(
                client,
                &request(order, XINPUT_MAJOR_OPCODE, 47, &xi_version),
            )
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(u16::from_le_bytes(response[8..10].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(response[10..12].try_into().unwrap()), 4);
    }

    #[test]
    fn xkeyboard_map_and_names_are_structurally_coherent_for_both_byte_orders() {
        for order in [ClientByteOrder::LittleEndian, ClientByteOrder::BigEndian] {
            let (mut server, client) = ready_server(order);
            server
                .feed(client, &query_extension_request(order, "XKEYBOARD"))
                .unwrap();
            let extension = drain_all(&mut server, client);
            assert_eq!(
                (extension[8], extension[9], extension[10], extension[11]),
                (1, XKB_MAJOR_OPCODE, XKB_FIRST_EVENT, XKB_FIRST_ERROR)
            );

            let mut version = Vec::new();
            push_u16(order, &mut version, 1);
            push_u16(order, &mut version, 0);
            server
                .feed(client, &request(order, XKB_MAJOR_OPCODE, 0, &version))
                .unwrap();
            let response = drain_all(&mut server, client);
            assert_eq!(response.len(), 32);
            assert_eq!(response[1], 1);
            assert_eq!(read_u16(order, &response[8..10]), 1);
            assert_eq!(read_u16(order, &response[10..12]), 0);

            let mut get_map = vec![0; 28];
            write_u16_at(order, &mut get_map[0..2], 0x0100);
            write_u16_at(order, &mut get_map[2..4], 0x00df);
            server
                .feed(client, &request(order, XKB_MAJOR_OPCODE, 8, &get_map))
                .unwrap();
            let map = drain_all(&mut server, client);
            assert_eq!(
                usize::try_from(read_u32(order, &map[4..8])).unwrap() * 4 + 32,
                map.len()
            );
            assert_eq!((map[10], map[11], map[15], map[16]), (8, 255, 4, 4));
            assert_eq!(read_u16(order, &map[12..14]), 0x00df);

            let mut offset = 40usize;
            assert_eq!(map[offset + 4], 1);
            offset += 8;
            for _ in 0..3 {
                assert_eq!((map[offset + 4], map[offset + 5]), (2, 1));
                offset += 16;
            }
            for _ in 0..248 {
                let n_syms = usize::from(read_u16(order, &map[offset + 6..offset + 8]));
                offset += 8 + n_syms * 4;
            }
            let action_count: usize = map[offset..offset + 248]
                .iter()
                .map(|&count| usize::from(count))
                .sum();
            assert_eq!(action_count, usize::from(read_u16(order, &map[22..24])));
            offset += 248 + action_count * 8;
            offset += 4;
            offset += x11::pad4(usize::from(map[33]) * 2);
            offset += usize::from(map[36]) * 4;
            assert_eq!(offset, map.len());

            let mut get_names = vec![0; 8];
            write_u16_at(order, &mut get_names[0..2], 0x0100);
            write_u32_at(order, &mut get_names[4..8], 0x0000_3fff);
            server
                .feed(client, &request(order, XKB_MAJOR_OPCODE, 17, &get_names))
                .unwrap();
            let names = drain_all(&mut server, client);
            assert_eq!(
                usize::try_from(read_u32(order, &names[4..8])).unwrap() * 4 + 32,
                names.len()
            );
            assert_eq!((names[12], names[13], names[14]), (8, 255, 4));
            assert_eq!(read_u16(order, &names[16..18]), 0x000f);
            assert_eq!((names[18], names[19]), (8, 248));
            assert_eq!(read_u16(order, &names[26..28]), 7);
            for atom_offset in (32..64).step_by(4) {
                assert_ne!(read_u32(order, &names[atom_offset..atom_offset + 4]), 0);
            }
        }
    }

    #[test]
    fn xkeyboard_controls_state_and_core_modifier_map_agree() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);

        server.feed(client, &request(order, 119, 0, &[])).unwrap();
        let core_map = drain_all(&mut server, client);
        assert_eq!(core_map[1], 2);
        assert_eq!(
            &core_map[32..48],
            &[
                50, 62, 66, 0, 37, 105, 64, 108, 77, 0, 78, 0, 133, 134, 0, 0
            ]
        );

        server.inject_key(true, 38, 0x0005);
        server
            .feed(client, &request(order, XKB_MAJOR_OPCODE, 4, &[0, 1, 0, 0]))
            .unwrap();
        let state = drain_all(&mut server, client);
        assert_eq!((state[8], state[9]), (0x05, 0x05));

        server
            .feed(client, &request(order, XKB_MAJOR_OPCODE, 6, &[0, 1, 0, 0]))
            .unwrap();
        let controls = drain_all(&mut server, client);
        assert_eq!(controls.len(), 92);
        assert_eq!(controls[9], 1);
        assert_eq!(read_u16(order, &controls[20..22]), 500);
        assert_eq!(read_u16(order, &controls[22..24]), 33);
        assert_eq!(read_u32(order, &controls[56..60]), 1);
    }
