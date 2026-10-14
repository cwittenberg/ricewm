use super::*;
use rice_native_app_protocol::{
    APPLICATION_CATALOG_VERSION, DataProviderOperation, DataProviderRequest, DataProviderResponse,
    DataProviderResult, DirectoryEntry, DragAction, DragEnvelope, DragId, DragPayload, DragPhase,
    DragSourceEvent, DragSourceResult, DragSourceState, FileKind, FileMetadata, HostEvent,
    HostEventKind, HostOperation, HostRequest, InstalledApplication, MediaKind, MediaMetadata,
    MediaResponse, MediaResult, SettingEntry, SettingsOperation, SettingsRequest, SettingsResponse,
    SettingsResult, UiPatch, UiPatchEnvelope, VfsError, VfsErrorCode, VfsOperation, VfsRequest,
    VfsResponse, VfsResult,
};

fn app(value: u64) -> AppId {
    AppId::new(value).expect("test application ID")
}

fn request(value: u64) -> RequestId {
    RequestId::new(value).expect("test request ID")
}

fn event(target: u64, event: UiEvent) -> UiEventEnvelope {
    UiEventEnvelope {
        app: app(7),
        window: window_id(WINDOW),
        target: widget_id(target),
        event,
    }
}

fn envelope(message: Message) -> UiPatchEnvelope {
    let Message::UiPatch(envelope) = message else {
        panic!("desktop emits a UI patch");
    };
    envelope
}

fn directory_entry(name: &str, kind: FileKind) -> DirectoryEntry {
    DirectoryEntry {
        name: name.to_owned(),
        metadata: FileMetadata {
            kind,
            size: 12,
            modified_millis: 1,
            mode: 0o644,
        },
        mime_type: None,
    }
}

fn launch() -> RiceDesktop {
    let (desktop, messages) =
        RiceDesktop::launch(app(7), &[String::from("--workspace=work")], request(1))
            .expect("launch");
    assert_eq!(messages.len(), 5);
    let Message::VfsRequest(VfsRequest {
        request: emitted,
        operation:
            VfsOperation::ReadDirectory {
                path,
                cursor,
                limit,
            },
        ..
    }) = &messages[1]
    else {
        panic!("launch requests desktop enumeration");
    };
    assert_eq!(*emitted, request(1));
    assert_eq!(path, desktop_vfs::DESKTOP_PATH);
    assert_eq!((*cursor, *limit), (0, 128));
    assert!(matches!(
        &messages[2],
        Message::SettingsRequest(SettingsRequest {
            operation: SettingsOperation::Load,
            ..
        })
    ));
    for (message, token) in [
        (&messages[3], WATCH_EVENT_TOKEN),
        (&messages[4], TRASH_WATCH_EVENT_TOKEN),
    ] {
        assert!(matches!(
            message,
            Message::HostRequest(HostRequest {
                operation: HostOperation::SetTimer {
                    token: actual,
                    enabled: true,
                    ..
                },
                ..
            }) if *actual == token
        ));
    }
    desktop
}

#[test]
fn trash_watch_repaints_empty_and_full_recycle_bin_states() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let status = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::Timer {
                    token: TRASH_WATCH_EVENT_TOKEN,
                },
            },
            request(2),
        )
        .expect("trash timer");
    assert!(matches!(
        status.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == desktop_vfs::TRASH_FILES_PATH
    ));
    let full = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: vec![directory_entry("deleted.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("nonempty trash status");
    assert!(matches!(
        desktop
            .items
            .iter()
            .find(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
            .map(|item| &item.kind),
        Some(DesktopItemKind::Trash { empty: false })
    ));
    assert!(matches!(full.as_slice(), [Message::UiPatch(_)]));

    let status = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::Timer {
                    token: TRASH_WATCH_EVENT_TOKEN,
                },
            },
            request(3),
        )
        .expect("second trash timer");
    assert_eq!(status.len(), 1);
    let empty = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("empty trash status");
    assert!(matches!(
        desktop
            .items
            .iter()
            .find(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
            .map(|item| &item.kind),
        Some(DesktopItemKind::Trash { empty: true })
    ));
    assert!(matches!(empty.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn redmond_desktop_uses_classic_vertical_pitch_without_changing_other_themes() {
    let orchard = desktop_grid_config(849, 799, 4, false);
    let redmond = desktop_grid_config(849, 799, 4, true);

    assert_eq!(orchard.cell_height, GridConfig::default().cell_height);
    assert_eq!(orchard.vertical_gap, GridConfig::default().vertical_gap);
    assert_eq!(
        orchard.cell_height + orchard.vertical_gap,
        GridConfig::default().cell_height + GridConfig::default().vertical_gap,
        "non-Redmond desktop geometry must remain unchanged",
    );
    assert_eq!(redmond.cell_width + redmond.horizontal_gap, 75);
    assert_eq!(redmond.cell_height + redmond.vertical_gap, 75);
}

#[test]
fn redmond_launch_serializes_the_same_grid_geometry_used_for_hit_testing() {
    let (_, messages) = RiceDesktop::launch(
        app(7),
        &[
            String::from("--workspace=work"),
            String::from("--theme-profile=redmond-98"),
        ],
        request(1),
    )
    .expect("Redmond desktop launch");
    let initial = envelope(messages.into_iter().next().expect("initial UI"));
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(ICON_GRID)
                && text.starts_with("desktop-grid:")
                && text.ends_with(":68:68:7:7:4:4")
    )));
}

#[test]
fn live_theme_change_restores_desktop_geometry_without_leaking_into_orchard() {
    let mut desktop = launch();
    let orchard = desktop.model.grid().config();
    let redmond_update = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::RiceBus {
                    topic: THEME_CHANGED_TOPIC.to_owned(),
                    payload: String::from("redmond-98"),
                },
            },
            request(2),
        )
        .expect("enter Redmond");
    assert!(desktop.redmond98);
    assert_eq!(desktop.model.grid().config().cell_width, 68);
    assert_eq!(desktop.model.grid().config().cell_height, 68);
    assert_eq!(desktop.model.grid().config().horizontal_gap, 7);
    assert_eq!(desktop.model.grid().config().vertical_gap, 7);
    assert!(matches!(redmond_update.as_slice(), [Message::UiPatch(_)]));

    let orchard_update = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::RiceBus {
                    topic: THEME_CHANGED_TOPIC.to_owned(),
                    payload: String::from("orchard"),
                },
            },
            request(3),
        )
        .expect("leave Redmond");
    assert!(!desktop.redmond98);
    assert_eq!(desktop.model.grid().config(), orchard);
    assert!(matches!(orchard_update.as_slice(), [Message::UiPatch(_)]));
}

#[test]
fn wallpaper_is_never_an_interactive_desktop_hit_surface() {
    let (_, messages) = RiceDesktop::launch(
        app(7),
        &[
            String::from("--workspace=work"),
            String::from("--wallpaper=/root/Pictures/wallpaper.png"),
        ],
        request(1),
    )
    .expect("desktop launch with wallpaper");
    let Message::UiPatch(initial) = &messages[0] else {
        panic!("desktop launch emits its retained tree");
    };
    assert!(initial.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, state, .. }
            if *id == widget_id(WALLPAPER) && state & WidgetState::ENABLED.bits() == 0
    )));

    let mut desktop = RiceDesktop::launch(
        app(7),
        &[
            String::from("--workspace=work"),
            String::from("--wallpaper=/root/Pictures/wallpaper.png"),
        ],
        request(1),
    )
    .expect("desktop launch with wallpaper")
    .0;
    let ready = desktop
        .handle_media_response(
            &MediaResponse {
                request: request(1),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Image,
                    width: 1920,
                    height: 1080,
                    duration_millis: None,
                    mime_type: String::from("image/png"),
                }),
            },
            request(2),
        )
        .expect("decoded wallpaper");
    let [Message::UiPatch(updated)] = ready.as_slice() else {
        panic!("decoded wallpaper updates its presentation node");
    };
    assert!(updated.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Update {
            id,
            state: rice_native_app_protocol::PatchValue::Set(state),
            ..
        }
            if *id == widget_id(WALLPAPER) && state & WidgetState::ENABLED.bits() == 0
    )));
}

