use super::{
    ENTRY_BASE, Message, NEW_WINDOW, RiceFiles, UiEvent, files_dialogs, files_menu,
    files_view::{
        BOOKMARK_BASE, DETAIL_SIZE_BASE, DIRECTORY_SCROLL, PLACE_DESKTOP, PLACE_DOCUMENTS,
        PLACE_MOUNTS, PLACE_TRASH, PLACES, widget_id,
    },
    tests::{app, event, loaded_actor, request},
};
use rice_files::{Bookmark, GuestPath};
use rice_native_app_protocol::{
    ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection, DragAction,
    DragEnvelope, DragId, DragPayload, DragPhase, FileKind, FileMetadata, PointerEventKind,
    SettingEntry, SettingsResponse, SettingsResult, VfsError, VfsErrorCode, VfsOperation,
    VfsRequest, VfsResponse, VfsResult,
};

fn select_file(actor: &mut RiceFiles) {
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
}

fn set_dialog_field(actor: &mut RiceFiles, value: &str) {
    actor
        .handle_event(
            &event(
                files_dialogs::FIELD,
                UiEvent::ValueChanged(value.to_owned()),
            ),
            request(3),
        )
        .expect("edit dialog");
}

fn confirm(actor: &mut RiceFiles, id: u64) -> Vec<Message> {
    actor
        .handle_event(
            &event(files_dialogs::CONFIRM, UiEvent::Activate),
            request(id),
        )
        .expect("confirm dialog")
}

#[test]
fn copy_to_and_permanent_delete_emit_real_vfs_operations() {
    let (mut copy_actor, _) = loaded_actor();
    select_file(&mut copy_actor);
    copy_actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("open copy");
    set_dialog_field(&mut copy_actor, "/root/Folder");
    assert!(matches!(
        confirm(&mut copy_actor, 4).as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy {
                    from,
                    to,
                    recursive: false,
                },
                ..
            })
        ] if from == "/root/file2.txt" && to == "/root/Folder/file2.txt"
    ));

    let (mut delete_actor, _) = loaded_actor();
    select_file(&mut delete_actor);
    delete_actor
        .handle_event(
            &event(files_menu::DELETE_PERMANENTLY, UiEvent::Activate),
            request(3),
        )
        .expect("open delete confirmation");
    assert!(matches!(
        confirm(&mut delete_actor, 4).as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Remove {
                    path,
                    recursive: false,
                },
                ..
            })
        ] if path == "/root/file2.txt"
    ));
}

#[test]
fn archive_context_shortcuts_emit_background_vfs_jobs() {
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    let messages = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::COMPRESS_ZIP),
                UiEvent::Activate,
            ),
            request(30),
        )
        .expect("compress selection");
    assert!(matches!(
        messages.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Archive {
                sources,
                destination,
                format: rice_native_app_protocol::ArchiveFormat::Zip,
            },
            ..
        })] if sources == &[String::from("/root/file2.txt")]
            && destination == "/root/file2.txt.zip"
    ));
}

#[test]
fn download_to_host_handles_files_and_persists_the_one_time_tip() {
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    let messages = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::DOWNLOAD_TO_HOST),
                UiEvent::Activate,
            ),
            request(31),
        )
        .expect("download selected file");
    assert!(matches!(
        messages.as_slice(),
        [
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::DownloadPath {
                    path,
                    filename,
                    show_storage_tip: true,
                },
                ..
            }),
            Message::SettingsRequest(rice_native_app_protocol::SettingsRequest {
                operation: rice_native_app_protocol::SettingsOperation::Set { key, value },
                ..
            })
        ] if path == "/root/file2.txt"
            && filename == "file2.txt"
            && key == "ricefiles.download-tip-shown"
            && value == "1"
    ));
    actor
        .handle_settings_response(&SettingsResponse {
            request: request(31),
            result: SettingsResult::Applied(SettingEntry {
                key: String::from("ricefiles.download-tip-shown"),
                value: String::from("1"),
            }),
        })
        .expect("persist tip");
    let repeated = actor
        .handle_event(
            &event(files_menu::DOWNLOAD_TO_HOST, UiEvent::Activate),
            request(32),
        )
        .expect("repeat download");
    assert!(matches!(
        repeated.as_slice(),
        [Message::HostRequest(
            rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::DownloadPath {
                    show_storage_tip: false,
                    ..
                },
                ..
            }
        )]
    ));
}

