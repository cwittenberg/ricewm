use super::files_menu::menu_widget;
use super::files_view::{
    CONTENT_NOTICE, DIRECTORY, DIRECTORY_SCROLL, MENU_BAR, NAV_TOOLBAR, PLACES, PREVIEW_PANEL,
    STATUS_PANEL, TAB_PAGE, TABS, UP, ZOOM_SLIDER, widget_id,
};
use super::*;
use rice_native_app_protocol::{
    DirectoryEntry, FileMetadata, PointerEventKind, SettingsOperation, SettingsRequest, UiPatch,
    VfsError, VfsErrorCode, VfsResponse, VfsResult,
};
use rice_ui::WidgetState;

pub(super) fn app() -> AppId {
    AppId::new(7).expect("test app")
}

pub(super) fn request(value: u64) -> RequestId {
    RequestId::new(value).expect("test request")
}

fn metadata(kind: FileKind, size: u64) -> FileMetadata {
    FileMetadata {
        kind,
        size,
        modified_millis: 10,
        mode: 0o644,
    }
}

pub(super) fn event(target: u64, event: UiEvent) -> UiEventEnvelope {
    UiEventEnvelope {
        app: app(),
        window: window_id(WINDOW),
        target: widget_id(target),
        event,
    }
}

pub(super) fn directory_response(request: u64) -> VfsResponse {
    VfsResponse {
        request: self::request(request),
        result: VfsResult::Directory {
            entries: vec![
                DirectoryEntry {
                    name: String::from("file10.txt"),
                    metadata: metadata(FileKind::File, 10),
                    mime_type: None,
                },
                DirectoryEntry {
                    name: String::from("Folder"),
                    metadata: metadata(FileKind::Directory, 0),
                    mime_type: None,
                },
                DirectoryEntry {
                    name: String::from("file2.txt"),
                    metadata: metadata(FileKind::File, 2),
                    mime_type: None,
                },
            ],
            next_cursor: None,
        },
    }
}

#[test]
fn reload_is_the_first_visible_navigation_toolbar_action() {
    let mut patches = Vec::new();
    files_view::toolbar_patches(&mut patches, false, "", false);
    let first_action = patches.iter().find_map(|patch| match patch {
        rice_ui::Patch::Create { widget, parent, .. }
            if *parent == Some(widget_id(NAV_TOOLBAR))
                && widget.kind == rice_ui::WidgetKind::Button =>
        {
            Some(widget.id)
        }
        _ => None,
    });
    assert_eq!(first_action, Some(widget_id(RELOAD)));
}

#[test]
fn presented_paths_keep_the_tail_while_the_editable_location_stays_complete() {
    let location = "/usr/local/a/very/long/prefix/usr/share/hello.mp4";
    assert_eq!(
        files_view::tail_elided_path(location, 20),
        "…/share/hello.mp4"
    );

    let (_, messages) = RiceFiles::launch_at(app(), &[location.to_owned()], request(1))
        .expect("long absolute location launches");
    let [
        Message::UiPatch(ui_update),
        Message::VfsRequest(_),
        Message::DataProviderRequest(_),
        Message::SettingsRequest(_),
        Message::HostRequest(_),
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::RiceBusSubscribe { .. },
            ..
        }),
    ] = messages.as_slice()
    else {
        panic!("launch emits UI, directory, and mount-catalog requests");
    };
    let text_for = |target| {
        ui_update
            .patches
            .iter()
            .find_map(|candidate| match candidate {
                UiPatch::Create {
                    id,
                    text: Some(text),
                    ..
                } if *id == widget_id(target) => Some(text.as_str()),
                _ => None,
            })
    };

    assert_eq!(text_for(BREADCRUMB), Some(location));
    for target in [
        files_view::pane_ids(PaneSlot::Primary).location,
        files_view::LOCATION_STATUS,
    ] {
        let presented = text_for(target).expect("presented location exists");
        assert!(presented.starts_with('…'));
        assert!(presented.ends_with("/share/hello.mp4"));
        assert!(!presented.contains("/usr/local"));
    }
}

#[test]
fn directory_response_sort_selection_view_and_navigation_are_monotonic() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let update = actor
        .handle_vfs_response(&directory_response(1))
        .expect("directory response");
    let [Message::UiPatch(update)] = update.as_slice() else {
        panic!("directory emits UI");
    };
    assert_eq!(update.sequence, 2);
    assert!(update.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(DIRECTORY) && *kind == WidgetKind::IconGrid as u16
        )
    }));
    assert_eq!(
        actor
            .runtime_for_test()
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["Folder", "file2.txt", "file10.txt"]
    );

    let selected = actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select");
    let [Message::UiPatch(selected), Message::VfsRequest(preview)] = selected.as_slice() else {
        panic!("selection emits UI");
    };
    assert_eq!(preview.request, request(2));
    assert_eq!(selected.sequence, 3);
    assert_eq!(actor.pane().expect("pane").selection().count(), 1);
    assert!(
        selected.patches.iter().any(|patch| {
            matches!(
                patch,
                UiPatch::Create { id, state, .. }
                    if *id == widget_id(ENTRY_BASE + 1)
                        && state & WidgetState::FOCUSED.bits() != 0
            )
        }),
        "the selected file must retain keyboard focus after a full rebuild"
    );

    let details = actor
        .handle_event(&event(LIST_VIEW, UiEvent::Activate), request(3))
        .expect("details");
    assert_eq!(actor.pane().expect("pane").view, ViewMode::Details);
    let [Message::UiPatch(details)] = details.as_slice() else {
        panic!("details emits UI");
    };
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(DIRECTORY) && *kind == WidgetKind::Table as u16
        )
    }));

    let navigation = actor
        .handle_event(&event(UP, UiEvent::Activate), request(4))
        .expect("up");
    assert!(matches!(
        navigation.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("up response");
    assert_eq!(actor.location().as_str(), "/");
}

#[test]
fn new_navigation_supersedes_a_stuck_directory_request_and_ignores_its_late_response() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let replacement = actor
        .begin_navigation(
            NavigationCommand::Enter(String::from("/root/Downloads")),
            request(2),
        )
        .expect("replacement navigation");
    assert!(matches!(
        replacement.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: id,
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if *id == request(2) && path == "/root/Downloads"
    ));
    assert!(
        actor
            .handle_vfs_response(&directory_response(1))
            .expect("late retired response")
            .is_empty()
    );
    assert_eq!(
        actor
            .runtime_for_test()
            .pending_directory
            .as_ref()
            .map(|pending| pending.request),
        Some(request(2)),
    );

    actor
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("replacement response");
    assert!(actor.runtime_for_test().pending_directory.is_none());
    assert_eq!(actor.location().as_str(), "/root/Downloads");
}

