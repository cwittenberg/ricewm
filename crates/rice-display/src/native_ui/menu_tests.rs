use super::*;
use rice_native_app_protocol::UiPatch;
use rice_ui::{ChromeProfile, Color, IconId, WidgetKind, WidgetState};

fn app() -> AppId {
    AppId::new(1).expect("test app")
}

fn window() -> WindowId {
    WindowId::new(1).expect("test window")
}

fn widget(value: u64) -> WidgetId {
    WidgetId::new(value).expect("test widget")
}

fn create(
    id: u64,
    parent: Option<u64>,
    index: u32,
    kind: WidgetKind,
    state: WidgetState,
    text: Option<&str>,
) -> UiPatch {
    UiPatch::Create {
        id: widget(id),
        parent: parent.map(widget),
        index,
        kind: kind as u16,
        state: state.bits(),
        text: text.map(str::to_owned),
        icon: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn create_with_icon(
    id: u64,
    parent: Option<u64>,
    index: u32,
    kind: WidgetKind,
    state: WidgetState,
    text: Option<&str>,
    icon: IconId,
) -> UiPatch {
    UiPatch::Create {
        id: widget(id),
        parent: parent.map(widget),
        index,
        kind: kind as u16,
        state: state.bits(),
        text: text.map(str::to_owned),
        icon: Some(icon as u16),
    }
}

fn menu_tree() -> UiPatchEnvelope {
    UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None),
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
                Some("View"),
            ),
            create(
                4,
                Some(3),
                0,
                WidgetKind::CheckBox,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some("Show Hidden Files\tAlt+H"),
            ),
            create(
                5,
                Some(3),
                1,
                WidgetKind::MenuItem,
                WidgetState::empty(),
                Some("menu:separator"),
            ),
            create(
                6,
                Some(3),
                2,
                WidgetKind::RadioButton,
                WidgetState::ENABLED | WidgetState::CHECKED,
                Some("Detail View"),
            ),
            create(
                9,
                Some(3),
                3,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Export"),
            ),
            create(
                10,
                Some(9),
                0,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("Export"),
            ),
            create(
                11,
                Some(10),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Archive"),
            ),
            create(
                7,
                Some(2),
                1,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("File"),
            ),
            create(
                8,
                Some(7),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Open"),
            ),
        ],
    }
}

fn menu_alignment_tree() -> UiPatchEnvelope {
    UiPatchEnvelope {
        app: app(),
        window: window(),
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, WidgetState::ENABLED, None),
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
                Some("Edit"),
            ),
            create_with_icon(
                4,
                Some(3),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Undo"),
                IconId::EditUndo,
            ),
            create(
                5,
                Some(3),
                1,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Select All"),
            ),
            create(
                6,
                Some(2),
                1,
                WidgetKind::Menu,
                WidgetState::ENABLED,
                Some("Search"),
            ),
            create(
                7,
                Some(6),
                0,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Find Next"),
            ),
            create(
                8,
                Some(6),
                1,
                WidgetKind::MenuItem,
                WidgetState::ENABLED,
                Some("Find Previous"),
            ),
        ],
    }
}

#[test]
fn popup_labels_share_a_leading_column_only_when_the_menu_needs_one() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&menu_alignment_tree())
        .expect("menu alignment tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let edit_heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("Edit heading");
    let search_heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(6))
        .copied()
        .expect("Search heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        edit_heading.rect.origin,
        1,
        0,
    )
    .expect("open Edit menu");
    let edit = host
        .render(app(), window(), constraints)
        .expect("Edit menu");
    let text_x = |render: &RenderOutput, label: &str| {
        render
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text { rect, text, .. } if text == label => Some(rect.origin.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label} label"))
    };
    assert_eq!(text_x(&edit, "Undo"), text_x(&edit, "Select All"));
    let undo_row = edit
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("Undo row");
    assert!(text_x(&edit, "Undo") > undo_row.rect.origin.x + 12);

    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        search_heading.rect.origin,
        0,
        0,
    )
    .expect("switch to Search menu");
    let search = host
        .render(app(), window(), constraints)
        .expect("Search menu");
    for (id, label) in [(7, "Find Next"), (8, "Find Previous")] {
        let row = search
            .hit_targets
            .iter()
            .find(|target| target.widget == widget(id))
            .unwrap_or_else(|| panic!("missing {label} row"));
        assert_eq!(text_x(&search, label), row.rect.origin.x + 12);
    }
}

