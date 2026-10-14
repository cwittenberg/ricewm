//! Shell hit testing operations owned by DisplayServer.
use super::*;

impl DisplayServer {
    pub(super) fn wm_compact_shell(&self) -> bool {
        { let (_, _, width, height) = self.wm_dock_output_rect(); width < 700 || height < 500 }
    }


    /// GNOME's panel boxes lay indicators out in insertion order: the left
    /// box grows from the left edge, the right box grows inward from the
    /// right edge, and the center box is centered as a group. Each
    /// extension's requested position and index are honored, and indicators
    /// are never allowed to overlap.
    pub(super) const fn wm_system_controls_width(&self) -> usize {
        if self.wm_battery_percentage.is_some() {
            WM_SYSTEM_CONTROLS_BATTERY_WIDTH
        } else {
            WM_SYSTEM_CONTROLS_WIDTH
        }
    }

    pub(super) fn wm_gnome_indicator_layout(&self) -> Vec<(u8, i16, u16)> {
        if !self.wm_gnome_panel {
            return Vec::new();
        }
        if self.wm_topbar_config.is_some() {
            return self.wm_topbar_layout().iter().filter(|item| item.width > 0).filter_map(|item| {
                self.wm_gnome_indicators.iter().position(|a| a.uuid == item.key.as_bytes())
                    .map(|index| (index as u8, item.x, item.width))
            }).collect();
        }
        let panel_width = i32::from(self.wm_panel_output_rect().2);
        let mut visible = self
            .wm_gnome_indicators
            .iter()
            .enumerate()
            .filter(|(_, indicator)| indicator.visible)
            .map(|(order, indicator)| {
                let text_width = indicator.label.len().min(128).saturating_mul(8);
                let icon_width = usize::from(indicator.has_icon) * 20;
                let width = u16::try_from(text_width.saturating_add(icon_width).saturating_add(24))
                    .unwrap_or(180)
                    .clamp(36, self.config.width_px.max(36));
                (order, indicator.index, indicator.position, width)
            })
            .collect::<Vec<_>>();
        visible.sort_by_key(|&(order, index, position, _)| (position, index, order));
        let mut output = Vec::with_capacity(visible.len());
        let mut left_x = 8i32;
        let controls_top = self.wm_system_controls & 0x41 == 1;
        // The shared canvas panel always displays Rice Box and reserves two
        // accessory buttons (search and applications) immediately to its left.
        // Keep these bays free even when the target does not retain mail pixels.
        let mail_reservation = WM_RICE_BOX_INDICATOR_WIDTH
            .saturating_add(WM_SYSTEM_CONTROLS_EXTENSION_GAP)
            .saturating_add(74);
        let right_reservation = if controls_top {
            self.wm_system_controls_width()
                .saturating_add(WM_SYSTEM_CONTROLS_RIGHT_MARGIN)
                .saturating_add(WM_SYSTEM_CONTROLS_EXTENSION_GAP)
                .saturating_add(mail_reservation)
        } else {
            94usize.saturating_add(mail_reservation)
        };
        let mut right_x = panel_width.saturating_sub(
            i32::try_from(right_reservation).unwrap_or(i32::MAX));
        let center_width: i32 = visible
            .iter()
            .filter(|&&(_, _, position, _)| position == 1)
            .map(|&(_, _, _, width)| i32::from(width).saturating_add(6))
            .sum::<i32>()
            .saturating_sub(6)
            .max(0);
        let center_start = (panel_width - center_width) / 2;
        let mut center_x = center_start;
        for (order, _index, position, width) in visible {
            let width_i32 = i32::from(width);
            let x = match position {
                0 => {
                    let x = left_x;
                    left_x = left_x.saturating_add(width_i32.saturating_add(6));
                    x
                }
                1 => {
                    let x = center_x;
                    center_x = center_x.saturating_add(width_i32.saturating_add(6));
                    x
                }
                _ => {
                    right_x = right_x.saturating_sub(width_i32);
                    let x = right_x;
                    right_x = right_x.saturating_sub(6);
                    x
                }
            };
            output.push((
                u8::try_from(order).unwrap_or(255),
                i16::try_from(x.max(0)).unwrap_or(0),
                width,
            ));
        }
        output
    }

    pub(super) fn wm_gnome_indicator_at(&self, root_x: i16) -> Option<u8> {
        self.wm_gnome_indicator_layout()
            .into_iter()
            .find_map(|(order, x, width)| {
                let offset = i32::from(root_x).saturating_sub(i32::from(x));
                (0..i32::from(width)).contains(&offset).then_some(order)
            })
    }

