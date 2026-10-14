use super::*;
use rice_native_app_protocol::{PatchValue, UiPatch};
use rice_ui::{TreeError, WidgetKind, WidgetState};

#[test]
fn chat_composer_multiline_draws_unicode_selection_without_changing_other_fields() {
    let text = "A👋\nBé";
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
        create(2, Some(1), 0, WidgetKind::Row, WidgetState::ENABLED, Some("panel:toolbar:chat-composer"), None),
        create(3, Some(2), 0, WidgetKind::TextField, WidgetState::ENABLED, Some(text), None),
        create(4, Some(1), 1, WidgetKind::Row, WidgetState::ENABLED, Some("panel:toolbar"), None),
        create(5, Some(4), 0, WidgetKind::TextField, WidgetState::ENABLED, Some(text), None),
    ])).unwrap();
    let native = host.windows.get_mut(&(app(1), window(1))).unwrap();
    assert!(super::layout::preferred_size(&native.tree, widget(3), &host.theme).height
        > super::layout::preferred_size(&native.tree, widget(5), &host.theme).height);
    // Native editing offsets are UTF-8 bytes; editor presentation is Unicode
    // character offsets. Exercise both directions across emoji and a newline.
    for (anchor, caret, expected_anchor, expected_caret) in [
        (0, text.len(), 0, 5),
        (text.len(), 1, 5, 1),
    ] {
        let native = host.windows.get_mut(&(app(1), window(1))).unwrap();
        native.focused = Some(widget(3));
        native.text_edit = Some(TextEditState {widget: widget(3), anchor, caret});
        let output = host.render(app(1), window(1), fixed_constraints(500, 240)).unwrap();
        let encoded = output.commands.iter().find_map(|command| match command {
            DrawCommand::Text {text, role: TextRole::Editor, ..} => Some(text.as_str()),
            _ => None,
        }).expect("chat multiline editor drawing");
        let presentation = rice_ui::EditorText::parse(encoded).unwrap();
        assert_eq!(presentation.document, text);
        assert_eq!(presentation.caret, Some(expected_caret));
        assert_eq!(presentation.anchor, Some(expected_anchor));
        assert_eq!(presentation.wrap, rice_ui::EditorWrap::Word);
        let (rect, size) = output.commands.iter().find_map(|command| match command {
            DrawCommand::Text {rect, size_px, role: TextRole::Editor, ..} => Some((rect, u32::from(*size_px))),
            _ => None,
        }).unwrap();
        let line_height = ((size * 135 + 99) / 100).max(size + 2);
        assert!(rect.size.height >= line_height * 2 + 8,
            "both line boxes fit after the editor painter's vertical insets");
        assert!(output.commands.iter().any(|command| matches!(command,
            DrawCommand::Text {text: value, role: TextRole::Label, ..} if value == text)),
            "ordinary TextField preserves its existing drawing contract");
        assert_eq!(host.windows[&(app(1), window(1))].tree.get(widget(3)).unwrap().text.as_deref(), Some(text));
    }
    let native = &host.windows[&(app(1), window(1))];
    let mut tree = native.tree.clone();
    // Explicit newlines grow the compact field only through five lines.
    tree.apply(&[rice_ui::Patch::Update {id: widget(3), state: None,
        text: rice_ui::ValueChange::Set("1\n2\n3\n4\n5".into()), icon: rice_ui::ValueChange::Keep}]).unwrap();
    let five = super::layout::preferred_size(&tree, widget(3), &host.theme).height;
    tree.apply(&[rice_ui::Patch::Update {id: widget(3), state: None,
        text: rice_ui::ValueChange::Set("1\n2\n3\n4\n5\n6".into()), icon: rice_ui::ValueChange::Keep}]).unwrap();
    assert_eq!(super::layout::preferred_size(&tree, widget(3), &host.theme).height, five);
    use super::editor_measurement::EditorMeasurement;
    let rect = host.windows[&(app(1), window(1))].geometry[&widget(3)];
    let point = Point {x: rect.origin.x + 8, y: rect.origin.y + 8};
    let sample = EditorMeasurement {widget: widget(3), caret: 3,
        text: rice_ui::editor_text_selection("plain", rice_ui::EditorWrap::Word, false, 5, 1, text)};
    host.pointer_event_measured(app(1), window(1), PointerEventKind::Down, point, 1, 0, Some(&sample)).unwrap();
    assert_eq!(host.windows[&(app(1), window(1))].text_edit.unwrap().caret, 6,
        "measured Unicode scalar after newline maps to the plain field's byte offset");
    let tail = EditorMeasurement {caret: 5, ..sample.clone()};
    host.pointer_event_measured(app(1), window(1), PointerEventKind::Move, point, 0, 0, Some(&tail)).unwrap();
    let prior = host.windows[&(app(1), window(1))].text_edit;
    assert_eq!((prior.unwrap().anchor, prior.unwrap().caret), (6, text.len()));
    for invalid in [
        EditorMeasurement {caret: 6, ..sample.clone()},
        EditorMeasurement {text: rice_ui::editor_text("plain", rice_ui::EditorWrap::Word, false, "stale"), ..sample.clone()},
        EditorMeasurement {text: rice_ui::editor_text("javascript", rice_ui::EditorWrap::Word, false, text), ..sample.clone()},
        EditorMeasurement {widget: widget(5), ..sample.clone()},
    ] {
        assert!(host.pointer_event_measured(app(1), window(1), PointerEventKind::Move, point, 0, 0, Some(&invalid)).is_err());
        assert_eq!(host.windows[&(app(1), window(1))].text_edit, prior,
            "invalid or stale measurements cannot change the selection");
    }
}

#[test]
fn classic_caption_width_matches_the_browser_font_metrics() {
    assert_eq!(
        super::layout::estimated_classic_text_width("My Computer", 11),
        76
    );
    assert_eq!(
        super::layout::estimated_classic_text_width("Calculator", 11),
        57
    );
    assert_eq!(
        super::layout::estimated_classic_text_width("Pictures", 11),
        45
    );
}

#[test]
fn progress_width_accepts_completed_and_total_counts() {
    assert_eq!(super::render::progress_width_for_test(Some("3/8"), 160), 60);
    assert_eq!(
        super::render::progress_width_for_test(Some("slim:3/8"), 160),
        60
    );
    assert_eq!(
        super::render::progress_width_for_test(Some("12/8"), 160),
        160
    );
    assert_eq!(super::render::progress_width_for_test(Some("3/0"), 160), 0);
}

#[test]
fn menu_scroll_threshold_counts_commands_instead_of_separators() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![create(
        1,
        None,
        0,
        WidgetKind::Menu,
        WidgetState::ENABLED,
        Some("View"),
        None,
    )];
    let mut index = 0_u32;
    for command in 0..20_u64 {
        patches.push(create(
            10 + u64::from(index),
            Some(1),
            index,
            WidgetKind::MenuItem,
            WidgetState::ENABLED,
            Some(&format!("Command {command}")),
            None,
        ));
        index += 1;
        if matches!(command, 1 | 5 | 9 | 13 | 17) {
            patches.push(create(
                10 + u64::from(index),
                Some(1),
                index,
                WidgetKind::MenuItem,
                WidgetState::empty(),
                Some("menu:separator"),
                None,
            ));
            index += 1;
        }
    }
    host.apply_patch(&envelope(1, patches))
        .expect("twenty-command menu");
    let native_window = host.windows.get(&(app(1), window(1))).expect("menu window");
    assert_eq!(
        super::layout::maximum_menu_scroll_row(&native_window.tree, widget(1)),
        0,
        "separators must not make a twenty-command menu scroll",
    );

    host.apply_patch(&envelope(
        2,
        vec![create(
            100,
            Some(1),
            index,
            WidgetKind::MenuItem,
            WidgetState::ENABLED,
            Some("Command 20"),
            None,
        )],
    ))
    .expect("twenty-first menu command");
    let native_window = host.windows.get(&(app(1), window(1))).expect("menu window");
    assert!(
        super::layout::maximum_menu_scroll_row(&native_window.tree, widget(1)) > 0,
        "a twenty-first command should enable bounded menu scrolling",
    );
}

#[test]
fn redmond_menu_commands_use_classic_compact_row_height() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme.clone());
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("View"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::CheckBox,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some("Show Places"),
                None,
            ),
        ],
    ))
    .expect("Redmond compact menu fixture");
    let native_window = host.windows.get(&(app(1), window(1))).expect("menu window");
    assert_eq!(
        super::layout::preferred_size(&native_window.tree, widget(2), &theme).height,
        20
    );
}

#[test]
fn redmond_open_menubar_heading_stays_visibly_sunken_while_popup_is_open() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::MenuBar,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("Tools"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Insert Spaces Instead of Tabs"),
                None,
            ),
        ],
    ))
    .expect("Redmond menubar fixture");
    host.windows
        .get_mut(&(app(1), window(1)))
        .expect("menu window")
        .opened_menu = Some(widget(2));
    let output = host
        .render(app(1), window(1), fixed_constraints(420, 180))
        .expect("render open Redmond menu");
    let heading = host.windows[&(app(1), window(1))].geometry[&widget(2)];
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if rect.origin == heading.origin
                    && rect.size.width == heading.size.width
                    && rect.size.height == 1
                    && *color == Color::opaque(0, 0, 0)
        )),
        "the owning Tools heading must retain a pressed Win98 edge"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, color, .. }
                if text == "Tools" && *color == Color::opaque(0, 0, 0)
        )),
        "an open Win98 menubar heading keeps dark text on gray"
    );
}

#[test]
fn familiar_selection_uses_light_ink_for_monochrome_icons() {
    let theme = Theme::familiar_light();
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::List,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Downloads"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("familiar icon fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(220, 40))
        .expect("familiar selected row renders");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon { color, .. } if *color == theme.palette.selection_text
    )));
}

#[test]
fn start_aligned_icon_buttons_keep_a_professional_text_gap() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
            Some("APT"),
            Some(IconId::LinuxLogo),
        )],
    ))
    .expect("source button fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(96, 36))
        .expect("source button renders");
    let icon = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::LinuxLogo,
                ..
            } => Some(*rect),
            _ => None,
        })
        .expect("source icon");
    let text = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "APT" => Some(*rect),
            _ => None,
        })
        .expect("source label");
    assert!(
        text.origin.x >= icon.origin.x + i32::try_from(icon.size.width).unwrap() + 8,
        "icon {icon:?} and label {text:?} must retain an eight-pixel gap",
    );
}

#[test]
fn centered_icon_buttons_reserve_a_gap_for_selected_navigation_and_footer_actions() {
    for label in ["Reminder", "Today", "Clear", "Done"] {
        for state in [WidgetState::ENABLED, WidgetState::ENABLED | WidgetState::SELECTED | WidgetState::ACCENT] {
            let mut host = NativeUiHost::default();
            host.apply_patch(&envelope(1, vec![create(1, None, 0, WidgetKind::Button,
                state, Some(label), Some(IconId::Calendar))])).expect("centered button");
            let output = host.render(app(1), window(1), fixed_constraints(128,40)).expect("render");
            let icon = output.commands.iter().find_map(|c| match c {DrawCommand::Icon {rect,..}=>Some(*rect), _=>None}).unwrap();
            let text = output.commands.iter().find_map(|c| match c {DrawCommand::Text {rect,..}=>Some(*rect), _=>None}).unwrap();
            assert!(text.origin.x >= icon.origin.x + icon.size.width as i32 + 8, "{label}: {icon:?} {text:?}");
            assert!(text.origin.x + text.size.width as i32 <= 128, "caption must fit");
            assert!(icon.size.width >= 12, "icon must remain readable");
        }
    }
}

#[test]
fn selected_calendar_days_keep_space_for_two_digits_in_a_forty_pixel_target() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::Button,
        WidgetState::ENABLED|WidgetState::SELECTED|WidgetState::ACCENT,Some("11"),None)])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(40,40)).unwrap();
    let text=output.commands.iter().find_map(|c|match c {DrawCommand::Text {rect,text,..} if text=="11"=>Some(*rect),_=>None}).unwrap();
    assert!(text.size.width >= 20,"two digits retain room: {text:?}");
}

#[test]
fn glass_sidebar_uses_surface_ink_without_changing_its_color_icons() {
    let mut theme = Theme::ricewm_default();
    theme.glass_sidebars = true;
    theme.frosted_widgets.buttons = true;
    theme.menu_palette.text = rice_ui::Color::opaque(245, 247, 246);
    theme.palette.text = rice_ui::Color::opaque(31, 41, 55);
    theme.palette.selection = rice_ui::Color::rgba(255, 255, 255, 224);
    theme.palette.selection_text = rice_ui::Color::opaque(255, 255, 255);
    let icon_color = rice_ui::Color::opaque(255, 103, 66);
    theme.palette.accent = icon_color;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::List,
                WidgetState::ENABLED,
                Some("panel:sidebar"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Home"),
                Some(IconId::Home),
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Desktop"),
                Some(IconId::Desktop),
            ),
        ],
    ))
    .expect("glass sidebar fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(220, 80))
        .expect("glass sidebar renders");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Home" && *color == rice_ui::Color::opaque(10, 8, 13)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Desktop" && *color == theme.palette.text
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon { icon: IconId::Home, color, .. }
            if *color != theme.palette.text
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. } if *color == theme.palette.selection
    )));
    assert!(
        !output
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Glass { control: true, .. }))
    );
}

#[test]
fn root_client_surface_is_square_at_the_titlebar_seam() {
    let mut theme = Theme::ricewm_default();
    theme.metrics.corner_radius = 14;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            None,
            None,
        )],
    ))
    .expect("root client fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("root client renders");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, radius: 0, .. }
            if rect.origin.x == 0 && rect.origin.y == 0
                && rect.size.width == 320 && rect.size.height == 180
    )));
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, radius, .. }
            if rect.origin.x == 0 && rect.origin.y == 0
                && rect.size.width == 320 && rect.size.height == 180
                && *radius > 0
    )));
}

#[test]
fn progress_protocol_values_are_not_rendered_as_text() {
    let mut host = gallery_host();
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("progress fixture renders");
    assert!(
        !output
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "62%")),
        "progress metadata drives the meter without becoming visible copy",
    );
}

#[test]
fn large_image_zoom_scales_a_semantic_hero_without_painting_its_metadata() {
    let mut host = NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Image,
            WidgetState::ENABLED | WidgetState::LARGE_ICON,
            Some("zoom:1500"),
            Some(IconId::Bowl),
        )],
    ))
    .expect("large image fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(240, 240))
        .expect("large image render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon { rect, icon: IconId::Bowl, .. }
            if rect.size == (Size { width: 144, height: 144 })
    )));
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "zoom:1500"
    )));
}

#[test]
fn host_surface_fills_its_window_client_without_panel_bands() {
    let mut host = NativeUiHost::new(Theme::familiar_light());
    let expected_identity = "a".repeat(64);
    let surface_identity = format!("host-surface:{expected_identity}");
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Container,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::HostSurface,
                WidgetState::ENABLED | WidgetState::FOCUSED,
                Some(&surface_identity),
                None,
            ),
        ],
    ))
    .expect("host surface fixture");

    host.apply_patch(&envelope(2, vec![
        create(3, Some(1), 1, WidgetKind::MenuBar, WidgetState::ENABLED, Some("context"), None),
        create(4, Some(3), 0, WidgetKind::Menu, WidgetState::ENABLED, Some("Menu"), None),
        create(5, Some(4), 0, WidgetKind::MenuItem, WidgetState::ENABLED, Some("Copy"), None),
    ])).expect("context menu must not change the portal's rectangle");

    let output = host
        .render(app(1), window(1), fixed_constraints(720, 440))
        .expect("host surface render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::HostPortal { rect, identity }
            if *rect == (Rect {
                origin: Point { x: 0, y: 0 },
                size: Size { width: 720, height: 440 },
            }) && identity == &expected_identity
    )), "portal commands: {:?}", output.commands);
}

#[test]
fn selected_composite_button_gives_its_nested_label_selection_ink() {
    let mut theme = Theme::familiar_light();
    theme.palette.selection = Color::opaque(38, 112, 174);
    // A malformed/custom foreground hint must not produce black-on-blue.
    theme.palette.selection_text = Color::opaque(0, 0, 0);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::List,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                None,
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Appearance"),
                None,
            ),
        ],
    ))
    .expect("selected composite fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(240, 80))
        .expect("selected composite render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Appearance" && *color == Color::opaque(255, 255, 255)
    )));
}

#[test]
fn bright_accent_buttons_derive_dark_ink_from_their_actual_surface() {
    let mut theme = Theme::ricewm_default();
    theme.palette.accent = Color::opaque(62, 147, 245);
    theme.palette.accent_text = Color::opaque(255, 255, 255);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::ACCENT,
            Some("Apply"),
            None,
        )],
    ))
    .expect("bright action fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(180, 40))
        .expect("bright action render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. } if *color == Color::opaque(62, 147, 245)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Apply" && *color == Color::opaque(10, 8, 13)
    )));
}

fn app(value: u64) -> AppId {
    AppId::new(value).expect("nonzero test app")
}

fn window(value: u64) -> WindowId {
    WindowId::new(value).expect("nonzero test window")
}

fn widget(value: u64) -> WidgetId {
    WidgetId::new(value).expect("nonzero test widget")
}

fn create(
    id: u64,
    parent: Option<u64>,
    index: u32,
    kind: WidgetKind,
    state: WidgetState,
    text: Option<&str>,
    icon: Option<IconId>,
) -> UiPatch {
    UiPatch::Create {
        id: widget(id),
        parent: parent.map(widget),
        index,
        kind: kind as u16,
        state: state.bits(),
        text: text.map(str::to_owned),
        icon: icon.map(IconId::wire_value),
    }
}

fn envelope(sequence: u64, patches: Vec<UiPatch>) -> UiPatchEnvelope {
    UiPatchEnvelope {
        app: app(1),
        window: window(1),
        sequence,
        patches,
    }
}

fn gallery_host() -> NativeUiHost {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Open"),
                Some(IconId::DocumentOpen),
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::empty(),
                Some("Disabled"),
                None,
            ),
            create(
                5,
                Some(1),
                1,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                6,
                Some(5),
                0,
                WidgetKind::ProgressBar,
                WidgetState::ENABLED,
                Some("62%"),
                None,
            ),
        ],
    ))
    .expect("gallery patch");
    host
}

#[test]
fn root_theme_token_scopes_full_palette_and_font_scale_to_one_window() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("theme:dark:font-scale=150"),
            None,
        )],
    ))
    .expect("window theme patch");
    let native_window = host.windows.get(&(app(1), window(1))).unwrap();
    let theme = effective_window_theme(&native_window.tree, Theme::ricewm_default());
    assert_eq!(theme.palette.surface, Theme::ricewm_dark().palette.surface);
    assert_eq!(theme.palette.text, Theme::ricewm_dark().palette.text);
    assert_eq!(theme.typography.body_px, 19);
    assert_eq!(
        host.theme(),
        &Theme::ricewm_default(),
        "system palette is unchanged"
    );
}

#[test]
fn familiar_preview_uses_win32_metrics_and_preserves_system_theme() {
    let mut host = NativeUiHost::default();
    let system = Theme::ricewm_default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("theme:familiar:light:liquid-frost"),
            None,
        )],
    ))
    .expect("familiar preview patch");
    let native_window = host.windows.get(&(app(1), window(1))).unwrap();
    let theme = effective_window_theme(&native_window.tree, system);
    assert_eq!(theme.metrics.corner_radius, 0);
    assert_eq!(theme.metrics.control_height, 24);
    assert_eq!(theme.metrics.scrollbar_width, 17);
    assert_eq!(theme.palette.text, rice_ui::Color::opaque(0, 0, 0));
    assert_eq!(theme.palette.selection.channels()[3], 170);
    assert!(theme.frosted_widgets.buttons);
    assert_eq!(host.theme(), &system, "gallery preview stays window-scoped");
}

#[test]
fn configured_catalog_previews_keep_theme_family_separate_from_light_dark_mode() {
    let mut host = NativeUiHost::default();
    let system = Theme::ricewm_default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("theme:dark:light"),
            None,
        )],
    ))
    .expect("dark-family light preview patch");
    let native_window = host.windows.get(&(app(1), window(1))).unwrap();
    let light = effective_window_theme(&native_window.tree, system);
    assert_eq!(
        light.palette.surface,
        Theme::ricewm_default().palette.surface
    );
    assert_eq!(light.palette.accent, Color::opaque(96, 165, 250));

    let emerald_light = emerald_catalog_preview(false);
    let emerald_dark = emerald_catalog_preview(true);
    assert_ne!(emerald_light.palette.surface, emerald_dark.palette.surface);
    assert_eq!(emerald_dark.palette.accent, Color::opaque(52, 211, 153));
    let frost_light = frosted_ice_catalog_preview(false);
    let frost_dark = frosted_ice_catalog_preview(true);
    assert_ne!(frost_light.palette.surface, frost_dark.palette.surface);
    assert_eq!(frost_dark.palette.accent, Color::opaque(87, 199, 212));
}

#[test]
fn fixed_height_token_applies_to_leaf_web_surfaces() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::WebSurface,
                WidgetState::ENABLED,
                Some("panel:height:320"),
                None,
            ),
        ],
    ))
    .expect("fixed web surface patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 400))
        .expect("fixed web surface render");
    let surface = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("web surface hit target");
    assert_eq!(surface.rect.size.height, 320);
}

#[test]
fn form_control_row_gives_text_fields_a_comfortable_target() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:form-control"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::TextField,
                WidgetState::ENABLED,
                Some(""),
                None,
            ),
        ],
    ))
    .expect("form patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 160))
        .expect("form render");
    let field = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("text field hit target");
    assert_eq!(field.rect.size.height, 40);
}

#[test]
fn forced_dark_editor_overrides_a_light_theme_with_contrasting_ink() {
    let mut host = NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::TextEditor,
                WidgetState::ENABLED | WidgetState::FOCUSED,
                Some("editor:plain@12~dark:word:lines:0:0:currentline:1:adaptive\nhello"),
                None,
            ),
        ],
    ))
    .expect("forced dark editor patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 160))
        .expect("forced dark editor render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. }
            if *color == rice_ui::Color::opaque(18, 20, 24)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { role: TextRole::Editor, color, .. }
            if *color == rice_ui::Color::opaque(235, 238, 244)
    )));
}

#[test]
fn sibling_form_rows_share_one_label_column() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:form-control"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Label"),
                None,
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::TextField,
                WidgetState::ENABLED,
                Some("RiceWM"),
                None,
            ),
            create(
                5,
                Some(1),
                1,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:form-control"),
                None,
            ),
            create(
                6,
                Some(5),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("URL"),
                None,
            ),
            create(
                7,
                Some(5),
                1,
                WidgetKind::TextField,
                WidgetState::ENABLED,
                Some("https://ricewm.org/"),
                None,
            ),
        ],
    ))
    .expect("aligned form patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 160))
        .expect("aligned form render");
    let label_field = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("label field hit target");
    let url_field = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(7))
        .expect("URL field hit target");

    assert_eq!(label_field.rect.origin.x, url_field.rect.origin.x);
    assert_eq!(label_field.rect.size.width, url_field.rect.size.width);
}

