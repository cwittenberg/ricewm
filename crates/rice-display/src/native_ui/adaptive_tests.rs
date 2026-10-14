use super::*;
use rice_native_app_protocol::UiPatch;
use rice_ui::{WidgetKind, WidgetState};

fn app() -> AppId {
    AppId::new(1).expect("nonzero app")
}

fn window() -> WindowId {
    WindowId::new(1).expect("nonzero window")
}

fn widget(value: u64) -> WidgetId {
    WidgetId::new(value).expect("nonzero widget")
}

fn create(
    id: u64,
    parent: Option<u64>,
    index: u32,
    kind: WidgetKind,
    text: Option<&str>,
) -> UiPatch {
    UiPatch::Create {
        id: widget(id),
        parent: parent.map(widget),
        index,
        kind: kind as u16,
        state: WidgetState::ENABLED.bits(),
        text: text.map(str::to_owned),
        icon: None,
    }
}

fn render(patches: Vec<UiPatch>, width: u32, height: u32) -> RenderOutput {
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches,
    })
    .expect("valid adaptive fixture");
    host.render(
        app(),
        window(),
        Constraints::new(Size { width, height }, Size { width, height })
            .expect("fixed constraints"),
    )
    .expect("adaptive render")
}

#[test]
fn compact_navigation_reflows_to_two_columns_and_scroll_content_stays_clipped() {
    let mut patches = vec![
        create(1, None, 0, WidgetKind::SplitPane, Some("split:280")),
        create(2, Some(1), 0, WidgetKind::List, None),
        create(20, Some(1), 1, WidgetKind::ScrollView, None),
        create(21, Some(20), 0, WidgetKind::Column, None),
    ];
    for index in 0..6_u64 {
        patches.push(create(
            3 + index,
            Some(2),
            u32::try_from(index).expect("navigation index"),
            WidgetKind::Button,
            Some("Section"),
        ));
    }
    for index in 0..20_u64 {
        patches.push(create(
            30 + index,
            Some(21),
            u32::try_from(index).expect("content index"),
            WidgetKind::Button,
            Some("Setting"),
        ));
    }
    let output = render(patches, 360, 392);
    let target = |id| {
        output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .copied()
    };
    let first = target(3).expect("first navigation item");
    let second = target(4).expect("second navigation item");
    let third = target(5).expect("third navigation item");
    assert_eq!(first.rect.origin.y, second.rect.origin.y);
    assert!(second.rect.origin.x > first.rect.origin.x);
    assert!(third.rect.origin.y > first.rect.origin.y);
    assert!(target(30).is_some(), "visible content remains interactive");
    assert!(
        target(49).is_none(),
        "content beyond the scroll viewport must not retain a hit target"
    );
}

#[test]
fn table_rows_are_flat_columnar_controls_with_card_separators() {
    let output = render(
        vec![
            create(1, None, 0, WidgetKind::Table, None),
            create(2, Some(1), 0, WidgetKind::Row, Some("split:600")),
            create(3, Some(2), 0, WidgetKind::Button, Some("Name")),
            create(4, Some(2), 1, WidgetKind::Row, Some("split:500")),
            create(8, Some(4), 0, WidgetKind::Button, Some("Size")),
            create(9, Some(4), 1, WidgetKind::Button, Some("Modified")),
            create(5, Some(1), 1, WidgetKind::Row, Some("split:600")),
            create(6, Some(5), 0, WidgetKind::Button, Some("Documents")),
            create(7, Some(5), 1, WidgetKind::Label, Some("—")),
        ],
        640,
        240,
    );
    let name = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("sortable header")
        .rect;
    let nested_size = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(8))
        .expect("nested sortable header")
        .rect;
    assert!(
        !output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, .. } if *rect == name || *rect == nested_size
        )),
        "table cells use flat chrome instead of separate raised buttons"
    );
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.size.height == 1 && *color == Theme::ricewm_default().palette.border
    )));
}

#[test]
fn scrollbar_uses_a_proportional_thumb_without_painting_protocol_text() {
    let output = render(
        vec![create(1, None, 0, WidgetKind::ScrollBar, Some("40 / 100"))],
        16,
        160,
    );
    let fills = output
        .commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::Fill { .. }))
        .count();
    assert_eq!(fills, 2, "scrollbar paints one track and one thumb");
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "40 / 100"
    )));
}