fn enumerate(
    desktop: &mut RiceDesktop,
    request_id: RequestId,
    entries: Vec<DirectoryEntry>,
) -> UiPatchEnvelope {
    let messages = desktop
        .handle_vfs_response(&VfsResponse {
            request: request_id,
            result: VfsResult::Directory {
                entries,
                next_cursor: None,
            },
        })
        .expect("directory response");
    envelope(
        messages
            .into_iter()
            .find(|message| matches!(message, Message::UiPatch(_)))
            .expect("enumeration patch"),
    )
}

#[test]
fn ricefiles_uri_drop_is_accepted_by_the_desktop_and_starts_a_copy() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let drag = DragId::new(44).expect("drag");
    let enter = DragEnvelope {
        drag,
        app: app(7),
        window: window_id(WINDOW),
        phase: DragPhase::Enter,
        target: Some(widget_id(ICON_GRID)),
        x: 40,
        y: 40,
        mime_types: vec![String::from("text/uri-list")],
        action: DragAction::Copy,
        payload: None,
    };
    assert!(
        desktop
            .handle_external_drag(&enter, request(2))
            .expect("desktop accepts file drag")
            .is_empty()
    );
    let dropped = desktop
        .handle_external_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/report.txt\r\n".to_vec(),
                }),
                ..enter
            },
            request(3),
        )
        .expect("desktop drop");
    assert!(matches!(
        dropped.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/report.txt"
    ));

    let applying = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("not found"),
            }),
        })
        .expect("missing destination starts copy");
    assert!(matches!(
        applying.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Copy {
                from,
                to,
                recursive: true,
            },
            ..
        })] if from == "/root/report.txt" && to == "/root/Desktop/report.txt"
    ));

    assert!(
        desktop
            .handle_vfs_response(&VfsResponse {
                request: request(3),
                result: VfsResult::Progress(rice_native_app_protocol::VfsProgress {
                    sequence: 1,
                    completed_bytes: 12,
                    total_bytes: 12,
                }),
            })
            .expect("copy progress remains nonterminal")
            .is_empty()
    );
    assert!(desktop.pending_transfer.is_some());

    assert!(
        desktop
            .handle_host_event(
                &HostEvent {
                    app: app(7),
                    event: HostEventKind::Timer {
                        token: WATCH_EVENT_TOKEN,
                    },
                },
                request(4),
            )
            .expect("filesystem watch coalesces behind the active copy")
            .is_empty()
    );

    let completed = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Done,
        })
        .expect("copy completion follows progress");
    assert!(matches!(
        completed.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == "/root/Desktop"
    ));

    let interim = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: vec![directory_entry("photo.png", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("copy refresh with a coalesced filesystem wake");
    assert!(matches!(
        interim.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == "/root/Desktop"
    ));
    assert!(
        !interim
            .iter()
            .any(|message| matches!(message, Message::MediaRequest(_))),
        "the interim refresh must not claim the final request ID for thumbnails"
    );

    let final_refresh = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Directory {
                entries: vec![directory_entry("photo.png", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("final coalesced refresh");
    assert!(matches!(
        final_refresh.as_slice(),
        [Message::UiPatch(_), Message::MediaRequest(MediaRequest {
            path,
            mime_type,
            ..
        })] if path == "/root/Desktop/photo.png" && mime_type == "image/png"
    ));
}

#[test]
fn desktop_files_offer_uri_drags_with_shift_copy_and_refresh_after_a_move() {
    for (modifiers, expected) in [(0, DragAction::Move), (1, DragAction::Copy)] {
        let mut desktop = launch();
        enumerate(
            &mut desktop,
            request(1),
            vec![directory_entry("report 1.txt", FileKind::File)],
        );
        let item = desktop
            .items
            .iter()
            .find(|item| item.label == "report 1.txt")
            .expect("desktop file")
            .id;
        let offered = desktop
            .handle_event(
                &event(
                    widget_for_item(item).get(),
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 40,
                        y: 40,
                        button: 1,
                        modifiers,
                    },
                ),
                request(22),
            )
            .expect("desktop drag offer");
        let Message::DragSource(source) = &offered[1] else {
            panic!("desktop selection emits a typed drag offer");
        };
        let DragSourceState::Offer {
            action,
            mime_types,
            payload,
        } = &source.state
        else {
            panic!("source offer state");
        };
        assert_eq!(*action, expected);
        assert_eq!(
            mime_types,
            &[
                String::from("text/uri-list"),
                String::from("x-special/gnome-copied-files"),
            ]
        );
        assert_eq!(payload.mime_type, "text/uri-list");
        assert_eq!(payload.data, b"file:///root/Desktop/report%201.txt\r\n");
        let finished = desktop
            .finish_source_drag(&DragSourceEvent {
                drag: source.drag,
                app: source.app,
                window: source.window,
                state: DragSourceState::Finished(DragSourceResult::Completed(expected)),
            })
            .expect("desktop consumes host result");
        if expected == DragAction::Move {
            assert!(matches!(
                finished.as_slice(),
                [Message::VfsRequest(VfsRequest {
                    operation: VfsOperation::ReadDirectory { path, .. },
                    ..
                })] if path == desktop_vfs::DESKTOP_PATH
            ));
        } else {
            assert!(matches!(finished.as_slice(), [Message::UiPatch(_)]));
        }
        assert!(desktop.source_drag.is_none());
    }
}

#[test]
fn desktop_drag_to_a_folder_starts_a_move_but_background_drop_only_repositions() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("Folder", FileKind::Directory),
            directory_entry("move-me.txt", FileKind::File),
        ],
    );
    let folder = desktop
        .items
        .iter()
        .find(|item| item.label == "Folder")
        .expect("folder")
        .id;
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "move-me.txt")
        .expect("file")
        .id;
    let offered = desktop
        .handle_event(
            &event(
                widget_for_item(file).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(30),
        )
        .expect("desktop source");
    let Message::DragSource(source) = &offered[1] else {
        panic!("source offer");
    };
    let enter = DragEnvelope {
        drag: source.drag,
        app: app(7),
        window: window_id(WINDOW),
        target: Some(widget_for_item(folder)),
        phase: DragPhase::Enter,
        action: DragAction::Move,
        x: 80,
        y: 80,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    desktop
        .handle_external_drag(&enter, request(31))
        .expect("folder accepts desktop drag");
    let moved = desktop
        .handle_external_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Desktop/move-me.txt\r\n".to_vec(),
                }),
                ..enter
            },
            request(32),
        )
        .expect("desktop folder drop");
    assert!(matches!(
        moved.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/Folder/move-me.txt"
    ));

    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("move-me.txt", FileKind::File)],
    );
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "move-me.txt")
        .expect("desktop file")
        .id;
    let before = desktop.model().grid().cell_of(file).expect("initial cell");
    let origin = desktop.model().grid().config().rect(before);
    let offered = desktop
        .handle_event(
            &event(
                widget_for_item(file).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: origin.x + 20,
                    y: origin.y + 20,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(40),
        )
        .expect("desktop source");
    let Message::DragSource(source) = &offered[1] else {
        panic!("source offer");
    };
    let background = DragEnvelope {
        drag: source.drag,
        app: app(7),
        window: window_id(WINDOW),
        target: Some(widget_id(ICON_GRID)),
        phase: DragPhase::Enter,
        action: DragAction::Move,
        x: origin.x + 220,
        y: origin.y + 20,
        mime_types: vec![String::from("text/uri-list")],
        payload: None,
    };
    desktop
        .handle_external_drag(&background, request(41))
        .expect("background accepts reposition");
    let repositioned = desktop
        .handle_external_drag(
            &DragEnvelope {
                phase: DragPhase::Drop,
                payload: Some(DragPayload {
                    mime_type: String::from("text/uri-list"),
                    data: b"file:///root/Desktop/move-me.txt\r\n".to_vec(),
                }),
                ..background
            },
            request(42),
        )
        .expect("desktop background reposition");
    assert!(matches!(
        repositioned.as_slice(),
        [Message::UiPatch(_), Message::SettingsRequest(_)]
    ));
    assert_ne!(desktop.model().grid().cell_of(file), Some(before));
}

