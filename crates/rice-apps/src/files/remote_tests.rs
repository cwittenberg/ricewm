use super::{
    Message, UiEvent,
    files_menu::{CONNECT_SERVER, menu_widget},
    files_remote::{CANCEL, CONFIRM, PASSWORD_FIELD, URL_FIELD, USER_FIELD},
    files_view::widget_id,
    tests::{event, loaded_actor, request},
};
use rice_native_app_protocol::{
    DataProviderOperation, DataProviderRequest, DataProviderResponse, DataProviderResult,
    MountCapabilities, MountEntry, MountState, UiPatch, VfsOperation, VfsRequest,
};
use rice_ui::WidgetKind;

fn value(text: &str) -> UiEvent {
    UiEvent::ValueChanged(text.to_owned())
}

fn open_dialog(actor: &mut super::RiceFiles) {
    actor
        .handle_event(
            &event(menu_widget(CONNECT_SERVER), UiEvent::Activate),
            request(2),
        )
        .expect("open WebDAV dialog");
}

#[test]
fn connection_dialog_has_real_url_username_and_masked_password_fields() {
    let (mut actor, _) = loaded_actor();
    let messages = actor
        .handle_event(
            &event(menu_widget(CONNECT_SERVER), UiEvent::Activate),
            request(2),
        )
        .expect("open");
    let [Message::UiPatch(patch)] = messages.as_slice() else {
        panic!("dialog patch");
    };
    for (id, kind) in [
        (URL_FIELD, WidgetKind::TextField),
        (USER_FIELD, WidgetKind::TextField),
        (PASSWORD_FIELD, WidgetKind::PasswordField),
    ] {
        assert!(patch.patches.iter().any(|candidate| matches!(
            candidate,
            UiPatch::Create {
                id: candidate_id,
                kind: candidate_kind,
                ..
            } if *candidate_id == widget_id(id) && *candidate_kind == kind as u16
        )));
    }
    for label in ["Server address", "Username", "Password"] {
        assert!(patch.patches.iter().any(|candidate| matches!(
            candidate,
            UiPatch::Create {
                kind,
                text: Some(text),
                ..
            } if *kind == WidgetKind::Label as u16 && text == label
        )));
    }
}

#[test]
fn confirm_emits_only_a_secure_typed_webdav_request() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor
        .handle_event(
            &event(URL_FIELD, value("https://dav.example.test/files/")),
            request(3),
        )
        .expect("url");
    actor
        .handle_event(&event(USER_FIELD, value("ada")), request(4))
        .expect("username");
    actor
        .handle_event(&event(PASSWORD_FIELD, value("secret")), request(5))
        .expect("password");
    let messages = actor
        .handle_event(&event(CONFIRM, UiEvent::Activate), request(6))
        .expect("connect");
    assert!(matches!(
        messages.as_slice(),
        [
            Message::UiPatch(_),
            Message::DataProviderRequest(DataProviderRequest {
                request: provider_request,
                operation: DataProviderOperation::MountConnectWebDav {
                    url,
                    username,
                    password,
                },
                ..
            })
        ] if *provider_request == request(6)
            && url == "https://dav.example.test/files/"
            && username == "ada"
            && password == "secret"
    ));
}

#[test]
fn successful_connection_navigates_only_to_the_returned_open_mount() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor
        .handle_event(
            &event(URL_FIELD, value("https://dav.example.test/files/")),
            request(3),
        )
        .expect("url");
    actor
        .handle_event(&event(CONFIRM, UiEvent::Activate), request(4))
        .expect("connect");
    let messages = actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(4),
                result: DataProviderResult::MountConnected(MountEntry {
                    icon: None,
                    id: String::from("webdav-example"),
                    label: String::from("Example"),
                    path: String::from("/run/ricewm/mounts/webdav-example"),
                    state: MountState::Connected,
                    capabilities: MountCapabilities::OPEN
                        | MountCapabilities::DISCONNECT
                        | MountCapabilities::RECONNECT,
                }),
            },
            request(5),
        )
        .expect("connected");
    assert!(matches!(
        messages.as_slice(),
        [
            Message::UiPatch(_),
            Message::VfsRequest(VfsRequest {
                request: directory_request,
                operation: VfsOperation::ReadDirectory { path, .. },
                ..
            })
        ] if *directory_request == request(5)
            && path == "/run/ricewm/mounts/webdav-example"
    ));
}

