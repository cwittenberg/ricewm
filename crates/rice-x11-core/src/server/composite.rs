//! Composite operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn composite_request(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let result = match minor {
            x11composite::QUERY_VERSION => {
                if body.len() == 8 {
                    Ok(x11composite::encode_query_version_reply(
                        order,
                        sequence,
                        x11composite::MAJOR_VERSION,
                        x11composite::MINOR_VERSION,
                    ))
                } else {
                    Err(bad_length())
                }
            }
            x11composite::REDIRECT_WINDOW | x11composite::REDIRECT_SUBWINDOWS => {
                self.composite_redirect(handle, order, minor, body)
            }
            x11composite::UNREDIRECT_WINDOW | x11composite::UNREDIRECT_SUBWINDOWS => {
                self.composite_unredirect(handle, order, minor, body)
            }
            x11composite::CREATE_REGION_FROM_BORDER_CLIP => {
                self.composite_create_border_region(handle, order, body)
            }
            x11composite::NAME_WINDOW_PIXMAP => {
                self.composite_name_window_pixmap(handle, order, body)
            }
            x11composite::GET_OVERLAY_WINDOW => {
                self.composite_get_overlay_window(handle, order, sequence, body)
            }
            x11composite::RELEASE_OVERLAY_WINDOW => {
                self.composite_release_overlay_window(handle, order, body)
            }
            _ => Err(ProtocolError::new(
                x11::error::BAD_REQUEST,
                u32::from(minor),
            )),
        };
        result.map_err(|error| extension_error(error, minor))
    }

    pub(super) fn composite_redirect(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let window = read_u32(order, &body[0..4]);
        let update = body[4];
        if !self.windows.contains_key(&window) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window));
        }
        if update > 1 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(update)));
        }
        let subwindows = minor == x11composite::REDIRECT_SUBWINDOWS;
        if self
            .composite_redirects
            .iter()
            .any(|((owner, redirected), (_, subtree))| {
                *owner != handle.0 && *redirected == window && *subtree == subwindows
            })
        {
            return Err(ProtocolError::new(x11::error::BAD_ACCESS, window));
        }
        self.composite_redirects
            .insert((handle.0, window), (update, subwindows));
        Ok(Vec::new())
    }

    pub(super) fn composite_unredirect(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        minor: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let window = read_u32(order, &body[0..4]);
        if !self.windows.contains_key(&window) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window));
        }
        let subwindows = minor == x11composite::UNREDIRECT_SUBWINDOWS;
        if self
            .composite_redirects
            .get(&(handle.0, window))
            .is_some_and(|(_, subtree)| *subtree == subwindows)
        {
            self.composite_redirects.remove(&(handle.0, window));
        }
        Ok(Vec::new())
    }

    pub(super) fn composite_create_border_region(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let region = ResourceId(read_u32(order, &body[0..4]));
        let window_id = read_u32(order, &body[4..8]);
        let window = self
            .windows
            .get(&window_id)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window_id))?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(region) || self.resource_exists(region) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, region.0));
        }
        self.xfixes_regions.insert(
            region.0,
            XFixesRegion {
                owner: handle.0,
                rectangles: vec![x11xfixes::RegionRect {
                    x: -i16::try_from(window.border_width).unwrap_or(i16::MAX),
                    y: -i16::try_from(window.border_width).unwrap_or(i16::MAX),
                    width: window
                        .width
                        .saturating_add(window.border_width.saturating_mul(2)),
                    height: window
                        .height
                        .saturating_add(window.border_width.saturating_mul(2)),
                }],
            },
        );
        Ok(Vec::new())
    }

    pub(super) fn composite_name_window_pixmap(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if body.len() != 8 {
            return Err(bad_length());
        }
        let window = ResourceId(read_u32(order, &body[0..4]));
        let pixmap = ResourceId(read_u32(order, &body[4..8]));
        let state = self
            .windows
            .get(&window.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window.0))?;
        let redirected = self
            .composite_redirects
            .iter()
            .any(|((_, target), (_, subtree))| {
                *target == window.0 || (*subtree && state.parent.0 == *target)
            });
        if !redirected {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, window.0));
        }
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(pixmap) || self.resource_exists(pixmap) {
            return Err(ProtocolError::new(x11::error::BAD_ID_CHOICE, pixmap.0));
        }
        let surface = self
            .surfaces
            .get(&window.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_MATCH, window.0))?;
        self.pixmaps.insert(
            pixmap.0,
            Pixmap {
                id: pixmap,
                owner: handle.0,
            },
        );
        self.surfaces.insert(pixmap.0, surface);
        self.composite_pixmaps.insert(pixmap.0, window);
        Ok(Vec::new())
    }

    pub(super) fn composite_get_overlay_window(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = decode_u32_request(order, body)?;
        if !self.windows.contains_key(&window) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window));
        }
        if !self.windows.contains_key(&COMPOSITE_OVERLAY.0) {
            self.insert_internal_window(Window {
                id: COMPOSITE_OVERLAY,
                owner: None,
                parent: ROOT_WINDOW,
                children: Vec::new(),
                x: 0,
                y: 0,
                width: self.config.width_px,
                height: self.config.height_px,
                border_width: 0,
                depth: 32,
                class: 1,
                visual: ARGB_VISUAL,
                background_pixel: 0,
                bit_gravity: 0,
                win_gravity: 0,
                backing_store: 0,
                backing_planes: u32::MAX,
                backing_pixel: 0,
                override_redirect: true,
                save_under: false,
                do_not_propagate_mask: 0,
                colormap: ROOT_COLORMAP,
                mapped: true,
                properties: BTreeMap::new(),
            });
            self.scene_upsert(COMPOSITE_OVERLAY);
            self.scene_damage(COMPOSITE_OVERLAY);
        }
        self.composite_overlay_users.insert(handle.0);
        self.update_composite_overlay_mapping();
        Ok(x11composite::encode_get_overlay_window_reply(
            order,
            sequence,
            COMPOSITE_OVERLAY.0,
        ))
    }

    pub(super) fn composite_release_overlay_window(
        &mut self,
        handle: ClientHandle,
        order: ClientByteOrder,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = decode_u32_request(order, body)?;
        if !self.windows.contains_key(&window) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, window));
        }
        self.composite_overlay_users.remove(&handle.0);
        self.update_composite_overlay_mapping();
        Ok(Vec::new())
    }

    pub(super) fn update_composite_overlay_mapping(&mut self) {
        let mapped = !self.composite_overlay_users.is_empty();
        let changed = self
            .windows
            .get_mut(&COMPOSITE_OVERLAY.0)
            .is_some_and(|overlay| {
                if overlay.mapped == mapped {
                    return false;
                }
                overlay.mapped = mapped;
                true
            });
        if changed {
            self.scene_upsert(COMPOSITE_OVERLAY);
            self.scene_damage(COMPOSITE_OVERLAY);
        }
    }
}
