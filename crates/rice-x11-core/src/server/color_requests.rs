//! Color requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn create_colormap(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let window = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let visual = ResourceId(read_u32_le(body.get(8..12).ok_or_else(bad_length)?));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        if !matches!(visual, ROOT_VISUAL | ARGB_VISUAL) {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, visual.0));
        }
        self.colormaps.insert(
            id.0,
            Colormap {
                owner: Some(handle.0),
                visual,
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn free_colormap(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        if id == ROOT_COLORMAP {
            return Err(ProtocolError::new(x11::error::BAD_COLORMAP, id.0));
        }
        if self.colormaps.remove(&id.0).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_COLORMAP, id.0));
        }
        Ok(Vec::new())
    }

    pub(super) fn copy_colormap(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let destination = ResourceId(read_u32_le(body.get(0..4).ok_or_else(bad_length)?));
        let source = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(destination) || self.resource_exists(destination) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, destination.0));
        }
        let visual = self
            .colormaps
            .get(&source.0)
            .map(|colormap| colormap.visual)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_COLORMAP, source.0))?;
        self.colormaps.insert(
            destination.0,
            Colormap {
                owner: Some(handle.0),
                visual,
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn validate_colormap_request(&self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        if !self.colormaps.contains_key(&id.0) {
            return Err(ProtocolError::new(x11::error::BAD_COLORMAP, id.0));
        }
        Ok(Vec::new())
    }

    pub(super) fn list_installed_colormaps(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0));
        }
        let mut output = Vec::with_capacity(36);
        output.push(1);
        output.push(0);
        let mut sequence_bytes = [0; 2];
        write_u16_at(order, &mut sequence_bytes, sequence.0);
        output.extend_from_slice(&sequence_bytes);
        write_u32_vec(order, &mut output, 1);
        let mut count_bytes = [0; 2];
        write_u16_at(order, &mut count_bytes, 1);
        output.extend_from_slice(&count_bytes);
        output.resize(32, 0);
        write_u32_vec(order, &mut output, ROOT_COLORMAP.0);
        Ok(output)
    }

    pub(super) fn alloc_color(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        self.validate_colormap_request(body)?;
        let color = x11::alloc_color_request(body).ok_or_else(bad_length)?;
        let mut output = Vec::with_capacity(32);
        x11::write_alloc_color_reply(&mut output, order, sequence, color)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn alloc_named_color(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        self.validate_colormap_request(body)?;
        let name = x11::alloc_named_color_name(body);
        let color = x11::lookup_color_name(&name)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_NAME, 0))?;
        let mut output = Vec::with_capacity(32);
        x11::write_alloc_named_color_reply(&mut output, order, sequence, color)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn query_colors(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        self.validate_colormap_request(body)?;
        let pixels = x11::query_colors_pixels(body);
        let mut output = Vec::with_capacity(32 + pixels.len().saturating_mul(8));
        x11::write_query_colors_reply(&mut output, order, sequence, &pixels)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn lookup_color(
        &self,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        self.validate_colormap_request(body)?;
        let name = x11::alloc_named_color_name(body);
        let color = x11::lookup_color_name(&name)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_NAME, 0))?;
        let mut output = Vec::with_capacity(32);
        x11::write_lookup_color_reply(&mut output, order, sequence, color)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }
}
