//! Window mapping operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn map_window(
        &mut self,
        handle: ClientHandle,
        _sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        let mut window = self
            .windows
            .get(&id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, id.0))?;
        if window.mapped {
            return Ok(Vec::new());
        }
        if self.config.native_window_manager
            && window.parent == ROOT_WINDOW
            && !window.override_redirect
        {
            let frame = self.manage_top_level(id)?;
            if frame != id {
                if let Some(state) = self.windows.get_mut(&frame.0) {
                    state.mapped = true;
                }
                self.scene_upsert(frame);
                self.scene_damage(frame);
                window = self.windows.get(&id.0).cloned().unwrap();
            }
        }
        if !window.override_redirect
            && let Some(redirect) =
                self.redirecting_client(window.parent, SUBSTRUCTURE_REDIRECT_MASK, handle)
        {
            self.emit_to_client(redirect, |output, sequence, order| {
                x11::encode_map_request_event(output, sequence, order, window.parent, id);
            });
            return Ok(Vec::new());
        }
        if let Some(state) = self.windows.get_mut(&id.0) {
            state.mapped = true;
        }
        let desktop_layer = self.config.native_window_manager
            && self.wm_window_has_type(id, "_NET_WM_WINDOW_TYPE_DESKTOP");
        if desktop_layer {
            self.restack(id, None, Some(1));
            self.restack(WM_TASKBAR, None, Some(0));
            self.restack(WM_MENU, None, Some(0));
        }
        let updated = self.windows.get(&id.0).cloned().unwrap();
        self.emit_structure_event(&updated, |output, sequence, order, event_window| {
            x11::encode_map_notify_event(
                output,
                sequence,
                order,
                event_window,
                id,
                updated.override_redirect,
            );
        });
        self.emit_window_event(id, VISIBILITY_CHANGE_MASK, |output, sequence, order| {
            x11::encode_visibility_notify_event(output, sequence, order, id, 0);
        });
        self.emit_window_event(id, EXPOSURE_MASK, |output, sequence, order| {
            x11::encode_expose_event(
                output,
                sequence,
                order,
                id,
                0,
                0,
                updated.width,
                updated.height,
                0,
            );
        });
        // Mapping an ancestor changes X11 map_state for every already-mapped
        // descendant from Unviewable to Viewable. Re-announce the complete
        // retained subtree; updating only the parent leaves valid child
        // surfaces permanently hidden in the browser scene when a toolkit
        // maps dialog controls before it maps their top-level.
        self.scene_upsert_subtree(id);
        self.scene_damage(id);
        // Only managed top-levels receive automatic focus. Toolkits remap
        // child widgets during resize; those mappings must not activate an
        // otherwise background application.
        // Override-redirect windows are client-owned menus, completion
        // popups, tooltips, and similar transient surfaces. A window manager
        // must map/stack them without stealing X input focus from their
        // application; the client may still explicitly focus one if needed.
        if self.config.native_window_manager
            && !desktop_layer
            && !updated.override_redirect
            && self.managed_frames.contains_key(&id.0)
            && !self.is_parked_managed_descendant(id)
        {
            self.set_focus(id);
            self.notify_x11_foreground(id);
        }
        if self.managed_frames.contains_key(&id.0) && self.map_state(id) == 2 {
            // Bound diagnostic/readiness state independently of untrusted
            // client request volume. A client cannot have more live managed
            // top-levels than the configured window budget.
            if self.presented_managed_windows.len() < self.config.max_windows {
                self.presented_managed_windows.push_back(id);
            }
        }
        if desktop_layer {
            self.rebuild_scene();
        }
        Ok(Vec::new())
    }

    pub(super) fn map_subwindows(
        &mut self,
        handle: ClientHandle,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let parent = request_resource(body).ok_or_else(bad_length)?;
        let children = self
            .windows
            .get(&parent.0)
            .map(|window| window.children.clone())
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, parent.0))?;
        for child in children {
            let mut child_body = Vec::with_capacity(4);
            child_body.extend_from_slice(&child.0.to_le_bytes());
            self.map_window(handle, sequence, &child_body)?;
        }
        Ok(Vec::new())
    }

    pub(super) fn unmap_window(
        &mut self,
        _sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        let window = self
            .windows
            .get(&id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, id.0))?;
        if id == ROOT_WINDOW || !window.mapped {
            return Ok(Vec::new());
        }
        let focus_fallback = self
            .is_descendant(self.focus_window, id)
            .then(|| self.focus_fallback_for_withdrawn_window(id));
        if let Some(state) = self.windows.get_mut(&id.0) {
            state.mapped = false;
        }
        // BackingStore NotUseful permits the server to discard window pixels
        // while the window is unmapped. Do so deterministically: retaining
        // those pixels lets a client that uses sparse compositor damage show
        // content from its previous renderer after remapping. map_window()
        // already sends a full Expose, so conforming clients repaint the new
        // surface while the lazy background covers untouched regions.
        if window.class == 1 && window.backing_store == 0 {
            if let Some(surface) = self.surfaces.get_mut(&id.0) {
                surface.pixels = None;
                surface.alpha_mode = SurfaceAlphaMode::Unknown;
            }
        }
        if let Some(frame) = self.managed_frames.get(&id.0).copied() {
            if let Some(state) = self.windows.get_mut(&frame.0) {
                state.mapped = false;
            }
            self.scene_upsert(frame);
        }
        // X11 automatically releases active grabs when their grab window
        // becomes not viewable. GTK completion/menu popups rely on that
        // lifecycle rule and do not always send an explicit UngrabKeyboard.
        self.release_stale_input_grabs();
        if let Some(fallback) = focus_fallback {
            self.set_focus(fallback);
        }
        self.emit_structure_event(&window, |output, sequence, order, event_window| {
            x11::encode_unmap_notify_event(output, sequence, order, event_window, id, false);
        });
        self.scene_upsert(id);
        Ok(Vec::new())
    }

    pub(super) fn unmap_subwindows(
        &mut self,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let parent = request_resource(body).ok_or_else(bad_length)?;
        let children = self
            .windows
            .get(&parent.0)
            .map(|window| window.children.clone())
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, parent.0))?;
        for child in children {
            let mut child_body = Vec::with_capacity(4);
            child_body.extend_from_slice(&child.0.to_le_bytes());
            self.unmap_window(sequence, &child_body)?;
        }
        Ok(Vec::new())
    }
}
