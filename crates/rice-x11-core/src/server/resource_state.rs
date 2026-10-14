//! Resource state operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn map_state(&self, id: ResourceId) -> u8 {
        let Some(window) = self.windows.get(&id.0) else {
            return 0;
        };
        if !window.mapped {
            return 0;
        }
        if id == ROOT_WINDOW {
            return 2;
        }
        let mut current = window.parent;
        for _ in 0..256 {
            let Some(parent) = self.windows.get(&current.0) else {
                return 1;
            };
            if !parent.mapped {
                return 1;
            }
            if current == ROOT_WINDOW || parent.parent == current {
                return 2;
            }
            current = parent.parent;
        }
        1
    }

    pub(super) fn is_descendant(&self, candidate: ResourceId, ancestor: ResourceId) -> bool {
        let mut current = candidate;
        for _ in 0..256 {
            if current == ancestor {
                return true;
            }
            let Some(window) = self.windows.get(&current.0) else {
                return false;
            };
            if current == ROOT_WINDOW || window.parent == current {
                return false;
            }
            current = window.parent;
        }
        true
    }

    /// Supply the embedding host's monotonic millisecond clock (X11 wraps at
    /// 32 bits). Events in one millisecond may legitimately share a timestamp.
    pub fn set_timestamp_millis(&mut self, milliseconds: u32) {
        self.timestamp = milliseconds.max(1);
        self.timestamp_from_clock = true;
    }

    pub(super) fn next_timestamp(&mut self) -> u32 {
        let value = self.timestamp;
        if !self.timestamp_from_clock {
            self.timestamp = self.timestamp.wrapping_add(1).max(1);
        }
        value
    }

    pub(super) fn resource_exists(&self, id: ResourceId) -> bool {
        self.windows.contains_key(&id.0)
            || self.pixmaps.contains_key(&id.0)
            || self.graphics_contexts.contains_key(&id.0)
            || self.fonts.contains_key(&id.0)
            || self.colormaps.contains_key(&id.0)
            || self.cursors.contains_key(&id.0)
            || self.pictures.contains_key(&id.0)
            || self.glyphsets.contains_key(&id.0)
            || self.xfixes_regions.contains_key(&id.0)
            || self.xfixes_barriers.contains_key(&id.0)
            || self.sync_counters.contains_key(&id.0)
            || self.sync_alarms.contains_key(&id.0)
            || self.sync_fences.contains_key(&id.0)
            || self.damage_resources.contains_key(&id.0)
    }

    pub(super) fn validate_gc_pixmaps(
        &self,
        gc_depth: u8,
        tile: Option<ResourceId>,
        stipple: Option<ResourceId>,
        clip_mask: Option<ResourceId>,
    ) -> Result<(), ProtocolError> {
        for pixmap in [tile, stipple, clip_mask].into_iter().flatten() {
            if !self.pixmaps.contains_key(&pixmap.0) {
                return Err(ProtocolError::new(x11::error::BAD_PIXMAP, pixmap.0));
            }
        }
        if let Some(tile) = tile
            && self.surfaces.get(&tile.0).map(|surface| surface.depth) != Some(gc_depth)
        {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, tile.0));
        }
        if let Some(stipple) = stipple
            && self.surfaces.get(&stipple.0).map(|surface| surface.depth) != Some(1)
        {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, stipple.0));
        }
        if let Some(clip_mask) = clip_mask
            && self.surfaces.get(&clip_mask.0).map(|surface| surface.depth) != Some(1)
        {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, clip_mask.0));
        }
        Ok(())
    }

    pub(super) fn gc_for_drawable(
        &self,
        gc: ResourceId,
        drawable: ResourceId,
    ) -> Result<GraphicsContext, ProtocolError> {
        let depth = self
            .surfaces
            .get(&drawable.0)
            .map(|surface| surface.depth)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, drawable.0))?;
        let gc = self
            .graphics_contexts
            .get(&gc.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_GC, gc.0))?;
        if gc.depth != depth {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, drawable.0));
        }
        Ok(gc)
    }
}
