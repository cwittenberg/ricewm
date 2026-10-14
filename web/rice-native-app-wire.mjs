const RAPP_HEADER_BYTES = 12;
const RAPP_MAGIC = Uint8Array.of(0x52, 0x41, 0x50, 0x50);
const RAPP_KINDS = new Set([
    1, 2, 3, 4, 5, 6, 10, 11, 12, 13, 20, 21, 22, 30, 31, 40, 41, 50, 51, 60, 61,
    70, 71,
]);

export const RICE_NATIVE_APP_PROTOCOL_VERSION = 1;
export const RICE_NATIVE_APP_ABI_VERSION = 1;
export const RICE_NATIVE_APP_MAX_MESSAGE_BYTES = 72 * 1024 * 1024;

export const RiceNativeAppCapability = Object.freeze({
    CLIPBOARD: 1n << 0n,
    DRAG_AND_DROP: 1n << 1n,
    FILE_WATCH: 1n << 2n,
    IMAGE_DECODE: 1n << 3n,
    PRINT: 1n << 4n,
    PROCESS_LAUNCH: 1n << 5n,
    ENTROPY: 1n << 6n,
    CURRENCY_RATES: 1n << 7n,
    MOUNTS: 1n << 8n,
    NETWORK: 1n << 9n,
    GRID_DAMAGE: 1n << 10n,
    GRID_FILL_ENDPOINT: 1n << 11n,
    GRID_TEXT_STYLE: 1n << 12n,
    WIDGET_SIZE: 1n << 13n,
    GRID_LARGE_FRAME: 1n << 14n,
    GRID_FILTER_ROW: 1n << 15n,
    GRID_REFERENCES: 1n << 16n,
    TEXT_FIELD_SELECTION: 1n << 17n,
    TEXT_FIELD_DOCUMENT_FONT: 1n << 18n,
    GRID_PAINT: 1n << 19n,
    GRID_ZOOM: 1n << 20n,
    GRID_DOCUMENT_CANVAS: 1n << 21n,
    GRID_FRACTIONAL_FONT: 1n << 22n,
    GRID_MERGES: 1n << 24n,
    DOCUMENT_ALIGNMENT: 1n << 25n,
    SHEET_DATA_ICONS: 1n << 26n,
    ICON_SELECTORS: 1n << 28n,
    CONTENT_SHADOWS: 1n << 27n,
    ZOOM_ICONS: 1n << 23n,
    CONTROL_TYPOGRAPHY: 1n << 29n,
});

export const ALL_CAPABILITIES = Object.values(
    RiceNativeAppCapability,
).reduce((mask, capability) => mask | capability, 0n);

export const ADAPTER_CAPABILITIES = Object.freeze({
    clipboard: RiceNativeAppCapability.CLIPBOARD,
    drag: RiceNativeAppCapability.DRAG_AND_DROP,
    image: RiceNativeAppCapability.IMAGE_DECODE,
    print: RiceNativeAppCapability.PRINT,
    process: RiceNativeAppCapability.PROCESS_LAUNCH,
    entropy: RiceNativeAppCapability.ENTROPY,
    currency: RiceNativeAppCapability.CURRENCY_RATES,
    mounts: RiceNativeAppCapability.MOUNTS,
    network: RiceNativeAppCapability.NETWORK,
});

const CLOSE_REASONS = Object.freeze({
    requested: 0,
    "last-window-closed": 1,
    "host-shutdown": 2,
    "protocol-fault": 3,
});
const DRAG_PHASES = Object.freeze({
    enter: 0,
    motion: 1,
    leave: 2,
    drop: 3,
    finished: 4,
});
const DRAG_ACTIONS = Object.freeze({ copy: 0, move: 1, link: 2, ask: 3 });

export function asBytes(value, field) {
    if (value instanceof Uint8Array) return value;
    if (ArrayBuffer.isView(value)) {
        return new Uint8Array(
            value.buffer,
            value.byteOffset,
            value.byteLength,
        );
    }
    if (value instanceof ArrayBuffer) return new Uint8Array(value);
    throw new TypeError(`${field} must be a byte buffer`);
}

export function checkedLimit(value, fallback, maximum, field) {
    const result = value === undefined ? fallback : Number(value);
    if (
        !Number.isSafeInteger(result) ||
        result < 1 ||
        result > maximum
    ) {
        throw new RangeError(
            `${field} must be an integer from 1 through ${maximum}`,
        );
    }
    return result;
}

