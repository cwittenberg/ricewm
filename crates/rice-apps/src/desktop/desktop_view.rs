//! Retained `RiceUI` tree construction for `RiceDesktop`.

use super::{
    AppId, CELL_BASE, DESKTOP_MENU, DesktopActorError, DesktopItem, DesktopItemKind, ICON_GRID,
    IconId, MENU_BAR, Message, Patch, RENAME, RENAME_ACTIONS, RENAME_CANCEL, RENAME_CONTENT,
    RENAME_DIALOG, RENAME_FIELD, ROOT, RUBBER_BAND, ValueChange, WALLPAPER, WidgetId, WidgetKind,
    WidgetSpec, WidgetState, WindowId, widget_for_item,
};
use rice_desktop::{Cell, GridConfig, GridLayout, Rect, SelectionState};
use rice_native_app_protocol::UiPatchEnvelope;
use rice_ui::{file_icon, folder_icon};
use std::collections::BTreeMap;

pub(super) const THUMBNAIL_CELL_BASE: u64 = 2_000_000;
pub(super) const THUMBNAIL_IMAGE_BASE: u64 = 3_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ContextKind {
    Hidden,
    Background,
    Item,
    Rename,
}

pub(super) struct MenuActionSpec {
    pub id: u64,
    pub label: String,
    pub message: &'static str,
    pub icon: IconId,
    pub checked: Option<bool>,
}

pub(super) fn initial_patches(grid: GridConfig) -> Vec<Patch> {
    let mut patches = Vec::with_capacity(12);
    create(
        &mut patches,
        ROOT,
        None,
        WidgetKind::Stack,
        Some("desktop:transparent"),
        None,
    );
    create(
        &mut patches,
        WALLPAPER,
        Some(ROOT),
        WidgetKind::Image,
        None,
        None,
    );
    create(
        &mut patches,
        ICON_GRID,
        Some(ROOT),
        WidgetKind::IconGrid,
        Some(&desktop_grid_token(grid)),
        None,
    );
    patches
}

pub(super) fn switch_context_patches(
    current: ContextKind,
    next: ContextKind,
    rename: &str,
    current_actions: &[u64],
    entries: &[MenuActionSpec],
) -> Vec<Patch> {
    if current == next {
        return match next {
            ContextKind::Rename => {
                vec![update_text(
                    RENAME_FIELD,
                    rename,
                    Some(WidgetState::ENABLED),
                )]
            }
            ContextKind::Background | ContextKind::Item => {
                let mut patches = current_actions
                    .iter()
                    .map(|id| Patch::Remove { id: widget_id(*id) })
                    .collect::<Vec<_>>();
                create_action_menu(&mut patches, entries);
                patches
            }
            ContextKind::Hidden => Vec::new(),
        };
    }
    if next == ContextKind::Hidden {
        return vec![Patch::Remove {
            id: widget_id(if current == ContextKind::Rename {
                RENAME_DIALOG
            } else {
                MENU_BAR
            }),
        }];
    }
    let mut patches = match current {
        ContextKind::Hidden => Vec::new(),
        ContextKind::Background | ContextKind::Item => vec![Patch::Remove {
            id: widget_id(MENU_BAR),
        }],
        ContextKind::Rename => vec![Patch::Remove {
            id: widget_id(RENAME_DIALOG),
        }],
    };
    if matches!(next, ContextKind::Background | ContextKind::Item) {
        create_context_shell(&mut patches);
    }
    match next {
        ContextKind::Hidden => {}
        ContextKind::Background | ContextKind::Item => create_action_menu(&mut patches, entries),
        ContextKind::Rename => create_rename_menu(&mut patches, rename),
    }
    patches
}

fn create_context_shell(patches: &mut Vec<Patch>) {
    create(
        patches,
        MENU_BAR,
        Some(ROOT),
        WidgetKind::MenuBar,
        Some("context"),
        None,
    );
    create(
        patches,
        DESKTOP_MENU,
        Some(MENU_BAR),
        WidgetKind::Menu,
        Some(rice_i18n::t("Desktop")),
        None,
    );
}

