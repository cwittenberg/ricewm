//! Axis partitioning and flex policy for the retained `RiceUI` layout engine.

use rice_native_app_protocol::WidgetId;
use rice_ui::{RetainedTree, WidgetKind};

fn flexes(kind: WidgetKind, horizontal: bool) -> bool {
    if horizontal {
        return matches!(
            kind,
            WidgetKind::SplitPane
                | WidgetKind::List
                | WidgetKind::Table
            | WidgetKind::VirtualGrid
                | WidgetKind::Tree
                | WidgetKind::IconGrid
                | WidgetKind::ScrollView
                | WidgetKind::TextField
                | WidgetKind::PasswordField
                | WidgetKind::TextEditor
                | WidgetKind::ScrollBar
                | WidgetKind::ProgressBar
                | WidgetKind::Image
                | WidgetKind::WebSurface
            | WidgetKind::SceneSurface
                | WidgetKind::TerminalSurface
                | WidgetKind::TerminalGrid
                | WidgetKind::HostSurface
        );
    }
    matches!(
        kind,
        WidgetKind::Container
            | WidgetKind::GroupBox
            | WidgetKind::Column
            | WidgetKind::Stack
            | WidgetKind::SplitPane
            | WidgetKind::List
            | WidgetKind::Table
            | WidgetKind::VirtualGrid
            | WidgetKind::Tree
            | WidgetKind::IconGrid
            | WidgetKind::ScrollView
            | WidgetKind::TextEditor
            | WidgetKind::Image
            | WidgetKind::WebSurface
            | WidgetKind::SceneSurface
            | WidgetKind::TerminalSurface
            | WidgetKind::TerminalGrid
            | WidgetKind::HostSurface
    )
}

pub(super) fn child_flexes(tree: &RetainedTree, child: WidgetId, horizontal: bool) -> bool {
    tree.get(child).is_some_and(|widget| {
        !(matches!(widget.kind, WidgetKind::Container | WidgetKind::Column)
            && widget.text.as_deref().is_some_and(|text| {
                super::layout_tokens::is_fixed_panel(Some(text))
                    || text.starts_with("panel:fixed-height:")
            }))
            && !(!horizontal
                && widget.kind == WidgetKind::ScrollView
                && !widget.state.contains(rice_ui::WidgetState::EXPANSIVE)
                && super::layout_tokens::viewport_height(widget.text.as_deref()).is_some())
            && !(!horizontal
                && super::layout_tokens::fixed_height(widget.text.as_deref()).is_some())
            && !(!horizontal && super::layout_tokens::is_intrinsic_stack(widget.text.as_deref()))
            && !(!horizontal && super::layout_tokens::is_store_header(widget.text.as_deref()))
            && (flexes(widget.kind, horizontal)
                || (horizontal && widget.state.contains(rice_ui::WidgetState::EXPANSIVE))
                || (!horizontal
                    && widget.kind == WidgetKind::Tabs
                    && super::layout_tokens::is_integrated_tabs(widget.text.as_deref()))
                || (horizontal && super::layout_tokens::is_flexible(widget.text.as_deref())))
    })
}

#[derive(Clone, Copy)]
pub(super) struct SplitSpec {
    permille: u16,
    minimum_width: Option<u32>,
    maximum_width: Option<u32>,
    trailing: bool,
    intrinsic_control: bool,
}

pub(super) fn split_spec(text: &str) -> Option<SplitSpec> {
    let mut parts = text.strip_prefix("split:")?.split(':');
    let permille = parts.next()?.parse::<u16>().ok()?.clamp(100, 900);
    let mut minimum_width = None;
    let mut maximum_width = None;
    let mut trailing = false;
    let mut intrinsic_control = false;
    for part in parts {
        if let Some(value) = part.strip_prefix("min-width=") {
            minimum_width = value.parse::<u32>().ok();
        } else if let Some(value) = part.strip_prefix("max-width=") {
            maximum_width = value.parse::<u32>().ok();
        } else if part == "intrinsic-control" {
            intrinsic_control = true;
        } else if part == "trailing" {
            trailing = true;
        }
    }
    Some(SplitSpec {
        permille,
        minimum_width,
        maximum_width,
        trailing,
        intrinsic_control,
    })
}

