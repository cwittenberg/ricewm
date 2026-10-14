//! Protocol encoding and raster helpers.
use super::*;



#[allow(clippy::too_many_lines)]
pub(super) fn decode_image(
    byte_order: ClientByteOrder,
    request: &x11::PutImageRequest<'_>,
    gc: &GraphicsContext,
) -> Result<Vec<u32>, ProtocolError> {
    let pixel_count = usize::from(request.width)
        .checked_mul(usize::from(request.height))
        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
    match request.format {
        x11::ImageFormat::ZPixmap => {
            if request.left_pad != 0 || request.depth != gc.depth {
                return Err(ProtocolError::new(
                    x11::error::BAD_MATCH,
                    u32::from(request.depth),
                ));
            }
            let width = usize::from(request.width);
            let height = usize::from(request.height);
            let row_bytes = match request.depth {
                1 => width.div_ceil(32).saturating_mul(4),
                4 => width.div_ceil(2).div_ceil(4).saturating_mul(4),
                8 => width.div_ceil(4).saturating_mul(4),
                24 | 32 => width.saturating_mul(4),
                _ => {
                    return Err(ProtocolError::new(
                        x11::error::BAD_MATCH,
                        u32::from(request.depth),
                    ));
                }
            };
            let required = row_bytes
                .checked_mul(height)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let data = request
                .data
                .get(..required)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_LENGTH, 0))?;
            let mut pixels = Vec::with_capacity(pixel_count);
            for row in 0..height {
                for column in 0..width {
                    let pixel = match request.depth {
                        1 => {
                            let byte = data[row * row_bytes + column / 8];
                            let bit = match byte_order {
                                ClientByteOrder::LittleEndian => column % 8,
                                ClientByteOrder::BigEndian => 7 - (column % 8),
                            };
                            u32::from(byte & (1 << bit) != 0)
                        }
                        4 => {
                            let byte = data[row * row_bytes + column / 2];
                            u32::from(match (byte_order, column % 2) {
                                (ClientByteOrder::LittleEndian, 0)
                                | (ClientByteOrder::BigEndian, 1) => byte & 0x0f,
                                _ => byte >> 4,
                            })
                        }
                        8 => u32::from(data[row * row_bytes + column]),
                        24 | 32 => {
                            let start = row * row_bytes + column * 4;
                            read_u32(byte_order, &data[start..start + 4])
                                & depth_plane_mask(request.depth)
                        }
                        _ => unreachable!(),
                    };
                    pixels.push(pixel);
                }
            }
            Ok(pixels)
        }
        x11::ImageFormat::XyBitmap => {
            if request.depth != 1 {
                return Err(ProtocolError::new(
                    x11::error::BAD_MATCH,
                    u32::from(request.depth),
                ));
            }
            let row_bits = usize::from(request.left_pad)
                .checked_add(usize::from(request.width))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let stride = row_bits.div_ceil(32).saturating_mul(4);
            let required = stride
                .checked_mul(usize::from(request.height))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let data = request
                .data
                .get(..required)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_LENGTH, 0))?;
            let mut pixels = Vec::with_capacity(pixel_count);
            for y in 0..usize::from(request.height) {
                for x in 0..usize::from(request.width) {
                    let bit = usize::from(request.left_pad).saturating_add(x);
                    let byte = data[y.saturating_mul(stride).saturating_add(bit / 8)];
                    let mask = match byte_order {
                        ClientByteOrder::LittleEndian => 1 << (bit % 8),
                        ClientByteOrder::BigEndian => 0x80 >> (bit % 8),
                    };
                    pixels.push(if byte & mask == 0 {
                        gc.background
                    } else {
                        gc.foreground
                    });
                }
            }
            Ok(pixels)
        }
        x11::ImageFormat::XyPixmap => {
            if request.depth != gc.depth || request.depth == 0 || request.depth > 32 {
                return Err(ProtocolError::new(
                    x11::error::BAD_MATCH,
                    u32::from(request.depth),
                ));
            }
            let row_bits = usize::from(request.left_pad)
                .checked_add(usize::from(request.width))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let stride = row_bits.div_ceil(32).saturating_mul(4);
            let plane_bytes = stride
                .checked_mul(usize::from(request.height))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let required = plane_bytes
                .checked_mul(usize::from(request.depth))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let data = request
                .data
                .get(..required)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_LENGTH, 0))?;
            let mut pixels = Vec::with_capacity(pixel_count);
            for y in 0..usize::from(request.height) {
                for x in 0..usize::from(request.width) {
                    let bit = usize::from(request.left_pad).saturating_add(x);
                    let bit_mask = match byte_order {
                        ClientByteOrder::LittleEndian => 1 << (bit % 8),
                        ClientByteOrder::BigEndian => 0x80 >> (bit % 8),
                    };
                    let mut pixel = 0u32;
                    // The protocol carries XYPixmap planes most-significant
                    // first. Each plane uses the server-advertised bitmap bit
                    // order and scanline padding.
                    for plane_index in 0..usize::from(request.depth) {
                        let byte_index = plane_index
                            .saturating_mul(plane_bytes)
                            .saturating_add(y.saturating_mul(stride))
                            .saturating_add(bit / 8);
                        if data[byte_index] & bit_mask != 0 {
                            let plane = usize::from(request.depth)
                                .saturating_sub(1)
                                .saturating_sub(plane_index);
                            pixel |= 1u32 << plane;
                        }
                    }
                    pixels.push(pixel);
                }
            }
            Ok(pixels)
        }
        x11::ImageFormat::Unknown(format) => {
            Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(format)))
        }
    }
}

