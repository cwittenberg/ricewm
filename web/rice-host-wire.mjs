import { validateRiceNativeAppFrame } from "./rice-native-app-wire.mjs";

const HEADER_BYTES = 12;
const REQUEST_KIND = 70;
const EVENT_KIND = 71;
const MINIMUM_TIMER_INTERVAL_MILLIS = 16;

function id(view, offset, field) {
    const value = view.getBigUint64(offset, true);
    if (value === 0n) throw new RangeError(`${field} is invalid`);
    return value;
}

function boolean(value, field) {
    if (value > 1) throw new RangeError(`${field} is invalid`);
    return value === 1;
}

function text(bytes, view, offset, maximum, field) {
    if (offset + 4 > bytes.byteLength) throw new RangeError(`${field} is truncated`);
    const length = view.getUint32(offset, true);
    if (length > maximum || offset + 4 + length > bytes.byteLength) {
        throw new RangeError(`${field} is invalid`);
    }
    const value = new TextDecoder("utf-8", { fatal: true }).decode(
        bytes.subarray(offset + 4, offset + 4 + length),
    );
    return { value, offset: offset + 4 + length };
}

function byteString(bytes, view, offset, maximum, field) {
    if (offset + 4 > bytes.byteLength) throw new RangeError(`${field} is truncated`);
    const length = view.getUint32(offset, true);
    if (length > maximum || offset + 4 + length > bytes.byteLength) {
        throw new RangeError(`${field} is invalid`);
    }
    return {
        value: bytes.slice(offset + 4, offset + 4 + length),
        offset: offset + 4 + length,
    };
}

