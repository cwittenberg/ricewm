//! Recognizable desktop file-manager composition and stable semantic IDs.

use rice_files::{EntryKind, EntryMetadata, PaneSlot, ViewMode, infer_media_type};
use rice_native_app_protocol::{WidgetId, WindowId};
use rice_ui::{IconId, Patch, WidgetKind, WidgetSpec, WidgetState, file_icon, folder_icon};

use crate::file_tree_ui::{TreeItemState, create_file_tree, create_tree_item};

use super::files_detail_view;
pub(super) use super::files_pane_widgets::{
    DETAIL_DISCLOSURE_OFFSET, DETAIL_EMPTY_LABEL_OFFSET, DETAIL_EMPTY_ROW_OFFSET,
    DETAIL_NAME_CELL_OFFSET, PaneWidgetIds, THUMBNAIL_CELL_OFFSET, THUMBNAIL_IMAGE_OFFSET,
    detail_disclosure_for_widget, entry_for_drop_widget, entry_for_widget, pane_action,
    pane_for_widget, pane_ids, selection_background,
};

pub(super) const WINDOW: u64 = 1;
pub(super) const ROOT: u64 = 1;
pub(super) const MENU_BAR: u64 = 2;
pub(super) const TABS: u64 = 55;
pub(super) const TAB_PAGE: u64 = 56;
pub(super) const NAV_TOOLBAR: u64 = 60;
const TOOLTIP_BASE: u64 = 240;
pub(super) const BACK: u64 = 62;
pub(super) const FORWARD: u64 = 63;
pub(super) const UP: u64 = 64;
pub(super) const HOME: u64 = 65;
pub(super) const RELOAD: u64 = 66;
pub(super) const GRID_VIEW: u64 = 67;
pub(super) const LIST_VIEW: u64 = 68;
pub(super) const PREVIEW_TOGGLE: u64 = 69;
pub(super) const SPLIT_TOGGLE: u64 = 70;
pub(super) const SPLIT_NARROW: u64 = 71;
pub(super) const SPLIT_WIDE: u64 = 72;
pub(super) const PLACES_TOGGLE: u64 = 73;
pub(super) const SIMPLIFIED_VIEW: u64 = 74;
pub(super) const FOLDER_TREE: u64 = 75;
const LOCATION_ROW: u64 = 80;
pub(super) const BREADCRUMB: u64 = 81;
pub(super) const SEARCH_FIELD: u64 = 82;
pub(super) const SEARCH: u64 = 83;
const SEARCH_GROUP: u64 = 86;
pub(super) const RENAME_FIELD: u64 = 92;
pub(super) const RENAME: u64 = 93;
pub(super) const MOVE_FIELD: u64 = 94;
pub(super) const MOVE: u64 = 95;
pub(super) const DELETE: u64 = 96;
pub(super) const CONTENT: u64 = 100;
pub(super) const PLACES: u64 = 101;
pub(super) const PLACES_HEADING: u64 = 102;
pub(super) const PLACE_HOME: u64 = 103;
pub(super) const PLACE_DOCUMENTS: u64 = 104;
pub(super) const PLACE_PICTURES: u64 = 105;
pub(super) const PLACE_TRASH: u64 = 106;
pub(super) const PLACES_LIST: u64 = 107;
pub(super) const PLACE_MOUNTS: u64 = 108;
const PLACES_SEPARATOR: u64 = 109;
pub(super) const PLACE_ADD: u64 = 135;
pub(super) const PLACE_REMOVE: u64 = 136;
pub(super) const PLACE_DESKTOP: u64 = 137;
pub(super) const PLACE_DOWNLOADS: u64 = 138;
pub(super) const PLACE_VIDEOS: u64 = 139;
const PLACE_MOUNTS_TOOLTIP: u64 = 124;
pub(super) const MAIN_SPLIT: u64 = 110;
pub(super) const PRIMARY_PANE: u64 = 111;
pub(super) const DIRECTORY_SCROLL: u64 = 112;
pub(super) const DIRECTORY: u64 = 113;
pub(super) const CONTENT_NOTICE: u64 = 114;
pub(super) const PREVIEW_PANEL: u64 = 115;
pub(super) const SECONDARY_PANE: u64 = 116;
pub(super) const SECONDARY_DIRECTORY_SCROLL: u64 = 128;
pub(super) const SECONDARY_DIRECTORY: u64 = 129;
pub(super) const SECONDARY_CONTENT_NOTICE: u64 = 130;
pub(super) const DETAIL_HEADER: u64 = 117;
pub(super) const DETAIL_NAME: u64 = 118;
pub(super) const DETAIL_SIZE: u64 = 119;
pub(super) const DETAIL_MODIFIED: u64 = 126;
pub(super) const DETAIL_META_HEADER: u64 = 127;
pub(super) const DETAIL_TYPE: u64 = 133;
pub(super) const STATUS_PANEL: u64 = 120;
pub(super) const LOCATION_STATUS: u64 = 121;
const SELECTION_STATUS: u64 = 122;
const STATUS: u64 = 125;
pub(super) const ENTRY_BASE: u64 = 1_000;
pub(super) const SECONDARY_ENTRY_BASE: u64 = 100_000;
pub(super) const TAB_BASE: u64 = 10_000;
pub(super) const TAB_ROW_BASE: u64 = 20_000;
pub(super) const TAB_CLOSE_BASE: u64 = 30_000;
const TAB_CLOSE_TOOLTIP_BASE: u64 = 32_000;
pub(super) const COMPLETION_BASE: u64 = 12_000;
pub(super) const DETAIL_ROW_BASE: u64 = 40_000;
pub(super) const DETAIL_META_BASE: u64 = 50_000;
pub(super) const DETAIL_SIZE_BASE: u64 = 60_000;
pub(super) const DETAIL_MODIFIED_BASE: u64 = 70_000;
pub(super) const DETAIL_TYPE_BASE: u64 = 80_000;
pub(super) const BOOKMARK_BASE: u64 = 200_000;
pub(super) const BOOKMARK_CONTEXT_BAR_BASE: u64 = 210_000;
pub(super) const BOOKMARK_CONTEXT_MENU_BASE: u64 = 220_000;
pub(super) const BOOKMARK_REMOVE_BASE: u64 = 230_000;
const FOLDER_TREE_ENTRY_BASE: u64 = 400_000;
const FOLDER_TREE_ANCESTOR_BASE: u64 = 410_000;
pub(super) const NEW_WINDOW: u64 = 201;
pub(super) const NEW_TAB: u64 = 202;
pub(super) const VIEW_ICONS: u64 = 210;
pub(super) const VIEW_COMPACT: u64 = 211;
pub(super) const VIEW_THUMBNAILS: u64 = 212;
pub(super) const VIEW_DETAILS: u64 = 213;
pub(super) const SORT_NAME: u64 = 220;
pub(super) const SORT_TYPE: u64 = 221;
pub(super) const SORT_SIZE: u64 = 222;
pub(super) const SORT_EXTENSION: u64 = 223;
pub(super) const SORT_MODIFIED: u64 = 224;
pub(super) const HELP_ABOUT: u64 = 225;
pub(super) const ZOOM_OUT: u64 = 226;
pub(super) const ZOOM_IN: u64 = 227;
pub(super) const ZOOM_SLIDER: u64 = 228;
const ZOOM_PANEL: u64 = 229;