#[test]
fn keyboard_arrows_space_enter_and_backspace_drive_the_file_view() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(&event(LIST_VIEW, UiEvent::Activate), request(3))
        .expect("switch to details view");
    for (key_code, target, expected) in [
        (111, ENTRY_BASE, "/root/Folder"),
        (116, ENTRY_BASE + 1, "/root/file2.txt"),
        (116, ENTRY_BASE + 2, "/root/file10.txt"),
        (111, ENTRY_BASE + 1, "/root/file2.txt"),
    ] {
        actor
            .handle_event(
                &event(
                    target,
                    UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers: 0,
                        text: None,
                    },
                ),
                request(4),
            )
            .expect("move keyboard selection");
        assert_eq!(actor.selected_entries()[0].path.as_str(), expected);
    }
    for key_code in [113, 114] {
        let horizontal = actor
            .handle_event(
                &event(
                    ENTRY_BASE + 1,
                    UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers: 0,
                        text: None,
                    },
                ),
                request(4),
            )
            .expect("horizontal key on a regular list row");
        assert!(horizontal.is_empty());
        assert_eq!(actor.selected_entries()[0].path.as_str(), "/root/file2.txt");
    }
    let previewed = actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Key {
                    pressed: true,
                    key_code: 65,
                    modifiers: 0,
                    text: Some(String::from(" ")),
                },
            ),
            request(5),
        )
        .expect("toggle embedded preview");
    assert!(actor.runtime_for_test().controls.preview_visible);
    assert!(matches!(
        previewed.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));

    actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Key {
                    pressed: true,
                    key_code: 113,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(6),
        )
        .expect("move to folder");
    assert_eq!(actor.selected_entries()[0].path.as_str(), "/root/Folder");

    let opened = actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Key {
                    pressed: true,
                    key_code: 36,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(7),
        )
        .expect("open selected folder");
    assert!(matches!(
        opened.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == "/root/Folder"
    ));

    let (mut parent_actor, _) = loaded_actor();
    let parent = parent_actor
        .handle_event(
            &event(
                DIRECTORY,
                UiEvent::Key {
                    pressed: true,
                    key_code: 22,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(8),
        )
        .expect("Backspace navigates to parent");
    assert!(matches!(
        parent.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == "/"
    ));
}

#[test]
fn keyboard_shift_arrows_extend_and_shrink_ranges_and_control_a_selects_all() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select the keyboard anchor");

    actor
        .handle_event(
            &event(
                ENTRY_BASE + 2,
                UiEvent::Key {
                    pressed: true,
                    key_code: 39,
                    modifiers: 1,
                    text: None,
                },
            ),
            request(3),
        )
        .expect("Shift+Right extends the range");
    assert_eq!(
        actor
            .selected_entries()
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["/root/file2.txt", "/root/file10.txt"]
    );

    actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Key {
                    pressed: true,
                    key_code: 37,
                    modifiers: 1,
                    text: None,
                },
            ),
            request(4),
        )
        .expect("Shift+Left shrinks back to the anchor");
    assert_eq!(
        actor
            .selected_entries()
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["/root/file2.txt"]
    );

    actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Key {
                    pressed: true,
                    key_code: 38,
                    modifiers: 4,
                    text: Some(String::from("a")),
                },
            ),
            request(5),
        )
        .expect("Ctrl+A selects the directory contents");
    assert_eq!(actor.selected_entries().len(), 3);

    actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Key {
                    pressed: true,
                    key_code: 113,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(6),
        )
        .expect("an ordinary arrow returns to one item");
    assert_eq!(actor.selected_entries()[0].path.as_str(), "/root/Folder");
    assert_eq!(actor.selected_entries().len(), 1);
}

#[test]
fn modified_pointer_clicks_build_a_collective_properties_selection() {
    let (mut actor, _) = loaded_actor();
    for (target, modifiers) in [(ENTRY_BASE, 0), (ENTRY_BASE + 2, 4)] {
        actor
            .handle_event(
                &event(
                    target,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 10,
                        y: 10,
                        button: 1,
                        modifiers,
                    },
                ),
                request(10 + target),
            )
            .expect("pointer down");
        actor
            .handle_event(
                &event(
                    target,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Up,
                        x: 10,
                        y: 10,
                        button: 1,
                        modifiers,
                    },
                ),
                request(20 + target),
            )
            .expect("pointer up");
    }
    assert_eq!(
        actor
            .selected_entries()
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        ["/root/Folder", "/root/file10.txt"]
    );
    let properties = actor
        .handle_event(
            &event(files_menu::PROPERTIES, UiEvent::Activate),
            request(30),
        )
        .expect("open collective properties");
    let [Message::UiPatch(properties)] = properties.as_slice() else {
        panic!("collective properties need no single-file stat request");
    };
    assert!(properties.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. } if text == "Properties: 2 items"
    )));
}

