//! Render compositing operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn render_composite(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 32 {
            return Err(bad_length());
        }
        let operation = body[0];
        let source_id = ResourceId(read_u32(order, &body[4..8]));
        let mask_id = ResourceId(read_u32(order, &body[8..12]));
        let destination_id = ResourceId(read_u32(order, &body[12..16]));
        let src_x = read_i16(order, &body[16..18]);
        let src_y = read_i16(order, &body[18..20]);
        let mask_x = read_i16(order, &body[20..22]);
        let mask_y = read_i16(order, &body[22..24]);
        let dst_x = read_i16(order, &body[24..26]);
        let dst_y = read_i16(order, &body[26..28]);
        let width = read_u16(order, &body[28..30]);
        let height = read_u16(order, &body[30..32]);
        let source_picture = self
            .pictures
            .get(&source_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, source_id.0))?;
        let source = self.render_picture_samples(
            &source_picture,
            i32::from(src_x),
            i32::from(src_y),
            width,
            height,
        )?;
        let (mask, mask_component_alpha) = if mask_id.0 == 0 {
            (None, false)
        } else {
            let picture = self
                .pictures
                .get(&mask_id.0)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, mask_id.0))?;
            (
                Some(self.render_picture_samples(
                    picture,
                    i32::from(mask_x),
                    i32::from(mask_y),
                    width,
                    height,
                )?),
                picture.component_alpha,
            )
        };
        self.render_paint_samples(
            operation,
            destination_id,
            dst_x,
            dst_y,
            width,
            height,
            &source,
            mask.as_deref(),
            mask_component_alpha,
            None,
        )?;
        Ok(Vec::new())
    }

    pub(super) fn render_fill_rectangles(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 16 || !(body.len() - 16).is_multiple_of(8) {
            return Err(bad_length());
        }
        let operation = body[0];
        let destination = ResourceId(read_u32(order, &body[4..8]));
        if !self.pictures.contains_key(&destination.0) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, destination.0));
        }
        let pixel = render_pixel_from_argb(render_color(order, &body[8..16]));
        for rectangle in body[16..].chunks_exact(8) {
            let x = read_i16(order, &rectangle[0..2]);
            let y = read_i16(order, &rectangle[2..4]);
            let width = read_u16(order, &rectangle[4..6]);
            let height = read_u16(order, &rectangle[6..8]);
            let count = usize::from(width)
                .checked_mul(usize::from(height))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            self.render_paint_samples(
                operation,
                destination,
                x,
                y,
                width,
                height,
                &vec![pixel; count],
                None,
                false,
                None,
            )?;
        }
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_composite_glyphs(
        &mut self,
        order: ClientByteOrder,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 24 {
            return Err(bad_length());
        }
        let operation = body[0];
        let source_id = ResourceId(read_u32(order, &body[4..8]));
        let destination = ResourceId(read_u32(order, &body[8..12]));
        let mut active_glyphset = ResourceId(read_u32(order, &body[16..20]));
        let src_x = read_i16(order, &body[20..22]);
        let src_y = read_i16(order, &body[22..24]);
        let source_picture = self
            .pictures
            .get(&source_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, source_id.0))?;
        if !self.pictures.contains_key(&destination.0) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, destination.0));
        }
        if !self.glyphsets.contains_key(&active_glyphset.0) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, active_glyphset.0));
        }
        let id_size = match minor {
            23 => 1,
            24 => 2,
            25 => 4,
            _ => unreachable!(),
        };
        let mut pen_x = 0i32;
        let mut pen_y = 0i32;
        let mut position = 24;
        while position < body.len() {
            if body.len() - position < 8 {
                return Err(bad_length());
            }
            let count = usize::from(body[position]);
            if count == 255 {
                active_glyphset = ResourceId(read_u32(order, &body[position + 4..position + 8]));
                if !self.glyphsets.contains_key(&active_glyphset.0) {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, active_glyphset.0));
                }
                position += 8;
                continue;
            }
            pen_x = pen_x.saturating_add(i32::from(read_i16(
                order,
                &body[position + 4..position + 6],
            )));
            pen_y = pen_y.saturating_add(i32::from(read_i16(
                order,
                &body[position + 6..position + 8],
            )));
            let payload_bytes = count
                .checked_mul(id_size)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let padded = x11::pad4(payload_bytes);
            let payload_start = position + 8;
            let payload_end = payload_start
                .checked_add(padded)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            if payload_end > body.len() {
                return Err(bad_length());
            }
            for index in 0..count {
                let offset = payload_start + index * id_size;
                let glyph_id = match id_size {
                    1 => u32::from(body[offset]),
                    2 => u32::from(read_u16(order, &body[offset..offset + 2])),
                    4 => read_u32(order, &body[offset..offset + 4]),
                    _ => unreachable!(),
                };
                let glyph = self
                    .glyphsets
                    .get(&active_glyphset.0)
                    .and_then(|glyphset| self.glyph_storage.get(&glyphset.storage))
                    .and_then(|glyphs| glyphs.get(&glyph_id))
                    .cloned();
                let Some(glyph) = glyph else {
                    continue;
                };
                if glyph.width != 0 && glyph.height != 0 {
                    let dst_x = clamp_i16(pen_x.saturating_sub(i32::from(glyph.x)));
                    let dst_y = clamp_i16(pen_y.saturating_sub(i32::from(glyph.y)));
                    let source = self.render_picture_samples(
                        &source_picture,
                        i32::from(src_x),
                        i32::from(src_y),
                        glyph.width,
                        glyph.height,
                    )?;
                    self.render_paint_samples(
                        operation,
                        destination,
                        dst_x,
                        dst_y,
                        glyph.width,
                        glyph.height,
                        &source,
                        None,
                        false,
                        Some(&glyph.alpha),
                    )?;
                }
                pen_x = pen_x.saturating_add(i32::from(glyph.x_off));
                pen_y = pen_y.saturating_add(i32::from(glyph.y_off));
            }
            position = payload_end;
        }
        Ok(Vec::new())
    }
}
