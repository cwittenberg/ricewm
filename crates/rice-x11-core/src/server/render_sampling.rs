//! Render sampling operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_picture_samples(
        &self,
        picture: &Picture,
        x: i32,
        y: i32,
        width: u16,
        height: u16,
    ) -> Result<Vec<RenderPixel>, ProtocolError> {
        let count = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        match &picture.source {
            PictureSource::Solid(color) => Ok(vec![render_pixel_from_argb(*color); count]),
            PictureSource::Drawable(drawable) => {
                let surface = self
                    .surfaces
                    .get(&drawable.0)
                    .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
                // The usual untransformed nearest-neighbor blit can copy
                // pixels row by row. Avoid a matrix transform, floating-point
                // rounding, repeat lookup and bounds lookup for every pixel.
                // Keep the general sampler for edge/repeat/filter cases.
                if picture.transform == render_identity_transform()
                    && picture.filter == PictureFilter::Nearest
                {
                    if let (Ok(left), Ok(top)) = (usize::try_from(x), usize::try_from(y)) {
                        let source_width = usize::from(surface.width);
                        if left
                            .checked_add(usize::from(width))
                            .is_some_and(|right| right <= source_width)
                            && top
                                .checked_add(usize::from(height))
                                .is_some_and(|bottom| bottom <= usize::from(surface.height))
                        {
                            let mut output = Vec::with_capacity(count);
                if let Some(pixels) = surface.pixels.as_ref() {
                                for row in top..top + usize::from(height) {
                                    let start = row * source_width + left;
                                    if let Some(slice) =
                                        pixels.get(start..start + usize::from(width))
                                    {
                                        output.extend(slice.iter().copied().map(|pixel| {
                                            render_pixel_from_raw(
                                                pixel,
                                                surface.depth,
                                                picture.format,
                                            )
                                        }));
                                    } else {
                    for column in left..left + usize::from(width) {
                        output.push(render_pixel_from_surface(
                                                surface,
                                                picture.format,
                                                u16::try_from(column).unwrap_or_default(),
                                                u16::try_from(row).unwrap_or_default(),
                                            ));
                                        }
                                    }
                                }
                            } else {
                                output.resize(
                                    count,
                                    render_pixel_from_raw(
                            surface.background_pixel,
                                        surface.depth,
                            picture.format,
                            ),
                                );
                            }
                            return Ok(output)
            ;
                        }
                    }
                }
                let mut output = Vec::with_capacity(count);
                for row in 0..i32::from(height) {
                    for column in 0..i32::from(width) {
                        let point = transform_picture_point(
                            &picture.transform,
                            f64::from(x.saturating_add(column)) + 0.5,
                            f64::from(y.saturating_add(row)) + 0.5,
                        );
                        output.push(render_sample_surface(
                            surface,
                            picture.format,
                            point.0,
                            point.1,
                            picture.repeat,
                            picture.filter,
                        ));
                    }
                }
                Ok(output)
            }
            source => {
                let mut output = Vec::with_capacity(count);
                for row in 0..i32::from(height) {
                    for column in 0..i32::from(width) {
                        let point = transform_picture_point(
                            &picture.transform,
                            f64::from(x.saturating_add(column)) + 0.5,
                            f64::from(y.saturating_add(row)) + 0.5,
                        );
                        output.push(render_gradient_pixel(
                            source,
                            point.0,
                            point.1,
                            picture.repeat,
                        ));
                    }
                }
                Ok(output)
            }
        }
    }

    pub(super) fn render_picture_allows(&self, picture: &Picture, x: i32, y: i32) -> bool {
        if let Some(rectangles) = &picture.clip_rectangles {
            let local_x = x.saturating_sub(i32::from(picture.clip_x_origin));
            let local_y = y.saturating_sub(i32::from(picture.clip_y_origin));
            if !rectangles.iter().any(|&(rect_x, rect_y, width, height)| {
                local_x >= i32::from(rect_x)
                    && local_y >= i32::from(rect_y)
                    && local_x < i32::from(rect_x).saturating_add(i32::from(width))
                    && local_y < i32::from(rect_y).saturating_add(i32::from(height))
            }) {
                return false;
            }
        }
        if let Some(mask) = picture.clip_mask {
            let Some(surface) = self.surfaces.get(&mask.0) else {
                return false;
            };
            let mask_x = x.saturating_sub(i32::from(picture.clip_x_origin));
            let mask_y = y.saturating_sub(i32::from(picture.clip_y_origin));
            let (Ok(mask_x), Ok(mask_y)) = (u16::try_from(mask_x), u16::try_from(mask_y)) else {
                return false;
            };
            if mask_x >= surface.width || mask_y >= surface.height {
                return false;
            }
            if surface.pixel(mask_x, mask_y) & 1 == 0 {
                return false;
            }
        }
        true
    }

    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(super) fn render_paint_samples(
        &mut self,
        operation: u8,
        destination_picture: ResourceId,
        dst_x: i16,
        dst_y: i16,
        width: u16,
        height: u16,
        source: &[RenderPixel],
        mask: Option<&[RenderPixel]>,
        mask_component_alpha: bool,
        coverage: Option<&[u8]>,
    ) -> Result<(), ProtocolError> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        let required = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ALLOC, 0))?;
        if source.len() != required
            || mask.is_some_and(|pixels| pixels.len() != required)
            || coverage.is_some_and(|pixels| pixels.len() != required)
        {
            return Err(bad_length());
        }
        let picture = self
            .pictures
            .get(&destination_picture.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_VALUE, destination_picture.0))?;
        let PictureSource::Drawable(drawable) = picture.source else {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                destination_picture.0,
            ));
        };
        let destination = self
            .surfaces
            .get(&drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let Some(clipped) = clipped_rectangle(destination, dst_x, dst_y, width, height) else {
            return Ok(());
        };
        let clipped_len = usize::from(clipped.width).saturating_mul(usize::from(clipped.height));
        // Most application paints have no picture clip. The native raster ABI
        // still needs its coverage bitmap, but filling it once avoids a
        // per-pixel rectangle/mask decision on every full-frame paint.
        let mut allowed = vec![true; clipped_len];
        if picture.clip_rectangles.is_some() || picture.clip_mask.is_some() {
            for target_y in clipped.y..clipped.y.saturating_add(clipped.height) {
            for target_x in clipped.x..clipped.x.saturating_add(clipped.width) {
                let index = usize::from(target_y - clipped.y)
                        .saturating_mul(usize::from(clipped.width))
                        .saturating_add(usize::from(target_x - clipped.x));
                    allowed[index] = self.render_picture_allows(
                    &picture,
                    i32::from(target_x),
                    i32::from(target_y),
                );
                }
            }
        }
        let destination_width = usize::from(destination.width);
        let destination_format = picture.format;
        self.ensure_surface_materialized(drawable)?;
        let Some(surface) = self.surfaces.get_mut(&drawable.0) else {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0));
        };
        let Some(pixels) = surface.pixels.as_mut() else {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        };
        // Only desktop builds import this optional native raster boundary. The
        // browser build retains the original, synchronous Wasm implementation.
        // The host joins disjoint row jobs before returning: no request, scene
        // reader, checkpoint or Wasm allocation can observe a half-painted image.
        #[cfg(all(feature = "desktop-raster", target_arch = "wasm32"))]
        if usize::from(clipped.width) * usize::from(clipped.height) >= 4096 {
            #[link(wasm_import_module = "rice_desktop_raster")]
            unsafe extern "C" {
                fn paint(descriptor: *const u32) -> i32;
            }
            let format = match destination_format {
                x11::RENDER_FMT_A1 => 1,
                x11::RENDER_FMT_A8 => 2,
                x11::RENDER_FMT_ARGB32 if surface.depth == 32 => 3,
                _ => 0,
            };
            let descriptor = [
                1, u32::from(operation), format, u32::from(width), u32::from(height),
                source.as_ptr() as u32, mask.map_or(0, |v| v.as_ptr() as u32),
                coverage.map_or(0, |v| v.as_ptr() as u32), allowed.as_ptr() as u32,
                pixels.as_mut_ptr() as u32, destination_width as u32, pixels.len() as u32,
                u32::from(clipped.x), u32::from(clipped.y), u32::from(clipped.width),
                u32::from(clipped.height), (i32::from(clipped.x) - i32::from(dst_x)) as u32,
                (i32::from(clipped.y) - i32::from(dst_y)) as u32,
                u32::from(mask_component_alpha), 0,
            ];
            // All pointers are live for this synchronous call. The host validates
            // descriptor ranges and aliases before dispatch and never retains them.
            let result = unsafe { paint(descriptor.as_ptr()) };
            if result == 0 || result == 1 {
                if result == 1 {
                    self.damage_drawable(drawable, clipped);
                }
                return Ok(());
            }
        }
        let mut changed = false;
        for target_y in clipped.y..clipped.y.saturating_add(clipped.height) {
            for target_x in clipped.x..clipped.x.saturating_add(clipped.width) {
                let clipped_index = usize::from(target_y - clipped.y)
                    .saturating_mul(usize::from(clipped.width))
                    .saturating_add(usize::from(target_x - clipped.x));
                if !allowed[clipped_index] {
                    continue;
                }
                let source_x = i32::from(target_x).saturating_sub(i32::from(dst_x));
                let source_y = i32::from(target_y).saturating_sub(i32::from(dst_y));
                let Some(source_index) = usize::try_from(source_y)
                    .ok()
                    .and_then(|row| row.checked_mul(usize::from(width)))
                    .and_then(|base| {
                        usize::try_from(source_x)
                            .ok()
                            .and_then(|column| base.checked_add(column))
                    })
                else {
                    continue;
                };
                let mut incoming = source[source_index];
                if let Some(mask) = mask {
                    incoming =
                        render_apply_mask(incoming, mask[source_index], mask_component_alpha);
                }
                if let Some(coverage) = coverage {
                    incoming = render_scale_pixel(incoming, coverage[source_index]);
                }
                let destination_index = usize::from(target_y)
                    .saturating_mul(destination_width)
                    .saturating_add(usize::from(target_x));
                let current = render_pixel_from_raw(
                    pixels[destination_index],
                    surface.depth,
                    destination_format,
                );
                let blended = render_blend(operation, incoming, current)?;
                let encoded = render_pixel_to_raw(blended, surface.depth, destination_format);
                changed |= encoded != pixels[destination_index];
                pixels[destination_index] = encoded;
            }
        }
        if changed {
            self.damage_drawable(drawable, clipped);
        }
        Ok(())
    }
}
