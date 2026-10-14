//! Shell previews operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn relayout_wm_menu(&mut self) {
        let row_height = self.wm_menu_base_row_height();
        let columns = self.wm_menu_columns();
        let offsets = self.wm_menu_column_offsets();
        let content_height = columns
            .iter()
            .enumerate()
            .map(|(column, rows)| {
                offsets
                    .get(column)
                    .copied()
                    .unwrap_or_default()
                    .saturating_add(wm_menu_rows_height_with(rows, row_height))
            })
            .max()
            .unwrap_or(row_height);
        let available_height = u16::try_from(self.wm_menu_available_height()).unwrap_or(u16::MAX);
        let preserves_cascade = matches!(
            self.wm_menu_level,
            WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext
        ) && !self.wm_application_context_columns.is_empty();
        let height = if preserves_cascade
            || matches!(
                self.wm_menu_level,
                WmMenuLevel::Root
                    | WmMenuLevel::Categories
                    | WmMenuLevel::Applications(_)
                    | WmMenuLevel::MenuFolders
                    | WmMenuLevel::MenuFolder(_)
                    | WmMenuLevel::Themes
            ) {
            available_height
        } else {
            u16::try_from(content_height)
                .unwrap_or(u16::MAX)
                .min(available_height)
        };
        let widths = self.wm_menu_column_widths();
        let x_offsets = self.wm_menu_column_x_offsets();
        let width = u16::try_from(
            widths
                .iter()
                .enumerate()
                .map(|(column, width)| {
                    x_offsets
                        .get(column)
                        .copied()
                        .unwrap_or_default()
                        .saturating_add(*width)
                })
                .max()
                .unwrap_or(1),
        )
        .unwrap_or(u16::MAX);
        let redmond98 = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98);
        let taskbar_gap = if redmond98 || self.wm_embedded_shell { 0 } else { 4 };
        let mut y = self.wm_menu_window_anchor.map_or_else(
            || match self.wm_taskbar_edge {
                WmTaskbarEdge::Top => self
                    .wm_taskbar_y()
                    .saturating_add(WM_TASKBAR_HEIGHT)
                    .saturating_add(taskbar_gap),
                WmTaskbarEdge::Bottom => self
                    .wm_taskbar_y()
                    .saturating_sub(height.saturating_add(taskbar_gap)),
                WmTaskbarEdge::Left | WmTaskbarEdge::Right => {
                    self.wm_taskbar_rect().1.cast_unsigned()
                }
            },
            |(_, anchor_y)| {
                let maximum_y = i32::from(self.config.height_px.saturating_sub(height));
                u16::try_from(i32::from(anchor_y).clamp(0, maximum_y.max(0))).unwrap_or_default()
            },
        );
        let side_menu_x = (self.wm_menu_window_anchor.is_none()
            && self.wm_taskbar_edge.vertical()
            && self.wm_menu_is_start_cascade())
        .then(|| {
            let (taskbar_x, _, taskbar_width, _) = self.wm_taskbar_rect();
            let desired = if self.wm_taskbar_edge == WmTaskbarEdge::Left {
                i32::from(taskbar_x)
                    .saturating_add(i32::from(taskbar_width))
                    .saturating_add(i32::from(taskbar_gap))
            } else {
                i32::from(taskbar_x)
                    .saturating_sub(i32::from(width))
                    .saturating_sub(i32::from(taskbar_gap))
            };
            clamp_i16(desired.clamp(0, i32::from(self.config.width_px.saturating_sub(width))))
        });
        let mut menu_x = side_menu_x;
        if self.wm_menu_window_anchor.is_none() && self.wm_menu_is_start_cascade() {
            let (dock_x, dock_y, _dock_width, dock_height) = self.wm_taskbar_rect();
            if self.wm_dock_style && !self.wm_taskbar_edge.vertical() {
                menu_x = Some(clamp_i16(i32::from(dock_x) + 8 - self.wm_menu_column_x_offsets().first().copied().unwrap_or(0) as i32,
                ));
                y = match self.wm_taskbar_edge {
                    WmTaskbarEdge::Top => dock_y.cast_unsigned().saturating_add(dock_height).saturating_add(taskbar_gap),
                    _ => dock_y.cast_unsigned().saturating_sub(height.saturating_add(taskbar_gap)),
                };
            }
            if self.wm_embedded_shell && !self.wm_dock_style {
                y = match self.wm_taskbar_edge {
                    WmTaskbarEdge::Top => 8,
                    WmTaskbarEdge::Bottom => self.config.height_px.saturating_sub(height).saturating_sub(8),
                    _ => y.min(self.config.height_px.saturating_sub(height).saturating_sub(8),
                        ).max(8),
                };
            }
        }
        let primary = self.wm_shell_rect();
        let constrain_primary = self.wm_menu_window_anchor.is_none() && self.wm_menu_is_start_cascade();
        if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
            if let Some(x) = menu_x {
                menu.x = x;
            }
            menu.y = i16::try_from(y).unwrap_or(i16::MAX);
            menu.width = width;
            menu.height = height;
            if constrain_primary {
                menu.width = menu.width.min(primary.2);
                menu.height = menu.height.min(primary.3);
                menu.x = clamp_i16(i32::from(menu.x).clamp(i32::from(primary.0), i32::from(primary.0 + primary.2 - menu.width)));
                menu.y = clamp_i16(i32::from(menu.y).clamp(i32::from(primary.1), i32::from(primary.1 + primary.3 - menu.height)));
            }
            menu.depth = 32;
            menu.border_width = 0;
            menu.background_pixel = 0;
        }
        let (width,height) = self.windows.get(&WM_MENU.0).map_or((width,height), |menu| (menu.width,menu.height));
        self.surfaces
            .insert(WM_MENU.0, Surface::new(width, height, 32, 0));
        self.paint_wm_chrome(WM_MENU);
        self.scene_upsert(WM_MENU);
        self.scene_damage(WM_MENU);
    }

    pub(super) fn show_wm_task_menu(&mut self, level: WmMenuLevel, root_x: i16, root_y: i16) {
        self.wm_menu_window_anchor = None;
        self.wm_menu_level = level;
        self.wm_menu_offset = 0;
        self.wm_menu_hover_row = None;
        let desired_width = match level {
            WmMenuLevel::TaskContext(_)
            | WmMenuLevel::TaskbarContext
            | WmMenuLevel::QuickLaunchContext
            | WmMenuLevel::ApplicationContext(_)
            | WmMenuLevel::MenuDirectoryContext => 280,
            _ => WM_MENU_APPLICATION_WIDTH,
        };
        let menu_width = i16::try_from(desired_width).unwrap_or(i16::MAX);
        let redmond98 = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98);
        let side_menu_width = self
            .wm_menu_column_x_offsets()
            .into_iter()
            .zip(self.wm_menu_column_widths())
            .map(|(offset, width)| offset.saturating_add(width))
            .max()
            .unwrap_or(desired_width);
        let side_menu_x = self.wm_taskbar_edge.vertical().then(|| {
            let gap = if redmond98 { 0 } else { 4 };
            let (taskbar_x, _, taskbar_width, _) = self.wm_taskbar_rect();
            let desired = if self.wm_taskbar_edge == WmTaskbarEdge::Left {
                i32::from(taskbar_x)
                    .saturating_add(i32::from(taskbar_width))
                    .saturating_add(gap)
            } else {
                i32::from(taskbar_x)
                    .saturating_sub(i32::try_from(side_menu_width).unwrap_or(i32::MAX))
                    .saturating_sub(gap)
            };
            clamp_i16(
                desired.clamp(
                    0,
                    i32::from(self.config.width_px)
                        .saturating_sub(i32::try_from(side_menu_width).unwrap_or(i32::MAX))
                        .max(0),
                ),
            )
        });
        if let Some(side_x) = side_menu_x {
            self.wm_menu_window_anchor = Some((side_x, root_y));
        }
        let minimum_x = if redmond98 && matches!(level, WmMenuLevel::Root) {
            0
        } else {
            4
        };
        let maximum_x = i32::from(self.config.width_px)
            .saturating_sub(i32::from(menu_width))
            .max(minimum_x);
        let pointer_anchored = level == WmMenuLevel::MenuDirectoryContext
            && self.wm_application_context_columns.is_empty();
        let desired_x = if redmond98 && matches!(level, WmMenuLevel::Root) {
            0
        } else if pointer_anchored {
            i32::from(root_x)
        } else {
            i32::from(root_x).saturating_sub(24)
        }
        .clamp(minimum_x, maximum_x);
        let menu_x = if !self.wm_dock_style && matches!(level, WmMenuLevel::TaskGroup(_)) && self.wm_task_previews_enabled
        {
            let preview_width = i32::from(WM_TASK_PREVIEW_WIDTH.min(self.config.width_px));
            let gap = 6i32;
            let right_anchored = desired_x.min(
                i32::from(self.config.width_px)
                    .saturating_sub(i32::from(menu_width))
                    .saturating_sub(preview_width)
                    .saturating_sub(gap),
            );
            let left_anchored = desired_x
                .max(preview_width.saturating_add(gap))
                .min(maximum_x);
            if right_anchored >= minimum_x
                && desired_x.saturating_sub(right_anchored)
                    <= left_anchored.saturating_sub(desired_x)
            {
                right_anchored
            } else {
                left_anchored
            }
        } else {
            desired_x
        };
        if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
            menu.x = side_menu_x.unwrap_or_else(|| clamp_i16(menu_x));
            menu.mapped = true;
        }
        // The quick-launch tooltip is a top-stacked surface. Hide it as soon
        // as its context menu opens so it cannot obscure or intercept the menu.
        self.update_wm_quick_launch_tooltip();
        self.relayout_wm_menu();
        if self.wm_dock_style && matches!(level,WmMenuLevel::TaskGroup(_)) {
            let (_,dock_y,_,dock_height)=self.wm_taskbar_rect();
            if let Some(menu)=self.windows.get_mut(&WM_MENU.0) {
                if !self.wm_taskbar_edge.vertical() {
                    menu.x=clamp_i16((i32::from(root_x)-i32::from(menu.width)/2).clamp(8,i32::from(self.config.width_px.saturating_sub(menu.width+8)).max(8),
                    ));
                    menu.y=if self.wm_taskbar_edge==WmTaskbarEdge::Top {dock_y.saturating_add(dock_height as i16).saturating_add(12)} else {dock_y.saturating_sub(menu.height as i16).saturating_sub(12)};
                }
                menu.background_pixel=0x8000_0000 | u32::from(root_x.cast_unsigned());
            }
            self.scene_upsert(WM_MENU);
        }
        self.restack(WM_MENU, None, Some(0));
        self.rebuild_scene();
    }

    pub(super) fn show_wm_window_menu(&mut self, target: WmTaskTarget, root_x: i16, root_y: i16) {
        self.show_wm_task_menu(
            WmMenuLevel::TaskContext(target),
            root_x.saturating_add(24),
            root_y,
        );
        let maximum_x = i32::from(self.config.width_px).saturating_sub(280).max(4);
        let menu_x = clamp_i16(i32::from(root_x).clamp(4, maximum_x));
        self.wm_menu_window_anchor = Some((menu_x, root_y));
        if let Some(menu) = self.windows.get_mut(&WM_MENU.0) {
            menu.x = menu_x;
        }
        self.relayout_wm_menu();
        self.restack(WM_MENU, None, Some(0));
        self.rebuild_scene();
    }

    pub fn show_native_task_menu(&mut self, id: u16, root_x: i32, root_y: i32) -> bool {
        if !self
            .native_tasks
            .iter()
            .any(|task| task.id == id && task.workspace == self.current_workspace)
        {
            return false;
        }
        self.show_wm_window_menu(
            WmTaskTarget::Native(id),
            clamp_i16(root_x),
            clamp_i16(root_y),
        );
        true
    }

    pub(super) fn set_wm_task_preview(&mut self, target: Option<WmTaskTarget>, root_x: i16) {
        let preview_width = WM_TASK_PREVIEW_WIDTH.min(self.config.width_px);
        let preview_height = WM_TASK_PREVIEW_HEIGHT.min(self.config.height_px);
        let maximum_x = i32::from(self.config.width_px.saturating_sub(preview_width)).max(0);
        let maximum_y = i32::from(self.config.height_px.saturating_sub(preview_height)).max(0);
        let centered_x = i32::from(root_x)
            .saturating_sub(i32::from(preview_width) / 2)
            .clamp(0, maximum_x);
        let centered_y = i32::from(self.pointer_y)
            .saturating_sub(i32::from(preview_height) / 2)
            .clamp(0, maximum_y);
        let (dock_x, dock_y, dock_width, dock_height) = self.wm_taskbar_rect();
        let (placement, x, y) = match self.wm_taskbar_edge {
            WmTaskbarEdge::Top => (
                WmTaskPreviewPlacement::Taskbar,
                centered_x,
                if self.wm_dock_style {
                    i32::from(dock_y).saturating_add(i32::from(dock_height))
                        .clamp(0, maximum_y)
                } else {
                    i32::from(self.wm_taskbar_y().saturating_add(WM_TASKBAR_HEIGHT + 6))
                },
            ),
            WmTaskbarEdge::Bottom => (
                WmTaskPreviewPlacement::Taskbar,
                centered_x,
                if self.wm_dock_style {
                    i32::from(dock_y).saturating_sub(i32::from(preview_height))
                        .clamp(0, maximum_y)
                } else {
                    i32::from(
                        self.wm_taskbar_y()
                            .saturating_sub(preview_height.saturating_add(6)),
                    )
                },
            ),
            WmTaskbarEdge::Left => (
                WmTaskPreviewPlacement::MenuRight,
                if self.wm_dock_style {
                    i32::from(dock_x).saturating_add(i32::from(dock_width))
                        .clamp(0, maximum_x)
                } else {
                    i32::from(WM_TASKBAR_HEIGHT.saturating_add(6))
                },
                centered_y,
            ),
            WmTaskbarEdge::Right => (
                WmTaskPreviewPlacement::MenuLeft,
                if self.wm_dock_style {
                    i32::from(dock_x).saturating_sub(i32::from(preview_width))
                        .clamp(0, maximum_x)
                } else {
                    maximum_x.saturating_sub(i32::from(WM_TASKBAR_HEIGHT.saturating_add(6)))
                },
                centered_y,
            ),
        };
        self.apply_wm_task_preview(target, placement, clamp_i16(x), clamp_i16(y));
    }

    pub(super) fn set_wm_menu_task_preview(
        &mut self,
        target: Option<WmTaskTarget>,
        column: usize,
        row: usize,
    ) {
        let preview_width = WM_TASK_PREVIEW_WIDTH.min(self.config.width_px);
        let preview_height = WM_TASK_PREVIEW_HEIGHT.min(self.config.height_px);
        let Some(menu) = self.windows.get(&WM_MENU.0).cloned() else {
            self.set_wm_task_preview(None, self.pointer_x);
            return;
        };
        let gap = 6i32;
        let menu_left = i32::from(menu.x);
        let menu_right = menu_left.saturating_add(i32::from(menu.width));
        let preview_width_i32 = i32::from(preview_width);
        let output_width = i32::from(self.config.width_px);
        let right_space = output_width.saturating_sub(menu_right);
        let placement =
            if right_space >= preview_width_i32.saturating_add(gap) || right_space >= menu_left {
                WmTaskPreviewPlacement::MenuRight
            } else {
                WmTaskPreviewPlacement::MenuLeft
            };
        let maximum_x = output_width.saturating_sub(preview_width_i32).max(0);
        let x = match placement {
            WmTaskPreviewPlacement::MenuRight => menu_right.saturating_add(gap),
            WmTaskPreviewPlacement::MenuLeft => menu_left
                .saturating_sub(preview_width_i32)
                .saturating_sub(gap),
            WmTaskPreviewPlacement::Taskbar => menu_left,
        }
        .clamp(0, maximum_x);
        let column_y = self
            .wm_menu_column_offsets()
            .get(column)
            .copied()
            .unwrap_or_default();
        let menu_row_height = self.wm_menu_base_row_height();
        let row_center = i32::from(menu.y)
            .saturating_add(i32::try_from(column_y).unwrap_or(i32::MAX))
            .saturating_add(i32::try_from(row.saturating_mul(menu_row_height)).unwrap_or(i32::MAX))
            .saturating_add(i32::try_from(menu_row_height / 2).unwrap_or_default());
        let (work_y, work_height) = self.wm_workarea();
        let minimum_y = i32::from(work_y);
        let maximum_y = minimum_y
            .saturating_add(i32::from(work_height))
            .saturating_sub(i32::from(preview_height))
            .max(minimum_y);
        let y = row_center
            .saturating_sub(i32::from(preview_height) / 2)
            .clamp(minimum_y, maximum_y);
        self.apply_wm_task_preview(target, placement, clamp_i16(x), clamp_i16(y));
    }

    pub(super) fn apply_wm_task_preview(
        &mut self,
        target: Option<WmTaskTarget>,
        placement: WmTaskPreviewPlacement,
        x: i16,
        y: i16,
    ) {
        let target = target.filter(|_| self.wm_task_previews_enabled);
        if target == self.wm_task_preview_target
            && (target.is_none() || placement == self.wm_task_preview_placement)
            && (target.is_none()
                || self
                    .windows
                    .get(&WM_TASK_PREVIEW.0)
                    .is_some_and(|preview| preview.x == x && preview.y == y))
        {
            return;
        }
        self.wm_task_preview_target = target;
        self.wm_task_preview_placement = placement;
        let preview_source = target.and_then(|target| {
            let source = match target {
                WmTaskTarget::Group(group) => {
                    let members = self.wm_task_group(group);
                    members
                        .iter()
                        .copied()
                        .find(|member| match member {
                            WmTaskTarget::X11(client) => {
                                self.focus_window == *client
                                    && !self.minimized_windows.contains(&client.0)
                            }
                            WmTaskTarget::Native(id) => self
                                .native_tasks
                                .iter()
                                .any(|task| task.id == *id && task.active && !task.minimized),
                            WmTaskTarget::Group(_) => false,
                        })
                        .or_else(|| {
                            members.iter().copied().find(|member| match member {
                                WmTaskTarget::X11(client) => {
                                    !self.minimized_windows.contains(&client.0)
                                }
                                WmTaskTarget::Native(id) => self
                                    .native_tasks
                                    .iter()
                                    .any(|task| task.id == *id && !task.minimized),
                                WmTaskTarget::Group(_) => false,
                            })
                        })
                        .or_else(|| members.first().copied())?
                }
                source => source,
            };
            Some(match source {
                WmTaskTarget::X11(client) => client.0,
                WmTaskTarget::Native(id) => WM_TASK_PREVIEW_NATIVE_PREFIX | u32::from(id),
                WmTaskTarget::Group(_) => return None,
            })
        });
        if let Some(preview) = self.windows.get_mut(&WM_TASK_PREVIEW.0) {
            preview.x = x;
            preview.y = y;
            preview.mapped = target.is_some();
            preview.background_pixel = preview_source.unwrap_or(0);
        }
        if target.is_some() {
            self.paint_wm_chrome(WM_TASK_PREVIEW);
            self.restack(WM_TASK_PREVIEW, None, Some(0));
        }
        self.scene_upsert(WM_TASK_PREVIEW);
        self.scene_damage(WM_TASK_PREVIEW);
    }
}
