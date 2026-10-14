use super::{DecodeError, EncodeError, HEADER_BYTES, Kind, MAGIC};
use crate::*;

fn id<T: super::primitives::ProtocolId>(value: u64) -> T {
    T::new(value).expect("test ID must be nonzero")
}

fn roundtrip(message: Message) {
    let encoded = message.encode().expect("message should encode");
    assert_eq!(Message::decode(&encoded), Ok(message));
}

#[test]
fn native_message_limit_can_frame_the_reported_zoomit_archive() {
    const REPORTED_ZOOMIT_BYTES: usize = 25_335_044;
    assert!(MAX_HTTP_BODY_BYTES >= REPORTED_ZOOMIT_BYTES);
    assert!(MAX_MESSAGE_BYTES > MAX_HTTP_BODY_BYTES);
}

#[test]
fn roundtrips_control_and_ui_messages() {
    roundtrip(Message::HostHello {
        capabilities: HostCapabilities::CLIPBOARD
            | HostCapabilities::DRAG_AND_DROP
            | HostCapabilities::FILE_WATCH,
        max_message_bytes: u32::try_from(MAX_MESSAGE_BYTES).expect("limit fits in u32"),
    });
    roundtrip(Message::Launch {
        app: id(4),
        entrypoint: "ricepaper".into(),
        arguments: vec!["--line=12".into(), "/home/rice/notes.txt".into()],
    });
    roundtrip(Message::Open {
        app: id(4),
        paths: vec!["/tmp/one".into(), "/tmp/two".into()],
    });
    roundtrip(Message::Close {
        app: id(4),
        reason: CloseReason::LastWindowClosed,
    });
    roundtrip(Message::CloseRequest(CloseRequest {
        request: id(8),
        app: id(4),
        reason: CloseReason::HostShutdown,
    }));
    roundtrip(Message::CloseResponse(CloseResponse {
        request: id(8),
        app: id(4),
        decision: CloseDecision::Pending,
    }));
    roundtrip(Message::UiPatch(UiPatchEnvelope {
        app: id(4),
        window: id(5),
        sequence: 9,
        patches: vec![
            UiPatch::Create {
                id: id(10),
                parent: None,
                index: 0,
                kind: 3,
                state: 1,
                text: Some("Save".into()),
                icon: Some(12),
            },
            UiPatch::Move {
                id: id(10),
                parent: Some(id(11)),
                index: 2,
            },
            UiPatch::Clear,
        ],
    }));
    roundtrip(Message::UiEvent(UiEventEnvelope {
        app: id(4),
        window: id(5),
        target: id(10),
        event: UiEvent::Key {
            pressed: true,
            key_code: 0x53,
            modifiers: 2,
            text: Some("S".into()),
        },
    }));
    roundtrip(Message::UiEvent(UiEventEnvelope {
        app: id(4),
        window: id(5),
        target: id(10),
        event: UiEvent::TextSelection {
            anchor: 12,
            caret: 37,
        },
    }));
    let mut solid = vec![5, 0, 7, 0, 0, 0, 0, 0];
    for value in [0.0_f32, 0.0, 320.0, 200.0] {
        solid.extend_from_slice(&value.to_le_bytes());
    }
    solid.extend_from_slice(&0xff22_4466_u32.to_le_bytes());
    roundtrip(Message::WebRenderPatch(WebRenderPatch {
        app: id(4),
        window: id(5),
        surface: id(10_001),
        epoch: 9,
        viewport_width: 320,
        viewport_height: 200,
        display_list: solid,
    }));
}

