//! Entry activation and conventional multi-selection behavior.

use super::{FilesActorError, RiceFiles, files_view};
use rice_files::{EntryKind, GuestPath, PaneSlot, ViewMode};
use rice_native_app_protocol::{Message, RequestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PointerSelection {
    slot: PaneSlot,
    anchor: Option<GuestPath>,
    base: Vec<GuestPath>,
}

impl RiceFiles {
    pub(super) fn move_keyboard_selection(
        &mut self,
        target: u64,
        key_code: u32,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let visible = self.visible_entries();
        if visible.is_empty() {
            return Ok(Vec::new());
        }
        let active_slot = self.active_slot()?;
        let caret = self.runtime()?.selection_caret.as_ref();
        let selected_paths = self.pane()?.selection().cloned().collect::<Vec<_>>();
        let current = caret
            .and_then(|caret| visible.iter().position(|entry| &entry.path == caret))
            .or_else(|| {
                selected_paths
                    .iter()
                    .find_map(|selected| visible.iter().position(|entry| &entry.path == selected))
            });
        // The native UI host resolves arrow-key movement from the actual
        // laid-out rectangles. This keeps grid navigation aligned with the
        // dynamically computed column count at every window size and zoom.
        let resolved = files_view::entry_for_widget(target)
            .filter(|(slot, index)| *slot == active_slot && *index < visible.len())
            .map(|(_, index)| index);
        let extending = modifiers & 1 != 0;
        let next = match (current, resolved) {
            (None, _) => 0,
            (Some(current), Some(index)) if index != current => index,
            (Some(current), _) if self.pane()?.view == ViewMode::Details && !extending => {
                let entry = visible[current].clone();
                match key_code {
                    // Nautilus list view treats horizontal arrows as tree
                    // navigation: Right expands or enters the first child;
                    // Left collapses or returns to the expanded parent row.
                    39 | 114 if entry.kind == EntryKind::Directory => {
                        if self
                            .runtime()?
                            .expanded_directories
                            .contains_key(&entry.path)
                        {
                            let child = visible
                                .iter()
                                .enumerate()
                                .skip(current + 1)
                                .find(|(_, candidate)| candidate.path.parent() == entry.path)
                                .map(|(index, _)| index);
                            let Some(child) = child else {
                                return Ok(Vec::new());
                            };
                            child
                        } else {
                            return self.toggle_detail_directory(entry, request);
                        }
                    }
                    37 | 113 => {
                        if entry.kind == EntryKind::Directory
                            && self
                                .runtime()?
                                .expanded_directories
                                .contains_key(&entry.path)
                        {
                            return self.toggle_detail_directory(entry, request);
                        }
                        let parent = entry.path.parent();
                        if !self.runtime()?.expanded_directories.contains_key(&parent) {
                            return Ok(Vec::new());
                        }
                        let Some(parent_index) = visible
                            .iter()
                            .position(|candidate| candidate.path == parent)
                        else {
                            return Ok(Vec::new());
                        };
                        parent_index
                    }
                    _ => return Ok(Vec::new()),
                }
            }
            (Some(_), _) => return Ok(Vec::new()),
        };
        let entry = visible[next].clone();
        if extending {
            self.select_range(next, modifiers & 4 != 0)?;
        } else {
            self.set_single_selection(next)?;
        }
        // The UI host scrolls the resolved tile into view using its actual
        // layout before delivering this key. An entry index is not a grid row.
        self.refresh_preview(request, Some(&entry))
    }

    pub(super) fn select_all_entries(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let visible = self.visible_entries();
        if visible.is_empty() {
            return Ok(Vec::new());
        }
        let paths = visible
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<Vec<_>>();
        let anchor = paths.first().cloned();
        let caret = paths.last().cloned();
        self.pane_mut()?.set_selection(paths);
        let runtime = self.runtime_mut()?;
        runtime.selection_anchor = anchor;
        runtime.selection_caret = caret;
        self.refresh_preview(request, visible.last())
    }

    pub(super) fn clear_entry_selection(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pane()?.selection().next().is_none()
            && self.runtime()?.selection_anchor.is_none()
            && self.runtime()?.selection_caret.is_none()
        {
            return Ok(Vec::new());
        }
        self.pane_mut()?.set_selection([]);
        let runtime = self.runtime_mut()?;
        runtime.selection_anchor = None;
        runtime.selection_caret = None;
        self.refresh_preview(request, None)
    }

    pub(super) fn pointer_select_entry(
        &mut self,
        target: u64,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some((slot, index)) = files_view::entry_for_widget(target)
            .filter(|(_, index)| *index < self.visible_entries().len())
        else {
            return Ok(Vec::new());
        };
        if slot != self.active_slot()? {
            return Ok(Vec::new());
        }
        let entry = self.visible_entries()[index].clone();
        if modifiers & 1 != 0 {
            self.select_range(index, modifiers & 4 != 0)?;
        } else if modifiers & 4 != 0 {
            let mut selected = self.pane()?.selection().cloned().collect::<Vec<_>>();
            if let Some(position) = selected.iter().position(|path| path == &entry.path) {
                selected.remove(position);
            } else {
                selected.push(entry.path.clone());
            }
            self.pane_mut()?.set_selection(selected);
            let runtime = self.runtime_mut()?;
            runtime.selection_anchor = Some(entry.path.clone());
            runtime.selection_caret = Some(entry.path.clone());
        } else {
            self.set_single_selection(index)?;
        }
        self.refresh_preview(request, Some(&entry))
    }

    /// Starts a background sweep. The first cell crossed becomes the anchor;
    /// subsequent pointer motion selects the contiguous visual-order range.
    pub(super) fn begin_background_selection(
        &mut self,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let slot = self.active_slot()?;
        let additive = modifiers & 4 != 0;
        let extending = modifiers & 1 != 0;
        let base = if additive {
            self.pane()?.selection().cloned().collect()
        } else {
            Vec::new()
        };
        let anchor = if extending {
            self.runtime()?.selection_anchor.clone()
        } else {
            None
        };
        self.pointer_selection = Some(PointerSelection { slot, anchor, base });
        if additive || extending {
            return Ok(Vec::new());
        }
        self.clear_entry_selection(request)
    }

    pub(super) fn update_pointer_selection(
        &mut self,
        target: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(mut gesture) = self.pointer_selection.take() else {
            return Ok(Vec::new());
        };
        let Some((slot, index)) = files_view::entry_for_widget(target) else {
            self.pointer_selection = Some(gesture);
            return Ok(Vec::new());
        };
        let visible = self.visible_entries();
        if slot != gesture.slot || index >= visible.len() {
            self.pointer_selection = Some(gesture);
            return Ok(Vec::new());
        }
        let entry = visible[index].clone();
        let anchor = gesture
            .anchor
            .as_ref()
            .and_then(|path| visible.iter().position(|candidate| &candidate.path == path))
            .unwrap_or(index);
        if gesture.anchor.is_none() {
            gesture.anchor = Some(visible[anchor].path.clone());
        }
        let (start, end) = ordered(anchor, index);
        let mut selected = gesture.base.clone();
        selected.extend(
            visible[start..=end]
                .iter()
                .map(|candidate| candidate.path.clone()),
        );
        self.pane_mut()?.set_selection(selected);
        let runtime = self.runtime_mut()?;
        runtime.selection_anchor.clone_from(&gesture.anchor);
        runtime.selection_caret = Some(entry.path.clone());
        self.pointer_selection = Some(gesture);
        self.refresh_preview(request, Some(&entry))
    }

    pub(super) fn finish_pointer_selection(
        &mut self,
        target: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let messages = self.update_pointer_selection(target, request)?;
        self.pointer_selection = None;
        Ok(messages)
    }

    pub(super) fn cancel_pointer_selection(&mut self) {
        self.pointer_selection = None;
    }

    pub(super) fn pointer_selection_active(&self) -> bool {
        self.pointer_selection.is_some()
    }

    pub(super) fn activate_entry(
        &mut self,
        index: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.entry(index) else {
            return Ok(Vec::new());
        };
        let already_selected = self
            .pane()?
            .selection()
            .any(|selected| selected == &entry.path);
        if already_selected {
            if entry.kind == EntryKind::Directory && self.pane()?.view == ViewMode::Details {
                return self.toggle_detail_directory(entry, request);
            }
            return self.open_selected(request);
        }
        let Some(index) = usize::try_from(index).ok() else {
            return Ok(Vec::new());
        };
        self.set_single_selection(index)?;
        self.refresh_preview(request, Some(&entry))
    }

    /// Opens the entry represented by a native double-click. Details-view
    /// disclosure is a separate control, so double-clicking the row must
    /// navigate rather than toggling its inline children.
    pub(super) fn open_entry_immediately(
        &mut self,
        index: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(index) = usize::try_from(index).ok() else {
            return Ok(Vec::new());
        };
        if self.visible_entries().get(index).is_none() {
            return Ok(Vec::new());
        }
        self.set_single_selection(index)?;
        self.open_selected(request)
    }

    pub(super) fn select_context_entry(
        &mut self,
        target: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some((_, index)) = files_view::entry_for_widget(target) else {
            return Ok(Vec::new());
        };
        let Some(entry) = self.visible_entries().get(index).cloned() else {
            return Ok(Vec::new());
        };
        // Context-clicking any member of a multi-selection must preserve the
        // whole selection so Properties, Copy/Move, and Delete stay collective.
        if self
            .pane()?
            .selection()
            .any(|selected| selected == &entry.path)
        {
            let runtime = self.runtime_mut()?;
            runtime.selection_anchor = Some(entry.path.clone());
            runtime.selection_caret = Some(entry.path.clone());
            return self.refresh_preview(request, Some(&entry));
        }
        self.set_single_selection(index)?;
        self.refresh_preview(request, Some(&entry))
    }

    fn set_single_selection(&mut self, index: usize) -> Result<(), FilesActorError> {
        let Some(entry) = self.visible_entries().get(index).cloned() else {
            return Ok(());
        };
        self.pane_mut()?.set_selection([entry.path.clone()]);
        let runtime = self.runtime_mut()?;
        runtime.selection_anchor = Some(entry.path.clone());
        runtime.selection_caret = Some(entry.path);
        Ok(())
    }

    fn select_range(&mut self, index: usize, additive: bool) -> Result<(), FilesActorError> {
        let visible = self.visible_entries();
        let Some(entry) = visible.get(index).cloned() else {
            return Ok(());
        };
        let runtime_anchor = self.runtime()?.selection_anchor.clone();
        let runtime_caret = self.runtime()?.selection_caret.clone();
        let current_selection = self.pane()?.selection().cloned().collect::<Vec<_>>();
        let anchor_path = runtime_anchor
            .as_ref()
            .filter(|path| visible.iter().any(|candidate| &candidate.path == *path))
            .cloned()
            .or_else(|| {
                runtime_caret
                    .as_ref()
                    .filter(|path| visible.iter().any(|candidate| &candidate.path == *path))
                    .cloned()
            })
            .or_else(|| {
                current_selection.iter().find_map(|selected| {
                    visible
                        .iter()
                        .find(|candidate| &candidate.path == selected)
                        .map(|candidate| candidate.path.clone())
                })
            })
            .unwrap_or_else(|| entry.path.clone());
        let anchor = visible
            .iter()
            .position(|candidate| candidate.path == anchor_path)
            .unwrap_or(index);
        let (start, end) = ordered(anchor, index);
        let mut selected = if additive {
            self.pane()?.selection().cloned().collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        selected.extend(
            visible[start..=end]
                .iter()
                .map(|candidate| candidate.path.clone()),
        );
        self.pane_mut()?.set_selection(selected);
        let runtime = self.runtime_mut()?;
        runtime.selection_anchor = Some(anchor_path);
        runtime.selection_caret = Some(entry.path);
        Ok(())
    }

    fn entry(&self, index: u64) -> Option<rice_files::EntryMetadata> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.visible_entries().get(index).cloned())
    }

    pub(super) fn selected_caret_entry(&self) -> Option<rice_files::EntryMetadata> {
        let caret = self.runtime().ok()?.selection_caret.as_ref();
        let selected = self.selected_entries();
        selected
            .iter()
            .find(|entry| Some(&entry.path) == caret)
            .cloned()
            .or_else(|| selected.first().cloned())
    }
}

const fn ordered(first: usize, second: usize) -> (usize, usize) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}
