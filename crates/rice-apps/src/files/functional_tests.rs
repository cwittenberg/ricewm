use super::{
    BREADCRUMB, COMPLETION_BASE, ENTRY_BASE, Message, PREVIEW_TOGGLE, RiceFiles, SEARCH,
    SEARCH_FIELD, UiEvent, VIEW_DETAILS,
    files_preview_view::PREVIEW_PLAY,
    files_view::{DETAIL_DISCLOSURE_OFFSET, DETAIL_SIZE, DIRECTORY, widget_id},
    tests::{app, directory_response, event, loaded_actor, request},
};
use rice_files::{SortDirection, SortKey};
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, FileMetadata, HostEvent, HostEventKind, MediaKind, MediaMetadata,
    MediaResponse, MediaResult, PointerEventKind, UiPatch, VfsError, VfsErrorCode, VfsOperation,
    VfsRequest, VfsResponse, VfsResult,
};
use rice_ui::{WidgetKind, WidgetState};

fn enter_key() -> UiEvent {
    UiEvent::Key {
        pressed: true,
        key_code: 13,
        modifiers: 0,
        text: None,
    }
}

fn entry(name: &str, kind: FileKind, size: u64) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        metadata: FileMetadata {
            kind,
            size,
            modified_millis: 7,
            mode: 0o644,
        },
        mime_type: None,
    }
}

#[test]
fn office_files_request_content_thumbnails_and_keep_file_type_icons_on_failure() {
    for (name, supplied, mime, icon) in [
        ("BUDGET.XLS", "application/octet-stream", "application/vnd.ms-excel", rice_ui::IconId::SpreadsheetFile),
        ("slides.PPTX", "application/zip", "application/vnd.openxmlformats-officedocument.presentationml.presentation", rice_ui::IconId::PresentationFile),
    ] {
        let (mut actor, _) = RiceFiles::launch(app(), request(1)).unwrap();
        let mut file = entry(name, FileKind::File, 4096);
        file.mime_type = Some(supplied.to_owned());
        let loaded = actor.handle_vfs_response_with_request(&VfsResponse {
            request: request(1), result: VfsResult::Directory { entries: vec![file], next_cursor: None },
        }, request(10)).unwrap();
        let media = loaded.iter().find_map(|m| if let Message::MediaRequest(r) = m {Some(r)} else {None}).expect("office thumbnail requested");
        assert_eq!(media.mime_type, mime);
        let patch = loaded.iter().find_map(|m| if let Message::UiPatch(p) = m {Some(p)} else {None}).unwrap();
        assert!(patch.patches.iter().any(|p|matches!(p, UiPatch::Create {icon:Some(i),..} if *i == icon as u16)));
        let ready = actor.handle_media_response_with_request(&MediaResponse {
            request: request(10), result: MediaResult::Ready(MediaMetadata {
                kind: MediaKind::Image, width:384, height:216, duration_millis:None, mime_type:mime.into(),
            }),
        }, request(11)).unwrap();
        assert!(ready.iter().any(|m|matches!(m, Message::UiPatch(p) if p.patches.iter().any(|p|
            matches!(p,UiPatch::Create {kind,text:Some(t),..} if *kind == WidgetKind::Image as u16 && t == "media-request:10")))));
    }
}

#[test]
fn selected_large_movie_decodes_a_grid_thumbnail_without_opening_preview_pane() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let loaded = actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("movie.mp4"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 512 * 1024 * 1024,
                        modified_millis: 7,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("video/mp4")),
                }],
                next_cursor: None,
            },
        })
        .expect("movie directory");
    let [Message::UiPatch(_), Message::MediaRequest(batch)] = loaded.as_slice() else {
        panic!("directory starts batch thumbnail decode");
    };
    assert_eq!(batch.path, "/root/movie.mp4");
    let selected = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select movie");
    assert!(
        matches!(selected.as_slice(), [Message::UiPatch(_)]),
        "hidden preview must reuse the directory thumbnail job, not open another decoder"
    );
    let rendered = actor
        .handle_media_response(&MediaResponse {
            request: batch.request,
            result: MediaResult::Ready(MediaMetadata {
                kind: MediaKind::Video,
                width: 320,
                height: 180,
                duration_millis: Some(20_000),
                mime_type: String::from("video/mp4"),
            }),
        })
        .expect("movie frame");
    let [Message::UiPatch(rendered)] = rendered.as_slice() else {
        panic!("hidden preview must not start a playback timer");
    };
    assert!(rendered.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { kind, text: Some(text), .. }
            if *kind == WidgetKind::Image as u16
                && text == &format!("media-request:{}", batch.request.get())
    )));
}

