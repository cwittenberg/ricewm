//! Shared retained widget tree and semantic input adapter for file choosers.

use rice_native_app_protocol::{FileKind, RequestId, UiEvent};

use super::{ChooserEffect, FileChooser, FileChooserMode};
use crate::{
    IconId, Patch, ValueChange, WidgetId, WidgetKind, WidgetSpec, WidgetState, file_icon,
    folder_icon,
};

const BACK_OFFSET: u64 = 0;
const FORWARD_OFFSET: u64 = 1;
const UP_OFFSET: u64 = 2;
const LOCATION_OFFSET: u64 = 3;
const HIDDEN_OFFSET: u64 = 4;
const FILE_LIST_OFFSET: u64 = 5;
const FILENAME_OFFSET: u64 = 6;
const NOTICE_OFFSET: u64 = 7;
const OVERWRITE_OFFSET: u64 = 8;
const NAVIGATION_OFFSET: u64 = 10;
const BODY_OFFSET: u64 = 11;
const PLACES_OFFSET: u64 = 12;
const CONTENT_OFFSET: u64 = 13;
const COMPLETIONS_OFFSET: u64 = 14;
const OPTIONS_OFFSET: u64 = 15;
const ACTIONS_OFFSET: u64 = 16;
const BUSY_OFFSET: u64 = 17;
const PLACE_HEADING_OFFSET: u64 = 18;
const DIRECTORY_HEADING_OFFSET: u64 = 19;
const FILE_SCROLL_OFFSET: u64 = 20;
const FILTERS_OFFSET: u64 = 21;
const PLACES_LIST_OFFSET: u64 = 22;
const FILENAME_LABEL_OFFSET: u64 = 23;
const FILENAME_ROW_OFFSET: u64 = 24;
const FILTER_LABEL_OFFSET: u64 = 25;
const DETAILS_OFFSET: u64 = 26;
const DIRECTORY_ROW_OFFSET: u64 = 27;
const PREVIEWS_OFFSET: u64 = 28;
const PREVIEWS_TOOLTIP_OFFSET: u64 = 29;
const DETAILS_TOOLTIP_OFFSET: u64 = 30;
const DETAIL_HEADER_OFFSET: u64 = 31;
const DETAIL_NAME_HEADER_OFFSET: u64 = 32;
const DETAIL_SIZE_HEADER_OFFSET: u64 = 33;
const DETAIL_TYPE_HEADER_OFFSET: u64 = 34;
const DETAIL_MODIFIED_HEADER_OFFSET: u64 = 35;
const FILTER_MENU_OFFSET: u64 = 36;
const PLACE_OFFSET: u64 = 70;
const FILTER_OFFSET: u64 = 200;
const SUGGESTION_OFFSET: u64 = 300;
const ENTRY_OFFSET: u64 = 1_000;
const ENTRY_ROW_OFFSET: u64 = 5_500;
const ENTRY_CELL_OFFSET: u64 = 10_000;
const ENTRY_TYPE_OFFSET: u64 = 15_000;
const ENTRY_IMAGE_OFFSET: u64 = 20_000;
const MAX_RENDERED_ENTRIES: usize = 256;

/// Stable widget namespace supplied by each application embedding a chooser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileChooserViewIds {
    parent: u64,
    accept: u64,
    cancel: u64,
    base: u64,
}

impl FileChooserViewIds {
    #[must_use]
    pub const fn new(parent: u64, accept: u64, cancel: u64, base: u64) -> Self {
        Self {
            parent,
            accept,
            cancel,
            base,
        }
    }

    /// Current file format and the native popup containing its available choices.
    #[must_use]
    pub const fn filter_menu(self) -> u64 {
        self.base + FILTER_MENU_OFFSET
    }

    #[must_use]
    pub const fn parent(self) -> u64 {
        self.parent
    }

    #[must_use]
    pub const fn accept(self) -> u64 {
        self.accept
    }

    #[must_use]
    pub const fn cancel(self) -> u64 {
        self.cancel
    }

    #[must_use]
    pub const fn back(self) -> u64 {
        self.base + BACK_OFFSET
    }

    #[must_use]
    pub const fn forward(self) -> u64 {
        self.base + FORWARD_OFFSET
    }

    #[must_use]
    pub const fn up(self) -> u64 {
        self.base + UP_OFFSET
    }

    #[must_use]
    pub const fn location(self) -> u64 {
        self.base + LOCATION_OFFSET
    }

    #[must_use]
    pub const fn hidden(self) -> u64 {
        self.base + HIDDEN_OFFSET
    }

    #[must_use]
    pub const fn file_list(self) -> u64 {
        self.base + FILE_LIST_OFFSET
    }

    #[must_use]
    pub const fn filename(self) -> u64 {
        self.base + FILENAME_OFFSET
    }

    #[must_use]
    pub const fn notice(self) -> u64 {
        self.base + NOTICE_OFFSET
    }

    #[must_use]
    pub const fn overwrite(self) -> u64 {
        self.base + OVERWRITE_OFFSET
    }

    #[must_use]
    pub const fn details(self) -> u64 {
        self.base + DETAILS_OFFSET
    }

    #[must_use]
    pub const fn previews(self) -> u64 {
        self.base + PREVIEWS_OFFSET
    }

    #[must_use]
    pub const fn place(self, index: u64) -> u64 {
        self.base + PLACE_OFFSET + index
    }

    #[must_use]
    pub const fn filter(self, index: u64) -> u64 {
        self.base + FILTER_OFFSET + index
    }

    #[must_use]
    pub const fn suggestion(self, index: u64) -> u64 {
        self.base + SUGGESTION_OFFSET + index
    }

    #[must_use]
    pub const fn entry(self, index: u64) -> u64 {
        self.base + ENTRY_OFFSET + index
    }

    const fn navigation(self) -> u64 {
        self.base + NAVIGATION_OFFSET
    }

    const fn body(self) -> u64 {
        self.base + BODY_OFFSET
    }

    const fn places(self) -> u64 {
        self.base + PLACES_OFFSET
    }

    const fn content(self) -> u64 {
        self.base + CONTENT_OFFSET
    }

    const fn completions(self) -> u64 {
        self.base + COMPLETIONS_OFFSET
    }

    const fn options(self) -> u64 {
        self.base + OPTIONS_OFFSET
    }

    const fn actions(self) -> u64 {
        self.base + ACTIONS_OFFSET
    }

    const fn busy(self) -> u64 {
        self.base + BUSY_OFFSET
    }