const TAB_PATH_CHARACTERS: usize = 32;
const PANE_PATH_CHARACTERS: usize = 40;
const STATUS_PATH_CHARACTERS: usize = 48;

pub(super) fn toolbar_patches(
    patches: &mut Vec<Patch>,
    search_expanded: bool,
    search: &str,
    redmond98: bool,
) {
    create(
        patches,
        NAV_TOOLBAR,
        Some(ROOT),
        WidgetKind::ToolBar,
        Some(if redmond98 {
            "panel:files-toolbar:redmond98"
        } else {
            "panel:files-toolbar"
        }),
        None,
    );
    let actions = if redmond98 {
        vec![
            (BACK, "Back", IconId::GoBack),
            (FORWARD, "Forward", IconId::GoForward),
            (UP, "Up", IconId::GoUp),
            (RELOAD, "Refresh", IconId::Refresh),
            (HOME, "Home", IconId::Home),
            (PLACES_TOGGLE, "Folders", IconId::ViewList),
        ]
    } else {
        vec![
            // Keep Reload in the always-visible leading group even when the
            // window is narrowed and the toolbar has to compress later actions.
            (RELOAD, "Reload", IconId::Refresh),
            (PLACES_TOGGLE, "Places", IconId::ViewList),
            (BACK, "Back", IconId::GoBack),
            (FORWARD, "Forward", IconId::GoForward),
            (UP, "Up", IconId::GoUp),
            (HOME, "Home", IconId::Home),
        ]
    };
    for (index, (id, label, icon)) in actions.into_iter().enumerate() {
        create_with_state(
            patches,
            id,
            Some(NAV_TOOLBAR),
            WidgetKind::Button,
            Some(rice_i18n::t(label)),
            Some(icon),
            WidgetState::ENABLED
                | if redmond98 {
                    WidgetState::START_ALIGNED | WidgetState::FLAT
                } else {
                    WidgetState::empty()
                },
        );
        create(
            patches,
            TOOLTIP_BASE + u64::try_from(index).unwrap_or_default(),
            Some(id),
            WidgetKind::Tooltip,
            Some(rice_i18n::t(label)),
            None,
        );
    }
    if search_expanded {
        create_with_state(
            patches,
            SEARCH_FIELD,
            Some(NAV_TOOLBAR),
            WidgetKind::TextField,
            Some(search),
            Some(IconId::Search),
            WidgetState::ENABLED | WidgetState::FOCUSED,
        );
    } else {
        create_with_state(
            patches,
            SEARCH,
            Some(NAV_TOOLBAR),
            WidgetKind::Button,
            Some(rice_i18n::t("Search")),
            Some(IconId::Search),
            WidgetState::ENABLED
                | if redmond98 {
                    WidgetState::START_ALIGNED | WidgetState::FLAT
                } else {
                    WidgetState::empty()
                },
        );
        create(
            patches,
            TOOLTIP_BASE + 6,
            Some(SEARCH),
            WidgetKind::Tooltip,
            Some(rice_i18n::t("Search")),
            None,
        );
    }
}

pub(super) struct TabView<'a> {
    pub id: u64,
    pub location: &'a str,
    pub active: bool,
}

