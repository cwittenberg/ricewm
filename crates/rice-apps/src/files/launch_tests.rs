use super::{
    CONTENT, GuestPath, Message, PLACE_DESKTOP, PLACE_HOME, PLACE_MOUNTS, PLACE_TRASH, ROOT,
    RiceFiles, VfsOperation, VfsRequest, files_menu,
    files_view::{PLACES, PLACES_LIST, PREVIEW_PANEL, STATUS_PANEL, widget_id},
    tests::{app, request},
};
use rice_native_app_protocol::{
    DataProviderResponse, DataProviderResult, DirectoryEntry, FileKind, FileMetadata, HostEvent,
    HostEventKind, MountCapabilities, MountEntry, MountState, UiPatch, VfsResponse, VfsResult,
};
use rice_ui::{IconId, MenuText, WidgetKind, WidgetState};

#[test]
fn launch_emits_real_shell_and_bounded_vfs_enumeration() {
    let (actor, messages) = RiceFiles::launch(app(), request(1)).expect("launch");
    assert_eq!(actor.model().windows().len(), 1);
    assert_eq!(
        actor.location(),
        &GuestPath::absolute("/root").expect("home")
    );
    assert_eq!(messages.len(), 6);
    let Message::UiPatch(ui) = &messages[0] else {
        panic!("first output is UI");
    };
    for kind in [
        WidgetKind::MenuBar,
        WidgetKind::ToolBar,
        WidgetKind::TextField,
        WidgetKind::List,
        WidgetKind::Container,
    ] {
        assert!(ui.patches.iter().any(
            |patch| matches!(patch, UiPatch::Create { kind: value, .. } if *value == kind as u16)
        ));
    }
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                parent: Some(parent),
                kind,
                ..
            } if *id == widget_id(PLACES)
                && *parent == widget_id(CONTENT)
                && *kind == WidgetKind::List as u16
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, text: Some(text), .. }
                if *id == widget_id(PLACE_DESKTOP) && text == "Desktop"
        )
    }));
    for (place, expected) in [
        (PLACE_DESKTOP, IconId::Desktop),
        (PLACE_TRASH, IconId::Trash),
        (PLACE_MOUNTS, IconId::NetworkMounts),
    ] {
        assert!(ui.patches.iter().any(|patch| {
            matches!(
                patch,
                UiPatch::Create { id, icon: Some(icon), .. }
                    if *id == widget_id(place) && *icon == expected.wire_value()
            )
        }));
    }
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, state, .. }
                if *id == widget_id(PLACE_MOUNTS)
                    && *state & WidgetState::ENABLED.bits() == 0
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { kind, text: Some(text), .. }
                if *kind == WidgetKind::Tooltip as u16
                    && text == "No folders were mounted"
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { kind, text: Some(text), .. }
                if *kind == WidgetKind::Row as u16 && text == "panel:separator"
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, state, .. }
                if *id == widget_id(PLACE_HOME)
                    && *state & WidgetState::SELECTED.bits() != 0
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                parent: Some(parent),
                kind,
                ..
            } if *id == widget_id(PLACES_LIST)
                && *parent == widget_id(PLACES)
                && *kind == WidgetKind::List as u16
        )
    }));
    assert!(ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                id,
                parent: Some(parent),
                kind,
                text: Some(text),
                ..
            } if *id == widget_id(STATUS_PANEL)
                && *parent == widget_id(ROOT)
                && *kind == WidgetKind::Container as u16
                && text == "panel:status-bar:files-status"
        )
    }));
    assert!(!ui.patches.iter().any(|patch| {
        matches!(patch, UiPatch::Create { id, .. } if *id == widget_id(PREVIEW_PANEL))
    }));
    assert_menu_contract(ui);
    assert!(matches!(
        &messages[1],
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::ReadDirectory { path, limit: 128, .. },
            ..
        }) if *id == request(1) && path == "/root"
    ));
    assert!(matches!(
        &messages[2],
        Message::DataProviderRequest(rice_native_app_protocol::DataProviderRequest {
            request: id,
            operation: rice_native_app_protocol::DataProviderOperation::MountList,
            ..
        }) if *id == request(1)
    ));
    assert!(matches!(
        &messages[3],
        Message::SettingsRequest(rice_native_app_protocol::SettingsRequest {
            operation: rice_native_app_protocol::SettingsOperation::Load,
            ..
        })
    ));
    assert!(matches!(
        &messages[4],
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::SetTimer {
                interval_millis: 1_000,
                enabled: true,
                ..
            },
            ..
        })
    ));
    assert!(matches!(
        &messages[5],
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::RiceBusSubscribe {
                topic,
                filters,
            },
            ..
        }) if topic == super::VFS_CHANGED_TOPIC && filters == &[String::from("/root")]
    ));
}