fn open_item_context(
    desktop: &mut RiceDesktop,
    item: DesktopItemId,
    request_id: RequestId,
) -> UiPatchEnvelope {
    let messages = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 40,
                    y: 40,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request_id,
        )
        .expect("item context");
    envelope(messages[0].clone())
}

fn assert_initial_shell(shell: &UiPatchEnvelope) {
    assert!(shell.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                text: Some(text),
                ..
            } if *id == widget_id(ROOT) && text == "desktop:transparent"
        )
    }));
    assert!(
        !shell
            .patches
            .iter()
            .any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(SETTINGS)))
    );
    for removed in "Wallpaper Preferences|Center|Tile|Stretch|Fit|Crop".split('|') {
        assert!(
            !shell.patches.iter().any(|patch| {
                matches!(patch, UiPatch::Create { text: Some(text), .. } if text == removed)
            }),
            "background context retained inline setting {removed}"
        );
    }
    assert!(
        !shell
            .patches
            .iter()
            .any(|patch| matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(MENU_BAR)))
    );
    assert!(shell.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                kind,
                parent: Some(parent),
                ..
            } if *id == widget_id(ICON_GRID)
                && *kind == WidgetKind::IconGrid as u16
                && *parent == widget_id(ROOT)
        )
    }));
}

#[test]
fn launch_is_empty_until_bounded_desktop_enumeration_completes() {
    let (desktop, messages) =
        RiceDesktop::launch(app(7), &[String::from("--workspace=work")], request(1))
            .expect("launch");
    assert_eq!(desktop.model().workspace().as_str(), "work");
    let Message::UiPatch(shell) = &messages[0] else {
        panic!("launch emits desktop shell");
    };
    assert_initial_shell(shell);
    assert!(
        !shell.patches.iter().any(|patch| {
            matches!(patch, UiPatch::Create { kind, .. } if *kind == WidgetKind::ToolBar as u16)
        }),
        "desktop commands belong to the contextual menu"
    );
    assert!(!shell.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                parent: Some(parent),
                kind,
                ..
            } if *parent == widget_id(ROOT) && *kind == WidgetKind::Row as u16
        )
    }));
    assert_eq!(desktop.model().grid().len(), 0);

    let initial = envelope(messages[0].clone());
    assert_eq!(initial.sequence, 1);
    assert!(!initial.patches.iter().any(|entry| {
        matches!(
            entry,
            UiPatch::Create {
                id,
                ..
            } if id.get() >= ITEM_BASE
        )
    }));
    let Message::VfsRequest(request) = &messages[1] else {
        panic!("launch emits VFS request");
    };
    assert_eq!(request.app, app(7));
}

#[test]
fn directory_entries_become_semantic_items_in_stable_sorted_order() {
    let mut desktop = launch();
    let patch = enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("z.txt", FileKind::File),
            directory_entry("Apps", FileKind::Directory),
            directory_entry("Rice.desktop", FileKind::File),
        ],
    );
    assert_eq!(patch.sequence, 2);
    assert_eq!(desktop.items.len(), 6);
    assert_eq!(
        desktop
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["Apps", "Documents", "Home", "Pictures", "Trash", "z.txt"]
    );
    assert!(matches!(desktop.items[0].kind, DesktopItemKind::Folder));
    assert_eq!(
        desktop.items[5].path.as_deref(),
        Some("/root/Desktop/z.txt")
    );
    assert_eq!(
        patch
            .patches
            .iter()
            .filter(|entry| matches!(
                entry,
                UiPatch::Create {
                    kind,
                    parent: Some(parent),
                    ..
                } if *kind == WidgetKind::Button as u16 && *parent == widget_id(ICON_GRID)
            ))
            .count(),
        6
    );
}

#[test]
fn executable_desktop_entries_are_rendered_as_application_launchers() {
    let mut desktop = launch();
    let mut launcher = directory_entry("RicePlayer.desktop", FileKind::File);
    launcher.metadata.mode = 0o755;
    let patch = enumerate(&mut desktop, request(1), vec![launcher]);
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "RicePlayer")
        .expect("player launcher");
    assert!(matches!(
        &item.kind,
        DesktopItemKind::Application { desktop_id }
            if desktop_id == "RicePlayer.desktop"
    ));
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create { id, icon: Some(icon), .. }
            if *id == widget_for_item(item.id) && *icon == IconId::Application as u16
    )));
}

#[test]
fn secondary_click_selects_item_without_activating_it() {
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
    let messages = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
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
        .expect("secondary click");
    assert_eq!(messages.len(), 1);
    assert!(desktop.model().selection().contains(item));
    let patch = envelope(messages[0].clone());
    assert!(!patch.patches.iter().any(
        |entry| matches!(entry, UiPatch::Create { id, .. } if *id == widget_id(RENAME_FIELD))
    ));
    assert!(
        patch
            .patches
            .iter()
            .any(|entry| matches!(entry, UiPatch::Create { id, .. } if *id == widget_id(MENU_BAR)))
    );
    assert!(patch.patches.iter().any(|entry| {
        matches!(
            entry,
            UiPatch::Create {
                id,
                text: Some(text),
                ..
            } if *id == widget_id(DELETE) && text == "Move to Trash"
        )
    }));
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create {
            id,
            text: Some(text),
            ..
        } if *id == widget_id(PROPERTIES) && text == "Properties"
    )));
    assert!(
        desktop.pending_intents().is_empty(),
        "opening a context menu does not activate the item"
    );
    let background = desktop
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
            request(3),
        )
        .expect("background context");
    let background = envelope(background[0].clone());
    let labels = background
        .patches
        .iter()
        .filter_map(|entry| match entry {
            UiPatch::Create {
                text: Some(text), ..
            } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(labels.iter().filter(|label| **label == rice_ui::MENU_SEPARATOR).count(), 2,
        "desktop context keeps the policy separators between action groups");
    assert!(labels.windows(3).any(|group| group == ["Refresh", rice_ui::MENU_SEPARATOR, "Desktop Settings"]));
    for expected in [
        "New Folder",
        "New File",
        "Select All",
        "Auto-arrange icons",
        "Refresh",
        "Desktop Settings",
    ] {
        assert!(
            labels.contains(&expected),
            "missing background action {expected}"
        );
    }
    assert!(
        background
            .patches
            .iter()
            .any(|entry| { matches!(entry, UiPatch::Remove { id } if *id == widget_id(MENU_BAR)) })
    );
    assert!(background.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create {
            id,
            kind,
            state,
            ..
        } if *id == widget_id(ARRANGE)
            && *kind == WidgetKind::CheckBox as u16
            && state & WidgetState::CHECKED.bits() == 0
    )));

    desktop
        .handle_event(&event(ARRANGE, UiEvent::Activate), request(4))
        .expect("enable auto-arrange");
    let checked = desktop
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
            request(5),
        )
        .expect("reopen background context");
    let checked = envelope(checked[0].clone());
    assert!(checked.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Update {
            id,
            state: rice_native_app_protocol::PatchValue::Set(state),
            ..
        } if *id == widget_id(ARRANGE) && state & WidgetState::CHECKED.bits() != 0
    )));
}

