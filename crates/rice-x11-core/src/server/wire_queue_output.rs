//! Protocol encoding and raster helpers.
use super::*;

pub(super) fn queue_output(
    client: &mut Client,
    output: &[u8],
    max_egress_bytes: usize,
) -> Result<(), ClientError> {
    if output.len() > max_egress_bytes.saturating_sub(client.egress.len()) {
        return Err(ClientError::EgressLimit);
    }
    client.egress.extend(output);
    Ok(())
}

pub(super) fn render_format_depth(format: u32) -> Option<u8> {
    match format {
        x11::RENDER_FMT_A1 => Some(1),
        x11::RENDER_FMT_A8 => Some(8),
        x11::RENDER_FMT_RGB24 => Some(24),
        x11::RENDER_FMT_ARGB32 | x11::RENDER_FMT_XRGB32 => Some(32),
        _ => None,
    }
}

pub(super) fn low_i16(value: u32) -> i16 {
    let low = u16::try_from(value & 0xffff).unwrap_or_default();
    i16::from_le_bytes(low.to_le_bytes())
}

pub(super) fn render_color(order: ClientByteOrder, bytes: &[u8]) -> u32 {
    let red = render_component_u8(read_u16(order, &bytes[0..2]));
    let green = render_component_u8(read_u16(order, &bytes[2..4]));
    let blue = render_component_u8(read_u16(order, &bytes[4..6]));
    let alpha = render_component_u8(read_u16(order, &bytes[6..8]));
    let red = multiply_u8(red, alpha);
    let green = multiply_u8(green, alpha);
    let blue = multiply_u8(blue, alpha);
    u32::from(alpha) << 24 | u32::from(red) << 16 | u32::from(green) << 8 | u32::from(blue)
}

pub(super) fn render_component_u8(value: u16) -> u8 {
    u8::try_from((u32::from(value) + 128) / 257).unwrap_or(u8::MAX)
}

pub(super) fn render_pixel_from_argb(pixel: u32) -> RenderPixel {
    RenderPixel {
        r: u8::try_from((pixel >> 16) & 0xff).unwrap_or_default(),
        g: u8::try_from((pixel >> 8) & 0xff).unwrap_or_default(),
        b: u8::try_from(pixel & 0xff).unwrap_or_default(),
        a: u8::try_from((pixel >> 24) & 0xff).unwrap_or_default(),
    }
}

pub(super) fn render_pixel_from_surface(surface: &Surface, format: u32, x: u16, y: u16) -> RenderPixel {
    render_pixel_from_raw(surface.pixel(x, y), surface.depth, format)
}

pub(super) fn render_pixel_from_raw(pixel: u32, depth: u8, format: u32) -> RenderPixel {
    match format {
        x11::RENDER_FMT_A1 => RenderPixel {
            a: if pixel & 1 == 0 { 0 } else { 255 },
            ..RenderPixel::default()
        },
        x11::RENDER_FMT_A8 => RenderPixel {
            a: u8::try_from(pixel & 0xff).unwrap_or_default(),
            ..RenderPixel::default()
        },
        x11::RENDER_FMT_ARGB32 if depth == 32 => render_pixel_from_argb(pixel),
        _ => RenderPixel {
            r: u8::try_from((pixel >> 16) & 0xff).unwrap_or_default(),
            g: u8::try_from((pixel >> 8) & 0xff).unwrap_or_default(),
            b: u8::try_from(pixel & 0xff).unwrap_or_default(),
            a: 255,
        },
    }
}

pub(super) fn render_pixel_to_raw(pixel: RenderPixel, depth: u8, format: u32) -> u32 {
    match format {
        x11::RENDER_FMT_A1 => u32::from(pixel.a >= 128),
        x11::RENDER_FMT_A8 => u32::from(pixel.a),
        x11::RENDER_FMT_ARGB32 if depth == 32 => {
            u32::from(pixel.a) << 24
                | u32::from(pixel.r) << 16
                | u32::from(pixel.g) << 8
                | u32::from(pixel.b)
        }
        _ => u32::from(pixel.r) << 16 | u32::from(pixel.g) << 8 | u32::from(pixel.b),
    }
}

pub(super) fn multiply_u8(left: u8, right: u8) -> u8 {
    u8::try_from((u32::from(left) * u32::from(right) + 127) / 255).unwrap_or(u8::MAX)
}

