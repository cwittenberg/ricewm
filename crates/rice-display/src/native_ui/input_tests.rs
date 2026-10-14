use super::input::wrap_editor_line;
use super::*;
use rice_native_app_protocol::{UiPatch, WebRenderPatch};
use rice_ui::{WidgetKind, WidgetState};

fn app() -> AppId {
    AppId::new(1).expect("test app")
}

fn window() -> WindowId {
    WindowId::new(1).expect("test window")
}

fn widget(value: u64) -> WidgetId {
    WidgetId::new(value).expect("test widget")
}

fn web_command(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let byte_len = 8 + payload.len();
    assert!(byte_len.is_multiple_of(4));
    let mut bytes = vec![opcode, 0];
    bytes.extend(u16::try_from(byte_len / 4).unwrap().to_le_bytes());
    bytes.extend(0u32.to_le_bytes());
    bytes.extend(payload);
    bytes
}

fn web_region_payload(rect: [f32; 4], node: u64) -> Vec<u8> {
    let mut payload = Vec::new();
    for value in rect {
        payload.extend(value.to_le_bytes());
    }
    payload.extend(node.to_le_bytes());
    payload
}

fn web_solid_rect(rect: [f32; 4], color: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    for value in rect {
        payload.extend(value.to_le_bytes());
    }
    payload.extend(color.to_le_bytes());
    web_command(rice_web_render_protocol::SOLID_RECT, &payload)
}

#[test]
fn retained_web_scene_invalidates_the_surface_without_producer_damage() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::WebSurface as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .expect("web surface");
    let frame = |epoch, color| WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(1),
        epoch,
        viewport_width: 800,
        viewport_height: 600,
        display_list: web_solid_rect([120.0, 80.0, 90.0, 24.0], color),
    };
    host.apply_web_render(&frame(1, 0xff00_00ff))
        .expect("initial scene");
    host.apply_web_render(&frame(2, 0x00ff_00ff))
        .expect("hover scene");
    let constraints = Constraints::new(
        Size {
            width: 800,
            height: 600,
        },
        Size {
            width: 800,
            height: 600,
        },
    )
    .expect("constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let damage = output.commands.iter().find_map(|command| match command {
        DrawCommand::WebScene { damage, .. } => *damage,
        _ => None,
    });
    // Display lists are immutable producer-owned frames. Until the producer
    // supplies a bounded damage hint, a new epoch deliberately invalidates
    // the whole web surface instead of diffing command streams on the UI turn.
    assert_eq!(damage, None);
}

#[test]
fn ui_patch_preserves_large_web_scene_storage() {
    use rice_native_app_protocol::PatchValue;

    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::WebSurface as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .expect("web surface");
    host.apply_web_render(&WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(1),
        epoch: 1,
        viewport_width: 800,
        viewport_height: 600,
        display_list: web_solid_rect([0.0, 0.0, 800.0, 600.0], 0xff00_00ff),
    })
    .expect("scene");
    let constraints = Constraints::new(
        Size {
            width: 800,
            height: 600,
        },
        Size {
            width: 800,
            height: 600,
        },
    )
    .expect("constraints");
    let before = host.render(app(), window(), constraints).expect("before");
    let before = before
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::WebScene { display_list, .. } => Some(display_list.clone()),
            _ => None,
        })
        .expect("before scene");
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: PatchValue::Keep,
            text: PatchValue::Set("status update".to_owned()),
            icon: PatchValue::Keep,
        }],
    })
    .expect("ordinary UI patch");
    let after = host.render(app(), window(), constraints).expect("after");
    let after = after
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::WebScene { display_list, .. } => Some(display_list.clone()),
            _ => None,
        })
        .expect("after scene");
    assert!(std::sync::Arc::ptr_eq(&before, &after));
}

#[test]
fn text_field_keys_emit_bounded_value_changes() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Rice".into()),
                icon: None,
            },
        ],
    })
    .expect("text field patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 80,
        },
        Size {
            width: 240,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("text field target");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: target
                .rect
                .origin
                .x
                .saturating_add_unsigned(target.rect.size.width),
            y: target.rect.origin.y,
        },
        1,
        0,
    )
    .expect("focus text field");
    let append = host
        .key_event(app(), window(), true, 0, 0, Some("WM\n".into()))
        .expect("append key")
        .expect("value event");
    assert_eq!(append.event, UiEvent::ValueChanged("RiceWM".into()));
    let x11_s = host
        .key_event(app(), window(), true, 39, 0, Some("s".into()))
        .expect("X11 s key")
        .expect("value event");
    assert_eq!(x11_s.event, UiEvent::ValueChanged("Rices".into()));
    let x11_l = host
        .key_event(app(), window(), true, 46, 0, Some("l".into()))
        .expect("X11 l key")
        .expect("value event");
    assert_eq!(x11_l.event, UiEvent::ValueChanged("Ricel".into()));
    let x11_digit_four = host
        .key_event(app(), window(), true, 13, 0, Some("4".into()))
        .expect("X11 digit 4 key")
        .expect("value event");
    assert_eq!(x11_digit_four.event, UiEvent::ValueChanged("Rice4".into()));
    let erase = host
        .key_event(app(), window(), true, 8, 0, None)
        .expect("backspace key")
        .expect("value event");
    assert_eq!(erase.event, UiEvent::ValueChanged("Ric".into()));
    let alt_shortcut = host
        .key_event(app(), window(), true, 21, 8, None)
        .expect("Alt shortcut")
        .expect("key event");
    assert!(matches!(
        alt_shortcut.event,
        UiEvent::Key {
            key_code: 21,
            modifiers: 8,
            ..
        }
    ));
    let escape = host
        .key_event(app(), window(), true, 9, 0, None)
        .expect("escape shortcut")
        .expect("key event");
    assert!(matches!(escape.event, UiEvent::Key { key_code: 9, .. }));

    let select_all = host
        .key_event(app(), window(), true, 65, 4, None)
        .expect("Ctrl+A")
        .expect("shortcut event");
    assert!(matches!(
        select_all.event,
        UiEvent::Key {
            key_code: 65,
            modifiers: 4,
            ..
        }
    ));
    let replacement = host
        .key_event(app(), window(), true, 0, 0, Some("R".into()))
        .expect("replacement key")
        .expect("replacement value event");
    assert_eq!(replacement.event, UiEvent::ValueChanged("R".into()));
}

#[test]
fn focused_text_editor_receives_tab_instead_of_triggering_focus_traversal() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(),
                text: Some("editor:plain:none:nolines:0:0\n".into()),
                icon: None,
            },
        ],
    })
    .expect("editor patch");
    host.render(
        app(),
        window(),
        Constraints::new(
            Size {
                width: 240,
                height: 80,
            },
            Size {
                width: 240,
                height: 80,
            },
        )
        .unwrap(),
    )
    .expect("render");
    let event = host
        .key_event(app(), window(), true, 23, 0, None)
        .expect("tab key")
        .expect("editor receives key");
    assert!(matches!(
        event,
        UiEventEnvelope {
            target,
            event: UiEvent::Key { key_code: 23, .. },
            ..
        } if target == widget(2)
    ));
    let arrow = host
        .key_event(app(), window(), true, 111, 0, None)
        .expect("arrow key")
        .expect("editor receives arrow");
    assert!(matches!(
        arrow,
        UiEventEnvelope {
            target,
            event: UiEvent::Key { key_code: 111, .. },
            ..
        } if target == widget(2)
    ));
    let shift_arrow = host
        .key_event(app(), window(), true, 113, 1, None)
        .expect("Shift+Left key")
        .expect("editor receives Shift+Left");
    assert!(matches!(
        shift_arrow,
        UiEventEnvelope {
            target,
            event: UiEvent::Key {
                key_code: 113,
                modifiers: 1,
                ..
            },
            ..
        } if target == widget(2)
    ));
}

#[test]
fn text_editor_click_maps_visual_line_and_column_to_character_selection() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(
                    "editor:plain@18:none:nolines:0:0:currentline:1:adaptive\n0123456789012345678901234567890123456789\nabcdefghijklmnopqrstuvwxyz\nthree"
                        .into(),
                ),
                icon: None,
            },
        ],
    })
    .expect("editor patch");
    let constraints = Constraints::new(
        Size {
            width: 320,
            height: 120,
        },
        Size {
            width: 320,
            height: 120,
        },
    )
    .unwrap();
    let output = host.render(app(), window(), constraints).expect("render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("editor target");
    let size = 18_u32;
    let glyph_advance_milli = size.saturating_mul(615).max(1_000);
    let line_height = size
        .saturating_mul(135)
        .div_ceil(100)
        .max(size.saturating_add(2));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text {
            role: TextRole::Editor,
            size_px: 18,
            ..
        }
    )));
    let hover = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Move,
            Point {
                x: target
                    .rect
                    .origin
                    .x
                    .saturating_add_unsigned(4 + glyph_advance_milli.saturating_mul(20) / 1_000),
                y: target
                    .rect
                    .origin
                    .y
                    .saturating_add_unsigned(4 + line_height + 1),
            },
            0,
            0,
        )
        .expect("hover")
        .expect("semantic editor hover");
    assert!(matches!(
        hover.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Move,
            x: 61,
            y: -1,
            ..
        }
    ));
    let clicked = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Down,
            Point {
                x: target
                    .rect
                    .origin
                    .x
                    .saturating_add_unsigned(4 + glyph_advance_milli.saturating_mul(20) / 1_000),
                y: target
                    .rect
                    .origin
                    .y
                    .saturating_add_unsigned(4 + line_height + 1),
            },
            1,
            1,
        )
        .expect("click")
        .expect("selection event");
    assert_eq!(
        clicked.event,
        UiEvent::TextSelection {
            anchor: 0,
            caret: 61,
        }
    );

    let double_clicked = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Down,
            Point {
                x: target
                    .rect
                    .origin
                    .x
                    .saturating_add_unsigned(4 + glyph_advance_milli.saturating_mul(20) / 1_000),
                y: target
                    .rect
                    .origin
                    .y
                    .saturating_add_unsigned(4 + line_height + 1),
            },
            1,
            0x8000,
        )
        .expect("double click")
        .expect("word selection event");
    assert_eq!(
        double_clicked.event,
        UiEvent::TextSelection {
            anchor: 41,
            caret: 67,
        }
    );

    let document = (0..20)
        .map(|line| format!("line{line:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    let caret = document.chars().count();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(2),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set(format!(
                "editor:plain@18:none:nolines:{caret}:{caret}:currentline:1:adaptive\n{document}"
            )),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("scrolled editor patch");
    let output = host
        .render(app(), window(), constraints)
        .expect("scrolled render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("scrolled editor target");
    let maximum_lines = target.rect.size.height.saturating_sub(8) / line_height;
    let first_visible_line = 20_usize.saturating_sub(maximum_lines as usize);
    let clicked = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Down,
            Point {
                x: target.rect.origin.x.saturating_add_unsigned(4),
                y: target.rect.origin.y.saturating_add_unsigned(4),
            },
            1,
            0,
        )
        .expect("click scrolled editor")
        .expect("scrolled selection event");
    assert_eq!(
        clicked.event,
        UiEvent::TextSelection {
            anchor: u32::try_from(first_visible_line.saturating_mul(7)).unwrap(),
            caret: u32::try_from(first_visible_line.saturating_mul(7)).unwrap(),
        }
    );
}