pub(super) fn tab_patches(patches: &mut Vec<Patch>, tabs: &[TabView<'_>]) {
    if tabs.len() < 2 {
        return;
    }
    create(
        patches,
        TABS,
        Some(ROOT),
        WidgetKind::Tabs,
        Some("tabs:horizontal:integrated"),
        None,
    );
    for tab in tabs {
        let label = tab
            .location
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or("/");
        let label = tail_elided_path(label, TAB_PATH_CHARACTERS);
        create_with_state(
            patches,
            TAB_ROW_BASE.saturating_add(tab.id),
            Some(TABS),
            WidgetKind::Row,
            Some("panel:tab"),
            None,
            if tab.active {
                WidgetState::ENABLED | WidgetState::SELECTED
            } else {
                WidgetState::ENABLED
            },
        );
        create_with_state(
            patches,
            TAB_BASE.saturating_add(tab.id),
            Some(TAB_ROW_BASE.saturating_add(tab.id)),
            WidgetKind::Button,
            Some(&label),
            Some(IconId::Folder),
            WidgetState::ENABLED | WidgetState::FLAT | WidgetState::START_ALIGNED,
        );
        create_with_state(
            patches,
            TAB_CLOSE_BASE.saturating_add(tab.id),
            Some(TAB_ROW_BASE.saturating_add(tab.id)),
            WidgetKind::Button,
            Some("panel:tab-close"),
            Some(IconId::Close),
            WidgetState::ENABLED | WidgetState::FLAT,
        );
        create(
            patches,
            TAB_CLOSE_TOOLTIP_BASE.saturating_add(tab.id),
            Some(TAB_CLOSE_BASE.saturating_add(tab.id)),
            WidgetKind::Tooltip,
            Some(&rice_i18n::format("files.close-tab", &[("label", &label)])),
            None,
        );
    }
}

pub(super) fn tab_page_patches(patches: &mut Vec<Patch>, visible: bool, simplified_shell: bool) {
    if !visible {
        return;
    }
    create(
        patches,
        TAB_PAGE,
        Some(TABS),
        WidgetKind::Container,
        Some(if simplified_shell {
            "panel:tab-page:flex:files-shell:simplified"
        } else {
            "panel:tab-page:flex"
        }),
        None,
    );
}

pub(super) fn attach_shell_to_tab_page(patches: &mut [Patch], visible: bool) {
    if !visible {
        return;
    }
    let mut page_index = 0_usize;
    for patch in patches {
        if let Patch::Create {
            widget,
            parent: Some(parent),
            index,
        } = patch
            && *parent == widget_id(ROOT)
            && matches!(
                widget.id.get(),
                LOCATION_ROW | SEARCH_GROUP | NAV_TOOLBAR | CONTENT | STATUS_PANEL
            )
        {
            *parent = widget_id(TAB_PAGE);
            *index = page_index;
            page_index = page_index.saturating_add(1);
        }
    }
}

pub(super) fn location_patches(
    patches: &mut Vec<Patch>,
    location: &str,
    suggestions: &[rice_files::GuestPath],
    selected_suggestion: Option<usize>,
    redmond98: bool,
) {
    create(
        patches,
        LOCATION_ROW,
        Some(ROOT),
        // A generic container flexes vertically in the shared root column and
        // used to absorb surplus height, leaving a large empty band between
        // the path and navigation controls. This is a single compact row.
        WidgetKind::Row,
        Some("panel:files-location"),
        None,
    );
    if redmond98 {
        create(
            patches,
            87,
            Some(LOCATION_ROW),
            WidgetKind::Label,
            Some(rice_i18n::t("Address")),
            None,
        );
    }
    create(
        patches,
        BREADCRUMB,
        Some(LOCATION_ROW),
        WidgetKind::TextField,
        Some(if redmond98 && location == "/root" {
            "My Computer"
        } else {
            location
        }),
        Some(if redmond98 && location == "/root" { IconId::Computer } else { IconId::FolderOpen }),
    );
    completion_patches(patches, suggestions, selected_suggestion);
}

fn completion_patches(
    patches: &mut Vec<Patch>,
    suggestions: &[rice_files::GuestPath],
    selected: Option<usize>,
) {
    if suggestions.is_empty() {
        return;
    }
    create(
        patches,
        SEARCH_GROUP,
        Some(ROOT),
        WidgetKind::List,
        None,
        None,
    );
    for (index, suggestion) in suggestions.iter().enumerate() {
        let Ok(index_id) = u64::try_from(index) else {
            break;
        };
        let label = tail_elided_path(suggestion.as_str(), STATUS_PATH_CHARACTERS);
        create_with_state(
            patches,
            COMPLETION_BASE + index_id,
            Some(SEARCH_GROUP),
            WidgetKind::Button,
            Some(&label),
            Some(IconId::Folder),
            if selected == Some(index) {
                WidgetState::ENABLED | WidgetState::SELECTED
            } else {
                WidgetState::ENABLED
            },
        );
    }
}

pub(super) struct PaneContentView<'a> {
    pub location: &'a str,
    pub entries: &'a [EntryMetadata],
    pub selected: &'a [&'a rice_files::GuestPath],
    pub focused_entry: Option<&'a rice_files::GuestPath>,
    pub mode: ViewMode,
    pub sort: rice_files::SortStrategy,
    pub expanded_directories:
        &'a std::collections::BTreeMap<rice_files::GuestPath, Vec<EntryMetadata>>,
    pub scroll: u32,
    pub focused: bool,
    pub zoom_permille: u16,
    pub thumbnails:
        &'a std::collections::BTreeMap<rice_files::GuestPath, rice_native_app_protocol::RequestId>,
    pub content_notice: &'a str,
    pub loading: bool,
}

pub(super) struct ContentView<'a> {
    pub primary: PaneContentView<'a>,
    pub secondary: Option<PaneContentView<'a>>,
    pub preview: Option<&'a super::files_preview::PreviewState>,
    pub sidebar_visible: bool,
    pub sidebar_permille: u16,
    pub split_permille: u16,
    pub local_files_available: bool,
    pub local_mount_root: &'a str,
    pub mounts: &'a [rice_native_app_protocol::MountEntry],
    pub bookmarks: &'a [rice_files::Bookmark],
    pub folder_tree: bool,
    pub tree_directories: &'a std::collections::BTreeMap<rice_files::GuestPath, Vec<EntryMetadata>>,
    pub trash_nonempty: bool,
    pub simplified_view: bool,
    pub redmond98: bool,
}

