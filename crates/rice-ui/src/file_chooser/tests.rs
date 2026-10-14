use super::*;
use rice_native_app_protocol::{FileMetadata, VfsError};

#[test]
fn save_type_changes_known_suffix_and_supplies_missing_extension() {
    let filters = vec![FileFilter::new("PowerPoint", &["pptx"]), FileFilter::new("OpenDocument", &["odp"]), FileFilter::all_files("All files")];
    let mut chooser = FileChooser::new(FileChooserMode::Save, "/root/Talk.PPTX", filters.clone(), vec![]);
    chooser.select_filter(1);
    assert_eq!(chooser.filename(), "Talk.odp");
    chooser.select_filter(0);
    assert_eq!(chooser.filename(), "Talk.pptx");
    chooser.edit_filename("Custom.backup");
    chooser.select_filter(1);
    assert_eq!(chooser.filename(), "Custom.backup");
    chooser.edit_filename("Talk");
    assert!(matches!(chooser.submit(request(1)), ChooserEffect::Request(VfsOperation::Stat { path }) if path == "/root/Talk.odp"));
    let mut open = FileChooser::new(FileChooserMode::Open, "/root/", filters, vec![]);
    open.edit_filename("Talk.pptx");
    open.select_filter(1);
    assert_eq!(open.filename(), "Talk.pptx");
}

fn request(value: u64) -> RequestId {
    RequestId::new(value).expect("request")
}

fn metadata(kind: FileKind) -> FileMetadata {
    FileMetadata {
        kind,
        size: 42,
        modified_millis: 10,
        mode: 0o644,
    }
}

fn directory(request: u64, entries: &[(&str, FileKind)]) -> VfsResponse {
    VfsResponse {
        request: self::request(request),
        result: VfsResult::Directory {
            entries: entries
                .iter()
                .map(|(name, kind)| DirectoryEntry {
                    name: (*name).to_owned(),
                    metadata: metadata(*kind),
                    mime_type: None,
                })
                .collect(),
            next_cursor: None,
        },
    }
}

fn chooser(mode: FileChooserMode) -> FileChooser {
    FileChooser::new(
        mode,
        if mode == FileChooserMode::Save {
            "/root/note.txt"
        } else {
            "/root/"
        },
        vec![
            FileFilter::typed("Text", &["text/*", "application/json"], &["txt", "md"]),
            FileFilter::all_files("All Files"),
        ],
        vec![Place::new("Home", "/root")],
    )
}

#[test]
fn select_any_accepts_a_highlighted_directory_without_entering_it() {
    let mut chooser = chooser(FileChooserMode::SelectAny);
    let started = chooser.start(request(1));
    assert!(matches!(started, ChooserEffect::Request(_)));
    assert_eq!(
        chooser.handle_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("Documents"),
                    metadata: FileMetadata {
                        kind: FileKind::Directory,
                        size: 0,
                        modified_millis: 0,
                        mode: 0o755,
                    },
                    mime_type: None,
                }],
                next_cursor: None,
            },
        }),
        ChooserEffect::Render,
    );
    chooser.select_visible(0);
    assert_eq!(
        chooser.submit(request(2)),
        ChooserEffect::Accepted(String::from("/root/Documents")),
    );
}

#[test]
fn enumeration_is_bounded_filtered_and_supports_hidden_toggle() {
    let mut chooser = chooser(FileChooserMode::Open);
    assert!(matches!(
        chooser.start(request(1)),
        ChooserEffect::Request(VfsOperation::ReadDirectory {
            ref path,
            cursor: 0,
            limit: 128,
        }) if path == "/root"
    ));
    assert_eq!(
        chooser.handle_response(&directory(
            1,
            &[
                ("src", FileKind::Directory),
                ("note.txt", FileKind::File),
                ("image.png", FileKind::File),
                (".secret.txt", FileKind::File),
            ],
        )),
        ChooserEffect::Render
    );
    assert_eq!(
        chooser
            .visible_entries()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["src", "note.txt"]
    );
    chooser.toggle_hidden();
    assert!(
        chooser
            .visible_entries()
            .iter()
            .any(|entry| entry.name == ".secret.txt")
    );
    chooser.select_filter(1);
    assert!(
        chooser
            .visible_entries()
            .iter()
            .any(|entry| entry.name == "image.png")
    );
}

