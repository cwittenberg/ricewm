use crate::{
    ClipboardFormat, ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection,
    DragAction, DragEnvelope, DragPayload, DragPhase, DragSourceEvent, DragSourceResult,
    DragSourceState, MAX_CLIPBOARD_BYTES, MAX_MIME_TYPE_BYTES, MAX_MIME_TYPES, MAX_TEXT_BYTES,
    MAX_TRANSFER_BYTES,
};

use super::{DecodeError, EncodeError, Reader, Writer};

pub(super) fn encode_clipboard_request(
    writer: &mut Writer,
    request: &ClipboardRequest,
) -> Result<(), EncodeError> {
    match request {
        ClipboardRequest::Read {
            request,
            selection,
            accepted_mime_types,
        } => {
            writer.u8(0);
            writer.id(*request);
            writer.u8(*selection as u8);
            writer.string_list(accepted_mime_types, MAX_MIME_TYPES, "MIME types")?;
        }
        ClipboardRequest::Write {
            request,
            selection,
            mime_type,
            data,
        } => {
            writer.u8(1);
            writer.id(*request);
            writer.u8(*selection as u8);
            writer.string(mime_type, MAX_TEXT_BYTES, "MIME type")?;
            writer.bytes(data, MAX_CLIPBOARD_BYTES, "clipboard data")?;
        }
        ClipboardRequest::WriteFormats {
            request,
            selection,
            formats,
        } => {
            validate_formats(formats).map_err(EncodeError::InvalidValue)?;
            writer.u8(2);
            writer.id(*request);
            writer.u8(*selection as u8);
            writer.count(formats.len(), MAX_MIME_TYPES, "clipboard formats")?;
            for format in formats {
                writer.string(
                    &format.mime_type,
                    MAX_MIME_TYPE_BYTES,
                    "clipboard MIME type",
                )?;
                writer.bytes(&format.data, MAX_CLIPBOARD_BYTES, "clipboard data")?;
            }
        }
    }
    Ok(())
}

pub(super) fn decode_clipboard_request(
    reader: &mut Reader<'_>,
) -> Result<ClipboardRequest, DecodeError> {
    match reader.u8()? {
        0 => Ok(ClipboardRequest::Read {
            request: reader.id("request id")?,
            selection: selection(reader)?,
            accepted_mime_types: reader.string_list(MAX_MIME_TYPES, "MIME types")?,
        }),
        1 => Ok(ClipboardRequest::Write {
            request: reader.id("request id")?,
            selection: selection(reader)?,
            mime_type: reader.string(MAX_TEXT_BYTES, "MIME type")?,
            data: reader.bytes(MAX_CLIPBOARD_BYTES, "clipboard data")?,
        }),
        2 => {
            let request = reader.id("request id")?;
            let selection = selection(reader)?;
            let count = reader.count(MAX_MIME_TYPES, "clipboard formats")?;
            if count == 0 {
                return Err(DecodeError::InvalidValue("clipboard formats"));
            }
            let mut formats = Vec::with_capacity(count);
            let mut total = 0usize;
            for _ in 0..count {
                let mime_type = reader.string(MAX_MIME_TYPE_BYTES, "clipboard MIME type")?;
                let data = reader.bytes(MAX_CLIPBOARD_BYTES - total, "clipboard aggregate data")?;
                total += data.len();
                formats.push(ClipboardFormat { mime_type, data });
            }
            validate_formats(&formats).map_err(DecodeError::InvalidValue)?;
            Ok(ClipboardRequest::WriteFormats {
                request,
                selection,
                formats,
            })
        }
        value => Err(DecodeError::UnknownTag {
            field: "clipboard request",
            value,
        }),
    }
}

