//! Hit testing operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn propagated_targets(
        &self,
        source: ResourceId,
        mask: u32,
    ) -> Option<(ResourceId, ResourceId, Vec<ClientHandle>)> {
        let mut current = source;
        let mut child = ResourceId(0);
        for _ in 0..256 {
            let targets: Vec<ClientHandle> = self
                .clients
                .iter()
                .filter_map(|(id, client)| {
                    client
                        .event_masks
                        .get(&current.0)
                        .is_some_and(|selected| selected & mask != 0)
                        .then_some(ClientHandle(*id))
                })
                .collect();
            if !targets.is_empty() {
                return Some((current, child, targets));
            }
            let window = self.windows.get(&current.0)?;
            if u32::from(window.do_not_propagate_mask) & mask != 0
                || current == ROOT_WINDOW
                || window.parent == current
            {
                return None;
            }
            child = current;
            current = window.parent;
        }
        None
    }

    pub(super) fn hit_test(&self, root_x: i16, root_y: i16) -> ResourceId {
        self.hit_test_window(ROOT_WINDOW, 0, 0, i32::from(root_x), i32::from(root_y))
    }

    pub(super) fn child_at(&self, parent: ResourceId, root_x: i16, root_y: i16) -> ResourceId {
        let Some(window) = self.windows.get(&parent.0) else {
            return ResourceId(0);
        };
        let origin = self.window_origin(parent);
        for child_id in window.children.iter().rev() {
            if *child_id == WM_GNOME_BACKGROUND {
                continue;
            }
            let Some(child) = self.windows.get(&child_id.0) else {
                continue;
            };
            if self.map_state(*child_id) != 2 {
                continue;
            }
            if self.native_fullscreen_hides_shell(*child_id) { continue; }
            let left = origin.0.saturating_add(i32::from(child.x));
            let top = origin.1.saturating_add(i32::from(child.y));
            let border = i32::from(child.border_width);
            let right = left
                .saturating_add(i32::from(child.width))
                .saturating_add(border.saturating_mul(2));
            let bottom = top
                .saturating_add(i32::from(child.height))
                .saturating_add(border.saturating_mul(2));
            if i32::from(root_x) >= left
                && i32::from(root_x) < right
                && i32::from(root_y) >= top
                && i32::from(root_y) < bottom
            {
                return *child_id;
            }
        }
        ResourceId(0)
    }

    pub(super) fn hit_test_window(
        &self,
        id: ResourceId,
        origin_x: i32,
        origin_y: i32,
        root_x: i32,
        root_y: i32,
    ) -> ResourceId {
        let Some(window) = self.windows.get(&id.0) else {
            return ROOT_WINDOW;
        };
        for child_id in window.children.iter().rev() {
            let Some(child) = self.windows.get(&child_id.0) else {
                continue;
            };
            // The browser is RiceWM's compositor. Its protocol overlay is
            // never visible content, and EWMH desktop windows are deliberately
            // replaced by the native RiceWM desktop. Neither hidden layer may
            // intercept input intended for shell chrome, an X11 application,
            // or the native desktop.
            if *child_id == COMPOSITE_OVERLAY
                || *child_id == WM_GNOME_BACKGROUND
                || (self.config.native_window_manager && self.is_wm_desktop_descendant(*child_id))
                || self.is_parked_managed_descendant(*child_id)
                || self.native_fullscreen_hides_shell(*child_id)
            {
                continue;
            }
            if let Some(client)=self.frame_clients.get(&child_id.0) {
                if let Some(clip)=self.scrolling_clip(WmTaskTarget::X11(*client)) {
                    if root_x<clip.x||root_y<clip.y||root_x>=clip.x+i32::from(clip.width)||root_y>=clip.y+i32::from(clip.height){continue;}
                }
            }
            if self.map_state(*child_id) != 2 {
                continue;
            }
            let child_x = origin_x + i32::from(child.x);
            let child_y = origin_y + i32::from(child.y);
            let border = i32::from(child.border_width);
            let right = child_x + i32::from(child.width) + border.saturating_mul(2);
            let bottom = child_y + i32::from(child.height) + border.saturating_mul(2);
            if root_x >= child_x && root_x < right && root_y >= child_y && root_y < bottom {
                return self.hit_test_window(*child_id, child_x, child_y, root_x, root_y);
            }
        }
        id
    }

    pub(super) fn window_origin(&self, id: ResourceId) -> (i32, i32) {
        let mut current = id;
        let mut x = 0i32;
        let mut y = 0i32;
        for _ in 0..256 {
            let Some(window) = self.windows.get(&current.0) else {
                break;
            };
            x = x.saturating_add(i32::from(window.x));
            y = y.saturating_add(i32::from(window.y));
            if current == ROOT_WINDOW || window.parent == current {
                break;
            }
            current = window.parent;
        }
        (x, y)
    }
}