#[test]
fn trailing_action_stays_at_the_same_right_edge_without_a_spacer_widget() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                Some("panel:trailing-action"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("New tab"),
                Some(IconId::ObjectAdd),
            ),
            create(
                4,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some(""),
                Some(IconId::Settings),
            ),
        ],
    ))
    .expect("toolbar patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 48))
        .expect("toolbar render");
    let initial_settings = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("settings target")
        .rect;
    assert!(
        initial_settings
            .origin
            .x
            .saturating_add_unsigned(initial_settings.size.width)
            >= 470,
        "settings action was not right aligned: {initial_settings:?}"
    );

    host.apply_patch(&envelope(
        2,
        vec![create(
            3,
            Some(1),
            1,
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("Another tab"),
            Some(IconId::Application),
        )],
    ))
    .expect("insert another leading action");
    let updated = host
        .render(app(1), window(1), fixed_constraints(480, 48))
        .expect("updated toolbar render");
    let updated_settings = updated
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("updated settings target")
        .rect;
    assert_eq!(updated_settings.origin.x, initial_settings.origin.x);
    assert_eq!(updated_settings.size, initial_settings.size);
    assert_eq!(updated.hit_targets.len(), 3, "no spacer target may appear");
}

#[test]
fn settings_choice_button_is_compact_and_vertically_centered_in_a_tall_row() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:actions:setting-choice:choice-width=Activities"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                None,
                Some(IconId::GoBack),
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Activities"),
                None,
            ),
            create(
                4,
                Some(1),
                2,
                WidgetKind::Label,
                WidgetState::ENABLED,
                None,
                Some(IconId::GoForward),
            ),
        ],
    ))
    .expect("settings choice fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(360, 96))
        .expect("render settings choice");
    let button = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("choice button target")
        .rect;

    assert!((32..=40).contains(&button.size.height), "{button:?}");
    let top_space = button.origin.y.max(0).cast_unsigned();
    let bottom_space = 96_u32.saturating_sub(
        button
            .origin
            .y
            .max(0)
            .cast_unsigned()
            .saturating_add(button.size.height),
    );
    assert!(top_space.abs_diff(bottom_space) <= 1, "{button:?}");
}

#[test]
fn palette_rows_keep_swatches_and_hex_labels_in_straight_columns() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:settings-page:intrinsic-flow"),
                None,
            ),
            create(
                10,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:actions:label-width=Background color  ·  #000000"),
                None,
            ),
            create(
                11,
                Some(10),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("color-swatch:#2c001e"),
                None,
            ),
            create(
                12,
                Some(10),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED | WidgetState::MUTED | WidgetState::START_ALIGNED,
                Some("Start  ·  #2c001e"),
                None,
            ),
            create(
                13,
                Some(10),
                2,
                WidgetKind::Toggle,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some(""),
                None,
            ),
            create(
                20,
                Some(1),
                1,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:actions:label-width=Background color  ·  #000000"),
                None,
            ),
            create(
                21,
                Some(20),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("color-swatch:#e95420"),
                None,
            ),
            create(
                22,
                Some(20),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED | WidgetState::MUTED | WidgetState::START_ALIGNED,
                Some("Accent  ·  #e95420"),
                None,
            ),
            create(
                23,
                Some(20),
                2,
                WidgetKind::Toggle,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some(""),
                None,
            ),
        ],
    ))
    .expect("palette row fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(640, 120))
        .expect("palette rows render");
    let target = |id| {
        output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .expect("palette control target")
            .rect
    };
    assert_eq!(target(11).origin.x, target(21).origin.x);
    assert_eq!(target(11).size, target(21).size);
    assert_eq!(target(13).origin.x, target(23).origin.x);

    let label = |text: &str| {
        output
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text {
                    rect,
                    text: rendered,
                    ..
                } if rendered == text => Some(*rect),
                _ => None,
            })
            .expect("palette label")
    };
    assert_eq!(
        label("Start  ·  #2c001e").origin.x,
        label("Accent  ·  #e95420").origin.x,
    );
}

#[test]
fn settings_intrinsic_flow_prevents_sections_from_jumping_with_shorter_content() {
    let background_origin = |background_rows: u64| {
        let mut patches = vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:settings-page:intrinsic-flow"),
                None,
            ),
            create(
                10,
                Some(1),
                0,
                WidgetKind::Container,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                11,
                Some(10),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Theme action"),
                None,
            ),
            create(
                20,
                Some(1),
                1,
                WidgetKind::Container,
                WidgetState::ENABLED,
                None,
                None,
            ),
        ];
        for index in 0..background_rows {
            patches.push(create(
                21 + index,
                Some(20),
                u32::try_from(index).expect("bounded background row"),
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Background control"),
                None,
            ));
        }
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(1, patches))
            .expect("settings sections fixture");
        host.render(app(1), window(1), fixed_constraints(640, 420))
            .expect("settings sections render")
            .hit_targets
            .into_iter()
            .find(|target| target.widget == widget(21))
            .expect("first background control")
            .rect
            .origin
            .y
    };

    assert_eq!(background_origin(1), background_origin(5));
}

#[test]
fn tab_close_target_is_embedded_inside_one_selected_tab_surface() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT | WidgetState::START_ALIGNED,
                Some("Design standard"),
                Some(IconId::Document),
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("panel:tab-close"),
                Some(IconId::Close),
            ),
        ],
    ))
    .expect("tab fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(420, 40))
        .expect("tab render");
    let selected_surface = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Fill { rect, color, .. } if *color == host.theme().palette.surface => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("selected tab surface");
    let close = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("independent close target")
        .rect;
    assert!(selected_surface.contains(close.origin));
    assert_eq!(
        close.origin.x.saturating_add_unsigned(close.size.width),
        selected_surface
            .origin
            .x
            .saturating_add_unsigned(selected_surface.size.width)
    );
    assert!(close.size.width >= 24 && close.size.height >= 24);
}

#[test]
fn a_single_browser_tab_keeps_a_chromium_sized_width() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab:browser-tab:control-for=4"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::ENABLED,
                None,
                Some(IconId::Network),
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("A deliberately long browser tab title"),
                None,
            ),
            create(
                5,
                Some(2),
                2,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("panel:tab-close"),
                Some(IconId::Close),
            ),
            create(
                6,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("+"),
                Some(IconId::DocumentNew),
            ),
        ],
    ))
    .expect("browser tab fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(960, 48))
        .expect("browser tab render");
    let favicon = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("favicon target")
        .rect;
    let close = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(5))
        .expect("close target")
        .rect;
    assert!(
        close
            .origin
            .x
            .saturating_add_unsigned(close.size.width)
            .saturating_sub(favicon.origin.x)
            <= 220,
        "single browser tab expanded wider than its compact cap: {favicon:?} {close:?}"
    );
    let selected_surface = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Fill { rect, color, .. }
                if *color == host.theme().palette.surface_raised =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("selected browser tab surface");
    assert!(selected_surface.contains(favicon.origin));
    assert!(selected_surface.contains(close.origin));
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, color, .. }
                if *rect == selected_surface && *color == host.theme().palette.accent
        )),
        "the active tab has a folder-style border"
    );
    assert!(
        favicon.origin.x
            >= selected_surface
                .origin
                .x
                .saturating_add_unsigned(u32::from(host.theme().spacing.unit)),
        "favicon needs a visible leading gutter: {selected_surface:?} {favicon:?}",
    );

    let title = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("tab title target")
        .rect;
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        host.pointer_event(
            app(1),
            window(1),
            kind,
            Point {
                x: title.origin.x + 2,
                y: title.origin.y + 2,
            },
            1,
            0,
        )
        .expect("focus browser tab");
    }
    let focused = host
        .render(app(1), window(1), fixed_constraints(960, 48))
        .expect("focused browser tab render");
    let focus_borders = focused
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                DrawCommand::Border { color, .. } if *color == host.theme().palette.focus
            )
        })
        .count();
    assert_eq!(
        focus_borders, 0,
        "active browser tabs use a selected surface, not a persistent focus border"
    );
}

#[test]
fn browser_tabs_join_the_toolbar_and_keep_inactive_tabs_distinct() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                Some("panel:browser-tab-strip"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab:browser-tab:control-for=4"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:tab:browser-tab:control-for=5"),
                None,
            ),
            create(
                4,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Active"),
                None,
            ),
            create(
                5,
                Some(3),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Inactive"),
                None,
            ),
        ],
    ))
    .expect("browser tab folder fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(480, 42))
        .expect("browser tab folder render");
    let selected = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("selected tab")
        .rect;
    let inactive = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(5))
        .expect("inactive tab")
        .rect;
    assert!(output.commands.iter().any(|command| matches!(
        command, DrawCommand::Fill { rect, color, .. }
            if rect.origin == selected.origin && rect.size.height > selected.size.height && *color == host.theme().palette.surface_raised
    )));
    assert!(
        output.commands.iter().any(|command| matches!(
            command, DrawCommand::Border { rect, .. }
                if rect.origin.x == inactive.origin.x
                    && rect.origin.y == inactive.origin.y + 4
                    && rect.size.height > inactive.size.height - 4
        )),
        "inactive tabs sit below the active folder crest"
    );
}

#[test]
fn many_tabs_do_not_expand_or_lower_integrated_window_controls() {
    let mut patches = vec![create(
        1,
        None,
        0,
        WidgetKind::Tabs,
        WidgetState::ENABLED,
        Some("panel:browser-tab-strip"),
        None,
    )];
    for index in 0..12_u64 {
        patches.push(create(
            10 + index,
            Some(1),
            u32::try_from(index).unwrap(),
            WidgetKind::Row,
            WidgetState::ENABLED,
            Some("panel:tab:browser-tab"),
            None,
        ));
    }
    patches.push(create(
        30,
        Some(1),
        12,
        WidgetKind::Label,
        WidgetState::ENABLED,
        Some("panel:flex:window-drag-region"),
        None,
    ));
    for (order, id, icon) in [
        (13, 31, IconId::Minimize),
        (14, 32, IconId::Maximize),
        (15, 33, IconId::Close),
    ] {
        patches.push(create(
            id,
            Some(1),
            order,
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::FLAT,
            Some("panel:window-control"),
            Some(icon),
        ));
    }
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, patches))
        .expect("many tabs fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(760, 42))
        .expect("many tabs render");
    for id in [31, 32, 33] {
        let rect = output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .expect("window control")
            .rect;
        assert_eq!(
            rect.size,
            Size {
                width: 28,
                height: 28
            }
        );
        assert_eq!(rect.origin.y, 2);
    }
    for icon in [IconId::Minimize, IconId::Maximize, IconId::Close] {
        let rect = output
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Icon {
                    rect,
                    icon: candidate,
                    ..
                } if *candidate == icon => Some(*rect),
                _ => None,
            })
            .expect("window control icon");
        assert!(rect.size.width >= 16, "{icon:?} rendered at {rect:?}");
        assert!(rect.size.height >= 16, "{icon:?} rendered at {rect:?}");
    }
}

#[test]
fn window_drag_region_is_an_invisible_pointer_target() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                Some("panel:browser-tab-strip"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Tab"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("panel:flex:window-drag-region"),
                None,
            ),
        ],
    ))
    .expect("drag-region fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(640, 36))
        .expect("drag-region render");
    let drag = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("drag region hit target");
    assert!(!drag.focusable);
    assert!(!drag.activatable);
    let point = Point {
        x: drag.rect.origin.x.saturating_add(1),
        y: drag.rect.origin.y.saturating_add(1),
    };
    let event = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("drag pointer event")
        .expect("drag pointer envelope");
    assert_eq!(event.target, widget(3));
    assert!(matches!(
        event.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Down,
            button: 1,
            ..
        }
    ));
}

#[test]
fn bookmark_bar_is_smaller_than_the_tab_strip() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::ToolBar,
                WidgetState::ENABLED,
                Some("panel:compact:bookmark-bar"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:compact:bookmark:control-for=4"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::empty(),
                Some("inline-image:AAAA"),
                None,
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Example"),
                None,
            ),
        ],
    ))
    .expect("compact bookmark fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(420, 26))
        .expect("compact bookmark render");
    let button = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("bookmark button")
        .rect;
    assert_eq!(button.size.height, 22);
}

#[test]
fn toolbar_hamburger_is_a_neat_square_icon_control() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::ToolBar,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                None,
                Some(IconId::Menu),
            ),
        ],
    ))
    .expect("toolbar hamburger fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(240, 48))
        .expect("toolbar hamburger render");
    let hamburger = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("hamburger hit target")
        .rect;
    assert_eq!(hamburger.size.width, hamburger.size.height, "{hamburger:?}");
    assert!(
        hamburger.size.width <= u32::from(host.theme().metrics.control_height),
        "hamburger retained excess horizontal padding: {hamburger:?}",
    );
}

#[test]
fn accent_labels_use_the_accent_hue_instead_of_accent_surface_ink() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.accent = Color::opaque(255, 204, 102);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Label,
            WidgetState::ENABLED | WidgetState::ACCENT,
            Some("Accelerated"),
            None,
        )],
    ))
    .expect("accent label fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(220, 48))
        .expect("accent label render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Accelerated" && *color == Color::opaque(255, 204, 102)
    )));
}

#[test]
fn command_deck_markdown_uses_readable_frame_text_on_dark_panels() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.surface = Color::opaque(7, 7, 11);
    theme.palette.text = Color::opaque(255, 244, 220);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Markdown,
            WidgetState::ENABLED | WidgetState::START_ALIGNED,
            Some("## A browser within a browser"),
            None,
        )],
    ))
    .expect("Command Deck markdown fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(420, 80))
        .expect("Command Deck markdown render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, role: TextRole::Markdown, .. }
            if text.contains("browser within a browser")
                && *color == Color::opaque(255, 244, 220)
    )));
}

#[test]
fn command_deck_browser_tabs_join_the_page_with_legible_ink() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.selection = Color::opaque(255, 204, 102);
    theme.palette.selection_text = Color::opaque(10, 8, 13);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab:browser-tab:control-for=4"),
                None,
            ),
            create(
                4,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("LCARS browser tab"),
                None,
            ),
        ],
    ))
    .expect("Command Deck tab fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(480, 48))
        .expect("Command Deck tab render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, radius }
            if *color == host.theme().palette.surface_raised
                && u32::from(*radius) < rect.size.height / 2
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "LCARS browser tab" && *color == Color::opaque(255, 255, 255)
    )));
}

#[test]
fn command_deck_terminal_tab_uses_light_ink_on_its_dark_surface() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.surface = Color::opaque(12, 11, 16);
    theme.palette.surface_raised = Color::opaque(24, 22, 29);
    theme.palette.selection = Color::opaque(255, 204, 102);
    let expected_ink = Color::opaque(255, 255, 255);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                Some("panel:trailing-action"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT | WidgetState::START_ALIGNED,
                Some("Terminal"),
                Some(IconId::Terminal),
            ),
        ],
    ))
    .expect("Command Deck terminal tab fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(480, 48))
        .expect("Command Deck terminal tab render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. } if *color == theme.palette.surface
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Terminal" && *color == expected_ink
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::Terminal,
            color,
            ..
        } if *color == expected_ink
    )));
}

#[test]
fn command_deck_buttons_are_solid_lcars_keys_instead_of_outlined_pills() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.button_shape = rice_ui::ButtonShape::Pill;
    theme.palette.selection = Color::opaque(255, 204, 102);
    theme.palette.border = Color::opaque(204, 153, 204);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("Open folder"),
            None,
        )],
    ))
    .expect("Command Deck button fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(220, 48))
        .expect("Command Deck button render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::ShapedFill { color, shape, .. }
            if *color == Color::opaque(255, 204, 102)
                && *shape == rice_ui::ButtonShape::Pill
    )));
    assert!(
        !output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::ShapedBorder { color, .. }
                if *color == Color::opaque(204, 153, 204)
        )),
        "the solid LCARS key must not read as an outlined dark pill"
    );
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Open folder" && *color == Color::opaque(10, 8, 13)
    )));
}

#[test]
fn command_deck_inactive_list_rows_keep_light_ink_until_their_surface_is_filled() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.button_shape = rice_ui::ButtonShape::Pill;
    theme.palette.text = Color::opaque(255, 244, 220);
    theme.palette.selection = Color::opaque(255, 204, 102);
    theme.palette.border = Color::opaque(204, 153, 204);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::List,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Documents"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Places"),
                None,
            ),
        ],
    ))
    .expect("Command Deck list fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(240, 64))
        .expect("Command Deck list render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Documents" && *color == Color::opaque(255, 244, 220)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Places" && *color == Color::opaque(255, 244, 220)
    )));

    let row = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("Documents row target")
        .rect;
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        Point {
            x: row.origin.x + 2,
            y: row.origin.y + 2,
        },
        0,
        0,
    )
    .expect("hover Documents row");
    let hovered = host
        .render(app(1), window(1), fixed_constraints(240, 64))
        .expect("hovered Command Deck list render");
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::ShapedFill { rect, color, shape }
            if *rect == row && *color == Color::opaque(204, 153, 204)
                && *shape == rice_ui::ButtonShape::Pill
    )));
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Documents" && *color == Color::opaque(10, 8, 13)
    )));
}

#[test]
fn hovering_any_browser_tab_child_highlights_the_complete_tab_surface() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:tab:browser-tab:control-for=4"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::ENABLED,
                None,
                Some(IconId::Network),
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("A browser tab"),
                None,
            ),
            create(
                5,
                Some(2),
                2,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("panel:tab-close"),
                Some(IconId::Close),
            ),
        ],
    ))
    .expect("browser tab hover fixture");
    let constraints = fixed_constraints(480, 48);
    let initial = host
        .render(app(1), window(1), constraints)
        .expect("initial browser tab render");
    let complete_tab = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4) && target.rect.size.width > 150)
        .expect("complete tab activation target")
        .rect;
    let tab_face = Rect {
        origin: Point {
            x: complete_tab.origin.x,
            y: complete_tab.origin.y + 4,
        },
        size: Size {
            width: complete_tab.size.width,
            height: complete_tab.size.height - 4,
        },
    };
    let favicon = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("favicon target")
        .rect;
    assert!(
        initial.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, .. } if rect.origin == tab_face.origin && rect.size.width == tab_face.size.width && rect.size.height > tab_face.size.height
        )),
        "an inactive browser tab needs a visible boundary from the strip"
    );
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        Point {
            x: favicon.origin.x + 1,
            y: favicon.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover favicon");
    let hovered = host
        .render(app(1), window(1), constraints)
        .expect("hovered browser tab render");
    assert!(hovered.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if rect.origin == tab_face.origin && rect.size.width == tab_face.size.width && rect.size.height > tab_face.size.height
                    && *color != host.theme().palette.surface
        )
    }));
    assert!(
        hovered.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, color, .. }
                if rect.origin == tab_face.origin && rect.size.width == tab_face.size.width && rect.size.height > tab_face.size.height
                    && *color == host.theme().palette.accent
        )),
        "hover has a clearly visible, theme-colored edge"
    );
}

#[test]
fn decorative_bookmark_favicon_activates_its_row_and_exposes_the_url_tooltip() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::ToolBar,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:compact:bookmark:control-for=4"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::empty(),
                Some("inline-image:data:image/png;base64,iVBORw0KGgo="),
                None,
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Example"),
                None,
            ),
            create(
                5,
                Some(4),
                0,
                WidgetKind::Tooltip,
                WidgetState::ENABLED,
                Some("https://example.test/articles/one"),
                None,
            ),
        ],
    ))
    .expect("bookmark fixture");
    let constraints = fixed_constraints(420, 48);
    let output = host
        .render(app(1), window(1), constraints)
        .expect("bookmark render");
    let favicon = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::InlineImage { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("bookmark favicon");
    let label = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Example" => Some(*rect),
            _ => None,
        })
        .expect("bookmark label");
    assert!(
        label
            .origin
            .x
            .saturating_sub(favicon.origin.x.saturating_add_unsigned(favicon.size.width))
            <= 8,
        "bookmark label is too far from its favicon: favicon={favicon:?} label={label:?}",
    );
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(3))
    );
    let point = Point {
        x: favicon.origin.x.saturating_add(1),
        y: favicon.origin.y.saturating_add(1),
    };
    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("bookmark press");
    let activated = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("bookmark release")
        .expect("bookmark activation");
    assert_eq!(activated.target, widget(4));
    assert_eq!(activated.event, UiEvent::Activate);

    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 3, 0)
        .expect("bookmark context press");
    let context = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 3, 0)
        .expect("bookmark context release")
        .expect("bookmark context event");
    assert_eq!(context.target, widget(4));
    assert!(matches!(
        context.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Up,
            button: 3,
            ..
        }
    ));

    host.pointer_event(app(1), window(1), PointerEventKind::Move, point, 0, 0)
        .expect("bookmark hover");
    let hovered = host
        .render(app(1), window(1), constraints)
        .expect("hovered bookmark render");
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "https://example.test/articles/one"
    )));
}

#[test]
fn status_bar_height_is_invariant_when_loading_progress_appears() {
    let render = |with_progress: bool| {
        let mut patches = vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Container,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:status-bar:bowl-status"),
                None,
            ),
        ];
        if with_progress {
            patches.push(create(
                4,
                Some(3),
                0,
                WidgetKind::ProgressBar,
                WidgetState::ENABLED,
                Some("35/100"),
                None,
            ));
        }
        patches.push(create(
            5,
            Some(3),
            u32::from(with_progress),
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::FLAT,
            Some("Ready"),
            None,
        ));
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(1, patches))
            .expect("status fixture");
        host.render(app(1), window(1), fixed_constraints(640, 360))
            .expect("status fixture render")
            .hit_targets
            .into_iter()
            .find(|target| target.widget == widget(5))
            .expect("status child target")
            .rect
    };

    let idle = render(false);
    let loading = render(true);
    assert_eq!(loading.origin.y, idle.origin.y);
    assert_eq!(loading.size.height, idle.size.height);
    assert_eq!(loading.size.height, 32);
}

#[test]
fn ordinary_labels_wrap_without_requiring_a_settings_page_token() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Required labels wrap to complete visible lines instead of being capped by a decorative row."),
                None,
            ),
            create(3, Some(1), 1, WidgetKind::Container, WidgetState::ENABLED, None, None),
        ],
    ))
    .expect("label fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(180, 180))
        .expect("label render");
    let label = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text.starts_with("Required labels") => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("wrapped label");
    assert!(label.size.height > u32::from(host.theme().metrics.control_height));
}

fn fixed_constraints(width: u32, height: u32) -> Constraints {
    Constraints::new(Size { width, height }, Size { width, height })
        .expect("valid fixed constraints")
}