#[test]
fn wrapped_editor_double_click_selects_the_word_under_the_painted_pointer() {
    let document = "starfield behind your windows on every workspace and monitor. Fly forward through deep space, sweep sideways, or steer the vanishing point with the pointer. Respects your background image. Requires an OpenGL-capable compositor.";
    let expected_start = document.find("Respects").expect("target word");
    let expected_end = expected_start + "Respects".len();
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(format!(
                    "editor:plain@18:word:lines:0:0:currentline:1:adaptive\n{document}"
                )),
                icon: None,
            },
        ],
    })
    .expect("wrapped editor patch");
    let constraints = Constraints::new(
        Size {
            width: 520,
            height: 200,
        },
        Size {
            width: 520,
            height: 200,
        },
    )
    .expect("fixed editor constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("editor target");
    let size = 18_u32;
    let advance = size.saturating_mul(615).max(1_000);
    let line_height = size
        .saturating_mul(135)
        .div_ceil(100)
        .max(size.saturating_add(2));
    let gutter = advance.saturating_mul(2).div_ceil(1_000) + 12;
    let capacity = usize::try_from(
        target
            .rect
            .size
            .width
            .saturating_sub(8)
            .saturating_sub(gutter)
            .saturating_mul(1_000)
            / advance,
    )
    .unwrap()
    .max(1);
    let characters = document.chars().collect::<Vec<_>>();
    let mut visual_lines = Vec::new();
    wrap_editor_line(
        &characters,
        0,
        characters.len(),
        rice_ui::EditorWrap::Word,
        capacity,
        &mut visual_lines,
    );
    let (row, line) = visual_lines
        .iter()
        .enumerate()
        .find(|(_, line)| {
            expected_start >= line.start && expected_start < line.start.saturating_add(line.length)
        })
        .expect("target word visual line");
    let column = expected_start.saturating_sub(line.start).saturating_add(2);
    let selected = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Down,
            Point {
                x: target.rect.origin.x.saturating_add_unsigned(
                    4 + gutter + advance.saturating_mul(column as u32) / 1_000,
                ),
                y: target
                    .rect
                    .origin
                    .y
                    .saturating_add_unsigned(4 + line_height.saturating_mul(row as u32) + 1),
            },
            1,
            0x8000,
        )
        .expect("double click")
        .expect("word selection");
    assert_eq!(
        selected.event,
        UiEvent::TextSelection {
            anchor: expected_start as u32,
            caret: expected_end as u32,
        }
    );
}

#[test]
fn select_all_on_focus_replaces_text_or_collapses_to_the_end() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Row as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: (WidgetState::ENABLED | WidgetState::SELECT_ALL_ON_FOCUS).bits(),
                text: Some("https://example.test/".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(3),
                parent: Some(widget(1)),
                index: 1,
                kind: WidgetKind::Button as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Go".into()),
                icon: None,
            },
        ],
    })
    .expect("select-on-focus field patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 80,
        },
        Size {
            width: 360,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let point = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("address target")
        .rect
        .origin;

    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .expect("select address");
    let selected = host
        .render(app(), window(), constraints)
        .expect("selected address render");
    let field = selected
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("selected address target")
        .rect;
    let selection_color = super::render::text_selection_color(host.theme());
    let highlight = selected
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::TextSelection {
                rect,
                text,
                color,
                start,
                end,
                ..
            } if text == "https://example.test/" && *color == selection_color => {
                Some((*rect, *start, *end))
            }
            _ => None,
        })
        .expect("text-only selection highlight");
    assert!(highlight.0.size.width < field.size.width);
    assert!(highlight.0.size.height < field.size.height);
    assert_eq!((highlight.1, highlight.2), (0, 21));
    assert!(
        !selected.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { color, .. } if *color == selection_color
        )),
        "single-line selection must be glyph measured by the text renderer"
    );
    let replacement = host
        .key_event(app(), window(), true, 0, 0, Some("x".into()))
        .expect("replacement key")
        .expect("replacement value");
    assert_eq!(replacement.event, UiEvent::ValueChanged("x".into()));

    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .expect("place address caret");
    host.key_event(app(), window(), true, 38, 4, None)
        .expect("select address with Ctrl+A")
        .expect("select-all key event");
    // An unrelated actor patch clears render geometry until the next frame.
    // Keyboard editing must continue to follow semantic focus in that gap.
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(3),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set(String::from("Go now")),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("unrelated patch between focus and key input");
    let right = host
        .key_event(app(), window(), true, 114, 0, None)
        .expect("right key")
        .expect("right event");
    assert_eq!(
        right.target,
        widget(2),
        "the text field must own Right Arrow even with a spatial neighbour"
    );
    assert!(matches!(right.event, UiEvent::Key { key_code: 114, .. }));
    assert_eq!(host.selected_text(app(), window()), None);
    let appended = host
        .key_event(app(), window(), true, 0, 0, Some("x".into()))
        .expect("append key")
        .expect("appended value");
    assert_eq!(
        appended.event,
        UiEvent::ValueChanged("https://example.test/x".into())
    );
}

#[test]
fn select_all_tracks_an_address_value_expanded_by_the_focus_patch() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::TextField as u16,
            state: (WidgetState::ENABLED | WidgetState::SELECT_ALL_ON_FOCUS).bits(),
            text: Some("example.test/path".into()),
            icon: Some(IconId::Lock as u16),
        }],
    })
    .expect("compact address patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 40,
        },
        Size {
            width: 360,
            height: 40,
        },
    )
    .expect("address constraints");
    let point = host
        .render(app(), window(), constraints)
        .expect("compact address render")
        .hit_targets
        .into_iter()
        .find(|target| target.widget == widget(1))
        .expect("address target")
        .rect
        .origin;
    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .expect("select compact address");

    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set(String::from(
                "https://example.test/path",
            )),
            icon: rice_native_app_protocol::PatchValue::Clear,
        }],
    })
    .expect("focused full address patch");

    assert_eq!(
        host.selected_text(app(), window()),
        Some(String::from("https://example.test/path"))
    );
}

#[test]
fn retained_programmatic_focus_does_not_reselect_after_value_echo() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::TextField as u16,
            state: (WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::SELECT_ALL_ON_FOCUS)
                .bits(),
            text: Some("https://example.test/".into()),
            icon: None,
        }],
    })
    .expect("focused address patch");

    let first = host
        .key_event(app(), window(), true, 0, 0, Some("x".into()))
        .expect("first address key")
        .expect("first address value");
    assert_eq!(first.event, UiEvent::ValueChanged("x".into()));

    // Applications echo the accepted value while leaving their semantic
    // FOCUSED state set. That retained state must not restart select-all.
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set(String::from("x")),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("accepted address value echo");

    let second = host
        .key_event(app(), window(), true, 0, 0, Some("y".into()))
        .expect("second address key")
        .expect("second address value");
    assert_eq!(second.event, UiEvent::ValueChanged("xy".into()));
}

#[test]
fn address_refocus_selects_all_after_semantic_focus_is_released() {
    let mut host = NativeUiHost::default();
    let address = "https://example.test/reviews/article.html";
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::TextField as u16,
            state: (WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::SELECT_ALL_ON_FOCUS)
                .bits(),
            text: Some(address.into()),
            icon: Some(IconId::Lock as u16),
        }],
    })
    .expect("focused address patch");
    assert_eq!(host.selected_text(app(), window()), Some(address.into()));

    // Bowl removes semantic focus when the page is clicked. The native host
    // must release its matching internal focus too, otherwise the next click
    // is mistaken for a repeated click and only places the caret.
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Set(
                (WidgetState::ENABLED | WidgetState::SELECT_ALL_ON_FOCUS).bits(),
            ),
            text: rice_native_app_protocol::PatchValue::Keep,
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("release address focus patch");
    assert_eq!(host.selected_text(app(), window()), None);

    let constraints = Constraints::new(
        Size {
            width: 420,
            height: 40,
        },
        Size {
            width: 420,
            height: 40,
        },
    )
    .expect("address constraints");
    let point = host
        .render(app(), window(), constraints)
        .expect("unfocused address render")
        .hit_targets
        .into_iter()
        .find(|target| target.widget == widget(1))
        .expect("address target")
        .rect
        .origin;
    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .expect("refocus address");

    assert_eq!(host.selected_text(app(), window()), Some(address.into()));
}