pub(super) fn partition_split(
    available: u32,
    gap: u32,
    preferred: &[u32],
    split: Option<SplitSpec>,
    horizontal: bool,
) -> Vec<u32> {
    if preferred.len() < 2 {
        return partition_axis(available, gap, preferred);
    }
    let gaps = gap.saturating_mul(u32::try_from(preferred.len() - 1).unwrap_or(u32::MAX));
    let usable = available.saturating_sub(gaps);
    let split = split.unwrap_or(SplitSpec {
        permille: 500,
        minimum_width: None,
        maximum_width: None,
        trailing: false,
        intrinsic_control: false,
    });
    let minimum = if horizontal {
        split.minimum_width.unwrap_or(0)
    } else {
        0
    }
    .min(usable);
    let maximum = if horizontal {
        split.maximum_width.unwrap_or(usable)
    } else {
        usable
    }
    .max(minimum)
    .min(usable);
    let requested = u32::try_from(u64::from(usable) * u64::from(split.permille) / 1_000)
        .unwrap_or(usable)
        .clamp(minimum, maximum);
    let first = if split.trailing {
        usable.saturating_sub(requested)
    } else {
        requested
    };
    // Give settings choices their intrinsic width before wrapping the copy.
    // Keep a readable description column; narrow forms stack in layout.rs.
    let first = if horizontal && split.intrinsic_control && preferred.len() == 2 {
        first.min(usable.saturating_sub(preferred[1].min(usable * 3 / 5)))
    } else {
        first
    };
    let rest = equal_partition(usable.saturating_sub(first), preferred.len() - 1);
    let mut result = Vec::with_capacity(preferred.len());
    result.push(first);
    result.extend(rest);
    result
}

/// Stable five-column proportions for Bowl's compact resource overview.
/// Name receives the space reclaimed from the intentionally narrow Type cell.
pub(super) fn partition_resource_columns(available: u32, gap: u32) -> Vec<u32> {
    const WEIGHTS: [u32; 5] = [44, 18, 10, 10, 18];
    let gaps = gap.saturating_mul(4);
    let usable = available.saturating_sub(gaps);
    let mut used = 0_u32;
    let mut result = Vec::with_capacity(WEIGHTS.len());
    for weight in WEIGHTS.iter().take(WEIGHTS.len() - 1) {
        let width = u32::try_from(u64::from(usable) * u64::from(*weight) / 100).unwrap_or(usable);
        used = used.saturating_add(width);
        result.push(width);
    }
    result.push(usable.saturating_sub(used));
    result
}

/// Partitions an axis using caller-supplied relative table column weights.
/// The last column absorbs integer rounding so the row always fills exactly.
pub(super) fn partition_weighted(available: u32, gap: u32, weights: &[u16]) -> Vec<u32> {
    if weights.is_empty() {
        return Vec::new();
    }
    let gaps =
        gap.saturating_mul(u32::try_from(weights.len().saturating_sub(1)).unwrap_or(u32::MAX));
    let usable = available.saturating_sub(gaps);
    let total = weights.iter().copied().map(u64::from).sum::<u64>().max(1);
    let mut used = 0_u32;
    let mut result = Vec::with_capacity(weights.len());
    for weight in weights.iter().take(weights.len().saturating_sub(1)) {
        let width = u32::try_from(u64::from(usable).saturating_mul(u64::from(*weight)) / total)
            .unwrap_or(usable);
        used = used.saturating_add(width);
        result.push(width);
    }
    result.push(usable.saturating_sub(used));
    result
}

pub(super) fn partition_flex(
    available: u32,
    gap: u32,
    preferred: &[u32],
    flex: &[bool],
) -> Vec<u32> {
    if preferred.is_empty() || preferred.len() != flex.len() {
        return partition_axis(available, gap, preferred);
    }
    let gaps =
        gap.saturating_mul(u32::try_from(preferred.len().saturating_sub(1)).unwrap_or(u32::MAX));
    let usable = available.saturating_sub(gaps);
    let mut result = preferred.to_vec();
    let flex_count = flex.iter().filter(|value| **value).count();
    if flex_count == 0 {
        return if preferred.iter().copied().sum::<u32>() <= usable {
            result
        } else {
            partition_axis(available, gap, preferred)
        };
    }
    let fixed = preferred
        .iter()
        .zip(flex)
        .filter_map(|(size, is_flex)| (!is_flex).then_some(*size))
        .fold(0_u32, u32::saturating_add);
    if fixed > usable {
        return partition_axis(available, gap, preferred);
    }
    let flexible = usable - fixed;
    let wanted_flex = preferred
        .iter()
        .zip(flex)
        .filter_map(|(size, is_flex)| is_flex.then_some(*size))
        .fold(0_u32, u32::saturating_add);
    let mut used = 0_u32;
    let mut remaining = flex_count;
    for (value, is_flex) in result.iter_mut().zip(flex) {
        if !is_flex {
            continue;
        }
        remaining -= 1;
        *value = if remaining == 0 {
            flexible.saturating_sub(used)
        } else if wanted_flex == 0 {
            flexible / u32::try_from(flex_count).unwrap_or(1)
        } else {
            u32::try_from(
                u64::from(flexible).saturating_mul(u64::from(*value)) / u64::from(wanted_flex),
            )
            .unwrap_or(0)
        };
        used = used.saturating_add(*value);
    }
    result
}

