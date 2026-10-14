//! Vector-only scenes for ordinary native applications. Browser semantics never
//! cross this boundary, even when a caller bypasses the SDK's validation.

use crate::{Command, DisplayList, ProtocolError, StackKind};
use std::collections::BTreeSet;

/// Native app wire header and `WebRenderPatch` envelope consume 56 bytes.
pub const MAX_SCENE_BYTES: usize = 8 * 1024 * 1024 - 56;
pub const MAX_SCENE_DIMENSION: u32 = 16_384;

impl<'a> DisplayList<'a> {
    /// Validate a bounded native vector scene, excluding browser and media commands.
    ///
    /// # Errors
    /// Returns an error for malformed, oversized, or non-vector input.
    pub fn validate_scene(bytes: &'a [u8]) -> Result<Self, ProtocolError> {
        // Zero bytes represent a cached scene reference in the native snapshot.
        // SDK clear frames use an explicit transparent zero-size rectangle.
        if bytes.is_empty() {
            return Err(ProtocolError::InvalidLength);
        }
        if bytes.len() > MAX_SCENE_BYTES {
            return Err(ProtocolError::TooLarge);
        }
        let list = Self::validate(bytes)?;
        let mut decoder = list.commands();
        let mut images = BTreeSet::new();
        let mut paths = BTreeSet::new();
        let mut scopes = Vec::new();
        while let Some(command) = decoder.next_command()? {
            if !matches!(
                command,
                Command::PushStackingContext { .. }
                    | Command::PopStackingContext
                    | Command::PushClip { .. }
                    | Command::PopClip
                    | Command::SolidRect { .. }
                    | Command::LinearGradient { .. }
                    | Command::RadialGradient { .. }
                    | Command::SweepGradient { .. }
                    | Command::Image { .. }
                    | Command::DefineImage { .. }
                    | Command::HitRegion { .. }
                    | Command::GlyphRun { .. }
                    | Command::Border { .. }
                    | Command::BoxShadow { .. }
                    | Command::PushDropShadow { .. }
                    | Command::PopDropShadow
                    | Command::Line { .. }
                    | Command::Polygon { .. }
                    | Command::Arc { .. }
                    | Command::DefinePath { .. }
                    | Command::PushPathClip { .. }
                    | Command::FillPath { .. }
                    | Command::StrokePath { .. }
            ) {
                return Err(ProtocolError::NonVectorScene);
            }
            // A scene is a complete immutable frame. Resources must belong to
            // this frame and precede every use; no cross-surface/epoch cache IDs.
            match command {
                Command::DefineImage { texture, .. } if !images.insert(texture) => {
                    return Err(ProtocolError::InvalidSceneResource);
                }
                Command::Image { texture, .. } if !images.contains(&texture) => {
                    return Err(ProtocolError::InvalidSceneResource);
                }
                Command::DefinePath { path, .. } if !paths.insert(path) => {
                    return Err(ProtocolError::InvalidSceneResource);
                }
                Command::PushPathClip { path, .. }
                | Command::FillPath { path, .. }
                | Command::StrokePath { path, .. }
                    if !paths.contains(&path) =>
                {
                    return Err(ProtocolError::InvalidSceneResource);
                }
                _ => {}
            }
            match command {
                Command::PushStackingContext { .. } => scopes.push(StackKind::StackingContext),
                Command::PushDropShadow { .. } => scopes.push(StackKind::DropShadow),
                Command::PopDropShadow if scopes.pop() != Some(StackKind::DropShadow) => {
                    return Err(ProtocolError::UnbalancedStack(StackKind::DropShadow));
                }
                Command::PushClip { .. } | Command::PushPathClip { .. } => {
                    scopes.push(StackKind::Clip);
                }
                Command::PopStackingContext if scopes.pop() != Some(StackKind::StackingContext) => {
                    return Err(ProtocolError::UnbalancedStack(StackKind::StackingContext));
                }
                Command::PopClip if scopes.pop() != Some(StackKind::Clip) => {
                    return Err(ProtocolError::UnbalancedStack(StackKind::Clip));
                }
                _ => {}
            }
        }
        Ok(list)
    }
}