#[test]
fn directory_download_is_zipped_then_handled_off_to_the_host() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select folder");
    let started = actor
        .handle_event(
            &event(files_menu::DOWNLOAD_TO_HOST, UiEvent::Activate),
            request(40),
        )
        .expect("prepare folder download");
    assert!(matches!(
        started.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Archive {
                sources,
                destination,
                format: rice_native_app_protocol::ArchiveFormat::Zip,
            },
            ..
        })] if sources == &[String::from("/root/Folder")]
            && destination == "/root/Folder.zip"
    ));
    let completed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(40),
            result: VfsResult::Done,
        })
        .expect("finish folder archive");
    assert!(completed.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::DownloadPath {
                path,
                filename,
                show_storage_tip: true,
            },
            ..
        }) if path == "/root/Folder.zip" && filename == "Folder.zip"
    )));
}

#[test]
fn new_folder_and_close_are_real_file_menu_actions() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(
            &event(
                files_menu::menu_widget(files_menu::NEW_FOLDER),
                UiEvent::Activate,
            ),
            request(3),
        )
        .expect("open new folder dialog");
    set_dialog_field(&mut actor, "Projects");
    assert!(matches!(
        confirm(&mut actor, 4).as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::CreateDirectory { path },
            ..
        })] if path == "/root/Projects"
    ));

    let (mut close_actor, _) = loaded_actor();
    assert!(matches!(
        close_actor
            .handle_event(
                &event(
                    files_menu::menu_widget(files_menu::CLOSE),
                    UiEvent::Activate,
                ),
                request(5),
            )
            .expect("close action")
            .as_slice(),
        [Message::HostRequest(request)]
            if request.operation == rice_native_app_protocol::HostOperation::RequestClose
    ));
}

#[test]
fn cut_copy_use_shared_typed_clipboard() {
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    let copied = actor
        .handle_event(&event(files_menu::COPY, UiEvent::Activate), request(3))
        .expect("copy");
    assert!(matches!(
        copied.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write {
            selection: ClipboardSelection::Clipboard,
            mime_type,
            data,
            ..
        })] if mime_type == "x-special/gnome-copied-files"
            && data == b"copy\nfile:///root/file2.txt\n"
    ));

    let (mut cut_actor, _) = loaded_actor();
    select_file(&mut cut_actor);
    let cut = cut_actor
        .handle_event(&event(files_menu::CUT, UiEvent::Activate), request(4))
        .expect("cut");
    assert!(matches!(
        cut.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write {
            mime_type,
            data,
            ..
        })] if mime_type == "x-special/gnome-copied-files"
            && data == b"cut\nfile:///root/file2.txt\n"
    ));
}

#[test]
fn standard_clipboard_and_delete_keys_match_the_file_menu_actions() {
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    let copied = actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Key {
                    pressed: true,
                    key_code: 54,
                    modifiers: 4,
                    text: None,
                },
            ),
            request(3),
        )
        .expect("Ctrl+C");
    assert!(matches!(
        copied.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write { data, .. })]
            if data == b"copy\nfile:///root/file2.txt\n"
    ));

    let (mut delete_actor, _) = loaded_actor();
    select_file(&mut delete_actor);
    let confirmation = delete_actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Key {
                    pressed: true,
                    key_code: 119,
                    modifiers: 1,
                    text: None,
                },
            ),
            request(4),
        )
        .expect("Shift+Delete");
    assert!(matches!(
        delete_actor.dialog,
        Some(files_dialogs::Dialog::DeletePermanently)
    ));
    assert!(matches!(confirmation.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn paste_reads_gnome_then_uri_fallback_and_preserves_copy_action() {
    let (mut actor, _) = loaded_actor();
    let read = actor
        .handle_event(&event(files_menu::PASTE, UiEvent::Activate), request(3))
        .expect("paste");
    assert!(matches!(
        read.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Read {
            accepted_mime_types,
            ..
        })] if accepted_mime_types == &[
            "x-special/gnome-copied-files".to_owned(),
            "text/uri-list".to_owned(),
        ]
    ));
    let inspect = actor
        .handle_clipboard_response(&ClipboardResponse {
            request: request(3),
            result: ClipboardResult::Data {
                mime_type: String::from("x-special/gnome-copied-files"),
                data: b"copy\nfile:///root/Documents/source.txt\n".to_vec(),
            },
        })
        .expect("clipboard data");
    assert!(matches!(
        inspect.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Stat { path },
                ..
            })
        ] if path == "/root/Documents/source.txt"
    ));
    let transfer = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("source metadata");
    assert!(matches!(
        transfer.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy { from, to, .. },
                ..
            })
        ] if from == "/root/Documents/source.txt" && to == "/root/source.txt"
    ));
}

