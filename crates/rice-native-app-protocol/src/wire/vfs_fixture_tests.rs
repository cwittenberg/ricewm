use crate::{
    AppId, DirectoryEntry, FileKind, FileMetadata, Message, RequestId, VfsOperation, VfsRequest,
    VfsResponse, VfsResult,
};

fn id<T: super::primitives::ProtocolId>(value: u64) -> T {
    T::new(value).expect("test ID must be nonzero")
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[test]
fn vfs_frames_have_stable_javascript_adapter_fixtures() {
    let request = Message::VfsRequest(VfsRequest {
        request: id::<RequestId>(0x0102_0304_0506_0708),
        app: id::<AppId>(9),
        operation: VfsOperation::ReadDirectory {
            path: "/home/rice".into(),
            cursor: 0x0a0b_0c0d_0e0f_1011,
            limit: 42,
        },
    })
    .encode()
    .expect("golden VFS request encodes");
    let response = Message::VfsResponse(VfsResponse {
        request: id(0x0102_0304_0506_0708),
        result: VfsResult::Directory {
            entries: vec![DirectoryEntry {
                name: "Documents".into(),
                metadata: FileMetadata {
                    kind: FileKind::Directory,
                    size: 0,
                    modified_millis: -2,
                    mode: 0o755,
                },
                mime_type: None,
            }],
            next_cursor: Some(99),
        },
    })
    .encode()
    .expect("golden VFS response encodes");

    assert_eq!(
        hex(&request),
        concat!(
            "5241505001000a002900000008070605040302010900000000000000020a000000",
            "2f686f6d652f7269636511100f0e0d0c0b0a2a00",
        )
    );
    assert_eq!(
        hex(&response),
        concat!(
            "5241505001000b00390000000807060504030201020100000009000000446f6375",
            "6d656e7473010000000000000000feffffffffffffffed01000000016300000000",
            "000000",
        )
    );
}