    const fn place_heading(self) -> u64 {
        self.base + PLACE_HEADING_OFFSET
    }

    const fn directory_heading(self) -> u64 {
        self.base + DIRECTORY_HEADING_OFFSET
    }

    const fn directory_row(self) -> u64 {
        self.base + DIRECTORY_ROW_OFFSET
    }

    const fn file_scroll(self) -> u64 {
        self.base + FILE_SCROLL_OFFSET
    }

    const fn filters(self) -> u64 {
        self.base + FILTERS_OFFSET
    }

    const fn places_list(self) -> u64 {
        self.base + PLACES_LIST_OFFSET
    }

    const fn filename_label(self) -> u64 {
        self.base + FILENAME_LABEL_OFFSET
    }

    const fn filename_row(self) -> u64 {
        self.base + FILENAME_ROW_OFFSET
    }

    const fn filter_label(self) -> u64 {
        self.base + FILTER_LABEL_OFFSET
    }
}

/// Result of routing a native UI event through the shared chooser adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FileChooserEventResult {
    Unhandled,
    Cancel,
    Effect(ChooserEffect),
}

/// Composes one responsive chooser under the configured dialog parent.
pub fn compose_file_chooser(
    patches: &mut Vec<Patch>,
    chooser: &FileChooser,
    ids: FileChooserViewIds,
) {
    // File and save choosers need a workspace, not a compact prompt. The
    // expansive overlay tracks the containing window, so ordinary window
    // resize handles also expand and shrink the chooser.
    state(
        patches,
        ids.parent(),
        WidgetState::ENABLED | WidgetState::EXPANSIVE,
    );
    create(
        patches,
        ids.body(),
        Some(ids.parent()),
        WidgetKind::SplitPane,
        Some("split:210:min-width=140:max-width=220"),
        None,
    );
    places(patches, chooser, ids);
    create(
        patches,
        ids.content(),
        Some(ids.body()),
        WidgetKind::Column,
        Some("panel:tight-stack"),
        None,
    );
    navigation(patches, chooser, ids);
    completions(patches, chooser, ids);
    entries(patches, chooser, ids);
    options(patches, chooser, ids);
    if let Some(notice) = chooser.notice.as_ref().map(super::ChooserNotice::render) {
        create(
            patches,
            ids.notice(),
            Some(ids.content()),
            WidgetKind::Notification,
            Some(&notice),
            Some(if chooser.overwrite_path().is_some() {
                IconId::Warning
            } else {
                IconId::Error
            }),
        );
        state(
            patches,
            ids.notice(),
            WidgetState::ENABLED | WidgetState::INVALID,
        );
    }
    actions(patches, chooser, ids);
}

