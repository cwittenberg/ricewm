use super::*;


    #[test]
    fn multimonitor_x11_drag_commits_destination_and_retains_it_on_relayout() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 2560, height_px: 1600, native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.set_output_layout(2560,1600,vec![(0,0,1280,800),(1280,800,1280,800)]);
        let owner = server.open_client().unwrap();
        server.feed(owner,&setup_request(order,&[],&[])).unwrap();
        let client = ResourceId(0x0020_0186);
        server.feed(owner,&create_window_request(order,client.0,STRUCTURE_NOTIFY_MASK | EXPOSURE_MASK)).unwrap();
        server.feed(owner,&request(order,8,0,&client.0.to_le_bytes())).unwrap();
        let frame = server.managed_frames[&client.0];
        server.apply_managed_geometry(client,frame,400,200,480,300);
        server.inject_pointer_button(true,1,500,212,0);
        server.inject_pointer_motion(1800,1000,0x100);
        server.inject_pointer_button(false,1,1800,1000,0x100);
        assert_eq!((server.windows[&frame.0].x,server.windows[&frame.0].y),(1700,988));
        server.inject_pointer_button(true,1,1800,1000,0);
        server.inject_pointer_motion(1280,1100,0x100);
        let preview = &server.windows[&WM_SNAP_PREVIEW.0];
        assert_eq!((preview.x,preview.y,preview.width,preview.height),(1280,800,640,800));
        server.inject_pointer_button(false,1,1280,1100,0x100);
        let snapped = &server.windows[&frame.0];
        assert_eq!((snapped.x,snapped.y,snapped.width,snapped.height),(1280,800,640,800));
        server.relayout_wm_edge_windows();
        assert_eq!((server.windows[&frame.0].x,server.windows[&frame.0].y),(1280,800));
        server.inject_pointer_motion(200,200,0);
        server.snap_active_window(WmSnapTarget::Right);
        assert_eq!(server.windows[&frame.0].x,1920,"keyboard snapping follows the window, not the pointer");
    }

    #[test]
    fn native_wm_start_button_context_opens_the_menu_directory() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let taskbar_y = i16::try_from(server.wm_taskbar_y()).unwrap();
        server.inject_pointer_button(true, 3, 18, taskbar_y + 12, 0);

        assert_eq!(server.wm_menu_level, WmMenuLevel::MenuDirectoryContext);
        let menu = server.windows[&WM_MENU.0].clone();
        assert!(menu.mapped);
        assert_eq!(menu.x, 18, "the context pane starts at the pointer x");
        assert_eq!(menu.height, u16::try_from(WM_MENU_ROW_HEIGHT).unwrap());
        assert_eq!(
            server.wm_menu_columns(),
            vec![vec![WmMenuRow::MenuOpenDirectory]],
        );

        server.inject_pointer_button(
            true,
            1,
            menu.x + 12,
            menu.y + i16::try_from(WM_MENU_ROW_HEIGHT / 2).unwrap(),
            0,
        );
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_MENU_OPEN_DIRECTORY)
        );
    }

    #[test]
    fn native_wm_workspace_row_context_preferences_and_show_desktop_are_live() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.inject_pointer_button(true, 1, 18, 584, 0);
        let row = server
            .wm_root_rows()
            .iter()
            .position(|row| *row == WmMenuRow::WorkspaceNavigation)
            .unwrap();
        let menu = server.windows[&WM_MENU.0].clone();
        let y = menu.y
            + i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + row * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap();
        assert!(matches!(
            server.wm_menu_hit(menu.x + 20, y),
            Some((0, _, WmMenuRow::PreviousWorkspace))
        ));
        assert!(matches!(
            server.wm_menu_hit(menu.x + 200, y),
            Some((0, _, WmMenuRow::NextWorkspace))
        ));
        server.inject_pointer_motion(menu.x + 20, y, 0);
        assert!(matches!(
            server.wm_menu_hover_row.as_ref(),
            Some((0, _, WmMenuRow::PreviousWorkspace))
        ));
        server.inject_pointer_motion(menu.x + 200, y, 0);
        assert!(matches!(
            server.wm_menu_hover_row.as_ref(),
            Some((0, _, WmMenuRow::NextWorkspace))
        ));

        server.dismiss_wm_menu();
        let tray_left = server.wm_tray_layout().first().map(|item| item.2).unwrap();
        let taskbar_context_x =
            i16::try_from((usize::try_from(server.wm_task_button_x()).unwrap() + tray_left) / 2)
                .unwrap();
        server.inject_pointer_button(true, 3, taskbar_context_x, 584, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::TaskbarContext);
        let context_rows = server.wm_menu_columns().remove(0);
        assert_eq!(
            &context_rows[..3],
            &[
                WmMenuRow::StackTile,
                WmMenuRow::StackCascade,
                WmMenuRow::Divider,
            ],
        );
        let preferences = context_rows
            .iter()
            .position(|row| *row == WmMenuRow::ToolbarPreferences)
            .unwrap();
        let preferences_y = server.windows[&WM_MENU.0].y
            + i16::try_from(
                wm_menu_row_top(&context_rows, preferences)
                    + wm_menu_row_height(&context_rows[preferences]) / 2,
            )
            .unwrap();
        server.inject_pointer_button(true, 1, server.windows[&WM_MENU.0].x + 20, preferences_y, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_TOOLBAR_PREFERENCES)
        );

        server.inject_pointer_motion(WM_QUICK_LAUNCH_X + 4, 584, 0);
        assert!(server.windows[&WM_TOOLTIP.0].mapped);
        server.inject_pointer_button(true, 3, WM_QUICK_LAUNCH_X + 4, 584, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::QuickLaunchContext);
        assert!(
            !server.windows[&WM_TOOLTIP.0].mapped,
            "the quick-launch tooltip must close before its context menu is shown",
        );
        assert_eq!(
            server.wm_menu_columns(),
            vec![vec![WmMenuRow::QuickShortcutPreferences]],
        );
        let quick_preferences_y =
            server.windows[&WM_MENU.0].y + i16::try_from(WM_MENU_ROW_HEIGHT / 2).unwrap();
        server.inject_pointer_motion(server.windows[&WM_MENU.0].x + 20, quick_preferences_y, 0);
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::QuickLaunchContext,
            "hovering the shortcut context row must not reveal the RiceWM root menu",
        );
        server.inject_pointer_button(
            true,
            1,
            server.windows[&WM_MENU.0].x + 20,
            quick_preferences_y,
            0,
        );
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_QUICK_SHORTCUT_PREFERENCES)
        );

        server.inject_key(true, 40, 0x0008);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_SHOW_DESKTOP));
        assert!(server.replace_quick_launchers(
            true,
            vec![WmQuickLauncher {
                action: WM_ACTION_SHOW_DESKTOP,
                icon: 12,
                title: b"Show desktop".to_vec(),
            }],
        ));
        server.inject_pointer_button(true, 1, 125, 584, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_SHOW_DESKTOP));
        server.inject_pointer_button(true, 1, 125, 584, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_SHOW_DESKTOP));
        assert!(server.replace_quick_launchers(
            true,
            vec![
                WmQuickLauncher {
                    action: WM_ACTION_SHOW_DESKTOP,
                    icon: 12,
                    title: b"Show desktop".to_vec(),
                },
                WmQuickLauncher {
                    action: WM_ACTION_PCMANFM,
                    icon: 11,
                    title: rice_i18n::t("ui.file-manager").as_bytes().to_vec(),
                },
                WmQuickLauncher {
                    action: WM_ACTION_TERMINAL,
                    icon: 2,
                    title: b"Terminal".to_vec(),
                },
                WmQuickLauncher {
                    action: 41,
                    icon: 7,
                    title: b"Example Paint".to_vec(),
                },
            ],
        ));
        assert_eq!(server.wm_task_button_x(), WM_QUICK_LAUNCH_X + 4 * 28 + 8);
        server.inject_pointer_motion(WM_QUICK_LAUNCH_X + 3 * 28 + 4, 584, 0);
        assert_eq!(server.wm_chrome_hover, Some(WmChromeTarget::QuickLaunch(3)));
        assert!(
            server.wm_text_overlays.iter().any(|overlay| {
                overlay.window == WM_TOOLTIP && overlay.text == b"Example Paint"
            })
        );
        server.inject_pointer_button(true, 1, WM_QUICK_LAUNCH_X + 3 * 28 + 4, 584, 0);
        assert_eq!(server.drain_wm_action(), Some(41));
        assert!(server.replace_quick_launchers(
            false,
            vec![WmQuickLauncher {
                action: WM_ACTION_TERMINAL,
                icon: 2,
                title: b"Terminal".to_vec(),
            }],
        ));
        assert_eq!(server.wm_task_button_x(), WM_QUICK_LAUNCH_X);
    }

    #[test]
    fn taskbar_context_arranges_every_native_window_with_the_zero_target() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_native_tasks(vec![
            WmNativeTask {
                id: 7,
                workspace: 0,
                icon: 1,
                minimized: false,
                maximized: false,
                active: true,
                title: b"RiceFiles".to_vec(),
            },
            WmNativeTask {
                id: 8,
                workspace: 0,
                icon: 2,
                minimized: false,
                maximized: false,
                active: false,
                title: b"RicePaper".to_vec(),
            },
        ]));
        server.show_wm_task_menu(WmMenuLevel::TaskbarContext, 500, 584);
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 12, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_STACK_TILE_PREFIX),);
    }

    #[test]
    fn native_wm_themes_end_with_divider_and_customize_action() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let theme_rows = server.wm_theme_rows();
        assert_eq!(theme_rows[theme_rows.len() - 2], WmMenuRow::Divider);
        assert_eq!(theme_rows.last(), Some(&WmMenuRow::ThemeCustomize));

        server.inject_pointer_button(true, 1, 18, 584, 0);
        let themes = server
            .wm_root_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Themes)
            .unwrap();
        let themes_y = server.windows[&WM_MENU.0].y
            + i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + themes * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap();
        server.inject_pointer_motion(18, themes_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Themes);

        let theme_rows = server.wm_theme_rows();
        let customize = theme_rows.len() - 1;
        let customize_y = server.windows[&WM_MENU.0].y
            + i16::try_from(
                server.wm_menu_column_offsets()[1]
                    + wm_menu_row_top(&theme_rows, customize)
                    + wm_menu_row_height(&theme_rows[customize]) / 2,
            )
            .unwrap();
        let x = i16::try_from(server.wm_menu_column_widths()[0] + 12).unwrap();
        server.inject_pointer_button(true, 1, x, customize_y, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_THEME_CUSTOMIZE));
    }

    #[test]
    fn native_wm_publishes_data_only_gtk_palette_xsettings() {
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let selection = server.atoms.intern("_XSETTINGS_S0", false);
        assert_eq!(
            server.selections.get(&selection.0).map(|entry| entry.0),
            Some(WM_XSETTINGS),
        );
        let property = server.atoms.intern("_XSETTINGS_SETTINGS", false);
        let first = server.windows[&WM_XSETTINGS.0].properties[&property]
            .data
            .clone();
        assert_eq!(&first[..4], &[0, 0, 0, 0]);
        assert_eq!(u32::from_le_bytes(first[8..12].try_into().unwrap()), 4);
        assert!(
            first
                .windows(b"Gtk/ColorScheme".len())
                .any(|window| window == b"Gtk/ColorScheme")
        );
        assert!(
            first
                .windows(b"Gtk/FontName".len())
                .any(|window| window == b"Gtk/FontName")
        );
        assert!(
            first
                .windows(b"Net/IconThemeName".len())
                .any(|window| window == b"Net/IconThemeName")
        );
        assert!(
            first
                .windows(b"Adwaita".len())
                .any(|window| window == b"Adwaita"),
            "accelerated GTK images require the deterministic Adwaita icon closure",
        );
        assert!(
            !first
                .windows(b"nuoveXT2".len())
                .any(|window| window == b"nuoveXT2"),
            "XSETTINGS must not select the sparse legacy icon theme",
        );
        assert!(
            first
                .windows(b"Net/ThemeName".len())
                .any(|window| window == b"Net/ThemeName")
        );
        assert!(
            first
                .windows(b"RiceWM-00".len())
                .any(|window| window == b"RiceWM-00")
        );

        server.wm_theme = 2;
        server.publish_gtk_theme_settings();
        let updated = &server.windows[&WM_XSETTINGS.0].properties[&property].data;
        assert_ne!(&first, updated);
        assert!(
            updated
                .windows(b"RiceWM-02".len())
                .any(|window| window == b"RiceWM-02")
        );
        assert!(updated.len() <= 512, "XSETTINGS must remain bounded data");
        let mut reordered = server
            .wm_themes
            .iter()
            .map(|theme| {
                (
                    theme.name.clone(),
                    theme.backdrop_material,
                    theme.menu_backdrop_material,
                    theme.chrome_profile,
                    theme.taskbar,
                    theme.menu,
                    theme.frame,
                )
            })
            .collect::<Vec<_>>();
        reordered.swap(0, 2);
        assert!(server.replace_wm_themes(reordered));
        assert_eq!(
            server.wm_theme, 0,
            "live catalog refresh preserves the selected theme by name",
        );
        let preserved = &server.windows[&WM_XSETTINGS.0].properties[&property].data;
        assert!(
            preserved
                .windows(b"RiceWM-00".len())
                .any(|window| window == b"RiceWM-00"),
        );
    }

    #[test]
    fn native_wm_theme_menu_emits_application_theme_action() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });

        server.inject_pointer_button(true, 1, 18, 584, 0);
        let menu = server.windows[&WM_MENU.0].clone();
        let theme_row = server
            .wm_root_rows()
            .iter()
            .position(|row| *row == WmMenuRow::Themes)
            .unwrap();
        let themes_y = menu.y.saturating_add(
            i16::try_from(
                server.wm_menu_column_offsets()[0]
                    + theme_row * WM_MENU_ROW_HEIGHT
                    + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap(),
        );
        server.inject_pointer_button(true, 1, menu.x + 18, themes_y, 0);
        assert_eq!(server.wm_menu_level, WmMenuLevel::Themes);
        assert!(
            server.wm_menu_column_offsets()[1]
                <= server.wm_menu_column_offsets()[0] + theme_row * WM_MENU_ROW_HEIGHT,
            "the theme flyout aligns with its parent unless bounded by the output edge",
        );

        let menu = server.windows[&WM_MENU.0].clone();
        let dark_x = menu.x + i16::try_from(server.wm_menu_column_widths()[0]).unwrap() + 18;
        let dark_y = menu.y.saturating_add(
            i16::try_from(
                server.wm_menu_column_offsets()[1] + WM_MENU_ROW_HEIGHT + WM_MENU_ROW_HEIGHT / 2,
            )
            .unwrap(),
        );
        server.inject_pointer_button(true, 1, dark_x, dark_y, 0);

        assert_eq!(server.wm_theme, 1);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_THEME_PREFIX | 1),);
        assert!(!server.windows[&WM_MENU.0].mapped);
    }

    #[test]
    fn orchard_chrome_moves_traffic_light_controls_to_the_leading_edge() {
        let width = 420;
        assert_eq!(wm_frame_control_left(WmChromeProfile::Orchard, width, 2), 8);
        assert_eq!(
            wm_frame_control_left(WmChromeProfile::Orchard, width, 0),
            30
        );
        assert_eq!(
            wm_frame_control_left(WmChromeProfile::Orchard, width, 1),
            52
        );
        assert_eq!(
            wm_frame_control_at(WmChromeProfile::Orchard, width, 12),
            Some(2)
        );
        assert_eq!(
            wm_frame_control_at(WmChromeProfile::Orchard, width, 34),
            Some(0)
        );
        assert_eq!(
            wm_frame_control_at(WmChromeProfile::Orchard, width, 56),
            Some(1)
        );
        assert!(wm_frame_control_left(WmChromeProfile::Standard, width, 0) > width / 2);
        assert_eq!(wm_frame_icon_left(WmChromeProfile::Orchard), 76);
    }

    #[test]
    fn split_chrome_places_minimize_opposite_maximize_and_close() {
        let width = 420;
        assert_eq!(
            wm_frame_control_left(WmChromeProfile::SplitControls, width, 0),
            3
        );
        assert_eq!(
            wm_frame_control_left(WmChromeProfile::SplitControls, width, 1),
            width - usize::from(WM_TITLE_BUTTON_WIDTH * 2),
        );
        assert_eq!(
            wm_frame_control_left(WmChromeProfile::SplitControls, width, 2),
            width - usize::from(WM_TITLE_BUTTON_WIDTH),
        );
        assert_eq!(
            wm_frame_control_at(WmChromeProfile::SplitControls, width, 8),
            Some(0),
        );
        assert_eq!(wm_frame_icon_left(WmChromeProfile::SplitControls), 29);
    }

    #[test]
    fn native_wm_menu_keyboard_enters_returns_and_activates() {
        let mut server = DisplayServer::new(DisplayConfig { width_px: 800, height_px: 600, native_window_manager: true, ..DisplayConfig::default() });
        server.inject_key(true, 133, 4);
        assert!(server.windows[&WM_MENU.0].mapped);
        server.inject_key(false, 133, 0x44);
        server.inject_key(true, 116, 0);
        assert!(server.wm_menu_hover_row.is_some());
        let initial = server.wm_menu_hover_row.clone();
        server.inject_key(true, 114, 0);
        assert!(!server.wm_menu_keyboard_parents.is_empty());
        server.inject_key(true, 113, 0);
        assert_eq!(server.wm_menu_hover_row, initial);
        let rows = server.wm_menu_columns();
        let run = rows[0].iter().position(|row| *row == WmMenuRow::Run).unwrap();
        server.select_wm_menu_keyboard_row(0, run);
        server.inject_key(true, 36, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RUN));
        assert!(!server.windows[&WM_MENU.0].mapped);
        server.inject_key(true, 134, 0);
        server.inject_key(false, 134, 0x40);
        assert!(server.windows[&WM_MENU.0].mapped);
        let run = server.wm_menu_columns()[0].iter().position(|row| *row == WmMenuRow::Run).unwrap();
        server.select_wm_menu_keyboard_row(0, run);
        server.inject_key(true, 65, 0);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_RUN));
        assert!(!server.windows[&WM_MENU.0].mapped);
    }

    #[test]
    fn native_wm_menu_tab_and_shift_tab_move_selection() {
        let mut server = DisplayServer::new(DisplayConfig { native_window_manager: true, ..DisplayConfig::default() });
        server.inject_key(true, 133, 4);
        server.inject_key(false, 133, 0x44);
        server.inject_key(true, 23, 0);
        let first = server.wm_menu_hover_row.clone();
        assert!(first.is_some());
        server.inject_key(false, 23, 0);
        server.inject_key(true, 23, 0);
        assert_ne!(server.wm_menu_hover_row, first);
        server.inject_key(false, 23, 0);
        server.inject_key(true, 23, 1);
        assert_eq!(server.wm_menu_hover_row, first);
    }

    #[test]
    fn native_wm_menu_keyboard_skips_non_actions_and_consumes_shortcuts() {
        let mut server = DisplayServer::new(DisplayConfig { native_window_manager: true, ..DisplayConfig::default() });
        server.inject_key(true, 133, 0);
        server.inject_key(false, 133, 0x40);
        server.inject_key(true, 115, 0);
        assert!(!matches!(server.wm_menu_hover_row.as_ref().unwrap().2, WmMenuRow::Divider | WmMenuRow::Empty));
        server.inject_key(true, 27, 8); // Alt+R must not launch Run behind an open menu.
        assert_eq!(server.drain_wm_action(), None);
        server.inject_key(true, 9, 0);
        assert!(!server.windows[&WM_MENU.0].mapped);
    }
