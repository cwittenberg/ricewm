//! Menu widget composition separated from directory layout.

use super::{
    files_menu::{
        COMPRESS_SUBMENU, COMPRESS_TO, CONTEXT_BAR, CONTEXT_MENU, MenuEntry, MenuEntryKind,
        MenuKind, compress_entries, context_entries, context_widget, entries, menu_widget,
    },
    files_view::{
        DELETE, FOLDER_TREE, MENU_BAR, MOVE, PLACES_TOGGLE, PREVIEW_TOGGLE, RENAME, ROOT,
        SIMPLIFIED_VIEW, SORT_MODIFIED, SORT_NAME, SORT_SIZE, SORT_TYPE, SPLIT_TOGGLE,
        VIEW_DETAILS, VIEW_ICONS, create, create_with_state,
    },
};
use rice_files::{SortKey, ViewMode};
use rice_ui::{MENU_SEPARATOR, Patch, WidgetKind, WidgetState, menu_item};

pub(super) struct MenuPresentation {
    pub redmond98: bool,
    pub view: ViewMode,
    pub sort: SortKey,
    pub direction: rice_files::SortDirection,
    pub checked: MenuChecks,
    pub has_selection: bool,
    pub can_mutate_selection: bool,
    pub can_open_with: bool,
    pub selected_directory: bool,
    pub can_gzip: bool,
    pub can_extract: bool,
    pub can_publish: bool,
}

#[derive(Clone, Copy)]
pub(super) struct MenuChecks(u8);

impl MenuChecks {
    const PLACES: u8 = 1;
    const PREVIEW: u8 = 2;
    const DUAL_PANE: u8 = 4;
    const SIMPLIFIED_VIEW: u8 = 8;
    const FOLDER_TREE: u8 = 16;

    pub(super) const fn new(
        places: bool,
        preview: bool,
        dual_pane: bool,
        simplified_view: bool,
        folder_tree: bool,
    ) -> Self {
        let mut bits = 0;
        if places {
            bits |= Self::PLACES;
        }
        if preview {
            bits |= Self::PREVIEW;
        }
        if dual_pane {
            bits |= Self::DUAL_PANE;
        }
        if simplified_view {
            bits |= Self::SIMPLIFIED_VIEW;
        }
        if folder_tree {
            bits |= Self::FOLDER_TREE;
        }
        Self(bits)
    }

    const fn contains(self, flag: u8) -> bool {
        self.0 & flag == flag
    }
}

pub(super) fn menu_patches(patches: &mut Vec<Patch>, state: &MenuPresentation) {
    create(
        patches,
        MENU_BAR,
        Some(ROOT),
        WidgetKind::MenuBar,
        Some("panel:files-menu"),
        None,
    );
    for menu in MenuKind::all(state.redmond98) {
        create(
            patches,
            menu.widget(),
            Some(MENU_BAR),
            WidgetKind::Menu,
            Some(rice_i18n::t(menu.label())),
            None,
        );
        for entry in entries(menu) {
            menu_entry(
                patches,
                menu_widget(entry.action),
                Some(menu.widget()),
                entry,
                menu_state(entry, state),
            );
        }
    }
    create(
        patches,
        CONTEXT_BAR,
        Some(ROOT),
        WidgetKind::MenuBar,
        Some("context"),
        None,
    );
    create(
        patches,
        CONTEXT_MENU,
        Some(CONTEXT_BAR),
        WidgetKind::Menu,
        Some(rice_i18n::t("Item")),
        None,
    );
    for entry in context_entries() {
        if entry.action == super::files_menu::PUBLISH_MARKETPLACE && !state.can_publish {
            continue;
        }
        let selection_allowed = matches!(
            entry.action,
            super::files_menu::PASTE
                | super::files_menu::NEW_FOLDER
                | super::files_menu::OPEN_IN_TERMINAL
        ) || state.has_selection;
        menu_entry(
            patches,
            context_widget(entry.action),
            Some(CONTEXT_MENU),
            entry,
            if entry.implemented
                && selection_allowed
                && (entry.action != super::files_menu::OPEN_WITH || state.can_open_with)
                && (entry.action != super::files_menu::COMPRESS_GZIP || state.can_gzip)
                && (entry.action != super::files_menu::EXTRACT_HERE || state.can_extract)
                && (!matches!(
                    entry.action,
                    super::files_menu::CUT
                        | MOVE
                        | RENAME
                        | DELETE
                        | super::files_menu::DELETE_PERMANENTLY
                ) || state.can_mutate_selection)
                && (!matches!(
                    entry.action,
                    super::files_menu::OPEN_IN_NEW_TAB | super::files_menu::OPEN_IN_NEW_WINDOW
                ) || state.selected_directory)
                && (entry.action != super::files_menu::OPEN_IN_TERMINAL
                    || !state.has_selection
                    || state.selected_directory)
            {
                WidgetState::ENABLED
            } else {
                WidgetState::empty()
            },
        );
        if entry.action == COMPRESS_TO {
            create_with_state(
                patches,
                COMPRESS_SUBMENU,
                Some(context_widget(COMPRESS_TO)),
                WidgetKind::Menu,
                Some(rice_i18n::t("Compress to")),
                None,
                WidgetState::ENABLED,
            );
            for format in compress_entries() {
                menu_entry(
                    patches,
                    context_widget(format.action),
                    Some(COMPRESS_SUBMENU),
                    format,
                    if state.has_selection
                        && (format.action != super::files_menu::COMPRESS_GZIP || state.can_gzip)
                    {
                        WidgetState::ENABLED
                    } else {
                        WidgetState::empty()
                    },
                );
            }
        }
    }
}