pub(super) fn partition_distribute(
    available: u32,
    gap: u32,
    preferred: &[u32],
    fixed: &[bool],
) -> Vec<u32> {
    if preferred.len() != fixed.len() || preferred.is_empty() {
        return partition_axis(available, gap, preferred);
    }
    let gaps =
        gap.saturating_mul(u32::try_from(preferred.len().saturating_sub(1)).unwrap_or(u32::MAX));
    let usable = available.saturating_sub(gaps);
    let fixed_total = preferred
        .iter()
        .zip(fixed)
        .filter_map(|(size, fixed)| fixed.then_some(*size))
        .fold(0_u32, u32::saturating_add);
    let grow_count = fixed.iter().filter(|value| !**value).count();
    if fixed_total > usable || grow_count == 0 {
        return partition_axis(available, gap, preferred);
    }
    let grow = equal_partition(usable - fixed_total, grow_count);
    let mut grow = grow.into_iter();
    preferred
        .iter()
        .zip(fixed)
        .map(|(size, fixed)| {
            if *fixed {
                *size
            } else {
                grow.next().unwrap_or(0)
            }
        })
        .collect()
}

pub(super) fn partition_axis(available: u32, gap: u32, preferred: &[u32]) -> Vec<u32> {
    if preferred.is_empty() {
        return Vec::new();
    }
    let gaps =
        gap.saturating_mul(u32::try_from(preferred.len().saturating_sub(1)).unwrap_or(u32::MAX));
    let usable = available.saturating_sub(gaps);
    let wanted = preferred.iter().copied().fold(0_u32, u32::saturating_add);
    if wanted <= usable {
        let mut result = preferred.to_vec();
        let surplus = usable - wanted;
        let count = u32::try_from(result.len()).unwrap_or(u32::MAX);
        let each = surplus / count;
        let remainder = surplus % count;
        for (index, value) in result.iter_mut().enumerate() {
            *value = value.saturating_add(each);
            if u32::try_from(index).is_ok_and(|index| index < remainder) {
                *value = value.saturating_add(1);
            }
        }
        return result;
    }
    if wanted == 0 {
        return equal_partition(usable, preferred.len());
    }
    let mut result = preferred
        .iter()
        .map(|value| {
            u32::try_from(u64::from(*value) * u64::from(usable) / u64::from(wanted)).unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let used = result.iter().copied().fold(0_u32, u32::saturating_add);
    if let Some(last) = result.last_mut() {
        *last = last.saturating_add(usable.saturating_sub(used));
    }
    result
}

fn equal_partition(available: u32, count: usize) -> Vec<u32> {
    let Ok(count_u32) = u32::try_from(count) else {
        return vec![0; count];
    };
    let unit = available / count_u32;
    let mut result = vec![unit; count];
    let remainder = available % count_u32;
    for (index, value) in result.iter_mut().enumerate() {
        if u32::try_from(index).is_ok_and(|index| index < remainder) {
            *value = value.saturating_add(1);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{partition_resource_columns, partition_split, split_spec};

    #[test]
    fn resource_columns_keep_type_narrow_and_consume_the_available_width() {
        let columns = partition_resource_columns(720, 8);
        assert_eq!(columns, vec![302, 123, 68, 68, 127]);
        assert_eq!(columns.iter().sum::<u32>() + 32, 720);
        assert!(columns[2] < columns[1]);
        assert!(columns[2] < columns[0]);
    }

    #[test]
    fn trailing_split_bounds_and_sizes_the_second_pane() {
        let split = split_spec("split:260:trailing:min-width=190:max-width=420");
        assert_eq!(
            partition_split(1_000, 0, &[500, 200], split, true),
            [740, 260]
        );

        let narrow = split_spec("split:100:trailing:min-width=190:max-width=420");
        assert_eq!(
            partition_split(1_000, 0, &[500, 200], narrow, true),
            [810, 190]
        );

        let wide = split_spec("split:900:trailing:min-width=190:max-width=420");
        assert_eq!(
            partition_split(1_000, 0, &[500, 200], wide, true),
            [580, 420]
        );
    }
}
