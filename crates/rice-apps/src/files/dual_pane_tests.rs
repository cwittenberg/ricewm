use super::{
    BACK, ENTRY_BASE, Message, RiceFiles, SPLIT_TOGGLE, UiEvent, VIEW_DETAILS,
    files_view::{SECONDARY_DIRECTORY, SECONDARY_ENTRY_BASE, widget_id},
    tests::{app, directory_response, event, loaded_actor, request},
};
use rice_files::{PaneSlot, SortKey, ViewMode};
use rice_native_app_protocol::{
    DirectoryEntry, FileKind, FileMetadata, PointerEventKind, UiPatch, VfsResponse, VfsResult,
};
use rice_ui::WidgetKind;

fn response(request_id: u64, name: &str) -> VfsResponse {
    VfsResponse {
        request: request(request_id),
        result: VfsResult::Directory {
            entries: vec![DirectoryEntry {
                name: name.into(),
                metadata: FileMetadata {
                    kind: FileKind::File,
                    size: request_id,
                    modified_millis: i64::try_from(request_id).unwrap_or_default(),
                    mode: 0o644,
                },
                mime_type: None,
            }],
            next_cursor: None,
        },
    }
}

#[test]
fn dual_panes_enumerate_render_and_select_without_state_leakage() {
    let (mut actor, _) = loaded_actor();
    let opened = actor
        .handle_event(&event(SPLIT_TOGGLE, UiEvent::Activate), request(20))
        .expect("open secondary");
    assert!(matches!(
        opened.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    actor
        .handle_vfs_response(&response(20, "secondary.txt"))
        .expect("secondary enumeration");

    let tab_id = actor.active_tab_id().expect("tab");
    assert_eq!(
        actor
            .runtime_at(tab_id, PaneSlot::Primary)
            .expect("primary")
            .entries
            .len(),
        3
    );
    assert_eq!(
        actor
            .runtime_at(tab_id, PaneSlot::Secondary)
            .expect("secondary")
            .entries[0]
            .name,
        "secondary.txt"
    );
    assert_eq!(actor.active_slot().expect("slot"), PaneSlot::Secondary);

    let patch = actor.emit_full().expect("render both panes");
    let Message::UiPatch(patch) = patch else {
        panic!("full patch");
    };
    assert!(patch.patches.iter().any(|item| {
        matches!(
            item,
            UiPatch::Create { id, kind, .. }
                if *id == widget_id(SECONDARY_DIRECTORY)
                    && *kind == WidgetKind::IconGrid as u16
        )
    }));
    assert!(patch.patches.iter().any(|item| {
        matches!(
            item,
            UiPatch::Create { id, text, .. }
                if *id == widget_id(SECONDARY_ENTRY_BASE)
                    && text.as_deref() == Some("secondary.txt")
        )
    }));

    actor
        .handle_event(&event(ENTRY_BASE, UiEvent::Activate), request(21))
        .expect("focus primary entry");
    assert_eq!(actor.active_slot().expect("slot"), PaneSlot::Primary);
    assert_eq!(actor.pane().expect("primary").selection().count(), 1);
    assert_eq!(actor.tab().expect("tab").secondary.selection().count(), 0);

    actor
        .handle_event(&event(SECONDARY_ENTRY_BASE, UiEvent::Activate), request(22))
        .expect("focus secondary entry");
    assert_eq!(actor.active_slot().expect("slot"), PaneSlot::Secondary);
    assert_eq!(actor.pane().expect("secondary").selection().count(), 1);
    assert_eq!(actor.tab().expect("tab").primary.selection().count(), 1);
}

#[test]
fn rice_bus_subscription_tracks_both_visible_panes() {
    let (mut actor, _) = loaded_actor();
    let mut messages = actor
        .handle_event(&event(SPLIT_TOGGLE, UiEvent::Activate), request(40))
        .expect("open secondary");
    actor
        .sync_rice_bus_subscription(&mut messages)
        .expect("sync RiceBus subscription");
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::HostRequest(rice_native_app_protocol::HostRequest {
            operation: rice_native_app_protocol::HostOperation::RiceBusSubscribe {
                topic,
                filters,
            },
            ..
        }) if topic == super::VFS_CHANGED_TOPIC
            && filters == &[String::from("/root"), String::from("/root")]
    )));
}

#[test]
fn active_pane_navigation_view_scroll_and_close_are_independent() {
    let (mut actor, _) = loaded_actor();
    actor
        .handle_event(&event(SPLIT_TOGGLE, UiEvent::Activate), request(30))
        .expect("open secondary");
    actor
        .handle_vfs_response(&response(30, "secondary.txt"))
        .expect("secondary enumeration");

    actor
        .begin_navigation(
            rice_files::NavigationCommand::Enter(String::from("/root/Documents")),
            request(31),
        )
        .expect("secondary navigation");
    actor
        .handle_vfs_response(&response(31, "document.txt"))
        .expect("secondary directory");
    actor
        .handle_event(&event(VIEW_DETAILS, UiEvent::Activate), request(32))
        .expect("secondary detail view");
    actor.change_sort(SortKey::Size).expect("secondary sort");
    actor
        .handle_event(
            &event(
                SECONDARY_ENTRY_BASE,
                UiEvent::Pointer {
                    kind: PointerEventKind::Wheel,
                    x: 0,
                    y: 0,
                    button: 5,
                    modifiers: 0,
                },
            ),
            request(33),
        )
        .expect("secondary scroll");

    let tab = actor.tab().expect("tab");
    assert_eq!(tab.primary.location().as_str(), "/root");
    assert_eq!(tab.primary.view, ViewMode::Icons);
    assert_eq!(tab.primary.scroll.vertical, 0);
    assert_eq!(tab.secondary.location().as_str(), "/root/Documents");
    assert_eq!(tab.secondary.view, ViewMode::Details);
    assert!(tab.secondary.scroll.vertical > 0);
    let tab_id = tab.id;
    assert_eq!(
        actor
            .runtime_at(tab_id, PaneSlot::Primary)
            .expect("primary")
            .sort
            .key,
        SortKey::Name
    );
    assert_eq!(
        actor
            .runtime_at(tab_id, PaneSlot::Secondary)
            .expect("secondary")
            .sort
            .key,
        SortKey::Size
    );

    let back = actor
        .handle_event(&event(BACK, UiEvent::Activate), request(34))
        .expect("secondary back");
    assert!(matches!(
        back.as_slice(),
        [Message::UiPatch(_), Message::VfsRequest(_)]
    ));
    actor
        .handle_vfs_response(&directory_response(34))
        .expect("secondary back response");
    assert_eq!(
        actor.tab().expect("tab").primary.location().as_str(),
        "/root"
    );

    actor
        .handle_event(&event(SPLIT_TOGGLE, UiEvent::Activate), request(35))
        .expect("close secondary");
    assert!(!actor.tab().expect("tab").dual_pane);
    assert_eq!(actor.active_slot().expect("slot"), PaneSlot::Primary);
    assert_eq!(actor.runtime_for_test().entries.len(), 3);
}

#[test]
fn launching_actor_identity_is_unchanged_by_pane_focus() {
    let (actor, _) = RiceFiles::launch(app(), request(1)).expect("launch");
    assert_eq!(actor.active_slot().expect("slot"), PaneSlot::Primary);
}