#[test]
fn disabled_control_can_expose_a_hover_tooltip_without_becoming_activatable() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::empty(),
                Some("Local Files"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Tooltip,
                WidgetState::ENABLED,
                Some("No folders were mounted\nChoose a folder in Settings"),
                None,
            ),
        ],
    ))
    .expect("tooltip patch");
    let initial = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("initial render");
    assert!(!initial.commands.iter().any(
        |command| matches!(command, DrawCommand::Text { text, .. } if text.contains("No folders were mounted"))
    ));
    let anchor = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .copied()
        .expect("disabled tooltip anchor remains hoverable");
    assert!(!anchor.focusable && !anchor.activatable);
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        Point {
            x: anchor.rect.origin.x,
            y: anchor.rect.origin.y,
        },
        0,
        0,
    )
    .expect("hover accepted");
    let hovered = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("hovered render");
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, rect, .. }
            if text == "No folders were mounted\nChoose a folder in Settings"
                && rect.size.height >= 24
    )));
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::BalloonFrame { caret_x, .. } if *caret_x > 0
    )));
}

#[test]
fn tooltip_is_pointer_transparent_over_controls_beneath_it() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Tree,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::TreeItem,
                WidgetState::ENABLED,
                Some("Documents"),
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Tooltip,
                WidgetState::ENABLED,
                Some("/root/Documents"),
                None,
            ),
            create(
                5,
                Some(2),
                1,
                WidgetKind::TreeItem,
                WidgetState::ENABLED,
                Some("Pictures"),
                None,
            ),
            create(
                6,
                Some(2),
                2,
                WidgetKind::TreeItem,
                WidgetState::ENABLED,
                Some("Downloads"),
                None,
            ),
        ],
    ))
    .expect("tree tooltip fixture");
    let constraints = fixed_constraints(320, 240);
    let initial = host
        .render(app(1), window(1), constraints)
        .expect("initial tooltip render");
    let anchor = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("tooltip anchor")
        .rect;
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        anchor.origin,
        0,
        0,
    )
    .expect("anchor hover");
    let opened = host
        .render(app(1), window(1), constraints)
        .expect("opened tooltip render");
    let tooltip = opened
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::BalloonFrame { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("painted tooltip");
    assert!(
        !opened
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4)),
        "the tooltip surface must never become a pointer target",
    );
    let (underlying, overlap) = opened
        .hit_targets
        .iter()
        .filter(|target| matches!(target.widget.get(), 5 | 6))
        .find_map(|target| {
            target
                .rect
                .intersection(tooltip)
                .map(|overlap| (*target, overlap))
        })
        .expect("tooltip overlaps a later tree item");
    let point = Point {
        x: overlap.origin.x.saturating_add(1),
        y: overlap.origin.y.saturating_add(1),
    };
    assert_eq!(
        host.hit_test(app(1), window(1), point)
            .map(|target| target.widget),
        Some(underlying.widget),
        "painted tooltip pixels must hit the tree item beneath them",
    );
    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("tree item press through tooltip");
    let activated = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("tree item release through tooltip")
        .expect("tree item activation");
    assert_eq!(activated.target, underlying.widget);
}

#[test]
fn tooltip_identical_to_a_nested_visible_label_is_never_shown() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::TRAILING_ICON,
                None,
                Some(IconId::GoForward),
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:compact"),
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Open\nguide"),
                None,
            ),
            create(
                5,
                Some(2),
                1,
                WidgetKind::Tooltip,
                WidgetState::ENABLED,
                Some("Open guide"),
                None,
            ),
        ],
    ))
    .expect("redundant tooltip fixture");
    let initial = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("initial render");
    let anchor = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .copied()
        .expect("button target");
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        anchor.rect.origin,
        0,
        0,
    )
    .expect("hover accepted");
    let hovered = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("hovered render");
    assert_eq!(
        hovered
            .commands
            .iter()
            .filter(
                |command| matches!(command, DrawCommand::Text { text, .. } if text == "Open guide" || text == "Open\nguide")
            )
            .count(),
        1,
        "the visible button label must not be repeated in a tooltip",
    );
    assert!(
        !hovered
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::BalloonFrame { .. })),
        "an identical label must not create an empty balloon",
    );
}

#[test]
fn layout_and_draw_list_are_deterministic_and_semantic() {
    let mut host = gallery_host();
    let first = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    let second = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    assert_eq!(first, second);
    assert_eq!(
        first.size,
        Size {
            width: 320,
            height: 180
        }
    );
    assert!(first.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::DocumentOpen,
            ..
        }
    )));
    assert!(
        first
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Open"))
    );
    assert_eq!(first.hit_targets.len(), 2);
    assert!(
        first
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(6))
    );
}

#[test]
fn accordion_hides_and_reveals_markdown_without_reserving_dead_space() {
    let make_host = |expanded: bool| {
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(
            1,
            vec![
                create(
                    1,
                    None,
                    0,
                    WidgetKind::Column,
                    WidgetState::ENABLED,
                    None,
                    None,
                ),
                create(
                    2,
                    Some(1),
                    0,
                    WidgetKind::Accordion,
                    WidgetState::ENABLED
                        | if expanded {
                            WidgetState::CHECKED
                        } else {
                            WidgetState::empty()
                        },
                    Some("What is RiceWM?"),
                    Some(IconId::ChevronDown),
                ),
                create(
                    3,
                    Some(2),
                    0,
                    WidgetKind::Markdown,
                    WidgetState::ENABLED,
                    Some("**RiceWM** is browser-native.\n\n- Fast\n- Local-first"),
                    None,
                ),
            ],
        ))
        .expect("accordion patch");
        host
    };

    let mut collapsed = make_host(false);
    let collapsed = collapsed
        .render(app(1), window(1), fixed_constraints(560, 240))
        .expect("collapsed render");
    assert!(!collapsed.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, role: TextRole::Markdown, .. }
            if text.contains("browser-native")
    )));
    let header = collapsed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("accordion header target");
    assert!(header.activatable);
    assert!(header.rect.size.height <= 40);

    let mut expanded = make_host(true);
    let expanded = expanded
        .render(app(1), window(1), fixed_constraints(560, 240))
        .expect("expanded render");
    assert!(expanded.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, role: TextRole::Markdown, .. }
            if text.contains("browser-native")
    )));
    assert!(expanded.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::ChevronUp,
            ..
        }
    )));
}

#[test]
fn navigation_row_places_its_semantic_caret_at_the_trailing_edge() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON,
            Some("Show External IP"),
            Some(IconId::GoForward),
        )],
    ))
    .expect("navigation row patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(360, 40))
        .expect("navigation row render");
    let text_right = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Show External IP" => {
                Some(rect.origin.x.saturating_add_unsigned(rect.size.width))
            }
            _ => None,
        })
        .expect("navigation label");
    let caret = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::GoForward,
                ..
            } => Some(*rect),
            _ => None,
        })
        .expect("navigation caret");
    assert!(caret.origin.x >= text_right);
    assert!(caret.origin.x > 300);
    assert!(output.hit_targets[0].activatable);
    assert_eq!(output.hit_targets[0].rect.size.width, 360);
}

#[test]
fn child_bearing_icon_only_button_still_places_a_trailing_caret_at_the_edge() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON,
                None,
                Some(IconId::GoForward),
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
                Some("Composite row content"),
                None,
            ),
        ],
    ))
    .expect("composite navigation patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(360, 56))
        .expect("composite navigation render");
    let caret = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::GoForward,
                ..
            } => Some(*rect),
            _ => None,
        })
        .expect("trailing navigation caret");
    assert!(caret.origin.x > 300);
}

#[test]
fn compact_multiline_copy_reserves_two_canvas_line_boxes_in_familiar_themes() {
    let mut host = NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("split:40:compact:multiline"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:compact:multiline"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED | WidgetState::MUTED | WidgetState::START_ALIGNED,
                Some("The first overview line remains visible.\nThe second line remains visible too."),
                None,
            ),
        ],
    ))
    .expect("multiline overview patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(640, 80))
        .expect("multiline overview render");
    let copy = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text.starts_with("The first overview") => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("multiline overview copy");
    assert!(copy.size.height >= 28, "two 14 px canvas lines must fit");
}

#[test]
fn child_bearing_flat_button_lays_out_its_navigation_slots_horizontally() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT | WidgetState::START_ALIGNED,
                Some("panel:distribute"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Icon"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Title and supporting text"),
                None,
            ),
            create(
                4,
                Some(1),
                2,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Caret"),
                None,
            ),
        ],
    ))
    .expect("composite navigation row patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(420, 56))
        .expect("composite navigation row render");
    let rect_for = |needle: &str| {
        output
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text { rect, text, .. } if text == needle => Some(*rect),
                _ => None,
            })
            .expect("child label")
    };
    let icon = rect_for("Icon");
    let title = rect_for("Title and supporting text");
    let caret = rect_for("Caret");
    assert!(icon.origin.x < title.origin.x && title.origin.x < caret.origin.x);
    assert_eq!(icon.origin.y, title.origin.y);
    assert_eq!(title.origin.y, caret.origin.y);
    assert_eq!(output.hit_targets[0].rect.size.width, 420);
}

#[test]
fn responsive_material_navigation_rows_keep_a_compact_vertical_rhythm() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![create(
        1,
        None,
        0,
        WidgetKind::Column,
        WidgetState::ENABLED,
        Some("panel:settings-page:intrinsic-flow"),
        None,
    )];
    for (index, id) in [10_u64, 20, 30].into_iter().enumerate() {
        patches.extend([
            create(
                id,
                Some(1),
                u32::try_from(index).expect("row index"),
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT | WidgetState::START_ALIGNED,
                Some("panel:navigation-row"),
                None,
            ),
            create(
                id + 1,
                Some(id),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Icon"),
                None,
            ),
            create(
                id + 2,
                Some(id),
                1,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                id + 3,
                Some(id + 2),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Material title"),
                None,
            ),
            create(
                id + 4,
                Some(id + 2),
                1,
                WidgetKind::Markdown,
                WidgetState::ENABLED,
                Some("A concise supporting line that may wrap when required."),
                None,
            ),
            create(
                id + 5,
                Some(id),
                2,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Caret"),
                None,
            ),
        ]);
    }
    host.apply_patch(&envelope(1, patches))
        .expect("responsive material rows patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(640, 280))
        .expect("responsive material rows render");
    let rows = [10_u64, 20, 30].map(|id| {
        output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .expect("navigation row target")
            .rect
    });
    assert!(rows.iter().all(|rect| rect.size.height == 56));
    assert_eq!(rows[1].origin.y - rows[0].origin.y, 60);
    assert_eq!(rows[2].origin.y - rows[1].origin.y, 60);
}

#[test]
fn replacing_theme_changes_colors_without_changing_geometry() {
    let mut host = gallery_host();
    let original = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    let mut changed = Theme::ricewm_default();
    changed.palette.surface = Color::opaque(1, 2, 3);
    changed.palette.surface_raised = Color::opaque(4, 5, 6);
    host.set_theme(changed);
    let themed = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    assert_eq!(original.hit_targets, themed.hit_targets);
    assert_ne!(original.commands, themed.commands);
    assert!(themed.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. } if *color == Color::opaque(4, 5, 6)
    )));
}

#[test]
fn render_measurements_do_not_survive_text_theme_or_width_changes() {
    let mut reused = gallery_host();
    reused.render(app(1), window(1), fixed_constraints(320, 180)).unwrap();
    let update = UiPatch::Update {
        id: widget(3), state: PatchValue::Keep,
        text: PatchValue::Set("A longer caption that needs different geometry".into()),
        icon: PatchValue::Keep,
    };
    reused.apply_patch(&envelope(2, vec![update.clone()])).unwrap();
    for height in [32, 48] {
        let mut theme = Theme::ricewm_default();
        theme.metrics.control_height = height;
        reused.set_theme(theme);
        for width in [180, 480, 180] {
            let mut fresh = gallery_host();
            fresh.apply_patch(&envelope(2, vec![update.clone()])).unwrap();
            fresh.set_theme(theme);
            let constraints = fixed_constraints(width, 300);
            assert_eq!(
                reused.render(app(1), window(1), constraints).unwrap(),
                fresh.render(app(1), window(1), constraints).unwrap(),
            );
        }
    }
}

#[test]
fn calendar_day_columns_render_alternating_themed_bands_and_boundaries() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:distribute"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:calendar-day-band-even"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:calendar-day-band-odd"),
                None,
            ),
        ],
    ))
    .expect("calendar bands");

    let output = host
        .render(app(1), window(1), fixed_constraints(240, 48))
        .expect("calendar band render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.size.width > 80 && *color == host.theme().palette.surface
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.size.width > 80 && *color == host.theme().palette.surface_raised
    )));
    assert_eq!(
        output
            .commands
            .iter()
            .filter(|command| matches!(
                command,
                DrawCommand::Fill { rect, color, .. }
                    if rect.size.width == u32::from(host.theme().metrics.border_width).max(1)
                        && *color == host.theme().palette.border
            ))
            .count(),
        2,
        "each day band needs a visible leading boundary"
    );
}

#[test]
fn selected_calendar_slot_buttons_keep_day_bands_and_pointer_targets() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Table,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:distribute"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("panel:calendar-day-band-even:workday"),
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                None,
                None,
            ),
            create(
                5,
                Some(3),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED | WidgetState::ACCENT,
                Some("05:00–09:00 · Planning block"),
                None,
            ),
            create(
                6,
                Some(2),
                1,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("panel:calendar-day-band-odd"),
                None,
            ),
            create(
                7,
                Some(6),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                None,
                None,
            ),
        ],
    ))
    .expect("calendar slots");

    let output = host
        .render(app(1), window(1), fixed_constraints(240, 48))
        .expect("calendar slots render");
    assert_eq!(
        output
            .hit_targets
            .iter()
            .filter(|target| matches!(target.widget.get(), 4 | 5 | 7))
            .count(),
        3,
        "slot selection and its event overlay should both retain pointer targets"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if rect.size.width > 80 && *color == host.theme().palette.selection
        )),
        "the selected time slot should visibly preview its range"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, radius }
                if rect.size.width > 80
                    && *color == host.theme().palette.accent
                    && *radius == 0
        )),
        "the event overlay must remain visible as a seamless calendar block"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { color, radius: 0, .. }
                if *color == host.theme().palette.accent.with_alpha(18)
        )),
        "configured workday slots need a distinct themed wash"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if rect.size.width > 80 && *color == host.theme().palette.surface_raised
        )),
        "the unselected odd-day slot should retain its calendar band"
    );
}

#[test]
fn row_column_and_stack_reflow_with_window_and_refresh_hit_targets() {
    let mut host = gallery_host();
    let small = host
        .render(app(1), window(1), fixed_constraints(240, 120))
        .expect("small render");
    let small_button = small
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("small button target");
    let small_progress = small.commands.iter().find_map(|command| match command {
        DrawCommand::Fill { rect, color, .. } if *color == host.theme().palette.accent => {
            Some(*rect)
        }
        _ => None,
    });

    let large = host
        .render(app(1), window(1), fixed_constraints(640, 360))
        .expect("large render");
    let large_button = large
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("large button target");
    let large_progress = large.commands.iter().find_map(|command| match command {
        DrawCommand::Fill { rect, color, .. } if *color == host.theme().palette.accent => {
            Some(*rect)
        }
        _ => None,
    });

    assert_eq!(large_button.rect.size.width, small_button.rect.size.width);
    assert_eq!(large_button.rect.size.height, small_button.rect.size.height);
    assert!(large_progress.unwrap().size.width > small_progress.unwrap().size.width);
    assert_eq!(
        host.hit_test(
            app(1),
            window(1),
            Point {
                x: large_button.rect.origin.x,
                y: large_button.rect.origin.y,
            },
        )
        .map(|target| target.widget),
        Some(widget(3))
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn semantic_shell_layout_and_popup_are_visually_bounded() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            10,
            Some(1),
            0,
            WidgetKind::MenuBar,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            11,
            Some(10),
            0,
            WidgetKind::Menu,
            WidgetState::ENABLED,
            Some("File"),
            None,
        ),
        create(
            12,
            Some(11),
            0,
            WidgetKind::MenuItem,
            WidgetState::ENABLED,
            Some("Open"),
            Some(IconId::DocumentOpen),
        ),
        create(
            13,
            Some(10),
            1,
            WidgetKind::Menu,
            WidgetState::ENABLED,
            Some("View"),
            None,
        ),
        create(
            20,
            Some(1),
            1,
            WidgetKind::ToolBar,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            21,
            Some(20),
            0,
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("Back"),
            None,
        ),
        create(
            22,
            Some(20),
            1,
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("Search"),
            None,
        ),
        create(
            30,
            Some(1),
            2,
            WidgetKind::SplitPane,
            WidgetState::ENABLED,
            Some("split:250"),
            None,
        ),
        create(
            31,
            Some(30),
            0,
            WidgetKind::List,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            32,
            Some(30),
            1,
            WidgetKind::IconGrid,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            40,
            Some(1),
            3,
            WidgetKind::Container,
            WidgetState::ENABLED,
            Some("panel:fixed"),
            None,
        ),
        create(
            41,
            Some(40),
            0,
            WidgetKind::Label,
            WidgetState::ENABLED,
            Some("Ready"),
            None,
        ),
    ];
    for index in 0..6_u64 {
        patches.push(create(
            50 + index,
            Some(32),
            u32::try_from(index).expect("grid index"),
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("File"),
            Some(IconId::Document),
        ));
    }
    host.apply_patch(&envelope(1, patches))
        .expect("semantic shell");
    let closed = host
        .render(app(1), window(1), fixed_constraints(480, 320))
        .expect("closed shell");
    let target = |output: &RenderOutput, id| {
        output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .copied()
            .expect("semantic target")
    };
    let file = target(&closed, 11);
    let view = target(&closed, 13);
    let back = target(&closed, 21);
    let search = target(&closed, 22);
    let split = target(&closed, 30);
    let places = target(&closed, 31);
    let grid = target(&closed, 32);
    assert_eq!(file.rect.origin.y, view.rect.origin.y);
    assert!(view.rect.origin.x > file.rect.origin.x);
    assert_eq!(back.rect.origin.y, search.rect.origin.y);
    assert!(search.rect.origin.x > back.rect.origin.x);
    assert!(split.rect.size.height > file.rect.size.height * 3);
    assert!(places.rect.size.width < grid.rect.size.width);
    let split_bottom = split
        .rect
        .origin
        .y
        .saturating_add_unsigned(split.rect.size.height);
    assert!(closed.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Border { rect, .. }
                if rect.origin.y >= split_bottom && rect.size.height <= 64
        )
    }));
    assert!(
        !closed
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Open"))
    );

    let menu_point = Point {
        x: file.rect.origin.x + 2,
        y: file.rect.origin.y + 2,
    };
    host.pointer_event(app(1), window(1), PointerEventKind::Up, menu_point, 1, 0)
        .expect("open menu");
    let opened = host
        .render(app(1), window(1), fixed_constraints(480, 320))
        .expect("opened shell");
    let item = target(&opened, 12);
    assert!(
        item.rect.origin.y
            >= file
                .rect
                .origin
                .y
                .saturating_add_unsigned(file.rect.size.height)
    );
    assert!(
        opened
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Open"))
    );
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Down,
        Point {
            x: item.rect.origin.x + 2,
            y: item.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("menu item press");
    let activation = host
        .pointer_event(
            app(1),
            window(1),
            PointerEventKind::Up,
            Point {
                x: item.rect.origin.x + 2,
                y: item.rect.origin.y + 2,
            },
            1,
            0,
        )
        .expect("menu item")
        .expect("application activation");
    assert_eq!(activation.target, widget(12));
    assert_eq!(activation.event, UiEvent::Activate);
    let closed_again = host
        .render(app(1), window(1), fixed_constraints(480, 320))
        .expect("closed again");
    assert!(
        closed_again
            .hit_targets
            .iter()
            .all(|target| target.widget != widget(12))
    );

    let wide = host
        .render(app(1), window(1), fixed_constraints(900, 420))
        .expect("wide shell");
    let compact_rows = (50..56)
        .map(|id| target(&closed, id).rect.origin.y)
        .collect::<BTreeSet<_>>();
    let wide_rows = (50..56)
        .map(|id| target(&wide, id).rect.origin.y)
        .collect::<BTreeSet<_>>();
    assert!(wide_rows.len() < compact_rows.len());
}

#[test]
fn rejects_stale_and_out_of_order_sequences_without_mutation() {
    let mut host = gallery_host();
    let update = UiPatch::Update {
        id: widget(3),
        state: PatchValue::Keep,
        text: PatchValue::Set("Changed".into()),
        icon: PatchValue::Keep,
    };
    assert!(matches!(
        host.apply_patch(&envelope(1, vec![update.clone()])),
        Err(NativeUiError::StaleSequence {
            received: 1,
            current: 1
        })
    ));
    assert!(matches!(
        host.apply_patch(&envelope(3, vec![update])),
        Err(NativeUiError::OutOfOrderSequence {
            received: 3,
            expected: 2
        })
    ));
    assert_eq!(host.sequence(app(1), window(1)), Some(1));
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    assert!(
        !output
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Changed"))
    );
}

#[test]
fn patch_conversion_and_tree_failures_are_atomic() {
    let mut host = gallery_host();
    let invalid = envelope(
        2,
        vec![create(
            99,
            Some(999),
            0,
            WidgetKind::Label,
            WidgetState::ENABLED,
            None,
            None,
        )],
    );
    assert!(matches!(
        host.apply_patch(&invalid),
        Err(NativeUiError::Tree(TreeError::MissingParent(_)))
    ));
    assert_eq!(host.sequence(app(1), window(1)), Some(1));
}

#[test]
fn hit_testing_focus_activation_and_disabled_controls_are_semantic() {
    let mut host = gallery_host();
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("render");
    let enabled = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("enabled target");
    let point = Point {
        x: enabled.rect.origin.x,
        y: enabled.rect.origin.y,
    };
    let down = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("pointer")
        .expect("event");
    assert!(matches!(down.event, UiEvent::Pointer { .. }));
    let up = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("pointer")
        .expect("event");
    assert_eq!(up.target, widget(3));
    assert_eq!(up.event, UiEvent::Activate);
    let key = host
        .key_event(app(1), window(1), true, 36, 0, None)
        .expect("key")
        .expect("event");
    assert_eq!(key.event, UiEvent::Activate);
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4))
    );
}

#[test]
fn primary_button_is_visibly_held_until_pointer_up() {
    let mut host = gallery_host();
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("initial render");
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("button target");
    let point = Point {
        x: target.rect.origin.x + 1,
        y: target.rect.origin.y + 1,
    };
    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("button down");
    let held = host
        .render(app(1), window(1), fixed_constraints(320, 180))
        .expect("held render");
    assert!(held.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if *rect == target.rect && *color == host.theme().palette.selection
    )));

    host.pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("button up");
    assert_eq!(
        host.windows
            .get(&(app(1), window(1)))
            .and_then(|window| window.pressed),
        None,
    );
}

