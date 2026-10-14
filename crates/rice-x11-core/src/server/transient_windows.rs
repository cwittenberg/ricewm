//! Transient windows operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub(super) fn managed_client_ancestor(&self, mut window: ResourceId) -> Option<ResourceId> {
        for _ in 0..64 {
            if self.managed_frames.contains_key(&window.0) {
                return Some(window);
            }
            if let Some(client) = self.frame_clients.get(&window.0).copied() {
                return Some(client);
            }
            let parent = self.windows.get(&window.0)?.parent;
            if parent == window || parent == ROOT_WINDOW {
                return None;
            }
            window = parent;
        }
        None
    }

    pub(super) fn managed_resize_frame_at(
        &self,
        source: ResourceId,
        root_x: i16,
        root_y: i16,
    ) -> Option<(ResourceId, u8)> {
        let client = self.managed_client_ancestor(source)?;
        if self.non_resizable_managed_windows.contains(&client.0)
            || self.maximized_geometries.contains_key(&client.0)
        {
            return None;
        }
        let frame = *self.managed_frames.get(&client.0)?;
        let state = self.windows.get(&frame.0)?;
        if !state.mapped {
            return None;
        }
        let local_x = i32::from(root_x).saturating_sub(i32::from(state.x));
        let local_y = i32::from(root_y).saturating_sub(i32::from(state.y));
        if local_x < 0
            || local_y < 0
            || local_x >= i32::from(state.width)
            || local_y >= i32::from(state.height)
        {
            return None;
        }
        let mut edges = 0;
        if local_x < WM_RESIZE_HIT_WIDTH {
            edges |= WM_RESIZE_LEFT;
        } else if local_x >= i32::from(state.width).saturating_sub(WM_RESIZE_HIT_WIDTH) {
            edges |= WM_RESIZE_RIGHT;
        }
        if local_y < WM_RESIZE_HIT_WIDTH {
            edges |= WM_RESIZE_TOP;
        } else if local_y >= i32::from(state.height).saturating_sub(WM_RESIZE_HIT_WIDTH) {
            edges |= WM_RESIZE_BOTTOM;
        }
        (edges != 0).then_some((frame, edges))
    }

    /// Return the managed owner named by an ICCCM `WM_TRANSIENT_FOR` property.
    /// This structural relation remains valid while either side is hidden, so
    /// taskbar/Activities grouping cannot split during minimize/show-desktop.
    pub(super) fn managed_transient_owner_relation(&self, client: ResourceId) -> Option<ResourceId> {
        let transient_for = self.windows.get(&client.0)?.properties.iter().find_map(
            |(property_atom, property)| {
                (self.atoms.name(*property_atom) == Some("WM_TRANSIENT_FOR")
                    && self.atoms.name(property.r#type) == Some("WINDOW")
                    && property.format == 32
                    && property.data.len() == 4)
                    .then(|| ResourceId(u32::from_le_bytes(property.data[..4].try_into().unwrap())))
            },
        )?;
        let owner = self.managed_client_ancestor(transient_for)?;
        (owner != client).then_some(owner)
    }

    /// A malformed, withdrawn, or parked structural owner is not a valid
    /// destination for focus restoration.
    pub(super) fn managed_transient_owner(&self, client: ResourceId) -> Option<ResourceId> {
        let owner = self.managed_transient_owner_relation(client)?;
        (self.map_state(owner) == 2 && !self.parked_managed_windows.contains(&owner.0))
            .then_some(owner)
    }

    /// Resolve the outermost application owner for an ICCCM transient chain.
    /// Cycles and unreasonably deep/malformed chains are bounded defensively.
    pub(super) fn managed_transient_root(&self, mut client: ResourceId) -> ResourceId {
        for _ in 0..64 {
            let Some(owner) = self.managed_transient_owner_relation(client) else {
                break;
            };
            if owner == client {
                break;
            }
            client = owner;
        }
        client
    }

    /// Return the topmost mapped transient owned directly or indirectly by
    /// `owner`. Root-child stacking is authoritative because managed client
    /// windows are reparented into those frames.
    #[cfg(test)]
    pub(super) fn topmost_managed_transient(&self, owner: ResourceId) -> Option<ResourceId> {
        self.windows
            .get(&ROOT_WINDOW.0)?
            .children
            .iter()
            .rev()
            .filter_map(|frame| self.frame_clients.get(&frame.0).copied())
            .find(|candidate| {
                *candidate != owner
                    && self.map_state(*candidate) == 2
                    && !self.minimized_windows.contains(&candidate.0)
                    && !self.parked_managed_windows.contains(&candidate.0)
                    && self.managed_transient_root(*candidate) == owner
            })
    }

    /// EWMH distinguishes ownership from modality. ICCCM
    /// `WM_TRANSIENT_FOR` establishes the owner/stacking relationship, while
    /// `_NET_WM_STATE_MODAL` is what disables interaction with that owner.
    pub(super) fn managed_client_is_modal(&self, client: ResourceId) -> bool {
        self.windows.get(&client.0).is_some_and(|window| {
            window.properties.iter().any(|(property_atom, property)| {
                self.atoms.name(*property_atom) == Some("_NET_WM_STATE")
                    && self.atoms.name(property.r#type) == Some("ATOM")
                    && property.format == 32
                    && property.data.chunks_exact(4).any(|chunk| {
                        self.atoms
                            .name(AtomId(u32::from_le_bytes(chunk.try_into().unwrap())))
                            == Some("_NET_WM_STATE_MODAL")
                    })
            })
        })
    }

    pub(super) fn topmost_managed_modal_transient(&self, owner: ResourceId) -> Option<ResourceId> {
        self.windows
            .get(&ROOT_WINDOW.0)?
            .children
            .iter()
            .rev()
            .filter_map(|frame| self.frame_clients.get(&frame.0).copied())
            .find(|candidate| {
                *candidate != owner
                    && self.map_state(*candidate) == 2
                    && !self.minimized_windows.contains(&candidate.0)
                    && !self.parked_managed_windows.contains(&candidate.0)
                    && self.managed_transient_root(*candidate) == owner
                    && self.managed_client_is_modal(*candidate)
            })
    }

    pub(super) fn wm_owned_frame_window(&self, id: ResourceId) -> bool {
        self.frame_clients
            .get(&id.0)
            .copied()
            .is_some_and(|client| self.managed_transient_owner_relation(client).is_some())
    }

    pub(super) fn focus_fallback_for_withdrawn_window(&self, window: ResourceId) -> ResourceId {
        self.managed_client_ancestor(window)
            .and_then(|client| self.managed_transient_owner(client))
            .unwrap_or(ROOT_WINDOW)
    }

    pub(super) fn cycle_managed_focus(&mut self) {
        let clients: Vec<ResourceId> = self
            .managed_frames
            .keys()
            .copied()
            .filter(|client| {
                self.window_workspaces.get(client).copied() == Some(self.current_workspace)
                    && !self.minimized_windows.contains(client)
                    && !self.parked_managed_windows.contains(client)
            })
            .map(ResourceId)
            .collect();
        if clients.is_empty() {
            return;
        }
        let next = clients
            .iter()
            .position(|client| *client == self.focus_window)
            .map_or(clients[0], |index| clients[(index + 1) % clients.len()]);
        self.activate_managed_window(next);
    }
}