#[test]
fn yellow_jacket_scrollbar_has_square_arrow_ends_and_a_proportional_thumb() {
    let mut host = NativeUiHost::default();
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::YellowJacket;
    host.set_theme(theme);
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![create(1, None, 0, WidgetKind::ScrollBar, Some("40 / 100"))],
    })
    .expect("valid Yellow Jacket scrollbar");
    let output = host
        .render(
            app(),
            window(),
            Constraints::new(
                Size {
                    width: 17,
                    height: 160,
                },
                Size {
                    width: 17,
                    height: 160,
                },
            )
            .expect("fixed scrollbar constraints"),
        )
        .expect("Yellow Jacket scrollbar render");
    let square_fills = output
        .commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::Fill { radius: 0, .. }))
        .count();
    let square_borders = output
        .commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::Border { radius: 0, .. }))
        .count();
    assert_eq!(
        square_fills, 17,
        "track, ends, arrows, thumb, and grip marks are painted"
    );
    assert_eq!(square_borders, 4, "track, two ends, and thumb are outlined");
}

#[test]
fn scroll_view_does_not_paint_internal_offset_metadata() {
    let snapshot = render(
        vec![
            create(1, None, 0, WidgetKind::Column, None),
            create(2, Some(1), 0, WidgetKind::ScrollView, Some("40/800")),
            create(3, Some(2), 0, WidgetKind::Label, Some("Visible content")),
        ],
        320,
        200,
    );
    assert!(snapshot.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text == "Visible content")
    }));
    assert!(
        !snapshot
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "40/800"))
    );
}

#[test]
fn scroll_view_repaints_its_stationary_frame_after_scrolled_content() {
    let snapshot = render(
        vec![
            create(1, None, 0, WidgetKind::ScrollView, Some("24/240")),
            create(2, Some(1), 0, WidgetKind::Column, None),
            create(3, Some(2), 0, WidgetKind::Label, Some("Moving row")),
        ],
        320,
        200,
    );
    let text = snapshot
        .commands
        .iter()
        .position(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "Moving row"),
        )
        .expect("scroll content text");
    let frame = snapshot
        .commands
        .iter()
        .enumerate()
        .filter_map(|(index, command)| match command {
            DrawCommand::Border { rect, .. }
                if rect.origin.x == 0
                    && rect.origin.y == 0
                    && rect.size.width == 320
                    && rect.size.height == 200 =>
            {
                Some(index)
            }
            _ => None,
        })
        .last()
        .expect("stationary viewport frame");
    assert!(
        frame > text,
        "the viewport frame must be painted after its moving child"
    );
}

#[test]
fn checkbox_radio_and_toggle_use_native_indicators() {
    let output = render(
        vec![
            create(1, None, 0, WidgetKind::Column, None),
            create(2, Some(1), 0, WidgetKind::CheckBox, Some("Wrap lines")),
            create(3, Some(1), 1, WidgetKind::RadioButton, Some("Words")),
            create(4, Some(1), 2, WidgetKind::Toggle, Some("On")),
            UiPatch::Update {
                id: widget(2),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::CHECKED).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
            UiPatch::Update {
                id: widget(3),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::CHECKED).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
            UiPatch::Update {
                id: widget(4),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::CHECKED).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
        ],
        240,
        100,
    );
    for label in ["Wrap lines", "Words"] {
        assert!(output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text == label
        )));
    }
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "On"
    )));
    assert!(
        output
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                DrawCommand::Fill { color, .. }
                    if *color == Theme::ricewm_default().palette.accent
                        || *color == Theme::ricewm_default().palette.accent.with_alpha(245)
            ))
            .count()
            >= 3
    );
}

