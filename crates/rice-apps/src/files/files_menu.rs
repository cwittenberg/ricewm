//! Conventional menu models for `RiceFiles`.

use super::files_view::{
    DELETE, FOLDER_TREE, HELP_ABOUT, MOVE, NEW_TAB, NEW_WINDOW, PLACES_TOGGLE, PREVIEW_TOGGLE,
    RENAME, SIMPLIFIED_VIEW, SORT_MODIFIED, SORT_NAME, SORT_SIZE, SORT_TYPE, SPLIT_TOGGLE,
    VIEW_DETAILS, VIEW_ICONS, ZOOM_IN, ZOOM_OUT,
};
use rice_ui::IconId;

pub(super) const PREFERENCES: u64 = 940;
pub(super) const SORT_ASCENDING: u64 = 941;
pub(super) const SORT_DESCENDING: u64 = 942;

pub(super) const FILE_MENU: u64 = 10;
pub(super) const EDIT_MENU: u64 = 20;
pub(super) const VIEW_MENU: u64 = 30;
pub(super) const HELP_MENU: u64 = 50;
pub(super) const CONTEXT_BAR: u64 = 51;
pub(super) const CONTEXT_MENU: u64 = 52;
pub(super) const OPEN: u64 = 910;
pub(super) const OPEN_WITH: u64 = 911;
pub(super) const CUT: u64 = 913;
pub(super) const COPY: u64 = 914;
pub(super) const COPY_TO: u64 = 915;
pub(super) const PASTE: u64 = 922;
pub(super) const DELETE_PERMANENTLY: u64 = 919;
pub(super) const PROPERTIES: u64 = 921;
pub(super) const CONNECT_SERVER: u64 = 923;
pub(super) const OPEN_IN_NEW_TAB: u64 = 924;
pub(super) const OPEN_IN_NEW_WINDOW: u64 = 925;
pub(super) const NEW_FOLDER: u64 = 926;
pub(super) const CLOSE: u64 = 927;
pub(super) const OPEN_IN_TERMINAL: u64 = 928;
pub(super) const COMPRESS_ZIP: u64 = 929;
pub(super) const COMPRESS_TAR_GZIP: u64 = 930;
pub(super) const COMPRESS_GZIP: u64 = 931;
pub(super) const EXTRACT_HERE: u64 = 932;
pub(super) const DOWNLOAD_TO_HOST: u64 = 933;
pub(super) const COMPRESS_TO: u64 = 934;
pub(super) const COMPRESS_SUBMENU: u64 = 53;
pub(super) const PUBLISH_MARKETPLACE: u64 = 935;
const MENU_ACTION_BASE: u64 = 20_000;
const CONTEXT_ACTION_BASE: u64 = 30_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuKind {
    File,
    Edit,
    View,
    Help,
}

impl MenuKind {
    pub(super) const fn widget(self) -> u64 {
        match self {
            Self::File => FILE_MENU,
            Self::Edit => EDIT_MENU,
            Self::View => VIEW_MENU,
            Self::Help => HELP_MENU,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Edit => "Edit",
            Self::View => "View",
            Self::Help => "Help",
        }
    }

