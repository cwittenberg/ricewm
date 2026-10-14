use super::{
    ENTRY_BASE, Message, RiceFiles, UiEvent, files_dialogs, files_menu, files_progress,
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    VfsError, VfsErrorCode, VfsOperation, VfsProgress, VfsRequest, VfsResponse, VfsResult,
};

fn copying_actor() -> RiceFiles {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select source");
    actor
        .handle_event(&event(files_menu::COPY_TO, UiEvent::Activate), request(3))
        .expect("open copy dialog");
    actor
        .handle_event(
            &event(
                files_dialogs::FIELD,
                UiEvent::ValueChanged(String::from("/root/Folder")),
            ),
            request(3),
        )
        .expect("set destination");
    let started = actor
        .handle_event(
            &event(files_dialogs::CONFIRM, UiEvent::Activate),
            request(4),
        )
        .expect("start copy");
    assert!(matches!(
        started.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            request: operation,
            operation: VfsOperation::Copy { .. },
            ..
        })] if *operation == request(4)
    ));
    actor
}

#[test]
fn provider_cancel_requires_ack_and_terminal_then_ignores_late_completion() {
    let mut actor = copying_actor();
    let requested = actor
        .handle_event(
            &event(files_progress::CANCEL, UiEvent::Activate),
            request(5),
        )
        .expect("request cancellation");
    assert!(matches!(
        requested.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            request: cancel,
            operation: VfsOperation::Cancel { target },
            ..
        })] if *cancel == request(5) && *target == request(4)
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Progress(VfsProgress {
                sequence: 1,
                completed_bytes: 65_536,
                total_bytes: 262_144,
            }),
        })
        .expect("ordered progress");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("cancel accepted");
    let terminal = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Cancelled,
        })
        .expect("operation actually cancelled");
    assert!(matches!(
        terminal.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: refresh,
                operation: VfsOperation::ReadDirectory { .. },
                ..
            }),
            Message::HostRequest(_)
        ] if *refresh == request(5)
    ));
    assert!(
        actor
            .handle_vfs_response(&VfsResponse {
                request: request(4),
                result: VfsResult::Done,
            })
            .expect("late completion is reconciled")
            .is_empty()
    );
}

#[test]
fn unsupported_cancel_keeps_the_real_operation_running() {
    let mut actor = copying_actor();
    actor
        .handle_event(
            &event(files_progress::CANCEL, UiEvent::Activate),
            request(5),
        )
        .expect("request cancellation");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotSupported,
                message: String::from("copy cannot be cancelled"),
            }),
        })
        .expect("denial returns operation to running");
    let completed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Done,
        })
        .expect("original operation completes normally");
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
}

#[test]
fn coalesced_progress_may_skip_sequences_but_never_move_backwards() {
    let mut actor = copying_actor();

    let update = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Progress(VfsProgress {
                sequence: 37,
                completed_bytes: 65_536,
                total_bytes: 1_048_576,
            }),
        })
        .expect("coalesced progress");
    assert!(matches!(update.as_slice(), [Message::UiPatch(_)]));
    assert!(
        actor
            .handle_vfs_response(&VfsResponse {
                request: request(4),
                result: VfsResult::Progress(VfsProgress {
                    sequence: 36,
                    completed_bytes: 65_536,
                    total_bytes: 1_048_576,
                }),
            })
            .is_err()
    );
}