#[test]
fn yellow_jacket_separates_persistent_blue_selection_from_gray_mouse_press() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::YellowJacket;
    theme.palette.surface_raised = Color::opaque(245, 245, 245);
    theme.palette.selection = Color::opaque(23, 105, 170);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Button,
            WidgetState::ENABLED | WidgetState::SELECTED,
            Some("Dock"),
            None,
        )],
    ))
    .expect("Yellow Jacket selected button");
    let selected = host
        .render(app(1), window(1), fixed_constraints(180, 40))
        .expect("selected render");
    let target = selected.hit_targets[0];
    assert!(selected.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if *rect == target.rect && *color == Color::opaque(23, 105, 170)
    )));

    let point = Point {
        x: target.rect.origin.x + 2,
        y: target.rect.origin.y + 2,
    };
    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("press selected button");
    let held = host
        .render(app(1), window(1), fixed_constraints(180, 40))
        .expect("held Yellow Jacket render");
    assert!(held.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if *rect == target.rect && *color == Color::opaque(191, 191, 191)
    )));
    assert!(held.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Dock" && *color == Color::opaque(0, 0, 0)
    )));

    host.pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("release selected button");
    let released = host
        .render(app(1), window(1), fixed_constraints(180, 40))
        .expect("released Yellow Jacket render");
    assert!(released.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if *rect == target.rect && *color == Color::opaque(23, 105, 170)
    )));
}

#[test]
fn yellow_jacket_active_browser_tab_keeps_dark_text_on_its_raised_surface() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::YellowJacket;
    theme.palette.text = Color::opaque(16, 16, 16);
    theme.palette.surface_raised = Color::opaque(245, 245, 245);
    theme.palette.selection = Color::opaque(23, 105, 170);
    let expected_text = theme.palette.text;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED,
                Some("panel:browser-tab-strip"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab:browser-tab:control-for=3"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Active tab"),
                None,
            ),
        ],
    ))
    .expect("Yellow Jacket active tab fixture");

    let output = host
        .render(app(1), window(1), fixed_constraints(320, 42))
        .expect("Yellow Jacket active tab render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Active tab" && *color == expected_text
    )));
}

#[test]
fn yellow_jacket_open_menu_heading_keeps_dark_text_on_its_raised_surface() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::YellowJacket;
    theme.palette.text = Color::opaque(16, 16, 16);
    theme.palette.surface_raised = Color::opaque(245, 245, 245);
    theme.palette.selection = Color::opaque(23, 105, 170);
    let expected_text = theme.palette.text;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::MenuBar,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("Files"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Open"),
                None,
            ),
        ],
    ))
    .expect("Yellow Jacket menu fixture");
    let closed = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("closed Yellow Jacket menu");
    let menu = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("Files menu target")
        .rect;
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Up,
        Point {
            x: menu.origin.x + 2,
            y: menu.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open Files menu");

    let opened = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("open Yellow Jacket menu render");
    assert!(opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Files" && *color == expected_text
    )));
}

#[test]
fn yellow_jacket_text_field_uses_a_quiet_idle_border_and_a_clear_focus_ring() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::YellowJacket;
    theme.palette.border = Color::opaque(85, 85, 85);
    theme.palette.focus = Color::opaque(23, 105, 170);
    theme.metrics.border_width = 2;
    let expected_idle = theme.palette.border.with_alpha(112);
    let expected_focus = theme.palette.focus;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::TextField,
            WidgetState::ENABLED,
            Some("ricewm.org"),
            None,
        )],
    ))
    .expect("Yellow Jacket address field fixture");
    let idle = host
        .render(app(1), window(1), fixed_constraints(360, 32))
        .expect("idle address field render");
    let target = idle.hit_targets[0].rect;
    assert!(idle.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { rect, color, width, .. }
            if *rect == target && *color == expected_idle && *width == 1
    )));

    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Down,
        Point {
            x: target.origin.x + 4,
            y: target.origin.y + 4,
        },
        1,
        0,
    )
    .expect("focus address field");
    let focused = host
        .render(app(1), window(1), fixed_constraints(360, 32))
        .expect("focused address field render");
    assert!(focused.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { color, .. } if *color == expected_focus
    )));
}

#[test]
fn slider_renders_a_track_and_drags_emit_value_changes() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Slider,
                WidgetState::ENABLED,
                Some("250/1000;ticks=5"),
                None,
            ),
        ],
    ))
    .expect("apply slider");
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 64))
        .expect("render");
    let fills = output
        .commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::Fill { .. }))
        .count();
    assert!(fills >= 3, "slider track, fill, and thumb render");
    assert!(
        !output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, .. } if text.contains("ticks=")
        )),
        "slider protocol metadata is never painted as user-visible text",
    );
    let target = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("slider hit target");
    let point = Point {
        x: target
            .rect
            .origin
            .x
            .saturating_add_unsigned(target.rect.size.width / 2),
        y: target
            .rect
            .origin
            .y
            .saturating_add_unsigned(target.rect.size.height / 2),
    };
    let down = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("pointer");
    assert!(down.is_none(), "slider movement is previewed locally");
    let preview = host
        .render(app(1), window(1), fixed_constraints(320, 64))
        .expect("render slider preview");
    assert!(
        preview.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, .. } if rect.size.width > 100
        )),
        "slider preview remains rendered during the drag"
    );
    let event = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("pointer")
        .expect("commit event");
    assert!(
        matches!(event.event, UiEvent::ValueChanged(ref value)
            if value.split('/').next().and_then(|part| part.parse::<u32>().ok())
                .is_some_and(|position| (490..=510).contains(&position))),
        "dragging the middle of the slider reports ~50%: {event:?}",
    );
}

#[test]
fn color_picker_previews_during_the_gesture_and_commits_only_on_release() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::ScrollBar,
            WidgetState::ENABLED,
            Some("color-picker:#000000"),
            None,
        )],
    ))
    .expect("apply color picker");
    let output = host
        .render(app(1), window(1), fixed_constraints(480, 112))
        .expect("render color picker");
    let rect = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("color picker hit target")
        .rect;
    let point = Point {
        x: rect
            .origin
            .x
            .saturating_add_unsigned(rect.size.width.saturating_sub(2)),
        y: rect.origin.y.saturating_add(2),
    };

    let down = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .expect("start color selection");
    assert!(down.is_none(), "mouse-down must not commit the setting");
    assert_eq!(
        host.windows
            .get(&(app(1), window(1)))
            .and_then(|window| window.tree.get(widget(1)))
            .and_then(|picker| picker.text.as_deref()),
        Some("color-picker:#ffffff"),
        "the marker should still preview the pointed color",
    );

    let event = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("release color selection")
        .expect("release commits color");
    assert!(matches!(event.event, UiEvent::ValueChanged(ref value) if value == "#ffffff"));
    assert_eq!(
        host.windows
            .get(&(app(1), window(1)))
            .and_then(|window| window.scroll_drag),
        None,
        "release must clear pointer capture",
    );
}

#[test]
fn split_pane_only_captures_its_divider_and_drags_without_layout_jumps() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::SplitPane,
                WidgetState::ENABLED,
                Some("split:300"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Places"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Files"),
                None,
            ),
        ],
    ))
    .expect("split fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(1_000, 300))
        .expect("split render");
    let divider = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("split divider target");
    assert_eq!(divider.rect.size.width, 10);
    assert_eq!(
        host.hit_test(app(1), window(1), Point { x: 100, y: 100 })
            .expect("left content target")
            .widget,
        widget(2),
    );
    let down = host
        .pointer_event(
            app(1),
            window(1),
            PointerEventKind::Down,
            Point {
                x: divider.rect.origin.x + 2,
                y: 100,
            },
            1,
            0,
        )
        .expect("divider down")
        .expect("divider event");
    assert!(matches!(down.event, UiEvent::Pointer { x: 295..=305, .. }));
    let drag = host
        .pointer_event(
            app(1),
            window(1),
            PointerEventKind::Move,
            Point { x: 800, y: 100 },
            1,
            0,
        )
        .expect("divider drag")
        .expect("captured divider event");
    assert!(matches!(drag.event, UiEvent::Pointer { x: 795..=805, .. }));

    // Missing button-up events must not leave the splitter captured. A leave
    // and a subsequent click in the Places content both target that content,
    // never the old divider.
    let left = Point { x: 100, y: 100 };
    let leave = host
        .pointer_event(app(1), window(1), PointerEventKind::Leave, left, 0, 0)
        .expect("pointer leave")
        .expect("left content leave");
    assert_eq!(leave.target, widget(2));
    let next_down = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, left, 1, 0)
        .expect("new content down")
        .expect("content pointer event");
    assert_eq!(next_down.target, widget(2));
}

#[test]
fn split_pane_honors_pixel_bounds_at_narrow_and_wide_sizes() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::SplitPane,
                WidgetState::ENABLED,
                Some("split:200:min-width=200:max-width=420"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Places"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Files"),
                None,
            ),
        ],
    ))
    .expect("bounded split fixture");
    let narrow = host
        .render(app(1), window(1), fixed_constraints(600, 300))
        .expect("narrow split");
    let narrow_divider = narrow
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("narrow divider");
    assert!((195..=205).contains(&narrow_divider.rect.origin.x));

    host.apply_patch(&envelope(
        2,
        vec![UiPatch::Update {
            id: widget(1),
            state: PatchValue::Keep,
            text: PatchValue::Set("split:800:min-width=200:max-width=420".into()),
            icon: PatchValue::Keep,
        }],
    ))
    .expect("wide ratio");
    let wide = host
        .render(app(1), window(1), fixed_constraints(1_200, 300))
        .expect("wide split");
    let wide_divider = wide
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .expect("wide divider");
    assert!((415..=425).contains(&wide_divider.rect.origin.x));
}

#[test]
fn indeterminate_progress_emits_a_bounded_animation_primitive() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::ProgressBar,
            WidgetState::ENABLED | WidgetState::INDETERMINATE,
            Some("Working…"),
            None,
        )],
    ))
    .expect("activity progress patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(300, 40))
        .expect("activity progress render");
    let segment = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Activity { rect, color } if *color == host.theme().palette.accent => {
                Some(*rect)
            }
            _ => None,
        })
        .expect("indeterminate activity segment");
    assert_eq!(segment.size.width, 300);
    assert_eq!(segment.origin.x, 0);
}

#[test]
fn icon_grid_long_names_do_not_collapse_a_wide_view_to_one_column() {
    let mut host = NativeUiHost::default();
    let long_name = "a-very-long-mounted-movie-filename-that-must-wrap-inside-the-tile-and-never-set-the-grid-column-width.mp4";
    let mut patches = vec![create(
        1,
        None,
        0,
        WidgetKind::IconGrid,
        WidgetState::ENABLED,
        None,
        None,
    )];
    for index in 0..6_u64 {
        patches.push(create(
            10 + index,
            Some(1),
            u32::try_from(index).expect("grid index"),
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some(long_name),
            Some(IconId::Document),
        ));
    }
    host.apply_patch(&envelope(1, patches))
        .expect("long-name grid");
    let output = host
        .render(app(1), window(1), fixed_constraints(900, 500))
        .expect("wide grid");
    let positions = (0..6_u64)
        .map(|index| {
            output
                .hit_targets
                .iter()
                .find(|target| target.widget == widget(10 + index))
                .expect("grid item target")
                .rect
                .origin
                .x
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        positions.len() >= 3,
        "a wide view should retain multiple columns despite long filenames"
    );
}

#[test]
fn file_grid_zoom_scales_plain_semantic_icons() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("file-grid:500"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Folder"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("small grid");
    let small = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("small render");
    let small_icon = small
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::Folder,
                ..
            } => Some(rect.size.width),
            _ => None,
        })
        .expect("small folder icon");
    host.apply_patch(&envelope(
        2,
        vec![UiPatch::Update {
            id: widget(1),
            state: PatchValue::Keep,
            text: PatchValue::Set(String::from("file-grid:2000")),
            icon: PatchValue::Keep,
        }],
    ))
    .expect("large grid");
    let large = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("large render");
    let large_icon = large
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::Folder,
                ..
            } => Some(rect.size.width),
            _ => None,
        })
        .expect("large folder icon");
    assert!(large_icon > small_icon.saturating_mul(3));
}

#[test]
fn file_grid_empty_state_fills_and_centers_in_the_viewport() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("file-grid:650"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED
                    | WidgetState::MUTED
                    | WidgetState::CENTER_ALIGNED
                    | WidgetState::BRAND_TYPE
                    | WidgetState::EMPHASIZED,
                Some("This folder is empty"),
                None,
            ),
        ],
    ))
    .expect("empty file grid");
    let output = host
        .render(app(1), window(1), fixed_constraints(640, 360))
        .expect("empty file grid render");
    let (rect, size_px, align) = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text {
                rect,
                text,
                size_px,
                align,
                ..
            } if text == "This folder is empty" => Some((*rect, *size_px, *align)),
            _ => None,
        })
        .expect("centered empty-state text");
    assert!(
        rect.size.width >= 600,
        "empty state width was {}",
        rect.size.width
    );
    assert!(
        rect.size.height >= 320,
        "empty state height was {}",
        rect.size.height
    );
    assert!(size_px >= 15, "empty state text was only {size_px}px");
    assert_eq!(align, TextAlign::Center);
}

#[test]
fn file_grid_icon_tightly_fits_its_selection_tile() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("file-grid:1000"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Pictures"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("selected file grid");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("selected file grid render");
    let tile = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("file tile target")
        .rect;
    let icon = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::Folder,
                ..
            } => Some(*rect),
            _ => None,
        })
        .expect("folder icon");
    assert!(
        icon.size.height.saturating_mul(3) >= tile.size.height.saturating_mul(2),
        "file icon should occupy at least two thirds of its tile: icon={icon:?}, tile={tile:?}"
    );
}

#[test]
fn frosted_selected_file_tile_uses_a_solid_selection_surface() {
    let mut theme = Theme::ricewm_default();
    theme.frosted_widgets.buttons = true;
    theme.palette.selection = Color::opaque(0, 122, 255);
    let expected = theme.palette.selection;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("file-grid:1000"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Folder"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("selected frosted file tile");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("selected frosted file tile render");
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { color, .. } if *color == expected
        )),
        "selected tile commands: {:?}",
        output.commands
    );
}

#[test]
fn redmond_selected_file_grid_item_uses_caption_and_icon_selection_only() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    theme.palette.selection = Color::opaque(0, 0, 128);
    theme.palette.selection_text = Color::opaque(255, 255, 255);
    let focus = theme.palette.focus;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED | WidgetState::FOCUSED,
                Some("file-grid:1000"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED | WidgetState::FOCUSED,
                Some("Pictures"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("Redmond file-grid fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("Redmond file-grid render");
    let tile = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("file tile target")
        .rect;
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, .. } if *rect == tile
    )));
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { rect, color, .. } if *rect == tile && *color == focus
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::Folder,
            color,
            ..
        } if color.channels()[3] == 176
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, rect, .. }
            if *color == Color::opaque(0, 0, 128)
                && rect.size.width < tile.size.width
                && rect.size.height < tile.size.height
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Pictures" && *color == Color::opaque(255, 255, 255)
    )));
}

#[test]
fn redmond_file_grid_keeps_its_first_icon_origin_across_navigation() {
    fn first_folder_x(labels: &[&str]) -> i32 {
        let mut theme = Theme::familiar_light();
        theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
        let mut host = NativeUiHost::new(theme);
        let mut patches = vec![create(
            1,
            None,
            0,
            WidgetKind::IconGrid,
            WidgetState::ENABLED,
            Some("file-grid:1000"),
            None,
        )];
        for (index, label) in labels.iter().enumerate() {
            patches.push(create(
                u64::try_from(index).unwrap() + 2,
                Some(1),
                u32::try_from(index).unwrap(),
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some(label),
                Some(IconId::Folder),
            ));
        }
        host.apply_patch(&envelope(1, patches)).expect("file grid");
        host.render(app(1), window(1), fixed_constraints(537, 300))
            .expect("file grid render")
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Icon {
                    rect,
                    icon: IconId::Folder,
                    ..
                } => Some(rect.origin.x),
                _ => None,
            })
            .expect("first folder icon")
    }

    assert_eq!(
        first_folder_x(&["Examples"]),
        first_folder_x(&["Calculator", "Welcome"]),
        "navigation must not recenter the first classic Explorer icon"
    );
}

#[test]
fn redmond_thumbnail_cell_never_paints_a_card_or_cell_border() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    theme.palette.selection = Color::opaque(0, 0, 128);
    theme.palette.surface_raised = Color::opaque(1, 2, 3);
    theme.palette.border = Color::opaque(4, 5, 6);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("file-grid:1000"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Container,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:thumbnail-cell"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::ENABLED | WidgetState::SELECTED,
                None,
                Some(IconId::Files),
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED | WidgetState::FLAT,
                Some("wallpaper.png"),
                None,
            ),
        ],
    ))
    .expect("Redmond thumbnail fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("Redmond thumbnail render");
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, .. } if *color == Color::opaque(1, 2, 3)
    )));
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { color, .. } if *color == Color::opaque(4, 5, 6)
    )));
}

#[test]
fn file_layout_metadata_is_never_rendered_as_user_text() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Table,
            WidgetState::ENABLED,
            Some("file-grid:1000"),
            None,
        )],
    ))
    .expect("file table");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 300))
        .expect("file table render");
    assert!(!output.commands.iter().any(|command| {
        matches!(command, DrawCommand::Text { text, .. } if text.starts_with("file-grid:"))
    }));
}

#[test]
fn file_detail_empty_row_metadata_is_never_rendered_as_user_text() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:file-detail-empty-row:fixed-height:18:dense-row"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED | WidgetState::MUTED,
                Some("This directory is empty"),
                Some(IconId::FolderOpen),
            ),
        ],
    ))
    .expect("empty detail row fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 18))
        .expect("empty detail row render");
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text.contains("file-detail-empty-row")
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "This directory is empty"
    )));
}

#[test]
fn redmond_detail_row_selection_colors_all_cells_without_child_bevels() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    theme.palette.selection = Color::opaque(0, 0, 128);
    theme.palette.selection_text = Color::opaque(255, 255, 255);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("split:440:control-for=3:file-detail-row:primary:fixed-height:18:dense-row"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:distribute:dense-table:detail-name-cell"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Downloads"),
                Some(IconId::Folder),
            ),
            create(
                4,
                Some(1),
                1,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("File Folder"),
                None,
            ),
        ],
    ))
    .expect("selected detail row fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(640, 18))
        .expect("selected detail row render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, rect, .. }
            if *color == Color::opaque(0, 0, 128) && rect.size.width == 640
    )));
    for label in ["Downloads", "File Folder"] {
        assert!(
            output.commands.iter().any(|command| matches!(
                command,
                DrawCommand::Text { text, color, .. }
                    if text == label && *color == Color::opaque(255, 255, 255)
            )),
            "selected detail text must use selection ink: {label}"
        );
    }
    assert!(
        !output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, .. } if rect.size.width < 640
        )),
        "detail row descendants must not paint push-button bevels"
    );
}

#[test]
fn every_redmond_file_detail_header_cell_uses_the_same_grey_bevel() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("split:440:file-detail-header:fixed-height:18:dense-row"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Name ↑"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("split:280"),
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Size"),
                None,
            ),
            create(
                5,
                Some(3),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Type"),
                None,
            ),
            create(
                6,
                Some(3),
                2,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Modified"),
                None,
            ),
        ],
    ))
    .expect("Redmond detail header fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(640, 18))
        .expect("Redmond detail header render");
    for id in 2..=6_u64 {
        if id == 3 {
            continue;
        }
        let rect = output
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .unwrap_or_else(|| panic!("header cell {id}"))
            .rect;
        assert!(output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect: fill, color, radius }
                if *fill == rect && *color == Color::opaque(192, 192, 192) && *radius == 0
        )));
    }
    for label in ["Name ↑", "Size", "Type", "Modified"] {
        assert!(
            output.commands.iter().any(|command| matches!(
                command,
                DrawCommand::Text { text, .. } if text == label
            )),
            "missing header label {label}: {:?}",
            output.commands
        );
    }
}

#[test]
fn file_detail_disclosure_spacer_is_geometry_not_a_painted_control() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Container,
            WidgetState::ENABLED,
            Some("panel:equal-fixed:fixed-width:18:detail-disclosure-spacer"),
            None,
        )],
    ))
    .expect("detail disclosure spacer fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(18, 18))
        .expect("detail disclosure spacer render");
    assert!(
        output.commands.is_empty(),
        "spacer painted: {:?}",
        output.commands
    );
}

#[test]
fn desktop_thumbnail_cell_is_transparent_at_rest() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("desktop:transparent"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Container,
                WidgetState::ENABLED,
                Some("desktop:thumbnail-cell"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Image,
                WidgetState::ENABLED,
                Some("media-request:9"),
                None,
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("photo.jpg"),
                None,
            ),
        ],
    ))
    .expect("desktop thumbnail tree");
    let output = host
        .render(app(1), window(1), fixed_constraints(96, 100))
        .expect("desktop thumbnail render");
    assert!(
        output
            .commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Media { request: 9, .. }))
    );
    assert!(
        output.commands.iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "photo.jpg")
        )
    );
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { .. } | DrawCommand::Border { .. }
    )));
}

#[test]
fn flat_collection_rows_have_a_visible_subtle_hover_fill() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::List,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
                Some("Downloads"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("places list");
    let constraints = fixed_constraints(320, 120);
    let initial = host
        .render(app(1), window(1), constraints)
        .expect("places render");
    let row = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("place row")
        .rect;
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        Point {
            x: row.origin.x + 1,
            y: row.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover place");
    let hovered = host
        .render(app(1), window(1), constraints)
        .expect("hovered places render");
    assert!(hovered.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == row && *color == host.theme().palette.selection.with_alpha(36)
        )
    }));
}

#[test]
fn full_surface_feature_cards_have_a_visible_subtle_hover_fill() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.selection = Color::opaque(255, 204, 102);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("panel:feature-card-action"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("GIMP"),
                Some(IconId::Application),
            ),
        ],
    ))
    .expect("feature card fixture");
    let constraints = fixed_constraints(320, 120);
    let initial = host
        .render(app(1), window(1), constraints)
        .expect("feature card render");
    let card = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("feature card target")
        .rect;
    assert!(initial.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == card && *color == host.theme().palette.surface_raised
        )
    }));
    assert!(!initial.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::ShapedFill { rect, color, .. }
                if *rect == card && *color == host.theme().palette.selection
        )
    }));
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Move,
        Point {
            x: card.origin.x + 1,
            y: card.origin.y + 1,
        },
        0,
        0,
    )
    .expect("hover feature card");
    let hovered = host
        .render(app(1), window(1), constraints)
        .expect("hovered feature card render");
    assert!(hovered.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == card && *color == host.theme().palette.selection.with_alpha(40)
        )
    }));
}

#[test]
fn detail_tree_depth_indents_the_file_icon_and_label() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Table,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
                Some("▾ Folder"),
                Some(IconId::Folder),
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
                Some("  child.txt"),
                Some(IconId::Document),
            ),
        ],
    ))
    .expect("detail tree");
    let output = host
        .render(app(1), window(1), fixed_constraints(500, 160))
        .expect("detail tree render");
    let icon_x = |icon| {
        output.commands.iter().find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: candidate,
                ..
            } if *candidate == icon => Some(rect.origin.x),
            _ => None,
        })
    };
    assert!(
        icon_x(IconId::Document).expect("child icon") > icon_x(IconId::Folder).expect("root icon")
    );
}

