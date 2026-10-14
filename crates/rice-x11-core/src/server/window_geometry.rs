//! Window geometry operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn configure_window(
        &mut self,
        handle: ClientHandle,
        _sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::configure_window_request(body).ok_or_else(bad_length)?;
        if request.window == ROOT_WINDOW {
            return Ok(Vec::new());
        }
        let window = self
            .windows
            .get(&request.window.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, request.window.0))?;
        if request.width == Some(0) || request.height == Some(0) {
            return Err(ProtocolError::new(x11::error::BAD_VALUE, 0));
        }
        if let Some(frame) = self.managed_frames.get(&request.window.0).copied() {
            let before=self.windows.get(&frame.0).map(|w|(w.x,w.y,w.width,w.height));
            let result=self.configure_managed_window(request, &window, frame);
            if result.is_ok()&&before!=self.windows.get(&frame.0).map(|w|(w.x,w.y,w.width,w.height)){self.x11_output_homes.remove(&window.id.0);}
            return result;
        }
        if let Some(sibling) = request.sibling {
            let sibling_window = self
                .windows
                .get(&sibling.0)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, sibling.0))?;
            if sibling_window.parent != window.parent {
                return Err(ProtocolError::new(x11::error::BAD_MATCH, sibling.0));
            }
        }
        if let Some(redirect) =
            self.redirecting_client(window.parent, SUBSTRUCTURE_REDIRECT_MASK, handle)
        {
            self.emit_to_client(redirect, |output, sequence, order| {
                x11::encode_configure_request_event(
                    output,
                    sequence,
                    order,
                    window.parent,
                    request.window,
                    &request,
                );
            });
            return Ok(Vec::new());
        }
        if self
            .redirecting_client(request.window, RESIZE_REDIRECT_MASK, handle)
            .is_some()
            && (request.width.is_some() || request.height.is_some())
        {
            return Ok(Vec::new());
        }
        let new_width = request.width.unwrap_or(window.width);
        let new_height = request.height.unwrap_or(window.height);
        let resized = new_width != window.width || new_height != window.height;
        if resized {
            self.resize_surface(request.window, new_width, new_height)?;
        }
        let Some(state) = self.windows.get_mut(&request.window.0) else {
            unreachable!();
        };
        if let Some(value) = request.x {
            state.x = value;
        }
        if let Some(value) = request.y {
            state.y = value;
        }
        if let Some(value) = request.width {
            state.width = value;
        }
        if let Some(value) = request.height {
            state.height = value;
        }
        if let Some(value) = request.border_width {
            state.border_width = value;
        }
        let restacked = request.stack_mode.is_some();
        self.restack(request.window, request.sibling, request.stack_mode);
        let updated = self.windows.get(&request.window.0).cloned().unwrap();
        self.emit_structure_event(&updated, |output, sequence, order, event_window| {
            x11::encode_configure_notify_event(
                output,
                sequence,
                order,
                event_window,
                request.window,
                None,
                geometry(&updated),
                updated.override_redirect,
            );
        });
        if resized {
            self.emit_full_exposure(&updated);
        }
        if restacked {
            self.rebuild_scene();
        } else {
            self.scene_upsert(request.window);
            self.scene_damage(request.window);
        }
        Ok(Vec::new())
    }

    // Toolkit file choosers may restore a size from a larger monitor. Keep
    // their action row reachable and report the actual allocation to the client.
    pub(super) fn fit_managed_dialog(&mut self, client: ResourceId, x: i16, y: i16,
                          width: u16, height: u16) -> (i16, i16, u16, u16) {
        if !self.wm_window_has_type(client, "_NET_WM_WINDOW_TYPE_DIALOG")
            && self.managed_transient_owner_relation(client).is_none() {
            return (x, y, width, height);
        }
        let (ax, ay, aw, ah) = self.wm_workarea_rect();
        let (left, right, top, bottom) = self.managed_frame_insets(client);
        let width = width.min(aw.saturating_sub(left).saturating_sub(right).max(1));
        let height = height.min(ah.saturating_sub(top).saturating_sub(bottom).max(1));
        let max_x = ax.saturating_add(aw.saturating_sub(width.saturating_add(left).saturating_add(right)));
        let max_y = ay.saturating_add(ah.saturating_sub(height.saturating_add(top).saturating_add(bottom)));
        let x = i32::from(x).clamp(i32::from(ax), i32::from(max_x));
        let y = i32::from(y).clamp(i32::from(ay), i32::from(max_y));
        (i16::try_from(x).unwrap_or(i16::MAX), i16::try_from(y).unwrap_or(i16::MAX), width, height)
    }

    pub(super) fn configure_managed_window(
        &mut self,
        request: x11::ConfigureWindowRequest,
        window: &Window,
        frame: ResourceId,
    ) -> Result<Vec<u8>, ProtocolError> {
        let frame_state = self.windows.get(&frame.0).unwrap();
        let (new_x, new_y, new_width, new_height) = self.fit_managed_dialog(
            request.window,
            request.x.unwrap_or(frame_state.x), request.y.unwrap_or(frame_state.y),
            request.width.unwrap_or(window.width), request.height.unwrap_or(window.height),
        );
        let (frame_left, frame_right, frame_top, frame_bottom) =
            self.managed_frame_insets(request.window);
        let resized = new_width != window.width || new_height != window.height;
        if resized {
            self.resize_surface(request.window, new_width, new_height)?;
            self.resize_surface(
                frame,
                new_width
                    .saturating_add(frame_left)
                    .saturating_add(frame_right),
                new_height
                    .saturating_add(frame_top)
                    .saturating_add(frame_bottom),
            )?;
        }
        if let Some(state) = self.windows.get_mut(&request.window.0) {
            state.width = new_width;
            state.height = new_height;
            state.border_width = 0;
            state.x = i16::try_from(frame_left).unwrap_or_default();
            state.y = i16::try_from(frame_top).unwrap_or_default();
        }
        if let Some(state) = self.windows.get_mut(&frame.0) {
            state.x = new_x;
            state.y = new_y;
            state.width = new_width
                .saturating_add(frame_left)
                .saturating_add(frame_right);
            state.height = new_height
                .saturating_add(frame_top)
                .saturating_add(frame_bottom);
        }
        if request.stack_mode.is_some() {
            let sibling_frame = request
                .sibling
                .and_then(|sibling| self.managed_frames.get(&sibling.0).copied());
            self.restack(frame, sibling_frame, request.stack_mode);
        }
        let updated = self.windows.get(&request.window.0).cloned().unwrap();
        // This geometry was actually applied in response to the client's
        // ConfigureWindow request.  A real reparenting WM performs the
        // equivalent configure as the WM client, after which the X server
        // delivers an ordinary, parent-relative ConfigureNotify.  Only the
        // initial ICCCM reparent notification uses root-relative coordinates
        // and the synthetic bit.
        self.emit_managed_configure_notify(&updated, geometry(&updated), false);
        if resized {
            self.emit_full_exposure(&updated);
        }
        self.paint_wm_chrome(frame);
        self.scene_upsert(frame);
        self.scene_upsert(request.window);
        self.scene_damage(frame);
        self.scene_damage(request.window);
        if request.stack_mode.is_some() {
            self.rebuild_scene();
        }
        Ok(Vec::new())
    }

    pub(super) fn restack(&mut self, window: ResourceId, sibling: Option<ResourceId>, stack_mode: Option<u8>) {
        let Some(parent_id) = self.windows.get(&window.0).map(|item| item.parent) else {
            return;
        };
        if stack_mode.is_none() {
            return;
        }
        {
            let Some(parent) = self.windows.get_mut(&parent_id.0) else {
                return;
            };
            let Some(old_index) = parent.children.iter().position(|child| *child == window) else {
                return;
            };
            parent.children.remove(old_index);
            let sibling_index = sibling
                .and_then(|target| parent.children.iter().position(|child| *child == target));
            let insert_at = match stack_mode.unwrap_or(0) {
                1 | 3 => sibling_index.unwrap_or(0),
                _ => sibling_index.map_or(parent.children.len(), |index| index + 1),
            };
            parent
                .children
                .insert(insert_at.min(parent.children.len()), window);
        }
        if parent_id == ROOT_WINDOW && self.config.native_window_manager {
            // An owned top-level is independently composited, but ICCCM
            // requires it to remain above its owner. Re-establish that partial
            // ordering after every root restack so a client-side ConfigureWindow
            // cannot expose the disabled owner over a modal transient.
            self.preserve_managed_transient_stacking();
            // Client activation/restacking must never cover persistent WM
            // chrome. Keep the transient notification above the menu and
            // taskbar without requiring every X11 stacking path to remember
            // each internal surface.
            for overlay in [
                WM_TASKBAR,
                WM_GNOME_PANEL,
                WM_GNOME_MENU,
                WM_MENU,
                WM_TASK_PREVIEW,
                WM_TOOLTIP,
                WM_SNAP_PREVIEW,
                // Blocking application dialogs must remain above both X11
                // clients and browser-native RiceWM task surfaces, while
                // time-critical shell notifications remain topmost.
                WM_GNOME_DIALOG,
                WM_NOTIFICATION,
            ] {
                if let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0)
                    && let Some(index) = root.children.iter().position(|child| *child == overlay)
                {
                    root.children.remove(index);
                    root.children.push(overlay);
                }
            }
        }
    }

    pub(super) fn preserve_managed_transient_stacking(&mut self) {
        let relations = self
            .frame_clients
            .iter()
            .filter_map(|(frame, client)| {
                let owner = self.managed_transient_owner_relation(*client)?;
                let owner_frame = self.managed_frames.get(&owner.0).copied()?;
                Some((ResourceId(*frame), owner_frame))
            })
            .collect::<Vec<_>>();
        let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0) else {
            return;
        };
        for _ in 0..=relations.len() {
            let mut changed = false;
            for (transient_frame, owner_frame) in relations.iter().copied() {
                let Some(transient_index) = root
                    .children
                    .iter()
                    .position(|child| *child == transient_frame)
                else {
                    continue;
                };
                let Some(owner_index) =
                    root.children.iter().position(|child| *child == owner_frame)
                else {
                    continue;
                };
                if transient_index > owner_index {
                    continue;
                }
                root.children.remove(transient_index);
                let owner_index = root
                    .children
                    .iter()
                    .position(|child| *child == owner_frame)
                    .unwrap_or(root.children.len().saturating_sub(1));
                root.children.insert(owner_index + 1, transient_frame);
                changed = true;
            }
            if !changed {
                break;
            }
        }
    }

    pub(super) fn get_geometry(
        &self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        let geometry = if let Some(window) = self.windows.get(&id.0) {
            geometry(window)
        } else if self.pixmaps.contains_key(&id.0) {
            let surface = self
                .surfaces
                .get(&id.0)
                .ok_or_else(|| ProtocolError::new(x11::error::BAD_DRAWABLE, id.0))?;
            Geometry {
                root: ROOT_WINDOW,
                x: 0,
                y: 0,
                width: surface.width,
                height: surface.height,
                border_width: 0,
                depth: surface.depth,
            }
        } else {
            return Err(ProtocolError::new(x11::error::BAD_DRAWABLE, id.0));
        };
        let mut output = Vec::with_capacity(32);
        x11::write_get_geometry_reply(&mut output, byte_order, sequence, geometry)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn query_tree(
        &self,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        let window = self
            .windows
            .get(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, id.0))?;
        let parent = if id == ROOT_WINDOW {
            ResourceId(0)
        } else {
            window.parent
        };
        let mut output = Vec::with_capacity(32 + window.children.len() * 4);
        x11::write_query_tree_reply(
            &mut output,
            byte_order,
            sequence,
            ROOT_WINDOW,
            parent,
            &window.children,
        )
        .map_err(|_| encoding_error())?;
        Ok(output)
    }
}