#[test]
fn context_menu_primary_press_does_not_clear_the_item_or_start_desktop_dragging() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Folder", FileKind::Directory)],
    );
    let folder = desktop
        .items
        .iter()
        .find(|item| item.label == "Folder")
        .expect("folder")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(folder).get(),
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
        .expect("folder context menu");

    let pressed = desktop
        .handle_event(
            &event(
                OPEN,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 60,
                    y: 60,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("press context action");
    assert!(pressed.is_empty());
    assert!(desktop.model().selection().contains(folder));
    assert!(desktop.drag.is_none());
    assert!(desktop.rubber_band.is_none());
    assert!(desktop.source_drag.is_none());

    let opened = desktop
        .handle_event(&event(OPEN, UiEvent::Activate), request(4))
        .expect("open selected folder");
    assert!(opened.iter().any(|message| matches!(
        message,
        Message::Launch {
            entrypoint,
            arguments,
            ..
        } if entrypoint == "rice-files"
            && arguments == &[String::from("/root/Desktop/Folder")]
    )));
}

#[test]
fn escape_cancels_an_armed_desktop_drag() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("drag-me.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "drag-me.txt")
        .expect("desktop file")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("arm drag");

    let cancelled = desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Key {
                    pressed: true,
                    key_code: 9,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(3),
        )
        .expect("Escape");
    assert!(desktop.drag.is_none());
    assert!(desktop.rubber_band.is_none());
    assert!(desktop.source_drag.is_none());
    assert!(cancelled.iter().any(|message| matches!(
        message,
        Message::DragSource(DragSourceEvent {
            state: DragSourceState::Cancel,
            ..
        })
    )));
}

#[test]
fn focus_loss_cancels_desktop_pointer_ownership_and_removes_the_context_menu() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("drag-me.txt", FileKind::File)],
    );
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
        .expect("open desktop context menu");
    assert_eq!(desktop.context_kind, ContextKind::Background);

    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "drag-me.txt")
        .expect("desktop file")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("arm desktop drag");
    assert!(desktop.drag.is_some());
    assert!(desktop.source_drag.is_some());

    let messages = desktop
        .handle_host_event(
            &HostEvent {
                app: app(7),
                event: HostEventKind::WindowState {
                    window: window_id(WINDOW),
                    content_width: DEFAULT_DESKTOP_WIDTH,
                    content_height: DEFAULT_DESKTOP_HEIGHT,
                    fullscreen: false,
                    maximized: false,
                    focused: false,
                    visible: true,
                },
            },
            request(4),
        )
        .expect("desktop focus loss");

    assert!(desktop.drag.is_none());
    assert!(desktop.rubber_band.is_none());
    assert!(desktop.source_drag.is_none());
    assert_eq!(desktop.context_kind, ContextKind::Hidden);
    assert!(desktop.context_action_ids.is_empty());
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::DragSource(DragSourceEvent {
            state: DragSourceState::Cancel,
            ..
        })
    )));
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::UiPatch(envelope)
            if envelope.patches.iter().any(|patch| matches!(
                patch,
                UiPatch::Remove { id } if *id == widget_id(MENU_BAR)
            ))
    )));

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
            request(5),
        )
        .expect("reopen desktop context menu after focus returns");
    assert_eq!(desktop.context_kind, ContextKind::Background);
}

#[test]
fn primary_click_selects_without_opening_and_double_click_activates() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("keep.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "keep.txt")
        .expect("keep")
        .id;
    let selected = desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("selection");
    assert!(matches!(
        selected.as_slice(),
        [Message::UiPatch(_), Message::DragSource(_)]
    ));
    assert!(desktop.model().selection().contains(item));
    assert!(desktop.pending_desktop_launch.is_none());

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
            request(3),
        )
        .expect("double click activation");
    assert!(desktop.model().selection().is_empty());

    let refresh = desktop
        .handle_event(&event(REFRESH, UiEvent::Activate), request(4))
        .expect("refresh");
    assert_eq!(refresh.len(), 1);
    assert!(desktop.model().selection().is_empty());
    enumerate(
        &mut desktop,
        request(4),
        vec![directory_entry("keep.txt", FileKind::File)],
    );
    assert!(desktop.model().selection().is_empty());
    assert_eq!(
        desktop
            .items
            .iter()
            .find(|item| item.label == "keep.txt")
            .expect("keep after refresh")
            .id,
        item
    );
}

#[test]
fn executable_desktop_script_double_click_runs_it_in_a_terminal() {
    let mut desktop = launch();
    let mut script = directory_entry("change wallpaper.run", FileKind::File);
    script.metadata.mode = 0o755;
    enumerate(&mut desktop, request(1), vec![script]);
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "change wallpaper.run")
        .expect("desktop script")
        .id;
    let launched = desktop
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
        .expect("double click script");
    assert!(launched.iter().any(|message| matches!(
        message,
        Message::Launch { entrypoint, arguments, .. }
            if entrypoint == "rice-term"
                && arguments == &[String::from(
                    "exec '/root/Desktop/change wallpaper.run'"
                )]
    )));
}

#[test]
fn desktop_media_thumbnails_render_and_movie_double_click_uses_application_provider() {
    let mut desktop = launch();
    let loaded = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![
                    directory_entry("clip.mp4", FileKind::File),
                    directory_entry("photo.png", FileKind::File),
                ],
                next_cursor: None,
            },
        })
        .expect("desktop media enumeration");
    let [Message::UiPatch(_), Message::MediaRequest(movie)] = loaded.as_slice() else {
        panic!("desktop starts its first thumbnail decode");
    };
    assert_eq!(movie.path, "/root/Desktop/clip.mp4");
    assert_eq!(movie.mime_type, "video/mp4");

    let movie_ready = desktop
        .handle_media_response(
            &MediaResponse {
                request: request(1),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Video,
                    width: 320,
                    height: 180,
                    duration_millis: Some(3_000),
                    mime_type: String::from("video/mp4"),
                }),
            },
            request(2),
        )
        .expect("movie thumbnail");
    let [Message::UiPatch(movie_patch), Message::MediaRequest(image)] = movie_ready.as_slice()
    else {
        panic!("movie thumbnail redraws and queues the image");
    };
    assert!(movie_patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { kind, text: Some(text), .. }
            if *kind == WidgetKind::Image as u16 && text == "media-request:1"
    )));
    assert!(movie_patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { kind, text: Some(text), .. }
            if *kind == WidgetKind::Container as u16 && text == "desktop:thumbnail-cell"
    )));
    assert!(movie_patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { kind, state, text: Some(text), .. }
            if *kind == WidgetKind::Button as u16
                && text == "clip.mp4"
                && state & WidgetState::FLAT.bits() != 0
    )));
    assert_eq!(image.path, "/root/Desktop/photo.png");
    assert_eq!(image.mime_type, "image/png");

    let image_ready = desktop
        .handle_media_response(
            &MediaResponse {
                request: request(2),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Image,
                    width: 200,
                    height: 120,
                    duration_millis: None,
                    mime_type: String::from("image/png"),
                }),
            },
            request(3),
        )
        .expect("image thumbnail");
    assert!(matches!(image_ready.as_slice(), [Message::UiPatch(_)]));

    let movie_item = desktop
        .items
        .iter()
        .find(|item| item.label == "clip.mp4")
        .expect("movie item")
        .id;
    let opening = desktop
        .handle_event(
            &event(
                desktop_view::THUMBNAIL_IMAGE_BASE + movie_item.0,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 40,
                    y: 40,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(4),
        )
        .expect("double-click movie thumbnail");
    assert!(matches!(
        opening.as_slice(),
        [Message::UiPatch(_), Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "video/mp4"
    ));

    let launching = desktop
        .handle_application_response(
            &DataProviderResponse {
                request: request(4),
                result: DataProviderResult::Applications {
                    schema_version: APPLICATION_CATALOG_VERSION,
                    mime_type: String::from("video/mp4"),
                    entries: vec![InstalledApplication {
                        id: String::from("native:rice-player"),
                        name: String::from("Rice Player"),
                        icon: String::from("media-play"),
                        mime_types: vec![String::from("video/*")],
                        matches_requested_type: true,
                        is_default: true,
                        is_native: true,
                    }],
                },
            },
            request(5),
        )
        .expect("application catalog");
    assert!(matches!(
        launching.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                mime_type,
                paths,
                ..
            },
            ..
        })] if application_id == "native:rice-player"
            && mime_type == "video/mp4"
            && paths == &[String::from("/root/Desktop/clip.mp4")]
    ));
}