#[test]
fn shift_click_and_background_sweeps_select_contiguous_ranges() {
    let (mut shift_actor, _) = loaded_actor();
    shift_actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("set the range anchor");
    shift_actor
        .handle_event(
            &event(
                ENTRY_BASE + 2,
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 100,
                    y: 10,
                    button: 1,
                    modifiers: 1,
                },
            ),
            request(3),
        )
        .expect("Shift-click selects the complete range");
    assert_eq!(shift_actor.selected_entries().len(), 3);

    let (mut sweep_actor, _) = loaded_actor();
    sweep_actor
        .handle_event(
            &event(
                DIRECTORY,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 4,
                    y: 4,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(4),
        )
        .expect("start background selection");
    for target in [ENTRY_BASE, ENTRY_BASE + 2] {
        sweep_actor
            .handle_event(
                &event(
                    target,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Move,
                        x: 40,
                        y: 40,
                        button: 0,
                        modifiers: 0,
                    },
                ),
                request(5),
            )
            .expect("sweep across entries");
    }
    sweep_actor
        .handle_event(
            &event(
                ENTRY_BASE + 2,
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 80,
                    y: 40,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(6),
        )
        .expect("finish background selection");
    assert_eq!(sweep_actor.selected_entries().len(), 3);
    assert!(!sweep_actor.pointer_selection_active());
}

#[test]
fn printable_typing_filters_immediately_and_backspace_restores_the_folder() {
    let (mut actor, _) = loaded_actor();
    let filtered = actor
        .handle_event(
            &event(
                RELOAD,
                UiEvent::Key {
                    pressed: true,
                    key_code: 20,
                    modifiers: 0,
                    text: Some(String::from("-")),
                },
            ),
            request(2),
        )
        .expect("type-to-filter");
    assert_eq!(actor.runtime_for_test().controls.search_text, "-");
    assert!(actor.runtime_for_test().entries.is_empty());
    assert!(matches!(filtered.as_slice(), [Message::UiPatch(_)]));

    actor
        .handle_event(
            &event(
                DIRECTORY,
                UiEvent::Key {
                    pressed: true,
                    key_code: 22,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(3),
        )
        .expect("typeahead backspace");
    assert!(actor.runtime_for_test().controls.search_text.is_empty());
    assert_eq!(actor.runtime_for_test().entries.len(), 3);
    assert_eq!(actor.location().as_str(), "/root");

    actor
        .handle_event(
            &event(SEARCH_FIELD, UiEvent::ValueChanged(String::from("file2"))),
            request(4),
        )
        .expect("search field filters while typing");
    assert_eq!(actor.runtime_for_test().entries.len(), 1);
    assert_eq!(actor.runtime_for_test().entries[0].name, "file2.txt");

    let reloading = actor
        .handle_event(&event(RELOAD, UiEvent::Activate), request(5))
        .expect("reload filtered folder");
    assert!(reloading.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest { request: id, .. }) if *id == request(5)
    )));
    actor
        .handle_vfs_response(&directory_response(5))
        .expect("filtered reload response");
    assert_eq!(actor.runtime_for_test().controls.search_text, "file2");
    assert_eq!(actor.runtime_for_test().entries.len(), 1);
    assert_eq!(actor.runtime_for_test().entries[0].name, "file2.txt");
    assert!(
        actor
            .runtime_for_test()
            .content_notice
            .contains("1 immediate match")
    );
}

#[test]
fn typeahead_ignores_editable_fields_and_modal_dialogs() {
    let (mut actor, _) = loaded_actor();
    let printable = || UiEvent::Key {
        pressed: true,
        key_code: 20,
        modifiers: 0,
        text: Some(String::from("-")),
    };

    let editable = actor
        .handle_event(&event(BREADCRUMB, printable()), request(2))
        .expect("breadcrumb key");
    assert!(editable.is_empty());
    assert!(actor.runtime_for_test().controls.search_text.is_empty());

    actor
        .handle_event(
            &event(files_view::HELP_ABOUT, UiEvent::Activate),
            request(3),
        )
        .expect("open About dialog");
    let modal = actor
        .handle_event(&event(files_dialogs::CANCEL, printable()), request(4))
        .expect("modal key");
    assert!(modal.is_empty());
    assert!(actor.runtime_for_test().controls.search_text.is_empty());
}

#[test]
fn scroll_view_drag_value_updates_the_active_pane() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(
            &event(
                DIRECTORY_SCROLL,
                UiEvent::ValueChanged(String::from("240/900")),
            ),
            request(2),
        )
        .expect("scrollbar value");
    assert_eq!(actor.pane().expect("pane").scroll.vertical, 240);
    actor.begin_navigation(NavigationCommand::Reload, request(3)).expect("refresh directory");
    actor.handle_vfs_response(&directory_response(3)).expect("directory refreshed");
    assert_eq!(actor.pane().expect("pane").scroll.vertical, 240, "background directory refresh must retain scroll");
    actor.handle_event(&event(SEARCH_FIELD, UiEvent::ValueChanged("file".into())), request(4)).expect("new filter");
    assert_eq!(actor.pane().expect("pane").scroll.vertical, 0, "changing the filter still starts at the top");
}

