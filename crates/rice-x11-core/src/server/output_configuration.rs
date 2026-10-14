//! Output configuration operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Resize the browser-owned output and publish the corresponding root,
    /// WM-chrome, and RANDR changes to connected X11 clients.
    pub fn set_theme_menu_row_height(&mut self, height: usize) -> bool {
        if height != 0 && !(16..=72).contains(&height) {return false;}
        self.wm_theme_menu_row_height=height;
        self.wm_menu_offset=0;self.wm_menu_hover_row=None;
        self.relayout_wm_menu();self.rebuild_scene();
        true
    }

    pub fn resize_output(&mut self, width: u16, height: u16) -> bool {
        if width == 0 || height == 0 {
            return false;
        }
        self.resize_display(width, height).is_ok()
    }

    /// Configure the browser-owned RiceWM taskbar without restarting X11
    /// clients. Edges use the stable wire order Bottom, Top, Left, Right.
    pub fn set_dock_appearance(&mut self, launcher: bool, color: Option<u32>) -> bool {
        if !self.config.native_window_manager {return false;}
        self.wm_launcher_visible = launcher;
        self.wm_dock_icon_color = color;
        self.refresh_taskbar_geometry();
        self.repaint_wm_chrome();
        self.rebuild_scene();
        true
    }

    pub fn set_embedded_shell(&mut self, enabled: bool) -> bool {
        if !self.config.native_window_manager { return false; }
        self.wm_embedded_shell = enabled;
        self.refresh_taskbar_geometry();
        self.repaint_wm_chrome();
        self.relayout_wm_edge_windows();
        self.publish_wm_root_properties();
        self.rebuild_scene();
        true
    }

    pub fn set_dock_style(&mut self, enabled: bool) -> bool {
        if !self.config.native_window_manager { return false; }
        self.wm_dock_style = enabled;
        self.refresh_taskbar_geometry();
        self.relayout_wm_edge_windows();
        self.publish_wm_root_properties();
        self.rebuild_scene();
        true
    }

    pub fn set_dock_corner_radius(&mut self, radius: usize) -> bool {
        if !self.config.native_window_manager || radius > 32 { return false; }
        self.wm_dock_corner_radius = radius;
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        true
    }

    pub fn set_dock_height(&mut self, height: usize) -> bool {
        if !self.config.native_window_manager || !(32..=80).contains(&height) { return false; }
        if self.wm_dock_height == Some(height) { return true; }
        self.wm_dock_height = Some(height);
        self.refresh_taskbar_geometry();
        self.repaint_wm_chrome();
        self.relayout_wm_edge_windows();
        self.publish_wm_root_properties();
        self.rebuild_scene();
        true
    }

    pub(super) fn wm_dock_bar_height(&self) -> usize {
        self.wm_dock_height.unwrap_or_else(|| {
            if self.wm_themes.get(usize::from(self.wm_theme)).is_some_and(|theme|
                theme.chrome_profile == WmChromeProfile::Orchard) { 64 } else { WM_COMPACT_DOCK_BAR_THICKNESS }
        })
    }

    pub fn set_dock_magnification(&mut self, enabled: bool, zoom: u8, spread: u8) -> bool {
        if !self.config.native_window_manager || zoom > 100 || spread > 100 {
            return false;
        }
        self.wm_dock_magnification = enabled;
        self.wm_dock_magnification_zoom = zoom;
        self.wm_dock_magnification_spread = spread;
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        true
    }

    pub fn set_dock_identity(&mut self, kind: u32, id: u32, key: u32) -> bool {
        match kind {
            0 if id <= u32::from(u16::MAX) => { if id == 0 {self.wm_dock_task_keys.clear();} else {self.wm_dock_task_keys.insert(id as u16,key);} }
            1 => { if id == 0 {self.wm_dock_launcher_keys.clear();} else {self.wm_dock_launcher_keys.insert(id,key);} }
            _ => return false,
        }
        true
    }

    pub(super) fn wm_dock_launcher_task(&self, index: u8) -> Option<WmTaskTarget> {
        let launcher = self.wm_quick_launchers.get(usize::from(index))?;
        let key = self.wm_dock_launcher_keys.get(&launcher.action).filter(|key| **key != 0)?;
        self.wm_task_groups().iter().enumerate().find_map(|(group,members)| {
            members.iter().any(|target| matches!(target, WmTaskTarget::Native(id) if self.wm_dock_task_keys.get(id) == Some(key))).then(||
                if members.len()>1 {WmTaskTarget::Group(group as u16)} else {members[0]})
        })
    }

    pub(super) fn wm_dock_targets(&self) -> Vec<WmChromeTarget> {
        if !self.wm_gnome_dash_visible { return Vec::new(); }
        let mut targets = if self.wm_launcher_visible {vec![WmChromeTarget::Start]} else {Vec::new()};
        targets.push(WmChromeTarget::Activities);
        if self.wm_quick_launch_visible {
            targets.extend((0..self.wm_quick_launchers.len()).map(|i| WmChromeTarget::QuickLaunch(i as u8)),
            );
        }
        targets.extend(self.wm_task_targets().into_iter().filter(|target| {
                    !(0..self.wm_quick_launchers.len()).any(|index| {
                        self.wm_quick_launch_visible && self.wm_dock_launcher_task(index as u8) == Some(*target)})
                }).map(WmChromeTarget::Task),
        );
        targets
    }

    pub(super) fn wm_dock_step(&self) -> usize {
        let available = if self.wm_taskbar_edge.vertical() { self.config.height_px.saturating_sub(self.wm_reserved_gnome_panel_height()) } else { self.config.width_px };
        let step = if self.wm_themes.get(usize::from(self.wm_theme)).is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Orchard) { 52 } else { WM_COMPACT_DOCK_STEP };
        (usize::from(available.saturating_sub(40)) / self.wm_dock_targets().len().max(1))
            .clamp(1, step)
    }

    pub fn set_taskbar_configuration(&mut self, visible: bool, edge: u8, auto_hide: bool) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        let Some(edge) = WmTaskbarEdge::from_wire(edge) else {
            return false;
        };
        self.wm_taskbar_visible = visible;
        self.wm_taskbar_edge = edge;
        self.wm_taskbar_drag = None;
        self.wm_taskbar_auto_hide = auto_hide;
        self.wm_taskbar_revealed = visible && !auto_hide;
        if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
            menu.mapped = false;
        }
        self.wm_menu_hover_row = None;
        self.refresh_taskbar_geometry();
        self.relayout_wm_menu();
        self.publish_wm_root_properties();
        self.relayout_wm_edge_windows();
        self.rebuild_scene();
        true
    }

    /// Configure the root RiceWM menu without changing its application data.
    /// Applications-only mode opens the existing application cascade
    /// immediately and keeps the launcher row as its sole root entry.
    pub fn set_menu_layout(
        &mut self,
        applications_only: bool,
        show_themes: bool,
        show_workspaces: bool,
    ) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        self.wm_menu_applications_only = applications_only;
        self.wm_menu_show_themes = show_themes;
        self.wm_menu_show_workspaces = show_workspaces;
        self.wm_menu_level = self.wm_menu_initial_level();
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        self.wm_menu_window_anchor = None;
        self.relayout_wm_menu();
        self.rebuild_scene();
        true
    }

    /// Select the hosted-session wording without changing the exit Command.
    pub fn set_session_logout(&mut self, enabled: bool) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        self.wm_session_logout = enabled;
        self.relayout_wm_menu();
        self.rebuild_scene();
        true
    }

    /// Set the exact menu icon edge. Larger icons also increase row height so
    /// they cannot overlap adjacent entries.
    pub fn set_menu_icon_size(&mut self, size: u8) -> bool {
        if !self.config.native_window_manager || !matches!(size, 16 | 24 | 32 | 64) {
            return false;
        }
        self.wm_menu_icon_size = size;
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        self.relayout_wm_menu();
        self.rebuild_scene();
        true
    }

    /// Configure the main RiceWM launcher button. Appearance is icon-only (0),
    /// icon-and-text (1), or text-only (2).
    pub fn set_launcher_appearance(&mut self, appearance: u8, text: Vec<u8>) -> bool {
        let Ok(label) = core::str::from_utf8(&text) else {
            return false;
        };
        if appearance > 2
            || label.trim().is_empty()
            || label.chars().count() > 24
            || label.chars().any(char::is_control)
        {
            return false;
        }
        self.wm_launcher_appearance = appearance;
        self.wm_launcher_text = text;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        self.rebuild_scene();
        true
    }

    /// Apply GNOME overview visibility to RiceWM's canonical taskbar chrome.
    /// This is a compositor bridge: it hides the existing launcher/task area
    /// and workspace selector instead of drawing a second GNOME-like dock.
    pub fn set_gnome_overview_elements(
        &mut self,
        dash_visible: bool,
        workspaces_visible: bool,
    ) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        self.wm_gnome_dash_visible = dash_visible;
        self.wm_gnome_workspaces_visible = workspaces_visible;
        if !dash_visible {
            if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                menu.mapped = false;
            }
            self.wm_menu_hover_row = None;
        }
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.repaint_wm_chrome();
        self.rebuild_scene();
        true
    }
}