#[test]
fn address_field_mouse_drag_selects_and_replaces_only_the_text_range() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: (WidgetState::ENABLED | WidgetState::SELECT_ALL_ON_FOCUS).bits(),
                text: Some("abcdefghij".into()),
                icon: Some(rice_ui::IconId::Network as u16),
            },
        ],
    })
    .expect("address field patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 80,
        },
        Size {
            width: 360,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let field = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("address field target")
        .rect;
    let theme = host.theme().clone();
    let text_start = field
        .origin
        .x
        .saturating_add_unsigned(u32::from(theme.spacing.control_x))
        .saturating_add_unsigned(u32::from(theme.metrics.icon_small))
        .saturating_add_unsigned(u32::from(theme.spacing.unit));
    let point_at = |prefix: &str| Point {
        x: text_start.saturating_add_unsigned(
            super::layout::estimated_text_width(prefix, &theme).saturating_add(1),
        ),
        y: field
            .origin
            .y
            .saturating_add_unsigned(field.size.height / 2),
    };

    host.pointer_event(app(), window(), PointerEventKind::Down, point_at(""), 1, 0)
        .expect("focus address");
    host.pointer_event(app(), window(), PointerEventKind::Up, point_at(""), 1, 0)
        .expect("release initial focus");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        point_at("ab"),
        1,
        0,
    )
    .expect("start text drag");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        point_at("abcdef"),
        1,
        0,
    )
    .expect("drag text selection");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        point_at("abcdef"),
        1,
        0,
    )
    .expect("finish text drag");

    let selected = host
        .render(app(), window(), constraints)
        .expect("render text range");
    let selection_color = super::render::text_selection_color(&theme);
    let highlight = selected
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::TextSelection {
                text,
                color,
                start,
                end,
                ..
            } if text == "abcdefghij" && *color == selection_color => Some((*start, *end)),
            _ => None,
        })
        .expect("range selection highlight");
    let forward_caret = selected
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text {
                text,
                caret: true,
                caret_offset,
                ..
            } if text == "abcdefghij" => *caret_offset,
            _ => None,
        })
        .expect("active forward-selection caret");
    assert_eq!(forward_caret, 6);
    assert_eq!(host.selected_text(app(), window()).as_deref(), Some("cdef"));
    assert_eq!(highlight, (2, 6));

    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        point_at("abcdef"),
        1,
        0,
    )
    .expect("start reverse text drag");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        point_at("ab"),
        1,
        0,
    )
    .expect("reverse text selection");
    host.pointer_event(app(), window(), PointerEventKind::Up, point_at("ab"), 1, 0)
        .expect("finish reverse text drag");
    let reverse = host
        .render(app(), window(), constraints)
        .expect("render reverse text range");
    let reverse_caret = reverse
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text {
                text,
                caret: true,
                caret_offset,
                ..
            } if text == "abcdefghij" => *caret_offset,
            _ => None,
        })
        .expect("active reverse-selection caret");
    assert_eq!(reverse_caret, 2);
    assert_eq!(host.selected_text(app(), window()).as_deref(), Some("cdef"));
    let replacement = host
        .key_event(app(), window(), true, 0, 0, Some("X".into()))
        .expect("replace range")
        .expect("replacement value");
    assert_eq!(replacement.event, UiEvent::ValueChanged("abXghij".into()));
}

#[test]
fn shift_arrows_extend_a_native_text_selection_from_the_caret() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("abcdefghij".into()),
                icon: None,
            },
        ],
    })
    .expect("text field patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 80,
        },
        Size {
            width: 360,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let field = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("text field target")
        .rect;
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: field
                .origin
                .x
                .saturating_add_unsigned(field.size.width.saturating_sub(1)),
            y: field
                .origin
                .y
                .saturating_add_unsigned(field.size.height / 2),
        },
        1,
        0,
    )
    .expect("place caret at end");
    for _ in 0..2 {
        host.key_event(app(), window(), true, 113, 1, None)
            .expect("Shift+Left")
            .expect("selection key event");
    }
    assert_eq!(host.selected_text(app(), window()).as_deref(), Some("ij"));
    let replacement = host
        .key_event(app(), window(), true, 0, 0, Some("X".into()))
        .expect("replace Shift selection")
        .expect("replacement value");
    assert_eq!(replacement.event, UiEvent::ValueChanged("abcdefghX".into()));
}

#[test]
fn programmatic_focus_can_select_all_for_search_shortcuts() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(3),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::Button as u16,
                state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(),
                text: Some("Current result".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 1,
                kind: WidgetKind::TextField as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("existing search".into()),
                icon: None,
            },
        ],
    })
    .expect("shortcut-focused search patch");
    host.render(
        app(),
        window(),
        Constraints::new(
            Size {
                width: 320,
                height: 80,
            },
            Size {
                width: 320,
                height: 80,
            },
        )
        .expect("fixed constraints"),
    )
    .expect("old result focus render");
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![
            UiPatch::Clear,
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: (WidgetState::ENABLED
                    | WidgetState::FOCUSED
                    | WidgetState::SELECT_ALL_ON_FOCUS)
                    .bits(),
                text: Some("existing search".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(3),
                parent: Some(widget(1)),
                index: 1,
                kind: WidgetKind::Button as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Current result".into()),
                icon: None,
            },
        ],
    })
    .expect("slash-focused search patch");
    host.render(
        app(),
        window(),
        Constraints::new(
            Size {
                width: 320,
                height: 80,
            },
            Size {
                width: 320,
                height: 80,
            },
        )
        .expect("fixed constraints"),
    )
    .expect("shortcut search render");
    let replacement = host
        .key_event(app(), window(), true, 8, 0, None)
        .expect("backspace key")
        .expect("replacement value");
    assert_eq!(replacement.event, UiEvent::ValueChanged(String::new()));
}

#[test]
fn printable_r_is_not_mistaken_for_escape_while_menu_state_exists() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextField as u16,
                state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(),
                text: Some(String::new()),
                icon: None,
            },
        ],
    })
    .expect("search field patch");
    host.render(
        app(),
        window(),
        Constraints {
            min: Size {
                width: 320,
                height: 80,
            },
            max: Size {
                width: 320,
                height: 80,
            },
        },
    )
    .expect("search field render");
    let native_window = host
        .windows
        .get_mut(&(app(), window()))
        .expect("native window");
    native_window.focused = Some(widget(2));
    native_window.opened_menu = Some(widget(1));

    let typed = host
        .key_event(app(), window(), true, 27, 0, Some("r".into()))
        .expect("R key")
        .expect("search value");
    assert_eq!(typed.target, widget(2));
    assert_eq!(typed.event, UiEvent::ValueChanged("r".into()));
}

#[test]
fn tab_and_shift_tab_traverse_focusable_controls_without_pointer_priming() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(20),
                parent: None,
                index: 0,
                kind: WidgetKind::Row as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(21),
                parent: Some(widget(20)),
                index: 0,
                kind: WidgetKind::Button as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("One".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(22),
                parent: Some(widget(20)),
                index: 1,
                kind: WidgetKind::Button as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Two".into()),
                icon: None,
            },
        ],
    })
    .expect("buttons");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 80,
        },
        Size {
            width: 240,
            height: 80,
        },
    )
    .expect("constraints");
    host.render(app(), window(), constraints).expect("render");
    let first = host
        .key_event(app(), window(), true, 23, 0, None)
        .expect("tab")
        .expect("focus");
    assert_eq!(first.target, widget(21));
    let second = host
        .key_event(app(), window(), true, 23, 0, None)
        .expect("tab")
        .expect("focus");
    assert_eq!(second.target, widget(22));
    let reverse = host
        .key_event(app(), window(), true, 23, 1, None)
        .expect("shift tab")
        .expect("focus");
    assert_eq!(reverse.target, widget(21));
    for key_code in [36, 104] {
        let activation = host
            .key_event(app(), window(), true, key_code, 0, None)
            .expect("return key")
            .expect("activation");
        assert_eq!(activation.target, widget(21));
        assert_eq!(activation.event, UiEvent::Activate);
    }
}

#[test]
fn file_grid_arrow_navigation_uses_the_laid_out_columns() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![UiPatch::Create {
        id: widget(300),
        parent: None,
        index: 0,
        kind: WidgetKind::IconGrid as u16,
        state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(),
        text: Some(String::from("file-grid:1000")),
        icon: None,
    }];
    for index in 0..8_u64 {
        patches.push(UiPatch::Create {
            id: widget(301 + index),
            parent: Some(widget(300)),
            index: u32::try_from(index).expect("index"),
            kind: WidgetKind::Button as u16,
            state: (WidgetState::ENABLED | WidgetState::KEY_NAVIGATION).bits(),
            text: Some(format!("Item {index}")),
            icon: Some(rice_ui::IconId::Document.wire_value()),
        });
    }
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches,
    })
    .expect("grid patch");
    let constraints = Constraints::new(
        Size {
            width: 610,
            height: 300,
        },
        Size {
            width: 610,
            height: 300,
        },
    )
    .expect("fixed constraints");
    let output = host
        .render(app(), window(), constraints)
        .expect("grid render");
    let targets = (0..8_u64)
        .map(|index| {
            output
                .hit_targets
                .iter()
                .find(|target| target.widget == widget(301 + index))
                .copied()
                .expect("grid item")
        })
        .collect::<Vec<_>>();
    let first_row_y = targets[0].rect.origin.y;
    let first_row = targets
        .iter()
        .take_while(|target| target.rect.origin.y == first_row_y)
        .collect::<Vec<_>>();
    assert_eq!(
        first_row.len(),
        4,
        "fixture must expose four comfortably spaced dynamic columns"
    );
    let lower = targets[6];
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: lower.rect.origin.x + 2,
            y: lower.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("focus lower item");
    let expected = first_row
        .into_iter()
        .min_by_key(|target| target.rect.origin.x.abs_diff(lower.rect.origin.x))
        .expect("aligned upper item");
    let moved = host
        .key_event(app(), window(), true, 111, 0, None)
        .expect("up key")
        .expect("navigation event");
    assert_eq!(moved.target, expected.widget);
}