#[test]
fn roundtrips_vfs_clipboard_and_drag_messages() {
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(20),
        app: id(4),
        operation: VfsOperation::Write {
            path: "/home/rice/file.txt".into(),
            offset: 42,
            data: vec![1, 2, 3],
        },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(23),
        app: id(4),
        operation: VfsOperation::Copy {
            from: "/home/rice/source".into(),
            to: "/home/rice/copy".into(),
            recursive: true,
        },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(24),
        app: id(4),
        operation: VfsOperation::SetMode {
            path: "/home/rice/file.txt".into(),
            mode: 0o640,
        },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(28),
        app: id(4),
        operation: VfsOperation::ReadLink {
            path: "/home/rice/shortcut".into(),
        },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(29),
        app: id(4),
        operation: VfsOperation::ReplaceLink {
            path: "/home/rice/shortcut".into(),
            target: "/home/rice/new-target".into(),
        },
    }));
    for expected in [None,Some(vec![]),Some(vec![0,255,12])] {
        roundtrip(Message::VfsRequest(VfsRequest {
            request:id(30),app:id(4),operation:VfsOperation::CompareRename {
                from:"/home/rice/temp".into(),to:"/home/rice/saved".into(),expected,
            },
        }));
    }
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(25),
        app: id(4),
        operation: VfsOperation::Cancel { target: id(23) },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(26),
        app: id(4),
        operation: VfsOperation::Archive {
            sources: vec!["/home/rice/a".into(), "/home/rice/b".into()],
            destination: "/home/rice/Archive.zip".into(),
            format: ArchiveFormat::Zip,
        },
    }));
    roundtrip(Message::VfsRequest(VfsRequest {
        request: id(27),
        app: id(4),
        operation: VfsOperation::Extract {
            archive: "/home/rice/Archive.zip".into(),
            destination: "/home/rice".into(),
        },
    }));
    roundtrip(Message::VfsResponse(VfsResponse {
        request: id(20),
        result: VfsResult::Directory {
            entries: vec![DirectoryEntry {
                name: "file.txt".into(),
                metadata: FileMetadata {
                    kind: FileKind::File,
                    size: 3,
                    modified_millis: 1_234,
                    mode: 0o644,
                },
                mime_type: None,
            }],
            next_cursor: Some(7),
        },
    }));
    roundtrip(Message::VfsResponse(VfsResponse {
        request: id(23),
        result: VfsResult::Progress(VfsProgress {
            sequence: 2,
            completed_bytes: 64,
            total_bytes: 128,
        }),
    }));
    roundtrip(Message::VfsResponse(VfsResponse {
        request: id(23),
        result: VfsResult::Cancelled,
    }));
    roundtrip(Message::ClipboardRequest(ClipboardRequest::Write {
        request: id(21),
        selection: ClipboardSelection::Clipboard,
        mime_type: "text/plain;charset=utf-8".into(),
        data: b"hello".to_vec(),
    }));
    roundtrip(Message::ClipboardResponse(ClipboardResponse {
        request: id(21),
        result: ClipboardResult::Empty,
    }));
}

#[test]
fn roundtrips_drag_messages() {
    roundtrip(Message::Drag(DragEnvelope {
        drag: id(30),
        app: id(4),
        window: id(5),
        target: Some(id(10)),
        phase: DragPhase::Motion,
        action: DragAction::Copy,
        x: -20,
        y: 41,
        mime_types: vec!["text/uri-list".into()],
        payload: Some(DragPayload {
            mime_type: String::from("text/uri-list"),
            data: b"file:///root/a.txt\r\n".to_vec(),
        }),
    }));
    roundtrip(Message::DragSource(DragSourceEvent {
        drag: id(31),
        app: id(7),
        window: id(8),
        state: DragSourceState::Offer {
            action: DragAction::Ask,
            mime_types: vec![
                String::from("text/uri-list"),
                String::from("x-special/gnome-copied-files"),
            ],
            payload: DragPayload {
                mime_type: String::from("text/uri-list"),
                data: b"file:///root/report.txt\r\n".to_vec(),
            },
        },
    }));
    roundtrip(Message::DragSource(DragSourceEvent {
        drag: id(31),
        app: id(7),
        window: id(8),
        state: DragSourceState::Finished(DragSourceResult::Completed(DragAction::Move)),
    }));
}

#[test]
fn roundtrips_bounded_media_decode_messages() {
    roundtrip(Message::MediaRequest(MediaRequest {
        request: id(24),
        app: id(4),
        path: "/root/Pictures/photo.webp".into(),
        mime_type: "image/webp".into(),
        maximum_source_bytes: 8 * 1024 * 1024,
        target_width: 640,
        target_height: 480,
        frame_millis: 0,
    }));
    roundtrip(Message::MediaResponse(MediaResponse {
        request: id(24),
        result: MediaResult::Ready(MediaMetadata {
            kind: MediaKind::Image,
            width: 640,
            height: 426,
            duration_millis: None,
            mime_type: "image/webp".into(),
        }),
    }));
    assert!(matches!(
        Message::MediaRequest(MediaRequest {
            request: id(24),
            app: id(4),
            path: "/root/Pictures/huge.png".into(),
            mime_type: "image/png".into(),
            maximum_source_bytes: MAX_MEDIA_SOURCE_BYTES + 1,
            target_width: 640,
            target_height: 480,
            frame_millis: 0,
        })
        .encode(),
        Err(EncodeError::InvalidValue("media source byte limit"))
    ));
}

