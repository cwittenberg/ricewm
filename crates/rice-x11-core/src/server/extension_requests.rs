//! Extension requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn query_extension(
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let length = usize::from(read_u16(
            ClientByteOrder::LittleEndian,
            body.get(0..2).ok_or_else(bad_length)?,
        ));
        let name = std::str::from_utf8(
            body.get(4..4usize.saturating_add(length))
                .ok_or_else(bad_length)?,
        )
        .unwrap_or_default();
        let metadata = extension_metadata(name);
        let mut output = Vec::with_capacity(32);
        x11::write_query_extension_reply(
            &mut output,
            order,
            sequence,
            metadata.is_some(),
            metadata.map_or(0, |value| value.0),
            metadata.map_or(0, |value| value.1),
            metadata.map_or(0, |value| value.2),
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn list_extensions(
        order: ClientByteOrder,
        sequence: SequenceNumber,
    ) -> Result<Vec<u8>, ProtocolError> {
        let names = implemented_extension_names();
        let mut output = Vec::new();
        x11::write_list_extensions_reply(&mut output, order, sequence, &names)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn enable_big_requests(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
    ) -> Result<Vec<u8>, ProtocolError> {
        if minor != 0 {
            return Err(ProtocolError {
                code: x11::error::BAD_REQUEST,
                bad_value: u32::from(minor),
                minor_opcode: u16::from(minor),
            });
        }
        let client = self
            .clients
            .get_mut(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        client.big_requests_enabled = true;
        let max_units = u32::try_from(self.config.max_request_bytes / 4).unwrap_or(u32::MAX);
        let mut output = Vec::with_capacity(32);
        x11::write_big_requests_enable_reply(&mut output, order, sequence, max_units)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }
}
