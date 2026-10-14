//! Protocol encoding and raster helpers.
use super::*;



#[allow(clippy::too_many_lines)]
pub(super) fn encode_z_pixmap(
    order: ClientByteOrder,
    surface: &Surface,
    request: &x11::GetImageRequest,
    plane_mask: u32,
) -> Result<Vec<u8>, ProtocolError> {
    let width = usize::from(request.width);
    let height = usize::from(request.height);
    if surface.depth == 1 {
        let stride = width
            .div_ceil(32)
            .checked_mul(4)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        let mut output = vec![
            0;
            stride
                .checked_mul(height)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?
        ];
        if plane_mask & 1 != 0 {
            for y in 0..height {
                for x in 0..width {
                    let sx =
                        u16::try_from(i32::from(request.x) + i32::try_from(x).unwrap_or(i32::MAX))
                            .map_err(|_| {
                                ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0)
                            })?;
                    let sy =
                        u16::try_from(i32::from(request.y) + i32::try_from(y).unwrap_or(i32::MAX))
                            .map_err(|_| {
                                ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0)
                            })?;
                    if surface.pixel(sx, sy) & 1 != 0 {
                        let bit = match order {
                            ClientByteOrder::LittleEndian => x % 8,
                            ClientByteOrder::BigEndian => 7 - (x % 8),
                        };
                        output[y * stride + x / 8] |= 1 << bit;
                    }
                }
            }
        }
        return Ok(output);
    }
    if matches!(surface.depth, 4 | 8) {
        let packed_width = if surface.depth == 4 {
            width.div_ceil(2)
        } else {
            width
        };
        let stride = packed_width
            .div_ceil(4)
            .checked_mul(4)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        let mut output = vec![
            0;
            stride
                .checked_mul(height)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?
        ];
        for y in 0..height {
            for x in 0..width {
                let sx = u16::try_from(i32::from(request.x) + i32::try_from(x).unwrap_or(i32::MAX))
                    .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
                let sy = u16::try_from(i32::from(request.y) + i32::try_from(y).unwrap_or(i32::MAX))
                    .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
                let pixel = surface.pixel(sx, sy) & plane_mask;
                if surface.depth == 8 {
                    output[y * stride + x] = u8::try_from(pixel).unwrap_or_default();
                } else {
                    let value = u8::try_from(pixel & 0x0f).unwrap_or_default();
                    let target = &mut output[y * stride + x / 2];
                    match (order, x % 2) {
                        (ClientByteOrder::LittleEndian, 0) | (ClientByteOrder::BigEndian, 1) => {
                            *target |= value;
                        }
                        _ => *target |= value << 4,
                    }
                }
            }
        }
        return Ok(output);
    }
    if !matches!(surface.depth, 24 | 32) {
        return Err(ProtocolError::new(
            x11::error::BAD_MATCH,
            request.drawable.0,
        ));
    }
    let pixel_count = width
        .checked_mul(height)
        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
    let mut output = Vec::with_capacity(
        pixel_count
            .checked_mul(4)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?,
    );
    for y in 0..height {
        for x in 0..width {
            let sx = u16::try_from(i32::from(request.x) + i32::try_from(x).unwrap_or(i32::MAX))
                .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
            let sy = u16::try_from(i32::from(request.y) + i32::try_from(y).unwrap_or(i32::MAX))
                .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
            write_u32_vec(order, &mut output, surface.pixel(sx, sy) & plane_mask);
        }
    }
    Ok(output)
}

pub(super) fn encode_xy_pixmap(
    order: ClientByteOrder,
    surface: &Surface,
    request: &x11::GetImageRequest,
    plane_mask: u32,
) -> Result<Vec<u8>, ProtocolError> {
    let width = usize::from(request.width);
    let height = usize::from(request.height);
    let stride = width
        .div_ceil(32)
        .checked_mul(4)
        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
    let plane_bytes = stride
        .checked_mul(height)
        .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
    let plane_count = usize::try_from(plane_mask.count_ones()).unwrap_or(usize::MAX);
    let mut output = vec![
        0;
        plane_bytes
            .checked_mul(plane_count)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?
    ];
    let mut plane_base = 0;
    for plane in (0..32).rev().filter(|plane| plane_mask & (1 << plane) != 0) {
        for y in 0..height {
            for x in 0..width {
                let sx = u16::try_from(i32::from(request.x) + i32::try_from(x).unwrap_or(i32::MAX))
                    .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
                let sy = u16::try_from(i32::from(request.y) + i32::try_from(y).unwrap_or(i32::MAX))
                    .map_err(|_| ProtocolError::new(x11::error::BAD_MATCH, request.drawable.0))?;
                if surface.pixel(sx, sy) & (1 << plane) != 0 {
                    let bit = match order {
                        ClientByteOrder::LittleEndian => x % 8,
                        ClientByteOrder::BigEndian => 7 - (x % 8),
                    };
                    output[plane_base + y * stride + x / 8] |= 1 << bit;
                }
            }
        }
        plane_base += plane_bytes;
    }
    Ok(output)
}

