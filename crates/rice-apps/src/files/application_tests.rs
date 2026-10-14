use super::{
    ENTRY_BASE, Message, RiceFiles, UiEvent,
    files_menu::{OPEN_WITH, context_widget},
    files_open::{
        OPEN_WITH_APP_BASE, OPEN_WITH_COMMAND, OPEN_WITH_DIALOG, OPEN_WITH_LIST,
        OPEN_WITH_LIST_CONTENT, OPEN_WITH_REMEMBER, OPEN_WITH_RUN_COMMAND, OPEN_WITH_SEARCH,
    },
    files_view::widget_id,
    tests::{app, event, loaded_actor, request},
};
use rice_native_app_protocol::{
    APPLICATION_CATALOG_VERSION, DataProviderOperation, DataProviderRequest, DataProviderResponse,
    DataProviderResult, DirectoryEntry, FileKind, FileMetadata, InstalledApplication, UiPatch,
    VfsResponse, VfsResult,
};
use rice_ui::{WidgetKind, WidgetState};

fn applications(request_id: u64, mime_type: &str, matches: bool) -> DataProviderResponse {
    DataProviderResponse {
        request: request(request_id),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: mime_type.to_owned(),
            entries: vec![InstalledApplication {
                id: String::from("native:rice-paper"),
                name: String::from("RicePaper"),
                icon: String::from("rice-paper"),
                mime_types: vec![String::from("text/*")],
                matches_requested_type: matches,
                is_default: matches,
                is_native: true,
            }],
        },
    }
}

#[test]
fn executable_program_double_click_runs_the_file_in_a_terminal() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("say 'hello'.sh"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 24,
                        modified_millis: 0,
                        mode: 0o755,
                    },
                    mime_type: Some(String::from("text/x-shellscript")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select script");
    let launched = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("execute script");
    assert!(matches!(
        launched.as_slice(),
        [Message::Launch { entrypoint, arguments, .. }]
            if entrypoint == "rice-term"
                && arguments == &[String::from(
                    "exec '/root/say '\\''hello'\\''.sh'"
                )]
    ));
}

#[test]
fn non_executable_script_still_uses_the_mime_application_path() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("notes.sh"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 24,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("text/x-shellscript")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select script");
    let lookup = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("open script");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { .. },
            ..
        })]
    ));
}

#[test]
fn duplicate_open_gesture_coalesces_while_the_application_lookup_is_pending() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select image-like file");
    let lookup = actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(3))
        .expect("first open gesture");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { .. },
            ..
        })]
    ));

    let duplicate = actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(4))
        .expect("duplicate open gesture");
    assert!(
        duplicate.is_empty(),
        "the second half of a double-click must not emit an error UI or notification"
    );
}

#[test]
fn executable_suffix_uses_the_application_route_even_with_posix_mode_and_mime() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("PUTTY.EXE"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 1_708_656,
                        modified_millis: 0,
                        mode: 0o755,
                    },
                    mime_type: Some(String::from("application/x-executable")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select executable");
    let lookup = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("open executable");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "application/x-msdownload"
    ));
}

fn video_application(request_id: u64, mime_type: &str) -> DataProviderResponse {
    DataProviderResponse {
        request: request(request_id),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: mime_type.to_owned(),
            entries: vec![InstalledApplication {
                id: String::from("native:rice-player"),
                name: String::from("Rice Player"),
                icon: String::from("video-x-generic"),
                mime_types: vec![String::from("video/*")],
                matches_requested_type: true,
                is_default: false,
                is_native: true,
            }],
        },
    }
}

