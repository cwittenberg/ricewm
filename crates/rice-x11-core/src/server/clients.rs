//! Clients operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Open a new X11 client and allocate its resource-ID range.
    ///
    /// # Errors
    ///
    /// Returns [`OpenError`] when the configured client limit is reached or
    /// no unique handle can be allocated.
    pub fn open_client(&mut self) -> Result<ClientHandle, OpenError> {
        if self.clients.len() >= usize::from(self.config.max_clients) {
            return Err(OpenError::ClientLimit);
        }
        let resource_slot = (1..=self.config.max_clients)
            .find(|slot| {
                self.clients
                    .values()
                    .all(|client| u16::from(client.resource_slot) != *slot)
            })
            .and_then(|slot| u8::try_from(slot).ok())
            .ok_or(OpenError::ClientLimit)?;
        for _ in 0..=u16::MAX {
            let handle = self.next_client_handle;
            self.next_client_handle = self.next_client_handle.wrapping_add(1).max(1);
            if handle != 0 && !self.clients.contains_key(&handle) {
                self.clients.insert(handle, Client::new(resource_slot));
                return Ok(ClientHandle(handle));
            }
        }
        Err(OpenError::HandleExhausted)
    }

    pub fn close_client(&mut self, handle: ClientHandle) -> bool {
        let Some(client) = self.clients.remove(&handle.0) else {
            return false;
        };
        // Save-set windows belong to another client. Move them out of any
        // subtree owned by the disconnecting client before that subtree is
        // destroyed.
        for id in client.save_set {
            let Some(saved) = self.windows.get(&id).cloned() else {
                continue;
            };
            if saved.owner == Some(handle.0) || saved.id == ROOT_WINDOW {
                continue;
            }
            if let Some(parent) = self.windows.get_mut(&saved.parent.0) {
                parent.children.retain(|child| child.0 != id);
            }
            if let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0)
                && !root.children.contains(&saved.id)
            {
                root.children.push(saved.id);
            }
            if let Some(window) = self.windows.get_mut(&id) {
                window.parent = ROOT_WINDOW;
                window.mapped = true;
            }
            self.scene_upsert(saved.id);
        }
        let owned: Vec<ResourceId> = self
            .windows
            .values()
            .filter(|window| window.owner == Some(handle.0))
            .map(|window| window.id)
            .collect();
        for window in owned {
            if self.windows.contains_key(&window.0) {
                self.destroy_window_tree(window);
            }
        }
        let pixmaps: Vec<u32> = self
            .pixmaps
            .values()
            .filter(|pixmap| pixmap.owner == handle.0)
            .map(|pixmap| pixmap.id.0)
            .collect();
        for pixmap in pixmaps {
            self.release_pixmap_resource(ResourceId(pixmap));
        }
        self.graphics_contexts.retain(|_, gc| gc.owner != handle.0);
        self.fonts.retain(|_, font| font.owner != handle.0);
        self.colormaps
            .retain(|_, colormap| colormap.owner != Some(handle.0));
        self.cursors.retain(|_, cursor| cursor.owner != handle.0);
        self.cursor_names
            .retain(|cursor, _| self.cursors.contains_key(cursor));
        self.pictures.retain(|_, picture| picture.owner != handle.0);
        let owned_glyphsets = self
            .glyphsets
            .iter()
            .filter_map(|(id, glyphset)| (glyphset.owner == handle.0).then_some(*id))
            .collect::<Vec<_>>();
        for glyphset in owned_glyphsets {
            self.release_glyphset_resource(ResourceId(glyphset));
        }
        self.xfixes_regions
            .retain(|_, region| region.owner != handle.0);
        self.xfixes_selection_masks
            .retain(|(owner, _, _), _| *owner != handle.0);
        self.xfixes_cursor_masks
            .retain(|(owner, _), _| *owner != handle.0);
        self.xfixes_barriers.retain(|_, owner| *owner != handle.0);
        self.shape_selects.retain(|(owner, _)| *owner != handle.0);
        self.sync_counters
            .retain(|_, counter| counter.owner != handle.0);
        self.sync_alarms.retain(|_, alarm| alarm.owner != handle.0);
        self.sync_fences.retain(|_, fence| fence.owner != handle.0);
        self.client_priorities.remove(&handle.0);
        self.randr_selects
            .retain(|(owner, _), _| *owner != handle.0);
        self.damage_resources
            .retain(|_, damage| damage.owner != handle.0);
        self.composite_redirects
            .retain(|(owner, _), _| *owner != handle.0);
        self.composite_overlay_users.remove(&handle.0);
        self.update_composite_overlay_mapping();
        self.composite_pixmaps.retain(|pixmap, _| {
            self.pixmaps
                .get(pixmap)
                .is_some_and(|resource| resource.owner != handle.0)
        });
        self.collect_orphan_surfaces();
        self.selections
            .retain(|_, (_, _, owner)| *owner != handle.0);
        if self.pointer_grab.is_some_and(|grab| grab.owner == handle.0) {
            self.pointer_grab = None;
        }
        if self
            .keyboard_grab
            .is_some_and(|grab| grab.owner == handle.0)
        {
            self.keyboard_grab = None;
        }
        true
    }

    /// Feed an ordered byte batch into one X11 client stream.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] for an unknown or non-writable client, or when
    /// a configured ingress/egress/protocol limit is violated.
    pub fn feed(&mut self, handle: ClientHandle, bytes: &[u8]) -> Result<usize, ClientError> {
        {
            let client = self
                .clients
                .get_mut(&handle.0)
                .ok_or(ClientError::UnknownClient)?;
            if !matches!(
                client.phase,
                ClientPhase::AwaitingSetup | ClientPhase::Ready { .. }
            ) {
                return Err(ClientError::ClientNotWritable);
            }
            let available = self
                .config
                .max_ingress_bytes
                .saturating_sub(client.ingress.len());
            if bytes.len() > available {
                return Err(ClientError::IngressLimit);
            }
            client.ingress.extend_from_slice(bytes);
        }
        loop {
            let phase = self
                .clients
                .get(&handle.0)
                .map(|client| client.phase)
                .ok_or(ClientError::UnknownClient)?;
            let progress = match phase {
                ClientPhase::AwaitingSetup => self.process_setup(handle),
                ClientPhase::Ready { byte_order } => self.process_request(handle, byte_order),
                ClientPhase::Rejected | ClientPhase::Faulted => Progress::Stop,
            };
            match progress {
                Progress::Advanced => {}
                Progress::NeedMore | Progress::Stop => break,
            }
        }
        // Compact only once per batch. Draining the front for each request
        // repeatedly moved the remaining video stripes (quadratic copying).
        if let Some(client) = self.clients.get_mut(&handle.0) {
            client.ingress.drain(..client.ingress_offset);
            client.ingress_offset = 0;
        }
        Ok(bytes.len())
    }

    /// Drain queued replies, errors, and events for one client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::UnknownClient`] when `handle` is not open.
    pub fn drain(
        &mut self,
        handle: ClientHandle,
        destination: &mut [u8],
    ) -> Result<usize, ClientError> {
        let client = self
            .clients
            .get_mut(&handle.0)
            .ok_or(ClientError::UnknownClient)?;
        let count = destination.len().min(client.egress.len());
        for (index, byte) in destination[..count].iter_mut().enumerate() {
            let Some(queued) = client.egress.pop_front() else {
                return Ok(index);
            };
            *byte = queued;
        }
        Ok(count)
    }

    /// Return the protocol state of one client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::UnknownClient`] when `handle` is not open.
    pub fn status(&self, handle: ClientHandle) -> Result<ClientStatus, ClientError> {
        self.clients
            .get(&handle.0)
            .map(|client| client.phase.status())
            .ok_or(ClientError::UnknownClient)
    }

    /// Return the number of output bytes waiting for one client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::UnknownClient`] when `handle` is not open.
    pub fn pending_output(&self, handle: ClientHandle) -> Result<usize, ClientError> {
        self.clients
            .get(&handle.0)
            .map(|client| client.egress.len())
            .ok_or(ClientError::UnknownClient)
    }

    /// Return the number of incomplete input bytes held for one client.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::UnknownClient`] when `handle` is not open.
    pub fn pending_input(&self, handle: ClientHandle) -> Result<usize, ClientError> {
        self.clients
            .get(&handle.0)
            .map(|client| client.ingress.len())
            .ok_or(ClientError::UnknownClient)
    }

    #[must_use]
    pub fn pending_scene(&self) -> usize {
        self.scene.pending()
    }

    pub fn drain_scene(&mut self, destination: &mut [u8]) -> usize {
        self.scene.drain(destination)
    }

    /// Return the byte size of the current browser-native WOFF2 text model.
    ///
    /// Chrome pixels retain the compact bitmap text as a transport fallback;
    /// the browser consumes this semantic snapshot to cover those glyphs and
    /// redraw them with the loaded `RiceWM` WOFF2 face.
    #[must_use]
    pub fn shell_text_snapshot_len(&self) -> usize {
        self.wm_text_overlays
            .iter()
            .filter(|overlay| self.windows.contains_key(&overlay.window.0))
            .map(WmTextOverlay::record_len)
            .sum()
    }

    /// Serialize the current native-chrome text overlays.
    ///
    /// Each padded record has a 24-byte little-endian header followed by up
    /// to 255 bytes of text. Alignment 3 denotes nonpainting window-title
    /// metadata; these records have zero geometry and no icon/rotation flags.
    pub fn write_shell_text_snapshot(&self, destination: &mut [u8]) -> Option<usize> {
        let required = self.shell_text_snapshot_len();
        if destination.len() < required {
            return None;
        }
        destination[..required].fill(0);
        let mut cursor = 0usize;
        for overlay in self
            .wm_text_overlays
            .iter()
            .filter(|overlay| self.windows.contains_key(&overlay.window.0))
        {
            let record_len = overlay.record_len();
            let header = &mut destination[cursor..cursor + WM_SHELL_TEXT_HEADER_BYTES];
            header[0..4].copy_from_slice(&overlay.window.0.to_le_bytes());
            header[4..6].copy_from_slice(&overlay.x.to_le_bytes());
            header[6..8].copy_from_slice(&overlay.y.to_le_bytes());
            header[8..10].copy_from_slice(&overlay.width.to_le_bytes());
            header[10..12].copy_from_slice(&overlay.height.to_le_bytes());
            header[12] = overlay.font_px;
            header[13] = overlay.weight;
            header[14] = overlay.align;
            header[15] = u8::try_from(overlay.text.len()).unwrap_or(u8::MAX);
            header[16..20].copy_from_slice(&overlay.foreground.to_le_bytes());
            header[20..24].copy_from_slice(&overlay.background.to_le_bytes());
            let text_end = cursor + WM_SHELL_TEXT_HEADER_BYTES + overlay.text.len();
            destination[cursor + WM_SHELL_TEXT_HEADER_BYTES..text_end]
                .copy_from_slice(&overlay.text);
            cursor += record_len;
        }
        Some(cursor)
    }

    #[must_use]
    pub fn surface_geometry(&self, drawable: u32) -> Option<(u16, u16)> {
        self.surfaces
            .get(&drawable)
            .map(|surface| (surface.width, surface.height))
    }

    pub fn read_surface_rgba(
        &mut self,
        drawable: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        destination: &mut [u8],
    ) -> Option<usize> {
        let surface = self.surfaces.get_mut(&drawable)?;
        let right = u32::from(x).checked_add(u32::from(width))?;
        let bottom = u32::from(y).checked_add(u32::from(height))?;
        if right > u32::from(surface.width) || bottom > u32::from(surface.height) {
            return None;
        }
        let required = usize::from(width)
            .checked_mul(usize::from(height))?
            .checked_mul(4)?;
        if destination.len() < required {
            return None;
        }
        let source_width = usize::from(surface.width);
        let row_width = usize::from(width);
        // Desktop Chromium can convert large opaque damage rectangles in its
        // validated native row pool. The synchronous import pins this Wasm
        // backing store and joins every worker before returning. Any missing
        // host, rejected range, alpha surface, or small rectangle retains the
        // exact Wasm loop below.
        #[cfg(all(feature = "desktop-raster", target_arch = "wasm32"))]
        if surface.depth != 32 && required >= 4096 {
            if let Some(source) = surface.pixels.as_ref() {
                #[link(wasm_import_module = "rice_desktop_raster")]
                unsafe extern "C" {
                    fn readback(descriptor: *const u32) -> i32;
                }
                if let (Ok(source_len), Ok(output_len)) =
                    (u32::try_from(source.len()), u32::try_from(required))
                {
                    let descriptor = [
                        1,
                        source.as_ptr() as u32,
                        source_len,
                        destination.as_mut_ptr() as u32,
                        output_len,
                        u32::from(surface.width),
                        u32::from(surface.height),
                        u32::from(x),
                        u32::from(y),
                        u32::from(width),
                        u32::from(height),
                        0,
                    ];
                    // SAFETY: source, destination, and the descriptor remain
                    // live for the synchronous host call. The host validates
                    // all Wasm ranges and rejects aliases before dispatch.
                    if unsafe { readback(descriptor.as_ptr()) } == 1 {
                        return Some(required);
                    }
                }
            }
        }
        if surface.depth != 32 {
            // Opaque drawables do not need the ARGB/XRGB classification below.
            // Keep that work out of the full-frame readback loop.
            for row in 0..usize::from(height) {
                let source_start = (usize::from(y) + row) * source_width + usize::from(x);
                let output_start = row * row_width * 4;
                let output_row = &mut destination[output_start..output_start + row_width * 4];
                if let Some(source) = surface
                    .pixels
                    .as_ref()
                    .and_then(|pixels| pixels.get(source_start..source_start + row_width))
                {
                    for (pixel, rgba) in source.iter().copied().zip(output_row.chunks_exact_mut(4))
                    {
                        let argb = pixel.to_be_bytes();
                        rgba.copy_from_slice(&[argb[1], argb[2], argb[3], 0xff]);
                    }
                } else {
                    let argb = surface.background_pixel.to_be_bytes();
                    let rgba = [argb[1], argb[2], argb[3], 0xff];
                    for target in output_row.chunks_exact_mut(4) {
                        target.copy_from_slice(&rgba);
                    }
                }
            }
            return Some(required);
        }
        let alpha_mask = if surface.depth == 32 { 0 } else { 0xff };
        let mut saw_partial_alpha = false;
        let mut saw_zero_alpha_rgb = false;
        for row in 0..usize::from(height) {
            let source_start = usize::from(y)
                .saturating_add(row)
                .saturating_mul(source_width)
                .saturating_add(usize::from(x));
            let output_start = row.saturating_mul(row_width).saturating_mul(4);
            let output_row =
                &mut destination[output_start..output_start.saturating_add(row_width * 4)];
            if let Some(source) = surface
                .pixels
                .as_ref()
                .and_then(|pixels| pixels.get(source_start..source_start.saturating_add(row_width)))
            {
                for (pixel, rgba) in source.iter().copied().zip(output_row.chunks_exact_mut(4)) {
                    let argb = pixel.to_be_bytes();
                    saw_partial_alpha |= argb[0] > 0 && argb[0] < 0xff;
                    saw_zero_alpha_rgb |= argb[0] == 0 && (pixel & 0x00ff_ffff) != 0;
                    rgba[0] = argb[1];
                    rgba[1] = argb[2];
                    rgba[2] = argb[3];
                    rgba[3] = argb[0] | alpha_mask;
                }
            } else {
                let argb = surface.background_pixel.to_be_bytes();
                saw_partial_alpha |= argb[0] > 0 && argb[0] < 0xff;
                saw_zero_alpha_rgb |= argb[0] == 0 && (surface.background_pixel & 0x00ff_ffff) != 0;
                let rgba = [argb[1], argb[2], argb[3], argb[0] | alpha_mask];
                for target in output_row.chunks_exact_mut(4) {
                    target.copy_from_slice(&rgba);
                }
            }
        }
        if surface.depth == 32 {
            if saw_partial_alpha {
                surface.alpha_mode = crate::model::SurfaceAlphaMode::Argb;
            } else if surface.alpha_mode == crate::model::SurfaceAlphaMode::Unknown
                && saw_zero_alpha_rgb
            {
                // XRGB and ARGB share a 32-bit X11 visual. Toolkits using
                // premultiplied ARGB keep RGB zero wherever alpha is zero;
                // ordinary XRGB clients (notably Wine GDI child windows)
                // instead paint meaningful RGB while leaving the unused high
                // byte at zero. Remember that unambiguous distinction so the
                // browser does not composite a fully painted opaque client as
                // transparent.
                surface.alpha_mode = crate::model::SurfaceAlphaMode::OpaqueXrgb;
            }
            if surface.alpha_mode == crate::model::SurfaceAlphaMode::OpaqueXrgb {
                for alpha in destination[..required].iter_mut().skip(3).step_by(4) {
                    *alpha = 0xff;
                }
            }
        }
        Some(required)
    }

    /// Upload one tightly packed or row-padded BGRA8 rectangle directly into
    /// an existing drawable. This is the package-independent display-driver
    /// fast path used by native guest window-system adapters. Window creation,
    /// ownership, stacking, input, and lifecycle remain ordinary X11/RiceWM
    /// operations; this method only replaces an avoidable PutImage wire copy.
    pub fn write_surface_bgra(
        &mut self,
        drawable: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        stride: u32,
        source: &[u8],
    ) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        let input_row_bytes = usize::from(width).saturating_mul(4);
        let Ok(stride) = usize::try_from(stride) else {
            return false;
        };
        if stride < input_row_bytes {
            return false;
        }
        let Some(required) = stride
            .checked_mul(usize::from(height).saturating_sub(1))
            .and_then(|value| value.checked_add(input_row_bytes))
        else {
            return false;
        };
        if source.len() < required {
            return false;
        }
        let Some(surface) = self.surfaces.get(&drawable) else {
            return false;
        };
        if !matches!(surface.depth, 24 | 32) || x >= surface.width || y >= surface.height {
            return false;
        }
        // XPutImage clips damage at the destination drawable boundary. Native
        // drivers rely on that behavior during resize and non-client repaints,
        // so the direct path must clip identically instead of rejecting a
        // flush whose right or bottom edge races the latest window geometry.
        let width = width.min(surface.width - x);
        let height = height.min(surface.height - y);
        let row_bytes = usize::from(width).saturating_mul(4);
        let resource = ResourceId(drawable);
        if self.ensure_surface_materialized(resource).is_err() {
            return false;
        }
        let Some(surface) = self.surfaces.get_mut(&drawable) else {
            return false;
        };
        let surface_width = usize::from(surface.width);
        let depth = surface.depth;
        let Some(destination) = surface.pixels.as_mut() else {
            return false;
        };
        for row in 0..usize::from(height) {
            let source_start = row.saturating_mul(stride);
            let target_start = usize::from(y)
                .saturating_add(row)
                .saturating_mul(surface_width)
                .saturating_add(usize::from(x));
            let Some(source_row) = source.get(source_start..source_start + row_bytes) else {
                return false;
            };
            let Some(target_row) =
                destination.get_mut(target_start..target_start + usize::from(width))
            else {
                return false;
            };
            for (bgra, pixel) in source_row.chunks_exact(4).zip(target_row) {
                let alpha = if depth == 32 { u32::from(bgra[3]) } else { 0 };
                *pixel = (alpha << 24)
                    | (u32::from(bgra[2]) << 16)
                    | (u32::from(bgra[1]) << 8)
                    | u32::from(bgra[0]);
            }
        }
        self.damage_drawable(
            resource,
            ClippedRectangle {
                x,
                y,
                width,
                height,
            },
        );
        true
    }

    pub fn take_unsupported_request(&mut self) -> Option<(u8, u32)> {
        let (&opcode, &count) = self.unsupported_requests.iter().next()?;
        self.unsupported_requests.remove(&opcode);
        Some((opcode, count))
    }

    pub fn take_request_count(&mut self) -> Option<(u8, u32)> {
        let (&opcode, &count) = self.request_counts.iter().next()?;
        self.request_counts.remove(&opcode);
        Some((opcode, count))
    }
}
