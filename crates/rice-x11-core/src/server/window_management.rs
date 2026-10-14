//! Window management operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn allocate_wm_frame(&mut self) -> Result<ResourceId, ProtocolError> {
        for _ in 0..0x00ff_ffff {
            let id = ResourceId(self.next_wm_frame);
            self.next_wm_frame = self.next_wm_frame.wrapping_add(1).max(WM_FIRST_FRAME);
            if !self.resource_exists(id) {
                return Ok(id);
            }
        }
        Err(ProtocolError::new(x11::error::BAD_ALLOC, 0))
    }

    pub(super) fn wm_window_has_type(&mut self, window: ResourceId, type_name: &str) -> bool {
        let property_atom = self.atoms.intern("_NET_WM_WINDOW_TYPE", true);
        let type_atom = self.atoms.intern(type_name, true);
        if property_atom.0 == 0 || type_atom.0 == 0 {
            return false;
        }
        self.windows
            .get(&window.0)
            .and_then(|window| window.properties.get(&property_atom))
            .is_some_and(|property| {
                property.format == 32
                    && property
                        .data
                        .chunks_exact(4)
                        .any(|value| u32::from_le_bytes(value.try_into().unwrap()) == type_atom.0)
            })
    }

    pub(super) fn managed_frame_insets(&self, client: ResourceId) -> (u16, u16, u16, u16) {
        let decorations = self.windows.get(&client.0).and_then(|window| {
            window.properties.iter().find_map(|(atom, property)| {
                (self.atoms.name(*atom) == Some("_MOTIF_WM_HINTS")
                    && property.format == 32
                    && property.data.len() >= 12
                    && read_u32_le(&property.data[0..4]) & 2 != 0)
                    .then(|| read_u32_le(&property.data[8..12]))
            })
        });
        // Motif ALL inverts the named exclusions; BORDER/RESIZEH and TITLE
        // are independent so client titlebars can retain a minimal WM frame.
        let has = |bit| {
            decorations.is_none_or(|mask| {
            if mask & 1 != 0 { mask & bit == 0 } else { mask & bit != 0 }
        })};
        let border = if has(2) || has(4) { WM_BORDER_WIDTH } else { 0 };
        let top = if has(8) { WM_TITLE_HEIGHT } else { border };
        (border, border, top, border)
    }

    pub(super) fn publish_managed_frame_extents(&mut self, client: ResourceId) {
        let mut extents = Vec::with_capacity(16);
        let (left, right, top, bottom) = self.managed_frame_insets(client);
        for value in [left, right, top, bottom] {
            extents.extend_from_slice(&u32::from(value).to_le_bytes());
        }
        self.set_wm_property(client, "_NET_FRAME_EXTENTS", "CARDINAL", 32, extents);
    }

    pub(super) fn refresh_managed_decorations(&mut self, client: ResourceId) {
        let Some(frame) = self.managed_frames.get(&client.0).copied() else {
            return;
        };
        let Some(frame_state) = self.windows.get(&frame.0).cloned() else {
            return;
        };
        let Some(client_state) = self.windows.get(&client.0).cloned() else {
            return;
        };
        self.publish_managed_frame_extents(client);
        self.apply_managed_geometry(
            client,
            frame,
            frame_state.x,
            frame_state.y,
            client_state.width,
            client_state.height,
        );
    }

    pub(super) fn promote_wm_desktop_layer(&mut self, client: ResourceId) {
        let Some(frame) = self.managed_frames.remove(&client.0) else {
            self.restack(client, None, Some(1));
            self.restack(WM_TASKBAR, None, Some(0));
            self.restack(WM_MENU, None, Some(0));
            self.rebuild_scene();
            return;
        };
        self.frame_clients.remove(&frame.0);
        self.x11_output_homes.remove(&client.0);
        self.window_workspaces.remove(&client.0);
        self.auto_tile_restore.remove(&client.0);
        self.overview_placed_windows.remove(&client.0);
        self.minimized_windows.remove(&client.0);
        self.non_resizable_managed_windows.remove(&client.0);
        self.parked_managed_windows.remove(&client.0);
        self.restore_geometries.remove(&client.0);
        self.maximized_geometries.remove(&client.0);
        self.snapped_geometries.remove(&client.0);
        if let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0) {
            root.children
                .retain(|child| *child != frame && *child != client);
            root.children.insert(0, client);
        }
        if let Some(window) = self.windows.get_mut(&client.0) {
            window.parent = ROOT_WINDOW;
            window.x = 0;
            window.y = 0;
            window.border_width = 0;
        }
        self.windows.remove(&frame.0);
        self.surfaces.remove(&frame.0);
        self.scene_remove(frame);
        if let Some(window) = self.windows.get(&client.0).cloned() {
            self.emit_structure_event(&window, |output, sequence, order, event_window| {
                x11::encode_reparent_notify_event(
                    output,
                    sequence,
                    order,
                    event_window,
                    client,
                    ROOT_WINDOW,
                    0,
                    0,
                    false,
                );
            });
            self.emit_managed_configure_notify(&window, geometry(&window), true);
        }
        if self.focus_window == client {
            self.set_focus(ROOT_WINDOW);
        }
        self.restack(WM_TASKBAR, None, Some(0));
        self.restack(WM_MENU, None, Some(0));
        self.update_wm_client_properties();
        self.rebuild_scene();
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn manage_top_level(&mut self, client_id: ResourceId) -> Result<ResourceId, ProtocolError> {
        if let Some(frame) = self.managed_frames.get(&client_id.0) {
            return Ok(*frame);
        }
        let client = self
            .windows
            .get(&client_id.0)
            .cloned()
            .ok_or_else(|| ProtocolError::new(x11::error::BAD_WINDOW, client_id.0))?;
        if client.parent != ROOT_WINDOW || client.override_redirect || client.class != 1 {
            return Ok(client_id);
        }
        if self.wm_window_has_type(client_id, "_NET_WM_WINDOW_TYPE_DESKTOP") {
            return Ok(client_id);
        }
        let frame = self.allocate_wm_frame()?;
        let cascade = u16::try_from(self.managed_frames.len() % 12).unwrap_or_default();
        let (work_x, work_y, work_width, work_height) = self.wm_workarea_rect();
        let (frame_left, frame_right, frame_top, frame_bottom) =
            self.managed_frame_insets(client_id);
        let frame_width = client
            .width
            .saturating_add(frame_left)
            .saturating_add(frame_right);
        let frame_height = client
            .height
            .saturating_add(frame_top)
            .saturating_add(frame_bottom);
        let suggested_x = if client.x == 0 {
            work_x
                .saturating_add(24)
                .saturating_add(cascade.saturating_mul(22))
        } else {
            u16::try_from(client.x.max(0)).unwrap_or_default()
        };
        let suggested_y = if client.y == 0 {
            work_y
                .saturating_add(20)
                .saturating_add(cascade.saturating_mul(18))
        } else {
            u16::try_from(client.y.max(0)).unwrap_or_default()
        };
        let x = if frame_width <= work_width {
            i16::try_from(
                suggested_x.clamp(
                    work_x,
                    work_x
                        .saturating_add(work_width)
                        .saturating_sub(frame_width),
                ),
            )
            .unwrap_or(i16::MAX)
        } else {
            // Preserve a client's requested size, but keep its close control
            // inside the work area when the decorated frame is wider than
            // the display. This is also valid for left-control themes: the
            // permitted origin interval is derived from the active chrome
            // layout rather than assuming controls live on one side.
            let profile = self
                .wm_themes
                .get(usize::from(self.wm_theme))
                .map_or(WmChromeProfile::Standard, |theme| theme.chrome_profile);
            let frame_width_usize = usize::from(frame_width);
            let close_left = i32::try_from(wm_frame_control_left(profile, frame_width_usize, 2))
                .unwrap_or(i32::MAX);
            let close_width = if profile == WmChromeProfile::Redmond98 {
                18
            } else {
                i32::from(WM_TITLE_BUTTON_WIDTH)
            };
            let work_left = i32::from(work_x);
            let work_right = work_left.saturating_add(i32::from(work_width));
            let minimum = work_left.saturating_sub(close_left);
            let maximum = work_right
                .saturating_sub(close_left)
                .saturating_sub(close_width)
                .max(minimum);
            i16::try_from(i32::from(suggested_x).clamp(minimum, maximum))
                .unwrap_or_else(|_| if minimum < 0 { i16::MIN } else { i16::MAX })
        };
        let y = suggested_y.clamp(
            work_y,
            work_y
                .saturating_add(work_height)
                .saturating_sub(frame_height)
                .max(work_y),
        );
        if let Some(root) = self.windows.get_mut(&ROOT_WINDOW.0) {
            root.children.retain(|child| *child != client_id);
        }
        self.managed_frames.insert(client_id.0, frame);
        self.frame_clients.insert(frame.0, client_id);
        if self.park_next_managed_window {
            self.park_next_managed_window = false;
            self.parked_managed_windows.insert(client_id.0);
        }
        self.window_workspaces
            .insert(client_id.0, self.current_workspace);
        self.insert_internal_window(Window {
            id: frame,
            owner: None,
            parent: ROOT_WINDOW,
            children: vec![client_id],
            x,
            y: i16::try_from(y).unwrap_or(i16::MAX),
            width: frame_width,
            height: frame_height,
            border_width: 0,
            depth: 32,
            class: 1,
            visual: ROOT_VISUAL,
            background_pixel: 0x0010_151c,
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
        if let Some(window) = self.windows.get_mut(&client_id.0) {
            window.parent = frame;
            window.x = i16::try_from(frame_left).unwrap_or_default();
            window.y = i16::try_from(frame_top).unwrap_or_default();
            window.border_width = 0;
        }
        self.publish_managed_frame_extents(client_id);
        self.set_wm_property(
            client_id,
            "_NET_WM_DESKTOP",
            "CARDINAL",
            32,
            self.current_workspace.to_le_bytes().to_vec(),
        );
        let mut wm_state = Vec::with_capacity(8);
        wm_state.extend_from_slice(&1u32.to_le_bytes());
        wm_state.extend_from_slice(&0u32.to_le_bytes());
        self.set_wm_property(client_id, "WM_STATE", "WM_STATE", 32, wm_state);
        let updated = self.windows.get(&client_id.0).cloned().unwrap();
        self.emit_structure_event(&updated, |output, sequence, order, event_window| {
            x11::encode_reparent_notify_event(
                output,
                sequence,
                order,
                event_window,
                client_id,
                frame,
                updated.x,
                updated.y,
                false,
            );
        });
        // ICCCM reparenting window managers must report the client's
        // resulting root-relative geometry even when the client did not issue
        // a ConfigureWindow request.  GTK transient dialogs defer allocation
        // and expose processing until this synthetic ConfigureNotify arrives;
        // omitting it leaves the real file chooser mapped as an empty
        // background surface.
        let configured = Geometry {
            root: ROOT_WINDOW,
            x: x.saturating_add(i16::try_from(frame_left).unwrap_or_default()),
            y: i16::try_from(y)
                .unwrap_or(i16::MAX)
                .saturating_add(i16::try_from(frame_top).unwrap_or_default()),
            width: updated.width,
            height: updated.height,
            border_width: 0,
            depth: updated.depth,
        };
        self.emit_managed_configure_notify(&updated, configured, true);
        let fitted = self.fit_managed_dialog(client_id, x, i16::try_from(y).unwrap_or(i16::MAX), updated.width, updated.height);
        if fitted != (x, i16::try_from(y).unwrap_or(i16::MAX), updated.width, updated.height) {
            self.apply_managed_geometry(client_id, frame, fitted.0, fitted.1, fitted.2, fitted.3);
        }
        self.paint_wm_chrome(frame);
        self.restack(WM_TASKBAR, None, Some(0));
        self.restack(WM_MENU, None, Some(0));
        self.scene_upsert(frame);
        self.scene_damage(frame);
        self.update_wm_client_properties();
        self.rebuild_scene();
        Ok(frame)
    }
}