#[test]
fn command_deck_menubar_and_popup_selections_are_readable_pills() {
    let mut theme = Theme::ricewm_dark();
    theme.chrome_profile = ChromeProfile::CommandDeck;
    theme.palette.selection = Color::opaque(255, 204, 102);
    theme.palette.selection_text = Color::opaque(10, 8, 13);
    theme.menu_palette.selection = theme.palette.selection;
    theme.menu_palette.selection_text = theme.palette.selection_text;
    theme.palette.danger = Color::opaque(239, 139, 114);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("View heading");
    assert!(closed.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, radius }
            if rect.origin.x == heading.rect.origin.x + 4
                && rect.origin.y == heading.rect.origin.y + 3
                && [
                    Color::opaque(204, 153, 204),
                    Color::opaque(239, 139, 114),
                ].contains(color)
                && *radius > 0
    )));
    assert!(
        !closed.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Border { rect, .. }
                if rect.origin.x == heading.rect.origin.x + 4
                    && rect.origin.y == heading.rect.origin.y + 3
        )),
        "idle LCARS menu keys are solid fields, not outlined pills"
    );
    assert!(closed.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "View" && *color == Color::opaque(10, 8, 13)
    )));
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        heading.rect.origin,
        1,
        0,
    )
    .expect("open menu");
    let opened = host.render(app(), window(), constraints).expect("opened");
    assert!(opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "View" && *color == Color::opaque(10, 8, 13)
    )));
    let row = opened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .copied()
        .expect("first popup row");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        row.rect.origin,
        0,
        0,
    )
    .expect("hover row");
    let hovered = host.render(app(), window(), constraints).expect("hovered");
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, radius }
            if rect.origin.x == row.rect.origin.x + 4
                && rect.origin.y == row.rect.origin.y + 3
                && *color == Color::opaque(255, 204, 102)
                && *radius > 0
    )));
    assert!(hovered.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Show Hidden Files" && *color == Color::opaque(10, 8, 13)
    )));
}

#[test]
fn redmond_open_menu_heading_keeps_the_classic_grey_face() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::Redmond98;
    theme.palette.surface = Color::opaque(192, 192, 192);
    theme.palette.surface_raised = Color::opaque(255, 255, 255);
    theme.palette.selection = Color::opaque(0, 0, 128);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("View heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        Point {
            x: heading.rect.origin.x + 2,
            y: heading.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open menu");
    let opened = host.render(app(), window(), constraints).expect("opened");
    let heading_center = Point {
        x: heading.rect.origin.x + i32::try_from(heading.rect.size.width / 2).unwrap(),
        y: heading.rect.origin.y + i32::try_from(heading.rect.size.height / 2).unwrap(),
    };
    assert!(opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.contains(heading_center) && *color == Color::opaque(192, 192, 192)
    )));
    assert!(!opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.contains(heading_center) && *color == Color::opaque(255, 255, 255)
    )));
    assert!(!opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Fill { rect, color, .. }
            if rect.contains(heading_center) && *color == Color::opaque(0, 0, 128)
    )));
}

