//! Drawing requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn clear_area(
        &mut self,
        sequence: SequenceNumber,
        exposures: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::clear_area_request(body).ok_or_else(bad_length)?;
        let window = self
            .windows
            .get(&request.window.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, request.window.0))?;
        if window.class == 2 {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, request.window.0));
        }
        let surface = self
            .surfaces
            .get(&request.window.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, request.window.0))?;
        let nonnegative_x = u16::try_from(request.x.max(0)).unwrap_or(0);
        let nonnegative_y = u16::try_from(request.y.max(0)).unwrap_or(0);
        let width = if request.width == 0 {
            surface.width.saturating_sub(nonnegative_x)
        } else {
            request.width
        };
        let height = if request.height == 0 {
            surface.height.saturating_sub(nonnegative_y)
        } else {
            request.height
        };
        if let Some(rectangle) = clipped_rectangle(surface, request.x, request.y, width, height) {
            self.fill_surface(
                request.window,
                rectangle,
                window.background_pixel,
                3,
                u32::MAX,
            )?;
            self.damage_drawable(request.window, rectangle);
            if exposures != 0 {
                self.emit_window_event(
                    request.window,
                    EXPOSURE_MASK,
                    |output, target_sequence, order| {
                        x11::encode_expose_event(
                            output,
                            target_sequence,
                            order,
                            request.window,
                            rectangle.x,
                            rectangle.y,
                            rectangle.width,
                            rectangle.height,
                            0,
                        );
                    },
                );
            }
        }
        let _ = sequence;
        Ok(Vec::new())
    }

    pub(super) fn poly_point(&mut self, coordinate_mode: u8, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, wire_points) = x11::poly_line_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let points = decode_points(coordinate_mode, wire_points)?;
        let raster: Vec<(i32, i32, u32)> = points
            .into_iter()
            .map(|(x, y)| (x, y, gc.foreground))
            .collect();
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_line(&mut self, coordinate_mode: u8, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, wire_points) = x11::poly_line_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let points = decode_points(coordinate_mode, wire_points)?;
        let mut raster = Vec::new();
        let mut phase = usize::from(gc.dash_offset);
        if points.len() == 1 {
            raster.push((points[0].0, points[0].1, gc.foreground));
        }
        for (index, pair) in points.windows(2).enumerate() {
            rasterize_segment(&mut raster, pair[0], pair[1], &gc, index != 0, &mut phase)?;
        }
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_segment(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, segments) = x11::poly_segment_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let mut raster = Vec::new();
        for segment in segments.chunks_exact(8) {
            let start = (
                i32::from(read_i16_le(&segment[0..2])),
                i32::from(read_i16_le(&segment[2..4])),
            );
            let end = (
                i32::from(read_i16_le(&segment[4..6])),
                i32::from(read_i16_le(&segment[6..8])),
            );
            let mut phase = usize::from(gc.dash_offset);
            rasterize_segment(&mut raster, start, end, &gc, false, &mut phase)?;
        }
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_rectangle(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, rectangles) = x11::poly_fill_rectangle_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let mut raster = Vec::new();
        for rectangle in rectangles.chunks_exact(8) {
            let x = i32::from(read_i16_le(&rectangle[0..2]));
            let y = i32::from(read_i16_le(&rectangle[2..4]));
            let width = i32::from(read_u16(ClientByteOrder::LittleEndian, &rectangle[4..6]));
            let height = i32::from(read_u16(ClientByteOrder::LittleEndian, &rectangle[6..8]));
            let corners = [
                (x, y),
                (x.saturating_add(width), y),
                (x.saturating_add(width), y.saturating_add(height)),
                (x, y.saturating_add(height)),
                (x, y),
            ];
            let mut phase = usize::from(gc.dash_offset);
            for (index, pair) in corners.windows(2).enumerate() {
                rasterize_segment(&mut raster, pair[0], pair[1], &gc, index != 0, &mut phase)?;
            }
        }
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_arc(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, arcs) = x11::poly_arc_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let mut raster = Vec::new();
        for arc in arcs.chunks_exact(12) {
            let specification = decode_arc(arc);
            let points = arc_outline_points(specification);
            let mut phase = usize::from(gc.dash_offset);
            if points.len() == 1 {
                raster.push((points[0].0, points[0].1, gc.foreground));
            }
            for (index, pair) in points.windows(2).enumerate() {
                rasterize_segment(&mut raster, pair[0], pair[1], &gc, index != 0, &mut phase)?;
            }
        }
        if let Some(damage) = self.draw_raster_points(drawable, &gc, &raster)? {
            self.damage_drawable(drawable, damage);
        }
        Ok(Vec::new())
    }

    pub(super) fn fill_poly(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let gc_id = ResourceId(read_u32_le(body.get(4..8).ok_or_else(bad_length)?));
        let coordinate_mode = *body.get(9).ok_or_else(bad_length)?;
        let points = decode_points(coordinate_mode, body.get(12..).ok_or_else(bad_length)?)?;
        let gc = self.gc_for_drawable(gc_id, drawable)?;
        if points.len() < 3 {
            return Ok(Vec::new());
        }
        let (surface_width, surface_height) = self
            .surfaces
            .get(&drawable.0)
            .map(|surface| (i32::from(surface.width), i32::from(surface.height)))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let min_x = points.iter().map(|point| point.0).min().unwrap_or(0).max(0);
        let max_x = points
            .iter()
            .map(|point| point.0)
            .max()
            .unwrap_or(-1)
            .min(surface_width.saturating_sub(1));
        let min_y = points.iter().map(|point| point.1).min().unwrap_or(0).max(0);
        let max_y = points
            .iter()
            .map(|point| point.1)
            .max()
            .unwrap_or(-1)
            .min(surface_height.saturating_sub(1));
        for y in min_y..=max_y {
            let mut run_start = None;
            for x in min_x..=max_x.saturating_add(1) {
                let inside = x <= max_x && point_inside_polygon(x, y, &points, gc.fill_rule);
                match (run_start, inside) {
                    (None, true) => run_start = Some(x),
                    (Some(start), false) => {
                        let rectangle = ClippedRectangle {
                            x: u16::try_from(start).unwrap_or(0),
                            y: u16::try_from(y).unwrap_or(0),
                            width: u16::try_from(x.saturating_sub(start)).unwrap_or(u16::MAX),
                            height: 1,
                        };
                        if self.draw_gc_rectangle(drawable, rectangle, &gc)? {
                            self.damage_drawable(drawable, rectangle);
                        }
                        run_start = None;
                    }
                    _ => {}
                }
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_fill_rectangle(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, rectangles) = x11::poly_fill_rectangle_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        for rectangle in rectangles.chunks_exact(8) {
            let x = read_i16_le(&rectangle[0..2]);
            let y = read_i16_le(&rectangle[2..4]);
            let width = read_u16(ClientByteOrder::LittleEndian, &rectangle[4..6]);
            let height = read_u16(ClientByteOrder::LittleEndian, &rectangle[6..8]);
            let Some(clipped) = self
                .surfaces
                .get(&drawable.0)
                .and_then(|surface| clipped_rectangle(surface, x, y, width, height))
            else {
                continue;
            };
            if self.draw_gc_rectangle(drawable, clipped, &gc)? {
                self.damage_drawable(drawable, clipped);
            }
        }
        Ok(Vec::new())
    }

    pub(super) fn poly_fill_arc(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let drawable = request_resource(body).ok_or_else(bad_length)?;
        let (gc_id, arcs) = x11::poly_fill_arc_data(body).ok_or_else(bad_length)?;
        let gc = self.gc_for_drawable(ResourceId(gc_id), drawable)?;
        let (surface_width, surface_height) = self
            .surfaces
            .get(&drawable.0)
            .map(|surface| (i32::from(surface.width), i32::from(surface.height)))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        for wire in arcs.chunks_exact(12) {
            let arc = decode_arc(wire);
            let min_x = arc.x.max(0);
            let max_x = arc
                .x
                .saturating_add(i32::from(arc.width))
                .min(surface_width)
                .saturating_sub(1);
            let min_y = arc.y.max(0);
            let max_y = arc
                .y
                .saturating_add(i32::from(arc.height))
                .min(surface_height)
                .saturating_sub(1);
            for y in min_y..=max_y {
                let mut run_start = None;
                for x in min_x..=max_x.saturating_add(1) {
                    let inside = x <= max_x && point_inside_arc(x, y, arc);
                    match (run_start, inside) {
                        (None, true) => run_start = Some(x),
                        (Some(start), false) => {
                            let rectangle = ClippedRectangle {
                                x: u16::try_from(start).unwrap_or(0),
                                y: u16::try_from(y).unwrap_or(0),
                                width: u16::try_from(x.saturating_sub(start)).unwrap_or(u16::MAX),
                                height: 1,
                            };
                            if self.draw_gc_rectangle(drawable, rectangle, &gc)? {
                                self.damage_drawable(drawable, rectangle);
                            }
                            run_start = None;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(Vec::new())
    }
}
