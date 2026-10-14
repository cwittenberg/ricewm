const HEADER_BYTES = 12;
const MAGIC = Uint8Array.of(0x52, 0x41, 0x50, 0x50);
const VERSION = 1;
export const REQUEST_KIND = 10;
export const RESPONSE_KIND = 11;
const MAX_MESSAGE_BYTES = 20 * 1024 * 1024;
export const MAX_PATH_BYTES = 4 * 1024;
export const MAX_TEXT_BYTES = 16 * 1024;
export const MAX_TRANSFER_BYTES = 512 * 1024;
export const MAX_DIAGNOSTIC_BYTES = 16 * 1024 * 1024;
export const MAX_CLIPBOARD_BYTES = MAX_DIAGNOSTIC_BYTES;
export const MAX_DIRECTORY_ENTRIES = 4_096;
export const MAX_MIME_TYPE_BYTES = 255;
const U64_MAX = 0xffff_ffff_ffff_ffffn;
const I64_MIN = -0x8000_0000_0000_0000n;
const I64_MAX = 0x7fff_ffff_ffff_ffffn;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

export const RiceVfsFileKind = Object.freeze({
    FILE: "file",
    DIRECTORY: "directory",
    SYMLINK: "symlink",
    OTHER: "other",
});

export const RiceVfsErrorCode = Object.freeze({
    NOT_FOUND: "not-found",
    PERMISSION_DENIED: "permission-denied",
    ALREADY_EXISTS: "already-exists",
    INVALID_INPUT: "invalid-input",
    NOT_SUPPORTED: "not-supported",
    BUSY: "busy",
    IO: "io",
});

export const FILE_KINDS = Object.freeze([
    RiceVfsFileKind.FILE,
    RiceVfsFileKind.DIRECTORY,
    RiceVfsFileKind.SYMLINK,
    RiceVfsFileKind.OTHER,
]);
export const ERROR_CODES = Object.freeze([
    RiceVfsErrorCode.NOT_FOUND,
    RiceVfsErrorCode.PERMISSION_DENIED,
    RiceVfsErrorCode.ALREADY_EXISTS,
    RiceVfsErrorCode.INVALID_INPUT,
    RiceVfsErrorCode.NOT_SUPPORTED,
    RiceVfsErrorCode.BUSY,
    RiceVfsErrorCode.IO,
]);

function asBytes(value, field) {
    if (value instanceof Uint8Array) return value;
    if (value instanceof ArrayBuffer) return new Uint8Array(value);
    if (ArrayBuffer.isView(value)) {
        return new Uint8Array(
            value.buffer,
            value.byteOffset,
            value.byteLength,
        );
    }
    throw new TypeError(`${field} must be a byte buffer`);
}

function checkedInteger(value, maximum, field) {
    if (!Number.isSafeInteger(value) || value < 0 || value > maximum) {
        throw new RangeError(`${field} is outside its unsigned range`);
    }
    return value;
}

function checkedBigInt(value, minimum, maximum, field) {
    let result;
    try {
        result = BigInt(value);
    } catch {
        throw new TypeError(`${field} must be an integer`);
    }
    if (result < minimum || result > maximum) {
        throw new RangeError(`${field} is outside its 64-bit range`);
    }
    return result;
}

function checkedUtf8(value, field) {
    if (typeof value !== "string") {
        throw new TypeError(`${field} must be a string`);
    }
    for (let index = 0; index < value.length; index += 1) {
        const unit = value.charCodeAt(index);
        if (unit >= 0xd800 && unit <= 0xdbff) {
            const next = value.charCodeAt(index + 1);
            if (!(next >= 0xdc00 && next <= 0xdfff)) {
                throw new TypeError(`${field} contains an unpaired surrogate`);
            }
            index += 1;
        } else if (unit >= 0xdc00 && unit <= 0xdfff) {
            throw new TypeError(`${field} contains an unpaired surrogate`);
        }
    }
    return encoder.encode(value);
}

export class VfsWireWriter {
    constructor() {
        this.buffer = new Uint8Array(128);
        this.length = 0;
    }

    get bytes() { return this.buffer.subarray(0, this.length); }

    reserve(count) {
        const required = this.length + count;
        if (required > MAX_MESSAGE_BYTES - HEADER_BYTES)
            throw new RangeError("VFS frame exceeds the RAPP message limit");
        if (required > this.buffer.length) {
            const buffer = new Uint8Array(Math.min(MAX_MESSAGE_BYTES - HEADER_BYTES,
                Math.max(required, this.buffer.length * 2)));
            buffer.set(this.bytes);
            this.buffer = buffer;
        }
        const offset = this.length;
        this.length = required;
        return offset;
    }

    u8(value) {
        const checked = checkedInteger(value, 0xff, "u8");
        const offset = this.reserve(1);
        this.buffer[offset] = checked;
    }

    bool(value) {
        if (typeof value !== "boolean") {
            throw new TypeError("boolean field must be true or false");
        }
        this.u8(value ? 1 : 0);
    }

    u16(value) {
        const checked = checkedInteger(value, 0xffff, "u16");
        const offset = this.reserve(2);
        new DataView(this.buffer.buffer).setUint16(offset, checked, true);
    }

    u32(value) {
        const checked = checkedInteger(value, 0xffff_ffff, "u32");
        const offset = this.reserve(4);
        new DataView(this.buffer.buffer).setUint32(offset, checked, true);
    }

    u64(value, field = "u64") {
        const checked = checkedBigInt(value, 0n, U64_MAX, field);
        const offset = this.reserve(8);
        new DataView(this.buffer.buffer).setBigUint64(offset, checked, true);
    }

