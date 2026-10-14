use crate::{AppId, WindowId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostRequest {
    pub app: AppId,
    pub window: WindowId,
    pub operation: HostOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostOperation {
    /// Request bounded client geometry for this app-owned window.
    SetContentSize { token: u64, width: u32, height: u32, anchor: u8 },
    /// Fence this app's data requests and drain their underlying work. Resume
    /// requires the exact acknowledged token; it starts a fresh response scope.
    DataRequestFence { token: u64, resume: bool },
    /// Paddy account commands. The host runs the isolated WhatsApp Wasm engine;
    /// this bounded surface contains presentation commands, never protocol keys.
    PaddyWhatsApp { payload: String },
    SetFullscreen(bool),
    SetTimer {
        token: u64,
        interval_millis: u32,
        enabled: bool,
    },
    /// Subscribe this native window to a RiceBus topic. Filters are opaque to
    /// the protocol and interpreted by the host-owned topic publisher. Sending
    /// an empty filter list releases the subscription.
    RiceBusSubscribe {
        topic: String,
        filters: Vec<String>,
    },
    PrintText {
        title: String,
        text: String,
    },
    /// Download one bounded, app-authored UTF-8 report through the browser.
    DownloadText {
        filename: String,
        mime_type: String,
        text: String,
    },
    /// Save one bounded binary file supplied by a native application. The
    /// runtime chooses a tenant-owned destination inside the guest filesystem;
    /// these bytes must not escape through the containing browser's download UI.
    DownloadFile {
        filename: String,
        mime_type: String,
        data: Vec<u8>,
    },
    /// Download a guest file without copying it through the bounded app wire.
    DownloadPath {
        path: String,
        filename: String,
        show_storage_tip: bool,
    },
    /// Reveal a file previously saved by `DownloadFile`, or its containing
    /// Downloads folder. The host owns the final collision-safe VFS path.
    RevealDownload {
        filename: String,
        mime_type: String,
        open_file: bool,
    },
    /// Copy one bounded encoded image to the containing system clipboard.
    CopyImage {
        mime_type: String,
        data: Vec<u8>,
    },
    /// Atomically save a bounded file in guest Downloads and launch its
    /// registered default MIME application after persistence completes.
    SaveAndOpenFile {
        filename: String,
        mime_type: String,
        data: Vec<u8>,
    },
    /// Replace the host-owned titlebar text for this native window.
    SetTitle(String),
    /// Show or hide the host-owned titlebar while retaining the outer window
    /// frame. A hidden titlebar gives its height back to the app surface.
    SetTitlebarVisible(bool),
    /// Minimize this native window through the host window manager.
    MinimizeWindow,
    /// Toggle this native window between maximized and restored geometry.
    ToggleMaximizeWindow,
    /// Begin a host window move from app-local pointer coordinates. The host
    /// owns all subsequent global-pointer tracking and snap behavior.
    StartWindowDrag {
        x: i32,
        y: i32,
    },
    /// Present one standard shell notification and retain it in shell history.
    ShowNotification {
        kind: String,
        title: String,
        message: String,
    },
    /// App-owned chat notification, update, or action result. The host validates
    /// the bounded JSON schema and sends user actions only back to this app.
    InteractiveNotification { payload: String },
    /// Open one explicit HTTPS destination in the containing browser. The
    /// host validates the URL again and uses a separate tab with no opener.
    OpenExternalUrl(String),
    RequestClose,
    /// Complete the standard guest-VFS chooser for this window.
    FileDialogResult { path: Option<String> },
    /// Complete a retained native popup owned by this window.
    MenuResult { id: Option<String> },
    /// Control one software-decoded Rice media session. The session is owned
    /// by the host and never exposes decoder memory to application actors.
    MediaControl {
        session: u64,
        command: MediaCommand,
        value: i64,
    },
    /// Open a software-decoded media path for this window. The host opens the
    /// replacement before retiring the previous session, so a failed playlist
    /// item leaves the current movie intact.
    MediaOpen {
        replace_session: u64,
        path: String,
    },
    /// Open a persistent media session muted from its first audio sample.
    MediaOpenMuted {
        replace_session: u64,
        path: String,
    },
    /// Execute one bounded MSE/EME command in the trusted browser host. The
    /// guest retains page and license-network semantics; CDM objects, decoder
    /// state, and protected presentation surfaces never enter guest memory.
    ProtectedMedia {
        request: u64,
        handle: u64,
        action: String,
        metadata: String,
        data: Vec<u8>,
    },
    /// Execute one WebSocket lifecycle command through the selected guest
    /// network transport. The browser host owns the live socket while Bowl
    /// retains standards-facing object and event state in the originating realm.
    WebSocket {
        tab: u64,
        realm: u64,
        handle: u64,
        action: u8,
        metadata: String,
        data: Vec<u8>,
    },
    /// Ask the containing browser for its current position, or begin a
    /// browser-owned position watch. The options are bounded JSON produced by
    /// Bowl's standards shim; `tab` keeps equal realm-local request IDs apart.
    Geolocation {
        tab: u64,
        request: u64,
        watch: bool,
        options: String,
    },
    /// Stop a browser-owned geolocation watch previously started for a Bowl
    /// page realm.
    ClearGeolocation {
        tab: u64,
        request: u64,
    },
    /// Execute page JavaScript in a capability-confined host-engine realm.
    /// The realm has no ambient network or durable storage access; those APIs
    /// are replaced by the RiceWM network and VFS brokers before page code is
    /// admitted. `source` is UTF-8 document or script text.
    JavaScriptRealm {
        request: u64,
        tab: u64,
        realm: u64,
        action: String,
        url: String,
        source: Vec<u8>,
    },
    /// Export a decoded image surface without copying its pixels through the
    /// bounded application wire protocol.
    ImageExport {
        session: u64,
        action: ImageExportAction,
        width: u16,
        height: u16,
        quarter_turns: u8,
        preserve_original: bool,
        /// Source-pixel crop applied before rotation and resizing.
        crop: Option<[u16; 4]>,
        filename: String,
    },
    /// Request a bounded, host-authored snapshot for the system monitor.
    PerformanceSnapshot,
    /// Measure the browser-selected graphics, compute, and inference paths.
    AccelerationDiagnostics,
    /// Deliver a conventional POSIX signal to a guest process.
    ProcessSignal {
        pid: u32,
        signal: u8,
    },
    /// Change the scheduling niceness recorded for a guest process.
    ProcessNice {
        pid: u32,
        nice: i8,
    },
    /// Send one bounded request to the host-owned APT service. The native
    /// application owns presentation; the host owns package execution.
    AppStore {
        payload: String,
    },
    /// Send one bounded request to the host-owned GNOME 50 extension service.
    ExtensionStore {
        payload: String,
    },
    /// Query or mutate the shell-owned calendar server. Calendar providers are
    /// merged by the host so the tray and applications share one event model.
    CalendarStore {
        payload: String,
    },
    /// Query or mutate the host-owned local mail service. Provider credentials,
    /// network sessions, and the encrypted repository never cross this app
    /// protocol; only bounded commands and presentation records do.
    MailStore {
        payload: String,
    },
    /// Query or mutate the host-owned encrypted contact service. Provider
    /// credentials and synchronization remain outside the application actor.
    ContactStore {
        payload: String,
    },
    /// Control one headless guest TTY owned by a native Rice Terminal tab.
    Terminal {
        session: u64,
        action: TerminalAction,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalAction {
    /// Open the bundled native-Wasm xCodex engine in the existing terminal grid.
    OpenXCodex {
        columns: u16,
        rows: u16,
    },
    /// Open a headless parser feeding a native retained terminal grid.
    OpenGrid {
        columns: u16,
        rows: u16,
    },
    /// Open a new grid session with an explicit, bounded guest memory budget.
    OpenGridWithMemory {
        columns: u16,
        rows: u16,
        memory_mib: u16,
    },
    /// Scroll the native grid viewport; zero returns to live output.
    Scroll(i32),
    /// Copy at most the last 2000 physical buffer lines to the system clipboard.
    CopyAll,
    Appearance(String),
    /// Native Terminal agent controls; bounded JSON handled by its session host.
    Assistant(String),
    Open {
        columns: u16,
        rows: u16,
    },
    Input(Vec<u8>),
    Resize {
        columns: u16,
        rows: u16,
    },
    Close,
    Configure {
        copy_on_select: bool,
        transparency: u8,
    },
    /// Ask the containing host page to maximize this TTY and enter the
    /// browser Fullscreen API in one user-initiated operation.
    SuperFullscreen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MediaCommand {
    TogglePause = 0,
    SeekMillis = 1,
    SetVolume = 2,
    ToggleMute = 3,
    Stop = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ImageExportAction {
    Copy = 0,
    SaveAs = 1,
    SaveTenant = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostEvent {
    pub app: AppId,
    pub event: HostEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostEventKind {
    /// Authoritative, request-scoped resize completion, including clamping.
    WindowSizeResult { window: WindowId, token: u64, width: u32, height: u32, error: String },
    DataRequestFence { token: u64, resume: bool, ok: bool },
    /// Normalized presentation events from Paddy's isolated WhatsApp engine.
    PaddyWhatsApp { payload: String },
    Timer {
        token: u64,
    },
    /// One bounded RiceBus event delivered through a native app subscription.
    RiceBus {
        topic: String,
        payload: String,
    },
    WindowState {
        window: WindowId,
        content_width: u32,
        content_height: u32,
        fullscreen: bool,
        maximized: bool,
        focused: bool,
        visible: bool,
    },
    /// Completion of an asynchronous [`HostOperation::MediaOpen`] request.
    MediaOpened {
        path: String,
        session: u64,
        duration_millis: u64,
        error: String,
    },
    /// The decoder, rather than the UI's presentation clock, reached EOF.
    MediaEnded {
        session: u64,
    },
    /// Completion or asynchronous CDM/media event for a protected-media
    /// object. A zero request denotes an unsolicited object event.
    ProtectedMedia {
        request: u64,
        ok: bool,
        handle: u64,
        event: String,
        metadata: String,
        data: Vec<u8>,
    },
    /// Completion or asynchronous message/error/close event for a Bowl
    /// WebSocket, routed to the exact page, frame, or worker realm that owns it.
    WebSocket {
        tab: u64,
        realm: u64,
        handle: u64,
        event: String,
        metadata: String,
        data: Vec<u8>,
    },
    /// A host-browser Geolocation API success or error serialized as bounded
    /// JSON and routed back to the requesting Bowl page realm.
    Geolocation {
        tab: u64,
        request: u64,
        payload: String,
    },
    /// Completion or a coalesced DOM snapshot from a capability-confined
    /// host-native Bowl JavaScript realm.
    JavaScriptRealm {
        request: u64,
        tab: u64,
        realm: u64,
        event: String,
        metadata: String,
        source: Vec<u8>,
    },
    /// Tab/newline-delimited, bounded performance data owned by the host.
    PerformanceSnapshot {
        payload: String,
    },
    /// One bounded, host-measured acceleration report for Settings.
    AccelerationDiagnostics {
        payload: String,
    },
    /// One bounded catalog, progress, log, or completion update from APT.
    AppStore {
        payload: String,
    },
    /// One bounded catalog, install, preference, or compatibility update.
    ExtensionStore {
        payload: String,
    },
    /// One bounded snapshot or mutation result from the calendar server.
    CalendarStore {
        payload: String,
    },
    /// One bounded snapshot or command result from the local mail service.
    MailStore {
        payload: String,
    },
    /// One bounded snapshot or mutation result from the contact service.
    ContactStore {
        payload: String,
    },
    /// Output or lifecycle state from one headless native terminal session.
    Terminal {
        session: u64,
        event: TerminalEvent,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminalEvent {
    /// Bounded JSON grid presentation, validated by the native terminal actor.
    Grid(String),
    Appearance(String),
    Paste(String),
    GridSize {
        columns: u16,
        rows: u16,
    },
    /// A bounded terminal screen produced by the existing headless xterm
    /// engine. Native actors render this state; the host never owns a DOM
    /// surface for it.
    Screen(TerminalScreen),
    Ready,
    Exited {
        code: i32,
    },
    Preferences {
        copy_on_select: bool,
        transparency: u8,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalScreen {
    pub text: String,
    pub caret: u32,
    pub columns: u16,
    pub rows: u16,
    pub application_cursor: bool,
    pub application_keypad: bool,
    pub bracketed_paste: bool,
}
