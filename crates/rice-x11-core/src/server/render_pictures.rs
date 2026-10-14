//! Render pictures operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn render_create_picture(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 16 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let drawable = ResourceId(read_u32(order, &body[4..8]));
        let format = read_u32(order, &body[8..12]);
        let value_mask = read_u32(order, &body[12..16]);
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0));
        }
        let depth = self
            .surfaces
            .get(&drawable.0)
            .map(|surface| surface.depth)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        if render_format_depth(format) != Some(depth) {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, format));
        }
        let mut picture = Picture {
            owner: handle.0,
            source: PictureSource::Drawable(drawable),
            format,
            repeat: 0,
            component_alpha: false,
            clip_x_origin: 0,
            clip_y_origin: 0,
            clip_mask: None,
            clip_rectangles: None,
            transform: render_identity_transform(),
            filter: PictureFilter::Nearest,
        };
        self.apply_picture_attributes(order, &mut picture, value_mask, &body[16..])?;
        self.pictures.insert(id.0, picture);
        Ok(Vec::new())
    }

    pub(super) fn render_change_picture(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 8 {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let value_mask = read_u32(order, &body[4..8]);
        let mut picture = self
            .pictures
            .remove(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        let result = self.apply_picture_attributes(order, &mut picture, value_mask, &body[8..]);
        self.pictures.insert(id.0, picture);
        result.map(|()| Vec::new())
    }

    pub(super) fn apply_picture_attributes(
        &self,
        order: ClientByteOrder,
        picture: &mut Picture,
        value_mask: u32,
        values: &[u8],
    ) -> Result<(), ProtocolError> {
        if value_mask & !0x1fff != 0 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, value_mask));
        }
        let required = usize::try_from(value_mask.count_ones())
            .unwrap_or(usize::MAX)
            .saturating_mul(4);
        if values.len() != required {
            return Err(bad_length());
        }
        let mut offset = 0;
        for bit in 0..=12 {
            if value_mask & (1 << bit) == 0 {
                continue;
            }
            let value = read_u32(order, &values[offset..offset + 4]);
            offset += 4;
            match bit {
                0 => {
                    let repeat = u8::try_from(value)
                        .map_err(|_| ProtocolError::new(x11::error::BAD_VALUE, value))?;
                    if repeat > 3 {
                        return Err(ProtocolError::new(x11::error::BAD_VALUE, value));
                    }
                    picture.repeat = repeat;
                }
                4 => picture.clip_x_origin = low_i16(value),
                5 => picture.clip_y_origin = low_i16(value),
                6 => {
                    let mask = ResourceId(value);
                    if mask.0 == 0 {
                        picture.clip_mask = None;
                    } else {
                        let depth = self
                            .surfaces
                            .get(&mask.0)
                            .map(|surface| surface.depth)
                            .ok_or_else(|| ProtocolError::new(x11::error::BAD_PIXMAP, mask.0))?;
                        if depth != 1 {
                            return Err(ProtocolError::new(x11::error::BAD_MATCH, mask.0));
                        }
                        picture.clip_mask = Some(mask);
                    }
                    picture.clip_rectangles = None;
                }
                12 => picture.component_alpha = value != 0,
                // Alpha maps, origins, exposure, subwindow mode, edge/mode and
                // dither are accepted. They do not change the common GTK path.
                1..=3 | 7..=11 => {}
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    pub(super) fn render_set_picture_clip_rectangles(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() < 8 || !(body.len() - 8).is_multiple_of(8) {
            return Err(bad_length());
        }
        let id = ResourceId(read_u32(order, &body[0..4]));
        let x_origin = read_i16(order, &body[4..6]);
        let y_origin = read_i16(order, &body[6..8]);
        let rectangles = body[8..]
            .chunks_exact(8)
            .map(|rectangle| {
                (
                    read_i16(order, &rectangle[0..2]),
                    read_i16(order, &rectangle[2..4]),
                    read_u16(order, &rectangle[4..6]),
                    read_u16(order, &rectangle[6..8]),
                )
            })
            .collect();
        let picture = self
            .pictures
            .get_mut(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, id.0))?;
        picture.clip_x_origin = x_origin;
        picture.clip_y_origin = y_origin;
        picture.clip_mask = None;
        picture.clip_rectangles = Some(rectangles);
        Ok(Vec::new())
    }

    pub(super) fn render_free_picture(
        &mut self,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = ResourceId(read_u32(order, body.get(0..4).ok_or_else(bad_length)?));
        if self.pictures.remove(&id.0).is_none() {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, id.0));
        }
        self.collect_orphan_surfaces();
        Ok(Vec::new())
    }

    pub(super) fn render_create_solid_fill(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 12 {
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
        let color = render_color(order, &body[4..12]);
        self.pictures.insert(
            id.0,
            Picture {
                owner: handle.0,
                source: PictureSource::Solid(color),
                format: x11::RENDER_FMT_ARGB32,
                repeat: 1,
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
}