    pub(super) fn all(_redmond98: bool) -> Vec<Self> {
        vec![Self::File, Self::Edit, Self::View, Self::Help]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuEntryKind {
    Action,
    Submenu,
    Check,
    Radio,
    Separator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MenuEntry {
    pub action: u64,
    pub label: &'static str,
    pub shortcut: Option<&'static str>,
    pub icon: Option<IconId>,
    pub kind: MenuEntryKind,
    pub implemented: bool,
}

const fn action(
    action: u64,
    label: &'static str,
    shortcut: Option<&'static str>,
    icon: Option<IconId>,
) -> MenuEntry {
    MenuEntry {
        action,
        label,
        shortcut,
        icon,
        kind: MenuEntryKind::Action,
        implemented: true,
    }
}

const fn choice(action: u64, label: &'static str, kind: MenuEntryKind) -> MenuEntry {
    MenuEntry {
        action,
        label,
        shortcut: None,
        icon: None,
        kind,
        implemented: true,
    }
}

const fn submenu(action: u64, label: &'static str, icon: Option<IconId>) -> MenuEntry {
    MenuEntry {
        action,
        label,
        shortcut: None,
        icon,
        kind: MenuEntryKind::Submenu,
        implemented: true,
    }
}

const fn separator(action: u64) -> MenuEntry {
    MenuEntry {
        action,
        label: "",
        shortcut: None,
        icon: None,
        kind: MenuEntryKind::Separator,
        implemented: false,
    }
}

const FILE_ENTRIES: &[MenuEntry] = &[
    action(
        NEW_WINDOW,
        "New Window",
        Some("Alt+N"),
        Some(IconId::DocumentNew),
    ),
    action(NEW_TAB, "New Tab", Some("Alt+T"), Some(IconId::DocumentNew)),
    action(NEW_FOLDER, "New Folder…", None, Some(IconId::Folder)),
    separator(901),
    action(
        CONNECT_SERVER,
        "Connect to Server…",
        None,
        Some(IconId::Network),
    ),
    separator(908),
    action(CLOSE, "Close", Some("Alt+F4"), Some(IconId::Close)),
];

const EDIT_ENTRIES: &[MenuEntry] = &[
    action(PREFERENCES, "Preferences…", None, Some(IconId::Settings)),
    separator(909),
    action(PASTE, "Paste", Some("Ctrl+V"), Some(IconId::EditPaste)),
    separator(906),
    action(RENAME, "Rename…", Some("F2"), Some(IconId::EditRedo)),
    action(MOVE, "Move to…", None, Some(IconId::GoForward)),
    separator(902),
    action(DELETE, "Move to Trash", Some("Delete"), Some(IconId::Trash)),
];

const VIEW_ENTRIES: &[MenuEntry] = &[
    choice(PLACES_TOGGLE, "Show Places", MenuEntryKind::Check),
    choice(FOLDER_TREE, "Folder Tree", MenuEntryKind::Check),
    choice(SIMPLIFIED_VIEW, "Simplified View", MenuEntryKind::Check),
    separator(907),
    choice(VIEW_ICONS, "Icon View", MenuEntryKind::Radio),
    choice(VIEW_DETAILS, "Detail View", MenuEntryKind::Radio),
    action(ZOOM_IN, "Zoom In", Some("Alt++"), None),
    action(ZOOM_OUT, "Zoom Out", Some("Alt+-"), None),
    separator(903),
    choice(PREVIEW_TOGGLE, "Preview Pane", MenuEntryKind::Check),
    choice(SPLIT_TOGGLE, "Dual Pane", MenuEntryKind::Check),
    separator(904),
    choice(SORT_NAME, "Sort by Name", MenuEntryKind::Radio),
    choice(SORT_MODIFIED, "Sort by Modified", MenuEntryKind::Radio),
    choice(SORT_SIZE, "Sort by Size", MenuEntryKind::Radio),
    choice(SORT_TYPE, "Sort by Type", MenuEntryKind::Radio),
    separator(943),
    choice(SORT_ASCENDING, "Ascending", MenuEntryKind::Radio),
    choice(SORT_DESCENDING, "Descending", MenuEntryKind::Radio),
];

const HELP_ENTRIES: &[MenuEntry] = &[action(
    HELP_ABOUT,
    "About Files",
    None,
    Some(IconId::Information),
)];

const CONTEXT_ENTRIES: &[MenuEntry] = &[
    action(NEW_FOLDER, "New Folder…", None, Some(IconId::Folder)),
    separator(909),
    action(OPEN, "Open", Some("Return"), Some(IconId::FolderOpen)),
    action(PUBLISH_MARKETPLACE, "Publish to Marketplace", None, Some(IconId::GoForward)),
    action(
        OPEN_IN_NEW_TAB,
        "Open in New Tab",
        Some("Alt+T"),
        Some(IconId::DocumentNew),
    ),
    action(
        OPEN_IN_NEW_WINDOW,
        "Open in New Window",
        Some("Alt+N"),
        Some(IconId::DocumentNew),
    ),
    action(
        OPEN_IN_TERMINAL,
        "Open in Terminal",
        None,
        Some(IconId::Terminal),
    ),
    action(OPEN_WITH, "Open With…", None, Some(IconId::Application)),
    separator(912),
    action(CUT, "Cut", Some("Ctrl+X"), Some(IconId::EditCut)),
    action(COPY, "Copy", Some("Ctrl+C"), Some(IconId::EditCopy)),
    action(PASTE, "Paste", Some("Ctrl+V"), Some(IconId::EditPaste)),
    action(MOVE, "Move to…", None, Some(IconId::GoForward)),
    action(COPY_TO, "Copy to…", None, Some(IconId::EditCopy)),
    separator(916),
    submenu(COMPRESS_TO, "Compress to", Some(IconId::Archive)),
    action(EXTRACT_HERE, "Extract Here", None, Some(IconId::Archive)),
    action(
        DOWNLOAD_TO_HOST,
        "Download to Host",
        None,
        Some(IconId::Downloads),
    ),
    separator(918),
    action(RENAME, "Rename…", Some("F2"), Some(IconId::EditRedo)),
    action(DELETE, "Move to Trash", Some("Delete"), Some(IconId::Trash)),
    action(
        DELETE_PERMANENTLY,
        "Delete Permanently…",
        Some("Shift+Delete"),
        Some(IconId::Trash),
    ),
    separator(920),
    action(
        PROPERTIES,
        "Properties",
        Some("Alt+Return"),
        Some(IconId::Information),
    ),
];

const COMPRESS_ENTRIES: &[MenuEntry] = &[
    action(COMPRESS_ZIP, "Compress to ZIP", None, Some(IconId::Archive)),
    action(
        COMPRESS_TAR_GZIP,
        "Compress to TAR.GZ",
        None,
        Some(IconId::Archive),
    ),
    action(
        COMPRESS_GZIP,
        "Compress to GZIP",
        None,
        Some(IconId::Archive),
    ),
];

pub(super) const fn entries(menu: MenuKind) -> &'static [MenuEntry] {
    match menu {
        MenuKind::File => FILE_ENTRIES,
        MenuKind::Edit => EDIT_ENTRIES,
        MenuKind::View => VIEW_ENTRIES,
        MenuKind::Help => HELP_ENTRIES,
    }
}

pub(super) const fn context_entries() -> &'static [MenuEntry] {
    CONTEXT_ENTRIES
}

pub(super) const fn compress_entries() -> &'static [MenuEntry] {
    COMPRESS_ENTRIES
}