/** Decode a bounded app-originated host window/timer request. */
export function decodeRiceHostRequest(message) {
    const bytes = validateRiceNativeAppFrame(message);
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (view.getUint16(6, true) !== REQUEST_KIND) {
        throw new TypeError("RAPP frame is not a host request");
    }
    const app = id(view, HEADER_BYTES, "host request app");
    const window = id(view, HEADER_BYTES + 8, "host request window");
    const tag = view.getUint8(HEADER_BYTES + 16);
    if (tag === 43 && bytes.byteLength === HEADER_BYTES + 34) {
        const token = id(view, HEADER_BYTES + 17, 'window size token'), width = view.getUint32(HEADER_BYTES + 25, true), height = view.getUint32(HEADER_BYTES + 29, true);
        if (![width, height].every(value => value >= 96 && value <= 4096)) throw RangeError('Invalid native window content size');
        const anchor = ['right','left','top','bottom'][view.getUint8(HEADER_BYTES + 33)];
        if (!anchor) throw RangeError('Invalid content resize anchor');
        return Object.freeze({app, window, operation:'content-size', token, width, height, anchor});
    }
    if (tag === 42 && bytes.byteLength === HEADER_BYTES + 26) {
        return Object.freeze({app,window,operation:"data-request-fence",
            token:id(view,HEADER_BYTES+17,"data request fence token"),
            resume:boolean(view.getUint8(HEADER_BYTES+25),"data request resume")});
    }
    if (tag === 41) {
        const payload = text(bytes, view, HEADER_BYTES + 17, 256 * 1024, "interactive notification");
        if (payload.offset !== bytes.byteLength) throw new RangeError("interactive notification has trailing data");
        return Object.freeze({app, window, operation:"interactive-notification", payload:payload.value});
    }
    if (tag === 0 && bytes.byteLength === HEADER_BYTES + 18) {
        return Object.freeze({
            app,
            window,
            operation: "fullscreen",
            enabled: boolean(view.getUint8(HEADER_BYTES + 17), "fullscreen"),
        });
    }
    if (tag === 1 && bytes.byteLength === HEADER_BYTES + 30) {
        const token = id(view, HEADER_BYTES + 17, "timer token");
        const intervalMillis = view.getUint32(HEADER_BYTES + 25, true);
        const enabled = boolean(
            view.getUint8(HEADER_BYTES + 29),
            "timer enabled",
        );
        if (
            intervalMillis < MINIMUM_TIMER_INTERVAL_MILLIS ||
            intervalMillis > 86_400_000
        ) {
            throw new RangeError("host timer interval is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "timer",
            token,
            intervalMillis,
            enabled,
        });
    }
    if (tag === 2) {
        const title = text(bytes, view, HEADER_BYTES + 17, 16 * 1024, "print title");
        const documentText = text(
            bytes,
            view,
            title.offset,
            512 * 1024,
            "print text",
        );
        if (documentText.offset !== bytes.byteLength) {
            throw new RangeError("host print request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "print-text",
            title: title.value,
            text: documentText.value,
        });
    }
    if (tag === 31) {
        const accepted = boolean(view.getUint8(HEADER_BYTES + 17), "dialog accepted");
        const path = accepted ? text(bytes, view, HEADER_BYTES + 18, 4096, "dialog path")
            : {value: null, offset: HEADER_BYTES + 18};
        if (path.offset !== bytes.byteLength) throw new RangeError("dialog result has trailing bytes");
        return Object.freeze({app, window, operation: "file-dialog-result", path: path.value});
    }
    if (tag === 37) {
        const selected = boolean(view.getUint8(HEADER_BYTES + 17), "menu selected");
        const item = selected ? text(bytes, view, HEADER_BYTES + 18, 256, "menu item ID")
            : {value: null, offset: HEADER_BYTES + 18};
        if (item.offset !== bytes.byteLength) throw new RangeError("menu result has trailing bytes");
        return Object.freeze({app, window, operation: "menu-result", id: item.value});
    }
    if (tag === 3 && bytes.byteLength === HEADER_BYTES + 17) {
        return Object.freeze({ app, window, operation: "request-close" });
    }
    if (tag === 4 && bytes.byteLength === HEADER_BYTES + 34) {
        const session = id(view, HEADER_BYTES + 17, "media session");
        const command = [
            "toggle-pause", "seek", "volume", "toggle-mute", "stop",
        ][view.getUint8(HEADER_BYTES + 25)];
        if (!command) throw new RangeError("media command is invalid");
        return Object.freeze({
            app,
            window,
            operation: "media-control",
            session,
            command,
            value: view.getBigInt64(HEADER_BYTES + 26, true),
        });
    }
    if (tag === 18 || tag === 38) {
        const replaceSession = view.getBigUint64(HEADER_BYTES + 17, true);
        const mediaPath = text(
            bytes, view, HEADER_BYTES + 25, 4 * 1024, "media path",
        );
        if (
            mediaPath.offset !== bytes.byteLength ||
            !mediaPath.value.startsWith("/") ||
            mediaPath.value.includes("\0") || mediaPath.value.includes("\n")
        ) throw new RangeError("media path is invalid");
        return Object.freeze({
            app,
            window,
            operation: "media-open",
            replaceSession,
            ...(tag === 38 ? { muted: true } : {}),
            path: mediaPath.value,
        });
    }
    if (tag === 32) {
        const request = id(view, HEADER_BYTES + 17, "protected media request");
        const handle = view.getBigUint64(HEADER_BYTES + 25, true);
        const action = text(
            bytes, view, HEADER_BYTES + 33, 16 * 1024, "protected media action",
        );
        const metadata = text(
            bytes, view, action.offset, 128 * 1024, "protected media metadata",
        );
        const data = byteString(
            bytes, view, metadata.offset, 64 * 1024 * 1024, "protected media data",
        );
        if (!action.value || data.offset !== bytes.byteLength) {
            throw new RangeError("protected media request is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "protected-media",
            request,
            handle,
            action: action.value,
            metadata: metadata.value,
            data: data.value,
        });
    }
    if (tag === 33) {
        const tab = id(view, HEADER_BYTES + 17, "geolocation tab");
        const request = id(view, HEADER_BYTES + 25, "geolocation request");
        const watch = boolean(
            view.getUint8(HEADER_BYTES + 33),
            "geolocation watch",
        );
        const options = text(
            bytes, view, HEADER_BYTES + 34, 1024, "geolocation options",
        );
        if (options.offset !== bytes.byteLength) {
            throw new RangeError("geolocation request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "geolocation",
            tab,
            request,
            watch,
            options: options.value,
        });
    }
    if (tag === 34 && bytes.byteLength === HEADER_BYTES + 33) {
        return Object.freeze({
            app,
            window,
            operation: "clear-geolocation",
            tab: id(view, HEADER_BYTES + 17, "geolocation tab"),
            request: id(view, HEADER_BYTES + 25, "geolocation request"),
        });
    }
    if (tag === 35) {
        const tab = id(view, HEADER_BYTES + 17, "WebSocket tab");
        const realm = id(view, HEADER_BYTES + 25, "WebSocket realm");
        const handle = id(view, HEADER_BYTES + 33, "WebSocket handle");
        const action = view.getUint8(HEADER_BYTES + 41);
        const metadata = text(
            bytes, view, HEADER_BYTES + 42, 32 * 1024, "WebSocket metadata",
        );
        const data = byteString(
            bytes, view, metadata.offset, 1024 * 1024, "WebSocket data",
        );
        if (action < 1 || action > 3 || data.offset !== bytes.byteLength) {
            throw new RangeError("WebSocket request is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "websocket",
            tab,
            realm,
            handle,
            action,
            metadata: metadata.value,
            data: data.value,
        });
    }
    if (tag === 36) {
        const request = id(view, HEADER_BYTES + 17, "JavaScript realm request");
        const tab = id(view, HEADER_BYTES + 25, "JavaScript realm tab");
        const realm = id(view, HEADER_BYTES + 33, "JavaScript realm id");
        const action = text(
            bytes, view, HEADER_BYTES + 41, 16, "JavaScript realm action",
        );
        const url = text(
            bytes, view, action.offset, 16 * 1024, "JavaScript realm URL",
        );
        const source = byteString(
            bytes, view, url.offset, 32 * 1024 * 1024, "JavaScript realm source",
        );
        if (!action.value || source.offset !== bytes.byteLength) {
            throw new RangeError("JavaScript realm request is invalid");
        }
        return Object.freeze({
            app, window, operation: "javascript-realm", request, tab, realm,
            action: action.value, url: url.value,
            source: new TextDecoder("utf-8", { fatal: true }).decode(source.value),
        });
    }
    if (tag === 5 && bytes.byteLength === HEADER_BYTES + 17) {
        return Object.freeze({ app, window, operation: "performance-snapshot" });
    }
    if (tag === 13 && bytes.byteLength === HEADER_BYTES + 17) {
        return Object.freeze({ app, window, operation: "acceleration-diagnostics" });
    }
    if (tag === 6 && bytes.byteLength === HEADER_BYTES + 22) {
        const pid = view.getUint32(HEADER_BYTES + 17, true);
        const signal = view.getUint8(HEADER_BYTES + 21);
        if (pid === 0 || signal === 0 || signal > 64) {
            throw new RangeError("process signal is invalid");
        }
        return Object.freeze({ app, window, operation: "process-signal", pid, signal });
    }
    if (tag === 7 && bytes.byteLength === HEADER_BYTES + 22) {
        const pid = view.getUint32(HEADER_BYTES + 17, true);
        const nice = view.getInt8(HEADER_BYTES + 21);
        if (pid === 0 || nice < -20 || nice > 19) {
            throw new RangeError("process niceness is invalid");
        }
        return Object.freeze({ app, window, operation: "process-nice", pid, nice });
    }
    if (tag === 8) {
        const payload = text(
            bytes, view, HEADER_BYTES + 17, 512 * 1024, "app store request",
        );
        if (payload.offset !== bytes.byteLength) {
            throw new RangeError("app store request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "app-store", payload: payload.value,
        });
    }
    if (tag === 9) {
        const filename = text(
            bytes, view, HEADER_BYTES + 17, 16 * 1024, "download filename",
        );
        const mimeType = text(
            bytes, view, filename.offset, 16 * 1024, "download MIME type",
        );
        const documentText = text(
            bytes, view, mimeType.offset, 64 * 1024 * 1024, "download text",
        );
        if (documentText.offset !== bytes.byteLength) {
            throw new RangeError("host download request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "download-text",
            filename: filename.value,
            mimeType: mimeType.value,
            text: documentText.value,
        });
    }
    if (tag === 19) {
        const filename = text(bytes, view, HEADER_BYTES + 17, 16 * 1024, "download filename");
        const mimeType = text(bytes, view, filename.offset, 16 * 1024, "download MIME type");
        const data = byteString(bytes, view, mimeType.offset, 64 * 1024 * 1024, "download data");
        if (data.offset !== bytes.byteLength) {
            throw new RangeError("host file download request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "download-file",
            filename: filename.value,
            mimeType: mimeType.value,
            data: data.value,
        });
    }
    if (tag === 20) {
        const path = text(bytes, view, HEADER_BYTES + 17, 4 * 1024, "download path");
        const filename = text(bytes, view, path.offset, 16 * 1024, "download filename");
        if (filename.offset >= bytes.byteLength) {
            throw new RangeError("host path download request is truncated");
        }
        const showStorageTip = boolean(view.getUint8(filename.offset), "storage tip");
        if (filename.offset + 1 !== bytes.byteLength) {
            throw new RangeError("host path download request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "download-path",
            path: path.value,
            filename: filename.value,
            showStorageTip,
        });
    }
    if (tag === 21) {
        const filename = text(bytes, view, HEADER_BYTES + 17, 16 * 1024, "download filename");
        const mimeType = text(bytes, view, filename.offset, 16 * 1024, "download MIME type");
        if (mimeType.offset >= bytes.byteLength) {
            throw new RangeError("host reveal-download request is truncated");
        }
        const openFile = boolean(view.getUint8(mimeType.offset), "open download");
        if (mimeType.offset + 1 !== bytes.byteLength) {
            throw new RangeError("host reveal-download request contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "reveal-download",
            filename: filename.value,
            mimeType: mimeType.value,
            openFile,
        });
    }
    if (tag === 24) {
        const mimeType = text(bytes, view, HEADER_BYTES + 17, 16 * 1024, "clipboard image MIME type");
        const data = byteString(bytes, view, mimeType.offset, 64 * 1024 * 1024, "clipboard image data");
        if (data.offset !== bytes.byteLength || !mimeType.value.startsWith("image/")) {
            throw new RangeError("host clipboard image request is invalid");
        }
        return Object.freeze({
            app, window, operation: "copy-image", mimeType: mimeType.value, data: data.value,
        });
    }
    if (tag === 25) {
        const filename = text(bytes, view, HEADER_BYTES + 17, 16 * 1024, "open filename");
        const mimeType = text(bytes, view, filename.offset, 16 * 1024, "open MIME type");
        const data = byteString(bytes, view, mimeType.offset, 64 * 1024 * 1024, "open file data");
        if (data.offset !== bytes.byteLength) {
            throw new RangeError("host save-and-open request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "save-and-open-file", filename: filename.value,
            mimeType: mimeType.value, data: data.value,
        });
    }
    if (tag === 10) {
        const title = text(
            bytes, view, HEADER_BYTES + 17, 16 * 1024, "window title",
        );
        if (title.offset !== bytes.byteLength || !title.value.trim()) {
            throw new RangeError("host window title is invalid");
        }
        return Object.freeze({
            app, window, operation: "set-title", title: title.value,
        });
    }
    if (tag === 26 && bytes.byteLength === HEADER_BYTES + 18) {
        return Object.freeze({
            app,
            window,
            operation: "set-titlebar-visible",
            visible: boolean(view.getUint8(HEADER_BYTES + 17), "titlebar visible"),
        });
    }
    if (tag === 27 && bytes.byteLength === HEADER_BYTES + 17) {
        return Object.freeze({ app, window, operation: "minimize-window" });
    }
    if (tag === 28 && bytes.byteLength === HEADER_BYTES + 17) {
        return Object.freeze({ app, window, operation: "toggle-maximize-window" });
    }
    if (tag === 29 && bytes.byteLength === HEADER_BYTES + 25) {
        return Object.freeze({
            app,
            window,
            operation: "start-window-drag",
            x: view.getInt32(HEADER_BYTES + 17, true),
            y: view.getInt32(HEADER_BYTES + 21, true),
        });
    }
    if (tag === 30) {
        const topic = text(
            bytes, view, HEADER_BYTES + 17, 16 * 1024, "RiceBus topic",
        );
        if (!topic.value || topic.offset + 4 > bytes.byteLength) {
            throw new RangeError("RiceBus subscription is truncated");
        }
        const count = view.getUint32(topic.offset, true);
        if (count > 8) {
            throw new RangeError("RiceBus subscription has too many filters");
        }
        let offset = topic.offset + 4;
        const filters = [];
        for (let index = 0; index < count; index++) {
            const filter = text(
                bytes, view, offset, 4 * 1024, "RiceBus subscription filter",
            );
            filters.push(filter.value);
            offset = filter.offset;
        }
        if (offset !== bytes.byteLength) {
            throw new RangeError("RiceBus subscription contains trailing bytes");
        }
        return Object.freeze({
            app,
            window,
            operation: "rice-bus-subscribe",
            topic: topic.value,
            filters: Object.freeze(filters),
        });
    }
    if (tag === 14) {
        const kind = text(bytes, view, HEADER_BYTES + 17, 32, "notification kind");
        const title = text(bytes, view, kind.offset, 256, "notification title");
        const messageText = text(bytes, view, title.offset, 1024, "notification message");
        if (
            messageText.offset !== bytes.byteLength ||
            !kind.value || !title.value.trim()
        ) {
            throw new RangeError("host notification is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "show-notification",
            kind: kind.value,
            title: title.value,
            message: messageText.value,
        });
    }
    if (tag === 17) {
        const url = text(bytes, view, HEADER_BYTES + 17, 2048, "external URL");
        if (
            url.offset !== bytes.byteLength ||
            !url.value.startsWith("https://")
        ) {
            throw new RangeError("host external URL is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "open-external-url",
            url: url.value,
        });
    }
    if (tag === 40) {
        const payload = text(bytes, view, HEADER_BYTES + 17, 512 * 1024, "Paddy WhatsApp command");
        if (payload.offset !== bytes.byteLength) throw new RangeError("Paddy WhatsApp trailing bytes");
        return Object.freeze({app, window, operation: "paddy-whatsapp", payload: payload.value});
    }
    if (tag === 11) {
        const payload = text(
            bytes, view, HEADER_BYTES + 17, 512 * 1024, "extension store request",
        );
        if (payload.offset !== bytes.byteLength) {
            throw new RangeError("extension store request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "extension-store", payload: payload.value,
        });
    }
    if (tag === 15) {
        const payload = text(
            bytes, view, HEADER_BYTES + 17, 512 * 1024, "calendar store request",
        );
        if (payload.offset !== bytes.byteLength) {
            throw new RangeError("calendar store request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "calendar-store", payload: payload.value,
        });
    }
    if (tag === 22) {
        const payload = text(
            bytes, view, HEADER_BYTES + 17, 512 * 1024, "mail store request",
        );
        if (payload.offset !== bytes.byteLength) {
            throw new RangeError("mail store request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "mail-store", payload: payload.value,
        });
    }
    if (tag === 23) {
        const payload = text(
            bytes, view, HEADER_BYTES + 17, 512 * 1024, "contact store request",
        );
        if (payload.offset !== bytes.byteLength) {
            throw new RangeError("contact store request contains trailing bytes");
        }
        return Object.freeze({
            app, window, operation: "contact-store", payload: payload.value,
        });
    }
    if (tag === 16 || tag === 39) {
        const session = id(view, HEADER_BYTES + 17, "image export session");
        const actionTag = view.getUint8(HEADER_BYTES + 25);
        const width = view.getUint16(HEADER_BYTES + 26, true);
        const height = view.getUint16(HEADER_BYTES + 28, true);
        const quarterTurns = view.getUint8(HEADER_BYTES + 30);
        const preserveOriginal = boolean(
            view.getUint8(HEADER_BYTES + 31),
            "preserve original image",
        );
        if (
            actionTag > 2 || width < 1 || width > 4096 ||
            height < 1 || height > 4096 || quarterTurns > 3
        ) {
            throw new RangeError("image export request is invalid");
        }
        const filename = text(
            bytes, view, HEADER_BYTES + 32, 4 * 1024, "image export filename",
        );
        const crop = tag === 39 && filename.offset + 8 === bytes.byteLength
            ? Array.from({length: 4}, (_, i) => view.getUint16(filename.offset + i * 2, true)) : null;
        if (tag === 39 && (!crop || !crop[2] || !crop[3] || crop[0] + crop[2] > 4096 || crop[1] + crop[3] > 4096 || preserveOriginal)) {
            throw new RangeError("image crop is invalid");
        }
        if (filename.offset + (crop ? 8 : 0) !== bytes.byteLength || !filename.value.trim()) {
            throw new RangeError("image export filename is invalid");
        }
        return Object.freeze({
            app,
            window,
            operation: "image-export",
            session,
            action: ["copy", "save-as", "save-tenant"][actionTag],
            width,
            height,
            quarterTurns,
            preserveOriginal,
            ...(crop ? {crop} : {}),
            filename: filename.value,
        });
    }
    if (tag === 12) {
        const session = id(view, HEADER_BYTES + 17, "terminal session");
        const action = view.getUint8(HEADER_BYTES + 25);
        if (action === 11 && bytes.byteLength === HEADER_BYTES + 32) {
            const columns = view.getUint16(HEADER_BYTES + 26, true);
            const rows = view.getUint16(HEADER_BYTES + 28, true);
            const memoryMiB = view.getUint16(HEADER_BYTES + 30, true);
            if (columns < 2 || columns > 512 || rows < 1 || rows > 256 ||
                memoryMiB < 384 || memoryMiB > 2048) {
                throw new RangeError("terminal size or memory is invalid");
            }
            return Object.freeze({app, window, operation: "terminal", session,
                action: "open-grid", columns, rows, memoryMiB});
        }
        if ((action === 0 || action === 2 || action === 6 || action === 12) && bytes.byteLength === HEADER_BYTES + 30) {
            const columns = view.getUint16(HEADER_BYTES + 26, true);
            const rows = view.getUint16(HEADER_BYTES + 28, true);
            if (columns < 2 || columns > 512 || rows < 1 || rows > 256) {
                throw new RangeError("terminal size is invalid");
            }
            return Object.freeze({
                app,
                window,
                operation: "terminal",
                session,
                action: action === 0 ? "open" : action === 6 ? "open-grid" : action === 12 ? "open-xcodex" : "resize",
                columns,
                rows,
            });
        }
        if (action === 9 && bytes.byteLength === HEADER_BYTES + 26) {
            return Object.freeze({app, window, operation: "terminal", session, action: "copy-all"});
        }
        if (action === 10) {
            const data = byteString(bytes, view, HEADER_BYTES + 26, 16384, "terminal assistant");
            if (data.offset !== bytes.byteLength) throw new RangeError("terminal assistant trailing bytes");
            return Object.freeze({app, window, operation: "terminal", session, action: "assistant", payload: new TextDecoder('utf-8',{fatal:true}).decode(data.value)});
        }
        if (action === 8) {
            const data = byteString(bytes, view, HEADER_BYTES + 26, 1024, "terminal appearance");
            if (data.offset !== bytes.byteLength) throw new RangeError("terminal appearance trailing bytes");
            return Object.freeze({app, window, operation: "terminal", session, action: "appearance", payload: new TextDecoder().decode(data.value)});
        }
        if (action === 7 && bytes.byteLength === HEADER_BYTES + 30) {
            return Object.freeze({ app, window, operation: "terminal", session,
                action: "scroll", lines: view.getInt32(HEADER_BYTES + 26, true) });
        }
        if (action === 1) {
            const input = byteString(
                bytes, view, HEADER_BYTES + 26, 512 * 1024, "terminal input",
            );
            if (input.offset !== bytes.byteLength) {
                throw new RangeError("terminal input contains trailing bytes");
            }
            return Object.freeze({
                app,
                window,
                operation: "terminal",
                session,
                action: "input",
                bytes: input.value,
            });
        }
        if (action === 3 && bytes.byteLength === HEADER_BYTES + 26) {
            return Object.freeze({
                app, window, operation: "terminal", session, action: "close",
            });
        }
        if (action === 4 && bytes.byteLength === HEADER_BYTES + 28) {
            const copyOnSelect = view.getUint8(HEADER_BYTES + 26);
            const transparency = view.getUint8(HEADER_BYTES + 27);
            if (copyOnSelect > 1 || transparency > 80) {
                throw new RangeError("terminal preferences are invalid");
            }
            return Object.freeze({
                app,
                window,
                operation: "terminal",
                session,
                action: "configure",
                copyOnSelect: copyOnSelect === 1,
                transparency,
            });
        }
        if (action === 5 && bytes.byteLength === HEADER_BYTES + 26) {
            return Object.freeze({
                app,
                window,
                operation: "terminal",
                session,
                action: "super-fullscreen",
            });
        }
        throw new RangeError("terminal request payload is invalid");
    }
    throw new RangeError("host request payload is invalid");
}

