//! Shell menu operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn wm_notification_geometry(&self) -> (i16, i16, u16, u16) {
        let (work_x, work_y, work_width, work_height) = self.wm_workarea_rect();
        let horizontal_margin = WM_NOTIFICATION_MARGIN.saturating_mul(2);
        let width = WM_NOTIFICATION_WIDTH.min(work_width.saturating_sub(horizontal_margin).max(1));
        let available_height = work_height.saturating_sub(2).max(1);
        let height = WM_NOTIFICATION_HEIGHT.min(available_height);
        let x = work_x
            .saturating_add(work_width)
            .saturating_sub(width.saturating_add(WM_NOTIFICATION_MARGIN));
        let y = work_y
            .saturating_add(WM_NOTIFICATION_MARGIN)
            .min(work_y.saturating_add(work_height).saturating_sub(height));
        (
            i16::try_from(x).unwrap_or(i16::MAX),
            i16::try_from(y).unwrap_or(i16::MAX),
            width,
            height,
        )
    }

    pub(super) fn insert_internal_window(&mut self, window: Window) {
        if window.class == 1 {
            self.surfaces.insert(
                window.id.0,
                Surface::new(
                    window.width,
                    window.height,
                    window.depth,
                    window.background_pixel,
                ),
            );
        }
        if let Some(parent) = self.windows.get_mut(&window.parent.0) {
            parent.children.push(window.id);
        }
        self.windows.insert(window.id.0, window);
    }

    pub(super) fn wm_root_rows(&self) -> Vec<WmMenuRow> {
        if self.wm_menu_applications_only {
            return vec![WmMenuRow::Applications];
        }
        if self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98)
        {
            let mut rows = vec![
                WmMenuRow::Applications,
                WmMenuRow::RedmondShortcut(RedmondMenuShortcut::Documents),
            ];
            if self.wm_menu_show_themes {
                rows.push(WmMenuRow::Themes);
            }
            rows.extend([WmMenuRow::Run, WmMenuRow::Divider, WmMenuRow::Exit]);
            return rows;
        }
        let mut rows = vec![WmMenuRow::Applications];
        if !self.wm_menu_structured {
            rows.extend(
                self.wm_menu_entries
                    .iter()
                    .filter(|entry| entry.category == 0)
                    .cloned()
                    .map(WmMenuRow::Application),
            );
        }
        if self.wm_menu_show_themes {
            rows.push(WmMenuRow::Themes);
        }
        if self.wm_menu_show_workspaces && !self.wm_compact_shell() {
            rows.push(WmMenuRow::WorkspaceNavigation);
        }
        rows.extend([WmMenuRow::Run, WmMenuRow::Exit]);
        rows
    }

    pub(super) fn wm_menu_initial_level(&self) -> WmMenuLevel {
        if !self.wm_menu_applications_only {
            WmMenuLevel::Root
        } else if self.wm_menu_structured {
            WmMenuLevel::MenuFolders
        } else {
            WmMenuLevel::Categories
        }
    }

    pub(super) fn wm_menu_hides_applications_root(&self) -> bool {
        self.wm_menu_applications_only
            && matches!(
                self.wm_menu_level,
                WmMenuLevel::Categories
                    | WmMenuLevel::Applications(_)
                    | WmMenuLevel::MenuFolders
                    | WmMenuLevel::MenuFolder(_)
            )
    }

    pub(super) fn wm_menu_base_row_height(&self) -> usize {
        if self.wm_dock_style && matches!(self.wm_menu_level, WmMenuLevel::TaskGroup(_)) { return 144; }
        let redmond98 = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98);
        if self.wm_theme_menu_row_height>0 {self.wm_theme_menu_row_height} else {wm_menu_base_row_height_with(redmond98, usize::from(self.wm_menu_icon_size))}
    }

    pub(super) fn wm_menu_available_height(&self) -> usize {
        let (_, primary_y, _, primary_height) = self.wm_dock_output_rect();
        if self.wm_taskbar_edge.vertical() {
            usize::from(self.wm_taskbar_rect().3.saturating_sub(8))
        } else if self.wm_dock_style {
            // Floating docks reserve no workarea, but menus still need to fit
            // on the inward side of their surface, just like task previews.
            let (_, dock_y, _, dock_height) = self.wm_taskbar_rect();
            let available = if self.wm_taskbar_edge == WmTaskbarEdge::Top {
                primary_height.saturating_sub(
                    dock_y.cast_unsigned().saturating_sub(primary_y).saturating_add(dock_height))
            } else {
                dock_y.cast_unsigned().saturating_sub(primary_y)
            };
            usize::from(available.saturating_sub(8))
        } else {
            usize::from(
                primary_height
                    .saturating_sub(self.wm_reserved_taskbar_height().saturating_add(8)),
            )
        }
    }

    pub(super) fn wm_menu_is_start_cascade(&self) -> bool {
        matches!(
            self.wm_menu_level,
            WmMenuLevel::Root
                | WmMenuLevel::Categories
                | WmMenuLevel::Applications(_)
                | WmMenuLevel::MenuFolders
                | WmMenuLevel::MenuFolder(_)
                | WmMenuLevel::Themes
        )
    }

    pub(super) fn wm_menu_opens_left(&self) -> bool {
        self.wm_taskbar_edge == WmTaskbarEdge::Right
            && self.wm_menu_window_anchor.is_none()
            && self.wm_menu_is_start_cascade()
    }

    pub(super) fn wm_category_rows(&self) -> Vec<WmMenuRow> {
        let mut categories = self
            .wm_menu_entries
            .iter()
            .map(|entry| entry.category)
            .filter(|category| *category != 0)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(WmMenuRow::Category)
            .collect::<Vec<_>>();
        if categories.is_empty() {
            categories.push(WmMenuRow::Empty);
        }
        categories
    }

    pub(super) fn wm_menu_folder_rows(&self, parent: Option<u8>) -> Vec<WmMenuRow> {
        let mut rows = self
            .wm_menu_entries
            .iter()
            .filter(|entry| entry.folder == parent)
            .cloned()
            .map(WmMenuRow::Application)
            .collect::<Vec<_>>();
        rows.extend(
            self.wm_menu_folders
                .iter()
                .enumerate()
                .filter(|(_, folder)| folder.parent == parent)
                .filter_map(|(index, _)| u8::try_from(index).ok().map(WmMenuRow::MenuFolder)),
        );
        if rows.is_empty() {
            rows.push(WmMenuRow::Empty);
        }
        rows
    }

    pub(super) fn wm_menu_folder_chain(&self, mut folder: u8) -> Vec<u8> {
        let mut chain = Vec::new();
        loop {
            chain.push(folder);
            let Some(parent) = self
                .wm_menu_folders
                .get(usize::from(folder))
                .and_then(|entry| entry.parent)
            else {
                break;
            };
            folder = parent;
        }
        chain.reverse();
        chain
    }

    pub(super) fn wm_theme_rows(&self) -> Vec<WmMenuRow> {
        let visible_rows = self.wm_menu_visible_rows();
        let simple_capacity = visible_rows.saturating_sub(2).max(1);
        let scrolling = self.wm_themes.len() > simple_capacity;
        let capacity = if scrolling {
            visible_rows.saturating_sub(4).max(1)
        } else {
            simple_capacity
        };
        let start = self
            .wm_menu_offset
            .min(self.wm_themes.len().saturating_sub(capacity));
        let end = start.saturating_add(capacity).min(self.wm_themes.len());
        let mut rows = Vec::new();
        if scrolling {
            rows.push(WmMenuRow::ScrollUp);
        }
        rows.extend(
            (start..end).map(|index| WmMenuRow::Theme(u8::try_from(index).unwrap_or(u8::MAX))),
        );
        if scrolling {
            rows.push(WmMenuRow::ScrollDown);
        }
        rows.push(WmMenuRow::Divider);
        rows.push(WmMenuRow::ThemeCustomize);
        rows
    }

    pub(super) fn wm_task_window_rows(&self, target: WmTaskTarget) -> Vec<WmMenuRow> {
        let (minimized, maximized) = match target {
            WmTaskTarget::X11(client) => (
                self.minimized_windows.contains(&client.0),
                self.maximized_geometries.contains_key(&client.0),
            ),
            WmTaskTarget::Native(id) => self
                .native_tasks
                .iter()
                .find(|task| task.id == id)
                .map_or((false, false), |task| (task.minimized, task.maximized)),
            WmTaskTarget::Group(_) => (false, false),
        };
        let mut rows = Vec::with_capacity(5);
        if minimized || maximized {
            rows.push(WmMenuRow::WindowRestore);
        }
        if !minimized {
            rows.push(WmMenuRow::WindowMinimize);
        }
        if !maximized {
            rows.push(WmMenuRow::WindowMaximize);
        }
        rows.push(WmMenuRow::Divider);
        rows.push(WmMenuRow::WindowClose);
        rows
    }

    pub(super) fn wm_menu_columns(&self) -> Vec<Vec<WmMenuRow>> {
        let mut columns = match self.wm_menu_level {
            WmMenuLevel::Root => vec![self.wm_root_rows()],
            WmMenuLevel::Categories => {
                vec![self.wm_root_rows(), self.wm_category_rows()]
            }
            WmMenuLevel::Applications(category) => vec![
                self.wm_root_rows(),
                self.wm_category_rows(),
                self.wm_application_rows(category),
            ],
            WmMenuLevel::MenuFolders => {
                vec![self.wm_root_rows(), self.wm_menu_folder_rows(None)]
            }
            WmMenuLevel::MenuFolder(folder) => {
                let mut columns = vec![self.wm_root_rows(), self.wm_menu_folder_rows(None)];
                columns.extend(
                    self.wm_menu_folder_chain(folder)
                        .into_iter()
                        .map(|folder| self.wm_menu_folder_rows(Some(folder))),
                );
                columns
            }
            WmMenuLevel::ApplicationContext(_) => {
                let mut columns = self.wm_application_context_columns.clone();
                let mut actions = vec![
                    WmMenuRow::ApplicationLaunch,
                    WmMenuRow::ApplicationOpenFolder,
                ];
                if let WmMenuLevel::ApplicationContext(action) = self.wm_menu_level
                    && self
                        .wm_menu_entries
                        .iter()
                        .any(|entry| entry.action == action && entry.uninstallable)
                {
                    actions.push(WmMenuRow::ApplicationUninstall);
                }
                columns.push(actions);
                columns
            }
            WmMenuLevel::MenuDirectoryContext => {
                let mut columns = self.wm_application_context_columns.clone();
                columns.push(vec![WmMenuRow::MenuOpenDirectory]);
                columns
            }
            WmMenuLevel::Themes => {
                vec![self.wm_root_rows(), self.wm_theme_rows()]
            }
            WmMenuLevel::TaskbarContext => vec![vec![
                WmMenuRow::StackTile,
                WmMenuRow::StackCascade,
                WmMenuRow::Divider,
                WmMenuRow::RestoreWindows,
                WmMenuRow::ShowDesktop,
                WmMenuRow::Divider,
                if self.wm_dock_style { WmMenuRow::DockSettings } else { WmMenuRow::ToolbarPreferences },
            ]],
            WmMenuLevel::QuickLaunchContext => {
                let mut rows = vec![WmMenuRow::QuickShortcutPreferences];
                if self.wm_quick_context_index.and_then(|index| self.wm_quick_launchers.get(usize::from(index)))
                    .is_some_and(|entry| entry.action != WM_ACTION_SHOW_DESKTOP) {
                    rows.push(WmMenuRow::QuickShortcutRemove);
                }
                vec![rows]
            }
            WmMenuLevel::TaskGroup(group) => vec![self.wm_task_group_rows(group)],
            WmMenuLevel::TaskContext(target) => vec![match target {
                WmTaskTarget::Group(_) => vec![
                    WmMenuRow::StackTile,
                    WmMenuRow::StackCascade,
                    WmMenuRow::StackClose,
                ],
                _ => self.wm_task_window_rows(target),
            }],
        };
        if self.wm_dock_style
            && matches!(self.wm_menu_level, WmMenuLevel::QuickLaunchContext | WmMenuLevel::TaskContext(_) | WmMenuLevel::MenuDirectoryContext)
        {
            if let Some(rows) = columns.last_mut() {
                rows.push(WmMenuRow::Divider);
                rows.push(WmMenuRow::DockSettings);
            }
        }
        if self.wm_menu_hides_applications_root() && !columns.is_empty() {
            columns.remove(0);
        }
        columns
    }

    pub(super) fn wm_menu_column_widths(&self) -> Vec<usize> {
        let output_width = if self.wm_compact_shell() { self.wm_dock_output_rect().2 } else { self.config.width_px };
        let available = usize::from(output_width.saturating_sub(8)).max(1);
        let redmond98 = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| theme.chrome_profile == WmChromeProfile::Redmond98);
        let category_width = if redmond98 {
            188
        } else {
            WM_MENU_CATEGORY_WIDTH
        };
        let application_width = if redmond98 {
            210
        } else {
            WM_MENU_APPLICATION_WIDTH
        };
        // Keep mobile cascades legible. Overlap their parents when neither
        // side has room, instead of reducing the newest pane to one pixel.
        if self.wm_compact_shell() && self.wm_menu_columns().len() > 1 {
            let count = self.wm_menu_columns().len();
            // Reserve a small reveal even on a phone where a full-width pane
            // would leave no room to distinguish the next cascade. This width
            // is independent of depth, so opening a child never resizes its parent.
            let reveal = 32.min(available / 8);
            return (0..count).map(|index| {
                (if index == 0 && !self.wm_menu_hides_applications_root() {
                    WM_MENU_ROOT_WIDTH.min(available)
                } else { application_width.min(available.saturating_sub(reveal)) }).max(1)
            }).collect();
        }
        let mut widths = match self.wm_menu_level {
            // A cascade must never resize the pane that opened it. Keeping the
            // root width identical at every level also keeps its disclosure
            // carets stationary while the child pane is being mapped.
            WmMenuLevel::Root => vec![WM_MENU_ROOT_WIDTH.min(available)],
            WmMenuLevel::Categories | WmMenuLevel::Themes => {
                let total = (WM_MENU_ROOT_WIDTH + application_width).min(available);
                let root = WM_MENU_ROOT_WIDTH.min(total.saturating_sub(1));
                vec![root, total.saturating_sub(root)]
            }
            WmMenuLevel::Applications(_) => {
                let total =
                    (WM_MENU_ROOT_WIDTH + category_width + application_width).min(available);
                // Protect the already-visible panes first. The newest, last
                // cascade absorbs any narrow-output reduction so opening it
                // cannot move a disclosure caret under the pointer.
                let root = WM_MENU_ROOT_WIDTH.min(total.saturating_sub(2)).max(1);
                let category = category_width.min(total.saturating_sub(root + 1)).max(1);
                vec![root, category, total.saturating_sub(root + category)]
            }
            WmMenuLevel::MenuFolders | WmMenuLevel::MenuFolder(_) => {
                let folder_depth = match self.wm_menu_level {
                    WmMenuLevel::MenuFolder(folder) => self.wm_menu_folder_chain(folder).len(),
                    _ => 0,
                };
                let pane_count = 2usize.saturating_add(folder_depth);
                let desired_total = WM_MENU_ROOT_WIDTH
                    .saturating_add(category_width.saturating_mul(folder_depth))
                    .saturating_add(application_width);
                let total = desired_total.min(available).max(pane_count);
                let mut remaining = total;
                let mut widths = Vec::with_capacity(pane_count);
                for pane in 0..pane_count {
                    let panes_after = pane_count.saturating_sub(pane + 1);
                    let desired = if pane == 0 {
                        WM_MENU_ROOT_WIDTH
                    } else if pane + 1 == pane_count {
                        application_width
                    } else {
                        category_width
                    };
                    let width = desired.min(remaining.saturating_sub(panes_after)).max(1);
                    widths.push(width);
                    remaining = remaining.saturating_sub(width);
                }
                widths
            }
            WmMenuLevel::TaskGroup(_) => vec![application_width.min(available)],
            WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext => {
                let mut widths = self.wm_application_context_widths.clone();
                widths.push(240.min(available));
                widths
            }
            WmMenuLevel::TaskContext(_)
            | WmMenuLevel::TaskbarContext
            | WmMenuLevel::QuickLaunchContext => {
                vec![280.min(available)]
            }
        };
        if self.wm_menu_hides_applications_root() && !widths.is_empty() {
            widths.remove(0);
        }
        if self.wm_dock_style && !self.wm_taskbar_edge.vertical() && self.wm_menu_window_anchor.is_none() && self.wm_menu_is_start_cascade() {
            let mut left=i32::from(self.wm_taskbar_rect().0)+8;
            let mut right=left+widths.first().copied().unwrap_or(0) as i32;
            for width in widths.iter_mut().skip(1) {
                let right_space=(i32::from(self.config.width_px)-4-right).max(0) as usize;
                let left_space=(left-4).max(0) as usize;
                if *width<=right_space {right+=*width as i32;}
                else { *width=(*width).min(left_space.max(right_space)).max(1); if left_space>=right_space {left-=*width as i32;} else {right+=*width as i32;} }
            }
        }
        widths
    }

    pub(super) fn wm_menu_column_x_offsets(&self) -> Vec<usize> {
        let widths = self.wm_menu_column_widths();
        if self.wm_compact_shell() && self.wm_menu_columns().len() > 1 {
            let available = usize::from(self.wm_dock_output_rect().2.saturating_sub(8));
            let mut offsets = vec![0];
            for index in 1..widths.len() {
                let previous = offsets[index - 1];
                let right = previous + widths[index - 1];
                let left = previous.saturating_sub(widths[index]);
                offsets.push(if right + widths[index] <= available { right }
                    else if previous >= widths[index] { left }
                    else {
                        let end = available.saturating_sub(widths[index]);
                        let reveal = 32.min(available / 8);
                        // Alternate the inset when successive panes would hit
                        // the same edge. Keep the previous pane's edge visible.
                        if index > 1 && previous.abs_diff(end) < reveal {
                            previous.saturating_sub(reveal)
                        } else { end }
                    });
            }
            return offsets;
        }
        if self.wm_dock_style && !self.wm_taskbar_edge.vertical() && self.wm_menu_window_anchor.is_none() && self.wm_menu_is_start_cascade() {
            let anchor=i32::from(self.wm_taskbar_rect().0)+8;
            let mut left=anchor;
            let mut right=anchor+widths.first().copied().unwrap_or(0) as i32;
            let mut positions=vec![anchor];
            for width in widths.iter().skip(1) {
                let right_space=(i32::from(self.config.width_px)-4-right).max(0) as usize;
                let left_space=(left-4).max(0) as usize;
                if *width<=right_space {positions.push(right);right+=*width as i32;}
                else if left_space>=right_space {left-=*width as i32;positions.push(left);}
                else {positions.push(right);right+=*width as i32;}
            }
            return positions.into_iter().map(|x|(x-left).max(0) as usize).collect();
        }
        let column_count = widths.len();
        let chrome_profile = self
            .wm_themes
            .get(usize::from(self.wm_theme))
            .map_or(WmChromeProfile::Standard, |theme| theme.chrome_profile);
        let column_gap = (chrome_profile == WmChromeProfile::CommandDeck)
            .then_some(4)
            .unwrap_or_default();
        let column_overlap = usize::from(chrome_profile == WmChromeProfile::Redmond98) * 2;
        let mut offsets = Vec::with_capacity(widths.len());
        let mut next = 0usize;
        for (column, width) in widths.into_iter().enumerate() {
            let context = matches!(
                self.wm_menu_level,
                WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext
            ) && column + 1 == column_count;
            let x = if context {
                self.wm_application_context_x
            } else {
                next
            };
            offsets.push(x);
            if !context {
                next = next
                    .saturating_add(width)
                    .saturating_add(column_gap)
                    .saturating_sub(column_overlap);
            }
        }
        if self.wm_menu_opens_left() {
            let extent = offsets
                .iter()
                .zip(self.wm_menu_column_widths())
                .map(|(offset, width)| offset.saturating_add(width))
                .max()
                .unwrap_or_default();
            for (offset, width) in offsets.iter_mut().zip(self.wm_menu_column_widths()) {
                *offset = extent.saturating_sub(offset.saturating_add(width));
            }
        }
        offsets
    }
}