fn menu_entry(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    entry: &MenuEntry,
    state: WidgetState,
) {
    let (kind, text) = match entry.kind {
        MenuEntryKind::Action => (WidgetKind::MenuItem, menu_item(rice_i18n::t(entry.label), entry.shortcut)),
        MenuEntryKind::Submenu => (WidgetKind::MenuItem, menu_item(rice_i18n::t(entry.label), entry.shortcut)),
        MenuEntryKind::Check => (WidgetKind::CheckBox, menu_item(rice_i18n::t(entry.label), entry.shortcut)),
        MenuEntryKind::Radio => (
            WidgetKind::RadioButton,
            menu_item(rice_i18n::t(entry.label), entry.shortcut),
        ),
        MenuEntryKind::Separator => (WidgetKind::MenuItem, String::from(MENU_SEPARATOR)),
    };
    let state = if entry.kind == MenuEntryKind::Submenu {
        state | WidgetState::TRAILING_ICON
    } else {
        state
    };
    create_with_state(patches, id, parent, kind, Some(&text), entry.icon, state);
}

fn menu_state(entry: &MenuEntry, state: &MenuPresentation) -> WidgetState {
    if !entry.implemented || entry.kind == MenuEntryKind::Separator {
        return WidgetState::empty();
    }
    let checked = match entry.action {
        PLACES_TOGGLE => state.checked.contains(MenuChecks::PLACES),
        PREVIEW_TOGGLE => state.checked.contains(MenuChecks::PREVIEW),
        SPLIT_TOGGLE => state.checked.contains(MenuChecks::DUAL_PANE),
        SIMPLIFIED_VIEW => state.checked.contains(MenuChecks::SIMPLIFIED_VIEW),
        FOLDER_TREE => state.checked.contains(MenuChecks::FOLDER_TREE),
        VIEW_ICONS => matches!(state.view, ViewMode::Icons | ViewMode::Thumbnails),
        VIEW_DETAILS => matches!(state.view, ViewMode::Compact | ViewMode::Details),
        super::files_menu::SORT_ASCENDING => state.direction == rice_files::SortDirection::Ascending,
        super::files_menu::SORT_DESCENDING => state.direction == rice_files::SortDirection::Descending,
        SORT_NAME => state.sort == SortKey::Name,
        SORT_MODIFIED => state.sort == SortKey::Modified,
        SORT_SIZE => state.sort == SortKey::Size,
        SORT_TYPE => state.sort == SortKey::Type,
        RENAME | MOVE | DELETE => state.can_mutate_selection,
        _ => false,
    };
    if checked {
        WidgetState::ENABLED | WidgetState::CHECKED
    } else {
        WidgetState::ENABLED
    }
}
