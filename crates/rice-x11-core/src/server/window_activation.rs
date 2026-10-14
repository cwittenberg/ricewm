//! Window activation operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn active_wm_task_target(&self) -> Option<WmTaskTarget> {
        if let Some(task) = self
            .native_tasks
            .iter()
            .find(|task| task.workspace == self.current_workspace && task.active && !task.minimized)
        {
            return Some(WmTaskTarget::Native(task.id));
        }
        let client = self.managed_client_ancestor(self.focus_window)?;
        (self.window_workspaces.get(&client.0).copied() == Some(self.current_workspace)
            && !self.minimized_windows.contains(&client.0))
        .then_some(WmTaskTarget::X11(client))
    }

    pub(super) fn snap_active_window(&mut self, target: WmSnapTarget) {
        match self.active_wm_task_target() {
            Some(WmTaskTarget::X11(client)) => {
                let Some(frame) = self.managed_frames.get(&client.0).copied() else {
                    return;
                };
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
                    .get(&client.0)
                    .map(|state| state.restore_geometry)
                    .or_else(|| self.maximized_geometries.get(&client.0).copied())
                    .unwrap_or(current_geometry);
                self.apply_wm_snap(client, target, restore_geometry, self.wm_window_output_rect(frame, false));
                self.activate_managed_window(client);
            }
            Some(WmTaskTarget::Native(id)) => {
                let prefix = match target {
                    WmSnapTarget::Left => WM_ACTION_TASK_SNAP_LEFT_PREFIX,
                    WmSnapTarget::Right => WM_ACTION_TASK_SNAP_RIGHT_PREFIX,
                    WmSnapTarget::Maximize => WM_ACTION_TASK_MAXIMIZE_PREFIX,
                };
                self.wm_actions.push_back(prefix | u32::from(id));
            }
            Some(WmTaskTarget::Group(_)) | None => {}
        }
    }

    pub(super) fn move_active_window_to_adjacent_workspace(&mut self, next: bool) {
        let Some(target) = self.active_wm_task_target() else {
            return;
        };
        match target {
            WmTaskTarget::X11(client) => {
                let current = self
                    .window_workspaces
                    .get(&client.0)
                    .copied()
                    .unwrap_or(self.current_workspace);
                let workspace=self.adjacent_workspace(current,next);
                self.move_managed_window_to_workspace(client, workspace);
            }
            WmTaskTarget::Native(id) => {
                let prefix = if next {
                    WM_ACTION_TASK_WORKSPACE_NEXT_PREFIX
                } else {
                    WM_ACTION_TASK_WORKSPACE_PREVIOUS_PREFIX
                };
                self.wm_actions.push_back(prefix | u32::from(id));
            }
            WmTaskTarget::Group(_) => {}
        }
    }

    pub(super) fn activate_managed_window(&mut self, client: ResourceId) {
        let workspace_visible=self.x11_workspace_visible(client.0);
        self.minimized_windows.remove(&client.0);
        if let Some(frame) = self.managed_frames.get(&client.0).copied() {
            if let Some(window) = self.windows.get_mut(&frame.0) {
                window.mapped = workspace_visible;
            }
            // A minimized frame makes every mapped client descendant
            // unviewable without changing the descendants' own map flags.
            // Restoring only the frame leaves stale `viewable = false`
            // records in the retained browser scene. Refresh the complete
            // subtree while preserving all already-uploaded surfaces.
            self.scene_upsert_subtree(frame);
            self.set_focus(client);
            self.notify_x11_foreground(client);
        }
    }

    /// Restore only this workspace; command providers may repeat this after a
    /// compositor action without emitting another action or changing neighbors.
    pub fn restore_workspace_windows(&mut self) {
        let workspace = self.current_workspace;
        self.minimized_windows.retain(|client| self.window_workspaces.get(client).copied() != Some(workspace));
        self.wm_show_desktop_windows.remove(&workspace);
        self.apply_workspace_visibility();
    }

    pub(super) fn restore_all_windows(&mut self) {
        self.restore_workspace_windows();
        self.wm_actions.push_back(WM_ACTION_RESTORE_WINDOWS);
    }

    /// Idempotent visibility state shared by direct commands and chrome actions.
    pub fn set_desktop_hidden(&mut self, hidden: bool) {
        let workspace = self.current_workspace;
        if hidden {
            if self.wm_show_desktop_windows.contains_key(&workspace) { return; }
            let visible: BTreeSet<_> = self.managed_frames.keys().copied().filter(|client| {
                self.window_workspaces.get(client).copied() == Some(workspace)
                    && !self.minimized_windows.contains(client)
                    && !self.parked_managed_windows.contains(client)
            }).collect();
            self.minimized_windows.extend(visible.iter().copied());
            // Empty workspaces still have a phase, including native-only ones.
            self.wm_show_desktop_windows.insert(workspace, visible);
        } else {
            let Some(saved) = self.wm_show_desktop_windows.remove(&workspace) else { return; };
            for client in saved {
                if self.window_workspaces.get(&client).copied() == Some(workspace) {
                    self.minimized_windows.remove(&client);
                }
            }
        }
        self.apply_workspace_visibility();
        self.set_focus(ROOT_WINDOW);
    }

    pub(super) fn toggle_show_desktop(&mut self) {
        self.set_desktop_hidden(!self.wm_show_desktop_windows.contains_key(&self.current_workspace),
        );
        self.wm_actions.push_back(WM_ACTION_SHOW_DESKTOP);
    }

    pub(super) fn notify_x11_foreground(&mut self, window: ResourceId) {
        if self.managed_client_ancestor(window).is_some()
            && self.wm_actions.back().copied() != Some(WM_ACTION_X11_FOREGROUND_PREFIX)
        {
            self.wm_actions.push_back(WM_ACTION_X11_FOREGROUND_PREFIX);
        }
    }
}
