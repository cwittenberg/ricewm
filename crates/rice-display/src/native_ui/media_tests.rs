use super::*;
use rice_native_app_protocol::{AppId, UiPatch, UiPatchEnvelope, WidgetId, WindowId};
use rice_ui::{Constraints, IconId, Size, WidgetKind, WidgetState};

fn rendered(text: &str) -> Vec<DrawCommand> {
    rendered_image(text, None)
}

fn rendered_image(text: &str, icon: Option<IconId>) -> Vec<DrawCommand> {
    let app = AppId::new(1).unwrap();
    let window = WindowId::new(1).unwrap();
    let id = WidgetId::new(1).unwrap();
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![UiPatch::Create {
            id,
            parent: None,
            index: 0,
            kind: WidgetKind::Image as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some(text.to_owned()),
            icon: icon.map(|icon| icon as u16),
        }],
    })
    .unwrap();
    host.render(
        app,
        window,
        Constraints::new(
            Size {
                width: 800,
                height: 600,
            },
            Size {
                width: 800,
                height: 600,
            },
        )
        .unwrap(),
    )
    .unwrap()
    .commands
}

#[test]
fn decoded_icon_media_uses_the_same_bounded_box_as_its_fallback() {
    let commands = rendered_image("media-request:8", Some(IconId::Application));
    let rect = commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Media {
                rect, request: 8, ..
            } => Some(*rect),
            _ => None,
        })
        .expect("decoded icon media");
    assert_eq!(rect.size.width, 24);
    assert_eq!(rect.size.height, 24);
    assert_eq!(rect.origin.x, 388);
    assert_eq!(rect.origin.y, 288);
}

#[test]
fn media_descriptor_carries_bounded_rotation_and_scale() {
    assert!(
        rendered("media-request:7:3:actual:1250")
            .iter()
            .any(|command| matches!(
                command,
                DrawCommand::Media {
                    request: 7,
                    quarter_turns: 3,
                    scale_mode: MediaScaleMode::Actual,
                    scale_permille: 1_250,
                    ..
                }
            ))
    );
    assert!(rendered("media-request:8").iter().any(|command| matches!(
        command,
        DrawCommand::Media {
            request: 8,
            quarter_turns: 0,
            scale_mode: MediaScaleMode::Fit,
            scale_permille: 1_000,
            ..
        }
    )));
}

#[test]
fn malformed_media_descriptor_never_emits_a_surface_command() {
    for value in [
        "media-request:0",
        "media-request:7:4:fit:1000",
        "media-request:7:0:stretch:1000",
        "media-request:7:0:fit:0",
        "media-request:7:0:fit:1000:extra",
    ] {
        assert!(
            !rendered(value)
                .iter()
                .any(|command| matches!(command, DrawCommand::Media { .. }))
        );
    }
}

#[test]
fn bounded_inline_favicon_emits_a_browser_decoded_image_command() {
    let commands = rendered("inline-image:data:image/png;base64,iVBORw0KGgo=");
    assert!(commands.iter().any(|command| matches!(
        command,
        DrawCommand::InlineImage { data_url, .. }
            if data_url == "data:image/png;base64,iVBORw0KGgo="
    )));
}

#[test]
fn a_compressed_app_screenshot_uses_the_bounded_image_wire_allowance() {
    let image = format!("inline-image:data:image/webp;base64,{}", "A".repeat(48 * 1024));
    assert!(rendered(&image).iter().any(|command| matches!(command,
        DrawCommand::InlineImage { data_url, .. } if data_url.len() > 48 * 1024)));
}

#[test]
fn image_edits_preserve_source_crop_and_output_dimensions() {
    assert!(rendered("media-request:7:1:actual:1000:0:0:100:0:100:100:30:40")
        .iter().any(|command| matches!(command, DrawCommand::Media {
            image_edit: Some([100, 0, 100, 100, 30, 40]), quarter_turns: 1, ..
        })));
    for descriptor in [
        "media-request:7:0:fit:1000:0:0:0:0:0:100:30:40",
        "media-request:7:0:fit:1000:0:0:4090:0:100:100:30:40",
        "media-request:7:0:fit:1000:0:0:0:0:100:100:0:40",
    ] {
        assert!(!rendered(descriptor).iter().any(|c| matches!(c, DrawCommand::Media { .. })));
    }
}
