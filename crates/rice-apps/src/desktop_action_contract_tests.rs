use super::*;
use rice_native_app_protocol::{ClipboardRequest, UiPatch};
use std::collections::BTreeSet;

fn background() -> RiceDesktop {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("Folder", FileKind::Directory)],
    );
    desktop
}

fn folder_context() -> RiceDesktop {
    let mut desktop = background();
    let folder = desktop
        .items
        .iter()
        .find(|item| item.label == "Folder")
        .expect("folder")
        .id;
    open_item_context(&mut desktop, folder, request(2));
    desktop
}

fn script_context() -> RiceDesktop {
    let mut desktop = launch();
    enumerate(
        &mut desktop,
        request(1),
        vec![directory_entry("change_wallpaper.sh", FileKind::File)],
    );
    let script = desktop
        .items
        .iter()
        .find(|item| item.label == "change_wallpaper.sh")
        .expect("script")
        .id;
    open_item_context(&mut desktop, script, request(2));
    desktop
}

#[test]
fn every_enabled_background_action_has_a_real_actor_effect() {
    let mut menu = background();
    let ids = menu
        .context_menu_entries(false)
        .expect("background actions")
        .into_iter()
        .filter(|entry| entry.label != rice_ui::MENU_SEPARATOR)
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from([NEW_FOLDER, NEW_FILE, SELECT_ALL, ARRANGE, REFRESH, SETTINGS,])
    );

    for id in ids {
        let mut desktop = background();
        let messages = desktop
            .handle_event(&event(id, UiEvent::Activate), request(3))
            .expect("enabled background action");
        match id {
            NEW_FOLDER | NEW_FILE => assert!(matches!(
                &messages[0],
                Message::VfsRequest(VfsRequest {
                    operation: VfsOperation::Stat { .. },
                    ..
                })
            )),
            REFRESH => assert!(matches!(
                &messages[0],
                Message::VfsRequest(VfsRequest {
                    operation: VfsOperation::ReadDirectory { .. },
                    ..
                })
            )),
            SETTINGS => assert!(matches!(
                &messages[0],
                Message::Launch { entrypoint, .. } if entrypoint == "rice-settings"
            )),
            SELECT_ALL | ARRANGE => {
                assert!(matches!(&messages[0], Message::UiPatch(_)));
                if id == SELECT_ALL {
                    assert!(!desktop.model.selection().is_empty());
                }
            }
            _ => unreachable!("exact background action set"),
        }
    }
}

#[test]
fn every_enabled_item_action_has_a_real_actor_effect() {
    let mut menu = folder_context();
    let ids = menu
        .context_menu_entries(true)
        .expect("item actions")
        .into_iter()
        .filter(|entry| entry.label != rice_ui::MENU_SEPARATOR)
        .map(|entry| entry.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        ids,
        BTreeSet::from([OPEN, CUT, COPY, PASTE, LINK, RENAME, DELETE, PROPERTIES])
    );

    for id in ids {
        let mut desktop = folder_context();
        let messages = desktop
            .handle_event(&event(id, UiEvent::Activate), request(3))
            .expect("enabled item action");
        match id {
            OPEN => assert!(messages.iter().any(|message| matches!(
                message,
                Message::Launch {
                    entrypoint,
                    arguments,
                    ..
                } if entrypoint == "rice-files"
                    && arguments == &[String::from("/root/Desktop/Folder")]
            ))),
            CUT | COPY => assert!(matches!(
                &messages[0],
                Message::ClipboardRequest(ClipboardRequest::Write { .. })
            )),
            PASTE => assert!(matches!(
                &messages[0],
                Message::ClipboardRequest(ClipboardRequest::Read { .. })
            )),
            LINK => assert!(
                messages.iter().any(|message| matches!(
                    message,
                    Message::UiPatch(envelope) if envelope.patches.iter().any(|patch| matches!(
                        patch,
                        UiPatch::Create { kind, text: Some(label), .. }
                            if *kind == WidgetKind::Button as u16 && label == "Find Target…"
                    ))
                )),
                "Create Shortcut must expose the real shared target chooser",
            ),
            DELETE => assert!(matches!(
                &messages[0],
                Message::VfsRequest(VfsRequest {
                    operation: VfsOperation::ReadDirectory { path, .. },
                    ..
                }) if path == desktop_vfs::TRASH_FILES_PATH
            )),
            RENAME => assert!(matches!(&messages[0], Message::UiPatch(_))),
            PROPERTIES => {
                let Message::UiPatch(envelope) = &messages[0] else {
                    panic!("properties opens a retained dialog")
                };
                assert!(envelope.patches.iter().any(|patch| matches!(
                    patch,
                    UiPatch::Create {
                        kind,
                        text: Some(title),
                        ..
                    } if *kind == WidgetKind::Dialog as u16 && title == "Properties"
                )));
                assert!(matches!(
                    &messages[1],
                    Message::VfsRequest(VfsRequest {
                        operation: VfsOperation::Stat { path },
                        ..
                    }) if path == "/root/Desktop/Folder"
                ));
            }
            _ => unreachable!("exact item action set"),
        }
    }
}

