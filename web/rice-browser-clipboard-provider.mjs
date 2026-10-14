import {
    MAX_TEXT_BYTES,
    MAX_CLIPBOARD_BYTES,
    VfsWireWriter,
    decodeVfsFrame,
    encodeVfsFrame,
} from "./rice-vfs-wire-core.mjs";

const REQUEST_KIND = 30;
const RESPONSE_KIND = 31;
const TEXT_UTF8 = "text/plain;charset=utf-8";
const TEXT_PLAIN = "text/plain";
const decoder = new TextDecoder("utf-8", { fatal: true });
const encoder = new TextEncoder();

/** Write through the host even in content_shell, whose async clipboard
 * permission manager denies writes. The native editing command uses the same
 * platform clipboard as keyboard Copy and Paste. Restore focus after copying. */
export async function writeHostClipboardText(text, {clipboard=globalThis.navigator?.clipboard, document=globalThis.document}={}) {
    let failure;
    try {
        if (!clipboard?.writeText) throw new Error("Host clipboard unavailable");
        await clipboard.writeText(text);
        return;
    } catch (error) { failure = error; }
    if (!document?.body || typeof document.execCommand !== "function") throw failure;
    const focused = document.activeElement;
    const selection = document.getSelection?.();
    const ranges = [];
    for (let i=0; i<(selection?.rangeCount || 0); i++) ranges.push(selection.getRangeAt(i).cloneRange());
    const input = document.createElement("textarea");
    input.dataset.riceClipboardTransfer = "true";
    input.value = text;
    input.style.cssText = "position:fixed;left:-10000px;top:0;opacity:0";
    document.body.append(input);
    try {
        input.focus(); input.select();
        if (!document.execCommand("copy")) throw failure;
    } finally {
        input.remove(); focused?.focus?.({preventScroll:true});
        if (selection && ranges.length) { selection.removeAllRanges(); for (const range of ranges) selection.addRange(range); }
    }
}

/** Browser/native clipboard bridge for native Wasm applications. */
export class RiceBrowserClipboardProvider {
    constructor({
        clipboard = globalThis.navigator?.clipboard,
        selectionBridge = null,
        ClipboardItem_ = globalThis.ClipboardItem,
    } = {}) {
        this.clipboard = clipboard || null;
        this.ClipboardItem = ClipboardItem_;
        this.onChange = null;
        this.selectionBridge = selectionBridge;
        this.owned = {
            clipboard: null,
            primary: null,
        };
        this.selectionBridge?.setOwnerLostHandler(selection => {
            this.owned[selection] = null;
        });
    }

    get available() {
        return true;
    }

    async handle(message) {
        const request = decodeRequest(message);
        if (request.operation === "read") return this.#read(request);
        if (request.operation === "writeFormats") return this.#writeFormats(request);
        return this.#write(request);
    }

    /** Shared text-copy capability for shell adapters and native apps. */
    async writeText(value, {requireHost = false} = {}) {
        const data = encoder.encode(String(value));
        if (data.byteLength > MAX_CLIPBOARD_BYTES) {
            throw new RangeError("clipboard text exceeds the RiceWM transfer limit");
        }
        this.owned.clipboard = { mimeType: TEXT_UTF8, data: data.slice() };
        this.onChange?.({ mimeType: TEXT_UTF8, data: data.slice() });
        let selectionAccepted = false;
        let selectionError = null;
        if (this.selectionBridge) {
            try {
                await this.selectionBridge.write("clipboard", TEXT_UTF8, data);
                selectionAccepted = true;
            } catch (error) {
                selectionError = error;
            }
        }
        if (typeof this.clipboard?.writeText === "function" || globalThis.document?.execCommand) {
            try {
                await writeHostClipboardText(decoder.decode(data), {clipboard:this.clipboard});
            } catch (error) {
                if (requireHost || !selectionAccepted) throw selectionError || error;
            }
        } else if (requireHost || selectionError) {
            throw selectionError || new Error("Host clipboard unavailable");
        }
        return true;
    }