pub(super) const fn menu_widget(action: u64) -> u64 {
    MENU_ACTION_BASE + action
}

pub(super) const fn context_widget(action: u64) -> u64 {
    CONTEXT_ACTION_BASE + action
}

pub(super) const fn action_for_widget(widget: u64) -> Option<u64> {
    if widget >= CONTEXT_ACTION_BASE && widget < CONTEXT_ACTION_BASE + 1_000 {
        Some(widget - CONTEXT_ACTION_BASE)
    } else if widget >= MENU_ACTION_BASE && widget < MENU_ACTION_BASE + 1_000 {
        Some(widget - MENU_ACTION_BASE)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn menus_have_unique_rows_and_conventional_headings() {
        assert_eq!(
            MenuKind::all(false)
                .into_iter()
                .map(MenuKind::label)
                .collect::<Vec<_>>(),
            ["File", "Edit", "View", "Help"]
        );
        for menu in MenuKind::all(false) {
            let ids = entries(menu)
                .iter()
                .map(|entry| menu_widget(entry.action))
                .collect::<BTreeSet<_>>();
            assert_eq!(ids.len(), entries(menu).len());
        }
        assert!(
            VIEW_ENTRIES.iter().any(|entry| {
                entry.action == VIEW_DETAILS && entry.kind == MenuEntryKind::Radio
            })
        );
        assert!(
            VIEW_ENTRIES
                .iter()
                .any(|entry| entry.action == PLACES_TOGGLE && entry.kind == MenuEntryKind::Check)
        );
    }

    #[test]
    fn contextual_menu_contains_only_real_operations() {
        assert!(
            CONTEXT_ENTRIES
                .iter()
                .any(|entry| { entry.label == "Move to Trash" && entry.implemented })
        );
        assert!(
            CONTEXT_ENTRIES
                .iter()
                .all(|entry| { entry.kind == MenuEntryKind::Separator || entry.implemented })
        );
        assert!(CONTEXT_ENTRIES.iter().any(|entry| {
            entry.label == "Compress to"
                && entry.kind == MenuEntryKind::Submenu
                && entry.implemented
        }));
        assert_eq!(
            COMPRESS_ENTRIES
                .iter()
                .map(|entry| entry.label)
                .collect::<Vec<_>>(),
            ["Compress to ZIP", "Compress to TAR.GZ", "Compress to GZIP"]
        );
        assert!(
            CONTEXT_ENTRIES
                .iter()
                .any(|entry| { entry.label == "Extract Here" && entry.implemented })
        );
    }
}
