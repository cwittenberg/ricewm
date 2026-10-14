import {
    normalizeGuestEntryName,
    normalizeGuestPath,
    RiceGuestPathError,
} from "./rice-guest-path.mjs";
import {
    decodeRiceVfsRequest,
    encodeRiceVfsResponse,
    RiceVfsErrorCode,
    RiceVfsFileKind,
    RICE_VFS_LIMITS,
} from "./rice-vfs-wire.mjs";

const encoder = new TextEncoder();
const decoder = new TextDecoder();
const POSIX_ERROR_CODES = Object.freeze({
    ENOENT: RiceVfsErrorCode.NOT_FOUND,
    EACCES: RiceVfsErrorCode.PERMISSION_DENIED,
    EPERM: RiceVfsErrorCode.PERMISSION_DENIED,
    EEXIST: RiceVfsErrorCode.ALREADY_EXISTS,
    EINVAL: RiceVfsErrorCode.INVALID_INPUT,
    EISDIR: RiceVfsErrorCode.INVALID_INPUT,
    ENOTDIR: RiceVfsErrorCode.INVALID_INPUT,
    ENOTEMPTY: RiceVfsErrorCode.BUSY,
    ENOTSUP: RiceVfsErrorCode.NOT_SUPPORTED,
    ENOSYS: RiceVfsErrorCode.NOT_SUPPORTED,
    EBUSY: RiceVfsErrorCode.BUSY,
});
const FILE_KINDS = new Set(Object.values(RiceVfsFileKind));

export class RiceVfsProviderError extends Error {
    constructor(code, message) {
        super(message);
        this.name = "RiceVfsProviderError";
        this.code = code;
    }
}

function checkedCallbacks(callbacks) {
    if (!callbacks || typeof callbacks !== "object") {
        throw new TypeError("VFS provider callbacks must be an object");
    }
    const output = {};
    for (const name of [
        "stat",
        "read",
        "readDirectory",
        "write",
        "mkdir",
        "rename",
        "remove",
        "copy",
        "link",
        "readLink",
        "replaceLink",
        "compareRename",
        "setMode",
        "archive",
        "extract",
        "watch",
    ]) {
        if (callbacks[name] !== undefined) {
            if (typeof callbacks[name] !== "function") {
                throw new TypeError(`VFS callback ${name} must be a function`);
            }
            output[name] = callbacks[name];
        }
    }
    return Object.freeze(output);
}

function checkedBytes(value, field) {
    let bytes;
    if (value instanceof Uint8Array) {
        bytes = value;
    } else if (value instanceof ArrayBuffer) {
        bytes = new Uint8Array(value);
    } else if (ArrayBuffer.isView(value)) {
        bytes = new Uint8Array(
            value.buffer,
            value.byteOffset,
            value.byteLength,
        );
    } else {
        throw new TypeError(`${field} must be a byte buffer`);
    }
    return bytes.slice();
}

function checkedU32(value, field) {
    if (
        !Number.isSafeInteger(value) ||
        value < 0 ||
        value > 0xffff_ffff
    ) {
        throw new RangeError(`${field} must be an unsigned 32-bit integer`);
    }
    return value;
}

function checkedU64(value, field) {
    let output;
    try {
        output = BigInt(value);
    } catch {
        throw new TypeError(`${field} must be an integer`);
    }
    if (output < 0n || output > 0xffff_ffff_ffff_ffffn) {
        throw new RangeError(`${field} must be an unsigned 64-bit integer`);
    }
    return output;
}

function checkedI64(value, field) {
    let output;
    try {
        output = BigInt(value);
    } catch {
        throw new TypeError(`${field} must be an integer`);
    }
    if (
        output < -0x8000_0000_0000_0000n ||
        output > 0x7fff_ffff_ffff_ffffn
    ) {
        throw new RangeError(`${field} must be a signed 64-bit integer`);
    }
    return output;
}

