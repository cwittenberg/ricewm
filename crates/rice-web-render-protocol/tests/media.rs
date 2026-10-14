use rice_web_render_protocol::{Command, DisplayList, MEDIA_SURFACE};

fn surface(session: u64) -> Vec<u8> {
    let mut bytes = vec![MEDIA_SURFACE, 0, 8, 0, 0, 0, 0, 0];
    for value in [10f32, 20., 640., 360.] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend(session.to_le_bytes());
    bytes
}

#[test]
fn media_surface_has_an_opaque_nonzero_session_and_finite_geometry() {
    let bytes = surface(91);
    let display = DisplayList::validate(&bytes).unwrap();
    assert!(matches!(display.commands().next().unwrap().unwrap(),
        Command::MediaSurface { session: 91, rect: [10., 20., 640., 360.] }));
    assert!(DisplayList::validate(&surface(0)).is_err());
    let mut invalid = surface(91);
    invalid[8..12].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(DisplayList::validate(&invalid).is_err());
    assert!(DisplayList::validate(&bytes[..28]).is_err());
}
