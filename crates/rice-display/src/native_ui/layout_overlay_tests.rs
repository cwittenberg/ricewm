use super::*;
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, FileMetadata, RequestId, UiEvent, UiPatch, VfsResponse, VfsResult,
};
use rice_ui::{
    ChooserEffect, FileChooser, FileChooserEventResult, FileChooserMode, FileChooserViewIds,
    FileFilter, Insets, Place, WidgetKind, WidgetState, compose_file_chooser,
};

fn widget(value: u64) -> WidgetId {
    WidgetId::new(value).expect("nonzero widget")
}

fn create(id: u64, parent: Option<u64>, index: u32, kind: WidgetKind, text: &str) -> UiPatch {
    UiPatch::Create {
        id: widget(id),
        parent: parent.map(widget),
        index,
        kind: kind as u16,
        state: WidgetState::ENABLED.bits(),
        text: Some(text.to_owned()),
        icon: None,
    }
}

#[test]
fn breadcrumb_lays_out_ancestor_carets_and_current_location_on_one_line() {
    let app = AppId::new(1).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    let mut caret = create(4, Some(2), 1, WidgetKind::Label, "");
    if let UiPatch::Create { icon, .. } = &mut caret {
        *icon = Some(rice_ui::IconId::GoForward.wire_value());
    }
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Breadcrumb, ""),
            create(3, Some(2), 0, WidgetKind::Button, "Home"),
            caret,
            create(5, Some(2), 2, WidgetKind::Label, "Design standard"),
        ],
    })
    .expect("breadcrumb fixture");

    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 520,
                    height: 120,
                },
                Size {
                    width: 520,
                    height: 120,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("breadcrumb render");
    let text_rect = |needle: &str| {
        output.commands.iter().find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == needle => Some(*rect),
            _ => None,
        })
    };
    let home = text_rect("Home").expect("ancestor text");
    let current = text_rect("Design standard").expect("current text");
    assert_eq!(home.origin.y, current.origin.y);
    assert!(current.origin.x > home.origin.x);
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Icon {
            icon: rice_ui::IconId::GoForward,
            ..
        }
    )));
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4)),
        "the semantic caret is not interactive"
    );
}

#[test]
fn dialog_is_centered_without_squeezing_content_and_isolates_hit_targets() {
    let app = AppId::new(1).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    let mut frosted = Theme::ricewm_default();
    frosted.palette.surface = Color::rgba(28, 42, 48, 24);
    host.set_theme(frosted);
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Button, "Underlying"),
            create(3, Some(1), 1, WidgetKind::ScrollView, "0/100"),
            create(4, Some(3), 0, WidgetKind::Dialog, "Confirm"),
            create(7, Some(4), 0, WidgetKind::Column, ""),
            create(8, Some(4), 1, WidgetKind::Row, "panel:actions"),
            create(5, Some(7), 0, WidgetKind::Label, "This cannot be undone."),
            create(6, Some(8), 0, WidgetKind::Button, "Cancel"),
        ],
    })
    .expect("valid modal fixture");

    let output = host
        .render(
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
            .expect("fixed constraints"),
        )
        .expect("modal render");

    let dialog = output.dialog.as_ref().expect("dialog metadata");
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, radius: 0 }
                if *rect == Rect {
                    origin: Point { x: 0, y: 0 },
                    size: Size { width: 800, height: 600 },
                } && *color == Color::rgba(0, 0, 0, 104)
        )),
        "modal dialogs dim the parent content"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *rect == dialog.rect && color.channels()[3] >= 235
        )),
        "even a highly translucent theme retains a readable dialog surface"
    );

    let cancel = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(6))
        .expect("dialog action hit target");
    assert!(cancel.rect.origin.x > 100);
    assert!(cancel.rect.origin.y > 100);
    assert!(cancel.rect.size.width > 0);
    assert!(cancel.rect.size.height > 0);
    assert!(
        cancel.rect.size.width < 160,
        "dialog actions retain their intrinsic width: {:?}",
        cancel.rect
    );
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(2))
    );
    assert!(
        output.commands.iter().any(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "Underlying")
        )
    );
    let clip_end = output
        .commands
        .iter()
        .rposition(|command| matches!(command, DrawCommand::PopClip))
        .expect("scroll view clip");
    let dialog_text = output
        .commands
        .iter()
        .position(|command| {
            matches!(command, DrawCommand::Text { text, .. } if text == "This cannot be undone.")
        })
        .expect("dialog text");
    let title_rect = output
        .commands
        .iter()
        .rev()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Confirm" => Some(*rect),
            _ => None,
        })
        .expect("dialog title geometry");
    let body_rect = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "This cannot be undone." => Some(*rect),
            _ => None,
        })
        .expect("dialog body geometry");
    assert!(
        title_rect
            .origin
            .y
            .saturating_add_unsigned(title_rect.size.height)
            <= body_rect.origin.y,
        "dialog header must not paint through body content"
    );
    assert!(
        dialog_text > clip_end,
        "modal dialog must render outside its ancestor scroll clip"
    );
}