// New multi-format messages use bounded MIME tokens and reject duplicate media
// types, including differently-cased names or alternate charset parameters.
fn validate_formats(formats: &[ClipboardFormat]) -> Result<(), &'static str> {
    if formats.is_empty() || formats.len() > MAX_MIME_TYPES {
        return Err("clipboard formats");
    }
    let mut total = 0usize;
    let mut types = std::collections::BTreeSet::new();
    for format in formats {
        let mut pieces = format.mime_type.split(';');
        let essence = pieces.next().unwrap_or_default();
        let Some((kind, subtype)) = essence.split_once('/') else {
            return Err("clipboard MIME type");
        };
        let token = |value: &str| {
            !value.is_empty()
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&b))
        };
        if format.mime_type.len() > MAX_MIME_TYPE_BYTES
            || !token(kind)
            || !token(subtype)
            || !pieces.all(|piece| {
                piece
                    .split_once('=')
                    .is_some_and(|(key, value)| token(key) && token(value))
            })
        {
            return Err("clipboard MIME type");
        }
        if !types.insert(essence.to_ascii_lowercase()) {
            return Err("duplicate clipboard MIME type");
        }
        total = total
            .checked_add(format.data.len())
            .ok_or("clipboard aggregate data")?;
        if total > MAX_CLIPBOARD_BYTES {
            return Err("clipboard aggregate data");
        }
    }
    Ok(())
}

pub(super) fn encode_clipboard_response(
    writer: &mut Writer,
    response: &ClipboardResponse,
) -> Result<(), EncodeError> {
    writer.id(response.request);
    match &response.result {
        ClipboardResult::Data { mime_type, data } => {
            writer.u8(0);
            writer.string(mime_type, MAX_TEXT_BYTES, "MIME type")?;
            writer.bytes(data, MAX_CLIPBOARD_BYTES, "clipboard data")?;
        }
        ClipboardResult::Empty => writer.u8(1),
        ClipboardResult::Rejected => writer.u8(2),
    }
    Ok(())
}

pub(super) fn decode_clipboard_response(
    reader: &mut Reader<'_>,
) -> Result<ClipboardResponse, DecodeError> {
    let request = reader.id("request id")?;
    let result = match reader.u8()? {
        0 => ClipboardResult::Data {
            mime_type: reader.string(MAX_TEXT_BYTES, "MIME type")?,
            data: reader.bytes(MAX_CLIPBOARD_BYTES, "clipboard data")?,
        },
        1 => ClipboardResult::Empty,
        2 => ClipboardResult::Rejected,
        value => {
            return Err(DecodeError::UnknownTag {
                field: "clipboard result",
                value,
            });
        }
    };
    Ok(ClipboardResponse { request, result })
}

pub(super) fn encode_drag(writer: &mut Writer, drag: &DragEnvelope) -> Result<(), EncodeError> {
    writer.id(drag.drag);
    writer.id(drag.app);
    writer.id(drag.window);
    writer.option_id(drag.target);
    writer.u8(drag.phase as u8);
    writer.u8(drag.action as u8);
    writer.i32(drag.x);
    writer.i32(drag.y);
    writer.string_list(&drag.mime_types, MAX_MIME_TYPES, "MIME types")?;
    writer.bool(drag.payload.is_some());
    if let Some(payload) = &drag.payload {
        writer.string(&payload.mime_type, MAX_TEXT_BYTES, "drag payload MIME type")?;
        writer.bytes(&payload.data, MAX_TRANSFER_BYTES, "drag payload")?;
    }
    Ok(())
}

pub(super) fn decode_drag(reader: &mut Reader<'_>) -> Result<DragEnvelope, DecodeError> {
    Ok(DragEnvelope {
        drag: reader.id("drag id")?,
        app: reader.id("app id")?,
        window: reader.id("window id")?,
        target: reader.option_id("target widget id")?,
        phase: drag_phase(reader)?,
        action: drag_action(reader)?,
        x: reader.i32()?,
        y: reader.i32()?,
        mime_types: reader.string_list(MAX_MIME_TYPES, "MIME types")?,
        payload: if reader.bool("drag payload")? {
            Some(DragPayload {
                mime_type: reader.string(MAX_TEXT_BYTES, "drag payload MIME type")?,
                data: reader.bytes(MAX_TRANSFER_BYTES, "drag payload")?,
            })
        } else {
            None
        },
    })
}

