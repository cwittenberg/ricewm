//! `RiceDesktop` aggregate and Command dispatcher.

use std::collections::{BTreeMap, BTreeSet};

use crate::events::{CoalescedEvent, DesktopEventCoalescer, MonitorEvent};
use crate::menu::{ContextMenuPolicy, MenuEntry, MenuRequest};
use crate::selection::{ClickMode, Direction, SelectionState};
use crate::wallpaper::{Wallpaper, WallpaperSettings, WorkspaceId};
use crate::{Cell, GridConfig, GridLayout, Rect};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DesktopItemId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopItemKind {
    Home,
    Documents,
    Pictures,
    Trash { empty: bool },
    Mount { mount_id: String },
    File,
    Folder,
    Application { desktop_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemAction {
    Open,
    Launch,
    Rename,
    Cut,
    Copy,
    Paste,
    Link,
    MoveToTrash,
    DeletePermanently,
    EmptyTrash,
    Mount,
    Unmount,
    Eject,
    OpenWith,
    Properties,
    NewFolder,
    NewFile,
    SelectAll,
    ArrangeIcons,
    DesktopSettings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopItem {
    pub id: DesktopItemId,
    pub label: String,
    pub icon_id: String,
    pub path: Option<String>,
    pub kind: DesktopItemKind,
    pub read_only: bool,
    /// Whether this file has at least one POSIX execute bit set.
    pub executable: bool,
    pub available: bool,
}

impl DesktopItem {
    #[must_use]
    pub fn primary_action(&self) -> Option<ItemAction> {
        if !self.available {
            return matches!(self.kind, DesktopItemKind::Mount { .. }).then_some(ItemAction::Mount);
        }
        Some(match self.kind {
            DesktopItemKind::Application { .. } => ItemAction::Launch,
            _ => ItemAction::Open,
        })
    }

    #[must_use]
    pub fn is_renameable(&self) -> bool {
        self.available
            && !self.read_only
            && matches!(
                self.kind,
                DesktopItemKind::File
                    | DesktopItemKind::Folder
                    | DesktopItemKind::Application { .. }
            )
    }

    #[must_use]
    pub fn can_delete(&self) -> bool {
        self.available
            && !self.read_only
            && matches!(
                self.kind,
                DesktopItemKind::File
                    | DesktopItemKind::Folder
                    | DesktopItemKind::Application { .. }
            )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopCommand {
    ReplaceItems {
        items: Vec<DesktopItem>,
        persisted: BTreeMap<DesktopItemId, Cell>,
    },
    Click {
        item: DesktopItemId,
        mode: ClickMode,
    },
    ClearSelection,
    SelectAll,
    RubberBand {
        area: Rect,
        additive: bool,
    },
    Navigate {
        direction: Direction,
        extend: bool,
    },
    Activate(DesktopItemId),
    BeginRename,
    MoveIcon {
        item: DesktopItemId,
        cell: Cell,
    },
    MoveIcons {
        items: Vec<DesktopItemId>,
        anchor: DesktopItemId,
        cell: Cell,
    },
    ArrangeIcons,
    ResizeGrid(GridConfig),
    RequestContextMenu {
        clipboard_has_files: bool,
        desktop_read_only: bool,
    },
    RequestRefresh,
    RequestDesktopSettings,
    QueueMonitorEvent(MonitorEvent),
    FlushMonitorEvents,
    SwitchWorkspace(WorkspaceId),
    SetCommonWallpaper(Wallpaper),
    SetWorkspaceWallpaper {
        workspace: WorkspaceId,
        wallpaper: Wallpaper,
    },
    ClearWorkspaceWallpaper(WorkspaceId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DesktopIntent {
    Perform {
        action: ItemAction,
        items: Vec<DesktopItemId>,
    },
    Rename {
        item: DesktopItemId,
        current_label: String,
    },
    ShowContextMenu(Vec<MenuEntry>),
    PersistPositions(Vec<(DesktopItemId, Cell)>),
    RefreshPath(String),
    RemovePath(String),
    RenamePath {
        from: String,
        to: String,
    },
    ReenumerateDesktop,
    MountArrived {
        mount_id: String,
        root: String,
        read_only: bool,
    },
    MountRemoved {
        mount_id: String,
        cancel_operations: bool,
    },
    RedrawWallpaper {
        workspace: WorkspaceId,
    },
    LaunchSettings {
        section: &'static str,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ModelUpdate {
    pub changed: bool,
    pub intents: Vec<DesktopIntent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    DuplicateItem(DesktopItemId),
    UnknownItem(DesktopItemId),
    RenameUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopModel {
    items: BTreeMap<DesktopItemId, DesktopItem>,
    grid: GridLayout,
    selection: SelectionState,
    wallpaper: WallpaperSettings,
    workspace: WorkspaceId,
    events: DesktopEventCoalescer,
}

impl DesktopModel {
    #[must_use]
    pub fn new(grid: GridConfig, wallpaper: WallpaperSettings, workspace: WorkspaceId) -> Self {
        Self {
            items: BTreeMap::new(),
            grid: GridLayout::new(grid),
            selection: SelectionState::default(),
            wallpaper,
            workspace,
            events: DesktopEventCoalescer::default(),
        }
    }

    #[must_use]
    pub const fn grid(&self) -> &GridLayout {
        &self.grid
    }

    #[must_use]
    pub const fn selection(&self) -> &SelectionState {
        &self.selection
    }

    #[must_use]
    pub const fn wallpaper(&self) -> &WallpaperSettings {
        &self.wallpaper
    }

    #[must_use]
    pub const fn workspace(&self) -> &WorkspaceId {
        &self.workspace
    }

    #[must_use]
    pub fn item(&self, id: DesktopItemId) -> Option<&DesktopItem> {
        self.items.get(&id)
    }

    #[must_use]
    pub fn item_mut(&mut self, id: DesktopItemId) -> Option<&mut DesktopItem> {
        self.items.get_mut(&id)
    }

    /// Executes one pure state transition and returns host adapter intents.
    ///
    /// # Errors
    ///
    /// Returns [`ModelError`] for unknown or duplicate item identifiers and
    /// for rename commands that do not have one writable selection.
    pub fn execute(&mut self, command: DesktopCommand) -> Result<ModelUpdate, ModelError> {
        match command {
            DesktopCommand::ReplaceItems { items, persisted } => {
                self.replace_items(items, &persisted)
            }
            DesktopCommand::Click { item, mode } => {
                self.require_item(item)?;
                self.selection.click(item, mode, &self.grid);
                Ok(changed())
            }
            DesktopCommand::ClearSelection => {
                self.selection.clear();
                Ok(changed())
            }
            DesktopCommand::SelectAll => {
                self.selection.select_all(&self.grid);
                Ok(changed())
            }
            DesktopCommand::RubberBand { area, additive } => {
                self.selection.rubber_band(area, additive, &self.grid);
                Ok(changed())
            }
            DesktopCommand::Navigate { direction, extend } => {
                self.selection.navigate(direction, extend, &self.grid);
                Ok(changed())
            }
            DesktopCommand::Activate(item) => self.activate(item),
            DesktopCommand::BeginRename => self.begin_rename(),
            DesktopCommand::MoveIcon { item, cell } => self.move_icon(item, cell),
            DesktopCommand::MoveIcons {
                items,
                anchor,
                cell,
            } => self.move_icons(&items, anchor, cell),
            DesktopCommand::ArrangeIcons => {
                let mut items = self.items.values().collect::<Vec<_>>();
                items.sort_by(|left, right| {
                    left.label
                        .to_lowercase()
                        .cmp(&right.label.to_lowercase())
                        .then_with(|| left.label.cmp(&right.label))
                        .then_with(|| left.id.cmp(&right.id))
                });
                self.grid
                    .arrange_in_order(items.into_iter().map(|item| item.id));
                Ok(self.positions_update())
            }
            DesktopCommand::ResizeGrid(config) => Ok(self.resize_grid(config)),
            DesktopCommand::RequestContextMenu {
                clipboard_has_files,
                desktop_read_only,
            } => Ok(self.context_menu(clipboard_has_files, desktop_read_only)),
            DesktopCommand::RequestRefresh => Ok(ModelUpdate {
                changed: false,
                intents: vec![DesktopIntent::ReenumerateDesktop],
            }),
            DesktopCommand::RequestDesktopSettings => Ok(ModelUpdate {
                changed: false,
                intents: vec![DesktopIntent::LaunchSettings {
                    section: "appearance",
                }],
            }),
            DesktopCommand::QueueMonitorEvent(event) => {
                self.events.push(event);
                Ok(ModelUpdate::default())
            }
            DesktopCommand::FlushMonitorEvents => Ok(self.flush_events()),
            DesktopCommand::SwitchWorkspace(workspace) => {
                self.workspace = workspace;
                Ok(self.wallpaper_update())
            }
            DesktopCommand::SetCommonWallpaper(wallpaper) => {
                self.wallpaper.set_common(wallpaper);
                Ok(self.wallpaper_update())
            }
            DesktopCommand::SetWorkspaceWallpaper {
                workspace,
                wallpaper,
            } => {
                self.wallpaper.set_workspace(workspace, wallpaper);
                Ok(self.wallpaper_update())
            }
            DesktopCommand::ClearWorkspaceWallpaper(workspace) => {
                self.wallpaper.clear_workspace(&workspace);
                Ok(self.wallpaper_update())
            }
        }
    }

    fn replace_items(
        &mut self,
        items: Vec<DesktopItem>,
        persisted: &BTreeMap<DesktopItemId, Cell>,
    ) -> Result<ModelUpdate, ModelError> {
        let ordered = items.iter().map(|item| item.id).collect::<Vec<_>>();
        let mut next = BTreeMap::new();
        for item in items {
            let id = item.id;
            if next.insert(id, item).is_some() {
                return Err(ModelError::DuplicateItem(id));
            }
        }
        self.grid.reconcile(ordered, persisted);
        self.items = next;
        self.selection.retain_layout_items(&self.grid);
        Ok(changed())
    }

    fn activate(&self, id: DesktopItemId) -> Result<ModelUpdate, ModelError> {
        let item = self.require_item(id)?;
        let Some(action) = item.primary_action() else {
            return Ok(ModelUpdate::default());
        };
        Ok(ModelUpdate {
            changed: false,
            intents: vec![DesktopIntent::Perform {
                action,
                items: vec![id],
            }],
        })
    }

    fn begin_rename(&self) -> Result<ModelUpdate, ModelError> {
        let Some(id) = self.selection.single() else {
            return Err(ModelError::RenameUnavailable);
        };
        let item = self.require_item(id)?;
        if !item.is_renameable() {
            return Err(ModelError::RenameUnavailable);
        }
        Ok(ModelUpdate {
            changed: false,
            intents: vec![DesktopIntent::Rename {
                item: id,
                current_label: item.label.clone(),
            }],
        })
    }

    fn move_icon(&mut self, item: DesktopItemId, cell: Cell) -> Result<ModelUpdate, ModelError> {
        self.require_item(item)?;
        if !self.grid.move_to(item, cell) {
            return Ok(ModelUpdate::default());
        }
        Ok(self.positions_update())
    }

    fn move_icons(
        &mut self,
        items: &[DesktopItemId],
        anchor: DesktopItemId,
        cell: Cell,
    ) -> Result<ModelUpdate, ModelError> {
        for item in items {
            self.require_item(*item)?;
        }
        self.require_item(anchor)?;
        if !self.grid.move_group(items, anchor, cell) {
            return Ok(ModelUpdate::default());
        }
        Ok(self.positions_update())
    }

    fn resize_grid(&mut self, config: GridConfig) -> ModelUpdate {
        self.grid.resize(config);
        self.selection.retain_layout_items(&self.grid);
        self.positions_update()
    }

    fn context_menu(&self, clipboard_has_files: bool, desktop_read_only: bool) -> ModelUpdate {
        let selected: Vec<_> = self
            .selection
            .selected()
            .filter_map(|id| self.items.get(&id))
            .collect();
        let request = if selected.is_empty() {
            MenuRequest::Background {
                desktop_read_only,
                clipboard_has_files,
            }
        } else {
            MenuRequest::Selection {
                items: &selected,
                clipboard_has_files,
            }
        };
        ModelUpdate {
            changed: false,
            intents: vec![DesktopIntent::ShowContextMenu(
                ContextMenuPolicy.entries(request),
            )],
        }
    }

    fn flush_events(&mut self) -> ModelUpdate {
        let intents = self.events.drain().into_iter().map(event_intent).collect();
        ModelUpdate {
            changed: true,
            intents,
        }
    }

    fn positions_update(&self) -> ModelUpdate {
        let positions = self
            .grid
            .placements()
            .into_iter()
            .map(|entry| (entry.item, entry.cell))
            .collect();
        ModelUpdate {
            changed: true,
            intents: vec![DesktopIntent::PersistPositions(positions)],
        }
    }

    fn wallpaper_update(&self) -> ModelUpdate {
        ModelUpdate {
            changed: true,
            intents: vec![DesktopIntent::RedrawWallpaper {
                workspace: self.workspace.clone(),
            }],
        }
    }

    fn require_item(&self, id: DesktopItemId) -> Result<&DesktopItem, ModelError> {
        self.items.get(&id).ok_or(ModelError::UnknownItem(id))
    }
}

fn changed() -> ModelUpdate {
    ModelUpdate {
        changed: true,
        intents: Vec::new(),
    }
}

fn event_intent(event: CoalescedEvent) -> DesktopIntent {
    match event {
        CoalescedEvent::Created(path) | CoalescedEvent::MetadataChanged(path) => {
            DesktopIntent::RefreshPath(path.as_str().to_owned())
        }
        CoalescedEvent::Deleted(path) => DesktopIntent::RemovePath(path.as_str().to_owned()),
        CoalescedEvent::Renamed { from, to } => DesktopIntent::RenamePath {
            from: from.as_str().to_owned(),
            to: to.as_str().to_owned(),
        },
        CoalescedEvent::MountArrived(mount) => DesktopIntent::MountArrived {
            mount_id: mount.id.as_str().to_owned(),
            root: mount.root.as_str().to_owned(),
            read_only: mount.read_only,
        },
        CoalescedEvent::MountRemoved(id) => DesktopIntent::MountRemoved {
            mount_id: id.as_str().to_owned(),
            cancel_operations: true,
        },
        CoalescedEvent::RescanRequired { .. } => DesktopIntent::ReenumerateDesktop,
    }
}

#[must_use]
pub fn duplicate_ids(items: &[DesktopItem]) -> BTreeSet<DesktopItemId> {
    let mut seen = BTreeSet::new();
    let mut duplicates = BTreeSet::new();
    for item in items {
        if !seen.insert(item.id) {
            duplicates.insert(item.id);
        }
    }
    duplicates
}
