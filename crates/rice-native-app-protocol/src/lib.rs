//! Versioned, bounded messages exchanged by `RiceWM`'s native application host.
//!
//! The protocol deliberately uses a small hand-written codec. This keeps the
//! Wasm dependency graph small and makes every allocation limit visible at the
//! trust boundary.

mod application;
mod host;
mod lifecycle;
mod mount;
mod transfer;
mod vfs;
mod wire;

pub use application::{
    APPLICATION_CATALOG_VERSION, InstalledApplication, MAX_APPLICATION_ID_BYTES, MAX_APPLICATIONS,
};
pub use host::{
    HostEvent, HostEventKind, HostOperation, HostRequest, ImageExportAction, MediaCommand,
    TerminalAction, TerminalEvent, TerminalScreen,
};
pub use lifecycle::{CloseDecision, CloseReason, CloseRequest, CloseResponse};
pub use mount::{MAX_MOUNTS, MountCapabilities, MountEntry, MountState};
pub use transfer::{
    ClipboardFormat, ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection,
    DragAction, DragEnvelope, DragPayload, DragPhase, DragSourceEvent, DragSourceResult, DragSourceState,
};
pub use vfs::{
    ArchiveFormat, DirectoryEntry, FileKind, FileMetadata, VfsError, VfsErrorCode, VfsOperation,
    VfsProgress, VfsRequest, VfsResponse, VfsResult,
};

use std::fmt;

pub use wire::{DecodeError, EncodeError};

pub const PROTOCOL_VERSION: u16 = 1;
// HTTP downloads include framing and metadata in addition to the body.
pub const MAX_MESSAGE_BYTES: usize = 72 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 16 * 1024;
/// Wide terminal rows need more space than ordinary control labels.
pub const MAX_WIDGET_PATCH_TEXT_BYTES: usize = 64 * 1024;
/// Negotiated large VirtualGrid frames; ordinary widget text retains its bound.
pub const MAX_GRID_PATCH_TEXT_BYTES: usize = 256 * 1024;
pub const MAX_TERMINAL_GRID_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PATH_BYTES: usize = 4 * 1024;
pub const MAX_ARGUMENTS: usize = 128;
pub const MAX_PATCHES: usize = 4_096;
pub const MAX_DIRECTORY_ENTRIES: usize = 4_096;
pub const MAX_TRANSFER_BYTES: usize = 512 * 1024;
/// Clipboard text and diagnostic reports routinely exceed the generic
/// drag/drop transfer ceiling on script-heavy pages. Keep this separate so
/// raising it does not enlarge every other provider allocation boundary.
pub const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_CLIPBOARD_BYTES: usize = MAX_DIAGNOSTIC_BYTES;
pub const MAX_MIME_TYPES: usize = 64;
pub const MAX_MIME_TYPE_BYTES: usize = 255;
/// Settings are catalog-backed and remain bounded at the native-app trust
/// boundary. Workspace layouts and shortcuts take the built-in catalog beyond
/// 128 entries; leave bounded room for native-provider settings as well.
pub const MAX_SETTINGS: usize = 512;
/// Media providers may stream large host-backed files. This field is retained
/// as a caller policy bound, not as a legacy whole-file preview ceiling.
pub const MAX_MEDIA_SOURCE_BYTES: u64 = u64::MAX - 1;
pub const MAX_MEDIA_DIMENSION: u16 = 4096;
pub const MAX_WEB_DISPLAY_LIST_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_HTTP_BODY_BYTES: usize = 64 * 1024 * 1024;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            /// Constructs an ID. Zero is reserved for "not present" on the wire.
            pub const fn new(value: u64) -> Option<Self> {
                if value == 0 { None } else { Some(Self(value)) }
            }

            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

id_type!(AppId);
id_type!(WindowId);
id_type!(WidgetId);
id_type!(RequestId);
id_type!(DragId);

/// Capabilities are additive bits negotiated in [`Message::HostHello`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostCapabilities(u64);