pub(super) fn encode_get_image_reply(
    order: ClientByteOrder,
    sequence: SequenceNumber,
    depth: u8,
    visual: u32,
    pixels: &[u8],
) -> Result<Vec<u8>, ProtocolError> {
    if !pixels.len().is_multiple_of(4) {
        return Err(encoding_error());
    }
    let units = u32::try_from(pixels.len() / 4).map_err(|_| encoding_error())?;
    let mut output = Vec::with_capacity(32usize.saturating_add(pixels.len()));
    output.push(1);
    output.push(depth);
    let mut sequence_bytes = [0; 2];
    write_u16_at(order, &mut sequence_bytes, sequence.0);
    output.extend_from_slice(&sequence_bytes);
    write_u32_vec(order, &mut output, units);
    write_u32_vec(order, &mut output, visual);
    output.resize(32, 0);
    output.extend_from_slice(pixels);
    Ok(output)
}

pub(super) fn request_resource(body: &[u8]) -> Option<ResourceId> {
    Some(ResourceId(read_u32_le(body.get(0..4)?)))
}

pub(super) fn geometry(window: &Window) -> Geometry {
    Geometry {
        root: ROOT_WINDOW,
        x: window.x,
        y: window.y,
        width: window.width,
        height: window.height,
        border_width: window.border_width,
        depth: window.depth,
    }
}

pub(super) fn clipped_rectangle(
    surface: &Surface,
    x: i16,
    y: i16,
    width: u16,
    height: u16,
) -> Option<ClippedRectangle> {
    if width == 0 || height == 0 {
        return None;
    }
    let left = i32::from(x).max(0);
    let top = i32::from(y).max(0);
    let right = i32::from(x)
        .saturating_add(i32::from(width))
        .min(i32::from(surface.width));
    let bottom = i32::from(y)
        .saturating_add(i32::from(height))
        .min(i32::from(surface.height));
    if right <= left || bottom <= top {
        return None;
    }
    Some(ClippedRectangle {
        x: u16::try_from(left).ok()?,
        y: u16::try_from(top).ok()?,
        width: u16::try_from(right - left).ok()?,
        height: u16::try_from(bottom - top).ok()?,
    })
}

pub(super) fn fixed_to_f64(value: i32) -> f64 {
    f64::from(value) / 65_536.0
}

#[allow(clippy::cast_possible_truncation)]
pub(super) fn f64_to_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

pub(super) fn fixed_point(order: ClientByteOrder, bytes: &[u8]) -> (f64, f64) {
    (
        fixed_to_f64(read_i32(order, &bytes[0..4])),
        fixed_to_f64(read_i32(order, &bytes[4..8])),
    )
}

pub(super) fn line_x_at_y(first: (f64, f64), second: (f64, f64), y: f64) -> f64 {
    let delta_y = second.1 - first.1;
    if delta_y.abs() < f64::EPSILON {
        return first.0;
    }
    first.0 + (y - first.1) * (second.0 - first.0) / delta_y
}

pub(super) fn polygon_bounds(
    polygons: &[Vec<(f64, f64)>],
    destination_width: u16,
    destination_height: u16,
) -> Option<ClippedRectangle> {
    let mut left = f64::INFINITY;
    let mut top = f64::INFINITY;
    let mut right = f64::NEG_INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    for point in polygons.iter().flatten() {
        left = left.min(point.0);
        top = top.min(point.1);
        right = right.max(point.0);
        bottom = bottom.max(point.1);
    }
    if !left.is_finite() || !top.is_finite() || right <= left || bottom <= top {
        return None;
    }
    let left = f64_to_i32(left.floor()).clamp(0, i32::from(destination_width));
    let top = f64_to_i32(top.floor()).clamp(0, i32::from(destination_height));
    let right = f64_to_i32(right.ceil()).clamp(0, i32::from(destination_width));
    let bottom = f64_to_i32(bottom.ceil()).clamp(0, i32::from(destination_height));
    if right <= left || bottom <= top {
        return None;
    }
    Some(ClippedRectangle {
        x: u16::try_from(left).ok()?,
        y: u16::try_from(top).ok()?,
        width: u16::try_from(right - left).ok()?,
        height: u16::try_from(bottom - top).ok()?,
    })
}