#[test]
fn directory_batch_decodes_every_image_and_movie_without_selection() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let loaded = actor
        .handle_vfs_response_with_request(
            &VfsResponse {
                request: request(1),
                result: VfsResult::Directory {
                    entries: vec![
                        DirectoryEntry {
                            name: String::from("photo.png"),
                            metadata: FileMetadata {
                                kind: FileKind::File,
                                size: 10,
                                modified_millis: 1,
                                mode: 0o644,
                            },
                            mime_type: None,
                        },
                        DirectoryEntry {
                            name: String::from("movie.mp4"),
                            metadata: FileMetadata {
                                kind: FileKind::File,
                                size: 20,
                                modified_millis: 1,
                                mode: 0o644,
                            },
                            mime_type: None,
                        },
                    ],
                    next_cursor: None,
                },
            },
            request(10),
        )
        .expect("directory");
    let Message::MediaRequest(first) = &loaded[1] else {
        panic!("first batch request");
    };
    assert_eq!(first.path, "/root/movie.mp4");
    assert_eq!(first.mime_type, "video/mp4");
    assert_eq!(first.frame_millis, 1_000);
    assert_eq!((first.target_width, first.target_height), (384, 288));

    let first_ready = actor
        .handle_media_response_with_request(
            &MediaResponse {
                request: request(10),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Video,
                    width: 320,
                    height: 180,
                    duration_millis: Some(5_000),
                    mime_type: String::from("video/mp4"),
                }),
            },
            request(11),
        )
        .expect("movie thumbnail");
    assert!(matches!(
        first_ready.as_slice(),
        [Message::UiPatch(_), Message::MediaRequest(next)]
            if next.path == "/root/photo.png"
                && next.mime_type == "image/png"
                && next.frame_millis == 0
    ));

    let complete = actor
        .handle_media_response_with_request(
            &MediaResponse {
                request: request(11),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Image,
                    width: 320,
                    height: 240,
                    duration_millis: None,
                    mime_type: String::from("image/png"),
                }),
            },
            request(12),
        )
        .expect("image thumbnail");
    let [Message::UiPatch(patch)] = complete.as_slice() else {
        panic!("completed batch UI");
    };
    let media = patch
        .patches
        .iter()
        .filter_map(|candidate| match candidate {
            UiPatch::Create {
                kind,
                text: Some(text),
                ..
            } if *kind == WidgetKind::Image as u16 && text.starts_with("media-request:") => {
                Some(text.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(media.contains(&"media-request:10"));
    assert!(media.contains(&"media-request:11"));
}

#[test]
fn executable_uses_exact_icon_media_path_with_generic_fallback_until_ready() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let loaded = actor
        .handle_vfs_response_with_request(
            &VfsResponse {
                request: request(1),
                result: VfsResult::Directory {
                    entries: vec![entry("sample.exe", FileKind::File, 4096)],
                    next_cursor: None,
                },
            },
            request(10),
        )
        .expect("executable directory");
    let [Message::UiPatch(initial), Message::MediaRequest(icon)] = loaded.as_slice() else {
        panic!("executable starts one bounded icon decode");
    };
    assert_eq!(icon.path, "/root/sample.exe");
    assert_eq!(icon.mime_type, "application/x-msdownload");
    assert!(
        initial.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { kind, icon: Some(_), text: None, .. }
                if *kind == WidgetKind::Image as u16
        )),
        "the semantic executable icon remains while exact PE pixels decode"
    );

    let ready = actor
        .handle_media_response_with_request(
            &MediaResponse {
                request: request(10),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Image,
                    width: 32,
                    height: 32,
                    duration_millis: None,
                    mime_type: String::from("application/x-msdownload"),
                }),
            },
            request(11),
        )
        .expect("exact executable icon");
    assert!(ready.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|candidate| matches!(
            candidate,
            UiPatch::Create { kind, text: Some(text), .. }
                if *kind == WidgetKind::Image as u16 && text == "media-request:10"
        ))
    )));
}

