use crate::{MAX_TEXTURE_BYTES, MAX_TEXTURE_DIMENSION, ProtocolError};

/// Validate a guest-decoded RGBA texture resource.
///
/// This validates an individual image resource; display-list submission is
/// still required, so the operation cannot act as a whole-window framebuffer.
///
/// # Errors
///
/// Returns an error for zero/oversized dimensions or mismatched byte counts.
pub fn validate_texture(width: u32, height: u32, rgba: &[u8]) -> Result<(), ProtocolError> {
    if width == 0
        || height == 0
        || width > MAX_TEXTURE_DIMENSION
        || height > MAX_TEXTURE_DIMENSION
        || rgba.len() > MAX_TEXTURE_BYTES
    {
        return Err(ProtocolError::InvalidTexture);
    }
    let required = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ProtocolError::InvalidTexture)?;
    if required != rgba.len() {
        return Err(ProtocolError::InvalidTexture);
    }
    Ok(())
}