    /** Shared text-paste capability for standard native text controls. */
    async readText() {
        if (typeof this.clipboard?.readText === "function") {
            try {
                const value = await this.clipboard.readText();
                if (value) {
                    const data = encoder.encode(value);
                    if (data.byteLength > MAX_CLIPBOARD_BYTES) {
                        throw new RangeError("clipboard text exceeds the RiceWM transfer limit");
                    }
                    this.#rememberPlain("clipboard", { mimeType: TEXT_UTF8, data: data.slice() });
                    this.onChange?.({ mimeType: TEXT_UTF8, data: data.slice() });
                    await this.selectionBridge?.write("clipboard", TEXT_UTF8, data);
                    return value;
                }
                if (this.selectionBridge === null) {
                    this.owned.clipboard = null;
                    return "";
                }
            } catch {
                // Browser permission may be unavailable outside a trusted
                // gesture. Fall through to shared RiceWM/X11 ownership.
            }
        }
        const shared = await this.selectionBridge?.read(
            "clipboard",
            [TEXT_UTF8, TEXT_PLAIN],
        );
        if (shared && matchesText(shared.mimeType)) {
            const value = decoder.decode(shared.data);
            this.#rememberPlain("clipboard", {
                mimeType: shared.mimeType,
                data: shared.data.slice(),
            });
            return value;
        }
        const owned = this.owned.clipboard;
        return owned && matchesText(owned.mimeType) ? decoder.decode(owned.data) : "";
    }

    async #read(request) {
        if (request.accepted.some(type => mimeEssence(type) === "text/html") ||
            (this.owned[request.selection]?.formats && request.accepted.some(type => !matchesText(type)))) return this.#readFormats(request);
        try {
            if (
                request.selection === "clipboard" &&
                request.accepted.some(matchesText) &&
                typeof this.clipboard?.readText === "function"
            ) {
                try {
                    const systemText = await this.clipboard.readText();
                    if (!systemText) {
                        if (this.selectionBridge === null) {
                            this.owned.clipboard = null;
                            return encodeResponse(request.request, { result: "empty" });
                        }
                    } else {
                        const system = {
                            mimeType: TEXT_UTF8,
                            data: encoder.encode(systemText),
                        };
                        this.#rememberPlain("clipboard", system);
                        await this.selectionBridge?.write(
                            "clipboard",
                            system.mimeType,
                            system.data,
                        );
                        return encodeResponse(request.request, {
                            result: "data",
                            mimeType: request.accepted.find(matchesText),
                            data: system.data,
                        });
                    }
                } catch {
                    // Browser permission can be unavailable outside a trusted
                    // paste gesture; shared RiceWM ownership remains usable.
                }
            }
            const shared = await this.selectionBridge?.read(
                request.selection,
                request.accepted,
            );
            if (shared) {
                return encodeResponse(request.request, {
                    result: "data",
                    mimeType: shared.mimeType,
                    data: shared.data,
                });
            }
            let owned = this.owned[request.selection];
            if (!owned) return encodeResponse(request.request, { result: "empty" });
            const mimeType = request.accepted.find(
                accepted => accepted === owned.mimeType ||
                    (matchesText(accepted) && matchesText(owned.mimeType)),
            );
            if (!mimeType) return encodeResponse(request.request, { result: "rejected" });
            return encodeResponse(request.request, {
                result: "data",
                mimeType,
                data: owned.data,
            });
        } catch {
            return encodeResponse(request.request, { result: "rejected" });
        }
    }

    #rememberPlain(selection, value) {
        const owned = this.owned[selection];
        const plain = owned?.formats?.find(format => matchesText(format.mimeType));
        if (!plain || !equalBytes(plain.data, value.data)) this.owned[selection] = value;
    }

    async #readFormats(request) {
        const reply = formats => {
            if (!formats?.length) return encodeResponse(request.request, { result: "empty" });
            for (const accepted of request.accepted) {
                const format = formats.find(value => mimeEssence(value.mimeType) === mimeEssence(accepted));
                if (format) return encodeResponse(request.request, { result: "data", mimeType: accepted, data: format.data });
            }
            return encodeResponse(request.request, { result: "rejected" });
        };
        const formatsOf = value => value?.formats || (value ? [value] : []);
        try {
            if (request.selection === "clipboard" && typeof this.clipboard?.read === "function") {
                let items;
                try { items = await this.clipboard.read(); } catch { /* Permission denied: try shared ownership. */ }
                if (items !== undefined) {
                    // A successful system read supersedes owned data, including
                    // when an external application now offers only plain text.
                    this.owned.clipboard = null;
                    if (!items.length) return reply([]);
                    const item = items[0];
                    const types = item.types.filter(type => request.accepted.some(accepted => mimeEssence(accepted) === mimeEssence(type)));
                    if (types.length > 64) throw new RangeError("clipboard MIME count");
                    const formats = [];
                    let total = 0;
                    for (const mimeType of types) {
                        let blob;
                        try { blob = await item.getType(mimeType); } catch { continue; }
                        if (blob.size > MAX_CLIPBOARD_BYTES - total) throw new RangeError("clipboard aggregate data");
                        const data = new Uint8Array(await blob.arrayBuffer());
                        total += data.length;
                        formats.push({ mimeType, data });
                    }
                    if (formats.length) {
                        validateFormats(formats);
                        const plain = formats.find(format => matchesText(format.mimeType));
                        if (plain) {
                            try { await this.selectionBridge?.write("clipboard", plain.mimeType, plain.data); } catch { /* System data remains usable. */ }
                        }
                        this.owned.clipboard = ownedFormats(formats);
                        return reply(formats);
                    }
                    // Some browsers grant read() but deny a particular rich
                    // representation. Try readText/shared plain data below;
                    // the stale owned value has already been invalidated.
                }
            }
            if (request.selection === "clipboard" && typeof this.clipboard?.readText === "function") {
                let text;
                try { text = await this.clipboard.readText(); } catch { /* Fall back to owned representations. */ }
                if (text !== undefined) {
                    if (text) {
                        const plain = { mimeType: TEXT_UTF8, data: encoder.encode(text) };
                        if (plain.data.length > MAX_CLIPBOARD_BYTES) throw new RangeError("clipboard text");
                        this.#rememberPlain("clipboard", plain);
                        try { await this.selectionBridge?.write("clipboard", plain.mimeType, plain.data); } catch { /* System data remains usable. */ }
                        return reply(formatsOf(this.owned.clipboard));
                    }
                    if (!this.selectionBridge) { this.owned.clipboard = null; return reply([]); }
                }
            }
            const accepted = [...request.accepted];
            if (!accepted.some(matchesText)) accepted.push(TEXT_UTF8, TEXT_PLAIN);
            const shared = await this.selectionBridge?.read(request.selection, accepted);
            if (shared) {
                validateFormats([shared]);
                if (matchesText(shared.mimeType)) this.#rememberPlain(request.selection, shared);
                else this.owned[request.selection] = shared;
            }
            return reply(formatsOf(this.owned[request.selection]));
        } catch {
            return encodeResponse(request.request, { result: "rejected" });
        }
    }

    async #writeFormats(request) {
        // Validation/copying completes before either host endpoint or owned
        // state changes. Failed writes do not replace the previous clipboard.
        const formats = request.formats.map(format => ({ mimeType: format.mimeType, data: format.data.slice() }));
        const plain = formats.find(format => matchesText(format.mimeType));
        const fallback = plain || formats[0];
        let accepted = false;
        let attempted = false;
        if (request.selection === "clipboard") {
            if (typeof this.clipboard?.write === "function" && typeof this.ClipboardItem === "function") {
                attempted = true;
                try {
                    const representations = Object.fromEntries(formats.map(format => {
                        const mimeType = mimeEssence(format.mimeType);
                        return [mimeType, new Blob([format.data], { type: mimeType })];
                    }));
                    await this.clipboard.write([new this.ClipboardItem(representations)]);
                    accepted = true;
                } catch { /* The X11 plain representation may still succeed. */ }
            } else if (plain && typeof this.clipboard?.writeText === "function") {
                attempted = true;
                try { await this.clipboard.writeText(decoder.decode(plain.data)); accepted = true; } catch { /* Try X11. */ }
            }
        }
        if (this.selectionBridge) {
            attempted = true;
            try {
                await this.selectionBridge.write(request.selection, fallback.mimeType, fallback.data);
                accepted = true;
            } catch { /* Browser ownership can independently succeed. */ }
        }
        if (attempted && !accepted) return encodeResponse(request.request, { result: "rejected" });
        this.owned[request.selection] = ownedFormats(formats);
        if (request.selection === "clipboard") this.onChange?.({ mimeType: fallback.mimeType, data: fallback.data.slice() });
        return encodeResponse(request.request, { result: "data", mimeType: fallback.mimeType, data: new Uint8Array() });
    }

    async writeData(mimeType, bytes) {
        const data = new Uint8Array(bytes);
        if (data.length > MAX_CLIPBOARD_BYTES) throw new RangeError("Clipboard data is too large");
        this.owned.clipboard = { mimeType, data };
        let selectionAccepted = false;
        let selectionError = null;
        if (this.selectionBridge) {
            try {
                await this.selectionBridge.write("clipboard", mimeType, data);
                selectionAccepted = true;
            } catch (error) { selectionError = error; }
        }
        // PNG ownership must also reach the containing system clipboard:
        // the X11 selection alone does not cover Wayland or browser paste.
        if (mimeType === "image/png" && typeof this.clipboard?.write === "function"
            && typeof this.ClipboardItem === "function") {
            try {
                await this.clipboard.write([new this.ClipboardItem({
                    [mimeType]: new Blob([data], { type: mimeType }),
                })]);
                selectionError = null;
            } catch (error) {
                if (!selectionAccepted) throw selectionError || error;
            }
        }
        if (selectionError) throw selectionError;
        this.onChange?.({ mimeType, data: data.slice() });
        return true;
    }

    async #write(request) {
        if (request.selection === "clipboard") this.onChange?.({ mimeType: request.mimeType, data: new Uint8Array(request.data) });
        this.owned[request.selection] = {
            mimeType: request.mimeType,
            data: request.data.slice(),
        };
        let selectionAccepted = false;
        if (this.selectionBridge) {
            try {
                await this.selectionBridge.write(
                    request.selection,
                    request.mimeType,
                    request.data,
                );
                selectionAccepted = true;
            } catch {
                // Still attempt the containing browser clipboard when the X11
                // selection broker is temporarily unavailable.
            }
        }
        const needsSystemClipboard = request.selection === "clipboard" &&
            matchesText(request.mimeType) &&
            typeof this.clipboard?.writeText === "function";
        let systemAccepted = !needsSystemClipboard;
        if (needsSystemClipboard) {
            try {
                await this.clipboard.writeText(decoder.decode(request.data));
                systemAccepted = true;
            } catch {
                systemAccepted = false;
            }
        }
        if (selectionAccepted || systemAccepted) {
            return encodeResponse(request.request, {
                result: "data",
                mimeType: request.mimeType,
                data: new Uint8Array(),
            });
        }
        return encodeResponse(request.request, { result: "rejected" });
    }
}