#[test]
fn negotiated_external_uri_drop_stats_then_uses_validated_copy_plan() {
    let (mut actor, _) = loaded_actor();
    let drag = DragId::new(44).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(ENTRY_BASE)),
        phase: DragPhase::Enter,
        action: DragAction::Copy,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(2)).expect("enter");
    let inspect = actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Documents/source.txt\r\n".to_vec(),
                }),
                ..base
            },
            request(3),
        )
        .expect("drop");
    assert!(matches!(
        inspect.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Stat { path },
                ..
            })
        ] if path == "/root/Documents/source.txt"
    ));
    let copied = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    assert!(matches!(
        copied.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy { from, to, .. },
                ..
            })
        ] if from == "/root/Documents/source.txt"
            && to == "/root/Folder/source.txt"
    ));
}

#[test]
fn default_drag_across_local_mount_boundary_copies_instead_of_renaming() {
    let (mut actor, _) = loaded_actor();
    let drag = DragId::new(144).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(PLACE_DOCUMENTS)),
        phase: DragPhase::Enter,
        action: DragAction::Move,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(40)).expect("enter");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///mnt/local/drive/source.txt\r\n".to_vec(),
                }),
                ..base
            },
            request(41),
        )
        .expect("drop");
    let copied = actor
        .handle_vfs_response(&VfsResponse {
            request: request(41),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    assert!(matches!(
        copied.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Copy { from, to, .. },
            ..
        })] if from == "/mnt/local/drive/source.txt"
            && to == "/root/Documents/source.txt"
    ));
}

#[test]
fn external_file_drops_accept_pane_backgrounds_files_and_places() {
    for (target, expected_directory) in [
        (DIRECTORY_SCROLL, "/root"),
        (ENTRY_BASE + 1, "/root"),
        (DETAIL_SIZE_BASE, "/root/Folder"),
        (PLACE_DESKTOP, "/root/Desktop"),
        (PLACE_DOCUMENTS, "/root/Documents"),
        (super::files_view::PLACE_VIDEOS, "/root/Videos"),
        (PLACE_MOUNTS, "/mnt/local"),
    ] {
        let (mut actor, _) = loaded_actor();
        if target == PLACE_MOUNTS {
            actor.local_files_available = true;
        }
        let drag = DragId::new(50 + target).expect("drag");
        let base = DragEnvelope {
            drag,
            app: app(),
            window: super::files_view::window_id(super::files_view::WINDOW),
            target: Some(widget_id(target)),
            phase: DragPhase::Enter,
            action: DragAction::Copy,
            x: 10,
            y: 10,
            mime_types: vec![String::from("text/uri-list")],
            payload: None,
        };
        actor.handle_drag(&base, request(20)).expect("enter");
        actor
            .handle_drag(
                &DragEnvelope {
                    phase: DragPhase::Drop,
                    payload: Some(DragPayload {
                        mime_type: String::from("text/uri-list"),
                        data: b"file:///root/Downloads/source.txt\r\n".to_vec(),
                    }),
                    ..base
                },
                request(21),
            )
            .expect("drop");
        let copied = actor
            .handle_vfs_response(&VfsResponse {
                request: request(21),
                result: VfsResult::Metadata(FileMetadata {
                    kind: FileKind::File,
                    size: 4,
                    modified_millis: 9,
                    mode: 0o644,
                }),
            })
            .expect("metadata");
        let expected = format!("{expected_directory}/source.txt");
        assert!(matches!(
            copied.as_slice(),
            [
                Message::UiPatch(_),
                Message::VfsRequest(VfsRequest {
                    operation: VfsOperation::Copy { to, .. },
                    ..
                })
            ] if to == &expected
        ));
    }
}

