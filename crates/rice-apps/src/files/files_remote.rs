//! Network server connection dialog and typed provider lifecycle.

use rice_files::{GuestPath, ProviderLocation, ProviderLocationError};
use rice_native_app_protocol::{
    DataProviderOperation, DataProviderRequest, DataProviderResponse, DataProviderResult, Message,
    MountCapabilities, MountState, RequestId,
};
use rice_ui::{IconId, Patch, WidgetKind, WidgetState};

use super::{FilesActorError, RiceFiles, files_view};

pub(super) const URL_FIELD: u64 = 310_001;
pub(super) const USER_FIELD: u64 = 310_002;
pub(super) const PASSWORD_FIELD: u64 = 310_003;
pub(super) const CONFIRM: u64 = 310_004;
pub(super) const CANCEL: u64 = 310_005;
pub(super) const DOMAIN_FIELD: u64 = 310_017;
pub(super) const UID_FIELD: u64 = 310_018;
pub(super) const GID_FIELD: u64 = 310_019;
pub(super) const SMB: u64 = 310_020;
pub(super) const NFS: u64 = 310_021;
pub(super) const WEBDAV: u64 = 310_022;
pub(super) const BRIDGE_URL: u64 = 310_025;
pub(super) const BRIDGE_TOKEN: u64 = 310_026;
pub(super) const BRIDGE: u64 = 310_027;
pub(super) const CLOUD: u64 = 310_054;
pub(super) const CLOUD_BACK: u64 = 310_056;
pub(super) const CLOUD_FIELD_FIRST: u64 = 312_000;
pub(super) const CLOUD_FIELD_LAST: u64 = CLOUD_FIELD_FIRST + 7;
pub(super) const CLOUD_FIRST: u64 = 311_000;
pub(super) const CLOUD_LAST: u64 = CLOUD_FIRST + 63;

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
struct CloudProvider { id: String, label: String, #[serde(default)] icon: Option<u16>, #[serde(default)] fields: Vec<CloudField>, #[serde(default)] help: String }
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CloudField { id: String, label: String, #[serde(default)] secret: bool, #[serde(default)] required: bool, #[serde(default)] default_value: String }
impl CloudProvider {
    fn valid(&self) -> bool {
        !self.id.is_empty() && self.id.len() <= 48 && self.id.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-') && !self.label.is_empty() && self.label.len() <= 256 && self.help.len() <= 512 && self.fields.len() <= 8 && self.fields.iter().all(|f| !f.id.is_empty() && f.id.len() <= 32 && f.id.bytes().all(|c| c.is_ascii_alphanumeric()) && !f.label.is_empty() && f.label.len() <= 128 && f.default_value.len() <= 4096)
    }
}
const DIALOG: u64 = 310_000;
const INTRO: u64 = 310_006;
const ERROR: u64 = 310_007;
const PROGRESS: u64 = 310_008;
const CONTENT: u64 = 310_009;
const URL_ROW: u64 = 310_010;
const USER_ROW: u64 = 310_011;
const PASSWORD_ROW: u64 = 310_012;
const URL_LABEL: u64 = 310_013;
const USER_LABEL: u64 = 310_014;
const PASSWORD_LABEL: u64 = 310_015;
const ACTIONS: u64 = 310_016;
const MAX_LOCATION_BYTES: usize = GuestPath::MAX_BYTES;
const MAX_CREDENTIAL_BYTES: usize = 4 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RemoteConnection {
    dialog: Option<ConnectDialog>,
    abandoned: Vec<RequestId>,
    describing: Option<RequestId>,
    cloud_catalog: Option<RequestId>,
    providers: Vec<CloudProvider>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConnectDialog {
    cloud: bool,
    provider_id: String,
    account_id: String,
    cloud_fields: Vec<CloudField>,
    cloud_values: std::collections::BTreeMap<String,String>,
    cloud_help: String,
    url: String,
    username: String,
    password: String,
    domain: String,
    bridge_url: String,
    bridge_token: String,
    bridge: bool,
    uid: String,
    gid: String,
    error: ConnectionError,
    pending: Option<RequestId>,
}

impl ConnectDialog {
    fn clear_cloud_secrets(&mut self) { for field in &self.cloud_fields { if field.secret { self.cloud_values.remove(&field.id); } } }
}

// Owned failures retain their identity until rendering; provider text is data.
#[derive(Clone, Debug, Eq, PartialEq)]
enum ConnectionError {
    None,
    Host(String),
    Message(&'static str),
    TooLong { address: bool, maximum: usize },
    InvalidAddress(ProviderLocationError),
    MissingShare(String),
}
impl ConnectionError {
    fn clear(&mut self) { *self = Self::None; }
    fn is_empty(&self) -> bool { matches!(self, Self::None) }
    fn render(&self) -> String {
        match self {
            Self::None => String::new(),
            Self::Host(reason) => reason.clone(),
            Self::Message(message) => rice_i18n::t(message).to_owned(),
            Self::TooLong { address, maximum } => rice_i18n::format(
                if *address { "Server address is too long (maximum {maximum} bytes)" }
                else { "Credential is too long (maximum {maximum} bytes)" },
                &[("maximum", &maximum.to_string())]),
            Self::MissingShare(example) => rice_i18n::format(
                "Include a share or export name, for example {example}", &[("example", example)]),
            Self::InvalidAddress(error) => rice_i18n::t(match error {
                ProviderLocationError::TooLong => "The server address exceeds the supported length",
                ProviderLocationError::MissingScheme => "Include a protocol in the server address, such as https://",
                ProviderLocationError::InvalidScheme => "The server address contains an invalid protocol",
                ProviderLocationError::InvalidAuthority => "Enter a valid server name without embedded credentials",
                ProviderLocationError::ZeroRequest => "The server connection request is invalid",
            }).to_owned(),
        }
    }
}

impl RemoteConnection {
    pub(super) const fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub(super) fn open(&mut self) {
        if let Some(dialog) = &mut self.dialog {
            dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
        }
        self.dialog = Some(ConnectDialog {
            cloud: false, provider_id: String::new(), account_id: String::new(),
            cloud_fields: Vec::new(), cloud_values: std::collections::BTreeMap::new(), cloud_help: String::new(),
            url: String::from("https://"),
            username: String::new(),
            password: String::new(),
            domain: String::new(),
            bridge_url: String::new(),
            bridge_token: String::new(),
            bridge: false,
            uid: String::from("65534"),
            gid: String::from("65534"),
            error: ConnectionError::None,
            pending: None,
        });
    }

    pub(super) fn open_url(&mut self, url: &str) {
        self.open();
        if let Some(dialog) = self.dialog_mut() {dialog.url = url.to_owned();}
    }

    fn dialog_mut(&mut self) -> Option<&mut ConnectDialog> {
        self.dialog.as_mut()
    }

    fn pending(&self) -> Option<RequestId> {
        self.dialog.as_ref()?.pending
    }

    fn abandon(&mut self, request: RequestId) {
        if self.abandoned.len() == 32 {
            self.abandoned.remove(0);
        }
        self.abandoned.push(request);
    }

    fn take_abandoned(&mut self, request: RequestId) -> bool {
        let Some(index) = self.abandoned.iter().position(|item| *item == request) else {
            return false;
        };
        self.abandoned.remove(index);
        true
    }
}

impl RiceFiles {
    pub(super) fn show_connect_server(&mut self) -> Result<Vec<Message>, FilesActorError> {
        self.remote.open();
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn show_cloud_providers(&mut self, request: RequestId) -> Result<Vec<Message>, FilesActorError> {
        if let Some(dialog) = self.remote.dialog_mut() {
            if dialog.pending.is_some() { return Ok(Vec::new()); }
            dialog.cloud = true; dialog.provider_id.clear(); dialog.account_id.clear();
            dialog.cloud_fields.clear(); dialog.cloud_values.clear(); dialog.cloud_help.clear();
            dialog.password.clear(); dialog.clear_cloud_secrets(); dialog.bridge_token.clear(); dialog.error.clear();
        }
        self.remote.cloud_catalog = Some(request);
        Ok(vec![self.emit_full()?, Message::DataProviderRequest(DataProviderRequest {
            request, app:self.app, operation:DataProviderOperation::MountCloudProviders,
        })])
    }

    pub(super) fn select_cloud_provider(&mut self, target: u64, request: RequestId) -> Result<Vec<Message>, FilesActorError> {
        let provider = self.remote.providers.get((target - CLOUD_FIRST) as usize).cloned();
        let (Some(dialog), Some(provider)) = (self.remote.dialog_mut(), provider) else { return Ok(Vec::new()); };
        if dialog.pending.is_some() || !dialog.cloud { return Ok(Vec::new()); }
        dialog.provider_id = provider.id;
        dialog.cloud_fields = provider.fields;
        dialog.cloud_help = provider.help;
        dialog.cloud_values = dialog.cloud_fields.iter().map(|f| (f.id.clone(), if f.secret { String::new() } else { f.default_value.clone() })).collect();
        dialog.account_id.clear();
        dialog.error.clear();
        if dialog.cloud_fields.is_empty() { self.confirm_remote(request) } else { self.emit_full().map(|message|vec![message]) }
    }

    pub(super) fn reopen_network_mount(&mut self, mount_id: String, request: RequestId) -> Result<Vec<Message>, FilesActorError> {
        self.remote.describing=Some(request);
        Ok(vec![Message::DataProviderRequest(DataProviderRequest {request,app:self.app,operation:DataProviderOperation::MountDescribeNetwork {mount_id}})])
    }
    pub(super) fn select_remote_protocol(&mut self, target: u64) -> Result<Vec<Message>, FilesActorError> {
        if let Some(dialog) = self.remote.dialog_mut() {
            if dialog.pending.is_none() {
                if target == BRIDGE {dialog.bridge = !dialog.bridge; return self.emit_full().map(|message| vec![message]);}
                dialog.cloud = false; dialog.provider_id.clear(); dialog.account_id.clear();
                dialog.url = String::from(match target { SMB => "smb://", NFS => "nfs://", _ => "https://" });
                dialog.username.clear(); dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear(); dialog.domain.clear(); dialog.error.clear();
            }
        }
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn update_remote_field(
        &mut self,
        target: u64,
        value: &str,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(dialog) = self.remote.dialog_mut() else {
            return Ok(Vec::new());
        };
        if dialog.pending.is_some() {
            return Ok(Vec::new());
        }
        let maximum = if target == URL_FIELD {
            MAX_LOCATION_BYTES
        } else {
            MAX_CREDENTIAL_BYTES
        };
        if value.len() > maximum {
            dialog.error = ConnectionError::TooLong { address: target == URL_FIELD, maximum };
            if target == PASSWORD_FIELD {
                dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
            }
            let notice = dialog.error.render();
            return Ok(vec![
                self.emit_full()?,
                self.system_notification("error", &notice),
            ]);
        }
        if (CLOUD_FIELD_FIRST..=CLOUD_FIELD_LAST).contains(&target) {
            if let Some(field) = dialog.cloud_fields.get((target-CLOUD_FIELD_FIRST) as usize) {
                dialog.cloud_values.insert(field.id.clone(),value.to_owned());
            }
            dialog.error.clear();
            return self.emit_full().map(|message|vec![message]);
        }
        match target {
            URL_FIELD => value.clone_into(&mut dialog.url),
            USER_FIELD => value.clone_into(&mut dialog.username),
            PASSWORD_FIELD => value.clone_into(&mut dialog.password),
            DOMAIN_FIELD => value.clone_into(&mut dialog.domain),
            BRIDGE_URL => value.clone_into(&mut dialog.bridge_url),
            BRIDGE_TOKEN => value.clone_into(&mut dialog.bridge_token),
            UID_FIELD => value.clone_into(&mut dialog.uid),
            GID_FIELD => value.clone_into(&mut dialog.gid),
            _ => return Ok(Vec::new()),
        }
        dialog.error.clear();
        self.emit_full().map(|message| vec![message])
    }

    pub(super) fn confirm_remote(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let Some(dialog) = self.remote.dialog_mut() else {
            return Ok(Vec::new());
        };
        if dialog.pending.is_some() {
            return Ok(Vec::new());
        }
        if dialog.cloud {
            if dialog.provider_id.is_empty() {
                dialog.error = ConnectionError::Message("Choose a cloud storage provider");
                return self.emit_full().map(|message| vec![message]);
            }
            if let Some(field) = dialog.cloud_fields.iter().find(|f| f.required && dialog.cloud_values.get(&f.id).is_none_or(|v|v.trim().is_empty())) {
                dialog.error=ConnectionError::Host(format!("{}: {}",field.label,rice_i18n::t("files.connection-required")));
                return self.emit_full().map(|message|vec![message]);
            }
            let operation = if dialog.cloud_fields.is_empty() { DataProviderOperation::MountConnectCloud {provider_id:dialog.provider_id.clone(), account_id:dialog.account_id.clone()} }
                else { DataProviderOperation::MountConnectCloudConfigured {provider_id:dialog.provider_id.clone(), account_id:dialog.account_id.clone(), config_json:serde_json::to_string(&dialog.cloud_values).unwrap_or_default()} };
            dialog.pending = Some(request); dialog.error.clear();
            return Ok(vec![self.emit_full()?, Message::DataProviderRequest(DataProviderRequest {request, app:self.app, operation})]);
        }
        let location = match supported_webdav_location(&dialog.url) {
            Ok(location) => location,
            Err(error) => {
                let notice = error.render();
                dialog.error = error;
                return Ok(vec![
                    self.emit_full()?,
                    self.system_notification("error", &notice),
                ]);
            }
        };
        if location.scheme() == "nfs" && location.path().trim_matches('/').is_empty() {
            dialog.error = ConnectionError::MissingShare(format!("{}://{}/Documents", location.scheme(), location.authority()));
            return self.emit_full().map(|message| vec![message]);
        }
        let url = format!("{}://{}{}", location.scheme(), location.authority(), location.path());
        let operation = if matches!(location.scheme(), "smb" | "nfs") {
            let (Ok(uid), Ok(gid)) = (dialog.uid.parse::<u32>(), dialog.gid.parse::<u32>()) else {
                dialog.error = ConnectionError::Message("NFS user and group IDs must be unsigned integers");
                return self.emit_full().map(|message| vec![message]);
            };
            DataProviderOperation::MountConnectNetwork {
                url, username: dialog.username.clone(), password: dialog.password.clone(), domain: dialog.domain.clone(), bridge_url: dialog.bridge_url.clone(), bridge_token: dialog.bridge_token.clone(), uid, gid,
            }
        } else { DataProviderOperation::MountConnectWebDav {
            url: format!(
                "{}://{}{}",
                location.scheme(),
                location.authority(),
                location.path()
            ),
            username: dialog.username.clone(),
            password: dialog.password.clone(),
        }};
        dialog.pending = Some(request);
        dialog.error.clear();
        Ok(vec![
            self.emit_full()?,
            Message::DataProviderRequest(DataProviderRequest {
                request,
                app: self.app,
                operation,
            }),
        ])
    }

    pub(super) fn cancel_remote(
        &mut self,
        request: RequestId,
    ) -> Result<Vec<Message>, FilesActorError> {
        let pending = self.remote.pending();
        if let Some(dialog) = self.remote.dialog_mut() {
            dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
        }
        self.remote.dialog = None;
        if let Some(catalog) = self.remote.cloud_catalog.take() { self.remote.abandon(catalog); }
        let mut messages = vec![self.emit_full()?];
        if let Some(connect_request) = pending {
            self.remote.abandon(connect_request);
            self.remote.abandon(request);
            messages.push(Message::DataProviderRequest(DataProviderRequest {
                request,
                app: self.app,
                operation: DataProviderOperation::MountCancelConnect { connect_request },
            }));
        }
        Ok(messages)
    }

    pub(super) fn handle_remote_response(
        &mut self,
        response: &DataProviderResponse,
        request: RequestId,
    ) -> Option<Result<Vec<Message>, FilesActorError>> {
        if self.remote.cloud_catalog == Some(response.request) {
            self.remote.cloud_catalog = None;
            let providers = match &response.result {
                DataProviderResult::CloudProviders {json} => serde_json::from_str::<Vec<CloudProvider>>(json).ok().filter(|items| items.len() <= 64 && items.iter().all(CloudProvider::valid)),
                _ => None,
            };
            match providers {
                Some(providers) => {
                    if let Some(dialog) = self.remote.dialog_mut() {
                        if dialog.cloud { dialog.provider_id = providers.first().map(|p| p.id.clone()).unwrap_or_default(); }
                    }
                    self.remote.providers = providers;
                }
                None => if let Some(dialog) = self.remote.dialog_mut() {
                    dialog.error = match &response.result { DataProviderResult::Unavailable {reason} => ConnectionError::Host(reason.clone()), _ => ConnectionError::Message("Cloud storage is unavailable in this session") };
                },
            }
            return Some(self.emit_full().map(|message| vec![message]));
        }
        if self.remote.describing == Some(response.request) {
            self.remote.describing = None;
            return Some(match &response.result {
                DataProviderResult::NetworkConnectionInfo {json} => {
                    let profile: serde_json::Value = match serde_json::from_str(json) {Ok(value)=>value,Err(_)=>return Some(self.emit_notice(rice_i18n::t("Invalid saved server connection")))};
                    self.remote.open();
                    if let Some(dialog) = self.remote.dialog_mut() {
                        dialog.provider_id=profile["providerId"].as_str().unwrap_or("").to_owned();
                        dialog.account_id=profile["accountId"].as_str().unwrap_or("").to_owned();
                        dialog.cloud=!dialog.provider_id.is_empty();
                        if let Ok(provider) = serde_json::from_value::<CloudProvider>(profile["provider"].clone()) {
                            if provider.valid() && provider.id == dialog.provider_id {
                                dialog.cloud_fields=provider.fields; dialog.cloud_help=provider.help;
                                for field in &dialog.cloud_fields { dialog.cloud_values.insert(field.id.clone(),if field.secret {String::new()} else {profile["config"][&field.id].as_str().unwrap_or(&field.default_value).to_owned()}); }
                            }
                        }
                        dialog.url=profile["url"].as_str().unwrap_or("").to_owned();
                        dialog.username=profile["username"].as_str().unwrap_or("").to_owned();
                        dialog.domain=profile["domain"].as_str().unwrap_or("").to_owned();
                        dialog.uid=profile["uid"].as_u64().unwrap_or(65534).to_string();
                        dialog.gid=profile["gid"].as_u64().unwrap_or(65534).to_string();
                        dialog.bridge_url=profile["bridgeUrl"].as_str().unwrap_or("").to_owned();
                        dialog.bridge=!dialog.bridge_url.is_empty();
                    }
                    self.emit_full().map(|message|vec![message])
                }
                DataProviderResult::Unavailable {reason} => self.emit_notice(reason),
                _ => Err(FilesActorError::UnexpectedDataProviderResponse),
            });
        }
        if self.remote.take_abandoned(response.request) {
            return Some(Ok(Vec::new()));
        }
        if self.remote.pending() != Some(response.request) {
            return None;
        }
        let result = match &response.result {
            DataProviderResult::MountConnected(entry)
                if entry.state == MountState::Connected
                    && entry.capabilities.contains(MountCapabilities::OPEN) =>
            {
                let target = match GuestPath::absolute(&entry.path) {
                    Ok(target) => target,
                    Err(error) => return Some(Err(error.into())),
                };
                if let Some(dialog) = self.remote.dialog_mut() {
                    dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
                }
                self.remote.dialog = None;
                self.mount_entries.retain(|mount| mount.id != entry.id);
                self.mount_entries.push(entry.clone());
                self.begin_navigation(
                    rice_files::NavigationCommand::Enter(target.as_str().to_owned()),
                    request,
                )
            }
            DataProviderResult::Unavailable { reason } => {
                if let Some(dialog) = self.remote.dialog_mut() {
                    dialog.pending = None;
                    dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
                    if reason.to_lowercase().contains("bridge") {dialog.bridge = true;}
                    dialog.error = ConnectionError::Host(reason.clone());
                }
                let notification = self.system_notification("error", reason);
                self.emit_full().map(|message| vec![message, notification])
            }
            DataProviderResult::MountConnected(_) => {
                let notice = "The server host did not return an open connected mount";
                if let Some(dialog) = self.remote.dialog_mut() {
                    dialog.pending = None;
                    dialog.password.clear(); dialog.clear_cloud_secrets();
            dialog.bridge_token.clear();
                    dialog.error = ConnectionError::Message(notice);
                }
                let notification = self.system_notification("error", rice_i18n::t(notice));
                self.emit_full().map(|message| vec![message, notification])
            }
            _ => Err(FilesActorError::UnexpectedDataProviderResponse),
        };
        Some(result)
    }
}

pub(super) fn patches(patches: &mut Vec<Patch>, remote: &RemoteConnection) {
    let Some(dialog) = &remote.dialog else {
        return;
    };
    files_view::create(
        patches,
        DIALOG,
        Some(files_view::ROOT),
        WidgetKind::Dialog,
        Some(rice_i18n::t("Connect to Server")),
        Some(IconId::Network),
    );
    files_view::create(
        patches,
        CONTENT,
        Some(DIALOG),
        WidgetKind::Column,
        None,
        None,
    );
    files_view::create(
        patches,
        ACTIONS,
        Some(DIALOG),
        WidgetKind::Row,
        Some("panel:actions"),
        None,
    );
    files_view::create(
        patches,
        INTRO,
        Some(CONTENT),
        WidgetKind::Label,
        Some(rice_i18n::t("Connect to a shared folder on another computer.")),
        Some(IconId::Information),
    );
    files_view::create(patches, 310_050, Some(CONTENT), WidgetKind::Row, Some("panel:actions:choice-width=WebDAV"), None);
    for (id, text, scheme) in [(SMB,"SMB","smb://"),(NFS,"NFS","nfs://"),(WEBDAV,"WebDAV","http"),(CLOUD,"Cloud storage","cloud")] {
        files_view::create_with_state(patches,id,Some(310_050),WidgetKind::Button,Some(text),Some(IconId::Network),
            (if dialog.pending.is_none() {WidgetState::ENABLED} else {WidgetState::empty()}) | if (id == CLOUD && dialog.cloud) || (!dialog.cloud && dialog.url.starts_with(scheme)) {WidgetState::ACCENT} else {WidgetState::empty()});
    }
    if dialog.cloud {
        if dialog.cloud_fields.is_empty() {
        for (index, provider) in remote.providers.iter().enumerate() {
            files_view::create_with_state(patches,CLOUD_FIRST + index as u64,Some(CONTENT),WidgetKind::Button,Some(&provider.label),Some(provider.icon.and_then(IconId::from_wire).unwrap_or(IconId::Network)),
                if dialog.pending.is_none() {WidgetState::ENABLED} else {WidgetState::empty()});
        }
        } else {
            files_view::create(patches,CLOUD_BACK,Some(CONTENT),WidgetKind::Button,Some(rice_i18n::t("files.change-provider")),Some(IconId::GoBack));
            for (index, field) in dialog.cloud_fields.iter().enumerate() {
                field_row(patches,312_100+index as u64,312_200+index as u64,&field.label,CLOUD_FIELD_FIRST+index as u64,
                    if field.secret {WidgetKind::PasswordField} else {WidgetKind::TextField},dialog.cloud_values.get(&field.id).map(String::as_str).unwrap_or(""),if field.secret {IconId::Lock}else{IconId::Network});
            }
        }
        let help = if !dialog.cloud_help.is_empty() { dialog.cloud_help.as_str() }
            else if remote.cloud_catalog.is_some() { "Loading cloud providers…" }
            else if dialog.provider_id.is_empty() { "No cloud storage providers are available in this session" }
            else { "Choose a provider to sign in. Saved drives can be reconnected here." };
        files_view::create(patches,310_055,Some(CONTENT),WidgetKind::Label,Some(rice_i18n::t(help)),None);
    } else {
    field_row(
        patches,
        URL_ROW,
        URL_LABEL,
        rice_i18n::t("Server address"),
        URL_FIELD,
        WidgetKind::TextField,
        &dialog.url,
        IconId::Network,
    );
    if dialog.url.starts_with("nfs://") {
        field_row(patches, USER_ROW, USER_LABEL, rice_i18n::t("User ID"), UID_FIELD, WidgetKind::TextField, &dialog.uid, IconId::Application);
        field_row(patches, PASSWORD_ROW, PASSWORD_LABEL, rice_i18n::t("Group ID"), GID_FIELD, WidgetKind::TextField, &dialog.gid, IconId::Application);
    } else {
        field_row(patches, USER_ROW, USER_LABEL, rice_i18n::t("Username"), USER_FIELD, WidgetKind::TextField, &dialog.username, IconId::Application);
        field_row(patches, PASSWORD_ROW, PASSWORD_LABEL, rice_i18n::t("Password"), PASSWORD_FIELD, WidgetKind::PasswordField, &dialog.password, IconId::Lock);
        if dialog.url.starts_with("smb://") {
            field_row(patches, 310_023, 310_024, rice_i18n::t("Domain (optional)"), DOMAIN_FIELD, WidgetKind::TextField, &dialog.domain, IconId::Network);
        }
    }
    if dialog.bridge {
        field_row(patches, 310_028, 310_029, rice_i18n::t("Bridge URL (wss://)"), BRIDGE_URL, WidgetKind::TextField, &dialog.bridge_url, IconId::Network);
        field_row(patches, 310_030, 310_031, rice_i18n::t("Pairing token"), BRIDGE_TOKEN, WidgetKind::PasswordField, &dialog.bridge_token, IconId::Lock);
    }
    }
    if !dialog.error.is_empty() {
        files_view::create(
            patches,
            ERROR,
            Some(CONTENT),
            WidgetKind::Label,
            Some(&dialog.error.render()),
            Some(IconId::Error),
        );
    }
    button(patches, CANCEL, rice_i18n::t("Cancel"), IconId::Close);
    if dialog.pending.is_some() {
        files_view::create(patches,310_052,Some(CONTENT),WidgetKind::Row,Some("panel:compact"),None);
        files_view::create(patches,PROGRESS,Some(310_052),WidgetKind::Spinner,None,None);
        files_view::create_with_state(patches,310_053,Some(310_052),WidgetKind::Label,
            Some(rice_i18n::t(if dialog.cloud { "Connecting to your cloud account…" } else { "Connecting to server…" })),None,WidgetState::ENABLED|WidgetState::EMPHASIZED);
        for id in [URL_FIELD,USER_FIELD,PASSWORD_FIELD,DOMAIN_FIELD,UID_FIELD,GID_FIELD,BRIDGE_URL,BRIDGE_TOKEN,CLOUD_BACK].into_iter().chain(CLOUD_FIELD_FIRST..=CLOUD_FIELD_LAST) {
            if patches.iter().any(|p| matches!(p,Patch::Create{widget,..} if widget.id.get()==id)) {
                patches.push(Patch::Update{id:files_view::widget_id(id),state:Some(WidgetState::empty()),text:rice_ui::ValueChange::Keep,icon:rice_ui::ValueChange::Keep});
            }
        }
    } else {
        button(patches, CONFIRM, rice_i18n::t("Connect"), IconId::Network);
    }
}

#[allow(clippy::too_many_arguments)]
fn field_row(
    patches: &mut Vec<Patch>,
    row: u64,
    label: u64,
    label_text: &str,
    field: u64,
    kind: WidgetKind,
    text: &str,
    icon: IconId,
) {
    files_view::create(patches, row, Some(CONTENT), WidgetKind::Row, Some("panel:compact:label-width=Domain (optional)"), None);
    files_view::create(
        patches,
        label,
        Some(row),
        WidgetKind::Label,
        Some(label_text),
        None,
    );
    files_view::create(patches, field, Some(row), kind, Some(text), Some(icon));
}

fn button(patches: &mut Vec<Patch>, id: u64, text: &str, icon: IconId) {
    files_view::create(
        patches,
        id,
        Some(ACTIONS),
        WidgetKind::Button,
        Some(text),
        Some(icon),
    );
}

fn supported_webdav_location(input: &str) -> Result<ProviderLocation, ConnectionError> {
    let location = ProviderLocation::parse(input.trim())
        .map_err(ConnectionError::InvalidAddress)?;
    let authority = location.authority();
    let loopback = matches!(authority, "localhost" | "127.0.0.1" | "[::1]")
        || authority.starts_with("localhost:")
        || authority.starts_with("127.0.0.1:")
        || authority.starts_with("[::1]:");
    if !matches!(location.scheme(), "https" | "smb" | "nfs") && !(location.scheme() == "http" && loopback) {
        return Err(ConnectionError::Message(
            "Use SMB, NFS, or HTTPS WebDAV (HTTP is allowed only for loopback)",
        ));
    }
    Ok(location)
}

#[cfg(test)]
mod translation_tests {
    use super::*;

    #[test]
    fn connection_errors_refresh_with_locale_and_preserve_host_data() {
        let mut remote = RemoteConnection::default();
        remote.open_url("nfs://server");
        remote.dialog_mut().unwrap().error = ConnectionError::MissingShare("nfs://{example}/Documents".into());
        let original = remote.clone();
        for (locale, expected) in [(1, "Incluya el nombre del recurso compartido o de la exportación, por ejemplo nfs://{example}/Documents"),
                                   (2, "请包含共享或导出名称，例如 nfs://{example}/Documents")] {
            rice_i18n::with_locale(locale, || {
                let mut output = Vec::new(); patches(&mut output, &remote);
                assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.id.get() == ERROR && widget.text.as_deref() == Some(expected))));
                assert_eq!(ConnectionError::Host("Server address {maximum} host diagnostic".into()).render(), "Server address {maximum} host diagnostic");
                for input in ["server", "https://user:secret@server/path", "ftp://server/path"] {
                    let error = supported_webdav_location(input).unwrap_err();
                    let english = rice_i18n::with_locale(0, || error.render());
                    assert_ne!(error.render(), english, "{input}");
                    assert!(!error.render().contains("secret"));
                }
                for address in [true, false] {
                    let error = ConnectionError::TooLong { address, maximum: 4096 };
                    assert!(error.render().contains("4096"));
                    assert_ne!(error.render(), rice_i18n::with_locale(0, || error.render()));
                }
                for input in ["smb://server/Documents", "nfs://server/export", "https://server/path", "http://localhost/path"] {
                    assert!(supported_webdav_location(input).is_ok(), "{input}");
                }
            });
            assert_eq!(remote, original);
        }
    }

    #[test]
    fn remote_labels_translate_while_credentials_and_schemes_remain_data() {
        let mut remote = RemoteConnection::default();
        remote.open_url("smb://server/Documents");
        let dialog = remote.dialog_mut().unwrap();
        dialog.username = "Username {name}".into();
        dialog.password = "Password {password}".into();
        dialog.domain = "Domain".into();
        dialog.bridge = true;
        dialog.bridge_url = "wss://bridge.example".into();
        dialog.bridge_token = "Pairing token {token}".into();
        let original = remote.clone();
        for (locale, labels) in [(1, ["Dirección del servidor", "Nombre de usuario", "Contraseña", "Dominio (opcional)", "URL del puente (wss://)", "Token de emparejamiento", "Cancelar", "Conectar"]),
                                 (2, ["服务器地址", "用户名", "密码", "域（可选）", "桥接 URL（wss://）", "配对令牌", "取消", "连接"])] {
            rice_i18n::with_locale(locale, || {
                let mut output = Vec::new();
                patches(&mut output, &remote);
                for label in labels {
                    assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.text.as_deref() == Some(label))), "missing {label}");
                }
                for (id, expected, kind) in [(URL_FIELD, "smb://server/Documents", WidgetKind::TextField), (USER_FIELD, "Username {name}", WidgetKind::TextField), (PASSWORD_FIELD, "Password {password}", WidgetKind::PasswordField), (BRIDGE_TOKEN, "Pairing token {token}", WidgetKind::PasswordField)] {
                    assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.id.get() == id && widget.kind == kind && widget.text.as_deref() == Some(expected))));
                }
            });
            assert_eq!(remote, original);
        }
        remote.dialog_mut().unwrap().url = "nfs://server/export".into();
        rice_i18n::with_locale(2, || {
            let mut output = Vec::new(); patches(&mut output, &remote);
            for label in ["用户 ID", "组 ID"] {
                assert!(output.iter().any(|p| matches!(p, Patch::Create {widget, ..} if widget.text.as_deref() == Some(label))));
            }
        });
    }
}