fn selection(reader: &mut Reader<'_>) -> Result<ClipboardSelection, DecodeError> {
    match reader.u8()? {
        0 => Ok(ClipboardSelection::Clipboard),
        1 => Ok(ClipboardSelection::Primary),
        value => Err(DecodeError::UnknownTag {
            field: "clipboard selection",
            value,
        }),
    }
}

fn drag_phase(reader: &mut Reader<'_>) -> Result<DragPhase, DecodeError> {
    match reader.u8()? {
        0 => Ok(DragPhase::Enter),
        1 => Ok(DragPhase::Motion),
        2 => Ok(DragPhase::Leave),
        3 => Ok(DragPhase::Drop),
        4 => Ok(DragPhase::Finished),
        value => Err(DecodeError::UnknownTag {
            field: "drag phase",
            value,
        }),
    }
}

fn drag_action(reader: &mut Reader<'_>) -> Result<DragAction, DecodeError> {
    match reader.u8()? {
        0 => Ok(DragAction::Copy),
        1 => Ok(DragAction::Move),
        2 => Ok(DragAction::Link),
        3 => Ok(DragAction::Ask),
        value => Err(DecodeError::UnknownTag {
            field: "drag action",
            value,
        }),
    }
}

pub(super) fn encode_drag_source(
    writer: &mut Writer,
    event: &DragSourceEvent,
) -> Result<(), EncodeError> {
    writer.id(event.drag);
    writer.id(event.app);
    writer.id(event.window);
    match &event.state {
        DragSourceState::Offer {
            action,
            mime_types,
            payload,
        } => {
            writer.u8(0);
            writer.u8(*action as u8);
            writer.string_list(mime_types, MAX_MIME_TYPES, "drag source MIME types")?;
            writer.string(
                &payload.mime_type,
                MAX_TEXT_BYTES,
                "drag source payload MIME type",
            )?;
            writer.bytes(&payload.data, MAX_TRANSFER_BYTES, "drag source payload")?;
        }
        DragSourceState::Cancel => writer.u8(1),
        DragSourceState::Finished(result) => {
            writer.u8(2);
            match result {
                DragSourceResult::Completed(action) => {
                    writer.u8(0);
                    writer.u8(*action as u8);
                }
                DragSourceResult::Rejected => writer.u8(1),
                DragSourceResult::Cancelled => writer.u8(2),
                DragSourceResult::Failed => writer.u8(3),
            }
        }
    }
    Ok(())
}

pub(super) fn decode_drag_source(reader: &mut Reader<'_>) -> Result<DragSourceEvent, DecodeError> {
    let drag = reader.id("drag source id")?;
    let app = reader.id("drag source app id")?;
    let window = reader.id("drag source window id")?;
    let state = match reader.u8()? {
        0 => DragSourceState::Offer {
            action: drag_action(reader)?,
            mime_types: reader.string_list(MAX_MIME_TYPES, "drag source MIME types")?,
            payload: DragPayload {
                mime_type: reader.string(MAX_TEXT_BYTES, "drag source payload MIME type")?,
                data: reader.bytes(MAX_TRANSFER_BYTES, "drag source payload")?,
            },
        },
        1 => DragSourceState::Cancel,
        2 => DragSourceState::Finished(drag_source_result(reader)?),
        value => {
            return Err(DecodeError::UnknownTag {
                field: "drag source state",
                value,
            });
        }
    };
    Ok(DragSourceEvent {
        drag,
        app,
        window,
        state,
    })
}

fn drag_source_result(reader: &mut Reader<'_>) -> Result<DragSourceResult, DecodeError> {
    match reader.u8()? {
        0 => Ok(DragSourceResult::Completed(drag_action(reader)?)),
        1 => Ok(DragSourceResult::Rejected),
        2 => Ok(DragSourceResult::Cancelled),
        3 => Ok(DragSourceResult::Failed),
        value => Err(DecodeError::UnknownTag {
            field: "drag source result",
            value,
        }),
    }
}
