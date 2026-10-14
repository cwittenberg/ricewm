//! Desktop path and directory-entry validation.

use rice_native_app_protocol::DirectoryEntry;

use super::DesktopActorError;

pub(super) fn validate_entries(entries: &[DirectoryEntry]) -> Result<(), DesktopActorError> {
    if entries.iter().all(|entry| valid_file_name(&entry.name)) {
        Ok(())
    } else {
        Err(DesktopActorError::InvalidDirectoryEntry)
    }
}

pub(super) fn valid_file_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 255
        && !name.contains('/')
        && !name.as_bytes().contains(&0)
}

pub(super) fn error_message(message: &str) -> &str {
    let message = message.trim();
    if message.is_empty() {
        "VFS operation failed"
    } else {
        message
    }
}
