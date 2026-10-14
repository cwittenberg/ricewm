//! GNOME-compatible file clipboard ownership and paste transfers.

use rice_files::{DropAction, EntryKind, GuestPath};
use rice_native_app_protocol::{
    ClipboardRequest, ClipboardResponse, ClipboardResult, ClipboardSelection, Message, RequestId,
};

use super::{FilesActorError, RiceFiles};

const GNOME_FILES: &str = "x-special/gnome-copied-files";
const URI_LIST: &str = "text/uri-list";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PendingClipboard {
    Write {
        request: RequestId,
        cut: bool,
    },
    Paste {
        request: RequestId,
        target: GuestPath,
    },
    Location {
        request: RequestId,
    },
}

impl PendingClipboard {
    const fn request(&self) -> RequestId {
        match self {
            Self::Write { request, .. }
            | Self::Paste { request, .. }
            | Self::Location { request } => *request,
        }
    }
}

pub(super) fn write_location(
    actor: &mut RiceFiles,
    request: RequestId,
    path: &str,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_clipboard.is_some() {
        return actor.emit_notice(rice_i18n::t("A clipboard operation is already in progress"));
    }
    actor.pending_clipboard = Some(PendingClipboard::Location { request });
    Ok(vec![Message::ClipboardRequest(ClipboardRequest::Write {
        request,
        selection: ClipboardSelection::Clipboard,
        mime_type: String::from("text/plain;charset=utf-8"),
        data: path.as_bytes().to_vec(),
    })])
}

pub(super) fn write(
    actor: &mut RiceFiles,
    request: RequestId,
    cut: bool,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_clipboard.is_some() {
        return actor.emit_notice(rice_i18n::t("A clipboard operation is already in progress"));
    }
    let selected = actor.selected_entries();
    if selected.is_empty() {
        return actor.emit_notice(rice_i18n::t("Select at least one item first"));
    }
    if cut
        && selected
            .iter()
            .any(|entry| super::files_preferences::is_builtin(entry.path.as_str()))
    {
        return actor.emit_notice(rice_i18n::t("Built-in Places cannot be moved"));
    }
    let mut text = String::from(if cut { "cut\n" } else { "copy\n" });
    for entry in selected {
        text.push_str("file://");
        text.push_str(&encode_uri_path(entry.path.as_str()));
        text.push('\n');
    }
    actor.pending_clipboard = Some(PendingClipboard::Write { request, cut });
    Ok(vec![Message::ClipboardRequest(ClipboardRequest::Write {
        request,
        selection: ClipboardSelection::Clipboard,
        mime_type: String::from(GNOME_FILES),
        data: text.into_bytes(),
    })])
}

pub(super) fn paste(
    actor: &mut RiceFiles,
    request: RequestId,
) -> Result<Vec<Message>, FilesActorError> {
    if actor.pending_clipboard.is_some()
        || actor.runtime()?.pending_directory.is_some()
        || actor.pending_mutation.is_some()
    {
        return actor.emit_notice(rice_i18n::t("Another filesystem operation is already in progress"));
    }
    let target = actor
        .selected_entries()
        .into_iter()
        .find(|entry| entry.kind == EntryKind::Directory)
        .map_or_else(|| actor.location().clone(), |entry| entry.path);
    actor.pending_clipboard = Some(PendingClipboard::Paste { request, target });
    Ok(vec![Message::ClipboardRequest(ClipboardRequest::Read {
        request,
        selection: ClipboardSelection::Clipboard,
        accepted_mime_types: vec![String::from(GNOME_FILES), String::from(URI_LIST)],
    })])
}

pub(super) fn response(
    actor: &mut RiceFiles,
    response: &ClipboardResponse,
) -> Result<Vec<Message>, FilesActorError> {
    let Some(pending) = actor.pending_clipboard.take() else {
        return Err(FilesActorError::UnexpectedClipboardResponse);
    };
    if pending.request() != response.request {
        actor.pending_clipboard = Some(pending);
        return Err(FilesActorError::UnexpectedClipboardResponse);
    }
    match (pending, &response.result) {
        (PendingClipboard::Write { cut, .. }, ClipboardResult::Data { .. }) => {
            actor.emit_notice(rice_i18n::t(if cut {
                "Selection cut to the clipboard"
            } else {
                "Selection copied to the clipboard"
            }))
        }
        (PendingClipboard::Location { .. }, ClipboardResult::Data { .. }) => {
            actor.emit_notice(rice_i18n::t("Location copied to the clipboard"))
        }
        (PendingClipboard::Paste { target, .. }, ClipboardResult::Data { mime_type, data })
            if matches!(mime_type.as_str(), GNOME_FILES | URI_LIST) =>
        {
            let (action, paths) = if mime_type == GNOME_FILES {
                super::files_external_drag::parse_gnome_files(data)?
            } else {
                (
                    DropAction::Copy,
                    super::files_external_drag::parse_uri_list(data)?,
                )
            };
            actor.start_external_transfer(paths, target, action, response.request, None)
        }
        (_, ClipboardResult::Empty) => actor.emit_notice(rice_i18n::t("The file clipboard is empty")),
        (_, ClipboardResult::Rejected) => {
            actor.emit_notice(rice_i18n::t("The clipboard host rejected the file operation"))
        }
        _ => actor.emit_notice(rice_i18n::t("The clipboard does not contain a supported file list")),
    }
}

fn encode_uri_path(path: &str) -> String {
    let mut output = String::new();
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~') {
            output.push(char::from(byte));
        } else {
            use core::fmt::Write;
            let _ = write!(output, "%{byte:02X}");
        }
    }
    output
}

pub(super) fn uri_list(entries: &[rice_files::EntryMetadata]) -> Option<Vec<u8>> {
    if entries.is_empty() || entries.len() > 128 {
        return None;
    }
    let mut text = String::new();
    for entry in entries {
        text.push_str("file://");
        text.push_str(&encode_uri_path(entry.path.as_str()));
        text.push_str("\r\n");
        if text.len() > rice_native_app_protocol::MAX_TRANSFER_BYTES {
            return None;
        }
    }
    Some(text.into_bytes())
}
