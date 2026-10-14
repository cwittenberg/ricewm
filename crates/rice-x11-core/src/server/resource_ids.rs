//! Resource ids operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn xcmisc_request(
        &self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        let result = match minor {
            0 => {
                if body.len() == 4 {
                    let mut output = Vec::with_capacity(32);
                    x11::write_xcmisc_get_version_reply(&mut output, order, sequence)
                        .map_err(|_| encoding_error())?;
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            1 => {
                if body.is_empty() {
                    let base = client.resource_id_base();
                    let end = base | RESOURCE_ID_MASK;
                    let start = (base..=end)
                        .find(|id| !self.resource_exists(ResourceId(*id)))
                        .unwrap_or(0);
                    let count = if start == 0 {
                        0
                    } else {
                        (start..=end)
                            .take(65_536)
                            .take_while(|id| !self.resource_exists(ResourceId(*id)))
                            .count()
                    };
                    let mut output = Vec::with_capacity(32);
                    x11::write_xcmisc_get_xid_range_reply(
                        &mut output,
                        order,
                        sequence,
                        start,
                        u32::try_from(count).unwrap_or(u32::MAX),
                    )
                    .map_err(|_| encoding_error())?;
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            2 => {
                if body.len() == 4 {
                    let requested = usize::try_from(read_u32(order, body))
                        .unwrap_or(usize::MAX)
                        .min(65_536);
                    let base = client.resource_id_base();
                    let end = base | RESOURCE_ID_MASK;
                    let ids: Vec<u32> = (base..=end)
                        .filter(|id| !self.resource_exists(ResourceId(*id)))
                        .take(requested)
                        .collect();
                    let mut output = Vec::with_capacity(32 + ids.len() * 4);
                    x11::write_xcmisc_get_xid_list_reply(&mut output, order, sequence, &ids)
                        .map_err(|_| encoding_error())?;
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|error| extension_error(error, minor))
    }
}
