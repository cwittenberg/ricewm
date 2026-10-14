mod errors;
mod host;
mod lifecycle;
mod media;
mod primitives;
mod provider;
mod settings;
mod transfer;
mod ui;
mod vfs;
#[cfg(test)]
mod vfs_fixture_tests;
mod web_render;

pub use errors::{DecodeError, EncodeError};
use primitives::{Reader, Writer};

use crate::{
    HostCapabilities, MAX_ARGUMENTS, MAX_MESSAGE_BYTES, MAX_PATH_BYTES, MAX_TEXT_BYTES, Message,
    PROTOCOL_VERSION,
};

const MAGIC: &[u8; 4] = b"RAPP";
const HEADER_BYTES: usize = 12;

#[repr(u16)]
enum Kind {
    HostHello = 1,
    Launch = 2,
    Open = 3,
    Close = 4,
    CloseRequest = 5,
    CloseResponse = 6,
    VfsRequest = 10,
    VfsResponse = 11,
    MediaRequest = 12,
    MediaResponse = 13,
    UiPatch = 20,
    UiEvent = 21,
    WebRenderPatch = 22,
    ClipboardRequest = 30,
    ClipboardResponse = 31,
    Drag = 40,
    DragSource = 41,
    SettingsRequest = 50,
    SettingsResponse = 51,
    DataProviderRequest = 60,
    DataProviderResponse = 61,
    HostRequest = 70,
    HostEvent = 71,
}

pub(crate) fn encode_message(message: &Message) -> Result<Vec<u8>, EncodeError> {
    let (kind, payload) = encode_payload(message)?;
    if payload.len() + HEADER_BYTES > MAX_MESSAGE_BYTES {
        return Err(EncodeError::LimitExceeded {
            field: "message",
            actual: payload.len() + HEADER_BYTES,
            maximum: MAX_MESSAGE_BYTES,
        });
    }
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| EncodeError::InvalidValue("payload length"))?;
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload.len());
    frame.extend_from_slice(MAGIC);
    frame.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    frame.extend_from_slice(&(kind as u16).to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub(crate) fn decode_message(bytes: &[u8]) -> Result<Message, DecodeError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(DecodeError::LimitExceeded {
            field: "message",
            actual: bytes.len(),
            maximum: MAX_MESSAGE_BYTES,
        });
    }
    let mut header = Reader::new(bytes);
    if header.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = header.u16()?;
    if version != PROTOCOL_VERSION {
        return Err(DecodeError::UnsupportedVersion(version));
    }
    let kind = header.u16()?;
    let payload_len =
        usize::try_from(header.u32()?).map_err(|_| DecodeError::InvalidValue("payload length"))?;
    if payload_len != header.remaining() {
        return Err(DecodeError::LengthMismatch);
    }
    decode_payload(kind, header.take(payload_len)?)
}

// This central match is the protocol's auditable mapping from public message
// variants to stable wire kinds; splitting it would duplicate that invariant.
#[allow(clippy::too_many_lines)]
fn encode_payload(message: &Message) -> Result<(Kind, Vec<u8>), EncodeError> {
    let mut writer = Writer::default();
    let kind = match message {
        Message::HostHello {
            capabilities,
            max_message_bytes,
        } => {
            if *max_message_bytes == 0
                || usize::try_from(*max_message_bytes).unwrap_or(usize::MAX) > MAX_MESSAGE_BYTES
            {
                return Err(EncodeError::InvalidValue("max_message_bytes"));
            }
            writer.u64(capabilities.bits());
            writer.u32(*max_message_bytes);
            Kind::HostHello
        }
        Message::Launch {
            app,
            entrypoint,
            arguments,
        } => {
            writer.id(*app);
            writer.string(entrypoint, MAX_TEXT_BYTES, "entrypoint")?;
            writer.count(arguments.len(), MAX_ARGUMENTS, "arguments")?;
            for argument in arguments {
                writer.string(argument, MAX_TEXT_BYTES, "argument")?;
            }
            Kind::Launch
        }
        Message::Open { app, paths } => {
            writer.id(*app);
            writer.count(paths.len(), MAX_ARGUMENTS, "paths")?;
            for path in paths {
                writer.string(path, MAX_PATH_BYTES, "path")?;
            }
            Kind::Open
        }
        Message::Close { app, reason } => {
            writer.id(*app);
            writer.u8(*reason as u8);
            Kind::Close
        }
        Message::CloseRequest(request) => {
            lifecycle::encode_request(&mut writer, request);
            Kind::CloseRequest
        }
        Message::CloseResponse(response) => {
            lifecycle::encode_response(&mut writer, response);
            Kind::CloseResponse
        }
        Message::VfsRequest(request) => {
            vfs::encode_request(&mut writer, request)?;
            Kind::VfsRequest
        }
        Message::VfsResponse(response) => {
            vfs::encode_response(&mut writer, response)?;
            Kind::VfsResponse
        }
        Message::MediaRequest(request) => {
            media::encode_request(&mut writer, request)?;
            Kind::MediaRequest
        }
        Message::MediaResponse(response) => {
            media::encode_response(&mut writer, response)?;
            Kind::MediaResponse
        }
        Message::UiPatch(patch) => {
            ui::encode_patch(&mut writer, patch)?;
            Kind::UiPatch
        }
        Message::UiEvent(event) => {
            ui::encode_event(&mut writer, event)?;
            Kind::UiEvent
        }
        Message::WebRenderPatch(patch) => {
            web_render::encode(&mut writer, patch)?;
            Kind::WebRenderPatch
        }
        Message::ClipboardRequest(request) => {
            transfer::encode_clipboard_request(&mut writer, request)?;
            Kind::ClipboardRequest
        }
        Message::ClipboardResponse(response) => {
            transfer::encode_clipboard_response(&mut writer, response)?;
            Kind::ClipboardResponse
        }
        Message::Drag(drag) => {
            transfer::encode_drag(&mut writer, drag)?;
            Kind::Drag
        }
        Message::DragSource(drag) => {
            transfer::encode_drag_source(&mut writer, drag)?;
            Kind::DragSource
        }
        Message::SettingsRequest(request) => {
            settings::encode_request(&mut writer, request)?;
            Kind::SettingsRequest
        }
        Message::SettingsResponse(response) => {
            settings::encode_response(&mut writer, response)?;
            Kind::SettingsResponse
        }
        Message::DataProviderRequest(request) => {
            provider::encode_request(&mut writer, request)?;
            Kind::DataProviderRequest
        }
        Message::DataProviderResponse(response) => {
            provider::encode_response(&mut writer, response)?;
            Kind::DataProviderResponse
        }
        Message::HostRequest(request) => {
            host::encode_request(&mut writer, request)?;
            Kind::HostRequest
        }
        Message::HostEvent(event) => {
            host::encode_event(&mut writer, event)?;
            Kind::HostEvent
        }
    };
    Ok((kind, writer.finish()))
}

