use super::*;

fn command(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let byte_len = COMMAND_HEADER_BYTES + payload.len();
    assert!(byte_len.is_multiple_of(4));
    let mut output = vec![opcode, 0];
    output.extend_from_slice(
        &u16::try_from(byte_len / 4)
            .expect("test command length")
            .to_le_bytes(),
    );
    output.extend_from_slice(&[0; 4]);
    output.extend_from_slice(payload);
    output
}

fn f32s(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

#[test]
fn validates_nested_retained_primitives() {
    let mut bytes = command(
        PUSH_STACKING_CONTEXT,
        &f32s(&[1.0, 0.0, 0.0, 1.0, 10.0, 20.0, 0.75]),
    );
    bytes.extend(command(PUSH_CLIP, &f32s(&[0.0; 8])));
    let mut rect = f32s(&[0.0, 0.0, 320.0, 200.0]);
    rect.extend_from_slice(&0xff33_6699u32.to_le_bytes());
    bytes.extend(command(SOLID_RECT, &rect));
    bytes.extend(command(POP_CLIP, &[]));
    bytes.extend(command(POP_STACKING_CONTEXT, &[]));

    let list = DisplayList::validate(&bytes).expect("valid display list");
    assert_eq!(list.commands().count(), 5);
}

#[test]
fn validates_bounded_radial_gradient() {
    let mut payload = f32s(&[
        0.0, 0.0, 320.0, 200.0, // paint rectangle
        160.0, -20.0, 0.0, // start circle
        160.0, -20.0, 180.0, // end circle
    ]);
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.extend_from_slice(&[0, 0]);
    payload.extend_from_slice(&0.0f32.to_le_bytes());
    payload.extend_from_slice(&0x1c_81_b9_10u32.to_le_bytes());
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    payload.extend_from_slice(&0x00_00_00_00u32.to_le_bytes());

    let bytes = command(RADIAL_GRADIENT, &payload);
    let list = DisplayList::validate(&bytes).expect("valid radial gradient");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::RadialGradient {
            start_radius: 0.0,
            end_radius: 180.0,
            ..
        }))
    ));
}

#[test]
fn validates_bounded_repeating_sweep_gradient() {
    let mut payload = f32s(&[
        0.0,
        0.0,
        320.0,
        200.0, // paint rectangle
        160.0,
        100.0, // center
        0.0,
        std::f32::consts::FRAC_PI_2, // angular period
    ]);
    payload.extend_from_slice(&[1, 0, 0, 0]); // repeat + reserved
    payload.extend_from_slice(&2u16.to_le_bytes());
    payload.extend_from_slice(&[0, 0]);
    payload.extend_from_slice(&0.0f32.to_le_bytes());
    payload.extend_from_slice(&0xff_00_00_ffu32.to_le_bytes());
    payload.extend_from_slice(&1.0f32.to_le_bytes());
    payload.extend_from_slice(&0x00_00_ffffu32.to_le_bytes());

    let bytes = command(SWEEP_GRADIENT, &payload);
    let list = DisplayList::validate(&bytes).expect("valid repeating sweep gradient");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::SweepGradient {
            extend: 1,
            start_angle: 0.0,
            ..
        }))
    ));
}

#[test]
fn rejects_unbalanced_or_non_finite_state() {
    let bytes = command(POP_CLIP, &[]);
    assert_eq!(
        DisplayList::validate(&bytes),
        Err(ProtocolError::StackUnderflow(StackKind::Clip))
    );

    let bytes = command(
        PUSH_STACKING_CONTEXT,
        &f32s(&[1.0, 0.0, 0.0, 1.0, f32::NAN, 0.0, 1.0]),
    );
    assert_eq!(
        DisplayList::validate(&bytes),
        Err(ProtocolError::InvalidNumber)
    );
}

#[test]
fn rejects_unknown_and_framebuffer_shaped_commands() {
    assert_eq!(
        DisplayList::validate(&command(0xff, &[])),
        Err(ProtocolError::UnknownCommand(0xff))
    );
    let fake_framebuffer = vec![0; 640 * 4];
    let bytes = command(IMAGE, &fake_framebuffer);
    assert_eq!(
        DisplayList::validate(&bytes),
        Err(ProtocolError::InvalidLength)
    );
}