#[test]
fn desktop_executable_replaces_generic_program_icon_only_after_exact_pe_decode() {
    let mut desktop = launch();
    let loaded = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![directory_entry("sample.exe", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("desktop executable enumeration");
    let [Message::UiPatch(initial), Message::MediaRequest(icon)] = loaded.as_slice() else {
        panic!("desktop starts one executable icon decode");
    };
    assert_eq!(icon.path, "/root/Desktop/sample.exe");
    assert_eq!(icon.mime_type, "application/x-msdownload");
    assert!(
        initial.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { kind, icon: Some(_), text: Some(text), .. }
                if *kind == WidgetKind::Button as u16 && text == "sample.exe"
        )),
        "generic executable icon remains visible while PE extraction is pending"
    );

    let ready = desktop
        .handle_media_response(
            &MediaResponse {
                request: request(1),
                result: MediaResult::Ready(MediaMetadata {
                    kind: MediaKind::Image,
                    width: 32,
                    height: 32,
                    duration_millis: None,
                    mime_type: String::from("application/x-msdownload"),
                }),
            },
            request(2),
        )
        .expect("desktop executable icon");
    assert!(ready.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|candidate| matches!(
            candidate,
            UiPatch::Create { kind, text: Some(text), .. }
                if *kind == WidgetKind::Image as u16 && text == "media-request:1"
        ))
    )));
}

#[test]
fn desktop_launcher_queues_its_pe_target_for_host_side_exact_icon_resolution() {
    let mut desktop = launch();
    let mut launcher = directory_entry("Sample.desktop", FileKind::File);
    launcher.metadata.mode = 0o755;
    let loaded = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![launcher],
                next_cursor: None,
            },
        })
        .expect("desktop launcher enumeration");
    assert!(matches!(
        loaded.as_slice(),
        [Message::UiPatch(_), Message::MediaRequest(icon)]
            if icon.path == "/root/Desktop/Sample.desktop"
                && icon.mime_type == "application/x-desktop"
    ));
}

#[test]
fn background_drag_rubber_bands_multiple_icons_and_removes_overlay_on_release() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("one.txt", FileKind::File),
            directory_entry("two.txt", FileKind::File),
        ],
    );
    desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("rubber band starts");
    let moved = desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: 199,
                    y: 699,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("rubber band moves");
    assert!(desktop.model().selection().len() >= 4);
    let release_target = desktop.items[0].id;
    let patch = envelope(moved[0].clone());
    assert!(patch.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Update { id, text: rice_native_app_protocol::PatchValue::Set(text), .. }
            if *id == widget_id(RUBBER_BAND) && text.starts_with("desktop-selection:")
    )));

    let released = desktop
        .handle_event(
            &event(
                widget_for_item(release_target).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 199,
                    y: 699,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(4),
        )
        .expect("rubber band ends");
    let patch = envelope(released[0].clone());
    assert!(
        patch
            .patches
            .iter()
            .any(|patch| matches!(patch, UiPatch::Remove { id } if *id == widget_id(RUBBER_BAND)))
    );
}

#[test]
fn desktop_items_keep_pointer_up_for_drag_completion() {
    let mut desktop = launch();
    let patch = enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("move-me.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "move-me.txt")
        .expect("desktop item")
        .id;
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create { id, state, .. }
            if *id == widget_for_item(item)
                && state & WidgetState::KEY_NAVIGATION.bits() != 0
    )));
}

#[test]
fn selected_group_moves_to_empty_desktop_cells_when_auto_arrange_is_off() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("one.txt", FileKind::File),
            directory_entry("two.txt", FileKind::File),
        ],
    );
    let ids = ["one.txt", "two.txt"].map(|name| {
        desktop
            .items
            .iter()
            .find(|item| item.label == name)
            .expect("desktop file")
            .id
    });
    for (index, item) in ids.iter().copied().enumerate() {
        let cell = desktop.model().grid().cell_of(item).expect("placed item");
        let rect = desktop.model().grid().config().rect(cell);
        let point = (rect.x + 20, rect.y + 20);
        desktop
            .handle_event(
                &event(
                    widget_for_item(item).get(),
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: point.0,
                        y: point.1,
                        button: 1,
                        modifiers: if index == 0 { 0 } else { 4 },
                    },
                ),
                request(2 + u64::try_from(index * 2).unwrap()),
            )
            .expect("select item");
        desktop
            .handle_event(
                &event(
                    widget_for_item(item).get(),
                    UiEvent::Pointer {
                        kind: PointerEventKind::Up,
                        x: point.0,
                        y: point.1,
                        button: 1,
                        modifiers: if index == 0 { 0 } else { 4 },
                    },
                ),
                request(3 + u64::try_from(index * 2).unwrap()),
            )
            .expect("finish selection click");
    }
    assert_eq!(desktop.model().selection().len(), 2);
    let before = ids.map(|item| desktop.model().grid().cell_of(item).expect("before cell"));
    let anchor_rect = desktop.model().grid().config().rect(before[0]);
    desktop
        .handle_event(
            &event(
                widget_for_item(ids[0]).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: anchor_rect.x + 20,
                    y: anchor_rect.y + 20,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(6),
        )
        .expect("group drag starts");
    let moved = desktop
        .handle_event(
            &event(
                widget_for_item(ids[0]).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x: anchor_rect.x + 320,
                    y: anchor_rect.y + 20,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(7),
        )
        .expect("group drag moves");
    assert_eq!(moved.len(), 2);
    let after = ids.map(|item| desktop.model().grid().cell_of(item).expect("after cell"));
    for index in 0..2 {
        assert_eq!(after[index].column, before[index].column + 3);
        assert_eq!(after[index].row, before[index].row);
    }
    desktop
        .handle_event(
            &event(
                widget_for_item(ids[0]).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: anchor_rect.x + 320,
                    y: anchor_rect.y + 20,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(8),
        )
        .expect("group drag ends");
    assert_eq!(desktop.model().selection().len(), 2);
}

#[test]
fn delete_moves_a_multi_selection_to_trash_as_one_bounded_batch() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("one.txt", FileKind::File),
            directory_entry("two.txt", FileKind::File),
        ],
    );
    let ids = ["one.txt", "two.txt"].map(|name| {
        desktop
            .items
            .iter()
            .find(|item| item.label == name)
            .expect("desktop file")
            .id
    });
    for (index, item) in ids.into_iter().enumerate() {
        desktop
            .handle_event(
                &event(
                    widget_for_item(item).get(),
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 40,
                        y: 40,
                        button: 1,
                        modifiers: if index == 0 { 0 } else { 4 },
                    },
                ),
                request(2 + u64::try_from(index).unwrap()),
            )
            .expect("multi-select file");
    }
    assert_eq!(desktop.model().selection().len(), 2);
    let inspect = desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Key {
                    pressed: true,
                    key_code: 46,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(4),
        )
        .expect("batch trash starts");
    assert!(matches!(
        &inspect[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == desktop_vfs::TRASH_FILES_PATH
    ));
    let first = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("trash preflight");
    assert!(matches!(
        &first[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { .. },
            ..
        })
    ));
    let second = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Done,
        })
        .expect("first move");
    assert!(matches!(
        &second[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { .. },
            ..
        })
    ));
    let refresh = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Done,
        })
        .expect("second move");
    assert!(refresh.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest { operation: VfsOperation::ReadDirectory { path, .. }, .. })
            if path == desktop_vfs::DESKTOP_PATH
    )));
}