/** Encode a host-originated timer event for one native app actor. */
export function encodeRiceTimerEvent(app, token) {
    const appId = BigInt(app);
    const timerToken = BigInt(token);
    if (appId <= 0n || timerToken <= 0n) {
        throw new RangeError("timer event identifiers are invalid");
    }
    const bytes = new Uint8Array(HEADER_BYTES + 17);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, 17, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 0);
    view.setBigUint64(HEADER_BYTES + 9, timerToken, true);
    return bytes;
}

/** Encode one bounded RiceBus event for a subscribed native app actor. */
export function encodeRiceBusEvent(app, topic, payload) {
    const appId = checkedEventId(app, "RiceBus event app");
    const encodedTopic = new TextEncoder().encode(String(topic));
    const encodedPayload = new TextEncoder().encode(String(payload));
    if (encodedTopic.byteLength < 1 || encodedTopic.byteLength > 16 * 1024) {
        throw new RangeError("RiceBus event topic is invalid");
    }
    if (encodedPayload.byteLength > 512 * 1024) {
        throw new RangeError("RiceBus event payload is oversized");
    }
    const bodyBytes = 8 + 1 + 4 + encodedTopic.byteLength +
        4 + encodedPayload.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyBytes);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyBytes, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 12);
    let offset = HEADER_BYTES + 9;
    view.setUint32(offset, encodedTopic.byteLength, true);
    offset += 4;
    bytes.set(encodedTopic, offset);
    offset += encodedTopic.byteLength;
    view.setUint32(offset, encodedPayload.byteLength, true);
    bytes.set(encodedPayload, offset + 4);
    return bytes;
}