pub(super) fn apply_gc_function(
    destination: u32,
    source: u32,
    function: u8,
    plane_mask: u32,
    drawable_mask: u32,
) -> u32 {
    let raster = match function {
        0 => 0,
        1 => source & destination,
        2 => source & !destination,
        3 => source,
        4 => !source & destination,
        5 => destination,
        6 => source ^ destination,
        7 => source | destination,
        8 => !(source | destination),
        9 => !(source ^ destination),
        10 => !destination,
        11 => source | !destination,
        12 => !source,
        13 => !source | destination,
        14 => !(source & destination),
        15 => u32::MAX,
        _ => unreachable!("GC function values are validated before dispatch"),
    };
    ((destination & !plane_mask) | (raster & plane_mask)) & drawable_mask
}

pub(super) fn copy_gc_fields(source: &GraphicsContext, destination: &mut GraphicsContext, value_mask: u32) {
    if value_mask & (1 << 0) != 0 {
        destination.function = source.function;
    }
    if value_mask & (1 << 1) != 0 {
        destination.plane_mask = source.plane_mask;
    }
    if value_mask & (1 << 2) != 0 {
        destination.foreground = source.foreground;
    }
    if value_mask & (1 << 3) != 0 {
        destination.background = source.background;
    }
    if value_mask & (1 << 4) != 0 {
        destination.line_width = source.line_width;
    }
    if value_mask & (1 << 5) != 0 {
        destination.line_style = source.line_style;
    }
    if value_mask & (1 << 6) != 0 {
        destination.cap_style = source.cap_style;
    }
    if value_mask & (1 << 7) != 0 {
        destination.join_style = source.join_style;
    }
    if value_mask & (1 << 8) != 0 {
        destination.fill_style = source.fill_style;
    }
    if value_mask & (1 << 9) != 0 {
        destination.fill_rule = source.fill_rule;
    }
    if value_mask & (1 << 10) != 0 {
        destination.tile = source.tile;
    }
    if value_mask & (1 << 11) != 0 {
        destination.stipple = source.stipple;
    }
    if value_mask & (1 << 12) != 0 {
        destination.tile_x_origin = source.tile_x_origin;
    }
    if value_mask & (1 << 13) != 0 {
        destination.tile_y_origin = source.tile_y_origin;
    }
    if value_mask & (1 << 14) != 0 {
        destination.font = source.font;
    }
    if value_mask & (1 << 15) != 0 {
        destination.subwindow_mode = source.subwindow_mode;
    }
    if value_mask & (1 << 16) != 0 {
        destination.graphics_exposures = source.graphics_exposures;
    }
    if value_mask & (1 << 17) != 0 {
        destination.clip_x_origin = source.clip_x_origin;
    }
    if value_mask & (1 << 18) != 0 {
        destination.clip_y_origin = source.clip_y_origin;
    }
    if value_mask & (1 << 19) != 0 {
        destination.clip_mask = source.clip_mask;
        destination
            .clip_rectangles
            .clone_from(&source.clip_rectangles);
    }
    if value_mask & (1 << 20) != 0 {
        destination.dash_offset = source.dash_offset;
    }
    if value_mask & (1 << 21) != 0 {
        destination.dashes.clone_from(&source.dashes);
    }
    if value_mask & (1 << 22) != 0 {
        destination.arc_mode = source.arc_mode;
    }
}

