//! Shape operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn shape_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = self.shape_request_inner(handle, order, sequence, minor, body);
        result.map_err(|error| extension_error(error, minor))
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn shape_request_inner(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        match minor {
            x11shape::QUERY_VERSION => {
                if !body.is_empty() {
                    return Err(bad_length());
                }
                Ok(x11shape::encode_query_version_reply(order, sequence))
            }
            x11shape::RECTANGLES => {
                if body.len() < 12 || !(body.len() - 12).is_multiple_of(8) {
                    return Err(bad_length());
                }
                let operation = body[0];
                let kind = body[1];
                let ordering = body[2];
                validate_shape_parameters(operation, kind, Some(ordering))?;
                let window = ResourceId(read_u32(order, &body[4..8]));
                self.require_window(window)?;
                let x_offset = read_i16(order, &body[8..10]);
                let y_offset = read_i16(order, &body[10..12]);
                let rectangles: Vec<x11xfixes::RegionRect> =
                    decode_region_rectangles(order, &body[12..])
                        .into_iter()
                        .map(|rectangle| offset_region_rectangle(rectangle, x_offset, y_offset))
                        .collect();
                self.apply_window_shape(window, kind, operation, &rectangles);
                self.emit_shape_notify(window, kind);
                Ok(Vec::new())
            }
            x11shape::MASK => {
                if body.len() != 16 {
                    return Err(bad_length());
                }
                let operation = body[0];
                let kind = body[1];
                validate_shape_parameters(operation, kind, None)?;
                let window = ResourceId(read_u32(order, &body[4..8]));
                self.require_window(window)?;
                let x_offset = read_i16(order, &body[8..10]);
                let y_offset = read_i16(order, &body[10..12]);
                let bitmap = ResourceId(read_u32(order, &body[12..16]));
                let rectangles = if bitmap.0 == 0 {
                    self.default_window_region(window)
                } else {
                    let surface = self
                        .surfaces
                        .get(&bitmap.0)
                        .ok_or_else(|| ProtocolError::new(x11::error::BAD_PIXMAP, bitmap.0))?;
                    if surface.depth != 1 {
                        return Err(ProtocolError::new(x11::error::BAD_MATCH, bitmap.0));
                    }
                    bitmap_region(surface)
                        .into_iter()
                        .map(|rectangle| offset_region_rectangle(rectangle, x_offset, y_offset))
                        .collect()
                };
                self.apply_window_shape(window, kind, operation, &rectangles);
                self.emit_shape_notify(window, kind);
                Ok(Vec::new())
            }
            x11shape::COMBINE => {
                if body.len() != 16 {
                    return Err(bad_length());
                }
                let operation = body[0];
                let destination_kind = body[1];
                let source_kind = body[2];
                validate_shape_parameters(operation, destination_kind, None)?;
                validate_shape_parameters(x11shape::OP_SET, source_kind, None)?;
                let destination = ResourceId(read_u32(order, &body[4..8]));
                let source = ResourceId(read_u32(order, &body[12..16]));
                self.require_window(destination)?;
                self.require_window(source)?;
                let x_offset = read_i16(order, &body[8..10]);
                let y_offset = read_i16(order, &body[10..12]);
                let rectangles: Vec<x11xfixes::RegionRect> = self
                    .shape_region(source, source_kind)
                    .into_iter()
                    .map(|rectangle| offset_region_rectangle(rectangle, x_offset, y_offset))
                    .collect();
                self.apply_window_shape(destination, destination_kind, operation, &rectangles);
                self.emit_shape_notify(destination, destination_kind);
                Ok(Vec::new())
            }
            x11shape::OFFSET => {
                if body.len() != 12 {
                    return Err(bad_length());
                }
                let kind = body[0];
                validate_shape_parameters(x11shape::OP_SET, kind, None)?;
                let window = ResourceId(read_u32(order, &body[4..8]));
                self.require_window(window)?;
                let x_offset = read_i16(order, &body[8..10]);
                let y_offset = read_i16(order, &body[10..12]);
                let key = (window.0, kind);
                if let Some(rectangles) = self.window_shapes.get_mut(&key) {
                    for rectangle in rectangles {
                        *rectangle = offset_region_rectangle(*rectangle, x_offset, y_offset);
                    }
                }
                self.emit_shape_notify(window, kind);
                Ok(Vec::new())
            }
            x11shape::QUERY_EXTENTS => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                let bounding_shaped = self.window_shapes.contains_key(&(window.0, 0));
                let clip_shaped = self.window_shapes.contains_key(&(window.0, 1));
                let bounding = region_extents(&self.shape_region(window, 0));
                let clip = region_extents(&self.shape_region(window, 1));
                Ok(x11shape::encode_query_extents_reply(
                    order,
                    sequence,
                    bounding_shaped,
                    clip_shaped,
                    bounding,
                    clip,
                ))
            }
            x11shape::SELECT_INPUT => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                match body[4] {
                    0 => {
                        self.shape_selects.remove(&(handle.0, window.0));
                    }
                    1 => {
                        self.shape_selects.insert((handle.0, window.0));
                    }
                    value => {
                        return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(value)));
                    }
                }
                Ok(Vec::new())
            }
            x11shape::INPUT_SELECTED => {
                if body.len() != 4 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                self.require_window(window)?;
                Ok(x11shape::encode_input_selected_reply(
                    order,
                    sequence,
                    self.shape_selects.contains(&(handle.0, window.0)),
                ))
            }
            x11shape::GET_RECTANGLES => {
                if body.len() != 8 {
                    return Err(bad_length());
                }
                let window = ResourceId(read_u32(order, body));
                let kind = body[4];
                validate_shape_parameters(x11shape::OP_SET, kind, None)?;
                self.require_window(window)?;
                Ok(x11shape::encode_get_rectangles_reply(
                    order,
                    sequence,
                    x11shape::ORDERING_YX_BANDED,
                    &self.shape_region(window, kind),
                ))
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        }
    }

    pub(super) fn require_window(&self, window: ResourceId) -> Result<(), ProtocolError> {
        if self.windows.contains_key(&window.0) {
            Ok(())
        } else {
            Err(ProtocolError::new(x11::error::BAD_WINDOW, window.0))
        }
    }

    pub(super) fn default_window_region(&self, window: ResourceId) -> Vec<x11xfixes::RegionRect> {
        self.windows.get(&window.0).map_or_else(Vec::new, |window| {
            vec![x11xfixes::RegionRect {
                x: 0,
                y: 0,
                width: window.width,
                height: window.height,
            }]
        })
    }

    pub(super) fn shape_region(&self, window: ResourceId, kind: u8) -> Vec<x11xfixes::RegionRect> {
        self.window_shapes
            .get(&(window.0, kind))
            .cloned()
            .unwrap_or_else(|| self.default_window_region(window))
    }

    pub(super) fn apply_window_shape(
        &mut self,
        window: ResourceId,
        kind: u8,
        operation: u8,
        source: &[x11xfixes::RegionRect],
    ) {
        let destination = self.shape_region(window, kind);
        let rectangles = apply_region_operation(operation, &destination, source);
        self.window_shapes
            .insert((window.0, kind), normalize_region(rectangles));
    }

    pub(super) fn emit_shape_notify(&mut self, window: ResourceId, kind: u8) {
        let targets: Vec<u32> = self
            .shape_selects
            .iter()
            .filter_map(|(owner, selected)| (*selected == window.0).then_some(*owner))
            .collect();
        if targets.is_empty() {
            return;
        }
        let shaped = self.window_shapes.contains_key(&(window.0, kind));
        let extents = region_extents(&self.shape_region(window, kind));
        let timestamp = self.next_timestamp();
        for target in targets {
            self.emit_to_client(ClientHandle(target), |output, sequence, order| {
                x11shape::encode_shape_notify_event(
                    output,
                    sequence,
                    order,
                    SHAPE_FIRST_EVENT,
                    kind,
                    window.0,
                    extents,
                    shaped,
                    timestamp,
                );
            });
        }
    }
}