#[test]
fn theme_file_offers_load_or_edit_even_with_a_default_editor() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("custom.theme"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 1024,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("application/octet-stream")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select theme");
    let lookup = actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("double-click theme");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "application/x-ricewm-theme"
    ));
    let chooser = actor
        .handle_data_response(&applications(3, "application/x-ricewm-theme", true), request(4))
        .expect("default Paper");
    let [Message::UiPatch(chooser)] = chooser.as_slice() else {
        panic!("an unassociated MIME type must present Open With");
    };
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, .. }
            if *id == widget_id(OPEN_WITH_DIALOG)
                && *kind == WidgetKind::Dialog as u16
    )));
    assert!(actor.open_with.is_some());
    let mut editor = actor.clone();
    let editing = editor.handle_event(&event(super::files_open::OPEN_THEME_EDIT, UiEvent::Activate), request(5)).unwrap();
    assert!(editing.iter().any(|message| matches!(message, Message::DataProviderRequest(DataProviderRequest {
        operation: DataProviderOperation::ApplicationLaunch { application_id, paths, .. }, ..
    }) if application_id == "native:rice-paper" && paths == &["/root/custom.theme"])));
    let loading = actor.handle_event(&event(super::files_open::OPEN_THEME_LOAD, UiEvent::Activate), request(5)).unwrap();
    assert!(loading.iter().any(|message| matches!(message, Message::Launch { entrypoint, arguments, .. }
        if entrypoint == "rice-settings" && arguments == &["--load-theme=/root/custom.theme"])));

}

#[test]
fn compatible_but_unassociated_file_opens_its_handler() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("clip.avi"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 1024,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("application/octet-stream")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select AVI");
    let lookup = actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("double-click AVI");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "video/x-msvideo"
    ));
    let opened = actor
        .handle_data_response(&video_application(3, "video/x-msvideo"), request(4))
        .expect("matching Rice Player");
    assert!(matches!(opened.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch { application_id, paths, .. }, ..
        })] if application_id == "native:rice-player" && paths == &["/root/clip.avi"]
    ));
    assert!(actor.open_with.is_none());
}

#[test]
fn default_open_and_open_with_emit_only_host_registered_launches() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select text file");
    let lookup = actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(3))
        .expect("default open");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "text/plain"
    ));
    let launched = actor
        .handle_data_response(&applications(3, "text/plain", true), request(4))
        .expect("matching application");
    assert!(matches!(
        launched.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                paths,
                ..
            },
            ..
        })] if application_id == "native:rice-paper" && paths == &["/root/file2.txt"]
    ));
    actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(4),
                result: DataProviderResult::ApplicationLaunched {
                    application_id: String::from("native:rice-paper"),
                    default_changed: false,
                },
            },
            request(5),
        )
        .expect("launch completion");
    let lookup = actor
        .handle_event(
            &event(context_widget(OPEN_WITH), UiEvent::Activate),
            request(5),
        )
        .expect("open with");
    assert!(matches!(
        lookup.as_slice(),
        [Message::UiPatch(_), Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { .. },
            ..
        })]
    ));
    let chooser = actor
        .handle_data_response(&applications(5, "text/plain", true), request(6))
        .expect("chooser catalog");
    let [Message::UiPatch(chooser)] = chooser.as_slice() else {
        panic!("chooser UI");
    };
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, .. }
            if *id == widget_id(OPEN_WITH_DIALOG) && *kind == WidgetKind::Dialog as u16
    )));
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, text: Some(text), .. }
            if *id == widget_id(OPEN_WITH_LIST)
                && *kind == WidgetKind::ScrollView as u16
                && text.starts_with("viewport:280|0/")
    )));
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, parent: Some(parent), kind, .. }
            if *id == widget_id(OPEN_WITH_LIST_CONTENT)
                && *parent == widget_id(OPEN_WITH_LIST)
                && *kind == WidgetKind::List as u16
    )));
    assert!(
        chooser.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { id, parent: Some(parent), kind, .. }
                if *id == widget_id(OPEN_WITH_APP_BASE)
                    && *parent == widget_id(OPEN_WITH_LIST_CONTENT)
                    && *kind == WidgetKind::Button as u16
        )),
        "application rows belong to one scrollable list instead of overlapping"
    );
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, state, text: Some(text), .. }
            if *id == widget_id(OPEN_WITH_REMEMBER)
                && *kind == WidgetKind::CheckBox as u16
                && text == "Always use for this file type"
                && *state & WidgetState::CHECKED.bits() == 0
    )));
    assert!(!chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. }
            if matches!(
                text.as_str(),
                "Open with this installed application"
                    | "Supports this file type"
                    | "Default application for this file type"
            )
    )));
    actor
        .handle_event(&event(OPEN_WITH_REMEMBER, UiEvent::Activate), request(7))
        .expect("remember association");
    let chosen = actor
        .handle_event(&event(OPEN_WITH_APP_BASE, UiEvent::Activate), request(8))
        .expect("choose");
    assert!(matches!(
        chosen.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                remember_default,
                ..
            },
            ..
        })] if application_id == "native:rice-paper" && *remember_default
    ));
}

