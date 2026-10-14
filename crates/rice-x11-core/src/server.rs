#[path = "server_scrolling_workspaces.rs"]
mod scrolling_workspaces;
#[path = "server_cursor.rs"]
mod cursor;
#[path = "server_output_layout.rs"]
mod output_layout;
#[path = "server_native_fullscreen.rs"]
mod native_fullscreen;
#[path = "server_topbar.rs"]
mod topbar;
use crate::tiling::{Axis as TileAxis, Rect as TileRect, TileLayout};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::Cursor,
};

mod redmond_task_icons {
    include!("public_task_icons.rs");
}

use rice_tray::{
    DEFAULT_TRAY_LAYOUT, LauncherTarget, MAX_TRAY_ITEMS, TrayItem, TrayPluginRegistry,
};
use yserver_protocol::x11::{
    self, AtomId, ClientByteOrder, Geometry, ResourceId, Screen, SequenceNumber, SetupSuccess,
    WindowAttributes, composite as x11composite, damage as x11damage, randr as x11randr,
    request_lengths::{
        exact_required_length, invalid_value, invalid_value_mask, validate_core_request_length,
    },
    shape as x11shape, sync as x11sync, xfixes as x11xfixes, xinerama as x11xinerama,
    xtest as x11xtest,
};

use crate::{
    model::{
        AtomTable, Colormap, Cursor as RiceCursor, Font, Glyph, GlyphSet, GradientStop,
        GraphicsContext, MAX_PROPERTY_BYTES, Picture, PictureFilter, PictureSource, Pixmap,
        Property, Surface, SurfaceAlphaMode, Window,
    },
    scene::{
        SCENE_COMMAND_BYTES, SCENE_OWNED_WINDOW_STACK_FLAG, SCENE_RESIZABLE_WINDOW_STACK_FLAG,
        SceneQueue,
    },
};

const ROOT_WINDOW: ResourceId = ResourceId(0x100);
const ROOT_COLORMAP: ResourceId = ResourceId(0x101);
const ROOT_VISUAL: ResourceId = ResourceId(0x102);
const ARGB_VISUAL: ResourceId = ResourceId(0x103);
const RESOURCE_ID_MASK: u32 = 0x001f_ffff;
const MAX_RESOURCE_SLOTS: u16 = 255;
const SETUP_HEADER_BYTES: usize = 12;
const MAX_SETUP_BYTES: usize = 64 * 1024;
const RANDR_MAJOR_OPCODE: u8 = 128;
const RENDER_MAJOR_OPCODE: u8 = 133;
const BIG_REQUESTS_MAJOR_OPCODE: u8 = 135;
const XKB_MAJOR_OPCODE: u8 = 136;
const XINPUT_MAJOR_OPCODE: u8 = 137;
const GENERIC_EVENT_MAJOR_OPCODE: u8 = 138;
const XFIXES_MAJOR_OPCODE: u8 = 140;
const SHAPE_MAJOR_OPCODE: u8 = 141;
const SYNC_MAJOR_OPCODE: u8 = 142;
const DAMAGE_MAJOR_OPCODE: u8 = 143;
const COMPOSITE_MAJOR_OPCODE: u8 = 144;
const XTEST_MAJOR_OPCODE: u8 = 146;
const XINERAMA_MAJOR_OPCODE: u8 = 151;
const XCMISC_MAJOR_OPCODE: u8 = 152;
const RANDR_FIRST_EVENT: u8 = 89;
const RANDR_FIRST_ERROR: u8 = 147;
const XFIXES_FIRST_EVENT: u8 = 87;
const XFIXES_FIRST_ERROR: u8 = 163;
const SHAPE_FIRST_EVENT: u8 = 64;
const SYNC_FIRST_EVENT: u8 = 83;
const SYNC_FIRST_ERROR: u8 = 164;
const DAMAGE_FIRST_EVENT: u8 = 91;
const DAMAGE_FIRST_ERROR: u8 = 167;
const XINPUT_FIRST_EVENT: u8 = 66;
const XINPUT_FIRST_ERROR: u8 = 157;
const XKB_FIRST_EVENT: u8 = 85;
const XKB_FIRST_ERROR: u8 = 162;
const RANDR_CRTC: u32 = 0xe200_0001;
const RANDR_OUTPUT: u32 = 0xe200_0002;
const RANDR_MODE: u32 = 0xe200_0003;
const WM_TASKBAR: ResourceId = ResourceId(0xe000_0001);
const WM_MENU: ResourceId = ResourceId(0xe000_0002);
const WM_CHECK: ResourceId = ResourceId(0xe000_0003);
const WM_NOTIFICATION: ResourceId = ResourceId(0xe000_0004);
const WM_XSETTINGS: ResourceId = ResourceId(0xe000_0005);
const WM_TASK_PREVIEW: ResourceId = ResourceId(0xe000_0006);
const WM_SNAP_PREVIEW: ResourceId = ResourceId(0xe000_0007);
const WM_GNOME_PANEL: ResourceId = ResourceId(0xe000_0008);
const WM_GNOME_MENU: ResourceId = ResourceId(0xe000_0009);
const WM_GNOME_DIALOG: ResourceId = ResourceId(0xe000_000a);
const WM_TOOLTIP: ResourceId = ResourceId(0xe000_000b);
const WM_GNOME_BACKGROUND: ResourceId = ResourceId(0xe000_000c);
const WM_FIRST_FRAME: u32 = 0xe100_0000;
const COMPOSITE_OVERLAY: ResourceId = ResourceId(0xe300_0001);
const WM_TITLE_HEIGHT: u16 = 24;
const WM_COMPACT_DOCK_SURFACE_THICKNESS: u16 = 72;
const WM_COMPACT_DOCK_BAR_THICKNESS: usize = 44;
const WM_COMPACT_DOCK_AXIS_PADDING: usize = 12;
const WM_COMPACT_DOCK_STEP: usize = 44;
const WM_COMPACT_DOCK_ICON_SIZE: usize = 32;
const WM_COMPACT_DOCK_ICON_MAX_SIZE: usize = 64;
const WM_COMPACT_DOCK_ICON_INSET: usize = 8;
// Shared Rice windows paint a one-device-pixel theme rim. Native X11 clients
// use the same compositor-owned frame geometry so Bowl and Debian apps do not
// acquire a second, heavier desktop-only outline.
const WM_BORDER_WIDTH: u16 = 1;
// Keep the painted frame slim while exposing a practical resize target inside
// the client edge. Borderless host windows (including Bowl) otherwise leave
// only the single painted frame pixel available to a physical pointer.
const WM_RESIZE_HIT_WIDTH: i32 = 7;
const WM_RESIZE_LEFT: u8 = 1 << 0;
const WM_RESIZE_RIGHT: u8 = 1 << 1;
const WM_RESIZE_TOP: u8 = 1 << 2;
const WM_RESIZE_BOTTOM: u8 = 1 << 3;
const WM_TASKBAR_HEIGHT: u16 = 32;
const WM_TITLE_BUTTON_WIDTH: u16 = 22;
const WM_TITLE_BUTTON_COUNT: u16 = 3;
const WM_MENU_ROW_HEIGHT: usize = 36;
const REDMOND_MENU_ROW_HEIGHT: usize = 32;
const WM_MENU_DIVIDER_HEIGHT: usize = 11;
const WM_GNOME_MENU_WIDTH: u16 = 360;
const WM_GNOME_MENU_ROW_HEIGHT: usize = 36;
const WM_GNOME_MENU_IMAGE_HEIGHT: usize = 264;
const WM_START_BUTTON_WIDTH: i16 = 112;
#[cfg(test)]
const WM_QUICK_LAUNCH_X: i16 = 120;
const WM_QUICK_LAUNCH_GAP: i16 = 8;
#[cfg(test)]
const WM_TASK_BUTTON_X: i16 = 212;
const WM_MENU_ROOT_WIDTH: usize = 230;
const WM_MENU_CATEGORY_WIDTH: usize = 210;
const WM_MENU_APPLICATION_WIDTH: usize = 360;
const WM_MENU_MAX_APPLICATIONS: usize = 128;
const WM_MENU_MAX_LABEL_BYTES: usize = 48;
// Standard menus retain a flush row highlight. Profiled themes may opt into
// inset geometry below without changing the basic shell metric contract.
const WM_MENU_SELECTION_INSET: usize = 0;
const WM_MENU_SELECTION_RADIUS: usize = 0;
const WM_FROSTED_ICE_SURFACE_ALPHA: u8 = 190;
const WM_FROSTED_ICE_RAISED_ALPHA: u8 = 150;
const WM_THEME_MAX_COUNT: usize = 32;
const WM_THEME_MAX_NAME_BYTES: usize = 48;
const WM_SHELL_TEXT_HEADER_BYTES: usize = 24;
const WM_TEXT_ALIGN_MENU_ICON_FALLBACK_OMITTED: u8 = 0x40;
const WM_NOTIFICATION_WIDTH: u16 = 360;
const WM_NOTIFICATION_HEIGHT: u16 = 84;
const WM_NOTIFICATION_MARGIN: u16 = 12;
const WM_NOTIFICATION_MAX_TITLE_BYTES: usize = 40;
const WM_NOTIFICATION_MAX_MESSAGE_BYTES: usize = 64;
const WM_TOOLTIP_WIDTH: u16 = 128;
const WM_TOOLTIP_HEIGHT: u16 = 28;
const WM_SYSTEM_CONTROLS_WIDTH: usize = 99;
const WM_SYSTEM_CONTROLS_BATTERY_WIDTH: usize = 143;
const WM_SYSTEM_CONTROLS_RIGHT_MARGIN: usize = 7;
const WM_SYSTEM_CONTROLS_EXTENSION_GAP: usize = 14;
const WM_RICE_BOX_INDICATOR_WIDTH: usize = 48;