#[test]
fn redmond_my_computer_renders_only_real_vfs_entries_and_opens_them() {
    let (mut actor, _) = RiceFiles::launch_at(
        app(),
        &[String::from("--theme-profile=redmond-98")],
        request(1),
    )
    .expect("Redmond RiceFiles");
    let messages = actor
        .handle_vfs_response(&super::tests::directory_response(1))
        .expect("real root directory");
    let update = messages
        .iter()
        .find_map(|message| match message {
            Message::UiPatch(update) => Some(update),
            _ => None,
        })
        .expect("real directory emits a UI update");
    assert!(
        messages.iter().any(|message| matches!(
            message,
            Message::VfsRequest(VfsRequest {
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            }) if path == "/root/Folder"
        )),
        "the tree discovers disclosure children from the real VFS"
    );
    for expected in ["Folder", "file2.txt", "file10.txt"] {
        assert!(update.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { text: Some(text), .. } if text == expected
        )));
    }
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), icon: Some(icon), .. }
            if text == "My Computer" && *icon == rice_ui::IconId::Computer.wire_value()
    )), "My Computer must use the computer identity, not Desktop");
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), icon: Some(icon), .. }
            if text == "Desktop" && *icon == rice_ui::IconId::Desktop.wire_value()
    )), "the Desktop tree node must retain its own identity");
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. }
            if text == "split:220:redmond98-my-computer"
    )));
    assert!(update.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { text: Some(text), .. }
            if text == "Select an item to"
    )));
    for fabricated in [
        "3½ Floppy (A:)",
        "Local Disk (C:)",
        "Printers",
        "Control Panel",
        "Scheduled Tasks",
    ] {
        assert!(!update.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { text: Some(text), .. } if text == fabricated
        )));
    }
    assert_eq!(
        actor
            .runtime_for_test()
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        ["Folder", "file2.txt", "file10.txt"]
    );

    actor
        .handle_event(
            &super::tests::event(super::ENTRY_BASE, super::UiEvent::Activate),
            request(2),
        )
        .expect("select real folder");
    let opened = actor
        .handle_event(
            &super::tests::event(files_menu::OPEN, super::UiEvent::Activate),
            request(3),
        )
        .expect("open real folder");
    assert!(opened.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if *id == request(3) && path == "/root/Folder"
    )));
}

#[test]
fn launch_selection_highlights_the_file_requested_by_show_in_files() {
    let (mut actor, _) = RiceFiles::launch_at(
        app(),
        &[String::from("--select=/root/project/app.mjs")],
        request(41),
    )
    .expect("launch");
    assert_eq!(actor.location().as_str(), "/root/project");
    actor
        .handle_vfs_response(&VfsResponse {
            request: request(41),
            result: VfsResult::Directory {
                entries: vec![DirectoryEntry {
                    name: String::from("app.mjs"),
                    metadata: FileMetadata {
                        kind: FileKind::File,
                        size: 12,
                        modified_millis: 0,
                        mode: 0o644,
                    },
                    mime_type: Some(String::from("application/javascript")),
                }],
                next_cursor: None,
            },
        })
        .expect("directory response");
    assert_eq!(
        actor
            .pane()
            .expect("pane")
            .selection()
            .map(GuestPath::as_str)
            .collect::<Vec<_>>(),
        ["/root/project/app.mjs"]
    );
}

#[test]
fn connected_local_mount_enables_the_local_files_place() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    let messages = actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(1),
                result: DataProviderResult::Mounts {
                    entries: vec![MountEntry {
                        icon: None,
                        id: String::from("documents"),
                        label: String::from("Documents"),
                        path: String::from("/mnt/local/Documents"),
                        state: MountState::Connected,
                        capabilities: MountCapabilities::OPEN,
                    }],
                    can_connect_local: true,
                },
            },
            request(2),
        )
        .expect("mount catalog applies");
    let [Message::UiPatch(update)] = messages.as_slice() else {
        panic!("mount catalog refreshes the UI");
    };
    assert!(update.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { id, state, .. }
                if *id == widget_id(PLACE_MOUNTS)
                    && *state & WidgetState::ENABLED.bits() != 0
        )
    }));
    assert!(!update.patches.iter().any(|patch| {
        matches!(patch, UiPatch::Create { text: Some(text), .. } if text == "No folders were mounted")
    }));
}