#[test]
fn validates_only_exact_texture_resources() {
    assert!(validate_texture(2, 2, &[0; 16]).is_ok());
    assert_eq!(
        validate_texture(2, 2, &[0; 15]),
        Err(ProtocolError::InvalidTexture)
    );
    assert_eq!(
        validate_texture(MAX_TEXTURE_DIMENSION + 1, 1, &[]),
        Err(ProtocolError::InvalidTexture)
    );
}

#[test]
fn validates_retained_image_definition_and_draw() {
    let rgba = [
        255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let mut definition = 7u32.to_le_bytes().to_vec();
    definition.extend_from_slice(&2u32.to_le_bytes());
    definition.extend_from_slice(&2u32.to_le_bytes());
    definition.extend_from_slice(
        &u32::try_from(rgba.len())
            .expect("fixture byte count fits")
            .to_le_bytes(),
    );
    definition.extend_from_slice(&rgba);
    let mut bytes = command(DEFINE_IMAGE, &definition);
    let mut image = f32s(&[10.0, 20.0, 80.0, 80.0, 0.0, 0.0, 1.0, 1.0]);
    image.extend_from_slice(&7u32.to_le_bytes());
    image.extend_from_slice(&1.0f32.to_le_bytes());
    image.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend(command(IMAGE, &image));
    let list = DisplayList::validate(&bytes).expect("valid retained image");
    assert_eq!(list.commands().count(), 2);
}

#[test]
fn validates_semantic_dom_hit_region() {
    let mut payload = f32s(&[12.0, 24.0, 160.0, 28.0]);
    payload.extend_from_slice(&42u64.to_le_bytes());
    payload.extend_from_slice(&[2, 0, 0, 0]);
    let bytes = command(HIT_REGION, &payload);
    let list = DisplayList::validate(&bytes).expect("valid DOM hit region");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::HitRegion {
            node: 42,
            kind: 2,
            ..
        }))
    ));
}

#[test]
fn validates_semantic_link_region() {
    let url = b"https://example.test/next";
    let mut payload = f32s(&[12.0, 24.0, 160.0, 28.0]);
    payload.extend_from_slice(&42u64.to_le_bytes());
    payload.extend_from_slice(
        &u32::try_from(url.len())
            .expect("fixture URL length fits")
            .to_le_bytes(),
    );
    payload.extend_from_slice(url);
    payload.resize(payload.len().next_multiple_of(4), 0);
    let bytes = command(LINK_REGION, &payload);
    let list = DisplayList::validate(&bytes).expect("valid link region");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::LinkRegion {
            node: 42,
            url: b"https://example.test/next",
            ..
        }))
    ));
}

#[test]
fn validates_semantic_input_region() {
    let value = b"initial value";
    let mut payload = f32s(&[12.0, 24.0, 160.0, 28.0]);
    payload.extend_from_slice(&42u64.to_le_bytes());
    payload.extend_from_slice(&[2, 0, 0, 0]);
    payload.extend_from_slice(
        &u32::try_from(value.len())
            .expect("fixture value length fits")
            .to_le_bytes(),
    );
    payload.extend_from_slice(value);
    payload.resize(payload.len().next_multiple_of(4), 0);
    let bytes = command(INPUT_REGION, &payload);
    let list = DisplayList::validate(&bytes).expect("valid input region");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::InputRegion {
            node: 42,
            kind: 2,
            value: b"initial value",
            ..
        }))
    ));
}

#[test]
fn validates_document_metrics() {
    let payload = [960u32, 2400, 0, 360]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect::<Vec<_>>();
    let bytes = command(DOCUMENT_METRICS, &payload);
    let list = DisplayList::validate(&bytes).expect("valid document metrics");
    assert!(matches!(
        list.commands().next(),
        Some(Ok(Command::DocumentMetrics {
            content_width: 960,
            content_height: 2400,
            scroll_y: 360,
            ..
        }))
    ));
}

#[test]
fn validates_dillo_vector_primitives() {
    let mut line = f32s(&[1.0, 2.0, 30.0, 40.0, 2.0]);
    line.extend_from_slice(&0xff33_6699u32.to_le_bytes());
    line.extend_from_slice(&[1, 0, 0, 0]);
    let mut bytes = command(LINE, &line);

    let mut polygon = 0xff11_2233u32.to_le_bytes().to_vec();
    polygon.extend_from_slice(&1.0f32.to_le_bytes());
    polygon.extend_from_slice(&3u16.to_le_bytes());
    polygon.extend_from_slice(&[3, 0]);
    polygon.extend(f32s(&[0.0, 0.0, 10.0, 0.0, 5.0, 8.0]));
    bytes.extend(command(POLYGON, &polygon));

    let mut arc = f32s(&[0.0, 0.0, 20.0, 20.0, 0.0, 180.0, 1.0]);
    arc.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    arc.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend(command(ARC, &arc));

    let list = DisplayList::validate(&bytes).expect("valid Dillo primitives");
    assert_eq!(list.commands().count(), 3);
}