    pub(super) fn wm_gnome_menu_hit(&self, root_y: i16) -> Option<u8> {
        let menu = self.windows.get(&WM_GNOME_MENU.0)?;
        if !menu.mapped {
            return None;
        }
        let local_y = usize::try_from(i32::from(root_y).saturating_sub(i32::from(menu.y))).ok()?;
        let mut y = 6usize;
        for item in &self.wm_gnome_menu_items {
            let height = wm_gnome_menu_item_height(item);
            if (y..y.saturating_add(height)).contains(&local_y) {
                return (!item.separator && item.reactive).then_some(item.index);
            }
            y = y.saturating_add(height);
        }
        None
    }

    pub(super) fn wm_chrome_hit(
        &self,
        source: ResourceId,
        root_x: i16,
        root_y: i16,
    ) -> Option<WmChromeTarget> {
        if source == WM_GNOME_PANEL {
            let root_x = root_x.saturating_sub(self.wm_panel_output_rect().0 as i16);
            if self.wm_topbar_config.is_some() {
                return self.wm_topbar_layout().iter().find(|item| {
                        item.width > 0 && root_x >= item.x && i32::from(root_x) < i32::from(item.x)+i32::from(item.width)}).and_then(|item| match item.key.as_str() {
                    "@mail" => Some(WmChromeTarget::RiceBox),
                    "@network" | "@audio" | "@battery" | "@session" => {
                            Some(WmChromeTarget::SystemControls)}
                        _ => None,
                });
            }
            if self.wm_system_controls & 0x41 == 1 &&
                i32::from(root_x) >= i32::from(self.wm_panel_output_rect().2).saturating_sub(
                    i32::try_from(self.wm_system_controls_width() + WM_SYSTEM_CONTROLS_RIGHT_MARGIN,
                        )
                        .unwrap_or(i32::MAX),
                ) {
                return Some(WmChromeTarget::SystemControls);
            }
            if self.wm_mail_unread.is_some() {
                let reserved_right = if self.wm_system_controls & 0x41 == 1 {
                    WM_SYSTEM_CONTROLS_RIGHT_MARGIN
                        .saturating_add(self.wm_system_controls_width())
                        .saturating_add(WM_SYSTEM_CONTROLS_EXTENSION_GAP)
                } else {
                    94
                };
                let mail_left = usize::from(self.wm_panel_output_rect().2)
                    .saturating_sub(reserved_right)
                    .saturating_sub(WM_RICE_BOX_INDICATOR_WIDTH);
                let mail_right = mail_left.saturating_add(WM_RICE_BOX_INDICATOR_WIDTH);
                if (i32::try_from(mail_left).unwrap_or(i32::MAX)
                    ..i32::try_from(mail_right).unwrap_or(i32::MAX))
                    .contains(&i32::from(root_x))
                {
                    return Some(WmChromeTarget::RiceBox);
                }
            }
            return None;
        }
        if source == WM_TASKBAR {
            if let Some(slot) = self.notification_icon_at(root_x, root_y) { return Some(WmChromeTarget::NotificationIcon(slot)); }
            if self.wm_dock_style {
                let (x, y, width, height) = self.wm_taskbar_rect();
                let lx = i32::from(root_x) - i32::from(x);
                let ly = i32::from(root_y) - i32::from(y);
                if lx < 0 || ly < 0 || lx >= i32::from(width) || ly >= i32::from(height) { return None; }
                let axis = if self.wm_taskbar_edge.vertical() { ly } else { lx };
                if axis < WM_COMPACT_DOCK_AXIS_PADDING as i32 { return None; }
                return self.wm_dock_targets().get((axis as usize - WM_COMPACT_DOCK_AXIS_PADDING) / self.wm_dock_step()).copied();
            }
            if self.wm_taskbar_edge.vertical() {
                let (_, taskbar_y, _, _) = self.wm_taskbar_rect();
                let local_y =
                    usize::try_from(i32::from(root_y).saturating_sub(i32::from(taskbar_y))).ok()?;
                if self.wm_gnome_dash_visible && self.wm_launcher_visible && local_y < 32 {
                    return Some(WmChromeTarget::Start);
                }
                let quick_y = self.wm_vertical_quick_launch_y();
                if self.wm_gnome_dash_visible && self.wm_quick_launch_visible {
                    let quick_extent = self.wm_quick_launchers.len().saturating_mul(28);
                    if (quick_y..quick_y.saturating_add(quick_extent)).contains(&local_y) {
                        let offset = local_y.saturating_sub(quick_y);
                        if offset % 28 < 25 {
                            return Some(WmChromeTarget::QuickLaunch(
                                u8::try_from(offset / 28).ok()?,
                            ));
                        }
                    }
                }
                for (index, item, top, extent) in self.wm_vertical_tray_layout() {
                    if !(top..top.saturating_add(extent)).contains(&local_y) {
                        continue;
                    }
                    let tray = u8::try_from(index).ok()?;
                    if item == TrayItem::Workspaces {
                        let workspace = *self.wm_visible_workspace_ids().get((local_y-top)/28)?;
                        return Some(WmChromeTarget::Workspace {tray,workspace});
                    }
                    return Some(WmChromeTarget::Tray(tray));
                }
                let task_y = self.wm_vertical_task_y();
                if self.wm_gnome_dash_visible && local_y >= task_y {
                    let offset = local_y.saturating_sub(task_y);
                    if offset % 30 < 26 {
                        return self
                            .wm_task_targets()
                            .get(offset / 30)
                            .copied()
                            .map(WmChromeTarget::Task);
                    }
                }
                return None;
            }
            let root_x = root_x.saturating_sub(self.wm_taskbar_rect().0);
            if !self.wm_compact_shell() && self.wm_system_controls & 0x41 == 0x41 &&
                i32::from(root_x) >= i32::from(self.wm_panel_output_rect().2).saturating_sub(
                    i32::try_from(self.wm_system_controls_width() + WM_SYSTEM_CONTROLS_RIGHT_MARGIN,
                        )
                        .unwrap_or(i32::MAX),
                ) {
                return Some(WmChromeTarget::SystemControls);
            }
            if self.wm_gnome_dash_visible && root_x < self.wm_start_button_width() {
                return Some(WmChromeTarget::Start);
            }
            let task_button_x = self.wm_task_button_x();
            if self.wm_gnome_dash_visible
                && self.wm_quick_launch_visible
                && (self.wm_quick_launch_x()..task_button_x).contains(&root_x)
            {
                let offset = usize::try_from(root_x - self.wm_quick_launch_x()).ok()?;
                let index = offset / 28;
                if index < self.wm_quick_launchers.len() && offset % 28 < 25 {
                    return Some(WmChromeTarget::QuickLaunch(
                        u8::try_from(index).unwrap_or_default(),
                    ));
                }
            }
            for (index, item, left, item_width) in self.wm_tray_layout() {
                let offset = i32::from(root_x).saturating_sub(i32::try_from(left).ok()?);
                if !(0..i32::try_from(item_width).ok()?).contains(&offset) {
                    continue;
                }
                let tray = u8::try_from(index).ok()?;
                if item == TrayItem::Workspaces {
                    let mut workspace_x = 0i32;
                    for workspace in self.wm_visible_workspace_ids() {
                        let width = i32::try_from(self.wm_workspace_item_width(workspace)).ok()?;
                        if (workspace_x..workspace_x + width - 4).contains(&offset) {
                            return Some(WmChromeTarget::Workspace { tray, workspace });
                        }
                        workspace_x += width;
                    }
                    return None;
                }
                return Some(WmChromeTarget::Tray(tray));
            }
            let task_offset = i32::from(root_x).saturating_sub(i32::from(task_button_x));
            if self.wm_gnome_dash_visible && task_offset >= 0 {
                let slot_width = self.wm_task_slot_width();
                let index = usize::try_from(task_offset / slot_width).ok()?;
                if task_offset % slot_width < slot_width.saturating_sub(6) {
                    return self
                        .wm_task_targets()
                        .get(index)
                        .copied()
                        .map(WmChromeTarget::Task);
                }
            }
            return None;
        }
        let client = self.frame_clients.get(&source.0).copied()?;
        let (_, _, title_height, _) = self.managed_frame_insets(client);
        if title_height <= WM_BORDER_WIDTH {
            return None;
        }
        let (origin_x, origin_y) = self.window_origin(source);
        let local_x = i32::from(root_x).saturating_sub(origin_x);
        let local_y = i32::from(root_y).saturating_sub(origin_y);
        let frame = self.windows.get(&source.0)?;
        if !(0..i32::from(title_height)).contains(&local_y) {
            return None;
        }
        // Keep the resize targets available at titlebar corners. The button
        // interiors retain their normal actions; fixed/maximized frames do
        // not advertise a resize target and retain their whole control hitbox.
        if (!self.auto_tile_enabled||self.live_tile_target(0,client.0).is_none())
            && self.managed_resize_frame_at(source,root_x,root_y).is_some_and(|(_,edges)|
                {
                    edges&(WM_RESIZE_LEFT|WM_RESIZE_RIGHT)!=0&&edges&(WM_RESIZE_TOP|WM_RESIZE_BOTTOM)!=0}) {
            return None;
        }
        let profile = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .map_or(WmChromeProfile::Standard, |theme| theme.chrome_profile);
        let icon_left = i32::try_from(wm_frame_icon_left(profile)).ok()?;
        if (icon_left..icon_left + 24).contains(&local_x) {
            return Some(WmChromeTarget::FrameIcon(source));
        }
        let control = wm_frame_control_at(
            profile,
            usize::from(frame.width),
            usize::try_from(local_x).ok()?,
        )?;
        Some(WmChromeTarget::FrameControl(source, control))
    }