// Dynamic .desktop entries use the compact 1..=128 range. Keep built-in
// launchers in a disjoint namespace so installing or alphabetically
// reordering an application can never retarget the taskbar folder button.
const WM_ACTION_PCMANFM: u32 = 0xffff_ff01;
const WM_ACTION_TERMINAL: u32 = 0xffff_ff02;
const WM_ACTION_IMAGE_VIEWER: u32 = 0xffff_ff03;
const WM_ACTION_EXIT: u32 = 0xffff_ff04;
const WM_ACTION_PERFORMANCE: u32 = 0xffff_ff05;
const WM_ACTION_RUN: u32 = 0xffff_ff06;
const WM_ACTION_SHOW_DESKTOP: u32 = 0xffff_ff07;
const WM_ACTION_THEME_CUSTOMIZE: u32 = 0xffff_ff08;
const WM_ACTION_TOOLBAR_PREFERENCES: u32 = 0xffff_ff09;
const WM_ACTION_RESTORE_WINDOWS: u32 = 0xffff_ff0a;
const WM_ACTION_GNOME_INDICATOR_PREFIX: u32 = 0xffff_f000;
const WM_ACTION_GNOME_DIALOG_PREFIX: u32 = 0xffec_0000;
const WM_ACTION_NETWORK_PREFERENCES: u32 = 0xffff_ff0f;
const WM_ACTION_SYSTEM_CONTROLS: u32 = 0xfffc_0020;
const WM_ACTION_RICE_BOX: u32 = 0xfffc_0021;
const WM_ACTION_RICE_BOX_COMPOSE: u32 = 0xfffc_0022;
const WM_ACTION_QUICK_SHORTCUT_PREFERENCES: u32 = 0xffff_ff11;
const WM_ACTION_GNOME_MENU_ITEM_PREFIX: u32 = 0xffed_0000;
const WM_ACTION_QUICK_PIN_PREFIX: u32 = 0xfb00_0000;
const WM_ACTION_QUICK_REMOVE_PREFIX: u32 = 0xfa00_0000;
const WM_ACTION_MENU_MOVE_PREFIX: u32 = 0xfe00_0000;
const WM_ACTION_MENU_OPEN_FOLDER_PREFIX: u32 = 0xfd00_0000;
const WM_ACTION_MENU_UNINSTALL_PREFIX: u32 = 0xfc00_0000;
const WM_ACTION_MENU_OPEN_DIRECTORY: u32 = 0xffff_ff10;
const WM_ACTION_VOLUME_PREFIX: u32 = 0xfffd_0000;
const WM_ACTION_NATIVE_TASK_PREFIX: u32 = 0xfffa_0000;
const WM_ACTION_WORKSPACE_PREFIX: u32 = 0xfffb_0000;
const WM_ACTION_X11_FOREGROUND_PREFIX: u32 = 0xfff9_0000;
const WM_ACTION_TASK_CLOSE_PREFIX: u32 = 0xfff8_0000;
const WM_ACTION_TASK_MAXIMIZE_PREFIX: u32 = 0xfff7_0000;
const WM_ACTION_TASK_MINIMIZE_PREFIX: u32 = 0xfff6_0000;
const WM_ACTION_TASK_RESTORE_PREFIX: u32 = 0xfff5_0000;
const WM_ACTION_STACK_CLOSE_PREFIX: u32 = 0xfff4_0000;
const WM_ACTION_STACK_TILE_PREFIX: u32 = 0xfff3_0000;
const WM_ACTION_STACK_CASCADE_PREFIX: u32 = 0xfff2_0000;
const WM_ACTION_THEME_PREFIX: u32 = 0xfff1_0000;
const WM_ACTION_TASK_SNAP_LEFT_PREFIX: u32 = 0xfff0_0000;
const WM_ACTION_TASK_SNAP_RIGHT_PREFIX: u32 = 0xffef_0000;
const WM_ACTION_TASK_WORKSPACE_NEXT_PREFIX: u32 = 0xffee_0000;
const WM_ACTION_TASK_WORKSPACE_PREVIOUS_PREFIX: u32 = 0xffec_0000;
const WM_ACTION_TASKBAR_EDGE_PREFIX: u32 = 0xffeb_0000;
const WM_NATIVE_TASK_MAX_COUNT: usize = 48;
const WM_NATIVE_TASK_MAX_TITLE_BYTES: usize = 48;
const WM_TASK_PREVIEW_WIDTH: u16 = 320;
const WM_TASK_PREVIEW_HEIGHT: u16 = 220;
const WM_TASK_PREVIEW_NATIVE_PREFIX: u32 = 0x8000_0000;
const WM_SNAP_EDGE_THRESHOLD: i16 = 8;