#[test]
fn start_aligned_toggle_places_the_switch_before_its_label() {
    let output = render(
        vec![
            create(1, None, 0, WidgetKind::Column, None),
            create(2, Some(1), 0, WidgetKind::Toggle, Some("Supported apps")),
            UiPatch::Update {
                id: widget(2),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::START_ALIGNED).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
        ],
        240,
        48,
    );
    let switch_left = output
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Border { rect, .. } if rect.size.width == 36 => Some(rect.origin.x),
            _ => None,
        })
        .next()
        .expect("switch track border");
    let label_left = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Supported apps" => Some(rect.origin.x),
            _ => None,
        })
        .expect("toggle label");
    assert!(switch_left < label_left);
}

#[test]
fn frosted_theme_uses_theme_tinted_glass_for_each_widget_category() {
    let mut host = NativeUiHost::default();
    let mut theme = Theme::ricewm_default();
    theme.frosted_widgets.buttons = true;
    theme.frosted_widgets.switches = true;
    theme.frosted_widgets.sliders = true;
    theme.frosted_widgets.scrollbars = true;
    let palette = theme.palette;
    host.set_theme(theme);
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, None),
            create(2, Some(1), 0, WidgetKind::Button, Some("Apply")),
            create(3, Some(1), 1, WidgetKind::Toggle, Some("Blur")),
            create(4, Some(1), 2, WidgetKind::Slider, Some("25/100")),
            create(5, Some(1), 3, WidgetKind::ScrollBar, Some("40/100")),
            UiPatch::Update {
                id: widget(3),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::CHECKED).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
        ],
    })
    .expect("valid frosted fixture");
    let output = host
        .render(
            app(),
            window(),
            Constraints::new(
                Size {
                    width: 240,
                    height: 180,
                },
                Size {
                    width: 240,
                    height: 180,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("frosted render");

    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Glass {
            control: true,
            active: false,
            tint,
            ..
        } if *tint == palette.surface_raised
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Glass {
            tone: 3,
            control: true,
            active: true,
            tint,
            ..
        } if *tint == palette.accent
    )));
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Glass { tint, .. } if *tint == palette.text_muted
        )),
        "the scrollbar thumb carries the active theme's muted color"
    );
    assert!(
        output
            .commands
            .iter()
            .filter(|command| matches!(command, DrawCommand::Glass { .. }))
            .count()
            >= 7,
        "button, switch, slider pieces, and scrollbar pieces all use glass",
    );
}

#[test]
fn orchard_slider_keeps_its_track_below_a_white_glass_thumb() {
    let mut theme = Theme::ricewm_default();
    theme.chrome_profile = rice_ui::ChromeProfile::Orchard;
    theme.metrics.corner_radius = 18;
    theme.frosted_widgets.sliders = true;
    theme.palette.surface = rice_ui::Color::opaque(255, 255, 255);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&UiPatchEnvelope {app:app(),window:window(),sequence:1,
        patches:vec![create(1,None,0,WidgetKind::Slider,Some("50/100"))],
    }).unwrap();
    let output = host.render(app(),window(),Constraints::new(
        Size{width:240,height:40},Size{width:240,height:40}).unwrap()).unwrap();
    let lenses: Vec<_> = output.commands.iter().filter_map(|command| match command {
        DrawCommand::Glass {rect,tint,control:true,..} => Some((*rect,*tint)), _=>None,
    }).collect();
    let (thumb,tint)=lenses.last().expect("slider thumb is glass");
    assert_eq!(thumb.size.width,thumb.size.height);
    assert_eq!(tint.channels()[..3],[255,255,255]);
    assert!(lenses[0].0.size.height < thumb.size.height / 2,
        "the window radius must not inflate the slider track");
    assert!(!output.commands.iter().any(|command| matches!(command,
        DrawCommand::Fill{rect,color,..} if rect.origin.x>=thumb.origin.x &&
            rect.origin.y>=thumb.origin.y && rect.size.width<thumb.size.width &&
            color.channels()[..3]!=[255,255,255])), "no solid accent dot covers the lens");
}

