//! Saved Files preferences and the modal editor for them.
use super::*;
use rice_ui::WidgetState;

pub(super) const KEY: &str = "ricefiles.preferences";
const DIALOG: u64 = 350_000;
const BODY: u64 = 350_001;
const THUMBNAILS: u64 = 350_002;
const LIMIT: u64 = 350_003;
const HIDDEN: u64 = 350_004;
const FOLDERS: u64 = 350_005;
const ZOOM: u64 = 350_006;
const PREVIEW: u64 = 350_007;
const SAVE: u64 = 350_008;
const CANCEL: u64 = 350_009;
const RESET: u64 = 350_010;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub(super) struct Options {
    pub thumbnails: bool,
    pub limit_mib: u32,
    pub show_hidden: bool,
    pub folders_first: bool,
    pub zoom: u16,
    pub preview: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            thumbnails: true,
            limit_mib: 64,
            show_hidden: false,
            folders_first: true,
            zoom: 1000,
            preview: false,
        }
    }
}
impl Options {
    pub fn maximum_bytes(&self) -> u64 {
        u64::from(self.limit_mib.clamp(16, 8192)) * 1024 * 1024
    }
    pub fn configure(&self, runtime: &mut files_pane::PaneRuntime) {
        runtime.sort.directories_first = self.folders_first;
        runtime.controls.preview_visible = self.preview;
    }
}