#[test]
fn filtering_never_evicts_thumbnails_needed_when_search_is_cleared() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![
                    entry("photo.png", FileKind::File, 10),
                    entry("movie.mp4", FileKind::File, 10),
                ],
                next_cursor: None,
            },
        })
        .expect("media directory");
    let photo = rice_files::GuestPath::absolute("/root/photo.png").expect("photo path");
    let movie = rice_files::GuestPath::absolute("/root/movie.mp4").expect("movie path");
    actor.thumbnail_cache.insert(photo.clone(), request(70));
    actor.thumbnail_cache.insert(movie.clone(), request(71));
    let filtered = vec![rice_files::EntryMetadata::new(
        movie,
        rice_files::EntryKind::File,
        10,
        1,
    )];

    assert!(
        actor
            .reset_thumbnail_batch(&filtered, request(72), false)
            .is_none()
    );
    assert_eq!(actor.thumbnail_cache.get(&photo), Some(&request(70)));

    let restored = actor
        .handle_event(
            &event(SEARCH_FIELD, UiEvent::ValueChanged(String::new())),
            request(73),
        )
        .expect("clear search");
    let [Message::UiPatch(restored)] = restored.as_slice() else {
        panic!("clear search redraws cached thumbnails");
    };
    assert!(restored.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { kind, text: Some(text), .. }
            if *kind == WidgetKind::Image as u16 && text == "media-request:70"
    )));
}

#[test]
fn thumbnail_zoom_slider_reaches_a_genuinely_large_grid() {
    let (mut actor, _) = loaded_actor();
    let messages = actor
        .handle_event(
            &event(
                super::files_view::ZOOM_SLIDER,
                UiEvent::ValueChanged(String::from("4500/4500")),
            ),
            request(20),
        )
        .expect("large zoom");
    let [Message::UiPatch(patch)] = messages.as_slice() else {
        panic!("zoom UI");
    };
    assert!(patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(DIRECTORY) && text == "file-grid:5000"
    )));
    assert!(patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(super::files_view::ZOOM_SLIDER)
                && text == "4500/4500"
    )));
}

#[test]
fn paged_directory_enumeration_commits_only_after_the_last_page() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let next = actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![entry("first.txt", FileKind::File, 1)],
                next_cursor: Some(8),
            },
        })
        .expect("first page");
    assert!(actor.runtime_for_test().entries.is_empty());
    assert!(matches!(
        next.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory {
                    path,
                    cursor: 8,
                    limit: 128,
                },
                ..
            })
        ] if path == "/root"
    ));
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![entry("second.txt", FileKind::File, 2)],
                next_cursor: None,
            },
        })
        .expect("last page");
    assert_eq!(
        actor
            .runtime_for_test()
            .entries
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["first.txt", "second.txt"]
    );
}

#[test]
fn expanded_detail_directory_routes_paged_vfs_results_into_inline_rows() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(VIEW_DETAILS, UiEvent::Activate), request(2))
        .expect("details");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("select folder");
    let request_children = actor
        .handle_event(
            &event(ENTRY_BASE + DETAIL_DISCLOSURE_OFFSET, UiEvent::Activate),
            request(4),
        )
        .expect("the disclosure expands the selected folder");
    assert!(matches!(
        request_children.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory {
                    path,
                    cursor: 0,
                    ..
                },
                ..
            })
        ] if path == "/root/Folder"
    ));
    let next = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: vec![entry("beta.txt", FileKind::File, 2)],
                next_cursor: Some(4),
            },
        })
        .expect("child page");
    assert!(matches!(
        next.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory {
                    path,
                    cursor: 4,
                    ..
                },
                ..
            })
        ] if path == "/root/Folder"
    ));
    let completed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: vec![entry("alpha.txt", FileKind::File, 1)],
                next_cursor: None,
            },
        })
        .expect("child completion");
    let [Message::UiPatch(completed)] = completed.as_slice() else {
        panic!("inline rows");
    };
    for label in ["alpha.txt", "beta.txt"] {
        assert!(completed.patches.iter().any(|patch| {
            matches!(patch, UiPatch::Create { text: Some(text), .. } if text.ends_with(label))
        }));
    }
    assert!(completed.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, text: Some(text), .. }
                if *id == widget_id(ENTRY_BASE) && text == "Folder"
        )
    }));
    assert!(completed.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, text: Some(text), .. }
                if *id == widget_id(ENTRY_BASE + 1) && text == "  alpha.txt"
        )
    }));
    assert!(completed.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { text: Some(text), .. }
                if text == &format!(
                    "split:600:control-for={}:file-detail-row:alternate",
                    ENTRY_BASE + 1
                )
        )
    }));
    assert_eq!(actor.visible_entries().len(), 5);
    assert_eq!(
        actor
            .visible_entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["Folder", "alpha.txt", "beta.txt", "file2.txt", "file10.txt"]
    );

    actor
        .handle_event(&event(DETAIL_SIZE, UiEvent::Activate), request(5))
        .expect("size ascending");
    actor
        .handle_event(&event(DETAIL_SIZE, UiEvent::Activate), request(6))
        .expect("size descending");
    assert_eq!(
        actor
            .visible_entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["Folder", "beta.txt", "alpha.txt", "file10.txt", "file2.txt"],
        "sorting a detail tree must reorder root and expanded sibling groups"
    );
}

