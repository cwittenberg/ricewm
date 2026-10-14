use super::*;

#[test]
fn trash_icon_visibility_is_configurable_at_launch() {
    let (mut desktop, _) = RiceDesktop::launch(app(7), &[String::from("--hide-trash")], request(1))
        .expect("hidden-trash launch");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: Vec::new(),
                next_cursor: None,
            },
        })
        .expect("empty desktop");
    assert_eq!(desktop.items.len(), 3);
    assert_eq!(desktop.model().grid().len(), 3);
}

#[test]
fn hiding_builtin_icons_keeps_desktop_folder_entries() {
    let (mut desktop, _) = RiceDesktop::launch(
        app(7),
        &[String::from("--hide-builtin-icons")],
        request(1),
    )
    .expect("hidden built-ins launch");
    desktop
        .handle_vfs_response(&VfsResponse {
            request: request(1),
            result: VfsResult::Directory {
                entries: vec![directory_entry("notes.txt", FileKind::File)],
                next_cursor: None,
            },
        })
        .expect("desktop folder entry");
    assert_eq!(desktop.items.len(), 1);
    assert_eq!(desktop.items[0].label, "notes.txt");
    assert_eq!(desktop.model().grid().len(), 1);
}

#[test]
fn desktop_settings_emits_appearance_launch_intent() {
    let mut desktop = launch();
    let messages = desktop
        .handle_event(&event(SETTINGS, UiEvent::Activate), request(2))
        .expect("settings event");
    assert!(matches!(
        &messages[0],
        Message::Launch {
            entrypoint,
            arguments,
            ..
        } if entrypoint == "rice-settings"
            && arguments == &[String::from("--section=desktop")]
    ));
    assert_eq!(
        desktop.pending_intents(),
        &[DesktopIntent::LaunchSettings {
            section: "appearance"
        }]
    );
}