function normalizeMetadata(value) {
    if (!value || typeof value !== "object" || !FILE_KINDS.has(value.kind)) {
        throw new TypeError("VFS metadata has an invalid file kind");
    }
    return Object.freeze({
        kind: value.kind,
        size: checkedU64(value.size, "file size"),
        modifiedMillis: checkedI64(
            value.modifiedMillis,
            "modified timestamp",
        ),
        mode: checkedU32(value.mode, "file mode"),
    });
}

function normalizeDirectory(value, limit) {
    const page = Array.isArray(value)
        ? { entries: value, nextCursor: null }
        : value;
    if (!page || typeof page !== "object" || !Array.isArray(page.entries)) {
        throw new TypeError(
            "readDirectory must return entries or a directory page",
        );
    }
    if (
        page.entries.length > limit ||
        page.entries.length > RICE_VFS_LIMITS.maximumDirectoryEntries
    ) {
        throw new RangeError("directory provider returned too many entries");
    }
    const entries = page.entries.map(entry => {
        if (!entry || typeof entry !== "object") {
            throw new TypeError("directory entry must be an object");
        }
        const name = normalizeGuestEntryName(entry.name);
        const metadata = normalizeMetadata(entry.metadata);
        const suppliedMime = normalizeMimeType(entry.mimeType);
        // Package suffixes select the installer on every VFS backend, including
        // older native clients. The installer independently validates bytes.
        const packageFile = /\.riceapp$/iu.test(name) &&
            metadata.kind !== RiceVfsFileKind.DIRECTORY;
        return Object.freeze({name, metadata,
            mimeType: packageFile ? "application/x-ricewm-app" : suppliedMime,
        });
    });
    return {
        kind: "directory",
        entries,
        nextCursor: page.nextCursor === undefined ||
            page.nextCursor === null
            ? null
            : checkedU64(page.nextCursor, "next directory cursor"),
    };
}