#[test]
fn selected_media_thumbnail_paints_a_full_cell_selection() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("photo.png"),
                    metadata: metadata(FileKind::File, 42),
                    mime_type: Some(String::from("image/png")),
                }],
                next_cursor: None,
            },
        })
        .expect("image folder");
    let selected = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select image");
    let patch = selected.iter().find_map(|message| match message {
        Message::UiPatch(patch) => Some(patch),
        _ => None,
    });
    let patch = patch.expect("selection patch");
    let cell = ENTRY_BASE + files_view::THUMBNAIL_CELL_OFFSET;
    assert!(patch.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create { id, kind, state, .. }
                if *id == widget_id(cell)
                    && *kind == WidgetKind::Container as u16
                    && state & WidgetState::SELECTED.bits() != 0
        )
    }));
}

#[test]
fn idle_preview_is_a_muted_icon_without_preview_copy() {
    let (mut actor, _) = loaded_actor();
    let shown = actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(2))
        .expect("show idle preview");
    let [Message::UiPatch(shown)] = shown.as_slice() else {
        panic!("idle preview patch");
    };
    assert!(!shown.patches.iter().any(|candidate| {
        matches!(candidate, UiPatch::Create { text: Some(text), .. } if text.eq_ignore_ascii_case("preview"))
    }));
    assert!(shown.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create { kind, icon: Some(icon), state, .. }
                if *kind == WidgetKind::Image as u16
                    && *icon == rice_ui::IconId::ImageViewer.wire_value()
                    && state & WidgetState::MUTED.bits() != 0
                    && state & WidgetState::ENABLED.bits() == 0
        )
    }));
}

#[test]
fn places_toggle_and_tabs_are_real_in_window_state() {
    let (mut actor, _) = loaded_actor();
    let hidden = actor
        .handle_event(&event(PLACES_TOGGLE, UiEvent::Activate), request(2))
        .expect("hide places");
    let [Message::UiPatch(hidden)] = hidden.as_slice() else {
        panic!("places toggle emits UI");
    };
    assert!(!actor.sidebar_visible);
    assert!(hidden.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(CONTENT) && text == "panel:files-content"
    )));
    assert!(
        !hidden
            .patches
            .iter()
            .any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(PLACES)))
    );

    let opened = actor
        .handle_event(&event(NEW_TAB, UiEvent::Activate), request(3))
        .expect("new tab");
    assert_eq!(actor.model().windows()[0].tabs().len(), 2);
    let root_children = actor.tree.get(widget_id(ROOT)).expect("root").children();
    assert!(root_children.contains(&widget_id(MENU_BAR)));
    assert!(root_children.contains(&widget_id(TABS)));
    assert!(
        actor
            .tree
            .get(widget_id(TABS))
            .expect("shared tabs")
            .children()
            .contains(&widget_id(TAB_PAGE))
    );
    assert_eq!(
        actor
            .tree
            .get(widget_id(TAB_PAGE))
            .expect("real tab page")
            .children(),
        &[
            widget_id(80),
            widget_id(NAV_TOOLBAR),
            widget_id(CONTENT),
            widget_id(STATUS_PANEL),
        ]
    );
    assert!(matches!(
        opened.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: id,
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if *id == request(3) && path == "/root"
    ));
    actor
        .handle_vfs_response(&directory_response(3))
        .expect("new tab directory");
    let switched = actor
        .handle_event(&event(TAB_BASE + 1, UiEvent::Activate), request(4))
        .expect("switch tab");
    assert_eq!(actor.tab().expect("active tab").id, 1);
    assert!(matches!(switched.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn places_are_start_aligned_and_alt_zoom_updates_the_footer_control() {
    let (mut actor, _) = loaded_actor();
    let initial = actor.emit_full().expect("initial UI");
    let Message::UiPatch(initial) = initial else {
        panic!("initial patch");
    };
    assert!(initial.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, state, .. }
                if *id == widget_id(PLACE_HOME)
                    && WidgetState::from_bits_retain(*state)
                        .contains(WidgetState::START_ALIGNED)
        )
    }));

    let zoomed = actor
        .handle_event(
            &event(
                DIRECTORY,
                UiEvent::Key {
                    pressed: true,
                    key_code: 21,
                    modifiers: 8,
                    text: None,
                },
            ),
            request(2),
        )
        .expect("Alt+ zoom");
    assert_eq!(actor.view_zoom_permille, 1_100);
    let [Message::UiPatch(zoomed)] = zoomed.as_slice() else {
        panic!("zoom patch");
    };
    assert!(zoomed.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(ZOOM_SLIDER) && text == "600/4500"
        )
    }));
}

#[test]
fn yellow_jacket_uses_compact_zoom_and_other_themes_restore_normal_zoom() {
    let (yellow_actor, _) = RiceFiles::launch_at(
        app(),
        &[String::from("--theme-profile=yellow-jacket")],
        request(1),
    )
    .expect("Yellow Jacket RiceFiles");
    assert_eq!(yellow_actor.view_zoom_permille, 650);

    let (mut actor, _) = loaded_actor();
    actor
        .set_view_zoom(1_600)
        .expect("manual zoom before theme change");
    actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::RiceBus {
                    topic: String::from(THEME_CHANGED_TOPIC),
                    payload: String::from("yellow-jacket"),
                },
            },
            request(2),
        )
        .expect("activate Yellow Jacket");
    assert_eq!(actor.view_zoom_permille, 650);

    actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::RiceBus {
                    topic: String::from(THEME_CHANGED_TOPIC),
                    payload: String::from("familiar"),
                },
            },
            request(3),
        )
        .expect("leave Yellow Jacket");
    assert_eq!(actor.view_zoom_permille, 1_000);
}