export function decodeRequest(message) {
    const reader = decodeVfsFrame(message, REQUEST_KIND);
    const tag = reader.u8("clipboard request");
    const request = reader.id("clipboard request id");
    const selectionTag = reader.u8("clipboard selection");
    if (selectionTag > 1) throw new RangeError("clipboard selection is invalid");
    const selection = selectionTag === 0 ? "clipboard" : "primary";
    if (tag === 0) {
        const count = reader.u32("accepted MIME type count");
        if (count > 64) throw new RangeError("too many clipboard MIME types");
        const accepted = [];
        for (let index = 0; index < count; index += 1) {
            accepted.push(reader.text(MAX_TEXT_BYTES, "clipboard MIME type"));
        }
        reader.finish();
        return Object.freeze({ operation: "read", request, selection, accepted });
    }
    if (tag === 1) {
        const mimeType = reader.text(MAX_TEXT_BYTES, "clipboard MIME type");
        const data = reader.data(MAX_CLIPBOARD_BYTES, "clipboard data");
        reader.finish();
        return Object.freeze({
            operation: "write",
            request,
            selection,
            mimeType,
            data,
        });
    }
    if (tag === 2) {
        const count = reader.u32("clipboard format count");
        if (!count || count > 64) throw new RangeError("clipboard format count is invalid");
        const formats = [];
        let total = 0;
        for (let index = 0; index < count; index++) {
            const mimeType = reader.text(255, "clipboard MIME type");
            const data = reader.data(MAX_CLIPBOARD_BYTES - total, "clipboard aggregate data");
            total += data.length;
            formats.push({ mimeType, data });
        }
        reader.finish();
        validateFormats(formats);
        return Object.freeze({ operation: "writeFormats", request, selection, formats });
    }
    throw new RangeError("clipboard request operation is invalid");
}

