//! Shell commands operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// A mapped shell menu owns key transitions before any client or global shortcut.
    pub(super) fn handle_wm_menu_key(&mut self, keycode: u8) {
        let columns = self.wm_menu_columns();
        // Execute the accelerators printed by the window menu against its
        // target, including an inactive window opened through a context menu.
        if self.input_state & 0x000c == 0x0008 {
            let command=columns.iter().flatten().find(|row| {
                    matches!((keycode,*row),
                (70,WmMenuRow::WindowClose) | (75,WmMenuRow::WindowMinimize) |
                (76,WmMenuRow::WindowMaximize | WmMenuRow::WindowRestore)
            )}).cloned();
            if let Some(command)=command {
                self.activate_wm_menu_row(Some(command),None);
                return;
            }
        }
        let column = self.wm_menu_hover_row.as_ref().map_or(columns.len().saturating_sub(1), |row| row.0);
        let Some(rows) = columns.get(column) else { return; };
        let selectable = |row: &WmMenuRow| {
            !matches!(row, WmMenuRow::Divider | WmMenuRow::Empty | WmMenuRow::WorkspaceNavigation)};
        let indices: Vec<usize> = rows.iter().enumerate().filter(|(_, row)| selectable(row)).map(|(i, _)| i).collect();
        if indices.is_empty() { if keycode == 9 { self.dismiss_wm_menu(); } return; }
        let current = self.wm_menu_hover_row.as_ref().and_then(|row| indices.iter().position(|i| *i == row.1));
        match keycode {
            23 | 111 | 116 | 110 | 115 | 112 | 117 => {
                let position = match keycode {
                    110 => 0,
                    115 => indices.len() - 1,
                    23 if self.input_state & 1 != 0 => current.map_or(indices.len() - 1, |i| {
                        (i + indices.len() - 1) % indices.len()}),
                    111 => current.map_or(indices.len() - 1, |i| {
                        (i + indices.len() - 1) % indices.len()}),
                    112 => current.unwrap_or(0).saturating_sub(8),
                    117 => (current.unwrap_or(0) + 8).min(indices.len() - 1),
                    _ => current.map_or(0, |i| (i + 1) % indices.len()),
                };
                let index = indices[position];
                if matches!(rows[index], WmMenuRow::ScrollUp | WmMenuRow::ScrollDown) {
                    self.activate_wm_menu_row(Some(rows[index].clone()), None);
                    let updated = self.wm_menu_columns();
                    if let Some(next) = updated.get(column) {
                        let candidates: Vec<_> = next.iter().enumerate().filter(|(_, row)| {
                                selectable(row) && !matches!(row, WmMenuRow::ScrollUp | WmMenuRow::ScrollDown)}).map(|(i, _)| i).collect();
                        if let Some(index) = if keycode == 111 || keycode == 112 { candidates.first() } else { candidates.last() } {
                            self.select_wm_menu_keyboard_row(column, *index);
                        }
                    }
                } else { self.select_wm_menu_keyboard_row(column, index); }
            }
            36 | 65 | 114 => {
                let index = current.map_or(indices[0], |i| indices[i]);
                let row = rows[index].clone();
                let submenu = wm_menu_row_has_submenu(&row);
                if keycode == 114 && !submenu { return; }
                if submenu {
                    self.wm_menu_keyboard_parents.push((self.wm_menu_level, self.wm_menu_offset, (column, index, row.clone()),
                    ));
                }
                self.activate_wm_menu_row(Some(row), None);
                if submenu {
                    let children = self.wm_menu_columns();
                    let last = children.len().saturating_sub(1);
                    if let Some(index) = children.get(last).and_then(|rows| rows.iter().position(selectable)) {
                        self.select_wm_menu_keyboard_row(last, index);
                    }
                }
            }
            113 | 9 => {
                if let Some((level, offset, selection)) = self.wm_menu_keyboard_parents.pop() {
                    self.wm_menu_level = level;
                    self.wm_menu_offset = offset;
                    self.relayout_wm_menu();
                    self.select_wm_menu_keyboard_row(selection.0, selection.1);
                } else { self.dismiss_wm_menu(); }
            }
            _ => {
                let symbol = default_keysyms(keycode)[0];
                if let Some(ch) = char::from_u32(symbol).filter(char::is_ascii_alphanumeric) {
                    let first = current.map_or(0, |i| i + 1);
                    for offset in 0..indices.len() {
                        let index = indices[(first + offset) % indices.len()];
                        let label: &[u8] = match &rows[index] {
                            WmMenuRow::Application(app) => &app.label,
                            WmMenuRow::MenuFolder(folder) => self.wm_menu_folders.get(usize::from(*folder)).map_or(b"", |f| f.label.as_slice()),
                            WmMenuRow::Category(category) => wm_category_label(*category),
                            WmMenuRow::Theme(theme) => self.wm_themes.get(usize::from(*theme)).map_or(b"", |theme| theme.name.as_slice()),
                            WmMenuRow::Applications => rice_i18n::t("Applications").as_bytes(),
                            WmMenuRow::Themes => rice_i18n::t("Themes").as_bytes(),
                            WmMenuRow::Run => b"Run",
                            WmMenuRow::ThemeCustomize => rice_i18n::t("Customize").as_bytes(),
                            WmMenuRow::Exit => rice_i18n::t("Exit").as_bytes(),
                            _ => b"",
                        };
                        if label.first().is_some_and(|byte| byte.eq_ignore_ascii_case(&(ch as u8))) {
                            self.select_wm_menu_keyboard_row(column, index); break;
                        }
                    }
                }
            }
        }
    }

    pub(super) fn handle_wm_shortcut(&mut self, keycode: u8, state: u16) -> bool {
        let alt = state & 0x0008 != 0;
        let control = state & 0x0004 != 0;
        let shift = state & 0x0001 != 0;
        match (keycode, alt, control, shift) {
            (27, true, false, _) => {
                self.wm_actions.push_back(WM_ACTION_RUN);
                true
            }
            (40, true, false, _) => {
                self.toggle_show_desktop();
                true
            }
            (23, true, _, _) => {
                self.cycle_managed_focus();
                true
            }
            (70, true, false, _) => {
                if let Some(target) = self.active_wm_task_target() {
                    self.handle_wm_task_command(target, WmMenuRow::WindowClose);
                }
                true
            }
            (75, true, false, _) => {
                if let Some(target) = self.active_wm_task_target() {
                    self.handle_wm_task_command(target, WmMenuRow::WindowMinimize);
                }
                true
            }
            (76, true, false, _) => {
                if let Some(target) = self.active_wm_task_target() {
                    let maximized = match target {
                        WmTaskTarget::X11(client) => {
                            self.maximized_geometries.contains_key(&client.0)
                        }
                        WmTaskTarget::Native(id) => self
                            .native_tasks
                            .iter()
                            .any(|task| task.id == id && task.maximized),
                        WmTaskTarget::Group(_) => false,
                    };
                    self.handle_wm_task_command(
                        target,
                        if maximized {
                            WmMenuRow::WindowRestore
                        } else {
                            WmMenuRow::WindowMaximize
                        },
                    );
                }
                true
            }
            (113, true, true, _) => {
                self.switch_workspace(
                    self.adjacent_workspace(self.current_workspace,false),
                );
                true
            }
            (114, true, true, _) => {
                self.switch_workspace(self.adjacent_workspace(self.current_workspace,true));
                true
            }
            (112, true, false, false) => {
                self.move_active_window_to_adjacent_workspace(false);
                true
            }
            (117, true, false, false) => {
                self.move_active_window_to_adjacent_workspace(true);
                true
            }
            (113, true, false, true) => {
                self.snap_active_window(WmSnapTarget::Left);
                true
            }
            (114, true, false, true) => {
                self.snap_active_window(WmSnapTarget::Right);
                true
            }
            (111, true, false, true) => {
                self.snap_active_window(WmSnapTarget::Maximize);
                true
            }
            (133 | 134, _, _, _) => {
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
                    self.relayout_wm_menu();
                    self.restack(WM_MENU, None, Some(0));
                }
                self.rebuild_scene();
                self.scene_damage(WM_MENU);
                true
            }
            _ => false,
        }
    }
}
