//! Installed-application catalog values exchanged with native applications.

use crate::{MAX_MIME_TYPES, MAX_PATH_BYTES, MAX_TEXT_BYTES};

pub const APPLICATION_CATALOG_VERSION: u16 = 1;
pub const MAX_APPLICATIONS: usize = 128;
pub const MAX_APPLICATION_ID_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstalledApplication {
    /// Stable identity, never an executable or native entrypoint supplied by Wasm.
    pub id: String,
    pub name: String,
    pub icon: String,
    pub mime_types: Vec<String>,
    pub matches_requested_type: bool,
    pub is_default: bool,
    pub is_native: bool,
}

pub(crate) fn validate_application_id(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_APPLICATION_ID_BYTES
        || value.contains('\0')
        || value.contains('/')
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        Err("application id")
    } else {
        Ok(())
    }
}

pub(crate) fn validate_mime_type(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.contains('\0')
        || !value.split_once('/').is_some_and(|(kind, subtype)| {
            !kind.is_empty()
                && !subtype.is_empty()
                && kind.bytes().all(valid_mime_byte)
                && subtype.bytes().all(valid_mime_byte)
        })
    {
        Err("MIME type")
    } else {
        Ok(())
    }
}

fn valid_mime_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'!' | b'#'..=b'.' | b'^'..=b'~')
}

pub(crate) fn validate_guest_paths(paths: &[String]) -> Result<(), &'static str> {
    if paths.is_empty() || paths.len() > crate::MAX_ARGUMENTS {
        return Err("application paths");
    }
    if paths.iter().any(|path| {
        !path.starts_with('/')
            || path.len() > MAX_PATH_BYTES
            || path.contains('\0')
            || path.contains('\n')
            || path.contains('\r')
    }) {
        return Err("application path");
    }
    Ok(())
}

pub(crate) fn validate_catalog(entries: &[InstalledApplication]) -> Result<(), &'static str> {
    if entries.len() > MAX_APPLICATIONS {
        return Err("application count");
    }
    for (index, entry) in entries.iter().enumerate() {
        validate_application_id(&entry.id)?;
        if entry.name.trim().is_empty()
            || entry.name.len() > MAX_TEXT_BYTES
            || entry.name.contains('\0')
            || entry.icon.len() > MAX_TEXT_BYTES
            || entry.icon.contains('\0')
            || entry.mime_types.len() > MAX_MIME_TYPES
            || entry
                .mime_types
                .iter()
                .any(|mime| validate_mime_type(mime).is_err())
            || entries[..index].iter().any(|prior| prior.id == entry.id)
        {
            return Err("application entry");
        }
    }
    Ok(())
}
