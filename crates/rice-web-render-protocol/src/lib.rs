//! Bounded retained-rendering protocol for guest web engines.
//!
//! This crate deliberately models paint primitives rather than pixels for a
//! complete page. It is suitable for Bowl's in-Wasm paint output and for
//! validation on both sides of `RiceWM`'s existing native-app/native-UI
//! boundary.

use core::fmt;

mod command;
mod decode_helpers;
mod texture;
mod scene;
pub use scene::{MAX_SCENE_BYTES, MAX_SCENE_DIMENSION};

pub use command::Command;
use decode_helpers::{
    finite, nonnegative, nonnegative_array, read_f32, read_f32_array, read_u16, read_u32,
    read_u32_array,
};
pub use texture::validate_texture;

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 6;
pub const COMMAND_HEADER_BYTES: usize = 8;
pub const MAX_TRANSACTION_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_COMMANDS: usize = 262_144;
pub const MAX_STACK_DEPTH: usize = 256;
pub const MAX_GRADIENT_STOPS: usize = 64;
pub const MAX_GLYPHS_PER_RUN: usize = 65_535;
pub const MAX_PATH_SEGMENTS: usize = 65_535;
pub const MAX_TEXTURE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_TEXTURE_DIMENSION: u32 = 16_384;

pub const PUSH_STACKING_CONTEXT: u8 = 1;
pub const POP_STACKING_CONTEXT: u8 = 2;
pub const PUSH_CLIP: u8 = 3;
pub const POP_CLIP: u8 = 4;
pub const SOLID_RECT: u8 = 5;
pub const LINEAR_GRADIENT: u8 = 6;
pub const IMAGE: u8 = 7;
pub const GLYPH_RUN: u8 = 8;
pub const BORDER: u8 = 9;
pub const BOX_SHADOW: u8 = 10;
pub const LINE: u8 = 11;
pub const POLYGON: u8 = 12;
pub const ARC: u8 = 13;
/// Defines one decoded RGBA image resource used by subsequent `IMAGE`
/// commands. This is an individual retained page asset, never a page-sized
/// framebuffer.
pub const DEFINE_IMAGE: u8 = 14;
/// Retained semantic hit region emitted for an interactive DOM node.
pub const HIT_REGION: u8 = 15;
/// Semantic retained link region. The URL is navigation data, not host-rendered
/// content; `RiceWM` routes it back to the Bowl guest browser actor.
pub const LINK_REGION: u8 = 16;
pub const MAX_LINK_URL_BYTES: usize = 4 * 1024;
/// Semantic retained form-control region including the guest-owned current
/// value. Password controls carry an empty semantic value. This lets `RiceWM`
/// edit text without scraping painted glyphs or exporting a password.
pub const INPUT_REGION: u8 = 17;
pub const INPUT_FLAG_DISABLED: u8 = 1;
pub const MAX_INPUT_VALUE_BYTES: usize = 4 * 1024;
/// Guest layout extent and the clamped viewport origin used for this frame.
pub const DOCUMENT_METRICS: u8 = 18;
/// Defines reusable guest-produced vector geometry. Each 28-byte segment is a
/// verb plus up to three finite coordinate pairs. Paths are retained geometry,
/// never a rasterized page.
pub const DEFINE_PATH: u8 = 19;
pub const PUSH_PATH_CLIP: u8 = 20;
pub const FILL_PATH: u8 = 21;
pub const STROKE_PATH: u8 = 22;
/// Native HTML `title` metadata for the smallest painted element under the
/// pointer. RiceWM presents this above the web surface without making Bowl
/// rasterize a tooltip into the page.
pub const TITLE_REGION: u8 = 23;
pub const MAX_TITLE_BYTES: usize = 4 * 1024;
/// Semantic image region used by Bowl's trusted image context actions. The
/// URL remains guest/browser data and is never fetched by the compositor.
pub const IMAGE_REGION: u8 = 24;
pub const MAX_IMAGE_URL_BYTES: usize = 4 * 1024;
/// A retained two-circle radial gradient. The browser compositor maps this
/// directly to Canvas2D's `createRadialGradient` without allocating a raster
/// tile or flattening the page.
pub const RADIAL_GRADIENT: u8 = 25;
/// A retained angular/sweep gradient. `extend` is validated as pad, repeat or
/// reflect so CSS conic gradients do not silently degrade to a solid fill.
pub const SWEEP_GRADIENT: u8 = 26;
/// An opaque media session painted in document order. The compositor must
/// authorize ownership before resolving the surface; no URL is carried.
pub const MEDIA_SURFACE: u8 = 27;
/// Inert HTML presented as a selectable, sandboxed browser document.
pub const HTML_DOCUMENT: u8 = 28;
/// Isolate enclosed retained paint and cast its composited alpha behind it.
pub const PUSH_DROP_SHADOW: u8 = 29;
pub const POP_DROP_SHADOW: u8 = 30;
pub const MAX_SHADOW_DEPTH: usize = 4;
pub const MAX_SHADOW_SCOPES: usize = 128;
pub const MAX_HTML_DOCUMENT_BYTES: usize = 512 * 1024;

