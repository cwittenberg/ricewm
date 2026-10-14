use super::*;


    #[test]
    fn native_wm_restores_transient_owner_focus_on_unmap_and_destroy() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let main_window = ResourceId(0x0020_0182);
        let unmap_dialog = ResourceId(0x0020_0183);
        let destroy_dialog = ResourceId(0x0020_0184);
        for window in [main_window, unmap_dialog, destroy_dialog] {
            server
                .feed(
                    client,
                    &create_window_request(
                        order,
                        window.0,
                        STRUCTURE_NOTIFY_MASK | FOCUS_CHANGE_MASK,
                    ),
                )
                .unwrap();
        }
        for dialog in [unmap_dialog, destroy_dialog] {
            server.set_wm_property(
                dialog,
                "WM_TRANSIENT_FOR",
                "WINDOW",
                32,
                main_window.0.to_le_bytes().to_vec(),
            );
            let modal = server.atoms.intern("_NET_WM_STATE_MODAL", false);
            server.set_wm_property(
                dialog,
                "_NET_WM_STATE",
                "ATOM",
                32,
                modal.0.to_le_bytes().to_vec(),
            );
        }
        server
            .feed(client, &request(order, 8, 0, &main_window.0.to_le_bytes()))
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &unmap_dialog.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let active_window = server.atoms.intern("_NET_ACTIVE_WINDOW", true);
        assert_ne!(active_window.0, 0);
        let assert_active_window = |server: &DisplayServer, expected: ResourceId| {
            let property = &server.windows[&ROOT_WINDOW.0].properties[&active_window];
            assert_eq!(server.atoms.name(property.r#type), Some("WINDOW"));
            assert_eq!(property.format, 32);
            assert_eq!(property.data, expected.0.to_le_bytes());
        };
        let focus_events = |events: &[u8]| {
            events
                .chunks_exact(32)
                .filter(|event| matches!(event[0] & 0x7f, 9 | 10))
                .map(|event| {
                    (
                        event[0] & 0x7f,
                        event[1],
                        u32::from_le_bytes(event[4..8].try_into().unwrap()),
                    )
                })
                .collect::<Vec<_>>()
        };
        let main_frame = server.managed_frames[&main_window.0];
        let unmap_frame = server.managed_frames[&unmap_dialog.0];
        assert_eq!(server.focus_window, unmap_dialog);
        assert!(server.wm_active_frame_window(unmap_frame));
        assert_eq!(
            server.wm_task_targets(),
            vec![WmTaskTarget::X11(main_window)],
            "a mapped owned dialog stays grouped behind its owner's task entry",
        );
        server.windows.get_mut(&main_frame.0).unwrap().mapped = false;
        assert_eq!(
            server.wm_task_targets(),
            vec![WmTaskTarget::X11(main_window)],
            "minimize/show-desktop cannot split an owned dialog into another task entry",
        );
        server.windows.get_mut(&main_frame.0).unwrap().mapped = true;
        assert_eq!(server.managed_transient_root(unmap_dialog), main_window);
        assert_eq!(
            server.topmost_managed_transient(main_window),
            Some(unmap_dialog)
        );
        let root_children = &server.windows[&ROOT_WINDOW.0].children;
        assert!(
            root_children
                .iter()
                .position(|window| *window == unmap_frame)
                > root_children
                    .iter()
                    .position(|window| *window == main_frame),
            "the modal transient frame must remain above its owner",
        );
        let _ = drain_scene(&mut server);
        server.scene_upsert(unmap_frame);
        let owned_scene = drain_scene(&mut server);
        let owned_frame_command = owned_scene
            .chunks_exact(SCENE_COMMAND_BYTES)
            .find(|command| {
                command[0] == SCENE_UPSERT_WINDOW
                    && u32::from_le_bytes(command[4..8].try_into().unwrap()) == unmap_frame.0
            })
            .expect("owned dialog frame scene upsert");
        assert_ne!(
            u16::from_le_bytes(owned_frame_command[26..28].try_into().unwrap())
                & SCENE_OWNED_WINDOW_STACK_FLAG,
            0,
            "the retained compositor must identify owned frames to Activities",
        );

        // Keep part of the disabled owner physically exposed so the release
        // below exercises a changed natural hit target rather than landing on
        // the dialog's compositor frame again.
        if let Some(frame) = server.windows.get_mut(&unmap_frame.0) {
            frame.x = 400;
            frame.y = 100;
        }

        // Standard child controls rely on X11's automatic pointer grab to
        // receive the release which completes a click. A repaint/focus change
        // between the physical press and release can make the natural hit
        // target resolve to the disabled modal owner. The active grab must
        // take precedence over the WM's disabled-owner guard so that generic
        // retained buttons receive a complete down/up transaction.
        let button = ResourceId(0x0020_0185);
        server
            .feed(
                client,
                &create_child_window_request(
                    order,
                    button.0,
                    unmap_dialog.0,
                    20,
                    24,
                    60,
                    23,
                    BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK,
                ),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &button.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);
        let (button_x, button_y) = server.window_origin(button);
        server.inject_pointer_button(
            true,
            1,
            clamp_i16(button_x + 30),
            clamp_i16(button_y + 11),
            0,
        );
        let press = drain_all(&mut server, client);
        assert!(press.chunks_exact(32).any(|event| {
            event[0] & 0x7f == 4
                && u32::from_le_bytes(event[12..16].try_into().unwrap()) == button.0
        }));
        assert!(
            server
                .pointer_grab
                .is_some_and(|grab| { grab.automatic && grab.window == button })
        );
        let main_geometry = &server.windows[&main_frame.0];
        server.inject_pointer_button(
            false,
            1,
            main_geometry.x.saturating_add(4),
            main_geometry.y.saturating_add(4),
            0x0100,
        );
        let release = drain_all(&mut server, client);
        assert!(
            release.chunks_exact(32).any(|event| {
                event[0] & 0x7f == 5
                    && u32::from_le_bytes(event[12..16].try_into().unwrap()) == button.0
            }),
            "an automatic grab must deliver ButtonRelease to its armed child"
        );
        assert!(server.pointer_grab.is_none());

        // Even if a client explicitly asks to raise its disabled owner, the
        // compositor preserves the transient-above-owner ordering. A physical
        // press on the exposed part of that owner is consumed and retains
        // modal focus rather than dispatching into the owner.
        server.restack(main_frame, None, Some(0));
        let root_children = &server.windows[&ROOT_WINDOW.0].children;
        assert!(
            root_children
                .iter()
                .position(|window| *window == unmap_frame)
                > root_children
                    .iter()
                    .position(|window| *window == main_frame),
        );
        let main_geometry = &server.windows[&main_frame.0];
        server.inject_pointer_button(
            true,
            1,
            main_geometry.x.saturating_add(4),
            main_geometry.y.saturating_add(4),
            0,
        );
        assert_eq!(server.focus_window, unmap_dialog);
        assert_eq!(
            server.topmost_managed_transient(main_window),
            Some(unmap_dialog)
        );
        assert!(
            drain_all(&mut server, client)
                .chunks_exact(32)
                .all(|event| event[0] & 0x7f != 4),
            "disabled-owner pointer input must not become a ButtonPress event",
        );

        server
            .feed(
                client,
                &request(order, 10, 0, &unmap_dialog.0.to_le_bytes()),
            )
            .unwrap();
        let unmap_events = drain_all(&mut server, client);
        assert_eq!(
            focus_events(&unmap_events),
            vec![
                (10, NOTIFY_NONLINEAR, unmap_dialog.0),
                (9, NOTIFY_NONLINEAR, main_window.0),
            ],
        );
        assert_eq!(server.focus_window, main_window);
        assert_active_window(&server, main_window);
        assert_eq!(server.map_state(main_window), 2);
        assert!(server.wm_active_frame_window(main_frame));
        assert!(!server.wm_active_frame_window(unmap_frame));

        server
            .feed(
                client,
                &request(order, 8, 0, &destroy_dialog.0.to_le_bytes()),
            )
            .unwrap();
        let _ = drain_all(&mut server, client);
        let destroy_frame = server.managed_frames[&destroy_dialog.0];
        assert_eq!(server.focus_window, destroy_dialog);
        assert!(server.wm_active_frame_window(destroy_frame));

        server
            .feed(
                client,
                &request(order, 4, 0, &destroy_dialog.0.to_le_bytes()),
            )
            .unwrap();
        let destroy_events = drain_all(&mut server, client);
        assert_eq!(
            focus_events(&destroy_events),
            vec![
                (10, NOTIFY_NONLINEAR, destroy_dialog.0),
                (9, NOTIFY_NONLINEAR, main_window.0),
            ],
        );
        assert_eq!(server.focus_window, main_window);
        assert_active_window(&server, main_window);
        assert!(server.wm_active_frame_window(main_frame));
        assert!(!server.windows.contains_key(&destroy_dialog.0));
        assert!(!server.windows.contains_key(&destroy_frame.0));
    }

    #[test]
    fn native_wm_nonmodal_transient_keeps_owner_interactive_and_grouped() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let owner = ResourceId(0x0020_0190);
        let help = ResourceId(0x0020_0191);
        let owner_child = ResourceId(0x0020_0192);
        for window in [owner, help] {
            server
                .feed(
                    client,
                    &create_window_request(
                        order,
                        window.0,
                        STRUCTURE_NOTIFY_MASK | FOCUS_CHANGE_MASK,
                    ),
                )
                .unwrap();
        }
        server.set_wm_property(
            help,
            "WM_TRANSIENT_FOR",
            "WINDOW",
            32,
            owner.0.to_le_bytes().to_vec(),
        );
        server
            .feed(
                client,
                &create_child_window_request(
                    order,
                    owner_child.0,
                    owner.0,
                    20,
                    24,
                    80,
                    24,
                    BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK,
                ),
            )
            .unwrap();
        for window in [owner, owner_child, help] {
            server
                .feed(client, &request(order, 8, 0, &window.0.to_le_bytes()))
                .unwrap();
        }
        let _ = drain_all(&mut server, client);

        assert_eq!(server.focus_window, help);
        assert_eq!(server.topmost_managed_transient(owner), Some(help));
        assert_eq!(server.topmost_managed_modal_transient(owner), None);
        assert_eq!(
            server.wm_task_targets(),
            vec![WmTaskTarget::X11(owner)],
            "a nonmodal owned window remains structurally grouped",
        );
        let help_frame = server.managed_frames[&help.0];
        if let Some(frame) = server.windows.get_mut(&help_frame.0) {
            frame.x = 400;
            frame.y = 100;
        }

        let (child_x, child_y) = server.window_origin(owner_child);
        server.inject_pointer_button(true, 1, clamp_i16(child_x + 20), clamp_i16(child_y + 10), 0);
        server.inject_pointer_button(
            false,
            1,
            clamp_i16(child_x + 20),
            clamp_i16(child_y + 10),
            0x0100,
        );
        let events = drain_all(&mut server, client);
        assert!(events.chunks_exact(32).any(|event| {
            event[0] & 0x7f == 4
                && u32::from_le_bytes(event[12..16].try_into().unwrap()) == owner_child.0
        }));
        assert!(events.chunks_exact(32).any(|event| {
            event[0] & 0x7f == 5
                && u32::from_le_bytes(event[12..16].try_into().unwrap()) == owner_child.0
        }));
        assert_eq!(
            server.focus_window, owner,
            "focus must move back to an interactive nonmodal owner",
        );
        assert_eq!(server.map_state(help), 2);
    }

    #[test]
    fn native_wm_transient_focus_fallback_requires_a_viewable_unparked_owner() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let owner = ResourceId(0x0020_0185);
        let dialog = ResourceId(0x0020_0186);
        for window in [owner, dialog] {
            server
                .feed(client, &create_window_request(order, window.0, 0))
                .unwrap();
        }
        server.set_wm_property(
            dialog,
            "WM_TRANSIENT_FOR",
            "WINDOW",
            32,
            owner.0.to_le_bytes().to_vec(),
        );
        server
            .feed(client, &request(order, 8, 0, &dialog.0.to_le_bytes()))
            .unwrap();
        assert_eq!(
            server.focus_fallback_for_withdrawn_window(dialog),
            ROOT_WINDOW,
            "an unmanaged/unmapped owner cannot receive focus",
        );

        server
            .feed(client, &request(order, 8, 0, &owner.0.to_le_bytes()))
            .unwrap();
        assert_eq!(server.focus_fallback_for_withdrawn_window(dialog), owner);

        server.parked_managed_windows.insert(owner.0);
        assert_eq!(
            server.focus_fallback_for_withdrawn_window(dialog),
            ROOT_WINDOW,
            "a parked owner cannot receive focus",
        );
        server.parked_managed_windows.remove(&owner.0);

        let owner_frame = server.managed_frames[&owner.0];
        server.windows.get_mut(&owner_frame.0).unwrap().mapped = false;
        assert_eq!(
            server.focus_fallback_for_withdrawn_window(dialog),
            ROOT_WINDOW,
            "a non-viewable owner cannot receive focus",
        );
    }

    #[test]
    fn native_wm_does_not_focus_override_redirect_popups() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        server
            .feed(client, &setup_request(order, &[], &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let main_window = 0x0020_0084;
        server
            .feed(
                client,
                &create_window_request(order, main_window, KEY_PRESS_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &main_window.to_le_bytes()))
            .unwrap();
        assert_eq!(server.focus_window, ResourceId(main_window));

        let popup = 0x0020_0085;
        server
            .feed(client, &create_window_request(order, popup, KEY_PRESS_MASK))
            .unwrap();
        server
            .windows
            .get_mut(&popup)
            .expect("popup exists")
            .override_redirect = true;
        server
            .feed(client, &request(order, 8, 0, &popup.to_le_bytes()))
            .unwrap();

        assert_eq!(server.focus_window, ResourceId(main_window));
        assert_eq!(server.map_state(ResourceId(popup)), 2);
    }

    #[test]
    fn desktop_visibility_is_idempotent_and_workspace_scoped() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig { native_window_manager: true, ..DisplayConfig::default() });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        for (id, workspace) in [(0x0020_0184_u32, 0), (0x0020_0185, 0), (0x0020_0186, 1)] {
            server.switch_workspace(workspace);
            server.feed(owner, &create_window_request(order, id, STRUCTURE_NOTIFY_MASK),
                ).unwrap();
            server.feed(owner, &request(order, 8, 0, &id.to_le_bytes())).unwrap();
        }
        let a = 0x0020_0184; let b = 0x0020_0185; let c = 0x0020_0186;
        server.switch_workspace(0);
        server.minimized_windows.insert(b);
        server.set_desktop_hidden(true);
        server.set_desktop_hidden(true);
        assert!(server.minimized_windows.contains(&a));
        server.switch_workspace(1);
        server.set_desktop_hidden(true);
        assert!(server.minimized_windows.contains(&c));
        server.switch_workspace(0);
        server.set_desktop_hidden(false);
        assert!(!server.minimized_windows.contains(&a));
        assert!(server.minimized_windows.contains(&b), "preexisting minimized windows stay minimized");
        assert!(server.minimized_windows.contains(&c), "other workspace remains hidden");
        server.restore_workspace_windows();
        assert!(!server.minimized_windows.contains(&b));
        assert!(server.minimized_windows.contains(&c));
        server.switch_workspace(1);
        server.set_desktop_hidden(false);
        assert!(!server.minimized_windows.contains(&c));
        server.switch_workspace(2);
        server.toggle_show_desktop();
        assert!(server.wm_show_desktop_windows.contains_key(&2));
        server.set_desktop_hidden(true); // provider follows an already applied chrome action
        server.toggle_show_desktop();
        assert!(!server.wm_show_desktop_windows.contains_key(&2));
    }

    #[test]
    fn overview_moves_across_outputs_and_workspaces_without_switching() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig { width_px: 1600, height_px: 700, native_window_manager: true, ..DisplayConfig::default() });
        assert!(server.set_output_layout(1600, 700, vec![(0,0,800,700),(800,0,800,700)]));
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0184);
        server.feed(owner, &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK)).unwrap();
        server.feed(owner, &request(order, 8, 0, &client.0.to_le_bytes())).unwrap();
        let frame = server.managed_frames[&client.0];
        assert!(server.move_overview_window(frame.0, 0, 900, 100));
        assert_eq!(server.windows[&frame.0].x, 900);
        assert!(server.windows[&frame.0].mapped);
        server.begin_auto_tile(true);
        server.apply_auto_tile();
        assert_eq!(server.windows[&frame.0].x, 900, "explicit display placement survives automatic tiling");
        assert!(server.move_overview_window(frame.0, 2, 950, 120));
        assert_eq!(server.current_workspace, 0);
        assert_eq!(server.window_workspaces[&client.0], 2);
        assert!(!server.windows[&frame.0].mapped);
        assert!(!server.move_overview_window(frame.0, 4, 0, 0));
        assert!(!server.move_overview_window(0, 0, 0, 0));
        assert!(!server.move_overview_window(frame.0, 0, i32::MAX, 0));
        assert_eq!(server.window_workspaces[&client.0], 2);
        server.switch_workspace(2);
        assert!(server.windows[&frame.0].mapped);
        assert_eq!(server.windows[&frame.0].x, 950);
    }

