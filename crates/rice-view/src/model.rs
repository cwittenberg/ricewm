use core::fmt;

/// Maximum accepted source path/URI length.
pub const MAX_SOURCE_LEN: usize = 4_096;
/// Maximum diagnostic length crossing the decoder boundary.
pub const MAX_ERROR_LEN: usize = 1_024;

/// A validated guest path or URI.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Source(String);

impl Source {
    /// Validates a path or URI without performing I/O.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError`] for invalid, oversized, or unsupported input.
    pub fn new(value: impl Into<String>) -> Result<Self, MetadataError> {
        let value = normalize_source(value.into())?;
        if value.is_empty() {
            return Err(MetadataError::EmptySource);
        }
        if value.len() > MAX_SOURCE_LEN {
            return Err(MetadataError::SourceTooLong);
        }
        if value
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        {
            return Err(MetadataError::InvalidSource);
        }
        if !value.starts_with('/') {
            return Err(MetadataError::InvalidSource);
        }
        if value.contains("://") {
            return Err(MetadataError::UnsupportedUri);
        }
        if value.split('/').any(|part| matches!(part, "." | "..")) {
            return Err(MetadataError::InvalidSource);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn extension(&self) -> Option<&str> {
        let path = self.0.split(['?', '#']).next().unwrap_or(&self.0);
        let name = path.rsplit('/').next().unwrap_or(path);
        let (_, extension) = name.rsplit_once('.')?;
        (!extension.is_empty()).then_some(extension)
    }
}

fn normalize_source(value: String) -> Result<String, MetadataError> {
    let Some(uri) = value.strip_prefix("file://") else {
        return Ok(value);
    };
    let encoded_path = if let Some(path) = uri.strip_prefix("localhost/") {
        format!("/{path}")
    } else if uri.starts_with('/') {
        uri.to_owned()
    } else {
        return Err(MetadataError::UnsupportedUri);
    };
    percent_decode_path(&encoded_path)
}

fn percent_decode_path(value: &str) -> Result<String, MetadataError> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            decoded.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = bytes
            .get(index + 1)
            .copied()
            .and_then(hex_value)
            .ok_or(MetadataError::InvalidSource)?;
        let low = bytes
            .get(index + 2)
            .copied()
            .and_then(hex_value)
            .ok_or(MetadataError::InvalidSource)?;
        decoded.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(decoded).map_err(|_| MetadataError::InvalidSource)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    WebP,
    Gif,
    Bmp,
    Svg,
    /// A PDF page rendered by the trusted host document adapter.
    Pdf,
}

impl ImageFormat {
    #[must_use]
    pub fn from_extension(extension: &str) -> Option<Self> {
        if extension.eq_ignore_ascii_case("png") {
            Some(Self::Png)
        } else if extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg") {
            Some(Self::Jpeg)
        } else if extension.eq_ignore_ascii_case("webp") {
            Some(Self::WebP)
        } else if extension.eq_ignore_ascii_case("gif") {
            Some(Self::Gif)
        } else if extension.eq_ignore_ascii_case("bmp") {
            Some(Self::Bmp)
        } else if extension.eq_ignore_ascii_case("svg") || extension.eq_ignore_ascii_case("svgz") {
            Some(Self::Svg)
        } else if extension.eq_ignore_ascii_case("pdf") {
            Some(Self::Pdf)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeLimits {
    pub max_width: u32,
    pub max_height: u32,
    pub max_pixels: u64,
    pub max_frames: u32,
    pub max_decoded_bytes: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_width: 32_768,
            max_height: 32_768,
            max_pixels: 268_435_456,
            max_frames: 1_000,
            max_decoded_bytes: 512 * 1_024 * 1_024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ExifOrientation {
    Normal = 1,
    MirrorHorizontal = 2,
    Rotate180 = 3,
    MirrorVertical = 4,
    MirrorHorizontalRotate270 = 5,
    Rotate90 = 6,
    MirrorHorizontalRotate90 = 7,
    Rotate270 = 8,
}

impl ExifOrientation {
    /// Converts the numeric EXIF orientation value.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError::InvalidOrientation`] outside values 1–8.
    pub fn from_exif(value: u8) -> Result<Self, MetadataError> {
        match value {
            1 => Ok(Self::Normal),
            2 => Ok(Self::MirrorHorizontal),
            3 => Ok(Self::Rotate180),
            4 => Ok(Self::MirrorVertical),
            5 => Ok(Self::MirrorHorizontalRotate270),
            6 => Ok(Self::Rotate90),
            7 => Ok(Self::MirrorHorizontalRotate90),
            8 => Ok(Self::Rotate270),
            _ => Err(MetadataError::InvalidOrientation),
        }
    }

    #[must_use]
    pub const fn swaps_dimensions(self) -> bool {
        matches!(
            self,
            Self::MirrorHorizontalRotate270
                | Self::Rotate90
                | Self::MirrorHorizontalRotate90
                | Self::Rotate270
        )
    }

    #[must_use]
    pub const fn pixel_transform(self, user_rotation: Rotation) -> PixelTransform {
        let (mirror_horizontal, exif_turns) = match self {
            Self::Normal => (false, 0),
            Self::MirrorHorizontal => (true, 0),
            Self::Rotate180 => (false, 2),
            Self::MirrorVertical => (true, 2),
            Self::MirrorHorizontalRotate270 => (true, 3),
            Self::Rotate90 => (false, 1),
            Self::MirrorHorizontalRotate90 => (true, 1),
            Self::Rotate270 => (false, 3),
        };
        PixelTransform {
            mirror_horizontal,
            quarter_turns_clockwise: (exif_turns + user_rotation.quarter_turns()) % 4,
        }
    }
}

/// Canonical pixel transform: mirror first, then rotate clockwise.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PixelTransform {
    pub mirror_horizontal: bool,
    pub quarter_turns_clockwise: u8,
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Rotation {
    #[default]
    None,
    Right90,
    Half,
    Left90,
}

impl Rotation {
    #[must_use]
    pub const fn right(self) -> Self {
        match self {
            Self::None => Self::Right90,
            Self::Right90 => Self::Half,
            Self::Half => Self::Left90,
            Self::Left90 => Self::None,
        }
    }

    #[must_use]
    pub const fn left(self) -> Self {
        match self {
            Self::None => Self::Left90,
            Self::Left90 => Self::Half,
            Self::Half => Self::Right90,
            Self::Right90 => Self::None,
        }
    }

    #[must_use]
    pub const fn swaps_dimensions(self) -> bool {
        matches!(self, Self::Right90 | Self::Left90)
    }

    #[must_use]
    pub const fn quarter_turns(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Right90 => 1,
            Self::Half => 2,
            Self::Left90 => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageMetadata {
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    pub frame_count: u32,
    pub orientation: ExifOrientation,
    pub decoded_bytes: u64,
}

impl ImageMetadata {
    /// Validates metadata against a resource budget.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError`] when any supplied limit is exceeded.
    pub fn validate(self, limits: DecodeLimits) -> Result<Self, MetadataError> {
        if self.width == 0 || self.height == 0 {
            return Err(MetadataError::EmptyDimensions);
        }
        if self.width > limits.max_width || self.height > limits.max_height {
            return Err(MetadataError::DimensionsExceeded);
        }
        let pixels = u64::from(self.width)
            .checked_mul(u64::from(self.height))
            .ok_or(MetadataError::PixelsExceeded)?;
        if pixels > limits.max_pixels {
            return Err(MetadataError::PixelsExceeded);
        }
        if self.frame_count == 0 || self.frame_count > limits.max_frames {
            return Err(MetadataError::FrameCountExceeded);
        }
        if self.decoded_bytes > limits.max_decoded_bytes {
            return Err(MetadataError::DecodedBytesExceeded);
        }
        Ok(self)
    }

    #[must_use]
    pub const fn oriented_dimensions(&self, user_rotation: Rotation) -> (u32, u32) {
        if self
            .orientation
            .pixel_transform(user_rotation)
            .quarter_turns_clockwise
            % 2
            == 1
        {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedImage {
    pub metadata: ImageMetadata,
    /// Adapter-owned opaque handle; pixel payloads never enter the model.
    pub handle: u64,
}

impl DecodedImage {
    /// Validates an opaque decoded-image result.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError`] for an invalid handle or metadata.
    pub fn validate(self, limits: DecodeLimits) -> Result<Self, MetadataError> {
        if self.handle == 0 {
            return Err(MetadataError::InvalidHandle);
        }
        self.metadata.clone().validate(limits)?;
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeIntent {
    pub source: Source,
    pub limits: DecodeLimits,
    pub preferred_format: Option<ImageFormat>,
}

/// Strategy boundary implemented by the browser/native codec adapter.
pub trait DecoderAdapter {
    /// Resolves an intent to an opaque decoded image.
    ///
    /// # Errors
    ///
    /// Returns a bounded [`DecodeError`] for adapter failures.
    fn decode(&mut self, intent: &DecodeIntent) -> Result<DecodedImage, DecodeError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeErrorKind {
    Unsupported,
    Corrupt,
    LimitExceeded,
    PermissionDenied,
    NotFound,
    AdapterFailure,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodeError {
    pub kind: DecodeErrorKind,
    message: String,
}

impl DecodeError {
    pub fn new(kind: DecodeErrorKind, message: impl Into<String>) -> Self {
        let mut message = message.into();
        if message.len() > MAX_ERROR_LEN {
            let mut boundary = MAX_ERROR_LEN;
            while !message.is_char_boundary(boundary) {
                boundary -= 1;
            }
            message.truncate(boundary);
        }
        Self { kind, message }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataError {
    EmptySource,
    SourceTooLong,
    InvalidSource,
    UnsupportedUri,
    EmptyDimensions,
    DimensionsExceeded,
    PixelsExceeded,
    FrameCountExceeded,
    DecodedBytesExceeded,
    InvalidOrientation,
    InvalidHandle,
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for MetadataError {}
