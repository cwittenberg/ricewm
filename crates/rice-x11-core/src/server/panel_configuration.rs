//! Panel configuration operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn relayout_wm_edge_windows(&mut self) {
        let maximized = self
            .managed_frames
            .iter()
            .filter(|(client, _)| self.maximized_geometries.contains_key(client))
            .map(|(client, frame)| (ResourceId(*client), *frame))
            .collect::<Vec<_>>();
        for (client, frame) in maximized {
            if !self.wm_outputs.is_empty() && self.restore_geometries.contains_key(&client.0) { continue; }
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
        }
        self.relayout_wm_fullscreen_outputs();
        let (work_x, work_y, work_width, work_height) = self.wm_workarea_rect();
        let snapped = self
            .snapped_geometries
            .iter()
            .filter_map(|(&client, state)| {
                self.managed_frames
                    .get(&client)
                    .copied()
                    .map(|frame| (ResourceId(client), frame, state.target))
            })
            .collect::<Vec<_>>();
        for (client, frame, target) in snapped {
            let (x, y, outer_width, outer_height) = self.wm_snap_geometry(target, self.wm_window_output_rect(frame, false));
            let (left, right, top, bottom) = self.managed_frame_insets(client);
            self.apply_managed_geometry(
                client,
                frame,
                x,
                y,
                outer_width.saturating_sub(left.saturating_add(right)),
                outer_height.saturating_sub(top.saturating_add(bottom)),
            );
        }

        // A panel or taskbar can appear, move, or change height after an
        // application has already mapped. Keep ordinary managed frames in the
        // resulting work area as well as relaying out maximized/snapped ones.
        // Preserve each client's size whenever it fits. If a newly docked
        // edge makes that impossible, shrink the frame just enough to keep
        // its content and chrome out of the reserved taskbar rectangle.
        let ordinary = self
            .managed_frames
            .iter()
            .filter(|(client, _)| {
                (self.wm_outputs.is_empty() || !self.restore_geometries.contains_key(client))
                    && !self.maximized_geometries.contains_key(client)
                    && !self.snapped_geometries.contains_key(client)
            })
            .filter_map(|(&client, &frame)| {
                let frame_state = self.windows.get(&frame.0)?;
                let client_state = self.windows.get(&client)?;
                Some((
                    ResourceId(client),
                    frame,
                    frame_state.x,
                    frame_state.y,
                    client_state.width,
                    client_state.height,
                ))
            })
            .collect::<Vec<_>>();
        let work_right = i32::from(work_x).saturating_add(i32::from(work_width));
        let work_bottom = i32::from(work_y).saturating_add(i32::from(work_height));
        for (client, frame, old_x, old_y, width, height) in ordinary {
            let (left, right, top, bottom) = self.managed_frame_insets(client);
            let next_width = width.min(work_width.saturating_sub(left.saturating_add(right)));
            let next_height = height.min(work_height.saturating_sub(top.saturating_add(bottom)));
            let next_frame_width = next_width.saturating_add(left).saturating_add(right);
            let next_frame_height = next_height
                .saturating_add(top)
                .saturating_add(bottom);
            let max_x = work_right
                .saturating_sub(i32::from(next_frame_width))
                .max(i32::from(work_x));
            let max_y = work_bottom
                .saturating_sub(i32::from(next_frame_height))
                .max(i32::from(work_y));
            let x = clamp_i16(i32::from(old_x).clamp(i32::from(work_x), max_x));
            let y = clamp_i16(i32::from(old_y).clamp(i32::from(work_y), max_y));
            if x != old_x || y != old_y || next_width != width || next_height != height {
                self.apply_managed_geometry(client, frame, x, y, next_width, next_height);
            }
        }
    }

    /// Enable or disable cross-toolkit task previews. Disabling immediately
    /// dismisses an open preview without changing task activation state.
    pub fn set_task_previews_enabled(&mut self, enabled: bool) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        self.wm_task_previews_enabled = enabled;
        if !enabled {
            self.set_wm_task_preview(None, self.pointer_x);
        }
        true
    }

    /// Atomically replace the ordered tray plugin instances.
    pub fn replace_tray_items(&mut self, items: Vec<TrayItem>) -> bool {
        let mut plugins = BTreeSet::new();
        if !self.config.native_window_manager
            || items.len() > MAX_TRAY_ITEMS
            || items.iter().any(|item| !plugins.insert(item.plugin_key()))
        {
            return false;
        }
        self.wm_tray_items = items;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    /// Replace bounded workspace labels; dynamic entries use numeric fallbacks.
    pub fn replace_workspace_names(&mut self, names: Vec<Vec<u8>>) -> bool {
        if !self.config.native_window_manager
            || (names.len()!=4&&(!self.scrolling_workspaces||names.is_empty()||names.len()>128))
            || names.iter().any(|name| {
                name.is_empty()
                    || name.len() > 64
                    || core::str::from_utf8(name).map_or(true, |text| {
                        text.chars().count() > 16 || text.chars().any(char::is_control)
                    })
            })
        {
            return false;
        }
        self.wm_workspace_names = names;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    /// Update the host-selected external connectivity strategy shown in the tray.
    pub fn set_connectivity_mode(&mut self, mode: u8) -> bool {
        if !self.config.native_window_manager || mode > 3 {
            return false;
        }
        if self.wm_connectivity_mode == mode {
            return true;
        }
        self.wm_connectivity_mode = mode;
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        true
    }

    /// Configure the desktop-only status/power cluster. Bit zero controls
    /// visibility; bits 1..=5 are Ethernet, Wi-Fi, Bluetooth, audio and mute;
    /// bit 6 selects the taskbar instead of the top bar.
    pub fn set_system_controls(&mut self, state: u8) -> bool {
        if !self.config.native_window_manager || state & !0x7f != 0 { return false; }
        if self.wm_system_controls == state { return true; }
        self.wm_system_controls = state;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    /// Publish the aggregate laptop battery percentage. `None` removes the
    /// battery slot completely on desktops and systems without a battery.
    pub fn set_system_battery(&mut self, percentage: Option<u8>) -> bool {
        if !self.config.native_window_manager || percentage.is_some_and(|value| value > 100) {
            return false;
        }
        if self.wm_battery_percentage == percentage {
            return true;
        }
        self.wm_battery_percentage = percentage;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    /// Keep the Rice Box shortcut in the retained panel surface. `None`
    /// disables it for targets which do not opt in; native desktop passes a
    /// bounded unread count and therefore cannot lose the shortcut on panel
    /// damage or a clock repaint.
    pub fn set_mail_indicator(&mut self, unread: Option<u16>) -> bool {
        if !self.config.native_window_manager || unread.is_some_and(|value| value > 999) {
            return false;
        }
        if self.wm_mail_unread == unread {
            return true;
        }
        self.wm_mail_unread = unread;
        self.repaint_wm_chrome();
        true
    }

    /// Replace the shortcuts immediately beside the RiceWM button.
    pub fn replace_quick_launchers(
        &mut self,
        visible: bool,
        launchers: Vec<WmQuickLauncher>,
    ) -> bool {
        if !self.config.native_window_manager
            || launchers.len() > 8
            || launchers.iter().any(|launcher| {
                launcher.action == 0
                    || launcher.title.is_empty()
                    || launcher.title.len() > 64
                    || core::str::from_utf8(&launcher.title)
                        .map_or(true, |title| title.chars().any(char::is_control))
            })
        {
            return false;
        }
        self.wm_quick_launch_visible = visible;
        self.wm_quick_launchers = launchers;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        true
    }

    /// Append one bounded CPU/network sample to the taskbar graph.
    pub fn set_performance_sample(&mut self, cpu: u8, network: u8) -> bool {
        if cpu > 100 || network > 100 || !self.config.native_window_manager {
            return false;
        }
        if self.wm_performance_cpu.len() >= 32 {
            self.wm_performance_cpu.pop_front();
        }
        if self.wm_performance_network.len() >= 32 {
            self.wm_performance_network.pop_front();
        }
        self.wm_performance_cpu.push_back(cpu);
        self.wm_performance_network.push_back(network);
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        true
    }
}
