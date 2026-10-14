//! Pointer input operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Inject a browser pointer button transition into the native X11 event stream.
    pub fn inject_pointer_button(
        &mut self,
        pressed: bool,
        button: u8,
        root_x: i16,
        root_y: i16,
        state: u16,
    ) {
        self.select_shell_output(root_x, root_y);
        self.release_stale_input_grabs();
        self.pointer_x = root_x;
        self.pointer_y = root_y;
        self.input_state = state;
        if button == 1
            && !pressed
            && let Some(drag) = self.wm_taskbar_drag.take()
        {
            self.set_wm_taskbar_drag_preview(None);
            if drag.moved && drag.candidate != self.wm_taskbar_edge {
                let edge = drag.candidate;
                if self.set_taskbar_configuration(
                    self.wm_taskbar_visible,
                    u8::try_from(edge.wire()).unwrap_or_default(),
                    self.wm_taskbar_auto_hide,
                ) {
                    self.wm_actions
                        .push_back(WM_ACTION_TASKBAR_EDGE_PREFIX | edge.wire());
                }
            }
            self.input_state &= !0x0100;
            return;
        }
        let source = self.hit_test(root_x, root_y);
        // A new press on compositor-owned chrome must remain usable even when
        // an application holds an explicit X11/XI2 pointer grab (SDL games do
        // this for relative mouse mode). Releases from an already-started
        // guest gesture still follow its grab, preserving press/release
        // ownership when the pointer crosses a decoration mid-gesture.
        if (pressed || self.pointer_grab.is_none())
            && self.handle_native_pointer_button(pressed, button, source, root_x, root_y)
        {
            return;
        }
        if pressed {
            self.set_focus(source);
            self.notify_x11_foreground(source);
        }
        let mask = if pressed {
            BUTTON_PRESS_MASK
        } else {
            BUTTON_RELEASE_MASK
        };
        // Wheel buttons are the core X11 scroll protocol. Chromium advertises
        // XI2 button masks as well, but treats XI_ButtonPress(4/5) as physical
        // buttons rather than scroll unless a smooth-scroll valuator is also
        // present. Deliver wheel transitions through the universally supported
        // core path; keep XI2 as the exclusive path for ordinary buttons so a
        // click still has exactly one representation.
        let xinput_targets = if matches!(button, 4 | 5) {
            BTreeSet::new()
        } else {
            self.emit_xinput_device_event(
                source,
                if pressed { 4 } else { 5 },
                u32::from(button),
                root_x,
                root_y,
                state,
                true,
            )
        };
        let target_owner = self.input_resource_owner(source);
        if xinput_targets.is_empty()
            || target_owner.is_some_and(|owner| !xinput_targets.contains(&owner))
        {
            self.emit_pointer_input(
                PointerInput {
                    source,
                    mask,
                    detail: button,
                    root_x,
                    root_y,
                    state,
                },
                target_owner.filter(|owner| !xinput_targets.is_empty() && !xinput_targets.contains(owner)),
                move |output, order, event| {
                    if pressed {
                        x11::encode_button_press_event(output, order, event);
                    } else {
                        x11::encode_button_release_event(output, order, event);
                    }
                },
            );
        }
        if pressed && self.pointer_grab.is_none() {
            let owner = self.propagated_targets(source, BUTTON_PRESS_MASK).and_then(
                |(window, _, targets)| targets.first().copied().map(|target| (target.0, window)),
            );
            if let Some((owner, window)) = owner {
                self.pointer_grab = Some(ActivePointerGrab {
                    owner,
                    window,
                    owner_events: true,
                    event_mask: u16::try_from(
                        BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK | POINTER_MOTION_MASK,
                    )
                    .unwrap_or(u16::MAX),
                    automatic: true,
                });
            }
        }
        if (1..=5).contains(&button) {
            let bit = 1u16 << (u16::from(button) + 7);
            if pressed {
                self.input_state |= bit;
            } else {
                self.input_state &= !bit;
            }
        }
        if !pressed
            && self
                .pointer_grab
                .is_some_and(|grab| grab.automatic && self.input_state & 0x1f00 == 0)
        {
            self.pointer_grab = None;
        }
    }

    pub(super) fn handle_native_pointer_button(
        &mut self,
        pressed: bool,
        button: u8,
        source: ResourceId,
        root_x: i16,
        root_y: i16,
    ) -> bool {
        if !self.config.native_window_manager {
            return false;
        }
        if source == WM_TASKBAR && let Some(slot) = self.notification_icon_at(root_x, root_y) {
            if matches!(button, 1..=3) {
                let event = match (button, pressed) { (1,true)=>0x201,(1,false)=>0x202,(2,true)=>0x207,(2,false)=>0x208,(3,true)=>0x204,_=>0x205 };
                self.push_notification_event(slot, event, root_x, root_y);
            }
            return true;
        }
        if pressed { self.wm_notification_focus = None; }
        let source = self
            .managed_resize_frame_at(source, root_x, root_y)
            .map_or(source, |(frame, _)| frame);
        // A mapped EWMH-modal ICCCM transient disables its owner for the
        // modal lifetime. WM_TRANSIENT_FOR alone also represents nonmodal
        // owned windows (for example help/tool windows), which remain grouped
        // and stacked with their owner but do not consume owner input.
        // Consume input anywhere in that owner's managed subtree, including
        // its compositor-owned title bar, and bring the owned dialog back to
        // the foreground. Shell chrome remains independently interactive.
        if let Some(owner) = self.managed_client_ancestor(source)
            && let Some(transient) = self.topmost_managed_modal_transient(owner)
        {
            if pressed {
                self.activate_managed_window(transient);
            }
            return true;
        }
        let on_gnome_panel = self.windows.get(&WM_GNOME_PANEL.0).is_some_and(|panel| {
            panel.mapped
                && (i32::from(panel.x)..i32::from(panel.x) + i32::from(panel.width))
                    .contains(&i32::from(root_x))
                && (i32::from(panel.y)..i32::from(panel.y) + i32::from(panel.height))
                    .contains(&i32::from(root_y))
        });
        if source == WM_GNOME_PANEL || on_gnome_panel {
            if pressed {
                match self.wm_chrome_hit(WM_GNOME_PANEL, root_x, root_y) {
                    Some(WmChromeTarget::SystemControls) => {
                        self.wm_actions.push_back(WM_ACTION_SYSTEM_CONTROLS);
                        return true;
                    }
                    Some(WmChromeTarget::RiceBox) if matches!(button, 1 | 2 | 3) => {
                        self.wm_actions.push_back(if button == 3 {
                            WM_ACTION_RICE_BOX_COMPOSE
                        } else {
                            WM_ACTION_RICE_BOX
                        });
                        return true;
                    }
                    _ => {}
                }
            }
            if let Some(indicator) = pressed
                .then(|| self.wm_gnome_indicator_at(root_x))
                .flatten()
            {
                let action = match button {
                    1 | 2 | 3 => Some(
                        WM_ACTION_GNOME_INDICATOR_PREFIX
                            | (u32::from(button) << 8)
                            | u32::from(indicator),
                    ),
                    _ => None,
                };
                if let Some(action) = action {
                    self.wm_actions.push_back(action);
                }
            }
            return true;
        }
        if source == WM_GNOME_MENU && matches!(button, 1 | 2 | 3) {
            if pressed && let Some(index) = self.wm_gnome_menu_hit(root_y) {
                self.wm_actions.push_back(
                    WM_ACTION_GNOME_MENU_ITEM_PREFIX | (u32::from(button) << 8) | u32::from(index),
                );
            }
            return true;
        }
        if source == WM_GNOME_DIALOG && matches!(button, 1 | 3) {
            if pressed && let Some(index) = self.wm_gnome_dialog_button_at(root_x, root_y) {
                self.wm_actions.push_back(
                    WM_ACTION_GNOME_DIALOG_PREFIX | (u32::from(button) << 8) | u32::from(index),
                );
            }
            return true;
        }
        if button == 6 {
            if pressed {
                self.handle_wm_double_click(source, root_x, root_y);
            }
            return true;
        }
        if source == WM_MENU && matches!(button, 4 | 5) {
            if pressed {
                let maximum_offset = match self.wm_menu_level {
                    WmMenuLevel::Applications(category) => self.wm_application_max_offset(category),
                    WmMenuLevel::Themes => self.wm_theme_max_offset(),
                    _ => 0,
                };
                if button == 4 {
                    self.wm_menu_offset = self.wm_menu_offset.saturating_sub(1);
                } else {
                    self.wm_menu_offset = self.wm_menu_offset.saturating_add(1).min(maximum_offset);
                }
                self.wm_menu_hover_row = None;
                self.relayout_wm_menu();
            }
            return true;
        }
        if source == WM_TASKBAR
            && self
                .wm_chrome_hit(source, root_x, root_y)
                .and_then(|target| self.wm_tray_target_item(target))
                == Some(TrayItem::Sound)
            && matches!(button, 4 | 5)
        {
            if pressed {
                let volume = if button == 4 {
                    self.wm_volume.saturating_add(5).min(100)
                } else {
                    self.wm_volume.saturating_sub(5)
                };
                self.set_wm_volume(volume, false);
            }
            return true;
        }
        if button == 3 && source == WM_TASKBAR {
            if pressed {
                match self.wm_chrome_hit(source, root_x, root_y) {
                    Some(WmChromeTarget::Start) => {
                        self.set_wm_task_preview(None, root_x);
                        self.wm_application_context_columns.clear();
                        self.wm_application_context_widths.clear();
                        self.wm_application_context_offsets.clear();
                        self.wm_application_context_x = 0;
                        self.wm_application_context_y = 0;
                        self.wm_application_context_parent_level = WmMenuLevel::Root;
                        self.show_wm_task_menu(WmMenuLevel::MenuDirectoryContext, root_x, root_y);
                    }
                    Some(WmChromeTarget::Task(target)) => {
                        self.set_wm_task_preview(None, root_x);
                        self.show_wm_task_menu(WmMenuLevel::TaskContext(target), root_x, root_y);
                    }
                    Some(WmChromeTarget::QuickLaunch(index)) => {
                        self.wm_quick_context_index = Some(index);
                        self.set_wm_task_preview(None, root_x);
                        let level = if self.wm_dock_style {
                            self.wm_dock_launcher_task(index).map(WmMenuLevel::TaskContext).unwrap_or(WmMenuLevel::QuickLaunchContext)
                        } else { WmMenuLevel::QuickLaunchContext };
                        self.show_wm_task_menu(level, root_x, root_y);
                    }
                    None => {
                        self.set_wm_task_preview(None, root_x);
                        self.show_wm_task_menu(WmMenuLevel::TaskbarContext, root_x, root_y);
                    }
                    _ => {}
                }
            }
            return true;
        }
        if button == 3 && source == WM_MENU {
            if pressed {
                let was_context = matches!(
                    self.wm_menu_level,
                    WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext
                );
                let parent_column = self.wm_menu_column_at(root_x);
                let parent_widths = self.wm_menu_column_widths();
                let parent_x_offsets = self.wm_menu_column_x_offsets();
                let application =
                    self.wm_menu_hit(root_x, root_y)
                        .and_then(|(_, _, row)| match row {
                            WmMenuRow::Application(application) => Some(application),
                            _ => None,
                        });
                if !was_context {
                    self.wm_application_context_columns = self.wm_menu_columns();
                    self.wm_application_context_widths = self.wm_menu_column_widths();
                    self.wm_application_context_offsets = self.wm_menu_column_offsets();
                    self.wm_application_context_parent_level = self.wm_menu_level;
                }
                let (menu_x, menu_y) = self.window_origin(WM_MENU);
                let available_width = usize::from(self.config.width_px)
                    .saturating_sub(usize::try_from(menu_x.max(0)).unwrap_or_default());
                if !was_context {
                    let parent_column =
                        parent_column.unwrap_or_else(|| parent_widths.len().saturating_sub(1));
                    let parent_left = parent_x_offsets
                        .get(parent_column)
                        .copied()
                        .unwrap_or_default();
                    let parent_width = parent_widths
                        .get(parent_column)
                        .copied()
                        .unwrap_or_default();
                    let redmond98 = self
                        .wm_themes
                        .get(usize::from(self.wm_theme))
                        .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98);
                    let overlap = usize::from(redmond98) * 2;
                    let context_width = 240usize.min(available_width);
                    let right = parent_left
                        .saturating_add(parent_width)
                        .saturating_sub(overlap);
                    self.wm_application_context_x =
                        if right.saturating_add(context_width) <= available_width {
                            right
                        } else {
                            parent_left.saturating_sub(context_width.saturating_sub(overlap))
                        };
                }
                let available_height = self.wm_menu_available_height();
                let context_rows =
                    application.as_ref().map_or(
                        1,
                        |application| if application.uninstallable { 3 } else { 2 },
                    );
                let menu_row_height = self.wm_menu_base_row_height();
                // Keep the complete popup bevel above the work-area edge.
                // Without this small clearance the taskbar's top border can
                // cover the context pane's bottom edge when the pointer is on
                // the final Start-menu row.
                let context_edge_clearance = 5;
                self.wm_application_context_y =
                    usize::try_from(i32::from(root_y).saturating_sub(menu_y))
                        .unwrap_or_default()
                        .min(
                            available_height
                                .saturating_sub(menu_row_height.saturating_mul(context_rows))
                                .saturating_sub(context_edge_clearance),
                        );
                self.wm_menu_level = application
                    .map_or(WmMenuLevel::MenuDirectoryContext, |application| {
                        WmMenuLevel::ApplicationContext(application.action)
                    });
                self.wm_menu_offset = 0;
                self.wm_menu_hover_row = None;
                self.relayout_wm_menu();
                // Relayout replaces the menu surface. Reassert its top-stack
                // position so the newly opened context pane cannot fall
                // behind a cascade column, desktop icon, or the taskbar.
                self.restack(WM_MENU, None, Some(0));
                self.rebuild_scene();
            }
            return true;
        }
        if button == 1 {
            if pressed
                && source == WM_MENU
                && let Some((_, _, WmMenuRow::Application(entry))) =
                    self.wm_menu_hit(root_x, root_y)
                && let Some(source_index) = self
                    .wm_menu_entries
                    .iter()
                    .position(|candidate| candidate.action == entry.action)
            {
                self.wm_menu_drag = Some(WmMenuDrag {
                    entry,
                    source_index,
                    pointer_x: root_x,
                    pointer_y: root_y,
                });
                self.input_state |= 0x0100;
                self.paint_wm_chrome(WM_MENU);
                self.scene_upsert(WM_MENU);
                return true;
            }
            if !pressed && self.wm_menu_drag.is_some() {
                self.finish_wm_menu_drag(root_x, root_y);
                self.paint_wm_chrome(WM_TASKBAR);
                self.scene_upsert(WM_TASKBAR);
                self.input_state &= !0x0100;
                return true;
            }
            let target = self.wm_chrome_hit(source, root_x, root_y);
            if pressed {
                let previous = self.wm_chrome_pressed;
                self.wm_chrome_pressed = target;
                self.repaint_wm_chrome_targets(previous, target);
            } else if self.wm_chrome_pressed.is_some() {
                let previous = self.wm_chrome_pressed.take();
                self.repaint_wm_chrome_targets(previous, target);
            }
            if !pressed && self.wm_drag.is_some() {
                self.finish_wm_drag(root_x, root_y);
                self.input_state &= !0x0100;
                return true;
            }
            if pressed && self.handle_wm_pointer_press(source, root_x, root_y) {
                self.input_state |= 0x0100;
                return true;
            }
        }
        false
    }

    pub(super) fn wm_quick_drop_position(&self, root_x: i16, root_y: i16) -> Option<u8> {
        if !self.wm_quick_launch_visible || !self.wm_gnome_dash_visible { return None; }
        let (x, y, width, height) = self.wm_taskbar_rect();
        if root_x < x || root_y < y || i32::from(root_x) >= i32::from(x) + i32::from(width)
            || i32::from(root_y) >= i32::from(y) + i32::from(height) { return None; }
        if self.wm_dock_style {
            let axis = if self.wm_taskbar_edge.vertical() { root_y - y } else { root_x - x };
            let offset = i32::from(axis) - WM_COMPACT_DOCK_AXIS_PADDING as i32 - (1 + i32::from(self.wm_launcher_visible)) * self.wm_dock_step() as i32;
            let step = self.wm_dock_step() as i32;
            if offset < 0 || offset > self.wm_quick_launchers.len() as i32 * step + 4 { return None; }
            return Some(((offset + step / 2) / step).min(self.wm_quick_launchers.len() as i32) as u8,
            );
        }
        let offset = if self.wm_taskbar_edge.vertical() {
            i32::from(root_y) - i32::from(y) - self.wm_vertical_quick_launch_y() as i32
        } else { i32::from(root_x) - i32::from(self.wm_quick_launch_x()) };
        let extent = self.wm_quick_launchers.len() as i32 * 28;
        if offset < 0 || offset > extent + 4 { return None; }
        Some(((offset + 14) / 28).min(self.wm_quick_launchers.len() as i32) as u8)
    }

    pub(super) fn finish_wm_menu_drag(&mut self, root_x: i16, root_y: i16) {
        let Some(drag) = self.wm_menu_drag.take() else {
            return;
        };
        let moved = drag.pointer_x.abs_diff(root_x) > 4 || drag.pointer_y.abs_diff(root_y) > 4;
        if !moved {
            self.wm_actions.push_back(drag.entry.action);
            if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
                menu.mapped = false;
            }
            self.wm_menu_hover_row = None;
            self.scene_upsert(WM_MENU);
            self.update_taskbar_pointer(root_x, root_y);
            return;
        }
        if let Some(position) = self.wm_quick_drop_position(root_x, root_y) {
            self.wm_actions.push_back(WM_ACTION_QUICK_PIN_PREFIX
                | (u32::try_from(drag.source_index).unwrap_or(0xffff).min(0xffff) << 8)
                | u32::from(position),
            );
            if let Some(menu) = self.windows.get_mut(&WM_MENU.0) { menu.mapped = false; }
            self.wm_menu_hover_row = None;
            self.scene_upsert(WM_MENU);
            self.update_taskbar_pointer(root_x, root_y);
            return;
        }
        if self.wm_menu_structured {
            self.paint_wm_chrome(WM_MENU);
            self.scene_upsert(WM_MENU);
            self.update_taskbar_pointer(root_x, root_y);
            return;
        }
        let drop_target = self.wm_menu_hit(root_x, root_y);
        let (category, before_action) = match drop_target {
            Some((_, _, WmMenuRow::Category(category))) => (category, None),
            Some((_, _, WmMenuRow::Application(target))) => (target.category, Some(target.action)),
            Some((0, _, _)) => (0, None),
            None if self.wm_menu_column_at(root_x) == Some(2)
                && matches!(self.wm_menu_level, WmMenuLevel::Applications(_)) =>
            {
                let WmMenuLevel::Applications(category) = self.wm_menu_level else {
                    unreachable!()
                };
                (category, None)
            }
            _ => {
                self.paint_wm_chrome(WM_MENU);
                self.scene_upsert(WM_MENU);
                return;
            }
        };
        let Some(current) = self
            .wm_menu_entries
            .iter()
            .position(|entry| entry.action == drag.entry.action)
        else {
            return;
        };
        let mut entry = self.wm_menu_entries.remove(current);
        entry.category = category;
        let insertion = before_action
            .and_then(|action| {
                self.wm_menu_entries
                    .iter()
                    .position(|candidate| candidate.action == action)
            })
            .unwrap_or_else(|| {
                self.wm_menu_entries
                    .iter()
                    .rposition(|candidate| candidate.category == category)
                    .map_or(self.wm_menu_entries.len(), |index| index + 1)
            });
        self.wm_menu_entries.insert(insertion, entry);
        let position = self.wm_menu_entries[..insertion]
            .iter()
            .filter(|candidate| candidate.category == category)
            .count();
        let encoded = WM_ACTION_MENU_MOVE_PREFIX
            | (u32::try_from(drag.source_index).unwrap_or(0xff).min(0xff) << 12)
            | (u32::from(category) << 8)
            | u32::try_from(position).unwrap_or(0xff).min(0xff);
        self.wm_actions.push_back(encoded);
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        self.relayout_wm_menu();
    }
}
