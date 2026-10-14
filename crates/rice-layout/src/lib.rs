//! Host-independent window geometry and scrolling arrangements.
pub mod tiling;
pub mod scrolling;
pub mod scrolling_workspace;
pub use tiling::{Axis, Rect, TileLayout};
