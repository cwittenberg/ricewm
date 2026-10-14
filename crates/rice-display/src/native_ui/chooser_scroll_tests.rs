//! Real chooser composition must expose every row through the native wheel path.
use super::*;
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, FileMetadata, RequestId, UiPatch, VfsResponse, VfsResult,
};
use rice_ui::{
    ChooserEffect, FileChooser, FileChooserEventResult, FileChooserMode, FileChooserViewIds,
    FileFilter, Insets, Place, WidgetKind, WidgetState, compose_file_chooser,
};
fn widget(id: u64) -> WidgetId {
    WidgetId::new(id).unwrap()
}
#[test]
fn default_size_file_chooser_scrolls_last_wrapped_detail_row_into_view() {
    exercise_wrapped_file_chooser(5);
}

#[test]
fn default_size_file_chooser_preserves_native_scroll_beyond_nominal_row_height() {
    exercise_wrapped_file_chooser(17);
}

fn exercise_wrapped_file_chooser(count: usize) {
    let app = AppId::new(331).unwrap();
    let window = WindowId::new(1).unwrap();
    let ids = FileChooserViewIds::new(100, 104, 108, 130);
    let mut chooser = FileChooser::new(
        FileChooserMode::Open,
        "/Documents/",
        vec![FileFilter::all_files("All Files")],
        vec![
            Place::new("Home", "/"),
            Place::new("Documents", "/Documents"),
        ],
    );
    let request = RequestId::new(1).unwrap();
    let _ = chooser.start(request);
    let _ = chooser.handle_response(&VfsResponse {
        request,
        result: VfsResult::Directory {
            entries: (0..count)
                .map(|i| DirectoryEntry {
                    name: format!("source-presentation-{i:02}.pptx"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 5632,
                        modified_millis: 1_777_593_600_000,
                        mode: 0o644,
                    },
                    mime_type: Some(
                        "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                            .into(),
                    ),
                })
                .collect(),
            next_cursor: None,
        },
    });
    let constraints = Constraints::new(
        Size {
            width: 960,
            height: 640,
        },
        Size {
            width: 960,
            height: 640,
        },
    )
    .unwrap();
    let mut host = NativeUiHost::new(Theme::ricewm_default());
    let mut last_output = None;
    for sequence in 1..=32 {
        // Compose exactly the shared chooser; protocol remove/create mirrors app rerender.
        let mut patches = Vec::new();
        if sequence > 1 {
            patches.push(UiPatch::Remove { id: widget(1) });
        }
        for (id, parent, kind, text) in [
            (1, None, WidgetKind::Column, ""),
            (100, Some(1), WidgetKind::Dialog, "Open presentation"),
        ] {
            patches.push(UiPatch::Create {
                id: widget(id),
                parent: parent.map(widget),
                index: 0,
                kind: kind as u16,
                state: WidgetState::ENABLED.bits(),
                text: Some(text.into()),
                icon: None,
            });
        }
        let mut composed = Vec::new();
        compose_file_chooser(&mut composed, &chooser, ids);
        patches.extend(composed.iter().map(|p| p.to_protocol().unwrap()));
        host.apply_patch(&UiPatchEnvelope {
            app,
            window,
            sequence,
            patches,
        })
        .unwrap();
        let output = host.render(app, window, constraints).unwrap();
        let native = &host.windows[&(app, window)];
        let table = native.tree.get(widget(ids.file_list())).unwrap();
        let scroll = table.parent.unwrap();
        let viewport =
            native.geometry[&scroll].inset(Insets::all(u32::from(host.theme.spacing.control_y)));
        let last = *table.children().last().unwrap();
        let row = native.geometry[&last];
        if sequence == 1 {
            assert!(
                row.origin.y.saturating_add_unsigned(row.size.height)
                    > viewport
                        .origin
                        .y
                        .saturating_add_unsigned(viewport.size.height),
                "fixture must overflow: row={row:?}, viewport={viewport:?}"
            );
        }
        if sequence == 32 {
            assert!(
                row.origin.y >= viewport.origin.y,
                "last row must not overscroll"
            );
            assert!(
                row.origin.y.saturating_add_unsigned(row.size.height)
                    <= viewport
                        .origin
                        .y
                        .saturating_add_unsigned(viewport.size.height),
                "last row bottom {:?} must be reachable inside {:?}",
                row,
                viewport
            );
            let target = output
                .hit_targets
                .iter()
                .find(|t| t.widget == widget(ids.entry(u64::try_from(count - 1).unwrap())))
                .expect("last file hit target");
            let point = Point {
                x: target.rect.origin.x + 10,
                y: target.rect.origin.y + 10,
            };
            assert_eq!(
                host.hit_test(app, window, point).map(|t| t.widget),
                Some(widget(ids.entry(u64::try_from(count - 1).unwrap())))
            );
            let event = host.pointer_event(app, window, PointerEventKind::Down, point, 1, 0)
                .unwrap().expect("last row selection event");
            assert_eq!(chooser.handle_ui_event(ids, event.target.get(), &event.event, request),
                FileChooserEventResult::Effect(ChooserEffect::Render));
            assert_eq!(chooser.submit(request), ChooserEffect::Accepted(
                format!("/Documents/source-presentation-{:02}.pptx", count - 1)));
            last_output = Some(output);
            break;
        }
        let event = host
            .pointer_event(
                app,
                window,
                PointerEventKind::Wheel,
                Point {
                    x: viewport.origin.x + 20,
                    y: viewport.origin.y + i32::try_from(viewport.size.height / 2).unwrap(),
                },
                5,
                0,
            )
            .unwrap()
            .expect("wheel scroll event");
        let result = chooser.handle_ui_event(ids, event.target.get(), &event.event, request);
        assert_eq!(
            result,
            FileChooserEventResult::Effect(ChooserEffect::Render),
            "{event:?}"
        );
    }
    assert!(last_output.is_some());
}