#[test]
fn detail_disclosure_expands_empty_directory_but_double_click_navigates() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(VIEW_DETAILS, UiEvent::Activate), request(2))
        .expect("details");
    let expansion = actor
        .handle_event(
            &event(ENTRY_BASE + DETAIL_DISCLOSURE_OFFSET, UiEvent::Activate),
            request(3),
        )
        .expect("expand disclosure");
    assert!(matches!(
        expansion.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == "/root/Folder"
    ));
    let empty = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("empty child");
    assert!(empty.iter().any(|message| matches!(
        message,
        Message::UiPatch(update) if update.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { text: Some(text), .. } if text.trim() == "This directory is empty"
        ))
    )));
    assert_eq!(actor.location().as_str(), "/root");

    let opened = actor
        .handle_event(
            &event(
                ENTRY_BASE,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(4),
        )
        .expect("double-click row");
    assert!(opened.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/root/Folder"
    )));
}

#[test]
fn search_expands_from_the_toolbar_and_recurses_over_real_vfs_pages() {
    let (mut actor, messages) = RiceFiles::launch(app(), request(1)).expect("launch");
    let Message::UiPatch(initial) = &messages[0] else {
        panic!("initial UI");
    };
    assert!(
        initial
            .patches
            .iter()
            .any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(SEARCH)))
    );
    let expanded = actor
        .handle_event(&event(SEARCH, UiEvent::Activate), request(2))
        .expect("expand search");
    let [Message::UiPatch(expanded)] = expanded.as_slice() else {
        panic!("expanded search UI");
    };
    assert!(expanded.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(SEARCH_FIELD) && *kind == WidgetKind::TextField as u16
        )
    }));
    actor
        .handle_vfs_response(&directory_response(1))
        .expect("directory");
    actor
        .handle_event(
            &event(SEARCH_FIELD, UiEvent::ValueChanged(String::from("FILE10"))),
            request(2),
        )
        .expect("query");
    let started = actor
        .handle_event(&event(SEARCH_FIELD, enter_key()), request(3))
        .expect("submit");
    assert!(matches!(
        started.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if path == "/root"
    ));
    let nested = actor
        .handle_vfs_response(&directory_response(3))
        .expect("root search page");
    assert!(matches!(
        nested.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if path == "/root/Folder"
    ));
    let completed = actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("unreadable"),
            }),
        })
        .expect("unreadable child is skipped");
    assert_eq!(actor.runtime_for_test().entries.len(), 1);
    assert_eq!(actor.runtime_for_test().entries[0].name, "file10.txt");
    assert!(
        actor
            .runtime_for_test()
            .content_notice
            .contains("unreadable folders skipped: 1")
    );
    assert!(completed.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::ShowNotification {
                kind,
                message,
                ..
            },
            ..
        }) if kind == "info" && message.contains("unreadable folders skipped: 1")
    )));
}

#[test]
fn search_stays_collapsed_on_hover_and_reveals_for_type_to_search() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(
            &event(
                SEARCH,
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: 10,
                    y: 10,
                    button: 0,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("hover Search");
    assert!(!actor.search_expanded, "hover must not reveal Search");

    let typed = actor
        .handle_event(
            &event(
                DIRECTORY,
                UiEvent::Key {
                    pressed: true,
                    key_code: 70,
                    modifiers: 0,
                    text: Some(String::from("f")),
                },
            ),
            request(3),
        )
        .expect("type-to-search");
    assert!(actor.search_expanded);
    assert!(typed.iter().any(|message| matches!(
        message,
        Message::UiPatch(update) if update.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { id, kind, text: Some(text), .. }
                if *id == widget_id(SEARCH_FIELD)
                    && *kind == WidgetKind::TextField as u16
                    && text == "f"
        ))
    )));
    assert!(
        actor
            .visible_entries()
            .iter()
            .all(|entry| entry.name.to_ascii_lowercase().contains('f'))
    );
}

