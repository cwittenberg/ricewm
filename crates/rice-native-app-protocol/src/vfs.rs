use crate::{AppId, RequestId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsRequest {
    pub request: RequestId,
    pub app: AppId,
    pub operation: VfsOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VfsOperation {
    Stat {
        path: String,
    },
    Read {
        path: String,
        offset: u64,
        length: u32,
    },
    ReadDirectory {
        path: String,
        cursor: u64,
        limit: u16,
    },
    Write {
        path: String,
        offset: u64,
        data: Vec<u8>,
    },
    CreateDirectory {
        path: String,
    },
    Rename {
        from: String,
        to: String,
    },
    /// Compare the destination and rename in one filesystem-owner turn.
    /// None requires a missing destination; Some requires an exact regular file.
    /// A mismatch returns Busy without mutating either path.
    CompareRename {
        from: String,
        to: String,
        expected: Option<Vec<u8>>,
    },
    Remove {
        path: String,
        recursive: bool,
    },
    Copy {
        from: String,
        to: String,
        recursive: bool,
    },
    Link {
        from: String,
        to: String,
    },
    ReadLink {
        path: String,
    },
    ReplaceLink {
        path: String,
        target: String,
    },
    SetMode {
        path: String,
        mode: u32,
    },
    Archive {
        sources: Vec<String>,
        destination: String,
        format: ArchiveFormat,
    },
    Extract {
        archive: String,
        destination: String,
    },
    Cancel {
        target: RequestId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ArchiveFormat {
    Zip = 0,
    TarGzip = 1,
    Gzip = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsResponse {
    pub request: RequestId,
    pub result: VfsResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VfsResult {
    Metadata(FileMetadata),
    Data(Vec<u8>),
    Directory {
        entries: Vec<DirectoryEntry>,
        next_cursor: Option<u64>,
    },
    Written(u32),
    Done,
    Error(VfsError),
    Progress(VfsProgress),
    Cancelled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VfsProgress {
    /// Monotonically increasing within one request. Zero is never emitted.
    pub sequence: u32,
    pub completed_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FileKind {
    File = 0,
    Directory = 1,
    Symlink = 2,
    Other = 3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub kind: FileKind,
    pub size: u64,
    pub modified_millis: i64,
    pub mode: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub name: String,
    pub metadata: FileMetadata,
    pub mime_type: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum VfsErrorCode {
    NotFound = 0,
    PermissionDenied = 1,
    AlreadyExists = 2,
    InvalidInput = 3,
    NotSupported = 4,
    Busy = 5,
    Io = 6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VfsError {
    pub code: VfsErrorCode,
    pub message: String,
}