#[test]
fn external_file_drop_targets_a_user_place() {
    let (mut actor, _) = loaded_actor();
    actor
        .preferences
        .add_bookmark(
            Bookmark::new(
                GuestPath::absolute("/root/Projects").expect("path"),
                "Projects",
            )
            .expect("bookmark"),
        )
        .expect("add bookmark");
    let drag = DragId::new(143).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(BOOKMARK_BASE)),
        phase: DragPhase::Enter,
        action: DragAction::Copy,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(35)).expect("enter");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Downloads/source.txt\r\n".to_vec(),
                }),
                ..base
            },
            request(36),
        )
        .expect("drop");
    let copied = actor
        .handle_vfs_response(&VfsResponse {
            request: request(36),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    assert!(matches!(
        copied.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Copy { to, .. },
            ..
        })] if to == "/root/Projects/source.txt"
    ));
}

#[test]
fn dropping_a_folder_on_places_adds_a_bookmark() {
    let (mut actor, _) = loaded_actor();
    let drag = DragId::new(144).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(PLACES)),
        phase: DragPhase::Enter,
        action: DragAction::Copy,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(37)).expect("enter Places");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Projects\r\n".to_vec(),
                }),
                ..base
            },
            request(38),
        )
        .expect("drop folder");
    let saved = actor
        .handle_vfs_response(&VfsResponse {
            request: request(38),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
                modified_millis: 9,
                mode: 0o755,
            }),
        })
        .expect("folder metadata");
    assert!(matches!(
        saved.as_slice(),
        [Message::UiPatch(_), Message::SettingsRequest(_)]
    ));
    assert_eq!(
        actor.preferences.bookmarks()[0].path.as_str(),
        "/root/Projects"
    );
}

#[test]
fn external_file_drag_rejects_non_filesystem_widgets_before_drop() {
    let (mut actor, _) = loaded_actor();
    let drag = DragEnvelope {
        drag: DragId::new(74).expect("drag"),
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(super::files_view::SEARCH_FIELD)),
        phase: DragPhase::Enter,
        action: DragAction::Copy,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };

    assert!(actor.handle_drag(&drag, request(30)).is_err());
    assert!(actor.active_external_drag.is_none());
}

#[test]
fn dropping_on_the_trash_place_uses_the_safe_trash_plan() {
    let (mut actor, _) = loaded_actor();
    let drag = DragId::new(75).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(PLACE_TRASH)),
        phase: DragPhase::Enter,
        action: DragAction::Move,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(30)).expect("enter");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/source.txt\r\n".to_vec(),
                }),
                ..base
            },
            request(31),
        )
        .expect("drop");
    let preflight = actor
        .handle_vfs_response(&VfsResponse {
            request: request(31),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    assert!(matches!(
        preflight.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Stat { path },
                ..
            })
        ] if path == "/root/.local/share/Trash/files/source.txt"
    ));
}

#[test]
fn finished_external_drag_cancels_after_the_inflight_stat_response() {
    let (mut actor, _) = loaded_actor();
    let drag = DragId::new(45).expect("drag");
    let base = DragEnvelope {
        drag,
        app: app(),
        window: super::files_view::window_id(super::files_view::WINDOW),
        target: Some(widget_id(ENTRY_BASE)),
        phase: DragPhase::Enter,
        action: DragAction::Copy,
        x: 10,
        y: 10,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    actor.handle_drag(&base, request(2)).expect("enter");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Documents/source.txt\r\n".to_vec(),
                }),
                ..base.clone()
            },
            request(3),
        )
        .expect("drop");
    actor
        .handle_drag(
            &DragEnvelope {
                phase: DragPhase::Finished,
                ..base
            },
            request(4),
        )
        .expect("finish");
    let cancelled = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 4,
                modified_millis: 9,
                mode: 0o644,
            }),
        })
        .expect("inflight response is consumed");
    assert!(matches!(
        cancelled.as_slice(),
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
        ] if kind == "info" && message == "Cross-application drop cancelled"
    ));
    assert!(actor.pending_external_drop.is_none());
    assert!(actor.runtime_for_test().content_notice.is_empty());
}