#[test]
fn validates_modern_retained_paths_and_clips() {
    let mut definition = 9u32.to_le_bytes().to_vec();
    definition.extend_from_slice(&4u32.to_le_bytes());
    for (verb, coordinates) in [
        (0, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        (1, [100.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
        (3, [110.0, 20.0, 100.0, 80.0, 80.0, 100.0]),
        (4, [0.0; 6]),
    ] {
        definition.extend_from_slice(&[verb, 0, 0, 0]);
        definition.extend(f32s(&coordinates));
    }
    let mut bytes = command(DEFINE_PATH, &definition);

    let mut clip = 9u32.to_le_bytes().to_vec();
    clip.extend(f32s(&[1.0, 0.0, 0.0, 1.0, 10.0, 20.0]));
    clip.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend(command(PUSH_PATH_CLIP, &clip));

    let mut fill = 9u32.to_le_bytes().to_vec();
    fill.extend(f32s(&[1.0, 0.0, 0.0, 1.0, 10.0, 20.0]));
    fill.extend_from_slice(&0xff22_4466u32.to_le_bytes());
    fill.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend(command(FILL_PATH, &fill));

    let mut stroke = 9u32.to_le_bytes().to_vec();
    stroke.extend(f32s(&[1.0, 0.0, 0.0, 1.0, 10.0, 20.0]));
    stroke.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
    stroke.extend_from_slice(&2.0f32.to_le_bytes());
    stroke.extend_from_slice(&4.0f32.to_le_bytes());
    stroke.extend_from_slice(&[1, 2, 0, 0]);
    bytes.extend(command(STROKE_PATH, &stroke));
    bytes.extend(command(POP_CLIP, &[]));

    let list = DisplayList::validate(&bytes).expect("valid modern retained paths");
    assert_eq!(list.commands().count(), 5);
}

#[test]
fn decodes_title_and_image_semantic_regions() {
    let mut title = f32s(&[4.0, 8.0, 120.0, 24.0]);
    title.extend_from_slice(&7u64.to_le_bytes());
    title.extend_from_slice(&5u32.to_le_bytes());
    title.extend_from_slice(b"Hello");
    title.resize(title.len().next_multiple_of(4), 0);
    let mut bytes = command(TITLE_REGION, &title);

    let url = b"https://example.test/photo.png";
    let mut image = f32s(&[4.0, 40.0, 80.0, 60.0]);
    image.extend_from_slice(&8u64.to_le_bytes());
    image.extend_from_slice(&(url.len() as u32).to_le_bytes());
    image.extend_from_slice(url);
    image.resize(image.len().next_multiple_of(4), 0);
    bytes.extend(command(IMAGE_REGION, &image));

    let commands = DisplayList::validate(&bytes)
        .unwrap()
        .commands()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(matches!(
        commands.as_slice(),
        [
            Command::TitleRegion { node: 7, title, .. },
            Command::ImageRegion { node: 8, url: decoded, .. }
        ] if *title == b"Hello" && *decoded == url
    ));
}

#[test]
fn live_html_document_is_bounded_utf8_and_exclusive() {
    let bytes = html_document("<p>Selectable café</p>").unwrap();
    let list = DisplayList::validate(&bytes).unwrap();
    assert!(matches!(list.commands().next(), Some(Ok(Command::HtmlDocument { html })) if html == "<p>Selectable café</p>"));
    assert!(html_document(&"x".repeat(MAX_HTML_DOCUMENT_BYTES + 1)).is_err());
    let mut invalid = bytes.clone();
    invalid[12] = 0xff;
    assert!(DisplayList::validate(&invalid).is_err());
    let mut mixed = bytes.clone();
    mixed.extend_from_slice(&bytes);
    assert!(DisplayList::validate(&mixed).is_err());
    let mut truncated = bytes;
    truncated.truncate(12);
    assert!(DisplayList::validate(&truncated).is_err());
}