#[test]
fn desktop_grid_keeps_empty_cells_and_pointer_up_for_free_positioning() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::Stack,
            WidgetState::ENABLED,
            Some("desktop:transparent"),
            None,
        ),
        create(
            2,
            Some(1),
            0,
            WidgetKind::IconGrid,
            WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::KEY_NAVIGATION,
            Some("desktop-grid:3:2"),
            None,
        ),
    ];
    for index in 0..6_u64 {
        let occupied = matches!(index, 0 | 5);
        patches.push(create(
            10 + index,
            Some(2),
            u32::try_from(index).expect("desktop cell index"),
            if occupied {
                WidgetKind::Button
            } else {
                WidgetKind::Stack
            },
            if occupied {
                WidgetState::ENABLED | WidgetState::KEY_NAVIGATION
            } else {
                WidgetState::empty()
            },
            occupied.then_some("Desktop item"),
            occupied.then_some(IconId::Document),
        ));
    }
    host.apply_patch(&envelope(1, patches))
        .expect("desktop grid patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(400, 300))
        .expect("desktop grid render");
    let first = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(10))
        .expect("first desktop icon");
    let last = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(15))
        .expect("last desktop icon");
    assert_eq!(first.rect.origin, Point { x: 10, y: 10 });
    assert_eq!(last.rect.origin.x - first.rect.origin.x, 200);
    assert_eq!(last.rect.origin.y - first.rect.origin.y, 100);
    let point = Point {
        x: first.rect.origin.x + 10,
        y: first.rect.origin.y + 10,
    };
    let released = host
        .pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0)
        .expect("pointer event")
        .expect("desktop item event");
    assert!(matches!(
        released.event,
        UiEvent::Pointer {
            kind: PointerEventKind::Up,
            ..
        }
    ));
}

#[test]
fn wallpaper_desktop_keeps_labels_light_without_recoloring_icons() {
    let mut theme = Theme::familiar_light();
    theme.palette.desktop = Color::opaque(244, 246, 248);
    let expected_icon_ink = Color::opaque(10, 8, 13);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("desktop:transparent"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED | WidgetState::FOCUSED,
                Some("desktop-grid:3:2"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED | WidgetState::FLAT,
                Some("Documents"),
                Some(IconId::Documents),
            ),
        ],
    ))
    .expect("selected desktop fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(400, 300))
        .expect("selected desktop render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Documents" && *color == Color::opaque(255, 255, 255)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Documents" && color.channels()[3] == 190
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::Documents,
            color,
            ..
        } if *color == expected_icon_ink
    )));
}

#[test]
fn redmond_desktop_selection_is_limited_to_caption_and_stippled_icon() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    theme.palette.selection = Color::opaque(0, 0, 128);
    theme.palette.selection_text = Color::opaque(255, 255, 255);
    let focus = theme.palette.focus;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("desktop:transparent"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED | WidgetState::FOCUSED,
                Some("desktop-grid:3:2"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED
                    | WidgetState::SELECTED
                    | WidgetState::FOCUSED
                    | WidgetState::FLAT,
                Some("Trash"),
                Some(IconId::Trash),
            ),
        ],
    ))
    .expect("Redmond desktop fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(400, 300))
        .expect("Redmond desktop render");
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { color, .. } if *color == focus
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: IconId::Trash,
            color,
            ..
        } if color.channels()[3] == 176
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { color, rect, .. }
            if *color == Color::opaque(0, 0, 128) && rect.size.width < 100
    )));
}

#[test]
fn redmond_desktop_recycle_bin_matches_other_classic_icon_artwork() {
    // Existing classic full-bleed assets use 32px; padded desktop artwork
    // remains 48px. Other themes retain the common 48px desktop extent.
    for classic in [true, false] {
        for icon in [
            IconId::Trash,
            IconId::TrashFull,
            IconId::Link,
            IconId::Folder,
        ] {
            let mut theme = Theme::familiar_light();
            if classic {
                theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
            }
            let expected = if classic && icon != IconId::Folder {
                32
            } else {
                48
            };
            let mut host = NativeUiHost::new(theme);
            host.apply_patch(&envelope(
                1,
                vec![
                    create(
                        1,
                        None,
                        0,
                        WidgetKind::Stack,
                        WidgetState::ENABLED,
                        Some("desktop:transparent"),
                        None,
                    ),
                    create(
                        2,
                        Some(1),
                        0,
                        WidgetKind::IconGrid,
                        WidgetState::ENABLED,
                        Some("desktop-grid:2:1:68:68:7:7:8:8"),
                        None,
                    ),
                    create(
                        3,
                        Some(2),
                        0,
                        WidgetKind::Button,
                        WidgetState::ENABLED | WidgetState::FLAT,
                        Some("Trash"),
                        Some(icon),
                    ),
                ],
            ))
            .expect("Redmond trash fixture");
            let output = host
                .render(app(1), window(1), fixed_constraints(300, 180))
                .expect("Redmond trash render");
            let artwork = output
                .commands
                .iter()
                .find_map(|command| match command {
                    DrawCommand::Icon {
                        rect, icon: drawn, ..
                    } if *drawn == icon => Some(*rect),
                    _ => None,
                })
                .expect("trash icon");
            assert_eq!(
                artwork.size,
                Size {
                    width: expected,
                    height: expected
                },
                "classic={classic}, icon={icon:?}"
            );
        }
    }
}

#[test]
fn redmond_desktop_caption_sits_directly_below_its_icon() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                Some("desktop:transparent"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                Some("desktop-grid:3:2"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::FLAT,
                Some("Documents"),
                Some(IconId::Documents),
            ),
        ],
    ))
    .expect("Redmond desktop fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(400, 300))
        .expect("Redmond desktop render");
    let icon = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Icon {
                rect,
                icon: IconId::Documents,
                ..
            } => Some(*rect),
            _ => None,
        })
        .expect("desktop icon");
    let caption = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Documents" => Some(*rect),
            _ => None,
        })
        .expect("desktop caption");
    assert!(
        caption
            .origin
            .y
            .abs_diff(icon.origin.y + i32::try_from(icon.size.height.saturating_sub(6)).unwrap())
            <= 1,
        "caption must consume the classic raster's transparent tail instead of adding a modern tile gap: icon={icon:?}, caption={caption:?}",
    );
}

#[test]
fn redmond_calculator_uses_classic_red_and_blue_key_legends_only_inside_its_keypad() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                Some("panel:redmond98-calculator-keypad"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED,
                Some("panel:equal"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::DESTRUCTIVE,
                Some("CE"),
                None,
            ),
            create(
                4,
                Some(2),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("7"),
                None,
            ),
        ],
    ))
    .expect("classic calculator fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(260, 120))
        .expect("classic calculator render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "CE" && *color == Color::opaque(255, 0, 0)
    )));
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "7" && *color == Color::opaque(0, 0, 255)
    )));
}

#[test]
fn redmond_unknown_directory_draws_a_classic_plus_disclosure() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tree,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::TreeItem,
                WidgetState::ENABLED | WidgetState::INDETERMINATE,
                Some("Folder"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("Redmond potential-directory fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(240, 80))
        .expect("Redmond tree render");
    let disclosure = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Fill { rect, color, .. }
                if rect.size
                    == (Size {
                        width: 9,
                        height: 9,
                    })
                    && *color == Color::opaque(255, 255, 255) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("classic disclosure box");
    let black_strokes = output
        .commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                DrawCommand::Fill { rect, color, .. }
                    if *color == Color::opaque(0, 0, 0)
                        && disclosure.contains(rect.origin)
                        && (rect.size.width == 1 || rect.size.height == 1)
            )
        })
        .count();
    assert_eq!(
        black_strokes, 2,
        "collapsed unknown directory needs both plus strokes"
    );
}

#[test]
fn held_redmond_tree_parent_does_not_select_its_descendant_branch() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let selected_ink = theme.palette.selection_text;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Tree,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::TreeItem,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some("My Computer"),
                Some(IconId::Files),
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::TreeItem,
                WidgetState::ENABLED,
                Some("Downloads"),
                Some(IconId::Folder),
            ),
        ],
    ))
    .expect("Redmond nested tree fixture");
    let initial = host
        .render(app(1), window(1), fixed_constraints(240, 100))
        .expect("initial tree render");
    let parent = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .copied()
        .expect("parent tree target");
    host.pointer_event(
        app(1),
        window(1),
        PointerEventKind::Down,
        Point {
            x: parent.rect.origin.x + parent.rect.size.width.saturating_sub(2) as i32,
            y: parent.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("hold parent row");
    let held = host
        .render(app(1), window(1), fixed_constraints(240, 100))
        .expect("held tree render");
    assert!(held.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "My Computer" && *color == selected_ink
    )));
    assert!(
        held.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, color, .. }
                if text == "Downloads" && *color != selected_ink
        )),
        "holding a parent row must not paint its child as selected"
    );
}

#[test]
fn redmond_toolbar_uses_the_classic_grey_surface() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    theme.palette.surface = Color::opaque(192, 192, 192);
    theme.palette.surface_raised = Color::opaque(255, 255, 255);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::ToolBar,
                WidgetState::ENABLED,
                Some("panel:files-toolbar:redmond98"),
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::FLAT,
                Some("Undo"),
                Some(IconId::EditUndo),
            ),
        ],
    ))
    .expect("Redmond toolbar fixture");
    let output = host
        .render(app(1), window(1), fixed_constraints(400, 28))
        .expect("Redmond toolbar render");
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.origin == Point { x: 0, y: 0 }
                && rect.size.width == 400
                && *color == Color::opaque(192, 192, 192)
    )));
    let button = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("Redmond toolbar button")
        .rect;
    assert!(button.size.width > button.size.height);
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Undo"
    )));
}

#[test]
fn tabs_support_joined_horizontal_and_right_vertical_strips() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut horizontal = NativeUiHost::new(theme.clone());
    horizontal
        .apply_patch(&envelope(
            1,
            vec![
                create(
                    1,
                    None,
                    0,
                    WidgetKind::Tabs,
                    WidgetState::ENABLED,
                    Some("tabs:horizontal:integrated"),
                    None,
                ),
                create(
                    2,
                    Some(1),
                    0,
                    WidgetKind::Row,
                    WidgetState::ENABLED | WidgetState::SELECTED,
                    Some("panel:tab"),
                    None,
                ),
                create(
                    3,
                    Some(1),
                    1,
                    WidgetKind::Row,
                    WidgetState::ENABLED,
                    Some("panel:tab"),
                    None,
                ),
                create(
                    4,
                    Some(2),
                    0,
                    WidgetKind::Label,
                    WidgetState::ENABLED,
                    Some("First"),
                    None,
                ),
                create(
                    5,
                    Some(3),
                    0,
                    WidgetKind::Label,
                    WidgetState::ENABLED,
                    Some("Second"),
                    None,
                ),
            ],
        ))
        .expect("horizontal tabs");
    let horizontal_output = horizontal
        .render(app(1), window(1), fixed_constraints(320, 34))
        .expect("horizontal tabs render");
    let window_state = horizontal
        .windows
        .get(&(app(1), window(1)))
        .expect("horizontal tab window");
    let first = window_state.geometry[&widget(2)];
    let second = window_state.geometry[&widget(3)];
    assert_eq!(
        first.origin.x.saturating_add_unsigned(first.size.width),
        second.origin.x,
        "integrated tabs must share an edge instead of looking like spaced buttons"
    );
    assert!(horizontal_output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.origin.y >= first.origin.y
                && rect.origin.y < first.origin.y.saturating_add_unsigned(first.size.height)
                && *color == Color::opaque(192, 192, 192)
    )));

    let mut vertical = NativeUiHost::new(theme);
    vertical
        .apply_patch(&envelope(
            1,
            vec![
                create(
                    1,
                    None,
                    0,
                    WidgetKind::Tabs,
                    WidgetState::ENABLED,
                    Some("tabs:vertical:right:integrated"),
                    None,
                ),
                create(
                    2,
                    Some(1),
                    0,
                    WidgetKind::Row,
                    WidgetState::ENABLED | WidgetState::SELECTED,
                    Some("panel:tab"),
                    None,
                ),
                create(
                    3,
                    Some(1),
                    1,
                    WidgetKind::Row,
                    WidgetState::ENABLED,
                    Some("panel:tab"),
                    None,
                ),
                create(
                    4,
                    Some(2),
                    0,
                    WidgetKind::Label,
                    WidgetState::ENABLED,
                    Some("One"),
                    None,
                ),
                create(
                    5,
                    Some(3),
                    0,
                    WidgetKind::Label,
                    WidgetState::ENABLED,
                    Some("Two"),
                    None,
                ),
            ],
        ))
        .expect("vertical tabs");
    vertical
        .render(app(1), window(1), fixed_constraints(96, 160))
        .expect("vertical tabs render");
    let window_state = vertical
        .windows
        .get(&(app(1), window(1)))
        .expect("vertical tab window");
    let first = window_state.geometry[&widget(2)];
    let second = window_state.geometry[&widget(3)];
    assert_eq!(first.origin.x, second.origin.x);
    assert_eq!(
        first.origin.y.saturating_add_unsigned(first.size.height),
        second.origin.y,
        "vertical integrated tabs must stack without button gaps"
    );
}

#[test]
fn active_redmond_tab_overlaps_and_erases_the_page_top_edge() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Tabs,
                WidgetState::ENABLED | WidgetState::EXPANSIVE,
                Some("tabs:horizontal:integrated"),
                None,
            ),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Row,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("panel:tab"),
                None,
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Active"),
                None,
            ),
            create(
                5,
                Some(2),
                1,
                WidgetKind::Container,
                WidgetState::ENABLED | WidgetState::EXPANSIVE,
                Some("panel:tab-page:flex"),
                None,
            ),
            create(
                6,
                Some(5),
                0,
                WidgetKind::Label,
                WidgetState::ENABLED,
                Some("Real page"),
                None,
            ),
        ],
    ))
    .expect("integrated tab page");
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 160))
        .expect("integrated tab page render");
    let window_state = host
        .windows
        .get(&(app(1), window(1)))
        .expect("tab page window");
    let active = window_state.geometry[&widget(3)];
    let page = window_state.geometry[&widget(5)];
    assert_eq!(
        page.origin.y,
        active
            .origin
            .y
            .saturating_add_unsigned(active.size.height.saturating_sub(2)),
        "the real page must extend beneath the selected folder tab"
    );
    let page_top = output
        .commands
        .iter()
        .position(|command| matches!(command,
            DrawCommand::Fill { rect, .. }
                if rect.origin == page.origin && rect.size.width == page.size.width && rect.size.height == 1
        ))
        .expect("page top bevel");
    let active_seam = output
        .commands
        .iter()
        .rposition(|command| {
            matches!(command,
                DrawCommand::Fill { rect, color, .. }
                    if *color == Color::opaque(192, 192, 192)
                        && rect.origin.x == active.origin.x.saturating_add(2)
                        && rect.origin.y == page.origin.y
                        && rect.size.height == 2
            )
        })
        .expect("selected tab seam eraser");
    assert!(
        active_seam > page_top,
        "the selected tab must paint over the page line; no line may cross it"
    );
}

#[test]
fn app_and_window_teardown_drop_retained_resources() {
    let mut host = gallery_host();
    let mut second = envelope(1, Vec::new());
    second.window = window(2);
    host.apply_patch(&second).expect("second window");
    assert_eq!(host.window_count(), 2);
    assert!(host.remove_window(app(1), window(2)));
    assert_eq!(host.remove_app(app(1)), 1);
    assert_eq!(host.window_count(), 0);
    assert_eq!(host.app_count(), 0);
}

#[test]
fn enforces_per_app_window_limit() {
    let mut host = NativeUiHost::default();
    for value in 1..=MAX_WINDOWS_PER_APP {
        let mut patch = envelope(1, Vec::new());
        patch.window = window(u64::try_from(value).expect("window index fits"));
        host.apply_patch(&patch).expect("within limit");
    }
    let mut excess = envelope(1, Vec::new());
    excess.window = window(10_000);
    assert_eq!(
        host.apply_patch(&excess),
        Err(NativeUiError::ResourceLimit {
            resource: "windows per app",
            maximum: MAX_WINDOWS_PER_APP,
        })
    );
    assert_eq!(host.window_count(), MAX_WINDOWS_PER_APP);
}

#[test]
fn stack_renders_and_hits_only_its_selected_child() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Stack,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Inactive tab"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                WidgetState::ENABLED | WidgetState::SELECTED,
                Some("Active tab"),
                None,
            ),
        ],
    ))
    .expect("stack patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 120))
        .expect("selected stack render");
    assert!(!output.commands.iter().any(
        |command| matches!(command, DrawCommand::Text { text, .. } if text == "Inactive tab")
    ));
    assert!(
        output.commands.iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "Active tab")
        )
    );
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(2))
    );
    assert!(
        output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(3))
    );
}

#[test]
fn a_large_chat_timeline_with_end_scroll_has_bounded_native_layout() {
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            2,
            Some(1),
            0,
            WidgetKind::ScrollView,
            WidgetState::ENABLED,
            Some("viewport:520|4294967295/0"),
            None,
        ),
        create(
            3,
            Some(2),
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("panel:clamp:920"),
            None,
        ),
    ];
    let message = std::iter::repeat_n("a realistic Matrix message", 90)
        .collect::<Vec<_>>()
        .join(" ");
    for index in 0_u64..32 {
        let card = 10 + index * 3;
        patches.push(create(
            card,
            Some(3),
            u32::try_from(index).expect("bounded index"),
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("panel:settings-card"),
            None,
        ));
        patches.push(create(
            card + 1,
            Some(card),
            0,
            WidgetKind::Label,
            WidgetState::ENABLED | WidgetState::MUTED,
            Some("Matrix member"),
            None,
        ));
        patches.push(create(
            card + 2,
            Some(card),
            1,
            WidgetKind::Label,
            WidgetState::ENABLED,
            Some(&message),
            None,
        ));
    }
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, patches))
        .expect("bounded timeline patch");
    let output = host
        .render(app(1), window(1), fixed_constraints(960, 700))
        .expect("bounded timeline render");
    assert!(output.commands.len() < 512);
    let geometry = super::layout::scroll_view_geometry(
        &host.windows[&(app(1), window(1))].tree,
        widget(2),
        &host.windows[&(app(1), window(1))].geometry,
        &host.theme,
    )
    .expect("timeline scrollbar");
    assert_eq!(
        geometry.position,
        geometry
            .content_extent
            .saturating_sub(geometry.viewport_extent),
    );
}

#[test]
fn continuous_slider_emits_while_pointer_is_still_down() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![create(
            1,
            None,
            0,
            WidgetKind::Slider,
            WidgetState::ENABLED,
            Some("20/80;continuous"),
            None,
        )],
    ))
    .unwrap();
    let output = host
        .render(app(1), window(1), fixed_constraints(320, 64))
        .unwrap();
    let rect = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(1))
        .unwrap()
        .rect;
    let point = Point {
        x: rect.origin.x + rect.size.width as i32 / 2,
        y: rect.origin.y + rect.size.height as i32 / 2,
    };
    let down = host
        .pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0)
        .unwrap()
        .unwrap();
    assert!(matches!(down.event, UiEvent::ValueChanged(_)));
    let moved = host
        .pointer_event(
            app(1),
            window(1),
            PointerEventKind::Move,
            Point {
                x: point.x + 20,
                ..point
            },
            1,
            0,
        )
        .unwrap()
        .unwrap();
    assert!(matches!(moved.event, UiEvent::ValueChanged(_)));
    assert_ne!(down.event, moved.event);
}

#[path = "../../tests/native_ui/tab_folders.rs"]
mod tab_folders;

#[test]
fn store_tiles_reflow_when_the_same_window_is_resized() {
    for rows in [6, 10] {
        let mut host = NativeUiHost::default();
        let grid = format!("app-grid:{rows}:store");
        let mut patches = vec![create(
            1,
            None,
            0,
            WidgetKind::IconGrid,
            WidgetState::ENABLED,
            Some(&grid),
            None,
        )];
        for index in 0..12 {
            patches.push(create(
                10 + index,
                Some(1),
                u32::try_from(index).unwrap(),
                WidgetKind::Button,
                WidgetState::ENABLED,
                Some("Application"),
                None,
            ));
        }
        host.apply_patch(&envelope(1, patches)).unwrap();
        let mut previous_columns = 0;
        for width in [480, 720, 1024, 1440] {
            let scene = host
                .render(app(1), window(1), fixed_constraints(width, 900))
                .unwrap();
            let first = scene
                .hit_targets
                .iter()
                .find(|target| target.widget == widget(10))
                .unwrap()
                .rect;
            let row = scene
                .hit_targets
                .iter()
                .filter(|target| target.widget.get() >= 10 && target.rect.origin.y == first.origin.y)
                .collect::<Vec<_>>();
            assert!(
                row.len() > previous_columns,
                "widening the window must expose more columns"
            );
            previous_columns = row.len();
            for pair in row.windows(2) {
                assert!(
                    pair[0].rect.origin.x + pair[0].rect.size.width as i32 <= pair[1].rect.origin.x
                );
            }
            let last = row.last().unwrap().rect;
            assert!(last.origin.x + last.size.width as i32 <= width as i32);
            assert_eq!(
                first.size.height,
                u32::from(host.theme().metrics.control_height) * rows
            );
        }
    }
}

#[test]
fn store_document_grid_exposes_its_full_scroll_extent() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::ScrollView,
            WidgetState::ENABLED,
            Some("0/0"),
            None,
        ),
        create(
            2,
            Some(1),
            0,
            WidgetKind::Column,
            WidgetState::ENABLED,
            Some("panel:store-document"),
            None,
        ),
        create(
            3,
            Some(2),
            0,
            WidgetKind::Label,
            WidgetState::ENABLED,
            Some("Explore the catalog"),
            None,
        ),
        create(
            4,
            Some(2),
            1,
            WidgetKind::IconGrid,
            WidgetState::ENABLED,
            Some("app-grid:6:store"),
            None,
        ),
    ];
    for index in 0_u32..36 {
        patches.push(create(
            u64::from(10 + index),
            Some(4),
            index,
            WidgetKind::Button,
            WidgetState::ENABLED,
            Some("Application"),
            None,
        ));
    }
    host.apply_patch(&envelope(1, patches)).unwrap();
    let output = host
        .render(app(1), window(1), fixed_constraints(960, 640))
        .unwrap();
    let scroll = super::layout::scroll_view_geometry(
        &host.windows[&(app(1), window(1))].tree,
        widget(1),
        &host.windows[&(app(1), window(1))].geometry,
        &host.theme,
    )
    .expect("store catalog must be scrollable");
    assert!(scroll.content_extent > scroll.viewport_extent);
    assert!(
        scroll.content_extent < 3_000,
        "store grid extent includes a blank preferred-size tail: {}",
        scroll.content_extent,
    );

    let first = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(10))
        .expect("first store card");
    let event = host
        .pointer_event(
            app(1),
            window(1),
            PointerEventKind::Wheel,
            Point {
                x: first.rect.origin.x + 2,
                y: first.rect.origin.y + 2,
            },
            5,
            0,
        )
        .unwrap()
        .expect("store wheel event");
    assert_eq!(event.target, widget(1));
    assert!(matches!(event.event, UiEvent::ValueChanged(ref value) if value.starts_with("96/")));
}