#[test]
fn provider_places_are_normalized_deduplicated_and_bounded() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.replace_places(vec![
        Place::new(" Local Files ", "/mnt/local/./"),
        Place::new("Duplicate", "/mnt/local"),
        Place::new("Relative", "mnt/local"),
        Place::new("", "/tmp"),
    ]);
    assert_eq!(chooser.places(), &[Place::new("Local Files", "/mnt/local")]);
}

#[test]
fn wheel_and_keyboard_selection_update_bounded_scroll_state() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.start(request(1));
    let entries = (0..20)
        .map(|index| (format!("note-{index}.txt"), FileKind::File))
        .collect::<Vec<_>>();
    chooser.handle_response(&VfsResponse {
        request: request(1),
        result: VfsResult::Directory {
            entries: entries
                .iter()
                .map(|(name, kind)| DirectoryEntry {
                    name: name.clone(),
                    metadata: metadata(*kind),
                    mime_type: None,
                })
                .collect(),
            next_cursor: None,
        },
    });
    chooser.scroll_wheel(5);
    assert_eq!(chooser.scroll(), CHOOSER_ROW_HEIGHT * 3);
    chooser.scroll_wheel(4);
    assert_eq!(chooser.scroll(), 0);
    for _ in 0..8 {
        chooser.move_selection(1);
    }
    assert!(chooser.scroll() > 0);
    assert!(
        chooser.scroll()
            < u32::try_from(chooser.visible_entries().len())
                .unwrap_or(u32::MAX)
                .saturating_mul(CHOOSER_ROW_HEIGHT)
    );
}

#[test]
fn location_autocomplete_is_local_sorted_and_bounded() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.start(request(1));
    let entries = (0..30)
        .map(|index| (format!("note-{index:02}.txt"), FileKind::File))
        .collect::<Vec<_>>();
    let response = VfsResponse {
        request: request(1),
        result: VfsResult::Directory {
            entries: entries
                .iter()
                .map(|(name, kind)| DirectoryEntry {
                    name: name.clone(),
                    metadata: metadata(*kind),
                    mime_type: None,
                })
                .collect(),
            next_cursor: None,
        },
    };
    chooser.handle_response(&response);
    chooser.edit_location("/root/note");
    assert_eq!(chooser.suggestions().len(), MAX_SUGGESTIONS);
    assert_eq!(chooser.suggestions()[0], "/root/note-00.txt");
}

#[test]
fn directory_navigation_commits_only_after_success_and_tracks_history() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.start(request(1));
    chooser.handle_response(&directory(1, &[("src", FileKind::Directory)]));
    let effect = chooser.activate_visible(0, request(2));
    assert!(matches!(
        effect,
        ChooserEffect::Request(VfsOperation::ReadDirectory { ref path, .. }) if path == "/root/src"
    ));
    assert_eq!(chooser.directory(), "/root");
    chooser.handle_response(&directory(2, &[]));
    assert_eq!(chooser.directory(), "/root/src");
    assert!(chooser.can_go_back());
    chooser.navigate_back(request(3));
    chooser.handle_response(&directory(3, &[("src", FileKind::Directory)]));
    assert_eq!(chooser.directory(), "/root");
    assert!(chooser.can_go_forward());
}

#[test]
fn save_requires_valid_filename_and_existing_file_requires_confirmation() {
    let mut chooser = chooser(FileChooserMode::Save);
    chooser.start(request(1));
    chooser.handle_response(&directory(1, &[("note.txt", FileKind::File)]));
    chooser.edit_filename("../bad");
    assert_eq!(chooser.submit(request(2)), ChooserEffect::Render);
    assert!(chooser.notice().is_some());
    chooser.edit_filename("note.txt");
    assert!(matches!(
        chooser.submit(request(3)),
        ChooserEffect::Request(VfsOperation::Stat { ref path }) if path == "/root/note.txt"
    ));
    assert_eq!(
        chooser.handle_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(metadata(FileKind::File)),
        }),
        ChooserEffect::Render
    );
    assert_eq!(chooser.overwrite_path(), Some("/root/note.txt"));
    assert_eq!(
        chooser.confirm_overwrite(),
        ChooserEffect::Accepted(String::from("/root/note.txt"))
    );
}

