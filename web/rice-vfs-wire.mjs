import {
    decodeVfsFrame,
    decodeVfsMetadata,
    encodeVfsFrame,
    encodeVfsMetadata,
    ERROR_CODES,
    MAX_DIRECTORY_ENTRIES,
    MAX_MIME_TYPE_BYTES,
    MAX_PATH_BYTES,
    MAX_TEXT_BYTES,
    MAX_TRANSFER_BYTES,
    REQUEST_KIND,
    RESPONSE_KIND,
    VfsWireWriter,
} from "./rice-vfs-wire-core.mjs";

export {
    RICE_VFS_LIMITS,
    RiceVfsErrorCode,
    RiceVfsFileKind,
} from "./rice-vfs-wire-core.mjs";

function encodeRequestPayload(request) {
    if (!request || typeof request !== "object") {
        throw new TypeError("VFS request must be an object");
    }
    const writer = new VfsWireWriter();
    writer.id(request.request, "request ID");
    writer.id(request.app, "application ID");
    const operation = request.operation;
    if (!operation || typeof operation !== "object") {
        throw new TypeError("VFS operation must be an object");
    }
    switch (operation.kind) {
    case "stat":
        writer.u8(0);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        break;
    case "read":
        writer.u8(1);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        writer.u64(operation.offset, "read offset");
        writer.u32(operation.length);
        if (operation.length > MAX_TRANSFER_BYTES) {
            throw new RangeError("read length exceeds the transfer limit");
        }
        break;
    case "read-directory":
        writer.u8(2);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        writer.u64(operation.cursor, "directory cursor");
        writer.u16(operation.limit);
        if (operation.limit === 0 || operation.limit > MAX_DIRECTORY_ENTRIES) {
            throw new RangeError("directory limit is invalid");
        }
        break;
    case "write":
        writer.u8(3);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        writer.u64(operation.offset, "write offset");
        writer.data(operation.data, MAX_TRANSFER_BYTES, "write data");
        break;
    case "create-directory":
        writer.u8(4);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        break;
    case "rename":
        writer.u8(5);
        writer.text(operation.from, MAX_PATH_BYTES, "source path");
        writer.text(operation.to, MAX_PATH_BYTES, "destination path");
        break;
    case "compare-rename":
        writer.u8(15);
        writer.text(operation.from, MAX_PATH_BYTES, "source path");
        writer.text(operation.to, MAX_PATH_BYTES, "destination path");
        if (operation.expected !== null && !(operation.expected instanceof Uint8Array)) {
            throw new TypeError("expected destination must be bytes or null");
        }
        writer.bool(operation.expected !== null);
        if (operation.expected !== null) writer.data(operation.expected, MAX_TRANSFER_BYTES, "expected destination");
        break;
    case "remove":
        writer.u8(6);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        writer.bool(operation.recursive);
        break;
    case "copy":
        writer.u8(7);
        writer.text(operation.from, MAX_PATH_BYTES, "source path");
        writer.text(operation.to, MAX_PATH_BYTES, "destination path");
        writer.bool(operation.recursive);
        break;
    case "link":
        writer.u8(8);
        writer.text(operation.from, MAX_PATH_BYTES, "source path");
        writer.text(operation.to, MAX_PATH_BYTES, "destination path");
        break;
    case "read-link":
        writer.u8(13);
        writer.text(operation.path, MAX_PATH_BYTES, "link path");
        break;
    case "replace-link":
        writer.u8(14);
        writer.text(operation.path, MAX_PATH_BYTES, "link path");
        writer.text(operation.target, MAX_PATH_BYTES, "link target");
        break;
    case "set-mode":
        writer.u8(9);
        writer.text(operation.path, MAX_PATH_BYTES, "path");
        writer.u32(operation.mode);
        break;
    case "archive":
        if (!Array.isArray(operation.sources) || operation.sources.length === 0 ||
            operation.sources.length > MAX_DIRECTORY_ENTRIES) {
            throw new RangeError("archive source count is invalid");
        }
        if (operation.format === "gzip" && operation.sources.length !== 1) {
            throw new RangeError("GZIP requires one archive source");
        }
        const format = ["zip", "tar-gzip", "gzip"].indexOf(operation.format);
        if (format < 0) throw new RangeError("archive format is unknown");
        writer.u8(11);
        writer.u32(operation.sources.length);
        for (const source of operation.sources) {
            writer.text(source, MAX_PATH_BYTES, "archive source path");
        }
        writer.text(operation.destination, MAX_PATH_BYTES, "archive destination");
        writer.u8(format);
        break;
    case "extract":
        writer.u8(12);
        writer.text(operation.archive, MAX_PATH_BYTES, "archive path");
        writer.text(operation.destination, MAX_PATH_BYTES, "extraction destination");
        break;
    case "cancel":
        if (BigInt(operation.target) === BigInt(request.request)) {
            throw new RangeError("cancel target must differ from request ID");
        }
        writer.u8(10);
        writer.id(operation.target, "cancel target");
        break;
    default:
        throw new RangeError("VFS operation kind is unknown");
    }
    return writer.bytes;
}

