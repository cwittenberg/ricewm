use std::collections::BTreeMap;

use rice_desktop::{
    Cell, ClickMode, Color, ContextMenuPolicy, DesktopCommand, DesktopIntent, DesktopItem,
    DesktopItemId, DesktopItemKind, DesktopModel, EventPath, GridConfig, ItemAction, MenuEntry,
    MenuRequest, ModelError, MonitorEvent, MountId, MountRecord, Wallpaper, WallpaperSettings,
    WorkspaceId,
};

fn item(id: u64, kind: DesktopItemKind, read_only: bool) -> DesktopItem {
    DesktopItem {
        id: DesktopItemId(id),
        label: format!("Item {id}"),
        icon_id: String::from("document"),
        path: Some(format!("/desktop/{id}")),
        kind,
        read_only,
        executable: false,
        available: true,
    }
}

fn grid() -> GridConfig {
    GridConfig {
        columns: 3,
        rows: 3,
        ..GridConfig::default()
    }
}

fn model() -> DesktopModel {
    DesktopModel::new(
        grid(),
        WallpaperSettings::new(Wallpaper::Solid(Color::rgb(10, 20, 30))),
        WorkspaceId::new("main"),
    )
}

fn enabled(entries: &[MenuEntry], action: ItemAction) -> Option<bool> {
    entries.iter().find_map(|entry| match entry {
        MenuEntry::Action {
            action: candidate,
            enabled,
            ..
        } if *candidate == action => Some(*enabled),
        _ => None,
    })
}

#[test]
fn semantic_item_kinds_have_correct_primary_behavior() {
    let application = item(
        1,
        DesktopItemKind::Application {
            desktop_id: String::from("leafpad.desktop"),
        },
        false,
    );
    assert_eq!(application.primary_action(), Some(ItemAction::Launch));
    assert!(application.is_renameable());

    let home = item(2, DesktopItemKind::Home, true);
    assert_eq!(home.primary_action(), Some(ItemAction::Open));
    assert!(!home.is_renameable());

    let mut unavailable_mount = item(
        3,
        DesktopItemKind::Mount {
            mount_id: String::from("usb"),
        },
        false,
    );
    unavailable_mount.available = false;
    assert_eq!(unavailable_mount.primary_action(), Some(ItemAction::Mount));
}

#[test]
fn menu_policy_distinguishes_background_single_multiple_and_read_only() {
    let policy = ContextMenuPolicy;
    let background = policy.entries(MenuRequest::Background {
        desktop_read_only: true,
        clipboard_has_files: true,
    });
    assert_eq!(enabled(&background, ItemAction::NewFolder), Some(false));
    assert_eq!(enabled(&background, ItemAction::Paste), Some(false));
    assert_eq!(
        enabled(&background, ItemAction::DesktopSettings),
        Some(true)
    );

    let writable = item(1, DesktopItemKind::File, false);
    let single = policy.entries(MenuRequest::Selection {
        items: &[&writable],
        clipboard_has_files: false,
    });
    assert_eq!(enabled(&single, ItemAction::Rename), Some(true));
    assert_eq!(enabled(&single, ItemAction::OpenWith), Some(true));
    assert_eq!(enabled(&single, ItemAction::DeletePermanently), None);

    let locked = item(2, DesktopItemKind::Folder, true);
    let multiple = policy.entries(MenuRequest::Selection {
        items: &[&writable, &locked],
        clipboard_has_files: true,
    });
    assert_eq!(enabled(&multiple, ItemAction::Cut), Some(false));
    assert_eq!(enabled(&multiple, ItemAction::Copy), Some(true));
    assert_eq!(enabled(&multiple, ItemAction::MoveToTrash), Some(false));
    assert_eq!(enabled(&multiple, ItemAction::DeletePermanently), None);
}

#[test]
fn commands_produce_adapter_intents_without_performing_io() {
    let mut model = model();
    let file = item(1, DesktopItemKind::File, false);
    model
        .execute(DesktopCommand::ReplaceItems {
            items: vec![file],
            persisted: BTreeMap::from([(DesktopItemId(1), Cell::new(2, 1))]),
        })
        .expect("replace");
    assert_eq!(
        model.grid().cell_of(DesktopItemId(1)),
        Some(Cell::new(2, 1))
    );

    let update = model
        .execute(DesktopCommand::Activate(DesktopItemId(1)))
        .expect("activate");
    assert_eq!(
        update.intents,
        vec![DesktopIntent::Perform {
            action: ItemAction::Open,
            items: vec![DesktopItemId(1)]
        }]
    );

    model
        .execute(DesktopCommand::Click {
            item: DesktopItemId(1),
            mode: ClickMode::Replace,
        })
        .expect("click");
    let update = model.execute(DesktopCommand::BeginRename).expect("rename");
    assert_eq!(
        update.intents,
        vec![DesktopIntent::Rename {
            item: DesktopItemId(1),
            current_label: String::from("Item 1")
        }]
    );
}

