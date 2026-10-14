//! Window, tab, pane, metadata, selection, and viewport state.

use crate::navigation::{LocationHistory, NavigationMode};
use crate::{GuestPath, NavigationCommand, NavigationError, NavigationIntent, NavigationPlan};
use core::fmt;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// Metadata supplied by a guest-VFS adapter. No filesystem calls occur here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryMetadata {
    pub path: GuestPath,
    pub name: String,
    pub kind: EntryKind,
    pub size: u64,
    pub modified_unix_ms: i64,
    /// POSIX permission and special bits reported by the guest VFS.
    pub mode: u32,
    pub media_type: Option<String>,
    pub symlink_target: Option<GuestPath>,
}

impl EntryMetadata {
    #[must_use]
    pub fn new(path: GuestPath, kind: EntryKind, size: u64, modified_unix_ms: i64) -> Self {
        let name = path.file_name().unwrap_or("/").to_owned();
        Self {
            path,
            name,
            kind,
            size,
            modified_unix_ms,
            mode: match kind {
                EntryKind::Directory => 0o755,
                EntryKind::Symlink => 0o777,
                EntryKind::File | EntryKind::Other => 0o644,
            },
            media_type: None,
            symlink_target: None,
        }
    }

    #[must_use]
    pub const fn with_mode(mut self, mode: u32) -> Self {
        self.mode = mode & 0o7777;
        self
    }

