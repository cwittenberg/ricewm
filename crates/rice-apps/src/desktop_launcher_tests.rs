use super::*;
use rice_native_app_protocol::{
    ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection,
};

#[test]
fn desktop_launcher_is_stat_checked_parsed_and_allowlisted_before_launch() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![{
            let mut entry = directory_entry("RicePaper.desktop", FileKind::File);
            entry.metadata.mode = 0o755;
            entry
        }],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "RicePaper")
        .expect("launcher")
        .id;
    let inspect = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("activation");
    assert!(matches!(
        inspect.last(),
        Some(Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })) if path == "/root/Desktop/RicePaper.desktop"
    ));
    let read = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 64,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    assert!(matches!(
        &read[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Read { path, offset: 0, .. },
            ..
        }) if path == "/root/Desktop/RicePaper.desktop"
    ));
    let launch = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Data(b"[Desktop Entry]\nType=Application\nExec=leafpad\n".to_vec()),
        })
        .expect("launcher data");
    assert!(matches!(
        &launch[0],
        Message::Launch {
            entrypoint,
            arguments,
            ..
        } if entrypoint == "rice-paper" && arguments.is_empty()
    ));
}

#[test]
fn browser_local_mount_desktop_link_opens_its_mnt_local_folder() {
    let mut desktop = launch();
    let mut launcher = directory_entry("ricewm-local-project.desktop", FileKind::File);
    launcher.metadata.mode = 0o755;
    enumerate(&mut desktop, request(1), vec![launcher]);
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "Project")
        .expect("browser mount desktop link")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("activation");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 128,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    let launched = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Data(
                b"[Desktop Entry]\nType=Link\nName=Project\nURL=file:///mnt/local/My%20Project\n"
                    .to_vec(),
            ),
        })
        .expect("desktop link");
    assert!(matches!(
        &launched[0],
        Message::Launch {
            entrypoint,
            arguments,
            ..
        } if entrypoint == "rice-files" && arguments == &[String::from("/mnt/local/My Project")]
    ));
}

#[test]
fn unsupported_desktop_launcher_is_reported_in_the_retained_ui() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![{
            let mut entry = directory_entry("Unsafe.desktop", FileKind::File);
            entry.metadata.mode = 0o755;
            entry
        }],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "Unsafe")
        .expect("launcher")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("activation");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 64,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    let rejected = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Data(
                b"[Desktop Entry]\nType=Application\nExec=leafpad %Z\n".to_vec(),
            ),
        })
        .expect("rejection");
    let patch = envelope(rejected[0].clone());
    assert!(patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            id,
            kind,
            text: Some(text),
            ..
        } if *id == widget_id(NOTICE)
            && *kind == WidgetKind::Notification as u16
            && text.contains("unsupported field code")
    )));
}

#[test]
fn installed_dynamic_desktop_launcher_is_delegated_by_path_only() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![{
            let mut entry = directory_entry("geany.desktop", FileKind::File);
            entry.metadata.mode = 0o755;
            entry
        }],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "Geany")
        .expect("dynamic launcher")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("activation");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 64,
                modified_millis: 1,
                mode: 0o755,
            }),
        })
        .expect("metadata");
    let delegated = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Data(b"[Desktop Entry]\nType=Application\nExec=geany %F\n".to_vec()),
        })
        .expect("dynamic launcher");
    assert!(matches!(
        &delegated[0],
        Message::Launch {
            entrypoint,
            arguments,
            ..
        } if entrypoint == "rice-desktop-entry"
            && arguments == &[String::from("/root/Desktop/geany.desktop")]
    ));
}

#[test]
fn new_file_checks_for_collision_before_issuing_a_create_write() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 300,
                    y: 300,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("background context");
    let inspect = desktop
        .handle_event(&event(NEW_FILE, UiEvent::Activate), request(3))
        .expect("new file");
    assert!(matches!(
        &inspect[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }) if path == "/root/Desktop/new file.txt"
    ));
    let create = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("available"),
            }),
        })
        .expect("name available");
    assert!(matches!(
        &create[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Write { path, data, .. },
            ..
        }) if path == "/root/Desktop/new file.txt" && data.is_empty()
    ));
}

#[test]
fn repeated_new_files_choose_a_numbered_txt_name_instead_of_showing_an_error() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let first_check = desktop
        .handle_event(&event(NEW_FILE, UiEvent::Activate), request(2))
        .expect("new file action");
    assert!(matches!(
        first_check.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/new file.txt"
    ));

    let second_check = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 0,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("default name collision");
    assert!(matches!(
        second_check.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/new file (2).txt"
    ));

    let create = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("available"),
            }),
        })
        .expect("numbered name available");
    assert!(matches!(
        create.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Write { path, data, .. },
            ..
        })] if path == "/root/Desktop/new file (2).txt" && data.is_empty()
    ));
}

