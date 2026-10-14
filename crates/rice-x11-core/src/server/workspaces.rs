//! Workspaces operations owned by DisplayServer.
use super::*;

impl DisplayServer {

    pub fn switch_workspace(&mut self, workspace: u32) {
        self.refresh_scrolling_workspaces();
        if workspace >= self.workspace_count || workspace == self.current_workspace {
            return;
        }
        self.current_workspace = workspace;
        if self.scrolling_workspaces {self.output_workspaces.insert(self.wm_shell_output as u32,workspace);}
        self.set_wm_property(
            ROOT_WINDOW,
            "_NET_CURRENT_DESKTOP",
            "CARDINAL",
            32,
            workspace.to_le_bytes().to_vec(),
        );
        self.apply_workspace_visibility();
        self.set_focus(ROOT_WINDOW);
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
        self.wm_actions
            .push_back(WM_ACTION_WORKSPACE_PREFIX | workspace);
    }

    pub(super) fn move_managed_window_to_workspace(&mut self, client: ResourceId, workspace: u32) {
        if workspace >= self.workspace_count
            || !self.managed_frames.contains_key(&client.0)
            || self.window_workspaces.get(&client.0).copied() == Some(workspace)
        {
            return;
        }
        self.x11_output_homes.remove(&client.0);
        self.window_workspaces.insert(client.0, workspace);
        self.set_wm_property(
            client,
            "_NET_WM_DESKTOP",
            "CARDINAL",
            32,
            workspace.to_le_bytes().to_vec(),
        );
        self.apply_workspace_visibility();
        if self.managed_client_ancestor(self.focus_window) == Some(client) {
            self.set_focus(ROOT_WINDOW);
        }
        self.paint_wm_chrome(WM_TASKBAR);
        self.scene_upsert(WM_TASKBAR);
        self.scene_damage(WM_TASKBAR);
    }

    pub(super) fn apply_workspace_visibility(&mut self) {
        let managed: Vec<(u32, ResourceId)> = self
            .managed_frames
            .iter()
            .map(|(client, frame)| (*client, *frame))
            .collect();
        for (client, frame) in managed {
            let visible = self.x11_workspace_visible(client)
                && !self.minimized_windows.contains(&client);
            if let Some(window) = self.windows.get_mut(&frame.0) {
                window.mapped = visible;
            }
            self.scene_upsert(frame);
        }
    }

    pub(super) fn request_close_window(&mut self, client: ResourceId) {
        if !self.send_wm_protocol(client, "WM_DELETE_WINDOW") {
            self.destroy_window_tree(client);
        }
    }

    pub(super) fn send_wm_protocol(&mut self, client: ResourceId, protocol_name: &str) -> bool {
        let protocols = self.atoms.intern("WM_PROTOCOLS", false);
        let protocol = self.atoms.intern(protocol_name, false);
        let supports_protocol = self
            .windows
            .get(&client.0)
            .and_then(|window| window.properties.get(&protocols))
            .is_some_and(|property| {
                property
                    .data
                    .chunks_exact(4)
                    .any(|atom| read_u32_le(atom) == protocol.0)
            });
        if !supports_protocol {
            return false;
        }
        let Some(owner) = self.windows.get(&client.0).and_then(|window| window.owner) else {
            return false;
        };
        let time = self.next_timestamp();
        self.emit_to_client(ClientHandle(owner), |output, sequence, order| {
            let mut data = [0; 20];
            let protocol_bytes = match order {
                ClientByteOrder::LittleEndian => protocol.0.to_le_bytes(),
                ClientByteOrder::BigEndian => protocol.0.to_be_bytes(),
            };
            let time_bytes = match order {
                ClientByteOrder::LittleEndian => time.to_le_bytes(),
                ClientByteOrder::BigEndian => time.to_be_bytes(),
            };
            data[0..4].copy_from_slice(&protocol_bytes);
            data[4..8].copy_from_slice(&time_bytes);
            x11::encode_client_message_event(
                output,
                order,
                x11::ClientMessageEvent {
                    sequence,
                    send_event: true,
                    format: 32,
                    window: client,
                    r#type: protocols,
                    data,
                },
            );
        });
        true
    }
}