pub(super) fn extension_error(mut error: ProtocolError, minor: u8) -> ProtocolError {
    error.minor_opcode = u16::from(minor);
    error
}

pub(super) fn validate_shape_parameters(
    operation: u8,
    kind: u8,
    ordering: Option<u8>,
) -> Result<(), ProtocolError> {
    if operation > x11shape::OP_INVERT {
        return Err(ProtocolError::new(
            x11::error::BAD_VALUE,
            u32::from(operation),
        ));
    }
    if kind > x11shape::KIND_INPUT {
        return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(kind)));
    }
    if ordering.is_some_and(|value| value > x11shape::ORDERING_YX_BANDED) {
        return Err(ProtocolError::new(
            x11::error::BAD_VALUE,
            u32::from(ordering.unwrap_or_default()),
        ));
    }
    Ok(())
}

pub(super) fn decode_region_rectangles(order: ClientByteOrder, bytes: &[u8]) -> Vec<x11xfixes::RegionRect> {
    bytes
        .chunks_exact(8)
        .filter_map(|rectangle| {
            let rectangle = x11xfixes::RegionRect {
                x: read_i16(order, &rectangle[0..2]),
                y: read_i16(order, &rectangle[2..4]),
                width: read_u16(order, &rectangle[4..6]),
                height: read_u16(order, &rectangle[6..8]),
            };
            (!rectangle.is_empty()).then_some(rectangle)
        })
        .collect()
}

pub(super) fn encode_region_rectangles_le(rectangles: &[x11xfixes::RegionRect]) -> Vec<u8> {
    let mut output = Vec::with_capacity(rectangles.len().saturating_mul(8));
    for rectangle in rectangles {
        output.extend_from_slice(&rectangle.x.to_le_bytes());
        output.extend_from_slice(&rectangle.y.to_le_bytes());
        output.extend_from_slice(&rectangle.width.to_le_bytes());
        output.extend_from_slice(&rectangle.height.to_le_bytes());
    }
    output
}

pub(super) fn offset_region_rectangle(
    rectangle: x11xfixes::RegionRect,
    x: i16,
    y: i16,
) -> x11xfixes::RegionRect {
    x11xfixes::RegionRect {
        x: clamp_i16(i32::from(rectangle.x).saturating_add(i32::from(x))),
        y: clamp_i16(i32::from(rectangle.y).saturating_add(i32::from(y))),
        ..rectangle
    }
}

pub(super) fn normalize_region(mut rectangles: Vec<x11xfixes::RegionRect>) -> Vec<x11xfixes::RegionRect> {
    rectangles.retain(|rectangle| !rectangle.is_empty());
    rectangles
        .sort_by_key(|rectangle| (rectangle.y, rectangle.x, rectangle.height, rectangle.width));
    rectangles.dedup();
    rectangles
}