impl FileChooser {
    /// Routes pointer, focus, and keyboard events through the common chooser contract.
    #[allow(clippy::too_many_lines)]
    pub fn handle_ui_event(
        &mut self,
        ids: FileChooserViewIds,
        target: u64,
        event: &UiEvent,
        request: RequestId,
    ) -> FileChooserEventResult {
        // Native input uses X11 keycodes. Browser keyCode aliases collide
        // with ordinary keys (27 is R, 13 is 4), even while editing a name.
        let effect = match event {
            UiEvent::ValueChanged(value) if target == ids.location() => {
                self.edit_location(value);
                ChooserEffect::Render
            }
            UiEvent::ValueChanged(value) if target == ids.filename() => {
                self.edit_filename(value);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.cancel() => return FileChooserEventResult::Cancel,
            UiEvent::Activate if target == ids.parent() => return FileChooserEventResult::Cancel,
            UiEvent::Activate if target == ids.back() => self.navigate_back(request),
            UiEvent::Activate if target == ids.forward() => self.navigate_forward(request),
            UiEvent::Activate if target == ids.up() => self.navigate_up(request),
            UiEvent::Activate
            | UiEvent::Pointer {
                kind: rice_native_app_protocol::PointerEventKind::Up,
                button: 1,
                ..
            } if target == ids.hidden() => {
                self.toggle_hidden();
                ChooserEffect::Render
            }
            UiEvent::ValueChanged(value) if target == ids.hidden() => {
                let show_hidden = match value.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "checked" | "on" => true,
                    "0" | "false" | "unchecked" | "off" => false,
                    _ => !self.show_hidden(),
                };
                self.set_show_hidden(show_hidden);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.details() => {
                self.set_show_details(true);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.base + DETAIL_NAME_HEADER_OFFSET => {
                self.select_sort(super::ChooserSortKey::Name);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.base + DETAIL_SIZE_HEADER_OFFSET => {
                self.select_sort(super::ChooserSortKey::Size);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.base + DETAIL_TYPE_HEADER_OFFSET => {
                self.select_sort(super::ChooserSortKey::Type);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.base + DETAIL_MODIFIED_HEADER_OFFSET => {
                self.select_sort(super::ChooserSortKey::Modified);
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.previews() => {
                self.set_show_details(false);
                ChooserEffect::Render
            }
            UiEvent::ValueChanged(value) if target == ids.file_scroll() => {
                if let Some((position, maximum)) = value.split_once('/')
                    && let (Ok(position), Ok(maximum)) =
                        (position.trim().parse::<u32>(), maximum.trim().parse::<u32>())
                    && position <= maximum
                {
                    if self.visible_entries().len() > MAX_RENDERED_ENTRIES {
                        // Virtual windows still use the bounded nominal-row model.
                        self.set_scroll(position);
                    } else {
                        // Native layout measured the actual wrapped rows and
                        // viewport. A nominal 40px clamp can strand final rows.
                        self.scroll = position;
                    }
                }
                ChooserEffect::Render
            }
            UiEvent::Activate if target == ids.accept() => self.submit(request),
            UiEvent::Activate if target == ids.overwrite() => self.confirm_overwrite(),
            UiEvent::Pointer {
                kind: rice_native_app_protocol::PointerEventKind::Down,
                button: 1,
                ..
            } if index(target, ids.entry(0), self.visible_entries().len()).is_some() => {
                self.select_visible(
                    index(target, ids.entry(0), self.visible_entries().len()).unwrap_or_default(),
                );
                ChooserEffect::Render
            }
            UiEvent::Pointer {
                kind: rice_native_app_protocol::PointerEventKind::Down,
                button: 6,
                ..
            } if index(target, ids.entry(0), self.visible_entries().len()).is_some() => self
                .activate_visible(
                    index(target, ids.entry(0), self.visible_entries().len()).unwrap_or_default(),
                    request,
                ),
            // A row's ordinary mouse release is surfaced as Activate after
            // its Pointer::Down selection event.  Treating that release as
            // navigation made a single click open folders and also meant a
            // browser double-click opened them twice.  Real activation is
            // deliberately handled by button 6 (double-click) above and by
            // Enter in the keyboard arm below.
            UiEvent::Activate
                if index(target, ids.entry(0), self.visible_entries().len()).is_some() =>
            {
                ChooserEffect::Render
            }
            UiEvent::Activate
                if index(target, ids.suggestion(0), self.suggestions().len()).is_some() =>
            {
                self.activate_suggestion(
                    index(target, ids.suggestion(0), self.suggestions().len()).unwrap_or_default(),
                    request,
                )
            }
            UiEvent::Activate if index(target, ids.filter(0), self.filters().len()).is_some() => {
                self.select_filter(
                    index(target, ids.filter(0), self.filters().len()).unwrap_or_default(),
                );
                ChooserEffect::Render
            }
            UiEvent::Activate if index(target, ids.place(0), self.places().len()).is_some() => self
                .navigate_place(
                    index(target, ids.place(0), self.places().len()).unwrap_or_default(),
                    request,
                ),
            UiEvent::Activate if target == ids.location() || target == ids.filename() => {
                self.submit(request)
            }
            UiEvent::Focus { focused: true }
                if index(target, ids.entry(0), self.visible_entries().len()).is_some() =>
            {
                self.select_visible(
                    index(target, ids.entry(0), self.visible_entries().len()).unwrap_or_default(),
                );
                ChooserEffect::Render
            }
            UiEvent::Key {
                pressed: true,
                key_code: 36 | 104,
                ..
            } if index(target, ids.entry(0), self.visible_entries().len()).is_some() => self
                .activate_visible(
                    index(target, ids.entry(0), self.visible_entries().len()).unwrap_or_default(),
                    request,
                ),
            UiEvent::Key {
                pressed: true,
                key_code: 9,
                ..
            } => return FileChooserEventResult::Cancel,
            UiEvent::Key {
                pressed: true,
                key_code: 36 | 104,
                ..
            } => self.submit(request),
            UiEvent::Key {
                pressed: true,
                key_code: 111,
                ..
            } if target == ids.file_scroll()
                || target == ids.file_list()
                || index(target, ids.entry(0), self.visible_entries().len()).is_some() =>
            {
                self.move_selection(-1);
                ChooserEffect::Render
            }
            UiEvent::Key {
                pressed: true,
                key_code: 116,
                ..
            } if target == ids.file_scroll()
                || target == ids.file_list()
                || index(target, ids.entry(0), self.visible_entries().len()).is_some() =>
            {
                self.move_selection(1);
                ChooserEffect::Render
            }
            UiEvent::Key {
                pressed: true,
                key_code: 22,
                ..
            } if entry_surface_target(ids, target, self.visible_entries().len()) => {
                self.navigate_up(request)
            }
            UiEvent::Key {
                pressed: true,
                key_code: 113,
                ..
            } if target == ids.file_list() => self.navigate_back(request),
            UiEvent::Key {
                pressed: true,
                key_code: 114,
                ..
            } if target == ids.file_list() => self.navigate_forward(request),
            UiEvent::Pointer {
                kind: rice_native_app_protocol::PointerEventKind::Wheel,
                button,
                ..
            } if entry_surface_target(ids, target, self.visible_entries().len()) => {
                self.scroll_wheel(*button);
                ChooserEffect::Render
            }
            _ => return FileChooserEventResult::Unhandled,
        };
        FileChooserEventResult::Effect(effect)
    }
}

fn navigation(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.navigation(),
        Some(ids.content()),
        WidgetKind::ToolBar,
        None,
        None,
    );
    button(
        patches,
        ids.back(),
        ids.navigation(),
        rice_i18n::t("Back"),
        IconId::GoBack,
    );
    button(
        patches,
        ids.forward(),
        ids.navigation(),
        rice_i18n::t("Forward"),
        IconId::GoForward,
    );
    button(
        patches,
        ids.up(),
        ids.navigation(),
        rice_i18n::t("ui.parent-folder"),
        IconId::GoUp,
    );
    create(
        patches,
        ids.location(),
        Some(ids.navigation()),
        WidgetKind::TextField,
        Some(chooser.location_text()),
        None,
    );
    if !chooser.can_go_back() {
        state(patches, ids.back(), WidgetState::empty());
    }
    if !chooser.can_go_forward() {
        state(patches, ids.forward(), WidgetState::empty());
    }
}

fn places(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.places(),
        Some(ids.body()),
        WidgetKind::Container,
        None,
        None,
    );
    create(
        patches,
        ids.place_heading(),
        Some(ids.places()),
        WidgetKind::Label,
        Some(rice_i18n::t("Places")),
        None,
    );
    create(
        patches,
        ids.places_list(),
        Some(ids.places()),
        WidgetKind::List,
        None,
        None,
    );
    for (index, place) in chooser.places().iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        let id = ids.place(index);
        create(
            patches,
            id,
            Some(ids.places_list()),
            WidgetKind::Button,
            Some(place.display_label()),
            Some(place_icon(&place.path)),
        );
        if place_selected(chooser.directory(), &place.path) {
            state(patches, id, WidgetState::ENABLED | WidgetState::SELECTED);
        }
    }
}

fn completions(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    if chooser.suggestions().is_empty() {
        return;
    }
    create(
        patches,
        ids.completions(),
        Some(ids.content()),
        WidgetKind::Popover,
        Some(rice_i18n::t("Path suggestions")),
        Some(IconId::Search),
    );
    for (index, suggestion) in chooser.suggestions().iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        button(
            patches,
            ids.suggestion(index),
            ids.completions(),
            suggestion,
            IconId::FolderOpen,
        );
    }
}

#[allow(clippy::too_many_lines)]
fn entries(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.directory_row(),
        Some(ids.content()),
        WidgetKind::Row,
        Some("split:700:min-width=240:max-width=720:compact:dense"),
        None,
    );
    create(
        patches,
        ids.directory_heading(),
        Some(ids.directory_row()),
        WidgetKind::Label,
        Some(chooser.directory()),
        Some(folder_icon(chooser.directory())),
    );
    create(
        patches,
        ids.busy(),
        Some(ids.directory_row()),
        WidgetKind::Label,
        Some(if chooser.is_busy() {
            rice_i18n::t("Loading folder…")
        } else {
            ""
        }),
        None,
    );
    if chooser.is_busy() {
        state(
            patches,
            ids.busy(),
            WidgetState::ENABLED | WidgetState::BUSY | WidgetState::MUTED,
        );
    }
    create(
        patches,
        ids.previews(),
        Some(ids.directory_row()),
        WidgetKind::RadioButton,
        Some(""),
        Some(IconId::ViewGrid),
    );
    create(
        patches,
        ids.base + PREVIEWS_TOOLTIP_OFFSET,
        Some(ids.previews()),
        WidgetKind::Tooltip,
        Some(rice_i18n::t("Picture previews")),
        None,
    );
    create(
        patches,
        ids.details(),
        Some(ids.directory_row()),
        WidgetKind::RadioButton,
        Some(""),
        Some(IconId::ViewDetail),
    );
    create(
        patches,
        ids.base + DETAILS_TOOLTIP_OFFSET,
        Some(ids.details()),
        WidgetKind::Tooltip,
        Some(rice_i18n::t("Detailed list")),
        None,
    );
    if chooser.show_details() {
        state(
            patches,
            ids.details(),
            WidgetState::ENABLED | WidgetState::CHECKED,
        );
    } else {
        state(
            patches,
            ids.previews(),
            WidgetState::ENABLED | WidgetState::CHECKED,
        );
    }
    let item_height = if chooser.show_details() { 40 } else { 128 };
    let visible_entries = chooser.visible_entries();
    let total = u32::try_from(visible_entries.len())
        .unwrap_or(u32::MAX)
        .saturating_mul(item_height);
    let virtualized = visible_entries.len() > MAX_RENDERED_ENTRIES;
    let render_start = if virtualized {
        usize::try_from(chooser.scroll() / item_height)
            .unwrap_or(usize::MAX)
            .saturating_sub(4)
            .min(visible_entries.len().saturating_sub(MAX_RENDERED_ENTRIES))
    } else {
        0
    };
    let scroll_text = if virtualized {
        let virtual_offset = u32::try_from(render_start)
            .unwrap_or(u32::MAX)
            .saturating_mul(item_height);
        format!(
            "virtual-window:{virtual_offset}|{}/{total}",
            chooser.scroll()
        )
    } else {
        format!("{}/{total}", chooser.scroll())
    };
    create(
        patches,
        ids.file_scroll(),
        Some(ids.content()),
        WidgetKind::ScrollView,
        Some(&scroll_text),
        None,
    );
    create(
        patches,
        ids.file_list(),
        Some(ids.file_scroll()),
        if chooser.show_details() {
            WidgetKind::Table
        } else {
            WidgetKind::IconGrid
        },
        if chooser.show_details() {
            Some("table-columns:480,110,180,230:resizable:redmond98-details-view")
        } else {
            Some("file-grid:1000")
        },
        None,
    );
    if chooser.show_details() {
        detail_header(patches, chooser, ids);
    }
    for (index, entry) in visible_entries
        .into_iter()
        .enumerate()
        .skip(render_start)
        .take(MAX_RENDERED_ENTRIES)
    {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        let id = ids.entry(index);
        let parent = if chooser.show_details() {
            let row = ids.base + ENTRY_ROW_OFFSET + index;
            create(
                patches,
                row,
                Some(ids.file_list()),
                WidgetKind::Row,
                Some(&format!(
                    "split:480:control-for={id}:file-detail-row:{}",
                    if index % 2 == 0 {
                        "primary"
                    } else {
                        "alternate"
                    }
                )),
                None,
            );
            row
        } else if entry.kind != FileKind::Directory {
            let cell = ids.base + ENTRY_CELL_OFFSET + index;
            let image = ids.base + ENTRY_IMAGE_OFFSET + index;
            create(
                patches,
                cell,
                Some(ids.file_list()),
                WidgetKind::Container,
                Some(&format!("panel:thumbnail-cell:control-for={id}")),
                None,
            );
            create(
                patches,
                image,
                Some(cell),
                WidgetKind::Image,
                chooser
                    .thumbnail(&entry.name)
                    .map(|request| format!("media-request:{}", request.get()))
                    .as_deref(),
                chooser
                    .thumbnail(&entry.name)
                    .is_none()
                    .then(|| file_icon(&entry.name, entry.mime_type.as_deref())),
            );
            cell
        } else {
            ids.file_list()
        };
        button(
            patches,
            id,
            parent,
            &entry.name,
            if entry.kind == FileKind::Directory {
                folder_icon(&format!(
                    "{}/{}",
                    chooser.directory().trim_end_matches('/'),
                    entry.name
                ))
            } else {
                file_icon(&entry.name, entry.mime_type.as_deref())
            },
        );
        state(
            patches,
            id,
            WidgetState::ENABLED
                | WidgetState::KEY_NAVIGATION
                | if chooser.show_details() {
                    WidgetState::START_ALIGNED
                } else {
                    WidgetState::empty()
                },
        );
        if chooser.show_details() {
            create(
                patches,
                ids.base + ENTRY_CELL_OFFSET + index,
                Some(parent),
                WidgetKind::Label,
                Some(&readable_size(entry.kind, entry.size)),
                None,
            );
            create(
                patches,
                ids.base + ENTRY_TYPE_OFFSET + index,
                Some(parent),
                WidgetKind::Label,
                Some(&type_label(entry.kind, &entry.name)),
                None,
            );
            create(
                patches,
                ids.base + ENTRY_IMAGE_OFFSET + index,
                Some(parent),
                WidgetKind::Label,
                Some(&modified_label(entry.modified_millis)),
                None,
            );
        }
        if chooser.selected_name() == Some(entry.name.as_str()) {
            if !chooser.show_details() && entry.kind != FileKind::Directory {
                state(patches, parent, WidgetState::ENABLED | WidgetState::SELECTED);
            }
            state(
                patches,
                id,
                WidgetState::ENABLED
                    | WidgetState::SELECTED
                    | WidgetState::FOCUSED
                    | WidgetState::START_ALIGNED
                    | WidgetState::KEY_NAVIGATION,
            );
        }
    }
}

fn detail_header(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.base + DETAIL_HEADER_OFFSET,
        Some(ids.file_list()),
        WidgetKind::Row,
        Some("split:480:file-detail-header"),
        None,
    );
    for (offset, key, label) in [
        (
            DETAIL_NAME_HEADER_OFFSET,
            super::ChooserSortKey::Name,
            "Name",
        ),
        (
            DETAIL_SIZE_HEADER_OFFSET,
            super::ChooserSortKey::Size,
            "Size",
        ),
        (
            DETAIL_TYPE_HEADER_OFFSET,
            super::ChooserSortKey::Type,
            "Type",
        ),
        (
            DETAIL_MODIFIED_HEADER_OFFSET,
            super::ChooserSortKey::Modified,
            "Modified",
        ),
    ] {
        let label = chooser.sort_label(key, rice_i18n::t(label));
        create(
            patches,
            ids.base + offset,
            Some(ids.base + DETAIL_HEADER_OFFSET),
            WidgetKind::Button,
            Some(&label),
            None,
        );
        state(
            patches,
            ids.base + offset,
            WidgetState::ENABLED | WidgetState::START_ALIGNED | WidgetState::EMPHASIZED,
        );
    }
}

