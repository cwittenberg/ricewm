//! Additive measured-editor pointer ABI. EDM1 staging is at most 16 KiB + 32.
use super::*;
use crate::native_ui::editor_measurement::EditorMeasurement;

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_editor_epoch(al: u32, ah: u32, wl: u32, wh: u32) -> u64 {
    let Some(address) = Address::from_halves(al, ah, wl, wh) else { return 0; };
    transport().lock().ok().and_then(|t| {
        let (epoch, sequence) = t.editor_epochs.get(&(address.app, address.window))?;
        (t.host.sequence(address.app, address.window) == Some(*sequence)).then_some(*epoch)
    }).unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_native_ui_pointer_measured(
    al: u32, ah: u32, wl: u32, wh: u32, kind: u32,
    x: i32, y: i32, button: u32, modifiers: u32, length: u32,
) -> u32 {
    if !(32..=16 * 1024 + 32).contains(&length) { return RESULT_ERROR; }
    let (Some(address), Some(kind), Ok(button), Ok(modifiers)) = (
        Address::from_halves(al, ah, wl, wh), pointer_kind(kind),
        u8::try_from(button), u16::try_from(modifiers),
    ) else { return RESULT_ERROR; };
    let Ok(bytes) = staging_input(length, 16 * 1024 + 32) else { return RESULT_ERROR; };
    let Some((epoch, sample)) = decode(&bytes) else { return RESULT_ERROR; };
    transport().lock().ok().and_then(|mut transport| {
        let (current, sequence) = *transport.editor_epochs.get(&(address.app, address.window))?;
        if epoch != current || transport.host.sequence(address.app, address.window) != Some(sequence) {
            return None;
        }
        transport.pointer_measured(address, kind, Point { x, y }, button, modifiers, Some(&sample)).ok()
    }).map_or(RESULT_ERROR, u32::from)
}

fn decode(bytes: &[u8]) -> Option<(u64, EditorMeasurement)> {
    if bytes.len() < 32 || bytes.get(..4)? != b"EDM1"
        || u32::from_le_bytes(bytes[4..8].try_into().ok()?) as usize != bytes.len()
        || u32::from_le_bytes(bytes[28..32].try_into().ok()?) as usize != bytes.len() - 32 { return None; }
    let widget = WidgetId::new(u64::from_le_bytes(bytes[8..16].try_into().ok()?))?;
    let epoch = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
    if epoch == 0 { return None; }
    Some((epoch, EditorMeasurement { widget,
        caret: u32::from_le_bytes(bytes[24..28].try_into().ok()?),
        text: std::str::from_utf8(&bytes[32..]).ok()?.to_owned() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rice_native_app_protocol::{UiEvent, UiPatch, UiPatchEnvelope};
    use rice_ui::{WidgetKind, WidgetState};

    fn fixture() -> (NativeUiTransport, Address, EditorMeasurement, Point) {
        let mut transport = NativeUiTransport::default();
        let address = Address::from_halves(7, 0, 1, 0).unwrap();
        let widget = WidgetId::new(2).unwrap();
        let text = "editor:plain:none:nolines:0:0\n日😀e\u{301}Ω".to_owned();
        let patches = vec![
            UiPatch::Create { id: WidgetId::new(1).unwrap(), parent: None, index: 0,
                kind: WidgetKind::Column as u16, state: WidgetState::ENABLED.bits(), text: None, icon: None },
            UiPatch::Create { id: widget, parent: Some(WidgetId::new(1).unwrap()), index: 0,
                kind: WidgetKind::TextEditor as u16, state: WidgetState::ENABLED.bits(), text: Some(text.clone()), icon: None },
        ];
        transport.apply(&Message::UiPatch(UiPatchEnvelope { app: address.app, window: address.window,
            sequence: 1, patches }).encode().unwrap()).unwrap();
        transport.render(address.app, address.window, 320, 200).unwrap();
        let size = Size { width: 320, height: 200 };
        let output = transport.host.render(address.app, address.window, Constraints::new(size, size).unwrap()).unwrap();
        let rect = output.hit_targets.iter().find(|hit| hit.widget == widget).unwrap().rect;
        let point = Point { x: rect.origin.x + 8, y: rect.origin.y + 8 };
        (transport, address, EditorMeasurement { widget, text, caret: 2 }, point)
    }

    #[test]
    fn invalid_measurement_cannot_move_focus_or_change_capture() {
        let (mut t, address, sample, point) = fixture();
        let before = t.clone();
        for bad in [EditorMeasurement { widget: WidgetId::new(99).unwrap(), ..sample.clone() },
            EditorMeasurement { text: "stale".into(), ..sample.clone() },
            EditorMeasurement { caret: 99, ..sample.clone() }]
        {
            assert!(t.pointer_measured(address, PointerEventKind::Down, point, 1, 0, Some(&bad)).is_err());
            assert_eq!(t, before);
        }
    }

    #[test]
    fn measured_drag_uses_scalar_offsets_outside_widget() {
        let (mut t, address, sample, point) = fixture();
        t.pointer_measured(address, PointerEventKind::Down, point, 1, 0, Some(&sample)).unwrap();
        t.events.frames.clear(); t.events.bytes = 0;
        let end = EditorMeasurement { caret: 5, ..sample };
        t.pointer_measured(address, PointerEventKind::Move, Point { x: 900, y: 500 }, 1, 0, Some(&end)).unwrap();
        let message = Message::decode(t.events.frames.back().unwrap()).unwrap();
        assert!(matches!(message, Message::UiEvent(event) if event.event == UiEvent::TextSelection { anchor: 2, caret: 5 }));
        t.pointer_measured(address, PointerEventKind::Up, Point { x: 900, y: 500 }, 1, 0, Some(&end)).unwrap();
        assert!(!t.pointer(address, PointerEventKind::Move, Point { x: 900, y: 500 }, 0, 0).unwrap());
    }

    #[test]
    fn staged_editor_record_rejects_truncation_lengths_and_invalid_utf8() {
        assert!(decode(&[]).is_none());
        let mut bytes = vec![0; 33]; bytes[..4].copy_from_slice(b"EDM1");
        bytes[4..8].copy_from_slice(&33u32.to_le_bytes());
        bytes[8..16].copy_from_slice(&2u64.to_le_bytes());
        bytes[16..24].copy_from_slice(&1u64.to_le_bytes());
        bytes[28..32].copy_from_slice(&1u32.to_le_bytes()); bytes[32] = b'A';
        assert!(decode(&bytes).is_some());
        bytes[32] = 0xff; assert!(decode(&bytes).is_none());
        bytes[32] = b'A'; bytes[28] = 2; assert!(decode(&bytes).is_none());
    }
}