    pub(super) fn wm_tray_layout(&self) -> Vec<(usize, TrayItem, usize, usize)> {
        if self.wm_compact_shell() { return Vec::new(); }
        let controls_taskbar=self.wm_system_controls&0x41==0x41;
        let mut cursor = usize::from(self.wm_taskbar_rect().2)
            .saturating_sub(if controls_taskbar {
                self.wm_system_controls_width()
                    .saturating_add(WM_SYSTEM_CONTROLS_RIGHT_MARGIN)
                    .saturating_add(8)
            } else {
                7
            });
        let mut layout = Vec::with_capacity(self.wm_tray_items.len());
        for (index, item) in self.wm_tray_items.iter().copied().enumerate().rev() {
            if self.wm_system_controls & 1 != 0 && item == TrayItem::Connectivity { continue; }
            if item == TrayItem::Workspaces && !self.wm_gnome_workspaces_visible {
                continue;
            }
            let width = self.wm_tray_item_width(item);
            cursor = cursor.saturating_sub(width);
            layout.push((index, item, cursor, width));
            cursor = cursor.saturating_sub(4);
        }
        layout.reverse();
        layout
    }

    pub(super) fn wm_tray_item_width(&self, item: TrayItem) -> usize {
        match item {
            TrayItem::Clock => 70,
            TrayItem::Workspaces => self.wm_visible_workspace_ids().into_iter()
                .map(|workspace| self.wm_workspace_item_width(workspace))
                .sum(),
            TrayItem::Performance => 66,
            TrayItem::Sound | TrayItem::Connectivity | TrayItem::Launcher(_) => 25,
        }
    }