export function checkedId(value, field = "application ID") {
    let result;
    try {
        result = BigInt(value);
    } catch {
        throw new TypeError(`${field} must be a nonzero integer`);
    }
    if (result <= 0n || result > 0xffff_ffff_ffff_ffffn) {
        throw new RangeError(`${field} is outside the RAPP u64 range`);
    }
    return result;
}

function pushU32(output, value) {
    output.push(
        value & 0xff,
        (value >>> 8) & 0xff,
        (value >>> 16) & 0xff,
        (value >>> 24) & 0xff,
    );
}

function pushI32(output, value, field) {
    if (!Number.isInteger(value) || value < -0x8000_0000 || value > 0x7fff_ffff) {
        throw new RangeError(`${field} must be a signed 32-bit integer`);
    }
    pushU32(output, value >>> 0);
}

function pushU64(output, value) {
    let remaining = BigInt(value);
    for (let byte = 0; byte < 8; byte += 1) {
        output.push(Number(remaining & 0xffn));
        remaining >>= 8n;
    }
}

function pushString(output, value, maximum, field) {
    if (typeof value !== "string") {
        throw new TypeError(`${field} must be a string`);
    }
    const encoded = new TextEncoder().encode(value);
    if (encoded.byteLength > maximum) {
        throw new RangeError(`${field} exceeds ${maximum} bytes`);
    }
    pushU32(output, encoded.byteLength);
    output.push(...encoded);
}

function frame(kind, payload, maximumMessageBytes) {
    const length = RAPP_HEADER_BYTES + payload.length;
    if (length > maximumMessageBytes) {
        throw new RangeError(
            `RAPP frame exceeds the ${maximumMessageBytes}-byte limit`,
        );
    }
    const output = new Uint8Array(length);
    output.set(RAPP_MAGIC);
    const view = new DataView(output.buffer);
    view.setUint16(4, RICE_NATIVE_APP_PROTOCOL_VERSION, true);
    view.setUint16(6, kind, true);
    view.setUint32(8, payload.length, true);
    output.set(payload, RAPP_HEADER_BYTES);
    return output;
}

function closeReasonTag(reason) {
    const tag = typeof reason === "number" ? reason : CLOSE_REASONS[reason];
    if (!Number.isInteger(tag) || tag < 0 || tag > 3) {
        throw new RangeError("unknown native-app close reason");
    }
    return tag;
}

export function decodeRiceNativeCloseResponse(input) {
    const bytes = validateRiceNativeAppFrame(input);
    const view = new DataView(
        bytes.buffer,
        bytes.byteOffset,
        bytes.byteLength,
    );
    if (view.getUint16(6, true) !== 6 || bytes.byteLength !== 29) {
        throw new TypeError("frame is not a canonical close response");
    }
    const decision = bytes[28];
    if (decision > 2) throw new RangeError("unknown close decision");
    return Object.freeze({
        request: view.getBigUint64(12, true),
        app: view.getBigUint64(20, true),
        decision: ["approved", "pending", "cancelled"][decision],
    });
}

export function validateRiceNativeAppFrame(
    input,
    maximumMessageBytes = RICE_NATIVE_APP_MAX_MESSAGE_BYTES,
) {
    const bytes = asBytes(input, "RAPP frame");
    if (
        bytes.byteLength < RAPP_HEADER_BYTES ||
        bytes.byteLength > maximumMessageBytes
    ) {
        throw new RangeError("RAPP frame has an invalid byte length");
    }
    for (let index = 0; index < RAPP_MAGIC.length; index += 1) {
        if (bytes[index] !== RAPP_MAGIC[index]) {
            throw new TypeError("RAPP frame has invalid magic");
        }
    }
    const view = new DataView(
        bytes.buffer,
        bytes.byteOffset,
        bytes.byteLength,
    );
    const version = view.getUint16(4, true);
    if (version !== RICE_NATIVE_APP_PROTOCOL_VERSION) {
        throw new RangeError(`unsupported RAPP protocol version ${version}`);
    }
    const kind = view.getUint16(6, true);
    if (!RAPP_KINDS.has(kind)) {
        throw new RangeError(`unknown RAPP message kind ${kind}`);
    }
    const payloadLength = view.getUint32(8, true);
    if (payloadLength !== bytes.byteLength - RAPP_HEADER_BYTES) {
        throw new RangeError("RAPP payload length does not match its frame");
    }
    return bytes;
}

