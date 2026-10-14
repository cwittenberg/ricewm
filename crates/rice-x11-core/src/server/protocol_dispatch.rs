//! Protocol dispatch operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn process_setup(&mut self, handle: ClientHandle) -> Progress {
        let Some(client) = self.clients.get_mut(&handle.0) else {
            return Progress::Stop;
        };
        if client.ingress.len() < SETUP_HEADER_BYTES {
            return Progress::NeedMore;
        }
        let byte_order = match client.ingress[0] {
            b'l' => ClientByteOrder::LittleEndian,
            b'B' => ClientByteOrder::BigEndian,
            _ => {
                Self::fault_client(client);
                return Progress::Stop;
            }
        };
        let auth_name_len = usize::from(read_u16(byte_order, &client.ingress[6..8]));
        let auth_data_len = usize::from(read_u16(byte_order, &client.ingress[8..10]));
        let setup_bytes = SETUP_HEADER_BYTES
            .checked_add(x11::pad4(auth_name_len))
            .and_then(|size| size.checked_add(x11::pad4(auth_data_len)));
        let Some(setup_bytes) = setup_bytes else {
            Self::fault_client(client);
            return Progress::Stop;
        };
        if setup_bytes > MAX_SETUP_BYTES {
            Self::fault_client(client);
            return Progress::Stop;
        }
        if client.ingress.len() < setup_bytes {
            return Progress::NeedMore;
        }
        let setup = {
            let mut cursor = Cursor::new(&client.ingress[..setup_bytes]);
            let Ok(setup) = x11::read_setup_request(&mut cursor) else {
                Self::fault_client(client);
                return Progress::Stop;
            };
            setup
        };
        client.ingress.drain(..setup_bytes);
        if setup.protocol_major != 11 || setup.protocol_minor != 0 {
            return Self::reject_setup(
                client,
                setup.byte_order,
                "RiceWM supports X11 protocol 11.0",
                self.config.max_egress_bytes,
            );
        }
        if !setup.auth_protocol_name.is_empty() || !setup.auth_protocol_data.is_empty() {
            return Self::reject_setup(
                client,
                setup.byte_order,
                "RiceWM authentication is not configured",
                self.config.max_egress_bytes,
            );
        }
        let Ok(reply) = encode_setup_success(
            setup.byte_order,
            client.resource_id_base(),
            self.config.width_px,
            self.config.height_px,
        ) else {
            Self::fault_client(client);
            return Progress::Stop;
        };
        if queue_output(client, &reply, self.config.max_egress_bytes).is_err() {
            Self::fault_client(client);
            return Progress::Stop;
        }
        client.phase = ClientPhase::Ready {
            byte_order: setup.byte_order,
        };
        Progress::Advanced
    }

    pub(super) fn reject_setup(
        client: &mut Client,
        byte_order: ClientByteOrder,
        reason: &str,
        max_egress_bytes: usize,
    ) -> Progress {
        let mut reply = Vec::new();
        if x11::write_setup_failed(&mut reply, byte_order, reason).is_err()
            || queue_output(client, &reply, max_egress_bytes).is_err()
        {
            Self::fault_client(client);
        } else {
            client.phase = ClientPhase::Rejected;
        }
        Progress::Stop
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn process_request(&mut self, handle: ClientHandle, byte_order: ClientByteOrder) -> Progress {
        let (mut frame, length_units, extended_length) = {
            let Some(client) = self.clients.get_mut(&handle.0) else {
                return Progress::Stop;
            };
            let ingress = &client.ingress[client.ingress_offset..];
            if ingress.len() < 4 {
                return Progress::NeedMore;
            }
            let short_length = read_u16(byte_order, &ingress[2..4]);
            let length_units = if short_length == 0 {
                if !client.big_requests_enabled {
                    Self::fault_client(client);
                    return Progress::Stop;
                }
                if ingress.len() < 8 {
                    return Progress::NeedMore;
                }
                read_u32(byte_order, &ingress[4..8])
            } else {
                u32::from(short_length)
            };
            let minimum_units = if short_length == 0 { 2 } else { 1 };
            if length_units < minimum_units {
                Self::fault_client(client);
                return Progress::Stop;
            }
            let Some(frame_bytes) = usize::try_from(length_units)
                .ok()
                .and_then(|units| units.checked_mul(4))
            else {
                Self::fault_client(client);
                return Progress::Stop;
            };
            if frame_bytes > self.config.max_request_bytes
                || frame_bytes > self.config.max_ingress_bytes
            {
                Self::fault_client(client);
                return Progress::Stop;
            }
            if ingress.len() < frame_bytes {
                return Progress::NeedMore;
            }
            let frame = ingress[..frame_bytes].to_vec();
            client.ingress_offset += frame_bytes;
            (
                frame,
                length_units,
                short_length == 0,
            )
        };
        // Framing has already been validated against the configured and
        // negotiated request limit above. Decode directly from that complete
        // frame so BIG-REQUESTS up to the advertised limit are preserved.
        // The protocol helper's stream reader has a smaller internal safety
        // ceiling and deliberately replaces larger bodies with an empty
        // malformed request; applying it after this server-owned framing made
        // valid multi-megabyte PutImage requests fail with BadLength.
        let body_offset = if extended_length { 8 } else { 4 };
        if frame.len() < body_offset {
            if let Some(client) = self.clients.get_mut(&handle.0) {
                Self::fault_client(client);
            }
            return Progress::Stop;
        }
        let header = x11::RequestHeader {
            opcode: frame[0],
            data: frame[1],
            length_units,
        };
        // Borrow the body in place; removing the wire header with Vec::drain
        // would shift every pixel in a large PutImage request.
        let body = &mut frame[body_offset..];
        let count = self.request_counts.entry(header.opcode).or_default();
        *count = count.saturating_add(1);
        x11::request_swap::swap_request_body(header.opcode, header.data, byte_order, body);
        let sequence = {
            let Some(client) = self.clients.get_mut(&handle.0) else {
                return Progress::Stop;
            };
            client.sequence = client.sequence.next();
            client.sequence
        };
        // BIG-REQUESTS inserts one extra 4-byte length word and includes it
        // in the wire length. Core request contracts are specified in their
        // ordinary form, so validate them against the semantic length after
        // removing that transport-only word. This matters for requests such
        // as GTK's large _NET_WM_ICON ChangeProperty payloads.
        let semantic_length_units = header.length_units.checked_sub(u32::from(extended_length));
        let valid_length = if header.opcode < 128 {
            semantic_length_units.is_some_and(|length_units| {
                validate_core_request_length(header.opcode, length_units)
                    && exact_required_length(header.opcode, header.data, body)
                        .is_none_or(|required| required == length_units)
            })
        } else {
            header.length_units >= 1
                && usize::try_from(header.length_units)
                    .ok()
                    .and_then(|units| units.checked_mul(4))
                    .is_some_and(|bytes| bytes <= self.config.max_request_bytes)
        };
        let result = if !valid_length {
            Err(ProtocolError::new(
                x11::error::BAD_LENGTH,
                header.data.into(),
            ))
        } else if let Some(bad) = invalid_value_mask(header.opcode, body) {
            Err(ProtocolError::new(x11::error::BAD_VALUE, bad))
        } else if let Some(bad) = invalid_value(header.opcode, header.data, body) {
            Err(ProtocolError::new(x11::error::BAD_VALUE, bad))
        } else {
            self.dispatch_request(
                handle,
                byte_order,
                sequence,
                header.opcode,
                header.data,
                body,
            )
        };
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                let Ok(output) = encode_error(
                    byte_order,
                    sequence,
                    error.code,
                    error.bad_value,
                    error.minor_opcode,
                    header.opcode,
                ) else {
                    if let Some(client) = self.clients.get_mut(&handle.0) {
                        Self::fault_client(client);
                    }
                    return Progress::Stop;
                };
                output
            }
        };
        if self.queue_to_client(handle, &output).is_err() {
            return Progress::Stop;
        }
        Progress::Advanced
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn dispatch_request(
        &mut self,
        handle: ClientHandle,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        opcode: u8,
        data: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        match opcode {
            1 => self.create_window(handle, sequence, data, body),
            2 => self.change_window_attributes(handle, body),
            3 => self.get_window_attributes(handle, byte_order, sequence, body),
            4 => self.destroy_window(body),
            5 => self.destroy_subwindows(body),
            6 => self.change_save_set(handle, data, body),
            7 => self.reparent_window(sequence, body),
            8 => self.map_window(handle, sequence, body),
            9 => self.map_subwindows(handle, sequence, body),
            10 => self.unmap_window(sequence, body),
            11 => self.unmap_subwindows(sequence, body),
            12 => self.configure_window(handle, sequence, body),
            13 => self.circulate_window(handle, data, body),
            14 => self.get_geometry(byte_order, sequence, body),
            15 => self.query_tree(byte_order, sequence, body),
            16 => self.intern_atom(byte_order, sequence, data, body),
            17 => self.get_atom_name(byte_order, sequence, body),
            18 => self.change_property(sequence, data, body),
            19 => self.delete_property(sequence, body),
            20 => self.get_property(handle, byte_order, sequence, data, body),
            21 => self.list_properties(byte_order, sequence, body),
            22 => self.set_selection_owner(handle, body),
            23 => self.get_selection_owner(byte_order, sequence, body),
            24 => self.convert_selection(body),
            25 => self.send_event(handle, byte_order, data, body),
            26 => self.grab_pointer(handle, byte_order, sequence, data, body),
            27 => Ok(self.ungrab_pointer(handle)),
            31 => self.grab_keyboard(handle, byte_order, sequence, data, body),
            32 => Ok(self.ungrab_keyboard(handle)),
            38 => self.query_pointer(byte_order, sequence, body),
            39 => self.get_motion_events(byte_order, sequence, body),
            40 => self.translate_coordinates(byte_order, sequence, body),
            41 => self.warp_pointer(body),
            42 => self.set_input_focus(body),
            43 => encode_get_input_focus(byte_order, sequence, self.focus_window)
                .map_err(|()| encoding_error()),
            44 => Ok(self.query_keymap(byte_order, sequence)),
            45 => self.open_font(handle, body),
            46 => self.close_font(body),
            47 => self.query_font(byte_order, sequence, body),
            48 => self.query_text_extents(byte_order, sequence, data, body),
            49 => Self::list_fonts(byte_order, sequence, body),
            50 => Self::list_fonts_with_info(byte_order, sequence, body),
            52 => Ok(Self::get_font_path(byte_order, sequence)),
            53 => self.create_pixmap(handle, data, body),
            54 => self.free_pixmap(body),
            55 => self.create_gc(handle, body),
            56 => self.change_gc(body),
            57 => self.copy_gc(body),
            58 => self.set_dashes(body),
            59 => self.set_clip_rectangles(data, body),
            60 => self.free_gc(body),
            61 => self.clear_area(sequence, data, body),
            62 => self.copy_area(handle, body),
            63 => self.copy_plane(handle, body),
            64 => self.poly_point(data, body),
            65 => self.poly_line(data, body),
            66 => self.poly_segment(body),
            67 => self.poly_rectangle(body),
            68 => self.poly_arc(body),
            69 => self.fill_poly(body),
            70 => self.poly_fill_rectangle(body),
            71 => self.poly_fill_arc(body),
            72 => self.put_image(byte_order, data, body),
            73 => self.get_image(byte_order, sequence, data, body),
            74 => self.poly_text(data, body, false),
            75 => self.poly_text(data, body, true),
            76 => self.image_text(data, body, false),
            77 => self.image_text(data, body, true),
            78 => self.create_colormap(handle, body),
            79 => self.free_colormap(body),
            80 => self.copy_colormap(handle, body),
            81 | 82 | 88 => self.validate_colormap_request(body),
            83 => self.list_installed_colormaps(byte_order, sequence, body),
            84 => self.alloc_color(byte_order, sequence, body),
            85 => self.alloc_named_color(byte_order, sequence, body),
            86 | 87 => Err(ProtocolError::new(x11::error::BAD_ALLOC, 0)),
            89 | 90 => Err(ProtocolError::new(x11::error::BAD_ACCESS, 0)),
            91 => self.query_colors(byte_order, sequence, body),
            92 => self.lookup_color(byte_order, sequence, body),
            93 => self.create_cursor(handle, body),
            94 => self.create_glyph_cursor(handle, body),
            95 => self.free_cursor(body),
            96 => self.recolor_cursor(body),
            97 => self.query_best_size(byte_order, sequence, data, body),
            98 => Self::query_extension(byte_order, sequence, body),
            99 => Self::list_extensions(byte_order, sequence),
            100 => self.change_keyboard_mapping(data, body),
            101 => self.get_keyboard_mapping(byte_order, sequence, body),
            103 => Ok(encode_keyboard_control_reply(byte_order, sequence)),
            104 => validate_bell(data),
            106 => Ok(encode_pointer_control_reply(byte_order, sequence)),
            107 => self.set_screensaver(body),
            108 => Ok(self.get_screensaver(byte_order, sequence)),
            110 => Self::list_hosts(byte_order, sequence),
            114 => self.rotate_properties(body),
            116 => self.set_pointer_mapping(byte_order, sequence, data, body),
            117 => Ok(self.get_pointer_mapping(byte_order, sequence)),
            118 => self.set_modifier_mapping(byte_order, sequence, data, body),
            119 => self.get_modifier_mapping(byte_order, sequence),
            BIG_REQUESTS_MAJOR_OPCODE => {
                self.enable_big_requests(handle, byte_order, sequence, data)
            }
            XKB_MAJOR_OPCODE => self.xkb_request(handle, byte_order, sequence, data, body),
            XINPUT_MAJOR_OPCODE => self.xinput_request(handle, byte_order, sequence, data, body),
            GENERIC_EVENT_MAJOR_OPCODE => {
                Self::generic_event_request(byte_order, sequence, data, body)
            }
            RANDR_MAJOR_OPCODE => self.randr_request(handle, byte_order, sequence, data, body),
            RENDER_MAJOR_OPCODE => self.render_request(handle, byte_order, sequence, data, body),
            XFIXES_MAJOR_OPCODE => self.xfixes_request(handle, byte_order, sequence, data, body),
            SHAPE_MAJOR_OPCODE => self.shape_request(handle, byte_order, sequence, data, body),
            SYNC_MAJOR_OPCODE => self.sync_request(handle, byte_order, sequence, data, body),
            DAMAGE_MAJOR_OPCODE => self.damage_request(handle, byte_order, sequence, data, body),
            COMPOSITE_MAJOR_OPCODE => {
                self.composite_request(handle, byte_order, sequence, data, body)
            }
            XTEST_MAJOR_OPCODE => self.xtest_request(byte_order, sequence, data, body),
            XINERAMA_MAJOR_OPCODE => self.xinerama_request(byte_order, sequence, data, body),
            XCMISC_MAJOR_OPCODE => self.xcmisc_request(handle, byte_order, sequence, data, body),
            28 | 29 | 30 | 33 | 34 | 35 | 36 | 37 | 51 | 102 | 105 | 109 | 111 | 112 | 113
            | 115 | 127 => Ok(Vec::new()),
            _ => {
                let count = self.unsupported_requests.entry(opcode).or_default();
                *count = count.saturating_add(1);
                Err(ProtocolError::new(x11::error::BAD_REQUEST, 0))
            }
        }
    }
}