#[test]
fn settings_side_sheet_expands_against_the_right_viewport_edge() {
    let app = AppId::new(1).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Label, "Appearance"),
            create(3, Some(1), 1, WidgetKind::Dialog, "panel:side-sheet"),
            create(4, Some(3), 0, WidgetKind::Label, "Frosted Ice"),
            create(5, Some(3), 1, WidgetKind::Toggle, "Windows"),
        ],
    })
    .expect("side-sheet fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 1_200,
                    height: 800,
                },
                Size {
                    width: 1_200,
                    height: 800,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("side-sheet render");
    let dialog = output.dialog.expect("rendered side sheet");
    assert!(dialog.rect.origin.x > 300, "side sheet is right aligned");
    assert!(dialog.rect.size.width >= 560 && dialog.rect.size.width <= 780);
    assert!(
        dialog.rect.size.height > 700,
        "side sheet uses the viewport height"
    );
}

#[test]
fn dialog_choice_popup_renders_above_the_modal_surface() {
    let app = AppId::new(1).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Dialog, "Preferences"),
            create(3, Some(2), 0, WidgetKind::Column, ""),
            create(4, Some(3), 0, WidgetKind::Label, "Panel Position"),
            create(5, Some(3), 1, WidgetKind::Menu, "Right"),
            create(6, Some(5), 0, WidgetKind::MenuItem, "Left"),
            create(7, Some(5), 1, WidgetKind::MenuItem, "Center"),
            create(8, Some(5), 2, WidgetKind::MenuItem, "Right"),
        ],
    })
    .expect("dialog menu fixture");
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
    .expect("fixed constraints");
    let closed = host
        .render(app, window, constraints)
        .expect("closed dialog");
    let choice = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(5))
        .expect("choice target");
    host.pointer_event(
        app,
        window,
        PointerEventKind::Up,
        Point {
            x: choice.rect.origin.x + 2,
            y: choice.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open choice menu");
    let opened = host
        .render(app, window, constraints)
        .expect("opened dialog");
    let dialog_label = opened
        .commands
        .iter()
        .rposition(
            |command| matches!(command, DrawCommand::Text { text, .. } if text == "Panel Position"),
        )
        .expect("dialog label");
    let popup_item = opened
        .commands
        .iter()
        .position(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Center"))
        .expect("popup item");
    assert!(
        popup_item > dialog_label,
        "choice popup must paint above its dialog"
    );
    assert!(opened.popup.is_some());
    assert!(
        opened
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(7))
    );
}

#[test]
fn redmond_side_sheet_has_a_real_caption_readable_copy_and_clear_client_controls() {
    let app = AppId::new(98).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    let mut muted = create(
        6,
        Some(2),
        1,
        WidgetKind::Label,
        "Configure window edges independently of the base theme.",
    );
    if let UiPatch::Create { state, .. } = &mut muted {
        *state = (WidgetState::ENABLED | WidgetState::MUTED).bits();
    }
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Dialog,
                "panel:side-sheet:Window decoration preferences",
            ),
            create(3, Some(2), 0, WidgetKind::Row, "split:720"),
            create(
                4,
                Some(3),
                0,
                WidgetKind::Label,
                "Window decoration preferences",
            ),
            create(5, Some(3), 1, WidgetKind::Button, "Close"),
            muted,
        ],
    })
    .expect("Redmond side-sheet fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 960,
                    height: 640,
                },
                Size {
                    width: 960,
                    height: 640,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("Redmond side-sheet render");
    let caption = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text {
                rect, text, color, ..
            } if text == "Window decoration preferences"
                && *color == Color::opaque(255, 255, 255) =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("native side-sheet caption");
    let close = host
        .windows
        .get(&(app, window))
        .and_then(|window| window.geometry.get(&widget(5)))
        .copied()
        .expect("Close button geometry");
    assert!(
        close.origin.y
            >= caption
                .origin
                .y
                .saturating_add_unsigned(caption.size.height),
        "client Close button must remain below the native caption"
    );
    assert!(
        output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, color, .. }
                if text.starts_with("Configure window edges")
                    && *color == Color::opaque(0, 0, 0)
        )),
        "enabled explanatory copy uses ordinary Redmond window text"
    );
}

#[test]
fn redmond_theme_color_picker_keeps_done_and_caption_close_above_picker_hits() {
    let app = AppId::new(99).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Dialog, "Theme Configurator"),
            create(3, Some(2), 0, WidgetKind::Dialog, "Choose Theme Color"),
            create(4, Some(3), 0, WidgetKind::Label, "Bar background"),
            create(5, Some(3), 1, WidgetKind::ScrollBar, "color-picker:#3d3846"),
            create(6, Some(3), 2, WidgetKind::Row, "panel:actions"),
            create(7, Some(6), 0, WidgetKind::Button, "Done"),
        ],
    })
    .expect("theme picker fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 900,
                    height: 620,
                },
                Size {
                    width: 900,
                    height: 620,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("theme picker render");
    let done = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(7))
        .copied()
        .expect("Done target");
    let picker = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(5))
        .copied()
        .expect("color picker target");
    assert!(
        done.rect.intersection(picker.rect).is_none(),
        "Done must not overlap the color picker's value-change hit region"
    );
    let done_point = Point {
        x: done
            .rect
            .origin
            .x
            .saturating_add_unsigned(done.rect.size.width / 2),
        y: done
            .rect
            .origin
            .y
            .saturating_add_unsigned(done.rect.size.height / 2),
    };
    host.pointer_event(app, window, PointerEventKind::Down, done_point, 1, 0)
        .expect("Done press");
    let done_event = host
        .pointer_event(app, window, PointerEventKind::Up, done_point, 1, 0)
        .expect("Done release")
        .expect("Done activation");
    assert_eq!(done_event.target, widget(7));
    assert_eq!(done_event.event, UiEvent::Activate);

    let close = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "×" => Some(*rect),
            _ => None,
        })
        .expect("caption close");
    let close_point = Point {
        x: close.origin.x.saturating_add_unsigned(close.size.width / 2),
        y: close
            .origin
            .y
            .saturating_add_unsigned(close.size.height / 2),
    };
    assert_eq!(
        host.hit_test(app, window, close_point)
            .map(|target| target.widget),
        Some(widget(3)),
        "the painted picker close box must win hit testing"
    );
}

