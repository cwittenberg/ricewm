use crate::{
    ArchiveFormat, DirectoryEntry, FileKind, FileMetadata, MAX_DIRECTORY_ENTRIES,
    MAX_MIME_TYPE_BYTES, MAX_PATH_BYTES, MAX_TEXT_BYTES, MAX_TRANSFER_BYTES, VfsError,
    VfsErrorCode, VfsOperation, VfsProgress, VfsRequest, VfsResponse, VfsResult,
};

use super::{DecodeError, EncodeError, Reader, Writer};

pub(super) fn encode_request(writer: &mut Writer, request: &VfsRequest) -> Result<(), EncodeError> {
    writer.id(request.request);
    writer.id(request.app);
    match &request.operation {
        VfsOperation::Stat { path } => {
            writer.u8(0);
            writer.path(path)?;
        }
        VfsOperation::Read {
            path,
            offset,
            length,
        } => {
            if usize::try_from(*length).unwrap_or(usize::MAX) > MAX_TRANSFER_BYTES {
                return Err(EncodeError::LimitExceeded {
                    field: "read length",
                    actual: usize::try_from(*length).unwrap_or(usize::MAX),
                    maximum: MAX_TRANSFER_BYTES,
                });
            }
            writer.u8(1);
            writer.path(path)?;
            writer.u64(*offset);
            writer.u32(*length);
        }
        VfsOperation::ReadDirectory {
            path,
            cursor,
            limit,
        } => {
            if *limit == 0 || usize::from(*limit) > MAX_DIRECTORY_ENTRIES {
                return Err(EncodeError::InvalidValue("directory limit"));
            }
            writer.u8(2);
            writer.path(path)?;
            writer.u64(*cursor);
            writer.u16(*limit);
        }
        VfsOperation::Write { path, offset, data } => {
            writer.u8(3);
            writer.path(path)?;
            writer.u64(*offset);
            writer.bytes(data, MAX_TRANSFER_BYTES, "write data")?;
        }
        VfsOperation::CreateDirectory { path } => {
            writer.u8(4);
            writer.path(path)?;
        }
        VfsOperation::Rename { from, to } => {
            writer.u8(5);
            writer.path(from)?;
            writer.path(to)?;
        }
        VfsOperation::CompareRename { from, to, expected } => {
            writer.u8(15);
            writer.path(from)?;
            writer.path(to)?;
            writer.bool(expected.is_some());
            if let Some(bytes)=expected {
                writer.bytes(bytes, MAX_TRANSFER_BYTES, "expected destination")?;
            }
        }
        VfsOperation::Remove { path, recursive } => {
            writer.u8(6);
            writer.path(path)?;
            writer.bool(*recursive);
        }
        VfsOperation::Copy {
            from,
            to,
            recursive,
        } => {
            writer.u8(7);
            writer.path(from)?;
            writer.path(to)?;
            writer.bool(*recursive);
        }
        VfsOperation::Link { from, to } => {
            writer.u8(8);
            writer.path(from)?;
            writer.path(to)?;
        }
        VfsOperation::ReadLink { path } => {
            writer.u8(13);
            writer.path(path)?;
        }
        VfsOperation::ReplaceLink { path, target } => {
            writer.u8(14);
            writer.path(path)?;
            writer.path(target)?;
        }
        VfsOperation::SetMode { path, mode } => {
            writer.u8(9);
            writer.path(path)?;
            writer.u32(*mode);
        }
        VfsOperation::Archive {
            sources,
            destination,
            format,
        } => {
            if sources.is_empty() || (*format == ArchiveFormat::Gzip && sources.len() != 1) {
                return Err(EncodeError::InvalidValue("archive sources"));
            }
            writer.u8(11);
            writer.count(sources.len(), MAX_DIRECTORY_ENTRIES, "archive sources")?;
            for source in sources {
                writer.path(source)?;
            }
            writer.path(destination)?;
            writer.u8(*format as u8);
        }
        VfsOperation::Extract {
            archive,
            destination,
        } => {
            writer.u8(12);
            writer.path(archive)?;
            writer.path(destination)?;
        }
        VfsOperation::Cancel { target } => {
            if *target == request.request {
                return Err(EncodeError::InvalidValue("cancel target"));
            }
            writer.u8(10);
            writer.id(*target);
        }
    }
    Ok(())
}