function normalizeMimeType(value) {
    if (value === undefined || value === null) return null;
    if (
        typeof value !== "string" ||
        encoder.encode(value).byteLength >
            RICE_VFS_LIMITS.maximumMimeTypeBytes ||
        !/^[a-z0-9!#$&^_.+-]+\/[a-z0-9!#$&^_.+-]+$/u.test(value)
    ) {
        throw new RiceVfsProviderError(
            RiceVfsErrorCode.INVALID_INPUT,
            "directory entry MIME type is not canonical",
        );
    }
    return value;
}

function boundedMessage(value) {
    const message = typeof value === "string" && value.length > 0
        ? value
        : "VFS host operation failed";
    const encoded = encoder.encode(message);
    if (encoded.byteLength <= RICE_VFS_LIMITS.maximumTextBytes) {
        return message;
    }
    return decoder.decode(
        encoded.subarray(0, RICE_VFS_LIMITS.maximumTextBytes),
    ).replace(/\uFFFD$/u, "");
}

function errorResult(error) {
    if (error instanceof RiceGuestPathError) {
        return {
            kind: "error",
            code: RiceVfsErrorCode.INVALID_INPUT,
            message: boundedMessage(error.message),
        };
    }
    const explicit = error instanceof RiceVfsProviderError
        ? error.code
        : POSIX_ERROR_CODES[error?.code];
    const code = Object.values(RiceVfsErrorCode).includes(explicit)
        ? explicit
        : RiceVfsErrorCode.IO;
    return {
        kind: "error",
        code,
        message: boundedMessage(error?.message),
    };
}

function normalizedOperation(operation) {
    switch (operation.kind) {
    case "stat":
    case "create-directory":
    case "read-link":
        return { ...operation, path: normalizeGuestPath(operation.path) };
    case "set-mode": {
        const mode = checkedU32(operation.mode, "file mode");
        if (mode > 0o7777) {
            throw new RiceVfsProviderError(
                RiceVfsErrorCode.INVALID_INPUT,
                "file mode exceeds POSIX permission and special bits",
            );
        }
        return {
            ...operation,
            path: normalizeGuestPath(operation.path),
            mode,
        };
    }
    case "read":
    case "write":
    case "remove":
        return { ...operation, path: normalizeGuestPath(operation.path) };
    case "read-directory":
        return { ...operation, path: normalizeGuestPath(operation.path) };
    case "rename":
    case "compare-rename":
    case "copy":
    case "link":
        return {
            ...operation,
            from: normalizeGuestPath(operation.from),
            to: normalizeGuestPath(operation.to),
        };
    case "replace-link":
        return {
            ...operation,
            path: normalizeGuestPath(operation.path),
            target: normalizeGuestPath(operation.target),
        };
    case "archive":
        if (!Array.isArray(operation.sources) || operation.sources.length === 0 ||
            operation.sources.length > RICE_VFS_LIMITS.maximumDirectoryEntries) {
            throw new RiceVfsProviderError(
                RiceVfsErrorCode.INVALID_INPUT,
                "archive source count is invalid",
            );
        }
        if (!["zip", "tar-gzip", "gzip"].includes(operation.format)) {
            throw new RiceVfsProviderError(
                RiceVfsErrorCode.INVALID_INPUT,
                "archive format is invalid",
            );
        }
        return {
            ...operation,
            sources: operation.sources.map(normalizeGuestPath),
            destination: normalizeGuestPath(operation.destination),
        };
    case "extract":
        return {
            ...operation,
            archive: normalizeGuestPath(operation.archive),
            destination: normalizeGuestPath(operation.destination),
        };
    case "cancel":
        return operation;
    default:
        throw new RiceVfsProviderError(
            RiceVfsErrorCode.NOT_SUPPORTED,
            "VFS operation is not supported",
        );
    }
}

function callbackName(kind) {
    switch (kind) {
    case "read-directory": return "readDirectory";
    case "create-directory": return "mkdir";
    case "set-mode": return "setMode";
    case "read-link": return "readLink";
    case "replace-link": return "replaceLink";
    case "compare-rename": return "compareRename";
    default: return kind;
    }
}

/**
 * Request/response provider for RAPP VFS frames.
 *
 * Each injected callback receives one frozen object containing `request`,
 * `app`, and the normalized operation fields. Malformed frames reject; errors
 * after a request ID is decoded become bounded VFS error responses.
 */
export class RiceVfsProvider {
    constructor(callbacks, options = {}) {
        this.callbacks = checkedCallbacks(callbacks);
        this.active = new Map();
        this.cancellable = new Set(options.cancellableOperations || []);
        for (const kind of this.cancellable) {
            if (typeof kind !== "string") {
                throw new TypeError("cancellable operation names must be strings");
            }
        }
    }

    async handle(frame, onResponse = undefined) {
        const request = decodeRiceVfsRequest(frame);
        if (onResponse !== undefined && typeof onResponse !== "function") {
            throw new TypeError("VFS progress receiver must be a function");
        }
        if (request.operation.kind === "cancel") {
            return this.#cancel(request);
        }
        const key = requestKey(request.app, request.request);
        if (this.active.has(key)) {
            return encodeRiceVfsResponse({
                request: request.request,
                result: {
                    kind: "error",
                    code: RiceVfsErrorCode.BUSY,
                    message: "VFS request is already active",
                },
            });
        }
        const controller = new AbortController();
        const active = {
            controller,
            kind: request.operation.kind,
            sequence: 0,
            completedBytes: 0n,
            totalBytes: null,
        };
        this.active.set(key, active);
        let result;
        try {
            const operation = normalizedOperation(request.operation);
            const name = callbackName(operation.kind);
            const callback = this.callbacks[name];
            if (!callback) {
                throw new RiceVfsProviderError(
                    RiceVfsErrorCode.NOT_SUPPORTED,
                    `VFS host does not implement ${name}`,
                );
            }
            const input = Object.freeze({
                request: request.request,
                app: request.app,
                ...operation,
                signal: controller.signal,
                onProgress: (completedBytes, totalBytes) => {
                    this.#progress(request, active, onResponse, completedBytes, totalBytes);
                },
            });
            result = await this.#invoke(name, callback, input);
            if (controller.signal.aborted) result = { kind: "cancelled" };
        } catch (error) {
            result = controller.signal.aborted
                ? { kind: "cancelled" }
                : errorResult(error);
        } finally {
            if (this.active.get(key) === active) this.active.delete(key);
        }
        return encodeRiceVfsResponse({ request: request.request, result });
    }

    #cancel(request) {
        const key = requestKey(request.app, request.operation.target);
        const active = this.active.get(key);
        let result;
        if (!active) {
            result = {
                kind: "error",
                code: RiceVfsErrorCode.NOT_FOUND,
                message: "VFS operation is not active",
            };
        } else if (!this.cancellable.has(active.kind)) {
            result = {
                kind: "error",
                code: RiceVfsErrorCode.NOT_SUPPORTED,
                message: `${active.kind} cannot be cancelled by this provider`,
            };
        } else {
            active.controller.abort();
            result = { kind: "done" };
        }
        return encodeRiceVfsResponse({ request: request.request, result });
    }

    #progress(request, active, onResponse, completedValue, totalValue) {
        if (active.controller.signal.aborted) return;
        const completedBytes = checkedU64(completedValue, "completed bytes");
        const totalBytes = checkedU64(totalValue, "total bytes");
        if (
            completedBytes < active.completedBytes ||
            completedBytes > totalBytes ||
            (active.totalBytes !== null && totalBytes !== active.totalBytes)
        ) {
            throw new RangeError("VFS byte progress is not monotonic");
        }
        active.sequence += 1;
        if (active.sequence > 0xffff_ffff) {
            throw new RangeError("VFS progress sequence overflow");
        }
        active.completedBytes = completedBytes;
        active.totalBytes = totalBytes;
        if (!onResponse) return;
        onResponse(encodeRiceVfsResponse({
            request: request.request,
            result: {
                kind: "progress",
                sequence: active.sequence,
                completedBytes,
                totalBytes,
            },
        }));
    }

    watch(path, onChange) {
        const callback = this.callbacks.watch;
        if (!callback) throw new Error("VFS host does not implement watch");
        if (typeof onChange !== "function") {
            throw new TypeError("VFS watch listener must be a function");
        }
        const stop = callback(Object.freeze({
            path: normalizeGuestPath(path),
            onChange,
        }));
        if (typeof stop !== "function") {
            throw new TypeError("VFS watch must return an unsubscribe function");
        }
        return stop;
    }

    async #invoke(name, callback, input) {
        const value = await callback(input);
        switch (name) {
        case "stat":
            return { kind: "metadata", metadata: normalizeMetadata(value) };
        case "read": {
            const data = checkedBytes(value, "read result");
            if (
                data.byteLength > input.length ||
                data.byteLength > RICE_VFS_LIMITS.maximumTransferBytes
            ) {
                throw new RangeError("read provider returned too many bytes");
            }
            return { kind: "data", data };
        }
        case "readLink": {
            const data = checkedBytes(value, "link target");
            if (data.byteLength > RICE_VFS_LIMITS.maximumPathBytes) {
                throw new RangeError("link target exceeds the path limit");
            }
            return { kind: "data", data };
        }
        case "readDirectory":
            return normalizeDirectory(value, input.limit);
        case "write": {
            const count = checkedU32(value, "written count");
            if (count > input.data.byteLength) {
                throw new RangeError(
                    "write provider count exceeds the supplied data",
                );
            }
            return { kind: "written", count };
        }
        case "mkdir":
        case "rename":
        case "compareRename":
        case "remove":
        case "copy":
        case "link":
        case "replaceLink":
        case "setMode":
        case "archive":
        case "extract":
            return { kind: "done" };
        default:
            throw new RiceVfsProviderError(
                RiceVfsErrorCode.NOT_SUPPORTED,
                "VFS host operation is not supported",
            );
        }
    }
}

function requestKey(app, request) {
    return `${BigInt(app)}:${BigInt(request)}`;
}