pub(super) fn content_patches(
    patches: &mut Vec<Patch>,
    view: &ContentView<'_>,
) -> Result<(), super::FilesActorError> {
    let content_kind = if view.sidebar_visible {
        WidgetKind::SplitPane
    } else {
        WidgetKind::Container
    };
    // Keep place names readable in narrow windows while still allowing the
    // user to resize the pane continuously. The pixel bounds are interpreted
    // by the shared split-view layout rather than by RiceFiles hit testing.
    let sidebar = format!(
        "split:{}:min-width=140:max-width=420:files-content",
        view.sidebar_permille
    );
    create(
        patches,
        CONTENT,
        Some(ROOT),
        content_kind,
        Some(if view.sidebar_visible {
            sidebar.as_str()
        } else {
            "panel:files-content"
        }),
        None,
    );
    if view.sidebar_visible {
        let active_pane = view
            .secondary
            .as_ref()
            .filter(|pane| pane.focused)
            .unwrap_or(&view.primary);
        places_patches(
            patches,
            active_pane,
            view.local_files_available,
            view.local_mount_root,
            view.bookmarks,
            view.folder_tree,
            view.tree_directories,
            view.trash_nonempty,
            view.redmond98,
        );
        for (index, mount) in view.mounts.iter().enumerate() {
            if !(mount.id.starts_with("smb-") || mount.id.starts_with("nfs-") || mount.id.starts_with("cloud-")) {continue;}
            let id = 320_000 + index as u64;
            let icon = mount.icon.and_then(IconId::from_wire).unwrap_or(IconId::Network);
            create_with_state(patches, id, Some(PLACES_LIST), WidgetKind::Button, Some(&mount.label), Some(icon), WidgetState::ENABLED | WidgetState::START_ALIGNED);
            create_with_state(patches, 320_500 + index as u64, Some(id), WidgetKind::MenuBar, Some("context"), None, WidgetState::ENABLED);
            create_with_state(patches, 320_100 + index as u64, Some(320_500 + index as u64), WidgetKind::Menu, Some(&mount.label), Some(icon), WidgetState::ENABLED);
            create_with_state(patches, 320_300 + index as u64, Some(320_100 + index as u64), WidgetKind::MenuItem, Some(rice_i18n::t("Reconnect")), Some(IconId::Network), WidgetState::ENABLED);
            create_with_state(patches, 320_400 + index as u64, Some(320_100 + index as u64), WidgetKind::MenuItem, Some(rice_i18n::t("Forget Connection")), Some(IconId::ObjectRemove), WidgetState::ENABLED);
            create_with_state(patches, 320_200 + index as u64, Some(320_100 + index as u64), WidgetKind::MenuItem, Some(rice_i18n::t("Disconnect")), Some(IconId::Network), WidgetState::ENABLED);
        }
    }
    let split = format!("split:{}", view.split_permille);
    create(
        patches,
        MAIN_SPLIT,
        Some(CONTENT),
        WidgetKind::SplitPane,
        Some(&split),
        None,
    );
    pane_patches(
        patches,
        PaneSlot::Primary,
        &view.primary,
        view.simplified_view,
        view.redmond98,
    )?;
    if let Some(secondary) = &view.secondary {
        pane_patches(
            patches,
            PaneSlot::Secondary,
            secondary,
            view.simplified_view,
            view.redmond98,
        )?;
    }
    if let Some(preview) = view.preview {
        super::files_preview_view::patches(patches, preview);
    }
    Ok(())
}

fn pane_patches(
    patches: &mut Vec<Patch>,
    slot: PaneSlot,
    view: &PaneContentView<'_>,
    simplified_view: bool,
    redmond98: bool,
) -> Result<(), super::FilesActorError> {
    let ids = pane_ids(slot);
    create_with_state(
        patches,
        ids.pane,
        Some(MAIN_SPLIT),
        WidgetKind::Column,
        None,
        None,
        if view.focused {
            WidgetState::ENABLED | WidgetState::SELECTED
        } else {
            WidgetState::ENABLED
        },
    );
    let presented_location = tail_elided_path(view.location, PANE_PATH_CHARACTERS);
    if !redmond98 && !simplified_view {
        create(
            patches,
            ids.location,
            Some(ids.pane),
            WidgetKind::Label,
            Some(&presented_location),
            Some(IconId::FolderOpen),
        );
    }
    // The Web View identity strip is the My Computer root presentation. A
    // normal Explorer folder uses the full content area.
    let scroll_parent = if redmond98 && view.location == "/root" {
        let body = ids.pane.saturating_add(700_000);
        let identity = ids.pane.saturating_add(710_000);
        create(
            patches,
            body,
            Some(ids.pane),
            WidgetKind::SplitPane,
            Some("split:220:redmond98-my-computer"),
            None,
        );
        create(
            patches,
            identity,
            Some(body),
            WidgetKind::Column,
            Some("panel:redmond98-webview"),
            None,
        );
        let identity_icon = identity.saturating_add(1);
        create(
            patches,
            identity_icon,
            Some(identity),
            WidgetKind::Column,
            Some("panel:fixed-height:48:redmond98-webview-icon"),
            None,
        );
        create_with_state(
            patches,
            identity_icon.saturating_add(100),
            Some(identity_icon),
            WidgetKind::Image,
            None,
            Some(IconId::Computer),
            WidgetState::ENABLED | WidgetState::DISPLAY | WidgetState::START_ALIGNED,
        );
        for (offset, heading) in [(2_u64, "My"), (4, "Computer")] {
            create_with_state(
                patches,
                identity.saturating_add(offset),
                Some(identity),
                WidgetKind::Label,
                Some(heading),
                None,
                WidgetState::ENABLED | WidgetState::DISPLAY | WidgetState::START_ALIGNED,
            );
        }
        create(
            patches,
            identity.saturating_add(3),
            Some(identity),
            WidgetKind::Row,
            Some("panel:redmond98-webview-rule"),
            None,
        );
        let detail_lines = vec![
            format!("{} object(s)", view.entries.len()),
            "Select an item to".to_owned(),
            "view its".to_owned(),
            "description.".to_owned(),
        ];
        for (index, line) in detail_lines.iter().enumerate() {
            create_with_state(
                patches,
                identity.saturating_add(10 + u64::try_from(index).unwrap_or_default()),
                Some(identity),
                WidgetKind::Label,
                Some(line),
                None,
                WidgetState::ENABLED | WidgetState::START_ALIGNED,
            );
        }
        body
    } else {
        ids.pane
    };
    let base_extent = if matches!(view.mode, ViewMode::Icons | ViewMode::Thumbnails) {
        128
    } else {
        40
    };
    let row_height = u32::from(view.zoom_permille).saturating_mul(base_extent) / 1_000;
    let total = u32::try_from(view.entries.len())
        .unwrap_or(u32::MAX)
        .saturating_mul(row_height.max(1));
    create(
        patches,
        ids.scroll,
        Some(scroll_parent),
        WidgetKind::ScrollView,
        Some(&format!("{}/{total}", view.scroll)),
        None,
    );
    create_with_state(
        patches,
        ids.directory,
        Some(ids.scroll),
        match view.mode {
            ViewMode::Icons | ViewMode::Thumbnails => WidgetKind::IconGrid,
            ViewMode::Compact => WidgetKind::List,
            ViewMode::Details => WidgetKind::Table,
        },
        Some(&format!("file-grid:{}", view.zoom_permille)),
        None,
        WidgetState::ENABLED
            | if view.focused
                && !view
                    .entries
                    .iter()
                    .any(|entry| view.selected.contains(&&entry.path))
            {
                WidgetState::FOCUSED
            } else {
                WidgetState::empty()
            },
    );
    if view.mode == ViewMode::Details {
        files_detail_view::header_patches(patches, ids, view.sort, redmond98);
    }
    entry_patches(patches, ids, view, redmond98)?;
    if view.entries.is_empty() {
        let notice = if view.content_notice.is_empty() {
            "This folder is empty."
        } else {
            view.content_notice
        };
        create_with_state(
            patches,
            ids.notice,
            Some(ids.directory),
            WidgetKind::Label,
            Some(notice),
            if view.loading {Some(IconId::Refresh)} else {None},
            WidgetState::ENABLED
                | if view.loading {WidgetState::AMBIENT_MOTION} else {WidgetState::empty()}
                | WidgetState::MUTED
                | WidgetState::CENTER_ALIGNED
                | WidgetState::BRAND_TYPE
                | WidgetState::EMPHASIZED,
        );
    }
    Ok(())
}