pub(super) fn decode_request(reader: &mut Reader<'_>) -> Result<VfsRequest, DecodeError> {
    let request = reader.id("request id")?;
    let app = reader.id("app id")?;
    let operation = match reader.u8()? {
        0 => VfsOperation::Stat {
            path: reader.path()?,
        },
        1 => {
            let path = reader.path()?;
            let offset = reader.u64()?;
            let length = reader.u32()?;
            if usize::try_from(length).unwrap_or(usize::MAX) > MAX_TRANSFER_BYTES {
                return Err(DecodeError::LimitExceeded {
                    field: "read length",
                    actual: usize::try_from(length).unwrap_or(usize::MAX),
                    maximum: MAX_TRANSFER_BYTES,
                });
            }
            VfsOperation::Read {
                path,
                offset,
                length,
            }
        }
        2 => {
            let path = reader.path()?;
            let cursor = reader.u64()?;
            let limit = reader.u16()?;
            if limit == 0 || usize::from(limit) > MAX_DIRECTORY_ENTRIES {
                return Err(DecodeError::InvalidValue("directory limit"));
            }
            VfsOperation::ReadDirectory {
                path,
                cursor,
                limit,
            }
        }
        3 => VfsOperation::Write {
            path: reader.path()?,
            offset: reader.u64()?,
            data: reader.bytes(MAX_TRANSFER_BYTES, "write data")?,
        },
        4 => VfsOperation::CreateDirectory {
            path: reader.path()?,
        },
        5 => VfsOperation::Rename {
            from: reader.path()?,
            to: reader.path()?,
        },
        6 => VfsOperation::Remove {
            path: reader.path()?,
            recursive: reader.bool("recursive")?,
        },
        7 => VfsOperation::Copy {
            from: reader.path()?,
            to: reader.path()?,
            recursive: reader.bool("recursive")?,
        },
        8 => VfsOperation::Link {
            from: reader.path()?,
            to: reader.path()?,
        },
        9 => VfsOperation::SetMode {
            path: reader.path()?,
            mode: reader.u32()?,
        },
        10 => {
            let target = reader.id("cancel target")?;
            if target == request {
                return Err(DecodeError::InvalidValue("cancel target"));
            }
            VfsOperation::Cancel { target }
        }
        11 => {
            let count = reader.count(MAX_DIRECTORY_ENTRIES, "archive sources")?;
            if count == 0 {
                return Err(DecodeError::InvalidValue("archive sources"));
            }
            let mut sources = Vec::with_capacity(count);
            for _ in 0..count {
                sources.push(reader.path()?);
            }
            let destination = reader.path()?;
            let format = match reader.u8()? {
                0 => ArchiveFormat::Zip,
                1 => ArchiveFormat::TarGzip,
                2 => ArchiveFormat::Gzip,
                value => {
                    return Err(DecodeError::UnknownTag {
                        field: "archive format",
                        value,
                    });
                }
            };
            if format == ArchiveFormat::Gzip && sources.len() != 1 {
                return Err(DecodeError::InvalidValue("archive sources"));
            }
            VfsOperation::Archive {
                sources,
                destination,
                format,
            }
        }
        12 => VfsOperation::Extract {
            archive: reader.path()?,
            destination: reader.path()?,
        },
        13 => VfsOperation::ReadLink {
            path: reader.path()?,
        },
        15 => VfsOperation::CompareRename {
            from: reader.path()?,
            to: reader.path()?,
            expected: if reader.bool("expected destination present")? {Some(reader.bytes(MAX_TRANSFER_BYTES, "expected destination")?.to_vec())}else{None},
        },
        14 => VfsOperation::ReplaceLink {
            path: reader.path()?,
            target: reader.path()?,
        },
        value => {
            return Err(DecodeError::UnknownTag {
                field: "VFS operation",
                value,
            });
        }
    };
    Ok(VfsRequest {
        request,
        app,
        operation,
    })
}

pub(super) fn encode_response(
    writer: &mut Writer,
    response: &VfsResponse,
) -> Result<(), EncodeError> {
    writer.id(response.request);
    match &response.result {
        VfsResult::Metadata(metadata) => {
            writer.u8(0);
            encode_metadata(writer, metadata);
        }
        VfsResult::Data(data) => {
            writer.u8(1);
            writer.bytes(data, MAX_TRANSFER_BYTES, "read data")?;
        }
        VfsResult::Directory {
            entries,
            next_cursor,
        } => {
            writer.u8(2);
            writer.count(entries.len(), MAX_DIRECTORY_ENTRIES, "directory entries")?;
            for entry in entries {
                writer.string(&entry.name, MAX_PATH_BYTES, "entry name")?;
                encode_metadata(writer, &entry.metadata);
                encode_mime_type(writer, entry.mime_type.as_deref())?;
            }
            writer.option_u64(*next_cursor);
        }
        VfsResult::Written(count) => {
            writer.u8(3);
            writer.u32(*count);
        }
        VfsResult::Done => writer.u8(4),
        VfsResult::Error(error) => {
            writer.u8(5);
            writer.u8(error.code as u8);
            writer.string(&error.message, MAX_TEXT_BYTES, "VFS error message")?;
        }
        VfsResult::Progress(progress) => {
            validate_progress(progress).map_err(EncodeError::InvalidValue)?;
            writer.u8(6);
            writer.u32(progress.sequence);
            writer.u64(progress.completed_bytes);
            writer.u64(progress.total_bytes);
        }
        VfsResult::Cancelled => writer.u8(7),
    }
    Ok(())
}