#[test]
fn mount_timer_refreshes_places_without_reopening_ricefiles() {
    let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(1),
                result: DataProviderResult::Mounts {
                    entries: Vec::new(),
                    can_connect_local: true,
                },
            },
            request(2),
        )
        .expect("initial catalog");
    let messages = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::Timer {
                    token: super::MOUNT_REFRESH_TIMER,
                },
            },
            request(3),
        )
        .expect("timer");
    assert!(matches!(
        messages.as_slice(),
        [Message::DataProviderRequest(rice_native_app_protocol::DataProviderRequest {
            request: id,
            operation: rice_native_app_protocol::DataProviderOperation::MountList,
            ..
        })] if *id == request(3)
    ));
}

#[test]
fn reload_syncs_the_containing_mount_before_reading_its_directory() {
    let mount_path = "/mnt/local/Documents";
    let (mut actor, _) = RiceFiles::launch_at(app(), &[format!("{mount_path}/work")], request(1))
        .expect("launch mounted directory");
    actor
        .handle_vfs_response(&super::tests::directory_response(1))
        .expect("initial directory");
    actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(1),
                result: DataProviderResult::Mounts {
                    entries: vec![MountEntry {
                        icon: None,
                        id: String::from("documents"),
                        label: String::from("Documents"),
                        path: String::from(mount_path),
                        state: MountState::Connected,
                        capabilities: MountCapabilities::OPEN | MountCapabilities::SYNC,
                    }],
                    can_connect_local: true,
                },
            },
            request(2),
        )
        .expect("mount catalog");

    let messages = actor
        .handle_event(
            &super::tests::event(super::RELOAD, super::UiEvent::Activate),
            request(2),
        )
        .expect("reload");
    assert!(matches!(
        messages.as_slice(),
        [Message::DataProviderRequest(rice_native_app_protocol::DataProviderRequest {
            request: id,
            operation: rice_native_app_protocol::DataProviderOperation::MountSync { mount_id },
            ..
        })] if *id == request(2) && mount_id == "documents"
    ));

    let messages = actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(2),
                result: DataProviderResult::Mounts {
                    entries: vec![MountEntry {
                        icon: None,
                        id: String::from("documents"),
                        label: String::from("Documents"),
                        path: String::from(mount_path),
                        state: MountState::Connected,
                        capabilities: MountCapabilities::OPEN | MountCapabilities::SYNC,
                    }],
                    can_connect_local: true,
                },
            },
            request(3),
        )
        .expect("sync response");
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if *id == request(3) && path == "/mnt/local/Documents/work"
    )));
}

#[test]
fn rice_bus_vfs_event_reloads_the_matching_open_directory() {
    let (mut actor, _) =
        RiceFiles::launch_at(app(), &[String::from("/mnt/local/Documents")], request(1))
            .expect("launch mounted directory");
    actor
        .handle_vfs_response(&super::tests::directory_response(1))
        .expect("initial directory");

    let messages = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::RiceBus {
                    topic: String::from(super::VFS_CHANGED_TOPIC),
                    payload: String::from(
                        r#"{"filterIndex":0,"filter":"/mnt/local/Documents","changedPaths":["/mnt/local/Documents/report.txt"]}"#,
                    ),
                },
            },
            request(2),
        )
        .expect("mounted tree watch");
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if *id == request(2) && path == "/mnt/local/Documents"
    )));
}

#[test]
fn rice_bus_vfs_event_is_deferred_while_the_visible_view_is_enumerating() {
    let (mut actor, _) =
        RiceFiles::launch_at(app(), &[String::from("/root/work")], request(1)).expect("launch");
    let deferred = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::RiceBus {
                    topic: String::from(super::VFS_CHANGED_TOPIC),
                    payload: String::from(
                        r#"{"filterIndex":0,"filter":"/root/work","changedPaths":["/root/work/new.txt"]}"#,
                    ),
                },
            },
            request(2),
        )
        .expect("RiceBus event");
    assert!(matches!(
        deferred.as_slice(),
        [Message::HostRequest(
            rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::SetTimer {
                    token: super::RICE_BUS_DEFERRED_REFRESH_TIMER,
                    interval_millis: 100,
                    enabled: true,
                },
                ..
            }
        )]
    ));

    actor
        .handle_vfs_response(&super::tests::directory_response(1))
        .expect("initial directory");
    let resumed = actor
        .handle_host_event(
            &HostEvent {
                app: app(),
                event: HostEventKind::Timer {
                    token: super::RICE_BUS_DEFERRED_REFRESH_TIMER,
                },
            },
            request(3),
        )
        .expect("deferred refresh");
    assert!(resumed.iter().any(|message| matches!(
        message,
        Message::VfsRequest(VfsRequest {
            request: id,
            operation: VfsOperation::ReadDirectory { path, .. },
            ..
        }) if *id == request(3) && path == "/root/work"
    )));
}

