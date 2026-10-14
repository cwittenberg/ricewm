use super::*;


    #[test]
    fn managed_right_edge_drag_resizes_live_before_button_release() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1200,
            height_px: 800,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0186);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, owner);

        let frame_id = server.managed_frames[&client.0];
        let before_frame = server.windows[&frame_id.0].clone();
        let before_client = server.windows[&client.0].clone();
        let _ = drain_scene(&mut server);
        server.scene_upsert(frame_id);
        let frame_scene = drain_scene(&mut server);
        let frame_command = frame_scene
            .chunks_exact(SCENE_COMMAND_BYTES)
            .find(|command| {
                command[0] == SCENE_UPSERT_WINDOW
                    && u32::from_le_bytes(command[4..8].try_into().unwrap()) == frame_id.0
            })
            .expect("managed frame scene upsert");
        assert_ne!(
            u16::from_le_bytes(frame_command[26..28].try_into().unwrap())
                & SCENE_RESIZABLE_WINDOW_STACK_FLAG,
            0,
            "managed frame resize capability must reach the page cursor contract",
        );
        server.ensure_surface_materialized(client).unwrap();
        let retained_row = before_client.height / 2;
        let retained_edge = 0x0011_2233;
        if let Some(surface) = server.surfaces.get_mut(&client.0) {
            let edge_index = usize::from(retained_row) * usize::from(before_client.width)
                + usize::from(before_client.width - 1);
            surface.pixels.as_mut().unwrap()[edge_index] = retained_edge;
        }
        let edge_x = before_frame
            .x
            .saturating_add(i16::try_from(before_frame.width).unwrap())
            .saturating_sub(5);
        let edge_y = before_frame
            .y
            .saturating_add(i16::try_from(before_frame.height / 2).unwrap());
        assert_eq!(
            server.hit_test(edge_x, edge_y),
            client,
            "the acceptance point must be inside the application client"
        );
        assert_eq!(server.pointer_layer(edge_x, edge_y), 3);
        server.inject_pointer_button(true, 1, edge_x, edge_y, 0);
        assert!(
            server
                .wm_drag
                .is_some_and(|drag| { drag.resize_edges == WM_RESIZE_RIGHT && !drag.moved })
        );
        let _ = drain_scene(&mut server);

        let held_x = edge_x.saturating_add(80);
        server.inject_pointer_motion(held_x, edge_y, 0x0100);
        let held_frame = server.windows[&frame_id.0].clone();
        let held_client = server.windows[&client.0].clone();
        assert_eq!(held_frame.x, before_frame.x);
        assert_eq!(held_frame.y, before_frame.y);
        assert_eq!(held_frame.width, before_frame.width + 80);
        assert_eq!(held_frame.height, before_frame.height);
        assert_eq!(held_client.width, before_client.width + 80);
        assert_eq!(held_client.height, before_client.height);
        let held_surface = &server.surfaces[&client.0];
        let exposed_index = usize::from(retained_row) * usize::from(held_client.width)
            + usize::from(before_client.width + 40);
        assert_eq!(
            held_surface.pixels.as_ref().unwrap()[exposed_index],
            retained_edge,
            "new managed-client columns must retain the application's edge pixels",
        );
        assert!(
            server.wm_drag.is_some_and(|drag| drag.moved),
            "resize must remain captured while the physical button is held"
        );

        let live_scene = drain_scene(&mut server);
        assert!(
            live_scene.chunks_exact(SCENE_COMMAND_BYTES).any(|command| {
                command[0] == SCENE_UPSERT_WINDOW
                    && u32::from_le_bytes(command[4..8].try_into().unwrap()) == frame_id.0
                    && u16::from_le_bytes(command[20..22].try_into().unwrap()) == held_frame.width
                    && u16::from_le_bytes(command[22..24].try_into().unwrap()) == held_frame.height
            }),
            "held edge motion must publish the resized managed frame immediately"
        );
        let configure = drain_all(&mut server, owner);
        assert!(
            configure.chunks_exact(32).any(|event| {
                event[0] & 0x7f == 22
                    && u32::from_le_bytes(event[8..12].try_into().unwrap()) == client.0
                    && u16::from_le_bytes(event[20..22].try_into().unwrap()) == held_client.width
                    && u16::from_le_bytes(event[22..24].try_into().unwrap()) == held_client.height
            }),
            "live edge resize must send the client ConfigureNotify before release"
        );

        server.inject_pointer_button(false, 1, held_x, edge_y, 0x0100);
        assert!(server.wm_drag.is_none());
        assert_eq!(server.windows[&frame_id.0].width, held_frame.width);
        assert_eq!(server.windows[&client.0].width, held_client.width);

        assert!(server.set_managed_window_resizable(client.0, false));
        let fixed_edge_x = server.windows[&frame_id.0]
            .x
            .saturating_add(i16::try_from(server.windows[&frame_id.0].width).unwrap())
            .saturating_sub(1);
        server.inject_pointer_button(true, 1, fixed_edge_x, edge_y, 0);
        server.inject_pointer_motion(fixed_edge_x.saturating_add(40), edge_y, 0x0100);
        assert_eq!(
            server.windows[&frame_id.0].width, held_frame.width,
            "fixed managed windows must reject compositor edge resize"
        );
        server.inject_pointer_button(false, 1, fixed_edge_x.saturating_add(40), edge_y, 0x0100);
    }

    #[test]
    fn ewmh_interactive_move_drags_an_undecorated_managed_window() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1200,
            height_px: 800,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0187);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, owner);
        let frame = server.managed_frames[&client.0];
        let before = server.windows[&frame.0].clone();

        server.set_wm_property(
            client,
            "_MOTIF_WM_HINTS",
            "_MOTIF_WM_HINTS",
            32,
            [2u32, 0, 0, 0, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        );
        server.refresh_managed_decorations(client);
        assert_eq!(server.managed_frame_insets(client), (0, 0, 0, 0));

        server.set_wm_property(client,"_NET_WM_NAME","UTF8_STRING",8,b"Client-side document title".to_vec(),
        );
        server.paint_wm_chrome(frame);
        let title=server.wm_text_overlays.iter().find(|row|row.window==frame&&row.align==3).unwrap();
        assert_eq!(title.text,b"Client-side document title");
        assert_eq!((title.width,title.height),(0,0));
        assert!(server.wm_text_overlays.iter().filter(|row|row.window==frame).all(|row|row.align==3),"undecorated titles must not add painted chrome");

        let root_x = before.x.saturating_add(600);
        let root_y = before.y.saturating_add(18);
        let moveresize = server.atoms.intern("_NET_WM_MOVERESIZE", false);
        let mut event = [0u8; 32];
        event[0] = 33;
        event[1] = 32;
        event[4..8].copy_from_slice(&client.0.to_le_bytes());
        event[8..12].copy_from_slice(&moveresize.0.to_le_bytes());
        event[12..16].copy_from_slice(&u32::try_from(root_x).unwrap().to_le_bytes());
        event[16..20].copy_from_slice(&u32::try_from(root_y).unwrap().to_le_bytes());
        event[20..24].copy_from_slice(&8u32.to_le_bytes());
        event[24..28].copy_from_slice(&1u32.to_le_bytes());
        assert!(server.handle_ewmh_client_message(&event));
        assert!(server.wm_drag.is_some_and(|drag| {
            drag.client == client && drag.frame == frame && drag.resize_edges == 0
        }));

        let mut unrelated_cancel=event;
        unrelated_cancel[4..8].copy_from_slice(&0u32.to_le_bytes());
        unrelated_cancel[20..24].copy_from_slice(&11u32.to_le_bytes());
        assert!(server.handle_ewmh_client_message(&unrelated_cancel));
        assert!(server.wm_drag.is_some(),"an unrelated cancel cannot stop this client's drag");

        server.inject_pointer_motion(root_x + 75, root_y + 30, 0x0100);
        let moved = &server.windows[&frame.0];
        assert_eq!(moved.x, before.x + 75);
        assert_eq!(moved.y, before.y + 30);
        server.inject_pointer_button(false, 1, root_x + 75, root_y + 30, 0x0100);
        assert!(server.wm_drag.is_none());

        // The same client-side request must hand off without moving a tile.
        server.auto_tile_enabled=true;
        server.apply_auto_tile();
        let tiled=server.windows[&frame.0].clone();
        assert!(server.handle_ewmh_client_message(&event));
        assert!(server.wm_drag.is_none());
        assert_eq!(server.tile_move_request_field(0),client.0);
        assert_eq!(server.tile_move_request_field(1),root_x as u32);
        assert_eq!(server.tile_move_request_field(3),0);
        server.inject_pointer_motion(root_x+90,root_y+40,0x0100);
        assert_eq!((server.windows[&frame.0].x,server.windows[&frame.0].y),(tiled.x,tiled.y));
        server.clear_tile_move_request();
        assert_eq!(server.tile_move_request_field(0),0);
        server.input_state=0x0105;
        let _=drain_all(&mut server,owner);
        server.release_tile_pointer(250,350,0x0105);
        assert_eq!(server.input_state,5);
        assert_eq!((server.pointer_x,server.pointer_y),(250,350));
        assert!(drain_all(&mut server,owner).is_empty(),"consumed drops must not emit guest releases");
        event[20..24].copy_from_slice(&11u32.to_le_bytes());
        assert!(server.handle_ewmh_client_message(&event));
        assert_eq!(server.tile_move_request_field(0),client.0);
        assert_eq!(server.tile_move_request_field(3),1);
        server.clear_tile_move_request();
        event[20..24].copy_from_slice(&3u32.to_le_bytes());
        assert!(server.handle_ewmh_client_message(&event));
        assert!(server.wm_drag.is_none(),"client edge resize cannot override tiled geometry");
        assert_eq!(server.tile_move_request_field(0),0);
    }

    #[test]
    fn desktop_output_layout_bounds_native_maximize_and_fullscreen() {
        let order=ClientByteOrder::LittleEndian;
        let mut server=DisplayServer::new(DisplayConfig {width_px:1600,height_px:900,native_window_manager:true,..DisplayConfig::default()});
        let owner=server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client=ResourceId(0x0020_0199);
        server.feed(owner,&create_window_request(order,client.0,STRUCTURE_NOTIFY_MASK|EXPOSURE_MASK),
            ).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame=server.managed_frames[&client.0];
        assert!(server.set_output_layout(1600,900,vec![(0,0,800,900),(800,100,800,600)]));
        server.apply_managed_geometry(client,frame,850,150,500,400);
        let bounds=|server:&DisplayServer|{let w=&server.windows[&frame.0];(w.x,w.y,w.width,w.height)};
        let original=bounds(&server);
        server.toggle_maximized(client,1);
        assert_eq!(bounds(&server),(800,100,800,600));
        server.toggle_fullscreen(client,1);
        assert_eq!(bounds(&server),(800,100,800,600));
        assert!(server.set_output_layout(1600,900,vec![(0,0,800,900),(800,100,800,600)]));
        assert_eq!(bounds(&server),(800,100,800,600));
        server.toggle_fullscreen(client,0);
        server.toggle_maximized(client,0);
        assert_eq!(bounds(&server),original);
        assert!(server.set_output_layout(800,900,vec![(0,0,800,900)]));
        let recovered=bounds(&server);
        assert!(recovered.0 >= 0 && recovered.0 < 800 && recovered.1 >= 0 && recovered.1 < 900,
            "floating window must return from the removed monitor");
        server.toggle_fullscreen(client,1);
        assert!(server.set_output_layout(800,900,vec![(0,0,800,900)]));
        assert_eq!(bounds(&server),(0,0,800,900));
        let before=server.wm_outputs.clone();
        assert!(!server.set_output_layout(800,900,vec![(750,0,800,900)]));
        assert_eq!(server.wm_outputs,before,"invalid topology must not change accepted outputs");
    }

    #[test]
    fn output_primary_panel_and_taskbar_follow_offset_display() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 3200, height_px: 1280, native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.wm_gnome_panel = true;
        assert!(server.set_output_layout(3200, 1280, vec![(1280,200,1920,1080),(0,0,1280,800)]));
        let panel = &server.windows[&WM_GNOME_PANEL.0];
        assert_eq!((panel.x,panel.y,panel.width),(1280,200,1920));
        assert!(server.update_gnome_menu(vec![WmGnomeMenuItem {
            index: 0, separator:false, reactive:true, toggle:false, icon_kind:0,
            image:false, checked:false, label:b"Settings".to_vec(),
        }]));
        let menu=&server.windows[&WM_GNOME_MENU.0];
        assert!(menu.x >= 1280 && menu.y >= 200 && i32::from(menu.x)+i32::from(menu.width) <= 3200);
        server.relayout_wm_menu();
        let menu=&server.windows[&WM_MENU.0];
        assert!(menu.x >= 1280 && menu.y >= 200 && i32::from(menu.y)+i32::from(menu.height) <= 1280);

        let (x,y,w,h) = server.wm_taskbar_rect();
        assert_eq!((x,w),(1280,1920)); assert_eq!(i32::from(y)+i32::from(h),1280);
        assert!(server.set_output_layout(3200,1280,vec![(0,0,1280,800),(1280,200,1920,1080)]));
        let panel = &server.windows[&WM_GNOME_PANEL.0];
        assert_eq!((panel.x,panel.y,panel.width),(0,0,1280));
        let (x,y,w,h) = server.wm_taskbar_rect();
        assert_eq!((x,w),(0,1280));assert_eq!(i32::from(y)+i32::from(h),800);
    }

    #[test]
    fn native_wm_drag_previews_and_snaps_to_left_right_and_top_edges() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0186);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, owner);

        let frame_id = server.managed_frames[&client.0];
        let original_frame = server.windows[&frame_id.0].clone();
        let original_client = server.windows[&client.0].clone();
        let original_geometry = (
            original_frame.x,
            original_frame.y,
            original_client.width,
            original_client.height,
        );
        assert_eq!(
            server.pointer_layer(
                original_frame.x.saturating_add(100),
                original_frame.y.saturating_add(12),
            ),
            3,
            "compositor-owned managed titlebars must retain managed-frame ownership",
        );
        assert_eq!(
            server.pointer_layer(
                original_frame.x.saturating_add(100),
                original_frame
                    .y
                    .saturating_add(i16::try_from(WM_TITLE_HEIGHT + 8).unwrap()),
            ),
            1,
            "the managed client's content remains ordinary X11 input",
        );

        server.inject_pointer_button(
            true,
            1,
            original_frame.x.saturating_add(100),
            original_frame.y.saturating_add(12),
            0,
        );
        let _ = drain_scene(&mut server);
        server.inject_pointer_motion(0, 300, 0x0100);
        let moving_frame = server.windows[&frame_id.0].clone();
        let live_scene = drain_scene(&mut server);
        assert!(
            live_scene.chunks_exact(SCENE_COMMAND_BYTES).any(|command| {
                command[0] == SCENE_UPSERT_WINDOW
                    && u32::from_le_bytes(command[4..8].try_into().unwrap()) == frame_id.0
                    && i32::from_le_bytes(command[12..16].try_into().unwrap())
                        == i32::from(moving_frame.x)
                    && i32::from_le_bytes(command[16..20].try_into().unwrap())
                        == i32::from(moving_frame.y)
            }),
            "managed motion must publish the retained frame before button release"
        );
        let preview = &server.windows[&WM_SNAP_PREVIEW.0];
        assert!(preview.mapped);
        assert_eq!((preview.x, preview.y, preview.width), (0, 0, 400));
        assert_eq!(preview.height, 600 - WM_TASKBAR_HEIGHT);
        server.inject_pointer_button(false, 1, 0, 300, 0x0100);
        let left = &server.windows[&frame_id.0];
        assert_eq!((left.x, left.y, left.width), (0, 0, 400));
        assert!(!server.windows[&WM_SNAP_PREVIEW.0].mapped);
        assert_eq!(
            server.snapped_geometries[&client.0],
            WmSnapState {
                target: WmSnapTarget::Left,
                restore_geometry: original_geometry,
            },
        );

        server.inject_pointer_button(true, 1, 100, 12, 0);
        server.inject_pointer_motion(799, 300, 0x0100);
        let preview = &server.windows[&WM_SNAP_PREVIEW.0];
        assert_eq!((preview.x, preview.width), (400, 400));
        server.inject_pointer_button(false, 1, 799, 300, 0x0100);
        let right = &server.windows[&frame_id.0];
        assert_eq!((right.x, right.y, right.width), (400, 0, 400));
        assert_eq!(
            server.snapped_geometries[&client.0],
            WmSnapState {
                target: WmSnapTarget::Right,
                restore_geometry: original_geometry,
            },
        );

        server.inject_pointer_button(true, 1, 500, 12, 0);
        server.inject_pointer_motion(500, 0, 0x0100);
        let preview = &server.windows[&WM_SNAP_PREVIEW.0];
        assert_eq!((preview.x, preview.y, preview.width), (0, 0, 800));
        server.inject_pointer_button(false, 1, 500, 0, 0x0100);
        let maximized = &server.windows[&frame_id.0];
        assert_eq!((maximized.x, maximized.y, maximized.width), (0, 0, 800));
        assert_eq!(server.maximized_geometries[&client.0], original_geometry);
        let state_atom = server.atoms.intern("_NET_WM_STATE", false);
        assert_eq!(server.windows[&client.0].properties[&state_atom].data.len(), 8,
            "top-edge snap must publish both maximize hints");

        assert!(server.set_taskbar_configuration(true, 1, false));
        assert_eq!(
            server.wm_snap_target_at(400, WM_TASKBAR_HEIGHT as i16),
            Some(WmSnapTarget::Maximize),
        );
        let maximized = server.windows[&frame_id.0].clone();
        assert_eq!(maximized.y, WM_TASKBAR_HEIGHT as i16);
        server.inject_pointer_button(true, 1, 500, maximized.y.saturating_add(12), 0);
        server.inject_pointer_motion(600, 140, 0x0100);
        let restored = &server.windows[&frame_id.0];
        assert!(!server.maximized_geometries.contains_key(&client.0));
        assert!(server.windows[&client.0].properties[&state_atom].data.is_empty(),
            "dragging out of maximize must clear the toolkit state");
        assert_eq!(
            restored.width,
            original_client.width.saturating_add(WM_BORDER_WIDTH * 2),
        );
        assert!(restored.x <= 600 && 600 < restored.x.saturating_add_unsigned(restored.width));
        assert!(restored.y <= 140 && 140 < restored.y.saturating_add(WM_TITLE_HEIGHT as i16));
        server.inject_pointer_button(false, 1, 600, 140, 0x0100);
    }

    #[test]
    fn native_wm_alt_shift_arrows_and_alt_page_keys_manage_the_active_window() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let owner = server.open_client().unwrap();
        server.feed(owner, &setup_request(order, &[], &[])).unwrap();
        let _ = drain_all(&mut server, owner);
        let client = ResourceId(0x0020_0187);
        server
            .feed(
                owner,
                &create_window_request(order, client.0, STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK),
            )
            .unwrap();
        server
            .feed(owner, &request(order, 8, 0, &client.0.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, owner);

        let frame = server.managed_frames[&client.0];
        let original_frame = server.windows[&frame.0].clone();
        let original_client = server.windows[&client.0].clone();
        let original_geometry = (
            original_frame.x,
            original_frame.y,
            original_client.width,
            original_client.height,
        );

        server.inject_key(true, 113, 0x0008);
        let unchanged = &server.windows[&frame.0];
        assert_eq!(
            (unchanged.x, unchanged.y, unchanged.width),
            (original_frame.x, original_frame.y, original_frame.width),
        );

        server.inject_key(true, 113, 0x0009);
        let left = &server.windows[&frame.0];
        assert_eq!((left.x, left.y, left.width), (0, 0, 400));
        assert_eq!(
            server.snapped_geometries[&client.0],
            WmSnapState {
                target: WmSnapTarget::Left,
                restore_geometry: original_geometry,
            }
        );

        server.inject_key(true, 114, 0x0009);
        let right = &server.windows[&frame.0];
        assert_eq!((right.x, right.y, right.width), (400, 0, 400));
        assert_eq!(
            server.snapped_geometries[&client.0].restore_geometry,
            original_geometry,
        );

        server.inject_key(true, 111, 0x0009);
        let maximized = &server.windows[&frame.0];
        assert_eq!((maximized.x, maximized.y, maximized.width), (0, 0, 800));
        assert_eq!(server.maximized_geometries[&client.0], original_geometry);

        server.inject_key(true, 117, 0x0008);
        assert_eq!(server.window_workspaces[&client.0], 1);
        assert!(!server.windows[&frame.0].mapped);
        assert_eq!(server.focus_window, ROOT_WINDOW);

        server.switch_workspace(1);
        server.activate_managed_window(client);
        while server.drain_wm_action().is_some() {}
        server.inject_key(true, 112, 0x0008);
        assert_eq!(server.window_workspaces[&client.0], 0);
        assert!(!server.windows[&frame.0].mapped);
        assert_eq!(server.focus_window, ROOT_WINDOW);

        server.switch_workspace(0);
        while server.drain_wm_action().is_some() {}

        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: 17,
            workspace: 0,
            icon: 1,
            minimized: false,
            maximized: false,
            active: true,
            title: b"RiceFiles".to_vec(),
        }]));
        server.inject_key(true, 113, 0x0009);
        server.inject_key(true, 114, 0x0009);
        server.inject_key(true, 111, 0x0009);
        server.inject_key(true, 117, 0x0008);
        server.inject_key(true, 112, 0x0008);
        assert_eq!(
            server.wm_actions.into_iter().collect::<Vec<_>>(),
            vec![
                WM_ACTION_TASK_SNAP_LEFT_PREFIX | 17,
                WM_ACTION_TASK_SNAP_RIGHT_PREFIX | 17,
                WM_ACTION_TASK_MAXIMIZE_PREFIX | 17,
                WM_ACTION_TASK_WORKSPACE_NEXT_PREFIX | 17,
                WM_ACTION_TASK_WORKSPACE_PREVIOUS_PREFIX | 17,
            ]
        );
    }