#[test]
fn key_navigable_button_keeps_pointer_up_for_select_then_double_click_workflows() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(390),
            parent: None,
            index: 0,
            kind: WidgetKind::Button as u16,
            state: (WidgetState::ENABLED | WidgetState::KEY_NAVIGATION).bits(),
            text: Some(String::from("Process row")),
            icon: None,
        }],
    })
    .expect("button patch");
    let output = host
        .render(
            app(),
            window(),
            Constraints::new(
                Size {
                    width: 240,
                    height: 48,
                },
                Size {
                    width: 240,
                    height: 48,
                },
            )
            .expect("button constraints"),
        )
        .expect("button render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(390))
        .expect("button target");
    let point = Point {
        x: target.rect.origin.x + 2,
        y: target.rect.origin.y + 2,
    };
    host.pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0)
        .expect("pointer down");
    let event = host
        .pointer_event(app(), window(), PointerEventKind::Up, point, 1, 0)
        .expect("pointer up")
        .expect("pointer event");
    assert_eq!(event.target, widget(390));
    assert!(matches!(
        event.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Up,
            button: 1,
            ..
        }
    ));
    let repeated_press = host
        .pointer_event(app(), window(), PointerEventKind::Down, point, 1, 0x8000)
        .expect("second primary down")
        .expect("repeated primary press event");
    assert_eq!(repeated_press.target, widget(390));
    assert!(matches!(
        repeated_press.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Down,
            button: 1,
            modifiers: 0x8000,
            ..
        }
    ));
}

#[test]
fn primary_release_activates_only_the_control_that_owned_the_press() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(395),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(396),
                parent: Some(widget(395)),
                index: 0,
                kind: WidgetKind::CheckBox as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(String::from("Remember")),
                icon: None,
            },
            UiPatch::Create {
                id: widget(397),
                parent: Some(widget(395)),
                index: 1,
                kind: WidgetKind::TreeItem as u16,
                state: (WidgetState::ENABLED | WidgetState::KEY_NAVIGATION).bits(),
                text: Some(String::from("Other row")),
                icon: Some(rice_ui::IconId::Folder.wire_value()),
            },
        ],
    })
    .expect("controls patch");
    let output = host
        .render(
            app(),
            window(),
            Constraints::new(
                Size {
                    width: 240,
                    height: 80,
                },
                Size {
                    width: 240,
                    height: 80,
                },
            )
            .expect("control constraints"),
        )
        .expect("controls render");
    let center = |id| {
        let rect = output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .expect("hit target")
            .rect;
        Point {
            x: rect.origin.x + i32::try_from(rect.size.width / 2).unwrap(),
            y: rect.origin.y + i32::try_from(rect.size.height / 2).unwrap(),
        }
    };
    let checkbox = center(396);
    let other = center(397);
    host.pointer_event(app(), window(), PointerEventKind::Down, checkbox, 1, 0)
        .expect("checkbox down");
    let activation = host
        .pointer_event(app(), window(), PointerEventKind::Up, checkbox, 1, 0)
        .expect("checkbox up")
        .expect("checkbox activation");
    assert_eq!(activation.target, widget(396));
    assert_eq!(activation.event, UiEvent::Activate);
    let duplicate = host
        .pointer_event(app(), window(), PointerEventKind::Up, checkbox, 1, 0)
        .expect("duplicate up");
    assert!(
        !duplicate.is_some_and(|event| event.event == UiEvent::Activate),
        "a duplicate release must not toggle the checkbox a second time"
    );

    host.pointer_event(app(), window(), PointerEventKind::Down, checkbox, 1, 0)
        .expect("held down");
    host.pointer_event(app(), window(), PointerEventKind::Move, other, 0, 0x0100)
        .expect("held move");
    let dragged_release = host
        .pointer_event(app(), window(), PointerEventKind::Up, other, 1, 0)
        .expect("dragged release");
    assert!(
        !dragged_release.is_some_and(|event| event.event == UiEvent::Activate),
        "holding the mouse while crossing a tree row must not activate that row"
    );
}

#[test]
fn tree_keyboard_collapses_expands_and_moves_through_the_visible_hierarchy() {
    let mut host = NativeUiHost::default();
    let navigable = (WidgetState::ENABLED | WidgetState::KEY_NAVIGATION).bits();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(400),
                parent: None,
                index: 0,
                kind: WidgetKind::Tree as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(String::from("panel:fixed:width=320")),
                icon: None,
            },
            UiPatch::Create {
                id: widget(401),
                parent: Some(widget(400)),
                index: 0,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Root")),
                icon: Some(rice_ui::IconId::GoForward.wire_value()),
            },
            UiPatch::Create {
                id: widget(402),
                parent: Some(widget(401)),
                index: 0,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Child")),
                icon: Some(rice_ui::IconId::Document.wire_value()),
            },
            UiPatch::Create {
                id: widget(403),
                parent: Some(widget(400)),
                index: 1,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Sibling")),
                icon: Some(rice_ui::IconId::Document.wire_value()),
            },
        ],
    })
    .expect("tree patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 180,
        },
        Size {
            width: 360,
            height: 180,
        },
    )
    .expect("tree constraints");
    let collapsed = host
        .render(app(), window(), constraints)
        .expect("collapsed tree");
    let root = collapsed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(401))
        .expect("root target");
    assert!(
        !collapsed
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402))
    );
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: root.rect.origin.x + 2,
            y: root.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("focus root");
    let expand = host
        .key_event(app(), window(), true, 114, 0, None)
        .expect("right key")
        .expect("expand event");
    assert_eq!(expand.target, widget(401));
    assert_eq!(
        expand.event,
        UiEvent::Key {
            pressed: true,
            key_code: 114,
            modifiers: 0,
            text: None,
        }
    );

    let expanded = host
        .render(app(), window(), constraints)
        .expect("locally expanded tree");
    assert!(
        expanded
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402))
    );
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![
            UiPatch::Clear,
            UiPatch::Create {
                id: widget(400),
                parent: None,
                index: 0,
                kind: WidgetKind::Tree as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(String::from("panel:fixed:width=320")),
                icon: None,
            },
            UiPatch::Create {
                id: widget(401),
                parent: Some(widget(400)),
                index: 0,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Root rebuilt")),
                icon: Some(rice_ui::IconId::GoForward.wire_value()),
            },
            UiPatch::Create {
                id: widget(402),
                parent: Some(widget(401)),
                index: 0,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Child")),
                icon: Some(rice_ui::IconId::Document.wire_value()),
            },
            UiPatch::Create {
                id: widget(403),
                parent: Some(widget(400)),
                index: 1,
                kind: WidgetKind::TreeItem as u16,
                state: navigable,
                text: Some(String::from("Sibling")),
                icon: Some(rice_ui::IconId::Document.wire_value()),
            },
        ],
    })
    .expect("application tree rebuild");
    let rebuilt = host
        .render(app(), window(), constraints)
        .expect("rebuilt expanded tree");
    assert!(
        rebuilt
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402)),
        "user expansion must survive an application Clear/Create redraw"
    );
    let child = host
        .key_event(app(), window(), true, 114, 0, None)
        .expect("right into child")
        .expect("child focus");
    assert_eq!(child.target, widget(402));
    assert_eq!(child.event, UiEvent::Focus { focused: true });
    let parent = host
        .key_event(app(), window(), true, 113, 0, None)
        .expect("left to parent")
        .expect("parent focus");
    assert_eq!(parent.target, widget(401));
    assert_eq!(parent.event, UiEvent::Focus { focused: true });
    let collapse = host
        .key_event(app(), window(), true, 113, 0, None)
        .expect("left collapse")
        .expect("collapse event");
    assert_eq!(collapse.target, widget(401));
    assert_eq!(
        collapse.event,
        UiEvent::Key {
            pressed: true,
            key_code: 113,
            modifiers: 0,
            text: None,
        }
    );
    let collapsed_again = host
        .render(app(), window(), constraints)
        .expect("locally collapsed tree");
    assert!(
        !collapsed_again
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402))
    );
    let sibling = host
        .key_event(app(), window(), true, 116, 0, None)
        .expect("down from collapsed root")
        .expect("sibling focus");
    assert_eq!(sibling.target, widget(403));
    assert_eq!(sibling.event, UiEvent::Focus { focused: true });
    let root_focus = host
        .key_event(app(), window(), true, 111, 0, None)
        .expect("up from sibling")
        .expect("root focus");
    assert_eq!(root_focus.target, widget(401));
    assert_eq!(root_focus.event, UiEvent::Focus { focused: true });

    let root = collapsed_again
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(401))
        .expect("collapsed root target");
    let reopened = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Up,
            Point {
                x: root.rect.origin.x + 12,
                y: root.rect.origin.y + 2,
            },
            1,
            0,
        )
        .expect("click collapsed branch")
        .expect("branch activation");
    assert_eq!(reopened.target, widget(401));
    assert!(matches!(
        reopened.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Up,
            ..
        }
    ));
    let reopened = host
        .render(app(), window(), constraints)
        .expect("pointer-reopened tree");
    assert!(
        reopened
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402))
    );

    let root = reopened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(401))
        .expect("reopened root target");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: root.rect.origin.x + 100,
            y: root.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("press branch label");
    let label_activation = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Up,
            Point {
                x: root.rect.origin.x + 100,
                y: root.rect.origin.y + 2,
            },
            1,
            0,
        )
        .expect("click branch label")
        .expect("branch label activation");
    assert_eq!(label_activation.target, widget(401));
    assert_eq!(label_activation.event, UiEvent::Activate);
    let still_open = host
        .render(app(), window(), constraints)
        .expect("tree after branch label activation");
    assert!(
        still_open
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(402))
    );
}

#[test]
fn scrollbar_drag_emits_bounded_values_and_captures_the_pointer() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::ScrollBar as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("0/100".into()),
            icon: None,
        }],
    })
    .expect("scrollbar patch");
    let constraints = Constraints::new(
        Size {
            width: 20,
            height: 200,
        },
        Size {
            width: 20,
            height: 200,
        },
    )
    .expect("fixed constraints");
    let target = host
        .render(app(), window(), constraints)
        .expect("render")
        .hit_targets[0];
    let middle = Point {
        x: target.rect.origin.x + 10,
        y: target.rect.origin.y + 100,
    };
    let down = host
        .pointer_event(app(), window(), PointerEventKind::Down, middle, 1, 0)
        .expect("down")
        .expect("value");
    assert_eq!(down.event, UiEvent::ValueChanged("50/100".into()));
    let beyond = Point {
        x: middle.x,
        y: target.rect.origin.y + 300,
    };
    let drag = host
        .pointer_event(app(), window(), PointerEventKind::Move, beyond, 0, 0)
        .expect("captured move")
        .expect("value");
    assert_eq!(drag.event, UiEvent::ValueChanged("100/100".into()));
    let up = host
        .pointer_event(app(), window(), PointerEventKind::Up, beyond, 1, 0)
        .expect("captured up")
        .expect("value");
    assert_eq!(up.event, UiEvent::ValueChanged("100/100".into()));
}

