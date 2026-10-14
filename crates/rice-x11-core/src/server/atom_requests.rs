//! Atom requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn intern_atom(
        &mut self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        only_if_exists: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let name = x11::intern_atom_name(body);
        if name.is_empty() {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        let atom = self.atoms.intern(&name, only_if_exists != 0);
        let mut output = Vec::with_capacity(32);
        x11::write_intern_atom_reply(&mut output, byte_order, sequence, atom)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn get_atom_name(
        &self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let atom = AtomId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let name = self
            .atoms
            .name(atom)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ATOM, atom.0))?;
        let mut output = Vec::with_capacity(32 + x11::pad4(name.len()));
        x11::write_get_atom_name_reply(&mut output, byte_order, sequence, name)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn change_property(
        &mut self,
        sequence: SequenceNumber,
        mode: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::change_property_request(mode, body).ok_or_else(bad_length)?;
        let changes_wm_window_type =
            self.atoms.name(request.property) == Some("_NET_WM_WINDOW_TYPE");
        let changes_wm_decorations =
            self.atoms.name(request.property) == Some("_MOTIF_WM_HINTS");
        let changes_wm_chrome = matches!(
            self.atoms.name(request.property),
            Some("_NET_WM_ICON" | "_NET_WM_NAME" | "WM_NAME")
        );
        if !self.windows.contains_key(&request.window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, request.window.0));
        }
        if !self.atoms.exists(request.property) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, request.property.0));
        }
        if !self.atoms.exists(request.r#type) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, request.r#type.0));
        }
        if !matches!(request.format, 8 | 16 | 32) {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(request.format),
            ));
        }
        let expected = usize::try_from(request.length)
            .ok()
            .and_then(|length| length.checked_mul(usize::from(request.format / 8)));
        if expected != Some(request.data.len()) {
            return Err(bad_length());
        }
        let current = self
            .windows
            .get(&request.window.0)
            .and_then(|window| window.properties.get(&request.property))
            .cloned();
        if mode > 2 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(mode)));
        }
        if mode != 0
            && let Some(existing) = &current
            && (existing.r#type != request.r#type || existing.format != request.format)
        {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                request.property.0,
            ));
        }
        let old_len = current.as_ref().map_or(0, |value| value.data.len());
        let combined_len = if mode == 0 {
            request.data.len()
        } else {
            old_len.saturating_add(request.data.len())
        };
        if combined_len > MAX_PROPERTY_BYTES {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        let mut property = Property {
            r#type: request.r#type,
            format: request.format,
            data: Vec::with_capacity(combined_len),
        };
        match (mode, current) {
            (1, Some(existing)) => {
                property.data.extend_from_slice(&request.data);
                property.data.extend_from_slice(&existing.data);
            }
            (2, Some(existing)) => {
                property.data.extend_from_slice(&existing.data);
                property.data.extend_from_slice(&request.data);
            }
            _ => property.data.extend_from_slice(&request.data),
        }
        if let Some(window) = self.windows.get_mut(&request.window.0) {
            window.properties.insert(request.property, property);
        }
        let timestamp = self.next_timestamp();
        self.emit_window_event(
            request.window,
            PROPERTY_CHANGE_MASK,
            |output, target_sequence, order| {
                x11::encode_property_notify_event(
                    output,
                    target_sequence,
                    order,
                    request.window,
                    request.property,
                    timestamp,
                    false,
                );
            },
        );
        if self.config.native_window_manager
            && changes_wm_window_type
            && self.wm_window_has_type(request.window, "_NET_WM_WINDOW_TYPE_DESKTOP")
        {
            self.promote_wm_desktop_layer(request.window);
        }
        // Applications commonly publish their effective icon or final title
        // after mapping the top-level. The property store is authoritative,
        // but managed-frame/task chrome is retained pixel content, so it must
        // be repainted when those properties change. Without this refresh an
        // already-created frame keeps the generic launch icon indefinitely
        // even though `_NET_WM_ICON` contains the application's exact pixels.
        if self.config.native_window_manager && changes_wm_chrome {
            self.repaint_wm_client_identity(request.window);
        }
        if self.config.native_window_manager && changes_wm_decorations {
            self.refresh_managed_decorations(request.window);
        }
        let _ = sequence;
        Ok(Vec::new())
    }

    pub(super) fn delete_property(
        &mut self,
        _sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::delete_property_request(body).ok_or_else(bad_length)?;
        let changes_wm_chrome = matches!(
            self.atoms.name(request.property),
            Some("_NET_WM_ICON" | "_NET_WM_NAME" | "WM_NAME")
        );
        let changes_wm_decorations =
            self.atoms.name(request.property) == Some("_MOTIF_WM_HINTS");
        let window = self
            .windows
            .get_mut(&request.window.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, request.window.0))?;
        if !self.atoms.exists(request.property) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, request.property.0));
        }
        if window.properties.remove(&request.property).is_some() {
            let timestamp = self.next_timestamp();
            self.emit_window_event(
                request.window,
                PROPERTY_CHANGE_MASK,
                |output, sequence, order| {
                    x11::encode_property_notify_event(
                        output,
                        sequence,
                        order,
                        request.window,
                        request.property,
                        timestamp,
                        true,
                    );
                },
            );
            if self.config.native_window_manager && changes_wm_chrome {
                self.repaint_wm_client_identity(request.window);
            }
            if self.config.native_window_manager && changes_wm_decorations {
                self.refresh_managed_decorations(request.window);
            }
        }
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn get_property(
        &mut self,
        handle: ClientHandle,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        delete: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::get_property_request(delete, body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&request.window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, request.window.0));
        }
        if !self.atoms.exists(request.property) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, request.property.0));
        }
        if request.r#type.0 != 0 && !self.atoms.exists(request.r#type) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, request.r#type.0));
        }
        let property = self
            .windows
            .get(&request.window.0)
            .and_then(|window| window.properties.get(&request.property))
            .cloned();
        let Some(property) = property else {
            return encode_get_property_reply(byte_order, sequence, 0, AtomId(0), 0, &[]);
        };
        let type_matches = request.r#type.0 == 0 || request.r#type == property.r#type;
        if !type_matches {
            return encode_get_property_reply(
                byte_order,
                sequence,
                property.format,
                property.r#type,
                u32::try_from(property.data.len()).unwrap_or(u32::MAX),
                &[],
            );
        }
        let start = usize::try_from(request.long_offset)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, request.long_offset))?;
        if start > property.data.len() {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                request.long_offset,
            ));
        }
        let requested_bytes = usize::try_from(request.long_length)
            .ok()
            .and_then(|value| value.checked_mul(4))
            .unwrap_or(usize::MAX);
        let unit = usize::from(property.format / 8);
        let mut length = requested_bytes.min(property.data.len().saturating_sub(start));
        length -= length % unit;
        let end = start + length;
        let bytes_after = property.data.len() - end;
        let output = encode_get_property_reply(
            byte_order,
            sequence,
            property.format,
            property.r#type,
            u32::try_from(bytes_after).unwrap_or(u32::MAX),
            &property.data[start..end],
        )?;
        if delete != 0 && bytes_after == 0 {
            self.queue_to_client(handle, &output)
                .map_err(|_| encoding_error())?;
            if let Some(window) = self.windows.get_mut(&request.window.0) {
                window.properties.remove(&request.property);
            }
            let timestamp = self.next_timestamp();
            self.emit_window_event(
                request.window,
                PROPERTY_CHANGE_MASK,
                |event, target_sequence, order| {
                    x11::encode_property_notify_event(
                        event,
                        target_sequence,
                        order,
                        request.window,
                        request.property,
                        timestamp,
                        true,
                    );
                },
            );
            return Ok(Vec::new());
        }
        Ok(output)
    }

    pub(super) fn list_properties(
        &self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        let state = self
            .windows
            .get(&window.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window.0))?;
        let atoms: Vec<AtomId> = state.properties.keys().copied().collect();
        Ok(encode_list_properties(byte_order, sequence, &atoms))
    }
}
