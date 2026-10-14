//! Semantic detail-table rows and sortable column headers.

use rice_files::{EntryMetadata, SortDirection, SortKey, SortStrategy};
use rice_ui::{Patch, WidgetKind, WidgetState};

use super::{
    FilesActorError, files_details,
    files_view::{
        DETAIL_DISCLOSURE_OFFSET, DETAIL_EMPTY_LABEL_OFFSET, DETAIL_EMPTY_ROW_OFFSET,
        DETAIL_NAME_CELL_OFFSET, PaneWidgetIds, create, create_with_state, entry_icon,
    },
};

pub(super) fn header_patches(
    patches: &mut Vec<Patch>,
    ids: PaneWidgetIds,
    sort: SortStrategy,
    redmond98: bool,
) {
    create(
        patches,
        ids.detail_header,
        Some(ids.directory),
        WidgetKind::Row,
        Some(if redmond98 {
            "split:440:file-detail-header:fixed-height:18:dense-row"
        } else {
            "split:600:file-detail-header"
        }),
        None,
    );
    create_with_state(
        patches,
        ids.detail_name,
        Some(ids.detail_header),
        WidgetKind::Button,
        Some(&header_label(
            rice_i18n::t("common.name"),
            SortKey::Name,
            sort,
        )),
        None,
        WidgetState::ENABLED | WidgetState::START_ALIGNED,
    );
    create(
        patches,
        ids.detail_meta_header,
        Some(ids.detail_header),
        WidgetKind::Row,
        Some(if redmond98 { "split:280" } else { "split:500" }),
        None,
    );
    let columns = if redmond98 {
        vec![
            (ids.detail_size, rice_i18n::t("common.size"), SortKey::Size),
            (ids.detail_type, rice_i18n::t("common.type"), SortKey::Type),
            (
                ids.detail_modified,
                rice_i18n::t("common.modified"),
                SortKey::Modified,
            ),
        ]
    } else {
        vec![
            (ids.detail_size, rice_i18n::t("common.size"), SortKey::Size),
            (
                ids.detail_modified,
                rice_i18n::t("common.modified"),
                SortKey::Modified,
            ),
        ]
    };
    for (id, label, key) in columns {
        create(
            patches,
            id,
            Some(ids.detail_meta_header),
            WidgetKind::Button,
            Some(&header_label(label, key, sort)),
            None,
        );
    }
}

