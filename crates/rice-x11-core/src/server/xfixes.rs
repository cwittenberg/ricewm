//! Xfixes operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn xfixes_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = self.xfixes_request_inner(handle, order, sequence, minor, body);
        result.map_err(|error| extension_error(error, minor))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn xfixes_request_inner(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        match minor {
            x11xfixes::QUERY_VERSION => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let requested_major = read_u32(order, body);
                let requested_minor = read_u32(order, &body[4..8]);
                let major = x11xfixes::MAJOR_VERSION.min(requested_major);
                let minor = if major < x11xfixes::MAJOR_VERSION {
                    requested_minor
                } else {
                    x11xfixes::MINOR_VERSION
                };
                Ok(x11xfixes::encode_query_version_reply(
                    order, sequence, major, minor,
                ))
            }
            x11xfixes::CHANGE_SAVE_SET => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, &body[4..8]));
                self.require_window(window)?;
                Ok(Vec::new())
            }
            x11xfixes::SELECT_SELECTION_INPUT => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                let selection = read_u32(order, &body[4..8]);
                let mask = read_u32(order, &body[8..12]);
                if mask & !x11xfixes::SELECTION_ALL_EVENTS_MASK != 0 {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, mask));
                }
                let key = (handle.0, window.0, selection);
                if mask == 0 {
                    self.xfixes_selection_masks.remove(&key);
                } else {
                    self.xfixes_selection_masks.insert(key, mask);
                }
                Ok(Vec::new())
            }
            x11xfixes::SELECT_CURSOR_INPUT => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                let mask = read_u32(order, &body[4..8]);
                if mask & !1 != 0 {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, mask));
                }
                let key = (handle.0, window.0);
                if mask == 0 {
                    self.xfixes_cursor_masks.remove(&key);
                } else {
                    self.xfixes_cursor_masks.insert(key, mask);
                }
                Ok(Vec::new())
            }
            x11xfixes::GET_CURSOR_IMAGE => {
                if !body.is_empty() {
                    return Err(bad_length());
                }
                Ok(x11xfixes::encode_get_cursor_image_empty_reply(
                    order, sequence,
                ))
            }
            x11xfixes::CREATE_REGION => {
                if body.len() < 4 || !(body.len() - 4).is_multiple_of(8) {
                    return Err(bad_length());
                }
                let id = ResourceId(read_u32(order, body));
                self.validate_new_extension_resource(handle, id)?;
                self.xfixes_regions.insert(
                    id.0,
                    XFixesRegion {
                        owner: handle.0,
                        rectangles: normalize_region(decode_region_rectangles(order, &body[4..])),
                    },
                );
                Ok(Vec::new())
            }
            x11xfixes::CREATE_REGION_FROM_BITMAP => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let id = ResourceId(read_u32(order, body));
                let bitmap = ResourceId(read_u32(order, &body[4..8]));
                self.validate_new_extension_resource(handle, id)?;
                let surface = self
                    .surfaces
                    .get(&bitmap.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_PIXMAP, bitmap.0))?;
                if surface.depth != 1 {
                    return Err(ProtocolError::new(x11::error::BAD_MATCH, bitmap.0));
                }
                self.xfixes_regions.insert(
                    id.0,
                    XFixesRegion {
                        owner: handle.0,
                        rectangles: bitmap_region(surface),
                    },
                );
                Ok(Vec::new())
            }
            x11xfixes::CREATE_REGION_FROM_WINDOW => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let id = ResourceId(read_u32(order, body));
                let window = ResourceId(read_u32(order, &body[4..8]));
                let kind = body[8];
                self.validate_new_extension_resource(handle, id)?;
                validate_shape_parameters(x11shape::OP_SET, kind, None)?;
                self.require_window(window)?;
                self.xfixes_regions.insert(
                    id.0,
                    XFixesRegion {
                        owner: handle.0,
                        rectangles: self.shape_region(window, kind),
                    },
                );
                Ok(Vec::new())
            }
            x11xfixes::CREATE_REGION_FROM_GC => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let id = ResourceId(read_u32(order, body));
                let gc = ResourceId(read_u32(order, &body[4..8]));
                self.validate_new_extension_resource(handle, id)?;
                let context = self
                    .graphics_contexts
                    .get(&gc.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, gc.0))?;
                let rectangles = context
                    .clip_rectangles
                    .as_ref()
                    .map_or_else(Vec::new, |clip| {
                        decode_region_rectangles(ClientByteOrder::LittleEndian, &clip.rectangles)
                            .into_iter()
                            .map(|rectangle| {
                                offset_region_rectangle(
                                    rectangle,
                                    context.clip_x_origin,
                                    context.clip_y_origin,
                                )
                            })
                            .collect()
                    });
                self.xfixes_regions.insert(
                    id.0,
                    XFixesRegion {
                        owner: handle.0,
                        rectangles,
                    },
                );
                Ok(Vec::new())
            }
            x11xfixes::CREATE_REGION_FROM_PICTURE => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let id = ResourceId(read_u32(order, body));
                let picture = ResourceId(read_u32(order, &body[4..8]));
                self.validate_new_extension_resource(handle, id)?;
                let picture_state = self
                    .pictures
                    .get(&picture.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, picture.0))?;
                let rectangles =
                    picture_state
                        .clip_rectangles
                        .clone()
                        .map_or_else(Vec::new, |rectangles| {
                            rectangles
                                .into_iter()
                                .map(|(x, y, width, height)| x11xfixes::RegionRect {
                                    x: x.saturating_add(picture_state.clip_x_origin),
                                    y: y.saturating_add(picture_state.clip_y_origin),
                                    width,
                                    height,
                                })
                                .collect()
                        });
                self.xfixes_regions.insert(
                    id.0,
                    XFixesRegion {
                        owner: handle.0,
                        rectangles,
                    },
                );
                Ok(Vec::new())
            }
            x11xfixes::DESTROY_REGION => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let region = read_u32(order, body);
                if self.xfixes_regions.remove(&region).is_none() {
                    return Err(ProtocolError::new(XFIXES_FIRST_ERROR, region));
                }
                Ok(Vec::new())
            }
            x11xfixes::SET_REGION => {
                if body.len() < 4 || !(body.len() - 4).is_multiple_of(8) {
                    return Err(bad_length());
                }
                let region = read_u32(order, body);
                let rectangles = normalize_region(decode_region_rectangles(order, &body[4..]));
                self.region_mut(region)?.rectangles = rectangles;
                Ok(Vec::new())
            }
            x11xfixes::COPY_REGION => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let source = read_u32(order, body);
                let destination = read_u32(order, &body[4..8]);
                let rectangles = self.region(source)?.rectangles.clone();
                self.region_mut(destination)?.rectangles = rectangles;
                Ok(Vec::new())
            }
            x11xfixes::UNION_REGION | x11xfixes::INTERSECT_REGION | x11xfixes::SUBTRACT_REGION => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let first = read_u32(order, body);
                let second = read_u32(order, &body[4..8]);
                let destination = read_u32(order, &body[8..12]);
                let first_rectangles = self.region(first)?.rectangles.clone();
                let second_rectangles = self.region(second)?.rectangles.clone();
                let operation = match minor {
                    x11xfixes::UNION_REGION => x11shape::OP_UNION,
                    x11xfixes::INTERSECT_REGION => x11shape::OP_INTERSECT,
                    _ => x11shape::OP_SUBTRACT,
                };
                self.region_mut(destination)?.rectangles =
                    apply_region_operation(operation, &first_rectangles, &second_rectangles);
                Ok(Vec::new())
            }
            x11xfixes::INVERT_REGION => {
                if body.len() != 16 {
                    return Err(bad_length());
                }
                let source = read_u32(order, body);
                let bounds = decode_region_rectangles(order, &body[4..12])
                    .into_iter()
                    .next()
                    .unwrap_or(x11xfixes::RegionRect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    });
                let destination = read_u32(order, &body[12..16]);
                let source_rectangles = self.region(source)?.rectangles.clone();
                self.region_mut(destination)?.rectangles =
                    subtract_region(&[bounds], &source_rectangles);
                Ok(Vec::new())
            }
            x11xfixes::TRANSLATE_REGION => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let region = read_u32(order, body);
                let x = read_i16(order, &body[4..6]);
                let y = read_i16(order, &body[6..8]);
                for rectangle in &mut self.region_mut(region)?.rectangles {
                    *rectangle = offset_region_rectangle(*rectangle, x, y);
                }
                Ok(Vec::new())
            }
            x11xfixes::REGION_EXTENTS => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let source = read_u32(order, body);
                let destination = read_u32(order, &body[4..8]);
                let extents = region_extents(&self.region(source)?.rectangles);
                self.region_mut(destination)?.rectangles = if extents.is_empty() {
                    Vec::new()
                } else {
                    vec![extents]
                };
                Ok(Vec::new())
            }
            x11xfixes::FETCH_REGION => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let region = self.region(read_u32(order, body))?;
                Ok(x11xfixes::encode_fetch_region_reply(
                    order,
                    sequence,
                    region_extents(&region.rectangles),
                    &region.rectangles,
                ))
            }
            x11xfixes::SET_GC_CLIP_REGION => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let gc = ResourceId(read_u32(order, body));
                let region_id = read_u32(order, &body[4..8]);
                let x_origin = read_i16(order, &body[8..10]);
                let y_origin = read_i16(order, &body[10..12]);
                let rectangles = if region_id == 0 {
                    None
                } else {
                    Some(self.region(region_id)?.rectangles.clone())
                };
                let context = self
                    .graphics_contexts
                    .get_mut(&gc.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, gc.0))?;
                context.clip_mask = None;
                context.clip_x_origin = x_origin;
                context.clip_y_origin = y_origin;
                context.clip_rectangles = rectangles.map(|rectangles| x11::ClipRectangles {
                    ordering: 0,
                    x_origin,
                    y_origin,
                    rectangles: encode_region_rectangles_le(&rectangles),
                });
                Ok(Vec::new())
            }
            x11xfixes::SET_WINDOW_SHAPE_REGION => {
                if body.len() != 16 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                let kind = body[4];
                let x_offset = read_i16(order, &body[8..10]);
                let y_offset = read_i16(order, &body[10..12]);
                let region = read_u32(order, &body[12..16]);
                self.require_window(window)?;
                validate_shape_parameters(x11shape::OP_SET, kind, None)?;
                if region == 0 {
                    self.window_shapes.remove(&(window.0, kind));
                } else {
                    let rectangles = self
                        .region(region)?
                        .rectangles
                        .iter()
                        .map(|rectangle| offset_region_rectangle(*rectangle, x_offset, y_offset))
                        .collect();
                    self.window_shapes.insert((window.0, kind), rectangles);
                }
                self.emit_shape_notify(window, kind);
                Ok(Vec::new())
            }
            x11xfixes::SET_PICTURE_CLIP_REGION => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let picture = ResourceId(read_u32(order, body));
                let region = read_u32(order, &body[4..8]);
                let x_origin = read_i16(order, &body[8..10]);
                let y_origin = read_i16(order, &body[10..12]);
                let rectangles = if region == 0 {
                    None
                } else {
                    Some(
                        self.region(region)?
                            .rectangles
                            .iter()
                            .map(|rectangle| {
                                (rectangle.x, rectangle.y, rectangle.width, rectangle.height)
                            })
                            .collect(),
                    )
                };
                let picture_state = self
                    .pictures
                    .get_mut(&picture.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, picture.0))?;
                picture_state.clip_mask = None;
                picture_state.clip_x_origin = x_origin;
                picture_state.clip_y_origin = y_origin;
                picture_state.clip_rectangles = rectangles;
                Ok(Vec::new())
            }
            x11xfixes::SET_CURSOR_NAME | x11xfixes::CHANGE_CURSOR_BY_NAME => {
                let (cursor, name) = decode_cursor_name_request(order, body)?;
                if !self.cursors.contains_key(&cursor) {
                    return Err(ProtocolError::new(x11::error::BAD_CURSOR, cursor));
                }
                self.cursors[&cursor].set_shape(cursor::named_shape(name));
                self.cursor_names.insert(cursor, name.to_vec());
                Ok(Vec::new())
            }
            x11xfixes::GET_CURSOR_NAME => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let cursor = read_u32(order, body);
                if !self.cursors.contains_key(&cursor) {
                    return Err(ProtocolError::new(x11::error::BAD_CURSOR, cursor));
                }
                let name = self.cursor_names.get(&cursor).cloned().unwrap_or_default();
                let atom = if name.is_empty() {
                    0
                } else {
                    self.atoms
                        .intern(std::str::from_utf8(&name).unwrap_or("cursor"), false)
                        .0
                };
                Ok(x11xfixes::encode_get_cursor_name_reply(
                    order, sequence, atom, &name,
                ))
            }
            x11xfixes::GET_CURSOR_IMAGE_AND_NAME => {
                if !body.is_empty() {
                    return Err(bad_length());
                }
                Ok(x11xfixes::encode_get_cursor_image_and_name_empty_reply(
                    order, sequence,
                ))
            }
            x11xfixes::HIDE_CURSOR | x11xfixes::SHOW_CURSOR => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                self.require_window(ResourceId(read_u32(order, body)))?;
                Ok(Vec::new())
            }
            x11xfixes::CREATE_POINTER_BARRIER => {
                if body.len() < 24 {
                    return Err(bad_length());
                }
                let barrier = ResourceId(read_u32(order, body));
                let window = ResourceId(read_u32(order, &body[4..8]));
                self.validate_new_extension_resource(handle, barrier)?;
                self.require_window(window)?;
                let devices = usize::from(read_u16(order, &body[22..24]));
                if body.len() != 24 + devices.saturating_mul(2) {
                    return Err(bad_length());
                }
                self.xfixes_barriers.insert(barrier.0, handle.0);
                Ok(Vec::new())
            }
            x11xfixes::DELETE_POINTER_BARRIER => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let barrier = read_u32(order, body);
                if self.xfixes_barriers.remove(&barrier).is_none() {
                    return Err(ProtocolError::new(x11::error::BAD_VALUE, barrier));
                }
                Ok(Vec::new())
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        }
    }

    pub(super) fn validate_new_extension_resource(
        &self,
        handle: ClientHandle,
        id: ResourceId,
    ) -> Result<(), ProtocolError> {
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(id) || self.resource_exists(id) {
            Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, id.0))
        } else {
            Ok(())
        }
    }

    pub(super) fn region(&self, id: u32) -> Result<&XFixesRegion, ProtocolError> {
        self.xfixes_regions
            .get(&id)
            .ok_or_else(|| ProtocolError::new(XFIXES_FIRST_ERROR, id))
    }

    pub(super) fn region_mut(&mut self, id: u32) -> Result<&mut XFixesRegion, ProtocolError> {
        self.xfixes_regions
            .get_mut(&id)
            .ok_or_else(|| ProtocolError::new(XFIXES_FIRST_ERROR, id))
    }
}