#[test]
fn save_chooser_format_popup_keeps_all_five_labels_readable_and_selectable() {
    let formats = [
        ("PowerPoint Presentation (.pptx)", "pptx"),
        ("OpenDocument Presentation (.odp)", "odp"),
        ("PowerPoint Template (.potx)", "potx"),
        ("OpenDocument Template (.otp)", "otp"),
        ("PowerPoint Show (.ppsx)", "ppsx"),
    ];
    for width in [800, 960] {
        let app = AppId::new(332).unwrap();
        let window = WindowId::new(1).unwrap();
        let ids = FileChooserViewIds::new(100, 104, 108, 130);
        let mut chooser = FileChooser::new(
            FileChooserMode::Save,
            "/Documents/Presentation.pptx",
            formats.iter().map(|(label, ext)| FileFilter::new(*label, &[*ext])).collect(),
            vec![Place::new("Documents", "/Documents")],
        );
        let request = RequestId::new(1).unwrap();
        chooser.start(request);
        chooser.handle_response(&VfsResponse { request, result: VfsResult::Directory {
            entries: vec![], next_cursor: None,
        }});
        let size = Size { width, height: 640 };
        let constraints = Constraints { min: size, max: size };
        let mut host = NativeUiHost::new(Theme::ricewm_default());
        for selected in 0..formats.len() {
            let mut patches = Vec::new();
            if selected > 0 { patches.push(UiPatch::Remove { id: widget(1) }); }
            for (id, parent, kind, text) in [
                (1, None, WidgetKind::Column, ""),
                (100, Some(1), WidgetKind::Dialog, "Save presentation"),
            ] {
                patches.push(UiPatch::Create { id: widget(id), parent: parent.map(widget),
                    index: 0, kind: kind as u16, state: WidgetState::ENABLED.bits(),
                    text: Some(text.into()), icon: None });
            }
            let mut composed = Vec::new();
            compose_file_chooser(&mut composed, &chooser, ids);
            patches.extend(composed.iter().map(|p| p.to_protocol().unwrap()));
            host.apply_patch(&UiPatchEnvelope { app, window, sequence: selected as u64 + 1, patches }).unwrap();
            let closed = host.render(app, window, constraints).unwrap();
            let native = &host.windows[&(app, window)];
            let menu = native.geometry[&widget(ids.filter_menu())];
            let hidden = native.geometry[&widget(ids.hidden())];
            assert!(hidden.origin.y >= menu.origin.y + menu.size.height as i32);
            assert!(menu.size.width >= 280, "full format selector at width {width}: {menu:?}");
            assert!(closed.commands.iter().any(|command| matches!(command,
                DrawCommand::Text {text, rect, ..} if text == "File type" && rect.size.width >= 55)));
            let point = Point { x: menu.origin.x + 8, y: menu.origin.y + 8 };
            host.pointer_event(app, window, PointerEventKind::Down, point, 1, 0).unwrap();
            host.pointer_event(app, window, PointerEventKind::Up, point, 1, 0).unwrap();
            let popup = host.render(app, window, constraints).unwrap();
            for (label, _) in formats {
                assert!(popup.commands.iter().any(|command| matches!(command,
                    DrawCommand::Text {text, rect, ..} if text == label && rect.size.width >= 260)),
                    "full popup choice {label} at width {width}");
            }
            let next = (selected + 1) % formats.len();
            let target = popup.hit_targets.iter().find(|target| target.widget == widget(ids.filter(next as u64))).unwrap();
            let point = Point { x: target.rect.origin.x + 8, y: target.rect.origin.y + 8 };
            host.pointer_event(app, window, PointerEventKind::Down, point, 1, 0).unwrap();
            let activated = host.pointer_event(app, window, PointerEventKind::Up, point, 1, 0).unwrap().unwrap();
            assert_eq!(activated.target, widget(ids.filter(next as u64)));
            assert_eq!(activated.event, UiEvent::Activate);
            chooser.handle_ui_event(ids, activated.target.get(), &activated.event, request);
            assert_eq!(chooser.filter_index(), next);
            assert_eq!(chooser.filename(), format!("Presentation.{}", formats[next].1));
            assert!(!host.render(app, window, constraints).unwrap().hit_targets.iter().any(|target| target.widget == widget(ids.filter(next as u64))));
        }
    }
}
