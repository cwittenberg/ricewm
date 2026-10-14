//! `RiceFiles` actor: file-manager composition over the pure `rice-files` model.

use std::collections::{BTreeMap, VecDeque};

mod files_actions;
mod files_user_actions;
mod files_clipboard;
mod files_completion;
mod files_conflict;
mod files_detail_view;
mod files_details;
mod files_dialogs;
mod files_directory_vfs;
mod files_download;
mod files_drag;
mod files_events;
mod files_external_drag;
mod files_interaction;
mod files_menu;
mod files_menu_view;
mod files_mutation_recovery;
mod files_mutation_steps;
mod files_open;
mod files_operation_ui;
mod files_pane;
mod files_pane_widgets;
mod files_preferences;
mod files_options;
mod files_preview;
mod files_preview_view;
mod files_progress;
mod files_properties;
mod files_remote;
mod files_search;
mod files_tree;
mod files_vfs;
mod files_view;

use files_actions::FileControls;
use files_directory_vfs::PendingDirectory;
use files_external_drag::{ActiveExternalDrag, PendingExternalDrop};
use files_interaction::PointerSelection;
use files_menu_view::{MenuChecks, MenuPresentation, menu_patches};
use files_open::{OpenWithState, PendingApplication};
#[cfg(test)]
use files_pane::PaneRuntime;
use files_pane::TabRuntime;
use files_preferences::PendingPreferences;
use files_preview::PreviewState;
use files_tree::PendingFolderTreeProbe;
use files_vfs::PendingMutation;
#[cfg(test)]
use files_view::ENTRY_BASE;
use files_view::{
    BACK, BREADCRUMB, COMPLETION_BASE, CONTENT, ContentView, DELETE, FOLDER_TREE, FORWARD,
    GRID_VIEW, HOME, LIST_VIEW, MAIN_SPLIT, MOVE, MOVE_FIELD, NEW_TAB, NEW_WINDOW, PLACE_DESKTOP,
    PLACE_DOCUMENTS, PLACE_HOME, PLACE_MOUNTS, PLACE_PICTURES, PLACE_TRASH, PLACES_TOGGLE,
    PREVIEW_TOGGLE, PaneContentView, RELOAD, RENAME, RENAME_FIELD, ROOT, SEARCH, SEARCH_FIELD,
    SIMPLIFIED_VIEW, SORT_EXTENSION, SORT_MODIFIED, SORT_NAME, SORT_SIZE, SORT_TYPE, SPLIT_NARROW,
    SPLIT_TOGGLE, SPLIT_WIDE, TAB_BASE, TAB_CLOSE_BASE, TabView, UP, VIEW_COMPACT, VIEW_DETAILS,
    VIEW_ICONS, VIEW_THUMBNAILS, WINDOW, attach_shell_to_tab_page, content_patches,
    location_patches, status_patches, tab_page_patches, tab_patches, toolbar_patches, window_id,
};
use rice_files::{
    EntryKind, EntryMetadata, FileManager, FilePreferences, GuestPath, ModelError,
    NavigationCommand, NavigationError, PaneSlot, SortDirection, SortKey, SortStrategy, ViewMode,
    sort_entries,
};
use rice_native_app_protocol::{
    AppId, DataProviderOperation, DataProviderRequest, DataProviderResult, DirectoryEntry,
    FileKind, HostEvent, HostEventKind, HostOperation, HostRequest, Message, MountCapabilities,
    MountEntry, MountState, RequestId, UiEvent, UiEventEnvelope, UiPatchEnvelope, WindowId,
};
#[cfg(test)]
use rice_native_app_protocol::{VfsOperation, VfsRequest};
use rice_ui::{Patch, PatchConversionError, RetainedTree, TreeError, WidgetKind};
use serde::Deserialize;

pub(crate) const ENTRYPOINT: &str = "rice-files";
pub(super) const MIN_VIEW_ZOOM_PERMILLE: u16 = 500;
pub(super) const MAX_VIEW_ZOOM_PERMILLE: u16 = 5_000;
const DEFAULT_VIEW_ZOOM_PERMILLE: u16 = 1_000;
const YELLOW_JACKET_VIEW_ZOOM_PERMILLE: u16 = 650;
const MOUNT_REFRESH_TIMER: u64 = 0x5246_4d54;
const RICE_BUS_DEFERRED_REFRESH_TIMER: u64 = 0x5246_4252;
const VFS_CHANGED_TOPIC: &str = "org.ricewm.vfs/changed";
const THEME_CHANGED_TOPIC: &str = "org.ricewm.theme/changed";

