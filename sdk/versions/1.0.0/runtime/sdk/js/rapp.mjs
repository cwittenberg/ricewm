const MAGIC = Uint8Array.of(0x52, 0x41, 0x50, 0x50);
const HEADER_BYTES = 12;
const VERSION = 1;
const MAX_TEXT_BYTES = 16 * 1024;
// Transport ceiling; native widget kinds retain their own smaller text bounds.
const MAX_WIDGET_PATCH_TEXT_BYTES = 256 * 1024;
const MAX_PATCHES = 4096;

class Writer {
    constructor() { this.bytes = []; }
    u8(value) { this.bytes.push(value & 0xff); }
    u16(value) { this.u8(value); this.u8(value >>> 8); }
    u32(value) { this.u16(value); this.u16(value >>> 16); }
    u64(value) {
        let remaining = BigInt(value);
        for (let index = 0; index < 8; index += 1) { this.u8(Number(remaining)); remaining >>= 8n; }
    }
    string(value, maximum = MAX_TEXT_BYTES) {
        const bytes = new TextEncoder().encode(String(value));
        if (bytes.length > maximum) throw new RangeError("RAPP text exceeds its limit");
        this.u32(bytes.length);
        for (const byte of bytes) this.bytes.push(byte);
    }
    optionString(value, maximum = MAX_TEXT_BYTES) { this.u8(value === null || value === undefined ? 0 : 1); if (value !== null && value !== undefined) this.string(value, maximum); }
    optionU16(value) { this.u8(value === null || value === undefined ? 0 : 1); if (value !== null && value !== undefined) this.u16(value); }
    optionId(value) { this.u64(value === null || value === undefined ? 0n : value); }
    patchU32(value, present) { this.u8(present ? 1 : 0); if (present) this.u32(value); }
    patchString(value, present, maximum = MAX_TEXT_BYTES) { this.u8(!present ? 0 : value === null ? 2 : 1); if (present && value !== null) this.string(value, maximum); }
    patchU16(value, present) { this.u8(!present ? 0 : value === null ? 2 : 1); if (present && value !== null) this.u16(value); }
}

class Reader {
    constructor(bytes) { this.bytes = bytes; this.position = HEADER_BYTES; }
    take(length) {
        const end = this.position + length;
        if (!Number.isSafeInteger(end) || end > this.bytes.length) throw new RangeError("truncated RAPP frame");
        const value = this.bytes.subarray(this.position, end);
        this.position = end;
        return value;
    }
    u8() { return this.take(1)[0]; }
    bool() { const value = this.u8(); if (value > 1) throw new RangeError("invalid RAPP boolean"); return value === 1; }
    u16() { const bytes = this.take(2); return bytes[0] | (bytes[1] << 8); }
    u32() { const bytes = this.take(4); return new DataView(bytes.buffer, bytes.byteOffset, 4).getUint32(0, true); }
    i32() { const bytes = this.take(4); return new DataView(bytes.buffer, bytes.byteOffset, 4).getInt32(0, true); }
    u64() { const bytes = this.take(8); return new DataView(bytes.buffer, bytes.byteOffset, 8).getBigUint64(0, true); }
    string(maximum = MAX_TEXT_BYTES) {
        const length = this.u32();
        if (length > maximum) throw new RangeError("RAPP string exceeds its limit");
        const bytes = this.take(length);
        // Shared Wasm/VFS frames are valid transport buffers, but Chromium's
        // TextDecoder requires nonshared storage. Copy only this bounded field.
        const textBytes = typeof SharedArrayBuffer !== "undefined" &&
            bytes.buffer instanceof SharedArrayBuffer ? Uint8Array.from(bytes) : bytes;
        return new TextDecoder("utf-8", { fatal: true }).decode(textBytes);
    }
    optionString() { return this.bool() ? this.string() : null; }
    finish() { if (this.position !== this.bytes.length) throw new RangeError("RAPP frame has trailing bytes"); }
}

function bytesOf(input) {
    if (input instanceof Uint8Array) return input;
    if (input instanceof ArrayBuffer) return new Uint8Array(input);
    if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
    throw new TypeError("RAPP value must be bytes");
}

export function validateRappFrame(input) {
    const bytes = bytesOf(input);
    if (bytes.length < HEADER_BYTES) throw new RangeError("RAPP frame is too short");
    if (!MAGIC.every((byte, index) => bytes[index] === byte)) throw new TypeError("invalid RAPP magic");
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint16(4, true) !== VERSION) throw new RangeError("unsupported RAPP version");
    if (view.getUint32(8, true) !== bytes.length - HEADER_BYTES) throw new RangeError("invalid RAPP payload length");
    return bytes;
}

