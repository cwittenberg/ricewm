//! Closed sorting strategies with deterministic natural-name tie breaking.

use crate::{EntryKind, EntryMetadata};
use core::cmp::Ordering;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortKey {
    #[default]
    Name,
    Type,
    Size,
    Extension,
    Modified,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

/// Strategy value persisted per folder and shared by all views.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SortStrategy {
    pub key: SortKey,
    pub direction: SortDirection,
    pub directories_first: bool,
}

impl Default for SortStrategy {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            direction: SortDirection::Ascending,
            directories_first: true,
        }
    }
}

/// Stable in-place sorting. Identical records retain adapter enumeration order.
pub fn sort_entries(entries: &mut [EntryMetadata], strategy: SortStrategy) {
    entries.sort_by(|left, right| compare_entries(left, right, strategy));
}

#[must_use]
pub fn compare_entries(
    left: &EntryMetadata,
    right: &EntryMetadata,
    strategy: SortStrategy,
) -> Ordering {
    if strategy.directories_first {
        let directory_order = is_directory(right.kind).cmp(&is_directory(left.kind));
        if directory_order != Ordering::Equal {
            return directory_order;
        }
    }

    let primary = match strategy.key {
        SortKey::Name => natural_cmp(&left.name, &right.name),
        SortKey::Type => type_key(left).cmp(&type_key(right)),
        SortKey::Size => left.size.cmp(&right.size),
        SortKey::Extension => extension(&left.name).cmp(extension(&right.name)),
        SortKey::Modified => left.modified_unix_ms.cmp(&right.modified_unix_ms),
    };
    let primary = match strategy.direction {
        SortDirection::Ascending => primary,
        SortDirection::Descending => primary.reverse(),
    };
    primary
        .then_with(|| natural_cmp(&left.name, &right.name))
        .then_with(|| left.name.as_bytes().cmp(right.name.as_bytes()))
        .then_with(|| left.path.cmp(&right.path))
}

const fn is_directory(kind: EntryKind) -> bool {
    matches!(kind, EntryKind::Directory)
}

fn type_key(entry: &EntryMetadata) -> (u8, &str) {
    let rank = match entry.kind {
        EntryKind::Directory => 0,
        EntryKind::File => 1,
        EntryKind::Symlink => 2,
        EntryKind::Other => 3,
    };
    (rank, entry.media_type.as_deref().unwrap_or(""))
}

fn extension(name: &str) -> &str {
    let Some(boundary) = name.rfind('.') else {
        return "";
    };
    if boundary == 0 || boundary + 1 == name.len() {
        ""
    } else {
        &name[boundary + 1..]
    }
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left = left.as_bytes();
    let right = right.as_bytes();
    let (mut left_at, mut right_at) = (0, 0);
    while left_at < left.len() && right_at < right.len() {
        if left[left_at].is_ascii_digit() && right[right_at].is_ascii_digit() {
            let left_end = digit_end(left, left_at);
            let right_end = digit_end(right, right_at);
            let order = numeric_run_cmp(&left[left_at..left_end], &right[right_at..right_end]);
            if order != Ordering::Equal {
                return order;
            }
            left_at = left_end;
            right_at = right_end;
            continue;
        }
        let order = left[left_at]
            .to_ascii_lowercase()
            .cmp(&right[right_at].to_ascii_lowercase());
        if order != Ordering::Equal {
            return order;
        }
        left_at += 1;
        right_at += 1;
    }
    left.len().cmp(&right.len())
}

fn digit_end(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        at += 1;
    }
    at
}

fn numeric_run_cmp(left: &[u8], right: &[u8]) -> Ordering {
    let left_zeroes = left.iter().take_while(|byte| **byte == b'0').count();
    let right_zeroes = right.iter().take_while(|byte| **byte == b'0').count();
    let left_significant = &left[left_zeroes..];
    let right_significant = &right[right_zeroes..];
    let left_significant = if left_significant.is_empty() {
        b"0".as_slice()
    } else {
        left_significant
    };
    let right_significant = if right_significant.is_empty() {
        b"0".as_slice()
    } else {
        right_significant
    };
    left_significant
        .len()
        .cmp(&right_significant.len())
        .then_with(|| left_significant.cmp(right_significant))
        .then_with(|| left.len().cmp(&right.len()))
}