fn valid_theme_profile(profile: &str) -> bool {
    !profile.is_empty()
        && profile.len() <= 64
        && profile
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn default_view_zoom_for_theme(profile: &str) -> u16 {
    if profile == "yellow-jacket" {
        YELLOW_JACKET_VIEW_ZOOM_PERMILLE
    } else {
        DEFAULT_VIEW_ZOOM_PERMILLE
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VfsBusPayload {
    filter_index: usize,
    filter: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiceFiles {
    app: AppId,
    window: WindowId,
    sequence: u64,
    model: FileManager,
    model_window: u64,
    home: GuestPath,
    launch_selection: Option<GuestPath>,
    launch_open_with: bool,
    pane_runtimes: std::collections::BTreeMap<u64, TabRuntime>,
    open_with: Option<OpenWithState>,
    pending_application: Option<PendingApplication>,
    abandoned_property_requests: Vec<RequestId>,
    remote: files_remote::RemoteConnection,
    pending_mount_catalog: Option<RequestId>,
    pending_mount_sync: Option<RequestId>,
    mount_entries: Vec<MountEntry>,
    local_files_available: bool,
    preferences: FilePreferences,
    pending_preferences: VecDeque<PendingPreferences>,
    user_actions: Vec<files_user_actions::FileAction>,
    preferences_revision: u64,
    dialog: Option<files_dialogs::Dialog>,
    options: files_options::Options,
    options_dialog: Option<files_options::Options>,
    preview_state: PreviewState,
    preview_positions: BTreeMap<String, u64>,
    preview_fullscreen: bool,
    window_fullscreen: bool,
    preview_restore_fullscreen: bool,
    retired_preview_requests: Vec<RequestId>,
    thumbnail_cache: BTreeMap<GuestPath, RequestId>,
    thumbnail_queue: VecDeque<(GuestPath, String)>,
    thumbnail_pending: Option<(RequestId, GuestPath, String)>,
    sidebar_permille: u16,
    sidebar_visible: bool,
    simplified_view: bool,
    folder_tree: bool,
    tree_directory_cache: BTreeMap<GuestPath, Vec<EntryMetadata>>,
    pending_folder_tree_probe: Option<PendingFolderTreeProbe>,
    trash_nonempty: bool,
    theme_profile: String,
    redmond98: bool,
    redmond_view_explicit: bool,
    views_before_redmond: Option<(ViewMode, ViewMode)>,
    view_zoom_permille: u16,
    search_expanded: bool,
    pending_mutation: Option<PendingMutation>,
    download_after_mutation: bool,
    download_tip_shown: bool,
    pending_mutation_pane: Option<PaneSlot>,
    retired_mutation_requests: Vec<RequestId>,
    retired_directory_requests: Vec<RequestId>,
    pending_clipboard: Option<files_clipboard::PendingClipboard>,
    pointer_selection: Option<PointerSelection>,
    drag_sources: Option<Vec<EntryMetadata>>,
    drag_origin: Option<GuestPath>,
    source_drag: Option<rice_native_app_protocol::DragId>,
    active_external_drag: Option<ActiveExternalDrag>,
    pending_external_drop: Option<PendingExternalDrop>,
    rice_bus_watch_paths: Vec<String>,
    rice_bus_deferred_refresh: [bool; 2],
    tree: RetainedTree,
}

impl RiceFiles {
    pub(crate) fn document_state(&self) -> Option<serde_json::Value> {
        let window = self.model.windows().iter()
            .find(|window| window.id == self.model_window)?;
        let mut paths = std::collections::BTreeSet::new();
        let mut ready = self.pending_mutation.is_none();
        for tab in window.tabs() {
            let runtime = self.pane_runtimes.get(&tab.id)?;
            for slot in [PaneSlot::Primary, PaneSlot::Secondary] {
                if slot == PaneSlot::Secondary && !tab.dual_pane {
                    continue;
                }
                let pane = runtime.pane(slot);
                ready &= pane.pending_directory.is_none();
                if let Some(path) = &pane.loaded_location {
                    if path == tab.pane(slot).location() {
                        paths.insert(path.as_str().to_owned());
                    }
                }
            }
        }
        // Never return a partial inventory as a complete document-state claim.
        if paths.len() > 32 {
            return None;
        }
        Some(serde_json::json!({
            "version": 1,
            "documents": paths.into_iter().collect::<Vec<_>>(),
            "ready": ready,
            "dirty": false,
            "openDocuments": "tabs",
        }))
    }

    #[cfg(test)]
    pub(crate) fn launch(
        app: AppId,
        request: RequestId,
    ) -> Result<(Self, Vec<Message>), FilesActorError> {
        Self::launch_at(app, &[], request)
    }

    pub(crate) fn launch_at(
        app: AppId,
        arguments: &[String],
        request: RequestId,
    ) -> Result<(Self, Vec<Message>), FilesActorError> {
        let home = GuestPath::absolute("/root")?;
        let launch_selection = arguments
            .iter()
            .find_map(|argument| argument.strip_prefix("--select="))
            .and_then(|path| GuestPath::absolute(path).ok());
        let remote_url = arguments.iter().find(|arg| arg.starts_with("smb://") || arg.starts_with("nfs://")).cloned();
        let location = match arguments
            .iter()
            .find(|argument| !argument.starts_with("--"))
        {
            Some(_) if remote_url.is_some() => home.clone(),
            Some(path) => GuestPath::absolute(path)?,
            None => launch_selection
                .as_ref()
                .map(GuestPath::parent)
                .unwrap_or_else(|| home.clone()),
        };
        let theme_profile = arguments
            .iter()
            .find_map(|argument| argument.strip_prefix("--theme-profile="))
            .filter(|profile| valid_theme_profile(profile))
            .unwrap_or("standard")
            .to_owned();
        let simplified_view = arguments.iter().any(|argument| {
            matches!(
                argument.as_str(),
                "--theme-profile=orchard" | "--simplified-view-default=true"
            )
        }) && !arguments
            .iter()
            .any(|argument| argument == "--simplified-view-default=false");
        let redmond98 = theme_profile == "redmond-98";
        let view_zoom_permille = default_view_zoom_for_theme(&theme_profile);
        let folder_tree = (redmond98
            || arguments
                .iter()
                .any(|argument| argument == "--folder-tree-default=true"))
            && !arguments
                .iter()
                .any(|argument| argument == "--folder-tree-default=false");
        let mut model = FileManager::new();
        let model_window = model.open_window(location.clone());
        let pending = PendingDirectory::new(request, location.clone(), None, None);
        let mut pane_runtimes = std::collections::BTreeMap::new();
        let mut runtime = TabRuntime::new(&location);
        runtime.primary.pending_directory = Some(pending);
        runtime.primary.content_notice = String::from("Loading folder contents…");
        pane_runtimes.insert(1, runtime);
        let mut actor = Self {
            app,
            window: window_id(WINDOW),
            sequence: 1,
            model,
            model_window,
            home,
            launch_selection,
            launch_open_with: arguments.iter().any(|argument| argument == "--open-with"),
            pane_runtimes,
            open_with: None,
            pending_application: None,
            abandoned_property_requests: Vec::new(),
            remote: files_remote::RemoteConnection::default(),
            pending_mount_catalog: Some(request),
            pending_mount_sync: None,
            mount_entries: Vec::new(),
            local_files_available: false,
            preferences: FilePreferences::default(),
            pending_preferences: VecDeque::new(),
            user_actions: Vec::new(),
            preferences_revision: 0,
            dialog: None,
            options: files_options::Options::default(),
            options_dialog: None,
            preview_state: PreviewState::Idle,
            preview_positions: BTreeMap::new(),
            preview_fullscreen: false,
            window_fullscreen: false,
            preview_restore_fullscreen: false,
            retired_preview_requests: Vec::new(),
            thumbnail_cache: BTreeMap::new(),
            thumbnail_queue: VecDeque::new(),
            thumbnail_pending: None,
            sidebar_permille: 220,
            sidebar_visible: true,
            simplified_view,
            folder_tree,
            tree_directory_cache: BTreeMap::new(),
            pending_folder_tree_probe: None,
            trash_nonempty: false,
            theme_profile,
            redmond98,
            redmond_view_explicit: false,
            views_before_redmond: None,
            view_zoom_permille,
            search_expanded: false,
            pending_mutation: None,
            download_after_mutation: false,
            download_tip_shown: false,
            pending_mutation_pane: None,
            retired_mutation_requests: Vec::new(),
            retired_directory_requests: Vec::new(),
            pending_clipboard: None,
            pointer_selection: None,
            drag_sources: None,
            drag_origin: None,
            source_drag: None,
            active_external_drag: None,
            pending_external_drop: None,
            rice_bus_watch_paths: Vec::new(),
            rice_bus_deferred_refresh: [false; 2],
            tree: RetainedTree::default(),
        };
        if let Some(url) = remote_url {actor.remote.open_url(&url);}
        let patches = actor.full_patches()?;
        actor.tree.apply(&patches)?;
        let ui = actor.patch_message(&patches)?;
        let vfs = actor.directory_request(PaneSlot::Primary, request, actor.location(), 0);
        let preferences = files_preferences::load(&mut actor, request);
        let mount_timer = actor.mount_refresh_timer();
        let mut messages = vec![
            ui,
            vfs,
            Message::DataProviderRequest(DataProviderRequest {
                request,
                app,
                operation: DataProviderOperation::MountList,
            }),
            preferences,
            mount_timer,
        ];
        actor.sync_rice_bus_subscription(&mut messages)?;
        Ok((actor, messages))
    }

    pub(crate) fn handle_clipboard_response(
        &mut self,
        response: &rice_native_app_protocol::ClipboardResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        files_clipboard::response(self, response)
    }

    pub(crate) fn handle_data_response(
        &mut self,
        response: &rice_native_app_protocol::DataProviderResponse,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_mount_sync == Some(response.request) {
            self.pending_mount_sync = None;
            match &response.result {
                DataProviderResult::Mounts { entries, .. } => {
                    self.apply_mount_catalog(entries);
                    let current = self.location().as_str();
                    if current.starts_with("/run/ricewm/mounts/") && !entries.iter().any(|m| m.state == rice_native_app_protocol::MountState::Connected && (current == m.path || current.starts_with(&(m.path.clone()+"/")))) {
                        self.begin_navigation(NavigationCommand::Enter("/root".to_owned()), request)
                    } else {self.begin_navigation(NavigationCommand::Reload, request)}
                }
                DataProviderResult::Unavailable { reason } => {
                    self.emit_system_notice("error", reason)
                }
                _ => Err(FilesActorError::UnexpectedDataProviderResponse),
            }
        } else if self.pending_mount_catalog == Some(response.request) {
            self.pending_mount_catalog = None;
            match &response.result {
                DataProviderResult::Mounts { entries, .. } => self.apply_mount_catalog(entries),
                DataProviderResult::Unavailable { .. } => {
                    self.mount_entries.clear();
                    self.local_files_available = false;
                }
                _ => return Err(FilesActorError::UnexpectedDataProviderResponse),
            }
            self.emit_full().map(|message| vec![message])
        } else if let Some(result) = self.handle_remote_response(response, request) {
            result
        } else {
            self.handle_application_response(response, request)
        }
    }

    pub(crate) fn handle_settings_response(
        &mut self,
        response: &rice_native_app_protocol::SettingsResponse,
    ) -> Result<Vec<Message>, FilesActorError> {
        files_preferences::handle_response(self, response)
    }

    pub(crate) fn handle_host_event(
        &mut self,
        event: &HostEvent,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if event.app != self.app {
            return Ok(Vec::new());
        }
        if event.event
            == (HostEventKind::Timer {
                token: MOUNT_REFRESH_TIMER,
            })
        {
            return Ok(self.refresh_mount_catalog(request).into_iter().collect());
        }
        if let HostEventKind::RiceBus { topic, payload } = &event.event {
            if topic == "org.ricewm.files/open-tab" {
                let value: serde_json::Value = serde_json::from_str(payload)
                    .map_err(|_| FilesActorError::UnexpectedHostEvent)?;
                let path = value.get("path").and_then(serde_json::Value::as_str)
                    .filter(|path| path.len() <= 4096)
                    .ok_or(FilesActorError::UnexpectedHostEvent)?;
                let target = GuestPath::absolute(path)?;
                // A retried request must not duplicate an already open folder.
                if self.model.windows().iter().flat_map(|window| window.tabs())
                    .any(|tab| tab.primary.location() == &target
                        || (tab.dual_pane && tab.secondary.location() == &target)) {
                    return Ok(Vec::new());
                }
                return self.open_tab_at(target, request);
            }
            if topic == "org.rice.media.state" {
                return self.handle_preview_host_event(event, request);
            }
            if topic == "org.ricewm.settings/changed" {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) {
                    if let Some(raw) = value.get("values").and_then(|values| values.get("desktop.actions")).and_then(|value| value.as_str()) {
                        self.user_actions = files_user_actions::decode(raw);
                        if let Some(extra) = value.get("values").and_then(|values| values.get("desktop.app-actions")).and_then(|value| value.as_str()) {
                            self.user_actions.extend(files_user_actions::decode(extra).into_iter().take(32));
                        }
                        return self.emit_full().map(|message| vec![message]);
                    }
                }
                return Ok(Vec::new());
            }
            if topic == THEME_CHANGED_TOPIC {
                return self.apply_theme_profile(payload, request);
            }
            if topic == VFS_CHANGED_TOPIC {
                let payload = serde_json::from_str::<VfsBusPayload>(payload)
                    .map_err(|_| FilesActorError::UnexpectedHostEvent)?;
                return self.handle_vfs_bus_event(payload, request);
            }
            return Ok(Vec::new());
        }
        if event.event
            == (HostEventKind::Timer {
                token: RICE_BUS_DEFERRED_REFRESH_TIMER,
            })
        {
            return self.resume_deferred_bus_refresh(request);
        }
        self.handle_preview_host_event(event, request)
    }

    fn apply_theme_profile(
        &mut self,
        profile: &str,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if !valid_theme_profile(profile) || profile == self.theme_profile {
            return Ok(Vec::new());
        }
        let entering_redmond = !self.redmond98 && profile == "redmond-98";
        let leaving_redmond = self.redmond98 && profile != "redmond-98";
        if entering_redmond {
            let tab = self.tab()?;
            self.views_before_redmond = Some((tab.primary.view, tab.secondary.view));
            self.redmond_view_explicit = false;
        }
        if leaving_redmond && !self.redmond_view_explicit {
            if let Some((primary, secondary)) = self.views_before_redmond.take() {
                let tab = self.tab_mut()?;
                tab.primary.set_view(primary);
                tab.secondary.set_view(secondary);
            }
        }
        self.theme_profile = profile.to_owned();
        self.view_zoom_permille = default_view_zoom_for_theme(profile);
        self.redmond98 = profile == "redmond-98";
        if self.redmond98 && !self.redmond_view_explicit {
            self.pane_mut()?.set_view(ViewMode::Icons);
        }
        self.simplified_view = !self.redmond98;
        self.folder_tree = self.redmond98;
        Ok(vec![
            self.emit_full()?,
            files_preferences::load(self, request),
        ])
    }

    pub(crate) fn sync_rice_bus_subscription(
        &mut self,
        messages: &mut Vec<Message>,
    ) -> Result<(), FilesActorError> {
        let paths = self.visible_view_paths()?;
        if paths == self.rice_bus_watch_paths {
            return Ok(());
        }
        self.rice_bus_watch_paths.clone_from(&paths);
        self.rice_bus_deferred_refresh = [false; 2];
        messages.push(Message::HostRequest(HostRequest {
            app: self.app,
            window: self.window,
            operation: HostOperation::RiceBusSubscribe {
                topic: String::from(VFS_CHANGED_TOPIC),
                filters: paths,
            },
        }));
        Ok(())
    }

    fn visible_view_paths(&self) -> Result<Vec<String>, FilesActorError> {
        let tab = self.tab()?;
        let mut paths = vec![tab.primary.location().as_str().to_owned()];
        if tab.dual_pane {
            paths.push(tab.secondary.location().as_str().to_owned());
        }
        Ok(paths)
    }

    fn handle_vfs_bus_event(
        &mut self,
        payload: VfsBusPayload,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let paths = self.visible_view_paths()?;
        if paths.get(payload.filter_index) != Some(&payload.filter) {
            return Ok(Vec::new());
        }
        let slot = match payload.filter_index {
            0 => PaneSlot::Primary,
            1 if self.tab()?.dual_pane => PaneSlot::Secondary,
            _ => return Ok(Vec::new()),
        };
        let tab = self.active_tab_id()?;
        if self.pending_mutation.is_some() || self.runtime_at(tab, slot)?.is_enumerating() {
            self.rice_bus_deferred_refresh[payload.filter_index] = true;
            return Ok(vec![self.deferred_bus_timer(true)]);
        }
        self.reload_visible_slot(slot, request)
    }

    fn resume_deferred_bus_refresh(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_mutation.is_some() {
            return Ok(Vec::new());
        }
        let tab = self.active_tab_id()?;
        let visible = if self.tab()?.dual_pane { 2 } else { 1 };
        let Some(index) = (0..visible).find(|index| {
            let slot = if *index == 0 {
                PaneSlot::Primary
            } else {
                PaneSlot::Secondary
            };
            self.rice_bus_deferred_refresh[*index]
                && self
                    .runtime_at(tab, slot)
                    .is_ok_and(|runtime| !runtime.is_enumerating())
        }) else {
            if !self.rice_bus_deferred_refresh[..visible]
                .iter()
                .any(|dirty| *dirty)
            {
                return Ok(vec![self.deferred_bus_timer(false)]);
            }
            return Ok(Vec::new());
        };
        self.rice_bus_deferred_refresh[index] = false;
        let slot = if index == 0 {
            PaneSlot::Primary
        } else {
            PaneSlot::Secondary
        };
        let mut messages = self.reload_visible_slot(slot, request)?;
        if !self.rice_bus_deferred_refresh[..visible]
            .iter()
            .any(|dirty| *dirty)
        {
            messages.push(self.deferred_bus_timer(false));
        }
        Ok(messages)
    }

    fn reload_visible_slot(
        &mut self,
        slot: PaneSlot,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let plan = self
            .tab()?
            .pane(slot)
            .plan_navigation(NavigationCommand::Reload, &self.home)?;
        let target = plan.target().clone();
        let tab = self.active_tab_id()?;
        self.retire_pending_directory(slot)?;
        self.runtime_at_mut(tab, slot)?.pending_directory = Some(PendingDirectory::new(
            request,
            target.clone(),
            Some(plan),
            None,
        ));
        self.runtime_at_mut(tab, slot)?.content_notice = format!("Loading {target}…");
        Ok(vec![
            self.emit_full()?,
            self.directory_request(slot, request, &target, 0),
        ])
    }

    fn deferred_bus_timer(&self, enabled: bool) -> Message {
        Message::HostRequest(HostRequest {
            app: self.app,
            window: self.window,
            operation: HostOperation::SetTimer {
                token: RICE_BUS_DEFERRED_REFRESH_TIMER,
                interval_millis: 100,
                enabled,
            },
        })
    }

    fn mount_refresh_timer(&self) -> Message {
        Message::HostRequest(HostRequest {
            app: self.app,
            window: self.window,
            operation: HostOperation::SetTimer {
                token: MOUNT_REFRESH_TIMER,
                interval_millis: 1_000,
                enabled: true,
            },
        })
    }

    pub(super) fn refresh_mount_catalog(&mut self, request: RequestId) -> Option<Message> {
        if self.pending_mount_catalog.is_some() {
            return None;
        }
        self.pending_mount_catalog = Some(request);
        Some(Message::DataProviderRequest(DataProviderRequest {
            request,
            app: self.app,
            operation: DataProviderOperation::MountList,
        }))
    }

    fn local_mount_root(&self) -> &str {
        if self.mount_entries.iter().any(|entry| {
            entry.path.starts_with("/mnt/")
                && !entry.path.starts_with("/mnt/local/")
                && matches!(entry.state, MountState::Connected | MountState::Syncing)
        }) {
            "/mnt"
        } else {
            "/mnt/local"
        }
    }

    fn apply_mount_catalog(&mut self, entries: &[MountEntry]) {
        self.local_files_available = entries.iter().any(|entry| {
            entry
                .path
                .strip_prefix("/mnt/")
                .is_some_and(|suffix| !suffix.is_empty())
                && matches!(entry.state, MountState::Connected | MountState::Syncing)
        });
        self.mount_entries = entries.to_vec();
    }

    fn current_syncable_mount(&self) -> Option<&MountEntry> {
        let location = self.location().as_str();
        self.mount_entries
            .iter()
            .filter(|entry| {
                (location == entry.path || location.starts_with(&format!("{}/", entry.path)))
                    && entry.state == MountState::Connected
                    && entry.capabilities.contains(MountCapabilities::SYNC)
            })
            .max_by_key(|entry| entry.path.len())
    }

    pub(super) fn refresh_current_location(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_mount_sync.is_some() {
            return self.emit_notice("The mounted folder is already being refreshed");
        }
        if let Some(mount_id) = self.current_syncable_mount().map(|entry| entry.id.clone()) {
            self.pending_mount_sync = Some(request);
            return Ok(vec![Message::DataProviderRequest(DataProviderRequest {
                request,
                app: self.app,
                operation: DataProviderOperation::MountSync { mount_id },
            })]);
        }
        let mut messages = self.begin_navigation(NavigationCommand::Reload, request)?;
        if let Some(catalog) = self.refresh_mount_catalog(request) {
            messages.push(catalog);
        }
        Ok(messages)
    }

    fn begin_navigation(
        &mut self,
        command: NavigationCommand,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.pending_mutation.is_some() {
            return self.emit_notice("A file operation is already in progress");
        }
        let slot = self.active_slot()?;
        let plan = match self.pane()?.plan_navigation(command, &self.home) {
            Ok(plan) => plan,
            Err(NavigationError::NoHistory) => {
                return self.emit_notice("No matching location in history");
            }
            Err(error) => return Err(error.into()),
        };
        let target = plan.target().clone();
        self.cancel_folder_tree_probe();
        self.retire_pending_directory(slot)?;
        self.runtime_mut()?.pending_directory = Some(PendingDirectory::new(
            request,
            target.clone(),
            Some(plan),
            None,
        ));
        self.runtime_mut()?.content_notice = format!("Loading {target}…");
        let mut messages = self.dismiss_preview()?;
        messages.push(self.directory_request(slot, request, &target, 0));
        Ok(messages)
    }

    fn change_view(&mut self, view: ViewMode) -> Result<Vec<Message>, FilesActorError> {
        if self.redmond98 {
            self.redmond_view_explicit = true;
        }
        self.pane_mut()?.set_view(view);
        self.emit_full().map(|message| vec![message])
    }

    fn folder_tree_entries(&self) -> Vec<EntryMetadata> {
        let mut entries = self
            .tree_directory_cache
            .values()
            .flatten()
            .filter(|entry| {
                entry.kind == EntryKind::Directory
                    && !matches!(entry.path.as_str(), "/root/Desktop" | "/root/Documents")
            })
            .cloned()
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        entries.dedup_by(|left, right| left.path == right.path);
        entries
    }

    fn folder_tree_path_for_target(&self, target: u64) -> Option<GuestPath> {
        let static_path = match target {
            PLACE_HOME => Some("/root"),
            PLACE_DESKTOP => Some("/root/Desktop"),
            PLACE_DOCUMENTS => Some("/root/Documents"),
            files_view::PLACE_DOWNLOADS => Some("/root/Downloads"),
            PLACE_PICTURES => Some("/root/Pictures"),
            files_view::PLACE_VIDEOS => Some("/root/Videos"),
            PLACE_TRASH => Some("/root/.local/share/Trash/files"),
            PLACE_MOUNTS => Some(self.local_mount_root()),
            _ => None,
        };
        if let Some(path) = static_path {
            return GuestPath::absolute(path).ok();
        }
        if let Some(index) = files_view::folder_tree_entry_index(target) {
            return self
                .folder_tree_entries()
                .into_iter()
                .nth(index)
                .map(|entry| entry.path);
        }
        files_view::folder_tree_ancestor_index(target)
            .and_then(|index| {
                files_view::folder_tree_ancestor_paths(
                    self.location().as_str(),
                    self.local_mount_root(),
                )
                .get(index)
                .cloned()
            })
            .and_then(|(path, _)| GuestPath::absolute(&path).ok())
    }

    fn open_tab(&mut self, request: RequestId) -> Result<Vec<Message>, FilesActorError> {
        self.open_tab_at(self.location().clone(), request)
    }

    fn open_tab_at(
        &mut self,
        target: GuestPath,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.runtime()?.pending_directory.is_some() || self.pending_mutation.is_some() {
            return self.emit_notice("Wait for the current file operation to finish");
        }
        let id = self
            .model
            .window_mut(self.model_window)?
            .open_tab(target.clone());
        let mut runtime = TabRuntime::new(&target);
        self.options.configure(&mut runtime.primary);
        self.options.configure(&mut runtime.secondary);
        runtime.primary.pending_directory =
            Some(PendingDirectory::new(request, target.clone(), None, None));
        runtime.primary.content_notice = format!("Loading {target}…");
        self.pane_runtimes.insert(id, runtime);
        let mut messages = self.dismiss_preview()?;
        messages.push(self.directory_request(PaneSlot::Primary, request, &target, 0));
        Ok(messages)
    }

    fn selected_directory(&self) -> Option<EntryMetadata> {
        let selected = self.selected_entries();
        (selected.len() == 1 && selected[0].kind == EntryKind::Directory)
            .then(|| selected[0].clone())
    }

    fn open_selected_directory_tab(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.selected_directory() else {
            return self.emit_notice("Select one folder to open in a new tab");
        };
        self.open_tab_at(entry.path, request)
    }

    fn open_selected_directory_window(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let Some(entry) = self.selected_directory() else {
            return self.emit_notice("Select one folder to open in a new window");
        };
        Ok(vec![Message::Launch {
            app: self.app,
            entrypoint: String::from(ENTRYPOINT),
            arguments: vec![entry.path.as_str().to_owned()],
        }])
    }

    fn open_folder_in_terminal(&mut self) -> Result<Vec<Message>, FilesActorError> {
        let selected = self.selected_entries();
        let path = match selected.as_slice() {
            [] => self.location(),
            [entry] if entry.kind == EntryKind::Directory => &entry.path,
            _ => return self.emit_notice("Select one folder to open in Terminal"),
        };
        Ok(vec![Message::Launch {
            app: self.app,
            entrypoint: String::from("rice-term"),
            arguments: vec![format!("cd -- {}", files_open::shell_quote(path.as_str()))],
        }])
    }

    fn set_view_zoom(&mut self, value: u16) -> Result<Vec<Message>, FilesActorError> {
        self.view_zoom_permille = value.clamp(MIN_VIEW_ZOOM_PERMILLE, MAX_VIEW_ZOOM_PERMILLE);
        self.emit_full().map(|message| vec![message])
    }

    fn adjust_view_zoom(&mut self, delta: i16) -> Result<Vec<Message>, FilesActorError> {
        let next = i32::from(self.view_zoom_permille) + i32::from(delta);
        self.set_view_zoom(
            u16::try_from(next.clamp(
                i32::from(MIN_VIEW_ZOOM_PERMILLE),
                i32::from(MAX_VIEW_ZOOM_PERMILLE),
            ))
            .unwrap_or(1_000),
        )
    }

    fn activate_tab(
        &mut self,
        id: u64,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        if self.tab()?.id == id {
            return Ok(Vec::new());
        }
        if self.runtime()?.pending_directory.is_some() || self.pending_mutation.is_some() {
            return self.emit_notice("Wait for the current file operation to finish");
        }
        self.model.window_mut(self.model_window)?.activate_tab(id)?;
        if self.pane_runtimes.contains_key(&id) {
            return self.dismiss_preview();
        }
        let target = self.location().clone();
        let mut runtime = TabRuntime::new(&target);
        self.options.configure(&mut runtime.primary);
        self.options.configure(&mut runtime.secondary);
        runtime.primary.pending_directory =
            Some(PendingDirectory::new(request, target.clone(), None, None));
        runtime.primary.content_notice = format!("Loading {target}…");
        self.pane_runtimes.insert(id, runtime);
        let mut messages = self.dismiss_preview()?;
        messages.push(self.directory_request(PaneSlot::Primary, request, &target, 0));
        Ok(messages)
    }

    fn close_tab(&mut self, id: u64) -> Result<Vec<Message>, FilesActorError> {
        if self.runtime()?.pending_directory.is_some() || self.pending_mutation.is_some() {
            return self.emit_notice("Wait for the current file operation to finish");
        }
        self.model.window_mut(self.model_window)?.close_tab(id)?;
        self.pane_runtimes.remove(&id);
        self.dismiss_preview()
    }

    fn change_sort(&mut self, key: SortKey) -> Result<Vec<Message>, FilesActorError> {
        let current = self.runtime()?.sort;
        let sort = if current.key == key {
            SortStrategy {
                direction: match current.direction {
                    SortDirection::Ascending => SortDirection::Descending,
                    SortDirection::Descending => SortDirection::Ascending,
                },
                ..current
            }
        } else {
            SortStrategy { key, ..current }
        };
        self.apply_sort(sort)
    }

    fn apply_sort(&mut self, sort: SortStrategy) -> Result<Vec<Message>, FilesActorError> {
        let runtime = self.runtime_mut()?;
        runtime.sort = sort;
        sort_entries(&mut runtime.entries, sort);
        sort_entries(&mut runtime.directory_entries, sort);
        for children in runtime.expanded_directories.values_mut() {
            sort_entries(children, sort);
        }
        self.emit_full().map(|message| vec![message])
    }

    fn resize_split(&mut self, delta: i16) -> Result<Vec<Message>, FilesActorError> {
        let current = self.tab()?.split_permille();
        let next = i32::from(current) + i32::from(delta);
        self.tab_mut()?
            .resize_split(u16::try_from(next.clamp(0, 1_000)).unwrap_or_default());
        self.emit_full().map(|message| vec![message])
    }

    fn selected_entries(&self) -> Vec<EntryMetadata> {
        let Ok(pane) = self.pane() else {
            return Vec::new();
        };
        self.visible_entries()
            .iter()
            .filter(|entry| pane.selection().any(|path| path == &entry.path))
            .cloned()
            .collect()
    }

    fn emit_notice(&mut self, notice: &str) -> Result<Vec<Message>, FilesActorError> {
        self.emit_system_notice("info", notice)
    }

    fn emit_system_notice(
        &mut self,
        kind: &str,
        notice: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        self.runtime_mut()?.content_notice.clear();
        Ok(vec![
            self.emit_full()?,
            self.system_notification(kind, notice),
        ])
    }

    fn system_notification(&self, kind: &str, message: &str) -> Message {
        Message::HostRequest(HostRequest {
            app: self.app,
            window: self.window,
            operation: HostOperation::ShowNotification {
                kind: kind.to_owned(),
                title: String::from(rice_i18n::t("Files")),
                message: message.to_owned(),
            },
        })
    }

    pub(crate) fn emit_full(&mut self) -> Result<Message, FilesActorError> {
        let patches = self.full_patches()?;
        self.tree.apply(&patches)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(FilesActorError::SequenceExhausted)?;
        self.patch_message(&patches)
    }

    fn patch_message(&self, patches: &[Patch]) -> Result<Message, FilesActorError> {
        Ok(Message::UiPatch(UiPatchEnvelope {
            app: self.app,
            window: self.window,
            sequence: self.sequence,
            patches: patches
                .iter()
                .map(Patch::to_protocol)
                .collect::<Result<_, _>>()?,
        }))
    }

    fn full_patches(&self) -> Result<Vec<Patch>, FilesActorError> {
        let mut patches = vec![Patch::Clear];
        files_view::create(
            &mut patches,
            ROOT,
            None,
            WidgetKind::Column,
            Some(if self.preview_fullscreen {
                "panel:preview-fullscreen"
            } else if self.redmond98 {
                "panel:files-shell:simplified:redmond98"
            } else if self.simplified_view {
                "panel:files-shell:simplified"
            } else {
                "panel:files-shell"
            }),
            None,
        );
        if self.preview_fullscreen {
            files_preview_view::fullscreen_patches(&mut patches, &self.preview_state);
            return Ok(patches);
        }
        let active_slot = self.active_slot()?;
        let selected: Vec<_> = self.pane()?.selection().collect();
        let selected_entries = self.selected_entries();
        let runtime = self.runtime()?;
        menu_patches(
            &mut patches,
            &MenuPresentation {
                redmond98: self.redmond98,
                view: self.pane()?.view,
                sort: runtime.sort.key,
                direction: runtime.sort.direction,
                checked: MenuChecks::new(
                    self.sidebar_visible,
                    runtime.controls.preview_visible,
                    self.tab()?.dual_pane,
                    self.simplified_view,
                    self.folder_tree,
                ),
                has_selection: !selected.is_empty(),
                can_publish: matches!(selected_entries.as_slice(), [entry] if entry.kind == EntryKind::File && entry.path.as_str().to_ascii_lowercase().ends_with(".riceapp")),
                can_mutate_selection: !selected_entries.is_empty()
                    && selected_entries
                        .iter()
                        .all(|entry| !files_preferences::is_builtin(entry.path.as_str())),
                can_open_with: selected_entries
                    .first()
                    .is_some_and(|entry| entry.kind == EntryKind::File),
                selected_directory: self.selected_directory().is_some(),
                can_gzip: matches!(
                    selected_entries.as_slice(),
                    [entry] if entry.kind == EntryKind::File
                ),
                can_extract: matches!(
                    selected_entries.as_slice(),
                    [entry] if entry.kind == EntryKind::File
                        && files_actions::is_archive_name(&entry.name)
                ),
            },
        );
        files_user_actions::menu_patches(self, &mut patches);
        let window = self
            .model
            .windows()
            .iter()
            .find(|window| window.id == self.model_window)
            .ok_or(ModelError::UnknownWindow(self.model_window))?;
        let tab_views = window
            .tabs()
            .iter()
            .map(|tab| TabView {
                id: tab.id,
                location: tab.primary.location().as_str(),
                active: tab.id == window.active_tab().id,
            })
            .collect::<Vec<_>>();
        tab_patches(&mut patches, &tab_views);
        let tab_control_visible = tab_views.len() >= 2;
        tab_page_patches(
            &mut patches,
            tab_control_visible,
            self.redmond98 || self.simplified_view,
        );
        location_patches(
            &mut patches,
            &runtime.location_text,
            runtime.completion.suggestions(),
            runtime.completion_index,
            self.redmond98,
        );
        toolbar_patches(
            &mut patches,
            self.search_expanded,
            &runtime.controls.search_text,
            self.redmond98,
        );
        let tab = self.tab()?;
        let tab_runtime = self
            .pane_runtimes
            .get(&tab.id)
            .ok_or(ModelError::UnknownTab(tab.id))?;
        let primary_entries = self.visible_entries_at(PaneSlot::Primary);
        let primary_selected = tab.primary.selection().collect::<Vec<_>>();
        let secondary_entries = self.visible_entries_at(PaneSlot::Secondary);
        let secondary_selected = tab.secondary.selection().collect::<Vec<_>>();
        let primary = PaneContentView {
            location: tab.primary.location().as_str(),
            entries: &primary_entries,
            selected: &primary_selected,
            focused_entry: if self.remote.is_open() {None} else {tab_runtime.primary.selection_caret.as_ref()},
            mode: tab.primary.view,
            sort: tab_runtime.primary.sort,
            expanded_directories: &tab_runtime.primary.expanded_directories,
            scroll: tab.primary.scroll.vertical,
            focused: active_slot == PaneSlot::Primary && !self.remote.is_open(),
            zoom_permille: self.view_zoom_permille,
            thumbnails: &self.thumbnail_cache,
            content_notice: &tab_runtime.primary.content_notice,
            loading: tab_runtime.primary.pending_directory.is_some(),
        };
        let secondary = tab.dual_pane.then_some(PaneContentView {
            location: tab.secondary.location().as_str(),
            entries: &secondary_entries,
            selected: &secondary_selected,
            focused_entry: if self.remote.is_open() {None} else {tab_runtime.secondary.selection_caret.as_ref()},
            mode: tab.secondary.view,
            sort: tab_runtime.secondary.sort,
            expanded_directories: &tab_runtime.secondary.expanded_directories,
            scroll: tab.secondary.scroll.vertical,
            focused: active_slot == PaneSlot::Secondary && !self.remote.is_open(),
            zoom_permille: self.view_zoom_permille,
            thumbnails: &self.thumbnail_cache,
            content_notice: &tab_runtime.secondary.content_notice,
            loading: tab_runtime.secondary.pending_directory.is_some(),
        });
        content_patches(
            &mut patches,
            &ContentView {
                primary,
                secondary,
                preview: runtime
                    .controls
                    .preview_visible
                    .then_some(&self.preview_state),
                sidebar_visible: self.sidebar_visible,
                // The places pane is user-sized in every theme. Redmond used
                // to force 36% here on every redraw, which both made the tree
                // much wider than Explorer and effectively defeated dragging.
                sidebar_permille: self.sidebar_permille,
                split_permille: self.tab()?.split_permille(),
                local_files_available: self.local_files_available,
                local_mount_root: self.local_mount_root(),
                mounts: &self.mount_entries,
                bookmarks: self.preferences.bookmarks(),
                folder_tree: self.folder_tree,
                tree_directories: &self.tree_directory_cache,
                trash_nonempty: self.trash_nonempty,
                simplified_view: self.simplified_view,
                redmond98: self.redmond98,
            },
        )?;
        if let Some(progress) = self.pending_mutation.as_ref().map(PendingMutation::view) {
            files_progress::patches(&mut patches, &progress);
        }
        status_patches(
            &mut patches,
            self.location().as_str(),
            self.pane()?.selection().count(),
            runtime.entries.len(),
            runtime
                .entries
                .iter()
                .fold(0_u64, |total, entry| total.saturating_add(entry.size)),
            self.view_zoom_permille,
            self.redmond98,
            runtime.pending_directory.as_ref().map(|pending| pending.loaded_count()),
        );
        attach_shell_to_tab_page(&mut patches, tab_control_visible);
        if let Some(open_with) = &self.open_with {
            files_open::dialog_patches(&mut patches, open_with);
        }
        if let Some(options) = &self.options_dialog {
            files_options::patches(&mut patches, options);
        }
        if let Some(dialog) = &self.dialog {
            files_dialogs::patches(&mut patches, dialog, self.redmond98);
        }
        files_remote::patches(&mut patches, &self.remote);
        Ok(patches)
    }

    #[cfg(test)]
    pub(crate) fn model(&self) -> &FileManager {
        &self.model
    }

    #[cfg(test)]
    fn runtime_for_test(&self) -> &PaneRuntime {
        self.runtime().expect("active pane runtime")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FilesActorError {
    Tree(TreeError),
    Conversion(PatchConversionError),
    Navigation(NavigationError),
    Model(ModelError),
    UnexpectedVfsResponse,
    UnexpectedMediaResponse,
    InvalidDirectoryEntry,
    EntryLimit,
    SequenceExhausted,
    UnexpectedClipboardResponse,
    UnexpectedDrag,
    UnexpectedDataProviderResponse,
    UnexpectedSettingsResponse,
    UnexpectedHostEvent,
    InvalidSettings,
    SettingsTooLarge,
}

impl From<TreeError> for FilesActorError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}

impl From<PatchConversionError> for FilesActorError {
    fn from(error: PatchConversionError) -> Self {
        Self::Conversion(error)
    }
}

impl From<NavigationError> for FilesActorError {
    fn from(error: NavigationError) -> Self {
        Self::Navigation(error)
    }
}

impl From<ModelError> for FilesActorError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

fn convert_entries(
    parent: &GuestPath,
    entries: &[DirectoryEntry],
) -> Result<Vec<EntryMetadata>, FilesActorError> {
    entries
        .iter()
        .map(|entry| {
            if entry.name.is_empty()
                || entry.name == "."
                || entry.name == ".."
                || entry.name.contains('/')
            {
                return Err(FilesActorError::InvalidDirectoryEntry);
            }
            let path = parent.join(&entry.name)?;
            let mut metadata = EntryMetadata::new(
                path,
                match entry.metadata.kind {
                    FileKind::File => EntryKind::File,
                    FileKind::Directory => EntryKind::Directory,
                    FileKind::Symlink => EntryKind::Symlink,
                    FileKind::Other => EntryKind::Other,
                },
                entry.metadata.size,
                entry.metadata.modified_millis,
            )
            .with_mode(entry.metadata.mode);
            if let Some(mime_type) = &entry.mime_type {
                metadata = metadata.with_media_type(mime_type);
            }
            Ok(metadata)
        })
        .collect()
}

#[cfg(test)]
#[path = "files/application_tests.rs"]
mod application_tests;
#[cfg(test)]
mod cancellation_protocol_tests;
#[cfg(test)]
mod conflict_recovery_tests;
#[cfg(test)]
mod dual_pane_tests;
#[cfg(test)]
mod functional_tests;
#[cfg(test)]
mod launch_tests;
#[cfg(test)]
mod operation_tests;
#[cfg(test)]
mod properties_actor_tests;
#[cfg(test)]
mod remote_tests;
#[cfg(test)]
mod safety_tests;
#[cfg(test)]
mod source_drag_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ux_tests;