    i64(value, field = "i64") {
        const checked = checkedBigInt(value, I64_MIN, I64_MAX, field);
        const offset = this.reserve(8);
        new DataView(this.buffer.buffer).setBigInt64(offset, checked, true);
    }

    id(value, field) {
        this.u64(checkedBigInt(value, 1n, U64_MAX, field), field);
    }

    data(value, maximum, field) {
        const bytes = asBytes(value, field);
        if (bytes.byteLength > maximum) {
            throw new RangeError(`${field} exceeds ${maximum} bytes`);
        }
        this.u32(bytes.byteLength);
        const offset = this.reserve(bytes.byteLength);
        this.buffer.set(bytes, offset);
    }

    text(value, maximum, field) {
        this.data(checkedUtf8(value, field), maximum, field);
    }
}

export class VfsWireReader {
    constructor(bytes) {
        this.bytes = bytes;
        this.view = new DataView(
            bytes.buffer,
            bytes.byteOffset,
            bytes.byteLength,
        );
        this.offset = 0;
    }

    take(length, field) {
        const end = this.offset + length;
        if (!Number.isSafeInteger(end) || end > this.bytes.byteLength) {
            throw new RangeError(`${field} is truncated`);
        }
        const output = this.bytes.subarray(this.offset, end);
        this.offset = end;
        return output;
    }

    u8(field = "u8") {
        return this.take(1, field)[0];
    }

    bool(field) {
        const value = this.u8(field);
        if (value > 1) throw new RangeError(`${field} is not canonical`);
        return value === 1;
    }

    u16(field = "u16") {
        const offset = this.offset;
        this.take(2, field);
        return this.view.getUint16(offset, true);
    }

    u32(field = "u32") {
        const offset = this.offset;
        this.take(4, field);
        return this.view.getUint32(offset, true);
    }

    u64(field = "u64") {
        const offset = this.offset;
        this.take(8, field);
        return this.view.getBigUint64(offset, true);
    }

    i64(field = "i64") {
        const offset = this.offset;
        this.take(8, field);
        return this.view.getBigInt64(offset, true);
    }

    id(field) {
        const value = this.u64(field);
        if (value === 0n) throw new RangeError(`${field} must be nonzero`);
        return value;
    }

    data(maximum, field) {
        const length = this.u32(`${field} length`);
        if (length > maximum) {
            throw new RangeError(`${field} exceeds ${maximum} bytes`);
        }
        return this.take(length, field).slice();
    }

    text(maximum, field) {
        try {
            return decoder.decode(this.data(maximum, field));
        } catch (error) {
            throw new TypeError(`${field} is not valid UTF-8`, {
                cause: error,
            });
        }
    }

    finish() {
        if (this.offset !== this.bytes.byteLength) {
            throw new RangeError("VFS frame contains trailing bytes");
        }
    }
}

export function encodeVfsFrame(kind, payload) {
    if (payload.length + HEADER_BYTES > MAX_MESSAGE_BYTES) {
        throw new RangeError("VFS frame exceeds the RAPP message limit");
    }
    const output = new Uint8Array(HEADER_BYTES + payload.length);
    output.set(MAGIC);
    const view = new DataView(output.buffer);
    view.setUint16(4, VERSION, true);
    view.setUint16(6, kind, true);
    view.setUint32(8, payload.length, true);
    output.set(payload, HEADER_BYTES);
    return output;
}

export function decodeVfsFrame(value, expectedKind) {
    const bytes = asBytes(value, "VFS frame");
    if (bytes.byteLength < HEADER_BYTES || bytes.byteLength > MAX_MESSAGE_BYTES) {
        throw new RangeError("VFS frame has an invalid byte length");
    }
    if (MAGIC.some((byte, index) => bytes[index] !== byte)) {
        throw new TypeError("VFS frame has invalid RAPP magic");
    }
    const view = new DataView(
        bytes.buffer,
        bytes.byteOffset,
        bytes.byteLength,
    );
    if (view.getUint16(4, true) !== VERSION) {
        throw new RangeError("VFS frame has an unsupported RAPP version");
    }
    if (view.getUint16(6, true) !== expectedKind) {
        throw new RangeError("VFS frame has the wrong message kind");
    }
    if (view.getUint32(8, true) !== bytes.byteLength - HEADER_BYTES) {
        throw new RangeError("VFS frame payload length does not match");
    }
    return new VfsWireReader(bytes.subarray(HEADER_BYTES));
}

export function encodeVfsMetadata(writer, metadata) {
    if (!metadata || typeof metadata !== "object") {
        throw new TypeError("file metadata must be an object");
    }
    const kind = FILE_KINDS.indexOf(metadata.kind);
    if (kind < 0) throw new RangeError("file metadata kind is unknown");
    writer.u8(kind);
    writer.u64(metadata.size, "file size");
    writer.i64(metadata.modifiedMillis, "modified timestamp");
    writer.u32(metadata.mode);
}

export function decodeVfsMetadata(reader) {
    const tag = reader.u8("file kind");
    if (tag >= FILE_KINDS.length) {
        throw new RangeError("file kind tag is unknown");
    }
    return Object.freeze({
        kind: FILE_KINDS[tag],
        size: reader.u64("file size"),
        modifiedMillis: reader.i64("modified timestamp"),
        mode: reader.u32("file mode"),
    });
}

export const RICE_VFS_LIMITS = Object.freeze({
    maximumMessageBytes: MAX_MESSAGE_BYTES,
    maximumPathBytes: MAX_PATH_BYTES,
    maximumTextBytes: MAX_TEXT_BYTES,
    maximumTransferBytes: MAX_TRANSFER_BYTES,
    maximumDirectoryEntries: MAX_DIRECTORY_ENTRIES,
    maximumMimeTypeBytes: MAX_MIME_TYPE_BYTES,
});
