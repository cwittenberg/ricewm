use super::{
    DELETE, ENTRY_BASE, Message, UiEvent, VfsOperation, VfsRequest,
    files_view::widget_id,
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    FileKind, FileMetadata, PointerEventKind, VfsError, VfsErrorCode, VfsResponse, VfsResult,
};

#[test]
fn trash_preflights_collision_and_uses_rename_never_remove() {
    let (mut actor, _) = loaded_actor();
    let selected = actor
        .runtime_for_test()
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    actor.pane_mut().expect("pane").set_selection(selected);
    let started = actor
        .handle_event(&event(DELETE, UiEvent::Activate), request(5))
        .expect("start trash");
    assert!(matches!(
        &started[1],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }) if path == "/root/.local/share/Trash/files/Folder"
    ));
    assert_no_remove(&started);

    let rename = actor
        .handle_vfs_response(&not_found_response(5))
        .expect("destination is clear");
    assert!(matches!(
        rename.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Rename { from, to },
                ..
            })
        ] if from == "/root/Folder"
            && to == "/root/.local/share/Trash/files/Folder"
    ));
    assert_no_remove(&rename);

    let next = actor
        .handle_vfs_response(&done_response(5))
        .expect("first rename done");
    assert!(matches!(
        next.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Stat { path },
                ..
            })
        ] if path.starts_with("/root/.local/share/Trash/files/")
    ));
    assert_no_remove(&next);
}

#[test]
fn trash_collision_fails_before_any_rename() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select");
    actor
        .handle_event(&event(DELETE, UiEvent::Activate), request(3))
        .expect("preflight");
    let stopped = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
                modified_millis: 0,
                mode: 0o755,
            }),
        })
        .expect("collision is a safe state");
    assert!(matches!(
        stopped.as_slice(),
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
        ] if kind == "error" && message.contains("already contains")
    ));
    assert!(actor.pending_mutation.is_none());
    assert!(actor.runtime_for_test().content_notice.is_empty());
}

#[test]
fn secondary_click_selects_context_target_not_stale_selection() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select first");
    actor
        .handle_event(
            &event(
                ENTRY_BASE + 1,
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 10,
                    y: 10,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("context-select second");
    assert_eq!(
        actor
            .pane()
            .expect("pane")
            .selection()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        [String::from("/root/file2.txt")]
    );
    assert_eq!(widget_id(ENTRY_BASE + 1).get(), ENTRY_BASE + 1);
}

fn assert_no_remove(messages: &[Message]) {
    assert!(!messages.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove { .. },
            ..
        })
    )));
}

fn done_response(value: u64) -> VfsResponse {
    VfsResponse {
        request: request(value),
        result: VfsResult::Done,
    }
}

fn not_found_response(value: u64) -> VfsResponse {
    VfsResponse {
        request: request(value),
        result: VfsResult::Error(VfsError {
            code: VfsErrorCode::NotFound,
            message: String::from("not found"),
        }),
    }
}