#[test]
fn properties_reads_metadata_and_closes_without_desktop_chrome() {
    let mut desktop = folder_context();
    let opened = desktop
        .handle_event(&event(PROPERTIES, UiEvent::Activate), request(8))
        .expect("open properties");
    assert_eq!(opened.len(), 2);

    let updated = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(8),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::Directory,
                size: 4_096,
                modified_millis: 1_704_067_201_234,
                mode: 0o755,
            }),
        })
        .expect("metadata response");
    let Message::UiPatch(envelope) = &updated[0] else {
        panic!("metadata redraws properties")
    };
    let visible_text = envelope
        .patches
        .iter()
        .filter_map(|patch| match patch {
            UiPatch::Create { text, .. } => text.as_deref(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(visible_text.contains(&"4.0 KiB (4096 bytes)"));
    assert!(visible_text.contains(&"1/1/2024, 00:00:01"));
    assert!(visible_text.contains(&"0755"));

    let closed = desktop
        .handle_event(&event(PROPERTIES_CLOSE, UiEvent::Activate), request(9))
        .expect("close properties");
    let Message::UiPatch(envelope) = &closed[0] else {
        panic!("close removes the modal")
    };
    assert!(envelope.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Remove { id } if *id == widget_id(PROPERTIES_DIALOG)
    )));
    assert!(desktop.tree.get(widget_id(PROPERTIES_DIALOG)).is_none());
}

#[test]
fn alt_return_routes_the_context_selected_item_to_properties() {
    let mut desktop = folder_context();
    let opened = desktop
        .handle_event(
            &event(
                ICON_GRID,
                UiEvent::Key {
                    pressed: true,
                    key_code: 36,
                    modifiers: 8,
                    text: None,
                },
            ),
            request(10),
        )
        .expect("Alt+Return properties");
    assert!(matches!(
        opened.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::Stat { path },
            ..
        })] if path == "/root/Desktop/Folder"
    ));
}

