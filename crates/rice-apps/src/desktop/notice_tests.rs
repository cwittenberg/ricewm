use super::*;
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, FileMetadata, VfsError, VfsErrorCode, VfsOperation, VfsResponse,
    VfsResult,
};

fn request(id: u64) -> RequestId {
    RequestId::new(id).unwrap()
}
fn entry(name: &str) -> DirectoryEntry {
    DirectoryEntry {
        name: name.into(),
        metadata: FileMetadata {
            kind: FileKind::File,
            size: 12,
            modified_millis: 1,
            mode: 0o644,
        },
        mime_type: None,
    }
}
fn respond(desktop: &mut RiceDesktop, id: u64, result: VfsResult) -> Vec<Message> {
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(id),
            result,
        })
        .unwrap()
}
fn directory(names: &[&str]) -> VfsResult {
    VfsResult::Directory {
        entries: names.iter().map(|name| entry(name)).collect(),
        next_cursor: None,
    }
}
fn launch(names: &[&str]) -> RiceDesktop {
    let (mut desktop, _) = RiceDesktop::launch(AppId::new(951).unwrap(), &[], request(1)).unwrap();
    respond(&mut desktop, 1, directory(names));
    desktop
}
fn select(desktop: &mut RiceDesktop, name: &str, mode: ClickMode) {
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == name)
        .unwrap()
        .id;
    desktop
        .model
        .execute(DesktopCommand::Click { item, mode })
        .unwrap();
}
fn error(value: &str) -> VfsResult {
    VfsResult::Error(VfsError {
        code: VfsErrorCode::PermissionDenied,
        message: value.into(),
    })
}
fn assert_refresh(desktop: &mut RiceDesktop, expected: impl Fn() -> String) {
    let before = desktop.clone();
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            let messages = desktop.refresh_language().unwrap();
            assert!(
                messages
                    .iter()
                    .all(|message| matches!(message, Message::UiPatch(_)))
            );
            let text = desktop
                .tree
                .get(widget_id(NOTICE))
                .unwrap()
                .text
                .as_deref()
                .unwrap();
            assert_eq!(text, expected());
            let mut after = desktop.clone();
            after.tree = before.tree.clone();
            after.sequence = before.sequence;
            assert_eq!(
                after, before,
                "notice refresh preserves model, requests, drafts and selection"
            );
        });
    }
}

#[test]
fn rename_vfs_failures_retain_owned_wrapper_and_exact_provider_detail() {
    for raw in ["Settings", "  Save {error} 中文 /root/Save  ", "", " \t "] {
        let mut desktop = launch(&["Save"]);
        select(&mut desktop, "Save", ClickMode::Replace);
        desktop.begin_keyboard_rename().unwrap();
        desktop
            .handle_event(
                &UiEventEnvelope {
                    app: desktop.app,
                    window: desktop.window,
                    target: widget_id(RENAME_FIELD),
                    event: UiEvent::ValueChanged("{error} 中文".into()),
                },
                request(2),
            )
            .unwrap();
        let messages = rice_i18n::with_locale(0, || desktop.begin_rename(request(3)).unwrap());
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::VfsRequest(vfs)
            if matches!(&vfs.operation, VfsOperation::Rename { from, to }
                if from == "/root/Desktop/Save" && to == "/root/Desktop/{error} 中文")))
        );
        rice_i18n::with_locale(rice_i18n::locale_code("zh-Hans"), || {
            respond(&mut desktop, 3, error(raw));
            let detail = if raw.trim().is_empty() {
                rice_i18n::t("files.recovery.generic")
            } else {
                raw
            };
            assert_eq!(
                desktop.tree.get(widget_id(NOTICE)).unwrap().text.as_deref(),
                Some(rice_i18n::format("desktop.rename.failed", &[("error", detail)]).as_str())
            );
        });
        assert_refresh(&mut desktop, || {
            let detail = if raw.trim().is_empty() {
                rice_i18n::t("files.recovery.generic")
            } else {
                raw
            };
            rice_i18n::format("desktop.rename.failed", &[("error", detail)])
        });
    }
}

