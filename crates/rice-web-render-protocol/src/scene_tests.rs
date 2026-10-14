use crate::*;

fn command(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![tag, 0, 0, 0];
    bytes.extend(u32::try_from(payload.len() + 8).unwrap().to_le_bytes());
    bytes.extend(payload);
    bytes
}
fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|n| n.to_le_bytes()).collect()
}
fn image_definition() -> Vec<u8> {
    let mut payload: Vec<_> = [7_u32, 1, 1, 4]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    payload.extend([255, 0, 0, 255]);
    command(DEFINE_IMAGE, &payload)
}
fn image() -> Vec<u8> {
    let mut payload = floats(&[0., 0., 10., 10., 0., 0., 1., 1.]);
    payload.extend(7_u32.to_le_bytes());
    payload.extend(1_f32.to_le_bytes());
    payload.extend([0; 4]);
    command(IMAGE, &payload)
}
fn path_definition() -> Vec<u8> {
    let mut payload = 9_u32.to_le_bytes().to_vec();
    payload.extend(1_u32.to_le_bytes());
    payload.extend([0; 4]);
    payload.extend(floats(&[0.; 6]));
    command(DEFINE_PATH, &payload)
}
fn path_use(tag: u8) -> Vec<u8> {
    let mut payload = 9_u32.to_le_bytes().to_vec();
    payload.extend(floats(&[1., 0., 0., 1., 0., 0.]));
    if tag != PUSH_PATH_CLIP {
        payload.extend(0xff00_00ff_u32.to_le_bytes());
    }
    if tag == STROKE_PATH {
        payload.extend(floats(&[2., 4.]));
    }
    payload.extend([0; 4]);
    command(tag, &payload)
}

#[test]
fn scene_accepts_frame_owned_images_paths_clips_strokes_and_hits() {
    let mut bytes = image_definition();
    bytes.extend(image());
    bytes.extend(path_definition());
    bytes.extend(path_use(PUSH_PATH_CLIP));
    bytes.extend(path_use(FILL_PATH));
    bytes.extend(path_use(STROKE_PATH));
    bytes.extend(command(POP_CLIP, &[]));
    let mut payload = floats(&[0., 0., 10., 10.]);
    payload.extend(1_u64.to_le_bytes());
    payload.extend([1, 0, 0, 0]);
    bytes.extend(command(HIT_REGION, &payload));
    assert!(DisplayList::validate_scene(&bytes).is_ok());
}

#[test]
fn scene_rejects_foreign_previous_late_and_duplicate_resources() {
    for bytes in [
        image(),
        path_use(FILL_PATH),
        path_use(STROKE_PATH),
        [image(), image_definition()].concat(),
        [image_definition(), image_definition()].concat(),
        [path_definition(), path_definition()].concat(),
    ] {
        assert!(DisplayList::validate(&bytes).is_ok());
        assert_eq!(
            DisplayList::validate_scene(&bytes),
            Err(ProtocolError::InvalidSceneResource)
        );
    }
    // A previous independently validated frame does not introduce global IDs.
    assert!(DisplayList::validate_scene(&image_definition()).is_ok());
    assert_eq!(
        DisplayList::validate_scene(&image()),
        Err(ProtocolError::InvalidSceneResource)
    );
}

#[test]
fn scene_rejects_each_browser_semantic_and_media_opcode() {
    let mut region = floats(&[0., 0., 10., 10.]);
    region.extend(1_u64.to_le_bytes());
    let mut bytes = vec![
        html_document("<p>inert but not vector</p>").unwrap(),
        command(MEDIA_SURFACE, &region),
    ];
    for tag in [LINK_REGION, TITLE_REGION, IMAGE_REGION] {
        let mut payload = region.clone();
        payload.extend(4_u32.to_le_bytes());
        payload.extend(b"test");
        bytes.push(command(tag, &payload));
    }
    let mut input = region;
    input.extend([2, 0, 0, 0, 0, 0, 0, 0]);
    bytes.push(command(INPUT_REGION, &input));
    let metrics: Vec<_> = [100_u32, 100, 0, 0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    bytes.push(command(DOCUMENT_METRICS, &metrics));
    for bytes in bytes {
        assert!(
            DisplayList::validate(&bytes).is_ok(),
            "fixture must be valid browser input"
        );
        assert_eq!(
            DisplayList::validate_scene(&bytes),
            Err(ProtocolError::NonVectorScene)
        );
    }
}

#[test]
fn scene_rejects_oversize_and_crossed_clip_context_scopes() {
    assert_eq!(DisplayList::validate_scene(&[]), Err(ProtocolError::InvalidLength));
    assert_eq!(
        DisplayList::validate_scene(&vec![0; MAX_SCENE_BYTES + 4]),
        Err(ProtocolError::TooLarge)
    );
    let mut context = floats(&[1., 0., 0., 1., 0., 0., 1.]);
    context.extend([0; 4]);
    let mut bytes = command(PUSH_STACKING_CONTEXT, &context);
    bytes.extend(command(PUSH_CLIP, &floats(&[0.; 8])));
    bytes.extend(command(POP_STACKING_CONTEXT, &[]));
    bytes.extend(command(POP_CLIP, &[]));
    assert!(DisplayList::validate(&bytes).is_ok());
    assert_eq!(
        DisplayList::validate_scene(&bytes),
        Err(ProtocolError::UnbalancedStack(StackKind::StackingContext))
    );
}