pub(super) fn region_intersection(
    first: x11xfixes::RegionRect,
    second: x11xfixes::RegionRect,
) -> Option<x11xfixes::RegionRect> {
    let left = i32::from(first.x).max(i32::from(second.x));
    let top = i32::from(first.y).max(i32::from(second.y));
    let right = i32::from(first.x)
        .saturating_add(i32::from(first.width))
        .min(i32::from(second.x).saturating_add(i32::from(second.width)));
    let bottom = i32::from(first.y)
        .saturating_add(i32::from(first.height))
        .min(i32::from(second.y).saturating_add(i32::from(second.height)));
    if right <= left || bottom <= top {
        return None;
    }
    Some(x11xfixes::RegionRect {
        x: clamp_i16(left),
        y: clamp_i16(top),
        width: u16::try_from(right - left).unwrap_or(u16::MAX),
        height: u16::try_from(bottom - top).unwrap_or(u16::MAX),
    })
}

pub(super) fn intersect_region(
    first: &[x11xfixes::RegionRect],
    second: &[x11xfixes::RegionRect],
) -> Vec<x11xfixes::RegionRect> {
    normalize_region(
        first
            .iter()
            .flat_map(|first| {
                second
                    .iter()
                    .filter_map(move |second| region_intersection(*first, *second))
            })
            .collect(),
    )
}

pub(super) fn subtract_rectangle(
    source: x11xfixes::RegionRect,
    cutter: x11xfixes::RegionRect,
) -> Vec<x11xfixes::RegionRect> {
    let Some(overlap) = region_intersection(source, cutter) else {
        return vec![source];
    };
    let source_left = i32::from(source.x);
    let source_top = i32::from(source.y);
    let source_right = source_left + i32::from(source.width);
    let source_bottom = source_top + i32::from(source.height);
    let overlap_left = i32::from(overlap.x);
    let overlap_top = i32::from(overlap.y);
    let overlap_right = overlap_left + i32::from(overlap.width);
    let overlap_bottom = overlap_top + i32::from(overlap.height);
    [
        (source_left, source_top, source_right, overlap_top),
        (source_left, overlap_bottom, source_right, source_bottom),
        (source_left, overlap_top, overlap_left, overlap_bottom),
        (overlap_right, overlap_top, source_right, overlap_bottom),
    ]
    .into_iter()
    .filter(|(left, top, right, bottom)| right > left && bottom > top)
    .map(|(left, top, right, bottom)| x11xfixes::RegionRect {
        x: clamp_i16(left),
        y: clamp_i16(top),
        width: u16::try_from(right - left).unwrap_or(u16::MAX),
        height: u16::try_from(bottom - top).unwrap_or(u16::MAX),
    })
    .collect()
}

pub(super) fn subtract_region(
    first: &[x11xfixes::RegionRect],
    second: &[x11xfixes::RegionRect],
) -> Vec<x11xfixes::RegionRect> {
    let mut output = first.to_vec();
    for cutter in second {
        output = output
            .into_iter()
            .flat_map(|source| subtract_rectangle(source, *cutter))
            .collect();
    }
    normalize_region(output)
}

pub(super) fn apply_region_operation(
    operation: u8,
    destination: &[x11xfixes::RegionRect],
    source: &[x11xfixes::RegionRect],
) -> Vec<x11xfixes::RegionRect> {
    match operation {
        x11shape::OP_SET => normalize_region(source.to_vec()),
        x11shape::OP_UNION => {
            let mut output = destination.to_vec();
            output.extend_from_slice(source);
            normalize_region(output)
        }
        x11shape::OP_INTERSECT => intersect_region(destination, source),
        x11shape::OP_SUBTRACT => subtract_region(destination, source),
        x11shape::OP_INVERT => subtract_region(source, destination),
        _ => Vec::new(),
    }
}