#[test]
fn choice_popup_overlays_content_matches_control_and_scrolls_with_wheel() {
    let app = AppId::new(41).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    let mut choice = create(4, Some(3), 0, WidgetKind::Menu, "Theme 1");
    if let UiPatch::Create { state, .. } = &mut choice {
        *state =
            (WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON).bits();
    }
    let mut patches = vec![
        create(1, None, 0, WidgetKind::Column, ""),
        create(2, Some(1), 0, WidgetKind::Dialog, "Theme Configurator"),
        create(3, Some(2), 0, WidgetKind::Column, "panel:clamp:420"),
        choice,
        create(40, Some(3), 1, WidgetKind::Label, "Content below"),
    ];
    for index in 0_u32..21 {
        patches.push(create(
            5 + u64::from(index),
            Some(4),
            index,
            WidgetKind::MenuItem,
            &format!("Theme {}", index + 1),
        ));
    }
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches,
    })
    .expect("choice fixture");
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
    .expect("fixed constraints");
    let closed = host
        .render(app, window, constraints)
        .expect("closed choice");
    let choice = closed
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(4))
        .copied()
        .expect("choice target");
    let content_before = closed.commands.iter().find_map(|command| match command {
        DrawCommand::Text { rect, text, .. } if text == "Content below" => Some(*rect),
        _ => None,
    });
    host.pointer_event(
        app,
        window,
        PointerEventKind::Up,
        Point {
            x: choice.rect.origin.x + 2,
            y: choice.rect.origin.y + 2,
        },
        1,
        0,
    )
    .expect("open choice");
    let opened = host
        .render(app, window, constraints)
        .expect("opened choice");
    let popup = opened.popup.as_ref().expect("popup").rect;
    assert_eq!(popup.size.width, choice.rect.size.width);
    let popup_commands =
        &opened.commands[opened.popup.as_ref().expect("popup metadata").command_start..];
    let visible = popup_commands
        .iter()
        .filter(|command| matches!(command, DrawCommand::Text { text, .. } if text.starts_with("Theme ")))
        .count();
    assert_eq!(visible, 20, "up to twenty popup commands are visible");
    let content_after = opened.commands.iter().find_map(|command| match command {
        DrawCommand::Text { rect, text, .. } if text == "Content below" => Some(*rect),
        _ => None,
    });
    assert_eq!(
        content_before, content_after,
        "popup must not reflow dialog content"
    );
    host.pointer_event(
        app,
        window,
        PointerEventKind::Wheel,
        Point {
            x: popup.origin.x + 12,
            y: popup.origin.y + 12,
        },
        5,
        0,
    )
    .expect("scroll choice");
    let scrolled = host
        .render(app, window, constraints)
        .expect("scrolled choice");
    let popup_commands = &scrolled.commands[scrolled
        .popup
        .as_ref()
        .expect("scrolled popup")
        .command_start..];
    assert!(
        !popup_commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Theme 1"))
    );
    assert!(
        popup_commands
            .iter()
            .any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Theme 9"))
    );
}

#[test]
fn settings_row_label_and_background_activate_the_associated_toggle() {
    let app = AppId::new(21).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Row, "split:660:control-for=4"),
            create(3, Some(2), 0, WidgetKind::Label, "Dark appearance"),
            create(5, Some(2), 1, WidgetKind::Row, "panel:actions"),
            create(6, Some(5), 0, WidgetKind::Label, "On"),
            create(4, Some(5), 1, WidgetKind::Toggle, "On"),
        ],
    })
    .expect("settings row fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 640,
                    height: 100,
                },
                Size {
                    width: 640,
                    height: 100,
                },
            )
            .expect("constraints"),
        )
        .expect("settings row render");
    let mut targets = output
        .hit_targets
        .iter()
        .filter(|target| target.widget == widget(4));
    let row_target = targets.next().expect("row proxy target");
    let switch_target = targets.next().expect("switch target");
    assert!(row_target.rect.size.width > switch_target.rect.size.width);
    let point = Point {
        x: row_target.rect.origin.x + 12,
        y: row_target.rect.origin.y + 12,
    };
    assert_eq!(
        host.hit_test(app, window, point)
            .map(|target| target.widget),
        Some(widget(4))
    );
}

#[test]
fn expansive_file_dialog_uses_a_desktop_workspace_footprint() {
    let app = AppId::new(22).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Dialog, "Open File"),
            UiPatch::Update {
                id: widget(2),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::EXPANSIVE).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
            create(3, Some(2), 0, WidgetKind::SplitPane, "split:230"),
            create(4, Some(3), 0, WidgetKind::List, ""),
            create(5, Some(3), 1, WidgetKind::List, ""),
        ],
    })
    .expect("expansive dialog fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 1_200,
                    height: 800,
                },
                Size {
                    width: 1_200,
                    height: 800,
                },
            )
            .expect("constraints"),
        )
        .expect("expansive dialog render");
    let dialog = output.dialog.expect("rendered dialog");
    assert_eq!(dialog.rect.size.width, 1_136);
    assert_eq!(dialog.rect.size.height, 736);
}

