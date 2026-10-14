use crate::{
    MAX_MEDIA_DIMENSION, MAX_MEDIA_SOURCE_BYTES, MAX_TEXT_BYTES, MediaError, MediaErrorCode,
    MediaKind, MediaMetadata, MediaRequest, MediaResponse, MediaResult,
};

use super::{DecodeError, EncodeError, Reader, Writer};

const MAX_FRAME_MILLIS: u32 = 86_400_000;

pub(super) fn encode_request(
    writer: &mut Writer,
    request: &MediaRequest,
) -> Result<(), EncodeError> {
    validate_request(
        request.maximum_source_bytes,
        request.target_width,
        request.target_height,
        request.frame_millis,
    )
    .map_err(EncodeError::InvalidValue)?;
    writer.id(request.request);
    writer.id(request.app);
    writer.path(&request.path)?;
    writer.string(&request.mime_type, MAX_TEXT_BYTES, "media MIME type")?;
    writer.u64(request.maximum_source_bytes);
    writer.u16(request.target_width);
    writer.u16(request.target_height);
    writer.u32(request.frame_millis);
    Ok(())
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<MediaRequest, DecodeError> {
    let request = MediaRequest {
        request: reader.id("request id")?,
        app: reader.id("app id")?,
        path: reader.path()?,
        mime_type: reader.string(MAX_TEXT_BYTES, "media MIME type")?,
        maximum_source_bytes: reader.u64()?,
        target_width: reader.u16()?,
        target_height: reader.u16()?,
        frame_millis: reader.u32()?,
    };
    validate_request(
        request.maximum_source_bytes,
        request.target_width,
        request.target_height,
        request.frame_millis,
    )
    .map_err(DecodeError::InvalidValue)?;
    Ok(request)
}

pub(super) fn encode_response(
    writer: &mut Writer,
    response: &MediaResponse,
) -> Result<(), EncodeError> {
    writer.id(response.request);
    match &response.result {
        MediaResult::Ready(metadata) => {
            validate_dimensions(metadata.width, metadata.height)
                .map_err(EncodeError::InvalidValue)?;
            writer.u8(0);
            writer.u8(metadata.kind as u8);
            writer.u16(metadata.width);
            writer.u16(metadata.height);
            writer.option_u64(metadata.duration_millis);
            writer.string(&metadata.mime_type, MAX_TEXT_BYTES, "media MIME type")?;
        }
        MediaResult::Error(error) => {
            writer.u8(1);
            writer.u8(error.code as u8);
            writer.string(&error.message, MAX_TEXT_BYTES, "media error")?;
        }
    }
    Ok(())
}

pub(super) fn decode_response(reader: &mut Reader<'_>) -> Result<MediaResponse, DecodeError> {
    let request = reader.id("request id")?;
    let result = match reader.u8()? {
        0 => {
            let metadata = MediaMetadata {
                kind: media_kind(reader)?,
                width: reader.u16()?,
                height: reader.u16()?,
                duration_millis: reader.option_u64()?,
                mime_type: reader.string(MAX_TEXT_BYTES, "media MIME type")?,
            };
            validate_dimensions(metadata.width, metadata.height)
                .map_err(DecodeError::InvalidValue)?;
            MediaResult::Ready(metadata)
        }
        1 => MediaResult::Error(MediaError {
            code: error_code(reader)?,
            message: reader.string(MAX_TEXT_BYTES, "media error")?,
        }),
        value => {
            return Err(DecodeError::UnknownTag {
                field: "media result",
                value,
            });
        }
    };
    Ok(MediaResponse { request, result })
}

fn validate_request(
    maximum_source_bytes: u64,
    width: u16,
    height: u16,
    frame_millis: u32,
) -> Result<(), &'static str> {
    if maximum_source_bytes == 0 || maximum_source_bytes > MAX_MEDIA_SOURCE_BYTES {
        return Err("media source byte limit");
    }
    validate_dimensions(width, height)?;
    if frame_millis > MAX_FRAME_MILLIS {
        return Err("media frame timestamp");
    }
    Ok(())
}

fn validate_dimensions(width: u16, height: u16) -> Result<(), &'static str> {
    if width == 0 || height == 0 || width > MAX_MEDIA_DIMENSION || height > MAX_MEDIA_DIMENSION {
        Err("media dimensions")
    } else {
        Ok(())
    }
}

fn media_kind(reader: &mut Reader<'_>) -> Result<MediaKind, DecodeError> {
    match reader.u8()? {
        0 => Ok(MediaKind::Image),
        1 => Ok(MediaKind::Video),
        value => Err(DecodeError::UnknownTag {
            field: "media kind",
            value,
        }),
    }
}

fn error_code(reader: &mut Reader<'_>) -> Result<MediaErrorCode, DecodeError> {
    match reader.u8()? {
        0 => Ok(MediaErrorCode::NotFound),
        1 => Ok(MediaErrorCode::TooLarge),
        2 => Ok(MediaErrorCode::Unsupported),
        3 => Ok(MediaErrorCode::DecodeFailed),
        4 => Ok(MediaErrorCode::InvalidInput),
        value => Err(DecodeError::UnknownTag {
            field: "media error code",
            value,
        }),
    }
}
