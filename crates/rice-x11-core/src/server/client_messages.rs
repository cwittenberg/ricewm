//! Client messages operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn send_event(
        &mut self,
        handle: ClientHandle,
        sender_order: ClientByteOrder,
        propagate: u8,
        body: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        let request = x11::send_event_request(propagate, body).ok_or_else(bad_length)?;
        if !self.windows.contains_key(&request.destination.0) {
            return Err(ProtocolError::new(
                x11::error::BAD_WINDOW,
                request.destination.0,
            ));
        }
        if self.config.native_window_manager
            && request.destination == ROOT_WINDOW
            && self.handle_ewmh_client_message(request.event)
        {
            return Ok(Vec::new());
        }
        let mut destination = request.destination;
        let targets = loop {
            let direct = if request.event_mask == 0 {
                self.windows
                    .get(&destination.0)
                    .and_then(|window| window.owner)
                    .into_iter()
                    .collect::<Vec<u32>>()
            } else {
                self.clients
                    .iter()
                    .filter_map(|(id, client)| {
                        let selected = client.event_masks.get(&destination.0).copied().unwrap_or(0);
                        (selected & request.event_mask != 0).then_some(*id)
                    })
                    .collect()
            };
            if !direct.is_empty() || !request.propagate {
                break direct;
            }
            let Some(window) = self.windows.get(&destination.0) else {
                break Vec::new();
            };
            if destination == ROOT_WINDOW || window.parent == destination {
                break Vec::new();
            }
            destination = window.parent;
        };
        for target in targets {
            let Some(target_order) =
                self.clients
                    .get(&target)
                    .and_then(|client| match client.phase {
                        ClientPhase::Ready { byte_order } => Some(byte_order),
                        _ => None,
                    })
            else {
                continue;
            };
            let mut event = request.event.to_vec();
            if target_order != sender_order {
                let fields = x11::wire_swap::core_event_swap_table(event[0] & 0x7f);
                x11::wire_swap::swap_in_place(fields, ClientByteOrder::BigEndian, &mut event);
            }
            event[0] |= 0x80;
            let sequence = self
                .clients
                .get(&target)
                .map_or(SequenceNumber(0), |client| client.sequence);
            write_u16_at(target_order, &mut event[2..4], sequence.0);
            let _ = self.queue_to_client(ClientHandle(target), &event);
        }
        let _ = handle;
        Ok(Vec::new())
    }

    pub(super) fn handle_ewmh_client_message(&mut self, event: &[u8]) -> bool {
        if event.len() != 32 || event[0] & 0x7f != 33 || event[1] != 32 {
            return false;
        }
        let window = ResourceId(read_u32_le(&event[4..8]));
        let message_type = AtomId(read_u32_le(&event[8..12]));
        let name = self.atoms.name(message_type).unwrap_or_default().to_owned();
        match name.as_str() {
            "_NET_ACTIVE_WINDOW" => {
                self.activate_managed_window(window);
                true
            }
            "_NET_CLOSE_WINDOW" => {
                self.request_close_window(window);
                true
            }
            "_NET_WM_MOVERESIZE" => {
                // EWMH interactive move/resize is how Chromium/Electron turns
                // a CSS app-region into an OS-owned window drag on X11. The
                // coordinates are root-relative and direction 8 is a move;
                // directions 0..=7 name the standard edge/corner resizes.
                let direction = read_u32_le(&event[20..24]);
                if direction == 11 {
                    if let Some(client)=self.managed_client_ancestor(window) {
                        if self.auto_tile_enabled && self.live_tile_target(0,client.0).is_some() {
                            self.tile_move_request=Some((client.0,0,0,true));
                        }
                        if self.wm_drag.is_some_and(|drag|drag.client==client) {
                            self.wm_drag = None;
                            self.set_wm_snap_preview(None);
                        }
                    }
                    return true;
                }
                let resize_edges = match direction {
                    0 => WM_RESIZE_LEFT | WM_RESIZE_TOP,
                    1 => WM_RESIZE_TOP,
                    2 => WM_RESIZE_RIGHT | WM_RESIZE_TOP,
                    3 => WM_RESIZE_RIGHT,
                    4 => WM_RESIZE_RIGHT | WM_RESIZE_BOTTOM,
                    5 => WM_RESIZE_BOTTOM,
                    6 => WM_RESIZE_LEFT | WM_RESIZE_BOTTOM,
                    7 => WM_RESIZE_LEFT,
                    8 => 0,
                    _ => return true,
                };
                let Some(client) = self.managed_client_ancestor(window) else {
                    return true;
                };
                if resize_edges != 0
                    && (self.non_resizable_managed_windows.contains(&client.0)
                        || self.maximized_geometries.contains_key(&client.0))
                {
                    return true;
                }
                let Some(frame) = self.managed_frames.get(&client.0).copied() else {
                    return true;
                };
                let (Some(frame_state), Some(client_state)) = (
                    self.windows.get(&frame.0).cloned(),
                    self.windows.get(&client.0).cloned(),
                ) else {
                    return true;
                };
                let root_x = clamp_i16(
                    i32::try_from(read_u32_le(&event[12..16])).unwrap_or(i32::MAX));
                let root_y = clamp_i16(
                    i32::try_from(read_u32_le(&event[16..20])).unwrap_or(i32::MAX));
                if self.auto_tile_enabled && self.live_tile_target(0,client.0).is_some() {
                    // Transfer interactive moves to the shared layout owner.
                    // Floating geometry must never race its insertion preview.
                    if direction==8 && read_u32_le(&event[24..28])==1 && !self.tile_input_blocked() {
                        self.tile_move_request=Some((client.0,root_x,root_y,false));
                    }
                    return true;
                }
                let current_geometry = (
                    frame_state.x,
                    frame_state.y,
                    client_state.width,
                    client_state.height,
                );
                let restore_geometry = self
                    .snapped_geometries
                    .get(&client.0)
                    .map(|state| state.restore_geometry)
                    .or_else(|| self.maximized_geometries.get(&client.0).copied())
                    .unwrap_or(current_geometry);
                self.activate_managed_window(client);
                self.wm_drag = Some(WmDrag {
                    frame,
                    client,
                    pointer_x: root_x,
                    pointer_y: root_y,
                    frame_x: frame_state.x,
                    frame_y: frame_state.y,
                    restore_geometry,
                    resize_geometry: current_geometry,
                    resize_edges,
                    moved: false,
                    restore_on_move: resize_edges == 0
                        && self.maximized_geometries.contains_key(&client.0),
                });
                true
            }
            "_NET_CURRENT_DESKTOP" => {
                self.switch_workspace(read_u32_le(&event[12..16]));
                true
            }
            "_NET_WM_DESKTOP" => {
                let workspace = read_u32_le(&event[12..16]);
                self.move_managed_window_to_workspace(window, workspace);
                true
            }
            "_NET_WM_STATE" => {
                let action = read_u32_le(&event[12..16]);
                let first = AtomId(read_u32_le(&event[16..20]));
                let second = AtomId(read_u32_le(&event[20..24]));
                let fullscreen = self.atoms.intern("_NET_WM_STATE_FULLSCREEN", false);
                let hidden = self.atoms.intern("_NET_WM_STATE_HIDDEN", false);
                let maximized_vert = self.atoms.intern("_NET_WM_STATE_MAXIMIZED_VERT", false);
                let maximized_horz = self.atoms.intern("_NET_WM_STATE_MAXIMIZED_HORZ", false);
                if first == hidden || second == hidden {
                    let currently = self.minimized_windows.contains(&window.0);
                    let enable = action == 1 || (action == 2 && !currently);
                    if enable {
                        self.minimized_windows.insert(window.0);
                    } else {
                        self.minimized_windows.remove(&window.0);
                    }
                    self.apply_workspace_visibility();
                }
                if first == fullscreen || second == fullscreen {
                    self.toggle_fullscreen(window, action);
                }
                if first == maximized_vert
                    || second == maximized_vert
                    || first == maximized_horz
                    || second == maximized_horz
                {
                    self.toggle_maximized(window, action);
                }
                true
            }
            "_NET_REQUEST_FRAME_EXTENTS" => {
                if self.managed_frames.contains_key(&window.0) {
                    self.publish_managed_frame_extents(window);
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn toggle_fullscreen(&mut self, client: ResourceId, action: u32) {
        let Some(frame) = self.managed_frames.get(&client.0).copied() else {
            return;
        };
        let currently = self.restore_geometries.contains_key(&client.0);
        let enable = action == 1 || (action == 2 && !currently);
        if enable == currently {
            return;
        }
        self.x11_output_homes.remove(&client.0);
        if enable {
            if let (Some(frame_state), Some(client_state)) =
                (self.windows.get(&frame.0), self.windows.get(&client.0))
            {
                self.restore_geometries.insert(
                    client.0,
                    (
                        frame_state.x,
                        frame_state.y,
                        client_state.width,
                        client_state.height,
                    ),
                );
            }
            let (x, y, outer_width, outer_height) = self.wm_window_output_rect(frame, true);
            let (left, right, top, bottom) = self.managed_frame_insets(client);
            let width = outer_width.saturating_sub(left.saturating_add(right));
            let height = outer_height.saturating_sub(top.saturating_add(bottom));
            self.apply_managed_geometry(client, frame, x as i16, y as i16, width, height);
        } else if let Some((x, y, width, height)) = self.restore_geometries.remove(&client.0) {
            self.apply_managed_geometry(client, frame, x, y, width, height);
        }
    }

    pub(super) fn minimize_managed_window(&mut self, client: ResourceId) {
        if !self.managed_frames.contains_key(&client.0) {
            return;
        }
        self.minimized_windows.insert(client.0);
        self.apply_workspace_visibility();
        if self.focus_window == client {
            self.set_focus(ROOT_WINDOW);
        }
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
    }
}
