//! `RiceDesktop` actor backed by the pure `rice-desktop` policy crate.

#[path = "desktop/desktop_actions.rs"]
mod desktop_actions;
mod desktop_clipboard;
mod desktop_create;
mod desktop_empty_trash;
mod desktop_launcher;
mod desktop_link;
mod desktop_menu;
mod desktop_path;
mod desktop_persistence;
mod desktop_properties;
mod desktop_vfs;
mod desktop_view;
mod desktop_watch;
mod notice;
use notice::DesktopNotice;
#[cfg(test)]
mod notice_tests;

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use desktop_actions::{
    ActivationTarget, PendingDesktopApplication, activation_target, media_type_for_path,
};
use desktop_clipboard::{PendingClipboard, PendingTransfer};
use desktop_create::PendingCreate;
use desktop_empty_trash::{PendingEmptyTrash, PendingTrashMenu};
use desktop_launcher::PendingDesktopLaunch;
use desktop_link::{LinkDraft, PendingLinkActivation, PendingLinkIcon};
use desktop_persistence::{
    DesktopPositionStore, load_request as load_positions_request,
    save_request as save_positions_request,
};
use desktop_vfs::{PendingDirectory, PendingMutation, PendingTrashStatus};
use desktop_view::{
    ContextKind, MenuActionSpec, initial_patches, message, replace_item_patches,
    switch_context_patches, update_text, widget_id, window_id,
};
use rice_desktop::{
    Cell, ClickMode, Color, DesktopCommand, DesktopIntent, DesktopItem, DesktopItemId,
    DesktopItemKind, DesktopModel, Direction, GridConfig, ModelError, Rect, Wallpaper,
    WallpaperSettings, WorkspaceId,
};
use rice_native_app_protocol::{
    AppId, DragId, MAX_MEDIA_DIMENSION, MAX_MEDIA_SOURCE_BYTES, MediaKind, MediaRequest,
    MediaResponse, MediaResult, Message, PointerEventKind, RequestId, UiEvent, UiEventEnvelope,
    WidgetId, WindowId,
};
use rice_ui::{
    FileChooser, IconId, Patch, PatchConversionError, RetainedTree, TreeError, ValueChange,
    WidgetKind, WidgetSpec, WidgetState,
};

pub(crate) const ENTRYPOINT: &str = "rice-desktop";
const THEME_CHANGED_TOPIC: &str = "org.ricewm.theme/changed";

pub(super) const WINDOW: u64 = 1;
pub(super) const ROOT: u64 = 1;
pub(super) const MENU_BAR: u64 = 2;
pub(super) const DESKTOP_MENU: u64 = 3;
pub(super) const REFRESH: u64 = 11;
pub(super) const SETTINGS: u64 = 12;
pub(super) const WALLPAPER: u64 = 29;
pub(super) const ICON_GRID: u64 = 31;
pub(super) const RUBBER_BAND: u64 = 32;
pub(super) const RENAME_CANCEL: u64 = 37;
pub(super) const RENAME_CONTENT: u64 = 38;
pub(super) const RENAME_ACTIONS: u64 = 39;
pub(super) const RENAME_DIALOG: u64 = 40;
pub(super) const RENAME_FIELD: u64 = 41;
pub(super) const RENAME: u64 = 42;
pub(super) const DELETE: u64 = 43;
pub(super) const OPEN: u64 = 44;
pub(super) const OPEN_WITH: u64 = 45;
pub(super) const NEW_FOLDER: u64 = 46;
pub(super) const NEW_FILE: u64 = 47;
pub(super) const SELECT_ALL: u64 = 48;
pub(super) const ARRANGE: u64 = 49;
pub(super) const PROPERTIES: u64 = 50;
pub(super) const CUT: u64 = 51;
pub(super) const COPY: u64 = 52;
pub(super) const PASTE: u64 = 53;
pub(super) const LINK: u64 = 54;
pub(super) const EMPTY_TRASH: u64 = 55;
pub(super) const PROPERTIES_DIALOG: u64 = 60;
pub(super) const PROPERTIES_CLOSE: u64 = 61;
pub(super) const NOTICE: u64 = 80;
pub(super) const ITEM_BASE: u64 = 1_000;
pub(super) const CELL_BASE: u64 = 1_000_000;
pub(super) const WATCH_EVENT_TOKEN: u64 = 0x5244_4553_4b57;
pub(super) const TRASH_WATCH_EVENT_TOKEN: u64 = 0x5244_5452_5348;