fn places_patches(
    patches: &mut Vec<Patch>,
    view: &PaneContentView<'_>,
    local_files_available: bool,
    local_mount_root: &str,
    bookmarks: &[rice_files::Bookmark],
    folder_tree: bool,
    tree_directories: &std::collections::BTreeMap<rice_files::GuestPath, Vec<EntryMetadata>>,
    trash_nonempty: bool,
    redmond98: bool,
) {
    let location = view.location;
    create(
        patches,
        PLACES,
        Some(CONTENT),
        // The whole sidebar is a semantic list/drop surface, including the
        // blank area below its rows. Child rows remain the more specific hit
        // targets when the pointer is over one of them.
        if folder_tree {
            WidgetKind::Column
        } else {
            WidgetKind::List
        },
        Some(if redmond98 && folder_tree {
            "panel:sidebar:redmond98-folder-tree"
        } else {
            "panel:sidebar"
        }),
        None,
    );
    create(
        patches,
        PLACES_HEADING,
        Some(PLACES),
        WidgetKind::Label,
        Some(rice_i18n::t(if redmond98 && folder_tree {
            "All Folders"
        } else if folder_tree {
            "Folders"
        } else {
            "Places"
        })),
        None,
    );
    if folder_tree {
        create_file_tree(patches, PLACES_LIST, PLACES, None);
    } else {
        create(
            patches,
            PLACES_LIST,
            Some(PLACES),
            WidgetKind::List,
            None,
            None,
        );
    }
    if folder_tree {
        folder_tree_patches(
            patches,
            view,
            local_files_available,
            local_mount_root,
            bookmarks,
            tree_directories,
            trash_nonempty,
            redmond98,
        );
        return;
    }
    for (id, label, path, icon) in [
        (PLACE_DESKTOP, "Desktop", "/root/Desktop", IconId::Desktop),
        (PLACE_HOME, "Home", "/root", IconId::Home),
        (
            PLACE_DOCUMENTS,
            "Documents",
            "/root/Documents",
            IconId::Documents,
        ),
        (
            PLACE_DOWNLOADS,
            "Downloads",
            "/root/Downloads",
            IconId::Downloads,
        ),
        (
            PLACE_PICTURES,
            "Pictures",
            "/root/Pictures",
            IconId::Pictures,
        ),
        (PLACE_VIDEOS, "Videos", "/root/Videos", IconId::Folder),
        (
            PLACE_TRASH,
            "Trash",
            "/root/.local/share/Trash/files",
            if trash_nonempty {
                IconId::TrashFull
            } else {
                IconId::Trash
            },
        ),
    ] {
        create_with_state(
            patches,
            id,
            Some(PLACES_LIST),
            WidgetKind::Button,
            Some(rice_i18n::t(label)),
            Some(icon),
            WidgetState::ENABLED
                | WidgetState::START_ALIGNED
                | if place_selected(location, path) {
                    WidgetState::SELECTED
                } else {
                    WidgetState::empty()
                },
        );
    }
    for (index, bookmark) in bookmarks.iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        create_with_state(
            patches,
            BOOKMARK_BASE.saturating_add(index),
            Some(PLACES_LIST),
            WidgetKind::Button,
            Some(super::files_preferences::bookmark_display_label(bookmark)),
            Some(IconId::Folder),
            WidgetState::ENABLED
                | WidgetState::START_ALIGNED
                | if place_selected(location, bookmark.path.as_str()) {
                    WidgetState::SELECTED
                } else {
                    WidgetState::empty()
                },
        );
        create_with_state(
            patches,
            BOOKMARK_CONTEXT_BAR_BASE.saturating_add(index),
            Some(BOOKMARK_BASE.saturating_add(index)),
            WidgetKind::MenuBar,
            Some("context"),
            None,
            WidgetState::ENABLED,
        );
        create_with_state(
            patches,
            BOOKMARK_CONTEXT_MENU_BASE.saturating_add(index),
            Some(BOOKMARK_CONTEXT_BAR_BASE.saturating_add(index)),
            WidgetKind::Menu,
            Some(super::files_preferences::bookmark_display_label(bookmark)),
            Some(IconId::Folder),
            WidgetState::ENABLED,
        );
        create_with_state(
            patches,
            BOOKMARK_REMOVE_BASE.saturating_add(index),
            Some(BOOKMARK_CONTEXT_MENU_BASE.saturating_add(index)),
            WidgetKind::MenuItem,
            Some(rice_i18n::t("Remove from Places")),
            Some(IconId::ObjectRemove),
            WidgetState::ENABLED,
        );
    }
    let current_is_bookmark = bookmarks
        .iter()
        .any(|bookmark| bookmark.path.as_str() == location);
    if !current_is_bookmark && !super::files_preferences::is_builtin(location)
        && location != local_mount_root
    {
        create_with_state(
            patches,
            PLACE_ADD,
            Some(PLACES_LIST),
            WidgetKind::Button,
            Some(rice_i18n::t("Add Current Folder")),
            Some(IconId::ObjectAdd),
            WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::FLAT,
        );
    }
    create(
        patches,
        PLACES_SEPARATOR,
        Some(PLACES_LIST),
        WidgetKind::Row,
        Some("panel:separator"),
        None,
    );
    let local_state = if local_files_available {
        let selected = place_selected(location, local_mount_root);
        WidgetState::ENABLED
            | WidgetState::START_ALIGNED
            | if selected {
                WidgetState::SELECTED
            } else {
                WidgetState::empty()
            }
    } else {
        WidgetState::START_ALIGNED
    };
    create_with_state(
        patches,
        PLACE_MOUNTS,
        Some(PLACES_LIST),
        WidgetKind::Button,
        Some(rice_i18n::t("Local Files")),
        Some(IconId::NetworkMounts),
        local_state,
    );
    if !local_files_available {
        create_with_state(
            patches,
            PLACE_MOUNTS_TOOLTIP,
            Some(PLACE_MOUNTS),
            WidgetKind::Tooltip,
            Some(rice_i18n::t("No folders were mounted")),
            None,
            WidgetState::ENABLED,
        );
    }
}