#[test]
fn roundtrips_host_window_state_acknowledgements() {
    roundtrip(Message::HostEvent(HostEvent {
        app: id(4),
        event: HostEventKind::WindowState {
            window: id(1),
            content_width: 1_280,
            content_height: 692,
            fullscreen: true,
            maximized: true,
            focused: true,
            visible: true,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::WebSocket {
            tab: 7,
            realm: 70,
            handle: 3,
            action: 2,
            metadata: String::from(r#"{"text":true}"#),
            data: b"hello".to_vec(),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::WebSocket {
            tab: 7,
            realm: 70,
            handle: 3,
            event: String::from("message"),
            metadata: String::from(r#"{"text":true}"#),
            data: b"world".to_vec(),
        },
    }));
}

#[test]
#[allow(clippy::too_many_lines)]
fn roundtrips_entropy_and_currency_provider_messages() {
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(31),
        app: id(7),
        operation: DataProviderOperation::Entropy { byte_len: 32 },
    }));
    for url in ["smb://server/share", "nfs://server/export"] {
        roundtrip(Message::DataProviderRequest(DataProviderRequest {
            request: id(41), app: id(7),
            operation: DataProviderOperation::MountConnectNetwork {
                url: url.to_owned(), username: "rice".to_owned(), password: "secret".to_owned(),
                domain: "WORKGROUP".to_owned(), bridge_url: "wss://bridge.test/network-tcp".to_owned(),
                bridge_token: "pairing-token".to_owned(), uid: 1000, gid: 1000,
            },
        }));
    }
    for operation in [DataProviderOperation::MountDescribeNetwork {mount_id:"smb-test".to_owned()}, DataProviderOperation::MountForgetNetwork {mount_id:"smb-test".to_owned()}] {
        roundtrip(Message::DataProviderRequest(DataProviderRequest {request:id(42),app:id(7),operation}));
    }
    roundtrip(Message::DataProviderResponse(DataProviderResponse {request:id(42),result:DataProviderResult::NetworkConnectionInfo {json:"{}".to_owned()}}));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(40),
        app: id(7),
        operation: DataProviderOperation::HttpRequest {
            url: String::from("https://example.test/choice"),
            method: String::from("POST"),
            content_type: String::from("application/json"),
            cookie: String::from("session=ready"),
            body: br#"{"accepted":true}"#.to_vec(),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(41),
        app: id(7),
        operation: DataProviderOperation::HttpRequestV3 {
            url: String::from("https://accounts.example/identifier"),
            method: String::from("POST"),
            headers: vec![crate::HttpHeader {
                name: String::from("x-same-domain"),
                value: String::from("1"),
            }],
            cookie: String::from("__Host-session=guest-only"),
            body: b"f.req=identity".to_vec(),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(42),
        app: id(7),
        operation: DataProviderOperation::Authorization {
            application_id: String::from("rice-paddy"),
            provider_id: String::from("matrix"),
            entry_url: String::from(
                "https://matrix.org/_matrix/client/v3/login/sso/redirect?redirectUrl=callback",
            ),
            callback_mode: String::from("https"),
            callback_url: String::from("https://matrix.org/_rice/paddy/sso/callback"),
            interaction_kind: String::from("follow-link-once"),
            interaction_value: String::from("Continue with Google"),
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(42),
        result: DataProviderResult::Authorization {
            callback_url: String::from(
                "https://matrix.org/_rice/paddy/sso/callback?state=nonce&loginToken=once",
            ),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(32),
        app: id(7),
        operation: DataProviderOperation::CurrencyRate {
            base_iso4217: String::from("EUR"),
            quote_iso4217: String::from("USD"),
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(39),
        result: DataProviderResult::MountConnected(MountEntry {
            icon: Some(159),
            id: String::from("webdav-example"),
            label: String::from("Example"),
            path: String::from("/run/ricewm/mounts/webdav-example"),
            state: MountState::Connected,
            capabilities: MountCapabilities::OPEN
                | MountCapabilities::SYNC
                | MountCapabilities::DISCONNECT
                | MountCapabilities::RECONNECT,
        }),
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(31),
        result: DataProviderResult::Entropy(vec![7; 32]),
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(32),
        result: DataProviderResult::CurrencyRate {
            decimal_rate: String::from("1.125"),
            observed_millis: 1_722_000_000_000,
            source: String::from("user-configured"),
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(33),
        result: DataProviderResult::Unavailable {
            reason: String::from("No currency provider configured"),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(34),
        app: id(7),
        operation: DataProviderOperation::MountList,
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(35),
        app: id(7),
        operation: DataProviderOperation::MountSync {
            mount_id: String::from("local-documents"),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(39),
        app: id(7),
        operation: DataProviderOperation::MountConnectWebDav {
            url: String::from("https://dav.example.test/files/"),
            username: String::from("ada"),
            password: String::from("secret"),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(40),
        app: id(7),
        operation: DataProviderOperation::MountCancelConnect {
            connect_request: id(39),
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(34),
        result: DataProviderResult::Mounts {
            entries: vec![MountEntry {
                icon: None,
                id: String::from("local-documents"),
                label: String::from("Documents"),
                path: String::from("/mnt/local/Documents"),
                state: MountState::Connected,
                capabilities: MountCapabilities::OPEN
                    | MountCapabilities::SYNC
                    | MountCapabilities::DISCONNECT,
            }],
            can_connect_local: true,
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(36),
        app: id(7),
        operation: DataProviderOperation::HttpGet {
            url: String::from("https://example.test/page"),
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(36),
        result: DataProviderResult::HttpResponse {
            final_url: String::from("https://example.test/page"),
            status: 200,
            content_type: String::from("text/html; charset=utf-8"),
            cookie: String::from("session=ready"),
            origin_cookies: vec![crate::OriginCookie {
                origin: String::from("https://redirect.example.test/start"),
                cookie: String::from("authId=redirect-ready; Path=/; Secure"),
            }],
            redirect_trace: String::from(
                "hop=0 status=302 origin=https://redirect.example.test set_cookie_count=1",
            ),
            body: b"<!doctype html><title>Example</title>".to_vec(),
        },
    }));
}

#[test]
fn roundtrips_bounded_pdf_document_provider_messages() {
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(80),
        app: id(7),
        operation: DataProviderOperation::DocumentInfo {
            path: String::from("/Documents/manual.pdf"),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(81),
        app: id(7),
        operation: DataProviderOperation::DocumentSearch {
            path: String::from("/Documents/manual.pdf"),
            query: String::from("installation"),
            maximum_results: 64,
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(80),
        result: DataProviderResult::DocumentInfo {
            page_count: 42,
            outline: vec![DocumentOutlineEntry {
                title: String::from("Getting started"),
                page: 3,
                depth: 0,
            }],
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(81),
        result: DataProviderResult::DocumentSearch {
            query: String::from("installation"),
            hits: vec![DocumentSearchHit {
                page: 7,
                snippet: String::from("Installation takes only a moment"),
            }],
            truncated: false,
        },
    }));
}

#[test]
fn roundtrips_application_provider_messages() {
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(37),
        app: id(2),
        operation: DataProviderOperation::ApplicationCatalog {
            mime_type: String::from("image/png"),
        },
    }));
    roundtrip(Message::DataProviderRequest(DataProviderRequest {
        request: id(38),
        app: id(2),
        operation: DataProviderOperation::ApplicationLaunch {
            application_id: String::from("org.gnome.eog.desktop"),
            mime_type: String::from("image/png"),
            paths: vec![String::from("/root/Pictures/photo.png")],
            remember_default: true,
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(37),
        result: DataProviderResult::Applications {
            schema_version: APPLICATION_CATALOG_VERSION,
            mime_type: String::from("image/png"),
            entries: vec![InstalledApplication {
                id: String::from("native:rice-view"),
                name: String::from("RiceView"),
                icon: String::from("rice-view"),
                mime_types: vec![String::from("image/*")],
                matches_requested_type: true,
                is_default: true,
                is_native: true,
            }],
        },
    }));
    roundtrip(Message::DataProviderResponse(DataProviderResponse {
        request: id(38),
        result: DataProviderResult::ApplicationLaunched {
            application_id: String::from("org.gnome.eog.desktop"),
            default_changed: true,
        },
    }));
}

#[test]
fn rejects_bad_frames_and_noncanonical_values() {
    let encoded = Message::Close {
        app: id(1),
        reason: CloseReason::Requested,
    }
    .encode()
    .expect("encode");

    let mut bad_magic = encoded.clone();
    bad_magic[0] = b'X';
    assert_eq!(Message::decode(&bad_magic), Err(DecodeError::BadMagic));

    let mut bad_version = encoded.clone();
    bad_version[4..6].copy_from_slice(&99_u16.to_le_bytes());
    assert_eq!(
        Message::decode(&bad_version),
        Err(DecodeError::UnsupportedVersion(99))
    );

    let mut bad_length = encoded.clone();
    bad_length[8..12].copy_from_slice(&999_u32.to_le_bytes());
    assert_eq!(
        Message::decode(&bad_length),
        Err(DecodeError::LengthMismatch)
    );

    let mut zero_id = encoded;
    zero_id[HEADER_BYTES..HEADER_BYTES + 8].fill(0);
    assert_eq!(
        Message::decode(&zero_id),
        Err(DecodeError::InvalidValue("app id"))
    );
}

#[test]
fn rejects_count_and_transfer_limits_before_allocating() {
    let message = Message::Open {
        app: id(1),
        paths: vec!["x".into(); MAX_ARGUMENTS + 1],
    };
    assert!(matches!(
        message.encode(),
        Err(EncodeError::LimitExceeded { field: "paths", .. })
    ));

    let mut payload = Vec::new();
    payload.extend_from_slice(&1_u64.to_le_bytes());
    payload.extend_from_slice(
        &u32::try_from(MAX_ARGUMENTS + 1)
            .expect("test count fits")
            .to_le_bytes(),
    );
    let mut frame = Vec::new();
    frame.extend_from_slice(MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&(Kind::Open as u16).to_le_bytes());
    frame.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload fits")
            .to_le_bytes(),
    );
    frame.extend_from_slice(&payload);
    assert!(matches!(
        Message::decode(&frame),
        Err(DecodeError::LimitExceeded { field: "paths", .. })
    ));

    let oversized = Message::ClipboardRequest(ClipboardRequest::Write {
        request: id(2),
        selection: ClipboardSelection::Clipboard,
        mime_type: "application/octet-stream".into(),
        data: vec![0; MAX_CLIPBOARD_BYTES + 1],
    });
    assert!(matches!(
        oversized.encode(),
        Err(EncodeError::LimitExceeded {
            field: "clipboard data",
            ..
        })
    ));
}

#[test]
fn rejects_invalid_utf8_and_trailing_payload() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_u64.to_le_bytes());
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.push(0xff);
    payload.extend_from_slice(&0_u32.to_le_bytes());
    let mut frame = Vec::new();
    frame.extend_from_slice(MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&(Kind::Launch as u16).to_le_bytes());
    frame.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload fits")
            .to_le_bytes(),
    );
    frame.extend_from_slice(&payload);
    assert_eq!(
        Message::decode(&frame),
        Err(DecodeError::InvalidUtf8("entrypoint"))
    );

    let mut valid = Message::Close {
        app: id(1),
        reason: CloseReason::Requested,
    }
    .encode()
    .expect("encode");
    valid.push(0);
    let payload_length = u32::try_from(valid.len() - HEADER_BYTES).expect("test payload fits");
    valid[8..12].copy_from_slice(&payload_length.to_le_bytes());
    assert_eq!(Message::decode(&valid), Err(DecodeError::TrailingBytes));
}

#[test]
fn host_window_and_timer_messages_roundtrip() {
    roundtrip(Message::HostRequest(HostRequest {app:id(8),window:id(1),operation:HostOperation::SetContentSize {token:10,width:640,height:480,anchor:1}}));
    for error in ["", "This window has fixed geometry"] {
        roundtrip(Message::HostEvent(HostEvent {app:id(8),event:HostEventKind::WindowSizeResult {window:id(1),token:10,width:600,height:440,error:error.to_owned()}}));
    }
    for resume in [false,true] {
        roundtrip(Message::HostRequest(HostRequest {app:id(8),window:id(1),
            operation:HostOperation::DataRequestFence {token:u64::MAX,resume}}));
        for ok in [false,true] {
            roundtrip(Message::HostEvent(HostEvent {app:id(8),event:HostEventKind::DataRequestFence {token:u64::MAX,resume,ok}}));
        }
    }
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::SetFullscreen(true),
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::SetTimer {
            token: 44,
            interval_millis: 16,
            enabled: true,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::SetTitle(String::from("notes.txt")),
    }));
    for operation in [
        HostOperation::FileDialogResult { path: None },
        HostOperation::MenuResult { id: None },
        HostOperation::MenuResult { id: Some(String::from("bookmark")) },
        HostOperation::FileDialogResult { path: Some(String::from("/root/Documents/space # diagram.drawio")) },
        HostOperation::SetTitlebarVisible(false),
        HostOperation::MinimizeWindow,
        HostOperation::ToggleMaximizeWindow,
        HostOperation::StartWindowDrag { x: 480, y: 18 },
    ] {
        roundtrip(Message::HostRequest(HostRequest {
            app: id(8),
            window: id(1),
            operation,
        }));
    }
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ShowNotification {
            kind: String::from("info"),
            title: String::from("Files"),
            message: String::from("Selection copied"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::OpenExternalUrl(String::from("https://example.test")),
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::Timer { token: 44 },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::RiceBusSubscribe {
            topic: String::from("org.ricewm.vfs/changed"),
            filters: vec![String::from("/root/work"), String::from("/root/other")],
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::RiceBus {
            topic: String::from("org.ricewm.vfs/changed"),
            payload: String::from(r#"{"filterIndex":0,"filter":"/root/work"}"#),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::PrintText {
            title: String::from("Notes"),
            text: String::from("<hello>"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::DownloadText {
            filename: String::from("rise-profiler.json"),
            mime_type: String::from("application/json"),
            text: String::from("{\"version\":1}"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::DownloadFile {
            filename: String::from("archive.zip"),
            mime_type: String::from("application/zip"),
            data: vec![0, 1, 2, 255],
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::DownloadPath {
            path: String::from("/root/selection.zip"),
            filename: String::from("selection.zip"),
            show_storage_tip: true,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::RevealDownload {
            filename: String::from("archive.zip"),
            mime_type: String::from("application/zip"),
            open_file: true,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::CopyImage {
            mime_type: String::from("image/png"),
            data: vec![137, 80, 78, 71],
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::SaveAndOpenFile {
            filename: String::from("photo.png"),
            mime_type: String::from("image/png"),
            data: vec![1, 2, 3],
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::RequestClose,
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::MediaControl {
            session: 0x8000_0000_0000_0001,
            command: MediaCommand::SeekMillis,
            value: -5_000,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::MediaOpen {
            replace_session: 0x8000_0000_0000_0001,
            path: String::from("/root/Videos/next.mkv"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8), window: id(1),
        operation: HostOperation::MediaOpenMuted {
            replace_session: 0,
            path: String::from("/root/Videos/preview.mp4"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::MediaOpened {
            path: String::from("/root/Videos/next.mkv"),
            session: 0x8000_0000_0000_0002,
            duration_millis: 12_345,
            error: String::new(),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::MediaEnded {
            session: 0x8000_0000_0000_0002,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ProtectedMedia {
            request: 44,
            handle: 91,
            action: String::from("session-update"),
            metadata: String::from("{\"sessionType\":\"temporary\"}"),
            data: vec![0, 1, 2, 255],
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::ProtectedMedia {
            request: 0,
            ok: true,
            handle: 91,
            event: String::from("message"),
            metadata: String::from("{\"messageType\":\"license-request\"}"),
            data: vec![9, 8, 7],
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::Geolocation {
            tab: 7,
            request: 12,
            watch: true,
            options: String::from(r#"{"enableHighAccuracy":true}"#),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ClearGeolocation {
            tab: 7,
            request: 12,
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::Geolocation {
            tab: 7,
            request: 12,
            payload: String::from(r#"{"ok":true,"coords":{"latitude":1.5}}"#),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::JavaScriptRealm {
            request: 45,
            tab: 7,
            realm: 70,
            action: String::from("evaluate"),
            url: String::from("https://example.test/app.js"),
            source: b"document.body.textContent = 'native';".to_vec(),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::JavaScriptRealm {
            request: 45,
            tab: 7,
            realm: 70,
            event: String::from("evaluated"),
            metadata: String::from(r#"{"elapsedMillis":1}"#),
            source: b"<!doctype html><html><body>native</body></html>".to_vec(),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ImageExport {
            session: 77,
            action: ImageExportAction::SaveAs,
            width: 1080,
            height: 748,
            quarter_turns: 1,
            preserve_original: false,
            crop: Some([1, 2, 30, 40]),
            filename: String::from("photo.png"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ImageExport {
            session: 78,
            action: ImageExportAction::SaveTenant,
            width: 640,
            height: 480,
            quarter_turns: 0,
            preserve_original: true,
            crop: None,
            filename: String::from("/root/Pictures/photo.png"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::PerformanceSnapshot,
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::AccelerationDiagnostics,
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ProcessSignal {
            pid: 42,
            signal: 15,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ProcessNice { pid: 42, nice: -5 },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ExtensionStore {
            payload: String::from("search\twallpaper"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::CalendarStore {
            payload: String::from("subscribe"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::MailStore {
            payload: String::from("subscribe"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::ContactStore {
            payload: String::from("{\"command\":\"subscribe\"}"),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::Terminal {
            session: 9,
            action: TerminalAction::Open {
                columns: 100,
                rows: 32,
            },
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::Terminal {
            session: 9,
            action: TerminalAction::Input(vec![0x1b, b'[', b'A']),
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::Terminal {
            session: 9,
            action: TerminalAction::SuperFullscreen,
        },
    }));
    roundtrip(Message::HostRequest(HostRequest {
        app: id(8),
        window: id(1),
        operation: HostOperation::Terminal {
            session: 9,
            action: TerminalAction::Configure {
                copy_on_select: true,
                transparency: 20,
            },
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::Terminal {
            session: 9,
            event: TerminalEvent::Preferences {
                copy_on_select: true,
                transparency: 20,
            },
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::PerformanceSnapshot {
            payload: String::from("S\t1\t0"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::AccelerationDiagnostics {
            payload: String::from("A\t1\tbrave\tLinux\tgpu\twebgpu"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::ExtensionStore {
            payload: String::from("catalog\t0"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::CalendarStore {
            payload: String::from("snapshot\t2026-08-09\t09:00"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::MailStore {
            payload: String::from("snapshot\tidle"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::ContactStore {
            payload: String::from("{\"kind\":\"snapshot\",\"contacts\":[]}"),
        },
    }));
    roundtrip(Message::HostEvent(HostEvent {
        app: id(8),
        event: HostEventKind::Terminal {
            session: 9,
            event: TerminalEvent::Screen(TerminalScreen {
                text: String::from("ready\n$ "),
                caret: 8,
                columns: 100,
                rows: 32,
                application_cursor: false,
                application_keypad: false,
                bracketed_paste: true,
            }),
        },
    }));
}

#[test]
fn preferences_import_and_export_messages_roundtrip() {
    roundtrip(Message::SettingsRequest(SettingsRequest {
        request: id(41),
        app: id(8),
        operation: SettingsOperation::ExportPreferences,
    }));
    let document =
        String::from(r#"{"schema":"ricewm.preferences/v1","preferences":{"dock.visible":"true"}}"#);
    roundtrip(Message::SettingsRequest(SettingsRequest {
        request: id(42),
        app: id(8),
        operation: SettingsOperation::ImportPreferences {
            document: document.clone(),
        },
    }));
    roundtrip(Message::SettingsResponse(SettingsResponse {
        request: id(42),
        result: SettingsResult::PreferencesDocument(document),
    }));
}

#[test]
fn full_settings_catalog_roundtrips_with_a_bounded_provider_margin() {
    let entries:Vec<_>=(0..MAX_SETTINGS).map(|index|SettingEntry{key:format!("catalog.{index}"),value:"true".into()}).collect();
    roundtrip(Message::SettingsResponse(SettingsResponse{request:id(1),result:SettingsResult::Values(entries.clone())}));
    let mut oversized=entries;oversized.push(SettingEntry{key:"overflow".into(),value:"false".into()});
    assert!(Message::SettingsResponse(SettingsResponse{request:id(1),result:SettingsResult::Values(oversized)}).encode().is_err());
}

#[test]
fn file_sync_inspection_roundtrips_without_a_settings_mutation() {
    roundtrip(Message::SettingsRequest(SettingsRequest { request: id(20), app: id(2), operation: SettingsOperation::InspectFileSync {path: "/root/Documents/report.odt".to_owned()} }));
}

#[test]
fn cloud_mount_provider_frames_roundtrip() {
    for operation in [DataProviderOperation::MountCloudProviders, DataProviderOperation::MountConnectCloud {provider_id:"google-drive".into(), account_id:"account".into()}, DataProviderOperation::MountConnectCloudConfigured {provider_id:"nextcloud".into(),account_id:"saved".into(),config_json:r#"{"server":"https://cloud.example.test","appPassword":"secret"}"#.into()}] {
        roundtrip(Message::DataProviderRequest(DataProviderRequest {request:id(42),app:id(7),operation}));
    }
    roundtrip(Message::DataProviderResponse(DataProviderResponse {request:id(42),result:DataProviderResult::CloudProviders {json:r#"[{"id":"google-drive","label":"Google Drive"}]"#.into()}}));
}

#[test]
fn clipboard_multiformat_roundtrip_and_legacy_payloads_are_stable() {
    let formats = vec![
        ClipboardFormat { mime_type: "text/html".into(), data: b"<b>hello</b>".to_vec() },
        ClipboardFormat { mime_type: "text/plain;charset=utf-8".into(), data: b"hello".to_vec() },
    ];
    roundtrip(Message::ClipboardRequest(ClipboardRequest::WriteFormats {
        request: id(21), selection: ClipboardSelection::Clipboard, formats,
    }));
    let request = Message::ClipboardRequest(ClipboardRequest::Write {
        request: id(21), selection: ClipboardSelection::Primary,
        mime_type: "text/plain".into(), data: b"hi".to_vec(),
    }).encode().unwrap();
    let mut expected = vec![1];
    expected.extend_from_slice(&21_u64.to_le_bytes()); expected.push(1);
    expected.extend_from_slice(&10_u32.to_le_bytes()); expected.extend_from_slice(b"text/plain");
    expected.extend_from_slice(&2_u32.to_le_bytes()); expected.extend_from_slice(b"hi");
    assert_eq!(&request[HEADER_BYTES..], expected);
    let request = Message::ClipboardRequest(ClipboardRequest::Read {
        request: id(21), selection: ClipboardSelection::Clipboard,
        accepted_mime_types: vec!["text/plain".into()],
    }).encode().unwrap();
    let mut expected = vec![0];
    expected.extend_from_slice(&21_u64.to_le_bytes()); expected.push(0);
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&10_u32.to_le_bytes()); expected.extend_from_slice(b"text/plain");
    assert_eq!(&request[HEADER_BYTES..], expected);
}

#[test]
fn clipboard_multiformat_rejects_ambiguous_malformed_and_aggregate_oversize() {
    fn raw(formats: &[ClipboardFormat]) -> Vec<u8> {
        let mut payload = vec![2];
        payload.extend_from_slice(&21_u64.to_le_bytes()); payload.push(0);
        payload.extend_from_slice(&(formats.len() as u32).to_le_bytes());
        for value in formats {
            payload.extend_from_slice(&(value.mime_type.len() as u32).to_le_bytes());
            payload.extend_from_slice(value.mime_type.as_bytes());
            payload.extend_from_slice(&(value.data.len() as u32).to_le_bytes());
            payload.extend_from_slice(&value.data);
        }
        let mut frame = Vec::from(MAGIC);
        frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
        frame.extend_from_slice(&(Kind::ClipboardRequest as u16).to_le_bytes());
        frame.extend_from_slice(&(payload.len() as u32).to_le_bytes()); frame.extend(payload); frame
    }
    let format = |mime: &str| ClipboardFormat { mime_type: mime.into(), data: vec![1] };
    let cases = vec![
        vec![], vec![format("text/html"), format("TEXT/HTML")],
        vec![format("text/plain"), format("text/plain;charset=utf-8")],
        vec![format("text/html\n")], vec![format("not-a-mime")],
        vec![format("text/plain;")], vec![format("text/plain"); MAX_MIME_TYPES + 1],
        vec![ClipboardFormat { mime_type: "text/plain".into(), data: vec![0; MAX_CLIPBOARD_BYTES] }, format("text/html")],
    ];
    for formats in cases {
        let wire = raw(&formats);
        assert!(Message::ClipboardRequest(ClipboardRequest::WriteFormats {
            request: id(21), selection: ClipboardSelection::Clipboard, formats,
        }).encode().is_err());
        assert!(Message::decode(&wire).is_err());
    }
    let mut wire = raw(&[format("text/html"), format("text/plain")]);
    wire.pop();
    assert!(Message::decode(&wire).is_err());
}

#[test]
fn paddy_whatsapp_commands_and_events_have_independent_stable_tags() {
    let command=Message::HostRequest(HostRequest { app:id(44),window:id(1),operation:HostOperation::PaddyWhatsApp{payload:r#"{"kind":"send_text","chat":"fixture","text":"hello"}"#.into()} });
    assert_eq!(command.encode().unwrap()[HEADER_BYTES+16],40);
    roundtrip(command);
    let event=Message::HostEvent(HostEvent{app:id(44),event:HostEventKind::PaddyWhatsApp{payload:r#"{"events":[{"type":"Connected"}]}"#.into()}});
    assert_eq!(event.encode().unwrap()[HEADER_BYTES+8],17);
    roundtrip(event);
}
