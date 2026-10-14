//! Events operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn validate_event_selection(
        &self,
        handle: ClientHandle,
        window: ResourceId,
        mask: u32,
    ) -> Result<(), ProtocolError> {
        let exclusive = mask & EXCLUSIVE_EVENT_MASKS;
        if exclusive == 0 {
            return Ok(());
        }
        let conflict = self.clients.iter().any(|(id, client)| {
            *id != handle.0
                && client
                    .event_masks
                    .get(&window.0)
                    .is_some_and(|selected| selected & exclusive != 0)
        });
        if conflict {
            Err(ProtocolError::new(x11::error::BAD_ACCESS, mask))
        } else {
            Ok(())
        }
    }

    pub(super) fn redirecting_client(
        &self,
        window: ResourceId,
        mask: u32,
        requester: ClientHandle,
    ) -> Option<ClientHandle> {
        self.clients.iter().find_map(|(id, client)| {
            (*id != requester.0
                && client
                    .event_masks
                    .get(&window.0)
                    .is_some_and(|selected| selected & mask != 0))
            .then_some(ClientHandle(*id))
        })
    }

    pub(super) fn emit_structure_event(
        &mut self,
        window: &Window,
        encode: impl Fn(&mut Vec<u8>, SequenceNumber, ClientByteOrder, ResourceId),
    ) {
        self.emit_window_event(
            window.id,
            STRUCTURE_NOTIFY_MASK,
            |output, sequence, order| {
                encode(output, sequence, order, window.id);
            },
        );
        self.emit_window_event(
            window.parent,
            SUBSTRUCTURE_NOTIFY_MASK,
            |output, sequence, order| {
                encode(output, sequence, order, window.parent);
            },
        );
    }

    pub(super) fn emit_managed_configure_notify(
        &mut self,
        window: &Window,
        geometry: Geometry,
        synthetic: bool,
    ) {
        self.emit_window_event(
            window.id,
            STRUCTURE_NOTIFY_MASK,
            |output, sequence, order| {
                let event_start = output.len();
                x11::encode_configure_notify_event(
                    output, sequence, order, window.id, window.id, None, geometry, false,
                );
                if synthetic && let Some(event_type) = output.get_mut(event_start) {
                    *event_type |= 0x80;
                }
            },
        );
    }

    pub(super) fn emit_full_exposure(&mut self, window: &Window) {
        if !window.mapped {
            return;
        }
        self.emit_window_event(window.id, EXPOSURE_MASK, |output, sequence, order| {
            x11::encode_expose_event(
                output,
                sequence,
                order,
                window.id,
                0,
                0,
                window.width,
                window.height,
                0,
            );
        });
    }

    pub(super) fn emit_window_event(
        &mut self,
        window: ResourceId,
        mask: u32,
        encode: impl Fn(&mut Vec<u8>, SequenceNumber, ClientByteOrder),
    ) {
        let targets: Vec<ClientHandle> = self
            .clients
            .iter()
            .filter_map(|(id, client)| {
                client
                    .event_masks
                    .get(&window.0)
                    .is_some_and(|selected| selected & mask != 0)
                    .then_some(ClientHandle(*id))
            })
            .collect();
        for target in targets {
            self.emit_to_client(target, &encode);
        }
    }

    pub(super) fn emit_to_client(
        &mut self,
        target: ClientHandle,
        encode: impl Fn(&mut Vec<u8>, SequenceNumber, ClientByteOrder),
    ) {
        let Some((sequence, order)) =
            self.clients
                .get(&target.0)
                .and_then(|client| match client.phase {
                    ClientPhase::Ready { byte_order } => Some((client.sequence, byte_order)),
                    _ => None,
                })
        else {
            return;
        };
        let mut output = Vec::with_capacity(32);
        encode(&mut output, sequence, order);
        let _ = self.queue_to_client(target, &output);
    }

    pub(super) fn queue_to_client(&mut self, handle: ClientHandle, output: &[u8]) -> Result<(), ClientError> {
        let client = self
            .clients
            .get_mut(&handle.0)
            .ok_or(ClientError::UnknownClient)?;
        if let Err(error) = queue_output(client, output, self.config.max_egress_bytes) {
            Self::fault_client(client);
            return Err(error);
        }
        Ok(())
    }
}
