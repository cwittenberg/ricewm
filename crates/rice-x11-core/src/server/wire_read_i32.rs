//! Protocol encoding and raster helpers.
use super::*;



pub(super) fn read_i32(byte_order: ClientByteOrder, bytes: &[u8]) -> i32 {
    let value = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match byte_order {
        ClientByteOrder::LittleEndian => i32::from_le_bytes(value),
        ClientByteOrder::BigEndian => i32::from_be_bytes(value),
    }
}

pub(super) fn read_u32(byte_order: ClientByteOrder, bytes: &[u8]) -> u32 {
    let value = [bytes[0], bytes[1], bytes[2], bytes[3]];
    match byte_order {
        ClientByteOrder::LittleEndian => u32::from_le_bytes(value),
        ClientByteOrder::BigEndian => u32::from_be_bytes(value),
    }
}

pub(super) fn read_sync_i64(byte_order: ClientByteOrder, bytes: &[u8]) -> i64 {
    let high = i64::from(read_i32(byte_order, &bytes[0..4]));
    let low = i64::from(read_u32(byte_order, &bytes[4..8]));
    (high << 32) | low
}

pub(super) fn decode_u32_request(order: ClientByteOrder, body: &[u8]) -> Result<u32, ProtocolError> {
    if body.len() != 4 {
        return Err(bad_length());
    }
    Ok(read_u32(order, body))
}

pub(super) fn decode_sync_counter_value(
    order: ClientByteOrder,
    body: &[u8],
) -> Result<(u32, i64), ProtocolError> {
    if body.len() != 12 {
        return Err(bad_length());
    }
    Ok((read_u32(order, body), read_sync_i64(order, &body[4..12])))
}

pub(super) fn decode_sync_alarm_attributes(
    order: ClientByteOrder,
    body: &[u8],
) -> Result<(u32, x11sync::AlarmAttributes), ProtocolError> {
    if body.len() < 8 {
        return Err(bad_length());
    }
    let id = read_u32(order, body);
    let mask = read_u32(order, &body[4..8]);
    if mask
        & !(x11sync::CA_COUNTER
            | x11sync::CA_VALUE_TYPE
            | x11sync::CA_VALUE
            | x11sync::CA_TEST_TYPE
            | x11sync::CA_DELTA
            | x11sync::CA_EVENTS)
        != 0
    {
        return Err(ProtocolError::new(x11::error::BAD_VALUE, mask));
    }
    let mut offset = 8;
    let mut attributes = x11sync::AlarmAttributes::default();
    for (bit, size) in [
        (x11sync::CA_COUNTER, 4usize),
        (x11sync::CA_VALUE_TYPE, 4),
        (x11sync::CA_VALUE, 8),
        (x11sync::CA_TEST_TYPE, 4),
        (x11sync::CA_DELTA, 8),
        (x11sync::CA_EVENTS, 4),
    ] {
        if mask & bit == 0 {
            continue;
        }
        let bytes = body.get(offset..offset + size).ok_or_else(bad_length)?;
        match bit {
            x11sync::CA_COUNTER => attributes.counter = Some(read_u32(order, bytes)),
            x11sync::CA_VALUE_TYPE => attributes.value_type = Some(read_u32(order, bytes)),
            x11sync::CA_VALUE => attributes.value = Some(read_sync_i64(order, bytes)),
            x11sync::CA_TEST_TYPE => attributes.test_type = Some(read_u32(order, bytes)),
            x11sync::CA_DELTA => attributes.delta = Some(read_sync_i64(order, bytes)),
            x11sync::CA_EVENTS => {
                let value = read_u32(order, bytes);
                if value > 1 {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, value));
                }
                attributes.events = Some(value != 0);
            }
            _ => unreachable!(),
        }
        offset += size;
    }
    if offset != body.len() {
        return Err(bad_length());
    }
    Ok((id, attributes))
}

pub(super) fn read_u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

