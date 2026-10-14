use crate::{
    MAX_SETTINGS, MAX_TEXT_BYTES, SettingEntry, SettingsOperation, SettingsRequest,
    SettingsResponse, SettingsResult,
};

use super::{DecodeError, EncodeError, Reader, Writer};

pub(super) fn encode_request(
    writer: &mut Writer,
    request: &SettingsRequest,
) -> Result<(), EncodeError> {
    writer.id(request.request);
    writer.id(request.app);
    match &request.operation {
        SettingsOperation::InspectFileSync { path } => { writer.u8(8); writer.string(path, MAX_TEXT_BYTES, "file sync path")?; }
        SettingsOperation::ApplyWallpaperTheme { document } => {
            writer.u8(7); writer.string(document, MAX_TEXT_BYTES, "wallpaper recipe")?;
        }
        SettingsOperation::CreateWallpaperTheme { name, path } => {
            writer.u8(6);
            writer.string(name, MAX_TEXT_BYTES, "theme name")?;
            writer.string(path, MAX_TEXT_BYTES, "wallpaper path")?;
        }
        SettingsOperation::Load => writer.u8(0),
        SettingsOperation::Set { key, value } => {
            writer.u8(1);
            writer.string(key, MAX_TEXT_BYTES, "setting key")?;
            writer.string(value, MAX_TEXT_BYTES, "setting value")?;
        }
        SettingsOperation::PrepareRecycle => writer.u8(2),
        SettingsOperation::Recycle { confirmation } => {
            writer.u8(3);
            writer.string(confirmation, MAX_TEXT_BYTES, "recycle confirmation")?;
        }
        SettingsOperation::ExportPreferences => writer.u8(4),
        SettingsOperation::ImportPreferences { document } => {
            writer.u8(5);
            writer.string(document, MAX_TEXT_BYTES, "preferences document")?;
        }
    }
    Ok(())
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<SettingsRequest, DecodeError> {
    let request = reader.id("settings request id")?;
    let app = reader.id("settings app id")?;
    let operation = match reader.u8()? {
        8 => SettingsOperation::InspectFileSync { path: reader.string(MAX_TEXT_BYTES, "file sync path")? },
        7 => SettingsOperation::ApplyWallpaperTheme { document: reader.string(MAX_TEXT_BYTES, "wallpaper recipe")? },
        6 => SettingsOperation::CreateWallpaperTheme {
            name: reader.string(MAX_TEXT_BYTES, "theme name")?,
            path: reader.string(MAX_TEXT_BYTES, "wallpaper path")?,
        },
        0 => SettingsOperation::Load,
        1 => SettingsOperation::Set {
            key: reader.string(MAX_TEXT_BYTES, "setting key")?,
            value: reader.string(MAX_TEXT_BYTES, "setting value")?,
        },
        2 => SettingsOperation::PrepareRecycle,
        3 => SettingsOperation::Recycle {
            confirmation: reader.string(MAX_TEXT_BYTES, "recycle confirmation")?,
        },
        4 => SettingsOperation::ExportPreferences,
        5 => SettingsOperation::ImportPreferences {
            document: reader.string(MAX_TEXT_BYTES, "preferences document")?,
        },
        value => {
            return Err(DecodeError::UnknownTag {
                field: "settings operation",
                value,
            });
        }
    };
    Ok(SettingsRequest {
        request,
        app,
        operation,
    })
}

pub(super) fn encode_response(
    writer: &mut Writer,
    response: &SettingsResponse,
) -> Result<(), EncodeError> {
    writer.id(response.request);
    match &response.result {
        SettingsResult::WallpaperThemeRecipe(document) => {
            writer.u8(6); writer.string(document, MAX_TEXT_BYTES, "wallpaper recipe")?;
        }
        SettingsResult::Values(entries) => {
            writer.u8(0);
            writer.count(entries.len(), MAX_SETTINGS, "settings")?;
            for entry in entries {
                encode_entry(writer, entry)?;
            }
        }
        SettingsResult::Applied(entry) => {
            writer.u8(1);
            encode_entry(writer, entry)?;
        }
        SettingsResult::Error(message) => {
            writer.u8(2);
            writer.string(message, MAX_TEXT_BYTES, "settings error")?;
        }
        SettingsResult::RecyclePrepared(confirmation) => {
            writer.u8(3);
            writer.string(confirmation, MAX_TEXT_BYTES, "recycle confirmation")?;
        }
        SettingsResult::Restarting => writer.u8(4),
        SettingsResult::PreferencesDocument(document) => {
            writer.u8(5);
            writer.string(document, MAX_TEXT_BYTES, "preferences document")?;
        }
    }
    Ok(())
}

pub(super) fn decode_response(reader: &mut Reader<'_>) -> Result<SettingsResponse, DecodeError> {
    let request = reader.id("settings request id")?;
    let result = match reader.u8()? {
        6 => SettingsResult::WallpaperThemeRecipe(reader.string(MAX_TEXT_BYTES, "wallpaper recipe")?),
        0 => {
            let count = reader.count(MAX_SETTINGS, "settings")?;
            let mut entries = Vec::with_capacity(count);
            for _ in 0..count {
                entries.push(decode_entry(reader)?);
            }
            SettingsResult::Values(entries)
        }
        1 => SettingsResult::Applied(decode_entry(reader)?),
        2 => SettingsResult::Error(reader.string(MAX_TEXT_BYTES, "settings error")?),
        3 => {
            SettingsResult::RecyclePrepared(reader.string(MAX_TEXT_BYTES, "recycle confirmation")?)
        }
        4 => SettingsResult::Restarting,
        5 => SettingsResult::PreferencesDocument(
            reader.string(MAX_TEXT_BYTES, "preferences document")?,
        ),
        value => {
            return Err(DecodeError::UnknownTag {
                field: "settings result",
                value,
            });
        }
    };
    Ok(SettingsResponse { request, result })
}

fn encode_entry(writer: &mut Writer, entry: &SettingEntry) -> Result<(), EncodeError> {
    writer.string(&entry.key, MAX_TEXT_BYTES, "setting key")?;
    writer.string(&entry.value, MAX_TEXT_BYTES, "setting value")
}

fn decode_entry(reader: &mut Reader<'_>) -> Result<SettingEntry, DecodeError> {
    Ok(SettingEntry {
        key: reader.string(MAX_TEXT_BYTES, "setting key")?,
        value: reader.string(MAX_TEXT_BYTES, "setting value")?,
    })
}