#[test]
fn popup_rows_render_shortcuts_separators_and_checked_choices_as_native_controls() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host
        .render(app(), window(), constraints)
        .expect("closed menu");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("View heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        Point {
            x: heading.rect.origin.x + 2,
            y: heading.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open menu");
    let opened = host
        .render(app(), window(), constraints)
        .expect("opened menu");
    let popup = opened.popup.expect("popup metadata");
    let popup_commands = &opened.commands[popup.command_start..];
    assert!(matches!(
        popup_commands.first(),
        Some(DrawCommand::Fill { rect, color, .. })
            if *rect == popup.rect && *color == host.theme().menu_palette.surface_raised
    ));
    assert!(
        !popup_commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, .. }
                if rect.size == popup.rect.size && rect.origin != popup.rect.origin
        )),
        "menu elevation belongs to the compositor, not an offset fill behind each pane"
    );
    for expected in ["Show Hidden Files", "Alt+H", "✓"] {
        assert!(
            opened.commands.iter().any(
                |command| matches!(command, DrawCommand::Text { text, .. } if text == expected)
            )
        );
    }
    assert!(opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: rice_ui::IconId::SubmenuRight,
            ..
        }
    )));
    assert!(
        opened.commands.iter().all(
            |command| !matches!(command, DrawCommand::Text { text, .. } if text.contains('\t'))
        )
    );
    assert!(
        opened
            .hit_targets
            .iter()
            .all(|target| target.widget != widget(5)),
        "separators must not be interactive"
    );
    let checkbox = opened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("checkbox row");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: checkbox.rect.origin.x + 2,
            y: checkbox.rect.origin.y + 2,
        },
        0,
        0,
    )
    .expect("hover checkbox");
    let hovered = host
        .render(app(), window(), constraints)
        .expect("hovered menu");
    assert!(hovered.commands.iter().any(|command| {
        matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == checkbox.rect
                    && *color == Theme::ricewm_default().palette.selection
        )
    }));
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Down,
        Point {
            x: checkbox.rect.origin.x + 2,
            y: checkbox.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("checkbox press");
    let activation = host
        .pointer_event(
            app(),
            window(),
            PointerEventKind::Up,
            Point {
                x: checkbox.rect.origin.x + 2,
                y: checkbox.rect.origin.y + 2,
            },
            1,
            0,
        )
        .expect("checkbox click")
        .expect("checkbox activation");
    assert_eq!(activation.target, widget(4));
    assert_eq!(activation.event, UiEvent::Activate);
    let closed = host
        .render(app(), window(), constraints)
        .expect("menu closes");
    assert!(
        closed
            .hit_targets
            .iter()
            .all(|target| target.widget != widget(4))
    );
}

#[test]
fn redmond_popup_uses_one_compact_indicator_gutter_for_every_command() {
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("View heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        heading.rect.origin,
        1,
        0,
    )
    .expect("open menu");
    let opened = host.render(app(), window(), constraints).expect("opened");
    let popup = opened.popup.expect("popup");
    let first_row = opened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .expect("checkbox row");
    assert_eq!(first_row.rect.origin.x, popup.rect.origin.x + 3);

    let label_x = |label: &str| {
        opened
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text { rect, text, .. } if text == label => Some(rect.origin.x),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing {label} label"))
    };
    assert_eq!(label_x("Show Hidden Files"), label_x("Detail View"));
    assert_eq!(label_x("Detail View"), label_x("Export"));
    let label_start = label_x("Detail View");

    for expected_width in [1, 3, 5, 3, 1] {
        assert!(opened.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, radius: 0 }
                if rect.size.width == expected_width
                    && rect.size.height == 1
                    && rect.origin.x.saturating_add_unsigned(rect.size.width) < label_start
                    && *color == Color::opaque(0, 0, 0)
        )));
    }

    let file_heading = opened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(7))
        .copied()
        .expect("File heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        file_heading.rect.origin,
        0,
        0,
    )
    .expect("switch to File menu");
    let file_menu = host
        .render(app(), window(), constraints)
        .expect("File menu");
    let open_row = file_menu
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(8))
        .expect("Open row");
    let open_label_x = file_menu
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Open" => Some(rect.origin.x),
            _ => None,
        })
        .expect("Open label");
    assert_eq!(open_label_x, open_row.rect.origin.x + 4);
}

