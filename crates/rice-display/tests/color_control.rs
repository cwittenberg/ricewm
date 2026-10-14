//! Actual native draw-command regression: palette entries are swatches while
//! toolbar color controls keep their icon and current-color underline.
use rice_display::native_ui::{DrawCommand, NativeUiHost};
use rice_native_app_protocol::{AppId, UiPatch, UiPatchEnvelope, WidgetId, WindowId};
use rice_ui::{Color, Constraints, IconId, Size, Theme, WidgetKind, WidgetState};

fn render(kind: WidgetKind, value: &str, enabled: bool) -> Vec<DrawCommand> {
    let app = AppId::new(73).unwrap();
    let window = WindowId::new(1).unwrap();
    let mut host = NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: WidgetId::new(1).unwrap(),
            parent: None,
            index: 0,
            kind: kind as u16,
            state: if enabled {
                WidgetState::ENABLED.bits()
            } else {
                0
            },
            text: Some(format!("panel:color-control:{value}|Sample color")),
            icon: Some(IconId::FillColor.wire_value()),
        }],
    })
    .unwrap();
    let size = Size {
        width: 200,
        height: 32,
    };
    host.render(app, window, Constraints::new(size, size).unwrap())
        .unwrap()
        .commands
}

#[test]
fn palette_color_is_bordered_swatch_without_paint_icon() {
    for enabled in [true, false] {
        let commands = render(WidgetKind::MenuItem, "#123456", enabled);
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, DrawCommand::Icon { .. }))
        );
        let expected = if enabled {
            Color::opaque(0x12, 0x34, 0x56)
        } else {
            Color::opaque(0x12, 0x34, 0x56).with_alpha(110)
        };
        let swatch = commands
            .iter()
            .find_map(|c| match c {
                DrawCommand::Fill { rect, color, .. } if *color == expected => Some(*rect),
                _ => None,
            })
            .expect("solid palette swatch");
        assert!(swatch.size.width >= 8 && swatch.size.height >= 8);
        assert!(
            commands.iter().any(
                |c| matches!(c, DrawCommand::Border { rect, width: 1, .. } if *rect == swatch)
            )
        );
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, DrawCommand::Text { text, .. } if text == "Sample color"))
        );
    }
}

#[test]
fn none_and_mixed_palette_entries_have_distinct_swatch_marks() {
    let theme = Theme::familiar_light();
    for value in ["none", "mixed"] {
        let commands = render(WidgetKind::MenuItem, value, true);
        assert!(
            !commands
                .iter()
                .any(|c| matches!(c, DrawCommand::Icon { .. }))
        );
        let marks = commands
            .iter()
            .filter_map(|c| match c {
                DrawCommand::Fill { rect, color, .. } if *color == theme.palette.text => {
                    Some(*rect)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if value == "none" {
            assert!(marks.len() >= 8, "diagonal no-color mark spans the swatch");
            assert!(marks.iter().all(|r| r.size.height == 1));
        } else {
            assert_eq!(marks.len(), 1);
            assert!(
                marks[0].size.height >= 8,
                "mixed color fills half the full-height swatch"
            );
        }
    }
}

#[test]
fn toolbar_controls_retain_icon_and_four_pixel_color_bar() {
    for kind in [WidgetKind::Menu, WidgetKind::Button] {
        let commands = render(kind, "#123456", true);
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, DrawCommand::Icon { .. }))
        );
        assert!(commands.iter().any(|c| matches!(c, DrawCommand::Fill { rect, color, .. } if *color == Color::opaque(0x12,0x34,0x56) && rect.size.height == 4)));
    }
}
