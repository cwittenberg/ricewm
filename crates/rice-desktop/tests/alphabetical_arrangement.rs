use std::collections::BTreeMap;

use rice_desktop::{
    DesktopCommand, DesktopItem, DesktopItemId, DesktopItemKind, DesktopModel, GridConfig,
    WallpaperSettings, WorkspaceId,
};

fn item(id: u64, label: &str, kind: DesktopItemKind) -> DesktopItem {
    DesktopItem {
        id: DesktopItemId(id),
        label: label.to_owned(),
        icon_id: String::from("folder"),
        path: None,
        kind,
        read_only: false,
        executable: false,
        available: true,
    }
}

#[test]
fn auto_arrange_uses_names_instead_of_virtual_item_ids() {
    let mut model = DesktopModel::new(
        GridConfig {
            columns: 2,
            rows: 2,
            ..GridConfig::default()
        },
        WallpaperSettings::default(),
        WorkspaceId::new("main"),
    );
    model
        .execute(DesktopCommand::ReplaceItems {
            items: vec![
                item(900, "Trash", DesktopItemKind::Trash { empty: true }),
                item(2, "zebra.txt", DesktopItemKind::File),
                item(1, "Alpha", DesktopItemKind::Folder),
            ],
            persisted: BTreeMap::new(),
        })
        .expect("replace items");
    model
        .execute(DesktopCommand::ArrangeIcons)
        .expect("arrange icons");

    assert_eq!(
        model.grid().ordered_items(),
        vec![DesktopItemId(1), DesktopItemId(900), DesktopItemId(2)]
    );
}
