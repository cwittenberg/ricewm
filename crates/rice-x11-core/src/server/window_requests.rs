//! Window requests operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn create_window(
        &mut self,
        handle: ClientHandle,
        sequence: SequenceNumber,
        depth: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::create_window_request(depth, body).ok_or_else(bad_length)?;
        let client = self
            .clients
            .get(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        if !client.owns_resource(request.window) || self.resource_exists(request.window) {
            return Err(ProtocolError::new(
                x11::error::BAD_ID_CHOICE,
                request.window.0,
            ));
        }
        if self.windows.len() > self.config.max_windows {
            return Err(ProtocolError::new(x11::error::BAD_ALLOC, 0));
        }
        let parent = self
            .windows
            .get(&request.parent.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, request.parent.0))?;
        if request.width == 0 || request.height == 0 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(request.width.min(request.height)),
            ));
        }
        let class = if request.class == 0 {
            parent.class
        } else {
            request.class
        };
        if class != 1 && class != 2 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(request.class),
            ));
        }
        if parent.class == 2 && class == 1 {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, request.parent.0));
        }
        if class == 2 && (depth != 0 || request.border_width != 0) {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, request.window.0));
        }
        let resolved_depth = if class == 2 {
            0
        } else if depth == 0 {
            parent.depth
        } else {
            depth
        };
        let visual = if request.visual.0 == 0 {
            parent.visual
        } else {
            request.visual
        };
        // InputOutput children may use any visual/depth pair advertised for
        // the screen; X11 does not require their depth to equal the parent's.
        // GTK4/libadwaita creates a 32-bit ARGB child below the 24-bit root,
        // which was incorrectly rejected here as CreateWindow BadMatch.
        let visual_depth = match visual {
            ROOT_VISUAL => 24,
            ARGB_VISUAL => 32,
            _ => return Err(ProtocolError::new(x11::error::BAD_MATCH, visual.0)),
        };
        if class == 1 && resolved_depth != visual_depth {
            return Err(ProtocolError::new(
                x11::error::BAD_MATCH,
                u32::from(resolved_depth),
            ));
        }
        let event_mask = request.event_mask.unwrap_or(0);
        self.validate_event_selection(handle, request.window, event_mask)?;
        let colormap = request.colormap.flatten().unwrap_or(parent.colormap);
        // CreateWindow's protocol struct does not expose CWCursor yet.
        let cursor = if request.value_mask & (1 << 14) != 0 {
            let offset = 28 + ((request.value_mask & ((1u32 << 14) - 1)).count_ones() as usize) * 4;
            Some(self.resolve_cursor(read_u32_le(body.get(offset..offset + 4).ok_or_else(bad_length)?))?)
        } else { None };
        let window = Window {
            id: request.window,
            owner: Some(handle.0),
            parent: request.parent,
            children: Vec::new(),
            x: request.x,
            y: request.y,
            width: request.width,
            height: request.height,
            border_width: request.border_width,
            depth: resolved_depth,
            class,
            visual,
            background_pixel: request.background_pixel.unwrap_or(parent.background_pixel),
            bit_gravity: request.bit_gravity.unwrap_or(0),
            win_gravity: request.win_gravity.unwrap_or(0),
            backing_store: request.backing_store.unwrap_or(0),
            backing_planes: request.backing_planes.unwrap_or(u32::MAX),
            backing_pixel: request.backing_pixel.unwrap_or(0),
            override_redirect: request.override_redirect.unwrap_or(false),
            save_under: request.save_under.unwrap_or(false),
            do_not_propagate_mask: request.do_not_propagate_mask.unwrap_or(0),
            colormap,
            mapped: false,
            properties: BTreeMap::new(),
        };
        self.windows.insert(request.window.0, window.clone());
        if let Some(Some(cursor)) = cursor {
            self.window_cursors.insert(request.window.0, cursor);
        }
        self.surfaces.insert(
            request.window.0,
            Surface::new(
                request.width,
                request.height,
                resolved_depth,
                window.background_pixel,
            ),
        );
        if let Some(parent) = self.windows.get_mut(&request.parent.0) {
            parent.children.push(request.window);
        }
        if event_mask != 0
            && let Some(client) = self.clients.get_mut(&handle.0)
        {
            client.event_masks.insert(request.window.0, event_mask);
        }
        self.emit_window_event(
            request.parent,
            SUBSTRUCTURE_NOTIFY_MASK,
            |output, target_sequence, order| {
                x11::encode_create_notify_event(
                    output,
                    target_sequence,
                    order,
                    request.parent,
                    request.window,
                    geometry(&window),
                    window.override_redirect,
                );
            },
        );
        self.scene_upsert(request.window);
        let _ = sequence;
        Ok(Vec::new())
    }

    pub(super) fn change_window_attributes(
        &mut self,
        handle: ClientHandle,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::change_window_attributes_request(body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&request.window.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, request.window.0));
        }
        let cursor = request.cursor.map(|id| self.resolve_cursor(id.0)).transpose()?;
        if let Some(mask) = request.event_mask {
            self.validate_event_selection(handle, request.window, mask)?;
            if let Some(client) = self.clients.get_mut(&handle.0) {
                if mask == 0 {
                    client.event_masks.remove(&request.window.0);
                } else {
                    client.event_masks.insert(request.window.0, mask);
                }
            }
        }
        let Some(window) = self.windows.get_mut(&request.window.0) else {
            unreachable!();
        };
        if let Some(pixel) = request.background_pixel {
            window.background_pixel = pixel;
            if let Some(surface) = self.surfaces.get_mut(&request.window.0) {
                surface.background_pixel = pixel & 0x00ff_ffff;
            }
        }
        if let Some(value) = request.bit_gravity {
            window.bit_gravity = value;
        }
        if let Some(value) = request.win_gravity {
            window.win_gravity = value;
        }
        if let Some(value) = request.backing_store {
            window.backing_store = value;
        }
        if let Some(value) = request.backing_planes {
            window.backing_planes = value;
        }
        if let Some(value) = request.backing_pixel {
            window.backing_pixel = value;
        }
        if let Some(value) = request.override_redirect {
            window.override_redirect = value;
        }
        if let Some(value) = request.save_under {
            window.save_under = value;
        }
        if let Some(value) = request.do_not_propagate_mask {
            window.do_not_propagate_mask = value;
        }
        if let Some(colormap) = request.colormap {
            window.colormap = colormap.unwrap_or(ROOT_COLORMAP);
        }
        if let Some(cursor) = cursor {
            self.assign_window_cursor(request.window, cursor);
        }
        self.scene_upsert(request.window);
        Ok(Vec::new())
    }

    pub(super) fn get_window_attributes(
        &self,
        handle: ClientHandle,
        byte_order: ClientByteOrder,
        sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        let window = self
            .windows
            .get(&id.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, id.0))?;
        let your_event_mask = self
            .clients
            .get(&handle.0)
            .and_then(|client| client.event_masks.get(&id.0))
            .copied()
            .unwrap_or(0);
        let all_event_masks = self
            .clients
            .values()
            .filter_map(|client| client.event_masks.get(&id.0))
            .fold(0, |combined, mask| combined | *mask);
        let attributes = WindowAttributes {
            visual: window.visual,
            class: window.class,
            bit_gravity: window.bit_gravity,
            win_gravity: window.win_gravity,
            backing_store: window.backing_store,
            backing_planes: window.backing_planes,
            backing_pixel: window.backing_pixel,
            save_under: window.save_under,
            map_is_installed: true,
            map_state: self.map_state(id),
            override_redirect: window.override_redirect,
            colormap: window.colormap,
            all_event_masks,
            your_event_mask,
            do_not_propagate_mask: window.do_not_propagate_mask,
        };
        let mut output = Vec::with_capacity(48);
        x11::write_get_window_attributes_reply(&mut output, byte_order, sequence, attributes)
            .map_err(|_| encoding_error())?;
        Ok(output)
    }

    pub(super) fn destroy_window(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let id = request_resource(body).ok_or_else(bad_length)?;
        if id == ROOT_WINDOW {
            return Ok(Vec::new());
        }
        if !self.windows.contains_key(&id.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, id.0));
        }
        self.destroy_window_tree(id);
        Ok(Vec::new())
    }

    pub(super) fn destroy_subwindows(&mut self, body: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let parent = request_resource(body).ok_or_else(bad_length)?;
        let children = self
            .windows
            .get(&parent.0)
            .map(|window| window.children.clone())
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, parent.0))?;
        for child in children {
            self.destroy_window_tree(child);
        }
        Ok(Vec::new())
    }

    pub(super) fn destroy_window_tree(&mut self, id: ResourceId) {
        let focus_fallback = self
            .is_descendant(self.focus_window, id)
            .then(|| self.focus_fallback_for_withdrawn_window(id));
        let associated_frame = self.managed_frames.remove(&id.0);
        if let Some(frame) = associated_frame {
            self.frame_clients.remove(&frame.0);
            self.x11_output_homes.remove(&id.0);
            self.window_workspaces.remove(&id.0);
            self.auto_tile_restore.remove(&id.0);
            self.overview_placed_windows.remove(&id.0);
            self.minimized_windows.remove(&id.0);
            self.non_resizable_managed_windows.remove(&id.0);
            self.restore_geometries.remove(&id.0);
            self.maximized_geometries.remove(&id.0);
            self.snapped_geometries.remove(&id.0);
            self.parked_managed_windows.remove(&id.0);
        }
        if let Some(client) = self.frame_clients.remove(&id.0) {
            self.managed_frames.remove(&client.0);
            self.x11_output_homes.remove(&client.0);
            self.window_workspaces.remove(&client.0);
            self.auto_tile_restore.remove(&client.0);
            self.overview_placed_windows.remove(&client.0);
            self.minimized_windows.remove(&client.0);
            self.non_resizable_managed_windows.remove(&client.0);
            self.restore_geometries.remove(&client.0);
            self.maximized_geometries.remove(&client.0);
            self.snapped_geometries.remove(&client.0);
            self.parked_managed_windows.remove(&client.0);
        }
        let Some(window) = self.windows.get(&id.0).cloned() else {
            return;
        };
        if let Some(fallback) = focus_fallback {
            self.set_focus(fallback);
        }
        for child in window.children.clone() {
            self.destroy_window_tree(child);
        }
        self.emit_structure_event(&window, |output, sequence, order, event_window| {
            x11::encode_destroy_notify_event(output, sequence, order, event_window, id);
        });
        if let Some(parent) = self.windows.get_mut(&window.parent.0) {
            parent.children.retain(|child| *child != id);
        }
        self.selections.retain(|_, (owner, _, _)| *owner != id);
        if self.pointer_grab.is_some_and(|grab| grab.window == id) {
            self.pointer_grab = None;
        }
        if self.keyboard_grab.is_some_and(|grab| grab.window == id) {
            self.keyboard_grab = None;
        }
        self.windows.remove(&id.0);
        self.window_cursors.remove(&id.0);
        self.surfaces.remove(&id.0);
        self.window_shapes.retain(|(window, _), _| *window != id.0);
        self.shape_selects.retain(|(_, window)| *window != id.0);
        self.xfixes_selection_masks
            .retain(|(_, window, _), _| *window != id.0);
        self.xfixes_cursor_masks
            .retain(|(_, window), _| *window != id.0);
        self.randr_selects.retain(|(_, window), _| *window != id.0);
        self.damage_resources
            .retain(|_, damage| damage.drawable != id);
        self.composite_redirects
            .retain(|(_, window), _| *window != id.0);
        self.composite_pixmaps.retain(|_, window| *window != id);
        for client in self.clients.values_mut() {
            client.event_masks.remove(&id.0);
            client.xi2_masks.retain(|(window, _), _| *window != id.0);
            client.save_set.remove(&id.0);
        }
        self.scene_remove(id);
        if let Some(frame) = associated_frame
            && self.windows.contains_key(&frame.0)
        {
            self.destroy_window_tree(frame);
        }
        if self.config.native_window_manager {
            self.update_wm_client_properties();
        }
    }

    pub(super) fn reparent_window(
        &mut self,
        _sequence: SequenceNumber,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        let parent = request_resource(body.get(4..).unwrap_or_default()).ok_or_else(bad_length)?;
        let x = read_i16_le(body.get(8..10).ok_or_else(bad_length)?);
        let y = read_i16_le(body.get(10..12).ok_or_else(bad_length)?);
        if window == ROOT_WINDOW {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, window.0));
        }
        let old = self
            .windows
            .get(&window.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window.0))?;
        if !self.windows.contains_key(&parent.0) {
            return Err(ProtocolError::new(x11::error::BAD_WINDOW, parent.0));
        }
        if self.is_descendant(parent, window) || parent == window {
            return Err(ProtocolError::new(x11::error::BAD_MATCH, parent.0));
        }
        if let Some(old_parent) = self.windows.get_mut(&old.parent.0) {
            old_parent.children.retain(|child| *child != window);
        }
        if let Some(new_parent) = self.windows.get_mut(&parent.0) {
            new_parent.children.push(window);
        }
        if let Some(state) = self.windows.get_mut(&window.0) {
            state.parent = parent;
            state.x = x;
            state.y = y;
        }
        let updated = self.windows.get(&window.0).cloned().unwrap();
        self.emit_structure_event(&updated, |output, sequence, order, event_window| {
            x11::encode_reparent_notify_event(
                output,
                sequence,
                order,
                event_window,
                window,
                parent,
                x,
                y,
                updated.override_redirect,
            );
        });
        self.scene_upsert(window);
        Ok(Vec::new())
    }

    pub(super) fn change_save_set(
        &mut self,
        handle: ClientHandle,
        mode: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let window = request_resource(body).ok_or_else(bad_length)?;
        let owner = self
            .windows
            .get(&window.0)
            .map(|state| state.owner)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, window.0))?;
        let client = self
            .clients
            .get_mut(&handle.0)
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_ACCESS, 0))?;
        match mode {
            0 => {
                if owner == Some(handle.0) || !client.save_set.insert(window.0) {
                    return Err(ProtocolError::new(x11::error::BAD_MATCH, window.0));
                }
            }
            1 => {
                if !client.save_set.remove(&window.0) {
                    return Err(ProtocolError::new(x11::error::BAD_MATCH, window.0));
                }
            }
            _ => return Err(ProtocolError::new(x11::error::BAD_VALUE, u32::from(mode))),
        }
        Ok(Vec::new())
    }

    pub(super) fn circulate_window(
        &mut self,
        handle: ClientHandle,
        direction: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let parent = request_resource(body).ok_or_else(bad_length)?;
        if direction > 1 {
            return Err(ProtocolError::new(
                x11::error::BAD_VALUE,
                u32::from(direction),
            ));
        }
        let children = self
            .windows
            .get(&parent.0)
            .map(|window| window.children.clone())
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, parent.0))?;
        let candidate = if direction == 0 {
            children.iter().find(|child| {
                self.windows
                    .get(&child.0)
                    .is_some_and(|window| window.mapped)
            })
        } else {
            children.iter().rev().find(|child| {
                self.windows
                    .get(&child.0)
                    .is_some_and(|window| window.mapped)
            })
        }
        .copied();
        let Some(child) = candidate else {
            return Ok(Vec::new());
        };
        if let Some(redirect) = self.redirecting_client(parent, SUBSTRUCTURE_REDIRECT_MASK, handle)
        {
            self.emit_to_client(redirect, |output, sequence, order| {
                let _ = x11::write_circulate_request_event(
                    output, order, sequence, parent, child, direction,
                );
            });
            return Ok(Vec::new());
        }
        if let Some(state) = self.windows.get_mut(&parent.0) {
            state.children.retain(|item| *item != child);
            if direction == 0 {
                state.children.push(child);
            } else {
                state.children.insert(0, child);
            }
        }
        let child_state = self.windows.get(&child.0).cloned().unwrap();
        self.emit_structure_event(&child_state, |output, sequence, order, event_window| {
            let _ = x11::write_circulate_notify_event(
                output,
                order,
                sequence,
                event_window,
                child,
                direction,
            );
        });
        self.rebuild_scene();
        Ok(Vec::new())
    }
}