function decodeRequestPayload(reader) {
    const request = reader.id("request ID");
    const app = reader.id("application ID");
    const tag = reader.u8("VFS operation");
    let operation;
    if (tag === 0) {
        operation = { kind: "stat", path: reader.text(MAX_PATH_BYTES, "path") };
    } else if (tag === 1) {
        const path = reader.text(MAX_PATH_BYTES, "path");
        const offset = reader.u64("read offset");
        const length = reader.u32("read length");
        if (length > MAX_TRANSFER_BYTES) {
            throw new RangeError("read length exceeds the transfer limit");
        }
        operation = { kind: "read", path, offset, length };
    } else if (tag === 2) {
        const path = reader.text(MAX_PATH_BYTES, "path");
        const cursor = reader.u64("directory cursor");
        const limit = reader.u16("directory limit");
        if (limit === 0 || limit > MAX_DIRECTORY_ENTRIES) {
            throw new RangeError("directory limit is invalid");
        }
        operation = { kind: "read-directory", path, cursor, limit };
    } else if (tag === 3) {
        operation = {
            kind: "write",
            path: reader.text(MAX_PATH_BYTES, "path"),
            offset: reader.u64("write offset"),
            data: reader.data(MAX_TRANSFER_BYTES, "write data"),
        };
    } else if (tag === 4) {
        operation = {
            kind: "create-directory",
            path: reader.text(MAX_PATH_BYTES, "path"),
        };
    } else if (tag === 5) {
        operation = {
            kind: "rename",
            from: reader.text(MAX_PATH_BYTES, "source path"),
            to: reader.text(MAX_PATH_BYTES, "destination path"),
        };
    } else if (tag === 6) {
        operation = {
            kind: "remove",
            path: reader.text(MAX_PATH_BYTES, "path"),
            recursive: reader.bool("recursive"),
        };
    } else if (tag === 7) {
        operation = {
            kind: "copy",
            from: reader.text(MAX_PATH_BYTES, "source path"),
            to: reader.text(MAX_PATH_BYTES, "destination path"),
            recursive: reader.bool("recursive"),
        };
    } else if (tag === 8) {
        operation = {
            kind: "link",
            from: reader.text(MAX_PATH_BYTES, "source path"),
            to: reader.text(MAX_PATH_BYTES, "destination path"),
        };
    } else if (tag === 9) {
        operation = {
            kind: "set-mode",
            path: reader.text(MAX_PATH_BYTES, "path"),
            mode: reader.u32("file mode"),
        };
    } else if (tag === 10) {
        const target = reader.id("cancel target");
        if (target === request) {
            throw new RangeError("cancel target must differ from request ID");
        }
        operation = { kind: "cancel", target };
    } else if (tag === 11) {
        const count = reader.u32("archive source count");
        if (count === 0 || count > MAX_DIRECTORY_ENTRIES) {
            throw new RangeError("archive source count is invalid");
        }
        const sources = [];
        for (let index = 0; index < count; index += 1) {
            sources.push(reader.text(MAX_PATH_BYTES, "archive source path"));
        }
        const destination = reader.text(MAX_PATH_BYTES, "archive destination");
        const formats = ["zip", "tar-gzip", "gzip"];
        const format = formats[reader.u8("archive format")];
        if (!format) throw new RangeError("archive format is unknown");
        if (format === "gzip" && sources.length !== 1) {
            throw new RangeError("GZIP requires one archive source");
        }
        operation = { kind: "archive", sources, destination, format };
    } else if (tag === 12) {
        operation = {
            kind: "extract",
            archive: reader.text(MAX_PATH_BYTES, "archive path"),
            destination: reader.text(MAX_PATH_BYTES, "extraction destination"),
        };
    } else if (tag === 13) {
        operation = {
            kind: "read-link",
            path: reader.text(MAX_PATH_BYTES, "link path"),
        };
    } else if (tag === 15) {
        operation = {
            kind: "compare-rename",
            from: reader.text(MAX_PATH_BYTES, "source path"),
            to: reader.text(MAX_PATH_BYTES, "destination path"),
            expected: reader.bool("expected destination present") ? reader.data(MAX_TRANSFER_BYTES, "expected destination") : null,
        };
    } else if (tag === 14) {
        operation = {
            kind: "replace-link",
            path: reader.text(MAX_PATH_BYTES, "link path"),
            target: reader.text(MAX_PATH_BYTES, "link target"),
        };
    } else {
        throw new RangeError("VFS operation tag is unknown");
    }
    reader.finish();
    return Object.freeze({
        request,
        app,
        operation: Object.freeze(operation),
    });
}

