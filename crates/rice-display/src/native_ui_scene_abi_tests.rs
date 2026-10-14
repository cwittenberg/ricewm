use super::*;
use rice_native_app_protocol::{UiPatch, UiPatchEnvelope, WebRenderPatch, WidgetId};
use rice_ui::{WidgetKind, WidgetState};

#[test]
fn scene_abi_rejects_html_and_reemits_recreated_surface_at_same_epoch() {
    let app = AppId::new(901).unwrap();
    let window = WindowId::new(1).unwrap();
    let surface = WidgetId::new(1).unwrap();
    let mut transport = NativeUiTransport::default();
    let create = |kind| UiPatch::Create {
        id: surface,
        parent: None,
        index: 0,
        kind: kind as u16,
        state: WidgetState::ENABLED.bits(),
        text: None,
        icon: None,
    };
    let ui = |sequence, patches| {
        Message::UiPatch(UiPatchEnvelope {
            app,
            window,
            sequence,
            patches,
        })
        .encode()
        .unwrap()
    };
    transport
        .apply(&ui(1, vec![create(WidgetKind::WebSurface)]))
        .unwrap();
    let frame = |display_list| {
        Message::WebRenderPatch(WebRenderPatch {
            app,
            window,
            surface,
            epoch: 1,
            viewport_width: 200,
            viewport_height: 100,
            display_list,
        })
        .encode()
        .unwrap()
    };
    transport
        .apply(&frame(
            rice_web_render_protocol::html_document("<p>browser only</p>").unwrap(),
        ))
        .unwrap();
    transport.render(app, window, 400, 300).unwrap();
    transport
        .apply(&ui(
            2,
            vec![
                UiPatch::Remove { id: surface },
                create(WidgetKind::SceneSurface),
            ],
        ))
        .unwrap();
    assert!(
        transport
            .apply(&frame(
                rice_web_render_protocol::html_document("<p>denied</p>").unwrap()
            ))
            .is_err()
    );
    let mut rect = vec![rice_web_render_protocol::SOLID_RECT, 0, 7, 0, 0, 0, 0, 0];
    for value in [0_f32, 0., 200., 100.] {
        rect.extend(value.to_le_bytes());
    }
    rect.extend(0x00ff_00ff_u32.to_le_bytes());
    transport.apply(&frame(rect.repeat(100))).unwrap();
    let first = transport.render(app, window, 400, 300).unwrap();
    let repeated = transport.render(app, window, 400, 300).unwrap();
    assert!(
        first > 2800,
        "new owned scene bytes must replace old browser cache despite reused epoch"
    );
    assert!(repeated < 512, "unchanged scene payload remains retained");
    // AppWindow.setRoot preserves positional IDs while creating new controls.
    // Their epochs restart; the native scene and transport cache must follow.
    transport
        .apply(&ui(
            3,
            vec![UiPatch::Clear, create(WidgetKind::SceneSurface)],
        ))
        .unwrap();
    let mut replacement = rect;
    replacement[24..28].copy_from_slice(&0xffff_0000_u32.to_le_bytes());
    transport.apply(&frame(replacement.repeat(100))).unwrap();
    assert!(transport.render(app, window, 400, 300).unwrap() > 2800);
    assert!(transport.render(app, window, 400, 300).unwrap() < 512);
    let retained = transport.host.clone();
    transport.forget_scene(AppId::new(999).unwrap(), window, surface);
    assert!(
        transport.render(app, window, 400, 300).unwrap() < 512,
        "other app keys are isolated"
    );
    transport.forget_scene(app, window, surface);
    assert_eq!(
        transport.host, retained,
        "eviction must not mutate the retained native scene"
    );
    assert!(
        transport.render(app, window, 400, 300).unwrap() > 2800,
        "forgotten acknowledgement retransmits payload"
    );
    assert!(transport.render(app, window, 400, 300).unwrap() < 512);
}