#[test]
fn script_properties_offer_access_presets_executable_toggle_and_location_copy() {
    let mut desktop = script_context();
    desktop
        .handle_event(&event(PROPERTIES, UiEvent::Activate), request(20))
        .expect("open script properties");
    let updated = desktop
        .handle_vfs_response(&VfsResponse {
            request: request(20),
            result: VfsResult::Metadata(FileMetadata {
                kind: FileKind::File,
                size: 925,
                modified_millis: 1_704_067_201_000,
                mode: 0o644,
            }),
        })
        .expect("script metadata");
    let Message::UiPatch(envelope) = &updated[0] else {
        panic!("metadata redraws properties")
    };
    assert!(envelope.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, kind, state, .. }
            if *id == widget_id(desktop_properties::EXECUTABLE)
                && *kind == WidgetKind::Toggle as u16
                && state & WidgetState::CHECKED.bits() == 0
    )));
    assert!(envelope.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(desktop_properties::ACCESS_MENU)
                && text == "Read and write"
    )));

    let access = desktop
        .handle_event(
            &event(desktop_properties::ACCESS_READ_ONLY, UiEvent::Activate),
            request(21),
        )
        .expect("choose read only");
    let Message::UiPatch(access) = &access[0] else {
        panic!("access preset redraws properties")
    };
    assert!(access.patches.iter().any(|patch| matches!(
        patch,
        UiPatch::Create { id, text: Some(text), .. }
            if *id == widget_id(desktop_properties::MODE_FIELD) && text == "0444"
    )));

    let executable = desktop
        .handle_event(
            &event(desktop_properties::EXECUTABLE, UiEvent::Activate),
            request(22),
        )
        .expect("make executable");
    assert!(matches!(
        executable.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(VfsRequest {
            operation: VfsOperation::SetMode { path, mode },
            ..
        })] if path == "/root/Desktop/change_wallpaper.sh" && *mode == 0o555
    ));

    let mut clipboard_desktop = script_context();
    clipboard_desktop
        .handle_event(&event(PROPERTIES, UiEvent::Activate), request(30))
        .expect("open properties for copy");
    let copied = clipboard_desktop
        .handle_event(
            &event(desktop_properties::COPY_LOCATION, UiEvent::Activate),
            request(31),
        )
        .expect("copy location");
    assert!(matches!(
        copied.as_slice(),
        [Message::ClipboardRequest(ClipboardRequest::Write {
            mime_type,
            data,
            ..
        })] if mime_type == "text/plain;charset=utf-8"
            && data == b"/root/Desktop/change_wallpaper.sh"
    ));
}


#[test]
fn language_refresh_updates_open_menu_without_rebuilding_actions_or_changing_item_names() {
    let mut desktop = rice_i18n::with_locale(rice_i18n::locale_code("en"), folder_context);
    let ids = desktop.context_action_ids.clone();
    let items = desktop.items.clone();
    let selection = desktop.model.selection().clone();
    for locale in ["en", "es", "zh-Hans", "id", "ja", "fr", "pt", "ko", "sw", "ar", "de",
        "hi", "it", "fa", "bn", "tr", "ur", "mr", "th", "vi", "ru", "te", "ta"] {
        rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
            let messages = desktop.refresh_language().unwrap();
            let Message::UiPatch(envelope) = &messages[0] else { panic!("UI refresh"); };
            assert_eq!(desktop.context_action_ids, ids);
            assert_eq!(desktop.context_kind, ContextKind::Item);
            assert_eq!(desktop.items, items);
            assert_eq!(desktop.model.selection(), &selection);
            for &(id, message) in &desktop.context_messages {
                assert!(envelope.patches.iter().any(|patch| matches!(patch,
                    UiPatch::Update { id: target, text: rice_native_app_protocol::PatchValue::Set(text), state: rice_native_app_protocol::PatchValue::Keep, .. }
                    if *target == widget_id(id) && text == rice_i18n::t(message)
                )), "{locale}: {message}");
            }
            assert!(desktop.context_messages.contains(&(PASTE, "desktop.paste-into")));
            assert!(!envelope.patches.iter().any(|patch| matches!(patch,
                UiPatch::Remove { id } if ids.contains(&id.get()) || *id == widget_id(MENU_BAR)
            )));
        });
    }
}

