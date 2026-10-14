const HEADER_BYTES=12,REQUEST_KIND=50,RESPONSE_KIND=51,MAX_TEXT_BYTES=16*1024;
function checkedFrame(input) {
    const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
    if (bytes.byteLength < HEADER_BYTES) throw new RangeError("short settings frame");
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (
        bytes[0] !== 0x52 || bytes[1] !== 0x41 ||
        bytes[2] !== 0x50 || bytes[3] !== 0x50 ||
        view.getUint16(6, true) !== REQUEST_KIND
    ) throw new TypeError("invalid RAPP settings request");
    if (view.getUint32(8, true) !== bytes.byteLength - HEADER_BYTES) {
        throw new RangeError("settings payload length mismatch");
    }
    return { bytes, view, cursor: HEADER_BYTES };
}

function readU64(frame) {
    const value = frame.view.getBigUint64(frame.cursor, true);
    frame.cursor += 8;
    return value;
}

function readString(frame) {
    const length = frame.view.getUint32(frame.cursor, true);
    frame.cursor += 4;
    if (length > MAX_TEXT_BYTES || frame.cursor + length > frame.bytes.byteLength) {
        throw new RangeError("invalid settings string");
    }
    const value = new TextDecoder("utf-8", { fatal: true }).decode(
        frame.bytes.subarray(frame.cursor, frame.cursor + length),
    );
    frame.cursor += length;
    return value;
}

export function decodeSettingsRequest(input) {
    const frame = checkedFrame(input);
    const request = readU64(frame);
    const app = readU64(frame);
    const tag = frame.bytes[frame.cursor++];
    const operation = tag === 0
        ? { kind: "load" }
        : tag === 1
            ? { kind: "set", key: readString(frame), value: readString(frame) }
            : tag === 2
                ? { kind: "prepare-recycle" }
                : tag === 3
                    ? { kind: "recycle", confirmation: readString(frame) }
                    : tag === 4
                        ? { kind: "export-preferences" }
                        : tag === 5
                            ? { kind: "import-preferences", document: readString(frame) }
                            : tag === 6
                                ? { kind: "wallpaper-theme", name: readString(frame), path: readString(frame) }
                                : tag === 7 ? {kind: "wallpaper-theme", document: readString(frame)}
                    : null;
    if (!operation || frame.cursor !== frame.bytes.byteLength) {
        throw new TypeError("invalid settings operation");
    }
    return { request, app, operation };
}

function pushU32(output, value) {
    output.push(value, value >>> 8, value >>> 16, value >>> 24);
}

function pushU64(output, value) {
    let remaining = BigInt(value);
    for (let index = 0; index < 8; index += 1) {
        output.push(Number(remaining & 0xffn));
        remaining >>= 8n;
    }
}

function pushString(output, value) {
    const bytes = new TextEncoder().encode(value);
    if (bytes.byteLength > MAX_TEXT_BYTES) throw new RangeError("settings string too long");
    pushU32(output, bytes.byteLength);
    for (const byte of bytes) output.push(byte);
}

export function encodeSettingsResponse(request, result) {
    const payload = [];
    pushU64(payload, request);
    if (result.kind === "wallpaper-recipe") {
        payload.push(6); pushString(payload, JSON.stringify(result.recipe));
    } else if (result.kind === "values") {
        payload.push(0);
        const entries = Object.entries(result.values).filter(([,value])=>value!==undefined);
        if(entries.length>512)throw new RangeError('too many settings');
        pushU32(payload, entries.length);
        for (const [key, value] of entries) {
            pushString(payload, key);
            pushString(payload, value);
        }
    } else if (result.kind === "applied") {
        payload.push(1);
        pushString(payload, result.key);
        pushString(payload, result.value);
    } else if (result.kind === "recycle-prepared") {
        payload.push(3);
        pushString(payload, result.confirmation);
    } else if (result.kind === "restarting") {
        payload.push(4);
    } else if (result.kind === "preferences-document") {
        payload.push(5);
        pushString(payload, result.document);
    } else {
        payload.push(2);
        pushString(payload, result.message);
    }
    const output = new Uint8Array(HEADER_BYTES + payload.length);
    output.set([0x52, 0x41, 0x50, 0x50]);
    const view = new DataView(output.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, RESPONSE_KIND, true);
    view.setUint32(8, payload.length, true);
    output.set(payload, HEADER_BYTES);
    return output;
}