#[test]
fn provider_denial_stays_in_dialog_and_clears_the_password() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor
        .handle_event(
            &event(URL_FIELD, value("https://dav.example.test/files/")),
            request(3),
        )
        .expect("url");
    actor
        .handle_event(&event(PASSWORD_FIELD, value("wrong")), request(4))
        .expect("password");
    actor
        .handle_event(&event(CONFIRM, UiEvent::Activate), request(5))
        .expect("connect");
    let messages = actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(5),
                result: DataProviderResult::Unavailable {
                    reason: String::from(
                        "Reading remote properties failed: WebDAV returned HTTP 401",
                    ),
                },
            },
            request(6),
        )
        .expect("denied");
    let [Message::UiPatch(patch), Message::HostRequest(notification)] = messages.as_slice() else {
        panic!("retry dialog");
    };
    assert!(matches!(
        &notification.operation,
        rice_native_app_protocol::HostOperation::ShowNotification {
            kind,
            title,
            message,
        } if kind == "error" && title == "Files" && message.contains("HTTP 401")
    ));
    assert!(patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create {
            id,
            kind,
            text: Some(text),
            ..
        } if *id == widget_id(PASSWORD_FIELD)
            && *kind == WidgetKind::PasswordField as u16
            && text.is_empty()
    )));
    assert!(patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create {
            kind,
            text: Some(text),
            ..
        } if *kind == WidgetKind::Label as u16 && text.contains("HTTP 401")
    )));
    assert!(!patch.patches.iter().any(|candidate| matches!(
        candidate,
        UiPatch::Create { kind, .. } if *kind == WidgetKind::Notification as u16
    )));
}

#[test]
fn cancelling_a_pending_connection_sends_real_abort_and_ignores_late_result() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor
        .handle_event(
            &event(URL_FIELD, value("https://dav.example.test/files/")),
            request(3),
        )
        .expect("url");
    actor
        .handle_event(&event(CONFIRM, UiEvent::Activate), request(4))
        .expect("connect");
    let cancellation = actor
        .handle_event(&event(CANCEL, UiEvent::Activate), request(5))
        .expect("cancel");
    assert!(matches!(
        cancellation.as_slice(),
        [
            Message::UiPatch(_),
            Message::DataProviderRequest(DataProviderRequest {
                request: cancel_request,
                operation: DataProviderOperation::MountCancelConnect {
                    connect_request,
                },
                ..
            })
        ] if *cancel_request == request(5) && *connect_request == request(4)
    ));
    let late = actor
        .handle_data_response(
            &DataProviderResponse {
                request: request(4),
                result: DataProviderResult::Unavailable {
                    reason: String::from("cancelled"),
                },
            },
            request(6),
        )
        .expect("late response ignored");
    assert!(late.is_empty());
}

#[test]
fn unsupported_protocol_never_reaches_the_provider() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor
        .handle_event(
            &event(URL_FIELD, value("sftp://server.example/shared")),
            request(3),
        )
        .expect("url");
    let messages = actor
        .handle_event(&event(CONFIRM, UiEvent::Activate), request(4))
        .expect("validation");
    assert!(matches!(
        messages.as_slice(),
        [
            Message::UiPatch(_),
            Message::HostRequest(rice_native_app_protocol::HostRequest {
                operation: rice_native_app_protocol::HostOperation::ShowNotification {
                    kind,
                    title,
                    ..
                },
                ..
            })
        ] if kind == "error" && title == "Files"
    ));
}

#[test]
fn network_connect_displays_spinner_and_keeps_cancel_enabled() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor.handle_event(&event(URL_FIELD,value("smb://server/documents")),request(3)).unwrap();
    let messages=actor.handle_event(&event(CONFIRM,UiEvent::Activate),request(4)).unwrap();
    assert!(messages.iter().any(|m| matches!(m,Message::DataProviderRequest(DataProviderRequest {operation:DataProviderOperation::MountConnectNetwork {..},..}))));
    let patch=messages.iter().find_map(|m| if let Message::UiPatch(p)=m {Some(p)}else{None}).unwrap();
    assert!(patch.patches.iter().any(|p| matches!(p,UiPatch::Create{kind,..} if *kind==WidgetKind::Spinner as u16)));
    assert!(patch.patches.iter().any(|p| matches!(p,UiPatch::Create{id,state,..} if *id==widget_id(CANCEL) && (*state & rice_ui::WidgetState::ENABLED.bits())!=0)));
    let cancelled=actor.handle_event(&event(CANCEL,UiEvent::Activate),request(5)).unwrap();
    assert!(cancelled.iter().any(|m|matches!(m,Message::DataProviderRequest(DataProviderRequest{operation:DataProviderOperation::MountCancelConnect{connect_request},..}) if *connect_request==request(4))));
}

#[test]
fn smb_server_address_enters_pending_connection_state() {
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    actor.handle_event(&event(URL_FIELD,value("smb://192.168.2.17")),request(3)).unwrap();
    let messages=actor.handle_event(&event(CONFIRM,UiEvent::Activate),request(4)).unwrap();
    assert!(messages.iter().any(|message|matches!(message,Message::DataProviderRequest(_))));
    let cancelled=actor.handle_event(&event(CANCEL,UiEvent::Activate),request(5)).unwrap();
    assert!(cancelled.iter().any(|message|matches!(message,Message::DataProviderRequest(_))));
    assert!(messages.iter().any(|message|matches!(message,Message::UiPatch(_))));
}