#[test]
fn glass_sidebar_material_covers_the_entire_leading_split_panel() {
    let mut host = NativeUiHost::default();
    let mut theme = Theme::ricewm_default();
    theme.glass_sidebars = true;
    host.set_theme(theme);
    host.apply_patch(&UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::SplitPane, Some("split:260")),
            create(2, Some(1), 0, WidgetKind::Column, Some("panel:sidebar")),
            create(3, Some(1), 1, WidgetKind::List, None),
            create(4, Some(2), 0, WidgetKind::List, None),
            create(5, Some(3), 0, WidgetKind::Button, Some("Files")),
            create(6, Some(4), 0, WidgetKind::Button, Some("Places")),
        ],
    })
    .expect("valid sidebar fixture");
    let output = host
        .render(
            app(),
            window(),
            Constraints::new(
                Size {
                    width: 640,
                    height: 360,
                },
                Size {
                    width: 640,
                    height: 360,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("glass sidebar render");
    let glass = output
        .commands
        .iter()
        .filter_map(|command| match command {
            DrawCommand::Glass {
                rect,
                control: false,
                structural: true,
                radius: 0,
                ..
            } => Some(*rect),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(glass.len(), 1, "only the semantic sidebar becomes glass");
    assert_eq!(glass[0].origin, Point { x: 0, y: 0 });
    assert_eq!(glass[0].size.height, 360);
    assert_eq!(glass[0].size.width, 166);
}

#[test]
fn spinner_emits_an_animated_semantic_primitive_beside_its_label() {
    let output = render(
        vec![create(
            1,
            None,
            0,
            WidgetKind::Spinner,
            Some("Loading folder…"),
        )],
        240,
        40,
    );
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Spinner { rect, .. } if rect.size.width == 16 && rect.size.height == 16
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Loading folder…"
    )));
}

#[test]
fn choice_indicators_remain_bounded_at_tiny_layout_sizes() {
    for kind in [
        WidgetKind::CheckBox,
        WidgetKind::RadioButton,
        WidgetKind::Toggle,
    ] {
        let output = render(
            vec![
                create(1, None, 0, kind, Some("x")),
                UiPatch::Update {
                    id: widget(1),
                    state: rice_native_app_protocol::PatchValue::Set(
                        (WidgetState::ENABLED | WidgetState::CHECKED).bits(),
                    ),
                    text: rice_native_app_protocol::PatchValue::Keep,
                    icon: rice_native_app_protocol::PatchValue::Keep,
                },
            ],
            1,
            1,
        );
        assert!(output.commands.iter().all(|command| match command {
            DrawCommand::Fill { rect, .. } | DrawCommand::Border { rect, .. } => {
                rect.size.width <= 1 && rect.size.height <= 1
            }
            _ => true,
        }));
    }
}


#[test]
fn growing_scroll_list_preserves_fixed_column_form_controls() {
    let mut patches=vec![
        create(1,None,0,WidgetKind::Column,Some("panel:padding=12:gap=8")),
        create(2,Some(1),0,WidgetKind::Column,Some("panel:fixed:gap=8")),
        create(3,Some(2),0,WidgetKind::Label,Some("Record")),
        create(4,Some(2),1,WidgetKind::TextField,Some("")),
        create(5,Some(2),2,WidgetKind::Button,Some("Add")),
        create(6,Some(2),3,WidgetKind::Label,Some("Saved")),
        create(10,Some(1),1,WidgetKind::ScrollView,None),
        create(11,Some(10),0,WidgetKind::Column,Some("panel:fixed:gap=8")),
    ];
    for patch in &mut patches { if let UiPatch::Create{id,state,..}=patch {if *id==widget(1)||*id==widget(10){*state|=WidgetState::EXPANSIVE.bits();}} }
    let initial=render(patches.clone(),340,370);
    let initial_add=initial.hit_targets.iter().find(|target|target.widget==widget(5)).expect("visible Add").rect;
    for index in 0..12_u64 {
        patches.push(create(20+index,Some(11),index as u32,WidgetKind::Button,Some("Saved record")));
    }
    let populated=render(patches,340,370);
    let add=populated.hit_targets.iter().find(|target|target.widget==widget(5)).expect("Add survives list growth").rect;
    assert_eq!(add,initial_add,"growing list must consume its own scroll viewport rather than squeeze the fixed form");
    assert!(populated.hit_targets.iter().any(|target|target.widget==widget(20)),"first list row is reachable");
    assert!(!populated.hit_targets.iter().any(|target|target.widget==widget(31)),"overflowing records remain clipped to the viewport");
}
