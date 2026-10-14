use super::*;
use rice_native_app_protocol::{PatchValue, UiEvent, UiPatch, UiPatchEnvelope};
use rice_ui::{WidgetKind, WidgetState};

fn address() -> Address { Address::from_halves(89, 0, 1, 0).unwrap() }
fn id() -> WidgetId { WidgetId::new(1).unwrap() }
fn apply(t: &mut NativeUiTransport, sequence: u64, patches: Vec<UiPatch>) -> Result<(), TransportError> {
    let a = address();
    t.apply(&Message::UiPatch(UiPatchEnvelope { app: a.app, window: a.window, sequence, patches }).encode().unwrap()).map(|_| ())
}
fn fixture(observe: bool) -> NativeUiTransport {
    let mut t = NativeUiTransport::default();
    let mut patches = vec![UiPatch::Create { id: id(), parent: None, index: 0, kind: WidgetKind::TextField as u16,
        state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(), text: Some("=日😀+A1".into()), icon: None }];
    if observe { patches.push(UiPatch::EditSelection { id: id(), anchor: 3, caret: 3 }); }
    apply(&mut t, 1, patches).unwrap();
    let a = address(); t.render(a.app, a.window, 300, 36).unwrap();
    t
}
fn events(t: &mut NativeUiTransport) -> Vec<UiEvent> {
    let mut result = Vec::new();
    while t.events.pending() > 0 {
        let mut bytes = vec![0; t.events.pending()]; t.events.drain(&mut bytes).unwrap();
        if let Message::UiEvent(event) = Message::decode(&bytes).unwrap() { result.push(event.event); }
    }
    result
}

#[test]
fn selection_is_scalar_indexed_and_follows_native_text_changes() {
    let mut t = fixture(true);
    t.key(address(), true, 113, 0, None).unwrap();
    assert!(matches!(events(&mut t).as_slice(), [UiEvent::Key { key_code: 113, .. }, UiEvent::TextSelection { anchor: 2, caret: 2 }]));
    t.key(address(), true, 0, 0, Some("X".into())).unwrap();
    assert_eq!(events(&mut t), vec![UiEvent::ValueChanged("=日X😀+A1".into()), UiEvent::TextSelection { anchor: 3, caret: 3 }]);
    // Replace a selected range at an explicitly requested caret; preserve the suffix.
    apply(&mut t, 2, vec![
        UiPatch::Update { id: id(), state: PatchValue::Keep, text: PatchValue::Set("=日X😀+A1".into()), icon: PatchValue::Keep },
        UiPatch::EditSelection { id: id(), anchor: 5, caret: 7 },
    ]).unwrap();
    t.key(address(), true, 0, 0, Some("B2".into())).unwrap();
    assert_eq!(events(&mut t), vec![UiEvent::ValueChanged("=日X😀+B2".into()), UiEvent::TextSelection { anchor: 7, caret: 7 }]);
}

#[test]
fn legacy_fields_do_not_emit_selection_metadata() {
    let mut t = fixture(false);
    t.key(address(), true, 113, 0, None).unwrap();
    assert!(matches!(events(&mut t).as_slice(), [UiEvent::Key { .. }]));
    t.key(address(), true, 0, 0, Some("X".into())).unwrap();
    assert!(matches!(events(&mut t).as_slice(), [UiEvent::ValueChanged(_)]));
}

#[test]
fn invalid_selection_rejects_the_whole_text_transaction() {
    let mut t = fixture(true); let before = t.clone();
    assert!(apply(&mut t, 2, vec![
        UiPatch::Update { id: id(), state: PatchValue::Keep, text: PatchValue::Set("abc".into()), icon: PatchValue::Keep },
        UiPatch::EditSelection { id: id(), anchor: 4, caret: 4 },
    ]).is_err());
    assert_eq!(t, before);
}

#[test]
fn new_selection_requests_and_removal_drop_queued_old_caret_metadata() {
    let mut t = fixture(true);
    t.key(address(), true, 113, 0, None).unwrap();
    apply(&mut t, 2, vec![UiPatch::EditSelection { id: id(), anchor: 0, caret: 0 }]).unwrap();
    assert!(events(&mut t).iter().all(|event| !matches!(event, UiEvent::TextSelection { .. })));
    t.key(address(), true, 114, 0, None).unwrap();
    apply(&mut t, 3, vec![UiPatch::Remove { id: id() }]).unwrap();
    assert!(events(&mut t).iter().all(|event| !matches!(event, UiEvent::TextSelection { .. })));
    assert!(!t.host.has_text_selection_observers(address().app, address().window));
}

#[test]
fn pointer_selection_metadata_is_bounded_and_queue_rejection_preserves_caret() {
    let mut t = fixture(true);
    t.pointer(address(), PointerEventKind::Down, Point { x: 8, y: 18 }, 1, 0).unwrap();
    assert!(events(&mut t).iter().any(|event| matches!(event, UiEvent::TextSelection { anchor, caret } if *anchor <= 6 && *caret <= 6)));
    t.events.frames = std::iter::repeat_n(vec![], MAX_EVENT_FRAMES - 1).collect();
    let before = t.host.clone();
    assert!(t.key(address(), true, 0, 0, Some("X".into())).is_err());
    assert_eq!(t.host, before);
}