fn readable_size(kind: FileKind, bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if kind == FileKind::Directory {
        return String::from("—");
    }
    match bytes {
        0..KIB => format!("{bytes} B"),
        KIB..MIB => scaled_size(bytes, KIB, "KB"),
        MIB..GIB => scaled_size(bytes, MIB, "MB"),
        _ => scaled_size(bytes, GIB, "GB"),
    }
}

fn scaled_size(bytes: u64, unit: u64, suffix: &str) -> String {
    let whole = bytes / unit;
    let tenth = bytes % unit * 10 / unit;
    format!("{whole}.{tenth} {suffix}")
}

fn type_label(kind: FileKind, name: &str) -> String {
    match kind {
        FileKind::Directory => String::from(rice_i18n::t("File Folder")),
        FileKind::Symlink => String::from(rice_i18n::t("Shortcut")),
        FileKind::Other => String::from(rice_i18n::t("Object")),
        FileKind::File => {
            let extension = name
                .rsplit_once('.')
                .map(|(_, extension)| extension.to_ascii_uppercase())
                .filter(|extension| !extension.is_empty());
            match extension.as_deref() {
                Some("BMP") => String::from(rice_i18n::t("Bitmap Image")),
                Some("CAB") => String::from(rice_i18n::t("Cabinet")),
                Some("JPG" | "JPEG") => String::from(rice_i18n::t("JPEG Image")),
                Some("PNG") => String::from(rice_i18n::t("PNG Image")),
                Some("GIF") => String::from(rice_i18n::t("GIF Image")),
                Some("TXT") => String::from(rice_i18n::t("Text Document")),
                Some(extension) => rice_i18n::format("{extension} File", &[("extension", extension)]),
                None => String::from(rice_i18n::t("File")),
            }
        }
    }
}