const DEFAULT_DESKTOP_WIDTH: u32 = 849;
const DEFAULT_DESKTOP_HEIGHT: u32 = 799;
const MAX_DESKTOP_COLUMNS: u16 = 32;
const MAX_DESKTOP_ROWS: u16 = 24;
const DRAG_THRESHOLD_SQUARED: i64 = 144;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiceDesktop {
    app: AppId,
    window: WindowId,
    sequence: u64,
    redmond98: bool,
    content_width: u32,
    content_height: u32,
    model: DesktopModel,
    items: Vec<DesktopItem>,
    rendered_items: Vec<DesktopItemId>,
    next_item_id: u64,
    context_kind: ContextKind,
    context_action_ids: Vec<u64>,
    context_messages: Vec<(u64, &'static str)>,
    retained_notice: Option<DesktopNotice>,
    show_trash: bool,
    show_builtin_icons: bool,
    show_mounts: bool,
    auto_arrange: bool,
    drag: Option<DesktopIconDrag>,
    source_drag: Option<DragId>,
    source_repositioned: bool,
    rubber_band: Option<DesktopRubberBand>,
    wallpaper_request: Option<RequestId>,
    rename_text: String,
    rename_source_path: Option<String>,
    rename_after_refresh: Option<String>,
    pending_directory: Option<PendingDirectory>,
    pending_rename: Option<desktop_vfs::PendingRename>,
    pending_mutation: Option<PendingMutation>,
    pending_trash: Option<desktop_vfs::PendingTrash>,
    pending_trash_status: Option<PendingTrashStatus>,
    trash_nonempty: bool,
    permanent_delete_confirmation: Vec<String>,
    permanent_delete_queue: Vec<String>,
    permanent_delete_total: usize,
    pending_desktop_launch: Option<PendingDesktopLaunch>,
    pending_application: Option<PendingDesktopApplication>,
    thumbnail_cache: BTreeMap<String, RequestId>,
    thumbnail_pending: Option<(RequestId, String, String)>,
    thumbnail_queue: VecDeque<(String, String)>,
    pending_create: Option<PendingCreate>,
    pending_clipboard: Option<PendingClipboard>,
    pending_transfer: Option<PendingTransfer>,
    link_draft: Option<LinkDraft>,
    link_chooser: Option<FileChooser>,
    pending_link_activation: Option<PendingLinkActivation>,
    pending_link_icon: Option<PendingLinkIcon>,
    link_icon_queue: VecDeque<String>,
    link_icon_ids: BTreeMap<String, String>,
    external_drag: Option<(DragId, desktop_clipboard::TransferAction)>,
    pending_trash_menu: Option<PendingTrashMenu>,
    pending_empty_trash: Option<PendingEmptyTrash>,
    pending_properties: Option<desktop_properties::DesktopProperties>,
    positions: DesktopPositionStore,
    positions_load: Option<RequestId>,
    position_saves: BTreeSet<RequestId>,
    auto_arrange_saves: BTreeSet<RequestId>,
    watch_refresh_pending: bool,
    trash_confirm_pending: bool,
    pending_intents: Vec<DesktopIntent>,
    tree: RetainedTree,
}

impl RiceDesktop {
    pub(crate) fn launch(
        app: AppId,
        arguments: &[String],
        request: RequestId,
    ) -> Result<(Self, Vec<Message>), DesktopActorError> {
        let workspace = launch_workspace(arguments);
        let redmond98 = arguments
            .iter()
            .any(|argument| argument == "--theme-profile=redmond-98");
        let model = initial_model(workspace, redmond98);
        let patches = initial_patches(model.grid().config());
        let mut tree = RetainedTree::default();
        tree.apply(&patches)?;
        let ui = message(app, window_id(WINDOW), 1, &patches)?;
        let mut actor = Self {
            app,
            window: window_id(WINDOW),
            sequence: 1,
            redmond98,
            content_width: DEFAULT_DESKTOP_WIDTH,
            content_height: DEFAULT_DESKTOP_HEIGHT,
            model,
            items: Vec::new(),
            rendered_items: Vec::new(),
            next_item_id: 1,
            context_kind: ContextKind::Hidden,
            context_action_ids: Vec::new(),
            context_messages: Vec::new(),
            retained_notice: None,
            show_trash: !arguments.iter().any(|argument| argument == "--hide-trash"),
            show_builtin_icons: !arguments
                .iter()
                .any(|argument| argument == "--hide-builtin-icons"),
            show_mounts: !arguments.iter().any(|argument| argument == "--hide-mounts"),
            auto_arrange: arguments
                .iter()
                .any(|argument| argument == "--auto-arrange"),
            drag: None,
            source_drag: None,
            source_repositioned: false,
            rubber_band: None,
            wallpaper_request: None,
            rename_text: String::new(),
            rename_source_path: None,
            rename_after_refresh: None,
            pending_directory: None,
            pending_rename: None,
            pending_mutation: None,
            pending_trash: None,
            pending_trash_status: None,
            trash_nonempty: false,
            permanent_delete_confirmation: Vec::new(),
            permanent_delete_queue: Vec::new(),
            permanent_delete_total: 0,
            pending_desktop_launch: None,
            pending_application: None,
            thumbnail_cache: BTreeMap::new(),
            thumbnail_pending: None,
            thumbnail_queue: VecDeque::new(),
            pending_create: None,
            pending_clipboard: None,
            pending_transfer: None,
            link_draft: None,
            link_chooser: None,
            pending_link_activation: None,
            pending_link_icon: None,
            link_icon_queue: VecDeque::new(),
            link_icon_ids: BTreeMap::new(),
            external_drag: None,
            pending_trash_menu: None,
            pending_empty_trash: None,
            pending_properties: None,
            positions: DesktopPositionStore::default(),
            positions_load: Some(request),
            position_saves: BTreeSet::new(),
            auto_arrange_saves: BTreeSet::new(),
            watch_refresh_pending: false,
            trash_confirm_pending: false,
            pending_intents: Vec::new(),
            tree,
        };
        let vfs = actor.initial_directory_request(request);
        let mut messages = vec![ui, vfs, load_positions_request(app, request)];
        messages.extend(actor.filesystem_watch_requests());
        if let Some(path) = wallpaper_path(arguments)
            && let Some(mime_type) = wallpaper_mime(path)
        {
            actor.wallpaper_request = Some(request);
            messages.push(Message::MediaRequest(MediaRequest {
                request,
                app,
                path: path.to_owned(),
                mime_type: mime_type.to_owned(),
                maximum_source_bytes: MAX_MEDIA_SOURCE_BYTES,
                target_width: MAX_MEDIA_DIMENSION,
                target_height: MAX_MEDIA_DIMENSION,
                frame_millis: 0,
            }));
        }
        Ok((actor, messages))
    }

    pub(crate) fn handle_event(
        &mut self,
        envelope: &UiEventEnvelope,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if envelope.app != self.app || envelope.window != self.window {
            return Ok(Vec::new());
        }
        let target = envelope.target.get();
        if let Some(messages) = self.handle_link_chooser_event(&envelope.event, target, request)? {
            return Ok(messages);
        }
        match (&envelope.event, target) {
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 27 | 9,
                    ..
                },
                _,
            ) if self.context_kind == ContextKind::Rename => self.close_rename_editor(),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 13 | 36,
                    ..
                },
                _,
            ) if self.context_kind == ContextKind::Rename => self.begin_rename(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 53,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 => self.copy_selection(request, true),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 54,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 => self.copy_selection(request, false),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 55,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 4 != 0 => self.paste_files(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 13 | 36,
                    modifiers: 8,
                    ..
                },
                _,
            ) => self.show_properties(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code,
                    modifiers,
                    ..
                },
                _,
            ) if direction_for_key(*key_code).is_some() => {
                self.model.execute(DesktopCommand::Navigate {
                    direction: direction_for_key(*key_code).expect("guarded direction"),
                    extend: modifiers & 1 != 0,
                })?;
                let patches = self.selection_patches("Desktop selection moved");
                self.emit(&patches).map(|message| vec![message])
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 13 | 36,
                    ..
                },
                _,
            ) => {
                let Some(item) = self.model.selection().single() else {
                    return Ok(Vec::new());
                };
                self.activate_item(item, request)
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 32 | 65,
                    modifiers,
                    ..
                },
                _,
            ) if self.context_kind != ContextKind::Rename && modifiers & (4 | 8 | 64) == 0 => {
                self.begin_keyboard_rename()
            }
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 46 | 119,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 1 != 0 => self.begin_permanent_delete(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 46 | 119,
                    modifiers,
                    ..
                },
                _,
            ) if modifiers & 1 == 0 => self.begin_move_to_trash(request),
            (
                UiEvent::Key {
                    pressed: true,
                    key_code: 27 | 9,
                    ..
                },
                _,
            ) => {
                let mut messages = self.finish_pointer_gesture()?;
                self.model.execute(DesktopCommand::ClearSelection)?;
                let patches = self.selection_patches("Desktop selection cleared");
                messages.push(self.emit(&patches)?);
                Ok(messages)
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    x,
                    y,
                    button: 1,
                    modifiers,
                },
                _,
            ) if target == ROOT
                || target == ICON_GRID
                || self.item_for_widget(target).is_some() =>
            {
                self.begin_pointer_selection(target, *x, *y, *modifiers, request)
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Down,
                    button: 6,
                    ..
                },
                _,
            ) => {
                let Some(item) = self.item_for_widget(target) else {
                    return Ok(Vec::new());
                };
                self.drag = None;
                self.activate_item(item, request)
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x,
                    y,
                    ..
                },
                _,
            ) if self.rubber_band.is_some() => self.update_rubber_band(*x, *y),
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Move,
                    x,
                    y,
                    ..
                },
                _,
            ) if self.drag.is_some() => self.drag_icons(*x, *y, request),
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    button: 1,
                    ..
                },
                _,
            ) if self.rubber_band.is_some()
                || self.drag.is_some()
                || self.source_drag.is_some() =>
            {
                self.finish_pointer_gesture()
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Leave,
                    ..
                },
                _,
            ) if self.rubber_band.is_some()
                || self.drag.is_some()
                || self.source_drag.is_some() =>
            {
                self.finish_pointer_gesture()
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    button: 3,
                    ..
                },
                ROOT | ICON_GRID,
            ) => {
                self.model.execute(DesktopCommand::ClearSelection)?;
                self.rename_text.clear();
                let mut patches = self.selection_patches("Desktop selection cleared");
                let entries = self.context_menu_entries(true)?;
                patches.extend(switch_context_patches(
                    self.context_kind,
                    ContextKind::Background,
                    "",
                    &self.context_action_ids,
                    &entries,
                ));
                self.context_kind = ContextKind::Background;
                self.context_action_ids = entries.iter().map(|entry| entry.id).collect();
                self.context_messages = entries.iter().map(|entry| (entry.id, entry.message)).collect();
                self.emit(&patches).map(|message| vec![message])
            }
            (
                UiEvent::Pointer {
                    kind: PointerEventKind::Up,
                    button: 3,
                    ..
                },
                _,
            ) => {
                let Some(item) = self.item_for_widget(target) else {
                    return Ok(Vec::new());
                };
                if !self.model.selection().contains(item) {
                    self.model.execute(DesktopCommand::Click {
                        item,
                        mode: ClickMode::Replace,
                    })?;
                }
                let label = self
                    .model
                    .item(item)
                    .map_or_else(|| String::from("desktop item"), |entry| entry.label.clone());
                self.rename_text.clone_from(&label);
                let mut patches = self.selection_patches(&format!("{label} selected"));
                if self
                    .model
                    .item(item)
                    .is_some_and(|item| matches!(item.kind, DesktopItemKind::Trash { .. }))
                {
                    let selected = self.emit(&patches)?;
                    let mut messages = vec![selected];
                    messages.extend(self.begin_trash_menu(item, request)?);
                    return Ok(messages);
                }
                let entries = self.context_menu_entries(true)?;
                patches.extend(switch_context_patches(
                    self.context_kind,
                    ContextKind::Item,
                    &self.rename_text,
                    &self.context_action_ids,
                    &entries,
                ));
                self.context_kind = ContextKind::Item;
                self.context_action_ids = entries.iter().map(|entry| entry.id).collect();
                self.context_messages = entries.iter().map(|entry| (entry.id, entry.message)).collect();
                self.emit(&patches).map(|message| vec![message])
            }
            (UiEvent::ValueChanged(value), RENAME_FIELD) => {
                if self.context_kind != ContextKind::Rename {
                    return Ok(Vec::new());
                }
                self.rename_text.clone_from(value);
                self.emit(&[update_text(
                    RENAME_FIELD,
                    &self.rename_text,
                    Some(WidgetState::ENABLED | WidgetState::FOCUSED),
                )])
                .map(|message| vec![message])
            }
            (UiEvent::Activate, REFRESH) => self.begin_refresh(request, None),
            (UiEvent::Activate, NEW_FOLDER) => self.begin_new_folder(request),
            (UiEvent::Activate, NEW_FILE) => self.begin_new_file(request),
            (UiEvent::Activate, SELECT_ALL) => self.select_all(),
            (UiEvent::Activate, ARRANGE) => self.toggle_auto_arrange(request),
            (UiEvent::Activate, OPEN_WITH) => {
                let Some(path) = self.selected_item().and_then(|item| item.path.clone()) else {
                    return self.notice("Select one desktop file to open");
                };
                Ok(vec![Message::Launch {
                    app: self.app,
                    entrypoint: String::from("rice-files"),
                    arguments: vec![format!("--select={path}"), String::from("--open-with")],
                }])
            }
            (UiEvent::Activate, OPEN) => {
                let Some(item) = self.model.selection().single() else {
                    return self.notice("Select one desktop item to open");
                };
                self.activate_item(item, request)
            }
            (UiEvent::Activate, PROPERTIES) => self.show_properties(request),
            (UiEvent::Activate, PROPERTIES_CLOSE) => self.close_properties(),
            (UiEvent::Activate, desktop_properties::COPY_LOCATION) => {
                self.copy_property_location(request)
            }
            (UiEvent::ValueChanged(value), desktop_properties::MODE_FIELD) => {
                self.update_property_mode(value)
            }
            (UiEvent::ValueChanged(value), desktop_properties::LINK_TARGET_FIELD) => {
                self.update_property_link_target(value)
            }
            (UiEvent::ValueChanged(value), desktop_properties::NAME_FIELD) => {
                self.update_property_name(value)
            }
            (UiEvent::ValueChanged(value), desktop_link::LINK_NAME_FIELD) => {
                self.update_link_name(value)
            }
            (UiEvent::ValueChanged(value), desktop_link::LINK_TARGET_FIELD) => {
                self.update_link_target(value)
            }
            (UiEvent::Activate, desktop_properties::APPLY_MODE) => {
                self.apply_property_mode(request)
            }
            (UiEvent::Activate, desktop_properties::APPLY_LINK_TARGET) => {
                self.apply_property_link_target(request)
            }
            (UiEvent::Activate, desktop_properties::APPLY_NAME) => {
                self.apply_property_name(request)
            }
            (UiEvent::Activate, desktop_properties::EXECUTABLE) => {
                self.toggle_property_executable(request)
            }
            (UiEvent::Activate, desktop_properties::ACCESS_READ_WRITE) => {
                self.choose_property_access(0o600)
            }
            (UiEvent::Activate, desktop_properties::ACCESS_READ_ONLY) => {
                self.choose_property_access(0o400)
            }
            (UiEvent::Activate, desktop_properties::ACCESS_WRITE_ONLY) => {
                self.choose_property_access(0o200)
            }
            (UiEvent::Activate, desktop_properties::ACCESS_NONE) => self.choose_property_access(0),
            (UiEvent::Activate, CUT) => self.copy_selection(request, true),
            (UiEvent::Activate, COPY) => self.copy_selection(request, false),
            (UiEvent::Activate, PASTE) => self.paste_files(request),
            (UiEvent::Activate, LINK) => self.begin_link_dialog(),
            (UiEvent::Activate, desktop_link::LINK_FIND_TARGET) => self.begin_link_chooser(request),
            (UiEvent::Activate, desktop_link::LINK_CREATE) => self.create_link(request),
            (UiEvent::Activate, desktop_link::LINK_CANCEL) => self.cancel_link_dialog(),
            (UiEvent::Activate, EMPTY_TRASH) => self.confirm_or_empty_trash(request),
            (UiEvent::Activate, RENAME) if self.context_kind == ContextKind::Item => {
                self.open_rename_editor()
            }
            (UiEvent::Activate, RENAME) if self.context_kind == ContextKind::Rename => {
                self.begin_rename(request)
            }
            (UiEvent::Activate, RENAME_CANCEL) if self.context_kind == ContextKind::Rename => {
                self.close_rename_editor()
            }
            (UiEvent::Activate, RENAME_DIALOG) if self.context_kind == ContextKind::Rename => {
                self.close_rename_editor()
            }
            (UiEvent::Activate, DELETE) if self.context_kind == ContextKind::Item => {
                self.begin_move_to_trash(request)
            }
            (UiEvent::Activate, SETTINGS) => self.launch_settings(),
            (UiEvent::Activate, ROOT | ICON_GRID) => {
                self.model.execute(DesktopCommand::ClearSelection)?;
                let patches = self.selection_patches("Desktop selection cleared");
                self.emit(&patches).map(|message| vec![message])
            }
            (UiEvent::Activate, _) => {
                let Some(item) = self.item_for_widget(target) else {
                    return Ok(Vec::new());
                };
                if self.rubber_band.take().is_some()
                    && self.tree.get(widget_id(RUBBER_BAND)).is_some()
                {
                    return self
                        .emit(&[Patch::Remove {
                            id: widget_id(RUBBER_BAND),
                        }])
                        .map(|message| vec![message]);
                }
                let moved = self.drag.take().is_some_and(|drag| drag.moved);
                if moved {
                    return Ok(Vec::new());
                }
                // Primary clicks select desktop icons. Opening is deliberately
                // reserved for the browser's trusted double-click gesture or
                // Enter, matching conventional desktop behavior.
                if self.model.selection().contains(item) {
                    Ok(Vec::new())
                } else {
                    self.model.execute(DesktopCommand::Click {
                        item,
                        mode: ClickMode::Replace,
                    })?;
                    let patches = self.selection_patches("Desktop item selected");
                    self.emit(&patches).map(|message| vec![message])
                }
            }
            _ => Ok(Vec::new()),
        }
    }

    pub(crate) fn handle_media_response(
        &mut self,
        response: &MediaResponse,
        next_request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        if self
            .thumbnail_pending
            .as_ref()
            .is_some_and(|(request, _, _)| *request == response.request)
        {
            let (_, path, mime_type) = self
                .thumbnail_pending
                .take()
                .expect("matched thumbnail request");
            let expected_kind = if mime_type.starts_with("video/") {
                MediaKind::Video
            } else {
                MediaKind::Image
            };
            if let MediaResult::Ready(metadata) = &response.result
                && metadata.mime_type == mime_type
                && metadata.kind == expected_kind
                && metadata.width > 0
                && metadata.height > 0
            {
                self.thumbnail_cache.insert(path, response.request);
            }
            let patches = replace_item_patches(
                &self.items,
                self.model.grid(),
                self.model.selection(),
                &self.thumbnail_cache,
            );
            let mut messages = vec![self.emit(&patches)?];
            if let Some(request) = self.start_next_thumbnail(next_request) {
                messages.push(request);
            }
            return Ok(messages);
        }
        if self.wallpaper_request != Some(response.request) {
            return Ok(Vec::new());
        }
        self.wallpaper_request = None;
        let MediaResult::Ready(metadata) = &response.result else {
            return Ok(Vec::new());
        };
        if metadata.kind != MediaKind::Image {
            return Ok(Vec::new());
        }
        self.emit(&[update_text(
            WALLPAPER,
            &format!("media-request:{}", response.request.get()),
            // Wallpaper media is presentation-only. Keeping it disabled
            // prevents its full-window Image geometry from owning pointer
            // focus after the icon grid is rebuilt.
            Some(WidgetState::empty()),
        )])
        .map(|message| vec![message])
    }

    fn reset_thumbnail_batch(&mut self, request: RequestId) -> Option<Message> {
        self.thumbnail_pending = None;
        let candidates = self
            .items
            .iter()
            .filter_map(|item| {
                let path = item.path.as_ref()?;
                if matches!(item.kind, DesktopItemKind::Application { .. }) {
                    return Some((path.clone(), String::from("application/x-desktop")));
                }
                if !matches!(item.kind, DesktopItemKind::File) {
                    return None;
                }
                let mime_type = media_type_for_path(path)?;
                (mime_type.starts_with("image/")
                    || mime_type.starts_with("video/")
                    || mime_type == "application/x-msdownload")
                    .then(|| (path.clone(), mime_type))
            })
            .take(160)
            .collect::<Vec<_>>();
        let visible = candidates
            .iter()
            .map(|(path, _)| path.clone())
            .collect::<BTreeSet<_>>();
        self.thumbnail_cache
            .retain(|path, _| visible.contains(path));
        self.thumbnail_queue.clear();
        self.thumbnail_queue.extend(
            candidates
                .into_iter()
                .filter(|(path, _)| !self.thumbnail_cache.contains_key(path)),
        );
        self.start_next_thumbnail(request)
    }

    fn start_next_thumbnail(&mut self, request: RequestId) -> Option<Message> {
        let (path, mime_type) = self.thumbnail_queue.pop_front()?;
        self.thumbnail_pending = Some((request, path.clone(), mime_type.clone()));
        Some(Message::MediaRequest(MediaRequest {
            request,
            app: self.app,
            path,
            mime_type,
            maximum_source_bytes: MAX_MEDIA_SOURCE_BYTES,
            target_width: 256,
            target_height: 192,
            frame_millis: 0,
        }))
    }

    pub(crate) fn refresh_language(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let mut patches = replace_item_patches(&self.items, self.model.grid(), self.model.selection(), &self.thumbnail_cache);
        match self.context_kind {
            ContextKind::Background | ContextKind::Item => {
                patches.push(update_text(DESKTOP_MENU, rice_i18n::t("Desktop"), None));
                // Text-only updates preserve the existing actions, check state and popup.
                for &(id, message) in &self.context_messages {
                    if self.tree.get(widget_id(id)).is_some() {
                        patches.push(update_text(id, rice_i18n::t(message), None));
                    }
                }
            }
            ContextKind::Rename => {
                for (id, message) in [(RENAME_DIALOG, "Rename Desktop Item"),
                    (RENAME, "Rename"), (RENAME_CANCEL, "Cancel")] {
                    patches.push(update_text(id, rice_i18n::t(message), None));
                }
            }
            ContextKind::Hidden => {}
        }
        if let Some(notice) = &self.retained_notice
            && !matches!(notice, DesktopNotice::Literal(_))
            && self.tree.get(widget_id(NOTICE)).is_some()
        {
            patches.push(update_text(NOTICE, &notice.render(), None));
        }
        self.emit(&patches).map(|message| vec![message])
    }

    fn selection_patches(&self, _status: &str) -> Vec<Patch> {
        self.items
            .iter()
            .map(|item| Patch::Update {
                id: widget_for_item(item.id),
                state: Some(if self.model.selection().contains(item.id) {
                    WidgetState::ENABLED
                        | WidgetState::SELECTED
                        | WidgetState::FOCUSED
                        | WidgetState::KEY_NAVIGATION
                } else {
                    WidgetState::ENABLED | WidgetState::KEY_NAVIGATION
                }),
                text: ValueChange::Keep,
                icon: ValueChange::Keep,
            })
            .collect()
    }

    fn begin_keyboard_rename(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        let Some(item) = self.model.selection().single() else {
            return Ok(Vec::new());
        };
        let Some(entry) = self.model.item(item) else {
            return Ok(Vec::new());
        };
        if !entry.is_renameable() {
            return self.notice("This desktop item cannot be renamed");
        }
        self.rename_text.clone_from(&entry.label);
        self.open_rename_editor()
    }

    fn item_for_widget(&self, widget: u64) -> Option<DesktopItemId> {
        [
            desktop_view::THUMBNAIL_IMAGE_BASE,
            desktop_view::THUMBNAIL_CELL_BASE,
            ITEM_BASE,
        ]
        .into_iter()
        .filter_map(|base| widget.checked_sub(base).map(DesktopItemId))
        .find(|id| self.items.iter().any(|item| item.id == *id))
    }

    fn begin_pointer_selection(
        &mut self,
        target: u64,
        x: i32,
        y: i32,
        modifiers: u16,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        self.rubber_band = None;
        if let Some(item) = self.item_for_widget(target) {
            let mode = if modifiers & 1 != 0 {
                Some(ClickMode::ExtendRange)
            } else if modifiers & 4 != 0 {
                Some(ClickMode::Toggle)
            } else if self.model.selection().contains(item) {
                None
            } else {
                Some(ClickMode::Replace)
            };
            if let Some(mode) = mode {
                self.model.execute(DesktopCommand::Click { item, mode })?;
            }
            self.drag = (!self.auto_arrange && self.model.selection().contains(item)).then(|| {
                DesktopIconDrag {
                    item,
                    origin: (x, y),
                    moved: false,
                    last_cell: self.model.grid().cell_of(item),
                    items: self.model.selection().selected_vec(),
                }
            });
            let patches = self.selection_patches("Desktop item selected");
            let mut messages = vec![self.emit(&patches)?];
            if let Some(source) = self.begin_source_drag(modifiers, request)? {
                messages.push(source);
            }
            return Ok(messages);
        }
        self.drag = None;
        let additive = modifiers & 4 != 0;
        if !additive {
            self.model.execute(DesktopCommand::ClearSelection)?;
        }
        self.rubber_band = Some(DesktopRubberBand {
            origin: (x, y),
            additive,
        });
        let mut patches = self.selection_patches("Desktop selection started");
        patches.push(desktop_view::rubber_band_patch(
            self.tree.get(widget_id(RUBBER_BAND)).is_some(),
            selection_rect(x, y, x, y),
        ));
        self.emit(&patches).map(|message| vec![message])
    }

    fn update_rubber_band(&mut self, x: i32, y: i32) -> Result<Vec<Message>, DesktopActorError> {
        let Some(band) = self.rubber_band else {
            return Ok(Vec::new());
        };
        let area = selection_rect(band.origin.0, band.origin.1, x, y);
        self.model.execute(DesktopCommand::RubberBand {
            area,
            additive: band.additive,
        })?;
        let mut patches = self.selection_patches("Desktop items selected");
        patches.push(desktop_view::rubber_band_patch(true, area));
        self.emit(&patches).map(|message| vec![message])
    }

    fn finish_pointer_gesture(&mut self) -> Result<Vec<Message>, DesktopActorError> {
        self.drag = None;
        let mut messages = Vec::new();
        if self.rubber_band.take().is_some() && self.tree.get(widget_id(RUBBER_BAND)).is_some() {
            messages.push(self.emit(&[Patch::Remove {
                id: widget_id(RUBBER_BAND),
            }])?);
        }
        if let Some(drag) = self.source_drag.take() {
            messages.push(self.cancel_source_drag(drag));
        }
        Ok(messages)
    }

    fn drag_icons(
        &mut self,
        x: i32,
        y: i32,
        request: RequestId,
    ) -> Result<Vec<Message>, DesktopActorError> {
        let Some(mut drag) = self.drag.take() else {
            return Ok(Vec::new());
        };
        let dx = i64::from(x) - i64::from(drag.origin.0);
        let dy = i64::from(y) - i64::from(drag.origin.1);
        if !drag.moved && dx * dx + dy * dy < DRAG_THRESHOLD_SQUARED {
            self.drag = Some(drag);
            return Ok(Vec::new());
        }
        drag.moved = true;
        let cell = cell_at(self.model.grid().config(), x, y);
        if drag.last_cell == Some(cell) {
            self.drag = Some(drag);
            return Ok(Vec::new());
        }
        drag.last_cell = Some(cell);
        let update = if drag.items.len() == 1 {
            self.model.execute(DesktopCommand::MoveIcon {
                item: drag.item,
                cell,
            })?
        } else {
            self.model.execute(DesktopCommand::MoveIcons {
                items: drag.items.clone(),
                anchor: drag.item,
                cell,
            })?
        };
        if !update.changed {
            self.drag = Some(drag);
            return Ok(Vec::new());
        }
        self.pending_intents = update.intents;
        let positions = self
            .pending_intents
            .iter()
            .find_map(|intent| match intent {
                DesktopIntent::PersistPositions(positions) => Some(positions.as_slice()),
                _ => None,
            })
            .unwrap_or_default();
        self.positions
            .replace_workspace(self.model.workspace(), &self.items, positions);
        let ordered = self.model.grid().ordered_items();
        let patches = replace_item_patches(
            &self.items,
            self.model.grid(),
            self.model.selection(),
            &self.thumbnail_cache,
        );
        self.rendered_items = ordered;
        self.drag = Some(drag);
        let ui = self.emit(&patches)?;
        self.position_saves.insert(request);
        Ok(vec![
            ui,
            save_positions_request(self.app, request, &self.positions),
        ])
    }

    fn emit(&mut self, patches: &[Patch]) -> Result<Message, DesktopActorError> {
        self.tree.apply(patches)?;
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(DesktopActorError::SequenceExhausted)?;
        message(self.app, self.window, self.sequence, patches)
    }

    #[cfg(test)]
    pub(crate) fn pending_intents(&self) -> &[DesktopIntent] {
        &self.pending_intents
    }

    #[cfg(test)]
    pub(crate) const fn model(&self) -> &DesktopModel {
        &self.model
    }
}