function encodeResponsePayload(response) {
    if (!response || typeof response !== "object") {
        throw new TypeError("VFS response must be an object");
    }
    const writer = new VfsWireWriter();
    writer.id(response.request, "request ID");
    const result = response.result;
    if (!result || typeof result !== "object") {
        throw new TypeError("VFS result must be an object");
    }
    switch (result.kind) {
    case "metadata":
        writer.u8(0);
        encodeVfsMetadata(writer, result.metadata);
        break;
    case "data":
        writer.u8(1);
        writer.data(result.data, MAX_TRANSFER_BYTES, "read data");
        break;
    case "directory":
        writer.u8(2);
        if (
            !Array.isArray(result.entries) ||
            result.entries.length > MAX_DIRECTORY_ENTRIES
        ) {
            throw new RangeError("directory entries exceed the protocol limit");
        }
        writer.u32(result.entries.length);
        for (const entry of result.entries) {
            writer.text(entry.name, MAX_PATH_BYTES, "entry name");
            encodeVfsMetadata(writer, entry.metadata);
            encodeMimeType(writer, entry.mimeType);
        }
        if (result.nextCursor === null || result.nextCursor === undefined) {
            writer.u8(0);
        } else {
            writer.u8(1);
            writer.u64(result.nextCursor, "next directory cursor");
        }
        break;
    case "written":
        writer.u8(3);
        writer.u32(result.count);
        break;
    case "done":
        writer.u8(4);
        break;
    case "error": {
        writer.u8(5);
        const code = ERROR_CODES.indexOf(result.code);
        if (code < 0) throw new RangeError("VFS error code is unknown");
        writer.u8(code);
        writer.text(result.message, MAX_TEXT_BYTES, "VFS error message");
        break;
    }
    case "progress":
        if (
            !Number.isSafeInteger(result.sequence) ||
            result.sequence <= 0 ||
            result.sequence > 0xffff_ffff
        ) {
            throw new RangeError("VFS progress sequence is invalid");
        }
        if (BigInt(result.completedBytes) > BigInt(result.totalBytes)) {
            throw new RangeError("VFS progress counters are invalid");
        }
        writer.u8(6);
        writer.u32(result.sequence);
        writer.u64(result.completedBytes, "completed bytes");
        writer.u64(result.totalBytes, "total bytes");
        break;
    case "cancelled":
        writer.u8(7);
        break;
    default:
        throw new RangeError("VFS result kind is unknown");
    }
    return writer.bytes;
}