#[test]
fn new_window_preserves_location_and_launch_arguments_are_validated() {
    let (mut actor, _) = loaded_actor();
    let launched = actor
        .handle_event(&event(NEW_WINDOW, UiEvent::Activate), request(2))
        .expect("new window");
    assert!(matches!(
        launched.as_slice(),
        [Message::Launch {
            entrypoint,
            arguments,
            ..
        }] if entrypoint == "rice-files" && arguments == &["/root".to_owned()]
    ));

    let (launched, messages) =
        RiceFiles::launch_at(app(), &["/root/Documents".to_owned()], request(3))
            .expect("launch at directory");
    assert_eq!(launched.location().as_str(), "/root/Documents");
    assert!(matches!(
        &messages[1],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/root/Documents"
    ));
    let (outside_home, messages) = RiceFiles::launch_at(app(), &["/etc".to_owned()], request(4))
        .expect("absolute mount and system paths are valid locations");
    assert_eq!(outside_home.location().as_str(), "/etc");
    assert!(matches!(
        &messages[1],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/etc"
    ));
}

#[test]
fn folder_context_actions_open_the_selected_folder_in_tab_window_or_terminal() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select folder");
    let opened_tab = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::OPEN_IN_NEW_TAB),
                UiEvent::Activate,
            ),
            request(3),
        )
        .expect("open folder tab");
    assert!(matches!(
        opened_tab.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if path == "/root/Folder"
    ));

    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(4))
        .expect("select folder");
    let opened_window = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::OPEN_IN_NEW_WINDOW),
                UiEvent::Activate,
            ),
            request(5),
        )
        .expect("open folder window");
    assert!(matches!(
        opened_window.as_slice(),
        [Message::Launch { arguments, .. }] if arguments == &[String::from("/root/Folder")]
    ));

    let opened_terminal = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::OPEN_IN_TERMINAL),
                UiEvent::Activate,
            ),
            request(6),
        )
        .expect("open folder terminal");
    assert!(matches!(
        opened_terminal.as_slice(),
        [Message::Launch {
            entrypoint,
            arguments,
            ..
        }] if entrypoint == "rice-term"
            && arguments == &[String::from("cd -- '/root/Folder'")]
    ));
}

#[test]
fn background_context_opens_terminal_in_the_current_folder() {
    let (mut actor, _) = loaded_actor();
    let opened = actor
        .handle_event(
            &event(
                files_menu::context_widget(files_menu::OPEN_IN_TERMINAL),
                UiEvent::Activate,
            ),
            request(2),
        )
        .expect("open current folder terminal");
    assert!(matches!(
        opened.as_slice(),
        [Message::Launch {
            entrypoint,
            arguments,
            ..
        }] if entrypoint == "rice-term" && arguments == &[String::from("cd -- '/root'")]
    ));
}

#[test]
fn multi_selection_copy_can_cancel_between_files() {
    let (mut actor, _) = loaded_actor();
    for (target, modifiers) in [(ENTRY_BASE + 1, 0), (ENTRY_BASE + 2, 4)] {
        actor
            .handle_event(
                &event(
                    target,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Up,
                        x: 0,
                        y: 0,
                        button: 1,
                        modifiers,
                    },
                ),
                request(2),
            )
            .expect("multi-select");
    }
    actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("copy dialog");
    set_dialog_field(&mut actor, "/root/Folder");
    let started = confirm(&mut actor, 4);
    assert!(matches!(
        started.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    let cancellation = actor
        .handle_event(
            &event(super::files_progress::CANCEL, UiEvent::Activate),
            request(5),
        )
        .expect("request cancellation");
    assert!(matches!(
        cancellation.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: cancel,
                operation: VfsOperation::Cancel { target },
                ..
            })
        ] if *cancel == request(5) && *target == request(4)
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("provider accepts cancellation");
    let stopped = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Cancelled,
        })
        .expect("provider stops current item");
    assert!(matches!(
        stopped.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { .. },
                ..
            }),
            Message::HostRequest(_)
        ]
    ));
    assert!(actor.pending_mutation.is_none());
    assert!(actor.runtime_for_test().content_notice.is_empty());
    assert!(
        actor
            .runtime_for_test()
            .pending_directory
            .as_ref()
            .and_then(|pending| pending.completion_notice.as_deref())
            .is_some_and(|notice| notice.contains("cancelled after 0/2"))
    );
}

