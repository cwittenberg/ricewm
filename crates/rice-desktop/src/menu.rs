//! Context-menu policy independent of renderer widgets.

use crate::{DesktopItem, DesktopItemKind, ItemAction};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MenuEntry {
    Action {
        label: &'static str,
        action: ItemAction,
        enabled: bool,
    },
    Separator,
}

impl MenuEntry {
    #[must_use]
    pub const fn action(label: &'static str, action: ItemAction) -> Self {
        Self::Action {
            label,
            action,
            enabled: true,
        }
    }

    #[must_use]
    pub const fn disabled(label: &'static str, action: ItemAction) -> Self {
        Self::Action {
            label,
            action,
            enabled: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MenuRequest<'a> {
    Background {
        desktop_read_only: bool,
        clipboard_has_files: bool,
    },
    Selection {
        items: &'a [&'a DesktopItem],
        clipboard_has_files: bool,
    },
}

/// Policy object implementing the Strategy seam used by the desktop aggregate.
#[derive(Clone, Copy, Debug, Default)]
pub struct ContextMenuPolicy;

impl ContextMenuPolicy {
    #[must_use]
    pub fn entries(self, request: MenuRequest<'_>) -> Vec<MenuEntry> {
        match request {
            MenuRequest::Background {
                desktop_read_only,
                clipboard_has_files,
            } => background_entries(desktop_read_only, clipboard_has_files),
            MenuRequest::Selection {
                items,
                clipboard_has_files,
            } => selection_entries(items, clipboard_has_files),
        }
    }
}

fn background_entries(read_only: bool, clipboard: bool) -> Vec<MenuEntry> {
    vec![
        enabled_if("New Folder", ItemAction::NewFolder, !read_only),
        enabled_if("New File", ItemAction::NewFile, !read_only),
        enabled_if("Paste", ItemAction::Paste, !read_only && clipboard),
        MenuEntry::Separator,
        MenuEntry::action("Select All", ItemAction::SelectAll),
        MenuEntry::action("Arrange Icons", ItemAction::ArrangeIcons),
        MenuEntry::Separator,
        MenuEntry::action("Desktop Settings", ItemAction::DesktopSettings),
    ]
}

fn selection_entries(items: &[&DesktopItem], clipboard: bool) -> Vec<MenuEntry> {
    if items.len() == 1 {
        return single_item_entries(items[0], clipboard);
    }
    let writable = items.iter().all(|item| item.available && !item.read_only);
    let deletable = items.iter().all(|item| item.can_delete());
    vec![
        MenuEntry::action("Open", ItemAction::Open),
        MenuEntry::Separator,
        enabled_if("Cut", ItemAction::Cut, writable),
        MenuEntry::action("Copy", ItemAction::Copy),
        enabled_if("Paste Into", ItemAction::Paste, writable && clipboard),
        enabled_if("Create Shortcut", ItemAction::Link, writable),
        MenuEntry::Separator,
        enabled_if("Move to Trash", ItemAction::MoveToTrash, deletable),
        MenuEntry::Separator,
        MenuEntry::action("Properties", ItemAction::Properties),
    ]
}

fn single_item_entries(item: &DesktopItem, clipboard: bool) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    if let Some(action) = item.primary_action() {
        entries.push(MenuEntry::action(action_label(action), action));
    }

    if matches!(item.kind, DesktopItemKind::File) {
        entries.push(MenuEntry::action("Open With…", ItemAction::OpenWith));
    }
    if matches!(item.kind, DesktopItemKind::Mount { .. }) && item.available {
        entries.push(MenuEntry::action("Unmount", ItemAction::Unmount));
        entries.push(MenuEntry::action("Eject", ItemAction::Eject));
    }
    if matches!(item.kind, DesktopItemKind::Trash { empty: false }) {
        entries.push(MenuEntry::action("Empty Trash", ItemAction::EmptyTrash));
    }

    entries.push(MenuEntry::Separator);
    entries.push(enabled_if("Cut", ItemAction::Cut, !item.read_only));
    entries.push(MenuEntry::action("Copy", ItemAction::Copy));
    if matches!(item.kind, DesktopItemKind::Folder) {
        entries.push(enabled_if(
            "Paste Into",
            ItemAction::Paste,
            !item.read_only && clipboard,
        ));
    }
    entries.push(enabled_if(
        "Create Shortcut",
        ItemAction::Link,
        !item.read_only,
    ));
    entries.push(MenuEntry::Separator);
    entries.push(enabled_if(
        "Rename",
        ItemAction::Rename,
        item.is_renameable(),
    ));
    entries.push(enabled_if(
        "Move to Trash",
        ItemAction::MoveToTrash,
        item.can_delete(),
    ));
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::action("Properties", ItemAction::Properties));
    entries
}

fn enabled_if(label: &'static str, action: ItemAction, enabled: bool) -> MenuEntry {
    if enabled {
        MenuEntry::action(label, action)
    } else {
        MenuEntry::disabled(label, action)
    }
}

fn action_label(action: ItemAction) -> &'static str {
    match action {
        ItemAction::Launch => "Launch",
        ItemAction::Mount => "Mount",
        _ => "Open",
    }
}
