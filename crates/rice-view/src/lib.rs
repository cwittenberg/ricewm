//! Pure application model for the `RiceView` native image and PDF viewer.
//!
//! The crate owns policy, state transitions, geometry, and host intents. It
//! deliberately contains no image codecs, filesystem access, clocks, or UI.

mod animation;
mod cache;
mod command;
mod geometry;
mod model;
mod navigation;

pub use animation::{ClockError, PlaybackClock, PlaybackEvent, Visibility};
pub use cache::{
    CacheKey, CachePurpose, CacheScope, Invalidation, InvalidationReason, SourceFingerprint,
    ThumbnailSize,
};
pub use command::{
    Command, CommandError, DeleteOutcome, Effect, LaunchError, LaunchPlan, Reducer, ViewerState,
};
pub use geometry::{FitPolicy, Point, Rect, Size, ViewTransform, Zoom, ZoomError};
pub use model::{
    DecodeError, DecodeErrorKind, DecodeIntent, DecodeLimits, DecodedImage, DecoderAdapter,
    ExifOrientation, ImageFormat, ImageMetadata, MetadataError, PixelTransform, Rotation, Source,
};
pub use navigation::{DirectoryImages, ImageCandidate, ReconcileOutcome};
