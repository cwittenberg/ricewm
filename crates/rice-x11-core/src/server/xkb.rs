//! Xkb operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn xkb_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            // UseExtension: XKB 1.0 is the stable protocol consumed by
            // libX11, xkbcommon-x11, and every supported GTK generation.
            0 => {
                if body.len() == 4 {
                    let mut output = fixed_reply_header(order, sequence, 1, 0);
                    write_u16_vec(order, &mut output, 1);
                    write_u16_vec(order, &mut output, 0);
                    output.resize(32, 0);
                    Ok(output)
                } else {
                    Err(bad_length())
                }
            }
            // SelectEvents is a void request. Keep the event-type selection
            // per client so XKB StateNotify can accompany core/XI2 key events.
            1 => {
                if body.len() < 12 {
                    Err(bad_length())
                } else {
                    let affect = read_u16(order, &body[2..4]);
                    let clear = read_u16(order, &body[4..6]);
                    let select_all = read_u16(order, &body[6..8]);
                    let client = self
                        .clients
                        .get_mut(&handle.0)
                        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
                    client.xkb_event_mask =
                        (client.xkb_event_mask & !affect) | (select_all & affect);
                    client.xkb_event_mask &= !clear;
                    Ok(Vec::new())
                }
            }
            4 => {
                if body.len() == 4 {
                    Ok(xkb_get_state_reply(order, sequence, self.input_state))
                } else {
                    Err(bad_length())
                }
            }
            6 => {
                if body.len() == 4 {
                    Ok(xkb_get_controls_reply(order, sequence))
                } else {
                    Err(bad_length())
                }
            }
            8 => {
                if body.len() < 6 {
                    Err(bad_length())
                } else {
                    Ok(self.xkb_get_map_reply(order, sequence, body))
                }
            }
            10 => Ok(xkb_get_compat_map_reply(order, sequence)),
            13 => Ok(xkb_get_indicator_map_reply(order, sequence)),
            15 => {
                if body.len() < 12 {
                    Err(bad_length())
                } else {
                    Ok(xkb_get_named_indicator_reply(
                        order,
                        sequence,
                        read_u32(order, &body[8..12]),
                    ))
                }
            }
            17 => Ok(self.xkb_get_names_reply(order, sequence)),
            21 => {
                if body.len() < 12 {
                    Err(bad_length())
                } else {
                    let change = read_u32(order, &body[4..8]) & 0x1f;
                    let value = read_u32(order, &body[8..12]) & 0x1f;
                    let client = self
                        .clients
                        .get_mut(&handle.0)
                        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
                    client.xkb_client_flags =
                        (client.xkb_client_flags & !change) | (value & change);
                    Ok(xkb_per_client_flags_reply(
                        order,
                        sequence,
                        client.xkb_client_flags,
                    ))
                }
            }
            24 => Ok(xkb_get_device_info_reply(order, sequence)),
            // GetGeometry/GetKbdByName/ListComponents/GetDeviceInfo-style
            // probes that tolerate an empty fixed reply.
            12 | 19 | 22 | 23 | 101 => Ok(xkb_minimal_reply(order, sequence)),
            // Mutating requests are acknowledged as void. The core
            // ChangeKeyboardMapping/ChangeModifierMapping paths remain the
            // authoritative map mutation APIs for this static US keymap.
            3 | 5 | 7 | 9 | 11 | 14 | 16 | 18 | 20 | 25 => Ok(Vec::new()),
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|error| extension_error(error, minor))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn xkb_get_map_reply(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Vec<u8> {
        const KEY_TYPES: u16 = 1 << 0;
        const KEY_SYMS: u16 = 1 << 1;
        const MODIFIER_MAP: u16 = 1 << 2;
        const EXPLICIT_COMPONENTS: u16 = 1 << 3;
        const KEY_ACTIONS: u16 = 1 << 4;
        const VIRTUAL_MODS: u16 = 1 << 6;
        const VIRTUAL_MOD_MAP: u16 = 1 << 7;
        const SUPPORTED: u16 = KEY_TYPES
            | KEY_SYMS
            | MODIFIER_MAP
            | EXPLICIT_COMPONENTS
            | KEY_ACTIONS
            | VIRTUAL_MODS
            | VIRTUAL_MOD_MAP;

        #[derive(Clone)]
        struct KeyData {
            type_index: u8,
            width: u8,
            syms: Vec<u32>,
            modifier: u8,
            lock: bool,
        }

        let full = read_u16(order, &body[2..4]);
        let partial = read_u16(order, &body[4..6]);
        let present = (full | partial) & SUPPORTED;
        let include_types = present & KEY_TYPES != 0;
        let include_syms = present & KEY_SYMS != 0;
        let include_actions = present & KEY_ACTIONS != 0;
        let include_vmods = present & VIRTUAL_MODS != 0;
        let include_modmap = present & MODIFIER_MAP != 0;
        let include_virtual_map = present & VIRTUAL_MOD_MAP != 0;

        let mut keys = Vec::with_capacity(248);
        let mut total_syms = 0u16;
        let mut total_actions = 0u16;
        let mut modifier_entries = Vec::new();
        for keycode in 8..=255 {
            let default = default_keysyms(keycode);
            let row = self.keysyms.get(&keycode);
            let first = row
                .and_then(|values| values.first())
                .copied()
                .unwrap_or(default[0]);
            let second = row
                .and_then(|values| values.get(1))
                .copied()
                .unwrap_or(default[1]);
            let syms = if first == 0 && second == 0 {
                Vec::new()
            } else if second == 0 || first == second {
                vec![first]
            } else {
                vec![first, second]
            };
            let width = u8::try_from(syms.len()).unwrap_or(0);
            let type_index = if width < 2 {
                0
            } else if (0x61..=0x7a).contains(&first) && (0x41..=0x5a).contains(&second) {
                2
            } else if (79..=91).contains(&keycode) || matches!(keycode, 63 | 77 | 82 | 86 | 106) {
                3
            } else {
                1
            };
            let modifier = xkb_modifier_for_keycode(keycode);
            if modifier != 0 {
                modifier_entries.push((keycode, modifier));
                total_actions = total_actions.saturating_add(u16::from(width.max(1)));
            }
            total_syms = total_syms.saturating_add(u16::try_from(syms.len()).unwrap_or(u16::MAX));
            keys.push(KeyData {
                type_index,
                width,
                syms,
                modifier,
                lock: matches!(keycode, 66 | 77),
            });
        }

        // Four required types: ONE_LEVEL, TWO_LEVEL, ALPHABETIC, KEYPAD.
        let type_bytes = if include_types { 8 + 16 * 3 } else { 0 };
        let sym_bytes = if include_syms {
            keys.iter()
                .map(|key| 8 + key.syms.len().saturating_mul(4))
                .sum()
        } else {
            0
        };
        let action_count_bytes = 248usize;
        let action_bytes = if include_actions {
            action_count_bytes + usize::from(total_actions).saturating_mul(8)
        } else {
            0
        };
        let modmap_raw = modifier_entries.len().saturating_mul(2);
        let modmap_bytes = if include_modmap {
            x11::pad4(modmap_raw)
        } else {
            0
        };
        // Alt, NumLock, Super, ScrollLock.
        let vmod_bytes = if include_vmods { 4 } else { 0 };
        // Seven physical keys carry one or more virtual modifier bindings.
        let vmod_entries = xkb_virtual_modifier_map();
        let vmodmap_bytes = if include_virtual_map {
            vmod_entries.len().saturating_mul(4)
        } else {
            0
        };
        let extra =
            type_bytes + sym_bytes + action_bytes + modmap_bytes + vmod_bytes + vmodmap_bytes;
        let total = 40usize.saturating_add(extra);
        let mut output = vec![0; total];
        output[0] = 1;
        output[1] = 1;
        write_u16_at(order, &mut output[2..4], sequence.0);
        write_u32_at(
            order,
            &mut output[4..8],
            u32::try_from((total - 32) / 4).unwrap_or(u32::MAX),
        );
        output[10] = 8;
        output[11] = 255;
        write_u16_at(order, &mut output[12..14], present);
        if include_types {
            output[15] = 4;
            output[16] = 4;
        }
        if include_syms {
            output[17] = 8;
            write_u16_at(order, &mut output[18..20], total_syms);
            output[20] = 248;
        }
        if include_actions {
            output[21] = 8;
            write_u16_at(order, &mut output[22..24], total_actions);
            output[24] = 248;
        }
        if present & EXPLICIT_COMPONENTS != 0 {
            output[28] = 8;
        }
        if include_modmap {
            output[31] = 8;
            output[32] = 248;
            output[33] = u8::try_from(modifier_entries.len()).unwrap_or(u8::MAX);
        }
        if include_virtual_map {
            output[34] = 8;
            output[35] = 248;
            output[36] = u8::try_from(vmod_entries.len()).unwrap_or(u8::MAX);
        }
        if include_vmods {
            write_u16_at(order, &mut output[38..40], 0x000f);
        }

        let mut offset = 40usize;
        if include_types {
            // ONE_LEVEL.
            output[offset + 4] = 1;
            offset += 8;
            // The remaining required types all use Shift to select level 2.
            for _ in 0..3 {
                output[offset] = 1;
                output[offset + 1] = 1;
                output[offset + 4] = 2;
                output[offset + 5] = 1;
                output[offset + 8] = 1;
                output[offset + 9] = 1;
                output[offset + 10] = 1;
                output[offset + 11] = 1;
                offset += 16;
            }
        }
        if include_syms {
            for key in &keys {
                output[offset] = key.type_index;
                output[offset + 4] = u8::from(key.width != 0);
                output[offset + 5] = key.width;
                write_u16_at(
                    order,
                    &mut output[offset + 6..offset + 8],
                    u16::try_from(key.syms.len()).unwrap_or(u16::MAX),
                );
                offset += 8;
                for &keysym in &key.syms {
                    write_u32_at(order, &mut output[offset..offset + 4], keysym);
                    offset += 4;
                }
            }
        }
        if include_actions {
            for key in &keys {
                output[offset] = if key.modifier == 0 {
                    0
                } else {
                    key.width.max(1)
                };
                offset += 1;
            }
            for key in &keys {
                if key.modifier == 0 {
                    continue;
                }
                for _ in 0..key.width.max(1) {
                    output[offset] = if key.lock { 3 } else { 1 };
                    output[offset + 2] = key.modifier;
                    output[offset + 3] = key.modifier;
                    offset += 8;
                }
            }
        }
        if include_vmods {
            output[offset..offset + 4].copy_from_slice(&[0x08, 0x10, 0x40, 0x20]);
            offset += 4;
        }
        if include_modmap {
            for &(keycode, modifier) in &modifier_entries {
                output[offset] = keycode;
                output[offset + 1] = modifier;
                offset += 2;
            }
            offset += x11::pad4(modmap_raw) - modmap_raw;
        }
        if include_virtual_map {
            for &(keycode, vmods) in &vmod_entries {
                output[offset] = keycode;
                write_u16_at(order, &mut output[offset + 2..offset + 4], vmods);
                offset += 4;
            }
        }
        debug_assert_eq!(offset, total);
        output
    }

    pub(super) fn xkb_get_names_reply(&mut self, order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
        const WHICH: u32 = 0x0000_0af5;
        let component_names = [
            "evdev+aliases(qwerty)",
            "pc+us+inet(evdev)",
            "complete",
            "complete",
        ];
        let type_names = ["ONE_LEVEL", "TWO_LEVEL", "ALPHABETIC", "KEYPAD"];
        let level_names = ["Base", "Base", "Shift", "Base", "Shift", "Base", "Shift"];
        let virtual_names = ["Alt", "NumLock", "Super", "ScrollLock"];
        let extra = 16 + 16 + 4 + level_names.len() * 4 + virtual_names.len() * 4 + 248 * 4;
        let mut output = vec![0; 32 + extra];
        output[0] = 1;
        output[1] = 1;
        write_u16_at(order, &mut output[2..4], sequence.0);
        write_u32_at(
            order,
            &mut output[4..8],
            u32::try_from(extra / 4).unwrap_or(u32::MAX),
        );
        write_u32_at(order, &mut output[8..12], WHICH);
        output[12] = 8;
        output[13] = 255;
        output[14] = 4;
        write_u16_at(order, &mut output[16..18], 0x000f);
        output[18] = 8;
        output[19] = 248;
        write_u16_at(
            order,
            &mut output[26..28],
            u16::try_from(level_names.len()).unwrap_or(u16::MAX),
        );

        let mut offset = 32usize;
        for name in component_names.iter().chain(type_names.iter()) {
            let atom = self.atoms.intern(name, false).0;
            write_u32_at(order, &mut output[offset..offset + 4], atom);
            offset += 4;
        }
        output[offset..offset + 4].copy_from_slice(&[1, 2, 2, 2]);
        offset += 4;
        for name in level_names.iter().chain(virtual_names.iter()) {
            let atom = self.atoms.intern(name, false).0;
            write_u32_at(order, &mut output[offset..offset + 4], atom);
            offset += 4;
        }
        for keycode in 8..=255u16 {
            let name = format!("K{keycode:03}");
            output[offset..offset + 4].copy_from_slice(name.as_bytes());
            offset += 4;
        }
        debug_assert_eq!(offset, output.len());
        output
    }
}
