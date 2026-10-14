//! Surfaces operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn ensure_surface_materialized(&mut self, drawable: ResourceId) -> Result<(), ProtocolError> {
        let Some(surface) = self.surfaces.get(&drawable.0) else {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
        };
        if surface.pixels.is_some() {
            return Ok(());
        }
        let required = surface
            .pixel_count()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        let allocated = self
            .surfaces
            .values()
            .map(Surface::materialized_bytes)
            .fold(0usize, usize::saturating_add);
        if required > self.config.max_surface_bytes.saturating_sub(allocated) {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        if !self
            .surfaces
            .get_mut(&drawable.0)
            .is_some_and(Surface::materialize)
        {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        Ok(())
    }

    pub(super) fn resize_surface(
        &mut self,
        drawable: ResourceId,
        width: u16,
        height: u16,
    ) -> Result<(), ProtocolError> {
        let Some(surface) = self.surfaces.get(&drawable.0) else {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
        };
        if surface.width == width && surface.height == height {
            return Ok(());
        }
        let old_width = surface.width;
        let old_height = surface.height;
        // Managed client pixels are compositor-retained while the application
        // processes the resulting ConfigureNotify/WM_SIZE.  Extend the last
        // application-authored row and column into newly exposed space so a
        // live resize never flashes the X11 background pixel between the
        // geometry update and the client's next paint.  Ordinary unmanaged
        // X11 pixmaps/windows retain the protocol background-fill behavior.
        let retain_managed_edge = self.managed_frames.contains_key(&drawable.0);
        let old_bytes = surface.materialized_bytes();
        let new_bytes = if surface.pixels.is_some() {
            usize::from(width)
                .checked_mul(usize::from(height))
                .and_then(|count| count.checked_mul(4))
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?
        } else {
            0
        };
        let allocated = self
            .surfaces
            .values()
            .map(Surface::materialized_bytes)
            .fold(0usize, usize::saturating_add);
        if new_bytes
            > self
                .config
                .max_surface_bytes
                .saturating_sub(allocated.saturating_sub(old_bytes))
        {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        let Some(surface) = self.surfaces.get_mut(&drawable.0) else {
            unreachable!();
        };
        let old_pixels = surface.pixels.take();
        surface.width = width;
        surface.height = height;
        if let Some(old_pixels) = old_pixels {
            let new_count = usize::from(width).saturating_mul(usize::from(height));
            let mut new_pixels = vec![surface.background_pixel; new_count];
            let copy_width = old_width.min(width);
            let copy_height = old_height.min(height);
            for y in 0..copy_height {
                let old_start = usize::from(y).saturating_mul(usize::from(old_width));
                let new_start = usize::from(y).saturating_mul(usize::from(width));
                let count = usize::from(copy_width);
                new_pixels[new_start..new_start + count]
                    .copy_from_slice(&old_pixels[old_start..old_start + count]);
            }
            if retain_managed_edge && old_width > 0 && old_height > 0 {
                for y in 0..copy_height {
                    let old_start = usize::from(y) * usize::from(old_width);
                    let new_start = usize::from(y) * usize::from(width);
                    let edge = old_pixels[old_start + usize::from(old_width) - 1];
                    new_pixels[new_start + usize::from(copy_width)..new_start + usize::from(width)]
                        .fill(edge);
                }
                let old_last_row = (usize::from(old_height) - 1) * usize::from(old_width);
                for y in copy_height..height {
                    let new_start = usize::from(y) * usize::from(width);
                    for x in 0..copy_width {
                        new_pixels[new_start + usize::from(x)] =
                            old_pixels[old_last_row + usize::from(x)];
                    }
                    let corner = old_pixels[old_last_row + usize::from(old_width) - 1];
                    new_pixels[new_start + usize::from(copy_width)..new_start + usize::from(width)]
                        .fill(corner);
                }
            }
            surface.pixels = Some(new_pixels);
        }
        Ok(())
    }

    pub(super) fn fill_surface(
        &mut self,
        drawable: ResourceId,
        rectangle: ClippedRectangle,
        source: u32,
        function: u8,
        plane_mask: u32,
    ) -> Result<(), ProtocolError> {
        let surface = self
            .surfaces
            .get(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let drawable_mask = depth_plane_mask(surface.depth);
        let unchanged_lazy = surface.pixels.is_none()
            && apply_gc_function(
                surface.background_pixel,
                source,
                function,
                plane_mask,
                drawable_mask,
            ) == surface.background_pixel;
        if unchanged_lazy {
            return Ok(());
        }
        self.ensure_surface_materialized(drawable)?;
        let Some(surface) = self.surfaces.get_mut(&drawable.0) else {
            unreachable!();
        };
        let stride = usize::from(surface.width);
        let Some(pixels) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        for y in rectangle.y..rectangle.y.saturating_add(rectangle.height) {
            for x in rectangle.x..rectangle.x.saturating_add(rectangle.width) {
                let index = usize::from(y)
                    .saturating_mul(stride)
                    .saturating_add(usize::from(x));
                pixels[index] =
                    apply_gc_function(pixels[index], source, function, plane_mask, drawable_mask);
            }
        }
        Ok(())
    }

    pub(super) fn gc_pixel_visible(&self, gc: &GraphicsContext, x: i32, y: i32) -> bool {
        if let Some(clip) = &gc.clip_rectangles
            && !clip.rectangles.chunks_exact(8).any(|wire| {
                let left =
                    i32::from(read_i16_le(&wire[0..2])).saturating_add(i32::from(gc.clip_x_origin));
                let top =
                    i32::from(read_i16_le(&wire[2..4])).saturating_add(i32::from(gc.clip_y_origin));
                let right = left.saturating_add(i32::from(read_u16(
                    ClientByteOrder::LittleEndian,
                    &wire[4..6],
                )));
                let bottom = top.saturating_add(i32::from(read_u16(
                    ClientByteOrder::LittleEndian,
                    &wire[6..8],
                )));
                x >= left && x < right && y >= top && y < bottom
            })
        {
            return false;
        }
        if let Some(mask_id) = gc.clip_mask {
            let mask_x = x - i32::from(gc.clip_x_origin);
            let mask_y = y - i32::from(gc.clip_y_origin);
            return self.surfaces.get(&mask_id.0).is_some_and(|surface| {
                mask_x >= 0
                    && mask_y >= 0
                    && mask_x < i32::from(surface.width)
                    && mask_y < i32::from(surface.height)
                    && surface.pixel(
                        u16::try_from(mask_x).unwrap_or(u16::MAX),
                        u16::try_from(mask_y).unwrap_or(u16::MAX),
                    ) & 1
                        != 0
            });
        }
        true
    }

    pub(super) fn draw_raster_points(
        &mut self,
        drawable: ResourceId,
        gc: &GraphicsContext,
        raster: &[(i32, i32, u32)],
    ) -> Result<Option<ClippedRectangle>, ProtocolError> {
        let (width, height) = self
            .surfaces
            .get(&drawable.0)
            .map(|surface| (i32::from(surface.width), i32::from(surface.height)))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let stroke_width = i32::from(gc.line_width.max(1));
        let low = -(stroke_width / 2);
        let high = low.saturating_add(stroke_width);
        let mut pixels = BTreeMap::<(u16, u16), u32>::new();
        for &(center_x, center_y, source) in raster {
            for offset_y in low..high {
                for offset_x in low..high {
                    let x = center_x.saturating_add(offset_x);
                    let y = center_y.saturating_add(offset_y);
                    if x < 0
                        || y < 0
                        || x >= width
                        || y >= height
                        || !self.gc_pixel_visible(gc, x, y)
                    {
                        continue;
                    }
                    pixels.insert(
                        (
                            u16::try_from(x).unwrap_or(u16::MAX),
                            u16::try_from(y).unwrap_or(u16::MAX),
                        ),
                        source,
                    );
                }
            }
        }
        if pixels.is_empty() {
            return Ok(None);
        }
        self.ensure_surface_materialized(drawable)?;
        let surface = self
            .surfaces
            .get_mut(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let stride = usize::from(surface.width);
        let Some(destination) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        let mut min_x = u16::MAX;
        let mut min_y = u16::MAX;
        let mut max_x = 0;
        let mut max_y = 0;
        for ((x, y), source) in pixels {
            let index = usize::from(y)
                .saturating_mul(stride)
                .saturating_add(usize::from(x));
            destination[index] = apply_gc_function(
                destination[index],
                source,
                gc.function,
                gc.plane_mask,
                depth_plane_mask(gc.depth),
            );
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        Ok(Some(ClippedRectangle {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        }))
    }

    pub(super) fn gc_clip_coverage(&self, gc: &GraphicsContext, rectangle: ClippedRectangle) -> Vec<bool> {
        let count = usize::from(rectangle.width).saturating_mul(usize::from(rectangle.height));
        let mut coverage = vec![true; count];
        if let Some(clip) = &gc.clip_rectangles {
            for (index, covered) in coverage.iter_mut().enumerate() {
                let x = i32::from(rectangle.x)
                    + i32::try_from(index % usize::from(rectangle.width)).unwrap_or(i32::MAX);
                let y = i32::from(rectangle.y)
                    + i32::try_from(index / usize::from(rectangle.width)).unwrap_or(i32::MAX);
                *covered = clip.rectangles.chunks_exact(8).any(|wire| {
                    let left = i32::from(read_i16_le(&wire[0..2]))
                        .saturating_add(i32::from(gc.clip_x_origin));
                    let top = i32::from(read_i16_le(&wire[2..4]))
                        .saturating_add(i32::from(gc.clip_y_origin));
                    let right = left.saturating_add(i32::from(read_u16(
                        ClientByteOrder::LittleEndian,
                        &wire[4..6],
                    )));
                    let bottom = top.saturating_add(i32::from(read_u16(
                        ClientByteOrder::LittleEndian,
                        &wire[6..8],
                    )));
                    x >= left && x < right && y >= top && y < bottom
                });
            }
        }
        if let Some(mask_id) = gc.clip_mask {
            let mask = self.surfaces.get(&mask_id.0);
            for (index, covered) in coverage.iter_mut().enumerate() {
                if !*covered {
                    continue;
                }
                let x = i32::from(rectangle.x)
                    + i32::try_from(index % usize::from(rectangle.width)).unwrap_or(i32::MAX)
                    - i32::from(gc.clip_x_origin);
                let y = i32::from(rectangle.y)
                    + i32::try_from(index / usize::from(rectangle.width)).unwrap_or(i32::MAX)
                    - i32::from(gc.clip_y_origin);
                *covered = mask.is_some_and(|surface| {
                    x >= 0
                        && y >= 0
                        && x < i32::from(surface.width)
                        && y < i32::from(surface.height)
                        && surface.pixel(
                            u16::try_from(x).unwrap_or(u16::MAX),
                            u16::try_from(y).unwrap_or(u16::MAX),
                        ) & 1
                            != 0
                });
            }
        }
        coverage
    }

    pub(super) fn draw_gc_rectangle(
        &mut self,
        drawable: ResourceId,
        rectangle: ClippedRectangle,
        gc: &GraphicsContext,
    ) -> Result<bool, ProtocolError> {
        if gc.fill_style == 0 && gc.clip_mask.is_none() && gc.clip_rectangles.is_none() {
            self.fill_surface(
                drawable,
                rectangle,
                gc.foreground,
                gc.function,
                gc.plane_mask,
            )?;
            return Ok(true);
        }
        let coverage = self.gc_clip_coverage(gc, rectangle);
        let pattern = match gc.fill_style {
            0 => None,
            1 => gc.tile.and_then(|id| self.surfaces.get(&id.0)).cloned(),
            2 | 3 => gc.stipple.and_then(|id| self.surfaces.get(&id.0)).cloned(),
            _ => {
                return Err(ProtocolError::new(
                    x11::error::BAD_VALUE,
                    u32::from(gc.fill_style),
                ));
            }
        };
        if gc.fill_style != 0 && pattern.is_none() {
            return Err(ProtocolError::new(x11::error::BAD_PIXMAP, 0));
        }
        self.ensure_surface_materialized(drawable)?;
        let surface = self
            .surfaces
            .get_mut(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let stride = usize::from(surface.width);
        let Some(pixels) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        let mut changed = false;
        for y_offset in 0..rectangle.height {
            for x_offset in 0..rectangle.width {
                let coverage_index = usize::from(y_offset)
                    .saturating_mul(usize::from(rectangle.width))
                    .saturating_add(usize::from(x_offset));
                if !coverage[coverage_index] {
                    continue;
                }
                let x = rectangle.x.saturating_add(x_offset);
                let y = rectangle.y.saturating_add(y_offset);
                let source = match (gc.fill_style, pattern.as_ref()) {
                    (0, _) => Some(gc.foreground),
                    (1, Some(pattern)) => Some(pattern_pixel(
                        pattern,
                        x,
                        y,
                        gc.tile_x_origin,
                        gc.tile_y_origin,
                    )),
                    (2, Some(pattern)) => {
                        (pattern_pixel(pattern, x, y, gc.tile_x_origin, gc.tile_y_origin) & 1 != 0)
                            .then_some(gc.foreground)
                    }
                    (3, Some(pattern)) => Some(
                        if pattern_pixel(pattern, x, y, gc.tile_x_origin, gc.tile_y_origin) & 1 != 0
                        {
                            gc.foreground
                        } else {
                            gc.background
                        },
                    ),
                    _ => None,
                };
                let Some(source) = source else {
                    continue;
                };
                let index = usize::from(y)
                    .saturating_mul(stride)
                    .saturating_add(usize::from(x));
                let updated = apply_gc_function(
                    pixels[index],
                    source,
                    gc.function,
                    gc.plane_mask,
                    depth_plane_mask(gc.depth),
                );
                changed |= updated != pixels[index];
                pixels[index] = updated;
            }
        }
        Ok(changed)
    }

    pub(super) fn damage_drawable(&mut self, drawable: ResourceId, rectangle: ClippedRectangle) {
        self.damage_drawable_with_scene(drawable, rectangle, Some(rectangle));
    }

    pub(super) fn damage_drawable_with_scene(
        &mut self,
        drawable: ResourceId,
        rectangle: ClippedRectangle,
        scene_rectangle: Option<ClippedRectangle>,
    ) {
        // XComposite NameWindowPixmap returns another resource name for the
        // redirected window's backing storage. Our bounded retained model
        // stores resources separately, so synchronize a draw through either
        // name before publishing damage. Without the reverse direction,
        // Chromium renderer swaps can update the named pixmap while Rice keeps
        // presenting the previous window pixels.
        let canonical = self
            .composite_pixmaps
            .get(&drawable.0)
            .copied()
            .unwrap_or(drawable);
        if canonical != drawable
            && let Some(surface) = self.surfaces.get(&drawable.0).cloned()
            && self.surfaces.get(&canonical.0).is_some_and(|window| {
                window.width == surface.width && window.height == surface.height
            })
        {
            self.surfaces.insert(canonical.0, surface);
        }
        let aliases: Vec<u32> = self
            .composite_pixmaps
            .iter()
            .filter_map(|(pixmap, source)| (*source == canonical).then_some(*pixmap))
            .collect();
        if !aliases.is_empty()
            && let Some(surface) = self.surfaces.get(&canonical.0).cloned()
        {
            for alias in aliases {
                self.surfaces.insert(alias, surface.clone());
            }
        }
        self.record_damage(
            canonical,
            &[x11xfixes::RegionRect {
                x: i16::try_from(rectangle.x).unwrap_or(i16::MAX),
                y: i16::try_from(rectangle.y).unwrap_or(i16::MAX),
                width: rectangle.width,
                height: rectangle.height,
            }],
        );
        if canonical != drawable {
            self.record_damage(
                drawable,
                &[x11xfixes::RegionRect {
                    x: i16::try_from(rectangle.x).unwrap_or(i16::MAX),
                    y: i16::try_from(rectangle.y).unwrap_or(i16::MAX),
                    width: rectangle.width,
                    height: rectangle.height,
                }],
            );
        }
        if self.windows.contains_key(&canonical.0)
            && let Some(rectangle) = scene_rectangle
        {
            self.scene_damage_rect(
                canonical,
                i32::from(rectangle.x),
                i32::from(rectangle.y),
                rectangle.width,
                rectangle.height,
            );
        }
    }

    pub(super) fn record_damage(&mut self, drawable: ResourceId, rectangles: &[x11xfixes::RegionRect]) {
        let ids: Vec<u32> = self
            .damage_resources
            .iter()
            .filter_map(|(id, damage)| (damage.drawable == drawable).then_some(*id))
            .collect();
        let Some(surface) = self.surfaces.get(&drawable.0) else {
            return;
        };
        let geometry = x11damage::Rectangle {
            x: 0,
            y: 0,
            width: surface.width,
            height: surface.height,
        };
        for id in ids {
            let Some(damage) = self.damage_resources.get_mut(&id) else {
                continue;
            };
            damage.pending.extend_from_slice(rectangles);
            damage.pending = normalize_region(std::mem::take(&mut damage.pending));
            let area = region_extents(&damage.pending);
            let owner = damage.owner;
            let level = damage.level;
            let timestamp = self.next_timestamp();
            self.emit_to_client(ClientHandle(owner), |output, sequence, order| {
                output.extend_from_slice(&x11damage::encode_damage_notify_event(
                    order,
                    DAMAGE_FIRST_EVENT,
                    sequence,
                    level,
                    drawable.0,
                    id,
                    timestamp,
                    x11damage::Rectangle {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: area.height,
                    },
                    geometry,
                ));
            });
        }
    }
}
