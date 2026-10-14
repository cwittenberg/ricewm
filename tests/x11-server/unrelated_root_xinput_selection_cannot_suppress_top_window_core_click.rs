use super::*;


    #[test]
    fn unrelated_root_xinput_selection_cannot_suppress_top_window_core_click() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig::default());
        let target = server.open_client().unwrap();
        let observer = server.open_client().unwrap();
        for client in [target, observer] {
            server.feed(client, &setup_request(order, &[], &[])).unwrap();
            let _ = drain_all(&mut server, client);
        }
        let target_window = server.clients[&target.0].resource_id_base() | 1;
        server.feed(target, &create_window_request(order, target_window, 0)).unwrap();
        server.feed(target, &request(order, 8, 0, &target_window.to_le_bytes())).unwrap();
        let _ = drain_all(&mut server, target);
        server.clients.get_mut(&target.0).unwrap().event_masks.insert(
            ROOT_WINDOW.0, BUTTON_PRESS_MASK | BUTTON_RELEASE_MASK);
        server.clients.get_mut(&observer.0).unwrap().xi2_masks.insert(
            (ROOT_WINDOW.0, 2), (1 << 4) | (1 << 5));

        server.inject_pointer_button(true, 1, 20, 30, 0);
        let target_event = drain_all(&mut server, target);
        let observer_event = drain_all(&mut server, observer);
        assert_eq!(target_event.len(), 32);
        assert_eq!(target_event[0], 4, "the top-window owner needs its core press");
        assert_eq!(u32::from_le_bytes(target_event[12..16].try_into().unwrap()), ROOT_WINDOW.0);
        assert_eq!(u32::from_le_bytes(target_event[16..20].try_into().unwrap()), target_window);
        assert_eq!(observer_event[0] & 0x7f, 35, "the XI2 observer still receives input");
    }

    #[test]
    fn unmapping_a_keyboard_grab_window_releases_the_grab() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0002;
        server
            .feed(
                client,
                &create_window_request(order, window, KEY_PRESS_MASK | KEY_RELEASE_MASK),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let mut grab = Vec::new();
        push_u32(order, &mut grab, window);
        push_u32(order, &mut grab, 0);
        grab.extend_from_slice(&[1, 1, 0, 0]);
        server.feed(client, &request(order, 31, 1, &grab)).unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!(reply[0], 1);
        assert_eq!(reply[1], 0);
        assert_eq!(
            server.keyboard_grab.map(|active| active.window),
            Some(ResourceId(window)),
        );

        server
            .feed(client, &request(order, 10, 0, &window.to_le_bytes()))
            .unwrap();
        assert!(server.keyboard_grab.is_none());
    }

    #[test]
    fn default_keyboard_mapping_covers_paths_and_desktop_navigation() {
        assert_eq!(default_keysyms(61), [u32::from(b'/'), u32::from(b'?')]);
        assert_eq!(default_keysyms(60), [u32::from(b'.'), u32::from(b'>')]);
        assert_eq!(default_keysyms(36), [0xff0d, 0xff0d]);
        assert_eq!(default_keysyms(50), [0xffe1, 0xffe1]);
        assert_eq!(default_keysyms(110), [0xff50, 0xff50]);
        assert_eq!(default_keysyms(119), [0xffff, 0xffff]);
    }

    #[test]
    fn native_wm_menu_cascades_on_hover_closes_away_and_keeps_fixed_launchers() {
        assert_eq!(
            contrasting_text(0x0000_0080),
            0x00ff_ffff,
            "blue theme accents require white controls and labels",
        );
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let mut palette = built_in_wm_themes().remove(0);
        palette.menu[3] = 0x00ff_cc66;
        palette.menu[4] = 0x00ff_cc66;
        assert!(server.replace_wm_themes(vec![(
            b"Command Deck".to_vec(),
            false,
            false,
            WmChromeProfile::CommandDeck,
            palette.taskbar,
            palette.menu,
            palette.frame,
        )]));
        assert!(server.replace_wm_menu_entries(vec![(41, 1, 4, b"DYNAMIC IMAGE APP".to_vec(),)]));

        assert!(!server.wm_session_logout, "web and direct sessions keep their existing exit mode");
        assert!(server.set_session_logout(true));
        server.inject_pointer_button(true, 1, 18, 584, 0);
        assert!(server.wm_text_overlays.iter().any(|overlay|
            overlay.window == WM_MENU && overlay.text.as_slice() == b"Log out..."
        ));
        assert!(
            server
                .windows
                .get(&WM_MENU.0)
                .is_some_and(|menu| menu.mapped)
        );
        let root_menu = server.windows.get(&WM_MENU.0).unwrap().clone();
        let root_column_width = server.wm_menu_column_widths()[0];
        let root_y = root_menu.y.saturating_add(
            i16::try_from(server.wm_menu_column_offsets()[0] + WM_MENU_ROW_HEIGHT / 2).unwrap(),
        );
        server.inject_pointer_motion(18, root_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Categories);
        assert_eq!(server.wm_menu_column_widths()[0], root_column_width);
        let category_y = server.windows[&WM_MENU.0].y.saturating_add(
            i16::try_from(server.wm_menu_column_offsets()[1] + WM_MENU_ROW_HEIGHT / 2).unwrap(),
        );
        server.inject_pointer_motion(250, category_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Applications(4));
        assert!(
            server.wm_text_overlays.iter().any(|overlay| {
                overlay.window == WM_MENU
                    && overlay.text == b"Graphics"
                    && overlay.foreground == 0x0000_0000
                    && overlay.background == 0xfeff_cc66
            }),
            "bright Command Deck menu selections use dark semantic ink"
        );
        let expanded_menu = server.windows.get(&WM_MENU.0).unwrap();
        assert_eq!(expanded_menu.y, root_menu.y);
        assert_eq!(expanded_menu.height, root_menu.height);
        assert_eq!(server.wm_menu_column_widths()[0], root_column_width);
        let offsets = server.wm_menu_column_offsets();
        let widths = server.wm_menu_column_widths();
        let x_offsets = server.wm_menu_column_x_offsets();
        assert_eq!(offsets[0], offsets[1]);
        assert_eq!(offsets[1], offsets[2]);
        let menu_surface = server.surfaces.get(&WM_MENU.0).unwrap();
        assert_eq!(menu_surface.depth, 32);
        assert_eq!(
            menu_surface.pixel(
                u16::try_from(x_offsets[1] + widths[1] - 15).unwrap(),
                u16::try_from(category_y.saturating_sub(root_menu.y)).unwrap(),
            ) & 0x00ff_ffff,
            0x00ff_cc66,
            "Command Deck parent pills stay uninterrupted at the cascade handoff",
        );
        assert_eq!(
            menu_surface.pixel(
                u16::try_from(x_offsets[1] + widths[1] - 1).unwrap(),
                u16::try_from(offsets[1]).unwrap(),
            ) & 0x00ff_ffff,
            0,
            "each Command Deck cascade card keeps its rounded right cap",
        );
        assert_eq!(
            menu_surface.pixel(
                u16::try_from(x_offsets[1] + widths[1]).unwrap(),
                u16::try_from(offsets[1] + 20).unwrap(),
            ),
            0,
            "Command Deck cascade cards retain a clean transparent gutter",
        );
        assert_eq!(
            menu_surface.pixel(
                u16::try_from(x_offsets[2] + widths[2] - 1).unwrap(),
                u16::try_from(offsets[2]).unwrap(),
            ) & 0x00ff_ffff,
            0,
            "the outer leaf keeps its rounded right cap",
        );
        assert_eq!(
            menu_surface.pixel(10, u16::try_from(offsets[0] + 100).unwrap()) >> 24,
            0xff,
            "the root column remains opaque through its own rows",
        );
        assert_eq!(
            menu_surface.pixel(240, u16::try_from(offsets[1] + 72).unwrap()) >> 24,
            0,
            "a short category fly-out must end after its final row",
        );
        assert_eq!(
            menu_surface.pixel(450, u16::try_from(offsets[2] + 72).unwrap()) >> 24,
            0,
            "a short application fly-out must end after its final row",
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .filter(|overlay| overlay.window == WM_MENU)
                .all(|overlay| !overlay.text.contains(&b'>')),
            "submenu direction is drawn as a glyph instead of text",
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .filter(|overlay| overlay.window == WM_MENU)
                .all(|overlay| overlay.text.as_slice() != b"Back"),
            "cascades close through pointer ancestry instead of Back rows",
        );

        let menu_y = server.windows.get(&WM_MENU.0).unwrap().y;
        server.inject_pointer_motion(250, menu_y.saturating_add(100), 0);
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::Categories,
            "hovering an empty part of the preceding column closes its child",
        );
        server.inject_pointer_button(true, 1, 250, menu_y.saturating_add(100), 0);
        assert!(
            server
                .windows
                .get(&WM_MENU.0)
                .is_some_and(|menu| !menu.mapped)
        );

        server.inject_pointer_button(true, 1, 125, 584, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_SHOW_DESKTOP));
        server.inject_pointer_button(true, 1, 153, 584, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_PCMANFM));

        server.inject_pointer_button(true, 1, 18, 584, 0);
        let exit_row = server
            .wm_root_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Exit)
            .unwrap();
        let exit_y = server.windows[&WM_MENU.0].y.saturating_add(
            i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + exit_row * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap(),
        );
        server.inject_pointer_button(true, 1, 18, exit_y, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_EXIT));
        assert!(!server.windows[&WM_MENU.0].mapped);
    }

    #[test]
    fn native_wm_run_menu_and_alt_r_emit_the_same_action() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let run_row = server
            .wm_root_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Run)
            .unwrap();
        server.inject_pointer_button(true, 1, 18, 584, 0);
        let run_y = server.windows[&WM_MENU.0].y.saturating_add(
            i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + run_row * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap(),
        );
        server.inject_pointer_button(true, 1, 18, run_y, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RUN));
        server.inject_key(true, 27, 0x0008);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RUN));
    }

    #[test]
    fn native_wm_applications_drag_between_categories_and_emit_persisted_move() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_wm_menu_entries(vec![
            (41, 1, 1, b"FILES".to_vec()),
            (42, 2, 4, b"TERMINAL".to_vec()),
            (43, 3, 4, b"VIEWER".to_vec()),
        ]));

        server.inject_pointer_button(true, 1, 18, 584, 0);
        let menu_y = server.windows[&WM_MENU.0].y;
        let applications_y = menu_y
            + i16::try_from(server.wm_menu_column_offsets()[0] + WM_MENU_ROW_HEIGHT / 2).unwrap();
        server.inject_pointer_motion(18, applications_y, 0);
        let category_four = server
            .wm_category_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Category(4))
            .unwrap();
        let category_four_y = menu_y
            + i16::try_from(
                server.wm_menu_column_offsets()[1]
                    + category_four * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap();
        let widths = server.wm_menu_column_widths();
        server.inject_pointer_motion(i16::try_from(widths[0] + 12).unwrap(), category_four_y, 0);

        let application_y = menu_y
            + i16::try_from(server.wm_menu_column_offsets()[2] + WM_MENU_ROW_HEIGHT / 2).unwrap();
        server.inject_pointer_button(
            true,
            1,
            i16::try_from(widths[0] + widths[1] + 12).unwrap(),
            application_y,
            0,
        );
        let category_one = server
            .wm_category_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Category(1))
            .unwrap();
        let category_one_y = menu_y
            + i16::try_from(
                server.wm_menu_column_offsets()[1]
                    + category_one * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap();
        server.inject_pointer_button(
            false,
            1,
            i16::try_from(widths[0] + 12).unwrap(),
            category_one_y,
            0,
        );

        assert_eq!(server.wm_menu_entries[1].action, 42);
        assert_eq!(server.wm_menu_entries[1].category, 1);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_MENU_MOVE_PREFIX | (1 << 12) | (1 << 8) | 1)
        );
    }

    #[test]
    fn native_wm_filesystem_menu_preserves_nested_folder_cascades() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1280,
            height_px: 720,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_wm_structured_menu_entries(vec![
            (41, 1, false, false, false, Vec::new(), b"ROOT APP".to_vec()),
            (42, 2, false, false, false, vec![b"TOOLS".to_vec()], b"TOOL".to_vec()),
            (
                43,
                3,
                false,
                false,
                false,
                vec![b"TOOLS".to_vec(), b"EDITORS".to_vec()],
                b"EDITOR".to_vec(),
            ),
        ]));
        assert_eq!(
            server.wm_root_rows(),
            vec![
                WmMenuRow::Applications,
                WmMenuRow::Themes,
                WmMenuRow::WorkspaceNavigation,
                WmMenuRow::Run,
                WmMenuRow::Exit,
            ],
            "filesystem applications belong only in the Applications cascade",
        );
        assert_eq!(
            server.wm_menu_folders,
            vec![
                WmMenuFolder {
                    parent: None,
                    label: b"TOOLS".to_vec(),
                },
                WmMenuFolder {
                    parent: Some(0),
                    label: b"EDITORS".to_vec(),
                },
            ],
        );

        server.wm_menu_level = WmMenuLevel::MenuFolder(1);
        let columns = server.wm_menu_columns();
        assert_eq!(columns.len(), 4);
        assert!(matches!(columns[1][0], WmMenuRow::Application(ref app) if app.action == 41));
        assert!(columns[1].contains(&WmMenuRow::MenuFolder(0)));
        assert!(matches!(columns[2][0], WmMenuRow::Application(ref app) if app.action == 42));
        assert!(columns[2].contains(&WmMenuRow::MenuFolder(1)));
        assert!(matches!(columns[3][0], WmMenuRow::Application(ref app) if app.action == 43));
        assert_eq!(server.wm_menu_column_widths().len(), columns.len());
        assert_eq!(server.wm_menu_column_offsets().len(), columns.len());

        server.wm_menu_level = WmMenuLevel::MenuFolders;
        server.relayout_wm_menu();
        server.windows.get_mut(&WM_MENU.0).unwrap().mapped = true;
        let widths = server.wm_menu_column_widths();
        let offsets = server.wm_menu_column_offsets();
        let menu = server.windows[&WM_MENU.0].clone();
        let app_x = menu.x + i16::try_from(widths[0] + 12).unwrap();
        let app_y = menu.y + i16::try_from(offsets[1] + WM_MENU_ROW_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 3, app_x, app_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::ApplicationContext(41));
        assert!(server.windows[&WM_MENU.0].mapped);
        let context_columns = server.wm_menu_columns();
        assert_eq!(context_columns.len(), 3);
        assert_eq!(context_columns[0], server.wm_root_rows());
        assert!(
            matches!(context_columns[1][0], WmMenuRow::Application(ref app) if app.action == 41)
        );
        assert_eq!(
            context_columns[2],
            vec![
                WmMenuRow::ApplicationLaunch,
                WmMenuRow::ApplicationOpenFolder,
            ],
        );
        let context_x_offsets = server.wm_menu_column_x_offsets();
        let context_offsets = server.wm_menu_column_offsets();
        let context_x = menu.x + i16::try_from(context_x_offsets[2] + 12).unwrap();
        let open_folder_y =
            menu.y + i16::try_from(context_offsets[2] + WM_MENU_ROW_HEIGHT + 12).unwrap();
        server.inject_pointer_button(true, 1, context_x, open_folder_y, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_MENU_OPEN_FOLDER_PREFIX | 41),
        );
    }

    #[test]
    fn native_wm_menu_background_context_opens_the_menu_directory() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1280,
            height_px: 720,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_wm_structured_menu_entries(vec![(
            41,
            1,
            false,
            false,
            false,
            vec![b"TOOLS".to_vec()],
            b"TOOL".to_vec(),
        )]));
        server.wm_menu_level = WmMenuLevel::MenuFolder(0);
        server.relayout_wm_menu();
        server.windows.get_mut(&WM_MENU.0).unwrap().mapped = true;

        let menu = server.windows[&WM_MENU.0].clone();
        let base_columns = server.wm_menu_columns();
        let background_y = menu.y + i16::try_from(menu.height.saturating_sub(12)).unwrap();
        server.inject_pointer_button(true, 3, menu.x + 12, background_y, 0);

        assert_eq!(server.wm_menu_level, WmMenuLevel::MenuDirectoryContext);
        assert!(server.windows[&WM_MENU.0].mapped);
        let context_columns = server.wm_menu_columns();
        assert_eq!(
            &context_columns[..base_columns.len()],
            base_columns.as_slice()
        );
        assert_eq!(
            context_columns.last(),
            Some(&vec![WmMenuRow::MenuOpenDirectory]),
        );

        let context_offsets = server.wm_menu_column_offsets();
        let context_column = context_columns.len() - 1;
        let context_bottom = usize::try_from(menu.y.max(0)).unwrap_or_default()
            + context_offsets[context_column]
            + WM_MENU_ROW_HEIGHT;
        assert!(
            context_bottom + 2 <= usize::from(server.wm_taskbar_y()),
            "the taskbar must not cover the context pane's bottom bevel",
        );

        let context_x_offsets = server.wm_menu_column_x_offsets();
        assert_eq!(
            context_x_offsets[context_column],
            usize::from(WM_MENU_ROOT_WIDTH)
        );
        let context_x = menu.x + i16::try_from(context_x_offsets[context_column] + 12).unwrap();
        let context_y = menu.y
            + i16::try_from(context_offsets[context_column] + WM_MENU_ROW_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 3, context_x, context_y, 0);
        assert_eq!(server.wm_menu_columns().len(), base_columns.len() + 1);
        assert_eq!(
            server.wm_menu_column_x_offsets()[context_column],
            usize::from(WM_MENU_ROOT_WIDTH)
        );
        let context_x =
            menu.x + i16::try_from(server.wm_menu_column_x_offsets()[context_column] + 12).unwrap();
        server.inject_pointer_button(true, 1, context_x, context_y, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_MENU_OPEN_DIRECTORY)
        );
    }

    #[test]
    fn multimonitor_primary_menu_and_pointer_snaps_use_output_coordinates() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 2560, height_px: 1914, native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.set_output_layout(2560,1914,vec![(321,0,1882,1060),(1280,1114,1280,800),(0,1114,1280,800)]));
        let y = server.wm_taskbar_y() as i16 + 12;
        assert_eq!(server.wm_chrome_hit(WM_TASKBAR,339,y),Some(WmChromeTarget::Start));
        server.inject_pointer_button(true,1,339,y,0);
        assert!(server.windows[&WM_MENU.0].mapped);
        server.inject_pointer_button(false,1,339,y,0x100);
        assert!(server.dismiss_wm_menu());
        assert_eq!(server.wm_snap_target_at(1280,1400),Some(WmSnapTarget::Left));
        assert_eq!(server.wm_snap_target_at(1600,1200),None);
        assert_eq!(server.wm_snap_target_at(1600,1114),Some(WmSnapTarget::Maximize));
        assert_eq!(server.wm_snap_target_at(1600,1080),None,"gaps are not snap targets");
        let area = server.wm_pointer_workarea(2559,1500).unwrap();
        assert_eq!(server.wm_snap_geometry(WmSnapTarget::Right,area),(1920,1114,640,800));
        // The same snap edge on a different output must move its preview.
        server.inject_pointer_motion(2559,1500,0);
        server.set_wm_snap_preview(Some(WmSnapTarget::Right));
        assert_eq!(server.windows[&WM_SNAP_PREVIEW.0].x,1920);
        server.inject_pointer_motion(1279,1500,0);
        server.set_wm_snap_preview(Some(WmSnapTarget::Right));
        assert_eq!(server.windows[&WM_SNAP_PREVIEW.0].x,640);
    }