pub(super) fn region_extents(rectangles: &[x11xfixes::RegionRect]) -> x11xfixes::RegionRect {
    if rectangles.is_empty() {
        return x11xfixes::RegionRect {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }
    let left = rectangles
        .iter()
        .map(|rectangle| i32::from(rectangle.x))
        .min()
        .unwrap_or_default();
    let top = rectangles
        .iter()
        .map(|rectangle| i32::from(rectangle.y))
        .min()
        .unwrap_or_default();
    let right = rectangles
        .iter()
        .map(|rectangle| i32::from(rectangle.x) + i32::from(rectangle.width))
        .max()
        .unwrap_or(left);
    let bottom = rectangles
        .iter()
        .map(|rectangle| i32::from(rectangle.y) + i32::from(rectangle.height))
        .max()
        .unwrap_or(top);
    x11xfixes::RegionRect {
        x: clamp_i16(left),
        y: clamp_i16(top),
        width: u16::try_from(right.saturating_sub(left)).unwrap_or(u16::MAX),
        height: u16::try_from(bottom.saturating_sub(top)).unwrap_or(u16::MAX),
    }
}

pub(super) fn bitmap_region(surface: &Surface) -> Vec<x11xfixes::RegionRect> {
    let mut rectangles = Vec::new();
    for y in 0..surface.height {
        let mut x = 0;
        while x < surface.width {
            while x < surface.width && surface.pixel(x, y) & 1 == 0 {
                x = x.saturating_add(1);
            }
            let start = x;
            while x < surface.width && surface.pixel(x, y) & 1 != 0 {
                x = x.saturating_add(1);
            }
            if x > start {
                rectangles.push(x11xfixes::RegionRect {
                    x: i16::try_from(start).unwrap_or(i16::MAX),
                    y: i16::try_from(y).unwrap_or(i16::MAX),
                    width: x - start,
                    height: 1,
                });
            }
        }
    }
    normalize_region(rectangles)
}

pub(super) fn decode_cursor_name_request(
    order: ClientByteOrder,
    body: &[u8],
) -> Result<(u32, &[u8]), ProtocolError> {
    if body.len() < 8 {
        return Err(bad_length());
    }
    let cursor = read_u32(order, body);
    let length = usize::from(read_u16(order, &body[4..6]));
    let name = body
        .get(8..8usize.saturating_add(length))
        .ok_or_else(bad_length)?;
    if body.len() != 8 + x11::pad4(length) {
        return Err(bad_length());
    }
    Ok((cursor, name))
}

pub(super) fn motion_mask_for_state(state: u16) -> u32 {
    let mut mask = 0;
    for (button_mask, motion_mask) in [
        (0x0100, BUTTON_1_MOTION_MASK),
        (0x0200, BUTTON_2_MOTION_MASK),
        (0x0400, BUTTON_3_MOTION_MASK),
        (0x0800, BUTTON_4_MOTION_MASK),
        (0x1000, BUTTON_5_MOTION_MASK),
    ] {
        if state & button_mask != 0 {
            mask |= BUTTON_MOTION_MASK | motion_mask;
        }
    }
    mask
}

pub(super) fn clamp_i16(value: i32) -> i16 {
    let clamped = value.clamp(i32::from(i16::MIN), i32::from(i16::MAX));
    match i16::try_from(clamped) {
        Ok(value) => value,
        Err(_) if clamped < 0 => i16::MIN,
        Err(_) => i16::MAX,
    }
}

pub(super) fn read_u16(byte_order: ClientByteOrder, bytes: &[u8]) -> u16 {
    let value = [bytes[0], bytes[1]];
    match byte_order {
        ClientByteOrder::LittleEndian => u16::from_le_bytes(value),
        ClientByteOrder::BigEndian => u16::from_be_bytes(value),
    }
}

pub(super) fn read_i16(byte_order: ClientByteOrder, bytes: &[u8]) -> i16 {
    let value = [bytes[0], bytes[1]];
    match byte_order {
        ClientByteOrder::LittleEndian => i16::from_le_bytes(value),
        ClientByteOrder::BigEndian => i16::from_be_bytes(value),
    }
}
