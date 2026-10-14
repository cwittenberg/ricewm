use super::*;

#[test]
fn orchard_compact_dock_floats_and_keeps_icons_on_their_click_targets() {
    let mut server = DisplayServer::new(DisplayConfig {
        width_px: 1000, height_px: 700, native_window_manager: true,
        ..DisplayConfig::default()
    });
    let palette = built_in_wm_themes().remove(0);
    assert!(server.replace_wm_themes(vec![(b"Orchard".to_vec(), true, false,
        WmChromeProfile::Orchard, palette.taskbar, palette.menu, palette.frame)]));
    assert!(server.set_dock_style(true));
    assert!(server.set_dock_magnification(false, 65, 45));
    for size in [64, 32, 44, 64] {
    assert!(server.set_dock_height(size));
    assert_eq!(server.wm_dock_bar_height(), size);
    let (x, y, width, height) = server.wm_taskbar_rect();
    let surface = &server.surfaces[&WM_TASKBAR.0];
    assert!(surface.pixel(width / 2, height - 1) == 0,
        "the floating dock leaves clear pixels at the screen edge");
    let icons: Vec<_> = server.wm_text_overlays.iter()
        .filter(|overlay| overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:"))
        .collect();
    assert_eq!(icons.len(), server.wm_dock_targets().len());
    for (icon, target) in icons.iter().zip(server.wm_dock_targets()) {
        assert!(icon.y + icon.height < height - 8, "icons fit inside the glass strip");
        assert_eq!(server.wm_chrome_hit(WM_TASKBAR,
            x + (icon.x + icon.width / 2) as i16,
            y + (icon.y + icon.height / 2) as i16), Some(target));
    }
    }
    assert!(!server.set_dock_height(31));
    assert!(!server.set_dock_height(81));
}


    #[test]
    fn native_wm_menu_can_be_dismissed_without_forwarding_a_pointer_press() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.inject_pointer_button(true, 1, 18, 584, 0);
        assert!(server.windows[&WM_MENU.0].mapped);
        assert!(server.dismiss_wm_menu());
        assert!(!server.windows[&WM_MENU.0].mapped);
        assert!(server.dismiss_wm_menu(), "dismissal is safely idempotent");
    }

    #[test]
    fn frosted_ice_shell_chrome_preserves_translucency_without_white_rim() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(300, 10) >> 24,
            u32::from(u8::MAX),
            "opaque themes must remain visible on the ARGB taskbar surface",
        );
        let palette = built_in_wm_themes().remove(0);
        assert!(server.replace_wm_themes(vec![(
            b"Frosted Ice".to_vec(),
            true,
            false,
            WmChromeProfile::Standard,
            palette.taskbar,
            palette.menu,
            palette.frame,
        )]));

        assert_eq!(server.windows[&WM_TASKBAR.0].depth, 32);
        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(300, 10) >> 24,
            u32::from(WM_FROSTED_ICE_SURFACE_ALPHA),
        );

        server.inject_pointer_button(true, 1, 18, 584, 0);
        let menu = &server.surfaces[&WM_MENU.0];
        let menu_y = server.wm_menu_column_offsets()[0];
        assert_eq!(
            menu.pixel(0, u16::try_from(menu_y).unwrap()),
            0,
            "rounded glass corner stays clear",
        );
        assert_eq!(
            menu.pixel(200, u16::try_from(menu_y + 15).unwrap()) >> 24,
            u32::from(WM_FROSTED_ICE_SURFACE_ALPHA),
        );
        assert_eq!(
            menu.pixel(10, u16::try_from(menu_y + 1).unwrap()),
            0xbe31_2b35,
            "frosted menu edge uses its dark surface tint without a white rim",
        );
    }

    #[test]
    fn orchard_taskbar_covers_the_reserved_strip_without_clear_margins() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let palette = built_in_wm_themes().remove(0);
        assert!(server.replace_wm_themes(vec![(
            b"Orchard".to_vec(),
            true,
            false,
            WmChromeProfile::Orchard,
            palette.taskbar,
            palette.menu,
            palette.frame,
        )]));
        let bar = &server.surfaces[&WM_TASKBAR.0];
        let expected = liquid_glass_color(palette.taskbar[0], &palette);
        for (x, y) in [(0, 0), (400, 0), (799, 0), (0, 31), (400, 31), (799, 31)] {
            assert_eq!(bar.pixel(x, y), expected, "bar margin at ({x}, {y})");
        }
    }

    #[test]
    fn menu_only_glass_keeps_taskbar_and_window_frames_opaque() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let palette = built_in_wm_themes().remove(0);
        assert!(server.replace_wm_themes(vec![(
            b"Jade".to_vec(),
            false,
            true,
            WmChromeProfile::Standard,
            palette.taskbar,
            palette.menu,
            palette.frame,
        )]));
        assert_eq!(
            server.surfaces[&WM_TASKBAR.0].pixel(300, 10) >> 24,
            u32::from(u8::MAX),
        );
        server.inject_pointer_button(true, 1, 18, 584, 0);
        let menu_y = server.wm_menu_column_offsets()[0];
        assert_eq!(
            server.surfaces[&WM_MENU.0].pixel(200, u16::try_from(menu_y + 15).unwrap()) >> 24,
            u32::from(WM_FROSTED_ICE_SURFACE_ALPHA),
        );
    }

    #[test]
    fn rounded_menu_caps_share_the_straight_sections_full_height() {
        const WIDTH: usize = 48;
        const HEIGHT: usize = 24;
        const RADIUS: usize = HEIGHT / 2;
        const COLOR: u32 = 0xff12_3456;
        let mut pixels = vec![0; WIDTH * HEIGHT];

        fill_rounded_rect(
            &mut pixels,
            WIDTH,
            HEIGHT,
            0,
            0,
            WIDTH,
            HEIGHT,
            RADIUS,
            COLOR,
        );

        for cap in [0..RADIUS, WIDTH - RADIUS..WIDTH] {
            assert!(
                cap.clone().any(|x| pixels[x] == COLOR),
                "a rounded menu cap must reach the straight section's top edge",
            );
            assert!(
                cap.clone()
                    .any(|x| pixels[(HEIGHT - 1) * WIDTH + x] == COLOR),
                "a rounded menu cap must reach the straight section's bottom edge",
            );
        }
        assert!((0..HEIGHT).all(|y| pixels[y * WIDTH + RADIUS] == COLOR));
        assert!((0..HEIGHT).all(|y| pixels[y * WIDTH + WIDTH - RADIUS - 1] == COLOR));
        assert!((0..HEIGHT).all(|y| {
            (0..WIDTH).all(|x| pixels[y * WIDTH + x] == pixels[(HEIGHT - 1 - y) * WIDTH + x])
        }));
    }

    #[test]
    fn neon_controls_cut_only_the_upper_left_and_lower_right_corners() {
        const WIDTH: usize = 40;
        const HEIGHT: usize = 22;
        const COLOR: u32 = 0xff00_f5ff;
        let mut pixels = vec![0; WIDTH * HEIGHT];

        fill_wedged_rect(&mut pixels, WIDTH, HEIGHT, 0, 0, WIDTH, HEIGHT, 7, COLOR);

        assert_eq!(pixels[0], 0, "upper-left is cut away");
        assert_eq!(pixels[7], COLOR, "the top edge begins after the cut");
        assert_eq!(pixels[WIDTH - 1], COLOR, "upper-right remains square");
        assert_eq!(
            pixels[(HEIGHT - 1) * WIDTH],
            COLOR,
            "lower-left remains square"
        );
        assert_eq!(pixels[HEIGHT * WIDTH - 1], 0, "lower-right is cut away");
        assert_eq!(
            pixels[(HEIGHT - 1) * WIDTH + WIDTH - 8],
            COLOR,
            "the bottom edge ends before the lower-right cut",
        );
    }

    #[test]
    fn menu_selection_is_a_flush_square_row() {
        assert_eq!(WM_MENU_SELECTION_INSET, 0);
        assert_eq!(WM_MENU_SELECTION_RADIUS, 0);
    }

    #[test]
    fn native_wm_application_overflow_scrolls_one_row_and_clamps() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let entries = (1..=20)
            .map(|action| {
                (
                    action,
                    1,
                    1,
                    format!("APPLICATION {action:02}").into_bytes(),
                )
            })
            .collect();
        assert!(server.replace_wm_menu_entries(entries));
        server.wm_menu_level = WmMenuLevel::Applications(1);
        server.windows.get_mut(&WM_MENU.0).unwrap().mapped = true;
        server.relayout_wm_menu();

        let first_action = |server: &DisplayServer| match &server.wm_application_rows(1)[1] {
            WmMenuRow::Application(entry) => entry.action,
            entry => panic!("expected first visible application, got {entry:?}"),
        };
        assert_eq!(first_action(&server), 1);
        assert!(matches!(
            server.wm_application_rows(1).first(),
            Some(WmMenuRow::ScrollUp),
        ));
        assert!(matches!(
            server.wm_application_rows(1).last(),
            Some(WmMenuRow::ScrollDown),
        ));

        let menu_y = server.windows.get(&WM_MENU.0).unwrap().y;
        server.inject_pointer_button(true, 5, 450, menu_y.saturating_add(80), 0);
        assert_eq!(server.wm_menu_offset, 1);
        assert_eq!(first_action(&server), 2);
        for _ in 0..40 {
            server.inject_pointer_button(true, 5, 450, menu_y.saturating_add(80), 0);
        }
        assert_eq!(server.wm_menu_offset, server.wm_application_max_offset(1),);
        server.inject_pointer_button(true, 4, 450, menu_y.saturating_add(80), 0);
        assert_eq!(
            server.wm_menu_offset,
            server.wm_application_max_offset(1).saturating_sub(1),
        );
    }

    #[test]
    fn native_wm_reports_reparented_geometry_before_mapping_a_client() {
        let order = ClientByteOrder::LittleEndian;
        let mut server = DisplayServer::new(DisplayConfig {
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let client = server.open_client().unwrap();
        let setup = setup_request(order, &[], &[]);
        server.feed(client, &setup).unwrap();
        let _ = drain_all(&mut server, client);
        let window = 0x0020_0080;
        server
            .feed(
                client,
                &create_window_request(
                    order,
                    window,
                    STRUCTURE_NOTIFY_MASK | VISIBILITY_CHANGE_MASK | EXPOSURE_MASK,
                ),
            )
            .unwrap();
        server
            .feed(client, &request(order, 8, 0, &window.to_le_bytes()))
            .unwrap();

        let events = drain_all(&mut server, client);
        let event_types = events
            .chunks_exact(32)
            .map(|event| event[0] & 0x7f)
            .collect::<Vec<_>>();
        let reparent = event_types.iter().position(|event| *event == 21).unwrap();
        let configure = event_types.iter().position(|event| *event == 22).unwrap();
        let mapped = event_types.iter().position(|event| *event == 19).unwrap();
        let visible = event_types.iter().position(|event| *event == 15).unwrap();
        let exposed = event_types.iter().position(|event| *event == 12).unwrap();
        assert!(reparent < configure);
        assert!(configure < mapped);
        assert!(mapped < visible);
        assert!(visible < exposed);
        let configure_event = &events[configure * 32..configure * 32 + 32];
        assert_ne!(configure_event[0] & 0x80, 0);
        assert_eq!(
            u32::from_le_bytes(configure_event[8..12].try_into().unwrap()),
            window
        );
        assert_eq!(
            i16::from_le_bytes(configure_event[16..18].try_into().unwrap()),
            12 + i16::try_from(WM_BORDER_WIDTH).unwrap()
        );
        assert_eq!(
            i16::from_le_bytes(configure_event[18..20].try_into().unwrap()),
            42
        );
    }

    #[test]
    fn native_wm_volume_tray_control_mutes_and_scrolls() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 640,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let (_, _, sound_x, sound_width) = server
            .wm_tray_layout()
            .into_iter()
            .find(|(_, item, _, _)| *item == TrayItem::Sound)
            .expect("sound plugin");
        let volume_x = i16::try_from(sound_x + sound_width / 2).unwrap();
        let taskbar_y = i16::try_from(server.config.height_px - 16).unwrap();

        server.inject_pointer_button(true, 1, volume_x, taskbar_y, 0);
        assert!(server.wm_volume_muted);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_VOLUME_PREFIX | 0x100 | 100),
        );

        server.inject_pointer_button(true, 5, volume_x, taskbar_y, 0);
        assert!(!server.wm_volume_muted);
        assert_eq!(server.wm_volume, 95);
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_VOLUME_PREFIX | 95),);
        assert_eq!(server.wm_notification_message, b"95%");
    }

    #[test]
    fn native_wm_tray_plugins_reorder_remove_and_launch() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 640,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_tray_items(vec![
            TrayItem::Clock,
            TrayItem::Launcher(rice_tray::TrayLauncher::new(LauncherTarget::ImageViewer)),
            TrayItem::Sound,
        ]));
        assert!(!server.replace_tray_items(vec![TrayItem::Clock, TrayItem::Clock]));
        let layout = server.wm_tray_layout();
        assert_eq!(
            layout.iter().map(|item| item.1).collect::<Vec<_>>(),
            [
                TrayItem::Clock,
                TrayItem::Launcher(rice_tray::TrayLauncher::new(LauncherTarget::ImageViewer)),
                TrayItem::Sound,
            ],
        );
        let (_, _, launcher_x, launcher_width) = layout[1];
        let taskbar_y = i16::try_from(server.config.height_px - 16).unwrap();
        server.inject_pointer_button(
            true,
            1,
            i16::try_from(launcher_x + launcher_width / 2).unwrap(),
            taskbar_y,
            0,
        );
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_IMAGE_VIEWER));
    }

    #[test]
    fn redmond_compacts_its_menus_and_keeps_a_classic_workspace_selector() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 640,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_tray_items(vec![
            TrayItem::Clock,
            TrayItem::Workspaces,
            TrayItem::Performance,
            TrayItem::Sound,
        ]));
        let palette = built_in_wm_themes().remove(0);
        let record = |name: &[u8], profile| {
            (
                name.to_vec(),
                palette.backdrop_material,
                palette.menu_backdrop_material,
                profile,
                palette.taskbar,
                palette.menu,
                palette.frame,
            )
        };
        assert!(server.replace_wm_themes(vec![
            record(b"Orchard", WmChromeProfile::Orchard),
            record(b"Redmond '98", WmChromeProfile::Redmond98),
        ]));

        assert_eq!(server.wm_menu_base_row_height(), WM_MENU_ROW_HEIGHT);
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Performance)
        );

        assert!(server.select_wm_theme(1));
        assert_eq!(server.wm_menu_base_row_height(), REDMOND_MENU_ROW_HEIGHT);
        server.wm_menu_level = WmMenuLevel::Categories;
        let redmond_widths = server.wm_menu_column_widths();
        let redmond_offsets = server.wm_menu_column_x_offsets();
        assert_eq!(redmond_offsets[1], redmond_widths[0].saturating_sub(2));
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Performance)
        );
        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: 7,
            workspace: 0,
            icon: 0,
            minimized: false,
            maximized: false,
            active: true,
            title: b"Unknown application title".to_vec(),
        }]));
        let overlay = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_TASKBAR && overlay.text.starts_with(b"Unknown"))
            .unwrap_or_else(|| panic!("Redmond task label: {:?}", server.wm_text_overlays));
        let icon_x = overlay.x.saturating_sub(22);
        let taskbar = &server.surfaces[&WM_TASKBAR.0];
        let icon_pixels = (7..23)
            .flat_map(|y| (icon_x..icon_x + 16).map(move |x| taskbar.pixel(x, y)))
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            icon_pixels.len() > 1,
            "even an unmapped Redmond title must retain its fallback task icon"
        );
        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: 7,
            workspace: 0,
            icon: 7,
            minimized: false,
            maximized: false,
            active: true,
            title: b"RicePaper".to_vec(),
        }]));
        let overlay = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_TASKBAR && overlay.text == b"RicePaper")
            .expect("Paper task label");
        let icon_x = overlay.x.saturating_sub(20);
        let icon_y = 9;
        let taskbar = &server.surfaces[&WM_TASKBAR.0];
        let copied_icon = redmond_task_icon(7).expect("copied Notepad raster");
        let opaque_pixels = copied_icon
            .iter()
            .enumerate()
            .filter(|(_, pixel)| **pixel >> 24 == 0xff)
            .count();
        let matching_pixels = copied_icon
            .iter()
            .enumerate()
            .filter(|(_, pixel)| **pixel >> 24 == 0xff)
            .filter(|(offset, pixel)| {
                let offset = u16::try_from(*offset).expect("16 px icon offset");
                let x = icon_x + offset % 16;
                let y = icon_y + offset / 16;
                taskbar.pixel(x, y) & 0x00ff_ffff == **pixel & 0x00ff_ffff
            })
            .count();
        assert_eq!(
            matching_pixels, opaque_pixels,
            "the Redmond Paper task must draw the copied Notepad raster exactly"
        );
        assert_ne!(
            redmond_task_icon(11),
            redmond_task_icon(12),
            "File Manager and Show Desktop must not reuse one shortcut raster"
        );
        for kind in [1_u8, 2, 3, 7, 9, 10, 113, 114, 115, 116, 117, 121, 122, 132] {
            assert!(
                redmond_task_icon(kind).is_some(),
                "native icon kind {kind} needs an authentic Redmond raster"
            );
        }
        assert!(server.select_wm_theme(0));
        assert_eq!(server.wm_menu_base_row_height(), WM_MENU_ROW_HEIGHT);
        let orchard_widths = server.wm_menu_column_widths();
        let orchard_offsets = server.wm_menu_column_x_offsets();
        assert_eq!(orchard_offsets[1], orchard_widths[0]);
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
    }

    #[test]
    fn connectivity_tray_reflects_strategy_and_opens_parent_preferences() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_tray_items(vec![TrayItem::Connectivity]));
        let before = server.surfaces[&WM_TASKBAR.0].pixels.clone();
        assert!(server.set_connectivity_mode(2));
        assert_ne!(server.surfaces[&WM_TASKBAR.0].pixels, before);
        assert!(!server.set_connectivity_mode(4));
        let (_, _, left, item_width) = server.wm_tray_layout()[0];
        server.inject_pointer_button(
            true,
            1,
            i16::try_from(left + item_width / 2).unwrap(),
            584,
            0,
        );
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_NETWORK_PREFERENCES),
        );
    }