/** Encode one bounded, host-authored system-monitor snapshot. */
export function encodeRicePerformanceSnapshotEvent(app, payload) {
    const appId = checkedEventId(app, "performance snapshot app");
    const encoded = new TextEncoder().encode(String(payload));
    if (encoded.byteLength > 512 * 1024) {
        throw new RangeError("performance snapshot is oversized");
    }
    const bytes = new Uint8Array(HEADER_BYTES + 13 + encoded.byteLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, 13 + encoded.byteLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 2);
    view.setUint32(HEADER_BYTES + 9, encoded.byteLength, true);
    bytes.set(encoded, HEADER_BYTES + 13);
    return bytes;
}

/** Encode one bounded, host-measured acceleration report. */
export function encodeRiceAccelerationDiagnosticsEvent(app, payload) {
    return encodeRiceStoreEvent(
        app,
        payload,
        6,
        "acceleration diagnostics event",
    );
}

/** Encode one bounded host-authored Apt App Store update. */
export function encodeRiceAppStoreEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 3, "app store event");
}

function encodeRiceStoreEvent(app, payload, tag, field) {
    const appId = checkedEventId(app, `${field} app`);
    const encoded = new TextEncoder().encode(String(payload));
    if (encoded.byteLength > 512 * 1024) {
        throw new RangeError(`${field} is oversized`);
    }
    const bytes = new Uint8Array(HEADER_BYTES + 13 + encoded.byteLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, 13 + encoded.byteLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, tag);
    view.setUint32(HEADER_BYTES + 9, encoded.byteLength, true);
    bytes.set(encoded, HEADER_BYTES + 13);
    return bytes;
}