#[test]
fn expansive_dialog_never_overflows_a_compact_resizable_window() {
    let app = AppId::new(23).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Dialog, "Save File"),
            UiPatch::Update {
                id: widget(2),
                state: rice_native_app_protocol::PatchValue::Set(
                    (WidgetState::ENABLED | WidgetState::EXPANSIVE).bits(),
                ),
                text: rice_native_app_protocol::PatchValue::Keep,
                icon: rice_native_app_protocol::PatchValue::Keep,
            },
            create(3, Some(2), 0, WidgetKind::List, ""),
            create(4, Some(2), 1, WidgetKind::Button, "Save"),
        ],
    })
    .expect("compact dialog fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 360,
                    height: 280,
                },
                Size {
                    width: 360,
                    height: 280,
                },
            )
            .expect("compact constraints"),
        )
        .expect("compact dialog render");
    let dialog = output.dialog.expect("rendered compact dialog");
    assert!(dialog.rect.origin.x >= 0 && dialog.rect.origin.y >= 0);
    assert!(dialog.rect.size.width <= 360 && dialog.rect.size.height <= 280);
    assert!(
        output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4))
    );
}

#[test]
fn shared_file_chooser_fills_the_dialog_and_routes_the_hidden_checkbox() {
    let app = AppId::new(230).expect("app");
    let window = WindowId::new(1).expect("window");
    let ids = FileChooserViewIds::new(200, 204, 208, 230);
    let mut chooser = FileChooser::new(
        FileChooserMode::Open,
        "/root/",
        vec![FileFilter::all_files("All Files")],
        vec![Place::new("Home", "/root")],
    );
    let request = RequestId::new(1).expect("request");
    let _ = chooser.start(request);
    let _ = chooser.handle_response(&VfsResponse {
        request,
        result: VfsResult::Directory {
            entries: vec![DirectoryEntry {
                name: String::from("Documents"),
                metadata: FileMetadata {
                    kind: FileKind::Directory,
                    size: 0,
                    modified_millis: 1_777_593_600_000,
                    mode: 0o755,
                },
                mime_type: None,
            }],
            next_cursor: None,
        },
    });
    let mut patches = vec![
        create(1, None, 0, WidgetKind::Column, ""),
        create(200, Some(1), 0, WidgetKind::Dialog, "Open Document"),
    ];
    let mut chooser_patches = Vec::new();
    compose_file_chooser(&mut chooser_patches, &chooser, ids);
    patches.extend(
        chooser_patches
            .iter()
            .map(|patch| patch.to_protocol().expect("chooser protocol patch")),
    );

    let mut theme = Theme::familiar_light();
    theme.chrome_profile = rice_ui::ChromeProfile::Redmond98;
    let mut host = NativeUiHost::new(theme);
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches,
    })
    .expect("shared chooser fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 910,
                    height: 600,
                },
                Size {
                    width: 910,
                    height: 600,
                },
            )
            .expect("chooser constraints"),
        )
        .expect("chooser render");
    let native_window = host.windows.get(&(app, window)).expect("chooser window");
    let table = native_window
        .geometry
        .get(&widget(ids.file_list()))
        .copied()
        .expect("detail table geometry");
    let options = native_window
        .geometry
        .get(&widget(245))
        .copied()
        .expect("intrinsic options geometry");
    let dialog = output.dialog.expect("chooser dialog").rect;
    assert!(table.size.width > dialog.size.width / 2);
    assert!(table.size.height > options.size.height.saturating_mul(2));
    let places = native_window
        .geometry
        .get(&widget(242))
        .copied()
        .expect("places geometry");
    let navigation = native_window
        .geometry
        .get(&widget(240))
        .copied()
        .expect("navigation geometry");
    let directory = native_window
        .geometry
        .get(&widget(257))
        .copied()
        .expect("directory heading geometry");
    assert!(places.origin.y.abs_diff(navigation.origin.y) <= 4);
    assert!(places.size.height > navigation.size.height.saturating_mul(4));
    assert!(navigation.origin.x >= places.origin.x.saturating_add_unsigned(places.size.width));
    assert!(
        navigation
            .origin
            .y
            .saturating_add_unsigned(navigation.size.height)
            <= directory.origin.y
    );
    assert!(output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Documents"
    )));
    assert!(!output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. }
            if text.contains("file-detail") || text.contains("control-for=")
    )));
    for label in ["Name ↑", "Size", "Type", "Modified"] {
        let header = output
            .commands
            .iter()
            .find_map(|command| match command {
                DrawCommand::Text { rect, text, .. } if text == label => Some(*rect),
                _ => None,
            })
            .unwrap_or_else(|| panic!("painted {label} header"));
        assert!(header.size.width > 0 && header.size.height > 0);
        assert!(
            host.hit_test(
                app,
                window,
                Point {
                    x: header
                        .origin
                        .x
                        .saturating_add_unsigned(header.size.width / 2),
                    y: header
                        .origin
                        .y
                        .saturating_add_unsigned(header.size.height / 2),
                },
            )
            .is_some_and(|target| target.activatable),
            "{label} is a clickable header"
        );
    }
    let home_ink = output.commands.iter().find_map(|command| match command {
        DrawCommand::Text { text, color, .. } if text == "Home" => Some(*color),
        _ => None,
    });
    assert_eq!(
        home_ink,
        Some(theme.palette.selection_text),
        "selected Redmond Places text must be white on navy"
    );

    let close = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "×" => Some(*rect),
            _ => None,
        })
        .expect("painted dialog close box");
    let close_point = Point {
        x: close.origin.x.saturating_add_unsigned(close.size.width / 2),
        y: close
            .origin
            .y
            .saturating_add_unsigned(close.size.height / 2),
    };
    assert_eq!(
        host.hit_test(app, window, close_point)
            .map(|hit| hit.widget),
        Some(widget(ids.parent()))
    );
    host.pointer_event(app, window, PointerEventKind::Down, close_point, 1, 0)
        .expect("close press");
    let close_event = host
        .pointer_event(app, window, PointerEventKind::Up, close_point, 1, 0)
        .expect("close release")
        .expect("close activation");
    assert_eq!(
        chooser.handle_ui_event(ids, close_event.target.get(), &close_event.event, request,),
        FileChooserEventResult::Cancel
    );

    let hidden = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(ids.hidden()))
        .copied()
        .expect("hidden checkbox target");
    // Exercise the painted 13px Redmond indicator itself.  The old test used
    // the middle of the label row, so it could pass even when the visible box
    // was outside/behind another hit target in the composed dialog.
    let point = Point {
        x: hidden.rect.origin.x.saturating_add(7),
        y: hidden
            .rect
            .origin
            .y
            .saturating_add_unsigned(hidden.rect.size.height / 2),
    };
    assert_eq!(
        host.hit_test(app, window, point).map(|hit| hit.widget),
        Some(widget(ids.hidden()))
    );
    host.pointer_event(app, window, PointerEventKind::Down, point, 1, 0)
        .expect("checkbox press");
    let event = host
        .pointer_event(app, window, PointerEventKind::Up, point, 1, 0)
        .expect("checkbox pointer")
        .expect("checkbox event");
    assert_eq!(event.target, widget(ids.hidden()));
    assert_eq!(
        chooser.handle_ui_event(ids, event.target.get(), &event.event, request),
        FileChooserEventResult::Effect(ChooserEffect::Render)
    );
    assert!(chooser.show_hidden());

    // Starting navigation retains the current rows. Its busy marker lives in
    // the fixed folder heading, so the list viewport must not shrink and then
    // spring back when the VFS response arrives.
    let busy_request = RequestId::new(2).expect("busy request");
    assert!(matches!(
        chooser.activate_visible(0, busy_request),
        ChooserEffect::Request(_)
    ));
    let busy_app = AppId::new(231).expect("busy app");
    let mut busy_patches = vec![
        create(1, None, 0, WidgetKind::Column, ""),
        create(200, Some(1), 0, WidgetKind::Dialog, "Open Document"),
    ];
    let mut chooser_patches = Vec::new();
    compose_file_chooser(&mut chooser_patches, &chooser, ids);
    busy_patches.extend(
        chooser_patches
            .iter()
            .map(|patch| patch.to_protocol().expect("busy chooser patch")),
    );
    host.apply_patch(&UiPatchEnvelope {
        app: busy_app,
        window,
        sequence: 1,
        patches: busy_patches,
    })
    .expect("busy chooser fixture");
    let busy_output = host
        .render(
            busy_app,
            window,
            Constraints::new(
                Size {
                    width: 910,
                    height: 600,
                },
                Size {
                    width: 910,
                    height: 600,
                },
            )
            .expect("busy constraints"),
        )
        .expect("busy chooser render");
    let busy_table = host
        .windows
        .get(&(busy_app, window))
        .and_then(|native_window| native_window.geometry.get(&widget(ids.file_list())))
        .copied()
        .expect("busy table geometry");
    assert_eq!(
        busy_table, table,
        "loading must not resize the list viewport"
    );
    assert!(busy_output.commands.iter().any(|command| matches!(
        command,
        DrawCommand::Text { text, .. } if text == "Loading folder…"
    )));
    assert!(
        busy_output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Text { text, color, .. }
                if text == "Documents" && *color == theme.palette.selection_text
        )),
        "selected detail-row text must use Redmond highlight ink"
    );
}

