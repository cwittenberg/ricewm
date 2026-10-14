//! Render glyphs operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn render_create_glyphset(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let format = read_u32(order, &body[4..8]);
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        if !matches!(
            format,
            x11::RENDER_FMT_A1 | x11::RENDER_FMT_A8 | x11::RENDER_FMT_ARGB32
        ) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, format));
        }
        self.glyphsets.insert(
            id.0,
            GlyphSet {
                owner: handle.0,
                format,
                storage: id.0,
            },
        );
        self.glyph_storage.insert(id.0, BTreeMap::new());
        Ok(Vec::new())
    }

    pub(super) fn render_reference_glyphset(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let existing = ResourceId(read_u32(order, &body[4..8]));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        let glyphset = self
            .glyphsets
            .get(&existing.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, existing.0))?;
        self.glyphsets.insert(
            id.0,
            GlyphSet {
                owner: handle.0,
                format: glyphset.format,
                storage: glyphset.storage,
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn render_free_glyphset(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = ResourceId(read_u32(order, body.get(0..4).ok_or_else(bad_length)?));
        if !self.release_glyphset_resource(id) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, id.0));
        }
        Ok(Vec::new())
    }

    pub(super) fn release_glyphset_resource(&mut self, id: ResourceId) -> bool {
        let Some(glyphset) = self.glyphsets.remove(&id.0) else {
            return false;
        };
        if !self
            .glyphsets
            .values()
            .any(|candidate| candidate.storage == glyphset.storage)
        {
            self.glyph_storage.remove(&glyphset.storage);
        }
        true
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_add_glyphs(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 8 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let count = usize::try_from(read_u32(order, &body[4..8]))
            .map_err(|_| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        let ids_end = 8usize
            .checked_add(
                count
                    .checked_mul(4)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?,
            )
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        let infos_end = ids_end
            .checked_add(
                count
                    .checked_mul(12)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?,
            )
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        if body.len() < infos_end {
            return Err(bad_length());
        }
        let (format, storage) = self
            .glyphsets
            .get(&id.0)
            .map(|glyphset| (glyphset.format, glyphset.storage))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        let mut data_offset = infos_end;
        let mut additions = Vec::with_capacity(count);
        for index in 0..count {
            let id_offset = 8 + index * 4;
            let info_offset = ids_end + index * 12;
            let glyph_id = read_u32(order, &body[id_offset..id_offset + 4]);
            let width = read_u16(order, &body[info_offset..info_offset + 2]);
            let height = read_u16(order, &body[info_offset + 2..info_offset + 4]);
            let x = read_i16(order, &body[info_offset + 4..info_offset + 6]);
            let y = read_i16(order, &body[info_offset + 6..info_offset + 8]);
            let x_off = read_i16(order, &body[info_offset + 8..info_offset + 10]);
            let y_off = read_i16(order, &body[info_offset + 10..info_offset + 12]);
            let row_bytes = match format {
                x11::RENDER_FMT_A1 => usize::from(width).div_ceil(32).saturating_mul(4),
                x11::RENDER_FMT_A8 => x11::pad4(usize::from(width)),
                x11::RENDER_FMT_ARGB32 => usize::from(width).saturating_mul(4),
                _ => unreachable!(),
            };
            let wire_bytes = row_bytes
                .checked_mul(usize::from(height))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let end = data_offset
                .checked_add(wire_bytes)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
            let wire = body.get(data_offset..end).ok_or_else(bad_length)?;
            data_offset = end;
            let mut alpha = vec![
                0;
                usize::from(width)
                    .checked_mul(usize::from(height))
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?
            ];
            for row in 0..usize::from(height) {
                for column in 0..usize::from(width) {
                    alpha[row * usize::from(width) + column] = match format {
                        x11::RENDER_FMT_A1 => {
                            let byte = wire[row * row_bytes + column / 8];
                            let bit = match order {
                                ClientByteOrder::LittleEndian => column % 8,
                                ClientByteOrder::BigEndian => 7 - (column % 8),
                            };
                            if byte & (1 << bit) == 0 { 0 } else { 255 }
                        }
                        x11::RENDER_FMT_A8 => wire[row * row_bytes + column],
                        x11::RENDER_FMT_ARGB32 => {
                            let pixel = read_u32(
                                order,
                                &wire[row * row_bytes + column * 4
                                    ..row * row_bytes + column * 4 + 4],
                            );
                            ((pixel >> 24) & 0xff).to_le_bytes()[0]
                        }
                        _ => unreachable!(),
                    };
                }
            }
            additions.push((
                glyph_id,
                Glyph {
                    width,
                    height,
                    x,
                    y,
                    x_off,
                    y_off,
                    alpha,
                },
            ));
        }
        if body[data_offset..].iter().any(|byte| *byte != 0) || body.len() - data_offset > 3 {
            return Err(bad_length());
        }
        self.glyph_storage
            .get_mut(&storage)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?
            .extend(additions);
        Ok(Vec::new())
    }

    pub(super) fn render_free_glyphs(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 4 || !(body.len() - 4).is_multiple_of(4) {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let storage = self
            .glyphsets
            .get(&id.0)
            .map(|glyphset| glyphset.storage)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        let glyphs = self
            .glyph_storage
            .get_mut(&storage)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        for glyph in body[4..].chunks_exact(4) {
            glyphs.remove(&read_u32(order, glyph));
        }
        Ok(Vec::new())
    }
}