/** Encode one bounded host-authored GNOME extension service update. */
export function encodeRiceExtensionStoreEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 4, "extension store event");
}

/** Encode one bounded shell calendar-server update. */
export function encodeRiceCalendarStoreEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 7, "calendar store event");
}

/** Encode one bounded local mail-service update. */
export function encodeRiceMailStoreEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 10, "mail store event");
}

/** Encode one bounded encrypted contact-service update. */
export function encodeRiceContactStoreEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 11, "contact store event");
}


/** Encode screen or lifecycle state from one headless native terminal TTY. */
export function encodeRiceTerminalEvent(app, session, event) {
    const appId = checkedEventId(app, "terminal event app");
    const sessionId = checkedEventId(session, "terminal event session");
    const kind = String(event?.kind || "");
    let payload;
    let eventTag;
    if (kind === "grid" || kind === "paste") {
        payload = new TextEncoder().encode(String((kind === "paste" ? event.text : event.payload) || ""));
        if (payload.length > (kind === "grid" ? 16 * 1024 * 1024 : 512 * 1024)) throw new RangeError("terminal grid or paste is oversized");
        eventTag = kind === "paste" ? 6 : 4;
    } else if (kind === "screen") {
        const columns = checkedDimension(event.columns, "terminal columns");
        const rows = checkedDimension(event.rows, "terminal rows");
        if (columns < 2 || columns > 512 || rows > 256) {
            throw new RangeError("terminal screen size is invalid");
        }
        const screenText = String(event.text || "");
        payload = new TextEncoder().encode(screenText);
        const caret = Number(event.caret);
        if (
            payload.byteLength > 512 * 1024 ||
            !Number.isInteger(caret) ||
            caret < 0 ||
            caret > Array.from(screenText).length
        ) {
            throw new RangeError("terminal screen is oversized or invalid");
        }
        eventTag = 0;
        event = {
            ...event,
            columns,
            rows,
            caret,
        };
    } else if (kind === "grid-size") {
        const columns = checkedDimension(event.columns, "terminal columns");
        const rows = checkedDimension(event.rows, "terminal rows");
        if (columns < 2 || columns > 512 || rows > 256) throw new RangeError("invalid grid size");
        payload = new Uint8Array(4);
        const size = new DataView(payload.buffer);
        size.setUint16(0, columns, true); size.setUint16(2, rows, true);
        eventTag = 5;
    } else if (kind === "ready") {
        payload = new Uint8Array();
        eventTag = 1;
    } else if (kind === "exited") {
        payload = new Uint8Array(4);
        new DataView(payload.buffer).setInt32(0, Number(event.code) || 0, true);
        eventTag = 2;
    } else if (kind === "appearance") {
        payload = new TextEncoder().encode(String(event.payload));
        if (payload.length > 1024) throw new RangeError("terminal appearance size");
        eventTag = 7;
    } else if (kind === "preferences") {
        const transparency = Number(event.transparency);
        if (!Number.isInteger(transparency) || transparency < 0 || transparency > 80) {
            throw new RangeError("terminal preferences are invalid");
        }
        payload = Uint8Array.of(
            Number(Boolean(event.copyOnSelect)),
            transparency,
        );
        eventTag = 3;
    } else {
        throw new RangeError("terminal event kind is invalid");
    }
    const variable = eventTag === 0 ? 13 : (eventTag === 4 || eventTag === 6 || eventTag === 7) ? 4 : 0;
    const bodyLength = 18 + variable + payload.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 5);
    view.setBigUint64(HEADER_BYTES + 9, sessionId, true);
    view.setUint8(HEADER_BYTES + 17, eventTag);
    if (eventTag === 0) {
        view.setUint16(HEADER_BYTES + 18, event.columns, true);
        view.setUint16(HEADER_BYTES + 20, event.rows, true);
        view.setUint32(HEADER_BYTES + 22, event.caret, true);
        view.setUint8(
            HEADER_BYTES + 26,
            Number(Boolean(event.applicationCursor)) |
                (Number(Boolean(event.applicationKeypad)) << 1) |
                (Number(Boolean(event.bracketedPaste)) << 2),
        );
        view.setUint32(HEADER_BYTES + 27, payload.byteLength, true);
        bytes.set(payload, HEADER_BYTES + 31);
    } else if (eventTag === 4 || eventTag === 6 || eventTag === 7) {
        view.setUint32(HEADER_BYTES + 18, payload.length, true);
        bytes.set(payload, HEADER_BYTES + 22);
    } else if (eventTag === 2 || eventTag === 3 || eventTag === 5) {
        bytes.set(payload, HEADER_BYTES + 18);
    }
    return bytes;
}