fn modified_label(unix_ms: i64) -> String {
    let days = unix_ms.div_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    rice_i18n::format("files.modified-date", &[("month", month_name(month)), ("day", &day.to_string()), ("year", &year.to_string())])
}

// Howard Hinnant's civil-from-days mapping, adapted to Unix epoch days.
fn civil_from_days(unix_days: i64) -> (i64, u8, u8) {
    let days = unix_days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (
        year,
        u8::try_from(month).unwrap_or(1),
        u8::try_from(day).unwrap_or(1),
    )
}

fn month_name(month: u8) -> &'static str {
    match month {
        1 => rice_i18n::t("files.month.1"),
        2 => rice_i18n::t("files.month.2"),
        3 => rice_i18n::t("files.month.3"),
        4 => rice_i18n::t("files.month.4"),
        5 => rice_i18n::t("ui.may"),
        6 => rice_i18n::t("files.month.6"),
        7 => rice_i18n::t("files.month.7"),
        8 => rice_i18n::t("files.month.8"),
        9 => rice_i18n::t("files.month.9"),
        10 => rice_i18n::t("files.month.10"),
        11 => rice_i18n::t("files.month.11"),
        12 => rice_i18n::t("files.month.12"),
        _ => "—",
    }
}

fn options(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.options(),
        Some(ids.content()),
        WidgetKind::Column,
        Some("panel:tight-stack"),
        None,
    );
    if chooser.mode() == FileChooserMode::Save {
        create(
            patches,
            ids.filename_row(),
            Some(ids.options()),
            WidgetKind::Row,
            None,
            None,
        );
        create(
            patches,
            ids.filename_label(),
            Some(ids.filename_row()),
            WidgetKind::Label,
            Some(rice_i18n::t("Name")),
            None,
        );
        create(
            patches,
            ids.filename(),
            Some(ids.filename_row()),
            WidgetKind::TextField,
            Some(chooser.filename()),
            None,
        );
    }
    create(
        patches,
        ids.filters(),
        Some(ids.options()),
        WidgetKind::Row,
        Some("panel:compact:dense:form-control"),
        None,
    );
    create(
        patches,
        ids.filter_label(),
        Some(ids.filters()),
        WidgetKind::Label,
        Some(rice_i18n::t("File type")),
        None,
    );
    create(
        patches,
        ids.filter_menu(),
        Some(ids.filters()),
        WidgetKind::Menu,
        chooser.filters().get(chooser.filter_index()).map(|filter| filter.label()),
        Some(IconId::ChevronDown),
    );
    state(
        patches,
        ids.filter_menu(),
        WidgetState::ENABLED | WidgetState::EXPANSIVE
            | WidgetState::START_ALIGNED | WidgetState::TRAILING_ICON,
    );
    for (index, filter) in chooser.filters().iter().enumerate() {
        let Ok(index) = u64::try_from(index) else {
            break;
        };
        let id = ids.filter(index);
        create(
            patches,
            id,
            Some(ids.filter_menu()),
            WidgetKind::MenuItem,
            Some(filter.label()),
            None,
        );
        if usize::try_from(index).ok() == Some(chooser.filter_index()) {
            state(patches, id, WidgetState::ENABLED | WidgetState::CHECKED);
        }
    }
    create(
        patches,
        ids.hidden(),
        Some(ids.options()),
        WidgetKind::CheckBox,
        Some(rice_i18n::t("Show Hidden Files")),
        None,
    );
    if chooser.show_hidden() {
        state(
            patches,
            ids.hidden(),
            WidgetState::ENABLED | WidgetState::CHECKED,
        );
    }
}

