use super::*;

    use super::*;

    include!("../../crates/rice-x11-core/tests/support/copy_area.rs");
    use crate::scene::{
        SCENE_DAMAGE_WINDOW, SCENE_REMOVE_WINDOW, SCENE_RESET, SCENE_UPSERT_WINDOW,
    };

    fn setup_request(byte_order: ClientByteOrder, auth_name: &[u8], auth_data: &[u8]) -> Vec<u8> {
        let mut request = Vec::new();
        request.push(match byte_order {
            ClientByteOrder::LittleEndian => b'l',
            ClientByteOrder::BigEndian => b'B',
        });
        request.push(0);
        push_u16(byte_order, &mut request, 11);
        push_u16(byte_order, &mut request, 0);
        push_u16(
            byte_order,
            &mut request,
            u16::try_from(auth_name.len()).unwrap(),
        );
        push_u16(
            byte_order,
            &mut request,
            u16::try_from(auth_data.len()).unwrap(),
        );
        request.extend_from_slice(&[0, 0]);
        request.extend_from_slice(auth_name);
        request.resize(SETUP_HEADER_BYTES + x11::pad4(auth_name.len()), 0);
        request.extend_from_slice(auth_data);
        request.resize(
            SETUP_HEADER_BYTES + x11::pad4(auth_name.len()) + x11::pad4(auth_data.len()),
            0,
        );
        request
    }

    fn request(byte_order: ClientByteOrder, opcode: u8, data: u8, body: &[u8]) -> Vec<u8> {
        assert!(body.len().is_multiple_of(4));
        let mut request = vec![opcode, data];
        push_u16(
            byte_order,
            &mut request,
            u16::try_from((body.len() + 4) / 4).unwrap(),
        );
        request.extend_from_slice(body);
        request
    }

    fn extended_request(byte_order: ClientByteOrder, opcode: u8, data: u8, body: &[u8]) -> Vec<u8> {
        assert!(body.len().is_multiple_of(4));
        let mut request = vec![opcode, data];
        push_u16(byte_order, &mut request, 0);
        push_u32(
            byte_order,
            &mut request,
            u32::try_from((body.len() + 8) / 4).unwrap(),
        );
        request.extend_from_slice(body);
        request
    }

    fn push_u16(byte_order: ClientByteOrder, output: &mut Vec<u8>, value: u16) {
        let bytes = match byte_order {
            ClientByteOrder::LittleEndian => value.to_le_bytes(),
            ClientByteOrder::BigEndian => value.to_be_bytes(),
        };
        output.extend_from_slice(&bytes);
    }

    fn push_u32(byte_order: ClientByteOrder, output: &mut Vec<u8>, value: u32) {
        let bytes = match byte_order {
            ClientByteOrder::LittleEndian => value.to_le_bytes(),
            ClientByteOrder::BigEndian => value.to_be_bytes(),
        };
        output.extend_from_slice(&bytes);
    }

    fn push_i32(byte_order: ClientByteOrder, output: &mut Vec<u8>, value: i32) {
        let bytes = match byte_order {
            ClientByteOrder::LittleEndian => value.to_le_bytes(),
            ClientByteOrder::BigEndian => value.to_be_bytes(),
        };
        output.extend_from_slice(&bytes);
    }

    fn drain_all(server: &mut DisplayServer, client: ClientHandle) -> Vec<u8> {
        let pending = server.pending_output(client).unwrap();
        let mut output = vec![0; pending];
        assert_eq!(server.drain(client, &mut output).unwrap(), pending);
        output
    }

    fn drain_scene(server: &mut DisplayServer) -> Vec<u8> {
        let pending = server.pending_scene();
        let mut output = vec![0; pending];
        assert_eq!(server.drain_scene(&mut output), pending);
        output
    }

    fn assert_incremental_restack(scene: &[u8], application_ids: &[u32]) {
        assert!(!scene.is_empty());
        assert!(
            scene
                .chunks_exact(SCENE_COMMAND_BYTES)
                .all(|command| command[0] != SCENE_RESET),
            "task activation must update stacking/chrome without a full scene reset",
        );
        assert!(
            scene
                .chunks_exact(SCENE_COMMAND_BYTES)
                .filter(|command| command[0] == SCENE_DAMAGE_WINDOW)
                .all(|command| {
                    let id = u32::from_le_bytes(command[4..8].try_into().unwrap());
                    !application_ids.contains(&id)
                }),
            "task activation must not re-upload unchanged application surfaces",
        );
    }

    fn ready_server(byte_order: ClientByteOrder) -> (DisplayServer, ClientHandle) {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let client = server.open_client().unwrap();
        let setup = setup_request(byte_order, &[], &[]);
        assert_eq!(server.feed(client, &setup).unwrap(), setup.len());
        assert_eq!(server.status(client).unwrap(), ClientStatus::Ready);
        let response = drain_all(&mut server, client);
        assert_eq!(response[0], 1);
        (server, client)
    }

    fn create_window_request(order: ClientByteOrder, window: u32, event_mask: u32) -> Vec<u8> {
        let mut body = Vec::new();
        push_u32(order, &mut body, window);
        push_u32(order, &mut body, ROOT_WINDOW.0);
        push_u16(order, &mut body, 12);
        push_u16(order, &mut body, 18);
        push_u16(order, &mut body, 320);
        push_u16(order, &mut body, 200);
        push_u16(order, &mut body, 1);
        push_u16(order, &mut body, 1);
        push_u32(order, &mut body, ROOT_VISUAL.0);
        let mask = (1 << 1) | (1 << 11);
        push_u32(order, &mut body, mask);
        push_u32(order, &mut body, 0x0022_3344);
        push_u32(order, &mut body, event_mask);
        request(order, 1, 24, &body)
    }

    fn create_child_window_request(
        order: ClientByteOrder,
        window: u32,
        parent: u32,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        event_mask: u32,
    ) -> Vec<u8> {
        let mut body = Vec::new();
        push_u32(order, &mut body, window);
        push_u32(order, &mut body, parent);
        push_u16(order, &mut body, x);
        push_u16(order, &mut body, y);
        push_u16(order, &mut body, width);
        push_u16(order, &mut body, height);
        push_u16(order, &mut body, 0);
        push_u16(order, &mut body, 1);
        push_u32(order, &mut body, ROOT_VISUAL.0);
        let mask = (1 << 1) | (1 << 11);
        push_u32(order, &mut body, mask);
        push_u32(order, &mut body, 0x00ee_eeee);
        push_u32(order, &mut body, event_mask);
        request(order, 1, 24, &body)
    }

    fn intern_atom_request(order: ClientByteOrder, name: &str) -> Vec<u8> {
        let mut body = Vec::new();
        push_u16(order, &mut body, name.len().try_into().unwrap());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(name.as_bytes());
        body.resize(x11::pad4(body.len()), 0);
        request(order, 16, 0, &body)
    }

    fn query_extension_request(order: ClientByteOrder, name: &str) -> Vec<u8> {
        let mut body = Vec::new();
        push_u16(order, &mut body, name.len().try_into().unwrap());
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(name.as_bytes());
        body.resize(x11::pad4(body.len()), 0);
        request(order, 98, 0, &body)
    }