pub fn html_document(html: &str) -> Result<Vec<u8>, ProtocolError> {
    if html.len() > MAX_HTML_DOCUMENT_BYTES { return Err(ProtocolError::TooLarge); }
    let length = (12 + html.len() + 3) & !3;
    let mut bytes = vec![0; length];
    bytes[0] = HTML_DOCUMENT;
    bytes[4..8].copy_from_slice(&(length as u32).to_le_bytes());
    bytes[8..12].copy_from_slice(&(html.len() as u32).to_le_bytes());
    bytes[12..12 + html.len()].copy_from_slice(html.as_bytes());
    Ok(bytes)
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackKind {
    StackingContext,
    Clip,
    DropShadow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    TooLarge,
    Truncated,
    Misaligned,
    UnknownCommand(u8),
    InvalidLength,
    InvalidNumber,
    InvalidSampling,
    InvalidGradientStops,
    InvalidGlyphs,
    StackOverflow,
    StackUnderflow(StackKind),
    UnbalancedStack(StackKind),
    TooManyCommands,
    InvalidTexture,
    NonVectorScene,
    InvalidSceneResource,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayList<'a> {
    bytes: &'a [u8],
}

impl<'a> DisplayList<'a> {
    /// Validate a complete display list, including balanced clip and stacking
    /// context scopes.
    ///
    /// # Errors
    ///
    /// Returns a bounded protocol error for malformed or unsupported input.
    pub fn validate(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        if bytes.len() > MAX_TRANSACTION_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        if !bytes.len().is_multiple_of(4) {
            return Err(ProtocolError::Misaligned);
        }
        let mut shadow_depth = 0usize;
        let mut shadow_count = 0usize;
        let mut shadow_nesting = Vec::new();
        let mut stacking_depth = 0usize;
        let mut clip_depth = 0usize;
        let mut commands = 0usize;
        let mut decoder = Decoder { remaining: bytes };
        while let Some(command) = decoder.next_command()? {
            commands = commands.saturating_add(1);
            if commands > MAX_COMMANDS {
                return Err(ProtocolError::TooManyCommands);
            }
            match command {
                Command::PushDropShadow { .. } => {
                    shadow_depth += 1;
                    shadow_count += 1;
                    shadow_nesting.push((stacking_depth, clip_depth));
                    if shadow_count > MAX_SHADOW_SCOPES { return Err(ProtocolError::TooManyCommands); }
                    if shadow_depth > MAX_SHADOW_DEPTH { return Err(ProtocolError::StackOverflow); }
                }
                Command::PopDropShadow => {
                    if shadow_nesting.pop() != Some((stacking_depth, clip_depth)) {
                        return Err(ProtocolError::UnbalancedStack(StackKind::DropShadow));
                    }
                    shadow_depth = shadow_depth.checked_sub(1).ok_or(ProtocolError::StackUnderflow(StackKind::DropShadow))?;
                }
                Command::HtmlDocument { .. } if commands != 1 || !decoder.remaining.is_empty() => {
                    return Err(ProtocolError::InvalidLength);
                }
                Command::PushStackingContext { .. } => {
                    stacking_depth = stacking_depth.saturating_add(1);
                    if stacking_depth > MAX_STACK_DEPTH {
                        return Err(ProtocolError::StackOverflow);
                    }
                }
                Command::PopStackingContext => {
                    stacking_depth = stacking_depth
                        .checked_sub(1)
                        .ok_or(ProtocolError::StackUnderflow(StackKind::StackingContext))?;
                    if shadow_nesting.last().is_some_and(|(depth, _)| stacking_depth < *depth) {
                        return Err(ProtocolError::UnbalancedStack(StackKind::DropShadow));
                    }
                }
                Command::PushClip { .. } => {
                    clip_depth = clip_depth.saturating_add(1);
                    if clip_depth > MAX_STACK_DEPTH {
                        return Err(ProtocolError::StackOverflow);
                    }
                }
                Command::PushPathClip { .. } => {
                    clip_depth = clip_depth.saturating_add(1);
                    if clip_depth > MAX_STACK_DEPTH {
                        return Err(ProtocolError::StackOverflow);
                    }
                }
                Command::PopClip => {
                    clip_depth = clip_depth
                        .checked_sub(1)
                        .ok_or(ProtocolError::StackUnderflow(StackKind::Clip))?;
                    if shadow_nesting.last().is_some_and(|(_, depth)| clip_depth < *depth) {
                        return Err(ProtocolError::UnbalancedStack(StackKind::DropShadow));
                    }
                }
                _ => {}
            }
        }
        if shadow_depth != 0 { return Err(ProtocolError::UnbalancedStack(StackKind::DropShadow)); }
        if stacking_depth != 0 {
            return Err(ProtocolError::UnbalancedStack(StackKind::StackingContext));
        }
        if clip_depth != 0 {
            return Err(ProtocolError::UnbalancedStack(StackKind::Clip));
        }
        Ok(Self { bytes })
    }

    #[must_use]
    pub const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[must_use]
    pub const fn commands(&self) -> Decoder<'a> {
        Decoder {
            remaining: self.bytes,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Decoder<'a> {
    remaining: &'a [u8],
}

impl<'a> Iterator for Decoder<'a> {
    type Item = Result<Command<'a>, ProtocolError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_command() {
            Ok(Some(command)) => Some(Ok(command)),
            Ok(None) => None,
            Err(error) => {
                self.remaining = &[];
                Some(Err(error))
            }
        }
    }
}

