//! Cursor requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn create_cursor(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let source = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let mask = ResourceId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        let hot_x = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(24..26).ok_or_else(bad_length)?,
        );
        let hot_y = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(26..28).ok_or_else(bad_length)?,
        );
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        let source_surface = self
            .surfaces
            .get(&source.0)
            .filter(|surface| surface.depth == 1)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_PIXMAP, source.0))?;
        // Xlib/GTK use a 1x1 source with a (1,1) hotspot for an invisible
        // cursor. Xorg accepts that edge coordinate, so keep the native
        // backend interoperable with unchanged clients while rejecting
        // coordinates beyond the source bounds.
        if hot_x > source_surface.width || hot_y > source_surface.height {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, u32::from(hot_x)));
        }
        if mask.0 != 0 {
            let mask_surface = self
                .surfaces
                .get(&mask.0)
                .filter(|surface| surface.depth == 1)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_PIXMAP, mask.0))?;
            if mask_surface.width != source_surface.width
                || mask_surface.height != source_surface.height
            {
                return Err(ProtocolError::new(x11::error::BAD_MATCH, mask.0));
            }
        }
        self.cursors.insert(id.0, RiceCursor::new(handle.0, 0));
        Ok(Vec::new())
    }

    pub(super) fn create_glyph_cursor(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let source_font = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let mask_font = ResourceId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        if !self.fonts.contains_key(&source_font.0) {
            return Err(ProtocolError::new(x11::error::BAD_FONT, source_font.0));
        }
        if mask_font.0 != 0 && !self.fonts.contains_key(&mask_font.0) {
            return Err(ProtocolError::new(x11::error::BAD_FONT, mask_font.0));
        }
        self.cursors.insert(id.0, RiceCursor::new(handle.0, cursor::glyph_shape(read_u16(ClientByteOrder::LittleEndian, &body[12..14]))));
        Ok(Vec::new())
    }

    pub(super) fn free_cursor(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        if self.cursors.remove(&id.0).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_CURSOR, id.0));
        }
        Ok(Vec::new())
    }

    pub(super) fn recolor_cursor(&self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        if !self.cursors.contains_key(&id.0) {
            return Err(ProtocolError::new(x11::error::BAD_CURSOR, id.0));
        }
        Ok(Vec::new())
    }

    pub(super) fn query_best_size(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        class: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if class > 2 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(class)));
        }
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        if !self.surfaces.contains_key(&drawable.0) {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
        }
        let width = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(4..6).ok_or_else(bad_length)?,
        );
        let height = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(6..8).ok_or_else(bad_length)?,
        );
        let (width, height) = if class == 0 {
            (width.min(64), height.min(64))
        } else {
            (width, height)
        };
        let mut output = Vec::with_capacity(32);
        x11::write_query_best_size_reply(&mut output, order, sequence, width, height)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }
}
