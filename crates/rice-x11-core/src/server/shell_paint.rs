//! Shell paint operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    #[allow(clippy::too_many_lines)]
    pub(super) fn paint_wm_chrome(&mut self, id: ResourceId) {
        let dock_targets = if id == WM_TASKBAR && self.wm_dock_style { self.wm_dock_targets() } else { Vec::new() };
        let compact_dock = self.wm_dock_style;
        let dock_corner_radius = self.wm_dock_corner_radius;
        let dock_tasks: Vec<Option<WmTaskTarget>> = dock_targets.iter().map(|target| match target {WmChromeTarget::Task(task) => Some(*task), WmChromeTarget::QuickLaunch(index) => self.wm_dock_launcher_task(*index), _ => None,
            }).collect();
        let dock_counts: Vec<usize> = dock_tasks.iter().map(|target| match target {Some(WmTaskTarget::Group(group)) => self.wm_task_group(*group).len(), Some(_) => 1, _ => 0,
            }).collect();
        let dock_icon_sources: Vec<Option<WmTaskTarget>> = dock_tasks.iter().map(|task| match task {Some(WmTaskTarget::Group(group)) => self.wm_task_group(*group).first().copied(), other => *other,
            }).collect();
        let dock_step = if self.wm_dock_style { self.wm_dock_step() } else { 52 };
        let dock_bar_height = self.wm_dock_bar_height();
        let dock_icon_size = WM_COMPACT_DOCK_ICON_SIZE.min(dock_bar_height.saturating_sub(12));
        let quick_drop = self.wm_menu_drag.as_ref().and_then(|_| self.wm_quick_drop_position(self.pointer_x, self.pointer_y));
        let native_task_active = self
            .native_tasks
            .iter()
            .any(|task| task.workspace == self.current_workspace && task.active && !task.minimized);
        let frame_client = self.frame_clients.get(&id.0).copied();
        let frame_is_active =
            !native_task_active && frame_client.is_some_and(|client| client == self.focus_window);
        let frame_is_undecorated = frame_client
            .is_some_and(|client| self.managed_frame_insets(client) == (0, 0, 0, 0));
        let frame_border_only = frame_client.is_some_and(|client| {
            let (left, _, top, _) = self.managed_frame_insets(client);
            left > 0 && top == left
        });
        let frame_title = frame_client
            .map(|client| self.wm_window_title(client))
            .unwrap_or_default();
        let frame_icon = frame_client.and_then(|client| self.wm_window_icon(client));
        let task_targets = if id == WM_TASKBAR {
            self.wm_task_targets()
        } else {
            Vec::new()
        };
        let task_items = task_targets
            .into_iter()
            .filter_map(|target| {
                let native_icon = match target {
                    WmTaskTarget::Native(id) => self
                        .native_tasks
                        .iter()
                        .find(|task| task.id == id)
                        .map(|task| task.icon),
                    _ => None,
                };
                self.wm_task_item(target)
                    .map(|(title, icon, active)| (target, title, icon, native_icon, active))
            })
            .collect::<Vec<_>>();
        let task_slot_width = usize::try_from(self.wm_task_slot_width()).unwrap_or(132);
        let task_button_x = self.wm_task_button_x();
        let wm_taskbar_edge = self.wm_taskbar_edge;
        let tray_layout = if id == WM_TASKBAR && wm_taskbar_edge.vertical() {
            self.wm_vertical_tray_layout()
        } else if id == WM_TASKBAR {
            self.wm_tray_layout()
        } else {
            Vec::new()
        };
        let notification_icons = if id == WM_TASKBAR { self.notification_icon_layout().into_iter().filter_map(|(slot,x,y)| self.wm_notification_icons.iter().find(|icon| icon.slot == slot).map(|icon| (icon.clone(),x,y))).collect::<Vec<_>>() } else { Vec::new() };
        let notification_focus = self.wm_notification_focus;
        let tray_left = tray_layout.first().map_or(
            usize::from(self.config.width_px).saturating_sub(7),
            |item| item.2,
        );
        let tray_left = notification_icons.first().map_or(tray_left, |(_,x,_)| tray_left.min(*x));
        let clock_minutes = self.wm_clock_minutes;
        let performance_cpu = self.wm_performance_cpu.clone();
        let performance_network = self.wm_performance_network.clone();
        let workspace_names = self.wm_workspace_names.clone();
        let workspace_ids=self.wm_visible_workspace_ids();
        let active_workspace=self.output_workspace(self.wm_shell_output as u32);
        let workspace_widths = workspace_ids.iter()
            .map(|&workspace| self.wm_workspace_item_width(workspace))
            .collect::<Vec<_>>();
        let wm_chrome_hover = self.wm_chrome_hover;
        let dock_magnification = self.wm_dock_magnification;
        let dock_magnification_zoom = self.wm_dock_magnification_zoom;
        let dock_magnification_spread = self.wm_dock_magnification_spread;
        let dock_pointer_axis = if compact_dock
            && dock_magnification
            && wm_chrome_hover.is_some_and(|target| dock_targets.contains(&target))
        {
            let (dock_x, dock_y, _, _) = self.wm_taskbar_rect();
            let pointer = if wm_taskbar_edge.vertical() {
                i32::from(self.pointer_y) - i32::from(dock_y)
            } else {
                i32::from(self.pointer_x) - i32::from(dock_x)
            };
            usize::try_from(pointer).ok()
        } else {
            None
        };
        let wm_chrome_pressed = self.wm_chrome_pressed;
        let wm_gnome_panel = self.wm_gnome_panel;
        let wm_system_controls = self.wm_system_controls;
        let wm_battery_percentage = self.wm_battery_percentage;
        let wm_mail_unread = self.wm_mail_unread;
        let wm_system_controls_width = self.wm_system_controls_width();
        let compact_shell = self.wm_compact_shell();
        let wm_topbar_layout = self.wm_topbar_layout();
        let wm_topbar_managed = self.wm_topbar_config.is_some();
        let wm_gnome_indicator_layout = if id == WM_GNOME_PANEL {
            self.wm_gnome_indicator_layout()
        } else {
            Vec::new()
        };
        let wm_gnome_indicators = if id == WM_GNOME_PANEL {
            self.wm_gnome_indicators.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_menu_items = if id == WM_GNOME_MENU {
            self.wm_gnome_menu_items.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_menu_image = (id == WM_GNOME_MENU)
            .then(|| self.wm_gnome_menu_image.clone())
            .flatten();
        let wm_gnome_menu_icons = if id == WM_GNOME_MENU {
            self.wm_gnome_menu_icons.clone()
        } else {
            BTreeMap::new()
        };
        let wm_gnome_dialog_title = if id == WM_GNOME_DIALOG {
            self.wm_gnome_dialog_title.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_dialog_body = if id == WM_GNOME_DIALOG {
            self.wm_gnome_dialog_body.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_dialog_buttons = if id == WM_GNOME_DIALOG {
            self.wm_gnome_dialog_buttons.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_dialog_fullscreen = id == WM_GNOME_DIALOG && self.wm_gnome_dialog_fullscreen;
        let wm_gnome_overlay_rects = if id == WM_GNOME_DIALOG {
            self.wm_gnome_overlay_rects.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_background_rects = if id == WM_GNOME_BACKGROUND {
            self.wm_gnome_background_rects.clone()
        } else {
            Vec::new()
        };
        let wm_gnome_background_texts = if id == WM_GNOME_BACKGROUND {
            self.wm_gnome_background_texts.clone()
        } else {
            Vec::new()
        };
        let wm_menu_mapped = self.windows.get(&WM_MENU.0).is_some_and(|menu| menu.mapped);
        let menu_hover_row = self.wm_menu_hover_row.clone();
        let wm_menu_drag_action = self.wm_menu_drag.as_ref().map(|drag| drag.entry.action);
        let wm_menu_level = if matches!(
            self.wm_menu_level,
            WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext
        ) {
            self.wm_application_context_parent_level
        } else {
            self.wm_menu_level
        };
        let wm_menu_applications_only = self.wm_menu_applications_only;
        let wm_menu_icon_size = self.wm_menu_icon_size;
        let wm_theme_menu_row_height = self.wm_theme_menu_row_height;
        let wm_menu_offset = self.wm_menu_offset;
        let wm_menu_max_offset = match wm_menu_level {
            WmMenuLevel::Applications(category) => self.wm_application_max_offset(category),
            WmMenuLevel::Themes => self.wm_theme_max_offset(),
            WmMenuLevel::TaskGroup(group) => self.wm_task_group_max_offset(group),
            _ => 0,
        };
        let wm_theme = self.wm_theme;
        let wm_theme_palette = self
            .wm_themes
            .get(usize::from(wm_theme))
            .cloned()
            .unwrap_or_else(|| built_in_wm_themes().remove(0));
        let liquid_glass = wm_theme_palette.backdrop_material
            || (id == WM_MENU && wm_theme_palette.menu_backdrop_material);
        let wm_theme_names = self
            .wm_themes
            .iter()
            .map(|theme| theme.name.clone())
            .collect::<Vec<_>>();
        let (menu_columns, menu_column_widths) = if id == WM_MENU {
            (self.wm_menu_columns(), self.wm_menu_column_widths())
        } else {
            (Vec::new(), Vec::new())
        };
        let menu_column_offsets = if id == WM_MENU {
            self.wm_menu_column_offsets()
        } else {
            Vec::new()
        };
        let menu_column_x_offsets = if id == WM_MENU {
            self.wm_menu_column_x_offsets()
        } else {
            Vec::new()
        };
        let active_menu_folder_chain = match (id, wm_menu_level) {
            (WM_MENU, WmMenuLevel::MenuFolder(folder)) => self.wm_menu_folder_chain(folder),
            _ => Vec::new(),
        };
        let task_menu_items = menu_columns
            .iter()
            .flatten()
            .filter_map(|row| match row {
                WmMenuRow::Task(target) => self
                    .wm_task_item(*target)
                    .map(|(title, icon, _)| (*target, title, icon)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let notification = (id == WM_NOTIFICATION).then(|| {
            (
                self.wm_notification_kind,
                self.wm_notification_title.clone(),
                self.wm_notification_message.clone(),
            )
        });
        let tooltip = (id == WM_TOOLTIP)
            .then(|| self.wm_quick_launch_tooltip().map(<[u8]>::to_vec))
            .flatten();
        let preview = if id == WM_TASK_PREVIEW {
            self.wm_task_preview_target
                .and_then(|target| self.wm_task_item(target))
                .map(|(title, icon, active)| (title, icon, active, self.current_workspace))
        } else {
            None
        };
        let preview_placement = self.wm_task_preview_placement;
        // Resolve taskbar geometry before borrowing its backing surface. The
        // geometry helpers read the WM model while materialization below owns
        // a mutable surface borrow for the rest of the paint pass.
        let taskbar_start_button_width = (id == WM_TASKBAR).then(|| self.wm_start_button_width());
        let taskbar_quick_launch_x = (id == WM_TASKBAR).then(|| self.wm_quick_launch_x());
        let taskbar_vertical_quick_y = (id == WM_TASKBAR && wm_taskbar_edge.vertical())
            .then(|| self.wm_vertical_quick_launch_y());
        let taskbar_vertical_task_y =
            (id == WM_TASKBAR && wm_taskbar_edge.vertical()).then(|| self.wm_vertical_task_y());
        let mut text_overlays = Vec::new();
        let Some(surface) = self.surfaces.get_mut(&id.0) else {
            return;
        };
        if !surface.materialize() {
            return;
        }
        let width = usize::from(surface.width);
        let height = usize::from(surface.height);
        let Some(pixels) = surface.pixels.as_mut() else {
            return;
        };
        if frame_is_undecorated {
            // The client fills the frame exactly. Keep the backing frame fully
            // transparent so a frame:false Electron/Chromium window never
            // leaks a compositor titlebar or outline through its edges.
            pixels.fill(0);
        } else if frame_border_only {
            pixels.fill(wm_theme_palette.frame[2]);
        } else if id == WM_GNOME_BACKGROUND {
            pixels.fill(0);
            for rectangle in &wm_gnome_background_rects {
                fill_rect(
                    pixels,
                    width,
                    height,
                    usize::from(rectangle.x),
                    usize::from(rectangle.y),
                    usize::from(rectangle.width),
                    usize::from(rectangle.height),
                    rectangle.color,
                );
            }
            for text in &wm_gnome_background_texts {
                text_overlays.push(WmTextOverlay::new(
                    id,
                    usize::from(text.x),
                    usize::from(text.y),
                    usize::from(text.width),
                    usize::from(text.height),
                    text.font_px,
                    text.weight,
                    text.align,
                    text.foreground,
                    0x0100_0000,
                    &text.text,
                    32,
                ));
            }
        } else if id == WM_GNOME_PANEL && wm_gnome_panel {
            let panel = wm_theme_palette.taskbar[0];
            let panel_text = wm_theme_palette.taskbar[5];
            pixels.fill(panel);
            let hours = clock_minutes / 60;
            let minutes = clock_minutes % 60;
            let clock = format!("{hours:02}:{minutes:02}");
            let clock_item = wm_topbar_layout.iter().find(|item| item.key == "@clock");
            text_overlays.push(WmTextOverlay::new(
                id,
                clock_item.map_or(width.saturating_sub(84) / 2, |item| item.x.max(0) as usize),
                5,
                if wm_topbar_managed { clock_item.map_or(0, |item| usize::from(item.width)) } else {84},
                24,
                13,
                7,
                1,
                panel_text,
                panel,
                clock.as_bytes(),
                5,
            ));
            for item in &wm_topbar_layout {
                let marker = format!("__topbar\t{}\t{}\t{}\t{}", item.key, item.title, item.icon, item.position);
                text_overlays.push(WmTextOverlay::new(id, item.x.max(0) as usize, 0, usize::from(item.width), height, 0, 0, 0, panel_text, panel, marker.as_bytes(), 255,
                ));
            }
            for (indicator_order, indicator_x, indicator_width) in wm_gnome_indicator_layout.iter()
            {
                let Some(indicator) = wm_gnome_indicators.get(usize::from(*indicator_order)) else {
                    continue;
                };
                let indicator_x = usize::try_from(*indicator_x).unwrap_or(0);
                let mut text_x = indicator_x.saturating_add(8);
                if indicator.has_icon {
                    let icon_x = indicator_x.saturating_add(if wm_topbar_managed && indicator.label.is_empty() { usize::from(*indicator_width).saturating_sub(16)/2 } else { 8 },
                    );
                    if let Some(image) = &indicator.image {
                        let source_width = usize::from(image.width).max(1);
                        let source_height = usize::from(image.height).max(1);
                        let limit = if wm_topbar_managed { 16 } else { 18 };
                        let (draw_width, draw_height) = if 20 * source_height <= limit * source_width {
                            (20, (20 * source_height / source_width).max(1))
                        } else {
                            ((limit * source_width / source_height).max(1), limit)
                        };
                        let icon_y = height.saturating_sub(draw_height) / 2;
                        for target_y in 0..draw_height {
                            let source_y =
                                target_y * usize::from(image.height) / draw_height.max(1);
                            for target_x in 0..draw_width {
                                let source_x =
                                    target_x * usize::from(image.width) / draw_width.max(1);
                                if let Some(source) = image
                                    .pixels
                                    .get(source_y * usize::from(image.width) + source_x)
                                    && source >> 24 != 0
                                {
                                    set_surface_pixel(
                                        pixels,
                                        width,
                                        height,
                                        icon_x + target_x,
                                        icon_y + target_y,
                                        *source,
                                    );
                                }
                            }
                        }
                    } else {
                        paint_gnome_symbolic_icon(
                            pixels,
                            width,
                            height,
                            icon_x,
                            height.saturating_sub(16) / 2,
                            1,
                            panel_text,
                        );
                    }
                    text_x = text_x.saturating_add(20);
                }
                if !indicator.label.is_empty() {
                    text_overlays.push(WmTextOverlay::new(
                        id,
                        text_x,
                        5,
                        usize::from(*indicator_width)
                            .saturating_sub(text_x.saturating_sub(indicator_x))
                            .saturating_sub(6),
                        24,
                        13,
                        7,
                        0,
                        panel_text,
                        panel,
                        &indicator.label,
                        128,
                    ));
                }
            }
            if let Some(unread) = wm_mail_unread.filter(|_| !wm_topbar_managed) {
                let controls_top = wm_system_controls & 0x41 == 1;
                let reserved_right = if controls_top {
                    WM_SYSTEM_CONTROLS_RIGHT_MARGIN
                        .saturating_add(wm_system_controls_width)
                        .saturating_add(WM_SYSTEM_CONTROLS_EXTENSION_GAP)
                } else {
                    94
                };
                draw_rice_box_indicator(
                    pixels,
                    width,
                    height,
                    width
                        .saturating_sub(reserved_right)
                        .saturating_sub(WM_RICE_BOX_INDICATOR_WIDTH),
                    height.saturating_sub(25) / 2,
                    unread,
                    panel_text,
                    panel,
                );
            }
            if self.wm_system_controls & 0x41 == 1 && !wm_topbar_managed {
                draw_system_controls_cluster(
                    pixels,
                    width,
                    height,
                    width.saturating_sub(
                        wm_system_controls_width + WM_SYSTEM_CONTROLS_RIGHT_MARGIN),
                    3,
                    wm_system_controls,
                    wm_battery_percentage,
                    panel_text,
                    0x0080_8080,
                    panel,
                    0x0044_4444,
                );
            }
        } else if id == WM_GNOME_MENU {
            let menu = wm_theme_palette.menu[0];
            let menu_row = wm_theme_palette.menu[1];
            let menu_text = wm_theme_palette.menu[5];
            let menu_muted = wm_theme_palette.menu[5];
            pixels.fill(menu);
            let mut row_y = 6usize;
            for item in &wm_gnome_menu_items {
                let row_height = wm_gnome_menu_item_height(item);
                if row_y >= height {
                    break;
                }
                if item.image {
                    let inset = 12usize;
                    let available_width = width.saturating_sub(inset * 2);
                    let available_height = row_height.saturating_sub(8).min(height - row_y);
                    if let Some(image) = &wm_gnome_menu_image
                        && image.width > 0
                        && image.height > 0
                        && !image.pixels.is_empty()
                    {
                        let draw_width = available_width.min(usize::from(image.width));
                        let draw_height = available_height.min(usize::from(image.height));
                        let draw_x = inset + available_width.saturating_sub(draw_width) / 2;
                        for target_y in 0..draw_height {
                            let source_y =
                                target_y * usize::from(image.height) / draw_height.max(1);
                            for target_x in 0..draw_width {
                                let source_x =
                                    target_x * usize::from(image.width) / draw_width.max(1);
                                if let Some(source) = image
                                    .pixels
                                    .get(source_y * usize::from(image.width) + source_x)
                                {
                                    set_surface_pixel(
                                        pixels,
                                        width,
                                        height,
                                        draw_x + target_x,
                                        row_y + target_y,
                                        *source,
                                    );
                                }
                            }
                        }
                    } else {
                        fill_rect(
                            pixels,
                            width,
                            height,
                            inset,
                            row_y,
                            available_width,
                            available_height,
                            menu_row,
                        );
                    }
                } else if item.separator {
                    if item.label.is_empty() {
                        fill_rect(
                            pixels,
                            width,
                            height,
                            12,
                            row_y + 4,
                            width.saturating_sub(24),
                            1,
                            0x0050_5050,
                        );
                    } else {
                        text_overlays.push(WmTextOverlay::new(
                            id,
                            14,
                            row_y + 4,
                            width.saturating_sub(28),
                            22,
                            12,
                            7,
                            0,
                            menu_muted,
                            menu,
                            &item.label,
                            42,
                        ));
                    }
                } else {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        6,
                        row_y,
                        width.saturating_sub(12),
                        row_height.saturating_sub(2),
                        menu_row,
                    );
                    let text_x = if item.icon_kind != 0 { 46 } else { 16 };
                    if item.icon_kind != 0 {
                        if let Some(image) = wm_gnome_menu_icons.get(&item.index) {
                            for target_y in 0..18usize {
                                let source_y = target_y * usize::from(image.height) / 18;
                                for target_x in 0..22usize {
                                    let source_x = target_x * usize::from(image.width) / 22;
                                    if let Some(source) = image
                                        .pixels
                                        .get(source_y * usize::from(image.width) + source_x)
                                    {
                                        set_surface_pixel(
                                            pixels,
                                            width,
                                            height,
                                            16 + target_x,
                                            row_y + 8 + target_y,
                                            *source,
                                        );
                                    }
                                }
                            }
                        } else {
                            paint_gnome_symbolic_icon(
                                pixels,
                                width,
                                height,
                                18,
                                row_y + 10,
                                item.icon_kind,
                                menu_text,
                            );
                        }
                    }
                    if item.toggle {
                        let switch_width = 44usize;
                        let switch_height = 24usize;
                        let switch_x = width.saturating_sub(switch_width + 14);
                        let switch_y = row_y + 5;
                        fill_rounded_rect(
                            pixels,
                            width,
                            height,
                            switch_x,
                            switch_y,
                            switch_width,
                            switch_height,
                            switch_height / 2,
                            if item.checked {
                                0x00e9_5420
                            } else {
                                0x005d_5d62
                            },
                        );
                        let knob_size = 18usize;
                        let knob_x = switch_x
                            + if item.checked {
                                switch_width.saturating_sub(knob_size + 3)
                            } else {
                                3
                            };
                        fill_rounded_rect(
                            pixels,
                            width,
                            height,
                            knob_x,
                            switch_y + 3,
                            knob_size,
                            knob_size,
                            knob_size / 2,
                            0x00f4_f4f4,
                        );
                    }
                    text_overlays.push(WmTextOverlay::new(
                        id,
                        text_x,
                        row_y + 6,
                        width.saturating_sub(text_x + if item.toggle { 72 } else { 14 }),
                        24,
                        13,
                        6,
                        0,
                        menu_text,
                        menu_row,
                        &item.label,
                        48,
                    ));
                }
                row_y = row_y.saturating_add(row_height);
            }
        } else if id == WM_TASKBAR && self.wm_dock_style {
            let [bar, ridge, _, _, _, _, accent] = wm_theme_palette.taskbar;
            let orchard = wm_theme_palette.chrome_profile == WmChromeProfile::Orchard;
            let bar_thickness = dock_bar_height;
            let edge_inset = if orchard { 8 } else { 0 };
            let icon_inset = if !orchard && bar_thickness == WM_COMPACT_DOCK_BAR_THICKNESS {
                WM_COMPACT_DOCK_ICON_INSET
            } else {
                (bar_thickness.saturating_sub(dock_icon_size) / 2).max(6)
            };
            pixels.fill(0);
            let vertical = wm_taskbar_edge.vertical();
            let (bar_x, bar_y, bar_width, bar_height) = if vertical {
                (
                    if wm_taskbar_edge == WmTaskbarEdge::Right {
                        width.saturating_sub(bar_thickness + edge_inset)
                    } else {
                        edge_inset
                    },
                    0,
                    bar_thickness.min(width),
                    height,
                )
            } else {
                (
                    0,
                    if wm_taskbar_edge == WmTaskbarEdge::Bottom {
                        height.saturating_sub(bar_thickness + edge_inset)
                    } else {
                        edge_inset
                    },
                    width,
                    bar_thickness.min(height),
                )
            };
            fill_rounded_rect(
                pixels,
                width,
                height,
                bar_x,
                bar_y,
                bar_width,
                bar_height,
                dock_corner_radius,
                ridge,
            );
            fill_rounded_rect(
                pixels,
                width,
                height,
                bar_x + 1,
                bar_y + 1,
                bar_width.saturating_sub(2),
                bar_height.saturating_sub(2),
                dock_corner_radius.saturating_sub(1),
                bar,
            );
            for (index, target) in dock_targets.iter().enumerate() {
                let axis = WM_COMPACT_DOCK_AXIS_PADDING + index * dock_step;
                let size = dock_pointer_axis.map_or(dock_icon_size, |pointer| {
                        let center = axis + dock_step / 2;
                        let distance = center.abs_diff(pointer);
                        let reach = dock_step
                            .saturating_add(
                                dock_step
                                    .saturating_mul(2)
                                    .saturating_mul(usize::from(dock_magnification_spread))
                                    / 100,
                            )
                            .max(1);
                        if distance >= reach {
                            return dock_icon_size;
                        }
                        let influence = reach.saturating_sub(distance).saturating_mul(1024) / reach;
                        // Integer smoothstep gives the same soft shoulder as a
                        // fisheye lens without making the pointer path steppy.
                        let eased = influence
                            .saturating_mul(influence)
                            .saturating_mul(3072usize.saturating_sub(influence.saturating_mul(2)))
                            / (1024 * 1024);
                        let maximum = dock_icon_size
                            + (WM_COMPACT_DOCK_ICON_MAX_SIZE - dock_icon_size)
                                * usize::from(dock_magnification_zoom)
                                / 100;
                        dock_icon_size
                            + maximum
                                .saturating_sub(dock_icon_size)
                                .saturating_mul(eased)
                                / 1024
                    })
                    .min(WM_COMPACT_DOCK_ICON_MAX_SIZE);
                let x = if vertical {
                    if wm_taskbar_edge == WmTaskbarEdge::Right {
                        bar_x
                            .saturating_add(bar_width)
                            .saturating_sub(icon_inset)
                            .saturating_sub(size)
                    } else {
                        bar_x.saturating_add(icon_inset)
                    }
                } else {
                    (axis + dock_step / 2).saturating_sub(size / 2)
                };
                let y = if vertical {
                    (axis + dock_step / 2).saturating_sub(size / 2)
                } else if wm_taskbar_edge == WmTaskbarEdge::Bottom {
                    bar_y
                        .saturating_add(bar_height)
                        .saturating_sub(icon_inset)
                        .saturating_sub(size)
                } else {
                    bar_y.saturating_add(icon_inset)
                };
                let mut icon_pixels = vec![bar; 20 * 20];
                let (icon, bitmap, running) = match target {
                    WmChromeTarget::Start | WmChromeTarget::Activities => (1, None, false),
                    WmChromeTarget::QuickLaunch(i) => (self.wm_quick_launchers[usize::from(*i)].icon, None, dock_counts[index] > 0,
                    ),
                    WmChromeTarget::Task(task) => {
                        let item = task_items.iter().find(|item| item.0 == *task);
                        (item.and_then(|i| i.3).unwrap_or(1), item.and_then(|i| i.2.as_ref()), true,
                        )
                    }
                    _ => continue,
                };
                if *target == WmChromeTarget::Activities {
                    for row in 0..3 {for col in 0..3 {fill_rounded_rect(&mut icon_pixels,20,20,3+col*5,3+row*5,3,3,1,accent,
                            );}}
                } else if *target == WmChromeTarget::Start {
                    // Shield silhouette; the browser overlays the canonical RiceWM logo.
                    for row in 2..18 { let inset = if row < 11 {3} else {3+(row-10)/2}; fill_rect(&mut icon_pixels,20,20,inset,row,20-inset*2,1,accent,
                        ); }
                } else if let Some(bitmap) = bitmap { draw_argb_icon(&mut icon_pixels, 20, 20, 1, 1, bitmap); }
                else { draw_app_icon(&mut icon_pixels, 20, 20, 0, 0, icon, accent); }
                if let Some(color) = self.wm_dock_icon_color {
                    for pixel in &mut icon_pixels {if *pixel != bar {
                        let light = (((*pixel >> 16) & 255) * 54 + ((*pixel >> 8) & 255) * 183 + (*pixel & 255) * 19) / 256;
                        *pixel = mix_rgb(color, 0, (255-light)/2, 255);
                    }}
                }
                if matches!(dock_icon_sources[index], Some(WmTaskTarget::X11(_))) && matches!(target,WmChromeTarget::Task(_)) {
                for dy in 0..size { for dx in 0..size {
                    if x + dx < width && y + dy < height { pixels[(y + dy) * width + x + dx] = icon_pixels[(dy * 20 / size) * 20 + dx * 20 / size]; }
                }}
                }
                let icon_source = match target {
                    WmChromeTarget::Start => Some(String::from("__dock_icon:logo:0")),
                    WmChromeTarget::Activities => Some(String::from("__dock_icon:activities:0")),
                    WmChromeTarget::QuickLaunch(index) => {
                        Some(format!("__dock_icon:quick:{index}"))}
                    WmChromeTarget::Task(_) => match dock_icon_sources[index] {Some(WmTaskTarget::Native(id)) => Some(format!("__dock_icon:native:{id}")),_=>None,
                    },
                    _=>None,
                };
                if let Some(source)=icon_source {text_overlays.push(WmTextOverlay::new(id,x,y,size,size,13,6,0,accent,bar,source.as_bytes(),80,
                    ));}
                if running {
                    let (dot_x, dot_y) = if vertical {
                        (
                            if wm_taskbar_edge == WmTaskbarEdge::Right {
                                bar_x + bar_width.saturating_sub(8)
                            } else {
                                bar_x + 4
                            },
                            axis + dock_step / 2,
                        )
                    } else {
                        (
                            axis + dock_step / 2,
                            if wm_taskbar_edge == WmTaskbarEdge::Bottom {
                                bar_y + bar_height.saturating_sub(8)
                            } else {
                                bar_y + 4
                            },
                        )
                    };
                    let count = dock_counts[index].min(5);
                    for dot in 0..count {
                        let offset = dot * 7;
                        let x = if vertical {dot_x} else {(dot_x + offset).saturating_sub(count.saturating_sub(1)*7/2)};
                        let y = if vertical {(dot_y + offset).saturating_sub(count.saturating_sub(1)*7/2)} else {dot_y};
                        fill_rounded_rect(pixels, width, height, x, y, 4, 4, 2, accent,
                        );
                    }
                }
            }
        } else if id == WM_TASKBAR && wm_taskbar_edge.vertical() {
            let [mut bar, mut ridge, menu, task, tray, text, accent] = wm_theme_palette.taskbar;
            if self.wm_embedded_shell { bar = wm_theme_palette.menu[0]; ridge = bar; }
            let redmond98 = wm_theme_palette.chrome_profile == WmChromeProfile::Redmond98;
            pixels.fill(bar);
            let ridge_x = if wm_taskbar_edge == WmTaskbarEdge::Right {
                0
            } else {
                width.saturating_sub(2)
            };
            fill_rect(
                pixels,
                width,
                height,
                ridge_x,
                0,
                2.min(width),
                height,
                ridge,
            );

            if self.wm_gnome_dash_visible && self.wm_launcher_visible {
                let target = WmChromeTarget::Start;
                let active = wm_menu_mapped || wm_chrome_pressed == Some(target);
                let background = if active { accent } else { menu };
                fill_taskbar_control(
                    wm_theme_palette.chrome_profile,
                    pixels,
                    width,
                    height,
                    3,
                    3,
                    width.saturating_sub(6),
                    26,
                    background,
                );
                if redmond98 {
                    fill_redmond_bevel(
                        pixels,
                        width,
                        height,
                        3,
                        3,
                        width.saturating_sub(6),
                        26,
                        active,
                    );
                }
                // The browser composites the real RiceWM logo asset over this
                // square control. Do not paint the generic application glyph
                // underneath it: its opaque center leaked through the SVG and
                // made the vertical launcher look different from the
                // horizontal one.
            }

            if self.wm_gnome_dash_visible && self.wm_quick_launch_visible {
                let quick_y = taskbar_vertical_quick_y.unwrap_or(36);
                for (index, launcher) in self.wm_quick_launchers.iter().enumerate() {
                    let y = quick_y.saturating_add(index.saturating_mul(28));
                    let target =
                        WmChromeTarget::QuickLaunch(u8::try_from(index).unwrap_or_default());
                    let pressed = wm_chrome_pressed == Some(target);
                    let background = if pressed {
                        accent
                    } else if wm_chrome_hover == Some(target) {
                        task
                    } else {
                        tray
                    };
                    if pressed || wm_chrome_hover == Some(target) {
                        fill_taskbar_control(
                            wm_theme_palette.chrome_profile,
                            pixels,
                            width,
                            height,
                            3,
                            y,
                            width.saturating_sub(6),
                            25,
                            background,
                        );
                    }
                    draw_app_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(20) / 2,
                        y.saturating_add(3),
                        launcher.icon,
                        contrasting_text(background),
                    );
                }
            }

            if let Some(position) = quick_drop {
                let y = taskbar_vertical_quick_y.unwrap_or(36) + usize::from(position) * 28;
                fill_rect(pixels, width, height, 2, y, width.saturating_sub(4), 2, accent,
                );
            }
            let task_y = taskbar_vertical_task_y.unwrap_or(42);
            let tray_top = tray_layout
                .first()
                .map_or(height.saturating_sub(6), |item| item.2);
            for (index, (target, _title, icon, native_icon, active)) in
                task_items.into_iter().enumerate()
            {
                let y = task_y.saturating_add(index.saturating_mul(30));
                if y.saturating_add(26) >= tray_top {
                    break;
                }
                let chrome_target = WmChromeTarget::Task(target);
                let pressed = active || wm_chrome_pressed == Some(chrome_target);
                let background = if pressed {
                    accent
                } else if wm_chrome_hover == Some(chrome_target) {
                    tray
                } else {
                    task
                };
                fill_taskbar_control(
                    wm_theme_palette.chrome_profile,
                    pixels,
                    width,
                    height,
                    3,
                    y,
                    width.saturating_sub(6),
                    26,
                    background,
                );
                if let Some(icon) = icon {
                    draw_argb_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(18) / 2,
                        y + 4,
                        &icon,
                    );
                } else if redmond98 && let Some(icon) = native_icon.and_then(redmond_task_icon) {
                    draw_argb_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(18) / 2,
                        y + 4,
                        icon,
                    );
                } else {
                    draw_app_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(20) / 2,
                        y + 3,
                        native_icon.unwrap_or(0),
                        contrasting_text(background),
                    );
                }
                if self.wm_embedded_shell && active {
                    fill_rounded_rect(pixels, width, height, width / 4, y + 25, width / 2, 3, 1, contrasting_text(background),
                    );
                }
            }

            for (index, item, top, extent) in tray_layout {
                let tray_index = u8::try_from(index).unwrap_or_default();
                let target = WmChromeTarget::Tray(tray_index);
                let background = if wm_chrome_pressed == Some(target) {
                    accent
                } else if wm_chrome_hover == Some(target) {
                    task
                } else {
                    tray
                };
                match item {
                    TrayItem::Clock => {
                        fill_taskbar_control(
                            wm_theme_palette.chrome_profile,
                            pixels,
                            width,
                            height,
                            3,
                            top,
                            width.saturating_sub(6),
                            extent,
                            background,
                        );
                        let clock = format!("{:02}", clock_minutes / 60);
                        text_overlays.push(WmTextOverlay::new(
                            id,
                            3,
                            top.saturating_add(4),
                            width.saturating_sub(6),
                            extent.saturating_sub(6),
                            11,
                            6,
                            1,
                            text,
                            background,
                            clock.as_bytes(),
                            2,
                        ));
                    }
                    TrayItem::Workspaces => {
                        for (slot,&workspace) in workspace_ids.iter().enumerate() {
                            let y = top.saturating_add(slot.saturating_mul(28));
                            let active=workspace==active_workspace;
                            let workspace_background = if active { accent } else { background };
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                3,
                                y,
                                width.saturating_sub(6),
                                24,
                                workspace_background,
                            );
                            let label = format!("{}", workspace + 1);
                            text_overlays.push(WmTextOverlay::new(
                                id,
                                3,
                                y + 2,
                                width.saturating_sub(6),
                                20,
                                11,
                                7,
                                1,
                                contrasting_text(workspace_background),
                                workspace_background,
                                label.as_bytes(),
                                1,
                            ));
                        }
                    }
                    TrayItem::Performance => {
                        fill_taskbar_control(
                            wm_theme_palette.chrome_profile,
                            pixels,
                            width,
                            height,
                            3,
                            top,
                            width.saturating_sub(6),
                            extent,
                            background,
                        );
                        let cpu = performance_cpu.back().copied().unwrap_or_default();
                        let bar_height =
                            usize::from(cpu).saturating_mul(extent.saturating_sub(8)) / 100;
                        fill_rect(
                            pixels,
                            width,
                            height,
                            width / 2 - 2,
                            top.saturating_add(extent.saturating_sub(4 + bar_height)),
                            4,
                            bar_height.max(1),
                            accent,
                        );
                    }
                    TrayItem::Sound => {
                        fill_rect(
                            pixels,
                            width,
                            height,
                            width / 2 - 6,
                            top + 9,
                            4,
                            8,
                            contrasting_text(background),
                        );
                        fill_rect(
                            pixels,
                            width,
                            height,
                            width / 2 - 2,
                            top + 7,
                            2,
                            12,
                            contrasting_text(background),
                        );
                    }
                    TrayItem::Connectivity => draw_connectivity_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(20) / 2,
                        top + 5,
                        self.wm_connectivity_mode,
                        contrasting_text(background),
                    ),
                    TrayItem::Launcher(_) => draw_app_icon(
                        pixels,
                        width,
                        height,
                        width.saturating_sub(20) / 2,
                        top + 4,
                        0,
                        contrasting_text(background),
                    ),
                }
            }
        } else if id == WM_TASKBAR {
            let [mut bar, mut ridge, menu, task, tray, text, accent] = wm_theme_palette.taskbar;
            if self.wm_embedded_shell { bar = wm_theme_palette.menu[0]; ridge = bar; }
            let redmond98 = wm_theme_palette.chrome_profile == WmChromeProfile::Redmond98;
            // A Windows 98 Start button remains raised on hover. It becomes
            // sunken only while pressed or while the Start menu is open.
            let start_active = wm_menu_mapped
                || wm_chrome_pressed == Some(WmChromeTarget::Start)
                || (!redmond98 && wm_chrome_hover == Some(WmChromeTarget::Start));
            let start_background = if redmond98 {
                menu
            } else if start_active {
                accent
            } else {
                menu
            };
            let start_text = contrasting_text(start_background);
            let start_button_width = taskbar_start_button_width.unwrap_or_default();
            let quick_launch_x = taskbar_quick_launch_x.unwrap_or_default();
            for y in 0..height {
                for x in 0..width {
                    let color = if y < 2 {
                        ridge
                    } else if self.wm_gnome_dash_visible
                        && wm_theme_palette.chrome_profile != WmChromeProfile::NeonGrid
                        && x < usize::try_from(start_button_width).unwrap_or(112)
                    {
                        start_background
                    } else {
                        bar
                    };
                    pixels[y * width + x] = color;
                }
            }
            match wm_theme_palette.chrome_profile {
                WmChromeProfile::YellowJacket => {
                    fill_rect(pixels, width, height, 0, 0, width, 1, 0x00ff_ffff);
                    fill_rect(
                        pixels,
                        width,
                        height,
                        0,
                        height.saturating_sub(1),
                        width,
                        1,
                        0x0068_6868,
                    );
                }
                WmChromeProfile::CommandDeck => {
                    fill_rect(pixels, width, height, 0, 0, width, 3, ridge);
                    fill_rounded_rect(
                        pixels,
                        width,
                        height,
                        4,
                        5,
                        usize::try_from(start_button_width)
                            .unwrap_or(112)
                            .saturating_sub(8),
                        height.saturating_sub(9),
                        10,
                        start_background,
                    );
                }
                WmChromeProfile::Orchard => {
                    // The dock reserves this entire strip from the desktop.
                    // Clear inset margins expose the black root underneath it.
                    pixels.fill(bar);
                    fill_rounded_rect(
                        pixels,
                        width,
                        height,
                        7,
                        5,
                        usize::try_from(start_button_width)
                            .unwrap_or(112)
                            .saturating_sub(12),
                        height.saturating_sub(10),
                        10,
                        start_background,
                    );
                }
                WmChromeProfile::NeonGrid => {
                    // Keep the taskbar a calm, continuous HUD rail. Repeated
                    // ticks read as one enormous dashed control and compete
                    // with the actual task buttons.
                    fill_rect(pixels, width, height, 0, 0, width, 1, accent);
                    fill_rect(
                        pixels,
                        width,
                        height,
                        0,
                        height.saturating_sub(1),
                        width,
                        1,
                        accent,
                    );
                    fill_rect(pixels, width, height, 0, 1, width, 1, bar);
                    fill_wedged_rect(
                        pixels,
                        width,
                        height,
                        4,
                        5,
                        usize::try_from(start_button_width)
                            .unwrap_or(112)
                            .saturating_sub(8),
                        height.saturating_sub(9),
                        6,
                        start_background,
                    );
                }
                WmChromeProfile::SplitControls => {
                    fill_rect(pixels, width, height, 0, 0, width, 1, ridge);
                    fill_rect(
                        pixels,
                        width,
                        height,
                        0,
                        height.saturating_sub(1),
                        width,
                        1,
                        mix_rgb(bar, 0, 1, 2),
                    );
                }
                WmChromeProfile::Redmond98 => {
                    fill_rect(pixels, width, height, 0, 0, width, 1, 0x00ff_ffff);
                    fill_rect(pixels, width, height, 0, 1, width, 1, 0x00df_dfdf);
                    if self.wm_gnome_dash_visible {
                        fill_redmond_bevel(
                            pixels,
                            width,
                            height,
                            2,
                            4,
                            usize::try_from(start_button_width)
                                .unwrap_or(112)
                                .saturating_sub(5),
                            24,
                            start_active,
                        );
                    }
                }
                WmChromeProfile::Standard | WmChromeProfile::Familiar => {}
            }
            if self.wm_gnome_dash_visible && self.wm_launcher_visible && (self.wm_launcher_appearance != 0 || redmond98) {
                let text_x = if redmond98 {
                    23
                } else if self.wm_launcher_appearance == 2 {
                    8
                } else {
                    32
                };
                let pressed_offset = usize::from(redmond98 && start_active);
                let label = if redmond98 {
                    rice_i18n::t("Start").as_bytes()
                } else {
                    self.wm_launcher_text.as_slice()
                };
                text_overlays.push(WmTextOverlay::new(
                    id,
                    text_x + pressed_offset,
                    if redmond98 { 5 + pressed_offset } else { 7 },
                    usize::try_from(start_button_width)
                        .unwrap_or(112)
                        .saturating_sub(text_x + 4),
                    if redmond98 { 22 } else { 21 },
                    13,
                    7,
                    1,
                    start_text,
                    start_background,
                    label,
                    24,
                ));
            }

            // User-configured quick shortcuts stay separate from task buttons.
            if redmond98 && self.wm_gnome_dash_visible && self.wm_quick_launch_visible {
                fill_rect(
                    pixels,
                    width,
                    height,
                    usize::try_from(quick_launch_x)
                        .unwrap_or(70)
                        .saturating_sub(5),
                    6,
                    1,
                    20,
                    0x0080_8080,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    usize::try_from(quick_launch_x)
                        .unwrap_or(70)
                        .saturating_sub(4),
                    6,
                    1,
                    20,
                    0x00ff_ffff,
                );
            }
            for (index, launcher) in self
                .wm_quick_launchers
                .iter()
                .enumerate()
                .filter(|_| self.wm_gnome_dash_visible && self.wm_quick_launch_visible)
            {
                let x = usize::try_from(quick_launch_x).unwrap_or(120) + index * 28;
                let target = WmChromeTarget::QuickLaunch(u8::try_from(index).unwrap_or_default());
                let pressed = wm_chrome_pressed == Some(target);
                let hovered = wm_chrome_hover == Some(target);
                let background = if pressed {
                    accent
                } else if hovered {
                    task
                } else if redmond98 {
                    bar
                } else {
                    tray
                };
                let icon_color = contrasting_text(background);
                if redmond98 {
                    if pressed || hovered {
                        fill_redmond_bevel(pixels, width, height, x, 4, 25, 24, pressed);
                    }
                } else if pressed || hovered {
                    fill_taskbar_control(
                        wm_theme_palette.chrome_profile,
                        pixels,
                        width,
                        height,
                        x,
                        4,
                        25,
                        24,
                        background,
                    );
                }
                if redmond98 && let Some(icon) = redmond_task_icon(launcher.icon) {
                    draw_argb_icon(pixels, width, height, x + 5, 8, icon);
                } else {
                    draw_app_icon(pixels, width, height, x + 3, 7, launcher.icon, icon_color);
                }
            }

            if let Some(position) = quick_drop {
                let x = taskbar_quick_launch_x.unwrap_or(120) as usize + usize::from(position) * 28;
                fill_rect(pixels, width, height, x.saturating_sub(2), 3, 2, height.saturating_sub(6), accent,
                );
            }
            let mut task_x = usize::try_from(task_button_x).unwrap_or(212);
            let task_button_width = task_slot_width.saturating_sub(6);
            for (target, title, icon, native_icon, active) in task_items {
                if task_x + task_button_width >= tray_left.saturating_sub(2) {
                    break;
                }
                let chrome_target = WmChromeTarget::Task(target);
                let task_pressed = wm_chrome_pressed == Some(chrome_target) || active;
                let content_offset = usize::from(redmond98 && task_pressed);
                let task_background = if redmond98 {
                    task
                } else if task_pressed {
                    accent
                } else if wm_chrome_hover == Some(chrome_target) {
                    tray
                } else {
                    task
                };
                let task_text = contrasting_text(task_background);
                if matches!(target, WmTaskTarget::Group(_)) {
                    fill_taskbar_control(
                        wm_theme_palette.chrome_profile,
                        pixels,
                        width,
                        height,
                        task_x + 4,
                        2,
                        task_button_width.saturating_sub(4),
                        22,
                        ridge,
                    );
                    fill_taskbar_control(
                        wm_theme_palette.chrome_profile,
                        pixels,
                        width,
                        height,
                        task_x + 2,
                        4,
                        task_button_width.saturating_sub(2),
                        22,
                        tray,
                    );
                }
                if wm_theme_palette.chrome_profile == WmChromeProfile::NeonGrid {
                    fill_wedged_rect(
                        pixels,
                        width,
                        height,
                        task_x,
                        5,
                        task_button_width,
                        22,
                        7,
                        task_background,
                    );
                } else {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        task_x,
                        5,
                        task_button_width,
                        22,
                        task_background,
                    );
                    if redmond98 {
                        fill_redmond_bevel(
                            pixels,
                            width,
                            height,
                            task_x,
                            5,
                            task_button_width,
                            22,
                            task_pressed,
                        );
                    }
                }
                if let Some(icon) = icon {
                    draw_argb_icon(
                        pixels,
                        width,
                        height,
                        task_x + 4 + content_offset,
                        8 + content_offset,
                        &icon,
                    );
                } else if redmond98 && let Some(icon) = native_icon.and_then(redmond_task_icon) {
                    draw_argb_icon(
                        pixels,
                        width,
                        height,
                        task_x + 5 + content_offset,
                        8 + content_offset,
                        icon,
                    );
                } else {
                    // Always retain a real task icon in the X11 surface. The
                    // browser may replace known Redmond applications with a
                    // copied colour icon, but unknown/truncated titles must
                    // never leave an empty task button.
                    draw_app_icon(
                        pixels,
                        width,
                        height,
                        task_x + 3,
                        7,
                        native_icon.unwrap_or(0),
                        task_text,
                    );
                }
                text_overlays.push(WmTextOverlay::new(
                    id,
                    task_x + 25 + content_offset,
                    5 + content_offset,
                    task_button_width.saturating_sub(27 + content_offset),
                    22,
                    12,
                    6,
                    0,
                    task_text,
                    task_background,
                    &title,
                    task_button_width.saturating_sub(27 + content_offset) / 6,
                ));
                if self.wm_embedded_shell && active {
                    fill_rounded_rect(pixels, width, height, task_x + task_button_width / 4, 28, task_button_width / 2, 3, 1, accent,
                    );
                }
                task_x += task_slot_width;
            }
            if redmond98
                && let (Some(first), Some(last)) = (tray_layout.first(), tray_layout.last())
            {
                let left = first.2.saturating_sub(2);
                let right = last.2.saturating_add(last.3).saturating_add(2);
                fill_redmond_bevel(
                    pixels,
                    width,
                    height,
                    left,
                    4,
                    right.saturating_sub(left),
                    24,
                    true,
                );
            }
            for (index, item, item_x, item_width) in tray_layout {
                let tray_index = u8::try_from(index).unwrap_or_default();
                let target = WmChromeTarget::Tray(tray_index);
                let background = if redmond98 {
                    tray
                } else if wm_chrome_pressed == Some(target) {
                    accent
                } else if wm_chrome_hover == Some(target) {
                    task
                } else {
                    tray
                };
                match item {
                    TrayItem::Clock => {
                        let hours = clock_minutes / 60;
                        let minutes = clock_minutes % 60;
                        let clock = format!("{hours:02}:{minutes:02}");
                        if !redmond98 {
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                item_x,
                                5,
                                item_width,
                                22,
                                background,
                            );
                        }
                        text_overlays.push(WmTextOverlay::new(
                            id,
                            item_x,
                            5,
                            item_width,
                            22,
                            12,
                            6,
                            1,
                            text,
                            background,
                            clock.as_bytes(),
                            5,
                        ));
                    }
                    TrayItem::Workspaces => {
                        let mut workspace_x = item_x;
                        for (slot,&workspace) in workspace_ids.iter().enumerate() {
                            let item_width = workspace_widths.get(slot).copied().unwrap_or(28);
                            let x = workspace_x;
                            let workspace_target = WmChromeTarget::Workspace {
                                tray: tray_index,
                                workspace,
                            };
                            let active=workspace==active_workspace;
                            let workspace_background =
                                if active || wm_chrome_pressed == Some(workspace_target) {
                                    accent
                                } else if wm_chrome_hover == Some(workspace_target) {
                                    task
                                } else {
                                    tray
                                };
                            let workspace_text = contrasting_text(workspace_background);
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                x,
                                6,
                                item_width.saturating_sub(4),
                                20,
                                workspace_background,
                            );
                            if redmond98 {
                                // RiceWM workspaces are an extension of the
                                // Windows 98 tray, so present them as compact
                                // physical selector buttons inside the
                                // right-anchored recessed tray. The current
                                // desktop is sunken; the others remain raised.
                                fill_redmond_bevel(
                                    pixels,
                                    width,
                                    height,
                                    x,
                                    6,
                                    item_width.saturating_sub(4),
                                    20,
                                    active || wm_chrome_pressed == Some(workspace_target),
                                );
                            }
                            let label = workspace_names
                                .get(workspace as usize)
                                .cloned()
                                .unwrap_or_else(|| format!("{}", workspace + 1).into_bytes());
                            text_overlays.push(WmTextOverlay::new(
                                id,
                                x,
                                6,
                                item_width.saturating_sub(4),
                                20,
                                12,
                                7,
                                1,
                                workspace_text,
                                workspace_background,
                                &label,
                                item_width.saturating_sub(12) / 7,
                            ));
                            workspace_x = workspace_x.saturating_add(item_width);
                        }
                    }
                    TrayItem::Performance => {
                        if redmond98 {
                            // Keep the CPU/storage monitor as one deliberate
                            // Win98 tray instrument: a sunken outer well, a
                            // reserved icon bay, and a black oscilloscope-like
                            // plot. The browser overlays the live storage/CPU
                            // glyph into that bay; it is not a loose icon on
                            // top of the taskbar anymore.
                            fill_redmond_bevel(
                                pixels, width, height, item_x, 4, item_width, 24, true,
                            );
                            fill_rect(
                                pixels,
                                width,
                                height,
                                item_x.saturating_add(20),
                                7,
                                item_width.saturating_sub(24),
                                18,
                                0x000000,
                            );
                        } else {
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                item_x,
                                4,
                                item_width,
                                24,
                                background,
                            );
                        }
                        let plot_height = 18usize;
                        let plot_left = if redmond98 { 21 } else { 2 };
                        let plot_width = item_width.saturating_sub(if redmond98 { 25 } else { 4 });
                        for (sample, cpu) in performance_cpu.iter().enumerate() {
                            let x = item_x
                                + plot_left
                                + sample * plot_width / performance_cpu.len().max(1);
                            let bar = usize::from(*cpu).min(100) * plot_height / 100;
                            fill_rect(
                                pixels,
                                width,
                                height,
                                x,
                                24usize.saturating_sub(bar),
                                2,
                                bar.max(1),
                                if redmond98 { 0x00ff00 } else { accent },
                            );
                        }
                        for (sample, network) in performance_network.iter().enumerate() {
                            let x = item_x
                                + plot_left
                                + sample * plot_width / performance_network.len().max(1);
                            let y = 23usize
                                .saturating_sub(usize::from(*network).min(100) * plot_height / 100);
                            fill_rect(
                                pixels,
                                width,
                                height,
                                x,
                                y,
                                2,
                                2,
                                if redmond98 { 0x80ff80 } else { text },
                            );
                        }
                    }
                    TrayItem::Sound => {
                        let color = contrasting_text(background);
                        if !redmond98 {
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                item_x,
                                4,
                                item_width,
                                24,
                                background,
                            );
                        }
                        fill_rect(pixels, width, height, item_x + 4, 12, 4, 8, color);
                        for offset in 0..5 {
                            fill_rect(
                                pixels,
                                width,
                                height,
                                item_x + 8 + offset,
                                10usize.saturating_sub(offset / 2),
                                1,
                                12 + offset,
                                color,
                            );
                        }
                        if self.wm_volume_muted || self.wm_volume == 0 {
                            for offset in 0..7 {
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    item_x + 15 + offset,
                                    11 + offset,
                                    2,
                                    2,
                                    0x00ef_4444,
                                );
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    item_x + 21usize.saturating_sub(offset),
                                    11 + offset,
                                    2,
                                    2,
                                    0x00ef_4444,
                                );
                            }
                        } else {
                            let waves = usize::from(self.wm_volume.saturating_add(24) / 25).min(4);
                            for wave in 0..waves {
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    item_x + 15 + wave * 2,
                                    12usize.saturating_sub(wave),
                                    1,
                                    8 + wave * 2,
                                    color,
                                );
                            }
                        }
                    }
                    TrayItem::Connectivity => {
                        let color = contrasting_text(background);
                        if !redmond98 {
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                item_x,
                                4,
                                item_width,
                                24,
                                background,
                            );
                        }
                        draw_connectivity_icon(
                            pixels,
                            width,
                            height,
                            item_x + 3,
                            7,
                            self.wm_connectivity_mode,
                            color,
                        );
                    }
                    TrayItem::Launcher(launcher) => {
                        let _ = launcher;
                        if !redmond98 {
                            fill_taskbar_control(
                                wm_theme_palette.chrome_profile,
                                pixels,
                                width,
                                height,
                                item_x,
                                4,
                                item_width,
                                24,
                                background,
                            );
                        }
                    }
                }
            }
            if !compact_shell && self.wm_system_controls & 0x41 == 0x41 {
                draw_system_controls_cluster(
                    pixels,
                    width,
                    height,
                    width.saturating_sub(
                        wm_system_controls_width + WM_SYSTEM_CONTROLS_RIGHT_MARGIN),
                    4,
                    wm_system_controls,
                    wm_battery_percentage,
                    text,
                    contrasting_text(bar),
                    bar,
                    tray,
                );
            }
        } else if id == WM_GNOME_DIALOG {
            const DIALOG_BG: u32 = 0x002e_2933;
            const DIALOG_TEXT: u32 = 0x00f4_f4f4;
            const DIALOG_BUTTON: u32 = 0x003d_3846;
            const DIALOG_BUTTON_TEXT: u32 = 0x00ff_ffff;
            pixels.fill(DIALOG_BG);
            if wm_gnome_dialog_fullscreen {
                fill_rect(
                    pixels,
                    width,
                    height,
                    0,
                    0,
                    width,
                    58usize.min(height),
                    0x001e_1e1e,
                );
                for rectangle in &wm_gnome_overlay_rects {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        usize::from(rectangle.x),
                        usize::from(rectangle.y),
                        usize::from(rectangle.width),
                        usize::from(rectangle.height),
                        rectangle.color & 0x00ff_ffff,
                    );
                }
            }
            if !wm_gnome_dialog_title.is_empty() {
                text_overlays.push(WmTextOverlay::new(
                    id,
                    24,
                    if wm_gnome_dialog_fullscreen { 16 } else { 18 },
                    width.saturating_sub(48),
                    28,
                    17,
                    0,
                    1,
                    DIALOG_TEXT,
                    DIALOG_BG,
                    &wm_gnome_dialog_title,
                    60,
                ));
            }
            let body_bottom = height.saturating_sub(132);
            for (index, line) in wm_gnome_dialog_body.iter().enumerate() {
                let line_y = if wm_gnome_dialog_fullscreen {
                    height
                        .saturating_sub(170)
                        .saturating_add(index.saturating_mul(22))
                } else {
                    58usize.saturating_add(index.saturating_mul(22))
                };
                if line_y.saturating_add(20) > body_bottom {
                    break;
                }
                text_overlays.push(WmTextOverlay::new(
                    id,
                    28,
                    line_y,
                    width.saturating_sub(56),
                    20,
                    14,
                    0,
                    0,
                    DIALOG_TEXT,
                    DIALOG_BG,
                    line,
                    120,
                ));
            }
            let count = wm_gnome_dialog_buttons.len();
            let columns = count.clamp(1, 4);
            let rows = count.div_ceil(columns);
            let button_width = width.saturating_sub(48 + (columns - 1) * 10) / columns;
            let total_width = columns.saturating_mul(button_width + 10).saturating_sub(10);
            let start_x = width.saturating_sub(total_width) / 2;
            let first_button_y = height.saturating_sub(rows.saturating_mul(44) + 10);
            for (index, label) in wm_gnome_dialog_buttons.iter().enumerate() {
                let column = index % columns;
                let row = index / columns;
                let left = start_x + column * (button_width + 10);
                let button_y = first_button_y + row * 44;
                fill_rect(
                    pixels,
                    width,
                    height,
                    left,
                    button_y,
                    button_width,
                    40,
                    DIALOG_BUTTON,
                );
                text_overlays.push(WmTextOverlay::new(
                    id,
                    left,
                    button_y + 10,
                    button_width,
                    24,
                    14,
                    0,
                    1,
                    DIALOG_BUTTON_TEXT,
                    DIALOG_BUTTON,
                    label,
                    30,
                ));
            }
        } else if id == WM_TOOLTIP {
            pixels.fill(0);
            if let Some(title) = tooltip {
                let [background, raised, border, _, _, text] = wm_theme_palette.menu;
                fill_rounded_rect(pixels, width, height, 0, 0, width, height, 5, background);
                fill_rect(
                    pixels,
                    width,
                    height,
                    4,
                    3,
                    width.saturating_sub(8),
                    1,
                    raised,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    4,
                    height.saturating_sub(2),
                    width.saturating_sub(8),
                    1,
                    border,
                );
                text_overlays.push(WmTextOverlay::new(
                    id,
                    8,
                    3,
                    width.saturating_sub(16),
                    height.saturating_sub(6),
                    12,
                    6,
                    1,
                    text,
                    background,
                    &title,
                    16,
                ));
            }
        } else if id == WM_MENU {
            let [background, alternate, separator, hover, selected, text] = wm_theme_palette.menu;
            let command_deck = wm_theme_palette.chrome_profile == WmChromeProfile::CommandDeck;
            let redmond98 = wm_theme_palette.chrome_profile == WmChromeProfile::Redmond98;
            let menu_row_height = if compact_dock && matches!(wm_menu_level, WmMenuLevel::TaskGroup(_)) {144} else {
                if wm_theme_menu_row_height>0 {wm_theme_menu_row_height} else {wm_menu_base_row_height_with(redmond98, usize::from(wm_menu_icon_size))}};
            let redmond_start_menu = redmond98
                && matches!(
                    wm_menu_level,
                    WmMenuLevel::Root
                        | WmMenuLevel::Categories
                        | WmMenuLevel::Applications(_)
                        | WmMenuLevel::MenuFolders
                        | WmMenuLevel::MenuFolder(_)
                        | WmMenuLevel::Themes
                );
            // The cascades share one retained drawable for fast, atomic
            // presentation. Keep the unused area transparent so every
            // fly-out visibly ends after its own final row.
            pixels.fill(0);
            let profiled_menu = matches!(
                wm_theme_palette.chrome_profile,
                WmChromeProfile::CommandDeck | WmChromeProfile::Orchard | WmChromeProfile::NeonGrid
            );
            let selection_inset = if redmond98 {
                // Preserve the popup's two-pixel classic bevel on every side.
                3
            } else if profiled_menu {
                3
            } else {
                WM_MENU_SELECTION_INSET
            };
            let selection_radius = if wm_theme_palette.chrome_profile == WmChromeProfile::NeonGrid {
                2
            } else if profiled_menu {
                15
            } else {
                WM_MENU_SELECTION_RADIUS
            };
            for (column, entries) in menu_columns.iter().enumerate() {
                let column_width = menu_column_widths.get(column).copied().unwrap_or(1);
                let column_x = menu_column_x_offsets
                    .get(column)
                    .copied()
                    .unwrap_or_default();
                let column_y = menu_column_offsets.get(column).copied().unwrap_or_default();
                let column_height = wm_menu_rows_height_with(entries, menu_row_height)
                    .min(height.saturating_sub(column_y));
                let column_background = if column == 0 { background } else { alternate };
                // The drawable reserves room for future cascades. Every panel
                // must expose its content bounds, even when no panels overlap,
                // so browser skins never fill the unused transparent area.
                text_overlays.push(WmTextOverlay::new(
                    id, column_x, column_y, column_width, column_height,
                    0, 0, 0, separator, column_background, b"__menu_layer", 32,
                ));
                if wm_theme_palette.chrome_profile == WmChromeProfile::NeonGrid {
                    fill_chamfered_rect(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y,
                        column_width,
                        column_height,
                        9,
                        column > 0,
                        column + 1 < menu_columns.len(),
                        column_background,
                    );
                } else if liquid_glass || profiled_menu {
                    fill_rounded_rect(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y,
                        column_width,
                        column_height,
                        12,
                        column_background,
                    );
                } else {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y,
                        column_width,
                        column_height,
                        column_background,
                    );
                }
                let has_rail = redmond_start_menu && column == 0;
                let rail_width = if has_rail { 24 } else { 0 };
                if redmond98 {
                    fill_redmond_bevel(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y,
                        column_width,
                        column_height,
                        false,
                    );
                    if has_rail && column_height > 6 {
                        let rail_height = column_height.saturating_sub(6);
                        for rail_y in 0..rail_height {
                            fill_rect(
                                pixels,
                                width,
                                height,
                                column_x + 3,
                                column_y + 3 + rail_y,
                                22,
                                1,
                                mix_rgb(
                                    0x0000_00b8,
                                    0x0000_0048,
                                    u32::try_from(rail_y).unwrap_or(u32::MAX),
                                    u32::try_from(rail_height.max(1)).unwrap_or(u32::MAX),
                                ),
                            );
                        }
                        let label_height = rail_height.min(92);
                        text_overlays.push(WmTextOverlay::new_rotated_ccw(
                            id,
                            column_x + 3,
                            column_y + 3 + rail_height.saturating_sub(label_height),
                            22,
                            label_height,
                            16,
                            7,
                            0x00ff_ffff,
                            0x0000_0048,
                            b"RiceWM",
                            6,
                        ));
                    }
                }
                if column > 0 && !command_deck && !redmond98 {
                    fill_rect(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y + 6,
                        1,
                        column_height.saturating_sub(12),
                        separator,
                    );
                }
                let mut row_y = column_y;
                for (row, entry) in entries.iter().enumerate() {
                    let row_height = wm_menu_row_height_with(entry, menu_row_height);
                    let y = row_y;
                    row_y = row_y.saturating_add(row_height);
                    if y + row_height > height {
                        break;
                    }
                    let hover_entry = menu_hover_row.as_ref().and_then(
                        |(hover_column, hover_row, hover_entry)| {
                            (*hover_column == column && *hover_row == row).then_some(hover_entry)
                        },
                    );
                    let workspace_navigation = matches!(entry, WmMenuRow::WorkspaceNavigation);
                    let browser_icon_kind = wm_menu_row_browser_icon_kind(entry);
                    let browser_icon = browser_icon_kind.is_some();
                    let icon_size = usize::from(wm_menu_icon_size);
                    let hovered = hover_entry.is_some()
                        && !workspace_navigation
                        && !matches!(entry, WmMenuRow::Divider);
                    let dragged = matches!(entry, WmMenuRow::Application(application)
                        if Some(application.action) == wm_menu_drag_action);
                    let parent_selected = match (column, entry, wm_menu_level) {
                        (
                            0,
                            WmMenuRow::Applications,
                            WmMenuLevel::Categories
                            | WmMenuLevel::Applications(_)
                            | WmMenuLevel::MenuFolders
                            | WmMenuLevel::MenuFolder(_),
                        )
                        | (0, WmMenuRow::Themes, WmMenuLevel::Themes) => true,
                        (
                            column,
                            WmMenuRow::Category(category),
                            WmMenuLevel::Applications(active_category),
                        ) if column == usize::from(!wm_menu_applications_only) => {
                            *category == active_category
                        }
                        (_, WmMenuRow::MenuFolder(folder), WmMenuLevel::MenuFolder(_)) => {
                            active_menu_folder_chain.contains(folder)
                        }
                        (1, WmMenuRow::Theme(theme), WmMenuLevel::Themes) => *theme == wm_theme,
                        _ => false,
                    };
                    if hovered || parent_selected || dragged {
                        fill_rounded_rect(
                            pixels,
                            width,
                            height,
                            column_x + selection_inset + rail_width,
                            y + selection_inset,
                            column_width
                                .saturating_sub(selection_inset.saturating_mul(2) + rail_width),
                            row_height.saturating_sub(selection_inset.saturating_mul(2)),
                            selection_radius,
                            if hovered { hover } else { selected },
                        );
                    }
                    let (label, icon): (&[u8], u8) = match entry {
                        WmMenuRow::Applications => (
                            if redmond98 {
                                rice_i18n::t("Programs").as_bytes()
                            } else {
                                rice_i18n::t("Applications").as_bytes()
                            },
                            1,
                        ),
                        WmMenuRow::Run => (rice_i18n::t("Run...").as_bytes(), 2),
                        WmMenuRow::Themes => (if redmond98 { rice_i18n::t("Settings").as_bytes() } else { rice_i18n::t("Themes").as_bytes() }, 4),
                        WmMenuRow::RedmondShortcut(shortcut) => match shortcut {
                            RedmondMenuShortcut::Documents => (rice_i18n::t("Documents").as_bytes(), 1),
                        },
                        WmMenuRow::Theme(theme) => (
                            wm_theme_names
                                .get(usize::from(*theme))
                                .map_or(rice_i18n::t("Theme").as_bytes(), Vec::as_slice),
                            4,
                        ),
                        WmMenuRow::Category(category) => {
                            (wm_category_label(*category), wm_category_icon(*category))
                        }
                        WmMenuRow::MenuFolder(folder) => (
                            self.wm_menu_folders
                                .get(usize::from(*folder))
                                .map_or(rice_i18n::t("Folder").as_bytes(), |folder| folder.label.as_slice()),
                            1,
                        ),
                        WmMenuRow::Application(application) => {
                            (&application.label, application.icon)
                        }
                        WmMenuRow::ApplicationLaunch => (rice_i18n::t("Launch").as_bytes(), 0),
                        WmMenuRow::ApplicationOpenFolder => (rice_i18n::t("Open containing folder").as_bytes(), 1),
                        WmMenuRow::ApplicationUninstall => (rice_i18n::t("Uninstall").as_bytes(), 6),
                        WmMenuRow::MenuOpenDirectory => (rice_i18n::t("Open Menu Directory").as_bytes(), 1),
                        WmMenuRow::ScrollDown | WmMenuRow::ScrollUp => (b"", 0),
                        WmMenuRow::RestoreWindows => (rice_i18n::t("Restore windows").as_bytes(), 4),
                        WmMenuRow::NextWorkspace => (rice_i18n::t("Next workspace").as_bytes(), 5),
                        WmMenuRow::PreviousWorkspace => (rice_i18n::t("Previous workspace").as_bytes(), 6),
                        WmMenuRow::WorkspaceNavigation => (rice_i18n::t("Workspaces").as_bytes(), 5),
                        WmMenuRow::ShowDesktop => (rice_i18n::t("Show desktop").as_bytes(), 12),
                        WmMenuRow::DockSettings => (rice_i18n::t("Open dock settings").as_bytes(), 13),
                        WmMenuRow::ToolbarPreferences => (rice_i18n::t("Toolbar preferences").as_bytes(), 13),
                        WmMenuRow::QuickShortcutPreferences => (rice_i18n::t("Adjust quick shortcuts").as_bytes(), 13),
                        WmMenuRow::QuickShortcutRemove => (rice_i18n::t("Remove shortcut").as_bytes(), 8),
                        WmMenuRow::ThemeCustomize => (rice_i18n::t("Customize").as_bytes(), 13),
                        WmMenuRow::Divider => (b"", 0),
                        WmMenuRow::Exit => (
                            if self.wm_session_logout {
                                rice_i18n::t("Log out...").as_bytes()
                            } else if redmond98 {
                                rice_i18n::t("Shut Down...").as_bytes()
                            } else {
                                rice_i18n::t("Exit RiceWM").as_bytes()
                            },
                            10,
                        ),
                        WmMenuRow::Task(target) => {
                            let title = task_menu_items
                                .iter()
                                .find(|(candidate, _, _)| candidate == target)
                                .map_or(rice_i18n::t("shell.application").as_bytes(), |(_, title, _)| {
                                    title.as_slice()
                                });
                            (title, 0)
                        }
                        WmMenuRow::WindowClose => (rice_i18n::t("Close").as_bytes(), 6),
                        WmMenuRow::WindowMaximize => (rice_i18n::t("Maximize").as_bytes(), 4),
                        WmMenuRow::WindowMinimize => (rice_i18n::t("Minimize").as_bytes(), 4),
                        WmMenuRow::WindowRestore => (rice_i18n::t("Restore").as_bytes(), 4),
                        WmMenuRow::StackClose => (rice_i18n::t("ui.close-all-windows").as_bytes(), 6),
                        WmMenuRow::StackTile => (
                            if wm_menu_level == WmMenuLevel::TaskbarContext {
                                rice_i18n::t("ui.tile-windows").as_bytes()
                            } else {
                                rice_i18n::t("ui.tile-stack").as_bytes()
                            },
                            4,
                        ),
                        WmMenuRow::StackCascade => (
                            if wm_menu_level == WmMenuLevel::TaskbarContext {
                                rice_i18n::t("ui.cascade-windows").as_bytes()
                            } else {
                                rice_i18n::t("ui.cascade-stack").as_bytes()
                            },
                            4,
                        ),
                        WmMenuRow::Empty => (rice_i18n::t("ui.no-applications").as_bytes(), 0),
                    };
                    let row_text = if hovered {
                        contrasting_text(hover)
                    } else if parent_selected || dragged {
                        contrasting_text(selected)
                    } else {
                        text
                    };
                    if matches!(entry, WmMenuRow::Divider) {
                        fill_rect(
                            pixels,
                            width,
                            height,
                            column_x + 10 + rail_width,
                            y + row_height / 2,
                            column_width.saturating_sub(20 + rail_width),
                            1,
                            separator,
                        );
                    } else if matches!(entry, WmMenuRow::WorkspaceNavigation) {
                        let half = column_width / 2;
                        let previous_hovered =
                            matches!(hover_entry, Some(WmMenuRow::PreviousWorkspace));
                        let next_hovered = matches!(hover_entry, Some(WmMenuRow::NextWorkspace));
                        for (offset, active) in [(0usize, previous_hovered), (half, next_hovered)] {
                            if active {
                                fill_rounded_rect(
                                    pixels,
                                    width,
                                    height,
                                    column_x + offset + selection_inset,
                                    y + selection_inset,
                                    half.saturating_sub(selection_inset.saturating_mul(2)),
                                    row_height.saturating_sub(selection_inset.saturating_mul(2)),
                                    selection_radius,
                                    hover,
                                );
                            }
                        }
                        fill_rect(
                            pixels,
                            width,
                            height,
                            column_x + half,
                            y + 7,
                            1,
                            row_height.saturating_sub(14),
                            separator,
                        );
                    } else if matches!(entry, WmMenuRow::ScrollDown | WmMenuRow::ScrollUp) {
                        let enabled = match entry {
                            WmMenuRow::ScrollUp => wm_menu_offset > 0,
                            WmMenuRow::ScrollDown => wm_menu_offset < wm_menu_max_offset,
                            _ => false,
                        };
                        draw_menu_triangle(
                            pixels,
                            width,
                            height,
                            column_x + column_width / 2,
                            y + row_height / 2,
                            matches!(entry, WmMenuRow::ScrollUp),
                            false,
                            if enabled { row_text } else { separator },
                        );
                    } else {
                        if !browser_icon {
                            draw_app_icon_sized(
                                pixels,
                                width,
                                height,
                                column_x + 9 + rail_width,
                                y + row_height.saturating_sub(icon_size) / 2,
                                icon_size,
                                icon,
                                row_text,
                            );
                        }
                        if !redmond98 && matches!(entry, WmMenuRow::Application(_)) {
                            for dot_y in [y + 13, y + 19] {
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    column_x + column_width.saturating_sub(15),
                                    dot_y,
                                    2,
                                    2,
                                    row_text,
                                );
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    column_x + column_width.saturating_sub(10),
                                    dot_y,
                                    2,
                                    2,
                                    row_text,
                                );
                            }
                        }
                    }
                    if compact_dock {
                        if let WmMenuRow::Task(target) = entry {
                            let source = match target {WmTaskTarget::Native(id) => format!("native:{id}"), WmTaskTarget::X11(id) => format!("x11:{}",id.0), _ => String::new(),
                            };
                            text_overlays.push(WmTextOverlay::new(id, column_x+10, y+7, column_width.saturating_sub(20), 100, 13, 6, 0, row_text, column_background, format!("__dock_preview:{source}").as_bytes(), 80,
                            ));
                        }
                    }
                    if !command_deck && wm_menu_row_has_submenu(entry) {
                        draw_menu_triangle(
                            pixels,
                            width,
                            height,
                            column_x + column_width.saturating_sub(15),
                            y + row_height / 2,
                            false,
                            true,
                            row_text,
                        );
                    }
                    let mut row_background = if hovered {
                        hover
                    } else if parent_selected || dragged {
                        selected
                    } else {
                        column_background
                    };
                    // The retained menu surface already contains Command Deck's
                    // rounded selection pill. Mark selected-row overlays so the
                    // browser erases its fallback icon inside that same pill
                    // instead of repainting rectangular text/icon backplates.
                    if command_deck && (hovered || parent_selected || dragged) {
                        row_background |= 0xfe00_0000;
                    }
                    if matches!(entry, WmMenuRow::WorkspaceNavigation) {
                        let half = column_width / 2;
                        for (offset, label, active) in [
                            (
                                0usize,
                                rice_i18n::t("Previous").as_bytes(),
                                matches!(hover_entry, Some(WmMenuRow::PreviousWorkspace)),
                            ),
                            (
                                half,
                                rice_i18n::t("Next").as_bytes(),
                                matches!(hover_entry, Some(WmMenuRow::NextWorkspace)),
                            ),
                        ] {
                            let item_text = if active {
                                contrasting_text(hover)
                            } else {
                                text
                            };
                            let item_background = if active { hover } else { column_background };
                            text_overlays.push(
                                WmTextOverlay::new(
                                    id,
                                    column_x + offset + icon_size + 18,
                                    y + 3,
                                    half.saturating_sub(icon_size + 22),
                                    row_height.saturating_sub(7),
                                    12,
                                    6,
                                    0,
                                    item_text,
                                    item_background,
                                    label,
                                    12,
                                )
                                .with_menu_icon(wm_menu_icon_size, 0),
                            );
                        }
                    } else if !matches!(
                        entry,
                        WmMenuRow::ScrollDown | WmMenuRow::ScrollUp | WmMenuRow::Divider
                    ) {
                        let has_submenu = wm_menu_row_has_submenu(entry);
                        let shortcut: Option<&[u8]> = match entry {
                            WmMenuRow::WindowClose => Some(b"Alt+F4"),
                            WmMenuRow::WindowMinimize => Some(b"Alt+F9"),
                            WmMenuRow::WindowMaximize | WmMenuRow::WindowRestore => {
                                Some(b"Alt+F10")
                            }
                            _ => None,
                        };
                        let overlay = WmTextOverlay::new(
                            id,
                            column_x + icon_size + 18 + rail_width,
                            y + if compact_dock && matches!(entry, WmMenuRow::Task(_)) {112} else {3},
                            column_width.saturating_sub(
                                rail_width
                                    + if has_submenu {
                                        icon_size + 46
                                    } else if matches!(entry, WmMenuRow::Application(_)) {
                                        icon_size + 38
                                    } else if shortcut.is_some() {
                                        icon_size + 92
                                    } else {
                                        icon_size + 22
                                    },
                            ),
                            if compact_dock && matches!(entry, WmMenuRow::Task(_)) {25} else {row_height.saturating_sub(7)},
                            13,
                            6,
                            0,
                            row_text,
                            row_background,
                            label,
                            28,
                        );
                        let overlay = if let Some(icon_kind) = browser_icon_kind {
                            overlay.with_menu_icon(wm_menu_icon_size, icon_kind)
                        } else {
                            overlay
                        };
                        text_overlays.push(overlay);
                        if let Some(shortcut) = shortcut {
                            text_overlays.push(WmTextOverlay::new(
                                id,
                                column_x + column_width.saturating_sub(74),
                                y + 3,
                                66,
                                row_height.saturating_sub(7),
                                12,
                                4,
                                2,
                                row_text,
                                row_background,
                                shortcut,
                                8,
                            ));
                        }
                    }
                }
                if !liquid_glass {
                    make_region_opaque(
                        pixels,
                        width,
                        height,
                        column_x,
                        column_y,
                        column_width,
                        column_height,
                    );
                }
            }
        } else if id == WM_TASK_PREVIEW {
            pixels.fill(0);
            if let Some((title, icon, active, workspace)) = preview {
                let [background, alternate, separator, hover, selected, text] =
                    wm_theme_palette.menu;
                let accent = wm_theme_palette.taskbar[6];
                let caret_color = if active { accent } else { separator };
                let (card_x, card_y, card_width, card_height) = match preview_placement {
                    WmTaskPreviewPlacement::Taskbar => {
                        let caret_center = width / 2;
                        if self.wm_taskbar_edge == WmTaskbarEdge::Top {
                            for row in 0..8usize {
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    caret_center.saturating_sub(row),
                                    row,
                                    row.saturating_mul(2).saturating_add(1),
                                    1,
                                    caret_color,
                                );
                            }
                        } else {
                            for row in 0..8usize {
                                fill_rect(
                                    pixels,
                                    width,
                                    height,
                                    caret_center.saturating_sub(7usize.saturating_sub(row)),
                                    height.saturating_sub(8).saturating_add(row),
                                    15usize.saturating_sub(row.saturating_mul(2)),
                                    1,
                                    caret_color,
                                );
                            }
                        }
                        (
                            0,
                            if self.wm_taskbar_edge == WmTaskbarEdge::Top {
                                7
                            } else {
                                0
                            },
                            width,
                            height.saturating_sub(8),
                        )
                    }
                    WmTaskPreviewPlacement::MenuRight => {
                        let caret_center = height / 2;
                        for column in 0..8usize {
                            fill_rect(
                                pixels,
                                width,
                                height,
                                column,
                                caret_center.saturating_sub(column),
                                1,
                                column.saturating_mul(2).saturating_add(1),
                                caret_color,
                            );
                        }
                        (7, 0, width.saturating_sub(7), height)
                    }
                    WmTaskPreviewPlacement::MenuLeft => {
                        let caret_center = height / 2;
                        for column in 0..8usize {
                            fill_rect(
                                pixels,
                                width,
                                height,
                                width.saturating_sub(column.saturating_add(1)),
                                caret_center.saturating_sub(column),
                                1,
                                column.saturating_mul(2).saturating_add(1),
                                caret_color,
                            );
                        }
                        (0, 0, width.saturating_sub(7), height)
                    }
                };
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x,
                    card_y,
                    card_width,
                    card_height,
                    separator,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 2,
                    card_y + 2,
                    card_width.saturating_sub(4),
                    card_height.saturating_sub(4),
                    background,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 2,
                    card_y + 2,
                    card_width.saturating_sub(4),
                    27,
                    if active { accent } else { alternate },
                );
                let title_background = if active { accent } else { alternate };
                let title_text = contrasting_text(title_background);
                if let Some(icon) = icon {
                    draw_argb_icon(pixels, width, height, card_x + 8, card_y + 7, &icon);
                } else {
                    draw_app_icon(pixels, width, height, card_x + 8, card_y + 7, 0, title_text);
                }
                text_overlays.push(WmTextOverlay::new(
                    id,
                    card_x + 31,
                    card_y + 3,
                    card_width.saturating_sub(38),
                    25,
                    13,
                    7,
                    0,
                    title_text,
                    title_background,
                    &title,
                    32,
                ));
                // A compact, deliberately abstract window thumbnail works for
                // X11, native and browser-owned apps without special casing a
                // compositor or exposing stale pixels from another workspace.
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 13,
                    card_y + 41,
                    card_width.saturating_sub(26),
                    68,
                    alternate,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 22,
                    card_y + 51,
                    card_width / 3,
                    8,
                    hover,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 22,
                    card_y + 66,
                    card_width.saturating_sub(60),
                    5,
                    selected,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + 22,
                    card_y + 78,
                    card_width.saturating_sub(83),
                    5,
                    separator,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    card_x + card_width.saturating_sub(73),
                    card_y + 88,
                    48,
                    13,
                    accent,
                );
                let subtitle = format!("Workspace {} · click to switch", workspace + 1);
                text_overlays.push(WmTextOverlay::new(
                    id,
                    card_x + 13,
                    card_y + 111,
                    card_width.saturating_sub(26),
                    17,
                    11,
                    5,
                    1,
                    text,
                    background,
                    subtitle.as_bytes(),
                    38,
                ));
            }
        } else if id == WM_SNAP_PREVIEW {
            let accent = wm_theme_palette.frame[0] & 0x00ff_ffff;
            pixels.fill(0x3800_0000 | accent);
            let border = 0xd000_0000 | accent;
            fill_rect(pixels, width, height, 0, 0, width, 2, border);
            fill_rect(
                pixels,
                width,
                height,
                0,
                height.saturating_sub(2),
                width,
                2,
                border,
            );
            fill_rect(pixels, width, height, 0, 0, 2, height, border);
            fill_rect(
                pixels,
                width,
                height,
                width.saturating_sub(2),
                0,
                2,
                height,
                border,
            );
        } else if id == WM_NOTIFICATION {
            let (kind, title, message) = notification.unwrap_or_default();
            let accent = match kind {
                1 => 0x0022_c55e,
                2 => 0x00f5_9e0b,
                3 => 0x00ef_4444,
                _ => 0x0038_bdf8,
            };
            let background = if liquid_glass {
                wm_theme_palette.menu[0]
            } else {
                0x0014_1f2e
            };
            let title_text = if liquid_glass {
                wm_theme_palette.menu[5]
            } else {
                0x00f8_fafc
            };
            let message_text = if liquid_glass {
                contrasting_text(background)
            } else {
                0x00bf_cada
            };
            if liquid_glass {
                pixels.fill(0);
                fill_rounded_rect(
                    pixels,
                    width,
                    height,
                    0,
                    0,
                    width,
                    height,
                    height.min(56) / 2,
                    background,
                );
            } else {
                fill_rect(pixels, width, height, 0, 0, width, height, background);
                fill_rect(pixels, width, height, 0, 0, 5, height, accent);
                fill_rect(pixels, width, height, 0, 0, width, 1, 0x0047_5f7d);
                fill_rect(
                    pixels,
                    width,
                    height,
                    0,
                    height.saturating_sub(1),
                    width,
                    1,
                    0x000b_111a,
                );
            }
            fill_rect(
                pixels,
                width,
                height,
                14,
                18,
                25,
                25,
                if liquid_glass {
                    wm_theme_palette.menu[1]
                } else {
                    0x0020_3146
                },
            );
            draw_app_icon(
                pixels,
                width,
                height,
                17,
                21,
                if kind == 1 { 1 } else { 7 },
                accent,
            );
            draw_bitmap_text(
                pixels,
                width,
                height,
                50,
                13,
                &title,
                title_text,
                2,
                width.saturating_sub(58) / 12,
            );
            text_overlays.push(WmTextOverlay::new(
                id,
                50,
                7,
                width.saturating_sub(58),
                28,
                14,
                7,
                0,
                title_text,
                background,
                &title,
                width.saturating_sub(58) / 12,
            ));
            draw_bitmap_text(
                pixels,
                width,
                height,
                50,
                49,
                &message,
                message_text,
                1,
                width.saturating_sub(58) / 6,
            );
            text_overlays.push(WmTextOverlay::new(
                id,
                50,
                39,
                width.saturating_sub(58),
                32,
                12,
                4,
                0,
                message_text,
                background,
                &message,
                width.saturating_sub(58) / 6,
            ));
        } else if self.frame_clients.contains_key(&id.0) {
            let [
                active_title,
                inactive_title,
                border,
                body,
                title_text,
                control,
                close,
                pressed_control,
            ] = wm_theme_palette.frame;
            let chrome_profile = wm_theme_palette.chrome_profile;
            let controls_width = if chrome_profile == WmChromeProfile::Redmond98 {
                58
            } else {
                usize::from(WM_TITLE_BUTTON_WIDTH.saturating_mul(WM_TITLE_BUTTON_COUNT))
            };
            let yellow_tab_width = width.min(320).max(width.min(150));
            for y in 0..height {
                for x in 0..width {
                    let on_border = x < usize::from(WM_BORDER_WIDTH)
                        || y < usize::from(WM_BORDER_WIDTH)
                        || x >= width.saturating_sub(usize::from(WM_BORDER_WIDTH))
                        || y >= height.saturating_sub(usize::from(WM_BORDER_WIDTH));
                    let title_control = (chrome_profile != WmChromeProfile::Orchard
                        && y < usize::from(WM_TITLE_HEIGHT))
                    .then(|| wm_frame_control_at(chrome_profile, width, x))
                    .flatten();
                    pixels[y * width + x] = if let Some(control_index) = title_control {
                        let target = WmChromeTarget::FrameControl(id, control_index);
                        if wm_chrome_pressed == Some(target) {
                            pressed_control
                        } else if wm_chrome_hover == Some(target) {
                            active_title
                        } else if control_index == 2 {
                            close
                        } else {
                            control
                        }
                    } else if y < usize::from(WM_TITLE_HEIGHT) {
                        if chrome_profile == WmChromeProfile::YellowJacket && x >= yellow_tab_width
                        {
                            body
                        } else if chrome_profile == WmChromeProfile::CommandDeck {
                            body
                        } else if frame_is_active {
                            active_title
                        } else {
                            inactive_title
                        }
                    } else if on_border {
                        border
                    } else {
                        body
                    };
                }
            }
            if chrome_profile == WmChromeProfile::YellowJacket {
                fill_rect(
                    pixels,
                    width,
                    height,
                    1,
                    1,
                    yellow_tab_width.saturating_sub(2),
                    1,
                    0x00ff_f4a8,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    1,
                    usize::from(WM_TITLE_HEIGHT).saturating_sub(2),
                    yellow_tab_width.saturating_sub(2),
                    1,
                    border,
                );
            } else if chrome_profile == WmChromeProfile::CommandDeck {
                fill_rounded_rect(
                    pixels,
                    width,
                    height,
                    2,
                    2,
                    width.saturating_sub(4).min(136),
                    usize::from(WM_TITLE_HEIGHT).saturating_sub(4),
                    10,
                    active_title,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    130.min(width),
                    5,
                    width.saturating_sub(130).saturating_sub(controls_width + 8),
                    5,
                    border,
                );
            } else if chrome_profile == WmChromeProfile::Redmond98 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    0,
                    0,
                    width,
                    usize::from(WM_TITLE_HEIGHT),
                    body,
                );
                let title_width = width.saturating_sub(6);
                for offset in 0..title_width {
                    let title_color = if frame_is_active {
                        mix_rgb(
                            active_title,
                            0x0010_84d0,
                            u32::try_from(offset).unwrap_or(u32::MAX),
                            u32::try_from(title_width.max(1)).unwrap_or(u32::MAX),
                        )
                    } else {
                        mix_rgb(
                            inactive_title,
                            0x00b5_b5b5,
                            u32::try_from(offset).unwrap_or(u32::MAX),
                            u32::try_from(title_width.max(1)).unwrap_or(u32::MAX),
                        )
                    };
                    fill_rect(
                        pixels,
                        width,
                        height,
                        3 + offset,
                        3,
                        1,
                        usize::from(WM_TITLE_HEIGHT).saturating_sub(5),
                        title_color,
                    );
                }
                fill_redmond_bevel(pixels, width, height, 0, 0, width, height, false);
            } else if chrome_profile == WmChromeProfile::NeonGrid {
                fill_rect(pixels, width, height, 0, 0, width, 2, border);
                fill_rect(
                    pixels,
                    width,
                    height,
                    0,
                    usize::from(WM_TITLE_HEIGHT).saturating_sub(2),
                    width,
                    2,
                    active_title,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    8,
                    5,
                    width.saturating_sub(150),
                    2,
                    active_title,
                );
                for x in (16..width.saturating_sub(controls_width + 16)).step_by(26) {
                    fill_rect(pixels, width, height, x, 9, 8, 1, border);
                }
            }
            let title_icon_x = wm_frame_icon_left(chrome_profile).saturating_add(2);
            let title_x = match chrome_profile {
                WmChromeProfile::Orchard => 101,
                WmChromeProfile::SplitControls => 53,
                _ => 27,
            };
            if let Some(icon) = frame_icon {
                draw_argb_icon(pixels, width, height, title_icon_x, 4, &icon);
            } else {
                draw_app_icon(
                    pixels,
                    width,
                    height,
                    title_icon_x.saturating_sub(1),
                    3,
                    0,
                    title_text,
                );
            }
            draw_bitmap_text(
                pixels,
                width,
                height,
                title_x,
                7,
                &frame_title,
                title_text,
                1,
                width.saturating_sub(controls_width + title_x + 5) / 6,
            );
            text_overlays.push(WmTextOverlay::new(
                id,
                title_x,
                2,
                width.saturating_sub(controls_width + title_x + 5),
                usize::from(WM_TITLE_HEIGHT).saturating_sub(4),
                13,
                6,
                0,
                title_text,
                if frame_is_active {
                    active_title
                } else {
                    inactive_title
                },
                &frame_title,
                width.saturating_sub(controls_width + title_x + 5) / 6,
            ));

            let minimize_left = wm_frame_control_left(chrome_profile, width, 0);
            let maximize_left = wm_frame_control_left(chrome_profile, width, 1);
            let close_left = wm_frame_control_left(chrome_profile, width, 2);
            let control_background = |index: u8, idle: u32| {
                let target = WmChromeTarget::FrameControl(id, index);
                if wm_chrome_pressed == Some(target) {
                    pressed_control
                } else if wm_chrome_hover == Some(target) {
                    active_title
                } else {
                    idle
                }
            };
            let minimize_color = contrasting_text(control_background(0, control));
            let maximize_color = contrasting_text(control_background(1, control));
            let close_color = contrasting_text(control_background(2, close));
            if chrome_profile == WmChromeProfile::Redmond98 {
                for (index, left) in [minimize_left, maximize_left, close_left]
                    .into_iter()
                    .enumerate()
                {
                    let target =
                        WmChromeTarget::FrameControl(id, u8::try_from(index).unwrap_or_default());
                    fill_rect(pixels, width, height, left + 3, 6, 16, 14, 0x00c0_c0c0);
                    fill_redmond_bevel(
                        pixels,
                        width,
                        height,
                        left + 3,
                        6,
                        16,
                        14,
                        wm_chrome_pressed == Some(target),
                    );
                }
            }
            if chrome_profile == WmChromeProfile::SplitControls {
                let light = mix_rgb(control, 0x00ff_ffff, 3, 5);
                let shadow = mix_rgb(control, 0, 1, 2);
                for left in [minimize_left, maximize_left, close_left] {
                    fill_rect(pixels, width, height, left + 1, 1, 20, 1, light);
                    fill_rect(pixels, width, height, left + 1, 1, 1, 22, light);
                    fill_rect(pixels, width, height, left + 1, 22, 20, 1, shadow);
                    fill_rect(pixels, width, height, left + 20, 1, 1, 22, shadow);
                }
            }
            if chrome_profile == WmChromeProfile::Orchard {
                for (left, color) in [
                    (close_left, 0x00ff_5f57),
                    (minimize_left, 0x00ff_bd2e),
                    (maximize_left, 0x0028_c840),
                ] {
                    fill_rounded_rect(pixels, width, height, left + 5, 6, 12, 12, 6, color);
                }
            }
            if chrome_profile == WmChromeProfile::Redmond98 {
                // Draw the glyph inside the 16x14 classic button instead of
                // reusing the modern 22px control geometry below.  The modern
                // coordinates overwrite the inner top bevel and visibly push
                // the symbols out of alignment.
                fill_rect(
                    pixels,
                    width,
                    height,
                    minimize_left + 7,
                    16,
                    6,
                    2,
                    0x0000_0000,
                );
            } else if chrome_profile != WmChromeProfile::Orchard
                || wm_chrome_hover == Some(WmChromeTarget::FrameControl(id, 0))
            {
                fill_rect(
                    pixels,
                    width,
                    height,
                    minimize_left + 6,
                    15,
                    10,
                    2,
                    minimize_color,
                );
            }
            if chrome_profile == WmChromeProfile::Redmond98 {
                fill_rect(
                    pixels,
                    width,
                    height,
                    maximize_left + 6,
                    9,
                    8,
                    2,
                    0x0000_0000,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    maximize_left + 6,
                    11,
                    1,
                    6,
                    0x0000_0000,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    maximize_left + 13,
                    11,
                    1,
                    6,
                    0x0000_0000,
                );
                fill_rect(
                    pixels,
                    width,
                    height,
                    maximize_left + 6,
                    16,
                    8,
                    1,
                    0x0000_0000,
                );
            } else if chrome_profile != WmChromeProfile::Orchard
                || wm_chrome_hover == Some(WmChromeTarget::FrameControl(id, 1))
            {
                for offset in 0..10 {
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        maximize_left + 6 + offset,
                        7,
                        maximize_color,
                    );
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        maximize_left + 6 + offset,
                        16,
                        maximize_color,
                    );
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        maximize_left + 6,
                        7 + offset,
                        maximize_color,
                    );
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        maximize_left + 15,
                        7 + offset,
                        maximize_color,
                    );
                }
            }
            let close_x = close_left + 6;
            if chrome_profile == WmChromeProfile::Redmond98 {
                for offset in 0..7 {
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        close_x + offset,
                        10 + offset,
                        0x0000_0000,
                    );
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        close_x + 6 - offset,
                        10 + offset,
                        0x0000_0000,
                    );
                }
            } else if chrome_profile != WmChromeProfile::Orchard
                || wm_chrome_hover == Some(WmChromeTarget::FrameControl(id, 2))
            {
                for offset in 0..9 {
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        close_x + offset,
                        7 + offset,
                        close_color,
                    );
                    set_surface_pixel(
                        pixels,
                        width,
                        height,
                        close_x + 8 - offset,
                        7 + offset,
                        close_color,
                    );
                }
            }
        }
        if id == WM_TASKBAR && self.wm_dock_style {
            for pixel in pixels.iter_mut().filter(|pixel| **pixel != 0) {
                *pixel = (*pixel & 0x00ff_ffff) | if *pixel == wm_theme_palette.taskbar[0] { 0xb800_0000 } else { 0xff00_0000 };
            }
        } else if liquid_glass {
            for pixel in pixels.iter_mut() {
                apply_liquid_glass_pixel(pixel, &wm_theme_palette);
            }
            for overlay in &mut text_overlays {
                overlay.foreground = 0xff00_0000 | (overlay.foreground & 0x00ff_ffff);
                overlay.background = liquid_glass_color(overlay.background, &wm_theme_palette);
            }
        } else if matches!(
            id,
            WM_TASKBAR
                | WM_GNOME_PANEL
                | WM_GNOME_MENU
                | WM_GNOME_DIALOG
                | WM_MENU
                | WM_TASK_PREVIEW
                | WM_TOOLTIP
        ) || frame_client.is_some()
        {
            // Depth-32 chrome is also used by opaque themes so the selected
            // material can change live without reallocating every surface.
            // Preserve truly clear menu corners/unused cascade space while
            // making every painted opaque-theme pixel visible.
            for pixel in pixels.iter_mut().filter(|pixel| **pixel != 0) {
                *pixel |= 0xff00_0000;
            }
        }
        for (icon, x, y) in notification_icons {
            if notification_focus == Some(icon.slot) || wm_chrome_hover == Some(WmChromeTarget::NotificationIcon(icon.slot)) {
                fill_rect(pixels, width, height, x, y, 24, 24, wm_theme_palette.taskbar[2] | 0xff000000);
            }
            for dy in 0..20usize { for dx in 0..20usize {
                let src = icon.pixels[(dy * usize::from(icon.height) / 20) * usize::from(icon.width) + dx * usize::from(icon.width) / 20];
                if x+2+dx < width && y+2+dy < height {
                    let dst = &mut pixels[(y+2+dy)*width+x+2+dx];
                    let a = src >> 24; if a == 0 { continue; } let inv = 255-a;
                    let channel = |shift: u32| (((src >> shift)&255u32)*a + ((*dst >> shift)&255u32)*inv +127)/255;
                    *dst = 0xff000000 | (channel(16)<<16) | (channel(8)<<8) | channel(0);
                }
            }}
        }
        if frame_client.is_some() {
            // Alignment value 3 is a semantic window-title record, not paint.
            // Zero geometry also keeps older renderers from drawing it.
            text_overlays.push(WmTextOverlay::new(id,0,0,0,0,0,0,3,0,0,&frame_title,255,
            ));
        }
        self.wm_text_overlays.retain(|overlay| overlay.window != id);
        self.wm_text_overlays.extend(text_overlays);
        self.scene_damage(id);
        self.paint_output_shell_replicas(id);
    }
}
