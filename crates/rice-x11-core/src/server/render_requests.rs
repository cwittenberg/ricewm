//! Render requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            0 => {
                let mut output = Vec::with_capacity(32);
                x11::write_render_query_version_reply(&mut output, order, sequence, 0, 11)
                    .map_err(|_| encoding_error())?;
                Ok(output)
            }
            1 => {
                let mut output = Vec::with_capacity(256);
                x11::write_render_query_pict_formats_reply(
                    &mut output,
                    order,
                    sequence,
                    ROOT_VISUAL,
                    ARGB_VISUAL,
                )
                .map_err(|_| encoding_error())?;
                Ok(output)
            }
            2 => {
                let mut output = Vec::with_capacity(32);
                x11::write_render_query_pict_index_values_reply(&mut output, order, sequence)
                    .map_err(|_| encoding_error())?;
                Ok(output)
            }
            4 => self.render_create_picture(handle, order, body),
            5 => self.render_change_picture(order, body),
            6 => self.render_set_picture_clip_rectangles(order, body),
            7 => self.render_free_picture(order, body),
            8 => self.render_composite(order, body),
            10..=13 => self.render_composite_geometry(order, minor, body),
            17 => self.render_create_glyphset(handle, order, body),
            18 => self.render_reference_glyphset(handle, order, body),
            19 => self.render_free_glyphset(order, body),
            20 => self.render_add_glyphs(order, body),
            22 => self.render_free_glyphs(order, body),
            23..=25 => self.render_composite_glyphs(order, minor, body),
            26 => self.render_fill_rectangles(order, body),
            27 => self.render_create_cursor(handle, order, body),
            28 => self.render_set_picture_transform(order, body),
            29 => {
                let drawable = ResourceId(read_u32(order, body.get(0..4).ok_or_else(bad_length)?));
                if self.surfaces.contains_key(&drawable.0) {
                    let mut output = Vec::new();
                    x11::write_render_query_filters_reply(&mut output, order, sequence)
                        .map_err(|_| encoding_error())?;
                    Ok(output)
                } else {
                    Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))
                }
            }
            30 => self.render_set_picture_filter(order, body),
            31 => self.render_create_animated_cursor(handle, order, body),
            33 => self.render_create_solid_fill(handle, order, body),
            34..=36 => self.render_create_gradient(handle, order, minor, body),
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|mut error| {
            error.minor_opcode = u16::from(minor);
            error
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_composite_geometry(
        &mut self,
        order: ClientByteOrder,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 20 {
            return Err(bad_length());
        }
        let operation = body[0];
        let source_id = ResourceId(read_u32(order, &body[4..8]));
        let destination_id = ResourceId(read_u32(order, &body[8..12]));
        let mask_format = read_u32(order, &body[12..16]);
        let src_x = read_i16(order, &body[16..18]);
        let src_y = read_i16(order, &body[18..20]);
        let source_picture = self
            .pictures
            .get(&source_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, source_id.0))?;
        let destination_picture = self
            .pictures
            .get(&destination_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, destination_id.0))?;
        let PictureSource::Drawable(destination_drawable) = destination_picture.source else {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, destination_id.0));
        };
        if mask_format != 0 && render_format_depth(mask_format).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, mask_format));
        }

        let primitives = &body[20..];
        let mut polygons: Vec<Vec<(f64, f64)>> = Vec::new();
        let source_anchor = match minor {
            10 => {
                if !primitives.len().is_multiple_of(40) {
                    return Err(bad_length());
                }
                let mut anchor = None;
                for trap in primitives.chunks_exact(40) {
                    let top = fixed_to_f64(read_i32(order, &trap[0..4]));
                    let bottom = fixed_to_f64(read_i32(order, &trap[4..8]));
                    let left_p1 = fixed_point(order, &trap[8..16]);
                    let left_p2 = fixed_point(order, &trap[16..24]);
                    let right_p1 = fixed_point(order, &trap[24..32]);
                    let right_p2 = fixed_point(order, &trap[32..40]);
                    anchor.get_or_insert(left_p1);
                    if bottom <= top {
                        continue;
                    }
                    let left_top = line_x_at_y(left_p1, left_p2, top);
                    let left_bottom = line_x_at_y(left_p1, left_p2, bottom);
                    let right_top = line_x_at_y(right_p1, right_p2, top);
                    let right_bottom = line_x_at_y(right_p1, right_p2, bottom);
                    polygons.push(vec![
                        (left_top, top),
                        (right_top, top),
                        (right_bottom, bottom),
                        (left_bottom, bottom),
                    ]);
                }
                anchor
            }
            11 => {
                if !primitives.len().is_multiple_of(24) {
                    return Err(bad_length());
                }
                let mut anchor = None;
                for triangle in primitives.chunks_exact(24) {
                    let points = vec![
                        fixed_point(order, &triangle[0..8]),
                        fixed_point(order, &triangle[8..16]),
                        fixed_point(order, &triangle[16..24]),
                    ];
                    anchor.get_or_insert(points[0]);
                    polygons.push(points);
                }
                anchor
            }
            12 | 13 => {
                if !primitives.len().is_multiple_of(8) {
                    return Err(bad_length());
                }
                let points: Vec<(f64, f64)> = primitives
                    .chunks_exact(8)
                    .map(|point| fixed_point(order, point))
                    .collect();
                let anchor = points.first().copied();
                if minor == 12 {
                    for points in points.windows(3) {
                        polygons.push(points.to_vec());
                    }
                } else if points.len() >= 3 {
                    for index in 1..points.len() - 1 {
                        polygons.push(vec![points[0], points[index], points[index + 1]]);
                    }
                }
                anchor
            }
            _ => unreachable!(),
        };
        let Some(source_anchor) = source_anchor else {
            return Ok(Vec::new());
        };
        let destination = self
            .surfaces
            .get(&destination_drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, destination_drawable.0))?;
        let Some(bounds) = polygon_bounds(&polygons, destination.width, destination.height) else {
            return Ok(Vec::new());
        };
        let coverage = rasterize_polygons(&polygons, bounds);
        if coverage.iter().all(|alpha| *alpha == 0) {
            return Ok(Vec::new());
        }
        let source_x = i32::from(src_x)
            .saturating_sub(f64_to_i32(source_anchor.0.floor()))
            .saturating_add(i32::from(bounds.x));
        let source_y = i32::from(src_y)
            .saturating_sub(f64_to_i32(source_anchor.1.floor()))
            .saturating_add(i32::from(bounds.y));
        let source = self.render_picture_samples(
            &source_picture,
            source_x,
            source_y,
            bounds.width,
            bounds.height,
        )?;
        self.render_paint_samples(
            operation,
            destination_id,
            i16::try_from(bounds.x).unwrap_or(i16::MAX),
            i16::try_from(bounds.y).unwrap_or(i16::MAX),
            bounds.width,
            bounds.height,
            &source,
            None,
            false,
            Some(&coverage),
        )?;
        Ok(Vec::new())
    }
}