fn folder_tree_patches(
    patches: &mut Vec<Patch>,
    view: &PaneContentView<'_>,
    local_files_available: bool,
    local_mount_root: &str,
    bookmarks: &[rice_files::Bookmark],
    tree_directories: &std::collections::BTreeMap<rice_files::GuestPath, Vec<EntryMetadata>>,
    trash_nonempty: bool,
    redmond98: bool,
) {
    let location = view.location;
    for (id, parent, label, path, icon, expanded) in [
        (
            PLACE_DESKTOP,
            PLACES_LIST,
            "Desktop",
            "/root/Desktop",
            IconId::Desktop,
            true,
        ),
        (
            PLACE_HOME,
            PLACE_DESKTOP,
            if redmond98 { "My Computer" } else { "Home" },
            "/root",
            if redmond98 {
                IconId::Computer
            } else {
                IconId::Home
            },
            true,
        ),
        (
            PLACE_DOCUMENTS,
            PLACE_DESKTOP,
            if redmond98 {
                "My Documents"
            } else {
                "Documents"
            },
            "/root/Documents",
            IconId::Documents,
            place_selected(location, "/root/Documents"),
        ),
        (
            PLACE_TRASH,
            PLACE_DESKTOP,
            if redmond98 { "Recycle Bin" } else { "Trash" },
            "/root/.local/share/Trash/files",
            if trash_nonempty {
                IconId::TrashFull
            } else {
                IconId::Trash
            },
            place_selected(location, "/root/.local/share/Trash/files"),
        ),
    ] {
        create_tree_item(
            patches,
            id,
            parent,
            rice_i18n::t(label),
            icon,
            TreeItemState {
                expanded,
                selected: !path.is_empty() && location == path,
                ..TreeItemState::default()
            },
        );
    }
    if local_files_available {
        create_tree_item(
            patches,
            PLACE_MOUNTS,
            PLACE_HOME,
            rice_i18n::t("Mounted Folders"),
            IconId::Drive,
            TreeItemState {
                selected: place_selected(location, local_mount_root),
                ..TreeItemState::default()
            },
        );
    }

    let mut directories = tree_directories
        .values()
        .flatten()
        .filter(|entry| {
            entry.kind == EntryKind::Directory
                && !matches!(entry.path.as_str(), "/root/Desktop" | "/root/Documents")
        })
        .collect::<Vec<_>>();
    directories.sort_by(|left, right| left.path.cmp(&right.path));
    directories.dedup_by(|left, right| left.path == right.path);

    let ancestors = folder_tree_ancestor_paths(location, local_mount_root);
    let mut ancestor_parent = folder_tree_static_parent(location, local_mount_root);
    for (index, (path, label)) in ancestors.iter().enumerate() {
        if let Some(cached) = directories
            .iter()
            .position(|entry| entry.path.as_str() == path)
        {
            let Ok(cached) = u64::try_from(cached) else {
                break;
            };
            ancestor_parent = FOLDER_TREE_ENTRY_BASE.saturating_add(cached);
            continue;
        }
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        let id = FOLDER_TREE_ANCESTOR_BASE.saturating_add(index);
        create_tree_item(
            patches,
            id,
            ancestor_parent,
            label,
            IconId::FolderOpen,
            TreeItemState {
                expanded: true,
                selected: path == location,
                ..TreeItemState::default()
            },
        );
        ancestor_parent = id;
    }

    let current_parent = ancestor_parent;
    for (index, entry) in directories.iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        let entry_parent = entry.path.parent();
        let parent = match entry_parent.as_str() {
            "/root" => PLACE_HOME,
            "/root/Documents" => PLACE_DOCUMENTS,
            "/root/Desktop" => PLACE_DESKTOP,
            "/root/.local/share/Trash/files" => PLACE_TRASH,
            "/mnt/local" => PLACE_MOUNTS,
            _ => directories
                .iter()
                .position(|candidate| candidate.path == entry_parent)
                .and_then(|parent| u64::try_from(parent).ok())
                .map(|parent| {
                    FOLDER_TREE_ENTRY_BASE.saturating_add(parent)
                })
                .or_else(|| {
                    // Mounted paths have synthetic ancestors above the first
                    // enumerated directory. Keep their cached children under
                    // that ancestor when navigation moves deeper; attaching
                    // them to the current folder can make it its own parent.
                    ancestors.iter().position(|(path, _)| path == entry_parent.as_str())
                        .and_then(|parent| u64::try_from(parent).ok())
                        .map(|parent| FOLDER_TREE_ANCESTOR_BASE.saturating_add(parent))
                })
                .unwrap_or(current_parent),
        };
        create_tree_item(
            patches,
            FOLDER_TREE_ENTRY_BASE.saturating_add(index),
            parent,
            super::files_details::display_name(entry),
            entry_icon(entry),
            TreeItemState {
                expanded: location.starts_with(&format!("{}/", entry.path)),
                selected: location == entry.path.as_str(),
                muted: entry.name.starts_with('.'),
                ..TreeItemState::default()
            },
        );
    }

    for (index, bookmark) in bookmarks.iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        create_tree_item(
            patches,
            BOOKMARK_BASE.saturating_add(index),
            PLACE_HOME,
            super::files_preferences::bookmark_display_label(bookmark),
            IconId::Folder,
            TreeItemState {
                selected: place_selected(location, bookmark.path.as_str()),
                ..TreeItemState::default()
            },
        );
    }
}