#[test]
fn ordinary_and_accent_buttons_change_fill_on_hover_without_changing_geometry() {
    for state in [WidgetState::ENABLED, WidgetState::ENABLED | WidgetState::ACCENT] {
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::Column,WidgetState::ENABLED,None,None),create(2,Some(1),0,WidgetKind::Button,state,Some("Apply theme"),None)])).unwrap();
        let constraints=fixed_constraints(320,180);
        let initial=host.render(app(1),window(1),constraints).unwrap();
        let rect=initial.hit_targets.iter().find(|target|target.widget==widget(2)).unwrap().rect;
        host.pointer_event(app(1),window(1),PointerEventKind::Move,Point{x:rect.origin.x+4,y:rect.origin.y+4},0,0).unwrap();
        let hovered=host.render(app(1),window(1),constraints).unwrap();
        let fills=|commands:&[DrawCommand]| commands.iter().filter_map(|command|match command {DrawCommand::Fill{rect:r,color,..} if *r==rect=>Some(*color),_=>None}).collect::<Vec<_>>();
        assert_ne!(fills(&initial.commands),fills(&hovered.commands));
        assert_eq!(hovered.hit_targets.iter().find(|target|target.widget==widget(2)).unwrap().rect,rect);
    }
}

#[test]
fn close_button_does_not_stretch_to_multiline_heading_height() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::Row,WidgetState::ENABLED,Some("split:720"),None),create(2,Some(1),0,WidgetKind::Label,WidgetState::ENABLED,Some("Window decoration preferences across several lines"),None),create(3,Some(1),1,WidgetKind::Button,WidgetState::ENABLED,Some("Close"),None)])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(420,160)).unwrap();
    let button=output.hit_targets.iter().find(|target|target.widget==widget(3)).unwrap();
    assert!(button.rect.size.height<=40,"action must retain its normal control height: {:?}",button.rect);
}

#[test]
fn empty_status_footers_release_space_and_return_when_updated() {
    for panel in [false, true] {
        let mut host = NativeUiHost::default();
        let mut patches = vec![
            create(1,None,0,WidgetKind::Column,WidgetState::ENABLED,None,None),
            create(2,Some(1),0,WidgetKind::TextEditor,WidgetState::ENABLED,Some("Content"),None),
            create(3,Some(1),1,if panel { WidgetKind::Row } else { WidgetKind::Label },WidgetState::ENABLED,Some(if panel { "panel:status-bar" } else { "" }),None),
        ];
        if panel { patches.push(create(4,Some(3),0,WidgetKind::Label,WidgetState::ENABLED,Some(""),None)); }
        host.apply_patch(&envelope(1, patches)).unwrap();
        let content_height = |host: &mut NativeUiHost| {
            host.render(app(1),window(1),fixed_constraints(500,300)).unwrap()
                .hit_targets.iter().find(|target| target.widget == widget(2)).unwrap().rect.size.height
        };
        let empty = content_height(&mut host);
        for (seq, text) in [(2,"Saving changes…"),(3,"  \n "),(4,"Saved")] {
            host.apply_patch(&envelope(seq,vec![UiPatch::Update {
                id: widget(if panel {4} else {3}), state: PatchValue::Keep,
                text: PatchValue::Set(text.into()), icon: PatchValue::Keep,
            }])).unwrap();
            let height = content_height(&mut host);
            if text.trim().is_empty() { assert_eq!(height, empty); }
            else { assert!(height < empty, "message must restore status space"); }
        }
    }
}

#[test]
fn settings_copy_is_compact_and_controls_stack_only_in_narrow_forms() {
    for width in [360, 680] {
        let mut host=NativeUiHost::default();
        host.apply_patch(&envelope(1,vec![
            create(1,None,0,WidgetKind::Row,WidgetState::ENABLED,Some("split:620:compact:form-field"),None),
            create(2,Some(1),0,WidgetKind::Column,WidgetState::ENABLED,Some("panel:tight-stack"),None),
            create(3,Some(2),0,WidgetKind::Label,WidgetState::ENABLED,Some("Window shape"),None),
            create(4,Some(2),1,WidgetKind::Label,WidgetState::ENABLED,Some("Choose the shape of window corners."),None),
            create(5,Some(1),1,WidgetKind::Button,WidgetState::ENABLED,Some("Theme Default"),None),
        ])).unwrap();
        let output=host.render(app(1),window(1),fixed_constraints(width,160)).unwrap();
        let text_rect=|text:&str| output.commands.iter().find_map(|command| match command {DrawCommand::Text{rect,text:value,..} if value==text=>Some(*rect),_=>None}).unwrap();
        let title=text_rect("Window shape");
        let description=text_rect("Choose the shape of window corners.");
        let control=output.hit_targets.iter().find(|target|target.widget==widget(5)).unwrap().rect;
        assert!(description.origin.y-title.origin.y<=28);
        if width<460 {assert!(control.origin.y>=description.origin.y+description.size.height as i32);}
        else {assert!(control.origin.x>=title.origin.x+title.size.width as i32);}
    }
}

#[test]
fn settings_menu_with_choices_keeps_control_height_beside_multiline_help() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::Row,WidgetState::ENABLED,Some("split:620:compact:form-field"),None),
        create(2,Some(1),0,WidgetKind::Column,WidgetState::ENABLED,Some("panel:tight-stack"),None),
        create(3,Some(2),0,WidgetKind::Label,WidgetState::ENABLED,Some("Environment"),None),
        create(4,Some(2),1,WidgetKind::Label,WidgetState::ENABLED,Some("Selecting an environment also applies its Rice appearance, workspace names and tiling preferences. Hardware settings stay on this desktop."),None),
        create(5,Some(1),1,WidgetKind::Menu,WidgetState::ENABLED,Some("My environment"),None),
        create(6,Some(5),0,WidgetKind::MenuItem,WidgetState::ENABLED,Some("My environment"),None),
    ])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(620,160)).unwrap();
    let control=output.hit_targets.iter().find(|target|target.widget==widget(5)).unwrap().rect;
    assert!(control.size.height<=40,"dropdown must retain control height despite its menu items: {control:?}");
}

#[test]
fn action_row_menu_and_buttons_share_chrome_and_height() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::Row,WidgetState::ENABLED,Some("panel:equal"),None),
        create(2,Some(1),0,WidgetKind::Button,WidgetState::ENABLED,Some("Settings"),Some(IconId::Settings)),
        create(3,Some(1),1,WidgetKind::Button,WidgetState::ENABLED,Some("Files"),Some(IconId::Folder)),
        create(4,Some(1),2,WidgetKind::Menu,WidgetState::ENABLED,Some("Session"),Some(IconId::Logout)),
        create(5,Some(4),0,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Log out"),None),
    ])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(420,44)).unwrap();
    let rects:Vec<_>=(2..=4).map(|id|output.hit_targets.iter().find(|target|target.widget==widget(id)).unwrap().rect).collect();
    assert_eq!(rects[0].size.height,rects[2].size.height);
    let surfaces:Vec<Vec<_>>=rects.iter().map(|target|output.commands.iter().filter_map(|command|match command {
        DrawCommand::Fill{rect,color,radius} if rect==target=>Some(format!("fill:{color:?}:{radius}")),
        DrawCommand::Border{rect,color,width,radius} if rect==target=>Some(format!("border:{color:?}:{width}:{radius}")),
        _=>None,
    }).collect()).collect();
    assert!(!surfaces[0].is_empty());assert_eq!(surfaces[0],surfaces[1]);assert_eq!(surfaces[0],surfaces[2]);
}

#[test]
fn settings_readonly_value_reserves_height_for_its_actual_split_width() {
    let status = "Software rendering — no compatible Vulkan GPU detected";
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
        create(2, Some(1), 0, WidgetKind::Row, WidgetState::ENABLED, Some("split:620:compact:form-field"), None),
        create(3, Some(2), 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:tight-stack"), None),
        create(4, Some(3), 0, WidgetKind::Label, WidgetState::ENABLED, Some("Renderer for new applications"), None),
        create(5, Some(2), 1, WidgetKind::Label, WidgetState::ENABLED, Some(status), None),
    ])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(680, 300)).unwrap();
    let rect = output.commands.iter().find_map(|command| match command {
        DrawCommand::Text { rect, text, .. } if text == status => Some(rect),
        _ => None,
    }).expect("complete renderer status");
    assert!(rect.size.height >= 48, "three wrapped lines need their full height: {rect:?}");
}

#[test]
fn toolbar_tooltips_do_not_inflate_icon_controls_in_classic_or_modern_themes() {
    for redmond in [false, true] {
        let mut theme = Theme::familiar_light();
        if redmond { theme.chrome_profile = rice_ui::ChromeProfile::Redmond98; }
        let measure = |tooltip| {
            let mut host = NativeUiHost::new(theme.clone());
            let mut patches = vec![
                create(1, None, 0, WidgetKind::ToolBar, WidgetState::ENABLED, None, None),
                create(2, Some(1), 0, WidgetKind::Button, WidgetState::ENABLED, None, Some(IconId::TextEditor)),
            ];
            if tooltip { patches.push(create(3, Some(2), 0, WidgetKind::Tooltip, WidgetState::ENABLED, Some("Edit theme code in RicePaper"), None)); }
            host.apply_patch(&envelope(1, patches)).unwrap();
            host.render(app(1), window(1), fixed_constraints(240, 48)).unwrap()
                .hit_targets.into_iter().find(|target| target.widget == widget(2)).unwrap().rect
        };
        let plain = measure(false);
        let titled = measure(true);
        assert_eq!(plain, titled);
        assert_eq!(titled.size.width, titled.size.height);
        assert!(titled.size.height <= 36);
    }
}

#[test]
fn window_editor_uses_owner_titlebar_without_a_second_classic_caption() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
        create(2, Some(1), 0, WidgetKind::Dialog, WidgetState::ENABLED | WidgetState::EXPANSIVE, Some("panel:window-editor:title=Theme Studio"), None),
        create(3, Some(2), 0, WidgetKind::Button, WidgetState::ENABLED, Some("Apply"), None),
    ])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(800, 600)).unwrap();
    assert!(!output.hit_targets.iter().any(|target| target.widget == widget(2)));
    assert!(!output.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "×")));
    let rect = output.dialog.unwrap().rect;
    assert_eq!(rect.origin, Point { x: 0, y: 0 });
    assert_eq!(rect.size, Size { width: 800, height: 600 });
}

#[test]
fn signal_slider_keeps_full_pointer_target_with_narrow_instrument_thumb() {
    let mut theme = Theme::ricewm_dark();
    theme.signal_controls = true;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(1, vec![create(1, None, 0, WidgetKind::Slider, WidgetState::ENABLED, Some("20/80;continuous"), None)])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(320, 64)).unwrap();
    assert!(output.commands.iter().any(|c| matches!(c, DrawCommand::Fill { rect, radius: 0, color } if rect.size.width == 4 && rect.size.height == 14 && *color == theme.palette.text)));
    let rect = output.hit_targets.iter().find(|t| t.widget == widget(1)).unwrap().rect;
    assert!(rect.size.height >= 20);
    let event = host.pointer_event(app(1), window(1), PointerEventKind::Down, Point { x: rect.origin.x + rect.size.width as i32 / 2, y: rect.origin.y + rect.size.height as i32 / 2 }, 1, 0).unwrap().unwrap();
    assert!(matches!(event.event, UiEvent::ValueChanged(_)));
}
#[test]
fn mail_document_height_preserves_long_content() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::Column,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::WebSurface,
                WidgetState::ENABLED,
                Some("panel:document-height:1900"),
                None,
            ),
        ],
    ))
    .expect("fixed web surface patch");

    let output = host
        .render(app(1), window(1), fixed_constraints(480, 2400))
        .expect("fixed web surface render");
    let surface = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("web surface hit target");
    assert_eq!(surface.rect.size.height, 1900);
}

#[test]
fn redmond_selected_tree_icon_and_tooltip_keep_surface_contrast() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme.clone());
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
        create(2, Some(1), 0, WidgetKind::Tree, WidgetState::ENABLED, None, None),
        create(3, Some(2), 0, WidgetKind::TreeItem, WidgetState::ENABLED | WidgetState::SELECTED, Some("Inbox"), Some(IconId::Folder)),
        create(4, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED | WidgetState::SELECTED, Some("Message"), None),
        create(5, Some(4), 0, WidgetKind::Tooltip, WidgetState::ENABLED, Some("The complete selected message subject"), None),
    ])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(400, 300)).unwrap();
    assert!(output.commands.iter().any(|command| matches!(command,
        DrawCommand::Icon { icon: IconId::Folder, color, .. } if *color == theme.palette.text)));
    let anchor = output.hit_targets.iter().find(|target| target.widget == widget(4)).unwrap().rect;
    host.pointer_event(app(1), window(1), PointerEventKind::Move, anchor.origin, 0, 0).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(400, 300)).unwrap();
    assert!(output.commands.iter().any(|command| matches!(command,
        DrawCommand::Text { text, color, .. } if text == "The complete selected message subject" && *color == theme.palette.text)));
}

#[test]
fn every_conversation_document_wheels_the_shared_scroll_view() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::ScrollView, WidgetState::ENABLED, Some("0/0"), None),
        create(2, Some(1), 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:intrinsic-flow"), None),
        create(3, Some(2), 0, WidgetKind::WebSurface, WidgetState::ENABLED, Some("panel:document-height:700"), None),
        create(4, Some(2), 1, WidgetKind::WebSurface, WidgetState::ENABLED, Some("panel:document-height:800"), None),
    ])).unwrap();
    for (sequence, position, surface) in [(2, "0/0", 3), (3, "750/0", 4), (4, "100000/0", 4)] {
        host.apply_patch(&envelope(sequence, vec![UiPatch::Update {
            id: widget(1), state: PatchValue::Keep, text: PatchValue::Set(position.into()), icon: PatchValue::Keep,
        }])).unwrap();
        let output = host.render(app(1), window(1), fixed_constraints(480, 400)).unwrap();
        let target = output.hit_targets.iter().find(|target| target.widget == widget(surface)).unwrap().rect;
        let point = Point { x: target.origin.x + 20, y: target.origin.y.max(0) + 30 };
        let event = host.pointer_event(app(1), window(1), PointerEventKind::Wheel, point, 5, 0).unwrap().unwrap();
        assert_eq!(event.target, widget(1), "Every document delegates to the enclosing scrollbar");
        assert!(matches!(event.event, UiEvent::ValueChanged(_)));
        let event = host.pointer_event(app(1), window(1), PointerEventKind::Wheel, point, 4, 0).unwrap().unwrap();
        assert_eq!(event.target, widget(1), "Upward wheel remains bounded after reaching the end");
    }
}

#[test]
fn conversation_scroll_preserves_each_message_height() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::ScrollView,WidgetState::ENABLED,Some("0/0"),None),
        create(2,Some(1),0,WidgetKind::Column,WidgetState::ENABLED,Some("panel:settings-card:intrinsic-flow"),None),
        create(3,Some(2),0,WidgetKind::Label,WidgetState::ENABLED,Some("Sender and a subject that can wrap onto another line at narrow window widths"),None),
        create(4,Some(2),1,WidgetKind::WebSurface,WidgetState::ENABLED,Some("panel:document-height:1900"),None),
        create(5,Some(2),2,WidgetKind::Column,WidgetState::ENABLED,Some("panel:settings-card:tight-stack:intrinsic-flow"),None),
        create(6,Some(5),0,WidgetKind::Label,WidgetState::ENABLED,Some("Earlier message from another sender"),None),
        create(7,Some(5),1,WidgetKind::WebSurface,WidgetState::ENABLED,Some("panel:document-height:140"),None),
    ])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(480,400)).unwrap();
    let height=output.commands.iter().find_map(|command| match command { DrawCommand::WebScene{surface,rect,..} if *surface == widget(4) => Some(rect.size.height), _ => None }).unwrap();
    assert_eq!(height,1900,"A scroll viewport must not shrink the message to fit");
}

#[test]
fn tall_menu_bar_dropdown_stays_below_button_and_scrolls() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(
            1,
            None,
            0,
            WidgetKind::MenuBar,
            WidgetState::ENABLED,
            None,
            None,
        ),
        create(
            2,
            Some(1),
            0,
            WidgetKind::Menu,
            WidgetState::ENABLED,
            Some("View"),
            None,
        ),
    ];
    for index in 0..18_u64 {
        patches.push(create(
            10 + index,
            Some(2),
            index as u32,
            WidgetKind::MenuItem,
            WidgetState::ENABLED,
            Some("Command"),
            None,
        ));
    }
    host.apply_patch(&envelope(1, patches)).unwrap();
    let theme = Theme::familiar_light();
    let window = host.windows.get_mut(&(app(1), window(1))).unwrap();
    let anchor = Rect {
        origin: Point { x: 120, y: 10 },
        size: Size {
            width: 70,
            height: 40,
        },
    };
    window.geometry.insert(widget(2), anchor);
    super::layout::layout_menu_popup(
        &window.tree,
        widget(2),
        Size {
            width: 600,
            height: 300,
        },
        None,
        0,
        &theme,
        &mut window.geometry,
    );
    let popup =
        super::render::menu_popup_rect(&window.tree, widget(2), &window.geometry, &theme).unwrap();
    assert!(
        popup.origin.y >= 50,
        "dropdown must stay below its menu button"
    );
    assert!(
        popup.origin.y + popup.size.height as i32 <= 300,
        "tall dropdown must fit the viewport"
    );
    assert!(!window.geometry.contains_key(&widget(27)));
    super::layout::layout_menu_popup(
        &window.tree,
        widget(2),
        Size {
            width: 600,
            height: 300,
        },
        None,
        17,
        &theme,
        &mut window.geometry,
    );
    assert!(
        window.geometry.contains_key(&widget(27)),
        "scrolling must expose the last command"
    );
}

#[test]
fn shifted_arrows_reach_the_focused_file_without_spatial_navigation() {
    let mut host = NativeUiHost::default();
    let enabled = WidgetState::ENABLED | WidgetState::KEY_NAVIGATION;
    host.apply_patch(&envelope(
        1,
        vec![
            create(
                1,
                None,
                0,
                WidgetKind::IconGrid,
                WidgetState::ENABLED,
                None,
                None,
            ),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Button,
                enabled,
                Some("First"),
                None,
            ),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Button,
                enabled,
                Some("Second"),
                None,
            ),
        ],
    ))
    .unwrap();
    host.render(app(1), window(1), fixed_constraints(600, 300))
        .unwrap();
    host.windows.get_mut(&(app(1), window(1))).unwrap().focused = Some(widget(2));
    let shifted = host
        .key_event(app(1), window(1), true, 114, 1, None)
        .unwrap()
        .unwrap();
    assert_eq!(shifted.target, widget(2));
    assert_eq!(
        host.windows.get(&(app(1), window(1))).unwrap().focused,
        Some(widget(2))
    );
    let plain = host
        .key_event(app(1), window(1), true, 114, 0, None)
        .unwrap()
        .unwrap();
    assert_eq!(plain.target, widget(3));
}

#[test]
fn file_icon_arrows_follow_grid_cells_with_mixed_thumbnails_and_icons() {
    // Icons only, thumbnails only, and mixed cells must share row/column semantics.
    for presentation in [0, 1, 2] {
        for width in [520, 900, 1280] {
            for zoom in [800, 1000, 1500] {
                let mut host = NativeUiHost::default();
                let enabled = WidgetState::ENABLED;
                let navigation = enabled | WidgetState::KEY_NAVIGATION;
                let mut patches = vec![create(
                    1,
                    None,
                    0,
                    WidgetKind::IconGrid,
                    enabled,
                    Some(&format!("file-grid:{zoom}")),
                    None,
                )];
                let mut tiles = Vec::new();
                for index in 0..19_u64 {
                    let target = 100 + index;
                    if presentation == 0 || presentation == 2 && index % 3 == 0 {
                        tiles.push(widget(target));
                        patches.push(create(
                            target,
                            Some(1),
                            index as u32,
                            WidgetKind::Button,
                            navigation,
                            Some("Folder"),
                            Some(IconId::Folder),
                        ));
                    } else {
                        let cell = 1000 + index;
                        tiles.push(widget(cell));
                        patches.push(create(
                            cell,
                            Some(1),
                            index as u32,
                            WidgetKind::Container,
                            enabled,
                            Some("panel:thumbnail-cell"),
                            None,
                        ));
                        patches.push(create(
                            2000 + index,
                            Some(cell),
                            0,
                            WidgetKind::Image,
                            enabled,
                            if index % 2 == 0 {
                                Some("media-request:123")
                            } else {
                                None
                            },
                            if index % 2 == 0 {
                                None
                            } else {
                                Some(IconId::Files)
                            },
                        ));
                        patches.push(create(
                            target,
                            Some(cell),
                            1,
                            WidgetKind::Button,
                            navigation | WidgetState::FLAT,
                            Some("A movie with a long filename.mp4"),
                            None,
                        ));
                    }
                }
                host.apply_patch(&envelope(1, patches)).unwrap();
                host.render(app(1), window(1), fixed_constraints(width, 500))
                    .unwrap();
                let geometry = &host.windows[&(app(1), window(1))].geometry;
                let columns = tiles
                    .iter()
                    .take_while(|tile| geometry[tile].origin.y == geometry[&tiles[0]].origin.y)
                    .count();
                assert!(columns >= 2);
                for index in 0..tiles.len() {
                    for (key, expected) in [
                        (111, index.checked_sub(columns)),
                        (
                            116,
                            (index / columns < (tiles.len() - 1) / columns)
                                .then_some((index + columns).min(tiles.len() - 1)),
                        ),
                        (113, (index % columns > 0).then(|| index - 1)),
                        (
                            114,
                            (index % columns + 1 < columns && index + 1 < tiles.len())
                                .then_some(index + 1),
                        ),
                    ] {
                        host.windows.get_mut(&(app(1), window(1))).unwrap().focused =
                            Some(widget(100 + index as u64));
                        let event = host
                            .key_event(app(1), window(1), true, key, 0, None)
                            .unwrap()
                            .unwrap();
                        assert_eq!(event.target, widget(100 + expected.unwrap_or(index) as u64),
                        "presentation {presentation}, key {key}, item {index}, {columns} columns at width {width}, zoom {zoom}");
                    }
                }
            }
        }
    }
}

