//! Graphics contexts operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn create_gc(&mut self, handle: ClientHandle, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::create_gc_request(body).ok_or_else(bad_length)?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(request.gc) || self.resource_exists(request.gc) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, request.gc.0));
        }
        let depth = self
            .surfaces
            .get(&request.drawable.0)
            .map(|surface| surface.depth)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.drawable.0))?;
        if depth == 0 {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                request.drawable.0,
            ));
        }
        if let Some(font) = request.font
            && !self.fonts.contains_key(&font.0)
        {
            return Err(ProtocolError::new(x11::error::BAD_FONT, font.0));
        }
        self.validate_gc_pixmaps(
            depth,
            request.tile,
            request.stipple,
            request.clip_mask.flatten(),
        )?;
        let mut gc = GraphicsContext::new(handle.0, depth);
        if let Some(value) = request.function {
            gc.function = value;
        }
        if let Some(value) = request.plane_mask {
            gc.plane_mask = value;
        }
        if let Some(value) = request.foreground {
            gc.foreground = value;
        }
        if let Some(value) = request.background {
            gc.background = value;
        }
        if let Some(value) = request.line_width {
            gc.line_width = value;
        }
        if let Some(value) = request.line_style {
            gc.line_style = value;
        }
        if let Some(value) = request.cap_style {
            gc.cap_style = value;
        }
        if let Some(value) = request.join_style {
            gc.join_style = value;
        }
        if let Some(value) = request.fill_style {
            gc.fill_style = value;
        }
        if let Some(value) = request.fill_rule {
            gc.fill_rule = value;
        }
        if let Some(value) = request.tile {
            gc.tile = Some(value);
        }
        if let Some(value) = request.stipple {
            gc.stipple = Some(value);
        }
        if let Some(value) = request.tile_x_origin {
            gc.tile_x_origin = value;
        }
        if let Some(value) = request.tile_y_origin {
            gc.tile_y_origin = value;
        }
        if let Some(value) = request.font {
            gc.font = Some(value);
        }
        if let Some(value) = request.subwindow_mode {
            gc.subwindow_mode = value;
        }
        if let Some(value) = request.graphics_exposures {
            gc.graphics_exposures = value;
        }
        if let Some(value) = request.clip_x_origin {
            gc.clip_x_origin = value;
        }
        if let Some(value) = request.clip_y_origin {
            gc.clip_y_origin = value;
        }
        if let Some(value) = request.clip_mask {
            gc.clip_mask = value;
            gc.clip_rectangles = None;
        }
        if let Some(value) = request.dash_offset {
            gc.dash_offset = value;
        }
        if let Some(value) = request.dashes {
            gc.dashes = vec![value, value];
            gc.dash_offset = 0;
        }
        if let Some(value) = request.arc_mode {
            gc.arc_mode = value;
        }
        self.graphics_contexts.insert(request.gc.0, gc);
        Ok(Vec::new())
    }

    pub(super) fn change_gc(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::change_gc_request(body).ok_or_else(bad_length)?;
        if !self.graphics_contexts.contains_key(&request.gc.0) {
            return Err(ProtocolError::new(x11::error::BAD_GC, request.gc.0));
        }
        let depth = self
            .graphics_contexts
            .get(&request.gc.0)
            .map(|gc| gc.depth)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, request.gc.0))?;
        if let Some(font) = request.font
            && !self.fonts.contains_key(&font.0)
        {
            return Err(ProtocolError::new(x11::error::BAD_FONT, font.0));
        }
        self.validate_gc_pixmaps(
            depth,
            request.tile,
            request.stipple,
            request.clip_mask.flatten(),
        )?;
        let Some(gc) = self.graphics_contexts.get_mut(&request.gc.0) else {
            unreachable!();
        };
        if let Some(value) = request.function {
            gc.function = value;
        }
        if let Some(value) = request.plane_mask {
            gc.plane_mask = value;
        }
        if let Some(value) = request.foreground {
            gc.foreground = value;
        }
        if let Some(value) = request.background {
            gc.background = value;
        }
        if let Some(value) = request.line_width {
            gc.line_width = value;
        }
        if let Some(value) = request.line_style {
            gc.line_style = value;
        }
        if let Some(value) = request.cap_style {
            gc.cap_style = value;
        }
        if let Some(value) = request.join_style {
            gc.join_style = value;
        }
        if let Some(value) = request.fill_style {
            gc.fill_style = value;
        }
        if let Some(value) = request.fill_rule {
            gc.fill_rule = value;
        }
        if let Some(value) = request.tile {
            gc.tile = Some(value);
        }
        if let Some(value) = request.stipple {
            gc.stipple = Some(value);
        }
        if let Some(value) = request.tile_x_origin {
            gc.tile_x_origin = value;
        }
        if let Some(value) = request.tile_y_origin {
            gc.tile_y_origin = value;
        }
        if let Some(value) = request.font {
            gc.font = Some(value);
        }
        if let Some(value) = request.subwindow_mode {
            gc.subwindow_mode = value;
        }
        if let Some(value) = request.graphics_exposures {
            gc.graphics_exposures = value;
        }
        if let Some(value) = request.clip_x_origin {
            gc.clip_x_origin = value;
        }
        if let Some(value) = request.clip_y_origin {
            gc.clip_y_origin = value;
        }
        if let Some(value) = request.clip_mask {
            gc.clip_mask = value;
            gc.clip_rectangles = None;
        }
        if let Some(value) = request.dash_offset {
            gc.dash_offset = value;
        }
        if let Some(value) = request.dashes {
            gc.dashes = vec![value, value];
            gc.dash_offset = 0;
        }
        if let Some(value) = request.arc_mode {
            gc.arc_mode = value;
        }
        Ok(Vec::new())
    }

    pub(super) fn copy_gc(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let src_id = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let dst_id = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let mask = read_u32_le(body.get(8..12).ok_or_else(bad_length)?);
        let source = self
            .graphics_contexts
            .get(&src_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, src_id.0))?;
        let destination = self
            .graphics_contexts
            .get_mut(&dst_id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, dst_id.0))?;
        if source.depth != destination.depth {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, 0));
        }
        copy_gc_fields(&source, destination, mask);
        Ok(Vec::new())
    }

    pub(super) fn set_dashes(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let gc_id = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let offset = read_u16(
            ClientByteOrder::LittleEndian,
            body.get(4..6).ok_or_else(bad_length)?,
        );
        let count = usize::from(read_u16(
            ClientByteOrder::LittleEndian,
            body.get(6..8).ok_or_else(bad_length)?,
        ));
        let dashes = body
            .get(8..8usize.saturating_add(count))
            .ok_or_else(bad_length)?;
        let gc = self
            .graphics_contexts
            .get_mut(&gc_id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, gc_id.0))?;
        if dashes.is_empty() || dashes.contains(&0) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        gc.dash_offset = offset;
        gc.dashes.clear();
        gc.dashes.extend_from_slice(dashes);
        if !gc.dashes.len().is_multiple_of(2) {
            gc.dashes.extend_from_slice(dashes);
        }
        Ok(Vec::new())
    }

    pub(super) fn set_clip_rectangles(&mut self, ordering: u8, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::set_clip_rectangles_request(ordering, body).ok_or_else(bad_length)?;
        let gc = self
            .graphics_contexts
            .get_mut(&request.gc.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, request.gc.0))?;
        gc.clip_x_origin = request.clip.x_origin;
        gc.clip_y_origin = request.clip.y_origin;
        gc.clip_mask = None;
        gc.clip_rectangles = Some(request.clip);
        Ok(Vec::new())
    }

    pub(super) fn free_gc(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let gc = x11::free_resource_id(body).ok_or_else(bad_length)?;
        if self.graphics_contexts.remove(&gc.0).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_GC, gc.0));
        }
        Ok(Vec::new())
    }
}
