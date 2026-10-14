//! Selection requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn set_selection_owner(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let owner = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let selection = AtomId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let requested_time = read_u32_le(body.get(8..12).ok_or_else(bad_length)?);
        if owner.0 != 0 && !self.windows.contains_key(&owner.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, owner.0));
        }
        if !self.atoms.exists(selection) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, selection.0));
        }
        let resolved_time = if requested_time == 0 {
            self.next_timestamp()
        } else {
            requested_time
        };
        if self
            .selections
            .get(&selection.0)
            .is_some_and(|(_, last_changed, _)| resolved_time < *last_changed)
        {
            return Ok(Vec::new());
        }
        let previous = self.selections.get(&selection.0).copied();
        if owner.0 == 0 {
            self.selections.remove(&selection.0);
        } else {
            self.selections
                .insert(selection.0, (owner, resolved_time, handle.0));
        }
        if let Some((old_owner, _, old_client)) = previous
            && (owner.0 == 0 || old_client != handle.0)
        {
            self.emit_to_client(ClientHandle(old_client), |output, sequence, order| {
                x11::encode_selection_clear_event(
                    output,
                    sequence,
                    order,
                    resolved_time,
                    old_owner,
                    selection,
                );
            });
        }
        let subscribers: Vec<(u32, u32)> = self
            .xfixes_selection_masks
            .iter()
            .filter_map(|((client, window, selected), mask)| {
                (*selected == selection.0 && *mask & x11xfixes::SELECTION_MASK_SET_OWNER != 0)
                    .then_some((*client, *window))
            })
            .collect();
        for (subscriber, window) in subscribers {
            self.emit_to_client(ClientHandle(subscriber), |output, sequence, order| {
                x11xfixes::encode_selection_notify_event(
                    output,
                    order,
                    XFIXES_FIRST_EVENT,
                    x11xfixes::SELECTION_NOTIFY_SET_OWNER,
                    sequence,
                    window,
                    owner.0,
                    selection.0,
                    resolved_time,
                    resolved_time,
                );
            });
        }
        Ok(Vec::new())
    }

    pub(super) fn get_selection_owner(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let selection = AtomId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        if !self.atoms.exists(selection) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, selection.0));
        }
        let owner = self
            .selections
            .get(&selection.0)
            .map_or(ResourceId(0), |(owner, _, _)| *owner);
        let mut output = Vec::with_capacity(32);
        x11::write_get_selection_owner_reply(&mut output, order, sequence, owner)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn convert_selection(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let requestor = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let selection = AtomId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let target = AtomId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        let property = AtomId(read_u32_le(body.get(12..16).ok_or_else(bad_length)?));
        let requested_time = read_u32_le(body.get(16..20).ok_or_else(bad_length)?);
        let requestor_client = self
            .windows
            .get(&requestor.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, requestor.0))?
            .owner;
        if !self.atoms.exists(selection) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, selection.0));
        }
        if !self.atoms.exists(target) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, target.0));
        }
        if property.0 != 0 && !self.atoms.exists(property) {
            return Err(ProtocolError::new(x11::error::BAD_ATOM, property.0));
        }
        let time = if requested_time == 0 {
            self.next_timestamp()
        } else {
            requested_time
        };
        if let Some((owner, _, owner_client)) = self.selections.get(&selection.0).copied() {
            self.emit_to_client(ClientHandle(owner_client), |output, sequence, order| {
                x11::encode_selection_request_event(
                    output, sequence, order, time, owner, requestor, selection, target, property,
                );
            });
        } else if let Some(requestor_client) = requestor_client {
            self.emit_to_client(ClientHandle(requestor_client), |output, sequence, order| {
                encode_selection_notify_event(
                    output,
                    sequence,
                    order,
                    time,
                    requestor,
                    selection,
                    target,
                    AtomId(0),
                );
            });
        }
        Ok(Vec::new())
    }
}
