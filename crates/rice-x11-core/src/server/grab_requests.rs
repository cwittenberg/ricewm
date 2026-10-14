//! Grab requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn grab_pointer(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        owner_events: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let event_mask = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(4..6).ok_or_else(bad_length)?,
        );
        let pointer_mode = *body.get(6).ok_or_else(bad_length)?;
        let keyboard_mode = *body.get(7).ok_or_else(bad_length)?;
        let confine_to = ResourceId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        if confine_to.0 != 0 && !self.windows.contains_key(&confine_to.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, confine_to.0));
        }
        if pointer_mode > 1 || keyboard_mode > 1 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(pointer_mode.max(keyboard_mode)),
            ));
        }
        let status = if self.native_task_owns_input() || self.pointer_grab.is_some_and(|grab| grab.owner != handle.0) {
            1
        } else if self.map_state(window) != 2 {
            3
        } else {
            self.pointer_grab = Some(ActivePointerGrab {
                owner: handle.0,
                window,
                owner_events: owner_events != 0,
                event_mask,
                automatic: false,
            });
            0
        };
        encode_grab_reply(order, sequence, status)
    }

    pub(super) fn ungrab_pointer(&mut self, handle: ClientHandle) -> Vec<u8> {
        if self.pointer_grab.is_some_and(|grab| grab.owner == handle.0) {
            self.pointer_grab = None;
        }
        Vec::new()
    }

    pub(super) fn grab_keyboard(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        owner_events: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let pointer_mode = *body.get(8).ok_or_else(bad_length)?;
        let keyboard_mode = *body.get(9).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        if pointer_mode > 1 || keyboard_mode > 1 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(pointer_mode.max(keyboard_mode)),
            ));
        }
        let status = if self.native_task_owns_input() || self
            .keyboard_grab
            .is_some_and(|grab| grab.owner != handle.0)
        {
            1
        } else if self.map_state(window) != 2 {
            3
        } else {
            self.keyboard_grab = Some(ActiveKeyboardGrab {
                owner: handle.0,
                window,
                owner_events: owner_events != 0,
            });
            0
        };
        encode_grab_reply(order, sequence, status)
    }

    pub(super) fn ungrab_keyboard(&mut self, handle: ClientHandle) -> Vec<u8> {
        if self
            .keyboard_grab
            .is_some_and(|grab| grab.owner == handle.0)
        {
            self.keyboard_grab = None;
        }
        Vec::new()
    }

    pub(super) fn query_pointer(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        let (origin_x, origin_y) = self.window_origin(window);
        let child = self.child_at(window, self.pointer_x, self.pointer_y);
        let mut output = Vec::with_capacity(32);
        x11::write_query_pointer_reply(
            &mut output,
            order,
            sequence,
            x11::QueryPointerReply {
                root: ROOT_WINDOW,
                child,
                root_x: self.pointer_x,
                root_y: self.pointer_y,
                win_x: clamp_i16(i32::from(self.pointer_x) - origin_x),
                win_y: clamp_i16(i32::from(self.pointer_y) - origin_y),
                mask: self.input_state,
            },
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn get_motion_events(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        let mut output = Vec::with_capacity(32);
        output.push(1);
        output.push(0);
        let mut sequence_bytes = [0; 2];
        write_u16_at(order, &mut sequence_bytes, sequence.0);
        output.extend_from_slice(&sequence_bytes);
        write_u32_vec(order, &mut output, 0);
        write_u32_vec(order, &mut output, 0);
        output.resize(32, 0);
        Ok(output)
    }

    pub(super) fn translate_coordinates(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let source = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let destination = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let source_x = read_i16_le(body.get(8..10).ok_or_else(bad_length)?);
        let source_y = read_i16_le(body.get(10..12).ok_or_else(bad_length)?);
        if !self.windows.contains_key(&source.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, source.0));
        }
        if !self.windows.contains_key(&destination.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, destination.0));
        }
        let source_origin = self.window_origin(source);
        let destination_origin = self.window_origin(destination);
        let root_x = source_origin.0.saturating_add(i32::from(source_x));
        let root_y = source_origin.1.saturating_add(i32::from(source_y));
        let child = self.child_at(destination, clamp_i16(root_x), clamp_i16(root_y));
        let mut output = Vec::with_capacity(32);
        x11::write_translate_coordinates_reply(
            &mut output,
            order,
            sequence,
            child,
            clamp_i16(root_x - destination_origin.0),
            clamp_i16(root_y - destination_origin.1),
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn warp_pointer(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let source = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let destination = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        if source.0 != 0 && !self.windows.contains_key(&source.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, source.0));
        }
        if destination.0 != 0 && !self.windows.contains_key(&destination.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, destination.0));
        }
        let destination_x = read_i16_le(body.get(16..18).ok_or_else(bad_length)?);
        let destination_y = read_i16_le(body.get(18..20).ok_or_else(bad_length)?);
        let (target_x, target_y) = if destination.0 == 0 {
            (
                i32::from(self.pointer_x).saturating_add(i32::from(destination_x)),
                i32::from(self.pointer_y).saturating_add(i32::from(destination_y)),
            )
        } else {
            let origin = self.window_origin(destination);
            (
                origin.0.saturating_add(i32::from(destination_x)),
                origin.1.saturating_add(i32::from(destination_y)),
            )
        };
        let target_x = target_x.clamp(0, i32::from(self.config.width_px.saturating_sub(1)));
        let target_y = target_y.clamp(0, i32::from(self.config.height_px.saturating_sub(1)));
        self.move_pointer(clamp_i16(target_x), clamp_i16(target_y), self.input_state);
        Ok(Vec::new())
    }

    pub(super) fn query_keymap(&self, order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
        let mut output = Vec::with_capacity(40);
        output.push(1);
        output.push(0);
        let mut sequence_bytes = [0; 2];
        write_u16_at(order, &mut sequence_bytes, sequence.0);
        output.extend_from_slice(&sequence_bytes);
        write_u32_vec(order, &mut output, 2);
        output.extend_from_slice(&self.keymap);
        output
    }
}