fn actions(patches: &mut Vec<Patch>, chooser: &FileChooser, ids: FileChooserViewIds) {
    create(
        patches,
        ids.actions(),
        Some(ids.parent()),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    create(
        patches,
        ids.cancel(),
        Some(ids.actions()),
        WidgetKind::Button,
        Some(rice_i18n::t("Cancel")),
        None,
    );
    let (id, label, icon) = if chooser.overwrite_path().is_some() {
        (ids.overwrite(), "Replace", IconId::Warning)
    } else {
        match chooser.mode() {
            FileChooserMode::Open => (ids.accept(), "Open", IconId::DocumentOpen),
            FileChooserMode::SelectAny => (ids.accept(), "Select", IconId::SelectionCheck),
            FileChooserMode::SelectDirectory => (ids.accept(), "Select Folder", IconId::FolderOpen),
            FileChooserMode::Save => (ids.accept(), "Save", IconId::DocumentSave),
        }
    };
    button(patches, id, ids.actions(), rice_i18n::t(label), icon);
    let mut action_state = WidgetState::ENABLED | WidgetState::ACCENT;
    if id == ids.overwrite() {
        action_state = action_state | WidgetState::DESTRUCTIVE;
    }
    if chooser.is_busy() {
        action_state = WidgetState::empty();
    }
    state(patches, id, action_state);
}

fn place_icon(path: &str) -> IconId {
    if path == "/" {
        IconId::Drive
    } else {
        folder_icon(path)
    }
}

fn place_selected(directory: &str, place: &str) -> bool {
    directory == place
        || (place != "/root"
            && place != "/"
            && directory
                .strip_prefix(place)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

fn index(target: u64, start: u64, length: usize) -> Option<usize> {
    let index = usize::try_from(target.checked_sub(start)?).ok()?;
    (index < length).then_some(index)
}

fn entry_surface_target(ids: FileChooserViewIds, target: u64, length: usize) -> bool {
    target == ids.file_scroll()
        || target == ids.file_list()
        || index(target, ids.entry(0), length).is_some()
        || index(target, ids.base + ENTRY_CELL_OFFSET, length).is_some()
        || index(target, ids.base + ENTRY_IMAGE_OFFSET, length).is_some()
}

fn button(patches: &mut Vec<Patch>, id: u64, parent: u64, text: &str, icon: IconId) {
    create(
        patches,
        id,
        Some(parent),
        WidgetKind::Button,
        Some(text),
        Some(icon),
    );
}

fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
) {
    let parent = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| {
            matches!(
                patch,
                Patch::Create {
                    parent: candidate,
                    ..
                } if *candidate == parent
            )
        })
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state: WidgetState::ENABLED,
            text: text.map(str::to_owned),
            icon,
        },
        parent,
        index,
    });
}

fn state(patches: &mut Vec<Patch>, id: u64, state: WidgetState) {
    patches.push(Patch::Update {
        id: widget_id(id),
        state: Some(state),
        text: ValueChange::Keep,
        icon: ValueChange::Keep,
    });
}

fn widget_id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("file chooser widget IDs are nonzero")
}

#[cfg(test)]
mod tests {
    use rice_native_app_protocol::{
        DirectoryEntry, FileMetadata, RequestId, UiEvent, VfsResponse, VfsResult,
    };

    use super::*;
    use crate::RetainedTree;

    fn request(value: u64) -> RequestId {
        RequestId::new(value).expect("request")
    }

    fn loaded_chooser() -> FileChooser {
        loaded_chooser_with_entries(24)
    }