pub(super) fn patches(patches: &mut Vec<Patch>, options: &Options) {
    use files_view::{create, create_with_state};
    create(
        patches,
        DIALOG,
        Some(ROOT),
        WidgetKind::Dialog,
        Some(rice_i18n::t("Files Preferences")),
        Some(rice_ui::IconId::Settings),
    );
    create(patches, BODY, Some(DIALOG), WidgetKind::Column, None, None);
    for (id, label, checked) in [
        (
            THUMBNAILS,
            rice_i18n::t("Show image and video thumbnails"),
            options.thumbnails,
        ),
        (HIDDEN, rice_i18n::t("Show hidden files and folders"), options.show_hidden),
        (FOLDERS, rice_i18n::t("Sort folders before files"), options.folders_first),
        (PREVIEW, rice_i18n::t("Show preview pane by default"), options.preview),
    ] {
        create_with_state(
            patches,
            id,
            Some(BODY),
            WidgetKind::CheckBox,
            Some(label),
            None,
            WidgetState::ENABLED
                | if checked {
                    WidgetState::CHECKED
                } else {
                    WidgetState::empty()
                },
        );
    }
    create(
        patches,
        350_011,
        Some(BODY),
        WidgetKind::Label,
        Some(&rice_i18n::format("Thumbnail file size limit: {limit} MiB", &[("limit", &options.limit_mib.to_string())])),
        None,
    );
    create_with_state(
        patches,
        LIMIT,
        Some(BODY),
        WidgetKind::ScrollBar,
        Some(&format!("{}/511", options.limit_mib / 16 - 1)),
        None,
        WidgetState::ENABLED | WidgetState::FLAT,
    );
    create(
        patches,
        350_012,
        Some(BODY),
        WidgetKind::Label,
        Some(rice_i18n::t("16 MiB – 8 GiB. Larger files keep a file icon.")),
        None,
    );
    create(
        patches,
        350_013,
        Some(BODY),
        WidgetKind::Label,
        Some(&rice_i18n::format("Default icon size: {percent}%", &[("percent", &(options.zoom / 10).to_string())])),
        None,
    );
    create_with_state(
        patches,
        ZOOM,
        Some(BODY),
        WidgetKind::ScrollBar,
        Some(&format!("{}/4500", options.zoom.saturating_sub(500))),
        None,
        WidgetState::ENABLED | WidgetState::FLAT,
    );
    create(
        patches,
        350_014,
        Some(DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    for (id, label) in [
        (RESET, rice_i18n::t("Restore Defaults")),
        (CANCEL, rice_i18n::t("Cancel")),
        (SAVE, rice_i18n::t("Save")),
    ] {
        create(
            patches,
            id,
            Some(350_014),
            WidgetKind::Button,
            Some(label),
            None,
        );
    }
}

pub(super) fn event(
    actor: &mut RiceFiles,
    envelope: &UiEventEnvelope,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    let Some(options) = actor.options_dialog.as_mut() else {
        return Ok(vec![]);
    };
    match (&envelope.event, envelope.target.get()) {
        (UiEvent::Activate, SAVE) => {
            let options = options.clone();
            return files_preferences::save_options(actor, request, options);
        }
        (UiEvent::Activate, CANCEL)
        | (
            UiEvent::Key {
                pressed: true,
                key_code: 9,
                ..
            },
            _,
        ) => actor.options_dialog = None,
        (UiEvent::Activate, RESET) => *options = Options::default(),
        (UiEvent::Activate, THUMBNAILS) => options.thumbnails = !options.thumbnails,
        (UiEvent::Activate, HIDDEN) => options.show_hidden = !options.show_hidden,
        (UiEvent::Activate, FOLDERS) => options.folders_first = !options.folders_first,
        (UiEvent::Activate, PREVIEW) => options.preview = !options.preview,
        (UiEvent::ValueChanged(value), LIMIT | ZOOM) => {
            let Some(position) = value.split('/').next().and_then(|v| v.parse::<u32>().ok()) else {
                return Ok(vec![]);
            };
            if envelope.target.get() == LIMIT {
                options.limit_mib = (position.min(511) + 1) * 16;
            } else {
                options.zoom = (position.min(4500) + 500) as u16;
            }
        }
        _ => return Ok(vec![]),
    }
    actor.emit_full().map(|message| vec![message])
}

pub(super) fn apply(
    actor: &mut RiceFiles,
    mut options: Options,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    options.limit_mib = options.limit_mib.clamp(16, 8192) / 16 * 16;
    options.zoom = options
        .zoom
        .clamp(MIN_VIEW_ZOOM_PERMILLE, MAX_VIEW_ZOOM_PERMILLE);
    let mut messages = if !options.preview {
        actor.dismiss_preview()?
    } else {
        Vec::new()
    };
    actor.view_zoom_permille = options.zoom;
    for tab in actor.pane_runtimes.values_mut() {
        for runtime in [&mut tab.primary, &mut tab.secondary] {
            options.configure(runtime);
            sort_entries(&mut runtime.entries, runtime.sort);
            sort_entries(&mut runtime.directory_entries, runtime.sort);
            for entries in runtime.expanded_directories.values_mut() {
                sort_entries(entries, runtime.sort);
            }
        }
    }
    actor.options = options;
    actor.thumbnail_cache.clear();
    let entries = actor.visible_entries();
    let thumbnail = actor.reset_thumbnail_batch(&entries, request, true);
    messages.push(actor.emit_full()?);
    if let Some(message) = thumbnail {
        messages.push(message);
    }
    Ok(messages)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{app, event, loaded_actor, request};
    use super::*;
    use rice_native_app_protocol::{
        SettingEntry, SettingsOperation, SettingsRequest, SettingsResponse, SettingsResult,
    };

    #[test]
    fn thumbnail_limit_supports_eight_gib_without_overflow() {
        let options = Options {
            limit_mib: 8192,
            ..Options::default()
        };
        assert_eq!(options.maximum_bytes(), 8_u64 * 1024 * 1024 * 1024);
    }

    #[test]
    fn preferences_save_restore_and_cancel() {
        let (mut actor, _) = loaded_actor();
        actor
            .handle_event(
                &event(
                    files_menu::menu_widget(files_menu::PREFERENCES),
                    UiEvent::Activate,
                ),
                request(90),
            )
            .unwrap();
        assert!(actor.options_dialog.is_some());
        actor
            .handle_event(
                &event(LIMIT, UiEvent::ValueChanged("127/511".into())),
                request(91),
            )
            .unwrap();
        actor
            .handle_event(&event(HIDDEN, UiEvent::Activate), request(92))
            .unwrap();
        actor
            .handle_event(&event(FOLDERS, UiEvent::Activate), request(93))
            .unwrap();
        assert_eq!(
            actor.options.limit_mib, 64,
            "draft does not affect live preferences"
        );
        let saved = actor
            .handle_event(&event(SAVE, UiEvent::Activate), request(94))
            .unwrap();
        let Message::SettingsRequest(SettingsRequest {
            operation: SettingsOperation::Set { key, value },
            ..
        }) = &saved[0]
        else {
            panic!("settings write");
        };
        assert_eq!(key, KEY);
        let setting = SettingEntry {
            key: key.clone(),
            value: value.clone(),
        };
        actor
            .handle_settings_response(&SettingsResponse {
                request: request(94),
                result: SettingsResult::Applied(setting.clone()),
            })
            .unwrap();
        assert!(actor.options_dialog.is_none());
        assert_eq!(actor.options.maximum_bytes(), 2048 * 1024 * 1024);
        assert!(!actor.runtime().unwrap().sort.directories_first);
        let (mut restored, _) = RiceFiles::launch(app(), request(1)).unwrap();
        restored
            .handle_settings_response(&SettingsResponse {
                request: request(1),
                result: SettingsResult::Values(vec![setting]),
            })
            .unwrap();
        assert_eq!(actor.options, restored.options);
        restored
            .handle_event(
                &event(files_menu::PREFERENCES, UiEvent::Activate),
                request(2),
            )
            .unwrap();
        restored
            .handle_event(&event(RESET, UiEvent::Activate), request(3))
            .unwrap();
        restored
            .handle_event(&event(CANCEL, UiEvent::Activate), request(4))
            .unwrap();
        assert_eq!(restored.options.limit_mib, 2048);
    }

    #[test]
    fn icon_sort_direction_and_control_wheel_are_explicit_and_bounded() {
        let (mut actor, _) = loaded_actor();
        actor
            .handle_event(
                &event(files_menu::SORT_DESCENDING, UiEvent::Activate),
                request(80),
            )
            .unwrap();
        assert_eq!(
            actor.runtime().unwrap().sort.direction,
            SortDirection::Descending
        );
        actor
            .handle_event(
                &event(files_menu::SORT_ASCENDING, UiEvent::Activate),
                request(81),
            )
            .unwrap();
        actor
            .handle_event(
                &event(files_menu::SORT_ASCENDING, UiEvent::Activate),
                request(82),
            )
            .unwrap();
        assert_eq!(
            actor.runtime().unwrap().sort.direction,
            SortDirection::Ascending
        );
        let wheel = |button| {
            event(
                files_view::DIRECTORY,
                UiEvent::Pointer {
                    kind: rice_native_app_protocol::PointerEventKind::Wheel,
                    x: 30,
                    y: 40,
                    button,
                    modifiers: 4,
                },
            )
        };
        let old = actor.view_zoom_permille;
        actor.handle_event(&wheel(4), request(83)).unwrap();
        assert_eq!(actor.view_zoom_permille, old + 100);
        actor.handle_event(&wheel(5), request(84)).unwrap();
        assert_eq!(actor.view_zoom_permille, old);
        actor.view_zoom_permille = MAX_VIEW_ZOOM_PERMILLE;
        actor.handle_event(&wheel(4), request(85)).unwrap();
        assert_eq!(actor.view_zoom_permille, MAX_VIEW_ZOOM_PERMILLE);
    }

    #[test]
    fn thumbnail_preferences_gate_requests_and_limit_bytes() {
        let (mut actor, _) = loaded_actor();
        let entries = vec![EntryMetadata {
            kind: EntryKind::File,
            name: "movie.mp4".into(),
            path: GuestPath::absolute("/root/movie.mp4").unwrap(),
            ..actor.runtime().unwrap().entries[0].clone()
        }];
        actor.options.limit_mib = 512;
        let Some(Message::MediaRequest(request)) =
            actor.reset_thumbnail_batch(&entries, request(91), true)
        else {
            panic!("thumbnail");
        };
        assert_eq!(request.maximum_source_bytes, 512 * 1024 * 1024);
        actor.options.thumbnails = false;
        assert!(
            actor
                .reset_thumbnail_batch(&entries, super::super::tests::request(92), true)
                .is_none()
        );
    }
}

#[cfg(test)]
mod translation_tests {
    use super::*;
    #[test]
    fn translated_preferences_keep_slider_protocol_and_saved_values() {
        let options=Options {limit_mib:128,zoom:1500,..Options::default()};
        let saved=serde_json::to_string(&options).unwrap();
        rice_i18n::with_locale(2, || {
            let mut output=Vec::new();patches(&mut output,&options);
            let text=|id| output.iter().find_map(|patch| match patch {
                Patch::Create {widget,..} if widget.id.get()==id => widget.text.as_deref(),
                _=>None,
            });
            assert_eq!(text(350_011),Some("缩略图文件大小上限：128 MiB"));
            assert_eq!(text(350_013),Some("默认图标大小：150%"));
            assert_eq!(text(RESET),Some("恢复默认设置"));
            assert_eq!(text(CANCEL),Some("取消"));
            assert_eq!(text(SAVE),Some("保存"));
            assert_eq!(text(LIMIT),Some("7/511"));
            assert_eq!(text(ZOOM),Some("1000/4500"));
            assert_eq!(serde_json::to_string(&options).unwrap(),saved);
        });
    }
}
