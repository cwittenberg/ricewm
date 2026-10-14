//! Window classification operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn window_stack_index(&self, window: &Window) -> u16 {
        self.windows
            .get(&window.parent.0)
            .and_then(|parent| parent.children.iter().position(|child| *child == window.id))
            .and_then(|index| u16::try_from(index).ok())
            .unwrap_or(0)
    }

    pub(super) fn wm_backdrop_material_window(&self, id: ResourceId) -> bool {
        let id = output_layout::shell_replica_source(id).unwrap_or(id);
        if id == WM_TASKBAR && self.wm_dock_style { return true; }
        self.wm_themes
            .get(usize::from(self.wm_theme))
            .is_some_and(|theme| {
                (theme.backdrop_material
                    && (Self::is_wm_shell_window(id) || self.frame_clients.contains_key(&id.0)))
                    || (id == WM_MENU && theme.menu_backdrop_material)
            })
    }

    pub(super) fn wm_active_frame_window(&self, id: ResourceId) -> bool {
        self.frame_clients
            .get(&id.0)
            .is_some_and(|client| *client == self.focus_window)
            && !self.native_tasks.iter().any(|task| {
                task.workspace == self.current_workspace && task.active && !task.minimized
            })
    }

    pub(super) fn wm_maximized_frame_window(&self, id: ResourceId) -> bool {
        self.frame_clients
            .get(&id.0)
            .is_some_and(|client| self.maximized_geometries.contains_key(&client.0))
    }

    pub(super) fn wm_resizable_frame_window(&self, id: ResourceId) -> bool {
        self.frame_clients
            .get(&id.0)
            .is_some_and(|client| !self.non_resizable_managed_windows.contains(&client.0))
    }

    pub(super) fn is_wm_shell_window(id: ResourceId) -> bool {
        let id = output_layout::shell_replica_source(id).unwrap_or(id);
        matches!(
            id,
            WM_TASKBAR
                | WM_GNOME_PANEL
                | WM_GNOME_MENU
                | WM_GNOME_DIALOG
                | WM_MENU
                | WM_TASK_PREVIEW
                | WM_TOOLTIP
                | WM_SNAP_PREVIEW
                | WM_NOTIFICATION
        )
    }

    pub(super) fn is_wm_desktop_descendant(&self, id: ResourceId) -> bool {
        let mut current = id;
        for _ in 0..256 {
            if self.window_has_named_type(current, "_NET_WM_WINDOW_TYPE_DESKTOP") {
                return true;
            }
            let Some(window) = self.windows.get(&current.0) else {
                break;
            };
            if current == ROOT_WINDOW || window.parent == current {
                break;
            }
            current = window.parent;
        }
        false
    }

    pub(super) fn is_parked_managed_descendant(&self, id: ResourceId) -> bool {
        self.managed_client_ancestor(id)
            .is_some_and(|client| self.parked_managed_windows.contains(&client.0))
    }

    pub(super) fn window_has_named_type(&self, id: ResourceId, type_name: &str) -> bool {
        self.windows.get(&id.0).is_some_and(|window| {
            window.properties.iter().any(|(property_atom, property)| {
                self.atoms.name(*property_atom) == Some("_NET_WM_WINDOW_TYPE")
                    && property.format == 32
                    && property.data.chunks_exact(4).any(|value| {
                        self.atoms
                            .name(AtomId(u32::from_le_bytes(value.try_into().unwrap())))
                            == Some(type_name)
                    })
            })
        })
    }

    pub(super) fn fault_client(client: &mut Client) {
        client.phase = ClientPhase::Faulted;
        client.ingress.clear();
        client.ingress_offset = 0;
    }
}