    fn loaded_chooser_with_entries(count: usize) -> FileChooser {
        let mut chooser = FileChooser::new(
            FileChooserMode::Save,
            "/root/note.txt",
            vec![
                super::super::FileFilter::new("Text", &["txt"]),
                super::super::FileFilter::all_files("All Files"),
            ],
            vec![
                super::super::Place::new("Home", "/root"),
                super::super::Place::new("Local Files", "/mnt/local"),
            ],
        );
        chooser.start(request(1));
        chooser.handle_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: (0..count)
                    .map(|index| DirectoryEntry {
                        name: format!("note-{index}.txt"),
                        metadata: FileMetadata {
                            kind: FileKind::File,
                            size: 1,
                            modified_millis: 0,
                            mode: 0o644,
                        },
                        mime_type: Some(String::from("text/plain")),
                    })
                    .collect(),
                next_cursor: None,
            },
        });
        chooser
    }

    #[test]
    fn catalog_looking_filenames_and_completions_stay_literal_in_every_locale() {
        let mut chooser = FileChooser::new(FileChooserMode::Open, "/root/", vec![
            super::super::FileFilter::all_files("common.save")
        ], vec![super::super::Place::new("Settings", "/root")]);
        chooser.start(request(1));
        chooser.handle_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: ["Settings", "Open", "common.save"].into_iter().map(|name| DirectoryEntry {
                    name: name.to_owned(),
                    metadata: FileMetadata { kind: FileKind::File, size: 1, modified_millis: 0, mode: 0o644 },
                    mime_type: None,
                }).collect(),
                next_cursor: None,
            },
        });
        chooser.edit_location("/root/");
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        assert!(!rice_i18n::LOCALE_CODES.is_empty());
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                let mut patches = Vec::new();
                compose_file_chooser(&mut patches, &chooser, ids);
                let label = |id| patches.iter().find_map(|patch| match patch {
                    Patch::Create { widget, .. } if widget.id == widget_id(id) => widget.text.as_deref(),
                    _ => None,
                }).unwrap();
                for (index, entry) in chooser.visible_entries().iter().enumerate() {
                    assert_eq!(label(ids.entry(index as u64)), entry.name, "locale {locale}");
                    let mut selected = chooser.clone();
                    assert_eq!(selected.handle_ui_event(ids, ids.entry(index as u64), &UiEvent::Pointer {
                        kind: rice_native_app_protocol::PointerEventKind::Down, x: 0, y: 0, button: 1, modifiers: 0,
                    }, request(2)), FileChooserEventResult::Effect(ChooserEffect::Render));
                    assert_eq!(selected.selected_name(), Some(entry.name.as_str()));
                    assert_eq!(selected.handle_ui_event(ids, ids.accept(), &UiEvent::Activate, request(3)),
                        FileChooserEventResult::Effect(ChooserEffect::Accepted(format!("/root/{}", entry.name))));
                }
                assert_eq!(chooser.suggestions().len(), 3);
                for (index, path) in chooser.suggestions().iter().enumerate() {
                    assert_eq!(label(ids.suggestion(index as u64)), path);
                }
                assert_eq!(label(ids.place(0)), "Settings");
                assert_eq!(label(ids.filter(0)), "common.save");
                for (id, english) in [(ids.back(), "Back"), (ids.forward(), "Forward"),
                    (ids.up(), "ui.parent-folder"), (ids.accept(), "Open")] {
                    assert_eq!(label(id), rice_i18n::t(english));
                }
            });
        }
    }

    #[test]
    fn ordinary_click_selects_a_folder_and_only_double_click_navigates() {
        let mut chooser = FileChooser::new(
            FileChooserMode::SelectAny,
            "/root/",
            vec![super::super::FileFilter::all_files("All Files and Folders")],
            vec![],
        );
        chooser.start(request(1));
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
        });
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.entry(0),
                &UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Down,
                    x: 0,
                    y: 0,
                    button: 1,
                    modifiers: 0,
                },
                request(2),
            ),
            FileChooserEventResult::Effect(ChooserEffect::Render),
        );
        assert_eq!(
            chooser.handle_ui_event(ids, ids.entry(0), &UiEvent::Activate, request(2),),
            FileChooserEventResult::Effect(ChooserEffect::Render),
        );
        assert_eq!(chooser.directory(), "/root");
        assert_eq!(
            chooser.handle_ui_event(ids, ids.accept(), &UiEvent::Activate, request(3)),
            FileChooserEventResult::Effect(ChooserEffect::Accepted(String::from(
                "/root/Documents",
            ))),
        );
    }

    #[test]
    fn detail_metadata_rerenders_in_spanish_and_chinese_without_changing_files() {
        let chooser = loaded_chooser();
        let before = chooser.entries.clone();
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        for (locale, date) in [(1, "1 ene 1970"), (2, "1970年1月1日")] {
            rice_i18n::with_locale(locale, || {
                let mut patches = Vec::new(); compose_file_chooser(&mut patches, &chooser, ids);
                assert!(patches.iter().any(|patch| matches!(patch, Patch::Create {widget, ..} if widget.text.as_deref() == Some(date))));
                assert_ne!(type_label(FileKind::File, "note.txt"), "Text Document");
                assert!(type_label(FileKind::File, "note.{kind}").contains("{KIND}"));
                assert_ne!(type_label(FileKind::Directory, "Documents"), "File Folder");
                assert_eq!(modified_label(0), date);
            });
            assert_eq!(chooser.entries, before);
        }
    }

    #[test]
    fn shared_composition_has_distinct_headings_scroll_surface_and_action_footer() {
        let chooser = loaded_chooser();
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        let mut patches = vec![Patch::Create {
            widget: WidgetSpec {
                id: widget_id(200),
                kind: WidgetKind::Dialog,
                state: WidgetState::ENABLED,
                text: Some(String::from("Save Document As")),
                icon: Some(IconId::DocumentSave),
            },
            parent: None,
            index: 0,
        }];
        compose_file_chooser(&mut patches, &chooser, ids);
        let mut tree = RetainedTree::default();
        tree.apply(&patches).expect("shared chooser tree");
        assert_eq!(
            tree.get(widget_id(ids.file_list()))
                .and_then(|widget| widget.parent),
            Some(widget_id(ids.file_scroll()))
        );
        assert_eq!(
            tree.get(widget_id(ids.navigation()))
                .and_then(|widget| widget.parent),
            Some(widget_id(ids.content()))
        );
        assert_eq!(
            tree.get(widget_id(ids.body()))
                .and_then(|widget| widget.children().first().copied()),
            Some(widget_id(ids.places()))
        );
        assert_eq!(
            tree.get(widget_id(ids.place_heading()))
                .and_then(|widget| widget.text.as_deref()),
            Some("Places")
        );
        assert_eq!(
            tree.get(widget_id(ids.actions()))
                .and_then(|widget| widget.text.as_deref()),
            Some("panel:actions")
        );
        assert_eq!(
            tree.get(widget_id(ids.filename()))
                .and_then(|widget| widget.parent),
            Some(widget_id(ids.filename_row()))
        );
        assert!(tree.get(widget_id(ids.parent())).is_some_and(|widget| {
            widget
                .state
                .contains(WidgetState::ENABLED | WidgetState::EXPANSIVE)
        }));
        assert!(
            tree.get(widget_id(ids.details()))
                .is_some_and(|widget| widget.state.contains(WidgetState::CHECKED))
        );
        assert!(
            tree.get(widget_id(ids.entry(0)))
                .and_then(|widget| widget.text.as_deref())
                .is_some_and(|text| text == "note-0.txt")
        );
        assert_eq!(
            tree.get(widget_id(ids.base + DETAIL_NAME_HEADER_OFFSET))
                .and_then(|widget| widget.text.as_deref()),
            Some("Name ↑")
        );
        assert_eq!(
            tree.get(widget_id(ids.base + DETAIL_NAME_HEADER_OFFSET))
                .map(|widget| widget.kind),
            Some(WidgetKind::Button)
        );
        assert_eq!(
            tree.get(widget_id(ids.base + ENTRY_CELL_OFFSET))
                .and_then(|widget| widget.text.as_deref()),
            Some("1 B")
        );
        assert_eq!(
            tree.get(widget_id(ids.base + ENTRY_TYPE_OFFSET))
                .and_then(|widget| widget.text.as_deref()),
            Some("Text Document")
        );
        assert_eq!(
            tree.get(widget_id(ids.base + ENTRY_IMAGE_OFFSET))
                .and_then(|widget| widget.text.as_deref()),
            Some("Jan 1, 1970")
        );
        assert_eq!(
            tree.get(widget_id(ids.options()))
                .and_then(|widget| widget.text.as_deref()),
            Some("panel:tight-stack")
        );
        assert_eq!(
            tree.get(widget_id(ids.filter(0)))
                .and_then(|widget| widget.text.as_deref()),
            Some("Text")
        );
        assert_eq!(
            tree.get(widget_id(ids.filter(1)))
                .and_then(|widget| widget.text.as_deref()),
            Some("All Files")
        );
        assert_eq!(
            tree.get(widget_id(ids.busy()))
                .and_then(|widget| widget.parent),
            Some(widget_id(ids.directory_row()))
        );
    }

    #[test]
    fn detail_headers_sort_rows_and_toggle_the_active_direction() {
        let mut chooser = FileChooser::new(
            FileChooserMode::Open,
            "/root/",
            vec![super::super::FileFilter::all_files("All Files")],
            vec![],
        );
        chooser.start(request(1));
        chooser.handle_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![
                    DirectoryEntry {
                        name: String::from("large.png"),
                        metadata: FileMetadata {
                            kind: FileKind::File,
                            size: 9_000,
                            modified_millis: 20,
                            mode: 0o644,
                        },
                        mime_type: Some(String::from("image/png")),
                    },
                    DirectoryEntry {
                        name: String::from("small.txt"),
                        metadata: FileMetadata {
                            kind: FileKind::File,
                            size: 10,
                            modified_millis: 10,
                            mode: 0o644,
                        },
                        mime_type: Some(String::from("text/plain")),
                    },
                ],
                next_cursor: None,
            },
        });
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.base + DETAIL_SIZE_HEADER_OFFSET,
                &UiEvent::Activate,
                request(2),
            ),
            FileChooserEventResult::Effect(ChooserEffect::Render)
        );
        assert_eq!(
            chooser
                .visible_entries()
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["small.txt", "large.png"]
        );
        assert_eq!(
            chooser.sort_label(super::super::ChooserSortKey::Size, "Size"),
            "Size ↑"
        );
        chooser.handle_ui_event(
            ids,
            ids.base + DETAIL_SIZE_HEADER_OFFSET,
            &UiEvent::Activate,
            request(3),
        );
        assert_eq!(
            chooser
                .visible_entries()
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["large.png", "small.txt"]
        );
        assert_eq!(
            chooser.sort_label(super::super::ChooserSortKey::Size, "Size"),
            "Size ↓"
        );
    }

    #[test]
    fn filename_keys_do_not_cancel_or_submit_the_shared_chooser() {
        let mut chooser = loaded_chooser();
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        for (key_code, text) in [(27, "r"), (13, "4")] {
            assert_eq!(
                chooser.handle_ui_event(
                    ids,
                    ids.filename(),
                    &UiEvent::Key {
                        pressed: true,
                        key_code,
                        modifiers: 0,
                        text: Some(text.into()),
                    },
                    request(2)
                ),
                FileChooserEventResult::Unhandled
            );
            assert_eq!(chooser.pending_request(), None);
        }
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.filename(),
                &UiEvent::Key {
                    pressed: true,
                    key_code: 9,
                    modifiers: 0,
                    text: None,
                },
                request(3)
            ),
            FileChooserEventResult::Cancel
        );
    }

    #[test]
    fn geometry_scroll_accepts_measured_range_and_rejects_malformed_bounds() {
        let mut chooser = loaded_chooser();
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        chooser.handle_ui_event(ids, ids.file_scroll(), &UiEvent::ValueChanged("520/600".into()), request(2));
        assert_eq!(chooser.scroll(), 520, "wrapped rows exceed the nominal row extent");
        for invalid in ["521", "521/no", "601/600", "-1/600", "1/2/3", "4294967296/4294967296"] {
            chooser.handle_ui_event(ids, ids.file_scroll(), &UiEvent::ValueChanged(invalid.into()), request(2));
            assert_eq!(chooser.scroll(), 520, "{invalid}");
        }
        chooser.handle_ui_event(ids, ids.file_scroll(), &UiEvent::ValueChanged("0/0".into()), request(2));
        assert_eq!(chooser.scroll(), 0);
    }

    #[test]
    fn shared_event_router_bounds_dynamic_ids_and_scrolls_entries() {
        let mut chooser = loaded_chooser();
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        assert_eq!(
            chooser.handle_ui_event(ids, u64::MAX, &UiEvent::Activate, request(2)),
            FileChooserEventResult::Unhandled
        );
        assert!(chooser.show_details());
        assert!(!chooser.show_hidden());
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.hidden(),
                &UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Up,
                    x: 0,
                    y: 0,
                    button: 1,
                    modifiers: 0,
                },
                request(2),
            ),
            FileChooserEventResult::Effect(ChooserEffect::Render)
        );
        assert!(chooser.show_hidden());
        let mut hidden_patches = Vec::new();
        compose_file_chooser(&mut hidden_patches, &chooser, ids);
        assert!(hidden_patches.iter().any(|patch| matches!(
            patch,
            Patch::Update {
                id,
                state: Some(state),
                ..
            } if *id == widget_id(ids.hidden()) && state.contains(WidgetState::CHECKED)
        )));
        assert_eq!(
            chooser.handle_ui_event(ids, ids.previews(), &UiEvent::Activate, request(2)),
            FileChooserEventResult::Effect(ChooserEffect::Render)
        );
        assert!(!chooser.show_details());
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.file_scroll(),
                &UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Wheel,
                    x: 0,
                    y: 0,
                    button: 5,
                    modifiers: 0,
                },
                request(3),
            ),
            FileChooserEventResult::Effect(ChooserEffect::Render)
        );
        assert!(chooser.scroll() > 0);
        assert_eq!(
            chooser.handle_ui_event(
                ids,
                ids.cancel(),
                &UiEvent::Key {
                    pressed: true,
                    key_code: 9,
                    modifiers: 0,
                    text: None,
                },
                request(4),
            ),
            FileChooserEventResult::Cancel
        );
    }

    #[test]
    fn detail_labels_match_ricefiles_human_readable_metadata() {
        assert_eq!(readable_size(FileKind::Directory, u64::MAX), "—");
        assert_eq!(readable_size(FileKind::File, 1_536), "1.5 KB");
        assert_eq!(type_label(FileKind::Directory, "Documents"), "File Folder");
        assert_eq!(type_label(FileKind::File, "photo.png"), "PNG Image");
        assert_eq!(modified_label(1_704_067_200_000), "Jan 1, 2024");
    }

    #[test]
    fn large_detail_folders_render_a_bounded_window_at_the_scroll_position() {
        let mut chooser = loaded_chooser_with_entries(1_000);
        chooser.set_scroll(40_000);
        let ids = FileChooserViewIds::new(200, 204, 208, 230);
        let mut patches = Vec::new();
        compose_file_chooser(&mut patches, &chooser, ids);

        assert!(patches.iter().any(|patch| matches!(
            patch,
            Patch::Create { widget, .. }
                if widget.id == widget_id(ids.file_scroll())
                    && widget.text.as_deref().is_some_and(|text| {
                        text.starts_with("virtual-window:29760|39960/40000")
                    })
        )));
        assert!(patches.iter().any(|patch| matches!(
            patch,
            Patch::Create { widget, .. } if widget.id == widget_id(ids.entry(744))
        )));
        assert!(!patches.iter().any(|patch| matches!(
            patch,
            Patch::Create { widget, .. } if widget.id == widget_id(ids.entry(0))
        )));
        assert!(
            patches
                .iter()
                .filter(|patch| matches!(patch, Patch::Create { .. }))
                .count()
                < 1_500
        );
    }
}
