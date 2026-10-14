use super::{
    DELETE, ENTRY_BASE, Message, RiceFiles, UiEvent, VIEW_DETAILS, VfsOperation, VfsRequest,
    files_menu,
    files_view::{
        DETAIL_DISCLOSURE_OFFSET, DETAIL_MODIFIED_BASE, DETAIL_NAME, DETAIL_SIZE_BASE, DIRECTORY,
        widget_id,
    },
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::UiPatch;
use rice_native_app_protocol::{DirectoryEntry, FileKind, FileMetadata, VfsResponse, VfsResult};
use rice_ui::{IconId, MenuText, WidgetKind, WidgetState};

#[test]
fn detail_view_exposes_tree_rows_and_context_operations_honestly() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select directory");
    let details = actor
        .handle_event(&event(VIEW_DETAILS, UiEvent::Activate), request(3))
        .expect("detail view");
    let [Message::UiPatch(details)] = details.as_slice() else {
        panic!("detail view emits UI");
    };
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                kind,
                text: Some(text),
                ..
            } if *id == widget_id(ENTRY_BASE)
                && *kind == WidgetKind::Button as u16
                && text == "Folder"
        )
    }));
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, kind, icon: Some(icon), .. }
                if *id == widget_id(ENTRY_BASE + DETAIL_DISCLOSURE_OFFSET)
                    && *kind == WidgetKind::Button as u16
                    && *icon == IconId::GoForward.wire_value()
        )
    }));
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                kind,
                text: Some(text),
                ..
            } if *id == widget_id(DETAIL_NAME)
                && *kind == WidgetKind::Button as u16
                && text == "Name ↑"
        )
    }));
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch, UiPatch::Create {
                id,
                parent: Some(parent),
                text: Some(text),
                ..
            } if *id == widget_id(DETAIL_SIZE_BASE)
                && *parent != widget_id(DIRECTORY)
                && text == "—"
        )
    }));
    assert!(details.patches.iter().any(|patch| {
        matches!(
            patch, UiPatch::Create {
                id,
                text: Some(text),
                ..
            } if *id == widget_id(DETAIL_MODIFIED_BASE) && text == "Jan 1, 1970"
        )
    }));
    assert_context_labels(details);
    assert_context_availability(details);
    let opened = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(4))
        .expect("second activation opens directory");
    assert!(matches!(
        opened.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if path == "/root/Folder"
    ));
}

fn assert_context_labels(patch: &rice_native_app_protocol::UiPatchEnvelope) {
    for label in [
        "Open",
        "Open in Terminal",
        "Open With…",
        "Cut",
        "Copy",
        "Move to…",
        "Copy to…",
        "Compress to",
        "Extract Here",
        "Download to Host",
        "Rename…",
        "Move to Trash",
        "Delete Permanently…",
        "Properties",
    ] {
        assert!(patch.patches.iter().any(|candidate| {
            matches!(
                candidate,
                UiPatch::Create {
                    parent: Some(parent),
                    text: Some(text),
                    ..
                } if *parent == widget_id(files_menu::CONTEXT_MENU)
                    && MenuText::parse(text).label == label
            )
        }));
    }
    for label in ["Compress to ZIP", "Compress to TAR.GZ", "Compress to GZIP"] {
        assert!(patch.patches.iter().any(|candidate| {
            matches!(
                candidate,
                UiPatch::Create {
                    parent: Some(parent),
                    text: Some(text),
                    ..
                } if *parent == widget_id(files_menu::COMPRESS_SUBMENU)
                    && MenuText::parse(text).label == label
            )
        }));
    }
}

fn assert_context_availability(patch: &rice_native_app_protocol::UiPatchEnvelope) {
    assert!(patch.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create {
                id,
                state,
                ..
            } if *id == widget_id(files_menu::context_widget(DELETE))
                && *state & WidgetState::ENABLED.bits() != 0
        )
    }));
    assert!(patch.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create {
                id,
                state,
                ..
            } if *id == widget_id(files_menu::context_widget(files_menu::PROPERTIES))
                && *state & WidgetState::ENABLED.bits() != 0
        )
    }));
}