#[test]
fn recursive_search_applies_type_size_date_filters_and_cancels_cooperatively() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(
            &event(
                SEARCH_FIELD,
                UiEvent::ValueChanged(String::from("photo type:image min:5 after:5")),
            ),
            request(2),
        )
        .expect("filters");
    actor
        .handle_event(&event(SEARCH_FIELD, enter_key()), request(3))
        .expect("start");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: vec![
                    entry("photo.png", FileKind::File, 8),
                    entry("photo.txt", FileKind::File, 8),
                    entry("photo-small.png", FileKind::File, 2),
                ],
                next_cursor: None,
            },
        })
        .expect("filtered result");
    assert_eq!(
        actor
            .runtime_for_test()
            .entries
            .iter()
            .map(|candidate| candidate.name.as_str())
            .collect::<Vec<_>>(),
        ["photo.png"]
    );

    actor
        .handle_event(
            &event(
                SEARCH_FIELD,
                UiEvent::ValueChanged(String::from("anything")),
            ),
            request(4),
        )
        .expect("new query");
    actor
        .handle_event(&event(SEARCH_FIELD, enter_key()), request(5))
        .expect("start cancellable search");
    actor
        .handle_event(&event(SEARCH_FIELD, enter_key()), request(6))
        .expect("request cancellation");
    let cancelled = actor
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Directory {
                entries: vec![entry("anything.txt", FileKind::File, 1)],
                next_cursor: None,
            },
        })
        .expect("finish current page");
    assert!(actor.runtime_for_test().pending_search.is_none());
    assert!(
        actor
            .runtime_for_test()
            .content_notice
            .contains("cancelled")
    );
    assert!(cancelled.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::ShowNotification {
                kind,
                message,
                ..
            },
            ..
        }) if kind == "info" && message.contains("Search cancelled")
    )));
}

#[test]
fn location_completion_is_bounded_selectable_and_navigates_on_enter() {
    let (mut actor, _) = loaded_actor();
    let changed = actor
        .handle_event(
            &event(BREADCRUMB, UiEvent::ValueChanged(String::from("/root/F"))),
            request(2),
        )
        .expect("completion");
    let [Message::UiPatch(changed)] = changed.as_slice() else {
        panic!("completion UI");
    };
    assert!(changed.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                text: Some(text),
                state,
                ..
            } if *id == widget_id(COMPLETION_BASE)
                && text == "/root/Folder"
                && state & WidgetState::SELECTED.bits() != 0
        )
    }));
    let opened = actor
        .handle_event(&event(BREADCRUMB, enter_key()), request(3))
        .expect("activate completion");
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

#[test]
fn detail_headers_sort_and_toggle_direction() {
    for &locale in rice_i18n::LOCALE_CODES {
        rice_i18n::with_locale(locale, || {
            let (mut actor, _) = loaded_actor();
            actor
                .handle_event(&event(VIEW_DETAILS, UiEvent::Activate), request(2))
                .expect("details");
            let ascending = actor
                .handle_event(&event(DETAIL_SIZE, UiEvent::Activate), request(3))
                .expect("sort ascending");
            assert_eq!(actor.runtime_for_test().sort.key, SortKey::Size);
            assert_eq!(
                actor.runtime_for_test().sort.direction,
                SortDirection::Ascending
            );
            assert_header(&ascending, &format!("{} ↑", rice_i18n::t("common.size")));
            let descending = actor
                .handle_event(&event(DETAIL_SIZE, UiEvent::Activate), request(4))
                .expect("sort descending");
            assert_eq!(
                actor.runtime_for_test().sort.direction,
                SortDirection::Descending
            );
            assert_header(&descending, &format!("{} ↓", rice_i18n::t("common.size")));
        });
    }
}

fn assert_header(messages: &[Message], expected: &str) {
    let [Message::UiPatch(patch)] = messages else {
        panic!("sort UI");
    };
    assert!(patch.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create {
                id,
                text: Some(text),
                ..
            } if *id == widget_id(DETAIL_SIZE) && text == expected
        )
    }));
    assert!(patch.patches.iter().any(|candidate| {
        matches!(
            candidate,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(DIRECTORY) && *kind == WidgetKind::Table as u16
        )
    }));
}

#[test]
fn image_preview_uses_typed_host_media_results() {
    assert_preview("/root/photo.png", "image/png", MediaKind::Image);
}

