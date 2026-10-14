//! Render transforms operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn render_set_picture_transform(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 40 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let picture = self
            .pictures
            .get_mut(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        for (index, value) in picture.transform.iter_mut().enumerate() {
            let offset = 4 + index * 4;
            *value = read_i32(order, &body[offset..offset + 4]);
        }
        Ok(Vec::new())
    }

    pub(super) fn render_set_picture_filter(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 8 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let length = usize::from(read_u16(order, &body[4..6]));
        let name_end = 8usize.saturating_add(x11::pad4(length));
        if body.len() < name_end || !(body.len() - name_end).is_multiple_of(4) {
            return Err(bad_length());
        }
        let name = std::str::from_utf8(&body[8..8 + length])
            .map_err(|_| ProtocolError::new(x11::error::BAD_VALUE, 0))?;
        let filter = match name {
            "nearest" | "fast" => PictureFilter::Nearest,
            "bilinear" | "good" | "best" | "convolution" => PictureFilter::Bilinear,
            _ => return Err(ProtocolError::new(x11::error::BAD_VALUE, 0)),
        };
        let picture = self
            .pictures
            .get_mut(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        picture.filter = filter;
        Ok(Vec::new())
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn render_create_gradient(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let prefix = match minor {
            34 => 24,
            35 => 32,
            36 => 20,
            _ => unreachable!(),
        };
        if body.len() < prefix {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        let count_offset = prefix - 4;
        let count = usize::try_from(read_u32(order, &body[count_offset..count_offset + 4]))
            .map_err(|_| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        if count < 2 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::try_from(count).unwrap_or(u32::MAX),
            ));
        }
        let stop_bytes = count
            .checked_mul(12)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        if body.len() != prefix.saturating_add(stop_bytes) {
            return Err(bad_length());
        }
        let positions_end = prefix.saturating_add(count.saturating_mul(4));
        let mut stops = Vec::with_capacity(count);
        for index in 0..count {
            let position_offset = prefix + index * 4;
            let color_offset = positions_end + index * 8;
            stops.push(GradientStop {
                position: read_i32(order, &body[position_offset..position_offset + 4]),
                color: render_color(order, &body[color_offset..color_offset + 8]),
            });
        }
        if stops
            .windows(2)
            .any(|pair| pair[0].position > pair[1].position)
        {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        let source = match minor {
            34 => PictureSource::LinearGradient {
                first: (read_i32(order, &body[4..8]), read_i32(order, &body[8..12])),
                second: (
                    read_i32(order, &body[12..16]),
                    read_i32(order, &body[16..20]),
                ),
                stops,
            },
            35 => PictureSource::RadialGradient {
                inner: (read_i32(order, &body[4..8]), read_i32(order, &body[8..12])),
                outer: (
                    read_i32(order, &body[12..16]),
                    read_i32(order, &body[16..20]),
                ),
                inner_radius: read_i32(order, &body[20..24]),
                outer_radius: read_i32(order, &body[24..28]),
                stops,
            },
            36 => PictureSource::ConicalGradient {
                center: (read_i32(order, &body[4..8]), read_i32(order, &body[8..12])),
                angle: read_i32(order, &body[12..16]),
                stops,
            },
            _ => unreachable!(),
        };
        self.pictures.insert(
            id.0,
            Picture {
                owner: handle.0,
                source,
                format: x11::RENDER_FMT_ARGB32,
                repeat: 0,
                component_alpha: false,
                clip_x_origin: 0,
                clip_y_origin: 0,
                clip_mask: None,
                clip_rectangles: None,
                transform: render_identity_transform(),
                filter: PictureFilter::Nearest,
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn render_create_cursor(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 12 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let source = ResourceId(read_u32(order, &body[4..8]));
        let x = read_u16(order, &body[8..10]);
        let y = read_u16(order, &body[10..12]);
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        let picture = self
            .pictures
            .get(&source.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, source.0))?;
        let PictureSource::Drawable(drawable) = picture.source else {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, source.0));
        };
        let surface = self
            .surfaces
            .get(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        if x >= surface.width || y >= surface.height {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, source.0));
        }
        self.cursors.insert(id.0, RiceCursor::new(handle.0, 0));
        Ok(Vec::new())
    }

    /// RENDER CreateAnimCursor. RiceWM lets the browser draw the pointer and
    /// currently exposes only cursor resource identity, so retaining the new
    /// cursor after validating every animation frame is sufficient. GTK uses
    /// this request while constructing its themed wait cursor; rejecting it
    /// with BadRequest makes otherwise healthy applications abort.
    pub(super) fn render_create_animated_cursor(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 12 || (body.len() - 4) % 8 != 0 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        for element in body[4..].chunks_exact(8) {
            let cursor = read_u32(order, &element[0..4]);
            if !self.cursors.contains_key(&cursor) {
                return Err(ProtocolError::new(x11::error::BAD_CURSOR, cursor));
            }
            // The following CARD32 is the frame delay in milliseconds. Zero
            // is legal and needs no browser-side state while cursor animation
            // is intentionally represented by its validated resource.
            let _delay = read_u32(order, &element[4..8]);
        }
        self.cursors.insert(id.0, RiceCursor::new(handle.0, 0));
        Ok(Vec::new())
    }
}
