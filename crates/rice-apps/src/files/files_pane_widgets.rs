//! Stable, disjoint widget namespaces for independently interactive file panes.

use rice_files::PaneSlot;

use super::files_view::{
    CONTENT_NOTICE, DETAIL_HEADER, DETAIL_META_BASE, DETAIL_META_HEADER, DETAIL_MODIFIED,
    DETAIL_MODIFIED_BASE, DETAIL_NAME, DETAIL_ROW_BASE, DETAIL_SIZE, DETAIL_SIZE_BASE, DETAIL_TYPE,
    DETAIL_TYPE_BASE, DIRECTORY, DIRECTORY_SCROLL, ENTRY_BASE, PRIMARY_PANE,
    SECONDARY_CONTENT_NOTICE, SECONDARY_DIRECTORY, SECONDARY_DIRECTORY_SCROLL,
    SECONDARY_ENTRY_BASE, SECONDARY_PANE, SORT_MODIFIED, SORT_NAME, SORT_SIZE, SORT_TYPE,
};

const PRIMARY_LOCATION_LABEL: u64 = 131;
const SECONDARY_LOCATION_LABEL: u64 = 132;
const SECONDARY_DETAIL_ROW_BASE: u64 = 110_000;
const SECONDARY_DETAIL_META_BASE: u64 = 120_000;
const SECONDARY_DETAIL_SIZE_BASE: u64 = 130_000;
const SECONDARY_DETAIL_MODIFIED_BASE: u64 = 140_000;
const SECONDARY_DETAIL_TYPE_BASE: u64 = 160_000;
const SECONDARY_DETAIL_HEADER: u64 = 150_000;
const SECONDARY_DETAIL_NAME: u64 = 150_001;
const SECONDARY_DETAIL_META_HEADER: u64 = 150_002;
const SECONDARY_DETAIL_SIZE: u64 = 150_003;
const SECONDARY_DETAIL_MODIFIED: u64 = 150_004;
const SECONDARY_DETAIL_TYPE: u64 = 150_005;
pub(super) const THUMBNAIL_CELL_OFFSET: u64 = 500_000;
pub(super) const THUMBNAIL_IMAGE_OFFSET: u64 = 600_000;
pub(super) const DETAIL_DISCLOSURE_OFFSET: u64 = 700_000;
pub(super) const DETAIL_NAME_CELL_OFFSET: u64 = 800_000;
pub(super) const DETAIL_EMPTY_ROW_OFFSET: u64 = 900_000;
pub(super) const DETAIL_EMPTY_LABEL_OFFSET: u64 = 1_000_000;

#[derive(Clone, Copy)]
pub(super) struct PaneWidgetIds {
    pub pane: u64,
    pub location: u64,
    pub scroll: u64,
    pub directory: u64,
    pub notice: u64,
    pub entry_base: u64,
    pub detail_header: u64,
    pub detail_name: u64,
    pub detail_meta_header: u64,
    pub detail_size: u64,
    pub detail_modified: u64,
    pub detail_type: u64,
    pub detail_row_base: u64,
    pub detail_meta_base: u64,
    pub detail_size_base: u64,
    pub detail_modified_base: u64,
    pub detail_type_base: u64,
}

pub(super) const fn pane_ids(slot: PaneSlot) -> PaneWidgetIds {
    match slot {
        PaneSlot::Primary => PaneWidgetIds {
            pane: PRIMARY_PANE,
            location: PRIMARY_LOCATION_LABEL,
            scroll: DIRECTORY_SCROLL,
            directory: DIRECTORY,
            notice: CONTENT_NOTICE,
            entry_base: ENTRY_BASE,
            detail_header: DETAIL_HEADER,
            detail_name: DETAIL_NAME,
            detail_meta_header: DETAIL_META_HEADER,
            detail_size: DETAIL_SIZE,
            detail_modified: DETAIL_MODIFIED,
            detail_type: DETAIL_TYPE,
            detail_row_base: DETAIL_ROW_BASE,
            detail_meta_base: DETAIL_META_BASE,
            detail_size_base: DETAIL_SIZE_BASE,
            detail_modified_base: DETAIL_MODIFIED_BASE,
            detail_type_base: DETAIL_TYPE_BASE,
        },
        PaneSlot::Secondary => PaneWidgetIds {
            pane: SECONDARY_PANE,
            location: SECONDARY_LOCATION_LABEL,
            scroll: SECONDARY_DIRECTORY_SCROLL,
            directory: SECONDARY_DIRECTORY,
            notice: SECONDARY_CONTENT_NOTICE,
            entry_base: SECONDARY_ENTRY_BASE,
            detail_header: SECONDARY_DETAIL_HEADER,
            detail_name: SECONDARY_DETAIL_NAME,
            detail_meta_header: SECONDARY_DETAIL_META_HEADER,
            detail_size: SECONDARY_DETAIL_SIZE,
            detail_modified: SECONDARY_DETAIL_MODIFIED,
            detail_type: SECONDARY_DETAIL_TYPE,
            detail_row_base: SECONDARY_DETAIL_ROW_BASE,
            detail_meta_base: SECONDARY_DETAIL_META_BASE,
            detail_size_base: SECONDARY_DETAIL_SIZE_BASE,
            detail_modified_base: SECONDARY_DETAIL_MODIFIED_BASE,
            detail_type_base: SECONDARY_DETAIL_TYPE_BASE,
        },
    }
}