fn folder_tree_static_parent(location: &str, local_mount_root: &str) -> u64 {
    if place_selected(location, "/root/Documents") {
        PLACE_DOCUMENTS
    } else if place_selected(location, "/root/Desktop") {
        PLACE_DESKTOP
    } else if place_selected(location, "/root/.local/share/Trash/files") {
        PLACE_TRASH
    } else if place_selected(location, local_mount_root) {
        PLACE_MOUNTS
    } else {
        PLACE_HOME
    }
}

pub(super) fn folder_tree_ancestor_paths(
    location: &str,
    local_mount_root: &str,
) -> Vec<(String, String)> {
    let base = [
        "/root/.local/share/Trash/files",
        "/root/Documents",
        "/root/Desktop",
        local_mount_root,
        "/root",
    ]
    .into_iter()
    .find(|base| location == *base || location.starts_with(&format!("{base}/")))
    .unwrap_or("");
    let mut current = base.trim_end_matches('/').to_owned();
    location
        .strip_prefix(base)
        .unwrap_or(location)
        .split('/')
        .filter(|component| !component.is_empty())
        .map(|component| {
            current.push('/');
            current.push_str(component);
            (current.clone(), component.to_owned())
        })
        .collect()
}

pub(super) fn folder_tree_entry_index(target: u64) -> Option<usize> {
    target
        .checked_sub(FOLDER_TREE_ENTRY_BASE)
        .filter(|index| *index < rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64)
        .and_then(|index| usize::try_from(index).ok())
}

pub(super) fn folder_tree_ancestor_index(target: u64) -> Option<usize> {
    target
        .checked_sub(FOLDER_TREE_ANCESTOR_BASE)
        .filter(|index| *index < rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64)
        .and_then(|index| usize::try_from(index).ok())
}

pub(super) fn bookmark_index(target: u64) -> Option<usize> {
    target
        .checked_sub(BOOKMARK_BASE)
        .and_then(|index| usize::try_from(index).ok())
}

pub(super) fn bookmark_remove_index(target: u64) -> Option<usize> {
    target
        .checked_sub(BOOKMARK_REMOVE_BASE)
        .and_then(|index| usize::try_from(index).ok())
}