pub(super) fn render_scale_pixel(pixel: RenderPixel, scale: u8) -> RenderPixel {
    RenderPixel {
        r: multiply_u8(pixel.r, scale),
        g: multiply_u8(pixel.g, scale),
        b: multiply_u8(pixel.b, scale),
        a: multiply_u8(pixel.a, scale),
    }
}

pub(super) fn render_apply_mask(source: RenderPixel, mask: RenderPixel, component_alpha: bool) -> RenderPixel {
    if component_alpha {
        RenderPixel {
            r: multiply_u8(source.r, mask.r),
            g: multiply_u8(source.g, mask.g),
            b: multiply_u8(source.b, mask.b),
            a: multiply_u8(source.a, mask.a),
        }
    } else {
        render_scale_pixel(source, mask.a)
    }
}

pub(super) fn render_blend(
    operation: u8,
    source: RenderPixel,
    destination: RenderPixel,
) -> Result<RenderPixel, ProtocolError> {
    if operation == 12 {
        return Ok(RenderPixel {
            r: source.r.saturating_add(destination.r),
            g: source.g.saturating_add(destination.g),
            b: source.b.saturating_add(destination.b),
            a: source.a.saturating_add(destination.a),
        });
    }
    let (source_factor, destination_factor) = match operation {
        0 => (0, 0),
        1 => (255, 0),
        2 => (0, 255),
        3 => (255, 255u8.saturating_sub(source.a)),
        4 => (255u8.saturating_sub(destination.a), 255),
        5 => (destination.a, 0),
        6 => (0, source.a),
        7 => (255u8.saturating_sub(destination.a), 0),
        8 => (0, 255u8.saturating_sub(source.a)),
        9 => (destination.a, 255u8.saturating_sub(source.a)),
        10 => (255u8.saturating_sub(destination.a), source.a),
        11 => (
            255u8.saturating_sub(destination.a),
            255u8.saturating_sub(source.a),
        ),
        13 => {
            let available = u32::from(255u8.saturating_sub(destination.a));
            let factor = if source.a == 0 {
                255
            } else {
                u8::try_from((available * 255 / u32::from(source.a)).min(255)).unwrap_or(u8::MAX)
            };
            (factor, 255)
        }
        _ => {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(operation),
            ));
        }
    };
    let combine = |source: u8, destination: u8| {
        multiply_u8(source, source_factor)
            .saturating_add(multiply_u8(destination, destination_factor))
    };
    Ok(RenderPixel {
        r: combine(source.r, destination.r),
        g: combine(source.g, destination.g),
        b: combine(source.b, destination.b),
        a: combine(source.a, destination.a),
    })
}

pub(super) fn repeat_coordinate(x: i32, y: i32, width: u16, height: u16, repeat: u8) -> Option<(u16, u16)> {
    let width = i32::from(width);
    let height = i32::from(height);
    if width == 0 || height == 0 {
        return None;
    }
    let axis = |coordinate: i32, extent: i32| match repeat {
        0 => (0..extent).contains(&coordinate).then_some(coordinate),
        1 => Some(coordinate.rem_euclid(extent)),
        2 => Some(coordinate.clamp(0, extent - 1)),
        3 => {
            let period = extent.saturating_mul(2);
            let reflected = coordinate.rem_euclid(period);
            Some(if reflected < extent {
                reflected
            } else {
                period - reflected - 1
            })
        }
        _ => None,
    };
    Some((
        u16::try_from(axis(x, width)?).ok()?,
        u16::try_from(axis(y, height)?).ok()?,
    ))
}

pub(super) const fn render_identity_transform() -> [i32; 9] {
    [65_536, 0, 0, 0, 65_536, 0, 0, 0, 65_536]
}

pub(super) fn transform_picture_point(transform: &[i32; 9], x: f64, y: f64) -> (f64, f64) {
    let fixed = |value: i32| f64::from(value) / 65_536.0;
    let transformed_x = fixed(transform[0]) * x + fixed(transform[1]) * y + fixed(transform[2]);
    let transformed_y = fixed(transform[3]) * x + fixed(transform[4]) * y + fixed(transform[5]);
    let transformed_w = fixed(transform[6]) * x + fixed(transform[7]) * y + fixed(transform[8]);
    if transformed_w.abs() < f64::EPSILON {
        (transformed_x, transformed_y)
    } else {
        (transformed_x / transformed_w, transformed_y / transformed_w)
    }
}