/** Encode authoritative host geometry and visibility for one native window. */
export function encodeRiceWindowStateEvent(app, window, state) {
    const appId = checkedEventId(app, "window state app");
    const windowId = checkedEventId(window, "window state window");
    const width = checkedDimension(state?.contentWidth, "window content width");
    const height = checkedDimension(state?.contentHeight, "window content height");
    const bytes = new Uint8Array(HEADER_BYTES + 29);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, 29, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 1);
    view.setBigUint64(HEADER_BYTES + 9, windowId, true);
    view.setUint32(HEADER_BYTES + 17, width, true);
    view.setUint32(HEADER_BYTES + 21, height, true);
    view.setUint8(HEADER_BYTES + 25, state.fullscreen ? 1 : 0);
    view.setUint8(HEADER_BYTES + 26, state.maximized ? 1 : 0);
    view.setUint8(HEADER_BYTES + 27, state.focused ? 1 : 0);
    view.setUint8(HEADER_BYTES + 28, state.visible ? 1 : 0);
    return bytes;
}

/** Encode completion of one asynchronous software-media replacement. */
export function encodeRiceMediaOpenedEvent(app, result) {
    const appId = checkedEventId(app, "media open app");
    const session = BigInt(result?.session || 0);
    const durationMillis = BigInt(result?.durationMillis || 0);
    const path = new TextEncoder().encode(String(result?.path || ""));
    const error = new TextEncoder().encode(String(result?.error || ""));
    if (
        path.byteLength > 4 * 1024 || error.byteLength > 16 * 1024 ||
        ((session === 0n) === (error.byteLength === 0))
    ) throw new RangeError("media open result is invalid");
    const bodyLength = 33 + path.byteLength + error.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 8);
    view.setBigUint64(HEADER_BYTES + 9, session, true);
    view.setBigUint64(HEADER_BYTES + 17, durationMillis, true);
    view.setUint32(HEADER_BYTES + 25, path.byteLength, true);
    bytes.set(path, HEADER_BYTES + 29);
    const errorOffset = HEADER_BYTES + 29 + path.byteLength;
    view.setUint32(errorOffset, error.byteLength, true);
    bytes.set(error, errorOffset + 4);
    return bytes;
}

