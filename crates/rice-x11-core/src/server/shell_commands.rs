//! Shell commands operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_wm_pointer_press(&mut self, source: ResourceId, root_x: i16, root_y: i16) -> bool {
        if self.wm_dock_style && source==WM_TASK_PREVIEW {
            if let Some(preview)=self.windows.get(&WM_TASK_PREVIEW.0) {
                let id=preview.background_pixel;
                let target=if id & WM_TASK_PREVIEW_NATIVE_PREFIX != 0 {WmTaskTarget::Native((id & 0xffff) as u16)} else {WmTaskTarget::X11(ResourceId(id))};
                let local_x=root_x-preview.x;let local_y=root_y-preview.y;
                let top=if self.wm_taskbar_edge==WmTaskbarEdge::Top {22}else{10};
                if local_x>=preview.width as i16-44 && local_x<preview.width as i16-20 && (top..top+24).contains(&local_y) {self.handle_wm_task_command(target,WmMenuRow::WindowClose);}
                else {match target {WmTaskTarget::Native(id)=>{
                            self.wm_actions.push_back(0xffe6_0000|u32::from(id))}
                        WmTaskTarget::X11(client)=>self.activate_managed_window(client),_=>{}}}
            }
            self.set_wm_task_preview(None,root_x);return true;
        }
        self.set_wm_task_preview(None, root_x);
        if source == WM_GNOME_PANEL {
            match self.wm_chrome_hit(source, root_x, root_y) {
                Some(WmChromeTarget::SystemControls) => {
                    self.wm_actions.push_back(WM_ACTION_SYSTEM_CONTROLS);
                }
                Some(WmChromeTarget::RiceBox) => {
                    self.wm_actions.push_back(WM_ACTION_RICE_BOX);
                }
                _ => {}
            }
            return true;
        }
        if source == WM_NOTIFICATION {
            self.clear_wm_notification();
            return true;
        }
        let start_button = source == WM_TASKBAR
            && matches!(
                self.wm_chrome_hit(source, root_x, root_y),
                Some(WmChromeTarget::Start)
            );
        if source != WM_MENU
            && !start_button
            && self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped)
        {
            if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                menu.mapped = false;
            }
            self.wm_menu_hover_row = None;
            self.scene_upsert(WM_MENU);
            self.scene_damage(WM_MENU);
            self.update_taskbar_pointer(root_x, root_y);
        }
        if source == WM_TASKBAR {
            if self.wm_dock_style || self.wm_taskbar_edge.vertical() {
                let target = self.wm_chrome_hit(source, root_x, root_y);
                match target {
                    Some(WmChromeTarget::SystemControls) => {
                        self.wm_actions.push_back(WM_ACTION_SYSTEM_CONTROLS);
                    }
                    Some(WmChromeTarget::Start) => {
                        let mapped = if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                            menu.mapped = !menu.mapped;
                            menu.mapped
                        } else {
                            false
                        };
                        if mapped {
                            self.wm_menu_window_anchor = None;
                            self.wm_menu_level = self.wm_menu_initial_level();
                            self.wm_menu_offset = 0;
                            self.wm_menu_hover_row = None;
                            self.relayout_wm_menu();
                            self.restack(WM_MENU, None, Some(0));
                        }
                        self.repaint_wm_chrome();
                    }
                    Some(WmChromeTarget::Activities) => { self.wm_actions.push_back(0xffff_ff0b); }
                    Some(WmChromeTarget::QuickLaunch(index)) => {
                        if self.wm_dock_style && self.input_state & 1 != 0 {
                            let task=self.wm_dock_launcher_task(index).and_then(|task|match task {WmTaskTarget::Group(group)=>{
                                            self.wm_task_group(group).first().copied()}
                                        _=>Some(task),
                                    });
                            if let Some(WmTaskTarget::Native(id))=task {self.wm_actions.push_back(0xffe8_0000 | u32::from(id));return true;}
                        }
                        if self.wm_dock_style && self.input_state & 1 == 0 {
                            if let Some(task) = self.wm_dock_launcher_task(index) {
                                if let WmTaskTarget::Group(group) = task { self.show_wm_task_menu(WmMenuLevel::TaskGroup(group),root_x,root_y,
                                    ); }
                                else { self.activate_wm_task_target(task); }
                                self.rebuild_scene(); return true;
                            }
                        }
                        if let Some(launcher) = self.wm_quick_launchers.get(usize::from(index)) {
                            if launcher.action == WM_ACTION_SHOW_DESKTOP {
                                self.toggle_show_desktop();
                            } else {
                                self.wm_actions.push_back(launcher.action);
                            }
                        }
                    }
                    Some(WmChromeTarget::Task(target)) => {
                        if self.input_state & 1 != 0 {
                            self.request_wm_new_instance(target);
                        } else if let WmTaskTarget::Group(group) = target {
                            self.show_wm_task_menu(WmMenuLevel::TaskGroup(group), root_x, root_y);
                        } else {
                            self.activate_wm_task_target(target);
                        }
                    }
                    Some(target) => match self.wm_tray_target_item(target) {
                        Some(TrayItem::Sound) => {
                            self.set_wm_volume(self.wm_volume, !self.wm_volume_muted);
                        }
                        Some(TrayItem::Workspaces) => {
                            if let WmChromeTarget::Workspace { workspace, .. } = target {
                                self.switch_workspace(workspace);
                            }
                        }
                        Some(TrayItem::Launcher(launcher)) => {
                            self.wm_actions.push_back(match launcher.target {
                                LauncherTarget::Files => WM_ACTION_PCMANFM,
                                LauncherTarget::Terminal => WM_ACTION_TERMINAL,
                                LauncherTarget::ImageViewer => WM_ACTION_IMAGE_VIEWER,
                            })
                        }
                        Some(TrayItem::Connectivity) => {
                            self.wm_actions.push_back(WM_ACTION_NETWORK_PREFERENCES);
                        }
                        Some(TrayItem::Clock | TrayItem::Performance) | None => {}
                    },
                    None => {
                        self.wm_taskbar_drag = Some(WmTaskbarDrag {
                            pointer_x: root_x,
                            pointer_y: root_y,
                            candidate: self.wm_taskbar_edge,
                            moved: false,
                        });
                    }
                }
                self.rebuild_scene();
                return true;
            }
            let taskbar_x = root_x.saturating_sub(self.wm_taskbar_rect().0);
            if taskbar_x < self.wm_start_button_width() {
                let mapped = if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                    menu.mapped = !menu.mapped;
                    menu.mapped
                } else {
                    false
                };
                if mapped {
                    self.wm_menu_keyboard_parents.clear();
                    self.wm_menu_window_anchor = None;
                    self.wm_menu_level = self.wm_menu_initial_level();
                    self.wm_menu_offset = 0;
                    self.wm_menu_hover_row = None;
                    if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                        menu.x = 4;
                    }
                    self.relayout_wm_menu();
                    self.restack(WM_MENU, None, Some(0));
                }
                self.paint_wm_chrome(WM_TASKBAR);
                self.scene_upsert(WM_TASKBAR);
                self.rebuild_scene();
                self.scene_damage(WM_MENU);
                return true;
            }
            let task_button_x = self.wm_task_button_x();
            let quick_launch_x = self.wm_quick_launch_x();
            if self.wm_quick_launch_visible && (quick_launch_x..task_button_x).contains(&taskbar_x) {
                let quick_launch = usize::try_from((taskbar_x - quick_launch_x) / 28).unwrap_or(0);
                if let Some(launcher) = self.wm_quick_launchers.get(quick_launch) {
                    if launcher.action == WM_ACTION_SHOW_DESKTOP {
                        self.toggle_show_desktop();
                    } else {
                        self.wm_actions.push_back(launcher.action);
                    }
                }
                return true;
            }
            if let Some(target) = self.wm_chrome_hit(source, root_x, root_y) {
                match (target, self.wm_tray_target_item(target)) {
                    (WmChromeTarget::SystemControls, _) => {
                        self.wm_actions.push_back(WM_ACTION_SYSTEM_CONTROLS);
                        return true;
                    }
                    (_, Some(TrayItem::Sound)) => {
                        self.set_wm_volume(self.wm_volume, !self.wm_volume_muted);
                        return true;
                    }
                    (WmChromeTarget::Workspace { workspace, .. }, Some(TrayItem::Workspaces)) => {
                        self.switch_workspace(workspace);
                        return true;
                    }
                    (_, Some(TrayItem::Launcher(launcher))) => {
                        self.wm_actions.push_back(match launcher.target {
                            LauncherTarget::Files => WM_ACTION_PCMANFM,
                            LauncherTarget::Terminal => WM_ACTION_TERMINAL,
                            LauncherTarget::ImageViewer => WM_ACTION_IMAGE_VIEWER,
                        });
                        return true;
                    }
                    (_, Some(TrayItem::Connectivity)) => {
                        self.wm_actions.push_back(WM_ACTION_NETWORK_PREFERENCES);
                        return true;
                    }
                    (_, Some(TrayItem::Clock)) => return true,
                    _ => {}
                }
            }
            let index = usize::try_from(
                i32::from(taskbar_x).saturating_sub(i32::from(task_button_x))
                    / self.wm_task_slot_width(),
            )
            .ok();
            let candidates = self.wm_task_targets();
            if let Some(target) = index.and_then(|index| candidates.get(index)).copied() {
                if let WmTaskTarget::Group(group) = target {
                    self.show_wm_task_menu(WmMenuLevel::TaskGroup(group), root_x, root_y);
                } else {
                    self.activate_wm_task_target(target);
                }
            } else if self.wm_chrome_hit(source, root_x, root_y).is_none() {
                self.wm_taskbar_drag = Some(WmTaskbarDrag {
                    pointer_x: root_x,
                    pointer_y: root_y,
                    candidate: self.wm_taskbar_edge,
                    moved: false,
                });
            }
            return true;
        }
        if source == WM_MENU {
            let selected = self.wm_menu_hit(root_x, root_y);
            self.activate_wm_menu_row(selected.map(|(_, _, entry)| entry), Some((root_x, root_y)));
            return true;
        }
        let Some(client) = self.frame_clients.get(&source.0).copied() else {
            return false;
        };
        let (origin_x, origin_y) = self.window_origin(source);
        let local_x = i32::from(root_x).saturating_sub(origin_x);
        let local_y = i32::from(root_y).saturating_sub(origin_y);
        let frame = self.windows.get(&source.0).cloned();
        match self.wm_chrome_hit(source, root_x, root_y) {
            Some(WmChromeTarget::FrameIcon(frame)) if frame == source => {
                self.activate_managed_window(client);
                let profile = self
                    .wm_themes
                    .get(usize::from(self.wm_theme))
                    .map_or(WmChromeProfile::Standard, |theme| theme.chrome_profile);
                self.show_wm_window_menu(
                    WmTaskTarget::X11(client),
                    clamp_i16(origin_x.saturating_add(
                        i32::try_from(wm_frame_icon_left(profile)).unwrap_or_default(),
                    )),
                    clamp_i16(origin_y.saturating_add(i32::from(WM_TITLE_HEIGHT))),
                );
                return true;
            }
            Some(WmChromeTarget::FrameControl(frame, control)) if frame == source => {
                match control {
                    0 => self.minimize_managed_window(client),
                    1 => self.toggle_maximized(client, 2),
                    2 => self.request_close_window(client),
                    _ => return false,
                }
                return true;
            }
            _ => {}
        }
        if self.tile_overflow()&&self.live_tile_target(0,client.0).is_some() {
            self.activate_managed_window(client);
            return true;
        }
        let resize_edges = frame.as_ref().map_or(0, |frame| {
            let border = WM_RESIZE_HIT_WIDTH;
            let mut edges = 0;
            if local_x < border {
                edges |= WM_RESIZE_LEFT;
            } else if local_x >= i32::from(frame.width).saturating_sub(border) {
                edges |= WM_RESIZE_RIGHT;
            }
            if local_y < border {
                edges |= WM_RESIZE_TOP;
            } else if local_y >= i32::from(frame.height).saturating_sub(border) {
                edges |= WM_RESIZE_BOTTOM;
            }
            edges
        });
        if resize_edges != 0
            && !self.non_resizable_managed_windows.contains(&client.0)
            && !self.maximized_geometries.contains_key(&client.0)
            && let (Some(frame), Some(client_state)) =
                (frame.as_ref(), self.windows.get(&client.0).cloned())
        {
            self.activate_managed_window(client);
            let current_geometry = (frame.x, frame.y, client_state.width, client_state.height);
            self.wm_drag = Some(WmDrag {
                frame: source,
                client,
                pointer_x: root_x,
                pointer_y: root_y,
                frame_x: frame.x,
                frame_y: frame.y,
                restore_geometry: current_geometry,
                resize_geometry: current_geometry,
                resize_edges,
                moved: false,
                restore_on_move: false,
            });
            return true;
        }
        self.activate_managed_window(client);
        let (_, _, title_height, _) = self.managed_frame_insets(client);
        if title_height > WM_BORDER_WIDTH && local_y < i32::from(title_height)
            && let Some(frame) = frame
        {
            let restore_geometry = self
                .snapped_geometries
                .get(&client.0)
                .map(|state| state.restore_geometry)
                .or_else(|| self.maximized_geometries.get(&client.0).copied())
                .or_else(|| {
                    self.windows.get(&client.0).map(|client_state| {
                        (frame.x, frame.y, client_state.width, client_state.height)
                    })
                })
                .unwrap_or((frame.x, frame.y, 1, 1));
            self.wm_drag = Some(WmDrag {
                frame: source,
                client,
                pointer_x: root_x,
                pointer_y: root_y,
                frame_x: frame.x,
                frame_y: frame.y,
                restore_geometry,
                resize_geometry: (frame.x, frame.y, frame.width, frame.height),
                resize_edges: 0,
                moved: false,
                restore_on_move: self.maximized_geometries.contains_key(&client.0),
            });
        }
        true
    }

    pub(super) fn handle_wm_double_click(&mut self, source: ResourceId, root_x: i16, root_y: i16) {
        if source == WM_TASKBAR
            && self
                .wm_chrome_hit(source, root_x, root_y)
                .and_then(|target| self.wm_tray_target_item(target))
                == Some(TrayItem::Performance)
        {
            self.wm_actions.push_back(WM_ACTION_PERFORMANCE);
            return;
        }
        let Some(client) = self.frame_clients.get(&source.0).copied() else {
            return;
        };
        let (_, origin_y) = self.window_origin(source);
        let local_y = i32::from(root_y).saturating_sub(origin_y);
        let Some(_) = self.windows.get(&source.0) else {
            return;
        };
        let (_, _, title_height, _) = self.managed_frame_insets(client);
        if title_height > WM_BORDER_WIDTH && (0..i32::from(title_height)).contains(&local_y)
            && self.wm_chrome_hit(source, root_x, root_y).is_none()
        {
            self.wm_drag = None;
            self.toggle_maximized(client, 2);
        }
    }

    /// Pointer and keyboard activation share the same action dispatch.
    pub(super) fn activate_wm_menu_row(&mut self, selected: Option<WmMenuRow>, pointer: Option<(i16, i16)>) {
            let mut keep_open = false;
            match selected {
                Some(WmMenuRow::Applications) => {
                    self.wm_menu_level = if self.wm_menu_structured {
                        WmMenuLevel::MenuFolders
                    } else {
                        WmMenuLevel::Categories
                    };
                    self.wm_menu_offset = 0;
                    self.wm_menu_hover_row = None;
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::Run) => {
                    self.wm_actions.push_back(WM_ACTION_RUN);
                }
                Some(WmMenuRow::Category(category)) => {
                    self.wm_menu_level = WmMenuLevel::Applications(category);
                    self.wm_menu_offset = 0;
                    self.wm_menu_hover_row = None;
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::MenuFolder(folder)) => {
                    self.wm_menu_level = WmMenuLevel::MenuFolder(folder);
                    self.wm_menu_offset = 0;
                    self.wm_menu_hover_row = None;
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::Themes) => {
                    self.wm_menu_level = WmMenuLevel::Themes;
                    self.wm_menu_offset = 0;
                    self.wm_menu_hover_row = None;
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::Theme(theme)) => {
                    if usize::from(theme) < self.wm_themes.len() {
                        self.wm_theme = theme;
                        self.publish_gtk_theme_settings();
                        self.repaint_wm_chrome();
                        self.wm_actions
                            .push_back(WM_ACTION_THEME_PREFIX | u32::from(theme));
                    }
                }
                Some(WmMenuRow::ThemeCustomize) => {
                    self.wm_actions.push_back(WM_ACTION_THEME_CUSTOMIZE);
                }
                Some(WmMenuRow::Application(application)) => {
                    self.wm_actions.push_back(application.action);
                }
                Some(WmMenuRow::ApplicationLaunch) => {
                    if let WmMenuLevel::ApplicationContext(action) = self.wm_menu_level {
                        self.wm_actions.push_back(action);
                    }
                }
                Some(WmMenuRow::ApplicationOpenFolder) => {
                    if let WmMenuLevel::ApplicationContext(action) = self.wm_menu_level {
                        self.wm_actions
                            .push_back(WM_ACTION_MENU_OPEN_FOLDER_PREFIX | (action & 0x00ff_ffff));
                    }
                }
                Some(WmMenuRow::ApplicationUninstall) => {
                    if let WmMenuLevel::ApplicationContext(action) = self.wm_menu_level
                        && self
                            .wm_menu_entries
                            .iter()
                            .any(|entry| entry.action == action && entry.uninstallable)
                    {
                        self.wm_actions
                            .push_back(WM_ACTION_MENU_UNINSTALL_PREFIX | (action & 0x00ff_ffff));
                    }
                }
                Some(WmMenuRow::MenuOpenDirectory) => {
                    if self.wm_menu_level == WmMenuLevel::MenuDirectoryContext {
                        self.wm_actions.push_back(WM_ACTION_MENU_OPEN_DIRECTORY);
                    }
                }
                Some(WmMenuRow::Task(target)) => {
                    if self.wm_dock_style && matches!(self.wm_menu_level,WmMenuLevel::TaskGroup(_)) {
                        let close=pointer.is_some_and(|(root_x, root_y)| {
                        self.wm_menu_hit(root_x,root_y).is_some_and(|(column,row,_)| {
                            let menu=&self.windows[&WM_MENU.0];
                            let x=i32::from(menu.x)+self.wm_menu_column_x_offsets()[column] as i32+self.wm_menu_column_widths()[column] as i32-38;
                            let y=i32::from(menu.y)+self.wm_menu_column_offsets()[column] as i32+(row*self.wm_menu_base_row_height()) as i32+11;
                            (x..x+24).contains(&i32::from(root_x)) && (y..y+24).contains(&i32::from(root_y))
                        })});
                        if close { self.handle_wm_task_command(target,WmMenuRow::WindowClose); }
                        else {match target {WmTaskTarget::Native(id)=>{
                                self.wm_actions.push_back(0xffe6_0000|u32::from(id))}
                            WmTaskTarget::X11(client)=>self.activate_managed_window(client),_=>{}}}
                    } else { self.activate_wm_task_target(target); }
                }
                Some(
                    row @ (WmMenuRow::WindowClose
                    | WmMenuRow::WindowMaximize
                    | WmMenuRow::WindowMinimize
                    | WmMenuRow::WindowRestore
                    | WmMenuRow::StackClose
                    | WmMenuRow::StackTile
                    | WmMenuRow::StackCascade),
                ) => match self.wm_menu_level {
                    WmMenuLevel::TaskContext(target) => {
                        self.handle_wm_task_command(target, row);
                    }
                    WmMenuLevel::TaskbarContext
                        if matches!(row, WmMenuRow::StackTile | WmMenuRow::StackCascade) =>
                    {
                        self.arrange_all_task_windows(row == WmMenuRow::StackTile);
                    }
                    _ => {}
                },
                Some(WmMenuRow::ScrollDown) => {
                    let maximum_offset = match self.wm_menu_level {
                        WmMenuLevel::Applications(category) => self.wm_application_max_offset(category)
                        ,
                    WmMenuLevel::Themes => self.wm_theme_max_offset(),
                        WmMenuLevel::TaskGroup(group) => self.wm_task_group_max_offset(group),
                        _ => 0,
                    };
                    self.wm_menu_offset = self.wm_menu_offset.saturating_add(1).min(maximum_offset);
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::ScrollUp) => {
                    self.wm_menu_offset = self.wm_menu_offset.saturating_sub(1);
                    self.relayout_wm_menu();
                    keep_open = true;
                }
                Some(WmMenuRow::RestoreWindows) => {
                    self.restore_all_windows();
                }
                Some(WmMenuRow::ShowDesktop) => self.toggle_show_desktop(),
                Some(WmMenuRow::ToolbarPreferences | WmMenuRow::DockSettings) => {
                    self.wm_actions.push_back(WM_ACTION_TOOLBAR_PREFERENCES);
                }
                Some(WmMenuRow::QuickShortcutRemove) => {
                    if let Some(index) = self.wm_quick_context_index {
                        self.wm_actions.push_back(WM_ACTION_QUICK_REMOVE_PREFIX | u32::from(index));
                    }
                }
                Some(WmMenuRow::QuickShortcutPreferences) => {
                    self.wm_actions
                        .push_back(WM_ACTION_QUICK_SHORTCUT_PREFERENCES);
                }
                Some(WmMenuRow::NextWorkspace) => {
                    self.switch_workspace(self.adjacent_workspace(self.current_workspace,true));
                }
                Some(WmMenuRow::PreviousWorkspace) => self.switch_workspace(
                    self.adjacent_workspace(self.current_workspace,false),
                ),
                Some(WmMenuRow::Exit) => {
                    self.wm_actions.push_back(WM_ACTION_EXIT);
                }
                Some(WmMenuRow::RedmondShortcut(shortcut)) => match shortcut {
                    RedmondMenuShortcut::Documents => {
                        self.wm_actions.push_back(WM_ACTION_PCMANFM);
                    }
                },
                Some(WmMenuRow::Divider | WmMenuRow::WorkspaceNavigation) => {
                    keep_open = true;
                }
                Some(WmMenuRow::Empty) | None => {}
            }
            if !keep_open {
                if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                    menu.mapped = false;
                }
                self.wm_menu_hover_row = None;
                self.scene_upsert(WM_MENU);
                self.update_taskbar_pointer(self.pointer_x, self.pointer_y);
            }
    }

    pub(super) fn select_wm_menu_keyboard_row(&mut self, column: usize, index: usize) {
        if let Some(row) = self.wm_menu_columns().get(column).and_then(|rows| rows.get(index)).cloned() {
            self.wm_menu_hover_row = Some((column, index, row));
            self.paint_wm_chrome(WM_MENU);
            self.scene_upsert(WM_MENU);
        }
    }
}
