//! Persistent, bounded user Places for RiceFiles.

use rice_files::{Bookmark, GuestPath};
use rice_native_app_protocol::{
    Message, RequestId, SettingEntry, SettingsOperation, SettingsRequest, SettingsResponse,
    SettingsResult,
};

use super::{FilesActorError, RiceFiles};

const SETTING_KEY: &str = "ricefiles.places";
const LABELS_KEY: &str = "ricefiles.places-with-labels";
const DOWNLOAD_TIP_KEY: &str = "ricefiles.download-tip-shown";
const SIMPLIFIED_VIEW_KEY_PREFIX: &str = "ricefiles.simplified-view.";
const FOLDER_TREE_KEY_PREFIX: &str = "ricefiles.folder-tree.";
const MAX_SETTING_BYTES: usize = 16 * 1024;
const MAX_PENDING: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingKind {
    Load {
        revision: u64,
        simplified_view_key: String,
        folder_tree_key: String,
    },
    Save {
        setting_key: &'static str,
        message_key: &'static str,
        count: Option<usize>,
    },
    SaveDownloadTip,
    SaveOptions(super::files_options::Options),
    SaveSimplifiedView {
        previous: bool,
        key: String,
    },
    SaveFolderTree {
        previous: bool,
        key: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingPreferences {
    request: RequestId,
    kind: PendingKind,
}

pub(super) fn load(actor: &mut RiceFiles, request: RequestId) -> Message {
    let simplified_view_key = simplified_view_key(actor);
    let folder_tree_key = folder_tree_key(actor);
    track(
        actor,
        PendingPreferences {
            request,
            kind: PendingKind::Load {
                revision: actor.preferences_revision,
                simplified_view_key,
                folder_tree_key,
            },
        },
    );
    settings_request(actor, request, SettingsOperation::Load)
}

pub(super) fn add_current(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    add_paths(actor, vec![actor.location().clone()], request)
}

pub(super) fn add_paths(
    actor: &mut RiceFiles,
    paths: Vec<GuestPath>,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let mut updated = actor.preferences.clone();
    let mut added = 0_usize;
    for path in paths {
        if is_builtin(path.as_str())
            || updated
                .bookmarks()
                .iter()
                .any(|bookmark| bookmark.path == path)
        {
            continue;
        }
        let label = path
            .as_str()
            .rsplit('/')
            .find(|component| !component.is_empty())
            .unwrap_or("Root")
            .to_owned();
        let bookmark = if path.is_root() { Bookmark::system_root() } else {
            Bookmark::new(path, label).map_err(|_| FilesActorError::InvalidSettings)?
        };
        updated
            .add_bookmark(bookmark)
            .map_err(|_| FilesActorError::InvalidSettings)?;
        added = added.saturating_add(1);
    }
    if added == 0 {
        return actor.emit_notice(rice_i18n::t("This folder is already in Places"));
    }
    actor.preferences = updated;
    actor.preferences_revision = actor.preferences_revision.saturating_add(1);
    let message_key = if added == 1 {
        "files.places.3341b4febf00"
    } else {
        "files.places.3e1bac45e84c"
    };
    save(actor, request, message_key, Some(added))
}

pub(super) fn remove_current(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let path = actor.location().clone();
    if !actor.preferences.remove_bookmark(&path) {
        return actor.emit_notice(rice_i18n::t("This folder is not a removable Place"));
    }
    actor.preferences_revision = actor.preferences_revision.saturating_add(1);
    save(actor, request, "files.places.b2cbe0107933", None)
}

pub(super) fn remove_bookmark(
    actor: &mut RiceFiles,
    index: usize,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let Some(path) = actor
        .preferences
        .bookmarks()
        .get(index)
        .map(|bookmark| bookmark.path.clone())
    else {
        return Ok(Vec::new());
    };
    if !actor.preferences.remove_bookmark(&path) {
        return Ok(Vec::new());
    }
    actor.preferences_revision = actor.preferences_revision.saturating_add(1);
    save(actor, request, "files.places.b2cbe0107933", None)
}

pub(super) fn remove_unavailable_paths(
    actor: &mut RiceFiles,
    paths: &[GuestPath],
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let removed = actor
        .preferences
        .bookmarks()
        .iter()
        .filter(|bookmark| {
            paths.iter().any(|path| {
                bookmark.path == *path
                    || bookmark
                        .path
                        .as_str()
                        .strip_prefix(path.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
        })
        .map(|bookmark| bookmark.path.clone())
        .collect::<Vec<_>>();
    if removed.is_empty() {
        return Ok(Vec::new());
    }
    for path in &removed {
        actor.preferences.remove_bookmark(path);
    }
    actor.preferences_revision = actor.preferences_revision.saturating_add(1);
    let message_key = if removed.len() == 1 {
        "files.places.265639747d55"
    } else {
        "files.places.98c0185b8697"
    };
    save(actor, request, message_key, Some(removed.len()))
}

pub(super) fn mark_download_tip_shown(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Option<Message> {
    if actor.download_tip_shown {
        return None;
    }
    actor.download_tip_shown = true;
    if actor.pending_preferences.len() == MAX_PENDING {
        return None;
    }
    track(
        actor,
        PendingPreferences {
            request,
            kind: PendingKind::SaveDownloadTip,
        },
    );
    Some(settings_request(
        actor,
        request,
        SettingsOperation::Set {
            key: DOWNLOAD_TIP_KEY.to_owned(),
            value: String::from("1"),
        },
    ))
}

pub(super) fn toggle_simplified_view(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_preferences.len() == MAX_PENDING {
        return actor.emit_notice(rice_i18n::t("The layout changed, but the settings host is still busy"));
    }
    let previous = actor.simplified_view;
    actor.simplified_view = !previous;
    let key = simplified_view_key(actor);
    track(
        actor,
        PendingPreferences {
            request,
            kind: PendingKind::SaveSimplifiedView {
                previous,
                key: key.clone(),
            },
        },
    );
    Ok(vec![
        actor.emit_full()?,
        settings_request(
            actor,
            request,
            SettingsOperation::Set {
                key,
                value: actor.simplified_view.to_string(),
            },
        ),
    ])
}

pub(super) fn toggle_folder_tree(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_preferences.len() == MAX_PENDING {
        return actor.emit_notice(rice_i18n::t("The folder tree changed, but the settings host is still busy"));
    }
    let previous = actor.folder_tree;
    actor.folder_tree = !previous;
    let key = folder_tree_key(actor);
    track(
        actor,
        PendingPreferences {
            request,
            kind: PendingKind::SaveFolderTree {
                previous,
                key: key.clone(),
            },
        },
    );
    Ok(vec![
        actor.emit_full()?,
        settings_request(
            actor,
            request,
            SettingsOperation::Set {
                key,
                value: actor.folder_tree.to_string(),
            },
        ),
    ])
}

fn save(
    actor: &mut RiceFiles,
    request: RequestId,
    message_key: &'static str,
    count: Option<usize>,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_preferences.len() == MAX_PENDING {
        return actor.emit_notice(rice_i18n::t("Places changed, but the settings host is still busy"));
    }
    let places = encode(actor)?;
    let system_root = actor.preferences.bookmarks().iter().any(Bookmark::has_system_root_label);
    let (setting_key, value) = if system_root {
        let value = serde_json::json!({"version":1,"places":places,"systemRootLabel":true}).to_string();
        if value.len() > MAX_SETTING_BYTES { return Err(FilesActorError::SettingsTooLarge); }
        (LABELS_KEY, value)
    } else { (SETTING_KEY, places) };
    track(
        actor,
        PendingPreferences {
            request,
            kind: PendingKind::Save { setting_key, message_key, count },
        },
    );
    actor.runtime_mut()?.content_notice.clear();
    Ok(vec![
        actor.emit_full()?,
        settings_request(
            actor,
            request,
            SettingsOperation::Set {
                key: setting_key.to_owned(),
                value,
            },
        ),
    ])
}

pub(super) fn handle_response(
    actor: &mut RiceFiles,
    response: &SettingsResponse,
) -> Result<Vec<Message>, FilesActorError> {
    let Some(index) = actor
        .pending_preferences
        .iter()
        .position(|pending| pending.request == response.request)
    else {
        return Err(FilesActorError::UnexpectedSettingsResponse);
    };
    let pending = actor
        .pending_preferences
        .remove(index)
        .ok_or(FilesActorError::UnexpectedSettingsResponse)?;
    match (pending.kind, &response.result) {
        (PendingKind::SaveOptions(options), SettingsResult::Applied(entry)) if entry.key == super::files_options::KEY => {
            actor.options_dialog = None;
            super::files_options::apply(actor, options, response.request)
        }
        (PendingKind::SaveOptions(_), SettingsResult::Error(error)) => actor.emit_notice(&rice_i18n::format("files.places.edc45b7a5fa9", &[("error", &error)])),
        (
            PendingKind::Load {
                revision,
                simplified_view_key: pending_simplified_view_key,
                folder_tree_key: pending_folder_tree_key,
            },
            SettingsResult::Values(entries),
        ) => {
            let previous_actions = actor.user_actions.clone();
            actor.user_actions = entries.iter().find(|entry| entry.key == "desktop.actions")
                .map(|entry| super::files_user_actions::decode(&entry.value)).unwrap_or_default();
            if let Some(entry) = entries.iter().find(|entry| entry.key == "desktop.app-actions") {
                actor.user_actions.extend(super::files_user_actions::decode(&entry.value).into_iter().take(32));
            }
            let previous_simplified_view = actor.simplified_view;
            let previous_folder_tree = actor.folder_tree;
            if pending_simplified_view_key == simplified_view_key(actor)
                && let Some(value) = entries
                    .iter()
                    .find(|entry| entry.key == pending_simplified_view_key)
                    .map(|entry| entry.value.as_str())
            {
                actor.simplified_view = match value {
                    "true" => true,
                    "false" => false,
                    _ => actor.simplified_view,
                };
            }
            if pending_folder_tree_key == folder_tree_key(actor)
                && let Some(value) = entries
                    .iter()
                    .find(|entry| entry.key == pending_folder_tree_key)
                    .map(|entry| entry.value.as_str())
            {
                actor.folder_tree = match value {
                    "true" => true,
                    "false" => false,
                    _ => actor.folder_tree,
                };
            }
            actor.download_tip_shown = entries
                .iter()
                .any(|entry| entry.key == DOWNLOAD_TIP_KEY && entry.value == "1");
            let mut changed = actor.user_actions != previous_actions || actor.simplified_view != previous_simplified_view
                || actor.folder_tree != previous_folder_tree;
            if actor.preferences_revision == revision
                && let Some(value) = setting_value(entries)
            {
                actor.preferences = decode_with_labels(value, entries.iter().find(|entry| entry.key == LABELS_KEY).map(|entry| entry.value.as_str()))?;
                changed = true;
            }
            if actor.preferences_revision == revision && let Some(options) = entries.iter().find(|entry| entry.key == super::files_options::KEY)
                .and_then(|entry| serde_json::from_str::<super::files_options::Options>(&entry.value).ok()) {
                return super::files_options::apply(actor, options, response.request);
            }
            if changed {
                actor.emit_full().map(|message| vec![message])
            } else {
                Ok(Vec::new())
            }
        }
        (PendingKind::Load { .. }, SettingsResult::Error(_)) => Ok(Vec::new()),
        (PendingKind::Save { setting_key, message_key, count }, SettingsResult::Applied(entry))
            if entry.key == setting_key =>
        {
            // Resolve owned prose only when the asynchronous save completes.
            // Bookmark labels and serialized preferences remain literal user data.
            let notice = match count {
                Some(count) => rice_i18n::format(message_key, &[("count", &count.to_string())]),
                None => rice_i18n::t(message_key).to_owned(),
            };
            actor.emit_notice(&notice)
        }
        (PendingKind::Save { .. }, SettingsResult::Error(error)) => {
            actor.emit_notice(&rice_i18n::format("files.places.68f65ffbcb9f", &[("error", &error)]))
        }
        (PendingKind::SaveDownloadTip, SettingsResult::Applied(entry))
            if entry.key == DOWNLOAD_TIP_KEY =>
        {
            Ok(Vec::new())
        }
        (PendingKind::SaveDownloadTip, SettingsResult::Error(_)) => {
            actor.download_tip_shown = false;
            Ok(Vec::new())
        }
        (PendingKind::SaveSimplifiedView { key, .. }, SettingsResult::Applied(entry))
            if entry.key == key =>
        {
            Ok(Vec::new())
        }
        (PendingKind::SaveSimplifiedView { previous, key }, SettingsResult::Error(error)) => {
            if key == simplified_view_key(actor) {
                actor.simplified_view = previous;
            }
            actor.emit_system_notice(
                "error",
                &rice_i18n::format("files.places.74ee8bb96f5e", &[("error", &error)]),
            )
        }
        (PendingKind::SaveFolderTree { key, .. }, SettingsResult::Applied(entry))
            if entry.key == key =>
        {
            Ok(Vec::new())
        }
        (PendingKind::SaveFolderTree { previous, key }, SettingsResult::Error(error)) => {
            if key == folder_tree_key(actor) {
                actor.folder_tree = previous;
            }
            actor.emit_system_notice(
                "error",
                &rice_i18n::format("files.places.41434f84187b", &[("error", &error)]),
            )
        }
        _ => Err(FilesActorError::UnexpectedSettingsResponse),
    }
}

fn simplified_view_key(actor: &RiceFiles) -> String {
    format!("{SIMPLIFIED_VIEW_KEY_PREFIX}{}", actor.theme_profile)
}

fn folder_tree_key(actor: &RiceFiles) -> String {
    format!("{FOLDER_TREE_KEY_PREFIX}{}", actor.theme_profile)
}

fn setting_value(entries: &[SettingEntry]) -> Option<&str> {
    entries
        .iter()
        .find(|entry| entry.key == SETTING_KEY)
        .map(|entry| entry.value.as_str())
}

fn settings_request(
    actor: &RiceFiles,
    request: RequestId,
    operation: SettingsOperation,
) -> Message {
    Message::SettingsRequest(SettingsRequest {
        request,
        app: actor.app,
        operation,
    })
}

fn track(actor: &mut RiceFiles, pending: PendingPreferences) {
    if actor.pending_preferences.len() < MAX_PENDING {
        actor.pending_preferences.push_back(pending);
    }
}

fn encode(actor: &RiceFiles) -> Result<String, FilesActorError> {
    let mut output = String::new();
    for bookmark in actor.preferences.bookmarks() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&hex_encode(bookmark.path.as_str().as_bytes()));
        output.push('\t');
        output.push_str(&hex_encode(bookmark.label.as_bytes()));
        if output.len() > MAX_SETTING_BYTES {
            return Err(FilesActorError::SettingsTooLarge);
        }
    }
    Ok(output)
}

#[cfg(test)]
fn decode(value: &str) -> Result<rice_files::FilePreferences, FilesActorError> {
    decode_with_labels(value, None)
}

fn decode_with_labels(value: &str, metadata: Option<&str>) -> Result<rice_files::FilePreferences, FilesActorError> {
    // A legacy or customized Root label is literal unless matching provenance is present.
    let owned_root = metadata.filter(|raw| raw.len() <= MAX_SETTING_BYTES)
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(raw).ok())
        .is_some_and(|data| data["version"].as_u64() == Some(1)
            && data["places"].as_str() == Some(value)
            && data["systemRootLabel"].as_bool() == Some(true));
    if value.len() > MAX_SETTING_BYTES {
        return Err(FilesActorError::SettingsTooLarge);
    }
    let mut preferences = rice_files::FilePreferences::default();
    if value.is_empty() {
        return Ok(preferences);
    }
    for line in value.lines() {
        let (path, label) = line
            .split_once('\t')
            .ok_or(FilesActorError::InvalidSettings)?;
        let path =
            String::from_utf8(hex_decode(path)?).map_err(|_| FilesActorError::InvalidSettings)?;
        let label =
            String::from_utf8(hex_decode(label)?).map_err(|_| FilesActorError::InvalidSettings)?;
        if is_builtin(&path) {
            continue;
        }
        let path = GuestPath::absolute(&path).map_err(|_| FilesActorError::InvalidSettings)?;
        let bookmark = if owned_root && path.is_root() && label == "Root" {
            Bookmark::system_root()
        } else { Bookmark::new(path, label).map_err(|_| FilesActorError::InvalidSettings)? };
        preferences
            .add_bookmark(bookmark)
            .map_err(|_| FilesActorError::InvalidSettings)?;
    }
    Ok(preferences)
}

pub(super) fn bookmark_display_label(bookmark: &Bookmark) -> &str {
    if bookmark.has_system_root_label() { rice_i18n::t("ui.root") } else { &bookmark.label }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn hex_decode(value: &str) -> Result<Vec<u8>, FilesActorError> {
    if !value.len().is_multiple_of(2) {
        return Err(FilesActorError::InvalidSettings);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0]).ok_or(FilesActorError::InvalidSettings)?;
            let low = hex_digit(pair[1]).ok_or(FilesActorError::InvalidSettings)?;
            Ok((high << 4) | low)
        })
        .collect()
}

const fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

pub(super) fn is_builtin(path: &str) -> bool {
    matches!(
        path,
        "/root"
            | "/root/Desktop"
            | "/root/Documents"
            | "/root/Downloads"
            | "/root/Pictures"
            | "/root/Videos"
            | "/root/.local/share/Trash/files"
            | "/mnt/local"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{
        THEME_CHANGED_TOPIC,
        files_menu::menu_widget,
        files_view::{BOOKMARK_BASE, ROOT, SIMPLIFIED_VIEW, widget_id},
        tests::{app, request},
    };
    use rice_native_app_protocol::{HostEvent, HostEventKind, SettingEntry, UiPatch};

    #[test]
    fn current_folder_becomes_a_persisted_navigable_place() {
        let (mut actor, _) =
            RiceFiles::launch_at(app(), &[String::from("/root/Projects")], request(1))
                .expect("launch folder");
        let messages = add_current(&mut actor, request(2)).expect("add place");
        assert!(matches!(
            messages.as_slice(),
            [Message::UiPatch(update), Message::SettingsRequest(SettingsRequest {
                operation: SettingsOperation::Set { key, value },
                ..
            })] if key == SETTING_KEY
                && !value.is_empty()
                && update.patches.iter().any(|patch| matches!(
                    patch,
                    UiPatch::Create { id, text: Some(text), .. }
                        if *id == widget_id(BOOKMARK_BASE) && text == "Projects"
                ))
        ));
        let saved = encode(&actor).expect("saved value");
        let applied = handle_response(
            &mut actor,
            &SettingsResponse {
                request: request(2),
                result: SettingsResult::Applied(SettingEntry {
                    key: SETTING_KEY.to_owned(),
                    value: saved,
                }),
            },
        )
        .expect("save acknowledgement");
        assert!(matches!(
            applied.as_slice(),
            [
                Message::UiPatch(_),
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification {
                        kind,
                        title,
                        message,
                    },
                    ..
                })
            ] if kind == "info"
                && title == "Files"
                && message == "Folder added to Places"
        ));
        let duplicate = add_current(&mut actor, request(3)).expect("duplicate place notice");
        assert!(actor.runtime_for_test().content_notice.is_empty());
        assert!(matches!(
            duplicate.as_slice(),
            [
                Message::UiPatch(_),
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification {
                        kind,
                        title,
                        message,
                    },
                    ..
                })
            ] if kind == "info"
                && title == "Files"
                && message == "This folder is already in Places"
        ));
    }

    #[test]
    fn saved_places_restore_from_bounded_settings_data() {
        let (mut source, _) =
            RiceFiles::launch_at(app(), &[String::from("/root/Projects")], request(1))
                .expect("source");
        add_current(&mut source, request(2)).expect("add");
        let value = encode(&source).expect("encode");

        let (mut restored, _) = RiceFiles::launch(app(), request(1)).expect("restore target");
        let messages = handle_response(
            &mut restored,
            &SettingsResponse {
                request: request(1),
                result: SettingsResult::Values(vec![SettingEntry {
                    key: SETTING_KEY.to_owned(),
                    value,
                }]),
            },
        )
        .expect("restore");
        assert_eq!(restored.preferences.bookmarks().len(), 1);
        assert_eq!(
            restored.preferences.bookmarks()[0].path.as_str(),
            "/root/Projects"
        );
        assert!(matches!(messages.as_slice(), [Message::UiPatch(_)]));
    }

    #[test]
    fn simplified_view_defaults_from_theme_and_persists_across_windows() {
        let (mut orchard, _) = RiceFiles::launch_at(
            app(),
            &[
                String::from("--theme-profile=orchard"),
                String::from("--simplified-view-default=true"),
            ],
            request(1),
        )
        .expect("Orchard RiceFiles");
        assert!(orchard.simplified_view);
        let orchard_key = simplified_view_key(&orchard);

        let messages = toggle_simplified_view(&mut orchard, request(2)).expect("disable layout");
        assert!(!orchard.simplified_view);
        assert!(matches!(
            messages.as_slice(),
            [Message::UiPatch(update), Message::SettingsRequest(SettingsRequest {
                operation: SettingsOperation::Set { key, value },
                ..
            })] if key == &orchard_key
                && value == "false"
                && update.patches.iter().any(|patch| matches!(
                    patch,
                    UiPatch::Create { id, text: Some(text), .. }
                        if *id == widget_id(ROOT) && text == "panel:files-shell"
                ))
                && update.patches.iter().any(|patch| matches!(
                    patch,
                    UiPatch::Create { id, state, .. }
                        if *id == widget_id(menu_widget(SIMPLIFIED_VIEW))
                            && *state & rice_ui::WidgetState::CHECKED.bits() == 0
                ))
        ));

        let (mut jade, _) = RiceFiles::launch_at(
            app(),
            &[
                String::from("--theme-profile=jade"),
                String::from("--simplified-view-default=true"),
            ],
            request(3),
        )
        .expect("Jade RiceFiles");
        let restored = handle_response(
            &mut jade,
            &SettingsResponse {
                request: request(3),
                result: SettingsResult::Values(vec![SettingEntry {
                    key: orchard_key,
                    value: String::from("false"),
                }]),
            },
        )
        .expect("another theme's saved layout is ignored");
        assert!(jade.simplified_view);
        assert!(restored.is_empty());

        let (mut restored_orchard, _) = RiceFiles::launch_at(
            app(),
            &[
                String::from("--theme-profile=orchard"),
                String::from("--simplified-view-default=true"),
            ],
            request(4),
        )
        .expect("second Orchard RiceFiles");
        let restored_orchard_key = simplified_view_key(&restored_orchard);
        let restored = handle_response(
            &mut restored_orchard,
            &SettingsResponse {
                request: request(4),
                result: SettingsResult::Values(vec![SettingEntry {
                    key: restored_orchard_key,
                    value: String::from("false"),
                }]),
            },
        )
        .expect("same theme's saved layout loads");
        assert!(!restored_orchard.simplified_view);
        assert!(matches!(restored.as_slice(), [Message::UiPatch(_)]));

        let (standard, _) = RiceFiles::launch(app(), request(5)).expect("standard RiceFiles");
        assert!(!standard.simplified_view);
    }

    #[test]
    fn folder_tree_defaults_from_theme_and_preserves_each_theme_override() {
        let (mut redmond, _) = RiceFiles::launch_at(
            app(),
            &[String::from("--theme-profile=redmond-98")],
            request(1),
        )
        .expect("Redmond RiceFiles");
        assert!(redmond.folder_tree);
        let key = folder_tree_key(&redmond);
        let changed = toggle_folder_tree(&mut redmond, request(2)).expect("disable tree");
        assert!(!redmond.folder_tree);
        assert!(matches!(
            changed.as_slice(),
            [Message::UiPatch(_), Message::SettingsRequest(SettingsRequest {
                operation: SettingsOperation::Set { key: saved_key, value },
                ..
            })] if saved_key == &key && value == "false"
        ));

        let (standard, _) = RiceFiles::launch(app(), request(3)).expect("standard RiceFiles");
        assert!(!standard.folder_tree);

        let (mut restored, _) = RiceFiles::launch_at(
            app(),
            &[String::from("--theme-profile=redmond-98")],
            request(4),
        )
        .expect("restored Redmond RiceFiles");
        let restored_key = folder_tree_key(&restored);
        let messages = handle_response(
            &mut restored,
            &SettingsResponse {
                request: request(4),
                result: SettingsResult::Values(vec![SettingEntry {
                    key: restored_key,
                    value: String::from("false"),
                }]),
            },
        )
        .expect("restore Redmond tree override");
        assert!(!restored.folder_tree);
        assert!(matches!(messages.as_slice(), [Message::UiPatch(_)]));
    }

    #[test]
    fn live_theme_changes_restore_the_profile_layout_without_relaunching() {
        let (mut actor, _) = RiceFiles::launch_at(
            app(),
            &[
                String::from("--theme-profile=orchard"),
                String::from("--simplified-view-default=true"),
            ],
            request(1),
        )
        .expect("Orchard RiceFiles");
        assert!(actor.simplified_view);

        let messages = actor
            .handle_host_event(
                &HostEvent {
                    app: app(),
                    event: HostEventKind::RiceBus {
                        topic: THEME_CHANGED_TOPIC.to_owned(),
                        payload: String::from("redmond-98"),
                    },
                },
                request(2),
            )
            .expect("apply Redmond profile");
        assert!(actor.redmond98);
        assert!(!actor.simplified_view);
        assert!(matches!(
            messages.as_slice(),
            [
                Message::UiPatch(_),
                Message::SettingsRequest(SettingsRequest {
                    operation: SettingsOperation::Load,
                    ..
                })
            ]
        ));

        let redmond_key = simplified_view_key(&actor);
        let restored = handle_response(
            &mut actor,
            &SettingsResponse {
                request: request(2),
                result: SettingsResult::Values(vec![SettingEntry {
                    key: redmond_key,
                    value: String::from("true"),
                }]),
            },
        )
        .expect("restore Redmond preference");
        assert!(actor.simplified_view);
        assert!(matches!(restored.as_slice(), [Message::UiPatch(_)]));

        actor
            .handle_host_event(
                &HostEvent {
                    app: app(),
                    event: HostEventKind::RiceBus {
                        topic: THEME_CHANGED_TOPIC.to_owned(),
                        payload: String::from("orchard"),
                    },
                },
                request(3),
            )
            .expect("restore Orchard profile");
        assert!(!actor.redmond98);
        assert!(actor.simplified_view);
    }

    #[test]
    fn custom_places_have_remove_actions_and_deleted_parents_prune_descendants() {
        let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
        let projects = GuestPath::absolute("/root/Projects").expect("projects");
        let nested = GuestPath::absolute("/root/Projects/archive").expect("nested");
        let messages =
            add_paths(&mut actor, vec![projects.clone(), nested], request(2)).expect("add places");
        let Message::UiPatch(update) = &messages[0] else {
            panic!("place update");
        };
        assert!(update.patches.iter().any(|patch| matches!(
            patch,
            UiPatch::Create { id, text: Some(text), .. }
                if *id == widget_id(crate::files::files_view::BOOKMARK_REMOVE_BASE)
                    && text == "Remove from Places"
        )));

        remove_unavailable_paths(&mut actor, &[projects], request(3)).expect("prune places");
        assert!(actor.preferences.bookmarks().is_empty());
    }

    fn complete_place_save_after_language_change(
        actor: &mut RiceFiles,
        queued: Vec<Message>,
        request_id: RequestId,
        expected_key: &str,
        count: Option<usize>,
    ) {
        let serialized = queued.iter().find_map(|message| match message {
            Message::SettingsRequest(SettingsRequest {
                operation: SettingsOperation::Set { key, value }, ..
            }) if key == SETTING_KEY => Some(value.clone()),
            _ => None,
        }).expect("real settings save request");
        let bookmarks = actor.preferences.bookmarks().to_vec();
        assert_eq!(decode(&serialized).unwrap().bookmarks(), bookmarks.as_slice());
        let expected_in = |locale| rice_i18n::with_locale(locale, || match count {
            Some(count) => rice_i18n::format(expected_key, &[("count", &count.to_string())]),
            None => rice_i18n::t(expected_key).to_owned(),
        });
        let chinese = expected_in(2);
        let spanish = expected_in(1);
        assert_ne!(chinese, spanish);
        rice_i18n::with_locale(1, || {
            // The global locale event rebuilds this same actor with emit_full().
            actor.emit_full().expect("rebuild after language change");
            let messages = handle_response(actor, &SettingsResponse {
                request: request_id,
                result: SettingsResult::Applied(SettingEntry {
                    key: SETTING_KEY.to_owned(), value: serialized.clone(),
                }),
            }).expect("save acknowledgment in new language");
            let notifications = messages.iter().filter_map(|message| match message {
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification {
                        kind, title, message,
                    }, ..
                }) => Some((kind, title, message)),
                _ => None,
            }).collect::<Vec<_>>();
            assert_eq!(notifications.len(), 1);
            assert_eq!(notifications[0].0, "info");
            assert_eq!(notifications[0].1, rice_i18n::t("Files"));
            assert_eq!(notifications[0].2, &spanish);
            assert_ne!(notifications[0].2, &chinese);
        });
        assert_eq!(actor.preferences.bookmarks(), bookmarks.as_slice());
        assert_eq!(encode(actor).unwrap(), serialized);
    }

    #[test]
    fn pending_place_success_messages_use_acknowledgment_language_and_preserve_data() {
        let names = ["Save {count} 中文", "Folder removed from Places"];
        for count in [1, 2] {
            let (mut actor, _) = RiceFiles::launch(app(), request(1)).unwrap();
            let paths = names[..count].iter().map(|name|
                GuestPath::absolute(&format!("/root/{name}")).unwrap()
            ).collect::<Vec<_>>();
            let queued = rice_i18n::with_locale(2, ||
                add_paths(&mut actor, paths.clone(), request(2)).unwrap());
            for (bookmark, name) in actor.preferences.bookmarks().iter().zip(&names[..count]) {
                assert_eq!(bookmark.label.as_str(), *name);
            }
            complete_place_save_after_language_change(&mut actor, queued, request(2),
                if count == 1 { "files.places.3341b4febf00" } else { "files.places.3e1bac45e84c" }, Some(count));
            let queued = rice_i18n::with_locale(2, ||
                remove_unavailable_paths(&mut actor, &paths, request(3)).unwrap());
            assert!(actor.preferences.bookmarks().is_empty());
            complete_place_save_after_language_change(&mut actor, queued, request(3),
                if count == 1 { "files.places.265639747d55" } else { "files.places.98c0185b8697" }, Some(count));
        }
        for remove_by_index in [false, true] {
            let path = "/root/Save {count} 中文";
            let (mut actor, _) = RiceFiles::launch_at(app(), &[path.into()], request(1)).unwrap();
            let queued = rice_i18n::with_locale(2, || add_current(&mut actor, request(2)).unwrap());
            complete_place_save_after_language_change(&mut actor, queued, request(2),
                "files.places.3341b4febf00", Some(1));
            let queued = rice_i18n::with_locale(2, || {
                if remove_by_index { remove_bookmark(&mut actor, 0, request(3)) }
                else { remove_current(&mut actor, request(3)) }
            }).unwrap();
            assert!(actor.preferences.bookmarks().is_empty());
            complete_place_save_after_language_change(&mut actor, queued, request(3),
                "files.places.b2cbe0107933", None);
            assert_eq!(actor.location().as_str(), path);
        }
    }

    #[test]
    fn pending_place_failure_preserves_raw_provider_detail_after_language_change() {
        let (mut actor, _) = RiceFiles::launch_at(app(), &["/root/Save {error}".into()], request(1)).unwrap();
        rice_i18n::with_locale(2, || add_current(&mut actor, request(2))).unwrap();
        let serialized = encode(&actor).unwrap();
        let detail = "Folder added to Places {error} 中文";
        rice_i18n::with_locale(1, || {
            let output = handle_response(&mut actor, &SettingsResponse {
                request: request(2), result: SettingsResult::Error(detail.into()),
            }).unwrap();
            let expected = rice_i18n::format("files.places.68f65ffbcb9f", &[("error", detail)]);
            assert!(expected.contains(detail));
            assert!(output.iter().any(|message| matches!(message,
                Message::HostRequest(rice_native_app_protocol::HostRequest {
                    operation: rice_native_app_protocol::HostOperation::ShowNotification { message, .. }, ..
                }) if message == &expected)));
        });
        assert_eq!(encode(&actor).unwrap(), serialized);
        assert_eq!(actor.preferences.bookmarks()[0].label, "Save {error}");
    }

    #[test]
    fn generated_root_roundtrips_settings_envelope_and_keeps_legacy_bookmark_bytes() {
        for &locale in rice_i18n::LOCALE_CODES {
            rice_i18n::with_locale(locale, || {
                let (mut actor, _) = RiceFiles::launch(app(), request(1)).unwrap();
                let messages = add_paths(&mut actor, vec![GuestPath::root()], request(2)).unwrap();
                let (key, value) = messages.iter().find_map(|message| match message {
                    Message::SettingsRequest(SettingsRequest { operation: SettingsOperation::Set { key, value }, .. }) => Some((key.clone(), value.clone())), _ => None,
                }).unwrap();
                assert_eq!(key, LABELS_KEY);
                let envelope: serde_json::Value = serde_json::from_str(&value).unwrap();
                let places = envelope["places"].as_str().unwrap();
                assert_eq!(places, "2f\t526f6f74");
                assert_eq!(decode(places).unwrap().bookmarks()[0].label, "Root", "old readers see valid unchanged data");
                handle_response(&mut actor, &SettingsResponse { request: request(2), result: SettingsResult::Applied(SettingEntry { key, value: value.clone() }) }).unwrap();
                let (mut restored, _) = RiceFiles::launch(app(), request(3)).unwrap();
                handle_response(&mut restored, &SettingsResponse { request: request(3), result: SettingsResult::Values(vec![
                    SettingEntry { key: SETTING_KEY.into(), value: places.into() },
                    SettingEntry { key: LABELS_KEY.into(), value: value.clone() },
                ]) }).unwrap();
                assert!(restored.preferences.bookmarks()[0].has_system_root_label());
                for &next_locale in rice_i18n::LOCALE_CODES {
                    rice_i18n::with_locale(next_locale, || {
                        for tree in [false, true] {
                            restored.folder_tree = tree;
                            let patches = restored.full_patches().unwrap();
                            for id in if tree { vec![BOOKMARK_BASE] } else { vec![BOOKMARK_BASE, super::super::files_view::BOOKMARK_CONTEXT_MENU_BASE] } {
                                let label = patches.iter().find_map(|patch| match patch {
                                    rice_ui::Patch::Create { widget, .. } if widget.id.get() == id => widget.text.as_deref(), _ => None,
                                }).unwrap();
                                assert_eq!(label, rice_i18n::t("ui.root"));
                            }
                        }
                        assert_eq!(encode(&restored).unwrap(), places);
                        assert_eq!(restored.preferences.bookmarks()[0].path.as_str(), "/");
                    });
                }
                let removed = remove_bookmark(&mut restored, 0, request(4)).unwrap();
                assert!(removed.iter().any(|message| matches!(message, Message::SettingsRequest(SettingsRequest {
                    operation: SettingsOperation::Set { key, value }, ..
                }) if key == SETTING_KEY && value.is_empty())));
            });
        }
    }

    #[test]
    fn legacy_custom_stale_or_invalid_root_metadata_never_translates_labels() {
        for (path, label) in [("/", "Root"), ("/custom", "Root"), ("/", "Settings"), ("/", "common.save")] {
            let places = format!("{}\t{}", hex_encode(path.as_bytes()), hex_encode(label.as_bytes()));
            let metadata = serde_json::json!({"version":1,"places":places,"systemRootLabel":true}).to_string();
            let stale = serde_json::json!({"version":1,"places":"different","systemRootLabel":true}).to_string();
            let unknown = serde_json::json!({"version":2,"places":places,"systemRootLabel":true}).to_string();
            for raw in [None, Some(""), Some("invalid"), Some(stale.as_str()), Some(unknown.as_str())].into_iter()
                .chain((path != "/" || label != "Root").then_some(Some(metadata.as_str()))) {
                let preferences = decode_with_labels(&places, raw).unwrap();
                let bookmark = &preferences.bookmarks()[0];
                assert!(!bookmark.has_system_root_label());
                for &locale in rice_i18n::LOCALE_CODES {
                    rice_i18n::with_locale(locale, || assert_eq!(bookmark_display_label(bookmark), label));
                }
                assert_eq!(bookmark.path.as_str(), path);
            }
        }
    }

    #[test]
    fn every_builtin_place_is_protected_from_custom_place_mutation() {
        let (mut actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
        for path in [
            "/root",
            "/root/Desktop",
            "/root/Documents",
            "/root/Downloads",
            "/root/Pictures",
            "/root/Videos",
            "/root/.local/share/Trash/files",
            "/mnt/local",
        ] {
            add_paths(
                &mut actor,
                vec![GuestPath::absolute(path).expect("builtin path")],
                request(2),
            )
            .expect("builtin is ignored");
        }
        assert!(actor.preferences.bookmarks().is_empty());
    }
}

pub(super) fn save_options(actor: &mut RiceFiles, request: RequestId, options: super::files_options::Options) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_preferences.len() == MAX_PENDING { return actor.emit_notice(rice_i18n::t("Wait for settings to finish saving")); }
    actor.preferences_revision = actor.preferences_revision.saturating_add(1);
    let value = serde_json::to_string(&options).map_err(|_| FilesActorError::InvalidSettings)?;
    track(actor, PendingPreferences { request, kind: PendingKind::SaveOptions(options) });
    Ok(vec![settings_request(actor, request, SettingsOperation::Set { key: super::files_options::KEY.to_owned(), value })])
}
