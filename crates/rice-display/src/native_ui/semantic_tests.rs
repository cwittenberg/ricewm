use super::*;
use rice_native_app_protocol::{UiPatch, UiPatchEnvelope};
use rice_ui::{Constraints, Size, WidgetKind, WidgetState};

fn id(value: u64) -> WidgetId {
    WidgetId::new(value).unwrap()
}

fn create(
    value: u64,
    parent: Option<u64>,
    index: u32,
    kind: WidgetKind,
    state: WidgetState,
    text: Option<&str>,
) -> UiPatch {
    UiPatch::Create {
        id: id(value),
        parent: parent.map(id),
        index,
        kind: kind as u16,
        state: state.bits(),
        text: text.map(str::to_owned),
        icon: None,
    }
}

fn fixed(width: u32, height: u32) -> Constraints {
    let size = Size { width, height };
    Constraints::new(size, size).unwrap()
}

fn calculator_host() -> NativeUiHost {
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("panel:clamp:520"),
        ),
        create(
            2,
            Some(1),
            0,
            WidgetKind::MenuBar,
            WidgetState::ENABLED,
            None,
        ),
        create(
            3,
            Some(2),
            0,
            WidgetKind::Menu,
            WidgetState::ENABLED,
            Some("Mode"),
        ),
        create(
            4,
            Some(1),
            1,
            WidgetKind::Container,
            WidgetState::ENABLED,
            Some("panel:fixed"),
        ),
        create(
            5,
            Some(4),
            0,
            WidgetKind::TextField,
            WidgetState::ENABLED
                | WidgetState::DISPLAY
                | WidgetState::END_ALIGNED
                | WidgetState::FLAT,
            Some("0"),
        ),
        create(
            6,
            Some(1),
            2,
            WidgetKind::SplitPane,
            WidgetState::ENABLED,
            Some("split:700"),
        ),
        create(
            7,
            Some(6),
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("panel:distribute"),
        ),
    ];
    for row in 0..6_u64 {
        let row_id = 10 + row;
        patches.push(create(
            row_id,
            Some(7),
            u32::try_from(row).unwrap(),
            WidgetKind::Row,
            WidgetState::ENABLED,
            Some("panel:equal"),
        ));
        for column in 0..4_u64 {
            let button = 100 + row * 4 + column;
            let state = if button == 123 {
                WidgetState::ENABLED | WidgetState::ACCENT
            } else if button == 100 {
                WidgetState::ENABLED | WidgetState::DESTRUCTIVE
            } else {
                WidgetState::ENABLED
            };
            patches.push(create(
                button,
                Some(row_id),
                u32::try_from(column).unwrap(),
                WidgetKind::Button,
                state,
                Some(if button == 123 { "=" } else { "8" }),
            ));
        }
    }
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: AppId::new(1).unwrap(),
        window: WindowId::new(1).unwrap(),
        sequence: 1,
        patches,
    })
    .unwrap();
    host
}

fn target(output: &RenderOutput, value: u64) -> Rect {
    output
        .hit_targets
        .iter()
        .find(|target| target.widget == id(value))
        .unwrap()
        .rect
}

#[test]
fn adaptive_calculator_primitives_fill_clamp_and_use_semantic_theme_tokens() {
    let mut host = calculator_host();
    for (width, height) in [(360, 392), (900, 700)] {
        let output = host
            .render(
                AppId::new(1).unwrap(),
                WindowId::new(1).unwrap(),
                fixed(width, height),
            )
            .unwrap();
        let first = target(&output, 100);
        let equals = target(&output, 123);
        assert!(
            equals.origin.y.saturating_add_unsigned(equals.size.height)
                >= i32::try_from(height).unwrap_or(i32::MAX) - 32
        );
        assert!(
            equals.size.height >= 32,
            "equals={equals:?}, first={first:?}"
        );
        assert!(
            first.size.width.abs_diff(equals.size.width) <= 1,
            "distributed keypad columns differ: first={first:?}, equals={equals:?}"
        );
        assert!(
            first.size.height.abs_diff(equals.size.height) <= 1,
            "distributed keypad rows differ: first={first:?}, equals={equals:?}"
        );
        if width > 520 {
            assert!(first.origin.x > 180);
            assert!(equals.origin.x.saturating_add_unsigned(equals.size.width) < 720);
        }
        assert!(output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == equals && *color == host.theme().palette.accent
        )));
        assert!(output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == first && *color == host.theme().palette.danger
        )));
        assert!(output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, size_px, align: TextAlign::End, .. }
                if text == "0" && *size_px == host.theme().typography.display_px
        )));
    }
}
