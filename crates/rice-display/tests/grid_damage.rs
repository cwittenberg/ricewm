use rice_display::native_ui::{DrawCommand, NativeUiHost};
use rice_native_app_protocol::{
    AppId, Message, PatchValue, UiPatch, UiPatchEnvelope, WidgetId, WindowId,
};
use rice_ui::grid_damage::geometry_key;
use rice_ui::grid_frame::GridFrame;
use rice_ui::{Constraints, Size, WidgetKind, WidgetState};
fn frame(revision: u64, value: &str) -> GridFrame {
    GridFrame::parse(&format!(r#"{{"version":1,"revision":{revision},"sheet":1,"row_count":100,"column_count":10,"width":120,"height":60,"row_header":40,"column_header":20,"rows":[{{"index":0,"start":0,"extent":20,"clip_start":0,"clip_end":20,"frozen":false}}],"columns":[{{"index":0,"start":0,"extent":80,"clip_start":0,"clip_end":80,"frozen":false}}],"strings":["{value}"],"styles":[{{"alignment":0,"role":0}}],"cells":[[0,0,0,0]],"selection":null,"active":[0,0]}}"#)).unwrap()
}
fn app() -> AppId {
    AppId::new(71).unwrap()
}
fn window() -> WindowId {
    WindowId::new(1).unwrap()
}
fn id(n: u64) -> WidgetId {
    WidgetId::new(n).unwrap()
}
fn create(value: &GridFrame, widget: u64) -> UiPatch {
    UiPatch::Create {
        id: id(widget),
        parent: None,
        index: 0,
        kind: WidgetKind::VirtualGrid as u16,
        state: WidgetState::ENABLED.bits(),
        text: Some(value.encode().unwrap()),
        icon: None,
    }
}
fn envelope(sequence: u64, patches: Vec<UiPatch>) -> UiPatchEnvelope {
    UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence,
        patches,
    }
}
fn damage(base: &GridFrame, revision: u64, cells: &str) -> UiPatch {
    UiPatch::GridDamage {
        id: id(1),
        damage: format!(
            r#"{{"version":1,"sheet":1,"base_revision":{},"revision":{revision},"base_geometry":{},"cells":{cells},"selection":null,"active":[0,0]}}"#,
            base.revision,
            geometry_key(base)
        ),
    }
}
fn texts(host: &mut NativeUiHost) -> Vec<String> {
    host.render(
        app(),
        window(),
        Constraints::new(
            Size {
                width: 120,
                height: 60,
            },
            Size {
                width: 120,
                height: 60,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .commands
    .iter()
    .filter_map(|c| {
        if let DrawCommand::Text { text, .. } = c {
            Some(text.clone())
        } else {
            None
        }
    })
    .collect()
}
#[test]
fn damage_wire_and_native_atomic_recovery_preserve_last_view_and_validate_queued_structures() {
    let initial = frame(1, "old");
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![create(&initial, 1)]))
        .unwrap();
    assert!(texts(&mut host).contains(&"old".into()));
    let changed = envelope(
        2,
        vec![damage(
            &initial,
            2,
            r#"[[0,0,"changed",{"alignment":0,"role":0}]]"#,
        )],
    );
    let wire = Message::UiPatch(changed.clone()).encode().unwrap();
    assert_eq!(
        Message::decode(&wire).unwrap(),
        Message::UiPatch(changed.clone())
    );
    host.apply_patch(&changed).unwrap();
    assert!(texts(&mut host).contains(&"changed".into()));
    // Old base: consume presentation sequence but preserve visible text/hits.
    host.apply_patch(&envelope(3, vec![damage(&initial, 3, "[]")]))
        .unwrap();
    assert!(texts(&mut host).contains(&"changed".into()));
    assert!(
        host.pointer_event(
            app(),
            window(),
            rice_native_app_protocol::PointerEventKind::Down,
            rice_ui::Point { x: 50, y: 30 },
            1,
            0
        )
        .unwrap()
        .is_none()
    );
    assert!(
        host.key_event(app(), window(), true, 37, 0, None)
            .unwrap()
            .is_none()
    );
    host.apply_patch(&envelope(
        4,
        vec![UiPatch::Create {
            id: id(2),
            parent: None,
            index: 1,
            kind: WidgetKind::Label as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("queued".into()),
            icon: None,
        }],
    ))
    .unwrap();
    host.apply_patch(&envelope(
        5,
        vec![UiPatch::Update {
            id: id(2),
            state: PatchValue::Keep,
            text: PatchValue::Set("queued update".into()),
            icon: PatchValue::Keep,
        }],
    ))
    .unwrap();
    assert!(!texts(&mut host).contains(&"queued update".into()));
    let authoritative = frame(5, "latest");
    host.apply_patch(&envelope(
        6,
        vec![UiPatch::Clear, create(&authoritative, 1)],
    ))
    .unwrap();
    assert!(texts(&mut host).contains(&"latest".into()));
    host.apply_patch(&envelope(
        7,
        vec![damage(
            &authoritative,
            6,
            r#"[[0,0,"after recovery",{"alignment":0,"role":0}]]"#,
        )],
    ))
    .unwrap();
    assert!(texts(&mut host).contains(&"after recovery".into()));
    assert_eq!(host.remove_app(app()), 1);
    host.apply_patch(&envelope(1, vec![create(&initial, 1)]))
        .unwrap();
    assert!(texts(&mut host).contains(&"old".into()));
}
#[test]
fn malformed_mixed_batch_rejects_without_consuming_sequence_or_changing_other_widgets() {
    let initial = frame(1, "original");
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![create(&initial, 1)]))
        .unwrap();
    let stale = frame(0, "stale");
    assert!(
        host.apply_patch(&envelope(
            2,
            vec![damage(&stale, 1, "[]"), UiPatch::Remove { id: id(999) }]
        ))
        .is_err()
    );
    assert!(texts(&mut host).contains(&"original".into()));
    assert!(
        host.apply_patch(&envelope(
            2,
            vec![damage(
                &initial,
                2,
                r#"[[0,0,"bad",{"alignment":9,"role":0}]]"#
            )]
        ))
        .is_err()
    );
    host.apply_patch(&envelope(2, vec![damage(&initial, 2, "[]")]))
        .unwrap();
    let mut wrong_geometry = frame(2, "original");
    wrong_geometry.rows[0].index = 90;
    host.apply_patch(&envelope(3, vec![damage(&wrong_geometry, 3, "[]")]))
        .unwrap();
    assert!(texts(&mut host).contains(&"original".into()));
    // Recovery accepts a complete replacement with different root identity.
    host.apply_patch(&envelope(
        4,
        vec![UiPatch::Clear, create(&frame(4, "replacement"), 8)],
    ))
    .unwrap();
    assert!(texts(&mut host).contains(&"replacement".into()));
}
#[test]
fn abi_emits_one_resync_event_and_removes_pending_feedback_on_close() {
    use rice_display::{native_ui_abi::*, rice_io_reserve};
    assert_eq!(rice_native_ui_init(), 1);
    assert_eq!(rice_native_ui_grid_damage_version(), 1);
    let apply = |e: UiPatchEnvelope| {
        let bytes = Message::UiPatch(e).encode().unwrap();
        let p = rice_io_reserve(bytes.len() as u32);
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        }
        assert_eq!(rice_native_ui_apply(bytes.len() as u32), bytes.len() as u32);
    };
    let initial = frame(1, "original");
    apply(envelope(1, vec![create(&initial, 1)]));
    let stale = frame(0, "stale");
    apply(envelope(2, vec![damage(&stale, 1, "[]")]));
    let first = rice_native_ui_event_pending();
    assert!(first > 0);
    apply(envelope(3, vec![damage(&stale, 2, "[]")]));
    assert_eq!(rice_native_ui_event_pending(), first);
    let p = rice_io_reserve(first);
    assert_eq!(rice_native_ui_event_drain(first), first);
    let bytes = unsafe { std::slice::from_raw_parts(p, first as usize) };
    assert!(matches!(
        Message::decode(bytes).unwrap(),
        Message::UiEvent(rice_native_app_protocol::UiEventEnvelope {
            event: rice_native_app_protocol::UiEvent::GridResync,
            ..
        })
    ));
    assert_eq!(rice_native_ui_event_pending(), 0);
    apply(envelope(
        4,
        vec![UiPatch::Clear, create(&frame(3, "fresh"), 1)],
    ));
    apply(envelope(5, vec![damage(&stale, 4, "[]")]));
    assert!(rice_native_ui_event_pending() > 0);
    assert_eq!(rice_native_ui_remove_app(app().get() as u32, 0), 1);
    assert_eq!(rice_native_ui_event_pending(), 0);
}