fn create_action_menu(patches: &mut Vec<Patch>, entries: &[MenuActionSpec]) {
    for entry in entries {
        create(
            patches,
            entry.id,
            Some(DESKTOP_MENU),
            if entry.checked.is_some() {
                WidgetKind::CheckBox
            } else {
                WidgetKind::MenuItem
            },
            Some(&entry.label),
            (entry.label != rice_ui::MENU_SEPARATOR).then_some(entry.icon),
        );
        if entry.label == rice_ui::MENU_SEPARATOR {
            patches.push(Patch::Update {
                id: widget_id(entry.id), state: Some(WidgetState::empty()),
                text: ValueChange::Keep, icon: ValueChange::Keep,
            });
        }
        if let Some(checked) = entry.checked {
            patches.push(Patch::Update {
                id: widget_id(entry.id),
                state: Some(if checked {
                    WidgetState::ENABLED | WidgetState::CHECKED
                } else {
                    WidgetState::ENABLED
                }),
                text: ValueChange::Keep,
                icon: ValueChange::Keep,
            });
        }
    }
}

fn create_rename_menu(patches: &mut Vec<Patch>, rename: &str) {
    create(
        patches,
        RENAME_DIALOG,
        Some(ROOT),
        WidgetKind::Dialog,
        Some(rice_i18n::t("Rename Desktop Item")),
        Some(IconId::EditRedo),
    );
    create(
        patches,
        RENAME_CONTENT,
        Some(RENAME_DIALOG),
        WidgetKind::Column,
        Some("panel:clamp:420"),
        None,
    );
    create(
        patches,
        RENAME_FIELD,
        Some(RENAME_CONTENT),
        WidgetKind::TextField,
        Some(rename),
        Some(IconId::Document),
    );
    patches.push(Patch::Update {
        id: widget_id(RENAME_FIELD),
        state: Some(WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::SELECT_ALL_ON_FOCUS),
        text: ValueChange::Keep,
        icon: ValueChange::Keep,
    });
    create(
        patches,
        RENAME_ACTIONS,
        Some(RENAME_DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    create(
        patches,
        RENAME,
        Some(RENAME_ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t("Rename")),
        Some(IconId::EditRedo),
    );
    create(
        patches,
        RENAME_CANCEL,
        Some(RENAME_ACTIONS),
        WidgetKind::Button,
        Some(rice_i18n::t("Cancel")),
        Some(IconId::Close),
    );
}

pub(super) fn replace_item_patches(
    items: &[DesktopItem],
    grid: &GridLayout,
    selection: &SelectionState,
    thumbnails: &BTreeMap<String, rice_native_app_protocol::RequestId>,
) -> Vec<Patch> {
    // The icon grid owns one child per physical desktop cell. Empty cells are
    // transparent stack placeholders, so moving an icon to an unoccupied cell
    // does not collapse the layout back into a packed list on the next render.
    let config = grid.config();
    let by_id = items
        .iter()
        .map(|item| (item.id, item))
        .collect::<BTreeMap<_, _>>();
    let by_cell = grid
        .placements()
        .into_iter()
        .map(|placement| (placement.cell, placement.item))
        .collect::<BTreeMap<_, _>>();
    let mut patches = vec![
        Patch::Remove {
            id: widget_id(ICON_GRID),
        },
        Patch::Create {
            widget: WidgetSpec {
                id: widget_id(ICON_GRID),
                kind: WidgetKind::IconGrid,
                state: WidgetState::ENABLED | WidgetState::FOCUSED | WidgetState::KEY_NAVIGATION,
                text: Some(desktop_grid_token(config)),
                icon: None,
            },
            parent: Some(widget_id(ROOT)),
            // Wallpaper is child zero; the grid must remain above it.
            index: 1,
        },
    ];
    let mut index = 0_usize;
    for column in 0..config.columns {
        for row in 0..config.rows {
            let cell = Cell::new(column, row);
            let item = by_cell.get(&cell).and_then(|id| by_id.get(id)).copied();
            let (id, kind, state, text, icon) = item.map_or_else(
                || {
                    (
                        CELL_BASE + u64::try_from(index).unwrap_or(u64::MAX - CELL_BASE),
                        WidgetKind::Stack,
                        WidgetState::empty(),
                        None,
                        None,
                    )
                },
                |item| {
                    let selected = selection.contains(item.id);
                    (
                        widget_for_item(item.id).get(),
                        WidgetKind::Button,
                        if selected {
                            WidgetState::ENABLED
                                | WidgetState::SELECTED
                                | WidgetState::FOCUSED
                                | WidgetState::KEY_NAVIGATION
                        } else {
                            WidgetState::ENABLED | WidgetState::KEY_NAVIGATION
                        },
                        Some(display_label(item).to_owned()),
                        Some(icon_for_item(item)),
                    )
                },
            );
            patches.push(Patch::Create {
                widget: WidgetSpec {
                    id: widget_id(id),
                    kind,
                    state,
                    text,
                    icon,
                },
                parent: Some(widget_id(ICON_GRID)),
                index,
            });
            if let Some(item)=item {
                let selector=match &item.kind {
                    DesktopItemKind::Application {desktop_id}=>Some(format!("app:{}",desktop_id)),
                    DesktopItemKind::File=>item.path.as_ref().and_then(|path|rice_files::GuestPath::absolute(path).ok()).map(|path|format!("mime:{}",rice_files::infer_media_type(&rice_files::EntryMetadata::new(path,rice_files::EntryKind::File,0,0)))),
                    _=>None,
                };
                if selector.is_some(){patches.push(Patch::IconSelector {id:widget_id(id),selector});}
            }
            if let Some(item) = item
                && let Some(request) = item.path.as_ref().and_then(|path| thumbnails.get(path))
            {
                let cell = widget_id(
                    THUMBNAIL_CELL_BASE
                        .checked_add(item.id.0)
                        .expect("bounded desktop thumbnail cell ID"),
                );
                let image = widget_id(
                    THUMBNAIL_IMAGE_BASE
                        .checked_add(item.id.0)
                        .expect("bounded desktop thumbnail image ID"),
                );
                let button = widget_for_item(item.id);
                if matches!(patches.last(),Some(Patch::IconSelector {..})){patches.pop();}
                patches.pop();
                patches.push(Patch::Create {
                    widget: WidgetSpec {
                        id: cell,
                        kind: WidgetKind::Container,
                        state: WidgetState::ENABLED,
                        text: Some(String::from("desktop:thumbnail-cell")),
                        icon: None,
                    },
                    parent: Some(widget_id(ICON_GRID)),
                    index,
                });
                patches.push(Patch::Create {
                    widget: WidgetSpec {
                        id: image,
                        kind: WidgetKind::Image,
                        state: WidgetState::ENABLED,
                        text: Some(format!("media-request:{}", request.get())),
                        icon: None,
                    },
                    parent: Some(cell),
                    index: 0,
                });
                patches.push(Patch::Create {
                    widget: WidgetSpec {
                        id: button,
                        kind: WidgetKind::Button,
                        state: state | WidgetState::FLAT,
                        text: Some(display_label(item).to_owned()),
                        icon: None,
                    },
                    parent: Some(cell),
                    index: 1,
                });
            }
            index = index.saturating_add(1);
        }
    }
    patches
}

fn desktop_grid_token(config: GridConfig) -> String {
    format!(
        "desktop-grid:{}:{}:{}:{}:{}:{}:{}:{}",
        config.columns,
        config.rows,
        config.cell_width,
        config.cell_height,
        config.horizontal_gap,
        config.vertical_gap,
        config.origin_x,
        config.origin_y,
    )
}

pub(super) fn icon_for_item(item: &DesktopItem) -> IconId {
    if let Some(target) = item.icon_id.strip_prefix("shortcut:folder:") {
        return folder_icon(target).shortcut();
    }
    if item.icon_id.starts_with("shortcut:application:") {
        return IconId::Application.shortcut();
    }
    if let Some(target) = item.icon_id.strip_prefix("shortcut:file:") {
        return file_icon(target, None).shortcut();
    }
    match item.kind {
        DesktopItemKind::Home => IconId::Home,
        DesktopItemKind::Documents => IconId::Documents,
        DesktopItemKind::Pictures => IconId::Pictures,
        DesktopItemKind::Trash { empty: true } => IconId::Trash,
        DesktopItemKind::Trash { empty: false } => IconId::TrashFull,
        DesktopItemKind::Application { .. } => IconId::Application,
        DesktopItemKind::File if item.icon_id == "link" => IconId::Document.shortcut(),
        DesktopItemKind::File => file_icon(item.path.as_deref().unwrap_or(&item.label), None),
        DesktopItemKind::Folder | DesktopItemKind::Mount { .. } => {
            folder_icon(item.path.as_deref().unwrap_or(&item.label))
        }
    }
}

pub(super) fn message(
    app: AppId,
    window: WindowId,
    sequence: u64,
    patches: &[Patch],
) -> Result<Message, DesktopActorError> {
    Ok(Message::UiPatch(UiPatchEnvelope {
        app,
        window,
        sequence,
        patches: patches
            .iter()
            .map(Patch::to_protocol)
            .collect::<Result<_, _>>()?,
    }))
}

pub(super) fn update_text(id: u64, text: &str, state: Option<WidgetState>) -> Patch {
    Patch::Update {
        id: widget_id(id),
        state,
        text: ValueChange::Set(text.to_owned()),
        icon: ValueChange::Keep,
    }
}

pub(super) fn rubber_band_patch(existing: bool, area: Rect) -> Patch {
    let text = format!(
        "desktop-selection:{}:{}:{}:{}",
        area.x, area.y, area.width, area.height
    );
    if existing {
        return update_text(RUBBER_BAND, &text, Some(WidgetState::ENABLED));
    }
    Patch::Create {
        widget: WidgetSpec {
            id: widget_id(RUBBER_BAND),
            kind: WidgetKind::Container,
            state: WidgetState::ENABLED,
            text: Some(text),
            icon: None,
        },
        parent: Some(widget_id(ROOT)),
        index: 2,
    }
}

fn create(
    patches: &mut Vec<Patch>,
    id: u64,
    parent: Option<u64>,
    kind: WidgetKind,
    text: Option<&str>,
    icon: Option<IconId>,
) {
    let parent = parent.map(widget_id);
    let index = patches
        .iter()
        .filter(|patch| {
            matches!(
                patch,
                Patch::Create {
                    parent: candidate,
                    ..
                } if *candidate == parent
            )
        })
        .count();
    patches.push(Patch::Create {
        widget: WidgetSpec {
            id: widget_id(id),
            kind,
            state: if id == WALLPAPER {
                // The wallpaper spans the complete Desktop. It is a visual
                // surface, never an input surface; enabling Image widgets
                // makes them focusable in the shared UI host. A grid rebuild
                // after Paste can otherwise leave this full-screen hit target
                // above every icon and make the Desktop appear frozen.
                WidgetState::empty()
            } else if id == ICON_GRID {
                WidgetState::ENABLED | WidgetState::FOCUSED
            } else {
                WidgetState::ENABLED
            },
            text: text.map(str::to_owned),
            icon,
        },
        parent,
        index,
    });
}

pub(super) fn widget_id(value: u64) -> WidgetId {
    WidgetId::new(value).expect("RiceDesktop widget IDs are nonzero")
}

pub(super) fn window_id(value: u64) -> WindowId {
    WindowId::new(value).expect("RiceDesktop window IDs are nonzero")
}

fn display_label(item: &DesktopItem) -> &str {
    match item.kind {
        DesktopItemKind::Home | DesktopItemKind::Documents | DesktopItemKind::Pictures | DesktopItemKind::Trash { .. } => rice_i18n::t(&item.label),
        _ => &item.label,
    }
}