#[test]
fn desktop_place_navigates_to_root_desktop() {
    let (mut actor, _) = loaded_actor();
    let messages = actor
        .handle_event(&event(PLACE_DESKTOP, UiEvent::Activate), request(2))
        .expect("open Desktop place");
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/root/Desktop"
    )));
}

#[test]
fn mounted_folder_tree_keeps_siblings_under_their_synthetic_ancestor() {
    for profile in ["redmond-98", "ricewm"] {
        let mount = "/run/ricewm/mounts/smb-test";
        let (mut actor, _) = RiceFiles::launch_at(app(), &[
            mount.to_owned(), format!("--theme-profile={profile}"),
            String::from("--folder-tree-default=true"),
        ], request(1)).expect("launch mounted folder");
        let response = |id, names: &[&str]| VfsResponse {
            request: request(id),
            result: VfsResult::Directory {
                entries: names.iter().map(|name| DirectoryEntry {
                    name: (*name).to_owned(), metadata: metadata(FileKind::Directory, 0),
                    mime_type: None,
                }).collect(), next_cursor: None,
            },
        };
        actor.handle_vfs_response(&response(1, &["christian", "nas"]))
            .expect("load mounted root");
        let tree_id = |actor: &mut RiceFiles, name: &str| {
            let Message::UiPatch(update) = actor.emit_full().expect("valid tree") else { panic!("UI patch") };
            update.patches.iter().find_map(|patch| match patch {
                UiPatch::Create { id, kind, text: Some(text), .. }
                    if *kind == WidgetKind::TreeItem as u16 && text == name => Some(*id),
                _ => None,
            }).expect("tree row")
        };
        for (id, name, children) in [(20, "nas", vec!["Backup", "Complete"]), (30, "Backup", vec![])] {
            let target = tree_id(&mut actor, name);
            actor.handle_event(&event(target.get(), UiEvent::Activate), request(id))
                .expect("click mounted folder");
            actor.handle_vfs_response(&response(id, &children))
                .expect("navigation must produce a valid tree");
            let root_id = tree_id(&mut actor, "smb-test");
            for sibling in ["christian", "nas"] {
                let sibling_id = tree_id(&mut actor, sibling);
                let row = actor.tree.get(sibling_id).unwrap();
                assert_eq!(row.parent, Some(root_id), "{profile}: keep {sibling} under mount root");
            }
            if name == "Backup" {
                assert_eq!(actor.location().as_str(), format!("{mount}/nas/Backup"));
            }
        }
    }
}

#[test]
fn redmond_folder_tree_uses_real_directories_and_navigates_them() {
    let (mut actor, _) = RiceFiles::launch_at(
        app(),
        &[String::from("--theme-profile=redmond-98")],
        request(1),
    )
    .expect("launch Redmond RiceFiles");
    assert!(actor.folder_tree);
    assert_eq!(
        actor.pane().expect("Redmond primary pane").view,
        ViewMode::Icons,
        "Redmond RiceFiles must open in icon view until the user explicitly chooses another view",
    );
    let loaded = actor
        .handle_vfs_response(&directory_response(1))
        .expect("load real root entries");
    let folder_target = loaded.iter().find_map(|message| {
        let Message::UiPatch(update) = message else {
            return None;
        };
        update.patches.iter().find_map(|patch| match patch {
            UiPatch::Create {
                id,
                kind,
                state,
                text: Some(text),
                ..
            } if *kind == WidgetKind::TreeItem as u16 && text == "Folder" => {
                assert!(
                    !WidgetState::from_bits_retain(*state).contains(WidgetState::INDETERMINATE),
                    "an unenumerated directory must not lie by exposing a plus before its children are known",
                );
                Some(id.get())
            }
            _ => None,
        })
    });
    let folder_target = folder_target.expect("real Folder appears in the folder tree");
    assert!(
        loaded.iter().any(|message| matches!(
            message,
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            }) if path == "/root/Folder"
        )),
        "the folder tree must discover real child directories before drawing a plus"
    );
    let navigated = actor
        .handle_event(&event(folder_target, UiEvent::Activate), request(2))
        .expect("activate real tree directory");
    assert!(navigated.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/root/Folder"
    )));
    let folder_loaded = actor
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("load empty child directory");
    assert_eq!(actor.pane().expect("active pane").view, ViewMode::Icons);
    let patch = folder_loaded
        .iter()
        .find_map(|message| match message {
            Message::UiPatch(patch) => Some(patch),
            _ => None,
        })
        .expect("folder update");
    assert!(
        patch.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { kind, text: Some(text), .. }
                if *kind == WidgetKind::TreeItem as u16 && text == "Folder"
        )),
        "the real root branch must survive navigation into an empty child"
    );
    assert!(
        !patch.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { text: Some(text), .. }
                if text.contains("redmond98-webview")
        )),
        "ordinary folders must not retain the My Computer identity pane"
    );
    let texts = patch
        .patches
        .iter()
        .filter_map(|patch| match patch {
            UiPatch::Create {
                text: Some(text), ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        texts.contains(&"This folder is empty"),
        "empty folder notice missing from {texts:?}"
    );
    assert!(patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            id,
            kind,
            state,
            text: Some(text),
            icon: None,
            ..
        } if *id == widget_id(CONTENT_NOTICE)
            && *kind == WidgetKind::Label as u16
            && text == "This folder is empty"
            && WidgetState::from_bits_retain(*state).contains(
                WidgetState::CENTER_ALIGNED
                    | WidgetState::BRAND_TYPE
                    | WidgetState::EMPHASIZED
            )
    )));
}