#[test]
fn overflowing_scroll_view_paints_and_drags_a_reserved_vertical_gutter() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("0/800".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::List as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(3),
                parent: Some(widget(2)),
                index: 0,
                kind: WidgetKind::Button as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("First row".into()),
                icon: None,
            },
        ],
    })
    .expect("scroll view patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 160,
        },
        Size {
            width: 240,
            height: 160,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let gutter = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("scroll view gutter");
    let list = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("list target");
    assert_eq!(
        gutter.rect.size.width,
        u32::from(host.theme().metrics.scrollbar_width)
    );
    assert!(
        list.rect
            .origin
            .x
            .saturating_add_unsigned(list.rect.size.width)
            <= gutter.rect.origin.x
    );
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, .. } if rect.size.width == gutter.rect.size.width
            && rect.size.height == gutter.rect.size.height
    )));

    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: gutter.rect.origin.x + 1,
            y: gutter.rect.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover scrollbar gutter");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("hovered scrollbar render")
            .cursor,
        PointerCursor::Pointer,
    );

    let wheel = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Wheel,
            Point {
                x: list.rect.origin.x + 2,
                y: list.rect.origin.y + 2,
            },
            5,
            0,
        )
        .expect("wheel over scroll child")
        .expect("scroll value");
    assert_eq!(wheel.target, widget(1));
    let UiEvent::ValueChanged(value) = wheel.event else {
        panic!("wheel scroll view value event");
    };
    let (position, maximum) = value.split_once('/').expect("wheel fraction");
    assert_eq!(position, "96");
    let maximum = maximum.parse::<u32>().expect("geometry maximum");
    assert!(maximum >= 96);
    assert!(maximum < 800, "wheel maximum comes from rendered geometry");

    let bottom = Point {
        x: gutter.rect.origin.x + i32::try_from(gutter.rect.size.width / 2).unwrap_or_default(),
        y: gutter.rect.origin.y
            + i32::try_from(gutter.rect.size.height.saturating_sub(1)).unwrap_or_default(),
    };
    let event = host
        .pointer_event(app(), window(), PointerEventKind::Down, bottom, 1, 0)
        .expect("scroll down")
        .expect("scroll value");
    let UiEvent::ValueChanged(value) = event.event else {
        panic!("scroll view value event");
    };
    let (position, total) = value.split_once('/').expect("scroll fraction");
    assert_eq!(position, total);
    assert!(total.parse::<u32>().expect("bounded total") > 0);
}

#[test]
fn repeated_wheel_scroll_survives_blank_content_gaps() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("0/800".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(3),
                parent: Some(widget(2)),
                index: 0,
                kind: WidgetKind::Label as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Top row".into()),
                icon: None,
            },
        ],
    })
    .expect("scroll gap fixture");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 160,
        },
        Size {
            width: 240,
            height: 160,
        },
    )
    .expect("fixed constraints");
    host.render(app(), window(), constraints)
        .expect("initial render");
    let gap = Point { x: 120, y: 140 };
    assert!(host.hit_test(app(), window(), gap).is_none());

    let first = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, gap, 5, 0)
        .expect("first wheel")
        .expect("first scroll value");
    assert_eq!(first.target, widget(1));
    assert!(matches!(&first.event, UiEvent::ValueChanged(value) if value.starts_with("96/")));

    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set("96/800".into()),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("retain first scroll position");
    host.render(app(), window(), constraints)
        .expect("scrolled render");
    let second = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, gap, 5, 0)
        .expect("second wheel")
        .expect("second scroll value");
    assert!(matches!(&second.event, UiEvent::ValueChanged(value) if value.starts_with("192/")));
}

#[test]
fn virtual_scroll_view_keeps_a_full_large_range_with_a_bounded_child_window() {
    let mut host = NativeUiHost::default();
    let document = rice_ui::editor_text(
        "log",
        rice_ui::EditorWrap::None,
        false,
        &(0..64)
            .map(|row| format!("visible timeline row {row}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("virtual-window:0|0/240000".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: (WidgetState::ENABLED | WidgetState::START_ALIGNED).bits(),
                text: Some(document),
                icon: None,
            },
        ],
    })
    .expect("virtual scroll patch");
    let constraints = Constraints::new(
        Size {
            width: 480,
            height: 240,
        },
        Size {
            width: 480,
            height: 240,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let scrollbar = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .copied()
        .expect("virtual scrollbar");
    let point = Point { x: 100, y: 100 };
    let wheel = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, point, 5, 0)
        .expect("wheel")
        .expect("virtual scroll value");
    assert!(matches!(
        &wheel.event,
        UiEvent::ValueChanged(value)
            if value.starts_with("96/")
                && value.split_once('/').unwrap().1.parse::<u32>().unwrap() > 239_000
    ));

    let bottom = Point {
        x: scrollbar
            .rect
            .origin
            .x
            .saturating_add_unsigned(scrollbar.rect.size.width / 2),
        y: scrollbar
            .rect
            .origin
            .y
            .saturating_add_unsigned(scrollbar.rect.size.height.saturating_sub(1)),
    };
    let dragged = host
        .pointer_event(app(), window(), PointerEventKind::Down, bottom, 1, 0)
        .expect("drag")
        .expect("bottom value");
    let UiEvent::ValueChanged(bottom_value) = dragged.event else {
        panic!("virtual scrollbar value");
    };
    let (position, maximum) = bottom_value.split_once('/').expect("scroll range");
    assert_eq!(position, maximum);

    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set(format!(
                "virtual-window:{position}|{position}/240000"
            )),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("bottom virtual window");
    host.render(app(), window(), constraints)
        .expect("bottom render");
    let upward = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, point, 4, 0)
        .expect("upward wheel")
        .expect("upward value");
    assert!(matches!(
        upward.event,
        UiEvent::ValueChanged(value) if value.split_once('/').is_some_and(|(next, total)| {
            next.parse::<u32>().unwrap() + 96 == total.parse::<u32>().unwrap()
        })
    ));
}

#[test]
fn wheel_over_non_scrolling_image_reaches_the_image_actor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::Image as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(String::from("media-request:7:0:fit:1000:0:0")),
                icon: None,
            },
        ],
    })
    .expect("image viewport patch");
    let constraints = Constraints::new(
        Size {
            width: 800,
            height: 600,
        },
        Size {
            width: 800,
            height: 600,
        },
    )
    .expect("image viewport constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let image = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("image hit target");
    let point = Point {
        x: image.rect.origin.x.saturating_add(10),
        y: image.rect.origin.y.saturating_add(10),
    };
    let wheel = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, point, 4, 0)
        .expect("wheel input")
        .expect("wheel event");
    assert_eq!(wheel.target, widget(2));
    assert!(matches!(
        wheel.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Wheel,
            button: 4,
            ..
        }
    ));
}

#[test]
fn ctrl_wheel_bypasses_native_scroll_and_reaches_the_content_actor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("0/800".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Zoom me".into()),
                icon: None,
            },
        ],
    })
    .expect("scrolling editor fixture");
    let constraints = Constraints::new(
        Size {
            width: 480,
            height: 240,
        },
        Size {
            width: 480,
            height: 240,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let editor = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("editor target");
    let point = Point {
        x: editor.rect.origin.x.saturating_add(8),
        y: editor.rect.origin.y.saturating_add(8),
    };
    let wheel = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, point, 4, 4)
        .expect("Ctrl+wheel")
        .expect("application event");
    assert_eq!(wheel.target, widget(2));
    assert!(matches!(
        wheel.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Wheel,
            button: 4,
            modifiers: 4,
            ..
        }
    ));
}

#[test]
fn wheel_over_web_surface_bypasses_enclosing_native_scroll_view() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(1),
                parent: None,
                index: 0,
                kind: WidgetKind::ScrollView as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("0/800".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(2),
                parent: Some(widget(1)),
                index: 0,
                kind: WidgetKind::WebSurface as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
        ],
    })
    .expect("web surface fixture");
    let constraints = Constraints::new(
        Size {
            width: 800,
            height: 600,
        },
        Size {
            width: 800,
            height: 600,
        },
    )
    .expect("web surface constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let surface = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("web surface hit target");
    let point = Point {
        x: surface.rect.origin.x.saturating_add(10),
        y: surface.rect.origin.y.saturating_add(10),
    };

    let wheel = host
        .pointer_event(app(), window(), PointerEventKind::Wheel, point, 5, 0)
        .expect("wheel input")
        .expect("web surface wheel event");

    assert_eq!(wheel.target, widget(2));
    assert!(matches!(
        wheel.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Wheel,
            button: 5,
            ..
        }
    ));
}

#[test]
fn geometry_backed_scroll_thumb_tracks_actor_positions_with_unknown_extent() {
    let mut patches = vec![
        UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::ScrollView as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("0/0".into()),
            icon: None,
        },
        UiPatch::Create {
            id: widget(2),
            parent: Some(widget(1)),
            index: 0,
            kind: WidgetKind::List as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        },
    ];
    for index in 0..16_u64 {
        patches.push(UiPatch::Create {
            id: widget(10 + index),
            parent: Some(widget(2)),
            index: u32::try_from(index).expect("row index"),
            kind: WidgetKind::Button as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some(format!("Package {}", index + 1)),
            icon: None,
        });
    }
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches,
    })
    .expect("scroll fixture");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 160,
        },
        Size {
            width: 240,
            height: 160,
        },
    )
    .expect("fixed constraints");
    let before = host
        .render(app(), window(), constraints)
        .expect("initial render");
    let gutter = before
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("scroll gutter")
        .rect;
    let muted = host.theme().palette.text_muted;
    let thumb = |output: &RenderOutput| {
        output.commands.iter().find_map(|command| match command {
            DrawCommand::Fill { rect, color, .. }
                if *color == muted
                    && rect.origin.x == gutter.origin.x
                    && rect.size.width == gutter.size.width =>
            {
                Some(*rect)
            }
            _ => None,
        })
    };
    let before_thumb = thumb(&before).expect("initial scroll thumb");
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 2,
        patches: vec![UiPatch::Update {
            id: widget(1),
            state: rice_native_app_protocol::PatchValue::Keep,
            text: rice_native_app_protocol::PatchValue::Set("96/0".into()),
            icon: rice_native_app_protocol::PatchValue::Keep,
        }],
    })
    .expect("scroll position patch");
    let after = host
        .render(app(), window(), constraints)
        .expect("scrolled render");
    let after_thumb = thumb(&after).expect("scrolled thumb");
    assert!(after_thumb.origin.y > before_thumb.origin.y);
    assert!(
        after_thumb
            .origin
            .y
            .saturating_add_unsigned(after_thumb.size.height)
            < gutter.origin.y.saturating_add_unsigned(gutter.size.height),
        "a partial content offset must not pin the thumb to the track end"
    );
}