#[test]
fn shared_file_chooser_thumbnail_surface_selects_and_double_click_opens() {
    let app = AppId::new(230).unwrap();
    let window = WindowId::new(1).unwrap();
    let ids = FileChooserViewIds::new(200, 204, 208, 230);
    let request = RequestId::new(1).unwrap();
    for theme in [Theme::familiar_light(), Theme::familiar_dark()] {
        let mut chooser = FileChooser::new(
            FileChooserMode::Open,
            "/root/",
            vec![FileFilter::new("Pictures", &["png"])],
            vec![Place::new("Home", "/root")],
        );
        chooser.start(request);
        chooser.handle_response(&VfsResponse {
            request,
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: "photo.png".into(),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 100,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some("image/png".into()),
                }],
                next_cursor: None,
            },
        });
        chooser.set_show_details(false);
        let mut patches = vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(200, Some(1), 0, WidgetKind::Dialog, "Choose Wallpaper"),
        ];
        let mut composed = Vec::new();
        compose_file_chooser(&mut composed, &chooser, ids);
        patches.extend(composed.iter().map(|patch| patch.to_protocol().unwrap()));
        let mut host = NativeUiHost::new(theme);
        host.apply_patch(&UiPatchEnvelope {
            app,
            window,
            sequence: 1,
            patches,
        })
        .unwrap();
        host.render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 873,
                    height: 627,
                },
                Size {
                    width: 873,
                    height: 627,
                },
            )
            .unwrap(),
        )
        .unwrap();
        let geometry = &host.windows.get(&(app, window)).unwrap().geometry;
        let cell = geometry[&widget(10_230)];
        let image = geometry[&widget(20_230)];
        let hidden = geometry[&widget(ids.hidden())];
        let filter = geometry[&widget(ids.filter_menu())];
        let list = geometry[&widget(ids.file_list())];
        assert!(hidden.size.height > 0 && filter.size.width > 0, "filter menu and hidden-file toggle remain visible");
        assert!(
            list.size.height > 250,
            "files retain most of the dialog height: {list:?}"
        );
        let points = [
            Point {
                x: image.origin.x + (image.size.width / 2) as i32,
                y: image.origin.y + (image.size.height / 2) as i32,
            },
            Point {
                x: cell.origin.x + 1,
                y: cell.origin.y + 1,
            },
            Point {
                x: cell.origin.x + cell.size.width as i32 - 2,
                y: cell.origin.y + 2,
            },
        ];
        for point in points {
            assert_eq!(
                host.hit_test(app, window, point).map(|hit| hit.widget),
                Some(widget(ids.entry(0))),
                "whole thumbnail at {point:?}"
            );
            let event = host
                .pointer_event(app, window, PointerEventKind::Down, point, 1, 0)
                .unwrap()
                .unwrap();
            assert_eq!(
                chooser.handle_ui_event(ids, event.target.get(), &event.event, request),
                FileChooserEventResult::Effect(ChooserEffect::Render)
            );
            let event = host
                .pointer_event(app, window, PointerEventKind::Up, point, 1, 0)
                .unwrap()
                .unwrap();
            chooser.handle_ui_event(ids, event.target.get(), &event.event, request);
            assert_eq!(
                chooser.handle_ui_event(ids, ids.accept(), &UiEvent::Activate, request),
                FileChooserEventResult::Effect(ChooserEffect::Accepted("/root/photo.png".into()))
            );
        }
        let event = host
            .pointer_event(app, window, PointerEventKind::Down, points[0], 6, 0)
            .unwrap()
            .unwrap();
        assert_eq!(
            chooser.handle_ui_event(ids, event.target.get(), &event.event, request),
            FileChooserEventResult::Effect(ChooserEffect::Accepted("/root/photo.png".into()))
        );
    }
}