#[test]
fn cloud_choices_are_host_supplied_and_connect_without_password_fields() {
    use super::files_remote::{CLOUD, CLOUD_FIRST};
    let (mut actor, _) = loaded_actor();
    open_dialog(&mut actor);
    let messages = actor.handle_event(&event(CLOUD, UiEvent::Activate), request(20)).unwrap();
    assert!(messages.iter().any(|m| matches!(m, Message::DataProviderRequest(DataProviderRequest {operation:DataProviderOperation::MountCloudProviders,..}))));
    let response = DataProviderResponse { request:request(20), result:DataProviderResult::CloudProviders {
        json:r#"[{"id":"google-drive","label":"Google Drive","icon":159},{"id":"future-drive","label":"Another provider"}]"#.into(),
    }};
    let messages = actor.handle_remote_response(&response, request(21)).unwrap().unwrap();
    let patch = messages.iter().find_map(|m| if let Message::UiPatch(p)=m {Some(p)} else {None}).unwrap();
    assert!(patch.patches.iter().any(|p| matches!(p,UiPatch::Create {text:Some(text),..} if text == "Another provider")));
    assert!(!patch.patches.iter().any(|p| matches!(p,UiPatch::Create {id,..} if *id == widget_id(PASSWORD_FIELD))));
    assert!(patch.patches.iter().any(|p| matches!(p,UiPatch::Create {text:Some(text),icon:Some(159),..} if text == "Google Drive")));
    assert!(patch.patches.iter().any(|p| matches!(p,UiPatch::Create {text:Some(text),icon:Some(icon),..} if text == "Another provider" && *icon == rice_ui::IconId::Network.wire_value())));
    let messages = actor.handle_event(&event(CLOUD_FIRST + 1, UiEvent::Activate), request(22)).unwrap();
    assert!(messages.iter().any(|m| matches!(m,Message::DataProviderRequest(DataProviderRequest {operation:DataProviderOperation::MountConnectCloud {provider_id,account_id},..}) if provider_id == "future-drive" && account_id.is_empty())));
    let duplicate = actor.handle_event(&event(CLOUD_FIRST + 1, UiEvent::Activate), request(23)).unwrap();
    assert!(!duplicate.iter().any(|m| matches!(m,Message::DataProviderRequest(_))));
    let messages = actor.handle_event(&event(CANCEL, UiEvent::Activate), request(24)).unwrap();
    assert!(messages.iter().any(|m| matches!(m,Message::DataProviderRequest(DataProviderRequest {operation:DataProviderOperation::MountCancelConnect {connect_request},..}) if *connect_request == request(22))));
}

#[test]
fn provider_fields_mask_secrets_validate_required_and_emit_configured_request() {
    use super::files_remote::{CLOUD, CLOUD_FIRST, CLOUD_FIELD_FIRST};
    let (mut actor, _) = loaded_actor(); open_dialog(&mut actor);
    actor.handle_event(&event(CLOUD, UiEvent::Activate),request(20)).unwrap();
    let response=DataProviderResponse{request:request(20),result:DataProviderResult::CloudProviders{json:r#"[{"id":"example","label":"Example","fields":[{"id":"server","label":"Server","required":true,"defaultValue":"https://example.test"},{"id":"secret","label":"Secret","required":true,"secret":true}]}]"#.into()}};
    actor.handle_remote_response(&response,request(21)).unwrap().unwrap();
    let messages=actor.handle_event(&event(CLOUD_FIRST,UiEvent::Activate),request(22)).unwrap();
    assert!(!messages.iter().any(|m|matches!(m,Message::DataProviderRequest(_))));
    let patch=messages.iter().find_map(|m|if let Message::UiPatch(p)=m{Some(p)}else{None}).unwrap();
    assert!(patch.patches.iter().any(|p|matches!(p,UiPatch::Create{id,kind,..}if *id==widget_id(CLOUD_FIELD_FIRST+1)&&*kind==WidgetKind::PasswordField as u16)));
    let missing=actor.handle_event(&event(CONFIRM,UiEvent::Activate),request(23)).unwrap();
    assert!(!missing.iter().any(|m|matches!(m,Message::DataProviderRequest(_))));
    actor.handle_event(&event(CLOUD_FIELD_FIRST+1,value("test-secret")),request(24)).unwrap();
    let messages=actor.handle_event(&event(CONFIRM,UiEvent::Activate),request(25)).unwrap();
    assert!(messages.iter().any(|m|matches!(m,Message::DataProviderRequest(DataProviderRequest{operation:DataProviderOperation::MountConnectCloudConfigured{provider_id,config_json,..},..})if provider_id=="example"&&serde_json::from_str::<serde_json::Value>(config_json).unwrap()["secret"]=="test-secret")));
    let error=DataProviderResponse{request:request(25),result:DataProviderResult::Unavailable{reason:"Connection failed".into()}};
    let messages=actor.handle_remote_response(&error,request(26)).unwrap().unwrap();
    assert!(!format!("{messages:?}").contains("test-secret"));
}
