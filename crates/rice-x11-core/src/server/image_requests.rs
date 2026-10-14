//! Image requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn put_image(
        &mut self,
        byte_order: ClientByteOrder,
        format: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::put_image_request(format, body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(request.gc, request.drawable)?;
        let surface = self
            .surfaces
            .get(&request.drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.drawable.0))?;
        let Some(clipped) = clipped_rectangle(
            surface,
            request.dst_x,
            request.dst_y,
            request.width,
            request.height,
        ) else {
            return Ok(Vec::new());
        };
        let destination_width = usize::from(surface.width);
        let drawable_mask = depth_plane_mask(gc.depth);
        if request.format == x11::ImageFormat::ZPixmap
            && matches!(request.depth, 24 | 32)
            && request.depth == gc.depth
            && request.left_pad == 0
            && gc.function == 3
            && gc.plane_mask & drawable_mask == drawable_mask
            && gc.clip_mask.is_none()
            && gc.clip_rectangles.is_none()
        {
            let row_bytes = usize::from(request.width).saturating_mul(4);
            let required = row_bytes
                .checked_mul(usize::from(request.height))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let data = request
                .data
                .get(..required)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_LENGTH, 0))?;
            self.ensure_surface_materialized(request.drawable)?;
            let Some(destination) = self
                .surfaces
                .get_mut(&request.drawable.0)
                .and_then(|surface| surface.pixels.as_mut())
            else {
                return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
            };
            let source_x = usize::try_from(i32::from(clipped.x) - i32::from(request.dst_x))
                .map_err(|_| ProtocolError::new(x11::error::BAD_VALUE, 0))?;
            let source_y = usize::try_from(i32::from(clipped.y) - i32::from(request.dst_y))
                .map_err(|_| ProtocolError::new(x11::error::BAD_VALUE, 0))?;
            let mut changed_left = clipped.width;
            let mut changed_top = clipped.height;
            let mut changed_right = 0_u16;
            let mut changed_bottom = 0_u16;
            for row in 0..usize::from(clipped.height) {
                let source_row = source_y.saturating_add(row);
                let target_row = usize::from(clipped.y).saturating_add(row);
                let source_start = source_row
                    .saturating_mul(row_bytes)
                    .saturating_add(source_x.saturating_mul(4));
                let target_start = target_row
                    .saturating_mul(destination_width)
                    .saturating_add(usize::from(clipped.x));
                let copy_width = usize::from(clipped.width);
                let source_end = source_start.saturating_add(copy_width.saturating_mul(4));
                let source = data
                    .get(source_start..source_end)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_LENGTH, 0))?;
                let target = destination
                    .get_mut(target_start..target_start.saturating_add(copy_width))
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, 0))?;
                // Locate the changed span once per row, then copy it without
                // per-pixel damage bookkeeping. Moving video usually changes
                // both ends; static rows still avoid any write or scene damage.
                let Some(first) = source.chunks_exact(4).zip(target.iter())
                    .position(|(bytes, pixel)| read_u32(byte_order, bytes) & drawable_mask != *pixel)
                else { continue; };
                let last = source.chunks_exact(4).zip(target.iter())
                    .rposition(|(bytes, pixel)| read_u32(byte_order, bytes) & drawable_mask != *pixel)
                    .unwrap_or(first);
                for (bytes, pixel) in source[first * 4..(last + 1) * 4]
                    .chunks_exact(4).zip(&mut target[first..=last]) {
                    *pixel = read_u32(byte_order, bytes) & drawable_mask;
                }
                changed_left = changed_left.min(first as u16);
                changed_top = changed_top.min(row as u16);
                changed_right = changed_right.max((last + 1) as u16);
                changed_bottom = changed_bottom.max(row as u16 + 1);
            }
            let changed = (changed_right > changed_left && changed_bottom > changed_top)
                .then(|| ClippedRectangle {
                    x: clipped.x + changed_left,
                    y: clipped.y + changed_top,
                    width: changed_right - changed_left,
                    height: changed_bottom - changed_top,
                });
            // Clients still receive damage for the requested write, but the
            // desktop only uploads and composes pixels that actually changed.
            self.damage_drawable_with_scene(request.drawable, clipped, changed);
            return Ok(Vec::new());
        }
        let pixels = decode_image(byte_order, &request, &gc)?;
        let coverage = self.gc_clip_coverage(&gc, clipped);
        self.ensure_surface_materialized(request.drawable)?;
        let Some(destination) = self
            .surfaces
            .get_mut(&request.drawable.0)
            .and_then(|surface| surface.pixels.as_mut())
        else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        for target_y in clipped.y..clipped.y.saturating_add(clipped.height) {
            for target_x in clipped.x..clipped.x.saturating_add(clipped.width) {
                let coverage_index = usize::from(target_y - clipped.y)
                    .saturating_mul(usize::from(clipped.width))
                    .saturating_add(usize::from(target_x - clipped.x));
                if !coverage[coverage_index] {
                    continue;
                }
                let source_x = i32::from(target_x) - i32::from(request.dst_x);
                let source_y = i32::from(target_y) - i32::from(request.dst_y);
                let source_index = usize::try_from(source_y)
                    .ok()
                    .and_then(|row| row.checked_mul(usize::from(request.width)))
                    .and_then(|base| {
                        usize::try_from(source_x)
                            .ok()
                            .and_then(|column| base.checked_add(column))
                    })
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, 0))?;
                let destination_index = usize::from(target_y)
                    .saturating_mul(destination_width)
                    .saturating_add(usize::from(target_x));
                let source = pixels[source_index];
                let current = destination[destination_index];
                destination[destination_index] = apply_gc_function(
                    current,
                    source,
                    gc.function,
                    gc.plane_mask,
                    depth_plane_mask(gc.depth),
                );
            }
        }
        self.damage_drawable(request.drawable, clipped);
        Ok(Vec::new())
    }

    pub(super) fn get_image(
        &self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        format: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::get_image_request(format, body).ok_or_else(bad_length)?;
        let surface = self
            .surfaces
            .get(&request.drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.drawable.0))?;
        let right = i32::from(request.x).saturating_add(i32::from(request.width));
        let bottom = i32::from(request.y).saturating_add(i32::from(request.height));
        if request.x < 0
            || request.y < 0
            || request.width == 0
            || request.height == 0
            || right > i32::from(surface.width)
            || bottom > i32::from(surface.height)
        {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                request.drawable.0,
            ));
        }
        let plane_mask = request.plane_mask & depth_plane_mask(surface.depth);
        let pixels = if format == 2 {
            encode_z_pixmap(byte_order, surface, &request, plane_mask)?
        } else if format == 1 {
            encode_xy_pixmap(byte_order, surface, &request, plane_mask)?
        } else {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(format)));
        };
        let visual = self
            .windows
            .get(&request.drawable.0)
            .map_or(0, |window| window.visual.0);
        encode_get_image_reply(byte_order, sequence, surface.depth, visual, &pixels)
    }

    pub(super) fn image_text(
        &mut self,
        character_count: u8,
        body: &[u8],
        two_byte: bool,
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 12 {
            return Err(bad_length());
        }
        let drawable = ResourceId(read_u32_le(&body[0..4]));
        let gc_id = ResourceId(read_u32_le(&body[4..8]));
        let x = read_i16_le(&body[8..10]);
        let y = read_i16_le(&body[10..12]);
        let character_bytes =
            usize::from(character_count).saturating_mul(if two_byte { 2 } else { 1 });
        let text = body
            .get(12..12usize.saturating_add(character_bytes))
            .ok_or_else(bad_length)?;
        if body.len() != 12usize.saturating_add(x11::pad4(character_bytes)) {
            return Err(bad_length());
        }
        let characters = decode_core_characters(text, two_byte);
        self.draw_core_text(drawable, gc_id, x, y, &characters, true)?;
        Ok(Vec::new())
    }

    pub(super) fn poly_text(
        &mut self,
        _unused: u8,
        body: &[u8],
        two_byte: bool,
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 12 {
            return Err(bad_length());
        }
        let drawable = ResourceId(read_u32_le(&body[0..4]));
        let gc_id = ResourceId(read_u32_le(&body[4..8]));
        let mut x = i32::from(read_i16_le(&body[8..10]));
        let y = read_i16_le(&body[10..12]);
        let character_size = if two_byte { 2 } else { 1 };
        let mut position = 12;
        while position < body.len() {
            let count = usize::from(body[position]);
            if count == 0 {
                break;
            }
            if count == 255 {
                let font_bytes = body
                    .get(position + 1..position + 5)
                    .ok_or_else(bad_length)?;
                let font = ResourceId(u32::from_be_bytes(font_bytes.try_into().unwrap()));
                if !self.fonts.contains_key(&font.0) {
                    return Err(ProtocolError::new(x11::error::BAD_FONT, font.0));
                }
                if let Some(gc) = self.graphics_contexts.get_mut(&gc_id.0) {
                    gc.font = Some(font);
                }
                position += 5;
                continue;
            }
            let delta = *body.get(position + 1).ok_or_else(bad_length)?;
            x = x.saturating_add(i32::from(i8::from_ne_bytes([delta])));
            let bytes = count
                .checked_mul(character_size)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let text = body
                .get(position + 2..position + 2 + bytes)
                .ok_or_else(bad_length)?;
            let characters = decode_core_characters(text, two_byte);
            self.draw_core_text(drawable, gc_id, clamp_i16(x), y, &characters, false)?;
            x = x.saturating_add(
                i32::try_from(characters.len())
                    .unwrap_or(i32::MAX)
                    .saturating_mul(8),
            );
            position = position.saturating_add(2 + bytes);
        }
        Ok(Vec::new())
    }

    pub(super) fn draw_core_text(
        &mut self,
        drawable: ResourceId,
        gc_id: ResourceId,
        x: i16,
        baseline_y: i16,
        characters: &[u16],
        image: bool,
    ) -> Result<(), ProtocolError> {
        let gc = self.gc_for_drawable(gc_id, drawable)?;
        if gc
            .font
            .is_some_and(|font| !self.fonts.contains_key(&font.0))
        {
            return Err(ProtocolError::new(x11::error::BAD_FONT, gc.font.unwrap().0));
        }
        let text_width = u16::try_from(characters.len().saturating_mul(8)).unwrap_or(u16::MAX);
        let top = baseline_y.saturating_sub(9);
        let surface = self
            .surfaces
            .get(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        if image && let Some(rectangle) = clipped_rectangle(surface, x, top, text_width, 12) {
            let mut background_gc = gc.clone();
            background_gc.foreground = gc.background;
            background_gc.fill_style = 0;
            if self.draw_gc_rectangle(drawable, rectangle, &background_gc)? {
                self.damage_drawable(drawable, rectangle);
            }
        }
        let mut raster = Vec::new();
        for (index, character) in characters.iter().copied().enumerate() {
            let glyph = core_font_glyph(character);
            let glyph_x = i32::from(x)
                .saturating_add(i32::try_from(index).unwrap_or(i32::MAX).saturating_mul(8));
            let glyph_y = i32::from(baseline_y).saturating_sub(8);
            for (row, bits) in glyph.iter().copied().enumerate() {
                for column in 0..5 {
                    if bits & (1 << (4 - column)) != 0 {
                        raster.push((
                            glyph_x.saturating_add(column),
                            glyph_y.saturating_add(i32::try_from(row).unwrap_or(i32::MAX)),
                            gc.foreground,
                        ));
                    }
                }
            }
        }
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(())
    }

    pub(super) fn copy_area(&mut self, handle: ClientHandle, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::copy_area_request(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(request.gc, request.dst)?;
        let source = self
            .surfaces
            .get(&request.src.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.src.0))?;
        let destination = self
            .surfaces
            .get(&request.dst.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.dst.0))?;
        if source.depth != destination.depth {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, request.dst.0));
        }
        let Some((source_rectangle, destination_rectangle)) =
            clipped_copy_rectangles(source, destination, &request)
        else {
            return Ok(Vec::new());
        };
        let drawable_mask = depth_plane_mask(gc.depth);
        let plain_copy = gc.clip_rectangles.is_none()
            && gc.clip_mask.is_none()
            && gc.function == 3
            && gc.plane_mask & drawable_mask == drawable_mask;
        let mut copied = Vec::with_capacity(
            usize::from(source_rectangle.width)
                .saturating_mul(usize::from(source_rectangle.height)),
        );
        for y in source_rectangle.y..source_rectangle.y.saturating_add(source_rectangle.height) {
            let start =
                usize::from(y) * usize::from(source.width) + usize::from(source_rectangle.x);
            let width = usize::from(source_rectangle.width);
            if let Some(row) = source
                .pixels
                .as_ref()
                .and_then(|pixels| pixels.get(start..start + width))
            {
                copied.extend_from_slice(row);
            } else {
                for x in source_rectangle.x..source_rectangle.x.saturating_add(source_rectangle.width) {
                copied.push(source.pixel(x, y));
            }
        }
        }
        // Self-copies must read their original pixels before writing.
        let coverage = (gc.clip_rectangles.is_some() || gc.clip_mask.is_some())
            .then(|| self.gc_clip_coverage(&gc, destination_rectangle));
        self.ensure_surface_materialized(request.dst)?;
        let Some(surface) = self.surfaces.get_mut(&request.dst.0) else {
            unreachable!();
        };
        let destination_width = usize::from(surface.width);
        let Some(pixels) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        for (row, source_row) in copied
            .chunks_exact(usize::from(destination_rectangle.width))
            .enumerate()
        {
            let target_y = usize::from(destination_rectangle.y).saturating_add(row);
            if plain_copy {
                let start = target_y * destination_width + usize::from(destination_rectangle.x);
                let target = &mut pixels[start..start + source_row.len()];
                if drawable_mask == u32::MAX {
                    target.copy_from_slice(source_row);
                } else {
                    for (destination, source) in target.iter_mut().zip(source_row) {
                        *destination = *source & drawable_mask;
                    }
                }
                continue;
            }
            for (column, source_pixel) in source_row.iter().enumerate() {
                let coverage_index = row
                    .saturating_mul(usize::from(destination_rectangle.width))
                    .saturating_add(column);
                if coverage
                    .as_ref()
                    .is_some_and(|coverage| !coverage[coverage_index] )
                {
                    continue;
                }
                let target_x = usize::from(destination_rectangle.x).saturating_add(column);
                let index = target_y
                    .saturating_mul(destination_width)
                    .saturating_add(target_x);
                pixels[index] = apply_gc_function(
                    pixels[index],
                    *source_pixel,
                    gc.function,
                    gc.plane_mask,
                    drawable_mask,
                );
            }
        }
        // CopyArea applies GC clipping while writing pixels. Publish those
        // same rectangles instead of the unclipped request bounding box: a
        // toolkit can copy two distant dirty regions in one X11 request.
        if let Some(clip) = &gc.clip_rectangles {
            let left = i32::from(destination_rectangle.x);
            let top = i32::from(destination_rectangle.y);
            let right = left + i32::from(destination_rectangle.width);
            let bottom = top + i32::from(destination_rectangle.height);
            let mut regions = Vec::new();
            let mut fallback = false;
            for wire in clip.rectangles.chunks_exact(8) {
                let x = i32::from(read_i16_le(&wire[0..2]))
                    .saturating_add(i32::from(gc.clip_x_origin));
                let y = i32::from(read_i16_le(&wire[2..4]))
                    .saturating_add(i32::from(gc.clip_y_origin));
                let width = i32::from(read_u16(ClientByteOrder::LittleEndian, &wire[4..6]));
                let height = i32::from(read_u16(ClientByteOrder::LittleEndian, &wire[6..8]));
                let x0 = left.max(x);
                let y0 = top.max(y);
                let x1 = right.min(x.saturating_add(width));
                let y1 = bottom.min(y.saturating_add(height));
                if x1 <= x0 || y1 <= y0 {
                    continue;
                }
                if regions.len() == 64 {
                    fallback = true;
                    break;
                }
                regions.push(ClippedRectangle {
                    x: x0 as u16,
                    y: y0 as u16,
                    width: (x1 - x0) as u16,
                    height: (y1 - y0) as u16,
                });
            }
            if fallback {
                self.damage_drawable(request.dst, destination_rectangle);
        } else {
                for region in regions {
                    self.damage_drawable(request.dst, region);
                }
            }
        } else {
            self.damage_drawable(request.dst, destination_rectangle);
        }
        if gc.graphics_exposures {
            self.emit_to_client(handle, |output, sequence, order| {
                x11::encode_no_exposure_event(output, sequence, order, request.dst, 0, 62);
            });
        }
        Ok(Vec::new())
    }

    pub(super) fn copy_plane(&mut self, handle: ClientHandle, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let src = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let dst = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let gc_id = ResourceId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        let src_x = read_i16_le(body.get(12..14).ok_or_else(bad_length)?);
        let src_y = read_i16_le(body.get(14..16).ok_or_else(bad_length)?);
        let dst_x = read_i16_le(body.get(16..18).ok_or_else(bad_length)?);
        let dst_y = read_i16_le(body.get(18..20).ok_or_else(bad_length)?);
        let width = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(20..22).ok_or_else(bad_length)?,
        );
        let height = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(22..24).ok_or_else(bad_length)?,
        );
        let plane = read_u32_le(body.get(24..28).ok_or_else(bad_length)?);
        let gc = self.gc_for_drawable(gc_id, dst)?;
        let source = self
            .surfaces
            .get(&src.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, src.0))?;
        if plane.count_ones() != 1 || plane & depth_plane_mask(source.depth) == 0 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, plane));
        }
        let destination = self
            .surfaces
            .get(&dst.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, dst.0))?;
        let request = x11::CopyAreaRequest {
            src,
            dst,
            gc: gc_id,
            src_x,
            src_y,
            dst_x,
            dst_y,
            width,
            height,
        };
        let Some((source_rectangle, destination_rectangle)) =
            clipped_copy_rectangles(source, destination, &request)
        else {
            return Ok(Vec::new());
        };
        let mut copied = Vec::with_capacity(
            usize::from(source_rectangle.width)
                .saturating_mul(usize::from(source_rectangle.height)),
        );
        for y in source_rectangle.y..source_rectangle.y.saturating_add(source_rectangle.height) {
            for x in source_rectangle.x..source_rectangle.x.saturating_add(source_rectangle.width) {
                copied.push(if source.pixel(x, y) & plane != 0 {
                    gc.foreground
                } else {
                    gc.background
                });
            }
        }
        let coverage = self.gc_clip_coverage(&gc, destination_rectangle);
        self.ensure_surface_materialized(dst)?;
        let surface = self
            .surfaces
            .get_mut(&dst.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, dst.0))?;
        let stride = usize::from(surface.width);
        let Some(destination) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        for (index, source_pixel) in copied.into_iter().enumerate() {
            if !coverage[index] {
                continue;
            }
            let x = usize::from(destination_rectangle.x)
                .saturating_add(index % usize::from(destination_rectangle.width));
            let y = usize::from(destination_rectangle.y)
                .saturating_add(index / usize::from(destination_rectangle.width));
            let destination_index = y.saturating_mul(stride).saturating_add(x);
            destination[destination_index] = apply_gc_function(
                destination[destination_index],
                source_pixel,
                gc.function,
                gc.plane_mask,
                depth_plane_mask(gc.depth),
            );
        }
        self.damage_drawable(dst, destination_rectangle);
        if gc.graphics_exposures {
            self.emit_to_client(handle, |output, sequence, order| {
                x11::encode_no_exposure_event(output, sequence, order, dst, 0, 63);
            });
        }
        Ok(Vec::new())
    }
}