#[test]
fn labeled_panels_allocate_distinct_header_and_body_rows() {
    let app = AppId::new(24).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::Popover, "Component metadata"),
            create(
                3,
                Some(2),
                0,
                WidgetKind::Label,
                "Tree view · Collection · Primitive",
            ),
            create(4, Some(1), 1, WidgetKind::GroupBox, "Export options"),
            create(5, Some(4), 0, WidgetKind::CheckBox, "Include comments"),
        ],
    })
    .expect("labeled panels fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 520,
                    height: 260,
                },
                Size {
                    width: 520,
                    height: 260,
                },
            )
            .expect("constraints"),
        )
        .expect("labeled panels render");
    let text_rect = |needle: &str| {
        output.commands.iter().find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == needle => Some(*rect),
            _ => None,
        })
    };
    let title = text_rect("Component metadata").expect("popover title");
    let body = text_rect("Tree view · Collection · Primitive").expect("popover body");
    assert!(body.origin.y >= title.origin.y.saturating_add_unsigned(title.size.height));
    assert!(text_rect("Export options").is_some());
    assert!(text_rect("Include comments").is_some());
    assert!(
        !output.commands.iter().any(|command| matches!(
            command,
            DrawCommand::Fill { rect, color, .. }
                if *color == host.theme().palette.surface
                    && rect.size.width < 180
                    && rect.size.height >= u32::from(host.theme().metrics.control_height)
        )),
        "the group legend must interrupt measured frame segments, not paint an opaque patch"
    );
    assert!(
        !output
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4))
    );
}

#[test]
fn menu_descendants_and_hidden_context_menus_never_reserve_document_space() {
    let app = AppId::new(28).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::ScrollView, "0/0"),
            create(
                2,
                Some(1),
                0,
                WidgetKind::Column,
                "panel:clamp:920:settings-page:intrinsic-flow",
            ),
            create(3, Some(2), 0, WidgetKind::Label, "Menu heading"),
            create(4, Some(2), 1, WidgetKind::MenuBar, ""),
            create(5, Some(4), 0, WidgetKind::Menu, "File"),
            create(6, Some(5), 0, WidgetKind::MenuItem, "Open"),
            create(7, Some(5), 1, WidgetKind::MenuItem, "Export"),
            create(8, Some(7), 0, WidgetKind::Menu, "Export"),
            create(9, Some(8), 0, WidgetKind::MenuItem, "PDF document"),
            create(10, Some(2), 2, WidgetKind::Label, "After menu"),
            create(11, Some(2), 3, WidgetKind::MenuBar, "context"),
            create(12, Some(11), 0, WidgetKind::Menu, "Selection"),
            create(13, Some(12), 0, WidgetKind::MenuItem, "Copy"),
            create(14, Some(12), 1, WidgetKind::MenuItem, "Paste"),
            create(15, Some(2), 4, WidgetKind::Label, "After context"),
        ],
    })
    .expect("menu flow fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 620,
                    height: 300,
                },
                Size {
                    width: 620,
                    height: 300,
                },
            )
            .expect("constraints"),
        )
        .expect("menu flow render");
    let text_y = |needle: &str| {
        output.commands.iter().find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == needle => Some(rect.origin.y),
            _ => None,
        })
    };
    let heading = text_y("Menu heading").expect("heading");
    let file = text_y("File").expect("menu heading");
    let after_menu = text_y("After menu").expect("after menu");
    let after_context = text_y("After context").expect("after hidden context menu");
    assert!(file.saturating_sub(heading) < 64);
    assert!(after_menu.saturating_sub(file) < 64);
    assert!(after_context.saturating_sub(after_menu) < 64);
}