fn assert_menu_contract(ui: &rice_native_app_protocol::UiPatchEnvelope) {
    for label in ["Preview Pane", "Dual Pane", "Move to Trash"] {
        assert!(ui.patches.iter().any(|patch| {
            matches!(
                patch,
                UiPatch::Create { text: Some(text), .. }
                    if MenuText::parse(text).label == label
            )
        }));
    }
    assert!(!ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create { text: Some(text), .. }
                if text.contains("VFS:") || text.contains("Split 50%")
        )
    }));
    for kind in [WidgetKind::CheckBox, WidgetKind::RadioButton] {
        assert!(ui.patches.iter().any(
            |patch| matches!(patch, UiPatch::Create { kind: value, .. } if *value == kind as u16)
        ));
    }
    let popup_ids = [
        files_menu::FILE_MENU,
        files_menu::EDIT_MENU,
        files_menu::VIEW_MENU,
        files_menu::HELP_MENU,
        files_menu::CONTEXT_MENU,
    ];
    assert!(!ui.patches.iter().any(|patch| {
        matches!(
            patch,
            UiPatch::Create {
                kind,
                parent: Some(parent),
                ..
            } if *kind == WidgetKind::TextField as u16 && popup_ids.contains(&parent.get())
        )
    }));
}

#[test]
fn mounted_places_follow_provider_root_and_keep_browser_mount_root() {
    for (path, expected) in [
        ("/mnt/local/Documents", "/mnt/local"),
        ("/mnt/rice-1000-8-0", "/mnt"),
    ] {
        let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
        actor.apply_mount_catalog(&[MountEntry {
            icon: None,
            id: String::from("volume"),
            label: String::from("Documents"),
            path: String::from(path),
            state: MountState::Connected,
            capabilities: MountCapabilities::OPEN,
        }]);
        assert!(actor.local_files_available);
        assert_eq!(actor.local_mount_root(), expected);
        assert_eq!(
            actor
                .folder_tree_path_for_target(PLACE_MOUNTS)
                .unwrap()
                .as_str(),
            expected
        );
    }
}

#[test]
fn launch_open_with_selects_then_requests_real_application_catalog() {
    let (mut actor, _) = RiceFiles::launch_at(app(),
        &[String::from("--select=/root/Videos/movie.mp4"), String::from("--open-with")],
        request(41)).expect("launch");
    let messages = actor.handle_vfs_response_with_request(&VfsResponse {
        request: request(41),
        result: VfsResult::Directory {entries: vec![DirectoryEntry {
            name: String::from("movie.mp4"),
            metadata: FileMetadata {kind: FileKind::File, size: 12, modified_millis: 0, mode: 0o644},
            mime_type: Some(String::from("video/mp4")),
        }], next_cursor: None},
    }, request(42)).expect("directory response");
    assert!(actor.open_with.is_some());
    assert!(messages.iter().any(|message| matches!(message,
        Message::DataProviderRequest(rice_native_app_protocol::DataProviderRequest {
            request: id, operation: rice_native_app_protocol::DataProviderOperation::ApplicationCatalog {mime_type}, ..
        }) if *id == request(42) && mime_type == "video/mp4")));
    assert!(!actor.launch_open_with);
    // A delayed mount lookup must not steal the application-catalog response.
    assert_eq!(actor.pending_mount_catalog, Some(request(41)));
    actor.handle_data_response(&DataProviderResponse {
        request: request(42), result: DataProviderResult::Applications {
            schema_version: rice_native_app_protocol::APPLICATION_CATALOG_VERSION,
            mime_type: String::from("video/mp4"), entries: Vec::new(),
        },
    }, request(43)).expect("application reply before mount reply");
    actor.handle_data_response(&DataProviderResponse {
        request: request(41), result: DataProviderResult::Unavailable {reason: String::from("No mounts")},
    }, request(44)).expect("late mount reply");
    assert!(actor.open_with.is_some());
}
