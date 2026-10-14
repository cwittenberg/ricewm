//! Xtest operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn xtest_request(
        &mut self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            x11xtest::GET_VERSION => {
                if body.len() == 4 {
                    let client_major = body[0];
                    let client_minor = read_u16(order, &body[2..4]);
                    Ok(x11xtest::encode_get_version_reply(
                        order,
                        sequence,
                        u8::try_from(x11xtest::MAJOR_VERSION)
                            .unwrap_or(u8::MAX)
                            .min(client_major),
                        x11xtest::MINOR_VERSION.min(client_minor),
                    ))
                } else {
                    Err(bad_length())
                }
            }
            x11xtest::COMPARE_CURSOR => {
                if body.len() != 8 {
                    return Err(extension_error(bad_length(), minor));
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                let cursor = read_u32(order, &body[4..8]);
                if cursor > 1 && !self.cursors.contains_key(&cursor) {
                    Err(ProtocolError::new(x11::error::BAD_CURSOR, cursor))
                } else {
                    // The Canvas renderer currently uses the browser's active
                    // pointer sprite. Cursor 0 ("current cursor") therefore
                    // compares equal; explicit guest cursors are conservative.
                    Ok(x11xtest::encode_compare_cursor_reply(
                        order,
                        sequence,
                        cursor == 0,
                    ))
                }
            }
            x11xtest::FAKE_INPUT => {
                if body.len() != 28 {
                    return Err(extension_error(bad_length(), minor));
                }
                let event_type = body[0];
                let detail = body[1];
                let root = read_u32(order, &body[8..12]);
                if root != 0 && root != ROOT_WINDOW.0 {
                    return Err(extension_error(
                        ProtocolError::new(x11::error::BAD_WINDOW, root),
                        minor,
                    ));
                }
                let x = read_i16(order, &body[20..22]);
                let y = read_i16(order, &body[22..24]);
                match event_type {
                    x11xtest::FAKE_KEY_PRESS | x11xtest::FAKE_KEY_RELEASE => {
                        self.inject_key(
                            event_type == x11xtest::FAKE_KEY_PRESS,
                            detail,
                            self.input_state,
                        );
                    }
                    x11xtest::FAKE_BUTTON_PRESS | x11xtest::FAKE_BUTTON_RELEASE => {
                        self.inject_pointer_button(
                            event_type == x11xtest::FAKE_BUTTON_PRESS,
                            detail,
                            self.pointer_x,
                            self.pointer_y,
                            self.input_state,
                        );
                    }
                    x11xtest::FAKE_MOTION_NOTIFY => {
                        let (x, y) = if detail == 1 {
                            (
                                self.pointer_x.saturating_add(x),
                                self.pointer_y.saturating_add(y),
                            )
                        } else {
                            (x, y)
                        };
                        self.inject_pointer_motion(x, y, self.input_state);
                    }
                    _ => {
                        return Err(extension_error(
                            ProtocolError::new(x11::error::BAD_VALUE, u32::from(event_type)),
                            minor,
                        ));
                    }
                }
                Ok(Vec::new())
            }
            x11xtest::GRAB_CONTROL => {
                if body.len() != 4 {
                    Err(bad_length())
                } else if body[0] > 1 {
                    Err(ProtocolError::new(
                        x11::error::BAD_VALUE,
                        u32::from(body[0]),
                    ))
                } else {
                    Ok(Vec::new())
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