#[test]
fn unknown_file_type_uses_installed_all_application_fallback() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("archive.unknown"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 3,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: None,
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("open");
    let chooser = actor
        .handle_data_response(
            &applications(3, "application/octet-stream", false),
            request(4),
        )
        .expect("unknown chooser");
    assert!(matches!(chooser.as_slice(), [Message::UiPatch(_)]));
    let chosen = actor
        .handle_event(&event(OPEN_WITH_APP_BASE, UiEvent::Activate), request(5))
        .expect("choose RicePaper");
    assert!(matches!(
        chosen.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                paths,
                ..
            },
            ..
        })] if application_id == "native:rice-paper"
            && paths == &["/root/archive.unknown"]
    ));
}

#[test]
fn explicit_open_with_lists_and_searches_nonmatching_installed_apps() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(
            &event(context_widget(OPEN_WITH), UiEvent::Activate),
            request(3),
        )
        .expect("catalog request");
    let response = DataProviderResponse {
        request: request(3),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: String::from("text/plain"),
            entries: vec![
                InstalledApplication {
                    id: String::from("native:rice-paper"),
                    name: String::from("RicePaper"),
                    icon: String::new(),
                    mime_types: vec![String::from("text/*")],
                    matches_requested_type: true,
                    is_default: true,
                    is_native: true,
                },
                InstalledApplication {
                    id: String::from("system-tool.desktop"),
                    name: String::from("System Tool"),
                    icon: String::new(),
                    mime_types: Vec::new(),
                    matches_requested_type: false,
                    is_default: false,
                    is_native: false,
                },
            ],
        },
    };
    let chooser = actor
        .handle_data_response(&response, request(4))
        .expect("open with chooser");
    let [Message::UiPatch(chooser)] = chooser.as_slice() else {
        panic!("chooser patch");
    };
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, .. }
            if *id == widget_id(OPEN_WITH_SEARCH)
                && *kind == WidgetKind::TextField as u16
    )));
    assert!(chooser.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. } if text == "System Tool"
    )));
    let filtered = actor
        .handle_event(
            &event(
                OPEN_WITH_SEARCH,
                UiEvent::ValueChanged(String::from("system")),
            ),
            request(5),
        )
        .expect("search applications");
    let [Message::UiPatch(filtered)] = filtered.as_slice() else {
        panic!("filtered chooser");
    };
    assert!(filtered.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. } if text == "System Tool"
    )));
    assert!(!filtered.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. } if text == "RicePaper"
    )));
    let launch = actor
        .handle_event(&event(OPEN_WITH_APP_BASE, UiEvent::Activate), request(6))
        .expect("launch filtered application");
    assert!(matches!(
        launch.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch { application_id, .. },
            ..
        })] if application_id == "system-tool.desktop"
    ));
}

#[test]
fn provider_mime_is_authoritative_over_misleading_extension() {
    let (mut actor, _) = RiceFiles::launch(app(), request(20)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(20),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("actually-an-image.txt"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 3,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("image/png")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(21))
        .expect("select");
    let lookup = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(22))
        .expect("open");
    assert!(matches!(
        lookup.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "image/png"
    ));
}

