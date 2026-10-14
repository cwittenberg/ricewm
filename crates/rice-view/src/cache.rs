//! `RiceView` aliases over the shared first-party image/cache contract.

use crate::{ExifOrientation, Rotation, Source};

pub use rice_image_contract::{CachePurpose, InvalidationReason, SourceFingerprint, ThumbnailSize};

pub type CacheKey = rice_image_contract::CacheKey<Source, ExifOrientation, Rotation>;
pub type CacheScope = rice_image_contract::CacheScope<Source, ExifOrientation, Rotation>;
pub type Invalidation = rice_image_contract::Invalidation<Source, ExifOrientation, Rotation>;
