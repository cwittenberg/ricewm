//! Shared semantic file-tree rows used by RiceFiles and RicePaper.

use rice_native_app_protocol::WidgetId;
use rice_ui::{IconId, Patch, WidgetKind, WidgetSpec, WidgetState};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TreeItemState {
    pub(crate) selected: bool,
    pub(crate) expanded: bool,
    /// The directory has not been enumerated yet and may expose child rows.
    pub(crate) potential_children: bool,
    pub(crate) muted: bool,
    pub(crate) focused: bool,
}

impl TreeItemState {
    pub(crate) fn widget_state(self) -> WidgetState {
        WidgetState::ENABLED
            | WidgetState::START_ALIGNED
            | WidgetState::KEY_NAVIGATION
            | if self.selected {
                WidgetState::SELECTED
            } else {
                WidgetState::empty()
            }
            | if self.expanded {
                WidgetState::CHECKED
            } else {
                WidgetState::empty()
            }
            | if self.potential_children {
                WidgetState::INDETERMINATE
            } else {
                WidgetState::empty()
            }
            | if self.muted {
                WidgetState::MUTED
            } else {
                WidgetState::empty()
            }
            | if self.focused {
                WidgetState::FOCUSED
            } else {
                WidgetState::empty()
            }
    }
}

pub(crate) fn create_tree_item(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: u64,
    label: &str,
    icon: IconId,
    state: TreeItemState,
) {
    let parent = Some(widget_id(parent));
    let index = patches
        .iter()
        .filter(|patch| {
            matches!(patch, Patch::Create { parent: candidate, .. } if *candidate == parent)
        })
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind: WidgetKind::TreeItem,
            state: state.widget_state(),
            text: Some(label.to_owned()),
            icon: Some(icon),
        },
        parent,
        index,
    });
}

/// Creates the one foundational file-tree surface used by file-oriented apps.
/// Apps supply real filesystem rows, while expansion, selection, keyboard
/// navigation, treelines, and disclosure rendering remain one control.
pub(crate) fn create_file_tree(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: u64,
    layout: Option<&str>,
) {
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind: WidgetKind::Tree,
            state: WidgetState::ENABLED | WidgetState::KEY_NAVIGATION,
            text: layout.map(str::to_owned),
            icon: None,
        },
        parent: Some(widget_id(parent)),
        index: patches
            .iter()
            .filter(|patch| {
                matches!(patch, Patch::Create { parent: candidate, .. } if *candidate == Some(widget_id(parent)))
            })
            .count(),
    });
}

fn widget_id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("shared file-tree widget IDs are nonzero")
}