pub(super) fn entry_patches(
    patches: &mut Vec<Patch>,
    ids: PaneWidgetIds,
    index: usize,
    entry: &EntryMetadata,
    selected: &[&rice_files::GuestPath],
    location: &str,
    expanded: bool,
    focused: bool,
    redmond98: bool,
) -> Result<(), FilesActorError> {
    let index = u64::try_from(index).map_err(|_| FilesActorError::EntryLimit)?;
    let entry_id = ids.entry_base + index;
    let stripe = if index % 2 == 0 {
        "primary"
    } else {
        "alternate"
    };
    let row_layout = if redmond98 {
        format!(
            "split:440:control-for={entry_id}:file-detail-row:{stripe}:fixed-height:18:dense-row"
        )
    } else {
        format!("split:600:control-for={entry_id}:file-detail-row:{stripe}")
    };
    create(
        patches,
        ids.detail_row_base + index,
        Some(ids.directory),
        WidgetKind::Row,
        Some(&row_layout),
        None,
    );
    let name_cell = ids
        .entry_base
        .saturating_add(DETAIL_NAME_CELL_OFFSET)
        .saturating_add(index);
    create(
        patches,
        name_cell,
        Some(ids.detail_row_base + index),
        WidgetKind::Row,
        Some("panel:distribute:dense-table:detail-name-cell"),
        None,
    );
    let disclosure = ids
        .entry_base
        .saturating_add(DETAIL_DISCLOSURE_OFFSET)
        .saturating_add(index);
    if entry.kind == rice_files::EntryKind::Directory {
        create_with_state(
            patches,
            disclosure,
            Some(name_cell),
            WidgetKind::Button,
            Some("panel:equal-fixed:fixed-width:18:detail-disclosure"),
            Some(if expanded {
                rice_ui::IconId::ChevronDown
            } else {
                rice_ui::IconId::GoForward
            }),
            WidgetState::ENABLED
                | WidgetState::FLAT
                | if expanded {
                    WidgetState::CHECKED
                } else {
                    WidgetState::empty()
                },
        );
    } else {
        create(
            patches,
            disclosure,
            Some(name_cell),
            WidgetKind::Container,
            Some("panel:equal-fixed:fixed-width:18:detail-disclosure-spacer"),
            None,
        );
    }
    create_with_state(
        patches,
        entry_id,
        Some(name_cell),
        WidgetKind::Button,
        Some(&files_details::name_label(entry, location)),
        Some(entry_icon(entry)),
        if selected.contains(&&entry.path) {
            WidgetState::ENABLED
                | WidgetState::SELECTED
                | WidgetState::START_ALIGNED
                | WidgetState::KEY_NAVIGATION
                | if focused {
                    WidgetState::FOCUSED
                } else {
                    WidgetState::empty()
                }
                | if entry.name.starts_with('.') {
                    WidgetState::MUTED
                } else {
                    WidgetState::empty()
                }
        } else {
            WidgetState::ENABLED
                | WidgetState::START_ALIGNED
                | WidgetState::KEY_NAVIGATION
                | if focused {
                    WidgetState::FOCUSED
                } else {
                    WidgetState::empty()
                }
                | if entry.name.starts_with('.') {
                    WidgetState::MUTED
                } else {
                    WidgetState::empty()
                }
        },
    );
    create(
        patches,
        ids.detail_meta_base + index,
        Some(ids.detail_row_base + index),
        WidgetKind::Row,
        Some(if redmond98 { "split:280" } else { "split:500" }),
        None,
    );
    create(
        patches,
        ids.detail_size_base + index,
        Some(ids.detail_meta_base + index),
        WidgetKind::Label,
        Some(&files_details::size_label(entry)),
        None,
    );
    if redmond98 {
        create(
            patches,
            ids.detail_type_base + index,
            Some(ids.detail_meta_base + index),
            WidgetKind::Label,
            Some(&files_details::type_label(entry)),
            None,
        );
    }
    create(
        patches,
        ids.detail_modified_base + index,
        Some(ids.detail_meta_base + index),
        WidgetKind::Label,
        Some(&files_details::modified_label(entry.modified_unix_ms)),
        None,
    );
    Ok(())
}

pub(super) fn empty_directory_patches(
    patches: &mut Vec<Patch>,
    ids: PaneWidgetIds,
    index: usize,
    entry: &EntryMetadata,
    location: &str,
    redmond98: bool,
) -> Result<(), FilesActorError> {
    let index = u64::try_from(index).map_err(|_| FilesActorError::EntryLimit)?;
    let row = ids
        .detail_row_base
        .saturating_add(DETAIL_EMPTY_ROW_OFFSET)
        .saturating_add(index);
    create(
        patches,
        row,
        Some(ids.directory),
        WidgetKind::Row,
        Some(if redmond98 {
            "panel:file-detail-empty-row:fixed-height:18:dense-row"
        } else {
            "panel:file-detail-empty-row"
        }),
        None,
    );
    create_with_state(
        patches,
        ids.entry_base
            .saturating_add(DETAIL_EMPTY_LABEL_OFFSET)
            .saturating_add(index),
        Some(row),
        WidgetKind::Label,
        Some(&files_details::empty_directory_label(entry, location)),
        Some(rice_ui::IconId::FolderOpen),
        WidgetState::ENABLED | WidgetState::MUTED | WidgetState::START_ALIGNED,
    );
    Ok(())
}

fn header_label(label: &str, key: SortKey, sort: SortStrategy) -> String {
    if sort.key != key {
        return label.to_owned();
    }
    let arrow = match sort.direction {
        SortDirection::Ascending => " ↑",
        SortDirection::Descending => " ↓",
    };
    format!("{label}{arrow}")
}