#[test]
fn shift_delete_confirms_then_physically_removes_the_desktop_selection() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("one.txt", FileKind::File),
            directory_entry("two.txt", FileKind::File),
        ],
    );
    let ids = ["one.txt", "two.txt"].map(|name| {
        desktop
            .items
            .iter()
            .find(|item| item.label == name)
            .expect("file")
            .id
    });
    for (index, item) in ids.into_iter().enumerate() {
        desktop
            .handle_event(
                &event(
                    widget_for_item(item).get(),
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 40,
                        y: 40,
                        button: 1,
                        modifiers: if index == 0 { 0 } else { 4 },
                    },
                ),
                request(2 + u64::try_from(index).unwrap()),
            )
            .expect("select");
    }
    let shortcut = |desktop: &mut RiceDesktop, request_id| {
        desktop
            .handle_event(
                &event(
                    ICON_GRID,
                    UiEvent::Key {
                        pressed: true,
                        key_code: 119,
                        modifiers: 1,
                        text: None,
                    },
                ),
                request(request_id),
            )
            .expect("Shift+Delete")
    };
    assert!(matches!(
        shortcut(&mut desktop, 4).as_slice(),
        [Message::UiPatch(_)]
    ));
    let first = shortcut(&mut desktop, 5);
    assert!(matches!(
        first.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove { path, recursive: true },
            ..
        })] if path == "/root/Desktop/one.txt"
    ));
    let second = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("first remove");
    assert!(matches!(
        second.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Remove { path, recursive: true },
            ..
        })] if path == "/root/Desktop/two.txt"
    ));
}

#[test]
fn desktop_keyboard_navigates_activates_and_opens_rename_editor() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![
            directory_entry("one.txt", FileKind::File),
            directory_entry("two.txt", FileKind::File),
        ],
    );
    desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Key {
                    pressed: true,
                    key_code: 114,
                    modifiers: 0,
                    text: None,
                },
            ),
            request(2),
        )
        .expect("right selects first item");
    assert_eq!(desktop.model().selection().len(), 1);
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "one.txt")
        .expect("file")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(file).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    x: 20,
                    y: 20,
                    button: 3,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("select file");
    let rename = desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Key {
                    pressed: true,
                    key_code: 65,
                    modifiers: 0,
                    text: Some(String::from(" ")),
                },
            ),
            request(4),
        )
        .expect("space opens rename");
    assert!(format!("{rename:?}").contains("Rename Desktop Item"));
}

#[test]
fn not_found_is_an_explicit_unavailable_empty_desktop() {
    let mut desktop = launch();
    let messages = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("missing"),
            }),
        })
        .expect("not found is represented");
    assert_eq!(desktop.items.len(), 4);
    assert!(matches!(
        desktop.items[3].kind,
        DesktopItemKind::Trash { empty: true }
    ));
    assert_eq!(desktop.model().grid().len(), 4);
    let patch = envelope(messages[0].clone());
    assert!(!patch.patches.is_empty());
}

#[test]
fn paged_enumeration_waits_for_the_final_response_before_replacing_items() {
    let mut desktop = launch();
    let first = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![directory_entry("one", FileKind::File)],
                next_cursor: Some(9),
            },
        })
        .expect("first page");
    assert!(desktop.items.is_empty());
    assert_eq!(first.len(), 1);
    assert!(matches!(
        &first[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { cursor: 9, .. },
            ..
        })
    ));
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("two", FileKind::File)],
    );
    assert_eq!(desktop.items.len(), 6);
}

#[test]
fn rename_is_validated_waits_for_done_and_refreshes_after_success() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("old.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "old.txt")
        .expect("old")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
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
        .expect("item context");
    let editor = desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(3))
        .expect("rename editor");
    let editor = envelope(editor[0].clone());
    assert!(editor.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                text: Some(text),
                ..
            } if *id == widget_id(RENAME_FIELD) && text == "old.txt"
        )
    }));
    let edited = desktop
        .handle_event(
            &event(RENAME_FIELD, UiEvent::ValueChanged(String::from("new.txt"))),
            request(4),
        )
        .expect("rename value");
    let edited = envelope(edited[0].clone());
    assert!(edited.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Update {
                id,
                text: rice_native_app_protocol::PatchValue::Set(text),
                ..
            } if *id == widget_id(RENAME_FIELD) && text == "new.txt"
        )
    }));
    // The native dialog is modal, but the desktop can still be refreshed or
    // receive queued pointer selection patches before its activation arrives.
    // Rename must stay bound to the item for which the editor was opened.
    let trash = desktop
        .items
        .iter()
        .find(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
        .expect("trash")
        .id;
    desktop
        .model
        .execute(DesktopCommand::Click {
            item: trash,
            mode: ClickMode::Replace,
        })
        .expect("selection changes while rename editor is open");
    let rename = desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(5))
        .expect("rename begins");
    assert!(matches!(
        &rename[1],
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::Rename { from, to },
            ..
        }) if *id == request(5)
            && from == "/root/Desktop/old.txt"
            && to == "/root/Desktop/new.txt"
    ));
    assert!(desktop.items.iter().any(|item| item.label == "old.txt"));

    let refresh = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("rename done");
    assert_eq!(refresh.len(), 1);
    assert!(matches!(
        &refresh[0],
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { .. },
            ..
        })
    ));
    assert!(desktop.items.iter().any(|item| item.label == "old.txt"));
    enumerate(
        &mut desktop,
        request(5),
        vec![directory_entry("new.txt", FileKind::File)],
    );
    assert!(desktop.items.iter().any(|item| item.label == "new.txt"));
}

#[test]
fn rename_waits_for_an_active_watch_refresh_instead_of_rejecting_the_click() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("old.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "old.txt")
        .expect("old file")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
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
        .expect("item context");
    desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(3))
        .expect("rename editor");
    let refresh = desktop
        .begin_refresh(request(4), None)
        .expect("watch refresh starts");
    assert!(matches!(refresh.as_slice(), [Message::VfsRequest(_)]));
    desktop
        .handle_event(
            &event(RENAME_FIELD, UiEvent::ValueChanged("new.txt".into())),
            request(5),
        )
        .expect("rename value");

    let queued = desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(6))
        .expect("rename click is queued");
    assert!(matches!(queued.as_slice(), [Message::UiPatch(_)]));
    assert_eq!(desktop.context_kind, ContextKind::Hidden);
    assert!(desktop.pending_rename.is_some());

    let resumed = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Directory {
                entries: vec![directory_entry("old.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("refresh completion starts queued rename");
    assert!(resumed.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::Rename { from, to },
            ..
        }) if *id == request(6)
            && from == "/root/Desktop/old.txt"
            && to == "/root/Desktop/new.txt"
    )));
    assert!(desktop.pending_rename.is_none());
}

#[test]
fn unchanged_rename_closes_without_a_persistent_notice_or_vfs_request() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("same.txt", FileKind::File)],
    );
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "same.txt")
        .expect("desktop item")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(item).get(),
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
        .expect("item context");
    desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(3))
        .expect("rename editor");

    let closed = desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(4))
        .expect("unchanged rename");
    assert!(matches!(closed.as_slice(), [Message::UiPatch(_)]));
    let closed = envelope(closed[0].clone());
    assert!(closed.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Remove { id } if *id == widget_id(RENAME_DIALOG)
    )));
    assert!(!format!("{closed:?}").contains("The new desktop item name is unchanged"));
}