/** Encode a decoder-authored end-of-stream event for its owning player. */
export function encodeRiceMediaEndedEvent(app, session) {
    const appId = checkedEventId(app, "media ended app");
    const sessionId = checkedEventId(session, "media ended session");
    const bytes = new Uint8Array(HEADER_BYTES + 17);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, 17, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 9);
    view.setBigUint64(HEADER_BYTES + 9, sessionId, true);
    return bytes;
}

/** Encode one real EME/MSE completion or asynchronous protected-media event. */
export function encodeRiceProtectedMediaEvent(app, result) {
    const appId = checkedEventId(app, "protected media app");
    const request = BigInt(result?.request || 0);
    const handle = BigInt(result?.handle || 0);
    if (
        request < 0n || request > 0xffff_ffff_ffff_ffffn ||
        handle < 0n || handle > 0xffff_ffff_ffff_ffffn
    ) throw new RangeError("protected media identifiers are invalid");
    const encoder = new TextEncoder();
    const event = encoder.encode(String(result?.event || ""));
    const metadata = encoder.encode(String(result?.metadata || ""));
    const source = result?.data ?? new Uint8Array();
    const data = source instanceof Uint8Array
        ? source
        : ArrayBuffer.isView(source)
            ? new Uint8Array(source.buffer, source.byteOffset, source.byteLength)
            : source instanceof ArrayBuffer
                ? new Uint8Array(source)
                : Uint8Array.from(source);
    if (
        event.byteLength === 0 || event.byteLength > 16 * 1024 ||
        metadata.byteLength > 128 * 1024 || data.byteLength > 64 * 1024 * 1024
    ) throw new RangeError("protected media event is invalid");
    const bodyLength = 38 + event.byteLength + metadata.byteLength + data.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 13);
    view.setBigUint64(HEADER_BYTES + 9, request, true);
    view.setUint8(HEADER_BYTES + 17, result?.ok ? 1 : 0);
    view.setBigUint64(HEADER_BYTES + 18, handle, true);
    let offset = HEADER_BYTES + 26;
    for (const value of [event, metadata, data]) {
        view.setUint32(offset, value.byteLength, true);
        offset += 4;
        bytes.set(value, offset);
        offset += value.byteLength;
    }
    return bytes;
}

/** Encode one guest-network WebSocket completion or asynchronous event. */
export function encodeRiceWebSocketEvent(app, result) {
    const appId = checkedEventId(app, "WebSocket app");
    const tab = checkedEventId(result?.tab, "WebSocket tab");
    const realm = checkedEventId(result?.realm, "WebSocket realm");
    const handle = checkedEventId(result?.handle, "WebSocket handle");
    const encoder = new TextEncoder();
    const event = encoder.encode(String(result?.event || ""));
    const metadata = encoder.encode(String(result?.metadata || ""));
    const source = result?.data ?? new Uint8Array();
    const data = source instanceof Uint8Array
        ? source
        : ArrayBuffer.isView(source)
            ? new Uint8Array(source.buffer, source.byteOffset, source.byteLength)
            : source instanceof ArrayBuffer
                ? new Uint8Array(source)
                : Uint8Array.from(source);
    if (
        event.byteLength === 0 || event.byteLength > 16 ||
        metadata.byteLength > 32 * 1024 || data.byteLength > 1024 * 1024
    ) throw new RangeError("WebSocket event is invalid");
    const bodyLength = 45 + event.byteLength + metadata.byteLength + data.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 15);
    view.setBigUint64(HEADER_BYTES + 9, tab, true);
    view.setBigUint64(HEADER_BYTES + 17, realm, true);
    view.setBigUint64(HEADER_BYTES + 25, handle, true);
    let offset = HEADER_BYTES + 33;
    for (const value of [event, metadata, data]) {
        view.setUint32(offset, value.byteLength, true);
        offset += 4;
        bytes.set(value, offset);
        offset += value.byteLength;
    }
    return bytes;
}