function decodeResponsePayload(reader) {
    const request = reader.id("request ID");
    const tag = reader.u8("VFS result");
    let result;
    if (tag === 0) {
        result = { kind: "metadata", metadata: decodeVfsMetadata(reader) };
    } else if (tag === 1) {
        result = {
            kind: "data",
            data: reader.data(MAX_TRANSFER_BYTES, "read data"),
        };
    } else if (tag === 2) {
        const count = reader.u32("directory entry count");
        if (count > MAX_DIRECTORY_ENTRIES) {
            throw new RangeError("directory entries exceed the protocol limit");
        }
        const entries = [];
        for (let index = 0; index < count; index += 1) {
            entries.push(Object.freeze({
                name: reader.text(MAX_PATH_BYTES, "entry name"),
                metadata: decodeVfsMetadata(reader),
                mimeType: decodeMimeType(reader),
            }));
        }
        const hasCursor = reader.u8("next cursor presence");
        if (hasCursor > 1) {
            throw new RangeError("next cursor presence is not canonical");
        }
        result = {
            kind: "directory",
            entries: Object.freeze(entries),
            nextCursor: hasCursor ? reader.u64("next directory cursor") : null,
        };
    } else if (tag === 3) {
        result = { kind: "written", count: reader.u32("written count") };
    } else if (tag === 4) {
        result = { kind: "done" };
    } else if (tag === 5) {
        const code = reader.u8("VFS error code");
        if (code >= ERROR_CODES.length) {
            throw new RangeError("VFS error code tag is unknown");
        }
        result = {
            kind: "error",
            code: ERROR_CODES[code],
            message: reader.text(MAX_TEXT_BYTES, "VFS error message"),
        };
    } else if (tag === 6) {
        const sequence = reader.u32("VFS progress sequence");
        const completedBytes = reader.u64("completed bytes");
        const totalBytes = reader.u64("total bytes");
        if (sequence === 0 || completedBytes > totalBytes) {
            throw new RangeError("VFS progress frame is invalid");
        }
        result = { kind: "progress", sequence, completedBytes, totalBytes };
    } else if (tag === 7) {
        result = { kind: "cancelled" };
    } else {
        throw new RangeError("VFS result tag is unknown");
    }
    reader.finish();
    return Object.freeze({
        request,
        result: Object.freeze(result),
    });
}

function canonicalMimeType(value) {
    return typeof value === "string" &&
        value.length > 0 &&
        value.length <= MAX_MIME_TYPE_BYTES &&
        /^[a-z0-9!#$&^_.+-]+\/[a-z0-9!#$&^_.+-]+$/u.test(value);
}

function encodeMimeType(writer, value) {
    if (value === null || value === undefined) {
        writer.bool(false);
        return;
    }
    if (!canonicalMimeType(value)) {
        throw new RangeError("MIME type is not canonical");
    }
    writer.bool(true);
    writer.text(value, MAX_MIME_TYPE_BYTES, "MIME type");
}

function decodeMimeType(reader) {
    if (!reader.bool("MIME type presence")) return null;
    const value = reader.text(MAX_MIME_TYPE_BYTES, "MIME type");
    if (!canonicalMimeType(value)) {
        throw new RangeError("MIME type is not canonical");
    }
    return value;
}

export function encodeRiceVfsRequest(request) {
    return encodeVfsFrame(REQUEST_KIND, encodeRequestPayload(request));
}

export function decodeRiceVfsRequest(frame) {
    return decodeRequestPayload(decodeVfsFrame(frame, REQUEST_KIND));
}

export function encodeRiceVfsResponse(response) {
    return encodeVfsFrame(RESPONSE_KIND, encodeResponsePayload(response));
}

export function decodeRiceVfsResponse(frame) {
    return decodeResponsePayload(decodeVfsFrame(frame, RESPONSE_KIND));
}