    #[must_use]
    pub fn with_media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    #[must_use]
    pub fn with_symlink_target(mut self, target: GuestPath) -> Self {
        self.symlink_target = Some(target);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    #[default]
    Icons,
    Compact,
    Thumbnails,
    Details,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ScrollPosition {
    pub horizontal: u32,
    pub vertical: u32,
    pub anchor: Option<GuestPath>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PaneSlot {
    #[default]
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneState {
    history: LocationHistory,
    selection: BTreeSet<GuestPath>,
    pub view: ViewMode,
    pub scroll: ScrollPosition,
    pub reload_generation: u64,
}

impl PaneState {
    #[must_use]
    pub fn new(location: GuestPath) -> Self {
        Self {
            history: LocationHistory::new(location),
            selection: BTreeSet::new(),
            view: ViewMode::default(),
            scroll: ScrollPosition::default(),
            reload_generation: 0,
        }
    }

    #[must_use]
    pub fn location(&self) -> &GuestPath {
        self.history.current()
    }

    pub fn selection(&self) -> impl Iterator<Item = &GuestPath> {
        self.selection.iter()
    }

    pub fn set_selection(&mut self, selection: impl IntoIterator<Item = GuestPath>) {
        self.selection = selection.into_iter().collect();
    }

    pub fn set_view(&mut self, view: ViewMode) {
        self.view = view;
    }

    pub fn set_scroll(&mut self, scroll: ScrollPosition) {
        self.scroll = scroll;
    }

    /// Retains only selection/anchor paths that still exist after an update.
    pub fn reconcile_entries(&mut self, entries: &[EntryMetadata]) {
        let present: BTreeSet<_> = entries.iter().map(|entry| &entry.path).collect();
        self.selection.retain(|path| present.contains(path));
        if self
            .scroll
            .anchor
            .as_ref()
            .is_some_and(|anchor| !present.contains(anchor))
        {
            self.scroll.anchor = None;
        }
    }

    /// Builds a guest-VFS intent without mutating pane state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid path input or unavailable history.
    pub fn plan_navigation(
        &self,
        command: NavigationCommand,
        home: &GuestPath,
    ) -> Result<NavigationPlan, NavigationError> {
        let (target, mode) = match command {
            NavigationCommand::Enter(input) => (
                GuestPath::resolve(self.location(), &input)?,
                NavigationMode::Push,
            ),
            NavigationCommand::Back => (
                self.history
                    .back()
                    .cloned()
                    .ok_or(NavigationError::NoHistory)?,
                NavigationMode::Back,
            ),
            NavigationCommand::Forward => (
                self.history
                    .forward()
                    .cloned()
                    .ok_or(NavigationError::NoHistory)?,
                NavigationMode::Forward,
            ),
            NavigationCommand::Up => (self.location().parent(), NavigationMode::Push),
            NavigationCommand::Home => (home.clone(), NavigationMode::Push),
            NavigationCommand::Reload => (self.location().clone(), NavigationMode::Reload),
        };
        Ok(NavigationPlan {
            intent: NavigationIntent {
                target,
                follow_symlinks: true,
            },
            mode,
            history_revision: self.history.revision(),
        })
    }

    /// Commits an adapter-confirmed directory. `resolved` may be a symlink's
    /// canonical guest location.
    ///
    /// # Errors
    ///
    /// Returns [`NavigationError::StalePlan`] if the pane moved since planning.
    pub fn commit_navigation(
        &mut self,
        plan: &NavigationPlan,
        resolved: GuestPath,
    ) -> Result<(), NavigationError> {
        if plan.history_revision != self.history.revision() {
            return Err(NavigationError::StalePlan);
        }
        let reload = plan.mode == NavigationMode::Reload;
        self.history.commit(plan.mode, resolved);
        if reload {
            self.reload_generation = self.reload_generation.saturating_add(1);
        } else {
            self.selection.clear();
            self.scroll = ScrollPosition::default();
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabState {
    pub id: u64,
    pub primary: PaneState,
    pub secondary: PaneState,
    pub dual_pane: bool,
    pub focused_pane: PaneSlot,
    split_permille: u16,
}

impl TabState {
    #[must_use]
    pub fn new(id: u64, location: GuestPath) -> Self {
        Self {
            id,
            primary: PaneState::new(location.clone()),
            secondary: PaneState::new(location),
            dual_pane: false,
            focused_pane: PaneSlot::Primary,
            split_permille: 500,
        }
    }

    #[must_use]
    pub fn pane(&self, slot: PaneSlot) -> &PaneState {
        match slot {
            PaneSlot::Primary => &self.primary,
            PaneSlot::Secondary => &self.secondary,
        }
    }

    pub fn pane_mut(&mut self, slot: PaneSlot) -> &mut PaneState {
        match slot {
            PaneSlot::Primary => &mut self.primary,
            PaneSlot::Secondary => &mut self.secondary,
        }
    }

    pub fn set_dual_pane(&mut self, enabled: bool) {
        self.dual_pane = enabled;
        if !enabled {
            self.focused_pane = PaneSlot::Primary;
        }
    }

    #[must_use]
    pub const fn split_permille(&self) -> u16 {
        self.split_permille
    }

    /// Applies a drag position while keeping both panes usable.
    pub fn resize_split(&mut self, position_permille: u16) {
        self.split_permille = position_permille.clamp(150, 850);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowState {
    pub id: u64,
    tabs: Vec<TabState>,
    active_tab: usize,
    closed_tabs: Vec<TabState>,
    next_tab_id: u64,
}

impl WindowState {
    const MAX_CLOSED_TABS: usize = 16;

    #[must_use]
    pub fn new(id: u64, location: GuestPath) -> Self {
        Self {
            id,
            tabs: vec![TabState::new(1, location)],
            active_tab: 0,
            closed_tabs: Vec::new(),
            next_tab_id: 2,
        }
    }

    #[must_use]
    pub fn tabs(&self) -> &[TabState] {
        &self.tabs
    }

    #[must_use]
    pub fn active_tab(&self) -> &TabState {
        &self.tabs[self.active_tab]
    }

    pub fn active_tab_mut(&mut self) -> &mut TabState {
        &mut self.tabs[self.active_tab]
    }

    pub fn open_tab(&mut self, location: GuestPath) -> u64 {
        let id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.saturating_add(1);
        self.tabs.push(TabState::new(id, location));
        self.active_tab = self.tabs.len() - 1;
        id
    }

    /// # Errors
    ///
    /// Returns [`ModelError::UnknownTab`] when `id` is not open.
    pub fn activate_tab(&mut self, id: u64) -> Result<(), ModelError> {
        self.active_tab = self
            .tabs
            .iter()
            .position(|tab| tab.id == id)
            .ok_or(ModelError::UnknownTab(id))?;
        Ok(())
    }

    /// # Errors
    ///
    /// Returns an error for an unknown tab or the window's final tab.
    pub fn close_tab(&mut self, id: u64) -> Result<(), ModelError> {
        if self.tabs.len() == 1 {
            return Err(ModelError::LastTab);
        }
        let index = self
            .tabs
            .iter()
            .position(|tab| tab.id == id)
            .ok_or(ModelError::UnknownTab(id))?;
        let closed = self.tabs.remove(index);
        push_bounded(&mut self.closed_tabs, closed, Self::MAX_CLOSED_TABS);
        self.active_tab = self.active_tab.min(self.tabs.len() - 1);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns [`ModelError::NoClosedTab`] when the bounded history is empty.
    pub fn reopen_tab(&mut self) -> Result<u64, ModelError> {
        let tab = self.closed_tabs.pop().ok_or(ModelError::NoClosedTab)?;
        let id = tab.id;
        self.tabs.push(tab);
        self.active_tab = self.tabs.len() - 1;
        Ok(id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileManager {
    windows: Vec<WindowState>,
    closed_windows: Vec<WindowState>,
    next_window_id: u64,
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManager {
    const MAX_CLOSED_WINDOWS: usize = 8;

    #[must_use]
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            closed_windows: Vec::new(),
            next_window_id: 1,
        }
    }

    #[must_use]
    pub fn windows(&self) -> &[WindowState] {
        &self.windows
    }

    pub fn open_window(&mut self, location: GuestPath) -> u64 {
        let id = self.next_window_id;
        self.next_window_id = self.next_window_id.saturating_add(1);
        self.windows.push(WindowState::new(id, location));
        id
    }

    /// # Errors
    ///
    /// Returns [`ModelError::UnknownWindow`] when `id` is not open.
    pub fn window_mut(&mut self, id: u64) -> Result<&mut WindowState, ModelError> {
        self.windows
            .iter_mut()
            .find(|window| window.id == id)
            .ok_or(ModelError::UnknownWindow(id))
    }

    /// # Errors
    ///
    /// Returns [`ModelError::UnknownWindow`] when `id` is not open.
    pub fn close_window(&mut self, id: u64) -> Result<(), ModelError> {
        let index = self
            .windows
            .iter()
            .position(|window| window.id == id)
            .ok_or(ModelError::UnknownWindow(id))?;
        let closed = self.windows.remove(index);
        push_bounded(&mut self.closed_windows, closed, Self::MAX_CLOSED_WINDOWS);
        Ok(())
    }

    /// # Errors
    ///
    /// Returns [`ModelError::NoClosedWindow`] when the bounded history is empty.
    pub fn reopen_window(&mut self) -> Result<u64, ModelError> {
        let mut window = self
            .closed_windows
            .pop()
            .ok_or(ModelError::NoClosedWindow)?;
        window.id = self.next_window_id;
        self.next_window_id = self.next_window_id.saturating_add(1);
        let id = window.id;
        self.windows.push(window);
        Ok(id)
    }
}

fn push_bounded<T>(items: &mut Vec<T>, item: T, maximum: usize) {
    if items.len() == maximum {
        items.remove(0);
    }
    items.push(item);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelError {
    UnknownWindow(u64),
    UnknownTab(u64),
    LastTab,
    NoClosedTab,
    NoClosedWindow,
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownWindow(id) => write!(formatter, "unknown RiceFiles window {id}"),
            Self::UnknownTab(id) => write!(formatter, "unknown RiceFiles tab {id}"),
            Self::LastTab => formatter.write_str("the final tab closes with its window"),
            Self::NoClosedTab => formatter.write_str("there is no closed tab to reopen"),
            Self::NoClosedWindow => formatter.write_str("there is no closed window to reopen"),
        }
    }
}

impl std::error::Error for ModelError {}
