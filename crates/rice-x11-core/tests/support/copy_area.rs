// Included in server::tests so the scalar oracle can inspect retained pixels.
#[test]
fn copy_area_matches_snapshot_oracle_for_overlap_masks_clipping_and_depths() {
    let order = ClientByteOrder::LittleEndian;
    let (mut server, client) = ready_server(order);
    let src_id = ResourceId(0x0020_0900);
    let dst_id = ResourceId(0x0020_0901);
    let mask_id = ResourceId(0x0020_0902);
    let gc_id = ResourceId(0x0020_0903);
    let mut random = 0x5721_0abcu32;
    let mut cases = 0;
    for depth in [1, 8, 24, 32] {
        for overlap in [0, 1, 2] {
            for materialized in [false, true] {
                for clip in 0..4 {
                    for plane_mask in [u32::MAX, 0x0055_aa55, 0] {
                        for function in 0..16 {
                            let dst_id = if overlap == 0 { dst_id } else { src_id };
                            for id in [src_id, dst_id] {
                                let mut surface = Surface::new(17, 13, depth, 0x1234_5678);
                                if materialized {
                                    surface.pixels = Some(
                                        (0..17 * 13)
                                            .map(|_| {
                                                random ^= random << 13;
                                                random ^= random >> 17;
                                                random ^= random << 5;
                                                random
                                            })
                                            .collect(),
                                    );
                                }
                                server.surfaces.insert(id.0, surface);
                            }
                            let mut gc = GraphicsContext::new(client.0, depth);
                            gc.function = function;
                            gc.plane_mask = plane_mask;
                            gc.graphics_exposures = cases % 2 == 0;
                            gc.clip_x_origin = -1;
                            gc.clip_y_origin = 2;
                            if clip == 1 || clip == 2 {
                                let mut rectangles = Vec::new();
                                if clip == 1 {
                                    for value in [2u16, 0, 6, 8] {
                                        push_u16(order, &mut rectangles, value);
                                    }
                                }
                                gc.clip_rectangles = Some(x11::ClipRectangles {
                                    ordering: 0,
                                    x_origin: -1,
                                    y_origin: 2,
                                    rectangles,
                                });
                            } else if clip == 3 {
                                let mut mask = Surface::new(17, 13, 1, 0);
                                mask.pixels =
                                    Some((0..17 * 13).map(|i| u32::from(i % 3 == 0)).collect());
                                server.surfaces.insert(mask_id.0, mask);
                                gc.clip_mask = Some(mask_id);
                            }
                            let source = server.surfaces[&src_id.0].clone();
                            let destination = server.surfaces[&dst_id.0].clone();
                            let mut expected: Vec<u32> = (0..13)
                                .flat_map(|y| {
                                    let destination = &destination;
                                    (0..17).map(move |x| destination.pixel(x, y))
                                })
                                .collect();
                            let request = x11::CopyAreaRequest {
                                src: src_id,
                                dst: dst_id,
                                gc: gc_id,
                                src_x: if overlap == 2 { 3 } else { -2 },
                                src_y: if overlap == 2 { 2 } else { 0 },
                                dst_x: if overlap == 2 { 0 } else { 2 },
                                dst_y: if overlap == 2 { -1 } else { 2 },
                                width: 19,
                                height: 14,
                            };
                            let (_, rectangle) =
                                clipped_copy_rectangles(&source, &destination, &request).unwrap();
                            let coverage = server.gc_clip_coverage(&gc, rectangle);
                            // Read from the frozen source even for either direction
                            // of overlapping self-copy; never read modified pixels.
                            for y in 0..request.height {
                                for x in 0..request.width {
                                    let sx = i32::from(request.src_x) + i32::from(x);
                                    let sy = i32::from(request.src_y) + i32::from(y);
                                    let dx = i32::from(request.dst_x) + i32::from(x);
                                    let dy = i32::from(request.dst_y) + i32::from(y);
                                    if !(0..17).contains(&sx)
                                        || !(0..13).contains(&sy)
                                        || !(0..17).contains(&dx)
                                        || !(0..13).contains(&dy)
                                    {
                                        continue;
                                    }
                                    let covered = (dy as usize - usize::from(rectangle.y))
                                        * usize::from(rectangle.width)
                                        + dx as usize
                                        - usize::from(rectangle.x);
                                    if !coverage[covered] {
                                        continue;
                                    }
                                    let index = dy as usize * 17 + dx as usize;
                                    expected[index] = apply_gc_function(
                                        expected[index],
                                        source.pixel(sx as u16, sy as u16),
                                        function,
                                        plane_mask,
                                        depth_plane_mask(depth),
                                    );
                                }
                            }
                            let exposures = gc.graphics_exposures;
                            server.graphics_contexts.insert(gc_id.0, gc);
                            let _ = drain_all(&mut server, client);
                            let mut body = Vec::new();
                            for id in [src_id, dst_id, gc_id] {
                                push_u32(order, &mut body, id.0);
                            }
                            for value in
                                [request.src_x, request.src_y, request.dst_x, request.dst_y]
                            {
                                push_u16(order, &mut body, value as u16);
                            }
                            push_u16(order, &mut body, request.width);
                            push_u16(order, &mut body, request.height);
                            server.copy_area(client, &body).unwrap();
                            assert_eq!(
                                server.surfaces[&dst_id.0].pixels.as_ref().unwrap(),
                                &expected,
                                "case {cases}: depth={depth} overlap={overlap} materialized={materialized} clip={clip} plane={plane_mask:#x} function={function}"
                            );
                            let reply = drain_all(&mut server, client);
                            assert_eq!(reply.len(), if exposures { 32 } else { 0 });
                            if exposures {
                                assert_eq!(reply[0], 14);
                            }
                            let _ = drain_scene(&mut server);
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 4608);
}

#[test]
fn copy_area_scene_damage_follows_distant_gc_clip_rectangles() {
    let order = ClientByteOrder::LittleEndian;
    let (mut server, client) = ready_server(order);
    let window = 0x0020_0910;
    let gc_id = 0x0020_0911;
    server
        .feed(client, &create_window_request(order, window, 0))
        .unwrap();
    let mut gc_body = Vec::new();
    push_u32(order, &mut gc_body, gc_id);
    push_u32(order, &mut gc_body, window);
    push_u32(order, &mut gc_body, 0);
    server
        .feed(client, &request(order, 55, 0, &gc_body))
        .unwrap();
    let _ = drain_scene(&mut server);
    let mut rectangles = Vec::new();
    for (x, y) in [(8u16, 17u16), (238, 137)] {
        for value in [x, y, 8, 6] {
            push_u16(order, &mut rectangles, value);
        }
    }
    let gc = server.graphics_contexts.get_mut(&gc_id).unwrap();
    gc.clip_x_origin = 2;
    gc.clip_y_origin = 3;
    gc.clip_rectangles = Some(x11::ClipRectangles {
        ordering: 0,
        x_origin: 2,
        y_origin: 3,
        rectangles,
    });
    let mut body = Vec::new();
    for id in [window, window, gc_id] {
        push_u32(order, &mut body, id);
    }
    for value in [0u16, 0, 0, 0, 300, 180] {
        push_u16(order, &mut body, value);
    }
    server.copy_area(client, &body).unwrap();
    let scene = drain_scene(&mut server);
    let damages: Vec<(i32, i32, u16, u16)> = scene
        .chunks_exact(SCENE_COMMAND_BYTES)
        .filter(|command| command[0] == SCENE_DAMAGE_WINDOW)
        .map(|command| {
            (
                i32::from_le_bytes(command[12..16].try_into().unwrap()),
                i32::from_le_bytes(command[16..20].try_into().unwrap()),
                u16::from_le_bytes(command[20..22].try_into().unwrap()),
                u16::from_le_bytes(command[22..24].try_into().unwrap()),
            )
        })
        .collect();
    assert_eq!(damages, [(10, 20, 8, 6), (240, 140, 8, 6)]);
}

#[test]
fn copy_area_sparse_distinct_surfaces_preserves_pixels_outside_both_clips() {
    let order = ClientByteOrder::LittleEndian;
    let (mut server, client) = ready_server(order);
    let source_id = ResourceId(0x0020_0920);
    let destination_id = ResourceId(0x0020_0921);
    let gc_id = ResourceId(0x0020_0922);
    for source_materialized in [false, true] {
        let mut source = Surface::new(256, 160, 24, 0x0012_3456);
        if source_materialized {
            source.pixels = Some(
                (0..256 * 160)
                    .map(|index| 0x00ab_0000 | (index as u32 & 0xffff))
                    .collect(),
            );
        }
        let mut destination = Surface::new(256, 160, 24, 0x0077_8899);
        destination.materialize();
        let mut expected = destination.pixels.as_ref().unwrap().clone();
        for (left, top) in [(10usize, 20usize), (200, 120)] {
            for y in top..top + 8 {
                for x in left..left + 8 {
                    expected[y * 256 + x] = source.pixel(x as u16, y as u16);
                }
            }
        }
        server.surfaces.insert(source_id.0, source);
        server.surfaces.insert(destination_id.0, destination);
        let mut gc = GraphicsContext::new(client.0, 24);
        let mut rectangles = Vec::new();
        for (left, top) in [(10u16, 20u16), (200, 120)] {
            for value in [left, top, 8, 8] {
                push_u16(order, &mut rectangles, value);
            }
        }
        gc.clip_rectangles = Some(x11::ClipRectangles {
            ordering: 0,
            x_origin: 0,
            y_origin: 0,
            rectangles,
        });
        server.graphics_contexts.insert(gc_id.0, gc);
        let mut body = Vec::new();
        for id in [source_id, destination_id, gc_id] {
            push_u32(order, &mut body, id.0);
        }
        for value in [0u16, 0, 0, 0, 256, 160] {
            push_u16(order, &mut body, value);
        }
        server.copy_area(client, &body).unwrap();
        assert_eq!(
            server.surfaces[&destination_id.0].pixels.as_ref().unwrap(),
            &expected,
            "sparse CopyArea changed pixels outside its clips (source materialized={source_materialized})"
        );
    }
}