#[test]
fn scroll_view_reserves_a_gutter_between_content_and_scrollbar() {
    let app = AppId::new(25).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    let mut patches = vec![
        create(1, None, 0, WidgetKind::Column, ""),
        create(2, Some(1), 0, WidgetKind::ScrollView, "0/640"),
        create(3, Some(2), 0, WidgetKind::Column, ""),
    ];
    for index in 0..16_u64 {
        patches.push(create(
            10 + index,
            Some(3),
            u32::try_from(index).expect("index"),
            WidgetKind::Label,
            "Scrollable content",
        ));
    }
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches,
    })
    .expect("scroll fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 420,
                    height: 220,
                },
                Size {
                    width: 420,
                    height: 220,
                },
            )
            .expect("constraints"),
        )
        .expect("scroll render");
    let track = output
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(2))
        .expect("scrollbar track")
        .rect;
    let content = output.commands.iter().find_map(|command| match command {
        DrawCommand::Text { rect, text, .. } if text == "Scrollable content" => Some(*rect),
        _ => None,
    });
    let content = content.expect("content text");
    let content_right = content.origin.x.saturating_add_unsigned(content.size.width);
    assert!(content_right.saturating_add(12) <= track.origin.x);
    let scroll_rect = host.windows[&(app, window)].geometry[&widget(2)];
    let inner_viewport = scroll_rect.inset(Insets::all(u32::from(host.theme.spacing.control_y)));
    let inner_clip = output.commands.iter().position(
        |command| matches!(command, DrawCommand::PushClip { rect } if *rect == inner_viewport),
    );
    let content_text = output.commands.iter().position(
        |command| matches!(command, DrawCommand::Text { text, .. } if text == "Scrollable content"),
    );
    assert!(
        inner_clip
            .zip(content_text)
            .is_some_and(|(clip, text)| clip < text),
        "scroll children must paint inside the inset viewport clip",
    );
    assert!(inner_viewport.origin.y > scroll_rect.origin.y);
    assert!(
        inner_viewport
            .origin
            .y
            .saturating_add_unsigned(inner_viewport.size.height)
            < scroll_rect
                .origin
                .y
                .saturating_add_unsigned(scroll_rect.size.height)
    );
}

#[test]
fn scroll_view_anchor_brings_a_nested_section_to_the_reading_origin() {
    let app = AppId::new(1).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::ScrollView, "anchor:5"),
            create(2, Some(1), 0, WidgetKind::Column, "panel:intrinsic-flow"),
            create(3, Some(2), 0, WidgetKind::GroupBox, "First"),
            create(4, Some(3), 0, WidgetKind::Label, "panel:height:180"),
            create(5, Some(2), 1, WidgetKind::GroupBox, "Second"),
            create(6, Some(5), 0, WidgetKind::Label, "Anchored content"),
            create(7, Some(2), 2, WidgetKind::GroupBox, "Third"),
            create(8, Some(7), 0, WidgetKind::Label, "panel:height:180"),
        ],
    })
    .expect("anchored scroll fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 360,
                    height: 180,
                },
                Size {
                    width: 360,
                    height: 180,
                },
            )
            .expect("fixed constraints"),
        )
        .expect("anchored scroll render");
    let anchored = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. } if text == "Second" => Some(*rect),
            _ => None,
        })
        .expect("anchored section legend");
    assert!(
        anchored.origin.y < 48,
        "anchor remained at {}",
        anchored.origin.y
    );
}

#[test]
fn submenu_keeps_parent_popup_open_and_uses_an_adjacent_popup() {
    let app = AppId::new(26).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::MenuBar, ""),
            create(3, Some(2), 0, WidgetKind::Menu, "File"),
            create(4, Some(3), 0, WidgetKind::MenuItem, "Open\tAlt+O"),
            create(5, Some(3), 1, WidgetKind::MenuItem, "Export"),
            create(6, Some(5), 0, WidgetKind::Menu, "Export"),
            create(7, Some(6), 0, WidgetKind::MenuItem, "PDF document"),
            create(8, Some(6), 1, WidgetKind::MenuItem, "Markdown"),
        ],
    })
    .expect("menu fixture");
    let constraints = Constraints::new(
        Size {
            width: 640,
            height: 400,
        },
        Size {
            width: 640,
            height: 400,
        },
    )
    .expect("constraints");
    let initial = host
        .render(app, window, constraints)
        .expect("initial render");
    let heading = initial
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(3))
        .expect("File menu")
        .rect;
    let point = Point {
        x: heading
            .origin
            .x
            .saturating_add_unsigned(heading.size.width / 2),
        y: heading
            .origin
            .y
            .saturating_add_unsigned(heading.size.height / 2),
    };
    host.pointer_event(app, window, PointerEventKind::Down, point, 1, 0)
        .expect("menu down");
    host.pointer_event(app, window, PointerEventKind::Up, point, 1, 0)
        .expect("menu up");
    let parent = host.render(app, window, constraints).expect("parent popup");
    let parent_rect = parent.popup.as_ref().expect("parent popup metadata").rect;
    let export = parent
        .hit_targets
        .iter()
        .find(|target| target.widget == widget(5))
        .expect("Export item")
        .rect;
    let export_point = Point {
        x: export
            .origin
            .x
            .saturating_add_unsigned(export.size.width / 2),
        y: export
            .origin
            .y
            .saturating_add_unsigned(export.size.height / 2),
    };
    host.pointer_event(app, window, PointerEventKind::Move, export_point, 0, 0)
        .expect("submenu hover");
    let nested = host.render(app, window, constraints).expect("nested popup");
    let nested_rect = nested.popup.expect("nested popup metadata").rect;
    assert!(nested_rect.size.width > parent_rect.size.width);
    assert!(
        nested
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(4))
    );
    assert!(
        nested
            .hit_targets
            .iter()
            .any(|target| target.widget == widget(7))
    );
}