fn decode_payload(kind: u16, payload: &[u8]) -> Result<Message, DecodeError> {
    let mut reader = Reader::new(payload);
    let message = match kind {
        value if value == Kind::HostHello as u16 => {
            let capabilities = HostCapabilities::from_bits_retain(reader.u64()?);
            let max_message_bytes = reader.u32()?;
            if max_message_bytes == 0
                || usize::try_from(max_message_bytes).unwrap_or(usize::MAX) > MAX_MESSAGE_BYTES
            {
                return Err(DecodeError::InvalidValue("max_message_bytes"));
            }
            Message::HostHello {
                capabilities,
                max_message_bytes,
            }
        }
        value if value == Kind::Launch as u16 => {
            let app = reader.id("app id")?;
            let entrypoint = reader.string(MAX_TEXT_BYTES, "entrypoint")?;
            let count = reader.count(MAX_ARGUMENTS, "arguments")?;
            let mut arguments = Vec::with_capacity(count);
            for _ in 0..count {
                arguments.push(reader.string(MAX_TEXT_BYTES, "argument")?);
            }
            Message::Launch {
                app,
                entrypoint,
                arguments,
            }
        }
        value if value == Kind::Open as u16 => {
            let app = reader.id("app id")?;
            let count = reader.count(MAX_ARGUMENTS, "paths")?;
            let mut paths = Vec::with_capacity(count);
            for _ in 0..count {
                paths.push(reader.string(MAX_PATH_BYTES, "path")?);
            }
            Message::Open { app, paths }
        }
        value if value == Kind::Close as u16 => Message::Close {
            app: reader.id("app id")?,
            reason: lifecycle::decode_reason(&mut reader)?,
        },
        value if value == Kind::CloseRequest as u16 => {
            Message::CloseRequest(lifecycle::decode_request(&mut reader)?)
        }
        value if value == Kind::CloseResponse as u16 => {
            Message::CloseResponse(lifecycle::decode_response(&mut reader)?)
        }
        value if value == Kind::VfsRequest as u16 => {
            Message::VfsRequest(vfs::decode_request(&mut reader)?)
        }
        value if value == Kind::VfsResponse as u16 => {
            Message::VfsResponse(vfs::decode_response(&mut reader)?)
        }
        value if value == Kind::MediaRequest as u16 => {
            Message::MediaRequest(media::decode_request(&mut reader)?)
        }
        value if value == Kind::MediaResponse as u16 => {
            Message::MediaResponse(media::decode_response(&mut reader)?)
        }
        value if value == Kind::UiPatch as u16 => Message::UiPatch(ui::decode_patch(&mut reader)?),
        value if value == Kind::UiEvent as u16 => Message::UiEvent(ui::decode_event(&mut reader)?),
        value if value == Kind::WebRenderPatch as u16 => {
            Message::WebRenderPatch(web_render::decode(&mut reader)?)
        }
        value if value == Kind::ClipboardRequest as u16 => {
            Message::ClipboardRequest(transfer::decode_clipboard_request(&mut reader)?)
        }
        value if value == Kind::ClipboardResponse as u16 => {
            Message::ClipboardResponse(transfer::decode_clipboard_response(&mut reader)?)
        }
        value if value == Kind::Drag as u16 => Message::Drag(transfer::decode_drag(&mut reader)?),
        value if value == Kind::DragSource as u16 => {
            Message::DragSource(transfer::decode_drag_source(&mut reader)?)
        }
        value if value == Kind::SettingsRequest as u16 => {
            Message::SettingsRequest(settings::decode_request(&mut reader)?)
        }
        value if value == Kind::SettingsResponse as u16 => {
            Message::SettingsResponse(settings::decode_response(&mut reader)?)
        }
        value if value == Kind::DataProviderRequest as u16 => {
            Message::DataProviderRequest(provider::decode_request(&mut reader)?)
        }
        value if value == Kind::DataProviderResponse as u16 => {
            Message::DataProviderResponse(provider::decode_response(&mut reader)?)
        }
        value if value == Kind::HostRequest as u16 => {
            Message::HostRequest(host::decode_request(&mut reader)?)
        }
        value if value == Kind::HostEvent as u16 => {
            Message::HostEvent(host::decode_event(&mut reader)?)
        }
        _ => return Err(DecodeError::UnknownMessageKind(kind)),
    };
    if reader.remaining() != 0 {
        return Err(DecodeError::TrailingBytes);
    }
    Ok(message)
}

#[cfg(test)]
mod tests;