#[test]
fn unavailable_preview_stays_inline_and_never_emits_a_toast() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![entry("archive.bin", FileKind::File, 8)],
                next_cursor: None,
            },
        })
        .expect("directory");
    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(2))
        .expect("select unsupported preview");
    let messages = actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(3))
        .expect("show preview");
    let [Message::UiPatch(patch)] = messages.as_slice() else {
        panic!("unsupported preview feedback must stay inside RiceFiles");
    };
    assert!(!patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create { kind, .. } if *kind == WidgetKind::Notification as u16
    )));
    assert!(patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create { text: Some(text), .. }
            if text == "No embedded preview is available for this file"
    )));
}

fn assert_preview(path: &str, mime_type: &str, kind: MediaKind) {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let name = path.rsplit('/').next().expect("name");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: name.to_owned(),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 1024,
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
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(2))
        .expect("show preview");
    let requested = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .expect("select media");
    assert!(matches!(
        requested.as_slice(),
        [
            Message::UiPatch(_),
            Message::MediaRequest(requested)
        ] if requested.path == path
            && requested.mime_type == mime_type
            && requested.request == request(3)
    ));
    let ready_messages = actor
        .handle_media_response(&MediaResponse {
            request: request(3),
            result: MediaResult::Ready(MediaMetadata {
                kind,
                width: 640,
                height: 360,
                duration_millis: (kind == MediaKind::Video).then_some(1_500),
                mime_type: mime_type.to_owned(),
            }),
        })
        .expect("media result");
    let Message::UiPatch(ready) = &ready_messages[0] else {
        panic!("ready UI");
    };
    assert_eq!(ready_messages.len(), 1);
    assert!(ready.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                kind,
                text: Some(text),
                ..
            } if *kind == WidgetKind::Image as u16 && text == "media-request:3"
        )
    }));
}

#[test]
fn movie_preview_resumes_each_file_after_switching_and_hiding() {
    use rice_native_app_protocol::{HostOperation, HostRequest, MediaCommand};
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).unwrap();
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![
                    entry("a.mp4", FileKind::File, 100),
                    entry("b.mp4", FileKind::File, 100),
                ],
                next_cursor: None,
            },
        })
        .unwrap();
    actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(2))
        .unwrap();
    // Every reopen gets a new decoder session. Positions must belong to paths.
    for (index, session, resume, next_position) in [
        (0, 900, 0, 12_500),
        (1, 901, 0, 7_000),
        (0, 902, 12_500, 18_000),
        (1, 903, 7_000, 9_000),
        (0, 904, 18_000, 20_000),
    ] {
        actor
            .handle_event(&event(ENTRY_BASE + index, UiEvent::Activate), request(3))
            .unwrap();
        let messages = actor
            .handle_host_event(
                &HostEvent {
                    app: app(),
                    event: HostEventKind::MediaOpened {
                        path: format!("/root/{}.mp4", if index == 0 { "a" } else { "b" }),
                        session,
                        duration_millis: 60_000,
                        error: String::new(),
                    },
                },
                request(4),
            )
            .unwrap();
        let seeks: Vec<_> = messages
            .iter()
            .filter_map(|message| match message {
                Message::HostRequest(HostRequest {
                    operation:
                        HostOperation::MediaControl {
                            session: target,
                            command: MediaCommand::SeekMillis,
                            value,
                        },
                    ..
                }) => Some((*target, *value)),
                _ => None,
            })
            .collect();
        assert_eq!(
            seeks,
            if resume == 0 {
                vec![]
            } else {
                vec![(session, resume)]
            }
        );
        actor.handle_host_event(&HostEvent {
            app: app(), event: HostEventKind::RiceBus {
                topic: "org.rice.media.state".into(),
                payload: serde_json::json!({"session": session.to_string(), "state": {
                    "currentTime": next_position as f64 / 1000.0, "duration": 60, "paused": false,
                }}).to_string(),
            },
        }, request(5)).unwrap();
    }
    actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(6))
        .unwrap();
    actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(7))
        .unwrap();
    let messages = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::MediaOpened {
                    path: "/root/a.mp4".into(),
                    session: 905,
                    duration_millis: 60_000,
                    error: String::new(),
                },
            },
            request(8),
        )
        .unwrap();
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::HostRequest(HostRequest {
            operation: HostOperation::MediaControl {
                session: 905,
                command: MediaCommand::SeekMillis,
                value: 20_000
            },
            ..
        })
    )));
}