function encodeResponse(request, response) {
    const writer = new VfsWireWriter();
    writer.id(request, "clipboard request id");
    if (response.result === "data") {
        writer.u8(0);
        writer.text(response.mimeType, MAX_TEXT_BYTES, "clipboard MIME type");
        writer.data(response.data, MAX_CLIPBOARD_BYTES, "clipboard data");
    } else {
        writer.u8(response.result === "empty" ? 1 : 2);
    }
    return encodeVfsFrame(RESPONSE_KIND, writer.bytes);
}

function matchesText(mimeType) {
    return mimeType === TEXT_UTF8 || mimeType === TEXT_PLAIN;
}

function mimeEssence(mimeType) { return String(mimeType).split(";", 1)[0].toLowerCase(); }

function validateFormats(formats) {
    const types = new Set();
    const mimePattern = /^[A-Za-z0-9!#$&^_.+-]+\/[A-Za-z0-9!#$&^_.+-]+(?:;[A-Za-z0-9!#$&^_.+-]+=[A-Za-z0-9!#$&^_.+-]+)*$/;
    if (!formats.length || formats.length > 64) throw new RangeError("clipboard format count is invalid");
    let total = 0;
    for (const format of formats) {
        if (format.mimeType.length > 255 || !mimePattern.test(format.mimeType)) throw new RangeError("clipboard MIME type is invalid");
        const essence = mimeEssence(format.mimeType);
        if (types.has(essence)) throw new RangeError("duplicate clipboard MIME type");
        types.add(essence);
        total += format.data.length;
        if (total > MAX_CLIPBOARD_BYTES) throw new RangeError("clipboard aggregate data is too large");
    }
}

function equalBytes(a, b) { return a.length === b.length && a.every((value, index) => value === b[index]); }

function ownedFormats(formats) {
    const preferred = formats.find(format => matchesText(format.mimeType)) || formats[0];
    return { ...preferred, formats };
}