pub(super) fn render_sample_surface(
    surface: &Surface,
    format: u32,
    x: f64,
    y: f64,
    repeat: u8,
    filter: PictureFilter,
) -> RenderPixel {
    let sample = |sample_x: i32, sample_y: i32| {
        repeat_coordinate(sample_x, sample_y, surface.width, surface.height, repeat)
            .map_or_else(RenderPixel::default, |(sample_x, sample_y)| {
                render_pixel_from_surface(surface, format, sample_x, sample_y)
            })
    };
    if filter == PictureFilter::Nearest {
        return sample(f64_to_i32(x.floor()), f64_to_i32(y.floor()));
    }
    let shifted_x = x - 0.5;
    let shifted_y = y - 0.5;
    let left = f64_to_i32(shifted_x.floor());
    let top = f64_to_i32(shifted_y.floor());
    let fraction_x = shifted_x - f64::from(left);
    let fraction_y = shifted_y - f64::from(top);
    let top_pixel = render_mix_pixels(
        sample(left, top),
        sample(left.saturating_add(1), top),
        fraction_x,
    );
    let bottom_pixel = render_mix_pixels(
        sample(left, top.saturating_add(1)),
        sample(left.saturating_add(1), top.saturating_add(1)),
        fraction_x,
    );
    render_mix_pixels(top_pixel, bottom_pixel, fraction_y)
}