#[test]
fn pointer_drag_moves_or_shift_copies_into_a_directory() {
    for (modifiers, expected_copy) in [(0, false), (1, true)] {
        let (mut actor, _) = loaded_actor();
        actor
            .handle_event(
                &event(
                    ENTRY_BASE + 1,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 12,
                        y: 12,
                        button: 1,
                        modifiers,
                    },
                ),
                request(2),
            )
            .expect("drag start");
        let dropped = actor
            .handle_event(
                &event(
                    ENTRY_BASE,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Up,
                        x: 16,
                        y: 16,
                        button: 1,
                        modifiers,
                    },
                ),
                request(3),
            )
            .expect("drop");
        assert!(
            matches!(
                dropped.as_slice(),
                [
                    Message::UiPatch(_),
                    Message::VfsRequest(VfsRequest {
                        operation: VfsOperation::Rename { from, to },
                        ..
                    }),
                    Message::DragSource(_)
                ] if !expected_copy
                    && from == "/root/file2.txt"
                    && to == "/root/Folder/file2.txt"
            ) || matches!(
                dropped.as_slice(),
                [
                    Message::UiPatch(_),
                    Message::VfsRequest(VfsRequest {
                        operation: VfsOperation::Copy {
                            from,
                            to,
                            recursive: false,
                        },
                        ..
                    }),
                    Message::DragSource(_)
                ] if expected_copy
                    && from == "/root/file2.txt"
                    && to == "/root/Folder/file2.txt"
            )
        );
    }
}

#[test]
fn copy_conflicts_can_rename_or_replace_the_destination() {
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("copy");
    set_dialog_field(&mut actor, "/root/Folder");
    let _started = confirm(&mut actor, 4);
    let conflict = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::AlreadyExists,
                message: String::from("exists"),
            }),
        })
        .expect("conflict");
    assert!(matches!(conflict.as_slice(), [Message::UiPatch(_)]));
    set_dialog_field(&mut actor, "/root/Folder/file2-copy.txt");
    let renamed = confirm(&mut actor, 5);
    assert!(matches!(
        renamed.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy { to, .. },
                ..
            })
        ] if to == "/root/Folder/file2-copy.txt"
    ));

    let (mut replace_actor, _) = loaded_actor();
    select_file(&mut replace_actor);
    replace_actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("copy");
    set_dialog_field(&mut replace_actor, "/root/Folder");
    let _started = confirm(&mut replace_actor, 4);
    replace_actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::AlreadyExists,
                message: String::from("exists"),
            }),
        })
        .expect("conflict");
    let replace = replace_actor
        .handle_event(
            &event(files_dialogs::CONFLICT_REPLACE, UiEvent::Activate),
            request(5),
        )
        .expect("replace");
    assert!(matches!(
        replace.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Rename {
                    from,
                    to,
                },
                ..
            })
        ] if from == "/root/Folder/file2.txt"
            && to == "/root/Folder/.file2.txt.ricewm-backup-4-0"
    ));
    let retry = replace_actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Done,
        })
        .expect("remove conflicting item");
    assert!(matches!(
        retry.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy { to, .. },
                ..
            })
        ] if to == "/root/Folder/file2.txt"
    ));
}