impl HostCapabilities {
    pub const CLIPBOARD: Self = Self(1 << 0);
    pub const DRAG_AND_DROP: Self = Self(1 << 1);
    pub const FILE_WATCH: Self = Self(1 << 2);
    pub const IMAGE_DECODE: Self = Self(1 << 3);
    pub const PRINT: Self = Self(1 << 4);
    pub const PROCESS_LAUNCH: Self = Self(1 << 5);
    pub const ENTROPY: Self = Self(1 << 6);
    pub const CURRENCY_RATES: Self = Self(1 << 7);
    pub const MOUNTS: Self = Self(1 << 8);
    pub const NETWORK: Self = Self(1 << 9);
    pub const GRID_DAMAGE: Self = Self(1 << 10);
    pub const GRID_FILL_ENDPOINT: Self = Self(1 << 11);
    pub const GRID_TEXT_STYLE: Self = Self(1 << 12);
    pub const WIDGET_SIZE: Self = Self(1 << 13);
    pub const GRID_LARGE_FRAME: Self = Self(1 << 14);
    pub const GRID_FILTER_ROW: Self = Self(1 << 15);
    /// Bounded transient formula-reference outlines.
    pub const GRID_REFERENCES: Self = Self(1 << 16);
    /// Exact single-line caret control and selection observation.
    pub const TEXT_FIELD_SELECTION: Self = Self(1 << 17);
    pub const TEXT_FIELD_DOCUMENT_FONT: Self = Self(1 << 18);
    pub const GRID_PAINT: Self = Self(1 << 19);
    pub const GRID_ZOOM: Self = Self(1 << 20);
    /// Document grid canvas and effective inline editor paint.
    pub const GRID_DOCUMENT_CANVAS: Self = Self(1 << 21);
    /// Lossless fractional document font points in grids and inline editors.
    pub const GRID_FRACTIONAL_FONT: Self = Self(1 << 22);
    /// Dedicated semantic zoom icons (516/517), avoiding destructive imagery.
    /// Merged spreadsheet geometry, hit targets and changed-cell presentation.
    pub const GRID_MERGES: Self = Self(1 << 24);
    /// Optional top/center/bottom placement in native document text fields.
    pub const DOCUMENT_ALIGNMENT: Self = Self(1 << 25);
    pub const SHEET_DATA_ICONS: Self = Self(1 << 26);
    /// Affine content-alpha shadow scopes and their semantic controls.
    pub const ICON_SELECTORS: Self = Self(1 << 28);
    pub const CONTENT_SHADOWS: Self = Self(1 << 27);
    pub const ZOOM_ICONS: Self = Self(1 << 23);
    /// Scoped typography and ink on native labels/actions.
    pub const CONTROL_TYPOGRAPHY: Self = Self(1 << 29);

    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn from_bits_retain(bits: u64) -> Self {
        Self(bits)
    }
}

impl std::ops::BitOr for HostCapabilities {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Message {
    HostHello {
        capabilities: HostCapabilities,
        max_message_bytes: u32,
    },
    Launch {
        app: AppId,
        entrypoint: String,
        arguments: Vec<String>,
    },
    Open {
        app: AppId,
        paths: Vec<String>,
    },
    Close {
        app: AppId,
        reason: CloseReason,
    },
    CloseRequest(CloseRequest),
    CloseResponse(CloseResponse),
    VfsRequest(VfsRequest),
    VfsResponse(VfsResponse),
    MediaRequest(MediaRequest),
    MediaResponse(MediaResponse),
    UiPatch(UiPatchEnvelope),
    UiEvent(UiEventEnvelope),
    WebRenderPatch(WebRenderPatch),
    ClipboardRequest(ClipboardRequest),
    ClipboardResponse(ClipboardResponse),
    Drag(DragEnvelope),
    DragSource(DragSourceEvent),
    SettingsRequest(SettingsRequest),
    SettingsResponse(SettingsResponse),
    DataProviderRequest(DataProviderRequest),
    DataProviderResponse(DataProviderResponse),
    HostRequest(HostRequest),
    HostEvent(HostEvent),
}

/// One complete retained web-scene replacement produced by an in-Wasm engine.
///
/// The byte stream is validated by `rice-web-render-protocol`; it contains
/// typed paint primitives and cannot contain a framebuffer operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WebRenderPatch {
    pub app: AppId,
    pub window: WindowId,
    pub surface: WidgetId,
    pub epoch: u64,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub display_list: Vec<u8>,
}

