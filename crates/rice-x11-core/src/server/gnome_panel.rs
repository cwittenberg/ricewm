//! Gnome panel operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    /// Render a GNOME 50-style top panel from the translated extension actor
    /// trees. Each enabled extension contributes its own indicator; the
    /// server lays them out left/center/right exactly like GNOME's panel
    /// boxes, so multiple extensions never overwrite each other.
    pub fn update_gnome_panel(&mut self, enabled: bool, indicators: Vec<WmGnomeIndicator>) -> bool {
        if !self.config.native_window_manager || indicators.len() > 16 {
            return false;
        }
        if indicators.iter().any(|indicator| {
            indicator.position > 2
                || indicator.uuid.len() > 128
                || indicator.label.len() > 128
                || std::str::from_utf8(&indicator.uuid).is_err()
                || std::str::from_utf8(&indicator.label).is_err()
                || indicator
                    .label
                    .iter()
                    .any(|byte| *byte < b' ' && !matches!(*byte, b'\t'))
                || indicator.uuid.is_empty()
        }) {
            return false;
        }
        let next_indicators = if enabled {
            // Panel snapshots describe the actor tree, while decoded icon
            // pixels arrive separately.  GNOME extensions publish snapshots
            // on every main-loop turn; retaining an already uploaded image is
            // therefore essential.  Dropping it here made file icons blink
            // and forced the worker to decode/upload them every 500 ms.
            indicators
                .into_iter()
                .map(|mut indicator| {
                    if indicator.image.is_none() {
                        indicator.image = self
                            .wm_gnome_indicators
                            .iter()
                            .find(|existing| existing.uuid == indicator.uuid)
                            .and_then(|existing| existing.image.clone());
                    }
                    indicator
                })
                .collect()
        } else {
            Vec::new()
        };
        // Animated extensions publish on every main-loop turn even when their
        // panel actors did not change. Treat that as retained state: rebuilding
        // the panel used to clear the compact-dock hover between animation
        // frames, making its magnification visibly collapse and reappear.
        if self.wm_gnome_panel == enabled && self.wm_gnome_indicators == next_indicators {
            return true;
        }
        self.wm_gnome_panel = enabled;
        self.wm_gnome_indicators = next_indicators;
        if !enabled {
            self.wm_gnome_menu_items.clear();
            self.wm_gnome_menu_image = None;
            self.wm_gnome_menu_icons.clear();
            if let Some(menu) = self.windows.get_mut(&WM_GNOME_MENU.0) {
                menu.mapped = false;
            }
        }
        if let Some(panel) = self.windows.get_mut(&WM_GNOME_PANEL.0) {
            panel.mapped = enabled;
        }
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        self.refresh_gnome_panel_geometry();
        self.refresh_taskbar_geometry();
        self.relayout_wm_menu();
        self.publish_wm_root_properties();
        self.relayout_wm_edge_windows();
        self.rebuild_scene();
        true
    }

    /// Apply GNOME Shell panel geometry changed by extensions such as Just
    /// Perfection. This affects the real compositor surface and work area.
    pub fn configure_gnome_panel(&mut self, height: u16, bottom: bool) -> bool {
        if !self.config.native_window_manager || !(1..=128).contains(&height) {
            return false;
        }
        let height = height.min(self.config.height_px);
        if self.wm_gnome_panel_height == height && self.wm_gnome_panel_bottom == bottom {
            return true;
        }
        self.wm_gnome_panel_height = height;
        self.wm_gnome_panel_bottom = bottom;
        self.refresh_gnome_panel_geometry();
        self.refresh_taskbar_geometry();
        self.publish_wm_root_properties();
        self.relayout_wm_edge_windows();
        self.rebuild_scene();
        true
    }

    /// Replace the translated extension-owned popup menu. Each retained row
    /// preserves the original GJS item's activation index.
    pub fn update_gnome_menu(&mut self, items: Vec<WmGnomeMenuItem>) -> bool {
        if !self.config.native_window_manager
            || items.len() > 64
            || items.iter().any(|item| {
                item.label.len() > 192
                    || std::str::from_utf8(&item.label).is_err()
                    || item
                        .label
                        .iter()
                        .any(|byte| *byte < b' ' && !matches!(*byte, b'\t'))
            })
        {
            return false;
        }
        self.wm_gnome_menu_items = items;
        let file_icon_indexes = self
            .wm_gnome_menu_items
            .iter()
            .filter(|item| item.icon_kind == 7)
            .map(|item| item.index)
            .collect::<BTreeSet<_>>();
        self.wm_gnome_menu_icons
            .retain(|index, _| file_icon_indexes.contains(index));
        if self.wm_gnome_menu_items.iter().all(|item| !item.image) {
            self.wm_gnome_menu_image = None;
        }
        let (primary_x, primary_y, primary_width, primary_height) = self.wm_panel_output_rect();
        let width = WM_GNOME_MENU_WIDTH.min(primary_width).max(1);
        let maximum_height = primary_height
            .saturating_sub(WM_TASKBAR_HEIGHT.saturating_mul(2))
            .max(1);
        let content_height = self
            .wm_gnome_menu_items
            .iter()
            .map(wm_gnome_menu_item_height)
            .sum::<usize>()
            .saturating_add(12);
        let height = u16::try_from(content_height)
            .unwrap_or(maximum_height)
            .min(maximum_height)
            .max(1);
        let indicator = self
            .wm_gnome_indicator_layout()
            .into_iter()
            .find(|(order, _, _)| {
                self.wm_gnome_indicators
                    .get(usize::from(*order))
                    .is_some_and(|indicator| indicator.uuid == self.wm_gnome_menu_indicator_uuid)
            })
            .or_else(|| self.wm_gnome_indicator_layout().into_iter().last());
        let x = indicator.map_or(0, |(order, indicator_x, indicator_width)| {
            let left = i32::from(indicator_x);
            let right = left.saturating_add(i32::from(indicator_width));
            let left_anchored = self
                .wm_gnome_indicators
                .get(usize::from(order))
                .is_some_and(|indicator| indicator.position == 0);
            let proposed = if left_anchored {
                left
            } else {
                right.saturating_sub(i32::from(width))
            };
            clamp_i16(proposed.clamp(0, i32::from(primary_width.saturating_sub(width))))
        });
        let _ = self.resize_surface(WM_GNOME_MENU, width, height);
        if let Some(menu) = self.windows.get_mut(&WM_GNOME_MENU.0) {
            menu.x = x.saturating_add(primary_x as i16);
            menu.y = clamp_i16(i32::from(primary_y) + if self.wm_gnome_panel_bottom { i32::from(primary_height.saturating_sub(self.wm_gnome_panel_height).saturating_sub(height)) } else { i32::from(self.wm_gnome_panel_height) });
            menu.width = width;
            menu.height = height;
            menu.mapped = self.wm_gnome_panel && !self.wm_gnome_menu_items.is_empty();
        }
        self.paint_wm_chrome(WM_GNOME_MENU);
        if self
            .windows
            .get(&WM_GNOME_MENU.0)
            .is_some_and(|menu| menu.mapped)
        {
            self.restack(WM_GNOME_MENU, None, Some(0));
        }
        self.scene_upsert(WM_GNOME_MENU);
        self.scene_damage(WM_GNOME_MENU);
        true
    }

    /// Remember which extension owns the open popup so the menu window
    /// anchors to that extension's indicator.
    pub fn set_gnome_menu_indicator(&mut self, uuid: Vec<u8>) -> bool {
        if !self.config.native_window_manager
            || uuid.is_empty()
            || uuid.len() > 128
            || std::str::from_utf8(&uuid).is_err()
        {
            return false;
        }
        if !self
            .wm_gnome_indicators
            .iter()
            .any(|indicator| indicator.uuid == uuid)
        {
            return false;
        }
        self.wm_gnome_menu_indicator_uuid = uuid;
        self.relayout_wm_menu();
        self.paint_wm_chrome(WM_GNOME_MENU);
        self.scene_upsert(WM_GNOME_MENU);
        self.scene_damage(WM_GNOME_MENU);
        true
    }

    /// Render a bounded centered modal dialog for an extension's first-run
    /// or blocking prompt. GNOME shells draw modal dialogs on top of the
    /// workspace; RiceWM mirrors that with a native overlay whose buttons
    /// route back to the extension's dialog interaction API.
    pub fn update_gnome_dialog(
        &mut self,
        uuid: Vec<u8>,
        title: Vec<u8>,
        body: Vec<Vec<u8>>,
        buttons: Vec<Vec<u8>>,
    ) -> bool {
        if !self.config.native_window_manager
            || uuid.is_empty()
            || uuid.len() > 128
            || title.len() > 256
            || body.len() > 24
            || buttons.len() > 16
            || body.iter().any(|line| line.is_empty() || line.len() > 256)
            || buttons
                .iter()
                .any(|label| label.is_empty() || label.len() > 64)
            || std::str::from_utf8(&uuid).is_err()
            || std::str::from_utf8(&title).is_err()
            || body.iter().any(|line| std::str::from_utf8(line).is_err())
            || buttons
                .iter()
                .any(|label| std::str::from_utf8(label).is_err())
        {
            return false;
        }
        self.wm_gnome_dialog_uuid = uuid;
        self.wm_gnome_dialog_title = title;
        self.wm_gnome_dialog_body = body;
        self.wm_gnome_dialog_buttons = buttons;
        self.wm_gnome_dialog_fullscreen = false;
        let width = 900u16.min(self.config.width_px.saturating_sub(40)).max(320);
        let height = 620u16
            .min(self.config.height_px.saturating_sub(80))
            .max(240);
        if let Some(dialog) = self.windows.get_mut(&WM_GNOME_DIALOG.0) {
            dialog.x = i16::try_from((i32::from(self.config.width_px) - i32::from(width)) / 2)
                .unwrap_or(0)
                .max(0);
            dialog.y = i16::try_from(
                i32::from(self.config.height_px).saturating_sub(i32::from(height)) / 2,
            )
            .unwrap_or(0)
            .max(0);
            dialog.width = width;
            dialog.height = height;
            dialog.mapped = true;
            dialog.depth = 32;
        }
        let _ = self.resize_surface(WM_GNOME_DIALOG, width, height);
        self.paint_wm_chrome(WM_GNOME_DIALOG);
        if let Some(dialog) = self.windows.get(&WM_GNOME_DIALOG.0) {
            self.restack(WM_GNOME_DIALOG, None, Some(dialog.depth));
        }
        // Publish the scene record only after restacking so the compositor
        // receives the dialog's final topmost stack index in this flush.
        self.scene_upsert(WM_GNOME_DIALOG);
        self.scene_damage(WM_GNOME_DIALOG);
        true
    }

    /// Switch the extension dialog surface into a stage-sized overlay. The
    /// content still comes from the extension's live St actor tree; this only
    /// changes the compositor geometry used to present it.
    pub fn set_gnome_dialog_fullscreen(&mut self, fullscreen: bool) -> bool {
        if !self.config.native_window_manager || self.wm_gnome_dialog_uuid.is_empty() {
            return false;
        }
        self.wm_gnome_dialog_fullscreen = fullscreen;
        let (width, height, x, y) = if fullscreen {
            (self.config.width_px, self.config.height_px, 0, 0)
        } else {
            let width = 900u16.min(self.config.width_px.saturating_sub(40)).max(320);
            let height = 620u16
                .min(self.config.height_px.saturating_sub(80))
                .max(240);
            let x = i16::try_from((i32::from(self.config.width_px) - i32::from(width)) / 2)
                .unwrap_or(0)
                .max(0);
            let y = i16::try_from((i32::from(self.config.height_px) - i32::from(height)) / 2)
                .unwrap_or(0)
                .max(0);
            (width, height, x, y)
        };
        if let Some(dialog) = self.windows.get_mut(&WM_GNOME_DIALOG.0) {
            dialog.x = x;
            dialog.y = y;
            dialog.width = width;
            dialog.height = height;
            dialog.mapped = true;
        }
        let _ = self.resize_surface(WM_GNOME_DIALOG, width, height);
        self.paint_wm_chrome(WM_GNOME_DIALOG);
        self.scene_upsert(WM_GNOME_DIALOG);
        self.scene_damage(WM_GNOME_DIALOG);
        true
    }

    pub fn update_gnome_overlay_rects(&mut self, rectangles: Vec<WmGnomeOverlayRect>) -> bool {
        if !self.config.native_window_manager || rectangles.len() > 512 {
            return false;
        }
        self.wm_gnome_overlay_rects = rectangles;
        if self.wm_gnome_dialog_fullscreen {
            self.paint_wm_chrome(WM_GNOME_DIALOG);
            self.scene_upsert(WM_GNOME_DIALOG);
            self.scene_damage(WM_GNOME_DIALOG);
        }
        true
    }

    pub fn update_gnome_background(
        &mut self,
        rectangles: Vec<WmGnomeOverlayRect>,
        texts: Vec<WmGnomeBackgroundText>,
    ) -> bool {
        if !self.config.native_window_manager || rectangles.len() > 512 || texts.len() > 4096 {
            return false;
        }
        if self.wm_gnome_background_rects == rectangles
            && self.wm_gnome_background_texts == texts
        {
            return true;
        }
        self.wm_gnome_background_rects = rectangles;
        self.wm_gnome_background_texts = texts;
        let mapped = !self.wm_gnome_background_rects.is_empty()
            || !self.wm_gnome_background_texts.is_empty();
        if let Some(background) = self.windows.get_mut(&WM_GNOME_BACKGROUND.0) {
            background.mapped = mapped;
            background.width = self.config.width_px;
            background.height = self.config.height_px;
        }
        let _ = self.resize_surface(
            WM_GNOME_BACKGROUND,
            self.config.width_px,
            self.config.height_px,
        );
        self.paint_wm_chrome(WM_GNOME_BACKGROUND);
        self.scene_upsert(WM_GNOME_BACKGROUND);
        self.scene_damage(WM_GNOME_BACKGROUND);
        true
    }

    pub fn clear_gnome_dialog(&mut self) -> bool {
        self.wm_gnome_dialog_uuid.clear();
        self.wm_gnome_dialog_title.clear();
        self.wm_gnome_dialog_body.clear();
        self.wm_gnome_dialog_buttons.clear();
        self.wm_gnome_dialog_fullscreen = false;
        self.wm_gnome_overlay_rects.clear();
        if let Some(dialog) = self.windows.get_mut(&WM_GNOME_DIALOG.0) {
            dialog.mapped = false;
        }
        self.scene_upsert(WM_GNOME_DIALOG);
        self.scene_damage(WM_GNOME_DIALOG);
        true
    }

    pub(super) fn wm_gnome_dialog_button_at(&self, root_x: i16, root_y: i16) -> Option<u8> {
        let dialog = self.windows.get(&WM_GNOME_DIALOG.0)?;
        if !dialog.mapped {
            return None;
        }
        let local_x = i32::from(root_x).saturating_sub(i32::from(dialog.x));
        let local_y = i32::from(root_y).saturating_sub(i32::from(dialog.y));
        let count = self.wm_gnome_dialog_buttons.len();
        if count == 0 {
            return None;
        }
        let columns = count.clamp(1, 4);
        let rows = count.div_ceil(columns);
        let button_width =
            (i32::from(dialog.width) - 48 - (columns as i32 - 1) * 10) / columns as i32;
        let total_width = columns as i32 * (button_width + 10) - 10;
        let start_x = (i32::from(dialog.width) - total_width) / 2;
        let first_button_y = i32::from(dialog.height).saturating_sub(rows as i32 * 44 + 10);
        for (index, _) in self.wm_gnome_dialog_buttons.iter().enumerate() {
            let column = index % columns;
            let row = index / columns;
            let left = start_x + column as i32 * (button_width + 10);
            let button_y = first_button_y + row as i32 * 44;
            if local_x >= left
                && local_x < left + button_width
                && local_y >= button_y
                && local_y < button_y + 40
            {
                return u8::try_from(index).ok();
            }
        }
        None
    }

    /// Upload one bounded decoded image used by an extension popup row.
    pub fn update_gnome_menu_image(&mut self, width: u16, height: u16, pixels: Vec<u32>) -> bool {
        let expected = usize::from(width).saturating_mul(usize::from(height));
        if !self.config.native_window_manager
            || width == 0
            || height == 0
            || width > 512
            || height > 512
            || expected != pixels.len()
        {
            return false;
        }
        self.wm_gnome_menu_image = Some(WmGnomeMenuImage {
            width,
            height,
            pixels,
        });
        self.paint_wm_chrome(WM_GNOME_MENU);
        self.scene_upsert(WM_GNOME_MENU);
        self.scene_damage(WM_GNOME_MENU);
        true
    }

    /// Upload a decoded file-backed icon for one retained popup item.
    pub fn update_gnome_menu_icon(
        &mut self,
        index: u8,
        width: u16,
        height: u16,
        pixels: Vec<u32>,
    ) -> bool {
        let expected = usize::from(width).saturating_mul(usize::from(height));
        if !self.config.native_window_manager
            || width == 0
            || height == 0
            || width > 128
            || height > 128
            || expected != pixels.len()
            || !self
                .wm_gnome_menu_items
                .iter()
                .any(|item| item.index == index && item.icon_kind == 7)
        {
            return false;
        }
        self.wm_gnome_menu_icons.insert(
            index,
            WmGnomeMenuImage {
                width,
                height,
                pixels,
            },
        );
        self.paint_wm_chrome(WM_GNOME_MENU);
        self.scene_upsert(WM_GNOME_MENU);
        self.scene_damage(WM_GNOME_MENU);
        true
    }

    /// Upload a decoded file-backed indicator icon such as a country flag.
    pub fn update_gnome_indicator_image(
        &mut self,
        uuid: Vec<u8>,
        width: u16,
        height: u16,
        pixels: Vec<u32>,
    ) -> bool {
        let expected = usize::from(width).saturating_mul(usize::from(height));
        if !self.config.native_window_manager
            || uuid.is_empty()
            || uuid.len() > 128
            || width == 0
            || height == 0
            || width > 128
            || height > 128
            || expected != pixels.len()
        {
            return false;
        }
        let Some(indicator) = self
            .wm_gnome_indicators
            .iter_mut()
            .find(|indicator| indicator.uuid == uuid)
        else {
            return false;
        };
        indicator.image = Some(WmGnomeMenuImage {
            width,
            height,
            pixels,
        });
        self.paint_wm_chrome(WM_GNOME_PANEL);
        self.scene_upsert(WM_GNOME_PANEL);
        self.scene_damage(WM_GNOME_PANEL);
        true
    }
}
