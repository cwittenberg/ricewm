use super::{
    ENTRY_BASE, Message, UiEvent,
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    DragAction, DragSourceEvent, DragSourceResult, DragSourceState, PointerEventKind,
};

#[test]
fn selected_files_emit_typed_source_offers_and_consume_host_results() {
    for (modifiers, expected) in [
        (0, DragAction::Move),
        (1, DragAction::Copy),
        (4, DragAction::Copy),
    ] {
        let (mut actor, _) = loaded_actor();
        let offered = actor
            .handle_event(
                &event(
                    ENTRY_BASE + 1,
                    UiEvent::Pointer {
                        kind: PointerEventKind::Down,
                        x: 12,
                        y: 12,
                        button: 1,
                        modifiers,
                    },
                ),
                request(22),
            )
            .expect("source offer");
        let Message::DragSource(source) = offered
            .iter()
            .find(|message| matches!(message, Message::DragSource(_)))
            .expect("typed drag source offer")
        else {
            unreachable!()
        };
        assert!(matches!(
            &source.state,
            DragSourceState::Offer {
                action,
                mime_types,
                payload,
            } if *action == expected
                && mime_types == &[
                    String::from("text/uri-list"),
                    String::from("x-special/gnome-copied-files"),
                ]
                && payload.mime_type == "text/uri-list"
                && payload.data == b"file:///root/file2.txt\r\n"
        ));
        let finished = actor
            .finish_source_drag(&DragSourceEvent {
                drag: source.drag,
                app: source.app,
                window: source.window,
                state: DragSourceState::Finished(DragSourceResult::Completed(expected)),
            })
            .expect("host result");
        if expected == DragAction::Move {
            assert!(matches!(
                finished.as_slice(),
                [Message::UiPatch(_), Message::VfsRequest(_)]
            ));
        } else {
            assert!(matches!(
                finished.as_slice(),
                [Message::UiPatch(_), Message::HostRequest(_)]
            ));
        }
        assert!(actor.source_drag.is_none());
        assert!(actor.drag_sources.is_none());
    }
}