fn place_selected(location: &str, place: &str) -> bool {
    location == place
        || (place != "/root"
            && location
                .strip_prefix(place)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

fn entry_patches(
    patches: &mut Vec<Patch>,
    ids: PaneWidgetIds,
    view: &PaneContentView<'_>,
    redmond98: bool,
) -> Result<(), super::FilesActorError> {
    let focused_path = focused_entry_path(view);
    for (index, entry) in view.entries.iter().enumerate() {
        let patch_start=patches.len();
        let focused = focused_path == Some(&entry.path);
        if view.mode == ViewMode::Details {
            files_detail_view::entry_patches(
                patches,
                ids,
                index,
                entry,
                view.selected,
                view.location,
                view.expanded_directories.contains_key(&entry.path),
                focused,
                redmond98,
            )?;
            if view
                .expanded_directories
                .get(&entry.path)
                .is_some_and(Vec::is_empty)
            {
                files_detail_view::empty_directory_patches(
                    patches,
                    ids,
                    index,
                    entry,
                    view.location,
                    redmond98,
                )?;
            }
            tag_file_icons(patches,patch_start,entry);
            continue;
        }
        let id = ids.entry_base
            + u64::try_from(index).map_err(|_| super::FilesActorError::EntryLimit)?;
        let mut state = WidgetState::ENABLED | WidgetState::KEY_NAVIGATION;
        if entry.name.starts_with('.') {
            state = state | WidgetState::MUTED;
        }
        if view.mode == ViewMode::Compact {
            state = state | WidgetState::START_ALIGNED;
        }
        if view.selected.contains(&&entry.path) {
            state = state | WidgetState::SELECTED;
        }
        if focused {
            state = state | WidgetState::FOCUSED;
        }
        let media_type = infer_media_type(entry);
        let media_cell = matches!(view.mode, ViewMode::Icons | ViewMode::Thumbnails)
            && rice_files::supports_thumbnail(media_type);
        if media_cell {
            let cell = id.saturating_add(THUMBNAIL_CELL_OFFSET);
            let image = id.saturating_add(THUMBNAIL_IMAGE_OFFSET);
            let cell_state = WidgetState::ENABLED
                | if view.selected.contains(&&entry.path) {
                    WidgetState::SELECTED
                } else {
                    WidgetState::empty()
                };
            create_with_state(
                patches,
                cell,
                Some(ids.directory),
                WidgetKind::Container,
                Some("panel:thumbnail-cell"),
                None,
                cell_state,
            );
            if let Some(request) = view.thumbnails.get(&entry.path).copied() {
                create(
                    patches,
                    image,
                    Some(cell),
                    WidgetKind::Image,
                    Some(&format!("media-request:{}", request.get())),
                    None,
                );
            } else {
                create_with_state(
                    patches,
                    image,
                    Some(cell),
                    WidgetKind::Image,
                    None,
                    Some(entry_icon(entry)),
                    cell_state | WidgetState::LARGE_ICON,
                );

            }
            create_with_state(
                patches,
                id,
                Some(cell),
                WidgetKind::Button,
                Some(super::files_details::display_name(entry)),
                None,
                state | WidgetState::FLAT,
            );
        } else {
            create_with_state(
                patches,
                id,
                Some(ids.directory),
                WidgetKind::Button,
                Some(super::files_details::display_name(entry)),
                Some(entry_icon(entry)),
                state,
            );

        }
        tag_file_icons(patches,patch_start,entry);
    }
    Ok(())
}

fn tag_file_icons(patches:&mut Vec<Patch>,start:usize,entry:&EntryMetadata){
    if entry.kind!=EntryKind::File{return;}
    let ids=patches[start..].iter().filter_map(|patch|if let Patch::Create {widget,..}=patch {if widget.icon==Some(entry_icon(entry)){Some(widget.id)}else{None}}else{None}).collect::<Vec<_>>();
    for id in ids {patches.push(Patch::IconSelector {id,selector:Some(format!("mime:{}",infer_media_type(entry)))});}
}

fn focused_entry_path<'a>(view: &'a PaneContentView<'a>) -> Option<&'a rice_files::GuestPath> {
    view.focused.then(|| {
        view.focused_entry
            .filter(|path| view.entries.iter().any(|entry| &entry.path == *path))
            .or_else(|| {
                view.entries
                    .iter()
                    .find(|entry| view.selected.contains(&&entry.path))
                    .map(|entry| &entry.path)
            })
    })?
}

pub(super) fn status_patches(
    patches: &mut Vec<Patch>,
    location: &str,
    selected: usize,
    entries: usize,
    total_bytes: u64,
    zoom_permille: u16,
    redmond98: bool,
    loading: Option<usize>,
) {
    let presented_location = tail_elided_path(location, STATUS_PATH_CHARACTERS);
    let object_count = format!("{selected} object(s) selected");
    create(
        patches,
        STATUS_PANEL,
        Some(ROOT),
        WidgetKind::Container,
        Some("panel:status-bar:files-status"),
        None,
    );
    create(
        patches,
        STATUS,
        Some(STATUS_PANEL),
        WidgetKind::Row,
        Some("panel:distribute:files-status-layout"),
        None,
    );
    create(
        patches,
        134,
        Some(STATUS),
        WidgetKind::Row,
        Some("panel:files-status-copy"),
        None,
    );
    create(
        patches,
        LOCATION_STATUS,
        Some(134),
        WidgetKind::Label,
        Some(if redmond98 {
            &object_count
        } else {
            &presented_location
        }),
        if redmond98 {
            None
        } else {
            Some(IconId::Folder)
        },
    );
    if let Some(loaded) = loading {
        create(patches,SELECTION_STATUS,Some(134),WidgetKind::Row,Some("panel:compact"),None);
        create(patches,310_040,Some(SELECTION_STATUS),WidgetKind::Spinner,None,None);
        create(patches,310_041,Some(SELECTION_STATUS),WidgetKind::Label,Some(&rice_i18n::format("files.loading-count", &[("count", &loaded.to_string())])),None);
    } else {
    create_with_state(
        patches,
        SELECTION_STATUS,
        Some(134),
        WidgetKind::Label,
        Some(&if let Some(loaded) = loading {
            rice_i18n::format("files.loading-count", &[("count", &loaded.to_string())])
        } else if redmond98 {
            rice_i18n::format("files.object-summary", &[
                ("entries", &entries.to_string()),
                ("size", &super::files_details::total_size_label(total_bytes)),
            ])
        } else {
            rice_i18n::format("files.selection-summary", &[
                ("selected", &selected.to_string()),
                ("entries", &entries.to_string()),
                ("size", &super::files_details::total_size_label(total_bytes)),
            ])
        }),
        None,
        WidgetState::ENABLED | WidgetState::CENTER_ALIGNED,
    );
    }
    if redmond98 {
        create(
            patches,
            ZOOM_PANEL,
            Some(STATUS),
            WidgetKind::Label,
            Some(rice_i18n::t("My Computer")),
            Some(IconId::Computer),
        );
    } else {
        create(
            patches,
            ZOOM_PANEL,
            Some(STATUS),
            WidgetKind::Row,
            Some("panel:equal-fixed"),
            None,
        );
        create_with_state(
            patches,
            ZOOM_SLIDER,
            Some(ZOOM_PANEL),
            WidgetKind::ScrollBar,
            Some(&format!(
                "{}/{}",
                zoom_permille.saturating_sub(super::MIN_VIEW_ZOOM_PERMILLE),
                super::MAX_VIEW_ZOOM_PERMILLE - super::MIN_VIEW_ZOOM_PERMILLE
            )),
            None,
            WidgetState::ENABLED | WidgetState::FLAT,
        );
    }
}

/// Keeps the basename and extension visible when a non-editable path label has
/// to be bounded. Editable path fields intentionally keep the complete value.
pub(super) fn tail_elided_path(path: &str, max_characters: usize) -> String {
    if path.chars().count() <= max_characters {
        return path.to_owned();
    }
    if max_characters <= 1 {
        return String::from("…");
    }

    let mut suffix = path
        .chars()
        .rev()
        .take(max_characters - 1)
        .collect::<Vec<_>>();
    suffix.reverse();
    let suffix = suffix.into_iter().collect::<String>();
    let clean_boundary = suffix
        .char_indices()
        .find_map(|(index, character)| {
            (index > 0 && matches!(character, '/' | '\\')).then_some(index)
        })
        .unwrap_or(0);
    format!("…{}", &suffix[clean_boundary..])
}

pub(super) fn entry_icon(entry: &EntryMetadata) -> IconId {
    match entry.kind {
        EntryKind::Directory => folder_icon(entry.path.as_str()),
        EntryKind::File => file_icon(&entry.name, Some(infer_media_type(entry))),
        EntryKind::Symlink => IconId::Link,
        EntryKind::Other => IconId::Application,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
) {
    create_with_state(patches, id, parent, kind, text, icon, WidgetState::ENABLED);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn create_with_state(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
    state: WidgetState,
) {
    let parent = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| matches!(patch, Patch::Create { parent: candidate, .. } if *candidate == parent))
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state,
            text: text.map(str::to_owned),
            icon,
        },
        parent,
        index,
    });
}

pub(super) fn widget_id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("RiceFiles widget IDs are nonzero")
}

pub(super) fn window_id(value: u64) -> WindowId {
    WindowId::new(value).expect("RiceFiles window IDs are nonzero")
}

#[cfg(test)]
mod tooltip_translation_tests {
    use super::*;
    #[test]
    fn chinese_toolbar_tooltips_and_custom_tab_names() {
        rice_i18n::with_locale(2, || {
            let mut patches = Vec::new();
            toolbar_patches(&mut patches, false, "", false);
            let labels = patches.iter().filter_map(|patch| match patch {
                Patch::Create { widget, .. } if widget.kind == WidgetKind::Tooltip => widget.text.as_deref(),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(labels.len(), 7);
            assert!(!labels.iter().any(|label| ["Reload", "Places", "Back", "Forward", "Up", "Home", "Search"].contains(label)));
            assert!(labels.contains(&"搜索"));
            assert_eq!(rice_i18n::format("files.close-tab", &[("label", "Documents {label}")]), "关闭 Documents {label} 标签页");
        });
    }
}