const fn direction_for_key(key_code: u32) -> Option<Direction> {
    match key_code {
        37 | 113 => Some(Direction::Left),
        39 | 114 => Some(Direction::Right),
        38 | 111 => Some(Direction::Up),
        40 | 116 => Some(Direction::Down),
        110 => Some(Direction::Home),
        115 => Some(Direction::End),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DesktopIconDrag {
    item: DesktopItemId,
    origin: (i32, i32),
    moved: bool,
    last_cell: Option<Cell>,
    items: Vec<DesktopItemId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DesktopRubberBand {
    origin: (i32, i32),
    additive: bool,
}

fn selection_rect(start_x: i32, start_y: i32, end_x: i32, end_y: i32) -> Rect {
    let x = start_x.min(end_x);
    let y = start_y.min(end_y);
    let width = start_x.abs_diff(end_x).saturating_add(1);
    let height = start_y.abs_diff(end_y).saturating_add(1);
    Rect::new(x, y, width, height)
}

fn cell_at(config: GridConfig, x: i32, y: i32) -> Cell {
    let stride_x = config
        .cell_width
        .saturating_add(config.horizontal_gap)
        .max(1);
    let stride_y = config
        .cell_height
        .saturating_add(config.vertical_gap)
        .max(1);
    let column = x.saturating_sub(config.origin_x).max(0).cast_unsigned() / stride_x;
    let row = y.saturating_sub(config.origin_y).max(0).cast_unsigned() / stride_y;
    Cell::new(
        u16::try_from(column.min(u32::from(config.columns.saturating_sub(1)))).unwrap_or_default(),
        u16::try_from(row.min(u32::from(config.rows.saturating_sub(1)))).unwrap_or_default(),
    )
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DesktopActorError {
    Model(ModelError),
    Tree(TreeError),
    Conversion(PatchConversionError),
    UnexpectedVfsResponse,
    UnexpectedDataProviderResponse,
    UnexpectedClipboardResponse,
    UnexpectedSettingsResponse,
    UnexpectedHostEvent,
    InvalidClipboard,
    InvalidDirectoryEntry,
    EntryLimit,
    SequenceExhausted,
}

impl From<ModelError> for DesktopActorError {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl From<TreeError> for DesktopActorError {
    fn from(error: TreeError) -> Self {
        Self::Tree(error)
    }
}

impl From<PatchConversionError> for DesktopActorError {
    fn from(error: PatchConversionError) -> Self {
        Self::Conversion(error)
    }
}

fn initial_model(workspace: WorkspaceId, redmond98: bool) -> DesktopModel {
    DesktopModel::new(
        desktop_grid_config(DEFAULT_DESKTOP_WIDTH, DEFAULT_DESKTOP_HEIGHT, 0, redmond98),
        WallpaperSettings::new(Wallpaper::Solid(Color::rgb(54, 79, 107))),
        workspace,
    )
}

fn desktop_grid_config(width: u32, height: u32, item_count: usize, redmond98: bool) -> GridConfig {
    let default = if redmond98 {
        GridConfig {
            // Windows 98's shell uses the classic 75 px icon-spacing pitch.
            // Keep the hit-test/persistence cell slightly inside that pitch
            // so adjacent captions never overlap while matching the compact
            // desktop rows and columns of the real shell.
            cell_width: 68,
            cell_height: 68,
            horizontal_gap: 7,
            vertical_gap: 7,
            origin_x: 4,
            origin_y: 4,
            ..GridConfig::default()
        }
    } else {
        GridConfig::default()
    };
    let stride_x = default
        .cell_width
        .saturating_add(default.horizontal_gap)
        .max(1);
    let stride_y = default
        .cell_height
        .saturating_add(default.vertical_gap)
        .max(1);
    let available_width = width.saturating_sub(default.origin_x.max(0).cast_unsigned());
    let available_height = height.saturating_sub(default.origin_y.max(0).cast_unsigned());
    let columns = u16::try_from(
        available_width
            .saturating_add(default.horizontal_gap)
            .checked_div(stride_x)
            .unwrap_or_default()
            .max(1),
    )
    .unwrap_or(MAX_DESKTOP_COLUMNS)
    .clamp(1, MAX_DESKTOP_COLUMNS);
    let visible_rows = u16::try_from(
        available_height
            .saturating_add(default.vertical_gap)
            .checked_div(stride_y)
            .unwrap_or_default()
            .max(1),
    )
    .unwrap_or(MAX_DESKTOP_ROWS)
    .clamp(1, MAX_DESKTOP_ROWS);
    let required_rows = item_count
        .div_ceil(usize::from(columns))
        .max(1)
        .min(usize::from(MAX_DESKTOP_ROWS));
    GridConfig {
        columns,
        rows: visible_rows.max(u16::try_from(required_rows).unwrap_or(MAX_DESKTOP_ROWS)),
        ..default
    }
}

fn widget_for_item(item: DesktopItemId) -> WidgetId {
    widget_id(
        ITEM_BASE
            .checked_add(item.0)
            .expect("bounded RiceDesktop item ID"),
    )
}

fn launch_workspace(arguments: &[String]) -> WorkspaceId {
    let name = arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--workspace="))
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    WorkspaceId::new(name)
}

fn wallpaper_path(arguments: &[String]) -> Option<&str> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix("--wallpaper="))
        .filter(|path| {
            path.starts_with('/')
                && path.len() <= 1_024
                && !path.split('/').any(|part| part == "..")
        })
}

fn wallpaper_mime(path: &str) -> Option<&'static str> {
    let extension = path.rsplit_once('.')?.1.to_ascii_lowercase();
    match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
