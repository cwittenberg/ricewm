use rice_native_app_protocol::{
    AppId, HostEvent, HostEventKind, HostOperation, HostRequest, Message, TerminalAction,
    TerminalEvent, WindowId,
};
#[test]
fn native_terminal_transport_roundtrips_without_changing_legacy_tags() {
    let app = AppId::new(1).unwrap();
    let window = WindowId::new(1).unwrap();
    for action in [
        TerminalAction::OpenGrid {
            columns: 100,
            rows: 24,
        },
        TerminalAction::OpenXCodex { columns: 80, rows: 24 },
        TerminalAction::OpenGridWithMemory {
            columns: 100, rows: 24, memory_mib: 2048,
        },
        TerminalAction::Scroll(-24),
        TerminalAction::CopyAll,
        TerminalAction::Open {
            columns: 80,
            rows: 24,
        },
    ] {
        let message = Message::HostRequest(HostRequest {
            app,
            window,
            operation: HostOperation::Terminal { session: 7, action },
        });
        assert_eq!(
            Message::decode(&message.encode().unwrap()).unwrap(),
            message
        );
    }
    for event in [
        TerminalEvent::Grid("{}".into()),
        TerminalEvent::GridSize {
            columns: 100,
            rows: 24,
        },
        TerminalEvent::Paste("one\ntwo".into()),
    ] {
        let message = Message::HostEvent(HostEvent {
            app,
            event: HostEventKind::Terminal { session: 7, event },
        });
        assert_eq!(
            Message::decode(&message.encode().unwrap()).unwrap(),
            message
        );
    }
}

#[test]
fn native_terminal_memory_is_bounded() {
    for memory_mib in [0, 383, 2049, u16::MAX] {
        let message = Message::HostRequest(HostRequest {
            app: AppId::new(1).unwrap(), window: WindowId::new(1).unwrap(),
            operation: HostOperation::Terminal { session: 1,
                action: TerminalAction::OpenGridWithMemory { columns: 80, rows: 24, memory_mib } },
        });
        assert!(message.encode().is_err());
    }
}