pub(super) fn pane_for_widget(target: u64) -> Option<PaneSlot> {
    [PaneSlot::Primary, PaneSlot::Secondary]
        .into_iter()
        .find(|slot| belongs_to(target, pane_ids(*slot)))
}

pub(super) fn selection_background(target: u64) -> Option<PaneSlot> {
    [PaneSlot::Primary, PaneSlot::Secondary]
        .into_iter()
        .find(|slot| {
            let ids = pane_ids(*slot);
            matches!(target, value if value == ids.scroll || value == ids.directory)
        })
}

fn belongs_to(target: u64, ids: PaneWidgetIds) -> bool {
    [
        ids.pane,
        ids.location,
        ids.scroll,
        ids.directory,
        ids.notice,
        ids.detail_header,
        ids.detail_name,
        ids.detail_meta_header,
        ids.detail_size,
        ids.detail_modified,
        ids.detail_type,
    ]
    .contains(&target)
        || in_entry_range(target, ids.entry_base)
        || in_entry_range(target, ids.detail_row_base)
        || in_entry_range(target, ids.detail_meta_base)
        || in_entry_range(target, ids.detail_size_base)
        || in_entry_range(target, ids.detail_modified_base)
        || in_entry_range(target, ids.detail_type_base)
        || in_entry_range(
            target,
            ids.entry_base.saturating_add(DETAIL_DISCLOSURE_OFFSET),
        )
        || in_entry_range(target, ids.entry_base.saturating_add(THUMBNAIL_CELL_OFFSET))
        || in_entry_range(
            target,
            ids.entry_base.saturating_add(THUMBNAIL_IMAGE_OFFSET),
        )
}

pub(super) fn detail_disclosure_for_widget(target: u64) -> Option<(PaneSlot, usize)> {
    [PaneSlot::Primary, PaneSlot::Secondary]
        .into_iter()
        .find_map(|slot| {
            let base = pane_ids(slot)
                .entry_base
                .saturating_add(DETAIL_DISCLOSURE_OFFSET);
            let index = target.checked_sub(base)?;
            (index < rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64)
                .then(|| usize::try_from(index).ok().map(|index| (slot, index)))
                .flatten()
        })
}

const fn in_entry_range(target: u64, base: u64) -> bool {
    target >= base && target < base + rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64
}

pub(super) fn entry_for_widget(target: u64) -> Option<(PaneSlot, usize)> {
    [PaneSlot::Primary, PaneSlot::Secondary]
        .into_iter()
        .find_map(|slot| {
            let base = pane_ids(slot).entry_base;
            [
                base,
                base + THUMBNAIL_CELL_OFFSET,
                base + THUMBNAIL_IMAGE_OFFSET,
            ]
            .into_iter()
            .find_map(|candidate| {
                let index = target.checked_sub(candidate)?;
                (index < rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64)
                    .then(|| usize::try_from(index).ok().map(|index| (slot, index)))
                    .flatten()
            })
        })
}

/// Resolves every visible part of an entry row to its owning entry. Pointer
/// selection remains limited to the primary entry control, while drag/drop
/// must also understand the size and modified cells in Details view.
pub(super) fn entry_for_drop_widget(target: u64) -> Option<(PaneSlot, usize)> {
    entry_for_widget(target).or_else(|| {
        [PaneSlot::Primary, PaneSlot::Secondary]
            .into_iter()
            .find_map(|slot| {
                let ids = pane_ids(slot);
                [
                    ids.detail_row_base,
                    ids.detail_meta_base,
                    ids.detail_size_base,
                    ids.detail_modified_base,
                    ids.detail_type_base,
                ]
                .into_iter()
                .find_map(|base| {
                    let index = target.checked_sub(base)?;
                    (index < rice_native_app_protocol::MAX_DIRECTORY_ENTRIES as u64)
                        .then(|| usize::try_from(index).ok().map(|index| (slot, index)))
                        .flatten()
                })
            })
    })
}

pub(super) fn pane_action(target: u64) -> Option<u64> {
    for slot in [PaneSlot::Primary, PaneSlot::Secondary] {
        let ids = pane_ids(slot);
        return match target {
            value if value == ids.detail_name => Some(SORT_NAME),
            value if value == ids.detail_size => Some(SORT_SIZE),
            value if value == ids.detail_modified => Some(SORT_MODIFIED),
            value if value == ids.detail_type => Some(SORT_TYPE),
            _ => continue,
        };
    }
    None
}
