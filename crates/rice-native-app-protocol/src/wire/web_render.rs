use rice_web_render_protocol::DisplayList;

use crate::{MAX_WEB_DISPLAY_LIST_BYTES, WebRenderPatch};

use super::{
    DecodeError, EncodeError,
    primitives::{Reader, Writer},
};

pub(super) fn encode(writer: &mut Writer, patch: &WebRenderPatch) -> Result<(), EncodeError> {
    validate_dimensions(patch.viewport_width, patch.viewport_height)
        .map_err(|()| EncodeError::InvalidValue("web viewport"))?;
    DisplayList::validate(&patch.display_list)
        .map_err(|_| EncodeError::InvalidValue("web display list"))?;
    writer.id(patch.app);
    writer.id(patch.window);
    writer.id(patch.surface);
    writer.u64(patch.epoch);
    writer.u32(patch.viewport_width);
    writer.u32(patch.viewport_height);
    writer.bytes(
        &patch.display_list,
        MAX_WEB_DISPLAY_LIST_BYTES,
        "web display list",
    )?;
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<WebRenderPatch, DecodeError> {
    let patch = WebRenderPatch {
        app: reader.id("app id")?,
        window: reader.id("window id")?,
        surface: reader.id("web surface id")?,
        epoch: reader.u64()?,
        viewport_width: reader.u32()?,
        viewport_height: reader.u32()?,
        display_list: reader.bytes(MAX_WEB_DISPLAY_LIST_BYTES, "web display list")?,
    };
    validate_dimensions(patch.viewport_width, patch.viewport_height)
        .map_err(|()| DecodeError::InvalidValue("web viewport"))?;
    DisplayList::validate(&patch.display_list)
        .map_err(|_| DecodeError::InvalidValue("web display list"))?;
    Ok(patch)
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), ()> {
    if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
        Err(())
    } else {
        Ok(())
    }
}
