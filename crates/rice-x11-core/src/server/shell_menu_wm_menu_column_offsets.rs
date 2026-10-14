//! Shell menu operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn wm_menu_column_offsets(&self) -> Vec<usize> {
        let row_height = self.wm_menu_base_row_height();
        let available_height = self.wm_menu_available_height();
        let root_rows = self.wm_root_rows();
        let root_height = wm_menu_rows_height_with(&root_rows, row_height).min(available_height);
        let root_y =
            if self.wm_taskbar_edge == WmTaskbarEdge::Top || self.wm_taskbar_edge.vertical() {
                0
            } else {
                available_height.saturating_sub(root_height)
            };
        let fit_flyout = |desired_y: usize, rows: &[WmMenuRow]| {
            desired_y
                .min(available_height.saturating_sub(wm_menu_rows_height_with(rows, row_height)))
        };
        let mut offsets = match self.wm_menu_level {
            WmMenuLevel::Root => vec![root_y],
            // Applications is the first root row, so its category fly-out is
            // already aligned with the disclosure caret.
            WmMenuLevel::Categories => {
                let categories = self.wm_category_rows();
                vec![root_y, fit_flyout(root_y, &categories)]
            }
            WmMenuLevel::Applications(category) => {
                let categories = self.wm_category_rows();
                let category_row = categories
                    .iter()
                    .position(|row| *row == WmMenuRow::Category(category))
                    .unwrap_or_default();
                let category_y = fit_flyout(root_y, &categories);
                let applications = self.wm_application_rows(category);
                let application_y = fit_flyout(
                    category_y.saturating_add(wm_menu_row_top_with(
                        &categories,
                        category_row,
                        row_height,
                    )),
                    &applications,
                );
                vec![root_y, category_y, application_y]
            }
            WmMenuLevel::MenuFolders => {
                let folders = self.wm_menu_folder_rows(None);
                vec![root_y, fit_flyout(root_y, &folders)]
            }
            WmMenuLevel::MenuFolder(folder) => {
                let chain = self.wm_menu_folder_chain(folder);
                let mut parent_rows = self.wm_menu_folder_rows(None);
                let mut parent_y = fit_flyout(root_y, &parent_rows);
                let mut offsets = vec![root_y, parent_y];
                for folder in chain {
                    let folder_row = parent_rows
                        .iter()
                        .position(|row| *row == WmMenuRow::MenuFolder(folder))
                        .unwrap_or_default();
                    let rows = self.wm_menu_folder_rows(Some(folder));
                    let y = fit_flyout(
                        parent_y.saturating_add(wm_menu_row_top_with(
                            &parent_rows,
                            folder_row,
                            row_height,
                        )),
                        &rows,
                    );
                    offsets.push(y);
                    parent_rows = rows;
                    parent_y = y;
                }
                offsets
            }
            WmMenuLevel::Themes => {
                let root_rows = self.wm_root_rows();
                let theme_row = root_rows
                    .iter()
                    .position(|row| *row == WmMenuRow::Themes)
                    .unwrap_or_default();
                let themes = self.wm_theme_rows();
                vec![
                    root_y,
                    fit_flyout(
                        root_y.saturating_add(wm_menu_row_top_with(
                            &root_rows, theme_row, row_height,
                        )),
                        &themes,
                    ),
                ]
            }
            WmMenuLevel::ApplicationContext(_) | WmMenuLevel::MenuDirectoryContext => {
                let mut offsets = self.wm_application_context_offsets.clone();
                offsets.push(self.wm_application_context_y);
                offsets
            }
            WmMenuLevel::TaskGroup(_)
            | WmMenuLevel::TaskContext(_)
            | WmMenuLevel::TaskbarContext
            | WmMenuLevel::QuickLaunchContext => vec![0],
        };
        if self.wm_menu_hides_applications_root() && !offsets.is_empty() {
            offsets.remove(0);
        }
        offsets
    }

    pub(super) fn wm_menu_hit(&self, root_x: i16, root_y: i16) -> Option<(usize, usize, WmMenuRow)> {
        let (origin_x, origin_y) = self.window_origin(WM_MENU);
        let local_x = i32::from(root_x).saturating_sub(origin_x);
        let local_y = i32::from(root_y).saturating_sub(origin_y);
        if local_x < 0 || local_y < 0 {
            return None;
        }
        let columns = self.wm_menu_columns();
        let widths = self.wm_menu_column_widths();
        let x_offsets = self.wm_menu_column_x_offsets();
        let offsets = self.wm_menu_column_offsets();
        for column in (0..widths.len()).rev() {
            let width = widths[column];
            let left = x_offsets.get(column).copied().unwrap_or_default();
            let right = left.saturating_add(width);
            let local_x = usize::try_from(local_x).ok()?;
            if (left..right).contains(&local_x) {
                let column_y = *offsets.get(column).unwrap_or(&0);
                let local_y = usize::try_from(local_y).ok()?;
                if local_y < column_y {
                    continue;
                }
                let Some((row, mut entry)) = wm_menu_row_at_with(
                    columns.get(column)?,
                    local_y.saturating_sub(column_y),
                    self.wm_menu_base_row_height(),
                ) else { continue; };
                if entry == WmMenuRow::WorkspaceNavigation {
                    let within = usize::try_from(local_x).ok()?.saturating_sub(left);
                    entry = if within < width / 2 {
                        WmMenuRow::PreviousWorkspace
                    } else {
                        WmMenuRow::NextWorkspace
                    };
                }
                return Some((column, row, entry));
            }
        }
        None
    }

    pub(super) fn wm_menu_column_at(&self, root_x: i16) -> Option<usize> {
        let (origin_x, _) = self.window_origin(WM_MENU);
        let local_x = i32::from(root_x).saturating_sub(origin_x);
        let local_x = usize::try_from(local_x).ok()?;
        let widths = self.wm_menu_column_widths();
        let x_offsets = self.wm_menu_column_x_offsets();
        for column in (0..widths.len()).rev() {
            let left = x_offsets.get(column).copied().unwrap_or_default();
            if (left..left.saturating_add(widths[column])).contains(&local_x) {
                return Some(column);
            }
        }
        None
    }

    pub(super) fn wm_application_rows(&self, category: u8) -> Vec<WmMenuRow> {
        let applications = self
            .wm_menu_entries
            .iter()
            .filter(|entry| entry.category == category)
            .cloned()
            .collect::<Vec<_>>();
        let visible_rows = self.wm_menu_visible_rows();
        let simple_capacity = visible_rows.max(1);
        let paginated = applications.len() > simple_capacity;
        let application_capacity = self.wm_application_capacity(applications.len());
        let maximum_offset = applications.len().saturating_sub(application_capacity);
        let start = self.wm_menu_offset.min(maximum_offset);
        let end = start
            .saturating_add(application_capacity)
            .min(applications.len());
        let mut rows = Vec::new();
        if paginated {
            rows.push(WmMenuRow::ScrollUp);
        }
        rows.extend(
            applications[start..end]
                .iter()
                .cloned()
                .map(WmMenuRow::Application),
        );
        if rows.is_empty() {
            rows.push(WmMenuRow::Empty);
        }
        if paginated {
            rows.push(WmMenuRow::ScrollDown);
        }
        rows
    }

    pub(super) fn wm_menu_visible_rows(&self) -> usize {
        let available_height = self.wm_menu_available_height();
        (available_height / self.wm_menu_base_row_height()).max(4)
    }

    pub(super) fn wm_application_capacity(&self, application_count: usize) -> usize {
        let visible_rows = self.wm_menu_visible_rows();
        let simple_capacity = visible_rows.max(1);
        if application_count > simple_capacity {
            visible_rows.saturating_sub(2).max(1)
        } else {
            simple_capacity
        }
    }

    pub(super) fn wm_application_max_offset(&self, category: u8) -> usize {
        let application_count = self
            .wm_menu_entries
            .iter()
            .filter(|entry| entry.category == category)
            .count();
        application_count.saturating_sub(self.wm_application_capacity(application_count))
    }

    pub(super) fn wm_theme_max_offset(&self) -> usize {
        let simple_capacity = self.wm_menu_visible_rows().saturating_sub(2).max(1);
        let capacity = if self.wm_themes.len() > simple_capacity {
            self.wm_menu_visible_rows().saturating_sub(4).max(1)
        } else {
            simple_capacity
        };
        self.wm_themes.len().saturating_sub(capacity)
    }
}