#[test]
fn background_context_keeps_paste_available_without_a_selection() {
    let (actor, _) = loaded_actor();
    let patches = actor.full_patches().expect("full file view");
    assert!(patches.iter().any(|candidate| {
        matches!(
            candidate,
            rice_ui::Patch::Create { widget, .. }
                if widget.id == widget_id(files_menu::context_widget(files_menu::PASTE))
                    && widget.state.contains(WidgetState::ENABLED)
        )
    }));
    assert!(patches.iter().any(|candidate| {
        matches!(
            candidate,
            rice_ui::Patch::Create { widget, .. }
                if widget.id
                    == widget_id(files_menu::context_widget(files_menu::OPEN_IN_TERMINAL))
                    && widget.state.contains(WidgetState::ENABLED)
        )
    }));
}

#[test]
fn hidden_entries_are_muted_and_symlinks_use_the_link_icon() {
    let (mut actor, _) = RiceFiles::launch(super::tests::app(), request(1)).expect("launch");
    actor.options.show_hidden = true;
    let update = actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![
                    DirectoryEntry {
                        name: String::from(".hidden"),
                        metadata: FileMetadata {
                            kind: FileKind::Directory,
                            size: 0,
                            modified_millis: 0,
                            mode: 0o755,
                        },
                        mime_type: None,
                    },
                    DirectoryEntry {
                        name: String::from("shortcut"),
                        metadata: FileMetadata {
                            kind: FileKind::Symlink,
                            size: 0,
                            modified_millis: 0,
                            mode: 0o777,
                        },
                        mime_type: None,
                    },
                ],
                next_cursor: None,
            },
        })
        .expect("directory");
    let [Message::UiPatch(update)] = update.as_slice() else {
        panic!("directory emits UI");
    };
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            text: Some(text),
            state,
            icon: Some(icon),
            ..
        } if text == ".hidden"
            && state & WidgetState::MUTED.bits() != 0
            && *icon == IconId::Folder.wire_value()
    )));
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            text: Some(text),
            icon: Some(icon),
            ..
        } if text == "shortcut" && *icon == IconId::Link.wire_value()
    )));
}

#[test]
fn places_only_offers_add_for_unsaved_folders() {
    use super::{files_preferences, files_view};
    for path in ["/root", "/root/Desktop", "/root/Documents", "/root/Downloads", "/root/Pictures", "/root/Videos", "/root/.local/share/Trash/files", "/mnt/local", "/root/Projects"] {
        let (mut actor, _) = RiceFiles::launch_at(super::tests::app(), &[path.to_owned()], request(1)).expect("launch");
        let Message::UiPatch(ui) = actor.emit_full().expect("render") else { panic!("UI"); };
        let has_add = ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(files_view::PLACE_ADD)));
        assert_eq!(has_add, path == "/root/Projects", "{path}");
        assert!(ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { id, icon: Some(icon), .. } if *id == widget_id(files_view::PLACE_VIDEOS) && *icon == IconId::Folder as u16)));
        if has_add {
            files_preferences::add_current(&mut actor, request(2)).expect("save place");
            let Message::UiPatch(ui) = actor.emit_full().expect("render saved") else { panic!("UI"); };
            assert!(!ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(files_view::PLACE_ADD) || *id == widget_id(files_view::PLACE_REMOVE))));
            assert!(ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text == "Remove from Places")));
        }
    }
}

#[test]
fn about_files_has_sections_credits_and_close() {
    use super::files_dialogs;
    let (mut actor, _) = loaded_actor();
    actor.show_about().expect("about");
    let Message::UiPatch(ui) = actor.emit_full().expect("render") else { panic!("UI"); };
    assert!(ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text == "About Files")));
    assert!(!ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text.contains("RiceFiles"))));
    let messages = actor.handle_event(&event(files_dialogs::ABOUT_NAV + 22, UiEvent::Activate), request(2)).expect("credits");
    let Message::UiPatch(ui) = &messages[0] else { panic!("UI"); };
    assert!(ui.patches.iter().any(|patch| matches!(patch, UiPatch::Create { text: Some(text), .. } if text.contains("adblock-rust") && text.contains("EasyList"))));
    actor.handle_event(&event(files_dialogs::ABOUT_SCROLL, UiEvent::ValueChanged("120/0".to_owned())), request(3)).expect("scroll");
    assert!(matches!(actor.dialog, Some(files_dialogs::Dialog::About { section: 2, scroll: 120 })));
    actor.handle_event(&event(files_dialogs::ABOUT_NAV + 20, UiEvent::Activate), request(4)).expect("overview");
    assert!(matches!(actor.dialog, Some(files_dialogs::Dialog::About { section: 0, scroll: 0 })));
    actor.handle_event(&event(files_dialogs::CANCEL, UiEvent::Activate), request(5)).expect("close");
    assert!(actor.dialog.is_none());
}