pub(super) fn decode_response(reader: &mut Reader<'_>) -> Result<VfsResponse, DecodeError> {
    let request = reader.id("request id")?;
    let result = match reader.u8()? {
        0 => VfsResult::Metadata(decode_metadata(reader)?),
        1 => VfsResult::Data(reader.bytes(MAX_TRANSFER_BYTES, "read data")?),
        2 => {
            let count = reader.count(MAX_DIRECTORY_ENTRIES, "directory entries")?;
            let mut entries = Vec::with_capacity(count);
            for _ in 0..count {
                entries.push(DirectoryEntry {
                    name: reader.string(MAX_PATH_BYTES, "entry name")?,
                    metadata: decode_metadata(reader)?,
                    mime_type: decode_mime_type(reader)?,
                });
            }
            VfsResult::Directory {
                entries,
                next_cursor: reader.option_u64()?,
            }
        }
        3 => VfsResult::Written(reader.u32()?),
        4 => VfsResult::Done,
        5 => VfsResult::Error(VfsError {
            code: error_code(reader)?,
            message: reader.string(MAX_TEXT_BYTES, "VFS error message")?,
        }),
        6 => {
            let progress = VfsProgress {
                sequence: reader.u32()?,
                completed_bytes: reader.u64()?,
                total_bytes: reader.u64()?,
            };
            validate_progress(&progress).map_err(DecodeError::InvalidValue)?;
            VfsResult::Progress(progress)
        }
        7 => VfsResult::Cancelled,
        value => {
            return Err(DecodeError::UnknownTag {
                field: "VFS result",
                value,
            });
        }
    };
    Ok(VfsResponse { request, result })
}

fn validate_progress(progress: &VfsProgress) -> Result<(), &'static str> {
    if progress.sequence == 0 {
        return Err("VFS progress sequence");
    }
    if progress.completed_bytes > progress.total_bytes {
        return Err("VFS progress counters");
    }
    Ok(())
}

fn encode_mime_type(writer: &mut Writer, mime_type: Option<&str>) -> Result<(), EncodeError> {
    writer.bool(mime_type.is_some());
    if let Some(value) = mime_type {
        if !is_canonical_mime(value) {
            return Err(EncodeError::InvalidValue("MIME type"));
        }
        writer.string(value, MAX_MIME_TYPE_BYTES, "MIME type")?;
    }
    Ok(())
}

fn decode_mime_type(reader: &mut Reader<'_>) -> Result<Option<String>, DecodeError> {
    if !reader.bool("MIME type presence")? {
        return Ok(None);
    }
    let value = reader.string(MAX_MIME_TYPE_BYTES, "MIME type")?;
    if !is_canonical_mime(&value) {
        return Err(DecodeError::InvalidValue("MIME type"));
    }
    Ok(Some(value))
}

fn is_canonical_mime(value: &str) -> bool {
    let Some((top, subtype)) = value.split_once('/') else {
        return false;
    };
    !top.is_empty()
        && !subtype.is_empty()
        && !subtype.contains('/')
        && value.len() <= MAX_MIME_TYPE_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                )
                || byte == b'/'
        })
}

fn encode_metadata(writer: &mut Writer, metadata: &FileMetadata) {
    writer.u8(metadata.kind as u8);
    writer.u64(metadata.size);
    writer.i64(metadata.modified_millis);
    writer.u32(metadata.mode);
}

fn decode_metadata(reader: &mut Reader<'_>) -> Result<FileMetadata, DecodeError> {
    Ok(FileMetadata {
        kind: file_kind(reader)?,
        size: reader.u64()?,
        modified_millis: reader.i64()?,
        mode: reader.u32()?,
    })
}

fn file_kind(reader: &mut Reader<'_>) -> Result<FileKind, DecodeError> {
    match reader.u8()? {
        0 => Ok(FileKind::File),
        1 => Ok(FileKind::Directory),
        2 => Ok(FileKind::Symlink),
        3 => Ok(FileKind::Other),
        value => Err(DecodeError::UnknownTag {
            field: "file kind",
            value,
        }),
    }
}

fn error_code(reader: &mut Reader<'_>) -> Result<VfsErrorCode, DecodeError> {
    match reader.u8()? {
        0 => Ok(VfsErrorCode::NotFound),
        1 => Ok(VfsErrorCode::PermissionDenied),
        2 => Ok(VfsErrorCode::AlreadyExists),
        3 => Ok(VfsErrorCode::InvalidInput),
        4 => Ok(VfsErrorCode::NotSupported),
        5 => Ok(VfsErrorCode::Busy),
        6 => Ok(VfsErrorCode::Io),
        value => Err(DecodeError::UnknownTag {
            field: "VFS error code",
            value,
        }),
    }
}
