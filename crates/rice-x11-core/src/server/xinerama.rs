//! Xinerama operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn xinerama_request(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            x11xinerama::QUERY_VERSION => {
                if body.len() == 4 {
                    Ok(x11xinerama::encode_query_version_reply(order, sequence))
                } else {
                    Err(bad_length())
                }
            }
            x11xinerama::GET_STATE | x11xinerama::GET_SCREEN_COUNT => {
                if body.len() != 4 {
                    return Err(extension_error(bad_length(), minor));
                }
                let window = ResourceId(read_u32(order, body));
                if !self.windows.contains_key(&window.0) {
                    Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0))
                } else if minor == x11xinerama::GET_STATE {
                    Ok(x11xinerama::encode_get_state_reply(
                        order, sequence, true, window.0,
                    ))
                } else {
                    Ok(x11xinerama::encode_get_screen_count_reply(
                        order, sequence, 1, window.0,
                    ))
                }
            }
            x11xinerama::GET_SCREEN_SIZE => {
                if body.len() != 8 {
                    return Err(extension_error(bad_length(), minor));
                }
                let window = ResourceId(read_u32(order, body));
                let screen = read_u32(order, &body[4..8]);
                if !self.windows.contains_key(&window.0) {
                    Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0))
                } else if screen != 0 {
                    Err(ProtocolError::new(x11::error::BAD_VALUE, screen))
                } else {
                    Ok(x11xinerama::encode_get_screen_size_reply(
                        order,
                        sequence,
                        u32::from(self.config.width_px),
                        u32::from(self.config.height_px),
                        window.0,
                        screen,
                    ))
                }
            }
            x11xinerama::IS_ACTIVE => {
                if body.is_empty() {
                    Ok(x11xinerama::encode_is_active_reply(order, sequence, true))
                } else {
                    Err(bad_length())
                }
            }
            x11xinerama::QUERY_SCREENS => {
                if body.is_empty() {
                    Ok(x11xinerama::encode_query_screens_reply(
                        order,
                        sequence,
                        &[x11xinerama::ScreenInfo {
                            x_org: 0,
                            y_org: 0,
                            width: self.config.width_px,
                            height: self.config.height_px,
                        }],
                    ))
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
