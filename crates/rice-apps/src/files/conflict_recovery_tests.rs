use super::{
    ENTRY_BASE, Message, RiceFiles, UiEvent, files_dialogs, files_menu, files_progress,
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    PointerEventKind, VfsError, VfsErrorCode, VfsOperation, VfsRequest, VfsResponse, VfsResult,
};

fn select_two(actor: &mut RiceFiles) {
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
            .expect("select files");
    }
}

fn begin_copy(actor: &mut RiceFiles, id: u64) -> Vec<Message> {
    actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("copy dialog");
    actor
        .handle_event(
            &event(
                files_dialogs::FIELD,
                UiEvent::ValueChanged(String::from("/root/Folder")),
            ),
            request(3),
        )
        .expect("copy destination");
    actor
        .handle_event(
            &event(files_dialogs::CONFIRM, UiEvent::Activate),
            request(id),
        )
        .expect("begin copy")
}

fn conflict(actor: &mut RiceFiles, id: u64) -> Vec<Message> {
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(id),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::AlreadyExists,
                message: String::from("destination exists"),
            }),
        })
        .expect("conflict response")
}

fn done(actor: &mut RiceFiles, id: u64) -> Vec<Message> {
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(id),
            result: VfsResult::Done,
        })
        .expect("VFS completion")
}

#[test]
fn apply_all_skip_resolves_later_conflicts_without_another_dialog() {
    let (mut actor, _) = loaded_actor();
    select_two(&mut actor);
    assert!(matches!(
        begin_copy(&mut actor, 4).as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    assert!(matches!(
        conflict(&mut actor, 4).as_slice(),
        [Message::UiPatch(_)]
    ));
    actor
        .handle_event(
            &event(files_dialogs::CONFLICT_APPLY_ALL, UiEvent::Activate),
            request(5),
        )
        .expect("apply all");
    let next = actor
        .handle_event(
            &event(files_dialogs::CONFLICT_SKIP, UiEvent::Activate),
            request(6),
        )
        .expect("skip first");
    assert!(matches!(
        next.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Copy { .. },
                ..
            })
        ]
    ));
    let completed = conflict(&mut actor, 4);
    assert!(matches!(
        completed.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { .. },
                ..
            }),
            Message::HostRequest(_)
        ]
    ));
    assert!(actor.dialog.is_none());
    assert!(actor.pending_mutation.is_none());
}

#[test]
fn failed_replace_removes_partial_and_restores_preserved_original() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select");
    let _ = begin_copy(&mut actor, 4);
    let _ = conflict(&mut actor, 4);
    let preserve = actor
        .handle_event(
            &event(files_dialogs::CONFLICT_REPLACE, UiEvent::Activate),
            request(5),
        )
        .expect("replace");
    assert!(matches!(
        preserve.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Rename { from, to },
                ..
            })
        ] if from == "/root/Folder/file2.txt"
            && to.contains(".file2.txt.ricewm-backup-4-0")
    ));
    let retry = done(&mut actor, 4);
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
    let cleanup = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("write denied"),
            }),
        })
        .expect("injected permission failure");
    assert!(matches!(
        cleanup.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Remove { path, .. },
                ..
            })
        ] if path == "/root/Folder/file2.txt"
    ));
    let restore = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("no partial was created"),
            }),
        })
        .expect("partial cleanup");
    assert!(matches!(
        restore.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Rename { from, to },
                ..
            })
        ] if from.contains(".file2.txt.ricewm-backup-4-0")
            && to == "/root/Folder/file2.txt"
    ));
    let restored = done(&mut actor, 4);
    assert!(matches!(
        restored.as_slice(),
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
        ] if kind == "error" && message.contains("original was restored")
    ));
    assert!(actor.pending_mutation.is_none());
    assert!(actor.runtime_for_test().content_notice.is_empty());
}

#[test]
fn cancellation_during_preservation_restores_before_terminating() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select");
    let _ = begin_copy(&mut actor, 4);
    let _ = conflict(&mut actor, 4);
    actor
        .handle_event(
            &event(files_dialogs::CONFLICT_REPLACE, UiEvent::Activate),
            request(5),
        )
        .expect("replace");
    actor
        .handle_event(
            &event(files_progress::CANCEL, UiEvent::Activate),
            request(6),
        )
        .expect("cancel replacement");
    let restore = done(&mut actor, 4);
    assert!(matches!(
        restore.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Rename { from, to },
                ..
            })
        ] if from.contains(".file2.txt.ricewm-backup-4-0")
            && to == "/root/Folder/file2.txt"
    ));
    let cancelled = done(&mut actor, 4);
    assert!(matches!(
        cancelled.as_slice(),
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
            .is_some_and(|notice| notice.contains("cancelled"))
    );
}
