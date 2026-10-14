//! Workareas operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) const fn wm_taskbar_mapped(&self) -> bool {
        self.wm_taskbar_visible && (!self.wm_taskbar_auto_hide || self.wm_taskbar_revealed)
    }

    pub(super) const fn wm_reserved_taskbar_height(&self) -> u16 {
        if self.wm_taskbar_visible && !self.wm_taskbar_auto_hide && !self.wm_taskbar_edge.vertical()
        {
            if self.wm_dock_style { 0 } else { WM_TASKBAR_HEIGHT }
        } else {
            0
        }
    }

    pub(super) const fn wm_reserved_taskbar_width(&self) -> u16 {
        if self.wm_taskbar_visible && !self.wm_taskbar_auto_hide && self.wm_taskbar_edge.vertical()
        {
            if self.wm_dock_style { 0 } else { WM_TASKBAR_HEIGHT }
        } else {
            0
        }
    }

    pub(super) const fn wm_reserved_gnome_panel_height(&self) -> u16 {
        if self.wm_gnome_panel {
            self.wm_gnome_panel_height
        } else {
            0
        }
    }

    /// Actual retained shell geometry, including a currently hidden auto-hide bar.
    /// Edge uses Windows ABE values; visibility is separate from ownership.
    pub fn taskbar_geometry(&self) -> Option<[i32; 8]> {
        if !self.config.native_window_manager { return None; }
        let window = self.windows.get(&WM_TASKBAR.0)?;
        let edge = match self.wm_taskbar_edge {
            WmTaskbarEdge::Left => 0, WmTaskbarEdge::Top => 1,
            WmTaskbarEdge::Right => 2, WmTaskbarEdge::Bottom => 3,
        };
        let left = i32::from(window.x); let top = i32::from(window.y);
        Some([edge, left, top, left + i32::from(window.width), top + i32::from(window.height),
            i32::from(Self::is_wm_shell_window(WM_TASKBAR)), i32::from(self.wm_taskbar_auto_hide),
            i32::from(self.map_state(WM_TASKBAR) == 2)])
    }

    pub(super) fn wm_taskbar_rect(&self) -> (i16, i16, u16, u16) {
        self.wm_taskbar_rect_for(self.wm_taskbar_edge)
    }

    pub(super) fn wm_primary_rect(&self) -> (u16, u16, u16, u16) {
        self.wm_outputs.get(self.wm_primary_output).copied().unwrap_or((0, 0, self.config.width_px, self.config.height_px))
    }

    pub(super) fn wm_shell_rect(&self) -> (u16, u16, u16, u16) {
        self.wm_outputs.get(self.wm_shell_output).copied().unwrap_or_else(|| self.wm_primary_rect())
    }

    pub(super) fn wm_panel_output_rect(&self) -> (u16, u16, u16, u16) {
        if self.wm_topbar_all_monitors { self.wm_shell_rect() } else { self.wm_primary_rect() }
    }

    pub(super) fn wm_dock_output_rect(&self) -> (u16, u16, u16, u16) {
        if self.wm_dock_all_monitors { self.wm_shell_rect() } else { self.wm_primary_rect() }
    }

    pub(super) fn wm_taskbar_rect_for(&self, edge: WmTaskbarEdge) -> (i16, i16, u16, u16) {
        let (x, y, width, height) = self.wm_taskbar_local_rect_for(edge);
        let (ox, oy, _, _) = self.wm_dock_output_rect();
        (x.saturating_add(ox as i16), y.saturating_add(oy as i16), width, height)
    }

    pub(super) fn wm_taskbar_local_rect_for(&self, edge: WmTaskbarEdge) -> (i16, i16, u16, u16) {
        if self.wm_dock_style {
            let vertical = edge.vertical();
            let panel = if self.wm_topbar_all_monitors || self.wm_dock_output_rect() == self.wm_primary_rect() { self.wm_reserved_gnome_panel_height() } else { 0 };
            let top = if self.wm_gnome_panel_bottom { 0 } else { panel };
            let available_height = self.wm_dock_output_rect().3.saturating_sub(panel);
            let axis = if vertical { available_height } else { self.wm_dock_output_rect().2 };
            let length = (self.wm_dock_targets().len().max(1) * self.wm_dock_step()
                + self.wm_notification_icons.iter().filter(|icon| !icon.hidden).count() * 28
                + WM_COMPACT_DOCK_AXIS_PADDING * 2)
                .min(usize::from(axis)) as u16;
            let thick = WM_COMPACT_DOCK_SURFACE_THICKNESS.max((self.wm_dock_bar_height() + 8) as u16)
                .min(if vertical { self.wm_dock_output_rect().2 } else { available_height });
            let centered = axis.saturating_sub(length) / 2;
            return match edge {
                WmTaskbarEdge::Bottom => (centered as i16, top.saturating_add(available_height.saturating_sub(thick)) as i16, length, thick,
                ),
                WmTaskbarEdge::Top => (centered as i16, top as i16, length, thick),
                WmTaskbarEdge::Left => (0, (top + centered) as i16, thick, length),
                WmTaskbarEdge::Right => (self.wm_dock_output_rect().2.saturating_sub(thick) as i16, (top + centered) as i16, thick, length,
                ),
            };
        }
        let thickness_x = WM_TASKBAR_HEIGHT.min(self.wm_dock_output_rect().2);
        let thickness_y = WM_TASKBAR_HEIGHT.min(self.wm_dock_output_rect().3);
        let panel = if self.wm_topbar_all_monitors || self.wm_dock_output_rect() == self.wm_primary_rect() { self.wm_reserved_gnome_panel_height() } else { 0 };
        let top = if self.wm_gnome_panel_bottom { 0 } else { panel };
        let available_height = self.wm_dock_output_rect().3.saturating_sub(panel);
        match edge {
            WmTaskbarEdge::Bottom => (
                0,
                i16::try_from(top.saturating_add(available_height.saturating_sub(thickness_y)))
                    .unwrap_or(i16::MAX),
                self.wm_dock_output_rect().2,
                thickness_y,
            ),
            WmTaskbarEdge::Top => (
                0,
                i16::try_from(top).unwrap_or(i16::MAX),
                self.wm_dock_output_rect().2,
                thickness_y,
            ),
            WmTaskbarEdge::Left => (
                0,
                i16::try_from(top).unwrap_or(i16::MAX),
                thickness_x,
                available_height,
            ),
            WmTaskbarEdge::Right => (
                i16::try_from(self.wm_dock_output_rect().2.saturating_sub(thickness_x)).unwrap_or(i16::MAX),
                i16::try_from(top).unwrap_or(i16::MAX),
                thickness_x,
                available_height,
            ),
        }
    }

    pub(super) fn wm_taskbar_y(&self) -> u16 {
        self.wm_taskbar_rect().1.cast_unsigned()
    }

    pub(super) fn wm_workarea_rect(&self) -> (u16, u16, u16, u16) {
        let taskbar_height = self.wm_reserved_taskbar_height();
        let taskbar_width = self.wm_reserved_taskbar_width();
        let panel = self.wm_reserved_gnome_panel_height();
        let y = if self.wm_gnome_panel_bottom { 0 } else { panel }.saturating_add(
            if self.wm_taskbar_edge == WmTaskbarEdge::Top {
                taskbar_height
            } else {
                0
            },
        );
        let x = if self.wm_taskbar_edge == WmTaskbarEdge::Left {
            taskbar_width
        } else {
            0
        };
        let inset = 0;
        (
            x.saturating_add(inset),
            y.saturating_add(inset),
            self.config.width_px.saturating_sub(taskbar_width).saturating_sub(inset * 2),
            self.config
                .height_px
                .saturating_sub(panel.saturating_add(taskbar_height)).saturating_sub(inset * 2),
        )
    }

    pub(super) fn wm_workarea(&self) -> (u16, u16) {
        let (_, y, _, height) = self.wm_workarea_rect();
        (y, height)
    }

    pub(super) fn wm_taskbar_edge_at(&self, root_x: i16, root_y: i16) -> WmTaskbarEdge {
        let x = i32::from(root_x).clamp(0, i32::from(self.config.width_px));
        let y = i32::from(root_y).clamp(0, i32::from(self.config.height_px));
        let distances = [
            (
                i32::from(self.config.height_px).saturating_sub(y),
                WmTaskbarEdge::Bottom,
            ),
            (y, WmTaskbarEdge::Top),
            (x, WmTaskbarEdge::Left),
            (
                i32::from(self.config.width_px).saturating_sub(x),
                WmTaskbarEdge::Right,
            ),
        ];
        distances
            .into_iter()
            .min_by_key(|(distance, _)| *distance)
            .map_or(self.wm_taskbar_edge, |(_, edge)| edge)
    }

    pub(super) fn wm_pointer_workarea(&self, x: i16, y: i16) -> Option<(u16,u16,u16,u16)> {
        if self.wm_outputs.is_empty() { return Some(self.wm_workarea_rect()); }
        let output = self.wm_outputs.iter().copied().find(|&(ox,oy,w,h)|
            i32::from(x) >= i32::from(ox) && i32::from(x) < i32::from(ox)+i32::from(w) &&
            i32::from(y) >= i32::from(oy) && i32::from(y) < i32::from(oy)+i32::from(h))?;
        Some(self.wm_output_workarea(output, false))
    }

    pub(super) fn wm_drag_workarea(&self) -> (u16,u16,u16,u16) {
        if self.wm_outputs.is_empty() { self.wm_workarea_rect() }
        else { (0,0,self.config.width_px,self.config.height_px) }
    }

    pub(super) fn wm_snap_target_at(&self, root_x: i16, root_y: i16) -> Option<WmSnapTarget> {
        let (work_x, work_y, work_width, _) = self.wm_pointer_workarea(root_x, root_y)?;
        let top_edge = i16::try_from(work_y)
            .unwrap_or(i16::MAX)
            .saturating_add(WM_SNAP_EDGE_THRESHOLD);
        if root_y <= top_edge {
            return Some(WmSnapTarget::Maximize);
        }
        let left_edge = i16::try_from(work_x)
            .unwrap_or(i16::MAX)
            .saturating_add(WM_SNAP_EDGE_THRESHOLD);
        if root_x <= left_edge {
            return Some(WmSnapTarget::Left);
        }
        let right_edge = i16::try_from(work_x.saturating_add(work_width).saturating_sub(1))
            .unwrap_or(i16::MAX)
            .saturating_sub(WM_SNAP_EDGE_THRESHOLD);
        (root_x >= right_edge).then_some(WmSnapTarget::Right)
    }

    pub(super) fn wm_snap_geometry(&self, target: WmSnapTarget, area: (u16,u16,u16,u16)) -> (i16, i16, u16, u16) {
        let (work_x, work_y, work_width, work_height) = area;
        let left_width = work_width / 2;
        match target {
            WmSnapTarget::Left => (
                i16::try_from(work_x).unwrap_or(i16::MAX),
                i16::try_from(work_y).unwrap_or(i16::MAX),
                left_width,
                work_height,
            ),
            WmSnapTarget::Right => (
                i16::try_from(work_x.saturating_add(left_width)).unwrap_or(i16::MAX),
                i16::try_from(work_y).unwrap_or(i16::MAX),
                work_width.saturating_sub(left_width),
                work_height,
            ),
            WmSnapTarget::Maximize => (
                i16::try_from(work_x).unwrap_or(i16::MAX),
                i16::try_from(work_y).unwrap_or(i16::MAX),
                work_width,
                work_height,
            ),
        }
    }

    pub(super) fn set_wm_snap_preview(&mut self, target: Option<WmSnapTarget>) {
        self.wm_snap_target = target;
        let Some(target) = target else {
            if let Some(preview) = self.windows.get_mut(&WM_SNAP_PREVIEW.0) {
                preview.mapped = false;
            }
            self.scene_upsert(WM_SNAP_PREVIEW);
            return;
        };
        let area = self.wm_pointer_workarea(self.pointer_x, self.pointer_y).unwrap_or_else(|| self.wm_workarea_rect());
        let (x, y, width, height) = self.wm_snap_geometry(target, area);
        if self.windows.get(&WM_SNAP_PREVIEW.0).is_some_and(|preview| {
            preview.mapped && (preview.x, preview.y, preview.width, preview.height) == (x, y, width, height)
        }) {
            return;
        }
        let _ = self.resize_surface(WM_SNAP_PREVIEW, width.max(1), height.max(1));
        if let Some(preview) = self.windows.get_mut(&WM_SNAP_PREVIEW.0) {
            preview.x = x;
            preview.y = y;
            preview.width = width.max(1);
            preview.height = height.max(1);
            preview.mapped = true;
        }
        self.paint_wm_chrome(WM_SNAP_PREVIEW);
        self.restack(WM_SNAP_PREVIEW, None, Some(0));
        self.scene_upsert(WM_SNAP_PREVIEW);
        self.scene_damage(WM_SNAP_PREVIEW);
    }

    pub(super) fn set_wm_taskbar_drag_preview(&mut self, edge: Option<WmTaskbarEdge>) {
        let Some(edge) = edge else {
            if let Some(preview) = self.windows.get_mut(&WM_SNAP_PREVIEW.0) {
                preview.mapped = false;
            }
            self.scene_upsert(WM_SNAP_PREVIEW);
            return;
        };
        let (x, y, width, height) = self.wm_taskbar_rect_for(edge);
        let _ = self.resize_surface(WM_SNAP_PREVIEW, width.max(1), height.max(1));
        if let Some(preview) = self.windows.get_mut(&WM_SNAP_PREVIEW.0) {
            preview.x = x;
            preview.y = y;
            preview.width = width.max(1);
            preview.height = height.max(1);
            preview.mapped = true;
        }
        self.paint_wm_chrome(WM_SNAP_PREVIEW);
        self.restack(WM_SNAP_PREVIEW, None, Some(0));
        self.scene_upsert(WM_SNAP_PREVIEW);
        self.scene_damage(WM_SNAP_PREVIEW);
    }

    pub(super) fn refresh_taskbar_geometry(&mut self) {
        let (x, y, width, height) = self.wm_taskbar_rect();
        let mapped = self.wm_taskbar_mapped();
        let _ = self.resize_surface(WM_TASKBAR, width.max(1), height.max(1));
        if let Some(taskbar) = self.windows.get_mut(&WM_TASKBAR.0) {
            taskbar.x = x;
            taskbar.y = y;
            taskbar.width = width;
            taskbar.height = height;
            taskbar.mapped = mapped;
        }
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
    }

    pub(super) fn refresh_gnome_panel_geometry(&mut self) {
        let (px, py, pw, ph) = self.wm_panel_output_rect();
        let height = self.wm_gnome_panel_height.min(ph);
        let y = if self.wm_gnome_panel_bottom {
            ph.saturating_sub(height)
        } else {
            0
        };
        let _ = self.resize_surface(WM_GNOME_PANEL, pw, height);
        if let Some(panel) = self.windows.get_mut(&WM_GNOME_PANEL.0) {
            panel.x = px as i16;
            panel.y = i16::try_from(py.saturating_add(y)).unwrap_or(i16::MAX);
            panel.width = pw;
            panel.height = height;
            panel.mapped = self.wm_gnome_panel;
        }
        self.paint_wm_chrome(WM_GNOME_PANEL);
        self.scene_upsert(WM_GNOME_PANEL);
        self.scene_damage(WM_GNOME_PANEL);
    }

    /// Update only the auto-hide edge sensor without dispatching pointer
    /// motion to an underlying X11 client. Native desktop windows can own the
    /// root layer, but the WM shell must still observe approach/leave motion.
    pub fn update_taskbar_pointer(&mut self, root_x: i16, root_y: i16) -> bool {
        if self.native_fullscreen_contains(i32::from(root_x), i32::from(root_y)) { return false; }
        let selected = self.select_shell_output(root_x, root_y);
        self.pointer_x = root_x;
        self.pointer_y = root_y;
        let changed = self.reconcile_taskbar_pointer(root_x, root_y) || selected;
        if changed { self.rebuild_scene(); }
        changed
    }

    pub(super) fn reconcile_taskbar_pointer(&mut self, root_x: i16, root_y: i16) -> bool {
        if !self.wm_taskbar_visible || !self.wm_taskbar_auto_hide {
            return false;
        }
        let was_revealed = self.wm_taskbar_revealed;
        const EDGE_REVEAL_PX: i16 = 8;
        let (ox,oy,ow,oh) = self.wm_dock_output_rect();
        let local_x = i32::from(root_x)-i32::from(ox);
        let local_y = i32::from(root_y)-i32::from(oy);
        let on_output = local_x >= 0 && local_y >= 0 && local_x < i32::from(ow) && local_y < i32::from(oh);
        let edge = on_output && match self.wm_taskbar_edge {
            WmTaskbarEdge::Top => local_y < i32::from(EDGE_REVEAL_PX),
            WmTaskbarEdge::Bottom => local_y >= i32::from(oh)-i32::from(EDGE_REVEAL_PX),
            WmTaskbarEdge::Left => local_x < i32::from(EDGE_REVEAL_PX),
            WmTaskbarEdge::Right => local_x >= i32::from(ow)-i32::from(EDGE_REVEAL_PX),
        };
        let (x, y, width, height) = self.wm_taskbar_rect();
        let within = i32::from(root_x) >= i32::from(x)
            && i32::from(root_x) < i32::from(x).saturating_add(i32::from(width))
            && i32::from(root_y) >= i32::from(y)
            && i32::from(root_y) < i32::from(y).saturating_add(i32::from(height));
        let floating_gap = self.wm_dock_style && match self.wm_taskbar_edge {
            WmTaskbarEdge::Bottom => {
                    root_x >= x && i32::from(root_x) < i32::from(x) + i32::from(width) && root_y >= y}
                WmTaskbarEdge::Top => {
                    root_x >= x && i32::from(root_x) < i32::from(x) + i32::from(width) && i32::from(root_y) < i32::from(y) + i32::from(height)}
                WmTaskbarEdge::Left => {
                    root_y >= y && i32::from(root_y) < i32::from(y) + i32::from(height) && i32::from(root_x) < i32::from(x) + i32::from(width)}
                WmTaskbarEdge::Right => {
                    root_y >= y && i32::from(root_y) < i32::from(y) + i32::from(height) && root_x >= x}
            };
        let menu_open = self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped);
        let near = i32::from(root_x) >= i32::from(x) - 12
            && i32::from(root_x) < i32::from(x) + i32::from(width) + 12
            && i32::from(root_y) >= i32::from(y) - 12
            && i32::from(root_y) < i32::from(y) + i32::from(height) + 12;
        let revealed = menu_open || edge || (self.wm_taskbar_revealed && (within || near || floating_gap));
        if revealed != self.wm_taskbar_revealed {
            self.wm_taskbar_revealed = revealed;
            self.refresh_taskbar_geometry();
        }
        was_revealed != self.wm_taskbar_revealed
    }
}
