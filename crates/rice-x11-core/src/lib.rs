mod model;
mod scene;
mod server;
mod tiling;
pub use tiling::{Axis, Rect, TileLayout};
mod scrolling;
mod scrolling_workspace;

pub use scene::{
    SCENE_COMMAND_BYTES, SCENE_DAMAGE_WINDOW, SCENE_REMOVE_WINDOW, SCENE_RESET, SCENE_UPSERT_WINDOW,
};
pub use server::{
    ClientError, ClientHandle, ClientStatus, DisplayConfig, DisplayServer, OpenError,
    WmChromeProfile, WmGnomeBackgroundText, WmGnomeIndicator, WmGnomeMenuImage, WmGnomeMenuItem,
    WmGnomeOverlayRect, WmNativeTask, WmQuickLauncher,
};