/** Encode a host-browser Geolocation API result for one Bowl page realm. */
export function encodeRiceGeolocationEvent(app, tab, request, payload) {
    const appId = checkedEventId(app, "geolocation event app");
    const tabId = checkedEventId(tab, "geolocation event tab");
    const requestId = checkedEventId(request, "geolocation event request");
    const encoded = new TextEncoder().encode(String(payload));
    if (encoded.byteLength > 4096) {
        throw new RangeError("geolocation event is oversized");
    }
    const bodyLength = 29 + encoded.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 14);
    view.setBigUint64(HEADER_BYTES + 9, tabId, true);
    view.setBigUint64(HEADER_BYTES + 17, requestId, true);
    view.setUint32(HEADER_BYTES + 25, encoded.byteLength, true);
    bytes.set(encoded, HEADER_BYTES + 29);
    return bytes;
}

/** Encode a result or DOM snapshot from one host-native Bowl JS realm. */
export function encodeRiceJavaScriptRealmEvent(app, result) {
    const appId = checkedEventId(app, "JavaScript realm app");
    const request = checkedEventId(result?.request, "JavaScript realm request");
    const tab = checkedEventId(result?.tab, "JavaScript realm tab");
    const realm = checkedEventId(result?.realm, "JavaScript realm id");
    const encoder = new TextEncoder();
    // A shell from the immediately preceding native-cookie revision returned
    // an acknowledgement label longer than this ABI's 16-byte event field.
    // Normalize that one transport-only acknowledgement while mixed release
    // generations drain; page-originated event names remain strict.
    const eventName = String(result?.event || "") === "network-cookie-acknowledged"
        ? "cookie"
        : String(result?.event || "");
    const event = encoder.encode(eventName);
    const metadata = encoder.encode(String(result?.metadata || ""));
    const source = encoder.encode(String(result?.source || ""));
    if (
        event.byteLength === 0 || event.byteLength > 16 ||
        metadata.byteLength > 128 * 1024 || source.byteLength > 32 * 1024 * 1024
    ) throw new RangeError(
        `JavaScript realm event is invalid ` +
        `(event=${JSON.stringify(eventName)} eventBytes=${event.byteLength} ` +
        `metadataBytes=${metadata.byteLength} sourceBytes=${source.byteLength})`,
    );
    const bodyLength = 45 + event.byteLength + metadata.byteLength + source.byteLength;
    const bytes = new Uint8Array(HEADER_BYTES + bodyLength);
    bytes.set([0x52, 0x41, 0x50, 0x50], 0);
    const view = new DataView(bytes.buffer);
    view.setUint16(4, 1, true);
    view.setUint16(6, EVENT_KIND, true);
    view.setUint32(8, bodyLength, true);
    view.setBigUint64(HEADER_BYTES, appId, true);
    view.setUint8(HEADER_BYTES + 8, 16);
    view.setBigUint64(HEADER_BYTES + 9, request, true);
    view.setBigUint64(HEADER_BYTES + 17, tab, true);
    view.setBigUint64(HEADER_BYTES + 25, realm, true);
    let offset = HEADER_BYTES + 33;
    for (const value of [event, metadata, source]) {
        view.setUint32(offset, value.byteLength, true);
        offset += 4;
        bytes.set(value, offset);
        offset += value.byteLength;
    }
    return bytes;
}

function checkedEventId(value, field) {
    const id = BigInt(value);
    if (id <= 0n || id > 0xffff_ffff_ffff_ffffn) {
        throw new RangeError(`${field} is invalid`);
    }
    return id;
}

function checkedDimension(value, field) {
    const dimension = Number(value);
    if (!Number.isInteger(dimension) || dimension < 1 || dimension > 65_535) {
        throw new RangeError(`${field} is invalid`);
    }
    return dimension;
}

/** Presentation events only: never log pairing codes or message payloads. */
export function encodeRicePaddyWhatsAppEvent(app, payload) {
    return encodeRiceStoreEvent(app, payload, 17, "Paddy WhatsApp event");
}

export function encodeRiceDataRequestFenceEvent(app, token, resume, ok) {
    const appId=checkedEventId(app,"data request fence app");
    const fence=checkedEventId(token,"data request fence token");
    if (typeof resume!=="boolean" || typeof ok!=="boolean") throw TypeError("Invalid data request fence result");
    const bytes=new Uint8Array(HEADER_BYTES+19),view=new DataView(bytes.buffer);
    bytes.set([0x52,0x41,0x50,0x50]);view.setUint16(4,1,true);view.setUint16(6,71,true);view.setUint32(8,19,true);
    view.setBigUint64(12,appId,true);view.setUint8(20,18);view.setBigUint64(21,fence,true);
    view.setUint8(29,Number(resume));view.setUint8(30,Number(ok));return bytes;
}

/** A resize result is tied to one window and request, including host clamping. */
export function encodeRiceWindowSizeResult(app, window, token, width, height, error = '') {
    const ids = [app, window, token].map(value => checkedEventId(value, 'window size result identity'));
    const dimensions = [checkedDimension(width, 'window width'), checkedDimension(height, 'window height')];
    const encoded = new TextEncoder().encode(error);
    if (encoded.length > 4096) throw RangeError('Window size error is too long');
    const bytes = new Uint8Array(HEADER_BYTES + 37 + encoded.length), view = new DataView(bytes.buffer);
    bytes.set([0x52,0x41,0x50,0x50]); view.setUint16(4,1,true); view.setUint16(6,71,true); view.setUint32(8,37+encoded.length,true);
    view.setBigUint64(12,ids[0],true); view.setUint8(20,19); view.setBigUint64(21,ids[1],true); view.setBigUint64(29,ids[2],true);
    view.setUint32(37,dimensions[0],true); view.setUint32(41,dimensions[1],true); view.setUint32(45,encoded.length,true); bytes.set(encoded,49);
    return bytes;
}