impl<'a> Decoder<'a> {
    fn next_command(&mut self) -> Result<Option<Command<'a>>, ProtocolError> {
        if self.remaining.is_empty() {
            return Ok(None);
        }
        let header = self
            .remaining
            .get(..COMMAND_HEADER_BYTES)
            .ok_or(ProtocolError::Truncated)?;
        let opcode = header[0];
        if header[1] != 0 {
            return Err(ProtocolError::InvalidLength);
        }
        let compact_words = read_u16(&header[2..4]);
        let extended_bytes = read_u32(&header[4..8]);
        let bytes = if compact_words == 0 {
            usize::try_from(extended_bytes).map_err(|_| ProtocolError::InvalidLength)?
        } else {
            if extended_bytes != 0 {
                return Err(ProtocolError::InvalidLength);
            }
            usize::from(compact_words)
                .checked_mul(4)
                .ok_or(ProtocolError::InvalidLength)?
        };
        if bytes < COMMAND_HEADER_BYTES || !bytes.is_multiple_of(4) {
            return Err(ProtocolError::InvalidLength);
        }
        let command = self
            .remaining
            .get(..bytes)
            .ok_or(ProtocolError::Truncated)?;
        self.remaining = &self.remaining[bytes..];
        decode_command(opcode, command)
    }
}