pub const MAX_ENTROPY_BYTES: usize = 64;
pub const MAX_HTTP_REQUEST_BODY_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_HTTP_HEADERS: usize = 32;
pub const MAX_HTTP_HEADER_NAME_BYTES: usize = 64;
pub const MAX_HTTP_HEADER_VALUE_BYTES: usize = 16 * 1024;
pub const MAX_HTTP_HEADER_BLOCK_BYTES: usize = 6 * 1024;
/// Rich browser-network response metadata carried alongside a bounded body.
pub const MAX_HTTP_METADATA_BYTES: usize = 128 * 1024;
pub const MAX_HTTP_TIMEOUT_MILLIS: u32 = 5 * 60 * 1_000;
pub const MAX_AUTH_PROVIDER_ID_BYTES: usize = 64;
pub const MAX_AUTH_INTERACTION_BYTES: usize = 255;
pub const MAX_DOCUMENT_OUTLINE_ENTRIES: usize = 512;
pub const MAX_DOCUMENT_SEARCH_RESULTS: usize = 128;
pub const MAX_DOCUMENT_PAGES: u32 = 1_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProviderRequest {
    pub request: RequestId,
    pub app: AppId,
    pub operation: DataProviderOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataProviderOperation {
    Entropy {
        byte_len: u16,
    },
    CurrencyRate {
        base_iso4217: String,
        quote_iso4217: String,
    },
    MountList,
    MountConnectLocal,
    MountSync {
        mount_id: String,
    },
    MountDisconnect {
        mount_id: String,
    },
    MountReconnect {
        mount_id: String,
    },
    HttpGet {
        url: String,
    },
    HttpRequest {
        url: String,
        method: String,
        content_type: String,
        cookie: String,
        body: Vec<u8>,
    },
    /// A generic bounded HTTP request for native applications. Unlike the
    /// legacy Bowl request, this form carries ordinary request headers and a
    /// per-request deadline. The provider strips credentials on cross-origin
    /// redirects and never exposes host-browser cookies or storage.
    HttpRequestV2 {
        url: String,
        method: String,
        headers: Vec<HttpHeader>,
        body: Vec<u8>,
        timeout_millis: u32,
        response_body_limit: u32,
    },
    /// A browser-owned HTTP request. Bowl supplies its isolated guest cookie
    /// jar separately from ordinary script headers so pages can use standard
    /// XHR/fetch headers without gaining access to host-browser credentials.
    HttpRequestV3 {
        url: String,
        method: String,
        headers: Vec<HttpHeader>,
        cookie: String,
        body: Vec<u8>,
    },
    /// Starts an interactive authorization session owned by the central Rice
    /// authentication service. The containing application supplies no browser
    /// credentials: it receives only the matched callback URL after the guest
    /// Bowl session reaches the declared callback boundary.
    Authorization {
        application_id: String,
        provider_id: String,
        entry_url: String,
        callback_mode: String,
        callback_url: String,
        interaction_kind: String,
        interaction_value: String,
    },
    ApplicationCatalog {
        mime_type: String,
    },
    ApplicationLaunch {
        application_id: String,
        mime_type: String,
        paths: Vec<String>,
        remember_default: bool,
    },
    MountConnectWebDav {
        url: String,
        username: String,
        password: String,
    },
    MountConnectNetwork {
        url: String,
        username: String,
        password: String,
        domain: String,
        bridge_url: String,
        bridge_token: String,
        uid: u32,
        gid: u32,
    },
    /// Provider catalog is host supplied, so Files does not encode vendor names.
    MountCloudProviders,
    MountConnectCloud { provider_id: String, account_id: String },
    MountConnectCloudConfigured { provider_id: String, account_id: String, config_json: String },
    MountDescribeNetwork { mount_id: String },
    MountForgetNetwork { mount_id: String },
    MountCancelConnect {
        connect_request: RequestId,
    },
    /// Reads the bounded page count and flattened table of contents for a PDF.
    DocumentInfo {
        path: String,
    },
    /// Searches extracted PDF page text without moving document bytes into Wasm.
    DocumentSearch {
        path: String,
        query: String,
        maximum_results: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataProviderResponse {
    pub request: RequestId,
    pub result: DataProviderResult,
}

/// One complete Set-Cookie response field collected from an HTTP response.
/// Browsers commit Set-Cookie on intermediate responses as well as the final
/// response, so collapsing a redirect chain must retain the complete response
/// URL for Domain and default-Path processing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginCookie {
    pub origin: String,
    pub cookie: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataProviderResult {
    Entropy(Vec<u8>),
    CurrencyRate {
        decimal_rate: String,
        observed_millis: u64,
        source: String,
    },
    Mounts {
        entries: Vec<MountEntry>,
        can_connect_local: bool,
    },
    HttpResponse {
        final_url: String,
        status: u16,
        content_type: String,
        cookie: String,
        origin_cookies: Vec<OriginCookie>,
        redirect_trace: String,
        body: Vec<u8>,
    },
    Applications {
        schema_version: u16,
        mime_type: String,
        entries: Vec<InstalledApplication>,
    },
    ApplicationLaunched {
        application_id: String,
        default_changed: bool,
    },
    MountConnected(MountEntry),
    NetworkConnectionInfo { json: String },
    CloudProviders { json: String },
    DocumentInfo {
        page_count: u32,
        outline: Vec<DocumentOutlineEntry>,
    },
    DocumentSearch {
        query: String,
        hits: Vec<DocumentSearchHit>,
        truncated: bool,
    },
    Authorization {
        callback_url: String,
    },
    Unavailable {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentOutlineEntry {
    pub title: String,
    pub page: u32,
    pub depth: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentSearchHit {
    pub page: u32,
    pub snippet: String,
}

impl Message {
    /// Encodes one complete framed message.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] when a field exceeds its protocol limit or a
    /// value cannot be represented canonically.
    pub fn encode(&self) -> Result<Vec<u8>, EncodeError> {
        wire::encode_message(self)
    }

    /// Decodes exactly one complete framed message.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] for malformed, unsupported, non-canonical, or
    /// over-limit input.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        wire::decode_message(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsRequest {
    pub request: RequestId,
    pub app: AppId,
    pub operation: SettingsOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsOperation {
    Load,
    InspectFileSync { path: String },
    Set { key: String, value: String },
    PrepareRecycle,
    Recycle { confirmation: String },
    ExportPreferences,
    ImportPreferences { document: String },
    ApplyWallpaperTheme { document: String },
    CreateWallpaperTheme { name: String, path: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingEntry {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettingsResponse {
    pub request: RequestId,
    pub result: SettingsResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SettingsResult {
    Values(Vec<SettingEntry>),
    Applied(SettingEntry),
    RecyclePrepared(String),
    Restarting,
    PreferencesDocument(String),
    WallpaperThemeRecipe(String),
    Error(String),
}

/// Requests a bounded browser-native decode of one guest file.
///
/// Pixel/frame data remains in the trusted host cache keyed by `request`; the
/// response carries only verified metadata back across the Wasm boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaRequest {
    pub request: RequestId,
    pub app: AppId,
    pub path: String,
    pub mime_type: String,
    pub maximum_source_bytes: u64,
    pub target_width: u16,
    pub target_height: u16,
    pub frame_millis: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaResponse {
    pub request: RequestId,
    pub result: MediaResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MediaResult {
    Ready(MediaMetadata),
    Error(MediaError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MediaKind {
    Image = 0,
    Video = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaMetadata {
    pub kind: MediaKind,
    pub width: u16,
    pub height: u16,
    pub duration_millis: Option<u64>,
    pub mime_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MediaErrorCode {
    NotFound = 0,
    TooLarge = 1,
    Unsupported = 2,
    DecodeFailed = 3,
    InvalidInput = 4,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaError {
    pub code: MediaErrorCode,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiPatchEnvelope {
    pub app: AppId,
    pub window: WindowId,
    pub sequence: u64,
    pub patches: Vec<UiPatch>,
}

/// A patch field that can remain unchanged, be replaced, or be cleared.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatchValue<T> {
    Keep,
    Set(T),
    Clear,
}

/// Toolkit-neutral retained-tree mutations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiPatch {
    Create {
        id: WidgetId,
        parent: Option<WidgetId>,
        index: u32,
        kind: u16,
        state: u32,
        text: Option<String>,
        icon: Option<u16>,
    },
    Update {
        id: WidgetId,
        state: PatchValue<u32>,
        text: PatchValue<String>,
        icon: PatchValue<u16>,
    },
    Move {
        id: WidgetId,
        parent: Option<WidgetId>,
        index: u32,
    },
    Remove {
        id: WidgetId,
    },
    Clear,
    /// Negotiated, bounded VirtualGrid damage JSON; never ordinary widget text.
    GridDamage { id: WidgetId, damage: String },
    /// Negotiated subscription to resolved native widget bounds.
    ObserveSize { id: WidgetId, enabled: bool },
    /// Unicode scalar offsets; also opts this field into TextSelection events.
    EditSelection { id: WidgetId, anchor: u32, caret: u32 },
    DocumentFont { id: WidgetId, font: Option<String> },
    /// Stable theme override selector such as app:native:rice-files or mime:application/pdf.
    IconSelector { id: WidgetId, selector: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiEventEnvelope {
    pub app: AppId,
    pub window: WindowId,
    pub target: WidgetId,
    pub event: UiEvent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiEvent {
    Activate,
    ValueChanged(String),
    Pointer {
        kind: PointerEventKind,
        x: i32,
        y: i32,
        button: u8,
        modifiers: u16,
    },
    Key {
        pressed: bool,
        key_code: u32,
        modifiers: u16,
        text: Option<String>,
    },
    Focus {
        focused: bool,
    },
    /// Request a complete presentation snapshot; never replay application commands.
    GridResync,
    /// Changed drawable size after native layout, in CSS pixels.
    SizeChanged { width: u32, height: u32 },
    /// Character-based selection offsets produced by a native text surface.
    TextSelection {
        anchor: u32,
        caret: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum PointerEventKind {
    Move = 0,
    Down = 1,
    Up = 2,
    Enter = 3,
    Leave = 4,
    Wheel = 5,
}
