//! Shell updates operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn repaint_wm_chrome(&mut self) {
        if self.wm_dock_style { self.refresh_taskbar_geometry(); }
        self.update_wm_quick_launch_tooltip();
        let mut windows = vec![
            WM_TASKBAR,
            WM_GNOME_BACKGROUND,
            WM_GNOME_PANEL,
            WM_GNOME_DIALOG,
            WM_GNOME_MENU,
            WM_MENU,
            WM_TASK_PREVIEW,
            WM_TOOLTIP,
            WM_SNAP_PREVIEW,
            WM_NOTIFICATION,
        ];
        windows.extend(self.frame_clients.keys().copied().map(ResourceId));
        for window in windows {
            self.paint_wm_chrome(window);
            self.scene_upsert(window);
        }
    }

    pub(super) fn repaint_wm_client_identity(&mut self, client: ResourceId) {
        if let Some(frame) = self.managed_frames.get(&client.0).copied()
            && self.map_state(frame) == 2
        {
            self.paint_wm_chrome(frame);
            self.scene_upsert(frame);
        }
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        if self.map_state(WM_TASK_PREVIEW) == 2 {
            self.paint_wm_chrome(WM_TASK_PREVIEW);
            self.scene_upsert(WM_TASK_PREVIEW);
        }
    }

    pub(super) fn wm_window_title(&mut self, window: ResourceId) -> Vec<u8> {
        let net_wm_name = self.atoms.intern("_NET_WM_NAME", false);
        let wm_name = self.atoms.intern("WM_NAME", false);
        self.windows
            .get(&window.0)
            .and_then(|window| {
                window
                    .properties
                    .get(&net_wm_name)
                    .or_else(|| window.properties.get(&wm_name))
            })
            .map(|property| property.data.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| b"APPLICATION".to_vec())
    }

    pub(super) fn wm_window_icon(&mut self, window: ResourceId) -> Option<Vec<u32>> {
        let net_wm_icon = self.atoms.intern("_NET_WM_ICON", false);
        let data = self
            .windows
            .get(&window.0)?
            .properties
            .get(&net_wm_icon)
            .filter(|property| property.format == 32)?
            .data
            .clone();
        let values = data
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        let mut offset = 0usize;
        let mut best: Option<(usize, usize, usize)> = None;
        while offset.saturating_add(2) <= values.len() {
            let width = usize::try_from(values[offset]).ok()?;
            let height = usize::try_from(values[offset + 1]).ok()?;
            let count = width.checked_mul(height)?;
            let pixels = offset.saturating_add(2);
            if width == 0
                || height == 0
                || width > 512
                || height > 512
                || pixels.saturating_add(count) > values.len()
            {
                break;
            }
            let score = width.abs_diff(16).saturating_add(height.abs_diff(16));
            if best.is_none_or(|(_, _, best_score)| score < best_score) {
                best = Some((pixels, width, score));
            }
            offset = pixels.saturating_add(count);
        }
        let (pixels, source_width, _) = best?;
        let source_count = if offset >= pixels {
            // The selected icon can precede later candidates, so recover its
            // height from the header directly rather than using `offset`.
            let header = pixels.saturating_sub(2);
            source_width.checked_mul(usize::try_from(values[header + 1]).ok()?)?
        } else {
            return None;
        };
        let source_height = source_count / source_width;
        let mut icon = vec![0u32; 16 * 16];
        for y in 0..16usize {
            let source_y = y.saturating_mul(source_height) / 16;
            for x in 0..16usize {
                let source_x = x.saturating_mul(source_width) / 16;
                icon[y * 16 + x] = values[pixels + source_y * source_width + source_x];
            }
        }
        Some(icon)
    }

    pub fn set_wall_clock(&mut self, hours: u8, minutes: u8) -> bool {
        if hours > 23 || minutes > 59 {
            return false;
        }
        let value = u16::from(hours) * 60 + u16::from(minutes);
        if value == self.wm_clock_minutes {
            return true;
        }
        self.wm_clock_minutes = value;
        if self.config.native_window_manager {
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
            self.paint_wm_chrome(WM_GNOME_PANEL);
            self.scene_upsert(WM_GNOME_PANEL);
        }
        true
    }

    pub fn show_wm_notification(&mut self, kind: u8, title: Vec<u8>, message: Vec<u8>) -> bool {
        let valid_text = |text: &[u8], maximum: usize, required: bool| {
            (!required || !text.is_empty())
                && text.len() <= maximum
                && text.iter().all(|byte| (0x20..=0x7e).contains(byte))
        };
        if !self.config.native_window_manager
            || kind > 3
            || !valid_text(&title, WM_NOTIFICATION_MAX_TITLE_BYTES, true)
            || !valid_text(&message, WM_NOTIFICATION_MAX_MESSAGE_BYTES, false)
        {
            return false;
        }
        self.wm_notification_kind = kind;
        self.wm_notification_title = title;
        self.wm_notification_message = message;
        let (x, y, width, height) = self.wm_notification_geometry();
        if let Some(window) = self.windows.get_mut(&WM_NOTIFICATION.0) {
            window.x = x;
            window.y = y;
            window.width = width;
            window.height = height;
            window.mapped = true;
        } else {
            return false;
        }
        self.paint_wm_chrome(WM_NOTIFICATION);
        self.restack(WM_NOTIFICATION, None, Some(0));
        self.rebuild_scene();
        true
    }

    pub fn clear_wm_notification(&mut self) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        let Some(window) = self.windows.get_mut(&WM_NOTIFICATION.0) else {
            return false;
        };
        window.mapped = false;
        self.wm_notification_title.clear();
        self.wm_notification_message.clear();
        self.rebuild_scene();
        true
    }

    /// Close the shell menu without forwarding the dismissing click to X11.
    ///
    /// Browser-native windows share the compositor with X11 but do not have
    /// an X11 window id, so their pointer presses use this explicit shell
    /// dismissal path before native input is dispatched.
    pub fn clear_dock_hover(&mut self) {
        self.set_wm_task_preview(None, self.pointer_x);
        if self.wm_dock_style && matches!(self.wm_menu_level, WmMenuLevel::TaskGroup(_)) {
            self.dismiss_wm_menu();
        }
        if self.wm_chrome_hover.take().is_some() {
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
        }
    }

    pub fn dismiss_wm_menu(&mut self) -> bool {
        self.set_wm_task_preview(None, self.pointer_x);
        if !self.config.native_window_manager {
            return false;
        }
        let Some(menu) = self.windows.get_mut(&WM_MENU.0) else {
            return false;
        };
        if !menu.mapped {
            return true;
        }
        menu.mapped = false;
        self.wm_menu_hover_row = None;
        self.set_wm_task_preview(None, 0);
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_upsert(WM_MENU);
        self.rebuild_scene();
        self.scene_damage(WM_MENU);
        true
    }
}