    pub(super) fn wm_workspace_item_width(&self, workspace: u32) -> usize {
        let characters = self
            .wm_workspace_names
            .get(usize::try_from(workspace).unwrap_or_default())
            .and_then(|name| core::str::from_utf8(name).ok())
            .map_or(1, |name| name.chars().count());
        characters
            .saturating_mul(7)
            .saturating_add(16)
            .clamp(28, 100)
    }

    pub(super) fn wm_tray_target_item(&self, target: WmChromeTarget) -> Option<TrayItem> {
        let (WmChromeTarget::Tray(index) | WmChromeTarget::Workspace { tray: index, .. }) = target
        else {
            return None;
        };
        self.wm_tray_items.get(usize::from(index)).copied()
    }

    pub(super) fn repaint_wm_chrome_targets(
        &mut self,
        first: Option<WmChromeTarget>,
        second: Option<WmChromeTarget>,
    ) {
        let first_window = first.map(WmChromeTarget::window);
        let second_window = second.map(WmChromeTarget::window);
        if let Some(window) = first_window {
            self.paint_wm_chrome(window);
            self.scene_upsert(window);
        }
        if let Some(window) = second_window
            && Some(window) != first_window
        {
            self.paint_wm_chrome(window);
            self.scene_upsert(window);
        }
    }

    pub(super) fn set_wm_volume(&mut self, volume: u8, muted: bool) {
        self.wm_volume = volume.min(100);
        self.wm_volume_muted = muted;
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        let message = if self.wm_volume_muted {
            b"MUTED".to_vec()
        } else {
            format!("{}%", self.wm_volume).into_bytes()
        };
        let _ = self.show_wm_notification(0, b"SYSTEM VOLUME".to_vec(), message);
        self.wm_actions.push_back(
            WM_ACTION_VOLUME_PREFIX
                | u32::from(self.wm_volume)
                | if self.wm_volume_muted { 0x100 } else { 0 },
        );
    }
}