#[test]
fn desktop_file_clipboard_roundtrips_uri_lists_into_collision_checked_copy() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("notes.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "notes.txt")
        .expect("notes")
        .id;
    open_item_context(&mut desktop, item, request(2));
    let copied = desktop
        .handle_event(&event(COPY, UiEvent::Activate), request(3))
        .expect("copy");
    assert!(matches!(
        &copied[0],
        Message::ClipboardRequest(ClipboardRequest::Write {
            selection: ClipboardSelection::Clipboard,
            mime_type,
            data,
            ..
        }) if mime_type == "x-special/gnome-copied-files"
            && String::from_utf8_lossy(data).contains("file:///root/Desktop/notes.txt")
    ));
    desktop
        .handle_clipboard_response(&ClipboardResponse {
            request: request(3),
            result: ClipboardResult::Data {
                mime_type: String::from("x-special/gnome-copied-files"),
                data: Vec::new(),
            },
        })
        .expect("copy acknowledged");
    desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 400,
                    y: 400,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request(4),
        )
        .expect("background");
    let paste = desktop
        .handle_event(&event(PASTE, UiEvent::Activate), request(5))
        .expect("paste");
    assert!(matches!(
        &paste[0],
        Message::ClipboardRequest(ClipboardRequest::Read { .. })
    ));
    let collision_check = desktop
        .handle_clipboard_response(&ClipboardResponse {
            request: request(5),
            result: ClipboardResult::Data {
                mime_type: String::from("x-special/gnome-copied-files"),
                data: b"copy\nfile:///root/Documents/source.txt\n".to_vec(),
            },
        })
        .expect("clipboard files");
    assert!(matches!(
        &collision_check[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }) if path == "/root/Desktop/source.txt"
    ));
    let copy = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("available"),
            }),
        })
        .expect("destination available");
    assert!(matches!(
        &copy[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Copy {
                from,
                to,
                recursive: true,
            },
            ..
        }) if from == "/root/Documents/source.txt" && to == "/root/Desktop/source.txt"
    ));
}

#[test]
fn empty_trash_requires_confirmation_then_removes_enumerated_items() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let trash = desktop
        .items
        .iter()
        .find(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
        .expect("Trash")
        .id;
    let inspect = desktop
        .handle_event(
            &event(
                widget_for_item(trash).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 40,
                    y: 40,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("Trash menu probe");
    assert!(matches!(
        inspect.last(),
        Some(Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })) if path == desktop_vfs::TRASH_FILES_PATH
    ));
    let menu = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: vec![directory_entry("old.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("Trash contents");
    let menu = envelope(menu[0].clone());
    assert!(menu.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            id,
            text: Some(text),
            ..
        } if *id == widget_id(EMPTY_TRASH) && text == "Empty Trash"
    )));
    let confirm = desktop
        .handle_event(&event(EMPTY_TRASH, UiEvent::Activate), request(3))
        .expect("confirmation");
    let confirm = envelope(confirm[0].clone());
    assert!(confirm.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Update {
            id,
            text: rice_native_app_protocol::PatchValue::Set(text),
            ..
        } if *id == widget_id(EMPTY_TRASH) && text.contains("Confirm")
    )));
    let enumerate_trash = desktop
        .handle_event(&event(EMPTY_TRASH, UiEvent::Activate), request(4))
        .expect("confirmed empty");
    assert!(matches!(
        &enumerate_trash[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { .. },
            ..
        })
    ));
    let remove = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: vec![directory_entry("old.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("confirmed enumeration");
    assert!(matches!(
        &remove[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove {
                path,
                recursive: false,
            },
            ..
        }) if path == "/root/.local/share/Trash/files/old.txt"
    ));
}

#[test]
fn empty_trash_rejects_unsafe_directory_entries_before_removing_anything() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    desktop.trash_confirm_pending = true;
    desktop
        .handle_event(&event(EMPTY_TRASH, UiEvent::Activate), request(3))
        .expect("confirmed enumeration");

    let stopped = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: vec![directory_entry("../escape", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("unsafe entry is contained");
    assert!(
        stopped
            .iter()
            .all(|message| !matches!(message, Message::VfsRequest(_)))
    );
    let stopped = envelope(stopped[0].clone());
    assert!(stopped.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create {
            kind,
            text: Some(text),
            ..
        } if *kind == WidgetKind::Notification as u16 && text.contains("unsafe name")
    )));
}