// Keeping the exhaustive opcode/length matrix together makes the security
// boundary auditable and prevents subtly different validation between
// command groups.
#[allow(clippy::too_many_lines)]
fn decode_command(opcode: u8, command: &[u8]) -> Result<Option<Command<'_>>, ProtocolError> {
    let payload = &command[COMMAND_HEADER_BYTES..];
    let decoded = match opcode {
        HTML_DOCUMENT if payload.len() >= 4 => {
            let length = read_u32(&payload[..4]) as usize;
            if length > MAX_HTML_DOCUMENT_BYTES || (4 + length + 3) & !3 != payload.len() {
                return Err(ProtocolError::InvalidLength);
            }
            let html = core::str::from_utf8(&payload[4..4 + length])
                .map_err(|_| ProtocolError::InvalidLength)?;
            Command::HtmlDocument { html }
        }
        PUSH_STACKING_CONTEXT if matches!(payload.len(), 28 | 32) => Command::PushStackingContext {
            transform: read_f32_array::<6>(&payload[..24])?,
            opacity: finite(read_f32(&payload[24..28]))?,
            blend: if payload.len() == 32 {
                if payload[28] > 15 || payload[29] > 13 || payload[30..32] != [0, 0] {
                    return Err(ProtocolError::InvalidNumber);
                }
                [payload[28], payload[29]]
            } else {
                [0, 3]
            },
        },
        POP_STACKING_CONTEXT if payload.is_empty() => Command::PopStackingContext,
        PUSH_CLIP if payload.len() == 32 => Command::PushClip {
            rect: read_f32_array::<4>(&payload[..16])?,
            radii: nonnegative_array(read_f32_array::<4>(&payload[16..32])?)?,
        },
        POP_CLIP if payload.is_empty() => Command::PopClip,
        SOLID_RECT if payload.len() == 20 => Command::SolidRect {
            rect: read_f32_array::<4>(&payload[..16])?,
            color: read_u32(&payload[16..20]),
        },
        LINEAR_GRADIENT if payload.len() >= 36 => {
            let count = usize::from(read_u16(&payload[32..34]));
            if payload[34] != 0 || payload[35] != 0 || count > MAX_GRADIENT_STOPS {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let stop_bytes = count
                .checked_mul(8)
                .ok_or(ProtocolError::InvalidGradientStops)?;
            if payload.len() != 36usize.saturating_add(stop_bytes) {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let stops = &payload[36..];
            for stop in stops.chunks_exact(8) {
                let offset = read_f32(&stop[..4]);
                if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
                    return Err(ProtocolError::InvalidGradientStops);
                }
            }
            Command::LinearGradient {
                rect: read_f32_array::<4>(&payload[..16])?,
                start: read_f32_array::<2>(&payload[16..24])?,
                end: read_f32_array::<2>(&payload[24..32])?,
                stops,
            }
        }
        RADIAL_GRADIENT if payload.len() >= 44 => {
            let count = usize::from(read_u16(&payload[40..42]));
            if payload[42] != 0 || payload[43] != 0 || count > MAX_GRADIENT_STOPS {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let stop_bytes = count
                .checked_mul(8)
                .ok_or(ProtocolError::InvalidGradientStops)?;
            if payload.len() != 44usize.saturating_add(stop_bytes) {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let start_radius = nonnegative(read_f32(&payload[24..28]))?;
            let end_radius = nonnegative(read_f32(&payload[36..40]))?;
            let stops = &payload[44..];
            for stop in stops.chunks_exact(8) {
                let offset = read_f32(&stop[..4]);
                if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
                    return Err(ProtocolError::InvalidGradientStops);
                }
            }
            Command::RadialGradient {
                rect: read_f32_array::<4>(&payload[..16])?,
                start_center: read_f32_array::<2>(&payload[16..24])?,
                start_radius,
                end_center: read_f32_array::<2>(&payload[28..36])?,
                end_radius,
                stops,
            }
        }
        SWEEP_GRADIENT if payload.len() >= 40 => {
            let extend = payload[32];
            let count = usize::from(read_u16(&payload[36..38]));
            if extend > 2
                || payload[33..36] != [0, 0, 0]
                || payload[38..40] != [0, 0]
                || count > MAX_GRADIENT_STOPS
            {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let stop_bytes = count
                .checked_mul(8)
                .ok_or(ProtocolError::InvalidGradientStops)?;
            if payload.len() != 40usize.saturating_add(stop_bytes) {
                return Err(ProtocolError::InvalidGradientStops);
            }
            let stops = &payload[40..];
            for stop in stops.chunks_exact(8) {
                let offset = read_f32(&stop[..4]);
                if !offset.is_finite() || !(0.0..=1.0).contains(&offset) {
                    return Err(ProtocolError::InvalidGradientStops);
                }
            }
            Command::SweepGradient {
                rect: read_f32_array::<4>(&payload[..16])?,
                center: read_f32_array::<2>(&payload[16..24])?,
                start_angle: finite(read_f32(&payload[24..28]))?,
                end_angle: finite(read_f32(&payload[28..32]))?,
                extend,
                stops,
            }
        }
        IMAGE if payload.len() == 44 => {
            let texture = read_u32(&payload[32..36]);
            let alpha = finite(read_f32(&payload[36..40]))?;
            let sampling = payload[40];
            if texture == 0 {
                return Err(ProtocolError::InvalidTexture);
            }
            if !(0.0..=1.0).contains(&alpha) {
                return Err(ProtocolError::InvalidNumber);
            }
            if sampling > 1 || payload[41..44] != [0, 0, 0] {
                return Err(ProtocolError::InvalidSampling);
            }
            Command::Image {
                rect: read_f32_array::<4>(&payload[..16])?,
                uv: read_f32_array::<4>(&payload[16..32])?,
                texture,
                alpha,
                sampling,
            }
        }
        DEFINE_IMAGE if payload.len() >= 16 => {
            let texture = read_u32(&payload[..4]);
            let width = read_u32(&payload[4..8]);
            let height = read_u32(&payload[8..12]);
            let byte_len = usize::try_from(read_u32(&payload[12..16]))
                .map_err(|_| ProtocolError::InvalidTexture)?;
            if texture == 0 || payload.len() != 16usize.saturating_add(byte_len) {
                return Err(ProtocolError::InvalidTexture);
            }
            let rgba = &payload[16..];
            validate_texture(width, height, rgba)?;
            Command::DefineImage {
                texture,
                width,
                height,
                rgba,
            }
        }
        HIT_REGION if payload.len() == 28 => {
            let node = u64::from_le_bytes(
                payload[16..24]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidLength)?,
            );
            let kind = payload[24];
            if node == 0 || kind > 4 || payload[25..28] != [0, 0, 0] {
                return Err(ProtocolError::InvalidLength);
            }
            Command::HitRegion {
                rect: read_f32_array::<4>(&payload[..16])?,
                node,
                kind,
            }
        }
        LINK_REGION if payload.len() >= 28 => {
            let node = u64::from_le_bytes(
                payload[16..24]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidLength)?,
            );
            let url_len = usize::try_from(read_u32(&payload[24..28]))
                .map_err(|_| ProtocolError::InvalidLength)?;
            let padded = url_len
                .checked_add(3)
                .map(|length| length & !3)
                .ok_or(ProtocolError::InvalidLength)?;
            if node == 0
                || url_len == 0
                || url_len > MAX_LINK_URL_BYTES
                || payload.len() != 28usize.saturating_add(padded)
                || payload[28 + url_len..].iter().any(|byte| *byte != 0)
            {
                return Err(ProtocolError::InvalidLength);
            }
            let url = &payload[28..28 + url_len];
            if core::str::from_utf8(url).is_err() || url.contains(&0) {
                return Err(ProtocolError::InvalidLength);
            }
            Command::LinkRegion {
                rect: read_f32_array::<4>(&payload[..16])?,
                node,
                url,
            }
        }
        TITLE_REGION if payload.len() >= 28 => {
            let node = u64::from_le_bytes(
                payload[16..24]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidLength)?,
            );
            let title_len = usize::try_from(read_u32(&payload[24..28]))
                .map_err(|_| ProtocolError::InvalidLength)?;
            let padded = title_len
                .checked_add(3)
                .map(|length| length & !3)
                .ok_or(ProtocolError::InvalidLength)?;
            if node == 0
                || title_len == 0
                || title_len > MAX_TITLE_BYTES
                || payload.len() != 28usize.saturating_add(padded)
                || payload[28 + title_len..].iter().any(|byte| *byte != 0)
            {
                return Err(ProtocolError::InvalidLength);
            }
            let title = &payload[28..28 + title_len];
            if core::str::from_utf8(title).is_err() || title.contains(&0) {
                return Err(ProtocolError::InvalidLength);
            }
            Command::TitleRegion {
                rect: read_f32_array::<4>(&payload[..16])?,
                node,
                title,
            }
        }
        MEDIA_SURFACE if payload.len() == 24 => {
            let session = u64::from_le_bytes(payload[16..24].try_into().unwrap());
            if session == 0 { return Err(ProtocolError::InvalidLength); }
            Command::MediaSurface { rect: read_f32_array::<4>(&payload[..16])?, session }
        }
        IMAGE_REGION if payload.len() >= 28 => {
            let node = u64::from_le_bytes(
                payload[16..24]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidLength)?,
            );
            let url_len = usize::try_from(read_u32(&payload[24..28]))
                .map_err(|_| ProtocolError::InvalidLength)?;
            let padded = url_len
                .checked_add(3)
                .map(|length| length & !3)
                .ok_or(ProtocolError::InvalidLength)?;
            if node == 0
                || url_len == 0
                || url_len > MAX_IMAGE_URL_BYTES
                || payload.len() != 28usize.saturating_add(padded)
                || payload[28 + url_len..].iter().any(|byte| *byte != 0)
            {
                return Err(ProtocolError::InvalidLength);
            }
            let url = &payload[28..28 + url_len];
            if core::str::from_utf8(url).is_err() || url.contains(&0) {
                return Err(ProtocolError::InvalidLength);
            }
            Command::ImageRegion {
                rect: read_f32_array::<4>(&payload[..16])?,
                node,
                url,
            }
        }
        INPUT_REGION if payload.len() >= 32 => {
            let node = u64::from_le_bytes(
                payload[16..24]
                    .try_into()
                    .map_err(|_| ProtocolError::InvalidLength)?,
            );
            let kind = payload[24];
            let flags = payload[25];
            let value_len = usize::try_from(read_u32(&payload[28..32]))
                .map_err(|_| ProtocolError::InvalidLength)?;
            let padded = value_len
                .checked_add(3)
                .map(|length| length & !3)
                .ok_or(ProtocolError::InvalidLength)?;
            if node == 0
                || !(2..=3).contains(&kind)
                || flags & !INPUT_FLAG_DISABLED != 0
                || payload[26..28] != [0, 0]
                || value_len > MAX_INPUT_VALUE_BYTES
                || payload.len() != 32usize.saturating_add(padded)
                || payload[32 + value_len..].iter().any(|byte| *byte != 0)
            {
                return Err(ProtocolError::InvalidLength);
            }
            let value = &payload[32..32 + value_len];
            if core::str::from_utf8(value).is_err() || value.contains(&0) {
                return Err(ProtocolError::InvalidLength);
            }
            Command::InputRegion {
                rect: read_f32_array::<4>(&payload[..16])?,
                node,
                kind,
                flags,
                value,
            }
        }
        DOCUMENT_METRICS if payload.len() == 16 => {
            let content_width = read_u32(&payload[..4]);
            let content_height = read_u32(&payload[4..8]);
            let scroll_x = read_u32(&payload[8..12]);
            let scroll_y = read_u32(&payload[12..16]);
            if content_width == 0
                || content_height == 0
                || scroll_x > content_width
                || scroll_y > content_height
            {
                return Err(ProtocolError::InvalidLength);
            }
            Command::DocumentMetrics {
                content_width,
                content_height,
                scroll_x,
                scroll_y,
            }
        }
        GLYPH_RUN if payload.len() >= 20 => {
            let count = usize::try_from(read_u32(&payload[16..20]))
                .map_err(|_| ProtocolError::InvalidGlyphs)?;
            if count > MAX_GLYPHS_PER_RUN
                || payload.len() != 20usize.saturating_add(count.saturating_mul(12))
            {
                return Err(ProtocolError::InvalidGlyphs);
            }
            let glyphs = &payload[20..];
            for glyph in glyphs.chunks_exact(12) {
                finite(read_f32(&glyph[4..8]))?;
                finite(read_f32(&glyph[8..12]))?;
            }
            Command::GlyphRun {
                font_instance: read_u32(&payload[..4]),
                origin: read_f32_array::<2>(&payload[4..12])?,
                color: read_u32(&payload[12..16]),
                glyphs,
            }
        }
        BORDER if payload.len() == 64 => Command::Border {
            rect: read_f32_array::<4>(&payload[..16])?,
            widths: nonnegative_array(read_f32_array::<4>(&payload[16..32])?)?,
            colors: read_u32_array::<4>(&payload[32..48]),
            radii: nonnegative_array(read_f32_array::<4>(&payload[48..64])?)?,
        },
        PUSH_DROP_SHADOW if matches!(payload.len(), 32 | 56) => {
            let bounds = read_f32_array::<4>(&payload[..16])?;
            let offset = read_f32_array::<2>(&payload[16..24])?;
            let blur = nonnegative(read_f32(&payload[24..28]))?;
            if bounds[..2].iter().chain(offset.iter()).any(|value| value.abs() > 1_000_000.0)
                || bounds[2..].iter().any(|value| *value <= 0.0 || *value > 16_384.0) || blur > 1_000_000.0 {
                return Err(ProtocolError::InvalidNumber);
            }
            let transform = if payload.len() == 56 {
                let matrix = read_f32_array::<6>(&payload[32..56])?;
                // Products of finite f32 coefficients are exact in f64. Only
                // singular matrices are invalid; tiny invertible scales remain valid.
                #[allow(clippy::float_cmp)]
                let singular = f64::from(matrix[0]) * f64::from(matrix[3]) == f64::from(matrix[1]) * f64::from(matrix[2]);
                if matrix[..4].iter().any(|v| v.abs() > 1024.0)
                    || matrix[4..].iter().any(|v| v.abs() > 1_000_000.0)
                    || singular {
                    return Err(ProtocolError::InvalidNumber);
                }
                Some(matrix)
            } else { None };
            Command::PushDropShadow { bounds, offset, blur, color: read_u32(&payload[28..32]), transform }
        }
        POP_DROP_SHADOW if payload.is_empty() => Command::PopDropShadow,
        BOX_SHADOW if payload.len() == 40 => {
            if payload[37..40] != [0, 0, 0] || payload[36] > 1 {
                return Err(ProtocolError::InvalidLength);
            }
            Command::BoxShadow {
                rect: read_f32_array::<4>(&payload[..16])?,
                offset: read_f32_array::<2>(&payload[16..24])?,
                blur: nonnegative(read_f32(&payload[24..28]))?,
                spread: finite(read_f32(&payload[28..32]))?,
                color: read_u32(&payload[32..36]),
                inset: payload[36] == 1,
            }
        }
        LINE if payload.len() == 28 => {
            if payload[25..28] != [0, 0, 0] || payload[24] > 2 {
                return Err(ProtocolError::InvalidLength);
            }
            Command::Line {
                from: read_f32_array::<2>(&payload[..8])?,
                to: read_f32_array::<2>(&payload[8..16])?,
                width: nonnegative(read_f32(&payload[16..20]))?,
                color: read_u32(&payload[20..24]),
                style: payload[24],
            }
        }
        POLYGON if payload.len() >= 36 => {
            let count = usize::from(read_u16(&payload[8..10]));
            let flags = payload[10];
            if count < 3
                || flags > 3
                || payload[11] != 0
                || payload.len() != 12usize.saturating_add(count.saturating_mul(8))
            {
                return Err(ProtocolError::InvalidLength);
            }
            let points = &payload[12..];
            for point in points.chunks_exact(8) {
                finite(read_f32(&point[..4]))?;
                finite(read_f32(&point[4..8]))?;
            }
            Command::Polygon {
                color: read_u32(&payload[..4]),
                stroke_width: nonnegative(read_f32(&payload[4..8]))?,
                filled: flags & 1 != 0,
                closed: flags & 2 != 0,
                points,
            }
        }
        ARC if payload.len() == 36 => {
            if payload[33..36] != [0, 0, 0] || payload[32] > 1 {
                return Err(ProtocolError::InvalidLength);
            }
            Command::Arc {
                rect: read_f32_array::<4>(&payload[..16])?,
                start_degrees: finite(read_f32(&payload[16..20]))?,
                end_degrees: finite(read_f32(&payload[20..24]))?,
                width: nonnegative(read_f32(&payload[24..28]))?,
                color: read_u32(&payload[28..32]),
                filled: payload[32] == 1,
            }
        }
        DEFINE_PATH if payload.len() >= 8 => {
            let path = read_u32(&payload[..4]);
            let count = usize::try_from(read_u32(&payload[4..8]))
                .map_err(|_| ProtocolError::InvalidLength)?;
            if path == 0
                || count == 0
                || count > MAX_PATH_SEGMENTS
                || payload.len() != 8usize.saturating_add(count.saturating_mul(28))
            {
                return Err(ProtocolError::InvalidLength);
            }
            let segments = &payload[8..];
            for segment in segments.chunks_exact(28) {
                if segment[0] > 4 || segment[1..4] != [0, 0, 0] {
                    return Err(ProtocolError::InvalidLength);
                }
                for coordinate in segment[4..].chunks_exact(4) {
                    finite(read_f32(coordinate))?;
                }
            }
            Command::DefinePath { path, segments }
        }
        PUSH_PATH_CLIP if payload.len() == 32 => {
            let path = read_u32(&payload[..4]);
            let fill_rule = payload[28];
            if path == 0 || fill_rule > 1 || payload[29..32] != [0, 0, 0] {
                return Err(ProtocolError::InvalidLength);
            }
            Command::PushPathClip {
                path,
                transform: read_f32_array::<6>(&payload[4..28])?,
                fill_rule,
            }
        }
        FILL_PATH if payload.len() == 36 => {
            let path = read_u32(&payload[..4]);
            let fill_rule = payload[32];
            if path == 0 || fill_rule > 1 || payload[33..36] != [0, 0, 0] {
                return Err(ProtocolError::InvalidLength);
            }
            Command::FillPath {
                path,
                transform: read_f32_array::<6>(&payload[4..28])?,
                color: read_u32(&payload[28..32]),
                fill_rule,
            }
        }
        STROKE_PATH if payload.len() == 44 => {
            let path = read_u32(&payload[..4]);
            let line_cap = payload[40];
            let line_join = payload[41];
            if path == 0 || line_cap > 2 || line_join > 2 || payload[42..44] != [0, 0] {
                return Err(ProtocolError::InvalidLength);
            }
            Command::StrokePath {
                path,
                transform: read_f32_array::<6>(&payload[4..28])?,
                color: read_u32(&payload[28..32]),
                width: nonnegative(read_f32(&payload[32..36]))?,
                miter_limit: nonnegative(read_f32(&payload[36..40]))?,
                line_cap,
                line_join,
            }
        }
        PUSH_STACKING_CONTEXT
        | POP_STACKING_CONTEXT
        | PUSH_CLIP
        | POP_CLIP
        | SOLID_RECT
        | LINEAR_GRADIENT
        | IMAGE
        | GLYPH_RUN
        | BORDER
        | BOX_SHADOW
        | PUSH_DROP_SHADOW
        | POP_DROP_SHADOW
        | LINE
        | POLYGON
        | ARC
        | DEFINE_IMAGE
        | HIT_REGION
        | LINK_REGION
        | INPUT_REGION
        | DOCUMENT_METRICS
        | DEFINE_PATH
        | PUSH_PATH_CLIP
        | FILL_PATH
        | STROKE_PATH
        | TITLE_REGION
        | IMAGE_REGION
        | MEDIA_SURFACE
        | RADIAL_GRADIENT
        | SWEEP_GRADIENT => {
            return Err(ProtocolError::InvalidLength);
        }
        _ => return Err(ProtocolError::UnknownCommand(opcode)),
    };
    Ok(Some(decoded))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod scene_tests;

#[cfg(test)]
mod shadow_tests;