pub(super) fn read_i16_le(bytes: &[u8]) -> i16 {
    i16::from_le_bytes([bytes[0], bytes[1]])
}

pub(super) fn write_u16_at(order: ClientByteOrder, output: &mut [u8], value: u16) {
    let bytes = match order {
        ClientByteOrder::LittleEndian => value.to_le_bytes(),
        ClientByteOrder::BigEndian => value.to_be_bytes(),
    };
    output.copy_from_slice(&bytes);
}

pub(super) fn write_u16_vec(order: ClientByteOrder, output: &mut Vec<u8>, value: u16) {
    let bytes = match order {
        ClientByteOrder::LittleEndian => value.to_le_bytes(),
        ClientByteOrder::BigEndian => value.to_be_bytes(),
    };
    output.extend_from_slice(&bytes);
}

pub(super) fn write_u32_vec(order: ClientByteOrder, output: &mut Vec<u8>, value: u32) {
    let bytes = match order {
        ClientByteOrder::LittleEndian => value.to_le_bytes(),
        ClientByteOrder::BigEndian => value.to_be_bytes(),
    };
    output.extend_from_slice(&bytes);
}

pub(super) fn write_u32_at(order: ClientByteOrder, output: &mut [u8], value: u32) {
    let bytes = match order {
        ClientByteOrder::LittleEndian => value.to_le_bytes(),
        ClientByteOrder::BigEndian => value.to_be_bytes(),
    };
    output.copy_from_slice(&bytes);
}

pub(super) fn default_modifier_mapping() -> (u8, Vec<u8>) {
    (
        2,
        vec![
            50, 62, // Shift
            66, 0, // Lock
            37, 105, // Control
            64, 108, // Mod1 / Alt
            77, 0, // Mod2 / NumLock
            78, 0, // Mod3 / ScrollLock
            133, 134, // Mod4 / Super
            0, 0, // Mod5
        ],
    )
}

pub(super) const fn xkb_modifier_for_keycode(keycode: u8) -> u8 {
    match keycode {
        50 | 62 => 0x01,
        66 => 0x02,
        37 | 105 => 0x04,
        64 | 108 => 0x08,
        77 => 0x10,
        78 => 0x20,
        133 | 134 => 0x40,
        _ => 0,
    }
}

pub(super) const fn key_state_after_transition(pressed: bool, keycode: u8, state: u16) -> u16 {
    let modifier = xkb_modifier_for_keycode(keycode) as u16;
    if modifier == 0 {
        return state;
    }
    if matches!(keycode, 66 | 77 | 78) {
        if pressed {
            return state ^ modifier;
        }
        return state;
    }
    if pressed {
        state | modifier
    } else {
        state & !modifier
    }
}

pub(super) fn xkb_virtual_modifier_map() -> [(u8, u16); 6] {
    [
        (64, 1 << 0),
        (108, 1 << 0),
        (77, 1 << 1),
        (133, 1 << 2),
        (134, 1 << 2),
        (78, 1 << 3),
    ]
}

pub(super) fn xkb_get_state_reply(order: ClientByteOrder, sequence: SequenceNumber, state: u16) -> Vec<u8> {
    let mut output = fixed_reply_32(order, sequence, 1);
    let modifiers = u8::try_from(state & 0x00ff).unwrap_or(0);
    output[8] = modifiers;
    output[9] = modifiers;
    output
}

pub(super) fn xkb_get_controls_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = vec![0; 92];
    output[0] = 1;
    output[1] = 1;
    write_u16_at(order, &mut output[2..4], sequence.0);
    write_u32_at(order, &mut output[4..8], 15);
    output[9] = 1;
    write_u16_at(order, &mut output[20..22], 500);
    write_u16_at(order, &mut output[22..24], 33);
    write_u32_at(order, &mut output[56..60], 1);
    output[60..92].fill(0xff);
    output
}