#[test]
fn clipped_label_tooltip_adds_full_text_without_repeating_a_fitting_label() {
    for kind in [WidgetKind::Label, WidgetKind::Button] {
    for (text, expected) in [("Short", false), ("Desktop: a very long comparison description that cannot fit into this narrow field", true)] {
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(1, vec![
            create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
            create(2, Some(1), 0, kind, WidgetState::ENABLED, Some(text), None),
            create(3, Some(2), 0, WidgetKind::Tooltip, WidgetState::ENABLED, Some(text), None),
        ])).unwrap();
        let constraints=fixed_constraints(180, 160);
        let initial=host.render(app(1),window(1),constraints).unwrap();
        let anchor=initial.hit_targets.iter().find(|target|target.widget==widget(2)).unwrap().rect;
        host.pointer_event(app(1),window(1),PointerEventKind::Move,anchor.origin,0,0).unwrap();
        let hovered=host.render(app(1),window(1),constraints).unwrap();
        assert_eq!(hovered.commands.iter().any(|command|matches!(command,DrawCommand::BalloonFrame {..})),expected);
    }
}
}


#[test]
fn rtl_split_geometry_preserves_panes_and_uses_logical_drag_ratios() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::SplitPane, WidgetState::ENABLED, Some("split:300:flush"), None),
        create(2, Some(1), 0, WidgetKind::Button, WidgetState::ENABLED, Some("First"), None),
        create(3, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Second"), None),
    ])).unwrap();
    let mut ltr_rects = None;
    for locale in ["en", "ar", "fa", "ur", "en"] {
        rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
            let output = host.render(app(1), window(1), fixed_constraints(1000, 300)).unwrap();
            let geometry = &host.windows[&(app(1), window(1))].geometry;
            let first = geometry[&widget(2)];
            let second = geometry[&widget(3)];
            let rtl = locale != "en";
            assert_eq!(first.origin.x > second.origin.x, rtl);
            assert_eq!(host.windows[&(app(1), window(1))].tree.get(widget(1)).unwrap().children(), &[widget(2), widget(3)]);
            if let Some((a, b)) = ltr_rects {
                let (a, b): (Rect, Rect) = (a, b);
                assert_eq!(first.size, a.size);
                assert_eq!(second.size, b.size);
                assert_eq!(first.origin.x, if rtl { 1000 - a.origin.x - a.size.width as i32 } else { a.origin.x });
                assert_eq!(second.origin.x, if rtl { 1000 - b.origin.x - b.size.width as i32 } else { b.origin.x });
            } else {
                ltr_rects = Some((first, second));
            }
            for (id, rect) in [(2, first), (3, second)] {
                let point = Point { x: rect.origin.x + rect.size.width as i32 / 2, y: 100 };
                assert_eq!(host.hit_test(app(1), window(1), point).unwrap().widget, widget(id));
            }
            let divider = output.hit_targets.iter().find(|target| target.widget == widget(1)).unwrap().rect;
            assert_eq!(divider.size.width, 10);
            let (left, right) = if rtl { (second, first) } else { (first, second) };
            let midpoint = (left.origin.x + left.size.width as i32 + right.origin.x) / 2;
            assert_eq!(divider.origin.x + 5, midpoint);
            let down = host.pointer_event(app(1), window(1), PointerEventKind::Down,
                Point { x: midpoint, y: 100 }, 1, 0).unwrap().unwrap();
            assert_eq!(down.target, widget(1));
            assert!(matches!(down.event, UiEvent::Pointer { x: 290..=310, .. }));
            let physical_x = if rtl { 200 } else { 800 };
            for kind in [PointerEventKind::Move, PointerEventKind::Up] {
                let event = host.pointer_event(app(1), window(1), kind,
                    Point { x: physical_x, y: 100 }, 1, 0).unwrap().unwrap();
                assert_eq!(event.target, widget(1));
                assert!(matches!(event.event, UiEvent::Pointer { x: 800, .. }));
            }
        });
    }
    host.apply_patch(&envelope(2, vec![UiPatch::Update {
        id: widget(1), state: PatchValue::Keep,
        text: PatchValue::Set("split:800:flush".into()), icon: PatchValue::Keep,
    }])).unwrap();
    rice_i18n::with_locale(rice_i18n::locale_code("ar"), || {
        let output = host.render(app(1), window(1), fixed_constraints(1000, 300)).unwrap();
        let geometry = &host.windows[&(app(1), window(1))].geometry;
        assert!(geometry[&widget(2)].size.width > geometry[&widget(3)].size.width);
        let divider = output.hit_targets.iter().find(|target| target.widget == widget(1)).unwrap().rect;
        assert!((190..=210).contains(&(divider.origin.x + 5)));
    });
}

#[test]
fn rtl_split_scope_leaves_rows_vertical_compact_and_custom_files_unchanged() {
    for (kind, token, width) in [
        (WidgetKind::Row, "panel:distribute", 1000),
        (WidgetKind::SplitPane, "split:300:vertical", 1000),
        (WidgetKind::SplitPane, "split:300", 320),
        (WidgetKind::SplitPane, "split:300:files-shell:simplified", 1000),
    ] {
        let mut host = NativeUiHost::default();
        host.apply_patch(&envelope(1, vec![
            create(1, None, 0, kind, WidgetState::ENABLED, Some(token), None),
            create(2, Some(1), 0, WidgetKind::Button, WidgetState::ENABLED, Some("First"), None),
            create(3, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Second"), None),
        ])).unwrap();
        let mut ltr = None;
        for locale in ["en", "ar"] {
            rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
                host.render(app(1), window(1), fixed_constraints(width, 300)).unwrap();
                let geometry = host.windows[&(app(1), window(1))].geometry.clone();
                if let Some(expected) = &ltr { assert_eq!(&geometry, expected); }
                else { ltr = Some(geometry); }
                let window = &host.windows[&(app(1), window(1))];
                assert!(!super::render::split_is_reversed(&window.tree, window.tree.get(widget(1)).unwrap(), &window.geometry));
            });
        }
    }
}

#[test]
fn rtl_compact_navigation_reflects_cells_preserving_hit_targets_and_ids() {
    // Odd width and an odd item count exercise remainder pixels and the final
    // unpaired item; a wrapper heading must retain its original placement.
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(1, None, 0, WidgetKind::SplitPane, WidgetState::ENABLED, Some("split:300:flush"), None),
        create(2, Some(1), 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:flush"), None),
        create(3, Some(2), 0, WidgetKind::Label, WidgetState::ENABLED, Some("Settings"), None),
        create(4, Some(2), 1, WidgetKind::List, WidgetState::ENABLED, None, None),
        create(5, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Content"), None),
    ];
    for index in 0..5 {
        patches.push(create(10 + u64::from(index), Some(4), index, WidgetKind::Button,
            WidgetState::ENABLED, Some("Navigation"), None));
    }
    host.apply_patch(&envelope(1, patches)).unwrap();
    let mut baseline: Option<BTreeMap<WidgetId, Rect>> = None;
    for locale in ["en", "ar", "fa", "ur", "en"] {
        rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
            host.render(app(1), window(1), fixed_constraints(351, 700)).unwrap();
            let geometry = host.windows[&(app(1), window(1))].geometry.clone();
            let list = geometry[&widget(4)];
            let tree = &host.windows[&(app(1), window(1))].tree;
            assert_eq!(tree.get(widget(4)).unwrap().children(), &[widget(10), widget(11), widget(12), widget(13), widget(14)]);
            assert_eq!(geometry[&widget(10)].origin.x > geometry[&widget(11)].origin.x, locale != "en");
            if let Some(ltr) = &baseline {
                for id in [1, 2, 3, 4, 5] { assert_eq!(geometry[&widget(id)], ltr[&widget(id)]); }
                for id in 10..15 {
                    let rect = geometry[&widget(id)]; let original = ltr[&widget(id)];
                    assert_eq!(rect.size, original.size);
                    assert_eq!(rect.origin.y, original.origin.y);
                    assert_eq!(rect.origin.x, if locale == "en" { original.origin.x } else {
                        2 * list.origin.x + list.size.width as i32 - original.origin.x - original.size.width as i32
                    });
                }
            } else { baseline = Some(geometry.clone()); }
            for id in 10..15 {
                let rect = geometry[&widget(id)];
                let point = Point { x: rect.origin.x + rect.size.width as i32 / 2,
                    y: rect.origin.y + rect.size.height as i32 / 2 };
                assert_eq!(host.hit_test(app(1), window(1), point).unwrap().widget, widget(id));
            }
        });
    }
}

#[test]
fn rtl_orchard_files_regions_preserve_internals_and_split_drag_direction() {
    let mut theme = Theme::ricewm_default();
    theme.chrome_profile = rice_ui::ChromeProfile::Orchard;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:files-shell:simplified:flush"), None),
        create(2, Some(1), 0, WidgetKind::Button, WidgetState::ENABLED, Some("Location"), None),
        create(3, Some(1), 1, WidgetKind::SplitPane, WidgetState::ENABLED, Some("split:250:files-content:flush"), None),
        create(4, Some(3), 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:sidebar:flush"), None),
        create(5, Some(4), 0, WidgetKind::Button, WidgetState::ENABLED, Some("Home"), None),
        create(6, Some(3), 1, WidgetKind::Row, WidgetState::ENABLED, Some("panel:distribute:flush"), None),
        create(7, Some(6), 0, WidgetKind::Button, WidgetState::ENABLED, Some("File A"), None),
        create(8, Some(6), 1, WidgetKind::Button, WidgetState::ENABLED, Some("File B"), None),
        create(9, Some(1), 2, WidgetKind::Container, WidgetState::ENABLED, Some("panel:files-status:flush"), None),
    ])).unwrap();
    let mut baseline: Option<BTreeMap<WidgetId, Rect>> = None;
    for locale in ["en", "ar", "fa", "ur", "en"] {
        rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
            let output = host.render(app(1), window(1), fixed_constraints(1000, 500)).unwrap();
            let geometry = host.windows[&(app(1), window(1))].geometry.clone();
            let sidebar = geometry[&widget(4)]; let main = geometry[&widget(6)];
            let split = geometry[&widget(3)]; let rtl = locale != "en";
            assert!(main.origin.y > sidebar.origin.y, "main must have a header offset");
            assert_eq!(sidebar.origin.x > main.origin.x, rtl);
            assert_eq!(host.windows[&(app(1), window(1))].tree.get(widget(3)).unwrap().children(), &[widget(4), widget(6)]);
            assert!(geometry[&widget(7)].origin.x < geometry[&widget(8)].origin.x, "file content row remains physical LTR");
            if let Some(ltr) = &baseline {
                for id in [2, 4, 6, 9] {
                    let rect = geometry[&widget(id)]; let original = ltr[&widget(id)];
                    assert_eq!(rect.size, original.size); assert_eq!(rect.origin.y, original.origin.y);
                    assert_eq!(rect.origin.x, if rtl {
                        2 * split.origin.x + split.size.width as i32 - original.origin.x - original.size.width as i32
                    } else { original.origin.x });
                }
                for (child, parent) in [(5, 4), (7, 6), (8, 6)] {
                    assert_eq!(geometry[&widget(child)].origin.x - geometry[&widget(parent)].origin.x,
                        ltr[&widget(child)].origin.x - ltr[&widget(parent)].origin.x);
                }
            } else { baseline = Some(geometry.clone()); }
            for id in [2, 5, 7, 8] {
                let rect = geometry[&widget(id)];
                let point = Point { x: rect.origin.x + rect.size.width as i32 / 2,
                    y: rect.origin.y + rect.size.height as i32 / 2 };
                assert_eq!(host.hit_test(app(1), window(1), point).unwrap().widget, widget(id));
            }
            let divider = output.hit_targets.iter().find(|hit| hit.widget == widget(3)).unwrap().rect;
            let boundary = if rtl { sidebar.origin.x } else { sidebar.origin.x + sidebar.size.width as i32 };
            assert_eq!(divider.origin.x + divider.size.width as i32 / 2, boundary);
            let y = main.origin.y + main.size.height as i32 / 2;
            let down = host.pointer_event(app(1), window(1), PointerEventKind::Down,
                Point { x: boundary, y }, 1, 0).unwrap().unwrap();
            assert_eq!(down.target, widget(3));
            assert!(matches!(down.event, UiEvent::Pointer { x: 240..=260, .. }));
            let local = if rtl { split.size.width / 5 } else { split.size.width * 4 / 5 };
            for kind in [PointerEventKind::Move, PointerEventKind::Up] {
                let event = host.pointer_event(app(1), window(1), kind,
                    Point { x: split.origin.x + local as i32, y }, 1, 0).unwrap().unwrap();
                assert_eq!(event.target, widget(3));
                assert!(matches!(event.event, UiEvent::Pointer { x: 798..=802, .. }));
            }
        });
    }
}

#[test]
fn labeled_authoring_toolbar_uses_theme_text_and_preserves_caption_width() {
    for mut theme in [Theme::familiar_light(), Theme::ricewm_dark()] {
        for classic in [false, true] {
            if classic { theme.chrome_profile = rice_ui::ChromeProfile::Redmond98; }
            let mut host = NativeUiHost::new(theme.clone());
            host.apply_patch(&envelope(1, vec![
                create(1, None, 0, WidgetKind::ToolBar, WidgetState::ENABLED, None, None),
                create(2, Some(1), 0, WidgetKind::Button, WidgetState::ENABLED | WidgetState::START_ALIGNED,
                    Some("Text box"), Some(IconId::TextEditor)),
            ])).unwrap();
            let output = host.render(app(1), window(1), fixed_constraints(640, 48)).unwrap();
            let rect = output.hit_targets.iter().find(|target| target.widget == widget(2)).unwrap().rect;
            assert!(rect.size.width > rect.size.height, "caption must not collapse into an icon");
            let text_rect = output.commands.iter().find_map(|command| match command { DrawCommand::Text { text, rect, .. } if text == "Text box" => Some(rect), _ => None }).unwrap();
            let icon_rect = output.commands.iter().find_map(|command| match command { DrawCommand::Icon { rect, .. } => Some(rect), _ => None }).unwrap();
            assert!(text_rect.origin.x >= icon_rect.origin.x + icon_rect.size.width as i32, "toolbar caption must not overlap its icon");
        }
    }
}

#[test]
fn compact_inline_fields_keep_theme_font_and_centered_baseline_without_growing() {
    for theme in [Theme::familiar_light(),Theme::ricewm_dark()] {
        for height in [21,28,40] {
            for kind in [WidgetKind::TextField,WidgetKind::PasswordField] {
                let mut host=NativeUiHost::new(theme);
                host.apply_patch(&envelope(1,vec![create(1,None,0,kind,WidgetState::ENABLED,Some("9"),None)])).unwrap();
                let output=host.render(app(1),window(1),fixed_constraints(90,height)).unwrap();
                let target=output.hit_targets.iter().find(|t|t.widget==widget(1)).unwrap();
                assert_eq!(target.rect.size,Size{width:90,height});
                let (rect,font)=output.commands.iter().find_map(|command|match command{
                    DrawCommand::Text{rect,size_px,..}=>Some((*rect,*size_px)),_=>None,
                }).unwrap();
                assert_eq!(font,theme.typography.body_px);
                assert!(rect.size.height>=u32::from(font),"{kind:?} height{height}: {rect:?} must not shrink {font}px text");
                assert_eq!(rect.origin.y*2+rect.size.height as i32,height as i32,"text remains vertically centered like the grid cell");
                if height>=28 {assert_eq!(rect.origin.y,u32::from(theme.spacing.control_y) as i32,"ordinary field padding remains unchanged");}
            }
        }
    }
}

#[test]
fn document_text_field_keeps_grid_font_and_unicode_caret_in_compact_rows() {
    let mut host = NativeUiHost::default();
    let font = rice_ui::grid_frame::GridFont { family: "Arial".into(), size_pt: rice_ui::grid_frame::GridFontSize::new(18.0).unwrap(), weight: 700, italic: true, underline: true };
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::TextField,WidgetState::ENABLED | WidgetState::FOCUSED,Some("9é😀"),None),
        UiPatch::DocumentFont { id: widget(1), font: Some(serde_json::to_string(&font).unwrap()) },
        UiPatch::EditSelection { id: widget(1), anchor: 1, caret: 3 },
    ])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(80,20)).unwrap();
    let command = output.commands.iter().find_map(|c| match c { DrawCommand::StyledEdit { rect,font,caret,selection,.. } => Some((rect,font,caret,selection)), _ => None }).unwrap();
    assert_eq!(command.1,&font); assert_eq!(*command.2,Some(3)); assert_eq!(*command.3,Some((1,3)));
    assert_eq!(command.0.size.height,18); assert_eq!(command.0.origin.x,4);
    host.apply_patch(&envelope(2,vec![UiPatch::DocumentFont{id:widget(1),font:None}])).unwrap();
    assert!(!host.render(app(1),window(1),fixed_constraints(80,20)).unwrap().commands.iter().any(|c|matches!(c,DrawCommand::StyledEdit {..})));
    assert!(host.apply_patch(&envelope(3,vec![UiPatch::DocumentFont{id:widget(1),font:Some("{}".into())}])).is_err());
}

#[test]
fn explicit_caret_request_reclaims_semantic_editor_focus_after_reference_click() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None, None),
        create(2, Some(1), 0, WidgetKind::TextField, WidgetState::ENABLED | WidgetState::FOCUSED,
            Some("=A1"), None),
        create(3, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Reference"), None),
        create(4, Some(1), 2, WidgetKind::TextField, WidgetState::ENABLED, Some("Background"), None),
        UiPatch::EditSelection { id: widget(2), anchor: 3, caret: 3 },
    ])).unwrap();
    let output = host.render(app(1), window(1), fixed_constraints(320, 180)).unwrap();
    let rect = output.hit_targets.iter().find(|target| target.widget == widget(3)).unwrap().rect;
    let point = Point { x: rect.origin.x + 4, y: rect.origin.y + 4 };
    host.pointer_event(app(1), window(1), PointerEventKind::Down, point, 1, 0).unwrap();
    host.pointer_event(app(1), window(1), PointerEventKind::Up, point, 1, 0).unwrap();
    assert_eq!(host.render(app(1), window(1), fixed_constraints(320, 180)).unwrap().focused, Some(widget(3)));
    // An actor value echo and a background caret request must preserve the click.
    host.apply_patch(&envelope(2, vec![
        UiPatch::Update { id: widget(2), state: PatchValue::Keep, text: PatchValue::Set("=B1".into()), icon: PatchValue::Keep },
        UiPatch::EditSelection { id: widget(4), anchor: 0, caret: 0 },
    ])).unwrap();
    assert_eq!(host.render(app(1), window(1), fixed_constraints(320, 180)).unwrap().focused, Some(widget(3)));
    // Explicit positioning in the still semantically focused editor restores
    // typing there, without replacing its native control or selecting all text.
    host.apply_patch(&envelope(3, vec![UiPatch::EditSelection { id: widget(2), anchor: 3, caret: 3 }])).unwrap();
    assert_eq!(host.render(app(1), window(1), fixed_constraints(320, 180)).unwrap().focused, Some(widget(2)));
    let event = host.key_event(app(1), window(1), true, 0, 0, Some("+".into())).unwrap().unwrap();
    assert_eq!(event.target, widget(2));
    assert_eq!(event.event, UiEvent::ValueChanged("=B1+".into()));
}

#[test]
fn document_field_measured_pointer_uses_validated_unicode_offsets_and_capture() {
    use super::editor_measurement::EditorMeasurement;
    let mut host=NativeUiHost::default();
    let font=r#"{"family":"Arial","size_pt":18,"weight":700,"italic":true,"underline":false}"#;
    host.apply_patch(&envelope(1,vec![
        create(10,None,0,WidgetKind::Stack,WidgetState::ENABLED,Some("panel:overlay:flush"),None),
        create(11,Some(10),0,WidgetKind::Stack,WidgetState::ENABLED,Some("positioned-item:700:400:100:100"),None),
        create(1,Some(11),0,WidgetKind::TextField,WidgetState::ENABLED,Some("Wi😀i"),None),UiPatch::DocumentFont{id:widget(1),font:Some(font.into())}])).unwrap();
    host.render(app(1),window(1),fixed_constraints(800,200)).unwrap();
    let point=Point{x:566,y:90};
    let sample=EditorMeasurement{widget:widget(1),caret:2,text:"Wi😀i".into()};
    host.pointer_event_measured(app(1),window(1),PointerEventKind::Down,point,1,0,Some(&sample)).unwrap();
    let edit=host.windows.get(&(app(1),window(1))).unwrap().text_edit.unwrap();
    assert_eq!((edit.anchor,edit.caret),(2,2));
    let tail=EditorMeasurement{caret:3,..sample.clone()};
    host.pointer_event_measured(app(1),window(1),PointerEventKind::Move,Point{x:100,y:10},0,0,Some(&tail)).unwrap();
    let edit=host.windows.get(&(app(1),window(1))).unwrap().text_edit.unwrap();
    assert_eq!((edit.anchor,edit.caret),(2,6),"scalar3 is byte6 after emoji, with drag anchor retained");
    let prior=host.windows.get(&(app(1),window(1))).unwrap().text_edit;
    for invalid in [EditorMeasurement{caret:5,..sample.clone()},EditorMeasurement{text:"stale".into(),..sample.clone()},EditorMeasurement{widget:widget(2),..sample.clone()}] {
        assert!(host.pointer_event_measured(app(1),window(1),PointerEventKind::Move,point,0,0,Some(&invalid)).is_err());
        assert_eq!(host.windows.get(&(app(1),window(1))).unwrap().text_edit,prior);
    }
    host.apply_patch(&envelope(2,vec![UiPatch::DocumentFont{id:widget(1),font:None}])).unwrap();
    assert!(host.pointer_event_measured(app(1),window(1),PointerEventKind::Down,point,1,0,Some(&sample)).is_err(),"ordinary themed fields preserve their prior path");
}

#[test]
fn content_cards_reflow_without_overlap_and_keep_opaque_chrome() {
    let mut host = NativeUiHost::default();
    let mut patches = vec![create(1, None, 0, WidgetKind::IconGrid,
        WidgetState::ENABLED, Some("app-grid:6:min-cell=240"), None)];
    for index in 0..4_u64 {
        patches.push(create(10 + index, Some(1), index as u32, WidgetKind::Container,
            WidgetState::ENABLED, Some("panel:fixed:content-card:tight-stack:gap=8"), None));
        patches.push(create(20 + index, Some(10 + index), 0, WidgetKind::Label,
            WidgetState::ENABLED, Some("A useful feature"), None));
    }
    host.apply_patch(&envelope(1, patches)).expect("card grid patch");
    let card_height = u32::from(host.theme().metrics.control_height) * 6;
    let card_radius = host.theme().metrics.corner_radius.min(18);
    let card_rects = |output: &super::RenderOutput| output.commands.iter().filter_map(|command| {
        if let DrawCommand::Fill { rect, color, radius } = command
            && rect.size.height == card_height
        {
            assert_eq!(*radius, card_radius, "cards use the active theme corners");
            assert_eq!(color.channels()[3], 255, "cards have opaque faces");
            Some(*rect)
        } else { None }
    }).collect::<Vec<_>>();
    let wide = host.render(app(1), window(1), fixed_constraints(580, 1200)).expect("wide cards");
    let wide = card_rects(&wide);
    assert_eq!(wide.len(), 4);
    assert_eq!(wide[0].origin.y, wide[1].origin.y);
    assert!(wide[0].size.width >= 240);
    assert_eq!(wide[0].size.width, wide[1].size.width);
    assert!(wide[0].origin.x + wide[0].size.width as i32 + 16 <= wide[1].origin.x);
    let narrow = host.render(app(1), window(1), fixed_constraints(420, 1200)).expect("narrow cards");
    let narrow = card_rects(&narrow);
    assert_eq!(narrow.len(), 4);
    for pair in narrow.windows(2) {
        assert_eq!(pair[0].origin.x, pair[1].origin.x);
        assert!(pair[0].origin.y + pair[0].size.height as i32 + 16 <= pair[1].origin.y);
    }
}

