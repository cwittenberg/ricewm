//! Per-tab, per-pane adapter state for independent file-manager sessions.

use std::collections::BTreeMap;

use rice_files::{
    DirectoryCompletion, EntryMetadata, GuestPath, ModelError, PaneSlot, PaneState, SortStrategy,
};
use rice_native_app_protocol::{Message, RequestId, VfsOperation, VfsRequest};

use super::{
    FileControls, FilesActorError, PendingDirectory, RiceFiles, files_search::PendingSearch,
    files_tree::PendingTreeDirectory,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PaneRuntime {
    pub entries: Vec<EntryMetadata>,
    pub directory_entries: Vec<EntryMetadata>,
    /// Last fully acknowledged directory listing, never a pending navigation target.
    pub loaded_location: Option<GuestPath>,
    pub location_text: String,
    pub completion: DirectoryCompletion,
    pub completion_index: Option<usize>,
    pub sort: SortStrategy,
    pub pending_directory: Option<PendingDirectory>,
    pub expanded_directories: BTreeMap<GuestPath, Vec<EntryMetadata>>,
    pub pending_tree_directory: Option<PendingTreeDirectory>,
    pub pending_search: Option<PendingSearch>,
    pub content_notice: String,
    pub controls: FileControls,
    /// Stable endpoints for conventional Shift-click and Shift+Arrow ranges.
    pub selection_anchor: Option<GuestPath>,
    pub selection_caret: Option<GuestPath>,
}

impl PaneRuntime {
    pub(super) fn new(location: &GuestPath) -> Self {
        Self {
            entries: Vec::new(),
            directory_entries: Vec::new(),
            loaded_location: None,
            location_text: location.as_str().to_owned(),
            completion: DirectoryCompletion::default(),
            completion_index: None,
            sort: SortStrategy::default(),
            pending_directory: None,
            expanded_directories: BTreeMap::new(),
            pending_tree_directory: None,
            pending_search: None,
            content_notice: String::new(),
            controls: FileControls {
                preview_visible: false,
                ..FileControls::default()
            },
            selection_anchor: None,
            selection_caret: None,
        }
    }

    pub(super) fn is_enumerating(&self) -> bool {
        self.pending_directory.is_some()
            || self.pending_tree_directory.is_some()
            || self.pending_search.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TabRuntime {
    pub primary: PaneRuntime,
    pub secondary: PaneRuntime,
}

impl TabRuntime {
    pub(super) fn new(location: &GuestPath) -> Self {
        Self {
            primary: PaneRuntime::new(location),
            secondary: PaneRuntime::new(location),
        }
    }

    pub(super) const fn pane(&self, slot: PaneSlot) -> &PaneRuntime {
        match slot {
            PaneSlot::Primary => &self.primary,
            PaneSlot::Secondary => &self.secondary,
        }
    }

    pub(super) const fn pane_mut(&mut self, slot: PaneSlot) -> &mut PaneRuntime {
        match slot {
            PaneSlot::Primary => &mut self.primary,
            PaneSlot::Secondary => &mut self.secondary,
        }
    }
}

impl RiceFiles {
    pub(super) fn active_slot(&self) -> Result<PaneSlot, FilesActorError> {
        let tab = self.tab()?;
        Ok(if tab.dual_pane {
            tab.focused_pane
        } else {
            PaneSlot::Primary
        })
    }

    pub(super) fn pane(&self) -> Result<&PaneState, FilesActorError> {
        let slot = self.active_slot()?;
        Ok(self
            .model
            .windows()
            .iter()
            .find(|window| window.id == self.model_window)
            .ok_or(ModelError::UnknownWindow(self.model_window))?
            .active_tab()
            .pane(slot))
    }

    pub(super) fn pane_mut(&mut self) -> Result<&mut PaneState, FilesActorError> {
        let slot = self.active_slot()?;
        Ok(self
            .model
            .window_mut(self.model_window)?
            .active_tab_mut()
            .pane_mut(slot))
    }

    pub(super) fn tab(&self) -> Result<&rice_files::TabState, FilesActorError> {
        Ok(self
            .model
            .windows()
            .iter()
            .find(|window| window.id == self.model_window)
            .ok_or(ModelError::UnknownWindow(self.model_window))?
            .active_tab())
    }

    pub(super) fn tab_mut(&mut self) -> Result<&mut rice_files::TabState, FilesActorError> {
        Ok(self.model.window_mut(self.model_window)?.active_tab_mut())
    }

    pub(super) fn location(&self) -> &GuestPath {
        self.pane()
            .expect("the actor owns one model window")
            .location()
    }

    pub(super) fn active_tab_id(&self) -> Result<u64, FilesActorError> {
        Ok(self.tab()?.id)
    }

    pub(super) fn runtime(&self) -> Result<&PaneRuntime, FilesActorError> {
        self.runtime_at(self.active_tab_id()?, self.active_slot()?)
    }

    pub(super) fn runtime_mut(&mut self) -> Result<&mut PaneRuntime, FilesActorError> {
        let tab = self.active_tab_id()?;
        let slot = self.active_slot()?;
        self.runtime_at_mut(tab, slot)
    }

    pub(super) fn runtime_at(
        &self,
        tab: u64,
        slot: PaneSlot,
    ) -> Result<&PaneRuntime, FilesActorError> {
        Ok(self
            .pane_runtimes
            .get(&tab)
            .ok_or(ModelError::UnknownTab(tab))?
            .pane(slot))
    }

    pub(super) fn runtime_at_mut(
        &mut self,
        tab: u64,
        slot: PaneSlot,
    ) -> Result<&mut PaneRuntime, FilesActorError> {
        Ok(self
            .pane_runtimes
            .get_mut(&tab)
            .ok_or(ModelError::UnknownTab(tab))?
            .pane_mut(slot))
    }

    pub(super) fn slot_for_directory_request(
        &self,
        request: RequestId,
    ) -> Result<PaneSlot, FilesActorError> {
        for slot in [PaneSlot::Primary, PaneSlot::Secondary] {
            if self
                .runtime_at(self.active_tab_id()?, slot)?
                .pending_directory
                .as_ref()
                .is_some_and(|pending| pending.request == request)
            {
                return Ok(slot);
            }
        }
        Err(FilesActorError::UnexpectedVfsResponse)
    }

    pub(super) fn slot_for_tree_request(&self, request: RequestId) -> Option<PaneSlot> {
        let tab = self.active_tab_id().ok()?;
        [PaneSlot::Primary, PaneSlot::Secondary]
            .into_iter()
            .find(|slot| {
                self.runtime_at(tab, *slot)
                    .ok()
                    .and_then(|runtime| runtime.pending_tree_directory.as_ref())
                    .is_some_and(|pending| pending.request_id() == request)
            })
    }

    pub(super) fn slot_for_search_request(&self, request: RequestId) -> Option<PaneSlot> {
        let tab = self.active_tab_id().ok()?;
        [PaneSlot::Primary, PaneSlot::Secondary]
            .into_iter()
            .find(|slot| {
                self.runtime_at(tab, *slot)
                    .ok()
                    .and_then(|runtime| runtime.pending_search.as_ref())
                    .is_some_and(|pending| pending.request_id() == request)
            })
    }

    pub(super) fn focus_pane(&mut self, slot: PaneSlot) -> Result<bool, FilesActorError> {
        let tab = self.tab_mut()?;
        if slot == PaneSlot::Secondary && !tab.dual_pane {
            return Ok(false);
        }
        let changed = tab.focused_pane != slot;
        tab.focused_pane = slot;
        Ok(changed)
    }

    pub(super) fn directory_request(
        &self,
        _slot: PaneSlot,
        request: RequestId,
        target: &GuestPath,
        cursor: u64,
    ) -> Message {
        Message::VfsRequest(VfsRequest {
            request,
            app: self.app,
            operation: VfsOperation::ReadDirectory {
                path: target.as_str().to_owned(),
                cursor,
                limit: 128,
            },
        })
    }

    pub(super) fn toggle_dual_pane(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let tab_id = self.active_tab_id()?;
        if self.tab()?.dual_pane {
            if self
                .pane_runtimes
                .get(&tab_id)
                .ok_or(ModelError::UnknownTab(tab_id))?
                .secondary
                .is_enumerating()
            {
                return self
                    .emit_notice(rice_i18n::t("Wait for the secondary pane filesystem request to finish"));
            }
            let location = self.tab()?.primary.location().clone();
            self.tab_mut()?.set_dual_pane(false);
            self.tab_mut()?.secondary = PaneState::new(location.clone());
            self.pane_runtimes
                .get_mut(&tab_id)
                .ok_or(ModelError::UnknownTab(tab_id))?
                .secondary = PaneRuntime::new(&location);
            return self.emit_full().map(|message| vec![message]);
        }
        let target = self.tab()?.primary.location().clone();
        self.tab_mut()?.secondary = PaneState::new(target.clone());
        self.tab_mut()?.set_dual_pane(true);
        self.tab_mut()?.focused_pane = PaneSlot::Secondary;
        let runtime = self
            .pane_runtimes
            .get_mut(&tab_id)
            .ok_or(ModelError::UnknownTab(tab_id))?;
        runtime.secondary = PaneRuntime::new(&target);
        self.options.configure(&mut runtime.secondary);
        runtime.secondary.pending_directory =
            Some(PendingDirectory::new(request, target.clone(), None, None));
        runtime.secondary.content_notice = format!("Loading {target}…");
        Ok(vec![
            self.emit_full()?,
            self.directory_request(PaneSlot::Secondary, request, &target, 0),
        ])
    }

    pub(super) fn scroll_active_pane(
        &mut self,
        button: u8,
    ) -> Result<Vec<Message>, FilesActorError> {
        let current = self.pane()?.scroll.clone();
        let base_extent = if matches!(
            self.pane()?.view,
            rice_files::ViewMode::Icons | rice_files::ViewMode::Thumbnails
        ) {
            128
        } else {
            40
        };
        let row_height = u32::from(self.view_zoom_permille).saturating_mul(base_extent) / 1_000;
        let total = u32::try_from(self.visible_entries().len())
            .unwrap_or(u32::MAX)
            .saturating_mul(row_height.max(1));
        let vertical = match button {
            4 => current.vertical.saturating_sub(96),
            5 => current.vertical.saturating_add(96).min(total),
            _ => return Ok(Vec::new()),
        };
        self.pane_mut()?.set_scroll(rice_files::ScrollPosition {
            vertical,
            ..current
        });
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn scroll_active_pane_to(
        &mut self,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(position) = value
            .split_once('/')
            .and_then(|(position, _)| position.trim().parse::<u32>().ok())
        else {
            return Ok(Vec::new());
        };
        let current = self.pane()?.scroll.clone();
        self.pane_mut()?.set_scroll(rice_files::ScrollPosition {
            vertical: position,
            ..current
        });
        self.emit_full().map(|message| vec![message])
    }
}
