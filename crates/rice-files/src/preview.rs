//! `RiceFiles` preview request over the shared `RiceFiles`/`RiceView` cache contract.

use crate::{EntryMetadata, GuestPath};

use rice_image_contract::{CacheKey, ImageRequest, ThumbnailSize};
pub use rice_image_contract::{
    CachePurpose as PreviewPurpose, SourceFingerprint as PreviewFingerprint,
};

pub type PreviewCacheKey = CacheKey<GuestPath, u8, i16>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreviewIntent(pub ImageRequest<PreviewCacheKey>);

impl PreviewIntent {
    #[must_use]
    pub fn for_entry(
        entry: &EntryMetadata,
        identity: u64,
        width: u16,
        height: u16,
        codec_version: u32,
    ) -> Option<Self> {
        let size = ThumbnailSize::new(width, height)?;
        if codec_version == 0 {
            return None;
        }
        Some(Self(ImageRequest {
            key: CacheKey {
                source: entry.path.clone(),
                fingerprint: PreviewFingerprint {
                    identity,
                    modified_ms: u64::try_from(entry.modified_unix_ms).unwrap_or_default(),
                    byte_len: entry.size,
                },
                purpose: PreviewPurpose::Thumbnail(size),
                exif_orientation: 1,
                user_rotation: 0,
                codec_version,
            },
            maximum_decoded_bytes: u64::from(width)
                .saturating_mul(u64::from(height))
                .saturating_mul(4),
        }))
    }

    #[must_use]
    pub fn key(&self) -> &PreviewCacheKey {
        &self.0.key
    }

    #[must_use]
    pub const fn maximum_decoded_bytes(&self) -> u64 {
        self.0.maximum_decoded_bytes
    }
}

/// Formats for which the host can produce a bounded content thumbnail.
#[must_use]
pub fn supports_thumbnail(mime: &str) -> bool {
    mime.starts_with("image/") || mime.starts_with("video/") || matches!(mime,
        "application/x-msdownload" | "application/pdf" |
        "application/vnd.ms-excel" |
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" |
        "application/vnd.oasis.opendocument.spreadsheet" |
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" |
        "application/vnd.openxmlformats-officedocument.presentationml.template" |
        "application/vnd.openxmlformats-officedocument.presentationml.slideshow" |
        "application/vnd.oasis.opendocument.presentation" |
        "application/vnd.oasis.opendocument.presentation-template")
}
