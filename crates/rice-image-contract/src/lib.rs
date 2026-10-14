//! Dependency-free cache identity and request contract shared by native apps.

#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceFingerprint {
    /// Filesystem identity where available, otherwise a stable adapter hash.
    pub identity: u64,
    pub modified_ms: u64,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ThumbnailSize {
    pub width: u16,
    pub height: u16,
}

impl ThumbnailSize {
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Option<Self> {
        if width == 0 || height == 0 {
            None
        } else {
            Some(Self { width, height })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CachePurpose {
    Decode,
    Thumbnail(ThumbnailSize),
}

/// Generic key keeps source and transform domain types owned by each app while
/// making fingerprint, purpose, version, and cache semantics identical.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CacheKey<Source, Orientation, Rotation> {
    pub source: Source,
    pub fingerprint: SourceFingerprint,
    pub purpose: CachePurpose,
    pub exif_orientation: Orientation,
    pub user_rotation: Rotation,
    pub codec_version: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CacheScope<Source, Orientation, Rotation> {
    Exact(CacheKey<Source, Orientation, Rotation>),
    Source(Source),
    Identity(u64),
    CodecVersion(u32),
    All,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invalidation<Source, Orientation, Rotation> {
    pub scope: CacheScope<Source, Orientation, Rotation>,
    pub reason: InvalidationReason,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidationReason {
    ContentChanged,
    Renamed,
    Trashed,
    RotationSaved,
    CodecUpgraded,
    MemoryPressure,
}

impl<Source: Eq, Orientation: Eq, Rotation: Eq> Invalidation<Source, Orientation, Rotation> {
    #[must_use]
    pub fn matches(&self, key: &CacheKey<Source, Orientation, Rotation>) -> bool {
        match &self.scope {
            CacheScope::Exact(exact) => exact == key,
            CacheScope::Source(source) => source == &key.source,
            CacheScope::Identity(identity) => *identity == key.fingerprint.identity,
            CacheScope::CodecVersion(version) => *version == key.codec_version,
            CacheScope::All => true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageRequest<Key> {
    pub key: Key,
    pub maximum_decoded_bytes: u64,
}