#[test]
fn typed_mutation_invalid_response_and_delete_failure_use_whole_messages() {
    let mut desktop = launch(&["Save"]);
    select(&mut desktop, "Save", ClickMode::Replace);
    desktop.begin_keyboard_rename().unwrap();
    desktop.rename_text = "Home".into();
    desktop.begin_rename(request(3)).unwrap();
    respond(&mut desktop, 3, directory(&[]));
    assert_refresh(&mut desktop, || {
        rice_i18n::t("desktop.rename.invalid-response").into()
    });
    desktop.begin_permanent_delete(request(4)).unwrap();
    assert_refresh(&mut desktop, || {
        rice_i18n::t("desktop.notice.3e358519ab7f").into()
    });
    let messages = desktop.begin_permanent_delete(request(5)).unwrap();
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::VfsRequest(vfs)
        if matches!(&vfs.operation, VfsOperation::Remove { path, recursive: true }
            if path == "/root/Desktop/Save")))
    );
    respond(&mut desktop, 5, error("  Home {error}  "));
    assert_refresh(&mut desktop, || {
        rice_i18n::format(
            "desktop.delete-permanently.failed",
            &[("error", "  Home {error}  ")],
        )
    });
    desktop.begin_permanent_delete(request(6)).unwrap();
    desktop.begin_permanent_delete(request(7)).unwrap();
    respond(&mut desktop, 7, directory(&[]));
    assert_refresh(&mut desktop, || {
        rice_i18n::t("desktop.delete-permanently.invalid-response").into()
    });
}

#[test]
fn trash_collision_and_preflight_errors_preserve_literal_names_and_queue() {
    for name in ["Home", "Save", "{error} 中文"] {
        let mut desktop = launch(&[name]);
        select(&mut desktop, name, ClickMode::Replace);
        desktop.begin_move_to_trash(request(2)).unwrap();
        let messages = respond(&mut desktop, 2, directory(&[name]));
        assert!(
            !messages
                .iter()
                .any(|message| matches!(message, Message::VfsRequest(_)))
        );
        assert!(desktop.pending_trash.is_none());
        assert_refresh(&mut desktop, || {
            rice_i18n::format("desktop.trash.collision", &[("name", name)])
        });
        desktop.begin_move_to_trash(request(3)).unwrap();
        respond(&mut desktop, 3, error("  Save {error}  "));
        assert_refresh(&mut desktop, || {
            rice_i18n::format("desktop.trash.stopped", &[("error", "  Save {error}  ")])
        });
        desktop.begin_move_to_trash(request(4)).unwrap();
        let messages = respond(&mut desktop, 4, directory(&[]));
        assert!(messages.iter().any(|message| matches!(message, Message::VfsRequest(vfs)
            if matches!(&vfs.operation, VfsOperation::Rename { from, to }
                if from == &format!("/root/Desktop/{name}") && to == &format!("/root/.local/share/Trash/files/{name}")))));
        respond(&mut desktop, 4, error(""));
        assert_refresh(&mut desktop, || {
            rice_i18n::format(
                "desktop.trash.failed",
                &[("error", rice_i18n::t("files.recovery.generic"))],
            )
        });
    }
}

#[test]
fn dismissed_notice_is_not_resurrected_and_raw_replacement_clears_ownership() {
    let mut desktop = launch(&["Save"]);
    desktop
        .unsupported_application_notice("/root/Save {path} 中文")
        .unwrap();
    assert_refresh(&mut desktop, || {
        rice_i18n::format(
            "desktop.unsupported-application",
            &[("path", "/root/Save {path} 中文")],
        )
    });
    let raw = "  Enter a valid file name without '/' {name}  ";
    desktop.notice(raw).unwrap();
    assert_refresh(&mut desktop, || raw.into());
    for close in [false, true] {
        desktop
            .notice_message(DesktopNotice::key("desktop.notice.ef20be863ab8"))
            .unwrap();
        select(&mut desktop, "Save", ClickMode::Replace);
        desktop.begin_keyboard_rename().unwrap();
        if close {
            desktop
                .notice_message(DesktopNotice::key("desktop.notice.ef20be863ab8"))
                .unwrap();
            desktop.close_rename_editor().unwrap();
        }
        assert!(desktop.retained_notice.is_none());
        let draft = desktop.rename_text.clone();
        let selection = desktop.model.selection().clone();
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                desktop.refresh_language().unwrap();
                assert!(desktop.tree.get(widget_id(NOTICE)).is_none());
                assert_eq!(desktop.rename_text, draft);
                assert_eq!(desktop.model.selection(), &selection);
            });
        }
    }
}

