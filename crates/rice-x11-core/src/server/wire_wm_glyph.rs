//! Protocol encoding and raster helpers.
use super::*;



pub(super) const fn wm_glyph(byte: u8) -> [u8; 7] {
    match byte.to_ascii_uppercase() {
        b'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        b'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        b'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        b'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'I' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x1f],
        b'J' => [0x01, 0x01, 0x01, 0x01, 0x11, 0x11, 0x0e],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        b'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        b'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        b'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        b'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        b'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        b'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        b'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        b'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        b'0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        b'1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        b'3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        b'4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        b'5' => [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        b'6' => [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        b'7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        b'9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        b':' => [0x00, 0x04, 0x04, 0x00, 0x04, 0x04, 0x00],
        b'/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        b'-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        b'>' => [0x10, 0x08, 0x04, 0x02, 0x04, 0x08, 0x10],
        b'<' => [0x01, 0x02, 0x04, 0x08, 0x04, 0x02, 0x01],
        b'+' => [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
        b'=' => [0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b'[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        b']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        b'{' => [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
        b'}' => [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
        b',' => [0x00, 0x00, 0x00, 0x00, 0x06, 0x04, 0x08],
        b';' => [0x00, 0x04, 0x04, 0x00, 0x06, 0x04, 0x08],
        b'\'' => [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00],
        b'"' => [0x0a, 0x0a, 0x14, 0x00, 0x00, 0x00, 0x00],
        b'!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        b'&' => [0x0c, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0d],
        b'@' => [0x0e, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0e],
        b'#' => [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
        b'$' => [0x04, 0x0f, 0x14, 0x0e, 0x05, 0x1e, 0x04],
        b'%' => [0x19, 0x1a, 0x02, 0x04, 0x08, 0x0b, 0x13],
        b'^' => [0x04, 0x0a, 0x11, 0x00, 0x00, 0x00, 0x00],
        b'*' => [0x00, 0x15, 0x0e, 0x1f, 0x0e, 0x15, 0x00],
        b'\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        b'|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'`' => [0x08, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'~' => [0x00, 0x00, 0x09, 0x16, 0x00, 0x00, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        b' ' | 0 => [0; 7],
        _ => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
    }
}

pub(super) fn default_keysyms(keycode: u8) -> [u32; 2] {
    const DIGITS: [[u32; 2]; 10] = [
        [0x31, 0x21],
        [0x32, 0x40],
        [0x33, 0x23],
        [0x34, 0x24],
        [0x35, 0x25],
        [0x36, 0x5e],
        [0x37, 0x26],
        [0x38, 0x2a],
        [0x39, 0x28],
        [0x30, 0x29],
    ];
    const LETTER_ROWS: &[(u8, &[u8])] =
        &[(24, b"qwertyuiop"), (38, b"asdfghjkl"), (52, b"zxcvbnm")];
    if (10..20).contains(&keycode) {
        return DIGITS[usize::from(keycode - 10)];
    }
    for &(start, letters) in LETTER_ROWS {
        let Some(offset) = keycode.checked_sub(start) else {
            continue;
        };
        if let Some(letter) = letters.get(usize::from(offset)).copied() {
            return [u32::from(letter), u32::from(letter.to_ascii_uppercase())];
        }
    }
    match keycode {
        9 => [0xff1b, 0xff1b],
        20 => [u32::from(b'-'), u32::from(b'_')],
        21 => [u32::from(b'='), u32::from(b'+')],
        22 => [0xff08, 0xff08],
        23 => [0xff09, 0xfe20],
        34 => [u32::from(b'['), u32::from(b'{')],
        35 => [u32::from(b']'), u32::from(b'}')],
        36 => [0xff0d, 0xff0d],
        37 => [0xffe3, 0xffe3],
        47 => [u32::from(b';'), u32::from(b':')],
        48 => [u32::from(b'\''), u32::from(b'"')],
        49 => [u32::from(b'`'), u32::from(b'~')],
        50 => [0xffe1, 0xffe1],
        51 => [u32::from(b'\\'), u32::from(b'|')],
        59 => [u32::from(b','), u32::from(b'<')],
        60 => [u32::from(b'.'), u32::from(b'>')],
        61 => [u32::from(b'/'), u32::from(b'?')],
        62 => [0xffe2, 0xffe2],
        63 => [0xffaa, 0xffaa],
        64 => [0xffe9, 0xffe9],
        65 => [u32::from(b' '), u32::from(b' ')],
        66 => [0xffe5, 0xffe5],
        67..=76 => {
            let function = 0xffbe + u32::from(keycode - 67);
            [function, function]
        }
        77 => [0xff7f, 0xff7f],
        78 => [0xff14, 0xff14],
        79 => [0xffb7, 0xffb7],
        80 => [0xffb8, 0xffb8],
        81 => [0xffb9, 0xffb9],
        82 => [0xffad, 0xffad],
        83 => [0xffb4, 0xffb4],
        84 => [0xffb5, 0xffb5],
        85 => [0xffb6, 0xffb6],
        86 => [0xffab, 0xffab],
        87 => [0xffb1, 0xffb1],
        88 => [0xffb2, 0xffb2],
        89 => [0xffb3, 0xffb3],
        90 => [0xffb0, 0xffb0],
        91 => [0xffae, 0xffae],
        94 => [u32::from(b'<'), u32::from(b'>')],
        95 => [0xffc8, 0xffc8],
        96 => [0xffc9, 0xffc9],
        104 => [0xff8d, 0xff8d],
        105 => [0xffe4, 0xffe4],
        106 => [0xffaf, 0xffaf],
        107 => [0xff61, 0xff61],
        108 => [0xffea, 0xffea],
        110 => [0xff50, 0xff50],
        111 => [0xff52, 0xff52],
        112 => [0xff55, 0xff55],
        113 => [0xff51, 0xff51],
        114 => [0xff53, 0xff53],
        115 => [0xff57, 0xff57],
        116 => [0xff54, 0xff54],
        117 => [0xff56, 0xff56],
        118 => [0xff63, 0xff63],
        119 => [0xffff, 0xffff],
        133 => [0xffeb, 0xffeb],
        134 => [0xffec, 0xffec],
        _ => [0, 0],
    }
}

pub(super) fn font_pattern_matches(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let mut remainder = name;
    let mut first = true;
    for part in pattern.split('*') {
        if part.is_empty() {
            continue;
        }
        let Some(index) = remainder.find(part) else {
            return false;
        };
        if first && !pattern.starts_with('*') && index != 0 {
            return false;
        }
        remainder = &remainder[index + part.len()..];
        first = false;
    }
    pattern.ends_with('*') || remainder.is_empty()
}

pub(super) fn matching_builtin_fonts(pattern: &str, max_names: u16) -> Vec<String> {
    ["fixed", "cursor"]
        .into_iter()
        .filter(|name| font_pattern_matches(pattern, name))
        .take(usize::from(max_names))
        .map(str::to_owned)
        .collect()
}

pub(super) fn encode_list_fonts_reply(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    names: &[String],
) -> Vec<u8> {
    let mut payload = Vec::new();
    for name in names {
        let length = name.len().min(255);
        payload.push(u8::try_from(length).unwrap_or(u8::MAX));
        payload.extend_from_slice(&name.as_bytes()[..length]);
    }
    payload.resize(x11::pad4(payload.len()), 0);
    let mut output = Vec::with_capacity(32 + payload.len());
    output.push(1);
    output.push(0);
    let mut sequence_bytes = [0; 2];
    write_u16_at(order, &mut sequence_bytes, sequence.0);
    output.extend_from_slice(&sequence_bytes);
    write_u32_vec(
        order,
        &mut output,
        u32::try_from(payload.len() / 4).unwrap_or(u32::MAX),
    );
    let mut count_bytes = [0; 2];
    write_u16_at(
        order,
        &mut count_bytes,
        u16::try_from(names.len()).unwrap_or(u16::MAX),
    );
    output.extend_from_slice(&count_bytes);
    output.resize(32, 0);
    output.extend_from_slice(&payload);
    output
}

pub(super) fn pattern_pixel(surface: &Surface, x: u16, y: u16, origin_x: i16, origin_y: i16) -> u32 {
    let pattern_x =
        (i32::from(x) - i32::from(origin_x)).rem_euclid(i32::from(surface.width.max(1)));
    let pattern_y =
        (i32::from(y) - i32::from(origin_y)).rem_euclid(i32::from(surface.height.max(1)));
    surface.pixel(
        u16::try_from(pattern_x).unwrap_or(0),
        u16::try_from(pattern_y).unwrap_or(0),
    )
}

pub(super) fn decode_points(
    coordinate_mode: u8,
    wire_points: &[u8],
) -> Result<Vec<(i32, i32)>, ProtocolError> {
    if coordinate_mode > 1 {
        return Err(ProtocolError::new(
            x11::error::BAD_VALUE,
            u32::from(coordinate_mode),
        ));
    }
    let mut points = Vec::with_capacity(wire_points.len() / 4);
    let mut previous = (0i32, 0i32);
    for (index, point) in wire_points.chunks_exact(4).enumerate() {
        let raw = (
            i32::from(read_i16_le(&point[0..2])),
            i32::from(read_i16_le(&point[2..4])),
        );
        let resolved = if coordinate_mode == 1 && index != 0 {
            (
                previous.0.saturating_add(raw.0),
                previous.1.saturating_add(raw.1),
            )
        } else {
            raw
        };
        points.push(resolved);
        previous = resolved;
    }
    Ok(points)
}

#[derive(Clone, Copy)]
pub(super) struct ArcSpecification {
    pub(super) x: i32,
    pub(super) y: i32,
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) angle1: i16,
    pub(super) angle2: i16,
}

pub(super) fn decode_arc(wire: &[u8]) -> ArcSpecification {
    ArcSpecification {
        x: i32::from(read_i16_le(&wire[0..2])),
        y: i32::from(read_i16_le(&wire[2..4])),
        width: read_u16(ClientByteOrder::LittleEndian, &wire[4..6]),
        height: read_u16(ClientByteOrder::LittleEndian, &wire[6..8]),
        angle1: read_i16_le(&wire[8..10]),
        angle2: read_i16_le(&wire[10..12]),
    }
}

pub(super) fn arc_outline_points(arc: ArcSpecification) -> Vec<(i32, i32)> {
    if arc.width == 0 || arc.height == 0 || arc.angle2 == 0 {
        return Vec::new();
    }
    let start = f64::from(arc.angle1) / 64.0;
    let sweep = f64::from(arc.angle2) / 64.0;
    let radius_x = f64::from(arc.width) / 2.0;
    let radius_y = f64::from(arc.height) / 2.0;
    let center_x = f64::from(arc.x) + radius_x;
    let center_y = f64::from(arc.y) + radius_y;
    let estimate = u32::from(arc.width.max(arc.height))
        .saturating_mul(u32::from(arc.angle2.unsigned_abs()))
        .div_ceil(64 * 57);
    let samples = estimate.clamp(1, 65_536);
    let mut points =
        Vec::with_capacity(usize::try_from(samples.saturating_add(1)).unwrap_or(65_537));
    for index in 0..=samples {
        let fraction = f64::from(index) / f64::from(samples);
        let radians = (start + sweep * fraction).to_radians();
        let point = (
            rounded_f64_to_i32(center_x + radius_x * radians.cos()),
            rounded_f64_to_i32(center_y - radius_y * radians.sin()),
        );
        if points.last().copied() != Some(point) {
            points.push(point);
        }
    }
    points
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn rounded_f64_to_i32(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

pub(super) fn angle_in_sweep(angle: f64, start: f64, sweep: f64) -> bool {
    if sweep.abs() >= 360.0 {
        return true;
    }
    let normalized_angle = angle.rem_euclid(360.0);
    let normalized_start = start.rem_euclid(360.0);
    if sweep >= 0.0 {
        (normalized_angle - normalized_start).rem_euclid(360.0) <= sweep
    } else {
        (normalized_start - normalized_angle).rem_euclid(360.0) <= -sweep
    }
}

pub(super) fn point_inside_arc(x: i32, y: i32, arc: ArcSpecification) -> bool {
    if arc.width == 0 || arc.height == 0 || arc.angle2 == 0 {
        return false;
    }
    let radius_x = f64::from(arc.width) / 2.0;
    let radius_y = f64::from(arc.height) / 2.0;
    let dx = (f64::from(x) + 0.5 - (f64::from(arc.x) + radius_x)) / radius_x;
    let dy = (f64::from(y) + 0.5 - (f64::from(arc.y) + radius_y)) / radius_y;
    if dx * dx + dy * dy > 1.0 {
        return false;
    }
    let angle = (-dy).atan2(dx).to_degrees();
    angle_in_sweep(
        angle,
        f64::from(arc.angle1) / 64.0,
        f64::from(arc.angle2) / 64.0,
    )
}

pub(super) fn point_inside_polygon(x: i32, y: i32, points: &[(i32, i32)], fill_rule: u8) -> bool {
    let test_x = f64::from(x) + 0.5;
    let test_y = f64::from(y) + 0.5;
    let mut crossings = 0i32;
    let mut winding = 0i32;
    for index in 0..points.len() {
        let first = points[index];
        let second = points[(index + 1) % points.len()];
        let first_y = f64::from(first.1);
        let second_y = f64::from(second.1);
        if (first_y > test_y) == (second_y > test_y) {
            continue;
        }
        let intersection = f64::from(first.0)
            + (test_y - first_y) * f64::from(second.0 - first.0) / (second_y - first_y);
        if intersection > test_x {
            crossings += 1;
            winding += if second.1 > first.1 { 1 } else { -1 };
        }
    }
    if fill_rule == 1 {
        winding != 0
    } else {
        crossings % 2 != 0
    }
}

pub(super) fn dash_source(gc: &GraphicsContext, phase: usize) -> Option<u32> {
    if gc.line_style == 0 || gc.dashes.is_empty() {
        return Some(gc.foreground);
    }
    let cycle = gc
        .dashes
        .iter()
        .map(|value| usize::from(*value))
        .sum::<usize>()
        .max(1);
    let mut position = phase % cycle;
    for (index, length) in gc.dashes.iter().copied().enumerate() {
        let length = usize::from(length);
        if position < length {
            return if index.is_multiple_of(2) {
                Some(gc.foreground)
            } else if gc.line_style == 2 {
                Some(gc.background)
            } else {
                None
            };
        }
        position = position.saturating_sub(length);
    }
    Some(gc.foreground)
}

pub(super) fn rasterize_segment(
    output: &mut Vec<(i32, i32, u32)>,
    start: (i32, i32),
    end: (i32, i32),
    gc: &GraphicsContext,
    skip_first: bool,
    phase: &mut usize,
) -> Result<(), ProtocolError> {
    const MAX_RASTER_SAMPLES: usize = 16 * 1024 * 1024;
    let (mut x, mut y) = start;
    let dx = (end.0 - start.0).unsigned_abs();
    let sx = if start.0 < end.0 { 1 } else { -1 };
    let dy = -(i64::from((end.1 - start.1).unsigned_abs()));
    let sy = if start.1 < end.1 { 1 } else { -1 };
    let mut error = i64::from(dx) + dy;
    let mut first = true;
    loop {
        if !(first && skip_first) {
            if output.len() >= MAX_RASTER_SAMPLES {
                return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
            }
            if let Some(source) = dash_source(gc, *phase) {
                output.push((x, y, source));
            }
            *phase = phase.saturating_add(1);
        }
        first = false;
        if x == end.0 && y == end.1 {
            break;
        }
        let twice = error.saturating_mul(2);
        if twice >= dy {
            error = error.saturating_add(dy);
            x = x.saturating_add(sx);
        }
        if twice <= i64::from(dx) {
            error = error.saturating_add(i64::from(dx));
            y = y.saturating_add(sy);
        }
    }
    Ok(())
}