const KEY_PRESS_MASK: u32 = 0x0000_0001;
const KEY_RELEASE_MASK: u32 = 0x0000_0002;
const BUTTON_PRESS_MASK: u32 = 0x0000_0004;
const BUTTON_RELEASE_MASK: u32 = 0x0000_0008;
const ENTER_WINDOW_MASK: u32 = 0x0000_0010;
const LEAVE_WINDOW_MASK: u32 = 0x0000_0020;
const POINTER_MOTION_MASK: u32 = 0x0000_0040;
const BUTTON_1_MOTION_MASK: u32 = 0x0000_0100;
const BUTTON_2_MOTION_MASK: u32 = 0x0000_0200;
const BUTTON_3_MOTION_MASK: u32 = 0x0000_0400;
const BUTTON_4_MOTION_MASK: u32 = 0x0000_0800;
const BUTTON_5_MOTION_MASK: u32 = 0x0000_1000;
const BUTTON_MOTION_MASK: u32 = 0x0000_2000;
const EXPOSURE_MASK: u32 = 0x0000_8000;
const VISIBILITY_CHANGE_MASK: u32 = 0x0001_0000;
const STRUCTURE_NOTIFY_MASK: u32 = 0x0002_0000;
const RESIZE_REDIRECT_MASK: u32 = 0x0004_0000;
const SUBSTRUCTURE_NOTIFY_MASK: u32 = 0x0008_0000;
const SUBSTRUCTURE_REDIRECT_MASK: u32 = 0x0010_0000;
const FOCUS_CHANGE_MASK: u32 = 0x0020_0000;
const PROPERTY_CHANGE_MASK: u32 = 0x0040_0000;
const EXCLUSIVE_EVENT_MASKS: u32 = 0x0000_0004 | RESIZE_REDIRECT_MASK | SUBSTRUCTURE_REDIRECT_MASK;

const NOTIFY_ANCESTOR: u8 = 0;
const NOTIFY_VIRTUAL: u8 = 1;
const NOTIFY_INFERIOR: u8 = 2;
const NOTIFY_NONLINEAR: u8 = 3;
const NOTIFY_NONLINEAR_VIRTUAL: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientHandle(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClientStatus {
    AwaitingSetup = 1,
    Ready = 2,
    Rejected = 3,
    Faulted = 4,
}

#[derive(Clone, Copy, Debug)]
pub struct DisplayConfig {
    pub width_px: u16,
    pub height_px: u16,
    pub max_clients: u16,
    pub max_windows: usize,
    pub max_ingress_bytes: usize,
    pub max_egress_bytes: usize,
    pub max_request_bytes: usize,
    pub max_scene_bytes: usize,
    pub max_surface_bytes: usize,
    pub native_window_manager: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            width_px: 1024,
            height_px: 768,
            max_clients: 64,
            max_windows: 4096,
            max_ingress_bytes: 32 * 1024 * 1024,
            max_egress_bytes: 64 * 1024 * 1024,
            max_request_bytes: 16 * 1024 * 1024,
            max_scene_bytes: 1024 * 1024,
            max_surface_bytes: 256 * 1024 * 1024,
            native_window_manager: false,
        }
    }
}