function frame(kind, writer) {
    const payload = Uint8Array.from(writer.bytes);
    const output = new Uint8Array(HEADER_BYTES + payload.length);
    output.set(MAGIC);
    const view = new DataView(output.buffer);
    view.setUint16(4, VERSION, true);
    view.setUint16(6, kind, true);
    view.setUint32(8, payload.length, true);
    output.set(payload, HEADER_BYTES);
    return output;
}

const CLOSE_REASONS = ["requested", "last-window-closed", "host-shutdown", "protocol-fault"];
const POINTER_KINDS = ["move", "down", "up", "enter", "leave", "wheel"];

export function decodeRappMessage(input) {
    const bytes = validateRappFrame(input);
    const kind = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint16(6, true);
    const reader = new Reader(bytes);
    let output;
    switch (kind) {
        case 1: output = { type: "host-hello", capabilities: reader.u64(), maxMessageBytes: reader.u32() }; break;
        case 2: {
            const app = reader.u64();
            const entrypoint = reader.string();
            const count = reader.u32();
            if (count > 128) throw new RangeError("too many launch arguments");
            output = { type: "launch", app, entrypoint, arguments: Array.from({ length: count }, () => reader.string()) };
            break;
        }
        case 3: {
            const app = reader.u64();
            const count = reader.u32();
            if (count > 128) throw new RangeError("too many open paths");
            output = { type: "open", app, paths: Array.from({ length: count }, () => reader.string(4096)) };
            break;
        }
        case 4: output = { type: "close", app: reader.u64(), reason: CLOSE_REASONS[reader.u8()] }; break;
        case 5: output = { type: "close-request", request: reader.u64(), app: reader.u64(), reason: CLOSE_REASONS[reader.u8()] }; break;
        case 21: {
            const app = reader.u64(); const window = reader.u64(); const target = reader.u64();
            const tag = reader.u8();
            let event;
            if (tag === 0) event = { type: "activate" };
            else if (tag === 1) event = { type: "value-changed", value: reader.string() };
            else if (tag === 2) {
                const pointerKind = POINTER_KINDS[reader.u8()];
                if (!pointerKind) throw new RangeError("unknown pointer event kind");
                event = { type: "pointer", kind: pointerKind, x: reader.i32(), y: reader.i32(), button: reader.u8(), modifiers: reader.u16() };
            } else if (tag === 3) {
                event = { type: "key", pressed: reader.bool(), keyCode: reader.u32(), modifiers: reader.u16(), text: reader.optionString() };
                event.key = event.text;
            } else if (tag === 4) event = { type: "focus", focused: reader.bool() };
            else if (tag === 6) event = { type: "grid-resync" };
            else if (tag === 7) event = { type: "size-changed", width: reader.u32(), height: reader.u32() };
            else if (tag === 5) event = { type: "text-selection", anchor: reader.u32(), caret: reader.u32() };
            else throw new RangeError("unknown UI event kind");
            output = { type: "ui-event", app, window, target, event };
            break;
        }
        case 71: {
            const app = reader.u64();
            if (bytes.length > reader.position && bytes[reader.position] === 1) {
                reader.u8();
                const window = reader.u64(), contentWidth = reader.u32(), contentHeight = reader.u32();
                if (app === 0n || window === 0n || contentWidth < 1 || contentWidth > 65535 || contentHeight < 1 || contentHeight > 65535) {
                    throw new RangeError('Invalid native window state identity or dimensions');
                }
                output = { type: 'window-state', kind, bytes, app, window, state: Object.freeze({ contentWidth, contentHeight,
                    fullscreen: reader.bool(), maximized: reader.bool(), focused: reader.bool(), visible: reader.bool() }) };
                break;
            }
            if (bytes.length > reader.position && bytes[reader.position] === 19) {
                reader.u8();
                const window = reader.u64(), token = reader.u64(), width = reader.u32(), height = reader.u32(), error = reader.string(4096);
                if (app === 0n || window === 0n || token === 0n || ![width, height].every(value => value >= 1 && value <= 65535)) throw new RangeError('Invalid window size result');
                output = { type: 'window-size-result', kind, bytes, app, window, token, width, height, error };
                break;
            }
            // Presentation metadata arrives through the native host channel,
            // not a peer-controlled RiceBus event. Other tag-12 consumers keep
            // their existing opaque provider envelope.
            if (bytes.length > reader.position && bytes[reader.position] === 12) {
                const start = reader.position;
                reader.u8();
                const topic = reader.string(16 * 1024);
                if (topic === 'org.ricewm.sdk/theme') {
                    const payload = reader.string(16 * 1024);
                    output = { type: 'desktop-theme', app, palette: JSON.parse(payload) };
                    break;
                }
                reader.position = start;
            }
            // Other host events (including terminal geometry) all
            // begin with the owning app ID. Preserve their payload for SDK
            // consumers while routing them to the correct instance.
            output = { type: "provider", kind, app, bytes };
            reader.position = bytes.length;
            break;
        }
        default: output = { type: "provider", kind, bytes }; reader.position = bytes.length;
    }
    reader.finish();
    return Object.freeze(output);
}