#[test]
fn language_refresh_keeps_rename_draft_literal() {
    let mut desktop = folder_context();
    desktop.rename_text = "Paste Into {path} 私の文書".into();
    desktop.open_rename_editor().unwrap();
    let source = desktop.rename_source_path.clone();
    rice_i18n::with_locale(rice_i18n::locale_code("zh-Hans"), || {
        let messages = desktop.refresh_language().unwrap();
        assert_eq!(desktop.rename_text, "Paste Into {path} 私の文書");
        assert_eq!(desktop.rename_source_path, source);
        assert_eq!(desktop.context_kind, ContextKind::Rename);
        let Message::UiPatch(envelope) = &messages[0] else { panic!("UI refresh"); };
        assert!(!envelope.patches.iter().any(|patch| matches!(patch,
            UiPatch::Update { id, .. } if *id == widget_id(RENAME_FIELD)
        )));
        assert!(envelope.patches.iter().any(|patch| matches!(patch,
            UiPatch::Update { id, text: rice_native_app_protocol::PatchValue::Set(text), .. }
                if *id == widget_id(RENAME_DIALOG) && text == rice_i18n::t("Rename Desktop Item")
        )));
    });
}

#[test]
fn unsupported_desktop_application_notice_keeps_path_a_literal_template_argument() {
    let path = "/root/Desktop/{path} 私の文書.unknown";
    rice_i18n::with_locale(rice_i18n::locale_code("zh-Hans"), || {
        let mut desktop = background();
        let item = desktop.items.iter().find(|item| item.label == "Folder").unwrap().id;
        let model_item = desktop.model.item_mut(item).unwrap();
        model_item.path = None;
        model_item.label = path.into();
        let messages = desktop.activate_item(item, request(90)).unwrap();
        assert!(messages.iter().any(|message| match message {
            Message::UiPatch(envelope) => envelope.patches.iter().any(|patch| matches!(patch,
                UiPatch::Create { text: Some(text), .. }
                    if text.contains(path) && !text.starts_with("No installed")
            )),
            _ => false,
        }));
    });
}


#[test]
fn visible_unsupported_notice_refreshes_all_locales_without_reinterpreting_literal_detail() {
    let path = "/root/Desktop/{path} 私の文書.unknown";
    let mut desktop = background();
    let item = desktop.items.iter().find(|item| item.label == "Folder").unwrap().id;
    let model_item = desktop.model.item_mut(item).unwrap();
    model_item.path = None;
    model_item.label = path.into();
    desktop.activate_item(item, request(91)).unwrap();
    for locale in ["en", "es", "zh-Hans", "id", "ja", "fr", "pt", "ko", "sw", "ar", "de",
        "hi", "it", "fa", "bn", "tr", "ur", "mr", "th", "vi", "ru", "te", "ta"] {
        rice_i18n::with_locale(rice_i18n::locale_code(locale), || {
            let messages = desktop.refresh_language().unwrap();
            let Message::UiPatch(envelope) = &messages[0] else { panic!("UI refresh"); };
            let expected = rice_i18n::format("desktop.unsupported-application", &[("path", path)]);
            assert!(expected.contains(path));
            assert_eq!(desktop.retained_notice.as_ref().unwrap().render(), expected);
            assert!(envelope.patches.iter().any(|patch| matches!(patch,
                UiPatch::Update { id, text: rice_native_app_protocol::PatchValue::Set(text), state: rice_native_app_protocol::PatchValue::Keep, .. }
                    if *id == widget_id(NOTICE) && text == &expected
            )), "{locale}");
            assert!(!envelope.patches.iter().any(|patch| matches!(patch,
                UiPatch::Remove { id } | UiPatch::Create { id, .. } if *id == widget_id(NOTICE)
            )));
        });
    }
    // A literal notice identical to an English owned phrase remains provider data.
    let raw = "No installed RiceWM application handles {path}";
    desktop.notice(raw).unwrap();
    assert_eq!(desktop.retained_notice, Some(DesktopNotice::literal(raw)));
    rice_i18n::with_locale(rice_i18n::locale_code("zh-Hans"), || {
        let messages = desktop.refresh_language().unwrap();
        let Message::UiPatch(envelope) = &messages[0] else { panic!("UI refresh"); };
        assert!(!envelope.patches.iter().any(|patch| matches!(patch,
            UiPatch::Update { id, .. } if *id == widget_id(NOTICE)
        )));
    });
}