pub(super) fn xkb_get_compat_map_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = vec![0; 48];
    output[0] = 1;
    output[1] = 1;
    write_u16_at(order, &mut output[2..4], sequence.0);
    write_u32_at(order, &mut output[4..8], 4);
    output[8] = 0x0f;
    output[32..48].copy_from_slice(&[
        0, 0, 0, 0, 0x80, 0x80, 0, 0, 0x80, 0x80, 0, 0, 0x80, 0x80, 0, 0,
    ]);
    output
}

pub(super) fn xkb_get_indicator_map_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = vec![0; 32 + 32 * 12];
    output[0] = 1;
    output[1] = 1;
    write_u16_at(order, &mut output[2..4], sequence.0);
    write_u32_at(order, &mut output[4..8], 96);
    write_u32_at(order, &mut output[8..12], u32::MAX);
    write_u32_at(order, &mut output[12..16], 0x0000_07ff);
    output[16] = 32;
    output
}

pub(super) fn xkb_get_named_indicator_reply(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    indicator: u32,
) -> Vec<u8> {
    let mut output = fixed_reply_32(order, sequence, 1);
    write_u32_at(order, &mut output[8..12], indicator);
    output[28] = 1;
    output
}

pub(super) fn xkb_per_client_flags_reply(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    value: u32,
) -> Vec<u8> {
    let mut output = fixed_reply_32(order, sequence, 1);
    write_u32_at(order, &mut output[8..12], 0x1f);
    write_u32_at(order, &mut output[12..16], value & 0x1f);
    output
}

pub(super) fn xkb_get_device_info_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = vec![0; 36];
    output[0] = 1;
    output[1] = 1;
    write_u16_at(order, &mut output[2..4], sequence.0);
    write_u32_at(order, &mut output[4..8], 1);
    output
}

pub(super) fn xkb_minimal_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    fixed_reply_32(order, sequence, 1)
}

pub(super) fn validate_xinput_device(device: u16, allow_wildcards: bool) -> Result<(), ProtocolError> {
    if (allow_wildcards && device <= 1) || (2..=5).contains(&device) {
        Ok(())
    } else {
        Err(ProtocolError::new(XINPUT_FIRST_ERROR, u32::from(device)))
    }
}

pub(super) fn pixels_to_millimetres(pixels: u16) -> u16 {
    let millimetres = (u32::from(pixels) * 254 + 480) / 960;
    u16::try_from(millimetres.max(1)).unwrap_or(u16::MAX)
}

pub(super) fn encode_setup_success(
    byte_order: ClientByteOrder,
    resource_id_base: u32,
    width_px: u16,
    height_px: u16,
) -> Result<Vec<u8>, ()> {
    let mut output = Vec::new();
    x11::write_setup_success(
        &mut output,
        byte_order,
        SetupSuccess {
            protocol_major: 11,
            protocol_minor: 0,
            release_number: 1,
            resource_id_base,
            resource_id_mask: RESOURCE_ID_MASK,
            motion_buffer_size: 0,
            maximum_request_length: u16::MAX,
            image_byte_order: byte_order,
            bitmap_format_bit_order: byte_order,
            bitmap_format_scanline_unit: 32,
            bitmap_format_scanline_pad: 32,
            min_keycode: 8,
            max_keycode: 255,
            vendor: "RiceWM",
            root: Screen {
                root: ROOT_WINDOW,
                default_colormap: ROOT_COLORMAP,
                white_pixel: 0x00ff_ffff,
                black_pixel: 0,
                current_input_masks: 0,
                width_px,
                height_px,
                width_mm: pixels_to_millimetres(width_px),
                height_mm: pixels_to_millimetres(height_px),
                min_installed_maps: 1,
                max_installed_maps: 1,
                root_visual: ROOT_VISUAL,
                argb_visual: ARGB_VISUAL,
                root_depth: 24,
            },
        },
    )
    .map_err(|_| ())?;
    Ok(output)
}