#[test]
fn open_with_runs_an_explicit_bash_command_with_a_shell_quoted_filename() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(
            &event(context_widget(OPEN_WITH), UiEvent::Activate),
            request(3),
        )
        .expect("catalog request");
    actor
        .handle_data_response(&applications(3, "text/plain", true), request(4))
        .expect("chooser");
    for (request_id, command) in [
        (5, "l"),
        (6, "le"),
        (7, "les"),
        (8, "less --RAW-CONTROL-CHARS"),
    ] {
        let updated = actor
            .handle_event(
                &event(
                    OPEN_WITH_COMMAND,
                    UiEvent::ValueChanged(String::from(command)),
                ),
                request(request_id),
            )
            .expect("command input");
        let [Message::UiPatch(updated)] = updated.as_slice() else {
            panic!("command input UI");
        };
        assert!(updated.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { id, state, text: Some(text), .. }
                if *id == widget_id(OPEN_WITH_COMMAND)
                    && text == command
                    && *state & WidgetState::FOCUSED.bits() != 0
        )));
        assert!(updated.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { id, state, .. }
                if *id == widget_id(OPEN_WITH_SEARCH)
                    && *state & WidgetState::FOCUSED.bits() == 0
        )));
    }
    let launched = actor
        .handle_event(&event(OPEN_WITH_RUN_COMMAND, UiEvent::Activate), request(9))
        .expect("run command");
    assert!(matches!(
        launched.as_slice(),
        [Message::UiPatch(_), Message::Launch { entrypoint, arguments, .. }]
            if entrypoint == "rice-term"
                && arguments == &[String::from(
                    "exec less --RAW-CONTROL-CHARS '/root/file2.txt'"
                )]
    ));
    assert!(actor.open_with.is_none());
}

#[test]
fn remembered_default_opens_directly_even_when_the_app_did_not_advertise_the_mime() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2))
        .expect("select file");
    actor
        .handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(3))
        .expect("application catalog");
    let response = DataProviderResponse {
        request: request(3),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: String::from("text/plain"),
            entries: vec![
                InstalledApplication {
                    id: String::from("custom.desktop"),
                    name: String::from("Custom Tool"),
                    icon: String::new(),
                    mime_types: Vec::new(),
                    matches_requested_type: false,
                    is_default: true,
                    is_native: false,
                },
                InstalledApplication {
                    id: String::from("native:rice-paper"),
                    name: String::from("RicePaper"),
                    icon: String::new(),
                    mime_types: vec![String::from("text/*")],
                    matches_requested_type: true,
                    is_default: false,
                    is_native: true,
                },
            ],
        },
    };
    let launch = actor
        .handle_data_response(&response, request(4))
        .expect("remembered launch");
    assert!(matches!(
        launch.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch { application_id, .. },
            ..
        })] if application_id == "custom.desktop"
    ));
    assert!(actor.open_with.is_none());
}

#[test]
fn open_with_shell_quoting_keeps_apostrophes_in_filenames_inert() {
    assert_eq!(
        super::files_open::shell_quote("/root/it's notes.txt"),
        "'/root/it'\\''s notes.txt'",
    );
}


#[test]
fn closing_open_with_during_lookup_does_not_reopen_it() {
    let (mut actor, _) = loaded_actor();
    actor.handle_event(&event(ENTRY_BASE + 1, UiEvent::Activate), request(2)).unwrap();
    let opened = actor.show_open_with(request(3)).unwrap();
    assert!(opened.iter().any(|message| matches!(message, Message::UiPatch(_))));
    assert!(actor.open_with.is_some());
    actor.close_open_with().unwrap();
    let mime = opened.iter().find_map(|message| match message {
        Message::DataProviderRequest(DataProviderRequest { operation: DataProviderOperation::ApplicationCatalog { mime_type }, .. }) => Some(mime_type.clone()), _ => None,
    }).unwrap();
    assert!(actor.handle_data_response(&applications(3, &mime, true), request(4)).unwrap().is_empty());
    assert!(actor.open_with.is_none());
}