#[test]
fn rename_rejects_special_or_multiple_selection() {
    let mut model = model();
    model
        .execute(DesktopCommand::ReplaceItems {
            items: vec![
                item(1, DesktopItemKind::Home, true),
                item(2, DesktopItemKind::File, false),
            ],
            persisted: BTreeMap::new(),
        })
        .expect("replace");
    model
        .execute(DesktopCommand::Click {
            item: DesktopItemId(1),
            mode: ClickMode::Replace,
        })
        .expect("click");
    assert_eq!(
        model.execute(DesktopCommand::BeginRename),
        Err(ModelError::RenameUnavailable)
    );
    model
        .execute(DesktopCommand::SelectAll)
        .expect("select all");
    assert_eq!(
        model.execute(DesktopCommand::BeginRename),
        Err(ModelError::RenameUnavailable)
    );
}

#[test]
fn resize_and_manual_move_emit_persistence_commands() {
    let mut model = model();
    model
        .execute(DesktopCommand::ReplaceItems {
            items: vec![
                item(1, DesktopItemKind::File, false),
                item(2, DesktopItemKind::Folder, false),
            ],
            persisted: BTreeMap::new(),
        })
        .expect("replace");
    let update = model
        .execute(DesktopCommand::MoveIcon {
            item: DesktopItemId(1),
            cell: Cell::new(2, 2),
        })
        .expect("move");
    assert!(matches!(
        update.intents.as_slice(),
        [DesktopIntent::PersistPositions(positions)]
            if positions.contains(&(DesktopItemId(1), Cell::new(2, 2)))
    ));

    let update = model
        .execute(DesktopCommand::ResizeGrid(GridConfig {
            columns: 1,
            rows: 1,
            ..grid()
        }))
        .expect("resize");
    assert!(matches!(
        update.intents.as_slice(),
        [DesktopIntent::PersistPositions(_)]
    ));
}

#[test]
fn monitor_and_mount_batches_translate_to_safe_adapter_intents() {
    let mut model = model();
    model
        .execute(DesktopCommand::QueueMonitorEvent(
            MonitorEvent::MetadataChanged(EventPath::new("/desktop/a")),
        ))
        .expect("metadata");
    model
        .execute(DesktopCommand::QueueMonitorEvent(
            MonitorEvent::MountArrived(MountRecord {
                id: MountId::new("usb"),
                label: String::from("USB"),
                root: EventPath::new("/media/usb"),
                read_only: true,
            }),
        ))
        .expect("mount arrival");
    let update = model
        .execute(DesktopCommand::FlushMonitorEvents)
        .expect("flush");
    assert_eq!(
        update.intents,
        vec![
            DesktopIntent::RefreshPath(String::from("/desktop/a")),
            DesktopIntent::MountArrived {
                mount_id: String::from("usb"),
                root: String::from("/media/usb"),
                read_only: true
            }
        ]
    );

    model
        .execute(DesktopCommand::QueueMonitorEvent(
            MonitorEvent::MountRemoved(MountId::new("usb")),
        ))
        .expect("mount removal");
    let update = model
        .execute(DesktopCommand::FlushMonitorEvents)
        .expect("flush");
    assert_eq!(
        update.intents,
        vec![DesktopIntent::MountRemoved {
            mount_id: String::from("usb"),
            cancel_operations: true
        }]
    );
}

#[test]
fn workspace_commands_redraw_current_workspace_and_keep_overrides() {
    let mut model = model();
    let alternate = Wallpaper::Solid(Color::rgb(40, 50, 60));
    model
        .execute(DesktopCommand::SetWorkspaceWallpaper {
            workspace: WorkspaceId::new("work"),
            wallpaper: alternate.clone(),
        })
        .expect("set wallpaper");
    let update = model
        .execute(DesktopCommand::SwitchWorkspace(WorkspaceId::new("work")))
        .expect("switch");
    assert_eq!(
        update.intents,
        vec![DesktopIntent::RedrawWallpaper {
            workspace: WorkspaceId::new("work")
        }]
    );
    assert_eq!(
        model.wallpaper().for_workspace(&WorkspaceId::new("work")),
        &alternate
    );
}

#[test]
fn duplicate_ids_are_rejected_atomically() {
    let mut model = model();
    let result = model.execute(DesktopCommand::ReplaceItems {
        items: vec![
            item(1, DesktopItemKind::File, false),
            item(1, DesktopItemKind::Folder, false),
        ],
        persisted: BTreeMap::new(),
    });
    assert_eq!(result, Err(ModelError::DuplicateItem(DesktopItemId(1))));
    assert!(model.grid().is_empty());
}