#[test]
fn redmond_tree_disclosure_exists_only_after_a_real_child_directory_is_found() {
    let launch = |name: &str| {
        let (mut actor, _) = RiceFiles::launch_at(
            app(),
            &[String::from("--theme-profile=redmond-98")],
            request(1),
        )
        .expect("launch Redmond RiceFiles");
        let root = VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: name.to_owned(),
                    metadata: metadata(FileKind::Directory, 0),
                    mime_type: None,
                }],
                next_cursor: None,
            },
        };
        let messages = actor
            .handle_vfs_response_with_request(&root, request(2))
            .expect("load root and begin real child probe");
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            }) if path == &format!("/root/{name}")
        )));
        actor
    };

    let mut empty = launch("Empty");
    let empty_update = empty
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("finish empty probe");
    let empty_patch = empty_update
        .iter()
        .find_map(|message| match message {
            Message::UiPatch(patch) => Some(patch),
            _ => None,
        })
        .expect("empty tree redraw");
    let empty_row = empty_patch
        .patches
        .iter()
        .find_map(|patch| match patch {
            UiPatch::Create {
                id,
                kind,
                state,
                text: Some(text),
                ..
            } if *kind == WidgetKind::TreeItem as u16 && text == "Empty" => Some((*id, *state)),
            _ => None,
        })
        .expect("empty row");
    assert!(!WidgetState::from_bits_retain(empty_row.1).contains(WidgetState::INDETERMINATE));
    assert!(!empty_patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { parent: Some(parent), kind, .. }
            if *parent == empty_row.0 && *kind == WidgetKind::TreeItem as u16
    )));

    let mut parent = launch("Parent");
    let parent_update = parent
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("Child"),
                    metadata: metadata(FileKind::Directory, 0),
                    mime_type: None,
                }],
                next_cursor: None,
            },
        })
        .expect("finish nonempty probe");
    let parent_patch = parent_update
        .iter()
        .find_map(|message| match message {
            Message::UiPatch(patch) => Some(patch),
            _ => None,
        })
        .expect("nonempty tree redraw");
    let parent_id = parent_patch
        .patches
        .iter()
        .find_map(|patch| match patch {
            UiPatch::Create {
                id,
                kind,
                text: Some(text),
                ..
            } if *kind == WidgetKind::TreeItem as u16 && text == "Parent" => Some(*id),
            _ => None,
        })
        .expect("parent row");
    assert!(parent_patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { parent: Some(parent), kind, text: Some(text), .. }
            if *parent == parent_id && *kind == WidgetKind::TreeItem as u16 && text == "Child"
    )));
}

#[test]
fn folder_tree_is_a_general_user_option_outside_redmond() {
    let (mut actor, _) = loaded_actor();
    assert!(!actor.folder_tree);
    let changed = actor
        .handle_event(&event(FOLDER_TREE, UiEvent::Activate), request(2))
        .expect("enable folder tree");
    assert!(actor.folder_tree);
    assert!(changed.iter().any(|message| matches!(
        message,
        Message::SettingsRequest(SettingsRequest {
            operation: SettingsOperation::Set { key, value },
            ..
        }) if key == "ricefiles.folder-tree.standard" && value == "true"
    )));
    assert!(changed.iter().any(|message| {
        let Message::UiPatch(update) = message else {
            return false;
        };
        update.patches.iter().any(|patch| {
            matches!(
                patch,
                UiPatch::Create { id, kind, .. }
                    if *id == widget_id(files_view::PLACES_LIST)
                        && *kind == WidgetKind::Tree as u16
            )
        })
    }));
}

#[test]
fn ordinary_pointer_motion_never_resizes_the_places_sidebar() {
    let (mut actor, _) = loaded_actor();
    assert_eq!(actor.sidebar_permille, 220);
    let moved = actor
        .handle_event(
            &event(
                CONTENT,
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: 15,
                    y: 200,
                    button: 0,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("content hover");
    assert!(moved.is_empty());
    assert_eq!(actor.sidebar_permille, 220);
}

#[test]
fn places_sidebar_drag_is_continuous_and_uses_layout_pixel_bounds() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(
            &event(
                CONTENT,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 25,
                    y: 200,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("sidebar drag start");
    assert_eq!(actor.sidebar_permille, 100);
    let moved = actor
        .handle_event(
            &event(
                CONTENT,
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: 480,
                    y: 200,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("sidebar drag move");
    assert_eq!(actor.sidebar_permille, 480);
    assert!(matches!(moved.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn search_split_and_preview_keep_validated_state() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&directory_response(1))
        .expect("directory");
    actor
        .handle_event(
            &event(SEARCH_FIELD, UiEvent::ValueChanged(String::from("file"))),
            request(4),
        )
        .expect("search text");
    let started = actor
        .handle_event(
            &event(
                SEARCH_FIELD,
                UiEvent::Key {
                    pressed: true,
                    key_code: 13,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(5),
        )
        .expect("search intent");
    assert!(actor.runtime_for_test().pending_search.is_some());
    assert!(matches!(
        started.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    actor
        .handle_vfs_response(&directory_response(5))
        .expect("search root");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("skip"),
            }),
        })
        .expect("skip unreadable child");
    assert_eq!(
        actor
            .runtime_for_test()
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["file2.txt", "file10.txt"]
    );
    assert!(
        actor
            .runtime_for_test()
            .content_notice
            .contains("Results: 2")
    );
    actor
        .handle_event(
            &event(SEARCH_FIELD, UiEvent::ValueChanged(String::new())),
            request(50),
        )
        .expect("clear search text");
    actor
        .handle_event(
            &event(
                SEARCH_FIELD,
                UiEvent::Key {
                    pressed: true,
                    key_code: 13,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(51),
        )
        .expect("restore loaded folder");
    assert_eq!(actor.runtime_for_test().entries.len(), 3);

    let split = actor
        .handle_event(&event(SPLIT_TOGGLE, UiEvent::Activate), request(6))
        .expect("open secondary pane");
    assert!(actor.tab().expect("tab").dual_pane);
    assert!(matches!(
        split.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));

    assert!(!actor.runtime_for_test().controls.preview_visible);

    let preview = actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(8))
        .expect("preview toggle");
    assert!(actor.runtime_for_test().controls.preview_visible);
    let [Message::UiPatch(preview)] = preview.as_slice() else {
        panic!("preview emits UI");
    };
    assert!(preview.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(PREVIEW_PANEL) && *kind == WidgetKind::Column as u16
        )
    }));

    actor
        .handle_event(
            &event(menu_widget(PREVIEW_TOGGLE), UiEvent::Activate),
            request(9),
        )
        .expect("popup menu action");
    assert!(!actor.runtime_for_test().controls.preview_visible);
}

#[test]
fn vfs_errors_never_create_fake_entries_or_commit_navigation() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let failed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("denied"),
            }),
        })
        .expect("error state");
    assert!(actor.runtime_for_test().entries.is_empty());
    assert_eq!(actor.location().as_str(), "/root");
    assert!(actor.runtime_for_test().content_notice.is_empty());
    assert!(matches!(
        failed.as_slice(),
        [
            Message::UiPatch(_),
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification {
                    kind,
                    title,
                    message,
                },
                ..
            })
        ] if kind == "error" && title == "Files" && message.contains("denied")
    ));
}