#[test]
fn notification_is_a_bounded_overlay_instead_of_a_page_wide_row() {
    let app = AppId::new(27).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app,
        window,
        sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::IconGrid, ""),
            create(
                3,
                Some(1),
                1,
                WidgetKind::Notification,
                "The new desktop item name is unchanged",
            ),
        ],
    })
    .expect("notification fixture");
    let output = host
        .render(
            app,
            window,
            Constraints::new(
                Size {
                    width: 1_200,
                    height: 800,
                },
                Size {
                    width: 1_200,
                    height: 800,
                },
            )
            .expect("constraints"),
        )
        .expect("notification render");
    let notice = output
        .commands
        .iter()
        .find_map(|command| match command {
            DrawCommand::Text { rect, text, .. }
                if text == "The new desktop item name is unchanged" =>
            {
                Some(*rect)
            }
            _ => None,
        })
        .expect("notification text");
    assert!(
        notice.size.width <= 420,
        "notice width was {}",
        notice.size.width
    );
    assert!(
        notice.origin.x < 440,
        "notice left its bounded corner overlay"
    );
}

#[test]
fn menu_tooltips_are_passive_and_only_real_submenus_receive_arrows() {
    let app = AppId::new(94).expect("app");
    let window = WindowId::new(1).expect("window");
    let mut host = NativeUiHost::default();
    host.apply_patch(&UiPatchEnvelope {
        app, window, sequence: 1,
        patches: vec![
            create(1, None, 0, WidgetKind::Column, ""),
            create(2, Some(1), 0, WidgetKind::MenuBar, ""),
            create(3, Some(2), 0, WidgetKind::Menu, "Number format"),
            create(4, Some(3), 0, WidgetKind::MenuItem, "General"),
            create(5, Some(4), 0, WidgetKind::Tooltip, "General format help"),
            create(6, Some(3), 1, WidgetKind::MenuItem, "Percentage"),
            create(7, Some(6), 0, WidgetKind::Tooltip, "Percentage format help"),
            create(8, Some(3), 2, WidgetKind::MenuItem, "More formats"),
            create(9, Some(8), 0, WidgetKind::Menu, "More formats submenu"),
            create(10, Some(9), 0, WidgetKind::MenuItem, "Custom"),
            create(11, Some(9), 1, WidgetKind::Tooltip, "Nested passive help"),
            create(12, Some(3), 3, WidgetKind::Tooltip, "Number format passive help"),
        ],
    }).expect("menu fixture");
    let constraints = Constraints::new(Size { width: 800, height: 500 }, Size { width: 800, height: 500 }).expect("constraints");
    let initial = host.render(app, window, constraints).expect("render");
    let heading = initial.hit_targets.iter().find(|target| target.widget == widget(3)).expect("menu heading").rect;
    let point = Point { x: heading.origin.x + 8, y: heading.origin.y + 8 };
    for kind in [PointerEventKind::Down, PointerEventKind::Up] {
        host.pointer_event(app, window, kind, point, 1, 0).expect("menu click");
    }
    host.pointer_event(app, window, PointerEventKind::Move, Point { x: 799, y: 499 }, 0, 0).expect("leave hover");
    let popup = host.render(app, window, constraints).expect("popup");
    assert!(!popup.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text.contains("help"))), "passive tooltip must not become a menu row");
    let arrows: Vec<_> = popup.commands.iter().filter_map(|command| match command {
        DrawCommand::Icon { icon: rice_ui::IconId::SubmenuRight | rice_ui::IconId::SubmenuLeft, rect, .. } => Some(*rect),
        _ => None,
    }).collect();
    assert_eq!(arrows.len(), 1, "only the genuine submenu gets a disclosure");
    let more = popup.hit_targets.iter().find(|target| target.widget == widget(8)).expect("submenu item").rect;
    assert!(more.contains(arrows[0].origin));
    let point = Point { x: more.origin.x + 16, y: more.origin.y + 8 };
    host.pointer_event(app, window, PointerEventKind::Move, point, 0, 0).expect("submenu hover");
    let nested = host.render(app, window, constraints).expect("nested popup");
    assert!(nested.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Custom")));
    assert!(!nested.commands.iter().any(|command| matches!(command, DrawCommand::Text { text, .. } if text == "Nested passive help")));
}