#[test]
fn movie_preview_uses_persistent_session_and_decoder_position() {
    use rice_native_app_protocol::{HostOperation, MediaCommand};
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![entry("clip.webm", FileKind::File, 3_200_000_000)],
                next_cursor: None,
            },
        })
        .unwrap();
    actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(2))
        .unwrap();
    let opened = actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(3))
        .unwrap();
    assert!(opened.iter().any(|message| matches!(message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::MediaOpen { replace_session: 0, path }, ..
        }) if path == "/root/clip.webm")));
    assert!(
        !opened
            .iter()
            .any(|message| matches!(message, Message::MediaRequest(_)))
    );
    let ready = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::MediaOpened {
                    path: "/root/clip.webm".into(),
                    session: 900,
                    duration_millis: 60_000,
                    error: String::new(),
                },
            },
            request(4),
        )
        .unwrap();
    assert_eq!(
        ready.len(),
        1,
        "playback must not use a frame-request timer"
    );
    let Message::UiPatch(patch) = &ready[0] else {
        panic!("preview UI")
    };
    let play = patch
        .patches
        .iter()
        .find_map(|patch| match patch {
            UiPatch::Create {
                id,
                parent,
                icon,
                text,
                ..
            } if *id == widget_id(PREVIEW_PLAY) => Some((parent, icon, text)),
            _ => None,
        })
        .unwrap();
    assert_eq!(*play.1, Some(rice_ui::IconId::MediaPause.wire_value()));
    assert!(play.2.is_none());
    assert!(patch.patches.iter().any(|patch| matches!(patch,
        UiPatch::Create { id, parent, .. } if *id == widget_id(super::files_preview_view::PREVIEW_PROGRESS) && parent == play.0)));
    let progress = actor.handle_host_event(&HostEvent { app: app(), event: HostEventKind::RiceBus {
        topic: "org.rice.media.state".into(), payload: r#"{"session":"900","state":{"currentTime":12.5,"duration":60,"paused":false}}"#.into(),
    }}, request(5)).unwrap();
    let Message::UiPatch(patch) = &progress[0] else {
        panic!("progress")
    };
    assert!(patch.patches.iter().any(|patch| matches!(patch, UiPatch::Update { text: rice_native_app_protocol::PatchValue::Set(text), .. } if text == "seek:12500/60000")));
    let paused = actor
        .handle_event(&event(PREVIEW_PLAY, UiEvent::Activate), request(6))
        .unwrap();
    assert!(paused.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::MediaControl {
                session: 900,
                command: MediaCommand::TogglePause,
                ..
            },
            ..
        })
    )));
    let seek = actor
        .seek_embedded_video("30000/60000", request(7))
        .unwrap();
    assert!(matches!(
        seek.as_slice(),
        [
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: HostOperation::MediaControl {
                    session: 900,
                    command: MediaCommand::SeekMillis,
                    value: 30000
                },
                ..
            }),
            Message::UiPatch(_)
        ]
    ));
    let double_click = actor
        .handle_event(
            &event(
                super::files_preview_view::PREVIEW_MEDIA,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 20,
                    y: 20,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(8),
        )
        .unwrap();
    assert!(double_click.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::SetFullscreen(true),
            ..
        })
    )));
    assert!(
        !double_click
            .iter()
            .any(|message| matches!(message, Message::MediaRequest(_)))
    );
    for (key_code, modifiers, expected) in [
        (113, 0, 25000),
        (114, 0, 30000),
        (113, 1, 21000),
        (114, 1, 30000),
    ] {
        let seek = actor
            .handle_event(
                &event(
                    super::files_preview_view::PREVIEW_MEDIA,
                    UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers,
                        text: None,
                    },
                ),
                request(8),
            )
            .unwrap();
        assert!(seek.iter().any(|message| matches!(message, Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::MediaControl { session: 900, command: MediaCommand::SeekMillis, value }, ..
        }) if *value == expected)));
    }
    // A visible preview accepts Shift+arrows from the file list without
    // transferring focus. Ordinary arrows retain file navigation semantics.
    actor.preview_fullscreen = false;
    for (key_code, expected) in [(113, 21000), (114, 30000)] {
        let messages = actor
            .handle_event(
                &event(
                    ENTRY_BASE,
                    UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers: 1,
                        text: None,
                    },
                ),
                request(18),
            )
            .unwrap();
        assert!(messages.iter().any(|message| matches!(message,
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: HostOperation::MediaControl { command: MediaCommand::SeekMillis, value, .. }, ..
            }) if *value == expected)));
    }
    for key_code in [111, 113, 114, 116] {
        let messages = actor
            .handle_event(
                &event(
                    ENTRY_BASE,
                    UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers: 0,
                        text: None,
                    },
                ),
                request(19),
            )
            .unwrap();
        assert!(!messages.iter().any(|message| matches!(
            message,
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: HostOperation::MediaControl {
                    command: MediaCommand::SeekMillis,
                    ..
                },
                ..
            })
        )));
    }
    actor.preview_fullscreen = true;
    let launched = actor
        .handle_event(
            &event(
                super::files_preview_view::PREVIEW_MEDIA,
                UiEvent::Key {
                    pressed: true,
                    key_code: 36,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(8),
        )
        .unwrap();
    assert!(launched.iter().any(|message| matches!(message, Message::DataProviderRequest(rice_native_app_protocol::DataProviderRequest {
        operation: rice_native_app_protocol::DataProviderOperation::ApplicationLaunch { application_id, paths, .. }, ..
    }) if application_id == "native:rice-player" && paths == &[String::from("/root/clip.webm")])));
    assert!(launched.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::SetFullscreen(false),
            ..
        })
    )));
    let hidden = actor
        .handle_event(&event(PREVIEW_TOGGLE, UiEvent::Activate), request(8))
        .unwrap();
    assert!(hidden.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: HostOperation::MediaControl {
                session: 900,
                command: MediaCommand::Stop,
                ..
            },
            ..
        })
    )));
    let stale = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::MediaOpened {
                    path: "/root/clip.webm".into(),
                    session: 901,
                    duration_millis: 60_000,
                    error: String::new(),
                },
            },
            request(9),
        )
        .unwrap();
    assert!(matches!(
        stale.as_slice(),
        [Message::HostRequest(
            rice_native_app_protocol::HostRequest {
                operation: HostOperation::MediaControl {
                    session: 901,
                    command: MediaCommand::Stop,
                    ..
                },
                ..
            }
        )]
    ));
}

