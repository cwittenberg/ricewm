//! Overview operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Reserve the next ordinary top-level X11 window for an invisible warm launch.
    ///
    /// The client remains mapped from X11's perspective, allowing a toolkit to
    /// finish allocation and painting. RiceWM withholds its managed subtree from
    /// composition, input, focus, and the taskbar until explicit activation.
    /// Only one reservation or parked application is admitted at a time so an
    /// optimizer cannot accumulate unbounded background processes.
    pub fn request_park_next_managed_window(&mut self) -> bool {
        if !self.config.native_window_manager
            || self.park_next_managed_window
            || !self.parked_managed_windows.is_empty()
        {
            return false;
        }
        self.park_next_managed_window = true;
        true
    }

    /// Cancel an unused warm-launch reservation.
    pub fn cancel_park_next_managed_window(&mut self) -> bool {
        std::mem::take(&mut self.park_next_managed_window)
    }

    /// Apply toolkit-provided resize capability metadata to a managed client.
    /// Fixed-size clients retain ordinary focus/minimize/close behavior, but
    /// RiceWM must not advertise or execute maximize/snap geometry which the
    /// client cannot lay out.
    pub fn set_managed_window_resizable(&mut self, client: u32, resizable: bool) -> bool {
        if !self.managed_frames.contains_key(&client) {
            return false;
        }
        if resizable {
            self.non_resizable_managed_windows.remove(&client);
        } else {
            self.non_resizable_managed_windows.insert(client);
        }
        if let Some(frame) = self.managed_frames.get(&client).copied() {
            self.scene_upsert(frame);
        }
        true
    }

    /// Return the sole parked client window, if a warm launch has reached X11.
    #[must_use]
    pub fn parked_managed_window(&self) -> Option<u32> {
        self.parked_managed_windows.iter().next().copied()
    }

    /// Return the X11 connection handle that owns the parked client window.
    #[must_use]
    pub fn parked_managed_window_owner(&self) -> Option<u32> {
        let window = self.parked_managed_window()?;
        self.windows.get(&window)?.owner
    }

    /// Atomically publish and focus a previously parked managed application.
    pub fn activate_parked_managed_window(&mut self, client: u32) -> bool {
        if !self.parked_managed_windows.remove(&client) {
            return false;
        }
        let client = ResourceId(client);
        self.activate_managed_window(client);
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        self.update_wm_client_properties();
        true
    }

    /// Searchable application windows across workspaces. Each record contains
    /// frame, workspace, minimized, title byte length (little-endian u32), then
    /// UTF-8 title bytes. Owned dialogs and unpublished preloads are excluded.
    pub fn search_window_snapshot(&mut self) -> Vec<u8> {
        let clients: Vec<_> = self.managed_frames.keys().copied().filter(|client| {
            !self.parked_managed_windows.contains(client)
                && self.managed_transient_owner_relation(ResourceId(*client)).is_none()
        }).collect();
        let mut bytes = Vec::new();
        for client in clients {
            let Some(workspace) = self.window_workspaces.get(&client).copied() else { continue; };
            let title = self.wm_window_title(ResourceId(client));
            let title = String::from_utf8_lossy(&title).chars().take(512).collect::<String>();
            for word in [self.managed_frames[&client].0, workspace,
                u32::from(self.minimized_windows.contains(&client)), title.len() as u32,
            ] {
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.extend_from_slice(title.as_bytes());
        }
        bytes
    }

    /// Trusted client/frame relationships for spatial composition. No titles or
    /// application-supplied process identifiers participate in ownership.
    /// Version/count header followed by six little-endian words per record:
    /// client, frame, connection, transient root client, workspace, flags.
    pub fn spatial_window_snapshot(&self) -> Option<Vec<u8>> {
        let mut records = Vec::new();
        for window in self.windows.values() {
            let Some(owner) = window.owner else { continue; };
            if !self.clients.contains_key(&owner) || window.class != 1 { continue; }
            let frame = self.managed_frames.get(&window.id.0).copied();
            if frame.is_none() && !(window.parent == ROOT_WINDOW && window.override_redirect) { continue; }
            if records.len() >= 1024 { return None; }
            let mut root = window.id;
            let mut seen = BTreeSet::new();
            let mut valid = true;
            for _ in 0..64 {
                if !seen.insert(root.0) { valid = false; break; }
                let Some(parent) = self.managed_transient_owner_relation(root) else { break; };
                if self.windows.get(&parent.0).and_then(|w| w.owner) != Some(owner) { valid = false; break; }
                root = parent;
            }
            if self.managed_transient_owner_relation(root).is_some() { valid = false; }
            let root_client = if valid && frame.is_some() { root.0 } else { 0 };
            let flags = u32::from(window.override_redirect)
                | (u32::from(self.map_state(window.id) == 2 && !self.parked_managed_windows.contains(&window.id.0)) << 1)
                | (u32::from(self.minimized_windows.contains(&window.id.0)) << 2)
                | (u32::from(self.parked_managed_windows.contains(&window.id.0)) << 3);
            records.push([window.id.0, frame.unwrap_or(window.id).0, owner, root_client,
                self.window_workspaces.get(&window.id.0).copied().unwrap_or(self.current_workspace), flags]);
        }
        let mut bytes = Vec::with_capacity(8 + records.len() * 24);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for record in records { for word in record { bytes.extend_from_slice(&word.to_le_bytes()); } }
        Some(bytes)
    }

    /// Enumerate task-list X11 frames on this workspace, including minimized
    /// windows. Owned dialogs and preloaded, unpublished windows stay excluded.
    pub fn switcher_window(&self, index: usize) -> Option<u32> {
        self.wm_raw_task_targets().into_iter().filter_map(|target| match target {
                WmTaskTarget::X11(client) => {
                    self.managed_frames.get(&client.0).map(|frame| frame.0)}
                _ => None,
            }).nth(index)
    }

    /// Resolve a visible overview task frame to its application client without
    /// activating it or changing input ownership.
    pub fn overview_client(&self, window: u32) -> Option<u32> {
        let client=self.frame_clients.get(&window).copied()?;
        if self.parked_managed_windows.contains(&client.0)||self.minimized_windows.contains(&client.0)
            ||self.managed_transient_owner_relation(client).is_some()||!self.x11_workspace_visible(client.0){return None;}
        Some(client.0)
    }

    /// Raise a current-workspace X11 window selected from the browser-owned
    /// Activities overview. The overview identifies the retained top-level
    /// frame, while focus remains attached to its managed client.
    pub fn activate_overview_window(&mut self, window: u32) -> bool {
        let frame = ResourceId(window);
        let Some(client) = self
            .frame_clients
            .get(&frame.0)
            .copied()
            .or_else(|| self.managed_frames.contains_key(&frame.0).then_some(frame))
        else {
            return false;
        };
        if self.window_workspaces.get(&client.0).copied() != Some(self.current_workspace) {
            return false;
        }
        self.activate_managed_window(client);
        true
    }

    /// Move an Activities card without activating it or switching workspaces.
    pub fn move_overview_window(&mut self, window: u32, workspace: u32, x: i32, y: i32) -> bool {
        if workspace >= self.workspace_count { return false; }
        let Some(client) = self.frame_clients.get(&window).copied() else { return false; };
        if self.parked_managed_windows.contains(&client.0)
            || self.managed_transient_owner_relation(client).is_some() { return false; }
        let Some(frame) = self.managed_frames.get(&client.0).copied() else { return false; };
        let Some(current) = self.windows.get(&client.0) else { return false; };
        let (width, height) = (current.width, current.height);
        let Ok(x) = i16::try_from(x) else { return false; };
        let Ok(y) = i16::try_from(y) else { return false; };
        // Resolve the destination output from the requested frame origin, then
        // fit large windows to its usable bounds before notifying the client.
        if let Some(window) = self.windows.get_mut(&frame.0) { window.x = x; window.y = y; }
        let (ax, ay, aw, ah) = self.wm_window_output_rect(frame, false);
        let (left, right, top, bottom) = self.managed_frame_insets(client);
        let width = width.min(aw.saturating_sub(left + right).max(1));
        let height = height.min(ah.saturating_sub(top + bottom).max(1));
        let x = i32::from(x).clamp(i32::from(ax), i32::from(ax) + i32::from(aw.saturating_sub(width + left + right))) as i16;
        let y = i32::from(y).clamp(i32::from(ay), i32::from(ay) + i32::from(ah.saturating_sub(height + top + bottom))) as i16;
        self.maximized_geometries.remove(&client.0);
        self.snapped_geometries.remove(&client.0);
        self.restore_geometries.remove(&client.0);
        self.x11_output_homes.remove(&client.0);
        self.overview_placed_windows.insert(client.0);
        self.auto_tile_restore.insert(client.0, (x, y, width, height));
        self.apply_managed_geometry(client, frame, x, y, width, height);
        self.move_managed_window_to_workspace(client, workspace);
        true
    }

    /// Request a standards-compliant close for a current-workspace X11 window
    /// chosen from the browser-owned Activities overview.
    pub fn close_overview_window(&mut self, window: u32) -> bool {
        let frame = ResourceId(window);
        let Some(client) = self
            .frame_clients
            .get(&frame.0)
            .copied()
            .or_else(|| self.managed_frames.contains_key(&frame.0).then_some(frame))
        else {
            return false;
        };
        if self.window_workspaces.get(&client.0).copied() != Some(self.current_workspace) {
            return false;
        }
        self.request_close_window(client);
        true
    }

    /// Close a parked application without ever publishing its retained scene.
    pub fn discard_parked_managed_window(&mut self, client: u32) -> bool {
        if !self.parked_managed_windows.remove(&client) {
            return false;
        }
        self.request_close_window(ResourceId(client));
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        self.update_wm_client_properties();
        true
    }

    /// Re-announce the visible contents of client-owned windows.
    ///
    /// Browser transport delivers a client's `MapWindow` batch and the
    /// resulting MapNotify/Expose events in separate host turns.  A delayed
    /// refresh models the exposure a native server generates once the mapped
    /// hierarchy is actually viewable, and prevents toolkits from discarding
    /// their only paint while they are still processing the map transition.
    pub fn refresh_viewable_client_windows(&mut self) -> usize {
        let windows = self
            .windows
            .values()
            .filter(|window| {
                window.owner.is_some() && window.class == 1 && self.map_state(window.id) == 2
            })
            .cloned()
            .collect::<Vec<_>>();
        for window in &windows {
            self.emit_full_exposure(window);
        }
        windows.len()
    }
}