pub(super) fn encode_error(
    byte_order: ClientByteOrder,
    sequence: SequenceNumber,
    error_code: u8,
    bad_value: u32,
    minor_opcode: u16,
    major_opcode: u8,
) -> Result<Vec<u8>, ()> {
    let mut output = Vec::with_capacity(32);
    x11::write_error(
        &mut output,
        byte_order,
        sequence,
        error_code,
        bad_value,
        minor_opcode,
        major_opcode,
    )
    .map_err(|_| ())?;
    Ok(output)
}

pub(super) fn fixed_reply_header(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    data: u8,
    length: u32,
) -> Vec<u8> {
    let mut output = Vec::with_capacity(32);
    output.push(1);
    output.push(data);
    let mut sequence_bytes = [0; 2];
    write_u16_at(order, &mut sequence_bytes, sequence.0);
    output.extend_from_slice(&sequence_bytes);
    write_u32_vec(order, &mut output, length);
    output
}

pub(super) fn fixed_reply_32(order: ClientByteOrder, sequence: SequenceNumber, data: u8) -> Vec<u8> {
    let mut output = fixed_reply_header(order, sequence, data, 0);
    output.resize(32, 0);
    output
}

pub(super) fn encode_keyboard_control_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = fixed_reply_header(order, sequence, 1, 5);
    write_u32_vec(order, &mut output, 0);
    output.push(0);
    output.push(50);
    let mut value = [0; 2];
    write_u16_at(order, &mut value, 400);
    output.extend_from_slice(&value);
    write_u16_at(order, &mut value, 100);
    output.extend_from_slice(&value);
    output.extend_from_slice(&[0, 0]);
    output.extend_from_slice(&[0xff; 32]);
    output
}

pub(super) fn encode_pointer_control_reply(order: ClientByteOrder, sequence: SequenceNumber) -> Vec<u8> {
    let mut output = fixed_reply_header(order, sequence, 0, 0);
    let mut value = [0; 2];
    for setting in [2, 1, 4] {
        write_u16_at(order, &mut value, setting);
        output.extend_from_slice(&value);
    }
    output.resize(32, 0);
    output
}

pub(super) fn validate_bell(percent: u8) -> Result<Vec<u8>, ProtocolError> {
    let signed = i8::from_ne_bytes([percent]);
    if !(-100..=100).contains(&signed) {
        return Err(ProtocolError::new(
            x11::error::BAD_VALUE,
            u32::from(percent),
        ));
    }
    Ok(Vec::new())
}

pub(super) fn encode_get_input_focus(
    byte_order: ClientByteOrder,
    sequence: SequenceNumber,
    focus: ResourceId,
) -> Result<Vec<u8>, ()> {
    let mut output = Vec::with_capacity(32);
    x11::write_get_input_focus_reply(&mut output, byte_order, sequence, focus, 0)
        .map_err(|_| ())?;
    Ok(output)
}

