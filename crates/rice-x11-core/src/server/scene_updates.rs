//! Scene updates operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn scene_upsert(&mut self, id: ResourceId) {
        let Some(window) = self.windows.get(&id.0).cloned() else {
            return;
        };
        let viewable = self.map_state(id) == 2 && !self.is_parked_managed_descendant(id)
            && !self.native_fullscreen_hides_shell(id);
        // The scene wire has a 14-bit practical root-child stack index. Its
        // high bits carry task-switcher ownership and managed resize
        // capability so the page can present exact frame cursors without
        // changing retained composition hierarchy.
        let stack_index = self.window_stack_index(&window)
            | if self.wm_owned_frame_window(id) {
                SCENE_OWNED_WINDOW_STACK_FLAG
            } else {
                0
            }
            | if self.wm_resizable_frame_window(id) {
                SCENE_RESIZABLE_WINDOW_STACK_FLAG
            } else {
                0
            };
        let desktop_layer = id == COMPOSITE_OVERLAY || self.is_wm_desktop_descendant(id);
        let shell_layer = Self::is_wm_shell_window(id);
        let backdrop_material = self.wm_backdrop_material_window(id);
        let active_window = self.wm_active_frame_window(id);
        let maximized_window = self.wm_maximized_frame_window(id);
        if !self.scene.upsert(
            &window,
            viewable,
            stack_index,
            desktop_layer,
            shell_layer,
            backdrop_material,
            active_window,
            maximized_window,
        ) {
            self.rebuild_scene();
        }
    }

    pub(super) fn scene_upsert_subtree(&mut self, root: ResourceId) {
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            let children = self
                .windows
                .get(&id.0)
                .map(|window| window.children.clone())
                .unwrap_or_default();
            self.scene_upsert(id);
            pending.extend(children);
        }
    }

    pub(super) fn scene_upsert_root_children(&mut self) {
        let children = self
            .windows
            .get(&ROOT_WINDOW.0)
            .map(|root| root.children.clone())
            .unwrap_or_default();
        for child in children {
            self.scene_upsert(child);
        }
    }

    pub(super) fn scene_damage(&mut self, id: ResourceId) {
        let Some(window) = self.windows.get(&id.0).cloned() else {
            return;
        };
        self.scene_damage_rect(id, 0, 0, window.width, window.height);
    }

    pub(super) fn scene_damage_rect(&mut self, id: ResourceId, x: i32, y: i32, width: u16, height: u16) {
        if width == 0 || height == 0 || !self.windows.contains_key(&id.0) {
            return;
        }
        if !self.scene.damage(id, x, y, width, height) {
            self.rebuild_scene();
        }
    }

    pub(super) fn scene_remove(&mut self, id: ResourceId) {
        if !self.scene.remove(id) {
            self.rebuild_scene();
        }
    }

    pub(super) fn rebuild_scene(&mut self) {
        self.reconcile_taskbar_pointer(self.pointer_x, self.pointer_y);
        self.scene.clear();
        let _ = self.scene.reset();
        let snapshot: Vec<(Window, bool, u16, bool, bool, bool, bool, bool)> = self
            .windows
            .values()
            .map(|window| {
                (
                    window.clone(),
                    self.map_state(window.id) == 2 && !self.is_parked_managed_descendant(window.id)
                        && !self.native_fullscreen_hides_shell(window.id),
                    self.window_stack_index(window)
                        | if self.wm_owned_frame_window(window.id) {
                            SCENE_OWNED_WINDOW_STACK_FLAG
                        } else {
                            0
                        }
                        | if self.wm_resizable_frame_window(window.id) {
                            SCENE_RESIZABLE_WINDOW_STACK_FLAG
                        } else {
                            0
                        },
                    window.id == COMPOSITE_OVERLAY || self.is_wm_desktop_descendant(window.id),
                    Self::is_wm_shell_window(window.id),
                    self.wm_backdrop_material_window(window.id),
                    self.wm_active_frame_window(window.id),
                    self.wm_maximized_frame_window(window.id),
                )
            })
            .collect();
        for (
            window,
            viewable,
            stack_index,
            desktop_layer,
            shell_layer,
            backdrop_material,
            active_window,
            maximized_window,
        ) in snapshot
        {
            let _ = self.scene.upsert(
                &window,
                viewable,
                stack_index,
                desktop_layer,
                shell_layer,
                backdrop_material,
                active_window,
                maximized_window,
            );
            if window.class != 2 {
                let _ = self
                    .scene
                    .damage(window.id, 0, 0, window.width, window.height);
            }
        }
    }
}
