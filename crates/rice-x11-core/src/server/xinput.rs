//! Xinput operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn xinput_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            // XI 1.x GetExtensionVersion. libXi uses this before
            // negotiating XI2, so the legacy and XI2 entry points must
            // describe the same server.
            1 => {
                if body.len() < 4 {
                    Err(bad_length())
                } else {
                    let mut output = fixed_reply_header(order, sequence, 0, 0);
                    let mut word = [0; 2];
                    write_u16_at(order, &mut word, 2);
                    output.extend_from_slice(&word);
                    write_u16_at(order, &mut word, 4);
                    output.extend_from_slice(&word);
                    output.push(1);
                    output.resize(32, 0);
                    Ok(output)
                }
            }
            2 => {
                if body.is_empty() {
                    let mouse = self.atoms.intern("MOUSE", false).0;
                    let keyboard = self.atoms.intern("KEYBOARD", false).0;
                    Ok(x11::encode_list_input_devices_reply(
                        order,
                        sequence,
                        [
                            "RiceWM Virtual Core Pointer",
                            "RiceWM Virtual Core Keyboard",
                            "RiceWM Browser Pointer",
                            "RiceWM Browser Keyboard",
                        ],
                        [mouse, keyboard, mouse, keyboard],
                    ))
                } else {
                    Err(bad_length())
                }
            }
            40 => self.xinput_query_pointer(order, sequence, body),
            41 => self.xinput_warp_pointer(body),
            // XIChangeCursor: RiceWM currently has one effective pointer
            // cursor, matching the core DefineCursor model. Validate both
            // resources even though the browser renderer owns the sprite.
            42 => {
                if body.len() == 12 {
                    let window = ResourceId(read_u32(order, body));
                    self.require_window(window)?;
                    let cursor = read_u32(order, &body[4..8]);
                    let cursor = self.resolve_cursor(cursor)?;
                    self.assign_window_cursor(window, cursor);
                    Ok(Vec::new())
                } else {
                    Err(bad_length())
                }
            }
            44 => {
                if body.len() == 8 {
                    let window = read_u32(order, body);
                    if window != 0 {
                        self.require_window(ResourceId(window))?;
                    }
                    let device = read_u16(order, &body[4..6]);
                    validate_xinput_device(device, true)?;
                    Ok(Vec::new())
                } else {
                    Err(bad_length())
                }
            }
            45 => {
                if body.len() == 4 {
                    let window = read_u32(order, body);
                    if window != 0 {
                        self.require_window(ResourceId(window))?;
                    }
                    let mut output = fixed_reply_header(order, sequence, 0, 0);
                    output.extend_from_slice(&[1, 0]);
                    let mut word = [0; 2];
                    write_u16_at(order, &mut word, 2);
                    output.extend_from_slice(&word);
                    output.resize(32, 0);
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            46 => self.xinput_select_events(handle, order, body),
            47 => {
                if body.len() == 4 {
                    let requested_major = read_u16(order, body);
                    let requested_minor = read_u16(order, &body[2..4]);
                    if requested_major < 2 {
                        Err(ProtocolError::new(
                            x11::error::BAD_VALUE,
                            u32::from(requested_major),
                        ))
                    } else {
                        let negotiated = (requested_major, requested_minor).min((2, 4));
                        let mut output = fixed_reply_header(order, sequence, 0, 0);
                        let mut word = [0; 2];
                        write_u16_at(order, &mut word, negotiated.0);
                        output.extend_from_slice(&word);
                        write_u16_at(order, &mut word, negotiated.1);
                        output.extend_from_slice(&word);
                        output.resize(32, 0);
                        Ok(output)
                    }
                } else {
                    Err(bad_length())
                }
            }
            48 => self.xinput_query_device(order, sequence, body),
            51 => self.xinput_grab_device(handle, order, sequence, body),
            // XIUngrabDevice. GTK uses the XI2 grab pair for popup menus and
            // treats an asynchronous protocol error while dismissing one as
            // fatal. XI2 device grabs share RiceWM's effective core grab
            // slots, so release only a slot owned by this client.
            52 => {
                if body.len() != 8 {
                    Err(bad_length())
                } else {
                    let device = read_u16(order, &body[4..6]);
                    validate_xinput_device(device, false)?;
                    match device {
                        2 | 4 => {
                            if self.pointer_grab.is_some_and(|grab| grab.owner == handle.0) {
                                self.pointer_grab = None;
                            }
                        }
                        3 | 5 => {
                            if self
                                .keyboard_grab
                                .is_some_and(|grab| grab.owner == handle.0)
                            {
                                self.keyboard_grab = None;
                            }
                        }
                        _ => unreachable!("validated XI2 device ID"),
                    }
                    Ok(Vec::new())
                }
            }
            // XIPassiveGrabDevice. SDL installs a passive XI2 grab when it
            // enables relative mouse input. RiceWM routes the same physical
            // pointer through its core grab slot and does not need a second
            // retained passive-grab table, but advertising XI2 requires a
            // successful, protocol-shaped reply here. Returning BadRequest
            // makes libX11 terminate otherwise healthy fullscreen games.
            54 => {
                if body.len() < 28 {
                    Err(bad_length())
                } else {
                    // request_swap normalizes the variable-length XI2 request
                    // body to little endian before extension dispatch.
                    let window = ResourceId(read_u32(
                        ClientByteOrder::LittleEndian,
                        &body[4..8]));
                    self.require_window(window)?;
                    let cursor = read_u32(
                        ClientByteOrder::LittleEndian,
                        &body[8..12]);
                    if cursor != 0 && !self.cursors.contains_key(&cursor) {
                        return Err(extension_error(
                            ProtocolError::new(x11::error::BAD_CURSOR, cursor),
                            minor,
                        ));
                    }
                    validate_xinput_device(
                        read_u16(
                            ClientByteOrder::LittleEndian,
                            &body[16..18]),
                        true,
                    )?;
                    let modifier_count = usize::from(read_u16(
                        ClientByteOrder::LittleEndian,
                        &body[18..20]));
                    let mask_words = usize::from(read_u16(
                        ClientByteOrder::LittleEndian,
                        &body[20..22]));
                    let expected_len = 28usize
                        .checked_add(mask_words.saturating_mul(4))
                        .and_then(|length| length.checked_add(modifier_count.saturating_mul(4))
                        )
                        .ok_or_else(bad_length)?;
                    if body.len() != expected_len {
                        Err(bad_length())
                    } else {
                        let grab_type = body[22];
                        let grab_mode = body[23];
                        let paired_device_mode = body[24];
                        let owner_events = body[25];
                        if grab_type > 4 {
                            Err(ProtocolError::new(
                                x11::error::BAD_VALUE,
                                u32::from(grab_type),
                            ))
                        // XI 2.2 adds Touch (2) to the primary grab mode;
                        // paired_device_mode remains the older Sync/Async
                        // enum. SDL uses Touch while installing its passive
                        // relative-pointer grab.
                        } else if grab_mode > 2 || paired_device_mode > 1 {
                            Err(ProtocolError::new(
                                x11::error::BAD_VALUE,
                                u32::from(grab_mode.max(paired_device_mode)),
                            ))
                        } else if owner_events > 1 {
                            Err(ProtocolError::new(
                                x11::error::BAD_VALUE,
                                u32::from(owner_events),
                            ))
                        } else {
                            // num_modifiers=0 means every requested modifier
                            // was accepted. The remaining reply is padding.
                            let mut output =
                                fixed_reply_header(order, sequence, 54, 0);
                            output.resize(32, 0);
                            Ok(output)
                        }
                    }
                }
            }
            // XIPassiveUngrabDevice. SDL and other XI2 clients clear stale
            // passive grabs while constructing their input seat even when
            // they have not installed one in this connection. RiceWM does
            // not currently retain passive XI2 grabs, so the successful
            // operation is a validated no-op, as the protocol requires when
            // no matching grab exists.
            55 => {
                if body.len() < 16 {
                    Err(bad_length())
                } else {
                    // request_swap normalizes every declared multi-byte XI2
                    // field to the server's little-endian representation.
                    let modifier_count = usize::from(read_u16(
                        ClientByteOrder::LittleEndian,
                        &body[10..12]));
                    let expected_len = 16usize
                        .checked_add(modifier_count.saturating_mul(4))
                        .ok_or_else(bad_length)?;
                    if body.len() != expected_len {
                        Err(bad_length())
                    } else {
                        self.require_window(ResourceId(read_u32(
                            ClientByteOrder::LittleEndian,
                            body,
                        )))?;
                        // XIAllDevices and XIAllMasterDevices are valid
                        // selectors for passive grabs and ungrabs.
                        validate_xinput_device(
                            read_u16(ClientByteOrder::LittleEndian, &body[8..10]),
                            true,
                        )?;
                        let grab_type = body[12];
                        if grab_type > 4 {
                            Err(ProtocolError::new(
                                x11::error::BAD_VALUE,
                                u32::from(grab_type),
                            ))
                        } else {
                            Ok(Vec::new())
                        }
                    }
                }
            }
            // XIListProperties. The virtual devices currently expose no
            // libinput-specific properties; the empty list is a complete,
            // valid reply and keeps toolkit device discovery deterministic.
            56 => {
                if body.len() == 4 {
                    validate_xinput_device(read_u16(order, body), false)?;
                    let mut output = fixed_reply_header(order, sequence, 0, 0);
                    output.resize(32, 0);
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            // XIGetProperty. RiceWM's virtual devices deliberately expose no
            // device-specific properties (matching XIListProperties above),
            // but GTK still probes well-known libinput atoms. Return the
            // protocol-defined None/empty reply instead of BadRequest so
            // toolkit input discovery completes without poisoning GDK's
            // asynchronous error stream.
            59 => {
                if body.len() == 20 {
                    validate_xinput_device(read_u16(order, body), false)?;
                    let delete = body[2];
                    if delete > 1 {
                        return Err(extension_error(
                            ProtocolError::new(x11::error::BAD_VALUE, u32::from(delete)),
                            minor,
                        ));
                    }
                    let property = read_u32(order, &body[4..8]);
                    if property == 0 {
                        return Err(extension_error(
                            ProtocolError::new(x11::error::BAD_ATOM, property),
                            minor,
                        ));
                    }
                    let mut output = fixed_reply_header(order, sequence, 59, 0);
                    // type=None, bytes_after=0, num_items=0, format=0.
                    output.resize(32, 0);
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            60 => self.xinput_get_selected_events(handle, order, sequence, body),
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|error| extension_error(error, minor))
    }

    pub(super) fn xinput_grab_device(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 20 {
            return Err(bad_length());
        }
        let mask_words = usize::from(read_u16(order, &body[18..20]));
        let mask_bytes = mask_words.checked_mul(4).ok_or_else(bad_length)?;
        if body.len() != 20usize.checked_add(mask_bytes).ok_or_else(bad_length)? {
            return Err(bad_length());
        }

        let window = ResourceId(read_u32(order, body));
        self.require_window(window)?;
        let cursor = read_u32(order, &body[8..12]);
        if cursor != 0 && !self.cursors.contains_key(&cursor) {
            return Err(ProtocolError::new(x11::error::BAD_CURSOR, cursor));
        }
        let device = read_u16(order, &body[12..14]);
        validate_xinput_device(device, false)?;
        let grab_mode = body[14];
        let paired_device_mode = body[15];
        let owner_events = body[16];
        if grab_mode > 1 || paired_device_mode > 1 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(grab_mode.max(paired_device_mode)),
            ));
        }
        if owner_events > 1 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(owner_events),
            ));
        }

        let xi_mask = if mask_words == 0 {
            0
        } else {
            read_u32(order, &body[20..24])
        };
        let mut core_pointer_mask = 0u32;
        for (xi_event, core_mask) in [
            (4, BUTTON_PRESS_MASK),
            (5, BUTTON_RELEASE_MASK),
            (6, POINTER_MOTION_MASK),
            (7, ENTER_WINDOW_MASK),
            (8, LEAVE_WINDOW_MASK),
        ] {
            if xi_mask & (1 << xi_event) != 0 {
                core_pointer_mask |= core_mask;
            }
        }

        // RiceWM exposes one effective pointer/keyboard pair. The master and
        // browser-slave IDs therefore share the same real grab slots used by
        // core GrabPointer/GrabKeyboard and by input event fanout.
        let status = match device {
            _ if self.native_task_owns_input() => 1,
            2 | 4 if self.pointer_grab.is_some_and(|grab| grab.owner != handle.0) => 1,
            3 | 5
                if self
                    .keyboard_grab
                    .is_some_and(|grab| grab.owner != handle.0) =>
            {
                1
            }
            _ if self.map_state(window) != 2 => 3,
            2 | 4 => {
                self.pointer_grab = Some(ActivePointerGrab {
                    owner: handle.0,
                    window,
                    owner_events: owner_events != 0,
                    event_mask: u16::try_from(core_pointer_mask).unwrap_or(u16::MAX),
                    automatic: false,
                });
                0
            }
            3 | 5 => {
                self.keyboard_grab = Some(ActiveKeyboardGrab {
                    owner: handle.0,
                    window,
                    owner_events: owner_events != 0,
                });
                0
            }
            _ => unreachable!("validated XI2 device ID"),
        };
        let mut output = fixed_reply_header(order, sequence, 51, 0);
        output.push(status);
        output.resize(32, 0);
        Ok(output)
    }

    pub(super) fn xinput_select_events(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 8 {
            return Err(bad_length());
        }
        // request_swap normalizes the fixed header, but leaves XISelectEvents'
        // variable-length mask records opaque in the client's byte order.
        let window = ResourceId(read_u32(ClientByteOrder::LittleEndian, body));
        self.require_window(window)?;
        let count = usize::from(read_u16(ClientByteOrder::LittleEndian, &body[4..6]));
        let mut offset = 8usize;
        let mut selections = Vec::with_capacity(count);
        for _ in 0..count {
            let header = body.get(offset..offset + 4).ok_or_else(bad_length)?;
            let device = read_u16(order, header);
            validate_xinput_device(device, true)?;
            let words = usize::from(read_u16(order, &header[2..4]));
            let bytes = words.checked_mul(4).ok_or_else(bad_length)?;
            offset = offset.checked_add(4).ok_or_else(bad_length)?;
            let mask_bytes = body.get(offset..offset + bytes).ok_or_else(bad_length)?;
            let mask = if words == 0 {
                0
            } else {
                read_u32(order, &mask_bytes[..4])
            };
            selections.push((device, mask));
            offset = offset.checked_add(bytes).ok_or_else(bad_length)?;
        }
        if offset != body.len() {
            return Err(bad_length());
        }
        let client = self
            .clients
            .get_mut(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        for (device, mask) in selections {
            if mask == 0 {
                client.xi2_masks.remove(&(window.0, device));
            } else {
                client.xi2_masks.insert((window.0, device), mask);
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn xinput_get_selected_events(
        &self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 4 {
            return Err(bad_length());
        }
        let window = ResourceId(read_u32(order, body));
        self.require_window(window)?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        let masks: Vec<(u16, u32)> = client
            .xi2_masks
            .iter()
            .filter_map(|(&(selected_window, device), &mask)| {
                (selected_window == window.0).then_some((device, mask))
            })
            .collect();
        let mut output = fixed_reply_header(
            order,
            sequence,
            0,
            u32::try_from(masks.len().saturating_mul(2)).unwrap_or(u32::MAX),
        );
        let mut word = [0; 2];
        write_u16_at(
            order,
            &mut word,
            u16::try_from(masks.len()).unwrap_or(u16::MAX),
        );
        output.extend_from_slice(&word);
        output.resize(32, 0);
        for (device, mask) in masks {
            write_u16_at(order, &mut word, device);
            output.extend_from_slice(&word);
            write_u16_at(order, &mut word, 1);
            output.extend_from_slice(&word);
            write_u32_vec(order, &mut output, mask);
        }
        Ok(output)
    }

    pub(super) fn xinput_warp_pointer(
        &mut self,
        body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 32 {
            return Err(bad_length());
        }
        // request_swap normalizes XIWarpPointer's complete wire body.
        let order = ClientByteOrder::LittleEndian;
        let source = ResourceId(read_u32(order, body));
        let destination = ResourceId(read_u32(order, &body[4..8]));
        if source.0 != 0 {
            self.require_window(source)?;
        }
        if destination.0 != 0 {
            self.require_window(destination)?;
        }
        let device = read_u16(order, &body[28..30]);
        // Only the master pointer and its attached browser pointer can warp.
        if device != 2 && device != 4 {
            return Err(ProtocolError::new(XINPUT_FIRST_ERROR, u32::from(device)));
        }
        let coordinate = |offset| i64::from(read_u32(order, &body[offset..offset + 4]) as i32);
        if source.0 != 0 {
            let window = &self.windows[&source.0];
            let origin = self.window_origin(source);
            let x = i64::from(i32::from(self.pointer_x) - origin.0) << 16;
            let y = i64::from(i32::from(self.pointer_y) - origin.1) << 16;
            let left = coordinate(8);
            let top = coordinate(12);
            let width = read_u16(order, &body[16..18]);
            let height = read_u16(order, &body[18..20]);
            let right = if width == 0 { i64::from(window.width) << 16 } else { left + (i64::from(width) << 16) };
            let bottom = if height == 0 { i64::from(window.height) << 16 } else { top + (i64::from(height) << 16) };
            if self.map_state(source) != 2 || x < left || x >= right || y < top || y >= bottom {
                return Ok(Vec::new());
            }
        }
        let origin = if destination.0 == 0 {
            (i32::from(self.pointer_x), i32::from(self.pointer_y))
        } else {
            self.window_origin(destination)
        };
        let x = (i64::from(origin.0) + (coordinate(20) >> 16))
            .clamp(0, i64::from(self.config.width_px.saturating_sub(1)));
        let y = (i64::from(origin.1) + (coordinate(24) >> 16))
            .clamp(0, i64::from(self.config.height_px.saturating_sub(1)));
        self.move_pointer(clamp_i16(x as i32), clamp_i16(y as i32), self.input_state);
        Ok(Vec::new())
    }
}
