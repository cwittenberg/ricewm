use super::*;

impl DisplayServer {
    /// Replace trusted native fullscreen ownership for complete physical outputs.
    /// Menus, dialogs and notifications remain above applications. Stored panel
    /// preferences and X11 map state are unchanged, so exit restores them exactly.
    pub fn set_native_fullscreen_regions(&mut self, regions: Vec<(u16, u16, u16, u16)>) -> bool {
        let valid = |region: &(u16, u16, u16, u16)| {
            if self.wm_outputs.is_empty() {
                *region == (0, 0, self.config.width_px, self.config.height_px)
            } else {
                self.wm_outputs.contains(region)
            }
        };
        if regions.len() > 16
            || regions
                .iter()
                .enumerate()
                .any(|(i, region)| !valid(region) || regions[..i].contains(region))
        {
            return false;
        }
        if self.native_fullscreen_regions == regions {
            return true;
        }
        self.native_fullscreen_regions = regions;
        self.wm_chrome_hover = None;
        self.wm_chrome_pressed = None;
        let panels: Vec<_> = self
            .windows
            .values()
            .filter_map(|window| Self::native_fullscreen_panel(window.id).then_some(window.id))
            .collect();
        for panel in panels {
            self.scene_upsert(panel);
            self.scene_damage(panel);
        }
        true
    }

    fn native_fullscreen_panel(id: ResourceId) -> bool {
        matches!(id, WM_TASKBAR | WM_GNOME_PANEL)
            || output_layout::shell_replica_source(id).is_some()
    }

    pub(super) fn native_fullscreen_contains(&self, x: i32, y: i32) -> bool {
        self.native_fullscreen_regions
            .iter()
            .any(|&(left, top, width, height)| {
                x >= i32::from(left)
                    && y >= i32::from(top)
                    && x < i32::from(left) + i32::from(width)
                    && y < i32::from(top) + i32::from(height)
            })
    }

    pub(super) fn native_fullscreen_hides_shell(&self, id: ResourceId) -> bool {
        if !Self::native_fullscreen_panel(id) {
            return false;
        }
        let Some(window) = self.windows.get(&id.0) else {
            return false;
        };
        self.native_fullscreen_regions
            .iter()
            .any(|&(x, y, width, height)| {
                i32::from(window.x) >= i32::from(x)
                    && i32::from(window.y) >= i32::from(y)
                    && i32::from(window.x) + i32::from(window.width)
                        <= i32::from(x) + i32::from(width)
                    && i32::from(window.y) + i32::from(window.height)
                        <= i32::from(y) + i32::from(height)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene_visibility(server: &mut DisplayServer) -> BTreeMap<u32, bool> {
        let mut bytes = vec![0; server.pending_scene()];
        server.drain_scene(&mut bytes);
        bytes
            .chunks_exact(SCENE_COMMAND_BYTES)
            .filter(|record| record[0] == 1)
            .map(|record| {
                (
                    u32::from_le_bytes(record[4..8].try_into().unwrap()),
                    record[1] & 1 != 0,
                )
            })
            .collect()
    }

    fn fixture() -> DisplayServer {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1600,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.wm_gnome_panel = true;
        server.configure_topbar("");
        assert!(server.set_output_layout(1600, 600, vec![(0, 0, 800, 600), (800, 0, 800, 600)]));
        assert!(server.set_shell_monitors(true, true));
        server
    }

    #[test]
    fn fullscreen_masks_only_owned_output_panels_in_paint_and_hit_testing() {
        let mut server = fixture();
        scene_visibility(&mut server);
        assert_eq!(server.pointer_layer(10, 10), 2);
        assert!(server.set_native_fullscreen_regions(vec![(0, 0, 800, 600)]));
        let scene = scene_visibility(&mut server);
        assert_eq!(scene[&WM_GNOME_PANEL.0], false);
        assert_eq!(scene[&WM_TASKBAR.0], false);
        assert_eq!(scene[&0xe000_0102], true);
        assert_eq!(scene[&0xe000_0103], true);
        assert_eq!(server.pointer_layer(10, 10), 0);
        assert_eq!(server.pointer_layer(10, 590), 0);
        assert!(server.select_shell_output(810, 10));
        assert_eq!(server.pointer_layer(810, 10), 2);
        assert_eq!(server.pointer_layer(810, 590), 2);
        assert!(!server.native_fullscreen_hides_shell(WM_MENU));
        assert!(!server.native_fullscreen_hides_shell(WM_GNOME_DIALOG));
        assert!(!server.native_fullscreen_hides_shell(WM_NOTIFICATION));
        server.rebuild_scene();
        assert_eq!(scene_visibility(&mut server)[&0xe000_0100], false);
        assert!(server.set_native_fullscreen_regions(Vec::new()));
        assert_eq!(scene_visibility(&mut server)[&WM_GNOME_PANEL.0], true);
        assert!(server.select_shell_output(10, 10));
        assert_eq!(server.pointer_layer(10, 10), 2);
        assert!(server.wm_taskbar_visible);
        assert!(server.wm_gnome_panel);
    }

    #[test]
    fn fullscreen_region_updates_are_atomic_bounded_and_survive_shell_output_switches() {
        let mut server = fixture();
        let region = (0, 0, 800, 600);
        assert!(server.set_native_fullscreen_regions(vec![region]));
        scene_visibility(&mut server);
        for invalid in [vec![(0, 0, 799, 600)], vec![region; 2], vec![region; 17]] {
            assert!(!server.set_native_fullscreen_regions(invalid));
            assert_eq!(server.native_fullscreen_regions, vec![region]);
            assert!(scene_visibility(&mut server).is_empty());
        }
        assert!(server.select_shell_output(810, 10));
        assert_eq!(server.pointer_layer(10, 10), 0);
        assert_eq!(server.pointer_layer(810, 10), 2);
        let scene = scene_visibility(&mut server);
        assert_eq!(scene[&WM_GNOME_PANEL.0], true);
        assert_eq!(scene[&0xe000_0100], false);
        server.set_taskbar_configuration(true, 0, true);
        assert!(!server.update_taskbar_pointer(10, 599));
        assert!(
            !server.wm_taskbar_revealed,
            "hidden output edge cannot reveal its dock"
        );
    }
}