#[test]
fn location_completion_remains_inside_the_active_tab_page() {
    let (mut actor, _) = loaded_actor();
    actor.open_tab(request(2)).expect("open second tab");
    actor
        .handle_vfs_response(&directory_response(2))
        .expect("load second tab");
    let messages = actor
        .handle_event(
            &event(BREADCRUMB, UiEvent::ValueChanged(String::from("/root/F"))),
            request(3),
        )
        .expect("completion with tabs must produce a valid tree");
    let [Message::UiPatch(patch)] = messages.as_slice() else {
        panic!("completion UI");
    };
    assert!(patch.patches.iter().any(|patch| matches!(patch,
        UiPatch::Create { id, parent: Some(parent), .. }
        if *id == widget_id(86)
            && *parent == widget_id(super::files_view::TAB_PAGE)
    )));
    actor
        .handle_event(&event(BREADCRUMB, enter_key()), request(4))
        .expect("navigate selected completion");
}

#[test]
fn recipe_open_tab_acknowledges_folder_and_does_not_duplicate_retries() {
    let (mut actor, _) = loaded_actor();
    let open = |path: &str| HostEvent {
        app: app(),
        event: HostEventKind::RiceBus {
            topic: String::from("org.ricewm.files/open-tab"),
            payload: serde_json::json!({"path": path}).to_string(),
        },
    };
    let messages = actor
        .handle_host_event(&open("/root/Folder"), request(2))
        .expect("open recipe tab");
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::VfsRequest(VfsRequest {
        operation: VfsOperation::ReadDirectory { path, .. }, ..
    }) if path == "/root/Folder"))
    );
    assert_eq!(actor.document_state().unwrap()["ready"], false);
    assert!(
        actor
            .handle_host_event(&open("/root/Folder"), request(3))
            .expect("retry pending tab")
            .is_empty()
    );
    actor
        .handle_vfs_response(&directory_response(2))
        .expect("load folder");
    let state = actor.document_state().expect("document state");
    assert_eq!(state["openDocuments"], "tabs");
    assert_eq!(state["ready"], true);
    assert_eq!(
        state["documents"],
        serde_json::json!(["/root", "/root/Folder"])
    );
    assert!(
        actor
            .handle_host_event(&open("/root/Folder"), request(4))
            .expect("retry ready tab")
            .is_empty()
    );
    assert_eq!(actor.model.windows()[0].tabs().len(), 2);
    assert!(
        actor
            .handle_host_event(&open("relative/path"), request(5))
            .is_err()
    );
    assert_eq!(actor.model.windows()[0].tabs().len(), 2);
}