#[test]
fn glass_menu_uses_its_popup_palette_without_opaquing_the_application_surface() {
    let mut theme = Theme::ricewm_default();
    theme.glass_menus = true;
    theme.menu_palette.surface = Color::opaque(23, 33, 31);
    theme.menu_palette.text = Color::opaque(245, 247, 246);
    let expected_text = theme.menu_palette.text;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host
        .render(app(), window(), constraints)
        .expect("closed menu");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("View heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        Point {
            x: heading.rect.origin.x + 2,
            y: heading.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open menu");
    let opened = host
        .render(app(), window(), constraints)
        .expect("opened menu");
    let popup = opened.popup.expect("popup metadata");
    let popup_commands = &opened.commands[popup.command_start..];
    assert!(matches!(
        popup_commands.first(),
        Some(DrawCommand::Glass { rect, tint, control: false, popup: true, .. })
            if *rect == popup.rect && tint.channels()[3] < u8::MAX
    ));
    assert!(matches!(
        popup_commands.get(1),
        Some(DrawCommand::Fill { rect, color, .. })
            if *rect == popup.rect && color.channels()[3] == 210
    ));
    assert!(!popup_commands.iter().any(|command| matches!(
        command,
        DrawCommand::Border { rect, .. } if *rect == popup.rect
    )));
    assert!(popup_commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "Detail View" && *color == expected_text
    )));
}

#[test]
fn familiar_light_menu_highlights_use_dark_ink_on_their_rendered_surfaces() {
    let mut theme = Theme::familiar_light();
    theme.palette.surface_raised = Color::rgba(255, 255, 255, 150);
    theme.palette.selection = Color::rgba(0, 120, 215, 170);
    theme.menu_palette.surface_raised = theme.palette.surface_raised;
    theme.menu_palette.selection = theme.palette.selection;
    let expected_ink = Color::opaque(10, 8, 13);
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let heading = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("View heading");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        Point {
            x: heading.rect.origin.x + 2,
            y: heading.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open menu");
    let opened = host.render(app(), window(), constraints).expect("opened");
    assert!(opened.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, color, .. }
            if text == "View" && *color == expected_ink
    )));

    let row = opened
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .copied()
        .expect("first popup row");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Move,
        Point {
            x: row.rect.origin.x + 2,
            y: row.rect.origin.y + 2,
        },
        0,
        0,
    )
    .expect("hover popup row");
    let hovered = host.render(app(), window(), constraints).expect("hovered");
    for text in ["Show Hidden Files", "Alt+H"] {
        assert!(hovered.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text: candidate, color, .. }
                if candidate == text && *color == expected_ink
        )));
    }
}

#[test]
fn open_menubar_tracks_hover_across_headings_without_another_click() {
    let mut host = NativeUiHost::default();
    host.apply_patch(&menu_tree()).expect("menu tree");
    let constraints = Constraints {
        min: Size {
            width: 420,
            height: 260,
        },
        max: Size {
            width: 420,
            height: 260,
        },
    };
    let closed = host.render(app(), window(), constraints).expect("closed");
    let view = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .copied()
        .expect("View");
    let file = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(7))
        .copied()
        .expect("File");
    host.pointer_event(
        app(),
        window(),
        PointerEventKind::Up,
        view.rect.origin,
        1,
        0,
    )
    .expect("open View");
    host.render(app(), window(), constraints)
        .expect("View popup");
    assert!(
        host.pointer_event(
            app(),
            window(),
            PointerEventKind::Move,
            file.rect.origin,
            0,
            0
        )
        .expect("hover File")
        .is_none()
    );
    let switched = host
        .render(app(), window(), constraints)
        .expect("File popup");
    assert!(
        switched
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(8))
    );
    assert!(
        switched
            .hit_targets
            .iter()
            .all(|target| target.widget != widget(4))
    );
}

#[path = "menu_gallery_tests.rs"]
mod gallery;
