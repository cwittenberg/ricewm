//! Workspace identities remain stable while empty entries disappear from the
//! visible list. Each output owns its selection and one empty destination.
use super::*;

impl DisplayServer {
    pub fn set_scrolling_workspaces(&mut self, enabled: bool) {
        let changed = self.scrolling_workspaces != enabled;
        self.scrolling_workspaces = enabled;
        if changed && enabled {
            for output in 0..self.wm_outputs.len().max(1) {
                self.output_workspaces
                    .insert(output as u32, self.current_workspace);
            }
        }
        self.refresh_scrolling_workspaces();
        if changed {
            self.apply_workspace_visibility();
        }
    }
    pub fn output_workspace(&self, output: u32) -> u32 {
        if self.scrolling_workspaces {
            *self.output_workspaces.get(&output).unwrap_or(&0)
        } else {
            self.current_workspace
        }
    }
    pub fn active_output(&self) -> u32 {
        self.wm_shell_output as u32
    }
    pub fn workspace_count(&self) -> u32 {
        self.workspace_count
    }
    pub fn set_native_output_position(&mut self, id: u16, x: i32, y: i32) {
        // Permit one complete task-list turnover before replace_native_tasks prunes it.
        if id != 0 && (self.native_output_positions.contains_key(&id) || self.native_output_positions.len()<256) {
            self.native_output_positions.insert(id, (x, y));
        }
    }
    pub(super) fn task_output(&self, target: WmTaskTarget) -> u32 {
        let point = match target {
            WmTaskTarget::Native(id) => self.native_output_positions.get(&id).copied(),
            WmTaskTarget::X11(id) => self
                .auto_tile_restore
                .get(&id.0)
                .map(|&(x, y, w, h)| {
                    (
                        i32::from(x) + i32::from(w) / 2,
                        i32::from(y) + i32::from(h) / 2,
                    )
                })
                .or_else(|| {
                    self.managed_frames
                        .get(&id.0)
                        .and_then(|f| self.windows.get(&f.0))
                        .map(|w| {
                            (
                                i32::from(w.x) + i32::from(w.width) / 2,
                                i32::from(w.y) + i32::from(w.height) / 2,
                            )
                        })
                }),
            WmTaskTarget::Group(_) => None,
        };
        point
            .and_then(|(x, y)| {
                self.wm_outputs.iter().position(|&(ox, oy, w, h)| {
                    x >= i32::from(ox)
                        && x < i32::from(ox) + i32::from(w)
                        && y >= i32::from(oy)
                        && y < i32::from(oy) + i32::from(h)
                })
            })
            .unwrap_or(0) as u32
    }
    pub fn workspace_window_visible(&self, kind: u32, id: u32, workspace: u32) -> bool {
        let target = match kind {
            0 if id != 0 => WmTaskTarget::X11(ResourceId(id)),
            1 if (1..=65535).contains(&id) => WmTaskTarget::Native(id as u16),
            _ => return false,
        };
        workspace == self.output_workspace(self.task_output(target))
    }
    pub(super) fn x11_workspace_visible(&self, id: u32) -> bool {
        self.window_workspaces
            .get(&id)
            .is_some_and(|&ws| self.workspace_window_visible(0, id, ws))
    }
    fn occupied_output_workspaces(&self, output: u32) -> BTreeSet<u32> {
        self.managed_frames
            .keys()
            .filter(|id| !self.parked_managed_windows.contains(id))
            .filter(|id| self.task_output(WmTaskTarget::X11(ResourceId(**id))) == output)
            .filter_map(|id| self.window_workspaces.get(id).copied())
            .chain(
                self.native_tasks
                    .iter()
                    .filter(|task| self.task_output(WmTaskTarget::Native(task.id)) == output)
                    .map(|task| task.workspace),
            )
            .collect()
    }
    pub fn output_workspace_ids(&self, output: u32) -> Vec<u32> {
        if !self.scrolling_workspaces {
            return (0..self.workspace_count).collect();
        }
        let mut ids = self.occupied_output_workspaces(output);
        let empty = ids.last().map_or(0, |last| last.saturating_add(1)).min(127);
        ids.insert(empty);
        ids.insert(self.output_workspace(output));
        ids.into_iter().collect()
    }
    pub(super) fn wm_visible_workspace_ids(&self) -> Vec<u32> {
        let ids = self.output_workspace_ids(self.wm_shell_output as u32);
        let active = self.output_workspace(self.wm_shell_output as u32);
        let index = ids.iter().position(|id| *id == active).unwrap_or(0);
        let start = index.saturating_sub(1).min(ids.len().saturating_sub(4));
        ids.into_iter().skip(start).take(4).collect()
    }
    pub(super) fn adjacent_workspace(&self, current: u32, next: bool) -> u32 {
        let ids = self.output_workspace_ids(self.wm_shell_output as u32);
        if ids.is_empty() {
            return current;
        }
        let index = ids.iter().position(|id| *id == current).unwrap_or(0);
        let index = if self.scrolling_workspaces {
            if next {
                (index + 1).min(ids.len() - 1)
            } else {
                index.saturating_sub(1)
            }
        } else if next {
            (index + 1) % ids.len()
        } else {
            (index + ids.len() - 1) % ids.len()
        };
        ids[index]
    }
    pub fn workspace_list_field(&self, output: u32, index: u32) -> u32 {
        let ids = self.output_workspace_ids(output);
        if index == u32::MAX {
            ids.len() as u32
        } else {
            ids.get(index as usize).copied().unwrap_or(u32::MAX)
        }
    }
    pub fn refresh_scrolling_workspaces(&mut self) {
        let occupied = self
            .managed_frames
            .keys()
            .filter_map(|id| self.window_workspaces.get(id))
            .copied()
            .chain(self.native_tasks.iter().map(|task| task.workspace))
            .max();
        let selected = if self.scrolling_workspaces {
            self.output_workspaces
                .values()
                .copied()
                .max()
                .unwrap_or(self.current_workspace)
        } else {
            self.current_workspace
        };
        let count = if self.scrolling_workspaces {
            occupied.map_or(1, |last| last.saturating_add(2).min(128))
        } else {
            occupied.map_or(4, |last| last.saturating_add(1).max(4))
        }
        .max(selected + 1);
        if self.workspace_count != count {
            self.workspace_count = count;
            while self.wm_workspace_names.len() < count as usize {
                self.wm_workspace_names
                    .push((self.wm_workspace_names.len() + 1).to_string().into_bytes());
            }
            self.set_wm_property(
                ROOT_WINDOW,
                "_NET_NUMBER_OF_DESKTOPS",
                "CARDINAL",
                32,
                count.to_le_bytes().to_vec(),
            );
        }
    }
    pub(super) fn select_output_workspace(&mut self, output: u32) {
        if !self.scrolling_workspaces {
            return;
        }
        let workspace = self.output_workspace(output);
        if self.current_workspace == workspace {
            return;
        }
        self.current_workspace = workspace;
        self.set_wm_property(
            ROOT_WINDOW,
            "_NET_CURRENT_DESKTOP",
            "CARDINAL",
            32,
            workspace.to_le_bytes().to_vec(),
        );
        self.wm_actions
            .push_back(WM_ACTION_WORKSPACE_PREFIX | workspace);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn server() -> DisplayServer {
        let mut s = DisplayServer::new(DisplayConfig {
            width_px: 1600,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(s.set_output_layout(1600, 600, vec![(0, 0, 800, 600), (800, 0, 800, 600)]));
        s
    }
    fn task(id: u16, workspace: u32) -> WmNativeTask {
        WmNativeTask {
            id,
            workspace,
            icon: 0,
            minimized: false,
            maximized: false,
            active: false,
            title: b"Window".to_vec(),
        }
    }
    #[test]
    fn invalid_tiling_context_cannot_change_the_previous_workspace() {
        let mut s=server();
        s.replace_native_tasks(vec![task(11,0),task(12,0)]);
        s.begin_auto_tile(true);
        s.set_auto_tile_scrolling(true);
        s.add_auto_tile_native(11);s.add_auto_tile_native(12);
        assert_eq!(s.apply_auto_tile(),2);
        let before=s.tile_layouts.get(&0).unwrap().clone();
        s.begin_auto_tile(false);
        s.set_auto_tile_workspace(u32::MAX);
        s.set_auto_tile_scrolling(false);s.set_auto_tile_master(true);
        s.add_auto_tile_native(11);s.set_auto_tile_position(11,900,100);
        assert_eq!(s.apply_auto_tile(),0);
        assert!(s.auto_tile_enabled);
        assert_eq!(s.tile_layouts.get(&0).unwrap().snapshot(|id|match id {WmTaskTarget::Native(n)=>u32::from(n),_=>0}),before.snapshot(|id|match id {WmTaskTarget::Native(n)=>u32::from(n),_=>0}));
        assert!(!s.native_output_positions.contains_key(&11));
        assert!(!s.workspace_window_visible(1,65547,0));
        assert!(!s.workspace_window_visible(2,11,0));
        assert!(!s.workspace_window_visible(1,0,0));
    }
    #[test]
    fn native_tiling_metadata_is_bounded_by_registered_plan_members() {
        let mut s=server();s.begin_auto_tile(true);s.add_auto_tile_native(11);
        for id in 0..1000 {s.set_auto_tile_position(id,100,100);s.set_auto_tile_fullscreen(id);s.set_native_output_position(id,100,100);}
        assert_eq!(s.auto_tile_positions.len(),1);
        assert_eq!(s.auto_tile_fullscreens,vec![11]);
        assert_eq!(s.native_output_positions.len(),256);
        s.set_native_output_position(11,900,100);
        assert_eq!(s.native_output_positions.get(&11),Some(&(900,100)));
    }
    #[test]
    fn each_monitor_has_an_empty_destination_and_independent_selection() {
        let mut s = server();
        s.set_scrolling_workspaces(true);
        assert_eq!(s.output_workspace_ids(0), vec![0]);
        s.set_native_output_position(11, 100, 100);
        s.set_native_output_position(12, 900, 100);
        assert!(s.replace_native_tasks(vec![task(11, 0), task(12, 0)]));
        assert_eq!(s.output_workspace_ids(0), vec![0, 1]);
        assert_eq!(s.output_workspace_ids(1), vec![0, 1]);
        s.switch_workspace(1);
        assert_eq!(s.output_workspace(0), 1);
        assert_eq!(s.output_workspace(1), 0);
        assert!(!s.workspace_window_visible(1, 11, 0));
        assert!(s.workspace_window_visible(1, 12, 0));
        assert!(s.select_shell_output(900, 100));
        assert_eq!(s.current_workspace(), 0);
        assert_eq!(s.output_workspace(0), 1);
        s.switch_workspace(1);
        assert!(!s.workspace_window_visible(1, 12, 0));
    }
    #[test]
    fn empty_workspaces_collapse_without_renumbering_occupied_or_active_ids() {
        let mut s = server();
        s.set_scrolling_workspaces(true);
        s.set_native_output_position(11, 100, 100);
        s.set_native_output_position(12, 100, 100);
        assert!(s.replace_native_tasks(vec![task(11, 0), task(12, 5)]));
        assert_eq!(s.output_workspace_ids(0), vec![0, 5, 6]);
        assert_eq!(s.workspace_count(), 7);
        s.switch_workspace(5);
        assert!(s.replace_native_tasks(vec![task(11, 0)]));
        assert_eq!(s.output_workspace_ids(0), vec![0, 1, 5]);
        s.switch_workspace(1);
        s.refresh_scrolling_workspaces();
        assert_eq!(s.output_workspace_ids(0), vec![0, 1]);
        assert_eq!(s.workspace_count(), 2);
        s.set_scrolling_workspaces(false);
        assert_eq!(s.workspace_count(), 4);
        assert_eq!(s.output_workspace(1), 1);
    }
    #[test]
    fn disconnect_keeps_workspaces_reachable_and_reconnect_restores_selection() {
        let mut s = server();
        s.set_scrolling_workspaces(true);
        s.set_native_output_position(11, 900, 100);
        assert!(s.replace_native_tasks(vec![task(11, 1)]));
        s.select_shell_output(900, 100);
        s.switch_workspace(1);
        assert!(s.set_output_layout(800, 600, vec![(0, 0, 800, 600)]));
        assert_eq!(s.current_workspace(), 0);
        assert!(s.output_workspace_ids(0).contains(&1));
        assert!(s.set_output_layout(1600, 600, vec![(0, 0, 800, 600), (800, 0, 800, 600)]));
        assert_eq!(s.output_workspace(1), 1);
        assert!(s.workspace_window_visible(1, 11, 1));
    }

    #[test]
    fn disabling_dynamic_mode_never_strands_an_occupied_workspace() {
        let mut s = server();
        s.set_scrolling_workspaces(true);
        assert!(s.replace_native_tasks(vec![task(11, 12)]));
        s.switch_workspace(12);
        s.set_scrolling_workspaces(false);
        assert_eq!(s.workspace_count(), 13);
        assert!(s.workspace_window_visible(1, 11, 12));
        s.switch_workspace(0);
        assert!(s.replace_native_tasks(vec![]));
        assert_eq!(s.workspace_count(), 4);
        assert!(!s.replace_native_tasks(vec![task(11, 128)]));
    }
}

#[cfg(test)]
mod taskbar_tests {
    use super::*;
    #[test]
    fn dynamic_taskbar_slots_hit_stable_ids_and_stay_within_each_monitor() {
        let mut s = DisplayServer::new(DisplayConfig {
            width_px: 1600,
            height_px: 700,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(s.set_output_layout(1600, 700, vec![(0, 0, 800, 700), (800, 0, 800, 700)]));
        s.set_shell_monitors(true, true);
        s.set_scrolling_workspaces(true);
        s.wm_gnome_workspaces_visible = true;
        s.replace_tray_items(vec![TrayItem::Workspaces]);
        assert!(s.replace_native_tasks(
            [0, 2, 5, 8, 11, 15]
                .into_iter()
                .enumerate()
                .map(|(i, workspace)| WmNativeTask {
                    id: i as u16 + 11,
                    workspace,
                    icon: 0,
                    minimized: false,
                    maximized: false,
                    active: false,
                    title: b"Window".to_vec()
                })
                .collect()
        ));
        s.switch_workspace(11);
        assert_eq!(s.wm_visible_workspace_ids(), vec![8, 11, 15, 16]);
        assert!(s.wm_text_overlays.iter().any(|text|text.window==WM_TASKBAR&&text.text==b"12"));
        assert_eq!(s.adjacent_workspace(11, true), 15);
        assert_eq!(s.adjacent_workspace(11, false), 8);
        assert_eq!(s.adjacent_workspace(16, true), 16);
        let (_, _, left, width) = s.wm_tray_layout()[0];
        assert!(left + width <= 800);
        let (ox, oy, _, _) = s.wm_taskbar_rect();
        let mut x = left;
        for workspace in s.wm_visible_workspace_ids() {
            assert_eq!(
                s.wm_chrome_hit(WM_TASKBAR, ox + x as i16 + 4, oy + 10),
                Some(WmChromeTarget::Workspace { tray: 0, workspace })
            );
            x += s.wm_workspace_item_width(workspace);
        }
        s.set_taskbar_configuration(true, 2, false);
        let (_, _, top, _) = s.wm_vertical_tray_layout()[0];
        let (ox, oy, _, _) = s.wm_taskbar_rect();
        for (slot, workspace) in s.wm_visible_workspace_ids().into_iter().enumerate() {
            assert_eq!(
                s.wm_chrome_hit(WM_TASKBAR, ox + 10, oy + (top + slot * 28 + 4) as i16),
                Some(WmChromeTarget::Workspace { tray: 0, workspace })
            );
        }
        s.select_shell_output(900, 100);
        assert_eq!(s.wm_visible_workspace_ids(), vec![0]);
        assert!(s.wm_raw_task_targets().is_empty());
    }
}
