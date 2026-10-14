use crate::{Command, DisplayList, ProtocolError, PUSH_DROP_SHADOW, POP_DROP_SHADOW};
fn record(op: u8, payload: &[u8]) -> Vec<u8> {
    let mut output = vec![0; 8 + payload.len()];
    output[0] = op;
    output[4..8].copy_from_slice(&u32::try_from(8 + payload.len()).unwrap().to_le_bytes());
    output[8..].copy_from_slice(payload); output
}
fn start() -> Vec<u8> {
    let mut payload = Vec::new();
    for value in [10.0_f32, 20.0, 80.0, 40.0, -3.0, 7.0, 4.0] { payload.extend(value.to_le_bytes()); }
    payload.extend(0x8040_2010_u32.to_le_bytes()); record(PUSH_DROP_SHADOW, &payload)
}
#[test]
fn actual_content_shadow_decodes_and_balances_in_vector_ingress() {
    let mut bytes=start(); bytes.extend(record(POP_DROP_SHADOW, &[]));
    let list=DisplayList::validate_scene(&bytes).unwrap();
    assert!(matches!(list.commands().next_command().unwrap(), Some(Command::PushDropShadow {
        bounds: [10.0,20.0,80.0,40.0], offset: [-3.0,7.0], blur: 4.0, color: 0x8040_2010, transform: None
    })));
}
#[test]
fn invalid_content_shadow_geometry_and_scope_are_rejected() {
    for (index,value) in [(0,f32::INFINITY),(1,1_000_001.0),(2,0.0),(3,16_385.0),(4,-1_000_001.0),(6,-1.0),(6,1_000_001.0)] {
        let mut bytes=start(); bytes[8+index*4..12+index*4].copy_from_slice(&value.to_le_bytes());bytes.extend(record(POP_DROP_SHADOW,&[]));
        assert_eq!(DisplayList::validate_scene(&bytes),Err(ProtocolError::InvalidNumber));
    }
    assert!(DisplayList::validate_scene(&start()).is_err());
    assert!(DisplayList::validate_scene(&record(POP_DROP_SHADOW,&[])).is_err());
    let mut crossing=start();crossing.extend(record(3,&[0;32]));crossing.extend(record(POP_DROP_SHADOW,&[]));crossing.extend(record(4,&[]));
    assert!(DisplayList::validate_scene(&crossing).is_err());
    let mut deep=Vec::new();for _ in 0..5 {deep.extend(start());}for _ in 0..5 {deep.extend(record(POP_DROP_SHADOW,&[]));}
    assert_eq!(DisplayList::validate_scene(&deep),Err(ProtocolError::StackOverflow));
}

#[test]
fn shadow_scope_count_and_general_protocol_nesting_are_bounded() {
    let mut many=Vec::new();
    for _ in 0..128 { many.extend(start()); many.extend(record(POP_DROP_SHADOW, &[])); }
    assert!(DisplayList::validate_scene(&many).is_ok());
    many.extend(start());many.extend(record(POP_DROP_SHADOW,&[]));
    assert_eq!(DisplayList::validate_scene(&many),Err(ProtocolError::TooManyCommands));
    let mut crossed=record(3,&[0;32]);crossed.extend(start());crossed.extend(record(4,&[]));crossed.extend(record(POP_DROP_SHADOW,&[]));
    assert!(DisplayList::validate(&crossed).is_err());
}

#[test]
fn independent_affine_shadow_accepts_reflections_and_large_sigma_but_rejects_singular_or_unbounded() {
    let make = |matrix: [f32; 6]| {
        let mut bytes = start();
        bytes[4..8].copy_from_slice(&64_u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&1_000_000_f32.to_le_bytes());
        for value in matrix { bytes.extend(value.to_le_bytes()); }
        bytes.extend(record(POP_DROP_SHADOW, &[])); bytes
    };
    let matrix = [-2.0, 0.25, 0.5, 1.0, 70.0, -20.0];
    let bytes = make(matrix);
    let list = DisplayList::validate_scene(&bytes).unwrap();
    assert!(matches!(list.commands().next_command().unwrap(), Some(Command::PushDropShadow { transform: Some(m), blur: 1_000_000.0, .. }) if m.map(f32::to_bits) == matrix.map(f32::to_bits)));
    for matrix in [[1.0,2.0,2.0,4.0,0.0,0.0], [1025.0,0.0,0.0,1.0,0.0,0.0], [1.0,0.0,0.0,1.0,1_000_001.0,0.0], [1.0,0.0,0.0,f32::INFINITY,0.0,0.0]] {
        assert_eq!(DisplayList::validate_scene(&make(matrix)), Err(ProtocolError::InvalidNumber));
    }
    for len in [36,40,48,52,60] {
        let mut bytes=record(PUSH_DROP_SHADOW,&vec![0;len]);bytes.extend(record(POP_DROP_SHADOW,&[]));
        assert!(DisplayList::validate_scene(&bytes).is_err());
    }
}
