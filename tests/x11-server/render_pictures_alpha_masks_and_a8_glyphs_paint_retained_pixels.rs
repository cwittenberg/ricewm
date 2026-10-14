use super::*;


    #[test]
    #[allow(clippy::too_many_lines)]
    fn render_pictures_alpha_masks_and_a8_glyphs_paint_retained_pixels() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0500;
        let destination_picture = 0x0020_0501;
        let solid_picture = 0x0020_0502;
        let glyphset = 0x0020_0503;
        let glyphset_alias = 0x0020_0504;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();

        let mut create_picture = Vec::new();
        push_u32(order, &mut create_picture, destination_picture);
        push_u32(order, &mut create_picture, window);
        push_u32(order, &mut create_picture, x11::RENDER_FMT_RGB24);
        push_u32(order, &mut create_picture, 0);
        server
            .feed(
                client,
                &request(order, RENDER_MAJOR_OPCODE, 4, &create_picture),
            )
            .unwrap();

        let mut solid = Vec::new();
        push_u32(order, &mut solid, solid_picture);
        for component in [u16::MAX, 0, 0, u16::MAX] {
            push_u16(order, &mut solid, component);
        }
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 33, &solid))
            .unwrap();

        let mut composite = vec![3, 0, 0, 0];
        push_u32(order, &mut composite, solid_picture);
        push_u32(order, &mut composite, 0);
        push_u32(order, &mut composite, destination_picture);
        for coordinate in [0u16, 0, 0, 0, 2, 3] {
            push_u16(order, &mut composite, coordinate);
        }
        push_u16(order, &mut composite, 2);
        push_u16(order, &mut composite, 1);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 8, &composite))
            .unwrap();
        let mut rgba = [0; 8];
        assert_eq!(
            server.read_surface_rgba(window, 2, 3, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(rgba, [255, 0, 0, 255, 255, 0, 0, 255]);

        let mut create_glyphset = Vec::new();
        push_u32(order, &mut create_glyphset, glyphset);
        push_u32(order, &mut create_glyphset, x11::RENDER_FMT_A8);
        server
            .feed(
                client,
                &request(order, RENDER_MAJOR_OPCODE, 17, &create_glyphset),
            )
            .unwrap();
        // Xft commonly references a glyph set before uploading glyphs. The
        // alias must observe later AddGlyphs/FreeGlyphs operations instead of
        // retaining a one-time clone of the original map.
        let mut reference_glyphset = Vec::new();
        push_u32(order, &mut reference_glyphset, glyphset_alias);
        push_u32(order, &mut reference_glyphset, glyphset);
        server
            .feed(
                client,
                &request(order, RENDER_MAJOR_OPCODE, 18, &reference_glyphset),
            )
            .unwrap();
        let mut add_glyph = Vec::new();
        push_u32(order, &mut add_glyph, glyphset);
        push_u32(order, &mut add_glyph, 1);
        push_u32(order, &mut add_glyph, 7);
        push_u16(order, &mut add_glyph, 2);
        push_u16(order, &mut add_glyph, 1);
        push_u16(order, &mut add_glyph, 0);
        push_u16(order, &mut add_glyph, 0);
        push_u16(order, &mut add_glyph, 2);
        push_u16(order, &mut add_glyph, 0);
        add_glyph.extend_from_slice(&[255, 128, 0, 0]);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 20, &add_glyph))
            .unwrap();

        let mut glyphs = vec![3, 0, 0, 0];
        push_u32(order, &mut glyphs, solid_picture);
        push_u32(order, &mut glyphs, destination_picture);
        push_u32(order, &mut glyphs, x11::RENDER_FMT_A8);
        push_u32(order, &mut glyphs, glyphset_alias);
        push_u16(order, &mut glyphs, 0);
        push_u16(order, &mut glyphs, 0);
        glyphs.extend_from_slice(&[1, 0, 0, 0]);
        push_u16(order, &mut glyphs, 4);
        push_u16(order, &mut glyphs, 5);
        glyphs.extend_from_slice(&[7, 0, 0, 0]);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 23, &glyphs))
            .unwrap();
        assert_eq!(
            server.read_surface_rgba(window, 4, 5, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
        assert!(rgba[4] > 140 && rgba[4] < 150);
        let mut free_original = Vec::new();
        push_u32(order, &mut free_original, glyphset);
        server
            .feed(
                client,
                &request(order, RENDER_MAJOR_OPCODE, 19, &free_original),
            )
            .unwrap();
        assert!(server.glyphsets.contains_key(&glyphset_alias));
        assert!(
            server
                .glyph_storage
                .values()
                .any(|glyphs| glyphs.contains_key(&7))
        );
        assert!(drain_all(&mut server, client).is_empty());
    }

    #[test]
    fn render_gradients_and_trapezoids_paint_real_pixels() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0600;
        let destination = 0x0020_0601;
        let gradient = 0x0020_0602;
        let solid = 0x0020_0603;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();

        let mut picture = Vec::new();
        push_u32(order, &mut picture, destination);
        push_u32(order, &mut picture, window);
        push_u32(order, &mut picture, x11::RENDER_FMT_RGB24);
        push_u32(order, &mut picture, 0);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 4, &picture))
            .unwrap();

        let mut linear = Vec::new();
        push_u32(order, &mut linear, gradient);
        for value in [0, 0, 10 << 16, 0] {
            push_i32(order, &mut linear, value);
        }
        push_u32(order, &mut linear, 2);
        push_i32(order, &mut linear, 0);
        push_i32(order, &mut linear, 1 << 16);
        for color in [[u16::MAX, 0, 0, u16::MAX], [0, 0, u16::MAX, u16::MAX]] {
            for component in color {
                push_u16(order, &mut linear, component);
            }
        }
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 34, &linear))
            .unwrap();

        let mut composite = vec![1, 0, 0, 0];
        push_u32(order, &mut composite, gradient);
        push_u32(order, &mut composite, 0);
        push_u32(order, &mut composite, destination);
        composite.extend_from_slice(&[0; 12]);
        push_u16(order, &mut composite, 10);
        push_u16(order, &mut composite, 1);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 8, &composite))
            .unwrap();
        let mut gradient_pixels = [0; 40];
        assert_eq!(
            server.read_surface_rgba(window, 0, 0, 10, 1, &mut gradient_pixels),
            Some(40)
        );
        assert!(gradient_pixels[0] > gradient_pixels[2]);
        assert!(gradient_pixels[36] < gradient_pixels[38]);

        let mut red = Vec::new();
        push_u32(order, &mut red, solid);
        for component in [u16::MAX, 0, 0, u16::MAX] {
            push_u16(order, &mut red, component);
        }
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 33, &red))
            .unwrap();
        let mut trapezoid = vec![1, 0, 0, 0];
        push_u32(order, &mut trapezoid, solid);
        push_u32(order, &mut trapezoid, destination);
        push_u32(order, &mut trapezoid, x11::RENDER_FMT_A8);
        push_u16(order, &mut trapezoid, 0);
        push_u16(order, &mut trapezoid, 0);
        for value in [
            20 << 16,
            24 << 16,
            20 << 16,
            20 << 16,
            20 << 16,
            24 << 16,
            24 << 16,
            20 << 16,
            24 << 16,
            24 << 16,
        ] {
            push_i32(order, &mut trapezoid, value);
        }
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 10, &trapezoid))
            .unwrap();
        let mut pixel = [0; 4];
        assert_eq!(
            server.read_surface_rgba(window, 21, 21, 1, 1, &mut pixel),
            Some(4)
        );
        assert_eq!(pixel, [255, 0, 0, 255]);
    }

    #[test]
    fn damage_composite_and_xcmisc_have_stateful_semantics() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0700;
        let damage = 0x0020_0701;
        let named_pixmap = 0x0020_0702;
        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();

        let mut create_damage = Vec::new();
        push_u32(order, &mut create_damage, damage);
        push_u32(order, &mut create_damage, window);
        create_damage.extend_from_slice(&[x11damage::report_level::BOUNDING_BOX, 0, 0, 0]);
        server
            .feed(
                client,
                &request(
                    order,
                    DAMAGE_MAJOR_OPCODE,
                    x11damage::CREATE,
                    &create_damage,
                ),
            )
            .unwrap();
        server.damage_drawable(
            ResourceId(window),
            ClippedRectangle {
                x: 3,
                y: 4,
                width: 5,
                height: 6,
            },
        );
        let event = drain_all(&mut server, client);
        assert_eq!(event[0], DAMAGE_FIRST_EVENT);
        assert_eq!(u32::from_le_bytes(event[8..12].try_into().unwrap()), damage);

        let mut redirect = Vec::new();
        push_u32(order, &mut redirect, window);
        redirect.extend_from_slice(&[1, 0, 0, 0]);
        server
            .feed(
                client,
                &request(
                    order,
                    COMPOSITE_MAJOR_OPCODE,
                    x11composite::REDIRECT_WINDOW,
                    &redirect,
                ),
            )
            .unwrap();
        let mut name = Vec::new();
        push_u32(order, &mut name, window);
        push_u32(order, &mut name, named_pixmap);
        server
            .feed(
                client,
                &request(
                    order,
                    COMPOSITE_MAJOR_OPCODE,
                    x11composite::NAME_WINDOW_PIXMAP,
                    &name,
                ),
            )
            .unwrap();
        assert_eq!(server.composite_pixmaps[&named_pixmap], ResourceId(window));
        assert_eq!(
            server.surface_geometry(named_pixmap),
            server.surface_geometry(window)
        );
        server.ensure_surface_materialized(ResourceId(named_pixmap)).unwrap();
        server.surfaces.get_mut(&named_pixmap).unwrap().pixels.as_mut().unwrap()[0] = 0x0012_3456;
        server.damage_drawable(
            ResourceId(named_pixmap),
            ClippedRectangle { x: 0, y: 0, width: 1, height: 1 ,
            },
        );
        assert_eq!(
            server.surfaces[&window].pixel(0, 0),
            0x0012_3456,
            "drawing through a named window pixmap must update its window backing storage",
        );
        let _ = drain_all(&mut server, client);

        server
            .feed(client, &request(order, XCMISC_MAJOR_OPCODE, 0, &[0; 4]))
            .unwrap();
        let version = drain_all(&mut server, client);
        assert_eq!(u16::from_le_bytes(version[8..10].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(version[10..12].try_into().unwrap()), 1);
    }

    #[test]
    fn composite_overlay_never_withholds_ricewm_pointer_input() {
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

        let mut root = Vec::new();
        push_u32(order, &mut root, ROOT_WINDOW.0);
        server
            .feed(
                client,
                &request(
                    order,
                    COMPOSITE_MAJOR_OPCODE,
                    x11composite::GET_OVERLAY_WINDOW,
                    &root,
                ),
            )
            .unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!(
            u32::from_le_bytes(reply[8..12].try_into().unwrap()),
            COMPOSITE_OVERLAY.0,
        );
        assert!(server.windows[&COMPOSITE_OVERLAY.0].mapped);

        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        assert_eq!(
            server.pointer_layer(10, taskbar_y),
            2,
            "a full-screen Composite overlay must not cover RiceWM chrome",
        );
        server.inject_pointer_button(true, 1, 10, taskbar_y, 0);
        assert!(server.windows[&WM_MENU.0].mapped);

        server
            .feed(
                client,
                &request(
                    order,
                    COMPOSITE_MAJOR_OPCODE,
                    x11composite::RELEASE_OVERLAY_WINDOW,
                    &root,
                ),
            )
            .unwrap();
        assert!(!server.windows[&COMPOSITE_OVERLAY.0].mapped);
    }

    #[test]
    fn native_tasks_share_the_taskbar_and_authoritative_workspaces() {
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
                icon: 7,
                minimized: false,
                maximized: false,
                active: true,
                title: b"RicePaper".to_vec(),
            },
            WmNativeTask {
                id: 9,
                workspace: 1,
                icon: 1,
                minimized: false,
                maximized: false,
                active: false,
                title: b"RiceFiles".to_vec(),
            },
            WmNativeTask {
                id: u16::MAX,
                workspace: 0,
                icon: 2,
                minimized: true,
                maximized: false,
                active: false,
                title: b"Rise Terminal".to_vec(),
            },
        ]));
        assert_eq!(
            server.wm_task_targets(),
            vec![WmTaskTarget::Native(7), WmTaskTarget::Native(u16::MAX)],
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| { overlay.window == WM_TASKBAR && overlay.text == b"RicePaper" })
        );
        let rice_paper_overlay = server
            .wm_text_overlays
            .iter()
            .find(|overlay| overlay.window == WM_TASKBAR && overlay.text == b"RicePaper")
            .unwrap();
        let taskbar = &server.surfaces[&WM_TASKBAR.0];
        assert!(
            (rice_paper_overlay.y..rice_paper_overlay.y + rice_paper_overlay.height).all(|y| {
                (rice_paper_overlay.x..rice_paper_overlay.x + rice_paper_overlay.width)
                    .all(|x| taskbar.pixel(x, y) & 0x00ff_ffff == 0x00e9_5420)
            }),
            "task labels must not retain a bitmap fallback beneath Canvas text"
        );

        let taskbar_y = i16::try_from(server.config.height_px - WM_TASKBAR_HEIGHT / 2).unwrap();
        server.inject_pointer_button(true, 1, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_NATIVE_TASK_PREFIX | 7),
        );

        server.switch_workspace(1);
        assert_eq!(server.current_workspace(), 1);
        assert_eq!(server.wm_task_targets(), vec![WmTaskTarget::Native(9)],);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_WORKSPACE_PREFIX | 1),
        );
        assert!(
            server
                .wm_text_overlays
                .iter()
                .any(|overlay| { overlay.window == WM_TASKBAR && overlay.text == b"RiceFiles" })
        );
        server.inject_pointer_button(true, 3, WM_TASK_BUTTON_X + 12, taskbar_y, 0);
        assert_eq!(
            server.wm_menu_level,
            WmMenuLevel::TaskContext(WmTaskTarget::Native(9)),
        );
        let menu = server.windows[&WM_MENU.0].clone();
        server.inject_pointer_button(true, 1, menu.x + 20, menu.y + 36 + 12, 0);
        assert_eq!(
            server.drain_wm_action(),
            Some(WM_ACTION_TASK_MAXIMIZE_PREFIX | 9),
        );
    }

    #[test]
    fn gnome_50_resource_strip_keeps_long_labels_and_bounds_width() {
        let mut server = DisplayServer::new(DisplayConfig {
            width_px: 1440, height_px: 900, native_window_manager: true,
            ..DisplayConfig::default()
        });
        server.configure_topbar("");
        let label = b"AB // 01 TILE CPU 12% MEM 256M UP 24K DOWN 128K THE ARCHIVE";
        server.update_gnome_panel(true, vec![WmGnomeIndicator {
            uuid: b"console".to_vec(), visible: true, has_icon: false,
            position: 0, index: 0, label: label.to_vec(), image: None,
        }]);
        let item = server.wm_topbar_layout().into_iter().find(|i| i.key == "console").unwrap();
        assert!(item.width > 400 && item.width <= 648);
        assert!(server.wm_text_overlays.iter().any(|o| o.text == label));
    }