#[test]
fn rename_waits_for_done_then_refreshes_directory() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select");
    let opened = actor
        .handle_event(&event(RENAME, UiEvent::Activate), request(3))
        .expect("open rename");
    assert!(matches!(opened.as_slice(), [Message::UiPatch(_)]));
    let edit = actor
        .handle_event(
            &event(
                files_dialogs::FIELD,
                UiEvent::ValueChanged(String::from("renamed.txt")),
            ),
            request(3),
        )
        .expect("edit target");
    assert!(matches!(edit.as_slice(), [Message::UiPatch(_)]));
    assert!(actor.pending_mutation.is_none());

    let started = actor
        .handle_event(
            &event(files_dialogs::CONFIRM, UiEvent::Activate),
            request(4),
        )
        .expect("start rename");
    assert!(matches!(
        started.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: id,
                operation: VfsOperation::Rename { from, to },
                ..
            })
        ] if *id == request(4)
            && from == "/root/file2.txt"
            && to == "/root/renamed.txt"
    ));
    assert!(actor.runtime_for_test().pending_directory.is_none());
    assert!(actor.pending_mutation.is_some());
    assert!(actor.runtime_for_test().content_notice.is_empty());
    assert!(
        !actor
            .runtime_for_test()
            .content_notice
            .contains("completed for")
    );

    let refresh = actor
        .handle_vfs_response(&done_response(4))
        .expect("rename done");
    assert!(matches!(
        refresh.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: id,
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            }),
            Message::HostRequest(_)
        ] if *id == request(4) && path == "/root"
    ));
    assert!(actor.pending_mutation.is_none());
    assert!(actor.runtime_for_test().pending_directory.is_some());
    assert!(actor.runtime_for_test().content_notice.is_empty());
    assert!(
        actor
            .runtime_for_test()
            .pending_directory
            .as_ref()
            .and_then(|pending| pending.completion_notice.as_deref())
            .is_some_and(|notice| notice.contains("completed"))
    );

    actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("refresh result");
    assert!(actor.runtime_for_test().pending_directory.is_none());
    assert_eq!(
        actor.runtime_for_test().content_notice,
        "This folder is empty"
    );
    assert!(
        !actor
            .runtime_for_test()
            .content_notice
            .contains("Rename completed")
    );
}

#[test]
fn mutation_error_stops_without_refresh_or_false_success() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select");
    actor
        .handle_event(&event(RENAME, UiEvent::Activate), request(3))
        .expect("open rename");
    actor
        .handle_event(
            &event(
                files_dialogs::FIELD,
                UiEvent::ValueChanged(String::from("blocked.txt")),
            ),
            request(3),
        )
        .expect("target");
    actor
        .handle_event(
            &event(files_dialogs::CONFIRM, UiEvent::Activate),
            request(4),
        )
        .expect("start");
    let failed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("denied by host"),
            }),
        })
        .expect("failure is state, not protocol error");
    assert!(matches!(
        failed.as_slice(),
        [
            Message::UiPatch(_),
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification {
                    kind,
                    message,
                    ..
                },
                ..
            })
        ] if kind == "error"
            && message.contains("stopped after 0/1")
            && message.contains("denied by host")
    ));
    assert!(actor.pending_mutation.is_none());
    assert!(actor.runtime_for_test().pending_directory.is_none());
    assert!(actor.runtime_for_test().content_notice.is_empty());
}

pub(super) fn loaded_actor() -> (RiceFiles, Vec<Message>) {
    let (mut actor, messages) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&directory_response(1))
        .expect("directory");
    (actor, messages)
}

