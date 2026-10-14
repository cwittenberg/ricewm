//! Generic events operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn generic_event_request(
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if minor != 0 {
            return Err(extension_error(
                ProtocolError::new(x11::error::BAD_REQUEST, u32::from(minor)),
                minor,
            ));
        }
        if body.len() != 4 {
            return Err(extension_error(bad_length(), minor));
        }
        let mut output = Vec::with_capacity(32);
        x11::write_ge_query_version_reply(&mut output, order, sequence)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }
}