#[test]
fn partial_trash_failure_remains_suppressed_by_existing_directory_completion_boundary() {
    let mut desktop = launch(&["one", "two"]);
    select(&mut desktop, "one", ClickMode::Replace);
    select(&mut desktop, "two", ClickMode::Toggle);
    desktop.begin_move_to_trash(request(2)).unwrap();
    respond(&mut desktop, 2, directory(&[]));
    respond(&mut desktop, 2, VfsResult::Done);
    let messages = respond(&mut desktop, 2, error("remaining item failed"));
    assert!(messages.iter().any(|message| matches!(message, Message::VfsRequest(vfs)
        if matches!(&vfs.operation, VfsOperation::ReadDirectory { path, .. } if path == "/root/Desktop"))));
    respond(&mut desktop, 2, directory(&["two"]));
    assert!(desktop.pending_trash.is_none());
    assert!(
        desktop.tree.get(widget_id(NOTICE)).is_none(),
        "Known existing behavior: apply_items ignores completion status, including partial failure"
    );
    assert!(desktop.retained_notice.is_none());
}

#[test]
fn refresh_vfs_error_and_success_after_language_change_preserve_paths() {
    let mut desktop = launch(&["Save"]);
    desktop.begin_refresh(request(8), None).unwrap();
    respond(&mut desktop, 8, error("  /root/Desktop {error}  "));
    assert_refresh(&mut desktop, || {
        rice_i18n::format(
            "desktop.refresh.failed",
            &[("error", "  /root/Desktop {error}  ")],
        )
    });
    desktop.begin_refresh(request(9), None).unwrap();
    respond(&mut desktop, 9, VfsResult::Done);
    assert_refresh(&mut desktop, || {
        rice_i18n::t("desktop.notice.b4aa5f428033").into()
    });
    select(&mut desktop, "Save", ClickMode::Replace);
    desktop.begin_keyboard_rename().unwrap();
    desktop.rename_text = "Home".into();
    rice_i18n::with_locale(0, || desktop.begin_rename(request(10)).unwrap());
    rice_i18n::with_locale(rice_i18n::locale_code("zh-Hans"), || {
        let messages = respond(&mut desktop, 10, VfsResult::Done);
        assert!(messages.iter().any(|message| matches!(message, Message::VfsRequest(vfs)
            if matches!(&vfs.operation, VfsOperation::ReadDirectory { path, .. } if path == "/root/Desktop"))));
        respond(&mut desktop, 10, directory(&["Home"]));
        assert!(
            desktop
                .items
                .iter()
                .any(|item| item.path.as_deref() == Some("/root/Desktop/Home")
                    && item.label == "Home")
        );
        assert!(desktop.tree.get(widget_id(NOTICE)).is_none());
    });
}

#[test]
fn new_desktop_messages_registered_for_all_languages() {
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            for key in [
                "desktop.notice.33c617d542f4",
                "desktop.notice.ef20be863ab8",
                "desktop.notice.9639deae6708",
                "desktop.notice.caecdc48adf3",
                "desktop.notice.0be4c8ab20b4",
                "desktop.notice.7ef96ba6f6de",
                "desktop.notice.833663669663",
                "desktop.notice.1c58d592132c",
                "desktop.notice.3082e580a36b",
                "desktop.notice.61c897b19a78",
                "desktop.notice.180146d2a43c",
                "desktop.notice.c43fe3dc7658",
                "desktop.notice.3e358519ab7f",
                "desktop.notice.7e4ff9600eb6",
                "desktop.notice.dc15de6e34a9",
                "desktop.notice.6b90eaef8658",
                "desktop.notice.b4aa5f428033",
                "desktop.trash.failed",
                "desktop.trash.collision",
                "desktop.trash.stopped",
                "desktop.rename.failed",
                "desktop.delete-permanently.failed",
                "desktop.rename.invalid-response",
                "desktop.delete-permanently.invalid-response",
                "desktop.refresh.failed",
            ] {
                assert_ne!(rice_i18n::t(key), key, "missing {key} in locale {locale}");
            }
        });
    }
}