#[test]
fn bounded_nested_scroll_view_keeps_its_viewport_and_handles_its_own_wheel() {
    let mut patches = vec![
        UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::Column as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        },
        UiPatch::Create {
            id: widget(2),
            parent: Some(widget(1)),
            index: 0,
            kind: WidgetKind::Label as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("APT transaction".into()),
            icon: None,
        },
        UiPatch::Create {
            id: widget(3),
            parent: Some(widget(1)),
            index: 1,
            kind: WidgetKind::ScrollView as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("viewport:180|0/0".into()),
            icon: None,
        },
        UiPatch::Create {
            id: widget(4),
            parent: Some(widget(3)),
            index: 0,
            kind: WidgetKind::List as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        },
    ];
    for index in 0..12_u64 {
        patches.push(UiPatch::Create {
            id: widget(10 + index),
            parent: Some(widget(4)),
            index: u32::try_from(index).expect("log line index"),
            kind: WidgetKind::Button as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some(format!("APT output line {}", index + 1)),
            icon: None,
        });
    }
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches,
    })
    .expect("bounded log scroll patch");
    let constraints = Constraints::new(
        Size {
            width: 360,
            height: 420,
        },
        Size {
            width: 360,
            height: 420,
        },
    )
    .expect("fixed constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    let gutter = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("bounded log scrollbar gutter");
    assert!(
        gutter.rect.size.height <= 180,
        "bounded log viewport must not expand into spare window height"
    );
    let first_line = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(10))
        .expect("visible first log line");
    let wheel = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Wheel,
            Point {
                x: first_line.rect.origin.x + 2,
                y: first_line.rect.origin.y + 2,
            },
            5,
            0,
        )
        .expect("wheel over log")
        .expect("log scroll value");
    assert_eq!(wheel.target, widget(3));
    assert!(matches!(wheel.event, UiEvent::ValueChanged(ref value) if value.starts_with("96/")));
}

#[test]
fn hovering_a_split_handle_exports_the_resize_cursor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(10),
                parent: None,
                index: 0,
                kind: WidgetKind::SplitPane as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("split:500".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(11),
                parent: Some(widget(10)),
                index: 0,
                kind: WidgetKind::Container as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Left".into()),
                icon: None,
            },
            UiPatch::Create {
                id: widget(12),
                parent: Some(widget(10)),
                index: 1,
                kind: WidgetKind::Container as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("Right".into()),
                icon: None,
            },
        ],
    })
    .expect("split patch");
    let constraints = Constraints::new(
        Size {
            width: 640,
            height: 240,
        },
        Size {
            width: 640,
            height: 240,
        },
    )
    .expect("fixed constraints");
    let initial = host.render(app(), window(), constraints).expect("render");
    assert_eq!(initial.cursor, PointerCursor::Default);
    let handle = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(10))
        .expect("split handle")
        .rect;
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: handle.origin.x + i32::try_from(handle.size.width / 2).unwrap_or_default(),
            y: handle.origin.y + i32::try_from(handle.size.height / 2).unwrap_or_default(),
        },
        0,
        0,
    )
    .expect("hover handle");
    let hovered = host
        .render(app(), window(), constraints)
        .expect("hovered render");
    assert_eq!(hovered.cursor, PointerCursor::ColumnResize);
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Leave,
        Point { x: -1, y: -1 },
        0,
        0,
    )
    .expect("leave native content");
    let left = host
        .render(app(), window(), constraints)
        .expect("left render");
    assert_eq!(left.cursor, PointerCursor::Default);
}

#[test]
fn progress_bar_click_and_drag_emit_seek_positions() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::ProgressBar as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("seek:25/100".into()),
            icon: None,
        }],
    })
    .expect("progress patch");
    let constraints = Constraints::new(
        Size {
            width: 200,
            height: 20,
        },
        Size {
            width: 200,
            height: 20,
        },
    )
    .expect("fixed constraints");
    let target = host
        .render(app(), window(), constraints)
        .expect("render")
        .hit_targets[0];
    let three_quarters = Point {
        x: target.rect.origin.x + 150,
        y: target.rect.origin.y + 10,
    };
    let down = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Down,
            three_quarters,
            1,
            0,
        )
        .expect("down")
        .expect("seek value");
    assert_eq!(down.event, UiEvent::ValueChanged("75/100".into()));
    let end = Point {
        x: target.rect.origin.x + 250,
        y: three_quarters.y,
    };
    let drag = host
        .pointer_event(app(), window(), PointerEventKind::Move, end, 0, 0)
        .expect("captured move")
        .expect("seek value");
    assert_eq!(drag.event, UiEvent::ValueChanged("100/100".into()));
    host.pointer_event(app(), window(), PointerEventKind::Up, end, 1, 0)
        .expect("captured up")
        .expect("seek value");
}

#[test]
fn hovering_an_enabled_button_exports_the_pointer_cursor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::Button as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("Open".into()),
            icon: None,
        }],
    })
    .expect("button patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 80,
        },
        Size {
            width: 240,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let initial = host.render(app(), window(), constraints).expect("render");
    let target = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("button target");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: target.rect.origin.x + 1,
            y: target.rect.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover button");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("hovered render")
            .cursor,
        PointerCursor::Pointer,
    );
}

#[test]
fn redmond_button_hover_draws_inner_dotted_focus_and_space_activates_it() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::Button as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("OK".into()),
            icon: None,
        }],
    })
    .expect("Redmond button patch");
    let constraints = Constraints::new(
        Size {
            width: 160,
            height: 60,
        },
        Size {
            width: 160,
            height: 60,
        },
    )
    .expect("fixed constraints");
    let target = host
        .render(app(), window(), constraints)
        .expect("initial render")
        .hit_targets[0];
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: target.rect.origin.x.saturating_add(8),
            y: target.rect.origin.y.saturating_add(8),
        },
        0,
        0,
    )
    .expect("hover button");
    let hovered = host
        .render(app(), window(), constraints)
        .expect("hovered render");
    assert_eq!(
        hovered
            .commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::DottedLine { .. }))
            .count(),
        4,
        "the classic inner focus rectangle has four dotted edges"
    );
    let event = host
        .key_event(app(), window(), true, 65, 0, Some(" ".into()))
        .expect("browser Space key with committed text")
        .expect("button activation");
    assert_eq!(event.target, widget(1));
    assert_eq!(event.event, UiEvent::Activate);
}

#[test]
fn hovering_empty_collection_space_keeps_the_default_cursor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::List as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .expect("list patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 160,
        },
        Size {
            width: 240,
            height: 160,
        },
    )
    .expect("fixed constraints");
    let initial = host.render(app(), window(), constraints).expect("render");
    let collection = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("list background target");
    assert!(collection.focusable);
    assert!(!collection.activatable);
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: collection.rect.origin.x + 1,
            y: collection.rect.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover empty list background");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("hovered render")
            .cursor,
        PointerCursor::Default,
        "focusable list background is not a clickable item",
    );
}

#[test]
fn hovering_an_enabled_slider_exports_the_pointer_cursor() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::Slider as u16,
            state: WidgetState::ENABLED.bits(),
            text: Some("250/1000".into()),
            icon: None,
        }],
    })
    .expect("slider patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 80,
        },
        Size {
            width: 240,
            height: 80,
        },
    )
    .expect("fixed constraints");
    let initial = host.render(app(), window(), constraints).expect("render");
    let target = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("slider target");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: target.rect.origin.x + 1,
            y: target.rect.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover slider");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("hovered render")
            .cursor,
        PointerCursor::Pointer,
    );
}

#[test]
fn password_field_masks_rendering_but_edits_the_real_typed_value() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            UiPatch::Create {
                id: widget(10),
                parent: None,
                index: 0,
                kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(),
                text: None,
                icon: None,
            },
            UiPatch::Create {
                id: widget(11),
                parent: Some(widget(10)),
                index: 0,
                kind: WidgetKind::PasswordField as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some("secret".into()),
                icon: None,
            },
        ],
    })
    .expect("password patch");
    let constraints = Constraints::new(
        Size {
            width: 240,
            height: 80,
        },
        Size {
            width: 240,
            height: 80,
        },
    )
    .expect("constraints");
    let output = host.render(app(), window(), constraints).expect("render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "••••••"
    )));
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(11))
        .expect("password target");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: target
                .rect
                .origin
                .x
                .saturating_add_unsigned(target.rect.size.width),
            y: target.rect.origin.y,
        },
        1,
        0,
    )
    .expect("focus");
    let append = host
        .key_event(app(), window(), true, 0, 0, Some("!".into()))
        .expect("key")
        .expect("value");
    assert_eq!(append.event, UiEvent::ValueChanged("secret!".into()));
}

