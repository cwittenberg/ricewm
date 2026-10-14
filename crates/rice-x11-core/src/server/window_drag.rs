//! Window drag operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn finish_wm_drag(&mut self, root_x: i16, root_y: i16) {
        let Some(drag) = self.wm_drag.take() else {
            self.set_wm_snap_preview(None);
            return;
        };
        if !drag.moved {
            self.set_wm_snap_preview(None);
            return;
        }
        self.x11_output_homes.remove(&drag.client.0);
        if drag.resize_edges != 0 {
            self.set_wm_snap_preview(None);
            self.maximized_geometries.remove(&drag.client.0);
            self.snapped_geometries.remove(&drag.client.0);
            return;
        }
        let target = self.wm_snap_target_at(root_x, root_y);
        self.set_wm_snap_preview(None);
        if let Some(target) = target {
            let area = self.wm_pointer_workarea(root_x, root_y).unwrap_or_else(|| self.wm_workarea_rect());
            self.apply_wm_snap(drag.client, target, drag.restore_geometry, area);
        } else {
            self.maximized_geometries.remove(&drag.client.0);
            self.snapped_geometries.remove(&drag.client.0);
        }
    }

    pub(super) fn apply_wm_snap(
        &mut self,
        client: ResourceId,
        target: WmSnapTarget,
        restore_geometry: (i16, i16, u16, u16),
        area: (u16,u16,u16,u16),
    ) {
        if self.non_resizable_managed_windows.contains(&client.0) {
            return;
        }
        let Some(frame) = self.managed_frames.get(&client.0).copied() else {
            return;
        };
        self.x11_output_homes.remove(&client.0);
        let (x, y, outer_width, outer_height) = self.wm_snap_geometry(target, area);
        let (left, right, top, bottom) = self.managed_frame_insets(client);
        let width = outer_width.saturating_sub(left.saturating_add(right));
        let height = outer_height.saturating_sub(top.saturating_add(bottom));
        match target {
            WmSnapTarget::Maximize => {
                self.snapped_geometries.remove(&client.0);
                self.maximized_geometries.insert(client.0, restore_geometry);
            }
            WmSnapTarget::Left | WmSnapTarget::Right => {
                self.maximized_geometries.remove(&client.0);
                self.snapped_geometries.insert(
                    client.0,
                    WmSnapState {
                        target,
                        restore_geometry,
                    },
                );
            }
        }
        self.apply_managed_geometry(client, frame, x, y, width, height);
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
    }

    pub(super) fn toggle_maximized(&mut self, client: ResourceId, action: u32) {
        if self.non_resizable_managed_windows.contains(&client.0) {
            return;
        }
        let Some(frame) = self.managed_frames.get(&client.0).copied() else {
            return;
        };
        let currently = self.maximized_geometries.contains_key(&client.0);
        let enable = action == 1 || (action == 2 && !currently);
        if enable == currently {
            return;
        }
        self.x11_output_homes.remove(&client.0);
        if enable {
            let Some(frame_state) = self.windows.get(&frame.0) else {
                return;
            };
            let Some(client_state) = self.windows.get(&client.0) else {
                return;
            };
            let current_geometry = (
                frame_state.x,
                frame_state.y,
                client_state.width,
                client_state.height,
            );
            let restore_geometry = self
                .snapped_geometries
                .remove(&client.0)
                .map_or(current_geometry, |state| state.restore_geometry);
            self.maximized_geometries.insert(client.0, restore_geometry);
            let (work_x, work_y, work_width, work_height) = self.wm_window_output_rect(frame, false);
            let (left, right, top, bottom) = self.managed_frame_insets(client);
            let width = work_width.saturating_sub(left.saturating_add(right));
            let height = work_height.saturating_sub(top.saturating_add(bottom));
            self.apply_managed_geometry(
                client,
                frame,
                i16::try_from(work_x).unwrap_or(i16::MAX),
                i16::try_from(work_y).unwrap_or(i16::MAX),
                width,
                height,
            );
        } else if let Some((x, y, width, height)) = self.maximized_geometries.remove(&client.0) {
            self.apply_managed_geometry(client, frame, x, y, width, height);
        }
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
    }

    pub(super) fn apply_managed_geometry(
        &mut self,
        client: ResourceId,
        frame: ResourceId,
        x: i16,
        y: i16,
        width: u16,
        height: u16,
    ) {
        // Publish the WM state before ConfigureNotify. Toolkits such as SDL
        // read _NET_WM_STATE to distinguish a maximize from an ordinary resize.
        // Every maximize/restore/snap/drag path converges here; do not emit a
        // redundant PropertyNotify on each pointer-sized geometry update.
        self.publish_managed_maximized_state(client);
        let (x, y, width, height) = self.fit_managed_dialog(client, x, y, width, height);
        let _ = self.resize_surface(client, width, height);
        let (frame_left, frame_right, frame_top, frame_bottom) =
            self.managed_frame_insets(client);
        let frame_width = width
            .saturating_add(frame_left)
            .saturating_add(frame_right);
        let frame_height = height
            .saturating_add(frame_top)
            .saturating_add(frame_bottom);
        let _ = self.resize_surface(frame, frame_width, frame_height);
        if let Some(window) = self.windows.get_mut(&client.0) {
            window.x = i16::try_from(frame_left).unwrap_or_default();
            window.y = i16::try_from(frame_top).unwrap_or_default();
            window.width = width;
            window.height = height;
        }
        if let Some(window) = self.windows.get_mut(&frame.0) {
            window.x = x;
            window.y = y;
            window.width = frame_width;
            window.height = frame_height;
        }
        let updated = self.windows.get(&client.0).cloned();
        if let Some(updated) = updated.as_ref() {
            // Resizing the reparented client itself produces an ordinary
            // parent-relative ConfigureNotify on a real X server. GTK2 uses
            // that non-synthetic event to run its top-level size allocation;
            // a synthetic root-coordinate event alone moves the frame but can
            // leave its widget tree at the old size until some later input.
            self.emit_managed_configure_notify(updated, geometry(updated), false);
            // The WM changed both the frame position and client size without
            // a client ConfigureWindow request. Also report the resulting
            // root-relative position as the ICCCM synthetic event applications
            // use to track the outer frame.
            let root_geometry = Geometry {
                root: ROOT_WINDOW,
                x: x.saturating_add(i16::try_from(frame_left).unwrap_or_default()),
                y: y.saturating_add(i16::try_from(frame_top).unwrap_or_default()),
                width: updated.width,
                height: updated.height,
                border_width: 0,
                depth: updated.depth,
            };
            self.emit_managed_configure_notify(updated, root_geometry, true);
            self.emit_full_exposure(updated);
        }
        self.paint_wm_chrome(frame);
        self.scene_upsert(frame);
        self.scene_upsert(client);
        self.scene_damage(frame);
        self.scene_damage(client);
        self.rebuild_scene();
    }

    pub(super) fn publish_managed_maximized_state(&mut self, client: ResourceId) {
        let property = self.atoms.intern("_NET_WM_STATE", false);
        let atom_type = self.atoms.intern("ATOM", false);
        let vertical = self.atoms.intern("_NET_WM_STATE_MAXIMIZED_VERT", false);
        let horizontal = self.atoms.intern("_NET_WM_STATE_MAXIMIZED_HORZ", false);
        let maximized = self.maximized_geometries.contains_key(&client.0);
        let existing = self
            .windows
            .get(&client.0)
            .and_then(|window| window.properties.get(&property));
        if existing.is_none() && !maximized {
            return;
        }
        let mut states: Vec<u32> = existing
            .filter(|value| value.r#type == atom_type && value.format == 32)
            .map(|value| value.data.chunks_exact(4).map(read_u32_le).collect())
            .unwrap_or_default();
        // Leave unrelated state hints intact, including modal/attention hints.
        states.retain(|atom| *atom != vertical.0 && *atom != horizontal.0);
        if maximized {
            states.extend([vertical.0, horizontal.0]);
        }
        let data = states.into_iter().flat_map(u32::to_le_bytes).collect();
        self.set_wm_property_if_changed(client, "_NET_WM_STATE", "ATOM", 32, data);
    }
}
