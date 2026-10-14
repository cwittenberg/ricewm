use super::{
    ENTRY_BASE, Message, UiEvent, files_dialogs, files_menu, files_properties,
    files_view::{HELP_ABOUT, widget_id},
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    ClipboardRequest, FileKind, FileMetadata, UiPatch, VfsOperation, VfsRequest, VfsResponse,
    VfsResult,
};

#[test]
fn properties_and_about_are_real_closeable_dialogs() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    let properties = actor
        .handle_event(
            &event(files_menu::PROPERTIES, UiEvent::Activate),
            request(3),
        )
        .expect("properties");
    let [
        Message::UiPatch(properties),
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }),
    ] = properties.as_slice()
    else {
        panic!("properties patch and metadata request");
    };
    assert_eq!(path, "/root/file2.txt");
    assert!(properties.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                text: Some(text), ..
            } if text.contains("file2.txt")
        )
    }));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 2,
                modified_millis: 10,
                mode: 0o644,
            }),
        })
        .expect("properties metadata");
    let access = actor
        .handle_event(
            &event(files_properties::ACCESS_READ_ONLY, UiEvent::Activate),
            request(4),
        )
        .expect("choose read-only access");
    let [Message::UiPatch(access)] = access.as_slice() else {
        panic!("access choice redraws properties");
    };
    assert!(access.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(files_properties::MODE_FIELD) && text == "0444"
    )));
    let copied = actor
        .handle_event(
            &event(files_properties::COPY_LOCATION, UiEvent::Activate),
            request(5),
        )
        .expect("copy location");
    assert!(matches!(
        copied.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write {
            mime_type,
            data,
            ..
        })] if mime_type == "text/plain;charset=utf-8" && data == b"/root/file2.txt"
    ));
    actor
        .handle_event(&event(files_dialogs::CANCEL, UiEvent::Activate), request(6))
        .expect("close");
    let about = actor
        .handle_event(&event(HELP_ABOUT, UiEvent::Activate), request(7))
        .expect("about");
    let [Message::UiPatch(about)] = about.as_slice() else {
        panic!("about patch");
    };
    assert!(about.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, .. }
                if *id == widget_id(files_dialogs::DIALOG)
        )
    }));
}

#[test]
fn redmond_property_edits_rename_chmod_verify_and_copy_the_real_new_path() {
    let (mut actor, _) = loaded_actor();
    actor.redmond98 = true;
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(
            &event(files_menu::PROPERTIES, UiEvent::Activate),
            request(3),
        )
        .expect("properties");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 2,
                modified_millis: 10,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    actor
        .handle_event(
            &event(
                files_properties::NAME_FIELD,
                UiEvent::ValueChanged(String::from("renamed.txt")),
            ),
            request(4),
        )
        .expect("edit real name");
    actor
        .handle_event(
            &event(files_properties::EXECUTABLE, UiEvent::Activate),
            request(5),
        )
        .expect("draft executable mode");
    let apply = actor
        .handle_event(
            &event(files_properties::APPLY_MODE, UiEvent::Activate),
            request(6),
        )
        .expect("apply properties");
    assert!(matches!(
        apply.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { from, to }, ..
        })] if from == "/root/file2.txt" && to == "/root/renamed.txt"
    ));
    let chmod = actor
        .handle_vfs_response(&VfsResponse {
            request: request(6),
            result: VfsResult::Done,
        })
        .expect("rename complete");
    assert!(matches!(
        chmod.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::SetMode { path, mode: 0o755 }, ..
        })] if path == "/root/renamed.txt"
    ));
    let verify = actor
        .handle_vfs_response(&VfsResponse {
            request: request(6),
            result: VfsResult::Done,
        })
        .expect("chmod complete");
    assert!(matches!(
        verify.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path }, ..
        })] if path == "/root/renamed.txt"
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(6),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 2,
                modified_millis: 10,
                mode: 0o755,
            }),
        })
        .expect("verified properties");
    let copied = actor
        .handle_event(
            &event(files_properties::COPY_LOCATION, UiEvent::Activate),
            request(7),
        )
        .expect("copy renamed path");
    assert!(matches!(
        copied.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write { data, .. })]
            if data == b"/root/renamed.txt"
    ));
}

#[test]
fn redmond_properties_ok_commits_real_changes_before_closing() {
    let (mut actor, _) = loaded_actor();
    actor.redmond98 = true;
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(
            &event(files_menu::PROPERTIES, UiEvent::Activate),
            request(3),
        )
        .expect("properties");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 2,
                modified_millis: 10,
                mode: 0o644,
            }),
        })
        .expect("metadata");
    actor
        .handle_event(
            &event(files_properties::EXECUTABLE, UiEvent::Activate),
            request(4),
        )
        .expect("draft executable mode");
    let commit = actor
        .handle_event(
            &event(files_dialogs::PROPERTIES_OK, UiEvent::Activate),
            request(5),
        )
        .expect("OK commits");
    assert!(matches!(
        commit.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::SetMode { path, mode: 0o755 }, ..
        })] if path == "/root/file2.txt"
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("chmod complete");
    let closed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 2,
                modified_millis: 10,
                mode: 0o755,
            }),
        })
        .expect("verified and closed");
    assert!(actor.dialog.is_none());
    assert!(matches!(closed.as_slice(), [Message::UiPatch(_)]));
}