#[test]
fn newly_created_folder_is_selected_and_its_complete_name_is_ready_to_replace() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let stat = desktop
        .handle_event(&event(NEW_FOLDER, UiEvent::Activate), request(2))
        .expect("new folder action");
    assert!(matches!(
        stat.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/New Folder"
    ));
    let create = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("missing"),
            }),
        })
        .expect("default name is available");
    assert!(matches!(
        create.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::CreateDirectory { path },
            ..
        })] if path == "/root/Desktop/New Folder"
    ));
    let refresh = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Done,
        })
        .expect("folder created");
    assert!(matches!(
        refresh.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == desktop_vfs::DESKTOP_PATH
    ));

    let patch = enumerate(
        &mut desktop,
        request(2),
        vec![directory_entry("New Folder", FileKind::Directory)],
    );
    let folder = desktop
        .items
        .iter()
        .find(|item| item.label == "New Folder")
        .expect("created folder");
    assert!(desktop.model().selection().contains(folder.id));
    assert_eq!(desktop.context_kind, ContextKind::Rename);
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Update {
            id,
            state: rice_native_app_protocol::PatchValue::Set(state),
            ..
        } if *id == widget_id(RENAME_FIELD)
            && state & WidgetState::FOCUSED.bits() != 0
            && state & WidgetState::SELECT_ALL_ON_FOCUS.bits() != 0
    )));
}

#[test]
fn newly_created_txt_file_uses_a_text_icon_and_a_real_rename_dialog() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    desktop
        .handle_event(&event(NEW_FILE, UiEvent::Activate), request(2))
        .expect("new file action");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("missing"),
            }),
        })
        .expect("default name available");
    let refresh = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Written(0),
        })
        .expect("file created");
    assert!(matches!(
        refresh.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        })] if path == desktop_vfs::DESKTOP_PATH
    ));

    let patch = enumerate(
        &mut desktop,
        request(2),
        vec![directory_entry("new file.txt", FileKind::File)],
    );
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "new file.txt")
        .expect("created text file");
    assert!(desktop.model().selection().contains(file.id));
    assert_eq!(desktop.context_kind, ContextKind::Rename);
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create { id, kind, text: Some(title), .. }
            if *id == widget_id(RENAME_DIALOG)
                && *kind == WidgetKind::Dialog as u16
                && title == "Rename Desktop Item"
    )));
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create { id, icon: Some(icon), text: Some(label), .. }
            if *id == widget_for_item(file.id)
                && *icon == IconId::Text as u16
                && label == "new file.txt"
    )));
    assert!(patch.patches.iter().any(|entry| matches!(
        entry,
        UiPatch::Create { id, kind, text: Some(value), .. }
            if *id == widget_id(RENAME_FIELD)
                && *kind == WidgetKind::TextField as u16
                && value == "new file.txt"
    )));

    let select_all = desktop
        .handle_event(
            &event(
                RENAME_FIELD,
                UiEvent::Key {
                    pressed: true,
                    key_code: 65,
                    modifiers: 4,
                    text: None,
                },
            ),
            request(3),
        )
        .expect("Ctrl+A stays owned by the rename field");
    assert!(select_all.is_empty());
    assert_eq!(desktop.context_kind, ContextKind::Rename);

    let edited = desktop
        .handle_event(
            &event(
                RENAME_FIELD,
                UiEvent::ValueChanged(String::from("new file.md")),
            ),
            request(3),
        )
        .expect("extension edit");
    assert!(matches!(edited.as_slice(), [Message::UiPatch(_)]));
    let renamed = desktop
        .handle_event(&event(RENAME, UiEvent::Activate), request(4))
        .expect("rename with changed extension");
    assert!(matches!(
        renamed.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { from, to },
            ..
        })] if from == "/root/Desktop/new file.txt"
            && to == "/root/Desktop/new file.md"
    ));
    let closed = envelope(renamed[0].clone());
    assert!(closed.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Remove { id } if *id == widget_id(RENAME_DIALOG)
    )));
}

#[test]
fn create_link_uses_the_shared_real_target_chooser_and_vfs_link_operation() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), Vec::new());
    let opened = desktop
        .handle_event(&event(LINK, UiEvent::Activate), request(2))
        .expect("create-link dialog opens");
    assert!(opened.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|entry| matches!(
            entry,
            UiPatch::Create { kind, text: Some(title), .. }
                if *kind == WidgetKind::Dialog as u16
                    && title == "Create Shortcut"
        ))
    )));
    assert!(opened.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|entry| matches!(
            entry,
            UiPatch::Create { kind, text: Some(label), .. }
                if *kind == WidgetKind::Button as u16 && label == "Find Target…"
        ))
    )));

    let chooser = desktop
        .handle_event(
            &event(desktop_link::LINK_FIND_TARGET, UiEvent::Activate),
            request(2),
        )
        .expect("target chooser opens");
    assert!(chooser.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if path == "/root"
    )));

    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Directory {
                entries: vec![directory_entry("Documents", FileKind::Directory)],
                next_cursor: None,
            },
        })
        .expect("real chooser directory response");
    desktop
        .handle_event(
            &event(
                1_500,
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(3),
        )
        .expect("chooser target selection");
    let selected = desktop
        .handle_event(
            &event(desktop_link::LINK_CHOOSER_ACCEPT, UiEvent::Activate),
            request(4),
        )
        .expect("chooser target acceptance");
    assert!(selected.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|entry| matches!(
            entry,
            UiPatch::Create { kind, text: Some(value), .. }
                if *kind == WidgetKind::TextField as u16 && value == "/root/Documents"
        ))
    )));
    let linked = desktop
        .handle_event(
            &event(desktop_link::LINK_CREATE, UiEvent::Activate),
            request(4),
        )
        .expect("create shortcut");
    assert!(
        linked.iter().any(|message| matches!(
            message,
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::Stat { path },
                ..
            }) if path == "/root/Desktop/Documents Shortcut"
        )),
        "accepted chooser messages: {linked:?}"
    );
    let create = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(4),
            result: VfsResult::Error(VfsError {
                code: VfsErrorCode::NotFound,
                message: String::from("available"),
            }),
        })
        .expect("link destination is available");
    assert!(matches!(
        create.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Link { from, to },
            ..
        })] if from == "/root/Documents"
            && to == "/root/Desktop/Documents Shortcut"
    ));
}

#[test]
fn desktop_link_icon_resolves_and_opens_its_real_directory_target() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Documents Link", FileKind::Symlink)],
    );
    let link = desktop
        .items
        .iter()
        .find(|item| item.label == "Documents Link")
        .expect("desktop link item")
        .clone();
    assert_eq!(link.icon_id, "link");
    let opening = desktop
        .handle_event(
            &event(
                widget_for_item(link.id).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 6,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("link activation");
    assert!(
        opening.iter().any(|message| matches!(
            message,
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadLink { path },
                ..
            }) if path == "/root/Desktop/Documents Link"
        )),
        "link activation messages: {opening:?}"
    );
    let inspect = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Data(b"/root/Documents".to_vec()),
        })
        .expect("link target read");
    assert!(matches!(
        inspect.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Documents"
    ));
    let opened = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(2),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
                modified_millis: 1,
                mode: 0o755,
            }),
        })
        .expect("resolved directory target");
    assert!(matches!(
        opened.as_slice(),
        [Message::Launch { entrypoint, arguments, .. }]
            if entrypoint == "rice-files" && arguments == &[String::from("/root/Documents")]
    ));
}