mod pointer_wire_timestamps_follow_elapsed_milliseconds_and_wrap;
mod modifier_transitions_keep_xkb_state_and_core_event_state_coherent;
mod coalesced_requests_preserve_partial_tail_at_every_boundary;
mod unrelated_root_xinput_selection_cannot_suppress_top_window_core_click;
mod multimonitor_x11_drag_commits_destination_and_retains_it_on_relayout;
mod native_wm_menu_can_be_dismissed_without_forwarding_a_pointer_press;
mod gnome_overview_visibility_controls_canonical_taskbar_elements;
mod side_taskbar_start_menu_opens_beside_its_button_and_cascades_inward;
mod native_wm_restores_transient_owner_focus_on_unmap_and_destroy;
mod window_search_includes_other_workspaces_and_minimized_but_excludes_preloads;
mod scrolling_workspace_switch_leaves_other_monitor_x11_windows_mapped;
mod managed_right_edge_drag_resizes_live_before_button_release;
mod window_icon_menu_and_alt_window_shortcuts_share_task_commands;
mod depth_32_put_image_preserves_argb_alpha;
mod render_pictures_alpha_masks_and_a8_glyphs_paint_retained_pixels;
mod gnome_50_panel_keeps_taskbar_and_renders_extension_indicator;
mod fourth_native_instance_becomes_an_exact_task_group_menu;
