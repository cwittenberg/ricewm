use super::*;


    #[test]
    fn depth_32_put_image_preserves_argb_alpha() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let pixmap = 0x0020_0160;
        let gc = 0x0020_0161;
        let argb = 0x8040_2010;

        let mut pixmap_body = Vec::new();
        push_u32(order, &mut pixmap_body, pixmap);
        push_u32(order, &mut pixmap_body, ROOT_WINDOW.0);
        push_u16(order, &mut pixmap_body, 1);
        push_u16(order, &mut pixmap_body, 1);
        server
            .feed(client, &request(order, 53, 32, &pixmap_body))
            .unwrap();

        let mut gc_body = Vec::new();
        push_u32(order, &mut gc_body, gc);
        push_u32(order, &mut gc_body, pixmap);
        push_u32(order, &mut gc_body, 0);
        server
            .feed(client, &request(order, 55, 0, &gc_body))
            .unwrap();

        let mut image = Vec::new();
        push_u32(order, &mut image, pixmap);
        push_u32(order, &mut image, gc);
        push_u16(order, &mut image, 1);
        push_u16(order, &mut image, 1);
        push_u16(order, &mut image, 0);
        push_u16(order, &mut image, 0);
        image.push(0);
        image.push(32);
        image.extend_from_slice(&[0, 0]);
        push_u32(order, &mut image, argb);
        server.feed(client, &request(order, 72, 2, &image)).unwrap();

        assert!(drain_all(&mut server, client).is_empty());
        assert_eq!(server.surfaces[&pixmap].pixel(0, 0), argb);
        let mut rgba = [0; 4];
        assert_eq!(
            server.read_surface_rgba(pixmap, 0, 0, 1, 1, &mut rgba),
            Some(4)
        );
        assert_eq!(rgba, [0x40, 0x20, 0x10, 0x80]);
    }

    #[test]
    fn depth_32_xrgb_pixels_with_an_unused_high_byte_render_opaque() {
        let mut server = DisplayServer::new(DisplayConfig::default());
        let drawable = 0x0020_0170;
        let mut surface = Surface::new(2, 1, 32, 0);
        assert!(surface.materialize());
        surface.pixels = Some(vec![0x00ff_ffff, 0x0000_0000]);
        server.surfaces.insert(drawable, surface);

        let mut rgba = [0; 8];
        assert_eq!(
            server.read_surface_rgba(drawable, 0, 0, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(rgba, [255, 255, 255, 255, 0, 0, 0, 255]);

        // Once meaningful XRGB is observed, later black drawing primitives
        // remain opaque instead of disappearing into the parent surface.
        server.surfaces.get_mut(&drawable).unwrap().pixels = Some(vec![0x0000_0000, 0x0000_0000]);
        assert_eq!(
            server.read_surface_rgba(drawable, 0, 0, 2, 1, &mut rgba),
            Some(8)
        );
        assert_eq!(rgba, [0, 0, 0, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn xy_pixmap_upload_and_pixmap_cursor_are_accepted() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let pixmap = 0x0020_0180;
        let gc = 0x0020_0181;
        let cursor = 0x0020_0182;

        let mut pixmap_body = Vec::new();
        push_u32(order, &mut pixmap_body, pixmap);
        push_u32(order, &mut pixmap_body, ROOT_WINDOW.0);
        push_u16(order, &mut pixmap_body, 1);
        push_u16(order, &mut pixmap_body, 1);
        server
            .feed(client, &request(order, 53, 1, &pixmap_body))
            .unwrap();

        let mut gc_body = Vec::new();
        push_u32(order, &mut gc_body, gc);
        push_u32(order, &mut gc_body, pixmap);
        push_u32(order, &mut gc_body, 0);
        server
            .feed(client, &request(order, 55, 0, &gc_body))
            .unwrap();

        let mut image = Vec::new();
        push_u32(order, &mut image, pixmap);
        push_u32(order, &mut image, gc);
        push_u16(order, &mut image, 1);
        push_u16(order, &mut image, 1);
        push_u16(order, &mut image, 0);
        push_u16(order, &mut image, 0);
        image.push(0);
        image.push(1);
        image.extend_from_slice(&[0, 0]);
        // One depth-1 XYPixmap plane, padded to the advertised 32-bit
        // scanline boundary. The only pixel is set.
        image.extend_from_slice(&[1, 0, 0, 0]);
        server.feed(client, &request(order, 72, 1, &image)).unwrap();
        assert!(drain_all(&mut server, client).is_empty());
        assert_eq!(server.surfaces[&pixmap].pixel(0, 0), 1);

        let mut cursor_body = Vec::new();
        push_u32(order, &mut cursor_body, cursor);
        push_u32(order, &mut cursor_body, pixmap);
        push_u32(order, &mut cursor_body, pixmap);
        // GTK uses the source-edge (1,1) hotspot for this 1x1 invisible
        // cursor. This exact sequence previously terminated PCManFM.
        for component in [u16::MAX, u16::MAX, u16::MAX, 0, 0, 0, 1, 1] {
            push_u16(order, &mut cursor_body, component);
        }
        server
            .feed(client, &request(order, 93, 0, &cursor_body))
            .unwrap();
        assert!(drain_all(&mut server, client).is_empty());
        assert!(server.cursors.contains_key(&cursor));
    }

    #[test]
    fn render_animated_cursor_used_by_gtk_is_accepted() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let first = 0x0020_0190;
        let second = 0x0020_0191;
        let animated = 0x0020_0192;
        server.cursors.insert(first, RiceCursor::new(client.0, 0));
        server
            .cursors
            .insert(second, RiceCursor::new(client.0, 0));

        let mut body = Vec::new();
        push_u32(order, &mut body, animated);
        push_u32(order, &mut body, first);
        push_u32(order, &mut body, 50);
        push_u32(order, &mut body, second);
        push_u32(order, &mut body, 75);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 31, &body))
            .unwrap();

        assert!(drain_all(&mut server, client).is_empty());
        assert!(server.cursors.contains_key(&animated));

        let mut invalid = Vec::new();
        push_u32(order, &mut invalid, 0x0020_0193);
        push_u32(order, &mut invalid, 0x0020_ffff);
        push_u32(order, &mut invalid, 50);
        server
            .feed(client, &request(order, RENDER_MAJOR_OPCODE, 31, &invalid))
            .unwrap();
        let error = drain_all(&mut server, client);
        assert_eq!(error.len(), 32);
        assert_eq!(error[0], 0);
        assert_eq!(error[1], x11::error::BAD_CURSOR);
        assert_eq!(error[10], RENDER_MAJOR_OPCODE);
        assert_eq!(u16::from_le_bytes(error[8..10].try_into().unwrap()), 31);
    }

    #[test]
    fn gc_state_clips_pixels_and_get_image_returns_real_z_and_xy_content() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0400;
        let gc = 0x0020_0401;
        let copied_gc = 0x0020_0402;

        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        let mut gc_body = Vec::new();
        push_u32(order, &mut gc_body, gc);
        push_u32(order, &mut gc_body, window);
        push_u32(order, &mut gc_body, 1 << 2);
        push_u32(order, &mut gc_body, 0x00ff_0000);
        server
            .feed(client, &request(order, 55, 0, &gc_body))
            .unwrap();

        let mut copied_gc_body = Vec::new();
        push_u32(order, &mut copied_gc_body, copied_gc);
        push_u32(order, &mut copied_gc_body, window);
        push_u32(order, &mut copied_gc_body, 0);
        server
            .feed(client, &request(order, 55, 0, &copied_gc_body))
            .unwrap();
        let mut copy_gc = Vec::new();
        push_u32(order, &mut copy_gc, gc);
        push_u32(order, &mut copy_gc, copied_gc);
        push_u32(order, &mut copy_gc, 1 << 2);
        server
            .feed(client, &request(order, 57, 0, &copy_gc))
            .unwrap();
        assert_eq!(server.graphics_contexts[&copied_gc].foreground, 0x00ff_0000);

        let mut dashes = Vec::new();
        push_u32(order, &mut dashes, gc);
        push_u16(order, &mut dashes, 7);
        push_u16(order, &mut dashes, 3);
        dashes.extend_from_slice(&[2, 3, 4, 0]);
        server
            .feed(client, &request(order, 58, 0, &dashes))
            .unwrap();
        assert_eq!(server.graphics_contexts[&gc].dash_offset, 7);
        assert_eq!(server.graphics_contexts[&gc].dashes, [2, 3, 4, 2, 3, 4]);

        let mut clip = Vec::new();
        push_u32(order, &mut clip, gc);
        push_u16(order, &mut clip, 0);
        push_u16(order, &mut clip, 0);
        push_u16(order, &mut clip, 1);
        push_u16(order, &mut clip, 0);
        push_u16(order, &mut clip, 2);
        push_u16(order, &mut clip, 1);
        server.feed(client, &request(order, 59, 0, &clip)).unwrap();

        let mut fill = Vec::new();
        push_u32(order, &mut fill, window);
        push_u32(order, &mut fill, gc);
        push_u16(order, &mut fill, 0);
        push_u16(order, &mut fill, 0);
        push_u16(order, &mut fill, 4);
        push_u16(order, &mut fill, 2);
        server.feed(client, &request(order, 70, 0, &fill)).unwrap();

        let mut rgba = [0; 32];
        assert_eq!(
            server.read_surface_rgba(window, 0, 0, 4, 2, &mut rgba),
            Some(32)
        );
        assert_eq!(&rgba[4..12], &[255, 0, 0, 255, 255, 0, 0, 255]);
        assert_eq!(&rgba[0..4], &[34, 51, 68, 255]);
        assert_eq!(&rgba[16..20], &[34, 51, 68, 255]);

        let mut get = Vec::new();
        push_u32(order, &mut get, window);
        push_u16(order, &mut get, 0);
        push_u16(order, &mut get, 0);
        push_u16(order, &mut get, 4);
        push_u16(order, &mut get, 2);
        push_u32(order, &mut get, u32::MAX);
        server.feed(client, &request(order, 73, 2, &get)).unwrap();
        let z_reply = drain_all(&mut server, client);
        assert_eq!(z_reply.len(), 64);
        assert_eq!(z_reply[1], 24);
        assert_eq!(
            u32::from_le_bytes(z_reply[36..40].try_into().unwrap()),
            0x00ff_0000
        );

        get[12..16].copy_from_slice(&0x0001_0000u32.to_le_bytes());
        server.feed(client, &request(order, 73, 1, &get)).unwrap();
        let xy_reply = drain_all(&mut server, client);
        assert_eq!(xy_reply.len(), 40);
        assert_eq!(xy_reply[32], 0b0000_0110);
        assert_eq!(xy_reply[36], 0);
    }

    #[test]
    fn gc_clip_rectangles_follow_changed_gc_origin() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_0410;
        let gc = 0x0020_0411;

        server
            .feed(client, &create_window_request(order, window, 0))
            .unwrap();
        let mut gc_body = Vec::new();
        push_u32(order, &mut gc_body, gc);
        push_u32(order, &mut gc_body, window);
        push_u32(order, &mut gc_body, 1 << 2);
        push_u32(order, &mut gc_body, 0x00ff_0000);
        server
            .feed(client, &request(order, 55, 0, &gc_body))
            .unwrap();

        let mut clip = Vec::new();
        push_u32(order, &mut clip, gc);
        push_u16(order, &mut clip, 0);
        push_u16(order, &mut clip, 0);
        push_u16(order, &mut clip, 1);
        push_u16(order, &mut clip, 1);
        push_u16(order, &mut clip, 2);
        push_u16(order, &mut clip, 2);
        server.feed(client, &request(order, 59, 0, &clip)).unwrap();

        // Xlib installs a region once and subsequently moves it for each
        // child drawable by changing the GC clip origin. The current origin,
        // not the coordinates carried by the old SetClipRectangles request,
        // defines where that region clips future drawing.
        let mut change_gc = Vec::new();
        push_u32(order, &mut change_gc, gc);
        push_u32(order, &mut change_gc, (1 << 17) | (1 << 18));
        push_i32(order, &mut change_gc, 4);
        push_i32(order, &mut change_gc, 3);
        server
            .feed(client, &request(order, 56, 0, &change_gc))
            .unwrap();

        let mut fill = Vec::new();
        push_u32(order, &mut fill, window);
        push_u32(order, &mut fill, gc);
        push_u16(order, &mut fill, 0);
        push_u16(order, &mut fill, 0);
        push_u16(order, &mut fill, 10);
        push_u16(order, &mut fill, 10);
        server.feed(client, &request(order, 70, 0, &fill)).unwrap();

        let surface = &server.surfaces[&window];
        assert_eq!(surface.pixel(1, 1) & 0x00ff_ffff, 0x0022_3344);
        assert_eq!(surface.pixel(5, 4) & 0x00ff_ffff, 0x00ff_0000);
        assert_eq!(surface.pixel(6, 5) & 0x00ff_ffff, 0x00ff_0000);
        assert_eq!(surface.pixel(7, 5) & 0x00ff_ffff, 0x0022_3344);
        assert!(server.gc_pixel_visible(&server.graphics_contexts[&gc], 5, 4));
        assert!(!server.gc_pixel_visible(&server.graphics_contexts[&gc], 1, 1));
    }

    #[test]
    fn big_requests_enables_extended_framing() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        server
            .feed(client, &request(order, BIG_REQUESTS_MAJOR_OPCODE, 0, &[]))
            .unwrap();
        let reply = drain_all(&mut server, client);
        assert_eq!(reply.len(), 32);
        assert_eq!(reply[0], 1);
        assert_eq!(
            u32::from_le_bytes(reply[8..12].try_into().unwrap()),
            u32::try_from(server.config.max_request_bytes / 4).unwrap()
        );

        let mut extended_noop = vec![127, 0, 0, 0];
        push_u32(order, &mut extended_noop, 2);
        server.feed(client, &extended_noop).unwrap();
        assert_eq!(server.status(client).unwrap(), ClientStatus::Ready);
        assert!(drain_all(&mut server, client).is_empty());
    }

    #[test]
    fn big_requests_change_property_uses_semantic_core_length() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let window = 0x0020_04f0;
        server
            .feed(
                client,
                &create_window_request(order, window, PROPERTY_CHANGE_MASK),
            )
            .unwrap();
        server
            .feed(client, &intern_atom_request(order, "_RICE_LARGE_PROPERTY"))
            .unwrap();
        let atom_reply = drain_all(&mut server, client);
        let property = u32::from_le_bytes(atom_reply[8..12].try_into().unwrap());

        server
            .feed(client, &request(order, BIG_REQUESTS_MAJOR_OPCODE, 0, &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        // Cross the 16-bit core request limit so this has the same extended
        // wire shape as GTK's large _NET_WM_ICON property request.
        let value = vec![0x5a; 256 * 1024];
        let mut change = Vec::with_capacity(20 + value.len());
        push_u32(order, &mut change, window);
        push_u32(order, &mut change, property);
        push_u32(order, &mut change, 31);
        change.push(8);
        change.extend_from_slice(&[0; 3]);
        push_u32(order, &mut change, u32::try_from(value.len()).unwrap());
        change.extend_from_slice(&value);
        change.resize(x11::pad4(change.len()), 0);

        server
            .feed(client, &extended_request(order, 18, 0, &change))
            .unwrap();
        let response = drain_all(&mut server, client);
        assert_eq!(response.len(), 32);
        assert_eq!(response[0], 28, "large property must emit PropertyNotify");
        assert_eq!(
            server.windows[&window].properties[&AtomId(property)].data,
            value
        );
    }

    #[test]
    fn big_requests_put_image_preserves_complete_depth_24_payload() {
        let order = ClientByteOrder::LittleEndian;
        let (mut server, client) = ready_server(order);
        let drawable = 0x0020_04f1;
        let gc = 0x0020_04f2;
        const WIDTH: u16 = 1180;
        const HEIGHT: u16 = 688;

        server.surfaces.insert(
            drawable,
            Surface::new(WIDTH, HEIGHT, 24, client.0));
        server
            .graphics_contexts
            .insert(gc, GraphicsContext::new(client.0, 24));
        server
            .feed(client, &request(order, BIG_REQUESTS_MAJOR_OPCODE, 0, &[]))
            .unwrap();
        let _ = drain_all(&mut server, client);

        let mut image = Vec::with_capacity(20 + usize::from(WIDTH) * usize::from(HEIGHT) * 4);
        push_u32(order, &mut image, drawable);
        push_u32(order, &mut image, gc);
        push_u16(order, &mut image, WIDTH);
        push_u16(order, &mut image, HEIGHT);
        push_u16(order, &mut image, 0);
        push_u16(order, &mut image, 0);
        image.push(0);
        image.push(24);
        image.extend_from_slice(&[0, 0]);
        for row in 0..HEIGHT {
            for column in 0..WIDTH {
                push_u32(
                    order,
                    &mut image,
                    0x0001_0000 | (u32::from(row) << 8) | u32::from(column & 0xff),
                );
            }
        }

        server
            .feed(client, &extended_request(order, 72, 2, &image))
            .unwrap();
        let response = drain_all(&mut server, client);
        assert!(response.is_empty(), "unexpected X11 response: {response:?}");
        assert_eq!(server.surfaces[&drawable].pixel(0, 0), 0x0001_0000);
        assert_eq!(
            server.surfaces[&drawable].pixel(WIDTH - 1, HEIGHT - 1),
            0x0001_0000 | (u32::from(HEIGHT - 1) << 8) | u32::from((WIDTH - 1) & 0xff)
        );
    }