#[test]
fn spatial_snapshot_has_exact_frame_client_connection_and_bounded_transient_relation() {
 let order=ClientByteOrder::LittleEndian;
 let mut server=DisplayServer::new(DisplayConfig{native_window_manager:true,..DisplayConfig::default()});
 let owner=server.open_client().unwrap();server.feed(owner,&setup_request(order,&[],&[])).unwrap();
 let foreign=server.open_client().unwrap();server.feed(foreign,&setup_request(order,&[],&[])).unwrap();
 let main=ResourceId(0x0020_0610);let dialog=ResourceId(0x0020_0611);let other=ResourceId(0x0040_0610);
 for (connection,id) in [(owner,main),(owner,dialog),(foreign,other)] {
  server.feed(connection,&create_window_request(order,id.0,STRUCTURE_NOTIFY_MASK)).unwrap();
  server.feed(connection,&request(order,8,0,&id.0.to_le_bytes())).unwrap();
 }
 server.set_wm_property(dialog,"WM_TRANSIENT_FOR","WINDOW",32,main.0.to_le_bytes().to_vec());
 let rows=|s:&DisplayServer| {let b=s.spatial_window_snapshot().unwrap();assert_eq!(&b[..4],&1u32.to_le_bytes());b[8..].chunks_exact(24).map(|row|{let mut words=[0u32;6];for(i,word)in row.chunks_exact(4).enumerate(){words[i]=u32::from_le_bytes(word.try_into().unwrap());}words}).collect::<Vec<_>>()};
 let r=rows(&server);let d=r.iter().find(|r|r[0]==dialog.0).unwrap();
 assert_eq!(d[1],server.managed_frames[&dialog.0].0);assert_eq!(d[2],owner.0);assert_eq!(d[3],main.0);assert_eq!(d[5]&2,2);
 server.set_wm_property(dialog,"WM_TRANSIENT_FOR","WINDOW",32,other.0.to_le_bytes().to_vec());
 assert_eq!(rows(&server).iter().find(|r|r[0]==dialog.0).unwrap()[3],0,"foreign connection cannot become spatial parent");
 server.set_wm_property(dialog,"WM_TRANSIENT_FOR","WINDOW",32,main.0.to_le_bytes().to_vec());
 server.set_wm_property(main,"WM_TRANSIENT_FOR","WINDOW",32,dialog.0.to_le_bytes().to_vec());
 assert!(rows(&server).iter().filter(|r|r[2]==owner.0).all(|r|r[3]==0),"cycle has no composition authority");
 server.close_client(owner);assert!(rows(&server).iter().all(|r|r[2]!=owner.0));
}