#[test]
fn riceapp_selection_offers_publication_and_launches_the_selected_package() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let mut response = directory_response(1);
    if let VfsResult::Directory { entries, .. } = &mut response.result {
        entries.truncate(1);
        entries[0].name = String::from("Reminder 日本語.riceapp");
    }
    actor.handle_vfs_response(&response).expect("directory");
    let selected = actor.handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2)).expect("select");
    assert!(selected.iter().any(|message| matches!(message, Message::UiPatch(batch) if batch.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text == "Publish to Marketplace")))));
    let launch = actor.handle_event(&event(files_menu::context_widget(files_menu::PUBLISH_MARKETPLACE), UiEvent::Activate), request(3)).expect("publish");
    assert!(matches!(launch.as_slice(), [Message::Launch { entrypoint, arguments, .. }] if entrypoint == "rice-marketplace-publish" && arguments == &[String::from("/root/Reminder 日本語.riceapp")]));
    let (mut ordinary, _) = loaded_actor();
    let selected = ordinary.handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2)).expect("select text file");
    assert!(!selected.iter().any(|message| matches!(message, Message::UiPatch(batch) if batch.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text == "Publish to Marketplace")))));
    let ignored = ordinary.handle_event(&event(files_menu::PUBLISH_MARKETPLACE, UiEvent::Activate), request(3)).expect("reject non app");
    assert!(!ignored.iter().any(|message| matches!(message, Message::Launch { .. })));
}

fn done_response(value: u64) -> VfsResponse {
    VfsResponse {
        request: request(value),
        result: VfsResult::Done,
    }
}

#[test]
fn advertised_alt_tab_and_window_keys_work_from_location_field() {
    let (mut actor, _) = loaded_actor();
    let key = |code, modifiers, pressed| event(BREADCRUMB, UiEvent::Key {
        pressed, key_code: code, modifiers, text: None,
    });
    for (modifiers, pressed) in [(9, true), (12, true), (8, false)] {
        actor.handle_event(&key(28, modifiers, pressed), request(50)).expect("unmatched chord");
        assert_eq!(actor.model().windows()[0].tabs().len(), 1);
    }
    actor.handle_event(&key(28, 8, true), request(51)).expect("new tab");
    assert_eq!(actor.model().windows()[0].tabs().len(), 2);
    let launched = actor.handle_event(&key(57, 8, true), request(52)).expect("new window");
    assert!(launched.iter().any(|message| matches!(message, Message::Launch { entrypoint, arguments, .. }
        if entrypoint == ENTRYPOINT && arguments == &vec![actor.location().as_str().to_owned()])));
    actor.handle_vfs_response(&directory_response(51)).expect("new tab directory loaded");
    actor.handle_event(&key(25, 8, true), request(53)).expect("close tab");
    assert_eq!(actor.model().windows()[0].tabs().len(), 1);
    let closing = actor.handle_event(&key(25, 8, true), request(54)).expect("close final tab");
    assert!(closing.iter().any(|message| matches!(message, Message::HostRequest(HostRequest {
        operation: HostOperation::RequestClose, ..
    }))));
}

#[test]
fn document_state_requires_complete_directory_reads_and_preserves_other_tabs() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).unwrap();
    let original = actor.location().as_str().to_owned();
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([]));
    assert_eq!(actor.document_state().unwrap()["ready"], false);
    actor.handle_vfs_response(&VfsResponse {request:request(1),result:VfsResult::Directory {
        entries:vec![],next_cursor:Some(1),
    }}).unwrap();
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([]));
    assert_eq!(actor.document_state().unwrap()["ready"], false);
    actor.handle_vfs_response(&directory_response(1)).unwrap();
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([original]));
    assert_eq!(actor.document_state().unwrap()["ready"], true);

    actor.begin_navigation(NavigationCommand::Enter("/missing".into()),request(2)).unwrap();
    let missing = |id| VfsResponse {request:request(id),result:VfsResult::Error(VfsError {
        code:VfsErrorCode::NotFound,message:"missing".into(),
    })};
    actor.handle_vfs_response(&missing(2)).unwrap();
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([original]));
    assert_eq!(actor.document_state().unwrap()["ready"], true);
    actor.begin_navigation(NavigationCommand::Reload,request(3)).unwrap();
    actor.handle_vfs_response(&missing(3)).unwrap();
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([]));
    actor.begin_navigation(NavigationCommand::Reload,request(4)).unwrap();
    actor.handle_vfs_response(&directory_response(4)).unwrap();

    actor.open_tab_at(GuestPath::absolute("/another").unwrap(),request(5)).unwrap();
    assert_eq!(actor.document_state().unwrap()["ready"], false);
    assert_eq!(actor.document_state().unwrap()["documents"], serde_json::json!([original]));
    actor.handle_vfs_response(&directory_response(5)).unwrap();
    let state=actor.document_state().unwrap();
    assert_eq!(state["ready"],true);
    assert_eq!(state["documents"].as_array().unwrap().len(),2);
    assert!(state["documents"].as_array().unwrap().contains(&serde_json::json!(original)));
    assert!(state["documents"].as_array().unwrap().contains(&serde_json::json!("/another")));
}


#[test]
fn videos_place_navigates_to_capitalized_home_directory() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    assert_eq!(actor.folder_tree_path_for_target(files_view::PLACE_VIDEOS).expect("video path").as_str(), "/root/Videos");
    let messages = actor.handle_event(&event(files_view::PLACE_VIDEOS, UiEvent::Activate), request(2)).expect("activate Videos");
    assert!(messages.iter().any(|message| matches!(message,
        Message::VfsRequest(VfsRequest { operation: VfsOperation::ReadDirectory { path, .. }, .. }) if path == "/root/Videos"
    )));
}