#[test]
fn missing_save_target_accepts_but_permission_failure_does_not() {
    let mut save_chooser = chooser(FileChooserMode::Save);
    save_chooser.start(request(1));
    save_chooser.handle_response(&directory(1, &[]));
    save_chooser.edit_filename("new.txt");
    save_chooser.submit(request(2));
    assert_eq!(
        save_chooser.handle_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("missing"),
            }),
        }),
        ChooserEffect::Accepted(String::from("/root/new.txt"))
    );

    let mut denied = chooser(FileChooserMode::Save);
    denied.start(request(3));
    denied.handle_response(&directory(3, &[]));
    denied.submit(request(4));
    assert_eq!(
        denied.handle_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::PermissionDenied,
                message: String::from("read only"),
            }),
        }),
        ChooserEffect::Render
    );
    assert!(
        denied
            .notice()
            .is_some_and(|notice| notice.contains("read only"))
    );
}

#[test]
fn paged_enumeration_requests_next_cursor_before_render() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.start(request(9));
    assert_eq!(chooser.pending_request(), Some(request(9)));
    let effect = chooser.handle_response(&VfsResponse {
        request: request(9),
        result: VfsResult::Directory {
            entries: vec![DirectoryEntry {
                name: String::from("a.txt"),
                metadata: metadata(FileKind::File),
                mime_type: None,
            }],
            next_cursor: Some(10),
        },
    });
    assert!(matches!(
        effect,
        ChooserEffect::Request(VfsOperation::ReadDirectory { cursor: 10, .. })
    ));
    chooser.handle_response(&directory(9, &[("b.txt", FileKind::File)]));
    assert_eq!(chooser.pending_request(), None);
    assert_eq!(chooser.visible_entries().len(), 2);
}

#[test]
fn supplied_mime_is_authoritative_and_missing_mime_uses_extension_fallback() {
    let mut chooser = chooser(FileChooserMode::Open);
    chooser.start(request(1));
    chooser.handle_response(&VfsResponse {
        request: request(1),
        result: VfsResult::Directory {
            entries: vec![
                mime_entry("looks-text.txt", "image/png"),
                mime_entry("README.bin", "text/plain"),
                DirectoryEntry {
                    name: String::from("fallback.md"),
                    metadata: metadata(FileKind::File),
                    mime_type: None,
                },
            ],
            next_cursor: None,
        },
    });
    let names = chooser
        .visible_entries()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["fallback.md", "README.bin"]);
    chooser.edit_location("/root/looks-text.txt");
    assert_eq!(chooser.submit(request(2)), ChooserEffect::Render);
    assert!(
        chooser
            .notice()
            .is_some_and(|notice| notice.contains("selected file type"))
    );
}