#[test]
fn desktop_shortcut_keeps_the_target_folder_icon_and_adds_the_shortcut_badge() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Documents Shortcut", FileKind::Symlink)],
    );
    assert!(desktop.owns_link_icon_request(request(1)));
    let inspect = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Data(b"/root/Documents".to_vec()),
        })
        .expect("shortcut target read");
    assert!(matches!(
        inspect.as_slice(),
        [Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Documents"
    ));
    let rendered = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
                modified_millis: 1,
                mode: 0o755,
            }),
        })
        .expect("shortcut target metadata");
    let item = desktop
        .items
        .iter()
        .find(|item| item.label == "Documents Shortcut")
        .expect("shortcut item");
    assert_eq!(item.icon_id, "shortcut:folder:/root/Documents");
    assert!(rendered.iter().any(|message| matches!(
        message,
        Message::UiPatch(patch) if patch.patches.iter().any(|entry| matches!(
            entry,
            UiPatch::Create { id, icon: Some(icon), .. }
                if *id == widget_for_item(item.id)
                    && *icon == IconId::ShortcutDocuments.wire_value()
        ))
    )));
}

#[test]
fn desktop_link_properties_read_and_replace_the_real_target() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Documents Link", FileKind::Symlink)],
    );
    let link = desktop
        .items
        .iter()
        .find(|item| item.label == "Documents Link")
        .expect("desktop link")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(link).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 10,
                    y: 10,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("select link");
    let properties = desktop
        .handle_event(&event(PROPERTIES, UiEvent::Activate), request(3))
        .expect("link properties");
    assert!(properties.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }) if path == "/root/Desktop/Documents Link"
    )));
    let read = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Symlink,
                size: 15,
                modified_millis: 1,
                mode: 0o777,
            }),
        })
        .expect("link metadata");
    assert!(read.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadLink { path },
            ..
        }) if path == "/root/Desktop/Documents Link"
    )));
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Data(b"/root/Documents".to_vec()),
        })
        .expect("link target");
    desktop
        .handle_event(
            &event(
                desktop_properties::LINK_TARGET_FIELD,
                UiEvent::ValueChanged(String::from("/root/Pictures")),
            ),
            request(4),
        )
        .expect("edit target");
    let apply = desktop
        .handle_event(
            &event(desktop_properties::APPLY_LINK_TARGET, UiEvent::Activate),
            request(5),
        )
        .expect("apply target");
    assert!(apply.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReplaceLink { path, target },
            ..
        }) if path == "/root/Desktop/Documents Link" && target == "/root/Pictures"
    )));
    let verify = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("target replaced");
    assert!(verify.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::ReadLink { path },
            ..
        }) if path == "/root/Desktop/Documents Link"
    )));
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Data(b"/root/Pictures".to_vec()),
        })
        .expect("target verification");
}

#[test]
fn desktop_properties_rename_is_vfs_backed_and_verified() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("old.txt", FileKind::File)],
    );
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "old.txt")
        .expect("desktop file")
        .id;
    desktop
        .handle_event(
            &event(
                widget_for_item(file).get(),
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x: 10,
                    y: 10,
                    button: 1,
                    modifiers: 0,
                },
            ),
            request(2),
        )
        .expect("select file");
    desktop
        .handle_event(&event(PROPERTIES, UiEvent::Activate), request(3))
        .expect("file properties");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(3),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 12,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("file metadata");
    desktop
        .handle_event(
            &event(
                desktop_properties::NAME_FIELD,
                UiEvent::ValueChanged(String::from("renamed.txt")),
            ),
            request(4),
        )
        .expect("edit property name");
    let apply = desktop
        .handle_event(
            &event(desktop_properties::APPLY_NAME, UiEvent::Activate),
            request(5),
        )
        .expect("apply property name");
    assert!(apply.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Rename { from, to },
            ..
        }) if from == "/root/Desktop/old.txt" && to == "/root/Desktop/renamed.txt"
    )));
    let verify = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Done,
        })
        .expect("rename done");
    assert!(verify.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        }) if path == "/root/Desktop/renamed.txt"
    )));
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(5),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 12,
                modified_millis: 1,
                mode: 0o644,
            }),
        })
        .expect("rename verified");
    assert_eq!(
        desktop
            .items
            .iter()
            .find(|item| item.id == file)
            .map(|item| (item.label.as_str(), item.path.as_deref())),
        Some(("renamed.txt", Some("/root/Desktop/renamed.txt"))),
    );
}

#[test]
fn desktop_txt_files_open_with_the_default_rice_paper_association() {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("new file.txt", FileKind::File)],
    );
    let file = desktop
        .items
        .iter()
        .find(|item| item.label == "new file.txt")
        .expect("text file")
        .id;
    let opening = desktop
        .handle_event(
            &event(
                widget_for_item(file).get(),
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
        .expect("open text file");
    assert!(matches!(
        opening.as_slice(),
        [Message::UiPatch(_), Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationCatalog { mime_type },
            ..
        })] if mime_type == "text/plain"
    ));

    let launching = desktop
        .handle_application_response(
            &DataProviderResponse {
                request: request(2),
                result: DataProviderResult::Applications {
                    schema_version: APPLICATION_CATALOG_VERSION,
                    mime_type: String::from("text/plain"),
                    entries: vec![InstalledApplication {
                        id: String::from("native:rice-paper"),
                        name: String::from("Rice Paper"),
                        icon: String::from("rice-paper"),
                        mime_types: vec![String::from("text/plain")],
                        matches_requested_type: true,
                        is_default: true,
                        is_native: true,
                    }],
                },
            },
            request(3),
        )
        .expect("text application catalog");
    assert!(matches!(
        launching.as_slice(),
        [Message::DataProviderRequest(DataProviderRequest {
            operation: DataProviderOperation::ApplicationLaunch {
                application_id,
                mime_type,
                paths,
                ..
            },
            ..
        })] if application_id == "native:rice-paper"
            && mime_type == "text/plain"
            && paths == &[String::from("/root/Desktop/new file.txt")]
    ));
}

#[path = "desktop_action_contract_tests.rs"]
mod action_contract;
#[path = "desktop_launcher_tests.rs"]
mod launcher;
#[path = "desktop_persistence_tests.rs"]
mod persistence;
#[path = "desktop_safety_tests.rs"]
mod safety;
#[path = "desktop_settings_tests.rs"]
mod settings;

#[test]
fn desktop_open_with_launches_chooser_instead_of_default_application() {
    let mut desktop = launch();
    enumerate(&mut desktop, request(1), vec![directory_entry("movie.mp4", FileKind::File)]);
    let file = desktop.items.iter().find(|item| item.label == "movie.mp4").unwrap().id;
    desktop.handle_event(&event(widget_for_item(file).get(), UiEvent::Pointer {
        kind: PointerEventKind::Down, x: 40, y: 40, button: 1, modifiers: 0,
    }), request(2)).unwrap();
    let messages = desktop.handle_event(&event(OPEN_WITH, UiEvent::Activate), request(3)).unwrap();
    assert!(matches!(messages.as_slice(), [Message::Launch {entrypoint, arguments, ..}]
        if entrypoint == "rice-files" && arguments == &[String::from("--select=/root/Desktop/movie.mp4"), String::from("--open-with")]));
}

#[test]
fn unrecognized_desktop_file_opens_the_application_chooser() {
    let mut desktop = launch();
    desktop.begin_file_activation("/root/Desktop/unknown.probe".into(), request(10)).unwrap();
    let messages = desktop.handle_application_response(&DataProviderResponse {
        request: request(10),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: "application/octet-stream".into(),
            entries: Vec::new(),
        },
    }, request(11)).unwrap();
    assert!(matches!(messages.as_slice(), [Message::Launch { entrypoint, arguments, .. }]
        if entrypoint == "rice-files" && arguments == &["--select=/root/Desktop/unknown.probe", "--open-with"]));
}
