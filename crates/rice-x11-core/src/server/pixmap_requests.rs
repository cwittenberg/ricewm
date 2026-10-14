//! Pixmap requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn create_pixmap(
        &mut self,
        handle: ClientHandle,
        depth: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::create_pixmap_request(depth, body).ok_or_else(bad_length)?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(request.pixmap) || self.resource_exists(request.pixmap) {
            return Err(ProtocolError::new(
                x11::error::BAD_ID_CHOICE,
                request.pixmap.0,
            ));
        }
        if request.width == 0 || request.height == 0 {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        let reference = self
            .surfaces
            .get(&request.drawable.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, request.drawable.0))?;
        if reference.depth == 0 {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                request.drawable.0,
            ));
        }
        if !matches!(request.depth, 1 | 4 | 8 | 24 | 32) {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(request.depth),
            ));
        }
        if self.pixmaps.len() >= self.config.max_windows {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        self.pixmaps.insert(
            request.pixmap.0,
            Pixmap {
                id: request.pixmap,
                owner: handle.0,
            },
        );
        self.surfaces.insert(
            request.pixmap.0,
            Surface::new(request.width, request.height, request.depth, 0),
        );
        Ok(Vec::new())
    }

    pub(super) fn free_pixmap(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let pixmap = x11::free_resource_id(body).ok_or_else(bad_length)?;
        if !self.pixmaps.contains_key(&pixmap.0) {
            return Err(ProtocolError::new(x11::error::BAD_PIXMAP, pixmap.0));
        }
        self.release_pixmap_resource(pixmap);
        Ok(Vec::new())
    }

    pub(super) fn release_pixmap_resource(&mut self, pixmap: ResourceId) {
        self.pixmaps.remove(&pixmap.0);
        self.composite_pixmaps.remove(&pixmap.0);
        let referenced = self.drawable_is_referenced(pixmap);
        if !referenced {
            self.surfaces.remove(&pixmap.0);
            return;
        }
        let mut internal = self.next_internal_drawable;
        while self.surfaces.contains_key(&internal) && internal >= 0xf000_0000 {
            internal = internal.saturating_sub(1);
        }
        if internal < 0xf000_0000 {
            // This requires billions of live retained pixmaps; keep the old
            // backing rather than invalidating existing Pictures.
            return;
        }
        self.next_internal_drawable = internal.saturating_sub(1);
        if let Some(surface) = self.surfaces.remove(&pixmap.0) {
            self.surfaces.insert(internal, surface);
            for picture in self.pictures.values_mut() {
                if matches!(picture.source, PictureSource::Drawable(id) if id == pixmap) {
                    picture.source = PictureSource::Drawable(ResourceId(internal));
                }
                if picture.clip_mask == Some(pixmap) {
                    picture.clip_mask = Some(ResourceId(internal));
                }
            }
            for gc in self.graphics_contexts.values_mut() {
                for reference in [&mut gc.tile, &mut gc.stipple, &mut gc.clip_mask] {
                    if *reference == Some(pixmap) {
                        *reference = Some(ResourceId(internal));
                    }
                }
            }
        }
    }

    pub(super) fn drawable_is_referenced(&self, drawable: ResourceId) -> bool {
        self.pictures.values().any(|picture| {
            matches!(picture.source, PictureSource::Drawable(id) if id == drawable)
                || picture.clip_mask == Some(drawable)
        }) || self.graphics_contexts.values().any(|gc| {
            [gc.tile, gc.stipple, gc.clip_mask]
                .into_iter()
                .flatten()
                .any(|reference| reference == drawable)
        })
    }

    pub(super) fn collect_orphan_surfaces(&mut self) {
        let candidates: Vec<u32> = self
            .surfaces
            .keys()
            .copied()
            .filter(|id| *id >= 0xf000_0000)
            .collect();
        for id in candidates {
            if !self.drawable_is_referenced(ResourceId(id)) {
                self.surfaces.remove(&id);
            }
        }
    }
}