/** Canonical actor-targeted envelopes. Unknown/provider request-ID payloads
 * never get interpreted as actor IDs. Callers with a known provider owner use
 * that existing routing authority instead. */
export function riceNativeAppMutationOwner(input){
    const bytes=validateRiceNativeAppFrame(input),view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),kind=view.getUint16(6,true);
    if(![2,3,4,20,21,22,71].includes(kind)||bytes.byteLength<20)return null;
    const app=view.getBigUint64(12,true);return app>0n?app:null;
}

export const RiceNativeAppWireCodec = Object.freeze({
    hostHello(capabilities, maximumMessageBytes) {
        const payload = [];
        pushU64(payload, capabilities);
        pushU32(payload, maximumMessageBytes);
        return frame(1, payload, maximumMessageBytes);
    },

    launch(app, entrypoint, arguments_, maximumMessageBytes) {
        if (!Array.isArray(arguments_) || arguments_.length > 128) {
            throw new RangeError("launch arguments exceed the RAPP limit");
        }
        const payload = [];
        pushU64(payload, checkedId(app));
        pushString(payload, entrypoint, 16 * 1024, "entrypoint");
        pushU32(payload, arguments_.length);
        for (const argument of arguments_) {
            pushString(payload, argument, 16 * 1024, "argument");
        }
        return frame(2, payload, maximumMessageBytes);
    },

    open(app, paths, maximumMessageBytes) {
        if (!Array.isArray(paths) || paths.length > 128) {
            throw new RangeError("open paths exceed the RAPP limit");
        }
        const payload = [];
        pushU64(payload, checkedId(app));
        pushU32(payload, paths.length);
        for (const path of paths) {
            pushString(payload, path, 4 * 1024, "path");
        }
        return frame(3, payload, maximumMessageBytes);
    },

    close(app, reason, maximumMessageBytes) {
        const payload = [];
        pushU64(payload, checkedId(app));
        payload.push(closeReasonTag(reason));
        return frame(4, payload, maximumMessageBytes);
    },

    closeRequest(request, app, reason, maximumMessageBytes) {
        const payload = [];
        pushU64(payload, checkedId(request, "close request ID"));
        pushU64(payload, checkedId(app));
        payload.push(closeReasonTag(reason));
        return frame(5, payload, maximumMessageBytes);
    },

    drag(drag, maximumMessageBytes) {
        if (!drag || typeof drag !== "object") {
            throw new TypeError("drag envelope must be an object");
        }
        const phase = typeof drag.phase === "number"
            ? drag.phase
            : DRAG_PHASES[drag.phase];
        const action = typeof drag.action === "number"
            ? drag.action
            : DRAG_ACTIONS[drag.action];
        if (!Number.isInteger(phase) || phase < 0 || phase > 4) {
            throw new RangeError("drag phase is invalid");
        }
        if (!Number.isInteger(action) || action < 0 || action > 3) {
            throw new RangeError("drag action is invalid");
        }
        if (!Array.isArray(drag.mimeTypes) || drag.mimeTypes.length > 64) {
            throw new RangeError("drag MIME types exceed the RAPP limit");
        }
        const payload = [];
        pushU64(payload, checkedId(drag.drag, "drag ID"));
        pushU64(payload, checkedId(drag.app));
        pushU64(payload, checkedId(drag.window, "window ID"));
        pushU64(
            payload,
            drag.target === undefined || drag.target === null
                ? 0n
                : checkedId(drag.target, "target widget ID"),
        );
        payload.push(phase, action);
        pushI32(payload, Number(drag.x) || 0, "drag x");
        pushI32(payload, Number(drag.y) || 0, "drag y");
        pushU32(payload, drag.mimeTypes.length);
        for (const mime of drag.mimeTypes) {
            pushString(payload, mime, 16 * 1024, "drag MIME type");
        }
        if (drag.payload === undefined || drag.payload === null) {
            payload.push(0);
        } else {
            const data = asBytes(drag.payload.data, "drag payload");
            if (data.byteLength > 512 * 1024) {
                throw new RangeError("drag payload exceeds the RAPP transfer limit");
            }
            payload.push(1);
            pushString(
                payload,
                drag.payload.mimeType,
                16 * 1024,
                "drag payload MIME type",
            );
            pushU32(payload, data.byteLength);
            for (const byte of data) payload.push(byte);
        }
        return frame(40, payload, maximumMessageBytes);
    },

    validate: validateRiceNativeAppFrame,
});
