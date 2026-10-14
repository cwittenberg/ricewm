use super::*;


    #[test]
    fn gnome_overview_visibility_controls_canonical_taskbar_elements() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        let taskbar_y = i16::try_from(server.wm_taskbar_y()).unwrap() + 12;
        assert_eq!(
            server.wm_chrome_hit(WM_TASKBAR, 10, taskbar_y),
            Some(WmChromeTarget::Start),
        );
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
        let visible_pixels = server.surfaces[&WM_TASKBAR.0].pixels.clone();

        assert!(server.set_gnome_overview_elements(false, false));
        assert_eq!(server.wm_chrome_hit(WM_TASKBAR, 10, taskbar_y), None);
        assert!(server.wm_task_targets().is_empty());
        assert!(
            !server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
        assert_ne!(server.surfaces[&WM_TASKBAR.0].pixels, visible_pixels);

        assert!(server.set_gnome_overview_elements(true, true));
        assert_eq!(
            server.wm_chrome_hit(WM_TASKBAR, 10, taskbar_y),
            Some(WmChromeTarget::Start),
        );
        assert!(
            server
                .wm_tray_layout()
                .iter()
                .any(|(_, item, _, _)| *item == TrayItem::Workspaces)
        );
    }

    #[test]
    fn performance_tray_graph_samples_and_double_click_launches_monitor() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.replace_tray_items(vec![TrayItem::Performance]));
        assert!(server.set_performance_sample(63, 28));
        assert_eq!(server.wm_performance_cpu.back(), Some(&63));
        assert_eq!(server.wm_performance_network.back(), Some(&28));
        let (_, _, left, width) = server.wm_tray_layout()[0];
        server.inject_pointer_button(
            true,
            6,
            i16::try_from(left + width / 2).unwrap(),
            i16::try_from(server.config.height_px - 16).unwrap(),
            0,
        );
        assert_eq!(server.drain_wm_action(), Some(WM_ACTION_PERFORMANCE));
    }

    #[test]
    fn embedded_shell_preserves_the_full_workarea_without_an_outer_border() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        for compact in [false, true] {
            server.set_dock_style(compact);
            for edge in 0..4 {
                server.set_taskbar_configuration(true, edge, false);
                let original = server.wm_workarea_rect();
                assert!(server.set_embedded_shell(true));
                assert_eq!(server.wm_workarea_rect(), original);
                assert!(server.set_embedded_shell(false));
                assert_eq!(server.wm_workarea_rect(), original);
            }
        }
    }

    #[test]
    fn floating_dock_keeps_geometry_clicks_and_auto_hide_in_sync() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.set_dock_style(true));
        for edge in 0..4 {
            assert!(server.set_taskbar_configuration(true, edge, false));
            let (x, y, width, height) = server.wm_taskbar_rect();
            assert!(width < 800 && height < 600);
            assert_eq!(width.min(height), WM_COMPACT_DOCK_SURFACE_THICKNESS);
            assert_eq!(server.wm_workarea_rect(), (0, 0, 800, 600));
            match edge {
                0 => assert_eq!(y + height as i16, 600),
                1 => assert_eq!(y, 0),
                2 => assert_eq!(x, 0),
                3 => assert_eq!(x + width as i16, 800),
                _ => unreachable!(),
            }
            assert_eq!(server.surfaces[&WM_TASKBAR.0].pixel(0, 0), 0);
            let targets = server.wm_dock_targets();
            for (i, expected) in targets.iter().enumerate() {
                let axis = (WM_COMPACT_DOCK_AXIS_PADDING
                    + i * server.wm_dock_step()
                    + server.wm_dock_step() / 2) as i16;
                let (px, py) = if edge >= 2 {
                    (x + i16::try_from(width / 2).unwrap(), y + axis)
                } else {
                    (x + axis, y + i16::try_from(height / 2).unwrap())
                };
                assert_eq!(server.wm_chrome_hit(WM_TASKBAR, px, py), Some(*expected));
            }
        }
        assert!(server.set_taskbar_configuration(true, 0, true));
        assert!(server.update_taskbar_pointer(400, 599));
        server.update_taskbar_pointer(400, 590);
        assert!(
            server.wm_taskbar_revealed,
            "the transparent animation surface must keep the dock revealed during approach"
        );
        server.update_taskbar_pointer(400, 400);
        assert!(!server.wm_taskbar_revealed);
        assert!(server.set_dock_style(false));
        assert_eq!(server.wm_taskbar_rect().2, 800);
    }

    #[test]
    fn compact_dock_magnification_tracks_pointer_with_a_bounded_neighbor_curve() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.set_dock_style(true));
        let (dock_x, dock_y, _, dock_height) = server.wm_taskbar_rect();
        let step = server.wm_dock_step();
        let pointer_x = dock_x
            + i16::try_from(WM_COMPACT_DOCK_AXIS_PADDING + step / 2).unwrap();
        let pointer_y = dock_y + i16::try_from(dock_height / 2).unwrap();

        assert!(server.set_dock_magnification(false, 100, 100));
        server.inject_pointer_motion(pointer_x, pointer_y, 0);
        let still = server
            .wm_text_overlays
            .iter()
            .filter(|overlay| {
                overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:")
            })
            .map(|overlay| overlay.width)
            .collect::<Vec<_>>();
        assert_eq!(still.len(), server.wm_dock_targets().len());
        assert!(
            still
                .iter()
                .all(|width| *width == WM_COMPACT_DOCK_ICON_SIZE as u16)
        );

        assert!(server.set_dock_magnification(true, 100, 100));
        server.inject_pointer_motion(pointer_x, pointer_y, 0);
        let magnified = server
            .wm_text_overlays
            .iter()
            .filter(|overlay| {
                overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:")
            })
            .map(|overlay| overlay.width)
            .collect::<Vec<_>>();
        assert_eq!(magnified[0], WM_COMPACT_DOCK_ICON_MAX_SIZE as u16);
        assert!(magnified[1] > WM_COMPACT_DOCK_ICON_SIZE as u16 && magnified[1] < magnified[0]);
        let first_icon = server
            .wm_text_overlays
            .iter()
            .find(|overlay| {
                overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:")
            })
            .unwrap();
        let bar_y = usize::from(dock_height) - WM_COMPACT_DOCK_BAR_THICKNESS;
        assert!(usize::from(first_icon.y) < bar_y);
        assert_eq!(server.surfaces[&WM_TASKBAR.0].pixel(0, 0), 0);
        assert_ne!(
            server.surfaces[&WM_TASKBAR.0]
                .pixel(server.surfaces[&WM_TASKBAR.0].width / 2, dock_height / 2),
            0
        );

        server.inject_pointer_motion(
            pointer_x + i16::try_from(step / 4).unwrap(),
            pointer_y,
            0);
        let followed = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_TASKBAR && overlay.text == b"__dock_icon:logo:0"
            )
            .unwrap();
        assert!(
            followed.width < WM_COMPACT_DOCK_ICON_MAX_SIZE as u16
                && followed.width > WM_COMPACT_DOCK_ICON_SIZE as u16
        );
        assert!(!server.set_dock_magnification(true, 101, 50));
    }

    #[test]
    fn compact_dock_start_menu_and_cascades_stay_inside_the_dock_edge() {
        for embedded in [false, true] {
            for edge in [WmTaskbarEdge::Bottom, WmTaskbarEdge::Top] {
                let mut server = DisplayServer::new(DisplayConfig {
                    width_px: 1263,
                    height_px: 893,
                    native_window_manager: true,
                    ..DisplayConfig::default()
                });
                server.set_embedded_shell(embedded);
                server.set_dock_style(true);
                server.set_taskbar_configuration(true, edge.wire() as u8, false);
                let (x, y, _, height) = server.wm_taskbar_rect();
                server.inject_pointer_button(true, 1,
                    x + (WM_COMPACT_DOCK_AXIS_PADDING + server.wm_dock_step() / 2) as i16,
                    y + (height / 2) as i16, 0,
                );
                assert!(server.windows[&WM_MENU.0].mapped);
                for level in [WmMenuLevel::Root, WmMenuLevel::Categories, WmMenuLevel::Themes,
                ] {
                    server.wm_menu_level = level;
                    server.relayout_wm_menu();
                    let menu = &server.windows[&WM_MENU.0];
                    assert!(menu.y >= 0);
                    assert!(menu.y as u16 + menu.height <= server.config.height_px);
                    if edge == WmTaskbarEdge::Bottom {
                        assert!(menu.y + menu.height as i16 <= y,
                            "embedded={embedded}, level={level:?}: menu overlaps bottom dock");
                    } else {
                        assert!(menu.y >= y + height as i16,
                            "embedded={embedded}, level={level:?}: menu overlaps top dock");
                    }
                }
            }
        }
    }

    #[test]
    fn compact_dock_preview_stays_outside_icons_and_retains_the_fisheye_hover() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.set_dock_style(true));
        assert!(server.set_dock_magnification(true, 100, 70));
        assert!(server.replace_native_tasks(vec![WmNativeTask {
            id: 7,
            workspace: 0,
            icon: 1,
            minimized: false,
            maximized: false,
            active: true,
            title: b"Bowl".to_vec(),
        }]));

        for edge in 0..4 {
            assert!(server.set_taskbar_configuration(true, edge, false));
            server.set_wm_task_preview(None, 0);
            let target = WmChromeTarget::Task(WmTaskTarget::Native(7));
            let index = server
                .wm_dock_targets()
                .iter()
                .position(|candidate| *candidate == target)
                .expect("native task target");
            let (dock_x, dock_y, dock_width, dock_height) = server.wm_taskbar_rect();
            let axis = WM_COMPACT_DOCK_AXIS_PADDING
                + index * server.wm_dock_step()
                + server.wm_dock_step() / 2;
            let (pointer_x, pointer_y) = if edge >= 2 {
                (
                    dock_x + i16::try_from(dock_width / 2).unwrap(),
                    dock_y + i16::try_from(axis).unwrap(),
                )
            } else {
                (
                    dock_x + i16::try_from(axis).unwrap(),
                    dock_y + i16::try_from(dock_height / 2).unwrap(),
                )
            };
            server.inject_pointer_motion(pointer_x, pointer_y, 0);

            let preview = server.windows[&WM_TASK_PREVIEW.0].clone();
            assert!(preview.mapped);
            assert_eq!(server.wm_chrome_hover, Some(target));
            match edge {
                0 => assert!(preview.y + preview.height as i16 <= dock_y),
                1 => assert!(preview.y >= dock_y + dock_height as i16),
                2 => assert!(preview.x >= dock_x + dock_width as i16),
                3 => assert!(preview.x + preview.width as i16 <= dock_x),
                _ => unreachable!(),
            }
            let magnified_icon = server
                .wm_text_overlays
                .iter()
                .filter(|overlay| {
                    overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:")
                })
                .nth(index)
                .expect("magnified native task icon")
                .clone();
            let magnified_width = magnified_icon.width;
            assert!(magnified_width > WM_COMPACT_DOCK_ICON_SIZE as u16);
            match edge {
                0 => assert!(
                    usize::from(magnified_icon.y)
                        < usize::from(dock_height) - WM_COMPACT_DOCK_BAR_THICKNESS,
                    "bottom dock icons must grow upward beyond the fixed bar"
                ),
                1 => assert!(
                    usize::from(magnified_icon.y) + usize::from(magnified_icon.height)
                        > WM_COMPACT_DOCK_BAR_THICKNESS,
                    "top dock icons must grow downward beyond the fixed bar"
                ),
                2 => assert!(
                    usize::from(magnified_icon.x) + usize::from(magnified_icon.width)
                        > WM_COMPACT_DOCK_BAR_THICKNESS,
                    "left dock icons must grow rightward beyond the fixed bar"
                ),
                3 => assert!(
                    usize::from(magnified_icon.x)
                        < usize::from(dock_width) - WM_COMPACT_DOCK_BAR_THICKNESS,
                    "right dock icons must grow leftward beyond the fixed bar"
                ),
                _ => unreachable!(),
            }

            server.inject_pointer_motion(
                preview.x + i16::try_from(preview.width / 2).unwrap(),
                preview.y + i16::try_from(preview.height / 2).unwrap(),
                0,
            );
            assert_eq!(server.wm_chrome_hover, Some(target));
            assert!(server.windows[&WM_TASK_PREVIEW.0].mapped);
            let retained_width = server
                .wm_text_overlays
                .iter()
                .filter(|overlay| {
                    overlay.window == WM_TASKBAR && overlay.text.starts_with(b"__dock_icon:")
                })
                .nth(index)
                .expect("retained native task icon")
                .width;
            assert_eq!(retained_width, magnified_width);

            let (blank_x, blank_y) = if edge >= 2 {
                (
                    dock_x + i16::try_from(dock_width / 2).unwrap(),
                    dock_y + i16::try_from(dock_height.saturating_sub(1)).unwrap(),
                )
            } else {
                (
                    dock_x + i16::try_from(dock_width.saturating_sub(1)).unwrap(),
                    dock_y + i16::try_from(dock_height / 2).unwrap(),
                )
            };
            assert_eq!(
                server.wm_chrome_hit(WM_TASKBAR, blank_x, blank_y),
                None,
                "the end padding is blank compact-dock space"
            );
            server.inject_pointer_motion(blank_x, blank_y, 0);
            assert_eq!(server.wm_chrome_hover, None);
            assert!(
                !server.windows[&WM_TASK_PREVIEW.0].mapped,
                "leaving the preview and its related icon must dismiss it immediately"
            );
        }
    }

    #[test]
    fn auto_hidden_taskbar_reveals_from_a_native_desktop_edge_sensor() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1024,
            height_px: 640,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        assert!(server.set_taskbar_configuration(true, 0, true));
        assert!(!server.windows[&WM_TASKBAR.0].mapped);

        assert!(server.update_taskbar_pointer(400, 635));
        assert!(server.windows[&WM_TASKBAR.0].mapped);
        assert_eq!(server.pointer_layer(10, 635), 2);
        assert!(!server.update_taskbar_pointer(400, 620));
        assert!(server.windows[&WM_TASKBAR.0].mapped);

        assert!(server.update_taskbar_pointer(400, 580));
        assert!(!server.windows[&WM_TASKBAR.0].mapped);

        assert!(server.set_taskbar_configuration(true, 1, true));
        assert!(server.update_taskbar_pointer(400, 6));
        assert!(server.windows[&WM_TASKBAR.0].mapped);
        assert_eq!(server.windows[&WM_TASKBAR.0].y, 0);
    }

    #[test]
    fn auto_hide_keeps_menu_open_and_rechecks_on_dismissal() {
        let mut server = DisplayServer::new(DisplayConfig {width_px:1024,height_px:640,native_window_manager:true,..DisplayConfig::default()});
        server.set_taskbar_configuration(true,0,true);
        server.update_taskbar_pointer(400,635);
        server.handle_wm_shortcut(133,0);
        server.update_taskbar_pointer(400,300);
        assert!(server.windows[&WM_MENU.0].mapped);
        assert!(server.windows[&WM_TASKBAR.0].mapped);
        server.dismiss_wm_menu();
        assert!(!server.windows[&WM_TASKBAR.0].mapped);
        server.handle_wm_shortcut(133,0);
        assert!(server.windows[&WM_TASKBAR.0].mapped,"keyboard menu reveals dock");
        server.update_taskbar_pointer(400,635);
        server.dismiss_wm_menu();
        assert!(server.windows[&WM_TASKBAR.0].mapped,"pointer still on dock");
        server.update_taskbar_pointer(400,300);
        assert!(!server.windows[&WM_TASKBAR.0].mapped);
    }

    #[test]
    fn taskbar_supports_every_docked_edge_and_reserves_the_matching_workarea() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 800,
            height_px: 600,
            native_window_manager: true,
            ..DisplayConfig::default()
        });
        for (edge, expected_taskbar, expected_workarea) in [
            (0, (0, 568, 800, 32), (0, 0, 800, 568)),
            (1, (0, 0, 800, 32), (0, 32, 800, 568)),
            (2, (0, 0, 32, 600), (32, 0, 768, 600)),
            (3, (768, 0, 32, 600), (0, 0, 768, 600)),
        ] {
            assert!(server.set_taskbar_configuration(true, edge, false));
            let taskbar = &server.windows[&WM_TASKBAR.0];
            assert_eq!(
                (taskbar.x, taskbar.y, taskbar.width, taskbar.height),
                expected_taskbar,
            );
            assert_eq!(server.wm_workarea_rect(), expected_workarea);
        }
        assert!(!server.set_taskbar_configuration(true, 4, false));
    }

    #[test]
    fn dock_context_menus_open_dock_settings_on_every_edge() {
        for edge in [WmTaskbarEdge::Bottom, WmTaskbarEdge::Top, WmTaskbarEdge::Left, WmTaskbarEdge::Right] {
            let mut server = DisplayServer::new(DisplayConfig {
                width_px: 800,
                height_px: 600,
                native_window_manager: true,
                ..DisplayConfig::default()
            });
            server.wm_dock_style = true;
            server.set_taskbar_configuration(true, edge.wire() as u8, false);
            for level in [
                WmMenuLevel::TaskbarContext,
                WmMenuLevel::QuickLaunchContext,
                WmMenuLevel::TaskContext(WmTaskTarget::Group(0)),
                WmMenuLevel::TaskContext(WmTaskTarget::Native(1)),
                WmMenuLevel::MenuDirectoryContext,
            ] {
                server.show_wm_task_menu(level, 400, 300);
                let rows = server.wm_menu_columns().pop().unwrap();
                let index = rows.iter().position(|row| *row == WmMenuRow::DockSettings)
                    .expect("dock context menu must offer settings");
                let menu = server.windows[&WM_MENU.0].clone();
                let y = menu.y + (wm_menu_row_top(&rows, index)
                    + wm_menu_row_height(&rows[index]) / 2) as i16;
                server.inject_pointer_button(true, 1, menu.x + 20, y, 0);
                assert_eq!(server.drain_wm_action(), Some(WM_ACTION_TOOLBAR_PREFERENCES));
                assert!(!server.windows[&WM_MENU.0].mapped);
                server.inject_pointer_button(false, 1, menu.x + 20, y, 0);
            }
        }
    }

    #[test]
    fn side_taskbar_context_menu_opens_beside_the_click_and_never_overlaps_the_bar() {
        for edge in [WmTaskbarEdge::Left, WmTaskbarEdge::Right] {
            let mut server = DisplayServer::new(DisplayConfig {
                width_px: 800,
                height_px: 600,
                native_window_manager: true,
                ..DisplayConfig::default()
            });
            assert!(server.set_taskbar_configuration(true, edge.wire() as u8, false));
            let taskbar = server.windows[&WM_TASKBAR.0].clone();
            let click_x = taskbar.x.saturating_add_unsigned(taskbar.width / 2);
            let click_y = 300;
            server.inject_pointer_button(true, 3, click_x, click_y, 0);

            assert_eq!(server.wm_menu_level, WmMenuLevel::TaskbarContext);
            let menu = server.windows[&WM_MENU.0].clone();
            assert!(menu.mapped);
            assert_eq!(menu.y, click_y);
            if edge == WmTaskbarEdge::Left {
                assert_eq!(menu.x, taskbar.x.saturating_add_unsigned(taskbar.width) + 4,);
            } else {
                assert_eq!(menu.x.saturating_add_unsigned(menu.width) + 4, taskbar.x,);
            }

            server.show_wm_task_menu(WmMenuLevel::TaskbarContext, click_x, 590);
            let clamped = &server.windows[&WM_MENU.0];
            assert_eq!(
                clamped.y,
                i16::try_from(server.config.height_px - clamped.height).unwrap(),
                "a low side-taskbar click must keep the whole context menu on screen",
            );
        }
    }