pub(super) fn render_mix_pixels(first: RenderPixel, second: RenderPixel, amount: f64) -> RenderPixel {
    let amount = amount.clamp(0.0, 1.0);
    let mix = |first: u8, second: u8| {
        f64_to_u8(f64::from(first) * (1.0 - amount) + f64::from(second) * amount)
    };
    RenderPixel {
        r: mix(first.r, second.r),
        g: mix(first.g, second.g),
        b: mix(first.b, second.b),
        a: mix(first.a, second.a),
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn f64_to_u8(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

pub(super) fn render_gradient_pixel(source: &PictureSource, x: f64, y: f64, repeat: u8) -> RenderPixel {
    let (parameter, stops) = match source {
        PictureSource::LinearGradient {
            first,
            second,
            stops,
        } => {
            let first = (fixed_to_f64(first.0), fixed_to_f64(first.1));
            let direction = (
                fixed_to_f64(second.0) - first.0,
                fixed_to_f64(second.1) - first.1,
            );
            let length_squared = direction.0 * direction.0 + direction.1 * direction.1;
            let parameter = if length_squared <= f64::EPSILON {
                0.0
            } else {
                ((x - first.0) * direction.0 + (y - first.1) * direction.1) / length_squared
            };
            (parameter, stops)
        }
        PictureSource::RadialGradient {
            inner,
            outer,
            inner_radius,
            outer_radius,
            stops,
        } => {
            let inner = (fixed_to_f64(inner.0), fixed_to_f64(inner.1));
            let center_delta = (
                fixed_to_f64(outer.0) - inner.0,
                fixed_to_f64(outer.1) - inner.1,
            );
            let inner_radius = fixed_to_f64(*inner_radius);
            let radius_delta = fixed_to_f64(*outer_radius) - inner_radius;
            let relative = (x - inner.0, y - inner.1);
            let quadratic = center_delta.0 * center_delta.0 + center_delta.1 * center_delta.1
                - radius_delta * radius_delta;
            let linear = -2.0
                * (relative.0 * center_delta.0
                    + relative.1 * center_delta.1
                    + inner_radius * radius_delta);
            let constant =
                relative.0 * relative.0 + relative.1 * relative.1 - inner_radius * inner_radius;
            let parameter = solve_radial_parameter(quadratic, linear, constant);
            (parameter, stops)
        }
        PictureSource::ConicalGradient {
            center,
            angle,
            stops,
        } => {
            let center = (fixed_to_f64(center.0), fixed_to_f64(center.1));
            let start = fixed_to_f64(*angle).to_radians();
            let parameter = ((y - center.1).atan2(x - center.0) - start) / std::f64::consts::TAU;
            (parameter, stops)
        }
        PictureSource::Drawable(_) | PictureSource::Solid(_) => {
            return RenderPixel::default();
        }
    };
    let Some(parameter) = repeat_gradient_parameter(parameter, repeat) else {
        return RenderPixel::default();
    };
    render_gradient_stops(stops, parameter)
}

pub(super) fn solve_radial_parameter(quadratic: f64, linear: f64, constant: f64) -> f64 {
    if quadratic.abs() < f64::EPSILON {
        return if linear.abs() < f64::EPSILON {
            0.0
        } else {
            -constant / linear
        };
    }
    let discriminant = linear * linear - 4.0 * quadratic * constant;
    if discriminant < 0.0 {
        return -1.0;
    }
    let root = discriminant.sqrt();
    let first = (-linear - root) / (2.0 * quadratic);
    let second = (-linear + root) / (2.0 * quadratic);
    [first, second]
        .into_iter()
        .filter(|value| *value >= 0.0)
        .reduce(f64::min)
        .unwrap_or(first.max(second))
}

pub(super) fn repeat_gradient_parameter(parameter: f64, repeat: u8) -> Option<f64> {
    match repeat {
        0 => (0.0..=1.0).contains(&parameter).then_some(parameter),
        1 => Some(parameter.rem_euclid(1.0)),
        2 => Some(parameter.clamp(0.0, 1.0)),
        3 => {
            let reflected = parameter.rem_euclid(2.0);
            Some(if reflected <= 1.0 {
                reflected
            } else {
                2.0 - reflected
            })
        }
        _ => None,
    }
}

pub(super) fn render_gradient_stops(stops: &[GradientStop], parameter: f64) -> RenderPixel {
    let parameter = parameter * 65_536.0;
    let Some(first) = stops.first() else {
        return RenderPixel::default();
    };
    if parameter <= f64::from(first.position) {
        return render_pixel_from_argb(first.color);
    }
    for pair in stops.windows(2) {
        if parameter <= f64::from(pair[1].position) {
            let span = f64::from(pair[1].position) - f64::from(pair[0].position);
            let amount = if span.abs() < f64::EPSILON {
                1.0
            } else {
                (parameter - f64::from(pair[0].position)) / span
            };
            return render_mix_pixels(
                render_pixel_from_argb(pair[0].color),
                render_pixel_from_argb(pair[1].color),
                amount,
            );
        }
    }
    render_pixel_from_argb(stops.last().unwrap().color)
}

pub(super) fn decode_core_characters(bytes: &[u8], two_byte: bool) -> Vec<u16> {
    if two_byte {
        bytes
            .chunks_exact(2)
            .map(|character| u16::from_be_bytes([character[0], character[1]]))
            .collect()
    } else {
        bytes.iter().copied().map(u16::from).collect()
    }
}

#[allow(clippy::too_many_lines)]
pub(super) fn core_font_glyph(character: u16) -> [u8; 7] {
    let character = u8::try_from(character & 0xff).unwrap_or(b'?');
    match character {
        b' ' => [0, 0, 0, 0, 0, 0, 0],
        b'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        b'D' => [0x1e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1e],
        b'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        b'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
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
        b'a' => [0, 0x0e, 0x01, 0x0f, 0x11, 0x13, 0x0d],
        b'b' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x1e],
        b'c' => [0, 0, 0x0e, 0x10, 0x10, 0x11, 0x0e],
        b'd' => [0x01, 0x01, 0x0d, 0x13, 0x11, 0x11, 0x0f],
        b'e' => [0, 0x0e, 0x11, 0x1f, 0x10, 0x11, 0x0e],
        b'f' => [0x06, 0x09, 0x08, 0x1c, 0x08, 0x08, 0x08],
        b'g' => [0, 0x0f, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        b'h' => [0x10, 0x10, 0x16, 0x19, 0x11, 0x11, 0x11],
        b'i' => [0x04, 0, 0x0c, 0x04, 0x04, 0x04, 0x0e],
        b'j' => [0x02, 0, 0x06, 0x02, 0x02, 0x12, 0x0c],
        b'k' => [0x10, 0x10, 0x12, 0x14, 0x18, 0x14, 0x12],
        b'l' => [0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'm' => [0, 0, 0x1a, 0x15, 0x15, 0x15, 0x15],
        b'n' => [0, 0, 0x16, 0x19, 0x11, 0x11, 0x11],
        b'o' => [0, 0, 0x0e, 0x11, 0x11, 0x11, 0x0e],
        b'p' => [0, 0, 0x1e, 0x11, 0x1e, 0x10, 0x10],
        b'q' => [0, 0, 0x0f, 0x11, 0x0f, 0x01, 0x01],
        b'r' => [0, 0, 0x16, 0x19, 0x10, 0x10, 0x10],
        b's' => [0, 0, 0x0f, 0x10, 0x0e, 0x01, 0x1e],
        b't' => [0x08, 0x08, 0x1c, 0x08, 0x08, 0x09, 0x06],
        b'u' => [0, 0, 0x11, 0x11, 0x11, 0x13, 0x0d],
        b'v' => [0, 0, 0x11, 0x11, 0x11, 0x0a, 0x04],
        b'w' => [0, 0, 0x11, 0x11, 0x15, 0x15, 0x0a],
        b'x' => [0, 0, 0x11, 0x0a, 0x04, 0x0a, 0x11],
        b'y' => [0, 0, 0x11, 0x11, 0x0f, 0x01, 0x0e],
        b'z' => [0, 0, 0x1f, 0x02, 0x04, 0x08, 0x1f],
        b'0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        b'1' => [0x04, 0x0c, 0x14, 0x04, 0x04, 0x04, 0x1f],
        b'2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        b'3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        b'4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        b'5' => [0x1f, 0x10, 0x10, 0x1e, 0x01, 0x01, 0x1e],
        b'6' => [0x0e, 0x10, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        b'7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        b'9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x01, 0x0e],
        b'.' => [0, 0, 0, 0, 0, 0x0c, 0x0c],
        b',' => [0, 0, 0, 0, 0x0c, 0x0c, 0x08],
        b':' => [0, 0x0c, 0x0c, 0, 0x0c, 0x0c, 0],
        b';' => [0, 0x0c, 0x0c, 0, 0x0c, 0x04, 0x08],
        b'-' => [0, 0, 0, 0x1f, 0, 0, 0],
        b'_' => [0, 0, 0, 0, 0, 0, 0x1f],
        b'/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        b'\\' => [0x10, 0x08, 0x08, 0x04, 0x02, 0x02, 0x01],
        b'+' => [0, 0x04, 0x04, 0x1f, 0x04, 0x04, 0],
        b'=' => [0, 0, 0x1f, 0, 0x1f, 0, 0],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b'[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        b']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        b'!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0, 0x04],
        b'?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
        b'<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        b'>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        b'\"' => [0x0a, 0x0a, 0x0a, 0, 0, 0, 0],
        b'\'' => [0x04, 0x04, 0x08, 0, 0, 0, 0],
        b'#' => [0x0a, 0x1f, 0x0a, 0x0a, 0x1f, 0x0a, 0],
        b'*' => [0, 0x15, 0x0e, 0x1f, 0x0e, 0x15, 0],
        b'&' => [0x0c, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0d],
        b'@' => [0x0e, 0x11, 0x17, 0x15, 0x17, 0x10, 0x0e],
        b'$' => [0x04, 0x0f, 0x14, 0x0e, 0x05, 0x1e, 0x04],
        b'%' => [0x19, 0x1a, 0x02, 0x04, 0x08, 0x0b, 0x13],
        b'^' => [0x04, 0x0a, 0x11, 0, 0, 0, 0],
        b'|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'{' => [0x02, 0x04, 0x04, 0x08, 0x04, 0x04, 0x02],
        b'}' => [0x08, 0x04, 0x04, 0x02, 0x04, 0x04, 0x08],
        b'`' => [0x08, 0x04, 0x02, 0, 0, 0, 0],
        b'~' => [0, 0, 0x09, 0x16, 0, 0, 0],
        _ => [0x1f, 0x11, 0x01, 0x02, 0x04, 0, 0x04],
    }
}

pub(super) fn depth_plane_mask(depth: u8) -> u32 {
    if depth >= 32 {
        u32::MAX
    } else {
        (1u32 << depth) - 1
    }
}

pub(super) fn builtin_font_metrics() -> x11::FontMetrics {
    let glyph = x11::CharInfo {
        left_side_bearing: 0,
        right_side_bearing: 8,
        character_width: 8,
        ascent: 9,
        descent: 3,
        attributes: 0,
    };
    x11::FontMetrics {
        min_bounds: glyph,
        max_bounds: glyph,
        min_char_or_byte2: 0,
        max_char_or_byte2: 255,
        default_char: u16::from(b'?'),
        draw_direction: 0,
        min_byte1: 0,
        max_byte1: 0,
        all_chars_exist: true,
        font_ascent: 9,
        font_descent: 3,
        properties: Vec::new(),
        named_properties: Vec::new(),
        char_infos: vec![glyph; 256],
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_rect(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    rect_width: usize,
    rect_height: usize,
    color: u32,
) {
    let end_y = y.saturating_add(rect_height).min(height);
    let end_x = x.saturating_add(rect_width).min(width);
    for row in y.min(height)..end_y {
        pixels[row * width + x.min(width)..row * width + end_x].fill(color);
    }
}