impl DisplayConfig {
    fn normalized(mut self) -> Self {
        self.width_px = self.width_px.max(1);
        self.height_px = self.height_px.max(1);
        self.max_clients = self.max_clients.clamp(1, MAX_RESOURCE_SLOTS);
        self.max_windows = self.max_windows.clamp(1, 32_768);
        self.max_ingress_bytes = self.max_ingress_bytes.max(MAX_SETUP_BYTES);
        self.max_egress_bytes = self.max_egress_bytes.max(1024);
        self.max_request_bytes = self.max_request_bytes.clamp(4, 64 * 1024 * 1024);
        self.max_scene_bytes = self
            .max_scene_bytes
            .max((self.max_windows + 1) * SCENE_COMMAND_BYTES);
        self.max_surface_bytes = self.max_surface_bytes.max(4 * 1024 * 1024);
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenError {
    ClientLimit,
    HandleExhausted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    UnknownClient,
    ClientNotWritable,
    IngressLimit,
    EgressLimit,
    InvalidProtocol,
    EncodingFailure,
}

#[derive(Clone, Copy, Debug)]
enum ClientPhase {
    AwaitingSetup,
    Ready { byte_order: ClientByteOrder },
    Rejected,
    Faulted,
}

impl ClientPhase {
    const fn status(self) -> ClientStatus {
        match self {
            Self::AwaitingSetup => ClientStatus::AwaitingSetup,
            Self::Ready { .. } => ClientStatus::Ready,
            Self::Rejected => ClientStatus::Rejected,
            Self::Faulted => ClientStatus::Faulted,
        }
    }
}

#[derive(Debug)]
struct Client {
    phase: ClientPhase,
    sequence: SequenceNumber,
    resource_slot: u8,
    ingress: Vec<u8>,
    ingress_offset: usize,
    egress: VecDeque<u8>,
    event_masks: BTreeMap<u32, u32>,
    xi2_masks: BTreeMap<(u32, u16), u32>,
    xkb_event_mask: u16,
    xkb_client_flags: u32,
    save_set: BTreeSet<u32>,
    big_requests_enabled: bool,
}

impl Client {
    fn new(resource_slot: u8) -> Self {
        Self {
            phase: ClientPhase::AwaitingSetup,
            sequence: SequenceNumber(0),
            resource_slot,
            ingress: Vec::new(),
            ingress_offset: 0,
            egress: VecDeque::new(),
            event_masks: BTreeMap::new(),
            xi2_masks: BTreeMap::new(),
            xkb_event_mask: 0,
            xkb_client_flags: 0,
            save_set: BTreeSet::new(),
            big_requests_enabled: false,
        }
    }

    fn resource_id_base(&self) -> u32 {
        u32::from(self.resource_slot) << 21
    }

    fn owns_resource(&self, id: ResourceId) -> bool {
        id.0 != 0 && (id.0 & !RESOURCE_ID_MASK) == self.resource_id_base()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Progress {
    Advanced,
    NeedMore,
    Stop,
}

#[derive(Clone, Copy, Debug)]
struct ProtocolError {
    code: u8,
    bad_value: u32,
    minor_opcode: u16,
}

#[derive(Clone, Copy, Debug)]
struct PointerInput {
    source: ResourceId,
    mask: u32,
    detail: u8,
    root_x: i16,
    root_y: i16,
    state: u16,
}

#[derive(Clone, Copy, Debug)]
struct ActivePointerGrab {
    owner: u32,
    window: ResourceId,
    owner_events: bool,
    event_mask: u16,
    automatic: bool,
}

#[derive(Clone, Copy, Debug)]
struct ActiveKeyboardGrab {
    owner: u32,
    window: ResourceId,
    owner_events: bool,
}

#[derive(Clone, Copy, Debug)]
struct WmDrag {
    frame: ResourceId,
    client: ResourceId,
    pointer_x: i16,
    pointer_y: i16,
    frame_x: i16,
    frame_y: i16,
    restore_geometry: (i16, i16, u16, u16),
    resize_geometry: (i16, i16, u16, u16),
    resize_edges: u8,
    moved: bool,
    restore_on_move: bool,
}

#[derive(Clone, Debug)]
struct WmMenuDrag {
    entry: WmMenuEntry,
    source_index: usize,
    pointer_x: i16,
    pointer_y: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WmSnapTarget {
    Left,
    Right,
    Maximize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum WmTaskbarEdge {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

impl WmTaskbarEdge {
    const fn from_wire(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Bottom),
            1 => Some(Self::Top),
            2 => Some(Self::Left),
            3 => Some(Self::Right),
            _ => None,
        }
    }

    const fn wire(self) -> u32 {
        match self {
            Self::Bottom => 0,
            Self::Top => 1,
            Self::Left => 2,
            Self::Right => 3,
        }
    }

    const fn vertical(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WmTaskbarDrag {
    pointer_x: i16,
    pointer_y: i16,
    candidate: WmTaskbarEdge,
    moved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WmSnapState {
    target: WmSnapTarget,
    restore_geometry: (i16, i16, u16, u16),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WmChromeTarget {
    Start,
    Activities,
    QuickLaunch(u8),
    Task(WmTaskTarget),
    Tray(u8),
    NotificationIcon(u32),
    Workspace { tray: u8, workspace: u32 },
    SystemControls,
    RiceBox,
    FrameIcon(ResourceId),
    FrameControl(ResourceId, u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WmTaskTarget {
    X11(ResourceId),
    Native(u16),
    Group(u16),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum WmTaskPreviewPlacement {
    #[default]
    Taskbar,
    MenuLeft,
    MenuRight,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmNativeTask {
    pub id: u16,
    pub workspace: u32,
    pub icon: u8,
    pub minimized: bool,
    pub maximized: bool,
    pub active: bool,
    pub title: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmQuickLauncher {
    pub action: u32,
    pub icon: u8,
    pub title: Vec<u8>,
}

impl WmChromeTarget {
    const fn window(self) -> ResourceId {
        match self {
            Self::Start
            | Self::Activities
            | Self::QuickLaunch(_)
            | Self::Task(_)
            | Self::NotificationIcon(_)
            | Self::Tray(_)
            | Self::Workspace { .. }
            | Self::SystemControls => WM_TASKBAR,
            Self::RiceBox => WM_GNOME_PANEL,
            Self::FrameIcon(frame) => frame,
            Self::FrameControl(frame, _) => frame,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmMenuEntry {
    action: u32,
    icon: u8,
    category: u8,
    folder: Option<u8>,
    uninstallable: bool,
    provisionable: bool,
    hydrating: bool,
    label: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmMenuFolder {
    parent: Option<u8>,
    label: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmGnomeMenuItem {
    pub index: u8,
    pub separator: bool,
    pub reactive: bool,
    pub toggle: bool,
    pub icon_kind: u8,
    pub image: bool,
    pub checked: bool,
    pub label: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmGnomeIndicator {
    pub uuid: Vec<u8>,
    pub visible: bool,
    pub has_icon: bool,
    pub position: u8,
    pub index: u8,
    pub label: Vec<u8>,
    pub image: Option<WmGnomeMenuImage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmGnomeMenuImage {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmGnomeOverlayRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub color: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmGnomeBackgroundText {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub font_px: u8,
    pub weight: u8,
    pub align: u8,
    pub foreground: u32,
    pub text: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmTextOverlay {
    window: ResourceId,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    font_px: u8,
    weight: u8,
    align: u8,
    foreground: u32,
    background: u32,
    text: Vec<u8>,
}

impl WmTextOverlay {
    #[allow(clippy::too_many_arguments)]
    fn new(
        window: ResourceId,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        font_px: u8,
        weight: u8,
        align: u8,
        foreground: u32,
        background: u32,
        text: &[u8],
        max_characters: usize,
    ) -> Self {
        Self {
            window,
            x: u16::try_from(x).unwrap_or(u16::MAX),
            y: u16::try_from(y).unwrap_or(u16::MAX),
            width: u16::try_from(width).unwrap_or(u16::MAX),
            height: u16::try_from(height).unwrap_or(u16::MAX),
            font_px,
            weight,
            align,
            foreground,
            background,
            text: text_overlay_bytes(text, max_characters.min(255)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn new_rotated_ccw(
        window: ResourceId,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        font_px: u8,
        weight: u8,
        foreground: u32,
        background: u32,
        text: &[u8],
        max_characters: usize,
    ) -> Self {
        let mut overlay = Self::new(
            window,
            x,
            y,
            width,
            height,
            font_px,
            weight,
            1,
            foreground,
            background,
            text,
            max_characters,
        );
        // The shell-text ABI reserves the high bits of alignment. Bit 7 asks
        // the browser text renderer to rotate the label counter-clockwise.
        overlay.align |= 0x80;
        overlay
    }

    fn with_menu_icon(mut self, size: u8, icon_kind: u8) -> Self {
        let size_code = match size {
            16 => 0,
            24 => 1,
            32 => 2,
            64 => 3,
            _ => 1,
        };
        // Bits 2..=3 carry the selected edge, bits 4..=5 carry the semantic
        // icon kind, and bit 6 tells the browser that no raster fallback was
        // painted beneath it.
        self.align |= (size_code << 2)
            | ((icon_kind & 0x03) << 4)
            | WM_TEXT_ALIGN_MENU_ICON_FALLBACK_OMITTED;
        self
    }

    fn record_len(&self) -> usize {
        (WM_SHELL_TEXT_HEADER_BYTES + self.text.len() + 3) & !3
    }
}

fn text_overlay_bytes(text: &[u8], maximum_bytes: usize) -> Vec<u8> {
    let maximum_bytes = maximum_bytes.min(255);
    let Ok(text) = core::str::from_utf8(text) else {
        return text.iter().copied().take(maximum_bytes).collect();
    };
    let mut output = Vec::with_capacity(text.len().min(maximum_bytes));
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        let regional = u32::from(character).checked_sub(0x1f1e6);
        let next_regional = characters
            .peek()
            .and_then(|next| u32::from(*next).checked_sub(0x1f1e6));
        if let (Some(first), Some(second)) = (regional, next_regional)
            && first < 26
            && second < 26
        {
            let fallback = [b'[', b'A' + first as u8, b'A' + second as u8, b']'];
            if output.len().saturating_add(fallback.len()) > maximum_bytes {
                break;
            }
            output.extend_from_slice(&fallback);
            characters.next();
            continue;
        }
        let mut encoded = [0u8; 4];
        let encoded = character.encode_utf8(&mut encoded).as_bytes();
        if output.len().saturating_add(encoded.len()) > maximum_bytes {
            break;
        }
        output.extend_from_slice(encoded);
    }
    output
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum WmMenuLevel {
    #[default]
    Root,
    Categories,
    Applications(u8),
    MenuFolders,
    MenuFolder(u8),
    ApplicationContext(u32),
    MenuDirectoryContext,
    Themes,
    TaskbarContext,
    QuickLaunchContext,
    TaskGroup(u16),
    TaskContext(WmTaskTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WmThemePalette {
    name: Vec<u8>,
    backdrop_material: bool,
    menu_backdrop_material: bool,
    chrome_profile: WmChromeProfile,
    taskbar: [u32; 7],
    menu: [u32; 6],
    frame: [u32; 8],
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WmChromeProfile {
    #[default]
    Standard,
    Familiar,
    Redmond98,
    YellowJacket,
    CommandDeck,
    Orchard,
    NeonGrid,
    SplitControls,
}

pub type WmThemeRecord = (
    Vec<u8>,
    bool,
    bool,
    WmChromeProfile,
    [u32; 7],
    [u32; 6],
    [u32; 8],
);

fn wm_frame_control_left(profile: WmChromeProfile, width: usize, index: u8) -> usize {
    if profile == WmChromeProfile::Orchard {
        return match index {
            2 => 8,
            0 => 30,
            1 => 52,
            _ => width,
        };
    }
    if profile == WmChromeProfile::SplitControls {
        return match index {
            0 => 3,
            1 => width.saturating_sub(usize::from(WM_TITLE_BUTTON_WIDTH * 2)),
            2 => width.saturating_sub(usize::from(WM_TITLE_BUTTON_WIDTH)),
            _ => width,
        };
    }
    if profile == WmChromeProfile::Redmond98 {
        // Win98 caption controls are 16 px wide with a 2 px gap. Keep a
        // visible right gutter inside the frame while packing the
        // other controls beside it instead of using the modern 22 px slots.
        return width
            .saturating_sub(60)
            .saturating_add(usize::from(index) * 18);
    }
    width
        .saturating_sub(usize::from(WM_TITLE_BUTTON_WIDTH * WM_TITLE_BUTTON_COUNT))
        .saturating_add(usize::from(index) * usize::from(WM_TITLE_BUTTON_WIDTH))
}

fn wm_frame_control_at(profile: WmChromeProfile, width: usize, x: usize) -> Option<u8> {
    [0_u8, 1, 2].into_iter().find(|index| {
        let left = wm_frame_control_left(profile, width, *index);
        let hit_width = if profile == WmChromeProfile::Redmond98 {
            18
        } else {
            usize::from(WM_TITLE_BUTTON_WIDTH)
        };
        (left..left.saturating_add(hit_width)).contains(&x)
    })
}

fn wm_frame_icon_left(profile: WmChromeProfile) -> usize {
    if profile == WmChromeProfile::Orchard {
        76
    } else if profile == WmChromeProfile::SplitControls {
        29
    } else {
        3
    }
}

fn built_in_wm_themes() -> Vec<WmThemePalette> {
    vec![
        WmThemePalette {
            name: b"RiceWM".to_vec(),
            backdrop_material: false,
            menu_backdrop_material: false,
            chrome_profile: WmChromeProfile::Standard,
            taskbar: [
                0x003d_3846,
                0x0061_5369,
                0x0050_4654,
                0x0050_4654,
                0x002c_001e,
                0x00f8_fafc,
                0x00e9_5420,
            ],
            menu: [
                0x0031_2b35,
                0x0038_303b,
                0x0056_4b5e,
                0x00e9_5420,
                0x007a_321f,
                0x00f8_fafc,
            ],
            frame: [
                0x00e9_5420,
                0x0050_4654,
                0x0061_5369,
                0x002c_001e,
                0x00ff_ffff,
                0x003e_526d,
                0x00b9_3c4c,
                0x007a_321f,
            ],
        },
        WmThemePalette {
            name: b"Familiar".to_vec(),
            backdrop_material: false,
            menu_backdrop_material: false,
            chrome_profile: WmChromeProfile::Familiar,
            taskbar: [
                0x00f0_f0f0,
                0x00ff_ffff,
                0x00f0_f0f0,
                0x00f0_f0f0,
                0x00f0_f0f0,
                0x0000_0000,
                0x0000_78d7,
            ],
            menu: [
                0x00f0_f0f0,
                0x00ff_ffff,
                0x00a0_a0a0,
                0x0000_78d7,
                0x00e5_f1fb,
                0x0000_0000,
            ],
            frame: [
                0x0000_78d7,
                0x00a0_a0a0,
                0x00f0_f0f0,
                0x00f0_f0f0,
                0x00ff_ffff,
                0x00f0_f0f0,
                0x00e8_1123,
                0x00c5_0f1f,
            ],
        },
        WmThemePalette {
            name: b"Dark".to_vec(),
            backdrop_material: false,
            menu_backdrop_material: false,
            chrome_profile: WmChromeProfile::Standard,
            taskbar: [
                0x0011_1827,
                0x0033_4155,
                0x0029_3548,
                0x001f_2937,
                0x000b_1120,
                0x00f8_fafc,
                0x0060_a5fa,
            ],
            menu: [
                0x0011_1827,
                0x0017_2233,
                0x0033_4155,
                0x00e9_5420,
                0x0048_2730,
                0x00f8_fafc,
            ],
            frame: [
                0x001e_40af,
                0x0033_4155,
                0x0047_5f7d,
                0x0010_151c,
                0x00ff_ffff,
                0x003e_526d,
                0x00b9_3c4c,
                0x000f_172a,
            ],
        },
    ]
}

fn append_xsettings_string(data: &mut Vec<u8>, serial: u32, name: &str, value: &str) {
    data.push(1);
    data.push(0);
    data.extend_from_slice(&u16::try_from(name.len()).unwrap_or(u16::MAX).to_le_bytes());
    data.extend_from_slice(name.as_bytes());
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
    data.extend_from_slice(&serial.to_le_bytes());
    data.extend_from_slice(&u32::try_from(value.len()).unwrap_or(u32::MAX).to_le_bytes());
    data.extend_from_slice(value.as_bytes());
    while !data.len().is_multiple_of(4) {
        data.push(0);
    }
}

fn xsettings_theme_payload(serial: u32, theme_index: u8, palette: &WmThemePalette) -> Vec<u8> {
    let [background, _, _, hover, _, foreground] = palette.menu;
    let selected_foreground = contrasting_text(hover);
    let base = palette.frame[3];
    let color_scheme = format!(
        "fg_color:#{foreground:06x}\n\
         bg_color:#{background:06x}\n\
         base_color:#{base:06x}\n\
         text_color:#{foreground:06x}\n\
         selected_bg_color:#{hover:06x}\n\
         selected_fg_color:#{selected_foreground:06x}",
    );
    let mut data = Vec::with_capacity(256);
    // XSETTINGS uses the X11 numeric byte-order values (LSBFirst = 0,
    // MSBFirst = 1), not the connection-setup ASCII markers ('l'/'B').
    data.extend_from_slice(&[0, 0, 0, 0]);
    data.extend_from_slice(&serial.to_le_bytes());
    data.extend_from_slice(&4u32.to_le_bytes());
    append_xsettings_string(&mut data, serial, "Gtk/ColorScheme", &color_scheme);
    append_xsettings_string(&mut data, serial, "Gtk/FontName", "Cantarell 11");
    // Accelerated GTK application images carry a deterministic, PNG-backed
    // Adwaita closure.  Advertising LXDE's legacy nuoveXT2 theme here makes
    // GTK resolve named toolbar actions through a sparse inheritance chain;
    // the file-manager place icons happen to survive while actions such as
    // go-up, go-previous, document-new, and user-home disappear.  Select the
    // same complete theme that the configured-image builder attests instead.
    append_xsettings_string(&mut data, serial, "Net/IconThemeName", "Adwaita");
    append_xsettings_string(
        &mut data,
        serial,
        "Net/ThemeName",
        &format!("RiceWM-{theme_index:02}"),
    );
    data
}

const fn contrasting_text(background: u32) -> u32 {
    let red = (background >> 16) & 0xff;
    let green = (background >> 8) & 0xff;
    let blue = background & 0xff;
    if red * 299 + green * 587 + blue * 114 >= 150_000 {
        0x0000_0000
    } else {
        0x00ff_ffff
    }
}

const fn mix_rgb(color: u32, target: u32, numerator: u32, denominator: u32) -> u32 {
    let red = (((color >> 16) & 0xff) * (denominator - numerator)
        + ((target >> 16) & 0xff) * numerator)
        / denominator;
    let green = (((color >> 8) & 0xff) * (denominator - numerator)
        + ((target >> 8) & 0xff) * numerator)
        / denominator;
    let blue =
        ((color & 0xff) * (denominator - numerator) + (target & 0xff) * numerator) / denominator;
    (red << 16) | (green << 8) | blue
}

fn strip_instance_suffix(title: &[u8]) -> &[u8] {
    if title.last() != Some(&b')') {
        return title;
    }
    let Some(open) = title.iter().rposition(|byte| *byte == b'(') else {
        return title;
    };
    if open == 0
        || title.get(open.saturating_sub(1)) != Some(&b' ')
        || open + 1 >= title.len().saturating_sub(1)
        || !title[open + 1..title.len() - 1]
            .iter()
            .all(u8::is_ascii_digit)
    {
        return title;
    }
    &title[..open - 1]
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WmMenuRow {
    Applications,
    Run,
    Category(u8),
    MenuFolder(u8),
    Application(WmMenuEntry),
    ApplicationLaunch,
    ApplicationOpenFolder,
    ApplicationUninstall,
    MenuOpenDirectory,
    ScrollDown,
    ScrollUp,
    Themes,
    Theme(u8),
    Divider,
    ThemeCustomize,
    RestoreWindows,
    NextWorkspace,
    PreviousWorkspace,
    WorkspaceNavigation,
    RedmondShortcut(RedmondMenuShortcut),
    ShowDesktop,
    ToolbarPreferences,
    DockSettings,
    QuickShortcutPreferences,
    QuickShortcutRemove,
    Exit,
    Task(WmTaskTarget),
    WindowClose,
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    StackClose,
    StackTile,
    StackCascade,
    Empty,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RedmondMenuShortcut {
    Documents,
}

#[cfg(test)]
const fn wm_menu_row_height(row: &WmMenuRow) -> usize {
    wm_menu_row_height_with(row, WM_MENU_ROW_HEIGHT)
}

const fn wm_menu_row_has_submenu(row: &WmMenuRow) -> bool {
    matches!(
        row,
        WmMenuRow::Applications
            | WmMenuRow::Themes
            | WmMenuRow::Category(_)
            | WmMenuRow::MenuFolder(_)
    )
}

const fn wm_menu_row_browser_icon_kind(row: &WmMenuRow) -> Option<u8> {
    match row {
        // Dynamic labels cannot be resolved through the browser's label map,
        // so carry their selected-library semantic explicitly.
        WmMenuRow::Theme(_) => Some(1),
        WmMenuRow::MenuFolder(_) => Some(2),
        WmMenuRow::ScrollDown
        | WmMenuRow::ScrollUp
        | WmMenuRow::Divider
        | WmMenuRow::Task(_) => {
            None}
        _ => Some(0),
    }
}

fn wm_menu_base_row_height_with(redmond98: bool, icon_size: usize) -> usize {
    if redmond98 {
        REDMOND_MENU_ROW_HEIGHT.max(icon_size.saturating_add(8))
    } else {
        WM_MENU_ROW_HEIGHT.max(icon_size.saturating_add(12))
    }
}

const fn wm_menu_row_height_with(row: &WmMenuRow, row_height: usize) -> usize {
    if matches!(row, WmMenuRow::Divider) {
        WM_MENU_DIVIDER_HEIGHT
    } else {
        row_height
    }
}

fn wm_menu_rows_height_with(rows: &[WmMenuRow], row_height: usize) -> usize {
    rows.iter()
        .map(|row| wm_menu_row_height_with(row, row_height))
        .sum()
}

fn wm_gnome_menu_item_height(item: &WmGnomeMenuItem) -> usize {
    if item.image {
        WM_GNOME_MENU_IMAGE_HEIGHT
    } else if item.separator {
        if item.label.is_empty() { 10 } else { 28 }
    } else {
        WM_GNOME_MENU_ROW_HEIGHT
    }
}

#[cfg(test)]
fn wm_menu_row_top(rows: &[WmMenuRow], index: usize) -> usize {
    rows.iter().take(index).map(wm_menu_row_height).sum()
}

fn wm_menu_row_top_with(rows: &[WmMenuRow], index: usize, row_height: usize) -> usize {
    rows.iter()
        .take(index)
        .map(|row| wm_menu_row_height_with(row, row_height))
        .sum()
}

fn wm_menu_row_at_with(
    rows: &[WmMenuRow],
    y: usize,
    row_height: usize,
) -> Option<(usize, WmMenuRow)> {
    let mut top = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let bottom = top.saturating_add(wm_menu_row_height_with(row, row_height));
        if y < bottom {
            return Some((index, row.clone()));
        }
        top = bottom;
    }
    None
}

fn wm_category_label(category: u8) -> &'static [u8] {
    match category {
        1 => rice_i18n::t("Accessories").as_bytes(),
        2 => rice_i18n::t("Development").as_bytes(),
        3 => rice_i18n::t("Games").as_bytes(),
        4 => rice_i18n::t("Graphics").as_bytes(),
        5 => rice_i18n::t("Internet").as_bytes(),
        6 => rice_i18n::t("Multimedia").as_bytes(),
        7 => rice_i18n::t("Office").as_bytes(),
        8 => rice_i18n::t("Settings & System").as_bytes(),
        _ => rice_i18n::t("Other").as_bytes(),
    }
}

fn wm_category_icon(category: u8) -> u8 {
    match category {
        2 => 7,
        4 => 3,
        5 => 8,
        6 => 9,
        8 => 4,
        _ => 1,
    }
}

#[derive(Clone, Debug)]
struct XFixesRegion {
    owner: u32,
    rectangles: Vec<x11xfixes::RegionRect>,
}

#[derive(Clone, Debug)]
struct DamageResource {
    owner: u32,
    drawable: ResourceId,
    level: u8,
    pending: Vec<x11xfixes::RegionRect>,
}

#[derive(Clone, Copy, Debug)]
struct SyncCounter {
    owner: u32,
    value: i64,
}

#[derive(Clone, Copy, Debug)]
struct SyncAlarm {
    owner: u32,
    counter: u32,
    wait_value: i64,
    delta: i64,
    test_type: u32,
    events: bool,
    state: u8,
}

#[derive(Clone, Copy, Debug)]
struct SyncFence {
    owner: u32,
    triggered: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClippedRectangle {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(feature = "desktop-raster", repr(C))]
struct RenderPixel {
    /// Premultiplied channels, matching X Render's ARGB convention.
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl ProtocolError {
    const fn new(code: u8, bad_value: u32) -> Self {
        Self {
            code,
            bad_value,
            minor_opcode: 0,
        }
    }
}

#[derive(Debug)]
pub struct DisplayServer {
    config: DisplayConfig,
    clients: BTreeMap<u32, Client>,
    windows: BTreeMap<u32, Window>,
    pixmaps: BTreeMap<u32, Pixmap>,
    graphics_contexts: BTreeMap<u32, GraphicsContext>,
    fonts: BTreeMap<u32, Font>,
    colormaps: BTreeMap<u32, Colormap>,
    cursors: BTreeMap<u32, RiceCursor>,
    window_cursors: BTreeMap<u32, RiceCursor>,
    pictures: BTreeMap<u32, Picture>,
    glyphsets: BTreeMap<u32, GlyphSet>,
    glyph_storage: BTreeMap<u32, BTreeMap<u32, Glyph>>,
    xfixes_regions: BTreeMap<u32, XFixesRegion>,
    xfixes_selection_masks: BTreeMap<(u32, u32, u32), u32>,
    xfixes_cursor_masks: BTreeMap<(u32, u32), u32>,
    xfixes_barriers: BTreeMap<u32, u32>,
    cursor_names: BTreeMap<u32, Vec<u8>>,
    window_shapes: BTreeMap<(u32, u8), Vec<x11xfixes::RegionRect>>,
    shape_selects: BTreeSet<(u32, u32)>,
    sync_counters: BTreeMap<u32, SyncCounter>,
    sync_alarms: BTreeMap<u32, SyncAlarm>,
    sync_fences: BTreeMap<u32, SyncFence>,
    client_priorities: BTreeMap<u32, i32>,
    randr_selects: BTreeMap<(u32, u32), u16>,
    damage_resources: BTreeMap<u32, DamageResource>,
    composite_redirects: BTreeMap<(u32, u32), (u8, bool)>,
    composite_pixmaps: BTreeMap<u32, ResourceId>,
    composite_overlay_users: BTreeSet<u32>,
    surfaces: BTreeMap<u32, Surface>,
    atoms: AtomTable,
    selections: BTreeMap<u32, (ResourceId, u32, u32)>,
    scene: SceneQueue,
    request_counts: BTreeMap<u8, u32>,
    presented_managed_windows: VecDeque<ResourceId>,
    unsupported_requests: BTreeMap<u8, u32>,
    next_client_handle: u32,
    next_internal_drawable: u32,
    timestamp: u32,
    timestamp_from_clock: bool,
    last_focus_time: u32,
    focus_window: ResourceId,
    pointer_x: i16,
    pointer_y: i16,
    physical_pointer_position: Option<(i16, i16)>,
    pointer_window: ResourceId,
    input_state: u16,
    wm_super_pending: bool,
    wm_super_chord: bool,
    keymap: [u8; 32],
    keysyms: BTreeMap<u8, Vec<u32>>,
    pointer_mapping: Vec<u8>,
    modifier_mapping: (u8, Vec<u8>),
    screensaver: (u16, u16, u8, u8),
    pointer_grab: Option<ActivePointerGrab>,
    keyboard_grab: Option<ActiveKeyboardGrab>,
    managed_frames: BTreeMap<u32, ResourceId>,
    auto_tile_enabled: bool,
    auto_tile_previous_enabled: bool,
    auto_tile_workspace: u32,
    tiled_workspaces: BTreeSet<u32>,
    auto_tile_focus: Option<WmTaskTarget>,
    auto_tile_native: Vec<u16>,
    auto_tile_positions: Vec<(u16,i32,i32)>,
    auto_tile_fullscreens: Vec<u16>,
    auto_tile_clips: Vec<(u32,u32,TileRect)>,
    overview_placed_windows: BTreeSet<u32>,
    auto_tile_minimums: Vec<(WmTaskTarget,u16,u16)>,
    auto_tile_rects: Vec<(u16, i32, i32, u16, u16)>,
    tile_snapshot_buffer: Vec<u32>,
    tile_insertion_preview: Option<(u32,TileRect)>,
    tile_identity_buffer: Vec<u8>,
    auto_tile_restore: BTreeMap<u32, (i16, i16, u16, u16)>,
    navigation_native: BTreeMap<u16, (i32, i32, u32, u32)>,
    tile_layouts: BTreeMap<u32, TileLayout<WmTaskTarget>>,
    frame_clients: BTreeMap<u32, ResourceId>,
    park_next_managed_window: bool,
    parked_managed_windows: BTreeSet<u32>,
    next_wm_frame: u32,
    current_workspace: u32,
    workspace_count: u32,
    scrolling_workspaces: bool,
    output_workspaces: BTreeMap<u32,u32>,
    native_output_positions: BTreeMap<u16,(i32,i32)>,
    window_workspaces: BTreeMap<u32, u32>,
    minimized_windows: BTreeSet<u32>,
    non_resizable_managed_windows: BTreeSet<u32>,
    native_tasks: Vec<WmNativeTask>,
    wm_drag: Option<WmDrag>,
    tile_move_request: Option<(u32, i16, i16, bool)>,
    wm_menu_drag: Option<WmMenuDrag>,
    wm_snap_target: Option<WmSnapTarget>,
    wm_chrome_hover: Option<WmChromeTarget>,
    wm_chrome_pressed: Option<WmChromeTarget>,
    wm_outputs: Vec<(u16, u16, u16, u16)>,
    native_fullscreen_regions: Vec<(u16, u16, u16, u16)>,
    wm_primary_output: usize,
    x11_output_homes: BTreeMap<u32,output_layout::X11OutputHome>,
    wm_shell_output: usize,
    wm_topbar_all_monitors: bool,
    wm_dock_all_monitors: bool,
    wm_painting_replicas: bool,
    restore_geometries: BTreeMap<u32, (i16, i16, u16, u16)>,
    maximized_geometries: BTreeMap<u32, (i16, i16, u16, u16)>,
    snapped_geometries: BTreeMap<u32, WmSnapState>,
    wm_actions: VecDeque<u32>,
    wm_relaunch_commands: VecDeque<Vec<u8>>,
    wm_clock_minutes: u16,
    wm_dock_style: bool,
    wm_dock_height: Option<usize>,
    wm_dock_corner_radius: usize,
    wm_dock_magnification: bool,
    wm_dock_magnification_zoom: u8,
    wm_dock_magnification_spread: u8,
    wm_dock_task_keys: BTreeMap<u16, u32>,
    wm_dock_launcher_keys: BTreeMap<u32, u32>,
    wm_embedded_shell: bool,
    wm_launcher_visible: bool,
    wm_dock_icon_color: Option<u32>,
    wm_taskbar_visible: bool,
    wm_taskbar_edge: WmTaskbarEdge,
    wm_taskbar_drag: Option<WmTaskbarDrag>,
    wm_taskbar_auto_hide: bool,
    wm_taskbar_revealed: bool,
    wm_gnome_dash_visible: bool,
    wm_gnome_workspaces_visible: bool,
    wm_gnome_panel: bool,
    wm_gnome_panel_height: u16,
    wm_gnome_panel_bottom: bool,
    wm_topbar_config: Option<String>,
    wm_topbar_metadata: String,
    wm_gnome_indicators: Vec<WmGnomeIndicator>,
    wm_gnome_menu_items: Vec<WmGnomeMenuItem>,
    wm_gnome_menu_indicator_uuid: Vec<u8>,
    wm_gnome_menu_image: Option<WmGnomeMenuImage>,
    wm_gnome_menu_icons: BTreeMap<u8, WmGnomeMenuImage>,
    wm_gnome_dialog_uuid: Vec<u8>,
    wm_gnome_dialog_title: Vec<u8>,
    wm_gnome_dialog_body: Vec<Vec<u8>>,
    wm_gnome_dialog_buttons: Vec<Vec<u8>>,
    wm_gnome_dialog_fullscreen: bool,
    wm_gnome_overlay_rects: Vec<WmGnomeOverlayRect>,
    wm_gnome_background_rects: Vec<WmGnomeOverlayRect>,
    wm_gnome_background_texts: Vec<WmGnomeBackgroundText>,
    wm_task_previews_enabled: bool,
    wm_launcher_appearance: u8,
    wm_launcher_text: Vec<u8>,
    wm_task_preview_target: Option<WmTaskTarget>,
    wm_task_preview_placement: WmTaskPreviewPlacement,
    wm_volume: u8,
    wm_volume_muted: bool,
    wm_quick_context_index: Option<u8>,
    wm_quick_launch_visible: bool,
    wm_quick_launchers: Vec<WmQuickLauncher>,
    wm_show_desktop_windows: BTreeMap<u32, BTreeSet<u32>>,
    wm_tray_items: Vec<TrayItem>,
    wm_notification_icons: Vec<WmNotificationIcon>,
    wm_notification_events: VecDeque<[u32; 4]>,
    wm_notification_focus: Option<u32>,
    wm_workspace_names: Vec<Vec<u8>>,
    wm_connectivity_mode: u8,
    // Desktop-target-only status cluster. Bit 0 enables it; subsequent bits
    // describe Ethernet, Wi-Fi, Bluetooth, audio, and mute state.
    wm_system_controls: u8,
    wm_battery_percentage: Option<u8>,
    // Desktop adapters opt into this retained top-bar shortcut. Keeping it in
    // the X11 surface makes Rice Box survive ordinary panel/clock damage; a
    // Canvas-only overlay disappeared as soon as the panel repainted.
    wm_mail_unread: Option<u16>,
    wm_performance_cpu: VecDeque<u8>,
    wm_performance_network: VecDeque<u8>,
    wm_menu_entries: Vec<WmMenuEntry>,
    wm_menu_folders: Vec<WmMenuFolder>,
    wm_menu_structured: bool,
    wm_menu_applications_only: bool,
    wm_menu_icon_size: u8,
    wm_theme_menu_row_height: usize,
    wm_menu_show_themes: bool,
    wm_menu_show_workspaces: bool,
    wm_session_logout: bool,
    wm_menu_level: WmMenuLevel,
    wm_menu_window_anchor: Option<(i16, i16)>,
    wm_application_context_columns: Vec<Vec<WmMenuRow>>,
    wm_application_context_widths: Vec<usize>,
    wm_application_context_offsets: Vec<usize>,
    wm_application_context_x: usize,
    wm_application_context_y: usize,
    wm_application_context_parent_level: WmMenuLevel,
    wm_menu_offset: usize,
    wm_menu_hover_row: Option<(usize, usize, WmMenuRow)>,
    wm_menu_keyboard_parents: Vec<(WmMenuLevel, usize, (usize, usize, WmMenuRow))>,
    wm_theme: u8,
    wm_themes: Vec<WmThemePalette>,
    wm_xsettings_serial: u32,
    wm_notification_kind: u8,
    wm_notification_title: Vec<u8>,
    wm_notification_message: Vec<u8>,
    wm_text_overlays: Vec<WmTextOverlay>,
}

mod lifecycle;
mod shell_setup;
mod shell_menu;
mod shell_menu_wm_menu_column_offsets;
mod task_groups;
mod tiling;
mod shell_hit_testing;
mod shell_previews;
mod shell_paint;
mod shell_updates;
mod overview;
mod shell_configuration;
mod output_configuration;
mod gnome_panel;
mod panel_configuration;
mod workareas;
mod properties;
mod clients;
mod clients_take_presented_managed_window;
mod pointer_input;
mod pointer_input_inject_pointer_button;
mod keyboard_input;
mod shell_commands;
mod shell_commands_handle_wm_menu_key;
mod navigation;
mod window_activation;
mod transient_windows;
mod workspaces;
mod protocol_dispatch;
mod window_requests;
mod window_management;
mod window_mapping;
mod window_geometry;
mod atom_requests;
mod pixmap_requests;
mod graphics_contexts;
mod drawing_requests;
mod image_requests;
mod selection_requests;
mod grab_requests;
mod font_requests;
mod color_requests;
mod cursor_requests;
mod keyboard_mapping;
mod render_requests;
mod render_pictures;
mod render_glyphs;
mod render_transforms;
mod render_sampling;
mod render_compositing;
mod xinerama;
mod resource_ids;
mod randr;
mod shape;
mod xfixes;
mod sync;
mod damage;
mod composite;
mod generic_events;
mod xkb;
mod xinput;
mod xinput_xinput_query_pointer;
mod xtest;
mod extension_requests;
mod client_messages;
mod window_drag;
mod focus_requests;
mod events;
mod resource_state;
mod surfaces;
mod focus_transitions;
mod input_events;
mod hit_testing;
mod scene_updates;
mod window_classification;

mod wire_queue_output;
use wire_queue_output::*;
mod wire_fill_rounded_rect;
use wire_fill_rounded_rect::*;
mod wire_redmond_task_icon;
use wire_redmond_task_icon::*;
mod wire_wm_glyph;
use wire_wm_glyph::*;
mod wire_encode_z_pixmap;
use wire_encode_z_pixmap::*;
mod wire_decode_image;
use wire_decode_image::*;
mod wire_read_i32;
use wire_read_i32::*;

#[cfg(test)]
#[path = "../../../tests/x11-server/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../tests/quick-launch-interactions.rs"]
mod quick_launch_interactions;

include!("notification_area.rs");

#[cfg(test)]
#[path = "../../../tests/taskbar-geometry-compositor.rs"]
mod taskbar_geometry_tests;