#[test]
fn web_surface_uses_pointer_for_every_retained_interactive_region() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::WebSurface as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .expect("web surface");
    let mut link = web_region_payload([10.0, 10.0, 120.0, 30.0], 2);
    let url = b"https://example.test/result";
    link.extend(u32::try_from(url.len()).unwrap().to_le_bytes());
    link.extend(url);
    link.resize(link.len().next_multiple_of(4), 0);
    let mut input = web_region_payload([10.0, 60.0, 180.0, 36.0], 3);
    input.extend([2, 0, 0, 0]);
    input.extend(0u32.to_le_bytes());
    let mut display_list = web_command(rice_web_render_protocol::LINK_REGION, &link);
    display_list.extend(web_command(rice_web_render_protocol::INPUT_REGION, &input));
    host.apply_web_render(&WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(1),
        epoch: 1,
        viewport_width: 320,
        viewport_height: 200,
        display_list,
    })
    .expect("retained web scene");
    let constraints = Constraints::new(
        Size {
            width: 320,
            height: 200,
        },
        Size {
            width: 320,
            height: 200,
        },
    )
    .expect("fixed constraints");
    let initial = host
        .render(app(), window(), constraints)
        .expect("initial render");
    let surface = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("web hit target")
        .rect;
    let at = |x: i32, y: i32| Point {
        x: surface.origin.x + x,
        y: surface.origin.y + y,
    };

    host.pointer_event(app(), window(), PointerEventKind::Move, at(20, 20), 0, 0)
        .expect("hover link");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("link cursor")
            .cursor,
        PointerCursor::Pointer
    );
    host.pointer_event(app(), window(), PointerEventKind::Move, at(20, 70), 0, 0)
        .expect("hover input");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("interactive input cursor")
            .cursor,
        PointerCursor::Pointer,
        "editable inputs follow the base interactive-control cursor rule",
    );
    host.pointer_event(app(), window(), PointerEventKind::Move, at(250, 150), 0, 0)
        .expect("hover page background");
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("default cursor")
            .cursor,
        PointerCursor::Default
    );
    let leave = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Leave,
            Point { x: -1, y: -1 },
            0,
            0,
        )
        .expect("leave surface")
        .expect("web surface leave envelope");
    assert_eq!(leave.target, widget(1));
    assert!(matches!(
        leave.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Leave,
            ..
        }
    ));
    assert_eq!(
        host.render(app(), window(), constraints)
            .expect("left cursor")
            .cursor,
        PointerCursor::Default
    );
}

#[test]
fn scaled_web_surface_emits_scene_logical_pointer_coordinates() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::WebSurface as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .expect("scaled web surface");
    host.apply_web_render(&WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(1),
        epoch: 1,
        viewport_width: 320,
        viewport_height: 200,
        display_list: web_solid_rect([0.0, 0.0, 320.0, 200.0], 0xff00_00ff),
    })
    .expect("scaled retained scene");
    let constraints = Constraints::new(
        Size {
            width: 640,
            height: 200,
        },
        Size {
            width: 640,
            height: 200,
        },
    )
    .expect("scaled constraints");
    let output = host
        .render(app(), window(), constraints)
        .expect("scaled render");
    let surface = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("web surface target")
        .rect;
    assert_eq!(
        surface.size,
        Size {
            width: 640,
            height: 200
        }
    );
    let event = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Up,
            Point {
                x: surface.origin.x + 80,
                y: surface.origin.y + 100,
            },
            1,
            0,
        )
        .expect("scaled pointer")
        .expect("web pointer envelope");
    assert!(matches!(event.event, UiEvent::Pointer { x: 40, y: 50, .. }));
}

#[test]
fn web_title_regions_render_browser_style_tooltips() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![UiPatch::Create {
            id: widget(1),
            parent: None,
            index: 0,
            kind: WidgetKind::WebSurface as u16,
            state: WidgetState::ENABLED.bits(),
            text: None,
            icon: None,
        }],
    })
    .unwrap();
    let mut title = web_region_payload([10.0, 10.0, 140.0, 32.0], 2);
    let text = b"Product details";
    title.extend((text.len() as u32).to_le_bytes());
    title.extend(text);
    title.resize(title.len().next_multiple_of(4), 0);
    host.apply_web_render(&WebRenderPatch {
        app: app(),
        window: window(),
        surface: widget(1),
        epoch: 1,
        viewport_width: 320,
        viewport_height: 200,
        display_list: web_command(rice_web_render_protocol::TITLE_REGION, &title),
    })
    .unwrap();
    let constraints = Constraints::new(
        Size {
            width: 320,
            height: 200,
        },
        Size {
            width: 320,
            height: 200,
        },
    )
    .unwrap();
    let initial = host.render(app(), window(), constraints).unwrap();
    let surface = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .unwrap()
        .rect;
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: surface.origin.x + 20,
            y: surface.origin.y + 20,
        },
        0,
        0,
    )
    .unwrap();
    let immediate = host.render(app(), window(), constraints).unwrap();
    assert!(immediate.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Product details"
    )));
    let hovered = host.render(app(), window(), constraints).unwrap();
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Product details"
    )));
}

#[test]
fn reorder_grips_capture_motion_report_drop_slots_and_release() {
    let mut host=NativeUiHost::default();
    let mut patches=vec![UiPatch::Create{id:widget(1),parent:None,index:0,kind:WidgetKind::Row as u16,state:WidgetState::ENABLED.bits(),text:None,icon:None}];
    for i in 0..3 {patches.push(UiPatch::Create{id:widget(2+i),parent:Some(widget(1)),index:i as u32,kind:WidgetKind::Button as u16,state:WidgetState::ENABLED.bits(),text:Some(format!("drag-handle:palette:{i}")),icon:None});}
    host.apply_patch(&UiPatchEnvelope{app:app(),window:window(),sequence:1,patches}).unwrap();
    let size=Size{width:300,height:44};let constraints=Constraints::new(size,size).unwrap();
    let output=host.render(app(),window(),constraints).unwrap();
    let point=|id|{let rect=output.hit_targets.iter().find(|t|t.widget==widget(id)).unwrap().rect;Point{x:rect.origin.x+5,y:rect.origin.y+5}};
    host.pointer_event(app(),window(),PointerEventKind::Down,point(2),1,0).unwrap();
    assert!(host.pointer_motion_continuous(app(),window()));
    let moved=host.pointer_event(app(),window(),PointerEventKind::Move,point(4),0,0).unwrap().unwrap();
    assert_eq!(moved.target,widget(2));
    assert_eq!(moved.event,UiEvent::ValueChanged("preview:2".into()));
    let dropped=host.pointer_event(app(),window(),PointerEventKind::Up,point(4),1,0).unwrap().unwrap();
    assert_eq!(dropped.event,UiEvent::ValueChanged("drop:2".into()));
    assert!(!host.pointer_motion_continuous(app(),window()));
    assert!(!output.commands.iter().any(|c| matches!(c,DrawCommand::Text{text,..} if text.starts_with("drag-handle:"))));
}

#[test]
fn positioned_surface_uses_themed_children_and_captures_canvas_coordinates() {
    let mut host=NativeUiHost::default();
    let node=|id:u64,parent:Option<u64>,kind:WidgetKind,text:&str| UiPatch::Create{id:widget(id),parent:parent.map(widget),index:0,kind:kind as u16,state:WidgetState::ENABLED.bits(),text:Some(text.into()),icon:None};
    host.apply_patch(&UiPatchEnvelope{app:app(),window:window(),sequence:1,patches:vec![
        node(1,None,WidgetKind::Stack,"positioned-canvas:240"),
        node(2,Some(1),WidgetKind::Stack,"positioned-item:100:100:300:400"),
        node(3,Some(2),WidgetKind::Button,"Monitor one"),
        node(4,Some(1),WidgetKind::Stack,"positioned-item:500:300:300:400"),
        node(5,Some(4),WidgetKind::Button,"Monitor two"),
    ]}).unwrap();
    let size=Size{width:620,height:240};let constraints=Constraints::new(size,size).unwrap();
    let output=host.render(app(),window(),constraints).unwrap();
    let rect=|id| output.hit_targets.iter().find(|hit|hit.widget==widget(id)).unwrap().rect;
    assert_eq!(rect(3).size,Size{width:186,height:96},"Container padding must not distort monitor proportions");
    assert!(rect(5).origin.x>rect(3).origin.x);assert!(rect(5).origin.y>rect(3).origin.y);
    let point=Point{x:rect(3).origin.x+10,y:rect(3).origin.y+10};
    let pressed=host.pointer_event(app(),window(),PointerEventKind::Down,point,1,0).unwrap().unwrap();
    assert_eq!(pressed.target,widget(3));assert!(matches!(pressed.event,UiEvent::Pointer{kind:PointerEventKind::Down,..}));
    assert!(host.pointer_motion_continuous(app(),window()));
    let moved=host.pointer_event(app(),window(),PointerEventKind::Move,Point{x:500,y:200},0,0).unwrap().unwrap();
    assert_eq!(moved.target,widget(3));assert!(matches!(moved.event,UiEvent::Pointer{kind:PointerEventKind::Move,x,y,..} if x>500&&y>500));
    host.pointer_event(app(),window(),PointerEventKind::Up,Point{x:500,y:200},1,0).unwrap();
    assert!(!host.pointer_motion_continuous(app(),window()));
    assert!(!output.commands.iter().any(|command|matches!(command,DrawCommand::Text{text,..} if text.starts_with("positioned-"))));
}

#[test]
fn unrelated_progress_updates_preserve_user_keyboard_focus() {
    use rice_native_app_protocol::PatchValue;
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1,
        patches: vec![
            UiPatch::Create { id: widget(1), parent: None, index: 0, kind: WidgetKind::Column as u16,
                state: WidgetState::ENABLED.bits(), text: None, icon: None },
            UiPatch::Create { id: widget(2), parent: Some(widget(1)), index: 0, kind: WidgetKind::Button as u16,
                state: (WidgetState::ENABLED | WidgetState::FOCUSED).bits(), text: Some("Selected file".into()), icon: None },
            UiPatch::Create { id: widget(3), parent: Some(widget(1)), index: 1, kind: WidgetKind::Image as u16,
                state: WidgetState::ENABLED.bits(), text: None, icon: None },
            UiPatch::Create { id: widget(4), parent: Some(widget(1)), index: 2, kind: WidgetKind::ProgressBar as u16,
                state: WidgetState::ENABLED.bits(), text: Some("seek:0/60000".into()), icon: None },
        ],
    }).unwrap();
    let size = Size { width: 400, height: 300 };
    host.render(app(), window(), Constraints::new(size, size).unwrap()).unwrap();
    let focused = host.key_event(app(), window(), true, 23, 0, None).unwrap().unwrap();
    assert_eq!(focused.target, widget(3));
    host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 2,
        patches: vec![UiPatch::Update { id: widget(4), state: PatchValue::Keep,
            text: PatchValue::Set("seek:5000/60000".into()), icon: PatchValue::Keep }],
    }).unwrap();
    let seek = host.key_event(app(), window(), true, 113, 0, None).unwrap().unwrap();
    assert_eq!(seek.target, widget(3), "progress must not steal focus from the preview");
}