#[test]
fn unicode_case_numeric_collation_is_stable_across_page_boundaries() {
    let expected = [
        "Angstrom",
        "ångström",
        "Banana",
        "eclair",
        "Éclair",
        "file2.txt",
        "file10.txt",
    ];
    let pages = [
        (
            vec!["file10.txt", "Éclair", "ångström", "Banana"],
            vec!["file2.txt", "eclair", "Angstrom"],
        ),
        (
            vec!["Angstrom", "file2.txt", "eclair"],
            vec!["Banana", "ångström", "Éclair", "file10.txt"],
        ),
    ];
    for (first, second) in pages {
        let mut chooser = chooser(FileChooserMode::Open);
        chooser.start(request(8));
        let first = first
            .into_iter()
            .map(|name| DirectoryEntry {
                name: name.to_owned(),
                metadata: metadata(FileKind::File),
                mime_type: Some(String::from("text/plain")),
            })
            .collect();
        assert!(matches!(
            chooser.handle_response(&VfsResponse {
                request: request(8),
                result: VfsResult::Directory {
                    entries: first,
                    next_cursor: Some(4),
                },
            }),
            ChooserEffect::Request(VfsOperation::ReadDirectory { cursor: 4, .. })
        ));
        let second = second
            .into_iter()
            .map(|name| DirectoryEntry {
                name: name.to_owned(),
                metadata: metadata(FileKind::File),
                mime_type: Some(String::from("text/plain")),
            })
            .collect();
        chooser.handle_response(&VfsResponse {
            request: request(8),
            result: VfsResult::Directory {
                entries: second,
                next_cursor: None,
            },
        });
        assert_eq!(
            chooser
                .visible_entries()
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

fn mime_entry(name: &str, mime_type: &str) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        metadata: metadata(FileKind::File),
        mime_type: Some(mime_type.to_owned()),
    }
}

#[test]
fn builtin_labels_refresh_but_custom_labels_and_filter_rules_remain_literal() {
    let chooser = FileChooser::new(FileChooserMode::Open, "/root/",
        vec![FileFilter::typed("Supported Videos", &["video/*"], &["mp4"]).localized(),
             FileFilter::all_files("Supported Videos")],
        vec![Place::localized("Home", "/root/"), Place::new("Home", "/mnt/custom/")]);
    for (locale, home, videos) in [(1, "Inicio", "Vídeos compatibles"), (2, "主目录", "支持的视频")] {
        rice_i18n::with_locale(locale, || {
            assert_eq!(chooser.places()[0].display_label(), home);
            assert_eq!(chooser.places()[0].label, "Home");
            assert_eq!(chooser.places()[0].path, "/root");
            assert_eq!(chooser.places()[1].display_label(), "Home");
            assert_eq!(chooser.places()[1].path, "/mnt/custom");
            assert_eq!(chooser.filters()[0].label(), videos);
            assert_eq!(chooser.filters()[1].label(), "Supported Videos");
            assert!(chooser.filters()[0].accepts("Home.MP4", None));
            assert!(chooser.filters()[0].accepts("unknown", Some("video/webm")));
            assert!(!chooser.filters()[0].accepts("Home.txt", Some("text/plain")));
            let mut patches = Vec::new();
            compose_file_chooser(&mut patches, &chooser, FileChooserViewIds::new(200, 204, 208, 230));
            for text in [home, videos, "Home", "Supported Videos"] {
                assert!(patches.iter().any(|patch| matches!(patch, crate::Patch::Create {widget, ..} if widget.text.as_deref() == Some(text))), "missing {text}");
            }
        });
    }
}

#[test]
fn existing_validation_and_host_errors_retranslate_without_changing_user_data() {
    let mut chooser = chooser(FileChooserMode::Save);
    chooser.edit_filename("bad/name");
    let ids = FileChooserViewIds::new(200, 204, 208, 230);
    for locale in [1, 2] {
        rice_i18n::with_locale(locale, || {
            let expected = rice_i18n::t("The file name contains an invalid character");
            assert_ne!(expected, "The file name contains an invalid character");
            let mut patches = Vec::new(); compose_file_chooser(&mut patches, &chooser, ids);
            assert!(patches.iter().any(|patch| matches!(patch, crate::Patch::Create {widget, ..} if widget.id.get() == ids.notice() && widget.text.as_deref() == Some(expected))));
        });
    }
    chooser.start(request(10));
    chooser.handle_response(&VfsResponse {request: request(10), result: VfsResult::Error(VfsError {
        code: VfsErrorCode::PermissionDenied, message: "The file name is too long {error}".into(),
    })});
    for (locale, expected) in [(1, "No se pudo abrir la carpeta: The file name is too long {error}"),
                               (2, "无法打开文件夹：The file name is too long {error}")] {
        rice_i18n::with_locale(locale, || {
            let mut patches = Vec::new(); compose_file_chooser(&mut patches, &chooser, ids);
            assert!(patches.iter().any(|patch| matches!(patch, crate::Patch::Create {widget, ..} if widget.id.get() == ids.notice() && widget.text.as_deref() == Some(expected))));
        });
    }
    assert_eq!(chooser.filename(), "bad/name");
    let empty = ChooserNotice::failure("Could not open folder: {error}", " ");
    rice_i18n::with_locale(2, || assert_eq!(empty.render(), "无法打开文件夹：VFS 操作失败"));
}