#[test]
fn document_view_zoom_scales_editor_insets_but_preserves_original_font_and_caret() {
    for zoom in [25_u16,125,200,400] {
        let mut host=NativeUiHost::default();let font=format!(r#"{{"family":"Inter","size_pt":11,"weight":700,"italic":true,"underline":false,"zoom_percent":{zoom}}}"#);
        host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::TextField,WidgetState::ENABLED|WidgetState::FOCUSED,Some("Wi😀"),None),UiPatch::DocumentFont{id:widget(1),font:Some(font)},UiPatch::EditSelection{id:widget(1),anchor:3,caret:3}])).unwrap();
        let output=host.render(app(1),window(1),fixed_constraints(500,160)).unwrap();
        let (scale,font,rect)=output.commands.iter().find_map(|c|if let DrawCommand::StyledEdit{zoom_percent,font,rect,caret:Some(3),..}=c{Some((*zoom_percent,font,rect))}else{None}).unwrap();
        assert_eq!(scale,zoom);assert_eq!(font.size_pt.points(),11.0);assert_eq!(rect.origin.x,((4*u32::from(zoom)+50)/100)as i32);
        let invalid=format!(r#"{{"family":"Inter","size_pt":11,"weight":700,"italic":true,"underline":false,"zoom_percent":401}}"#);
        assert!(host.apply_patch(&envelope(2,vec![UiPatch::DocumentFont{id:widget(1),font:Some(invalid)}])).is_err());
    }
}

#[test]
fn fractional_document_editor_keeps_exact_size_zoom_and_unicode_selection_atomically() {
    for size in [11.5_f64,14.5,11.25] {
        for zoom in [70_u16,100,150] {
            let mut host=NativeUiHost::default();
            let font=format!(r##"{{"family":"Arial","size_pt":{size},"weight":700,"italic":true,"underline":true,"zoom_percent":{zoom},"paint":{{"background":"#FFFFFF","foreground":"#000000"}}}}"##);
            host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::TextField,WidgetState::ENABLED|WidgetState::FOCUSED,Some("Wié😀"),None),UiPatch::DocumentFont{id:widget(1),font:Some(font.clone())},UiPatch::EditSelection{id:widget(1),anchor:1,caret:4}])).unwrap();
            let output=host.render(app(1),window(1),fixed_constraints(240,30)).unwrap();
            let (actual_zoom,actual_font,rect)=output.commands.iter().find_map(|c|match c {
                DrawCommand::StyledEdit{zoom_percent,font,rect,caret:Some(4),selection:Some((1,4)),color,..} if *color==rice_ui::Color::opaque(0,0,0)=>Some((*zoom_percent,font,*rect)),_=>None,
            }).unwrap();
            assert_eq!(actual_font.size_pt.points().to_bits(),size.to_bits());
            assert_eq!(actual_zoom,zoom);assert_eq!(rect.origin.x,((4*u32::from(zoom)+50)/100)as i32);
            for invalid in ["0","-11","409.5","null","\"11.5\"","1e999"] {
                let bad=font.replace(&format!("\"size_pt\":{size}"),&format!("\"size_pt\":{invalid}"));
                assert!(host.apply_patch(&envelope(2,vec![UiPatch::DocumentFont{id:widget(1),font:Some(bad)}])).is_err());
                let after=host.render(app(1),window(1),fixed_constraints(240,30)).unwrap();
                assert_eq!(after.commands,output.commands,"invalid font must not change retained edit/selection");
            }
        }
    }
}

#[test]
fn popup_keyboard_navigation_unwinds_levels_and_preserves_context_owner() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1,None,0,WidgetKind::Column,WidgetState::ENABLED,None,None),
        create(2,Some(1),0,WidgetKind::Button,WidgetState::ENABLED,Some("Worksheet"),None),
        create(10,Some(1),1,WidgetKind::Menu,WidgetState::ENABLED,Some("Context"),None),
        create(11,Some(10),0,WidgetKind::MenuItem,WidgetState::empty(),Some("Cut"),None),
        create(12,Some(10),1,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Copy"),None),
        create(13,Some(10),2,WidgetKind::MenuItem,WidgetState::ENABLED,Some("menu:separator"),None),
        create(14,Some(10),3,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Sort"),None),
        create(20,Some(14),0,WidgetKind::Menu,WidgetState::ENABLED,None,None),
        create(21,Some(20),0,WidgetKind::MenuItem,WidgetState::empty(),Some("Unavailable"),None),
        create(22,Some(20),1,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Ascending"),None),
        create(23,Some(20),2,WidgetKind::MenuItem,WidgetState::ENABLED,Some("More"),None),
        create(30,Some(23),0,WidgetKind::Menu,WidgetState::ENABLED,None,None),
        create(31,Some(30),0,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Custom"),None),
    ])).unwrap();
    {
        let native=host.windows.get_mut(&(app(1),window(1))).unwrap();
        native.opened_menu=Some(widget(10));
        native.popup_anchor=Some(Point{x:80,y:40});
        native.popup_context_target=Some(widget(2));
    }
    for (key, expected, depth) in [(116,12,0),(116,14,0),(114,22,1),(116,23,1),(114,31,2),(9,23,1),(113,14,0),(36,22,1),(9,14,0),(9,2,0)] {
        host.render(app(1),window(1),fixed_constraints(800,600)).unwrap();
        let event=host.key_event(app(1),window(1),true,key,0,None).unwrap().unwrap();
        assert_eq!(event.target,widget(expected),"key {key}");
        assert_eq!(event.event,UiEvent::Focus{focused:true});
        let native=host.windows.get(&(app(1),window(1))).unwrap();
        assert_eq!(native.focused,Some(widget(expected)));
        assert_eq!(native.opened_submenus.len(),depth);
        assert_eq!(native.opened_menu,if expected==2 {None}else{Some(widget(10))});
        assert_eq!(native.popup_context_target,if expected==2 {None}else{Some(widget(2))});
    }
}

#[test]
fn popup_arrow_navigation_wraps_skips_disabled_and_keeps_printable_keys() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::Menu,WidgetState::ENABLED,Some("Menu"),None),
        create(2,Some(1),0,WidgetKind::MenuItem,WidgetState::empty(),Some("Disabled"),None),
        create(3,Some(1),1,WidgetKind::MenuItem,WidgetState::ENABLED,Some("First"),None),
        create(4,Some(1),2,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Last"),None),
    ])).unwrap();
    host.windows.get_mut(&(app(1),window(1))).unwrap().opened_menu=Some(widget(1));
    for (key,expected) in [(111,4),(116,3),(115,4),(110,3)] {
        let event=host.key_event(app(1),window(1),true,key,0,None).unwrap().unwrap();
        assert_eq!(event.target,widget(expected));
    }
    let event=host.key_event(app(1),window(1),true,38,0,Some("a".into())).unwrap().unwrap();
    assert_eq!(event.event,UiEvent::Key{pressed:true,key_code:38,modifiers:0,text:Some("a".into())});
    assert_eq!(host.windows.get(&(app(1),window(1))).unwrap().opened_menu,Some(widget(1)));
}

#[test]
fn popup_arrows_cannot_leak_to_the_context_owner_during_a_focus_echo() {
    let mut host=NativeUiHost::default();
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::Column,WidgetState::ENABLED,None,None),
        create(2,Some(1),0,WidgetKind::TextField,WidgetState::ENABLED|WidgetState::KEY_NAVIGATION,Some("=A1"),None),
        create(10,Some(1),1,WidgetKind::Menu,WidgetState::ENABLED,Some("Context"),None),
        create(11,Some(10),0,WidgetKind::MenuItem,WidgetState::empty(),Some("Cut"),None),
        create(12,Some(10),1,WidgetKind::MenuItem,WidgetState::ENABLED,Some("Copy"),None),
    ])).unwrap();
    let native=host.windows.get_mut(&(app(1),window(1))).unwrap();
    native.focused=Some(widget(2));native.opened_menu=Some(widget(10));
    let event=host.key_event(app(1),window(1),true,116,0,None).unwrap().unwrap();
    assert_eq!(event.target,widget(12));
    assert_eq!(event.event,UiEvent::Focus{focused:true});
}

#[test]
fn document_alignment_keeps_full_editor_bounds_and_measured_unicode_hits() {
    use super::editor_measurement::EditorMeasurement;
    for theme in [Theme::ricewm_dark(),Theme::familiar_light()] {
        for size in [11.25,24.0,409.0] {
            for zoom in [70,100,150] {
                for height in [20,96] {
                    let mut geometry=None;
                    for (vertical,wire) in [(None,0),(Some("center"),0),(Some("top"),1),(Some("bottom"),2)] {
                        let mut host=NativeUiHost::new(theme);
                        let mut font=serde_json::json!({"family":"Arial","size_pt":size,"weight":700,"italic":true,"underline":false,"zoom_percent":zoom,"paint":{"background":"#FFFFFF"}});
                        if let Some(vertical)=vertical {font["paint"]["vertical"]=vertical.into();}
                        host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::TextField,WidgetState::ENABLED|WidgetState::FOCUSED,Some("Wi😀i"),None),UiPatch::DocumentFont{id:widget(1),font:Some(font.to_string())},UiPatch::EditSelection{id:widget(1),anchor:1,caret:3}])).unwrap();
                        let output=host.render(app(1),window(1),fixed_constraints(240,height)).unwrap();
                        let rect=output.commands.iter().find_map(|c|match c{DrawCommand::StyledEdit{vertical,rect,caret:Some(3),selection:Some((1,3)),font,..}=>{assert_eq!(*vertical,wire);assert_eq!(font.size_pt.points(),size);Some(*rect)},_=>None}).unwrap();
                        if let Some(expected)=geometry {assert_eq!(rect,expected,"alignment must not move or shrink editor command bounds");}else{geometry=Some(rect);}
                        let target=output.hit_targets.iter().find(|t|t.widget==widget(1)).unwrap();
                        assert_eq!(target.rect.size,Size{width:240,height});
                        // A line-box shortcut would put the center outside a
                        // short row for the409pt case and lose measured hits.
                        for y in [1,height as i32/2,height as i32-2] {
                            let point=Point{x:24,y};
                            assert_eq!(host.hit_test(app(1),window(1),point).unwrap().widget,widget(1));
                            let sample=EditorMeasurement{widget:widget(1),caret:3,text:"Wi😀i".into()};
                            host.pointer_event_measured(app(1),window(1),PointerEventKind::Down,point,1,0,Some(&sample)).unwrap();
                            host.pointer_event_measured(app(1),window(1),PointerEventKind::Up,point,1,0,Some(&sample)).unwrap();
                        }
                        let event=host.key_event(app(1),window(1),true,0,0,Some("X".into())).unwrap().unwrap();
                        assert!(matches!(event.event,UiEvent::ValueChanged(ref value) if value=="Wi😀Xi"));
                    }
                }
            }
        }
    }
}

#[test]
fn settings_help_hover_preserves_row_activation() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::Row, WidgetState::ENABLED, Some("split:620:compact:form-field:control-for=5"), None),
        create(2, Some(1), 0, WidgetKind::Column, WidgetState::ENABLED, Some("panel:tight-stack"), None),
        create(3, Some(2), 0, WidgetKind::Label, WidgetState::ENABLED, Some("Button shape"), None),
        create(4, Some(3), 0, WidgetKind::Tooltip, WidgetState::ENABLED, Some("Full button shape help"), None),
        create(5, Some(1), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Theme Default"), None),
    ])).unwrap();
    let constraints = fixed_constraints(560, 160);
    let output = host.render(app(1), window(1), constraints).unwrap();
    let label = output.hit_targets.iter().find(|hit| hit.widget == widget(3)).unwrap().rect;
    host.pointer_event(app(1), window(1), PointerEventKind::Move, label.origin, 0, 0).unwrap();
    let hover = host.render(app(1), window(1), constraints).unwrap();
    assert!(hover.commands.iter().any(|command| matches!(command, DrawCommand::BalloonFrame {..})));
    host.pointer_event(app(1), window(1), PointerEventKind::Down, label.origin, 1, 0).unwrap();
    let event = host.pointer_event(app(1), window(1), PointerEventKind::Up, label.origin, 1, 0).unwrap().unwrap();
    assert_eq!(event.target, widget(5));
    assert!(matches!(event.event, UiEvent::Activate));
}

#[test]
fn floating_drawer_editor_preserves_owner_geometry_and_routes_its_controls() {
    let mut host = NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&envelope(1, vec![
        create(1,None,0,WidgetKind::Column,WidgetState::ENABLED|WidgetState::FLAT,Some("panel:fixed:flush"),None),
        create(2,Some(1),0,WidgetKind::TextField,WidgetState::ENABLED,Some("Keep my draft"),None),
    ])).unwrap();
    host.render(app(1),window(1),fixed_constraints(400,240)).unwrap();
    let primary = host.windows[&(app(1),window(1))].geometry[&widget(2)];
    host.apply_patch(&envelope(2,vec![
        create(3,Some(1),1,WidgetKind::Dialog,WidgetState::ENABLED|WidgetState::EXPANSIVE,Some("panel:drawer-dialog:When"),None),
        create(4,Some(3),0,WidgetKind::Button,WidgetState::ENABLED,Some("Done"),None),
    ])).unwrap();
    assert!(host.set_dialog_work_area(app(1),window(1),Some(Rect{origin:Point{x:-200,y:-100},size:Size{width:1200,height:900}})));
    let rendered = host.render(app(1),window(1),fixed_constraints(400,240)).unwrap();
    assert_eq!(host.windows[&(app(1),window(1))].geometry[&widget(2)],primary);
    let pane = rendered.dialog.unwrap().rect;
    assert!(pane.origin.x>=412 || pane.origin.y>=252 || pane.origin.x+pane.size.width as i32<=-12 || pane.origin.y+pane.size.height as i32<=-12);
    assert!(rendered.commands.iter().any(|c|matches!(c,DrawCommand::Text{text,..} if text=="Keep my draft")));
    let target=rendered.hit_targets.iter().find(|hit|hit.widget==widget(4)).unwrap();
    assert_eq!(host.hit_test(app(1),window(1),target.rect.origin).unwrap().widget,widget(4));
    assert!(host.hit_test(app(1),window(1),primary.origin).is_none(),"modal ownership does not send input into the inactive primary form");
}

#[test]
fn flush_scroll_view_keeps_first_caption_and_avoids_a_spurious_scrollbar() {
    let mut host=NativeUiHost::new(Theme::familiar_light());
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::ScrollView,WidgetState::ENABLED|WidgetState::EXPANSIVE|WidgetState::FLAT,Some("panel:flush"),None),
        create(2,Some(1),0,WidgetKind::Column,WidgetState::ENABLED|WidgetState::FLAT,Some("panel:fixed:flush"),None),
        create(3,Some(2),0,WidgetKind::Label,WidgetState::ENABLED,Some("Hour"),None),
    ])).unwrap();
    let rendered=host.render(app(1),window(1),fixed_constraints(400,240)).unwrap();
    let native=&host.windows[&(app(1),window(1))];
    assert!(super::layout::scroll_view_geometry(&native.tree,widget(1),&native.geometry,&host.theme).is_none(),"fitting content needs no scroll track");
    let caption=rendered.commands.iter().find_map(|c|match c{DrawCommand::Text{text,rect,..} if text=="Hour"=>Some(*rect),_=>None}).unwrap();
    for command in &rendered.commands {
        if let DrawCommand::PushClip{rect}=command {
            assert!(caption.origin.x>=rect.origin.x&&caption.origin.y>=rect.origin.y);
        }
    }
}

#[test]
fn expanding_drawer_seam_follows_each_connected_edge() {
    for anchor in ["left","right","top","bottom"] {
        let mut host=NativeUiHost::new(Theme::familiar_light());
        let text=format!("panel:fixed:flush:drawer-edge={anchor}");
        host.apply_patch(&envelope(1,vec![create(1,None,0,WidgetKind::Column,WidgetState::ENABLED|WidgetState::FLAT,Some(&text),None)])).unwrap();
        let rendered=host.render(app(1),window(1),fixed_constraints(400,240)).unwrap();
        let expected=match anchor {
            "left"=>Rect{origin:Point{x:399,y:0},size:Size{width:1,height:240}},
            "right"=>Rect{origin:Point{x:0,y:0},size:Size{width:1,height:240}},
            "top"=>Rect{origin:Point{x:0,y:239},size:Size{width:400,height:1}},
            _=>Rect{origin:Point{x:0,y:0},size:Size{width:400,height:1}},
        };
        assert!(rendered.commands.iter().any(|c|matches!(c,DrawCommand::Fill{rect,color,..} if *rect==expected&&*color==host.theme.palette.border.with_alpha(160))),"{anchor} connects on the correct edge");
    }
}

#[test]
fn padded_drawer_scroll_view_preserves_reading_insets_without_nested_chrome() {
    for theme in [Theme::familiar_light(), Theme::familiar_dark()] {
        let mut host=NativeUiHost::new(theme);
        host.apply_patch(&envelope(1,vec![
            create(1,None,0,WidgetKind::Column,WidgetState::ENABLED|WidgetState::FLAT,Some("panel:fixed:padding=12:gap=8:drawer-surface:drawer-edge=bottom"),None),
            create(2,Some(1),0,WidgetKind::ScrollView,WidgetState::ENABLED|WidgetState::FLAT|WidgetState::EXPANSIVE,Some("panel:padding=8"),None),
            create(3,Some(2),0,WidgetKind::Column,WidgetState::ENABLED|WidgetState::FLAT,Some("panel:fixed:flush"),None),
            create(4,Some(3),0,WidgetKind::Label,WidgetState::ENABLED,Some("Hour"),None),
            create(5,Some(1),1,WidgetKind::Button,WidgetState::ENABLED,Some("Done"),None),
        ])).unwrap();
        let rendered=host.render(app(1),window(1),fixed_constraints(400,240)).unwrap();
        let native=&host.windows[&(app(1),window(1))];
        let panel=native.geometry[&widget(1)];
        let scroll=native.geometry[&widget(2)];
        let caption=native.geometry[&widget(4)];
        let done=native.geometry[&widget(5)];
        assert_eq!(scroll.origin.x-panel.origin.x,12);
        assert_eq!(scroll.origin.y-panel.origin.y,12);
        assert_eq!(caption.origin.x-scroll.origin.x,8);
        assert_eq!(caption.origin.y-scroll.origin.y,8);
        assert_eq!(panel.origin.y+panel.size.height as i32-done.origin.y-done.size.height as i32,12);
        assert!(super::layout::scroll_view_geometry(&native.tree,widget(2),&native.geometry,&host.theme).is_none());
        assert!(rendered.commands.iter().any(|c|matches!(c,DrawCommand::Fill{rect,color,..} if *rect==panel&&*color==host.theme.palette.surface_raised)));
        assert!(!rendered.commands.iter().any(|c|matches!(c,DrawCommand::Border{rect,..} if *rect==scroll)),"flat reading area has no nested border");
    }
}

#[test]
fn scoped_native_actions_keep_dark_ink_icons_and_no_editor_caret_on_dark_theme() {
    let mut host=NativeUiHost::default();
    let font=r##"{"family":"Liberation Sans","size_pt":11,"weight":400,"italic":false,"underline":false,"paint":{"foreground":"#262b35","background":"#ffffff"}}"##;
    host.apply_patch(&envelope(1,vec![
        create(1,None,0,WidgetKind::Button,WidgetState::ENABLED|WidgetState::FLAT|WidgetState::FOCUSED,Some("Add task"),Some(rice_ui::IconId::ObjectAdd)),
        UiPatch::DocumentFont{id:widget(1),font:Some(font.into())},
    ])).unwrap();
    let output=host.render(app(1),window(1),fixed_constraints(220,44)).unwrap();
    let ink=rice_ui::Color::opaque(38,43,53);
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::ScopedIcon{color,..}if *color==ink)));
    assert!(output.commands.iter().any(|c|matches!(c,DrawCommand::StyledEdit{color,caret,selection,..}if *color==ink&&caret.is_none()&&selection.is_none())));
}

#[test]
fn wrapped_action_rows_measure_their_actual_fixed_accessory_widths() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&envelope(1, vec![
        create(1, None, 0, WidgetKind::ScrollView, WidgetState::ENABLED | WidgetState::FLAT, Some("panel:flush"), None),
        create(2, Some(1), 0, WidgetKind::Column, WidgetState::ENABLED | WidgetState::FLAT, Some("panel:fixed:flush:intrinsic-flow:gap=4"), None),
        create(3, Some(2), 0, WidgetKind::Row, WidgetState::ENABLED | WidgetState::FLAT, Some("panel:fixed:flush:wrap-controls:gap=4"), None),
        create(4, Some(3), 0, WidgetKind::Button, WidgetState::ENABLED | WidgetState::FLAT | WidgetState::EXPANSIVE, Some("Regional campaign planning, product launches, design reviews and customer research"), None),
        create(5, Some(3), 1, WidgetKind::Column, WidgetState::ENABLED | WidgetState::FLAT, Some("panel:fixed:flush:width=48"), None),
        create(6, Some(5), 0, WidgetKind::Label, WidgetState::FLAT, Some("1"), None),
        create(7, Some(2), 1, WidgetKind::Button, WidgetState::ENABLED, Some("Next action"), None),
    ])).unwrap();
    for width in [200, 360] {
        host.render(app(1), window(1), fixed_constraints(width, 240)).unwrap();
        let native = &host.windows[&(app(1), window(1))];
        let caption = native.geometry[&widget(4)];
        let expected = super::layout::preferred_height_for_width(&native.tree, widget(4), caption.size.width, &host.theme);
        assert_eq!(caption.size.height, expected, "row height uses the caption's allocated width rather than equal columns");
        let count = native.geometry[&widget(5)];
        assert_eq!(count.size.width, 48);
        assert!(native.geometry[&widget(7)].origin.y >= caption.origin.y + caption.size.height as i32, "following actions cannot overlap wrapped navigation");
        let point = Point {x: caption.origin.x + 2, y: caption.origin.y + 2};
        assert_eq!(host.hit_test(app(1), window(1), point).unwrap().widget, widget(4));
        assert!(host.activate_event(app(1), window(1), widget(4)).unwrap().is_some());
    }
}