export function encodeUiPatch(envelope) {
    if (envelope.patches.length > MAX_PATCHES) throw new RangeError("too many UI patches");
    const writer = new Writer();
    writer.u64(envelope.app); writer.u64(envelope.window); writer.u64(envelope.sequence);
    writer.u32(envelope.patches.length);
    for (const patch of envelope.patches) {
        if (patch.type === "create") {
            writer.u8(0); writer.u64(patch.id); writer.optionId(patch.parent); writer.u32(patch.index);
            writer.u16(patch.kind); writer.u32(patch.state); writer.optionString(patch.text, MAX_WIDGET_PATCH_TEXT_BYTES); writer.optionU16(patch.icon);
        } else if (patch.type === "update") {
            writer.u8(1); writer.u64(patch.id);
            writer.patchU32(patch.state, Object.hasOwn(patch, "state"));
            writer.patchString(patch.text, Object.hasOwn(patch, "text"), MAX_WIDGET_PATCH_TEXT_BYTES);
            writer.patchU16(patch.icon, Object.hasOwn(patch, "icon"));
        } else if (patch.type === "move") {
            writer.u8(2); writer.u64(patch.id); writer.optionId(patch.parent); writer.u32(patch.index);
        } else if (patch.type === "remove") { writer.u8(3); writer.u64(patch.id); }
        else if (patch.type === "clear") writer.u8(4);
        else if (patch.type === "observe-size") { writer.u8(6); writer.u64(patch.id); writer.u8(patch.enabled ? 1 : 0); }
        else if(patch.type==='icon-selector'){writer.u8(9);writer.u64(patch.id);writer.optionString(patch.selector,320);}
        else if (patch.type === "document-font") { writer.u8(8); writer.u64(patch.id); writer.optionString(patch.font, 1024); }
        else if (patch.type === "edit-selection") { writer.u8(7); writer.u64(patch.id); writer.u32(patch.anchor); writer.u32(patch.caret); }
        else if (patch.type === "grid-damage") {
            writer.u8(5); writer.u64(patch.id); writer.string(patch.damage, MAX_WIDGET_PATCH_TEXT_BYTES);
        }
        else throw new TypeError(`unknown UI patch ${patch.type}`);
    }
    return frame(20, writer);
}

export function encodeSetWindowTitle({ app, window, title }) {
    title = String(title);
    if (!title.trim()) throw new RangeError("window title must not be empty");
    const writer = new Writer();
    writer.u64(app);
    writer.u64(window);
    writer.u8(10);
    writer.string(title);
    return frame(70, writer);
}

export function encodeWindowAction({ app, window, operation, enabled, token, width, height, anchor='right' }) {
    if (operation === 'content-size') {
        if (BigInt(app) <= 0n || BigInt(window) <= 0n || BigInt(token) <= 0n || BigInt(token) > 0xffffffffffffffffn ||
            ![width, height].every(value => Number.isInteger(value) && value >= 96 && value <= 4096)) throw new TypeError('Invalid native window content size');
        const anchors = ['right','left','top','bottom'];
        if (!anchors.includes(anchor)) throw new TypeError('Invalid content resize anchor');
        const writer = new Writer();
        writer.u64(app); writer.u64(window); writer.u8(43); writer.u64(token); writer.u32(width); writer.u32(height); writer.u8(anchors.indexOf(anchor));
        return frame(70, writer);
    }
    if (BigInt(app) <= 0n || BigInt(window) <= 0n || typeof enabled !== 'boolean') throw new TypeError('Invalid native window action');
    if (!['fullscreen', 'titlebar-visible', 'close'].includes(operation)) throw new TypeError('Unknown native window action');
    const writer = new Writer();
    writer.u64(app); writer.u64(window);
    if (operation === 'close') writer.u8(3);
    else { writer.u8(operation === 'fullscreen' ? 0 : 26); writer.u8(enabled ? 1 : 0); }
    return frame(70, writer);
}

export function encodeCloseResponse({ request, app, decision = "approved" }) {
    const decisions = { approved: 0, pending: 1, cancelled: 2 };
    if (!Object.hasOwn(decisions, decision)) throw new TypeError("invalid close decision");
    const writer = new Writer();
    writer.u64(request); writer.u64(app); writer.u8(decisions[decision]);
    return frame(6, writer);
}
