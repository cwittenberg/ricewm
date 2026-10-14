//! Task groups operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn wm_raw_task_targets(&self) -> Vec<WmTaskTarget> {
        self.managed_frames
            .keys()
            .copied()
            .filter(|client| {
                self.window_workspaces.get(client).copied() == Some(self.output_workspace(self.wm_shell_output as u32))
                    && (!self.scrolling_workspaces||self.task_output(WmTaskTarget::X11(ResourceId(*client)))==self.wm_shell_output as u32)
                    && !self.parked_managed_windows.contains(client)
                    // ICCCM owned/transient top-levels belong to their
                    // owner's application group. They remain independently
                    // composited and focused, but must not acquire another
                    // task-list entry beside the owner.
                    && self
                        .managed_transient_owner_relation(ResourceId(*client))
                        .is_none()
            })
            .map(|client| WmTaskTarget::X11(ResourceId(client)))
            .chain(
                self.native_tasks
                    .iter()
                    .filter(|task| task.workspace == self.output_workspace(self.wm_shell_output as u32)&&(!self.scrolling_workspaces||self.task_output(WmTaskTarget::Native(task.id))==self.wm_shell_output as u32))
                    .map(|task| WmTaskTarget::Native(task.id)),
            )
            .collect()
    }

    pub(super) fn wm_task_group_key(&self, target: WmTaskTarget) -> Vec<u8> {
        if self.wm_dock_style { if let WmTaskTarget::Native(id) = target { if let Some(key) = self.wm_dock_task_keys.get(&id).filter(|key| **key != 0) { return format!("app:{key}").into_bytes(); } } }
        match target {
            WmTaskTarget::X11(client) => self
                .windows
                .get(&client.0)
                .and_then(|window| {
                    window.properties.iter().find_map(|(atom, property)| {
                        (self.atoms.name(*atom) == Some("WM_CLASS")).then(|| {
                            property
                                .data
                                .split(|byte| *byte == 0)
                                .filter(|part| !part.is_empty())
                                .next_back()
                                .unwrap_or_default()
                                .iter()
                                .map(u8::to_ascii_lowercase)
                                .collect::<Vec<_>>()
                        })
                    })
                })
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| format!("x11:{}", client.0).into_bytes()),
            WmTaskTarget::Native(id) => self
                .native_tasks
                .iter()
                .find(|task| task.id == id)
                .map(|task| {
                    let title = strip_instance_suffix(&task.title);
                    let mut key = b"native:".to_vec();
                    key.extend(title.iter().map(u8::to_ascii_lowercase));
                    key
                })
                .unwrap_or_else(|| format!("native:{id}").into_bytes()),
            WmTaskTarget::Group(group) => format!("group:{group}").into_bytes(),
        }
    }

    pub(super) fn wm_task_groups(&self) -> Vec<Vec<WmTaskTarget>> {
        let mut groups: Vec<(Vec<u8>, Vec<WmTaskTarget>)> = Vec::new();
        for target in self.wm_raw_task_targets() {
            let key = self.wm_task_group_key(target);
            if let Some((_, members)) = groups.iter_mut().find(|(candidate, _)| *candidate == key) {
                members.push(target);
            } else {
                groups.push((key, vec![target]));
            }
        }
        groups.into_iter().map(|(_, members)| members).collect()
    }

    pub(super) fn wm_task_group(&self, group: u16) -> Vec<WmTaskTarget> {
        self.wm_task_groups()
            .get(usize::from(group))
            .cloned()
            .unwrap_or_default()
    }

    pub(super) fn wm_task_group_rows(&self, group: u16) -> Vec<WmMenuRow> {
        let members = self.wm_task_group(group);
        let visible_rows = self.wm_menu_visible_rows();
        let scrolling = members.len() > visible_rows;
        let capacity = if scrolling {
            visible_rows.saturating_sub(2).max(1)
        } else {
            visible_rows.max(1)
        };
        let start = self
            .wm_menu_offset
            .min(members.len().saturating_sub(capacity));
        let end = start.saturating_add(capacity).min(members.len());
        let mut rows = Vec::new();
        if scrolling {
            rows.push(WmMenuRow::ScrollUp);
        }
        rows.extend(members[start..end].iter().copied().map(WmMenuRow::Task));
        if scrolling {
            rows.push(WmMenuRow::ScrollDown);
        }
        rows
    }

    pub(super) fn wm_task_group_max_offset(&self, group: u16) -> usize {
        let count = self.wm_task_group(group).len();
        let capacity = if count > self.wm_menu_visible_rows() {
            self.wm_menu_visible_rows().saturating_sub(2).max(1)
        } else {
            self.wm_menu_visible_rows().max(1)
        };
        count.saturating_sub(capacity)
    }

    pub(super) fn wm_task_targets(&self) -> Vec<WmTaskTarget> {
        if !self.wm_gnome_dash_visible {
            return Vec::new();
        }
        self.wm_task_groups()
            .into_iter()
            .enumerate()
            .flat_map(|(index, members)| {
                if members.len() > if self.wm_dock_style { 1 } else { 3 } {
                    vec![WmTaskTarget::Group(
                        u16::try_from(index).unwrap_or(u16::MAX),
                    )]
                } else {
                    members
                }
            })
            .take(self.wm_task_capacity())
            .collect()
    }

    pub(super) fn wm_task_capacity(&self) -> usize {
        if self.wm_dock_style { return 32; }
        if self.wm_taskbar_edge.vertical() {
            let tray_top = self.wm_vertical_tray_layout().first().map_or(
                usize::from(self.wm_taskbar_rect().3).saturating_sub(6),
                |item| item.2,
            );
            return tray_top
                .saturating_sub(self.wm_vertical_task_y())
                .checked_div(30)
                .unwrap_or_default()
                .max(1);
        }
        let available = i32::from(self.config.width_px)
            .saturating_sub(i32::from(self.wm_task_button_x()))
            .saturating_sub(if self.wm_compact_shell() { 8 } else { 228 });
        usize::try_from((available / 60).max(1)).unwrap_or(1)
    }

    pub(super) fn wm_start_button_width(&self) -> i16 {
        if !self.wm_launcher_visible {return 0;}
        if self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98)
        {
            return 59;
        }
        match self.wm_launcher_appearance {
            0 => 36,
            2 => i16::try_from(
                self.wm_launcher_text
                    .len()
                    .saturating_mul(7)
                    .saturating_add(20),
            )
            .unwrap_or(200)
            .clamp(44, 200),
            _ => i16::try_from(
                self.wm_launcher_text
                    .len()
                    .saturating_mul(7)
                    .saturating_add(44),
            )
            .unwrap_or(200)
            .clamp(WM_START_BUTTON_WIDTH, 200),
        }
    }

    pub(super) fn wm_quick_launch_x(&self) -> i16 {
        self.wm_start_button_width().saturating_add(8)
    }

    pub(super) fn wm_task_slot_width(&self) -> i32 {
        let count = i32::try_from(self.wm_task_targets().len().max(1)).unwrap_or(1);
        let available = i32::from(self.config.width_px)
            .saturating_sub(i32::from(self.wm_task_button_x()))
            .saturating_sub(if self.wm_compact_shell() { 8 } else { 228 });
        (available / count).clamp(60, 132)
    }

    pub(super) fn wm_task_button_x(&self) -> i16 {
        if !self.wm_quick_launch_visible || self.wm_quick_launchers.is_empty() {
            return self.wm_quick_launch_x();
        }
        if self.wm_quick_launchers.len() == 3 {
            return self
                .wm_quick_launch_x()
                .saturating_add(3 * 28 + WM_QUICK_LAUNCH_GAP);
        }
        self.wm_quick_launch_x()
            .saturating_add(
                i16::try_from(self.wm_quick_launchers.len().saturating_mul(28)).unwrap_or(i16::MAX),
            )
            .saturating_add(WM_QUICK_LAUNCH_GAP)
    }

    pub(super) fn wm_vertical_quick_launch_y(&self) -> usize {
        36
    }

    pub(super) fn wm_vertical_task_y(&self) -> usize {
        self.wm_vertical_quick_launch_y()
            .saturating_add(
                usize::from(self.wm_quick_launch_visible)
                    .saturating_mul(self.wm_quick_launchers.len())
                    .saturating_mul(28),
            )
            .saturating_add(6)
    }

    pub(super) fn wm_vertical_tray_layout(&self) -> Vec<(usize, TrayItem, usize, usize)> {
        if self.wm_compact_shell() { return Vec::new(); }
        let mut cursor = usize::from(self.wm_taskbar_rect().3).saturating_sub(6);
        let mut layout = Vec::with_capacity(self.wm_tray_items.len());
        for (index, item) in self.wm_tray_items.iter().copied().enumerate().rev() {
            if item == TrayItem::Workspaces && !self.wm_gnome_workspaces_visible {
                continue;
            }
            let extent = match item {
                TrayItem::Workspaces => self.wm_visible_workspace_ids().len()
                    .saturating_mul(28),
                TrayItem::Clock | TrayItem::Performance => 30,
                TrayItem::Sound | TrayItem::Connectivity | TrayItem::Launcher(_) => 28,
            };
            cursor = cursor.saturating_sub(extent);
            layout.push((index, item, cursor, extent));
            cursor = cursor.saturating_sub(4);
        }
        layout.reverse();
        layout
    }

    pub(super) fn wm_quick_launch_tooltip(&self) -> Option<&[u8]> {
        if let Some(WmChromeTarget::NotificationIcon(slot)) = self.wm_chrome_hover {
            return self.wm_notification_icons.iter().find(|icon| icon.slot == slot)
                .filter(|icon| !icon.tip.is_empty()).map(|icon| icon.tip.as_slice());
        }
        if !self.wm_gnome_dash_visible {
            return None;
        }
        let WmChromeTarget::QuickLaunch(index) = self.wm_chrome_hover? else {
            return None;
        };
        self.wm_quick_launchers
            .get(usize::from(index))
            .map(|launcher| launcher.title.as_slice())
    }

    pub(super) fn update_wm_quick_launch_tooltip(&mut self) {
        let title = self.wm_quick_launch_tooltip().map(<[u8]>::to_vec);
        let index = match self.wm_chrome_hover {
            Some(WmChromeTarget::QuickLaunch(index)) if title.is_some() => usize::from(index),
            _ => 0,
        };
        let tooltip_width = WM_TOOLTIP_WIDTH.min(self.config.width_px);
        let tooltip_height = WM_TOOLTIP_HEIGHT.min(self.config.height_px);
        let _ = self.resize_surface(WM_TOOLTIP, tooltip_width, tooltip_height);
        let center = i32::from(self.wm_quick_launch_x())
            .saturating_add(i32::try_from(index.saturating_mul(28)).unwrap_or(i32::MAX))
            .saturating_add(12);
        let (dock_x, dock_y, dock_width, dock_height) = self.wm_taskbar_rect();
        let center = if self.wm_dock_style {
            i32::from(dock_x) + if self.wm_taskbar_edge.vertical() { i32::from(dock_width) / 2 } else { (WM_COMPACT_DOCK_AXIS_PADDING + (index + 1) * self.wm_dock_step() + self.wm_dock_step() / 2) as i32 }
        } else { center };
        let center = if let Some(WmChromeTarget::NotificationIcon(slot)) = self.wm_chrome_hover {
            self.notification_icon_layout().into_iter().find(|(id,_,_)| *id == slot)
                .map(|(_,x,_)| i32::from(dock_x) + x as i32 + 12).unwrap_or(center)
        } else { center };
        let maximum_x = i32::from(self.config.width_px.saturating_sub(tooltip_width));
        let x = center
            .saturating_sub(i32::from(tooltip_width) / 2)
            .clamp(0, maximum_x);
        let y = if self.wm_taskbar_edge == WmTaskbarEdge::Top {
            i32::from(dock_y).saturating_add(i32::from(dock_height)).saturating_add(4)
        } else {
            i32::from(dock_y)
                .saturating_sub(i32::from(tooltip_height))
                .saturating_sub(4)
        }
        .max(0);
        let menu_mapped = self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped);
        if let Some(window) = self.windows.get_mut(&WM_TOOLTIP.0) {
            window.x = clamp_i16(x);
            window.y = clamp_i16(y);
            window.width = tooltip_width;
            window.height = tooltip_height;
            window.mapped = title.is_some() && self.wm_quick_launch_visible && !menu_mapped;
        }
        self.paint_wm_chrome(WM_TOOLTIP);
        if title.is_some() {
            self.restack(WM_TOOLTIP, None, Some(0));
        }
        self.scene_upsert(WM_TOOLTIP);
        self.scene_damage(WM_TOOLTIP);
    }

    pub(super) fn wm_task_item(&mut self, target: WmTaskTarget) -> Option<(Vec<u8>, Option<Vec<u32>>, bool)> {
        match target {
            WmTaskTarget::X11(client) => {
                let native_task_active = self.native_tasks.iter().any(|task| {
                    task.workspace == self.current_workspace && task.active && !task.minimized
                });
                let focused_client = self.managed_client_ancestor(self.focus_window);
                Some((
                    self.wm_window_title(client),
                    self.wm_window_icon(client),
                    !native_task_active
                        && focused_client
                            .is_some_and(|focused| self.managed_transient_root(focused) == client)
                        && !self.minimized_windows.contains(&client.0),
                ))
            }
            WmTaskTarget::Native(id) => {
                let task = self.native_tasks.iter().find(|task| task.id == id)?;
                Some((task.title.clone(), None, task.active && !task.minimized))
            }
            WmTaskTarget::Group(group) => {
                let members = self.wm_task_group(group);
                let first = members.first().copied()?;
                let active = members.iter().copied().any(|member| {
                    self.wm_task_item(member)
                        .is_some_and(|(_, _, active)| active)
                });
                let (title, icon, _) = self.wm_task_item(first)?;
                let title = format!(
                    "{} ({})",
                    String::from_utf8_lossy(strip_instance_suffix(&title)),
                    members.len(),
                )
                .into_bytes();
                Some((title, icon, active))
            }
        }
    }

    pub(super) fn activate_wm_task_target(&mut self, target: WmTaskTarget) {
        match target {
            WmTaskTarget::X11(client) => {
                // A modal transient is the interactive representative of its
                // owner while it is mapped. Nonmodal owned windows remain in
                // the task group but must not disable their owner.
                if let Some(transient) = self.topmost_managed_modal_transient(client) {
                    self.activate_managed_window(transient);
                    return;
                }
                let native_task_active = self.native_tasks.iter().any(|task| {
                    task.workspace == self.current_workspace && task.active && !task.minimized
                });
                let was_active = (self.focus_window == client
                    || self.managed_client_ancestor(self.focus_window) == Some(client))
                    && !self.minimized_windows.contains(&client.0)
                    && !native_task_active;
                if was_active {
                    self.minimized_windows.insert(client.0);
                    if let Some(frame) = self.managed_frames.get(&client.0).copied() {
                        if let Some(window) = self.windows.get_mut(&frame.0) {
                            window.mapped = false;
                        }
                        self.scene_upsert(frame);
                    }
                    self.set_focus(ROOT_WINDOW);
                    self.notify_x11_foreground(client);
                } else {
                    self.activate_managed_window(client);
                }
            }
            WmTaskTarget::Native(id) => self
                .wm_actions
                .push_back(WM_ACTION_NATIVE_TASK_PREFIX | u32::from(id)),
            WmTaskTarget::Group(_) => {}
        }
    }

    pub(super) fn minimize_managed_task(&mut self, client: ResourceId) {
        self.minimized_windows.insert(client.0);
        if let Some(frame) = self.managed_frames.get(&client.0).copied() {
            if let Some(window) = self.windows.get_mut(&frame.0) {
                window.mapped = false;
            }
            self.scene_upsert(frame);
        }
        if self.focus_window == client
            || self.managed_client_ancestor(self.focus_window) == Some(client)
        {
            self.set_focus(ROOT_WINDOW);
        }
        self.notify_x11_foreground(client);
    }

    pub(super) fn handle_wm_task_command(&mut self, target: WmTaskTarget, row: WmMenuRow) {
        if let WmTaskTarget::Group(group) = target {
            let members = self.wm_task_group(group);
            match row {
                WmMenuRow::StackClose => {
                    if let Some(WmTaskTarget::Native(id)) = members.first().copied() {
                        self.wm_actions
                            .push_back(WM_ACTION_STACK_CLOSE_PREFIX | u32::from(id));
                    } else {
                        for member in members {
                            if let WmTaskTarget::X11(client) = member {
                                self.request_close_window(client);
                            }
                        }
                    }
                }
                WmMenuRow::StackTile | WmMenuRow::StackCascade => {
                    if let Some(WmTaskTarget::Native(id)) = members.first().copied() {
                        let prefix = if row == WmMenuRow::StackTile {
                            WM_ACTION_STACK_TILE_PREFIX
                        } else {
                            WM_ACTION_STACK_CASCADE_PREFIX
                        };
                        self.wm_actions.push_back(prefix | u32::from(id));
                    } else {
                        self.arrange_x11_task_stack(&members, row == WmMenuRow::StackTile);
                    }
                }
                _ => {}
            }
            return;
        }
        match target {
            WmTaskTarget::X11(client) => match row {
                WmMenuRow::WindowClose => self.request_close_window(client),
                WmMenuRow::WindowMinimize => self.minimize_managed_task(client),
                WmMenuRow::WindowMaximize => {
                    if !self.maximized_geometries.contains_key(&client.0) {
                        self.toggle_maximized(client, 2);
                    }
                    self.activate_managed_window(client);
                }
                WmMenuRow::WindowRestore => {
                    if self.maximized_geometries.contains_key(&client.0) {
                        self.toggle_maximized(client, 2);
                    }
                    self.activate_managed_window(client);
                }
                _ => {}
            },
            WmTaskTarget::Native(id) => {
                let prefix = match row {
                    WmMenuRow::WindowClose => WM_ACTION_TASK_CLOSE_PREFIX,
                    WmMenuRow::WindowMaximize => WM_ACTION_TASK_MAXIMIZE_PREFIX,
                    WmMenuRow::WindowMinimize => WM_ACTION_TASK_MINIMIZE_PREFIX,
                    WmMenuRow::WindowRestore => WM_ACTION_TASK_RESTORE_PREFIX,
                    _ => return,
                };
                self.wm_actions.push_back(prefix | u32::from(id));
            }
            WmTaskTarget::Group(_) => {}
        }
    }
}