pub(super) fn encode_grab_reply(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    status: u8,
) -> Result<Vec<u8>, ProtocolError> {
    let mut output = Vec::with_capacity(32);
    x11::write_grab_reply(&mut output, order, sequence, status).map_err(|_| encoding_error())?;
    Ok(output)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn encode_selection_notify_event(
    output: &mut Vec<u8>,
    sequence: SequenceNumber,
    order: ClientByteOrder,
    time: u32,
    requestor: ResourceId,
    selection: AtomId,
    target: AtomId,
    property: AtomId,
) {
    output.push(31);
    output.push(0);
    let mut sequence_bytes = [0; 2];
    write_u16_at(order, &mut sequence_bytes, sequence.0);
    output.extend_from_slice(&sequence_bytes);
    write_u32_vec(order, output, time);
    write_u32_vec(order, output, requestor.0);
    write_u32_vec(order, output, selection.0);
    write_u32_vec(order, output, target.0);
    write_u32_vec(order, output, property.0);
    output.resize(32, 0);
}

pub(super) fn implemented_extension_names() -> [&'static str; 14] {
    [
        "RANDR",
        "RENDER",
        "BIG-REQUESTS",
        "XKEYBOARD",
        "XInputExtension",
        "Generic Event Extension",
        "XFIXES",
        "SHAPE",
        "SYNC",
        "DAMAGE",
        "Composite",
        "XINERAMA",
        "XTEST",
        "XC-MISC",
    ]
}

pub(super) fn extension_metadata(name: &str) -> Option<(u8, u8, u8)> {
    match name {
        "RANDR" => Some((RANDR_MAJOR_OPCODE, RANDR_FIRST_EVENT, RANDR_FIRST_ERROR)),
        "RENDER" => Some((RENDER_MAJOR_OPCODE, 0, 0)),
        "BIG-REQUESTS" => Some((BIG_REQUESTS_MAJOR_OPCODE, 0, 0)),
        "XKEYBOARD" => Some((XKB_MAJOR_OPCODE, XKB_FIRST_EVENT, XKB_FIRST_ERROR)),
        "XInputExtension" => Some((XINPUT_MAJOR_OPCODE, XINPUT_FIRST_EVENT, XINPUT_FIRST_ERROR)),
        "Generic Event Extension" => Some((GENERIC_EVENT_MAJOR_OPCODE, 0, 0)),
        "XFIXES" => Some((XFIXES_MAJOR_OPCODE, XFIXES_FIRST_EVENT, XFIXES_FIRST_ERROR)),
        "SHAPE" => Some((SHAPE_MAJOR_OPCODE, SHAPE_FIRST_EVENT, 0)),
        "SYNC" => Some((SYNC_MAJOR_OPCODE, SYNC_FIRST_EVENT, SYNC_FIRST_ERROR)),
        "DAMAGE" => Some((DAMAGE_MAJOR_OPCODE, DAMAGE_FIRST_EVENT, DAMAGE_FIRST_ERROR)),
        "Composite" => Some((COMPOSITE_MAJOR_OPCODE, 0, 0)),
        "XINERAMA" => Some((XINERAMA_MAJOR_OPCODE, 0, 0)),
        "XTEST" => Some((XTEST_MAJOR_OPCODE, 0, 0)),
        "XC-MISC" => Some((XCMISC_MAJOR_OPCODE, 0, 0)),
        _ => None,
    }
}

pub(super) fn encode_get_property_reply(
    byte_order: ClientByteOrder,
    sequence: SequenceNumber,
    format: u8,
    r#type: AtomId,
    bytes_after: u32,
    value: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    let value_len = if format == 0 {
        0
    } else {
        u32::try_from(value.len() / usize::from(format / 8)).unwrap_or(u32::MAX)
    };
    let mut output = Vec::with_capacity(32 + x11::pad4(value.len()));
    x11::write_get_property_reply(
        &mut output,
        byte_order,
        sequence,
        x11::GetPropertyReply {
            format,
            r#type,
            bytes_after,
            value_len,
            value,
        },
    )
    .map_err(|_| encoding_error())?;
    Ok(output)
}

pub(super) fn encode_list_properties(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    atoms: &[AtomId],
) -> Vec<u8> {
    let count = u16::try_from(atoms.len()).unwrap_or(u16::MAX);
    let mut output = Vec::with_capacity(32 + usize::from(count) * 4);
    output.push(1);
    output.push(0);
    let mut sequence_bytes = [0; 2];
    write_u16_at(order, &mut sequence_bytes, sequence.0);
    output.extend_from_slice(&sequence_bytes);
    write_u32_vec(order, &mut output, u32::from(count));
    let mut count_bytes = [0; 2];
    write_u16_at(order, &mut count_bytes, count);
    output.extend_from_slice(&count_bytes);
    output.resize(32, 0);
    for atom in &atoms[..usize::from(count)] {
        write_u32_vec(order, &mut output, atom.0);
    }
    output
}

pub(super) fn bad_length() -> ProtocolError {
    ProtocolError::new(x11::error::BAD_LENGTH, 0)
}

pub(super) fn encoding_error() -> ProtocolError {
    ProtocolError::new(x11::error::BAD_IMPLEMENTATION, 0)
}
