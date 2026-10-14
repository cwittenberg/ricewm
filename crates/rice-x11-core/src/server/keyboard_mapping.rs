//! Keyboard mapping operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn change_keyboard_mapping(
        &mut self,
        keycode_count: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let first_keycode = *body.first().ok_or_else(bad_length)?;
        let keysyms_per_keycode = *body.get(1).ok_or_else(bad_length)?;
        if first_keycode < 8
            || u16::from(first_keycode).saturating_add(u16::from(keycode_count)) > 256
            || keysyms_per_keycode == 0
        {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(first_keycode),
            ));
        }
        let wire = body.get(4..).ok_or_else(bad_length)?;
        for index in 0..keycode_count {
            let mut row = Vec::with_capacity(usize::from(keysyms_per_keycode));
            for column in 0..keysyms_per_keycode {
                let offset = (usize::from(index)
                    .saturating_mul(usize::from(keysyms_per_keycode))
                    .saturating_add(usize::from(column)))
                .saturating_mul(4);
                row.push(read_u32_le(
                    wire.get(offset..offset.saturating_add(4))
                        .ok_or_else(bad_length)?,
                ));
            }
            self.keysyms.insert(first_keycode.wrapping_add(index), row);
        }
        self.emit_mapping_notify(1, first_keycode, keycode_count);
        Ok(Vec::new())
    }

    pub(super) fn get_keyboard_mapping(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let first = *body.first().ok_or_else(bad_length)?;
        let count = *body.get(1).ok_or_else(bad_length)?;
        if first < 8 || u16::from(first).saturating_add(u16::from(count)) > 256 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(first)));
        }
        let width = (0..count)
            .filter_map(|offset| self.keysyms.get(&first.wrapping_add(offset)))
            .map(Vec::len)
            .max()
            .unwrap_or(2)
            .clamp(2, 255);
        let mut keysyms = Vec::with_capacity(usize::from(count).saturating_mul(width));
        for offset in 0..count {
            let keycode = first.wrapping_add(offset);
            let default = default_keysyms(keycode);
            let row = self.keysyms.get(&keycode);
            for column in 0..width {
                keysyms.push(
                    row.and_then(|values| values.get(column))
                        .copied()
                        .unwrap_or_else(|| default.get(column).copied().unwrap_or(0)),
                );
            }
        }
        let mut output = Vec::with_capacity(32 + keysyms.len().saturating_mul(4));
        x11::write_get_keyboard_mapping_reply_from_keysyms(
            &mut output,
            order,
            sequence,
            u8::try_from(width).unwrap_or(u8::MAX),
            &keysyms,
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn set_screensaver(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let timeout = read_i16_le(body.get(0..2).ok_or_else(bad_length)?);
        let interval = read_i16_le(body.get(2..4).ok_or_else(bad_length)?);
        let prefer_blanking = *body.get(4).ok_or_else(bad_length)?;
        let allow_exposures = *body.get(5).ok_or_else(bad_length)?;
        if timeout < -1 || interval < -1 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        if prefer_blanking > 2 || allow_exposures > 2 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(prefer_blanking.max(allow_exposures)),
            ));
        }
        self.screensaver = (
            if timeout == -1 {
                600
            } else {
                u16::try_from(timeout).unwrap_or(0)
            },
            if interval == -1 {
                600
            } else {
                u16::try_from(interval).unwrap_or(0)
            },
            if prefer_blanking == 2 {
                1
            } else {
                prefer_blanking
            },
            if allow_exposures == 2 {
                1
            } else {
                allow_exposures
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn get_screensaver(&self, order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
        let mut output = fixed_reply_header(order, sequence, 0, 0);
        let mut value = [0; 2];
        write_u16_at(order, &mut value, self.screensaver.0);
        output.extend_from_slice(&value);
        write_u16_at(order, &mut value, self.screensaver.1);
        output.extend_from_slice(&value);
        output.push(self.screensaver.2);
        output.push(self.screensaver.3);
        output.resize(32, 0);
        output
    }

    pub(super) fn list_hosts(
        order: ClientByteOrder,
        sequence: SequenceNumber,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut output = Vec::with_capacity(32);
        x11::write_list_hosts_reply(&mut output, order, sequence).map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn rotate_properties(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let window = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let count = usize::from(read_u16(
            ClientByteOrder::LittleEndian,
            body.get(4..6).ok_or_else(bad_length)?,
        ));
        let delta = i32::from(read_i16_le(body.get(6..8).ok_or_else(bad_length)?));
        let mut atoms = Vec::with_capacity(count);
        for wire in body
            .get(8..8usize.saturating_add(count.saturating_mul(4)))
            .ok_or_else(bad_length)?
            .chunks_exact(4)
        {
            let atom = AtomId(read_u32_le(wire));
            if !self.atoms.exists(atom) {
                return Err(ProtocolError::new(x11::error::BAD_ATOM, atom.0));
            }
            if atoms.contains(&atom) {
                return Err(ProtocolError::new(x11::error::BAD_MATCH, atom.0));
            }
            atoms.push(atom);
        }
        let state = self
            .windows
            .get_mut(&window.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window.0))?;
        let mut values = Vec::with_capacity(count);
        for atom in &atoms {
            values.push(
                state
                    .properties
                    .get(atom)
                    .cloned()
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_MATCH, atom.0))?,
            );
        }
        if count != 0 {
            let shift = delta.rem_euclid(i32::try_from(count).unwrap_or(i32::MAX));
            values.rotate_right(usize::try_from(shift).unwrap_or(0));
            for (atom, property) in atoms.into_iter().zip(values) {
                state.properties.insert(atom, property);
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn set_pointer_mapping(
        &mut self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        count: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let mapping = body.get(..usize::from(count)).ok_or_else(bad_length)?;
        if mapping.len() != self.pointer_mapping.len() {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(count)));
        }
        let mut seen = [false; 256];
        for value in mapping.iter().copied().filter(|value| *value != 0) {
            if seen[usize::from(value)] {
                return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(value)));
            }
            seen[usize::from(value)] = true;
        }
        self.pointer_mapping.clear();
        self.pointer_mapping.extend_from_slice(mapping);
        self.emit_mapping_notify(2, 0, 0);
        Ok(fixed_reply_32(order, sequence, 0))
    }

    pub(super) fn get_pointer_mapping(&self, order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
        let mut output = fixed_reply_header(
            order,
            sequence,
            u8::try_from(self.pointer_mapping.len()).unwrap_or(u8::MAX),
            u32::try_from(self.pointer_mapping.len().div_ceil(4)).unwrap_or(u32::MAX),
        );
        output.extend_from_slice(&[0; 24]);
        output.extend_from_slice(&self.pointer_mapping);
        output.resize(x11::pad4(output.len()), 0);
        output
    }

    pub(super) fn set_modifier_mapping(
        &mut self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        keycodes_per_modifier: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let required = usize::from(keycodes_per_modifier).saturating_mul(8);
        let keycodes = body.get(..required).ok_or_else(bad_length)?;
        if let Some(invalid) = keycodes
            .iter()
            .copied()
            .find(|keycode| *keycode != 0 && *keycode < 8)
        {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(invalid),
            ));
        }
        self.modifier_mapping = (keycodes_per_modifier, keycodes.to_vec());
        self.emit_mapping_notify(0, 0, 0);
        Ok(fixed_reply_32(order, sequence, 0))
    }

    pub(super) fn get_modifier_mapping(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut output = Vec::new();
        x11::write_get_modifier_mapping_reply_with_keycodes(
            &mut output,
            order,
            sequence,
            self.modifier_mapping.0,
            &self.modifier_mapping.1,
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn emit_mapping_notify(&mut self, request: u8, first_keycode: u8, count: u8) {
        let targets: Vec<ClientHandle> = self.clients.keys().copied().map(ClientHandle).collect();
        for target in targets {
            self.emit_to_client(target, |output, sequence, order| {
                let _ = x11::write_mapping_notify_event(
                    output,
                    order,
                    sequence,
                    request,
                    first_keycode,
                    count,
                );
            });
        }
    }
}