#[cfg(test)]
pub(super) fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon.len() - 1;
    for current in 0..polygon.len() {
        let first = polygon[current];
        let second = polygon[previous];
        if (first.1 > point.1) != (second.1 > point.1) {
            let crossing =
                (second.0 - first.0) * (point.1 - first.1) / (second.1 - first.1) + first.0;
            if point.0 < crossing {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

pub(super) fn rasterize_polygons(polygons: &[Vec<(f64, f64)>], bounds: ClippedRectangle) -> Vec<u8> {
    const SAMPLE_OFFSETS: [f64; 4] = [0.125, 0.375, 0.625, 0.875];
    let mut output =
        vec![0u8; usize::from(bounds.width).saturating_mul(usize::from(bounds.height))];
    let subpixel_width = usize::from(bounds.width).saturating_mul(SAMPLE_OFFSETS.len());
    let mut scanline = vec![0u8; subpixel_width];
    let prepared: Vec<_> = polygons
        .iter()
        .filter(|polygon| polygon.len() >= 3)
        .map(|polygon| {
            let (mut top, mut bottom) = (f64::INFINITY, f64::NEG_INFINITY);
            for point in polygon {
                top = top.min(point.1);
                bottom = bottom.max(point.1);
            }
            (polygon.as_slice(), top, bottom)
        })
        .collect();
    let mut crossings = Vec::new();
    for row in 0..bounds.height {
        for sample_y in SAMPLE_OFFSETS {
            scanline.fill(0);
            let y = f64::from(bounds.y + row) + sample_y;
            for &(polygon, top, bottom) in &prepared {
                if y < top || y >= bottom {
                    continue;
                }
                crossings.clear();
                let mut previous = polygon.len() - 1;
                for current in 0..polygon.len() {
                    let first = polygon[current];
                    let second = polygon[previous];
                    if (first.1 > y) != (second.1 > y) {
                        crossings.push(
                            (second.0 - first.0) * (y - first.1) / (second.1 - first.1) + first.0,
                        );
                    }
                    previous = current;
                }
                crossings.sort_unstable_by(f64::total_cmp);
                for interval in crossings.chunks_exact(2) {
                    let start = subpixel_index_at_or_after(interval[0], bounds.x, subpixel_width);
                    let end = subpixel_index_at_or_after(interval[1], bounds.x, subpixel_width);
                    scanline[start..end].fill(1);
                }
            }
            let output_row = usize::from(row).saturating_mul(usize::from(bounds.width));
            for column in 0..usize::from(bounds.width) {
                output[output_row + column] = output[output_row + column]
                    .saturating_add(scanline[column * 4..column * 4 + 4].iter().copied().sum());
            }
        }
    }
    for alpha in &mut output {
        *alpha = u8::try_from(u16::from(*alpha) * 255 / 16).unwrap_or(255);
    }
    output
}

pub(super) fn subpixel_index_at_or_after(x: f64, origin: u16, length: usize) -> usize {
    let scaled = ((x - f64::from(origin)) * 4.0 - 0.5).ceil();
    if !scaled.is_finite() || scaled <= 0.0 {
        0
    } else if scaled >= length as f64 {
        length
    } else {
        scaled as usize
    }
}

pub(super) fn clipped_copy_rectangles(
    source: &Surface,
    destination: &Surface,
    request: &x11::CopyAreaRequest,
) -> Option<(ClippedRectangle, ClippedRectangle)> {
    if request.width == 0 || request.height == 0 {
        return None;
    }
    let start_x = 0i32
        .max(-i32::from(request.src_x))
        .max(-i32::from(request.dst_x));
    let start_y = 0i32
        .max(-i32::from(request.src_y))
        .max(-i32::from(request.dst_y));
    let end_x = i32::from(request.width)
        .min(i32::from(source.width) - i32::from(request.src_x))
        .min(i32::from(destination.width) - i32::from(request.dst_x));
    let end_y = i32::from(request.height)
        .min(i32::from(source.height) - i32::from(request.src_y))
        .min(i32::from(destination.height) - i32::from(request.dst_y));
    if end_x <= start_x || end_y <= start_y {
        return None;
    }
    let width = u16::try_from(end_x - start_x).ok()?;
    let height = u16::try_from(end_y - start_y).ok()?;
    Some((
        ClippedRectangle {
            x: u16::try_from(i32::from(request.src_x) + start_x).ok()?,
            y: u16::try_from(i32::from(request.src_y) + start_y).ok()?,
            width,
            height,
        },
        ClippedRectangle {
            x: u16::try_from(i32::from(request.dst_x) + start_x).ok()?,
            y: u16::try_from(i32::from(request.dst_y) + start_y).ok()?,
            width,
            height,
        },
    ))
}