#[test]
fn text_fields_forward_function_keys_and_toggle_formula_point_mode() {
    use rice_native_app_protocol::PatchValue;
    for kind in [WidgetKind::TextField, WidgetKind::PasswordField] {
        let mut host = NativeUiHost::default();
        let state = WidgetState::ENABLED | WidgetState::FOCUSED;
        host.apply_patch(&UiPatchEnvelope {
            app: app(), window: window(), sequence: 1,
            patches: vec![UiPatch::Create {
                id: widget(1), parent: None, index: 0, kind: kind as u16,
                state: state.bits(), text: Some("=B1+C1".into()), icon: None,
            }],
        }).expect("focused formula editor");
        let size = Size { width: 320, height: 40 };
        host.render(app(), window(), Constraints::new(size, size).unwrap()).unwrap();
        for key_code in [67, 68, 69, 96, 111, 116, 112, 117, 118, 999] {
            for pressed in [true, false] {
                let event = host.key_event(app(), window(), pressed, key_code, 0, None)
                    .unwrap().expect("nonprinting key forwarded");
                assert_eq!(event.target, widget(1));
                assert_eq!(event.event, UiEvent::Key { pressed, key_code, modifiers: 0, text: None });
            }
        }
        // The app receives F2 above, then opts into formula reference arrows.
        // This patch invalidates geometry; focus and the next key still survive.
        host.apply_patch(&UiPatchEnvelope {
            app: app(), window: window(), sequence: 2,
            patches: vec![UiPatch::Update {
                id: widget(1), state: PatchValue::Set((state | WidgetState::KEY_NAVIGATION).bits()),
                text: PatchValue::Keep, icon: PatchValue::Keep,
            }],
        }).expect("enter point mode");
        let arrow = host.key_event(app(), window(), true, 114, 0, None).unwrap().unwrap();
        assert_eq!(arrow.event, UiEvent::Key { pressed: true, key_code: 114, modifiers: 0, text: None });
        let unicode = host.key_event(app(), window(), true, 0, 0, Some("é米".into())).unwrap().unwrap();
        assert_eq!(unicode.event, UiEvent::ValueChanged("=B1+C1é米".into()));
        let backspace = host.key_event(app(), window(), true, 22, 0, None).unwrap().unwrap();
        assert_eq!(backspace.event, UiEvent::ValueChanged("=B1+C".into()));
    }
}

#[test]
fn positioned_overlay_keeps_grid_geometry_and_paints_only_the_cell_editor() {
    use rice_native_app_protocol::PatchValue;
    let frame = r#"{"version":1,"revision":1,"sheet":1,"row_count":10,"column_count":10,"width":240,"height":120,"row_header":40,"column_header":20,"rows":[{"index":0,"start":0,"extent":30,"clip_start":0,"clip_end":30,"frozen":false}],"columns":[{"index":0,"start":0,"extent":100,"clip_start":0,"clip_end":100,"frozen":false}],"strings":[],"styles":[{"alignment":0,"role":0}],"cells":[],"active":[0,0]}"#;
    for (width, height) in [(1000, 550), (800, 240), (320, 640)] {
        let mut host = NativeUiHost::default();
        let create = |id, parent: Option<u64>, index, kind: WidgetKind, text: &str| UiPatch::Create {
            id: widget(id), parent: parent.map(widget), index, kind: kind as u16,
            state: WidgetState::ENABLED.bits(), text: Some(text.into()), icon: None,
        };
        host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 1,
            patches: vec![create(1, None, 0, WidgetKind::Stack, "panel:flush"),
                create(2, Some(1), 0, WidgetKind::VirtualGrid, frame)],
        }).unwrap();
        let size = Size { width, height };
        let constraints = Constraints::new(size, size).unwrap();
        let before = host.render(app(), window(), constraints).unwrap();
        let grid_before = host.windows[&(app(), window())].geometry[&widget(2)];
        host.apply_patch(&UiPatchEnvelope { app: app(), window: window(), sequence: 2,
            patches: vec![
                create(3, Some(1), 1, WidgetKind::Stack, "panel:overlay:flush"),
                create(4, Some(3), 0, WidgetKind::Stack, "positioned-item:100:200:200:40"),
                create(5, Some(4), 0, WidgetKind::TextField, "=B1+C1"),
                UiPatch::Update { id: widget(5), state: PatchValue::Set((WidgetState::ENABLED | WidgetState::FOCUSED).bits()), text: PatchValue::Keep, icon: PatchValue::Keep },
            ],
        }).unwrap();
        let after = host.render(app(), window(), constraints).unwrap();
        let geometry = &host.windows[&(app(), window())].geometry;
        assert_eq!(geometry[&widget(2)], grid_before, "editing must not resize or move the grid");
        assert_eq!(geometry[&widget(3)], grid_before, "overlay follows the full grid without aspect fitting");
        let editor = geometry[&widget(5)];
        assert_eq!(editor.origin.x, (width / 10) as i32);
        assert_eq!(editor.origin.y, (height / 5) as i32);
        assert_eq!(editor.size, Size { width: width / 5, height: height * 40 / 1000 });
        for command in &after.commands {
            if let DrawCommand::Fill { rect, .. } = command {
                if !before.commands.contains(command) {
                    assert!(rect.size.width <= editor.size.width && rect.size.height <= editor.size.height,
                        "an editor overlay must not paint an opaque canvas: {rect:?}");
                }
            }
        }
    }
}

#[test]
fn anchored_popover_stays_by_its_parent_and_dismisses_outside_without_scrim() {
    let mut host = NativeUiHost::default();
    let create=|id,parent:Option<u64>,kind:WidgetKind,text:&str|UiPatch::Create {
        id:widget(id),parent:parent.map(widget),index:0,kind:kind as u16,
        state:WidgetState::ENABLED.bits(),text:Some(text.into()),icon:None,
    };
    host.apply_patch(&UiPatchEnvelope{app:app(),window:window(),sequence:1,patches:vec![
        create(1,None,WidgetKind::Stack,"panel:flush"),
        create(2,Some(1),WidgetKind::Popover,"panel:anchored-popup:100:120:360:320"),
        create(3,Some(2),WidgetKind::TextField,"Search values"),
    ]}).unwrap();
    let size=Size{width:800,height:600};
    let output=host.render(app(),window(),Constraints::new(size,size).unwrap()).unwrap();
    let popup=host.windows[&(app(),window())].geometry[&widget(2)];
    assert_eq!(popup,Rect{origin:Point{x:100,y:120},size:Size{width:360,height:320}});
    assert!(output.dialog.is_some());
    assert!(!output.commands.iter().any(|command|matches!(command,DrawCommand::Fill{rect,color,..}if rect.size==size&&color.channels()[3]!=255)),"a filter popup must not dim the worksheet");
    let dismissed=host.pointer_event(app(),window(),PointerEventKind::Down,Point{x:20,y:20},1,0).unwrap().unwrap();
    assert_eq!(dismissed.target,widget(2));
    assert!(matches!(dismissed.event,UiEvent::Key{key_code:9,pressed:true,..}),"outside click follows the popup Escape dismissal path");
    host.apply_patch(&UiPatchEnvelope{app:app(),window:window(),sequence:2,patches:vec![UiPatch::Update{id:widget(2),state:rice_native_app_protocol::PatchValue::Keep,text:rice_native_app_protocol::PatchValue::Set("panel:anchored-popup:100:200:360:520".into()),icon:rice_native_app_protocol::PatchValue::Keep}]}).unwrap();
    let medium=Size{width:800,height:600};
    host.render(app(),window(),Constraints::new(medium,medium).unwrap()).unwrap();
    let attached=host.windows[&(app(),window())].geometry[&widget(2)];
    assert_eq!(attached.origin.y,200,"a scrolled popup stays below its header when 320 pixels fit");
    assert_eq!(attached.size.height,396);
    let small=Size{width:320,height:240};
    host.render(app(),window(),Constraints::new(small,small).unwrap()).unwrap();
    let clipped=host.windows[&(app(),window())].geometry[&widget(2)];
    assert!(clipped.origin.x>=0&&clipped.origin.y>=0);
    assert!(clipped.origin.x as u32+clipped.size.width<=small.width);
    assert!(clipped.origin.y as u32+clipped.size.height<=small.height);
}

#[test]
fn sdk_plain_multiline_control_copies_scalar_selection_and_tabs_to_actions() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(), window: window(), sequence: 1,
        patches: vec![
            UiPatch::Create { id: widget(1), parent: None, index: 0,
                kind: WidgetKind::Column as u16, state: WidgetState::ENABLED.bits(), text: None, icon: None },
            UiPatch::Create { id: widget(2), parent: Some(widget(1)), index: 0,
                kind: WidgetKind::TextEditor as u16,
                state: (WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::KEY_NAVIGATION).bits(),
                text: Some("editor:plain:word:nolines:5:1\nA👋\nBé".into()), icon: None },
            UiPatch::Create { id: widget(3), parent: Some(widget(1)), index: 1,
                kind: WidgetKind::Button as u16, state: WidgetState::ENABLED.bits(), text: Some("Save".into()), icon: None },
        ],
    }).unwrap();
    let size = Size { width: 300, height: 180 };
    host.render(app(), window(), Constraints::new(size, size).unwrap()).unwrap();
    assert_eq!(host.selected_text(app(), window()).as_deref(), Some("👋\nBé"));
    let next = host.key_event(app(), window(), true, 23, 0, None).unwrap().unwrap();
    assert_eq!(next.target, widget(3));
    assert!(matches!(next.event, UiEvent::Focus { focused: true }));
    let previous = host.key_event(app(), window(), true, 23, 1, None).unwrap().unwrap();
    assert_eq!(previous.target, widget(2));
    assert!(matches!(previous.event, UiEvent::Focus { focused: true }));
}