#[test]
fn localized_operation_notices_and_new_folder_drafts_preserve_user_input() {
    let (mut actor, _) = loaded_actor();
    rice_i18n::with_locale(2, || {
        let messages = actor.show_rename().unwrap();
        assert!(messages.iter().any(|message| matches!(message,
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification {title, message, ..}, ..
            }) if title == "文件" && message == "请先选择一个要重命名的项目"
        )));
        actor.show_create_folder().unwrap();
        assert!(matches!(&actor.dialog, Some(files_dialogs::Dialog::CreateFolder {value}) if value == rice_i18n::t("New Folder")));
        actor.update_operation_field("Documents {name} 中文").unwrap();
    });
    rice_i18n::with_locale(1, || {
        actor.emit_full().unwrap();
        assert!(matches!(&actor.dialog, Some(files_dialogs::Dialog::CreateFolder {value}) if value == "Documents {name} 中文"));
    });
}

#[test]
fn translated_places_errors_preserve_bookmark_names_and_host_details() {
    rice_i18n::with_locale(2, || {
        let (mut actor, _) = loaded_actor();
        super::files_preferences::add_paths(&mut actor,
            vec![GuestPath::absolute("/root/Projects/Documents {error}").unwrap()], request(80)).unwrap();
        let bookmark = actor.preferences.bookmarks().iter().find(|bookmark| bookmark.label == "Documents {error}").unwrap();
        assert_eq!(bookmark.path.as_str(), "/root/Projects/Documents {error}");
        let messages = super::files_preferences::handle_response(&mut actor, &SettingsResponse {
            request: request(80), result: SettingsResult::Error("Cannot write /root/{error}".into()),
        }).unwrap();
        assert!(messages.iter().any(|message| matches!(message,
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification {message, ..}, ..
            }) if message == "无法保存位置：Cannot write /root/{error}"
        )));
    });
}

#[test]
fn mutation_notifications_translate_at_completion_and_keep_host_details() {
    for (locale, expected) in [(1, "Copiar se completó para 1 elemento"), (2, "复制已完成，共 1 项")] {
        let (mut actor, _) = loaded_actor();
        select_file(&mut actor);
        actor.handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3)).unwrap();
        set_dialog_field(&mut actor, "/root/Folder");
        let started = confirm(&mut actor, 4);
        assert!(started.iter().any(|message| matches!(message, Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Copy { to, .. }, ..
        }) if to == "/root/Folder/file2.txt")));
        rice_i18n::with_locale(locale, || {
            let messages = actor.handle_vfs_response(&VfsResponse { request: request(4), result: VfsResult::Done }).unwrap();
            assert!(messages.iter().any(|message| matches!(message,
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification { kind, message, .. }, ..
                }) if kind == "info" && message == expected)), "{messages:?}");
        });
    }
    let (mut actor, _) = loaded_actor();
    select_file(&mut actor);
    actor.handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3)).unwrap();
    set_dialog_field(&mut actor, "/root/Folder");
    confirm(&mut actor, 4);
    rice_i18n::with_locale(2, || {
        let messages = actor.handle_vfs_response(&VfsResponse { request: request(4), result: VfsResult::Error(VfsError {
            code: VfsErrorCode::PermissionDenied, message: "host denied {operation} /root/Folder".into(),
        }) }).unwrap();
        assert!(messages.iter().any(|message| matches!(message,
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification { kind, message, .. }, ..
            }) if kind == "error" && message == "复制在完成 0/1 项后停止：host denied {operation} /root/Folder")), "{messages:?}");
    });
}

#[test]
fn download_empty_selection_notification_uses_current_locale_without_starting_io() {
    let (mut actor, _) = loaded_actor();
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            let messages = actor.handle_event(
                &event(files_menu::DOWNLOAD_TO_HOST, UiEvent::Activate), request(91),
            ).unwrap();
            assert!(messages.iter().all(|message| !matches!(message,
                Message::VfsRequest(_) | Message::SettingsRequest(_))));
            let notices = messages.iter().filter_map(|message| match message {
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification { title, message, .. }, ..
                }) => Some((title.as_str(), message.as_str())),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(notices, vec![(rice_i18n::t("Files"), rice_i18n::t("Select at least one item before downloading"))]);
        });
    }
}
