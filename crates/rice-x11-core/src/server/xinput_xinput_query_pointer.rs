//! Xinput operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn xinput_query_pointer(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let window = ResourceId(read_u32(order, body));
        self.require_window(window)?;
        validate_xinput_device(read_u16(order, &body[4..6]), true)?;
        let root_x = self.pointer_x;
        let root_y = self.pointer_y;
        let (origin_x, origin_y) = self.window_origin(window);
        let event_x = clamp_i16(i32::from(root_x).saturating_sub(origin_x));
        let event_y = clamp_i16(i32::from(root_y).saturating_sub(origin_y));
        let child = self.child_at(window, self.pointer_x, self.pointer_y);
        let mut output = fixed_reply_header(order, sequence, 0, 7);
        write_u32_vec(order, &mut output, ROOT_WINDOW.0);
        write_u32_vec(order, &mut output, child.0);
        for coordinate in [self.pointer_x, self.pointer_y, event_x, event_y] {
            write_u32_vec(
                order,
                &mut output,
                (i32::from(coordinate) << 16).cast_unsigned(),
            );
        }
        output.extend_from_slice(&[1, 0]);
        let mut word = [0; 2];
        write_u16_at(order, &mut word, 1);
        output.extend_from_slice(&word);
        let modifiers = u32::from(self.input_state & 0x00ff);
        write_u32_vec(order, &mut output, modifiers);
        write_u32_vec(order, &mut output, 0);
        write_u32_vec(order, &mut output, 0);
        write_u32_vec(order, &mut output, modifiers);
        output.extend_from_slice(&[0; 4]);
        let mut buttons = 0u32;
        for (state_mask, button) in [
            (0x0100, 1u32),
            (0x0200, 2),
            (0x0400, 3),
            (0x0800, 4),
            (0x1000, 5),
        ] {
            if self.input_state & state_mask != 0 {
                buttons |= 1 << button;
            }
        }
        output.extend_from_slice(&buttons.to_le_bytes());
        Ok(output)
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn xinput_query_device(
        &mut self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        fn write_key_class(output: &mut Vec<u8>, order: ClientByteOrder, source: u16) {
            write_u16_vec(order, output, 0);
            write_u16_vec(order, output, 250);
            write_u16_vec(order, output, source);
            write_u16_vec(order, output, 248);
            for keycode in 8..=255u32 {
                write_u32_vec(order, output, keycode);
            }
        }

        fn write_button_class(output: &mut Vec<u8>, order: ClientByteOrder, source: u16) {
            write_u16_vec(order, output, 1);
            // Fixed header + one-word XIButtonState + seven label atoms.
            // XI2 derives the state-mask size from num_buttons; unlike the
            // event-mask structures this class contains no mask_len field.
            write_u16_vec(order, output, 10);
            write_u16_vec(order, output, source);
            write_u16_vec(order, output, 7);
            write_u32_vec(order, output, 0);
            for _ in 0..7 {
                write_u32_vec(order, output, 0);
            }
        }

        fn write_valuator_class(
            output: &mut Vec<u8>,
            order: ClientByteOrder,
            source: u16,
            number: u16,
            max: i32,
            value: i32,
        ) {
            write_u16_vec(order, output, 2);
            write_u16_vec(order, output, 11);
            write_u16_vec(order, output, source);
            write_u16_vec(order, output, number);
            write_u32_vec(order, output, 0);
            for component in [-1i32, 0, max, 0, value, 0, 0] {
                write_u32_vec(order, output, component.cast_unsigned());
            }
            output.extend_from_slice(&[0, 0, 0, 0]);
        }

        fn write_scroll_class(
            output: &mut Vec<u8>,
            order: ClientByteOrder,
            source: u16,
            number: u16,
            scroll_type: u16,
        ) {
            write_u16_vec(order, output, 3);
            write_u16_vec(order, output, 6);
            write_u16_vec(order, output, source);
            write_u16_vec(order, output, number);
            write_u16_vec(order, output, scroll_type);
            write_u16_vec(order, output, 0);
            // RiceWM also emits the legacy button 4..7 events, so the XI2
            // scroll class must not claim NoEmulation.
            write_u32_vec(order, output, 0);
            write_u32_vec(order, output, 1);
            write_u32_vec(order, output, 0);
        }

        #[allow(clippy::too_many_arguments)]
        fn write_device(
            output: &mut Vec<u8>,
            order: ClientByteOrder,
            id: u16,
            use_type: u16,
            attachment: u16,
            name: &str,
            class_count: u16,
            classes: &[u8],
        ) {
            write_u16_vec(order, output, id);
            write_u16_vec(order, output, use_type);
            write_u16_vec(order, output, attachment);
            write_u16_vec(order, output, class_count);
            write_u16_vec(order, output, u16::try_from(name.len()).unwrap_or(u16::MAX));
            output.extend_from_slice(&[1, 0]);
            output.extend_from_slice(name.as_bytes());
            output.resize(x11::pad4(output.len()), 0);
            output.extend_from_slice(classes);
        }

        if body.len() != 4 {
            return Err(bad_length());
        }
        let selector = read_u16(order, body);
        validate_xinput_device(selector, true)?;

        let devices: Vec<u16> = match selector {
            0 => vec![2, 3, 4, 5],
            1 => vec![2, 3],
            2..=5 => vec![selector],
            _ => unreachable!(),
        };
        let mut infos = Vec::new();
        for device in &devices {
            let mut classes = Vec::new();
            match *device {
                2 | 4 => {
                    write_button_class(&mut classes, order, *device);
                    write_valuator_class(
                        &mut classes,
                        order,
                        *device,
                        0,
                        -1,
                        i32::from(self.pointer_x),
                    );
                    write_valuator_class(
                        &mut classes,
                        order,
                        *device,
                        1,
                        -1,
                        i32::from(self.pointer_y),
                    );
                    // XI1 ListInputDevices advertises four pointer axes.
                    // Chromium cross-checks that declaration against this
                    // XI2 class list and intentionally traps on a mismatch.
                    write_valuator_class(&mut classes, order, *device, 2, 0, 0);
                    write_valuator_class(&mut classes, order, *device, 3, 0, 0);
                    write_scroll_class(&mut classes, order, *device, 2, 1);
                    write_scroll_class(&mut classes, order, *device, 3, 2);
                    let (use_type, attachment, name) = if *device == 2 {
                        (1, 3, "RiceWM Virtual Core Pointer")
                    } else {
                        (3, 2, "RiceWM Browser Pointer")
                    };
                    write_device(
                        &mut infos, order, *device, use_type, attachment, name, 7, &classes,
                    );
                }
                3 | 5 => {
                    write_key_class(&mut classes, order, *device);
                    let (use_type, attachment, name) = if *device == 3 {
                        (2, 2, "RiceWM Virtual Core Keyboard")
                    } else {
                        (4, 3, "RiceWM Browser Keyboard")
                    };
                    write_device(
                        &mut infos, order, *device, use_type, attachment, name, 1, &classes,
                    );
                }
                _ => unreachable!(),
            }
        }
        let mut output = fixed_reply_header(
            order,
            sequence,
            0,
            u32::try_from(infos.len() / 4).unwrap_or(u32::MAX),
        );
        write_u16_vec(
            order,
            &mut output,
            u16::try_from(devices.len()).unwrap_or(u16::MAX),
        );
        output.resize(32, 0);
        output.extend_from_slice(&infos);
        Ok(output)
    }
}
