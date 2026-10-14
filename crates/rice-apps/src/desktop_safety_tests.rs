use super::*;

#[test]
fn move_to_trash_checks_collisions_then_renames_without_removing() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Folder", FileKind::Directory)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "Folder")
        .expect("Folder")
        .id;
    open_item_context(&mut desktop, item, request(2));
    let probe = desktop
        .handle_event(&event(DELETE, UiEvent::Activate), request(3))
        .expect("trash probe begins");
    assert!(matches!(
        &probe[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, cursor: 0, .. },
            ..
        }) if path == desktop_vfs::TRASH_FILES_PATH
    ));
    assert!(!probe.iter().any(is_remove_request));
    let rename = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("collision-free Trash");
    assert!(matches!(
        &rename[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { from, to },
            ..
        }) if from == "/root/Desktop/Folder"
            && to == "/root/.local/share/Trash/files/Folder"
    ));
    assert!(!rename.iter().any(is_remove_request));
    let failed = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("read only"),
            }),
        })
        .expect("trash rename error represented");
    assert!(failed.iter().any(is_notification));
    assert!(desktop.items.iter().any(|item| item.label == "Folder"));
}

#[test]
fn move_to_trash_fails_safely_when_destination_is_missing_or_collides() {
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
    desktop
        .handle_event(&event(DELETE, UiEvent::Activate), request(3))
        .expect("trash probe");
    let collision = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: vec![directory_entry("notes.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("collision");
    assert!(collision.iter().any(is_notification));
    assert!(!collision.iter().any(is_destructive_vfs_request));

    desktop
        .handle_event(&event(DELETE, UiEvent::Activate), request(4))
        .expect("second trash probe");
    let missing = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("missing"),
            }),
        })
        .expect("missing Trash");
    assert!(missing.iter().any(is_notification));
    assert!(!missing.iter().any(is_destructive_vfs_request));
}

fn is_remove_request(message: &Message) -> bool {
    matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove { .. },
            ..
        })
    )
}

fn is_destructive_vfs_request(message: &Message) -> bool {
    matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove { .. } | VfsOperation::Rename { .. },
            ..
        })
    )
}

fn is_notification(message: &Message) -> bool {
    matches!(
        message,
        Message::UiPatch(envelope)
            if envelope.patches.iter().any(|patch| matches!(
                patch,
                UiPatch::Create { kind, .. } if *kind == WidgetKind::Notification as u16
            ) || matches!(patch, UiPatch::Update { id, .. } if *id == widget_id(NOTICE)))
    )
}
