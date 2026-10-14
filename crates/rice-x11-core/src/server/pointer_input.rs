//! Pointer input operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Inject browser pointer motion into the native X11 event stream.
    #[must_use]
    pub fn pointer_layer(&self, root_x: i16, root_y: i16) -> u8 {
        let target = self.hit_test(root_x, root_y);
        if Self::is_wm_shell_window(target) {
            2
        } else if self.frame_clients.contains_key(&target.0)
            || self.managed_resize_frame_at(target, root_x, root_y).is_some()
        {
            // Managed application decorations are compositor-owned, but are
            // not system shell chrome. Keeping this distinction lets a
            // foreground host-native window get first refusal at an overlap
            // while an exposed managed frame still receives its live drag.
            3
        } else {
            u8::from(target != ROOT_WINDOW && !self.is_wm_desktop_descendant(target))
        }
    }

    /// Inject browser pointer motion into the native X11 event stream.
    pub fn inject_pointer_motion(&mut self, root_x: i16, root_y: i16, state: u16) {
        // XWarpPointer changes the server cursor, not the browser's physical
        // position. Deriving raw deltas from that warped cursor introduces
        // artificial movement each time an SDL client recenters it.
        let previous = self.physical_pointer_position.unwrap_or((root_x, root_y));
        self.physical_pointer_position = Some((root_x, root_y));
        self.inject_pointer_motion_with_delta(
            root_x, root_y,
            i32::from(root_x) - i32::from(previous.0),
            i32::from(root_y) - i32::from(previous.1), state,
        );
    }

    /// Deliver physical deltas separately from the cursor position. Pointer
    /// lock may keep the latter stationary while movement remains unbounded.
    pub fn inject_pointer_motion_with_delta(
        &mut self, root_x: i16, root_y: i16, delta_x: i32, delta_y: i32, state: u16,
    ) {
        self.select_shell_output(root_x, root_y);
        self.physical_pointer_position = Some((root_x, root_y));
        if delta_x != 0 || delta_y != 0 {
            self.emit_xinput_raw_pointer_event(17, 0, delta_x, delta_y);
        }
        self.move_pointer(root_x, root_y, state);
    }

    /// Browser capture is appropriate only for an explicit relative-input grab,
    /// not ordinary GTK menus, implicit button grabs, or window decorations.
    pub fn relative_pointer_capture_requested(&self) -> bool {
        let Some(grab) = self.pointer_grab else { return false; };
        if grab.automatic || self.map_state(grab.window) != 2 { return false; }
        let source = self.hit_test(self.pointer_x, self.pointer_y);
        if source != grab.window && !self.is_descendant(source, grab.window) { return false; }
        self.clients.get(&grab.owner).is_some_and(|client|
            {
            [0u16, 1, 2, 4].iter().any(|selector|
                {
                client.xi2_masks.get(&(ROOT_WINDOW.0, *selector))
                    .is_some_and(|mask| mask & (1 << 17) != 0)})
        })
    }

    pub(super) fn move_pointer(&mut self, root_x: i16, root_y: i16, state: u16) {
        self.pointer_x = root_x;
        self.pointer_y = root_y;
        self.input_state = state;
        if self.config.native_window_manager {
            self.update_taskbar_pointer(root_x, root_y);
        }
        let source = self.hit_test(root_x, root_y);
        if self.config.native_window_manager {
            self.update_wm_pointer_hover(source, root_x, root_y);
        }
        if source != self.pointer_window {
            let previous = self.pointer_window;
            self.emit_crossing(previous, false, root_x, root_y, state);
            self.emit_xinput_crossing(previous, false, root_x, root_y, state);
            self.pointer_window = source;
            self.emit_crossing(source, true, root_x, root_y, state);
            self.emit_xinput_crossing(source, true, root_x, root_y, state);
        }
        if self.wm_menu_drag.is_some() {
            if state & 0x0100 == 0 { self.wm_menu_drag = None; }
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
        }
        if let Some(mut drag) = self.wm_taskbar_drag {
            if state & 0x0100 == 0 {
                self.wm_taskbar_drag = None;
                self.set_wm_taskbar_drag_preview(None);
            } else {
                drag.moved |=
                    drag.pointer_x.abs_diff(root_x) > 4 || drag.pointer_y.abs_diff(root_y) > 4;
                if drag.moved {
                    let candidate = self.wm_taskbar_edge_at(root_x, root_y);
                    if candidate != drag.candidate {
                        drag.candidate = candidate;
                    }
                    self.set_wm_taskbar_drag_preview(Some(candidate));
                }
                self.wm_taskbar_drag = Some(drag);
            }
        }
        if let Some(mut drag) = self.wm_drag {
            if state & 0x0100 == 0 {
                self.wm_drag = None;
                self.set_wm_snap_preview(None);
            } else {
                drag.moved |= root_x != drag.pointer_x || root_y != drag.pointer_y;
                if drag.resize_edges != 0 && drag.moved {
                    let (initial_x, initial_y, initial_width, initial_height) =
                        drag.resize_geometry;
                    let (left_inset, right_inset, top_inset, bottom_inset) =
                        self.managed_frame_insets(drag.client);
                    let horizontal_inset =
                        i32::from(left_inset).saturating_add(i32::from(right_inset));
                    let vertical_inset =
                        i32::from(top_inset).saturating_add(i32::from(bottom_inset));
                    let initial_left = i32::from(initial_x);
                    let initial_top = i32::from(initial_y);
                    let initial_right = initial_left
                        .saturating_add(i32::from(initial_width))
                        .saturating_add(horizontal_inset);
                    let initial_bottom = initial_top
                        .saturating_add(i32::from(initial_height))
                        .saturating_add(vertical_inset);
                    let delta_x = i32::from(root_x).saturating_sub(i32::from(drag.pointer_x));
                    let delta_y = i32::from(root_y).saturating_sub(i32::from(drag.pointer_y));
                    let (work_x, work_y, work_width, work_height) = self.wm_drag_workarea();
                    let work_right = i32::from(work_x).saturating_add(i32::from(work_width));
                    let work_bottom = i32::from(work_y).saturating_add(i32::from(work_height));
                    let (min_width,min_height,max_width,max_height)=self.managed_client_size_limits(drag.client);
                    // Client limits exclude the compositor's frame. Keep
                    // the opposite edge anchored and bound impossible hints by
                    // the space available on the dragged side of the output.
                    let available_width=if drag.resize_edges&WM_RESIZE_LEFT!=0 {initial_right-i32::from(work_x)}else{work_right-initial_left};
                    let available_height=if drag.resize_edges&WM_RESIZE_TOP!=0 {initial_bottom-i32::from(work_y)}else{work_bottom-initial_top};
                    let minimum_outer_width=(i32::from(min_width)+horizontal_inset).max(horizontal_inset+1).min(available_width.max(horizontal_inset+1));
                    let minimum_outer_height=(i32::from(min_height)+vertical_inset).max(vertical_inset+1).min(available_height.max(vertical_inset+1));
                    let maximum_outer_width=(i32::from(max_width)+horizontal_inset).min(available_width).max(minimum_outer_width);
                    let maximum_outer_height=(i32::from(max_height)+vertical_inset).min(available_height).max(minimum_outer_height);
                    let mut left = initial_left;
                    let mut top = initial_top;
                    let mut right = initial_right;
                    let mut bottom = initial_bottom;
                    // A new hint may invalidate the dimension not being
                    // dragged. Repair it from its existing top/left anchor.
                    if drag.resize_edges&(WM_RESIZE_LEFT|WM_RESIZE_RIGHT)==0 {
                        right=initial_left+(initial_right-initial_left).clamp(minimum_outer_width,maximum_outer_width);
                    }
                    if drag.resize_edges&(WM_RESIZE_TOP|WM_RESIZE_BOTTOM)==0 {
                        bottom=initial_top+(initial_bottom-initial_top).clamp(minimum_outer_height,maximum_outer_height);
                    }
                    if drag.resize_edges & WM_RESIZE_LEFT != 0 {
                        left = initial_left.saturating_add(delta_x).clamp(
                            initial_right.saturating_sub(maximum_outer_width).max(i32::from(work_x)),
                            initial_right
                                .saturating_sub(minimum_outer_width)
                                .max(i32::from(work_x)),
                        );
                    }
                    if drag.resize_edges & WM_RESIZE_RIGHT != 0 {
                        let minimum_right = initial_left.saturating_add(minimum_outer_width);
                        right = initial_right
                            .saturating_add(delta_x)
                            .clamp(minimum_right, initial_left.saturating_add(maximum_outer_width),
                        );
                    }
                    if drag.resize_edges & WM_RESIZE_TOP != 0 {
                        top = initial_top.saturating_add(delta_y).clamp(
                            initial_bottom.saturating_sub(maximum_outer_height).max(i32::from(work_y)),
                            initial_bottom
                                .saturating_sub(minimum_outer_height)
                                .max(i32::from(work_y)),
                        );
                    }
                    if drag.resize_edges & WM_RESIZE_BOTTOM != 0 {
                        let minimum_bottom = initial_top.saturating_add(minimum_outer_height);
                        bottom = initial_bottom
                            .saturating_add(delta_y)
                            .clamp(minimum_bottom, initial_top.saturating_add(maximum_outer_height),
                        );
                    }
                    let client_width = u16::try_from(
                        right
                            .saturating_sub(left)
                            .saturating_sub(horizontal_inset))
                    .unwrap_or(1)
                    .max(1);
                    let client_height = u16::try_from(
                        bottom
                            .saturating_sub(top)
                            .saturating_sub(vertical_inset))
                    .unwrap_or(1)
                    .max(1);
                    self.maximized_geometries.remove(&drag.client.0);
                    self.snapped_geometries.remove(&drag.client.0);
                    self.apply_managed_geometry(
                        drag.client,
                        drag.frame,
                        clamp_i16(left),
                        clamp_i16(top),
                        client_width,
                        client_height,
                    );
                } else if drag.restore_on_move && drag.moved {
                    let (_, _, width, height) = drag.restore_geometry;
                    let (left, right, top, bottom) = self.managed_frame_insets(drag.client);
                    let restored_width = width.saturating_add(left).saturating_add(right);
                    let current_width = self
                        .windows
                        .get(&drag.frame.0)
                        .map_or(restored_width, |frame| frame.width)
                        .max(1);
                    let pointer_offset_x = i32::from(drag.pointer_x)
                        .saturating_sub(i32::from(drag.frame_x))
                        .clamp(0, i32::from(current_width));
                    let restored_offset_x = pointer_offset_x
                        .saturating_mul(i32::from(restored_width))
                        / i32::from(current_width);
                    let pointer_offset_y = i32::from(drag.pointer_y)
                        .saturating_sub(i32::from(drag.frame_y))
                        .clamp(0, i32::from(top.saturating_sub(1)));
                    let (work_x, work_y, work_width, work_height) = self.wm_drag_workarea();
                    let max_x = i32::from(work_x)
                        .saturating_add(i32::from(work_width))
                        .saturating_sub(i32::from(restored_width));
                    let restored_height = height
                        .saturating_add(top)
                        .saturating_add(bottom);
                    let max_y = i32::from(work_y.saturating_add(work_height))
                        .saturating_sub(i32::from(restored_height));
                    let restored_x = i32::from(root_x)
                        .saturating_sub(restored_offset_x)
                        .clamp(i32::from(work_x), max_x.max(i32::from(work_x)));
                    let restored_y = i32::from(root_y)
                        .saturating_sub(pointer_offset_y)
                        .clamp(i32::from(work_y), max_y.max(i32::from(work_y)));
                    self.maximized_geometries.remove(&drag.client.0);
                    self.apply_managed_geometry(
                        drag.client,
                        drag.frame,
                        clamp_i16(restored_x),
                        clamp_i16(restored_y),
                        width,
                        height,
                    );
                    drag.pointer_x = root_x;
                    drag.pointer_y = root_y;
                    drag.frame_x = clamp_i16(restored_x);
                    drag.frame_y = clamp_i16(restored_y);
                    drag.restore_on_move = false;
                }
                if drag.resize_edges == 0 {
                    let x = drag
                        .frame_x
                        .saturating_add(root_x.saturating_sub(drag.pointer_x));
                    let y = drag
                        .frame_y
                        .saturating_add(root_y.saturating_sub(drag.pointer_y));
                    let (work_x, work_y, work_width, work_height) = self.wm_drag_workarea();
                    let max_x =
                        self.windows
                            .get(&drag.frame.0)
                            .map_or(i32::from(work_x), |frame| {
                                i32::from(work_x)
                                    .saturating_add(i32::from(work_width))
                                    .saturating_sub(i32::from(frame.width))
                            });
                    let max_y = self.windows.get(&drag.frame.0).map_or(0, |frame| {
                        i32::from(work_y.saturating_add(work_height))
                            .saturating_sub(i32::from(frame.height))
                    });
                    if let Some(frame) = self.windows.get_mut(&drag.frame.0) {
                        frame.y = clamp_i16(
                            i32::from(y).clamp(i32::from(work_y), max_y.max(i32::from(work_y))),
                        );
                        frame.x = clamp_i16(
                            i32::from(x).clamp(i32::from(work_x), max_x.max(i32::from(work_x))),
                        );
                    }
                    self.scene_upsert(drag.frame);
                    self.scene_damage(drag.frame);
                    let snap_target = self.wm_snap_target_at(root_x, root_y);
                    if snap_target != self.wm_snap_target || snap_target.is_some() {
                        self.set_wm_snap_preview(snap_target);
                    }
                }
                self.wm_drag = Some(drag);
            }
        }
        let mask = POINTER_MOTION_MASK | motion_mask_for_state(state);
        if self.emit_xinput_device_event(source, 6, 0, root_x, root_y, state, true).is_empty() {
            self.emit_pointer_input(
                PointerInput {
                    source,
                    mask,
                    detail: 0,
                    root_x,
                    root_y,
                    state,
                },
                None,
                |output, order, event| {
                    x11::encode_motion_notify_event(output, order, event);
                },
            );
        }
    }

    pub(super) fn update_wm_pointer_hover(&mut self, source: ResourceId, root_x: i16, root_y: i16) {
        let inside_task_preview = self
            .windows
            .get(&WM_TASK_PREVIEW.0)
            .is_some_and(|preview| {
                preview.mapped
                    && root_x >= preview.x
                    && root_y >= preview.y
                    && root_x < preview.x.saturating_add_unsigned(preview.width)
                    && root_y < preview.y.saturating_add_unsigned(preview.height)
            });
        let chrome_hover = if self.wm_dock_style
            && source == WM_TASK_PREVIEW
            && inside_task_preview
        {
            // The caretted preview is an extension of the hovered dock item.
            // Keep the fisheye active while the pointer crosses into it.
            self.wm_chrome_hover
        } else {
            self.wm_chrome_hit(source, root_x, root_y)
        };
        if chrome_hover != self.wm_chrome_hover {
            let previous = self.wm_chrome_hover;
            self.wm_chrome_hover = chrome_hover;
            self.repaint_wm_chrome_targets(previous, chrome_hover);
            self.update_wm_quick_launch_tooltip();
        } else if self.wm_dock_style && self.wm_dock_magnification && source == WM_TASKBAR {
            // The curve follows the pointer continuously within an icon slot,
            // not only when the discrete click target changes.
            self.paint_wm_chrome(WM_TASKBAR);
            self.scene_upsert(WM_TASKBAR);
        }
        let menu_hit = (source == WM_MENU)
            .then(|| self.wm_menu_hit(root_x, root_y))
            .flatten();
        let hover_level = menu_hit
            .as_ref()
            .and_then(|(column, _, entry)| match (*column, entry) {
                _ if matches!(
                    self.wm_menu_level,
                    WmMenuLevel::TaskGroup(_)
                        | WmMenuLevel::TaskContext(_)
                        | WmMenuLevel::TaskbarContext
                        | WmMenuLevel::QuickLaunchContext
                        | WmMenuLevel::ApplicationContext(_)
                        | WmMenuLevel::MenuDirectoryContext
                ) =>
                {
                    Some(self.wm_menu_level)
                }
                (0, WmMenuRow::Applications) => Some(match self.wm_menu_level {
                    WmMenuLevel::Applications(category) => WmMenuLevel::Applications(category),
                    WmMenuLevel::MenuFolder(folder) => WmMenuLevel::MenuFolder(folder),
                    _ if self.wm_menu_structured => WmMenuLevel::MenuFolders,
                    _ => WmMenuLevel::Categories,
                }),
                (0, WmMenuRow::Themes) => Some(WmMenuLevel::Themes),
                (column, WmMenuRow::Category(category))
                    if column == usize::from(!self.wm_menu_applications_only) =>
                {
                    Some(WmMenuLevel::Applications(*category))
                }
                (_, WmMenuRow::MenuFolder(folder)) => Some(WmMenuLevel::MenuFolder(*folder)),
                (0, _) => Some(WmMenuLevel::Root),
                _ => None,
            })
            .or_else(|| {
                (source == WM_MENU
                    && self.wm_menu_column_at(root_x)
                        == Some(usize::from(!self.wm_menu_applications_only))
                    && matches!(
                        self.wm_menu_level,
                        WmMenuLevel::Applications(_) | WmMenuLevel::MenuFolder(_)
                    ))
                .then_some(if self.wm_menu_structured {
                    WmMenuLevel::MenuFolders
                } else {
                    WmMenuLevel::Categories
                })
            });
        if let Some(hover_level) = hover_level
            && hover_level != self.wm_menu_level
        {
            self.wm_menu_level = hover_level;
            self.wm_menu_offset = 0;
            self.wm_menu_hover_row = None;
            self.relayout_wm_menu();
        }
        let menu_hover = if source == WM_MENU {
            self.wm_menu_hit(root_x, root_y)
                .map(|(column, row, entry)| (column, row, entry))
        } else {
            None
        };
        if menu_hover != self.wm_menu_hover_row {
            self.wm_menu_hover_row = menu_hover;
            self.paint_wm_chrome(WM_MENU);
            self.scene_upsert(WM_MENU);
        }
        if self.wm_dock_style && self.wm_task_previews_enabled {
            let hovered_task = chrome_hover.and_then(|target| match target {
                WmChromeTarget::Task(task) => Some(task),
                WmChromeTarget::QuickLaunch(index) => self.wm_dock_launcher_task(index),
                _ => None,
            });
            let context_open = matches!(self.wm_menu_level, WmMenuLevel::TaskContext(_) | WmMenuLevel::QuickLaunchContext)
                && self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped);
            if let Some(WmTaskTarget::Group(group)) = hovered_task.filter(|_| !context_open) {
                if self.wm_menu_level != WmMenuLevel::TaskGroup(group) || !self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped) {
                    self.show_wm_task_menu(WmMenuLevel::TaskGroup(group), root_x, root_y);
                }
            } else if matches!(self.wm_menu_level, WmMenuLevel::TaskGroup(_)) {
                let retain = self.windows.get(&WM_MENU.0).is_some_and(|menu| {
                    let (dx,dy,dw,dh) = self.wm_taskbar_rect();
                    menu.mapped && (source == WM_MENU || if self.wm_taskbar_edge.vertical() {
                        root_y >= menu.y && root_y < menu.y + menu.height as i16 && root_x >= menu.x.min(dx) && root_x < (menu.x + menu.width as i16).max(dx + dw as i16)
                    } else {
                        root_x >= menu.x && root_x < menu.x + menu.width as i16 && root_y >= menu.y.min(dy) && root_y < (menu.y + menu.height as i16).max(dy + dh as i16)
                    })
                });
                if !retain { self.dismiss_wm_menu(); }
            }
        }
        if self.wm_dock_style && matches!(self.wm_menu_level, WmMenuLevel::TaskGroup(_) | WmMenuLevel::TaskContext(_) | WmMenuLevel::QuickLaunchContext) && self.windows.get(&WM_MENU.0).is_some_and(|menu|menu.mapped) {
            self.set_wm_task_preview(None,root_x);
        } else if let Some((column, row, WmMenuRow::Task(target))) = menu_hit {
            self.set_wm_menu_task_preview(Some(target), column, row);
        } else {
            // The preview is flush with the dock's animation surface, so no
            // invisible bridge is necessary. Retaining that former corridor
            // kept previews open over blank dock and desktop space.
            let retained_preview = self.wm_dock_style && inside_task_preview;
            let preview_target = chrome_hover.and_then(|target| match target {
                WmChromeTarget::Task(task) => Some(task),
                WmChromeTarget::QuickLaunch(index) if self.wm_dock_style=>{
                        self.wm_dock_launcher_task(index)}
                    _ => None,
            }).or_else(||{
                    retained_preview.then_some(self.wm_task_preview_target).flatten()});
            self.set_wm_task_preview(preview_target, root_x);
        }
    }
}
