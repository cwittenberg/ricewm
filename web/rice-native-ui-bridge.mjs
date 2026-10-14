import {encodeUiPatch} from '../sdk/versions/1.0.0/runtime/sdk/js/rapp.mjs';
import {NativeGridFocus} from './rice-native-grid-focus.mjs';
import {DocumentEditorPointRequests} from './rice-document-editor-points.mjs';
import { RiceNativeEditorPointer } from "./rice-native-editor-pointer.mjs";
import { t } from "./rice-i18n.mjs";
import { embedBowlMedia } from './rice-bowl-media.mjs';
import {
    RICE_NATIVE_APP_MAX_MESSAGE_BYTES,
    validateRiceNativeAppFrame,
} from "./rice-native-app-wire.mjs";

const RESULT_ERROR = 0xffff_ffff;
const SNAPSHOT_HEADER_BYTES = 104;
const SNAPSHOT_VERSION = 13;
const MAX_SNAPSHOT_BYTES = 32 * 1024 * 1024;
const MAX_COMMANDS = 65_536;
const THEME_RECORD_BYTES = 96;
const MAX_CORNER_RADIUS = 32;
const MODERN_CORNER_RADIUS = 6;
const REDMOND98_RADIUS_MARKER = 0x8000;

const POINTER_KINDS = Object.freeze({
    move: 0,
    down: 1,
    up: 2,
    enter: 3,
    leave: 4,
    wheel: 5,
});

function requiredExport(exports_, name, arity) {
    const value = exports_[name];
    if (typeof value !== "function" || value.length !== arity) {
        throw new TypeError(
            `rice-display.wasm must export ${name}(${arity} arguments)`,
        );
    }
    return value;
}

function checkedId(value, field) {
    let id;
    try {
        id = BigInt(value);
    } catch {
        throw new TypeError(`${field} must be a nonzero integer`);
    }
    if (id <= 0n || id > 0xffff_ffff_ffff_ffffn) {
        throw new RangeError(`${field} is outside the RAPP u64 range`);
    }
    return id;
}

function idHalves(value, field) {
    const id = checkedId(value, field);
    return [
        Number(id & 0xffff_ffffn),
        Number((id >> 32n) & 0xffff_ffffn),
    ];
}

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

function rect(view, offset) {
    return Object.freeze({
        x: view.getInt32(offset, true),
        y: view.getInt32(offset + 4, true),
        width: view.getUint32(offset + 8, true),
        height: view.getUint32(offset + 12, true),
    });
}

function color(bytes, offset) {
    return Object.freeze(Array.from(bytes.subarray(offset, offset + 4)));
}

function validateRadius(radius, geometry) {
    if (radius > MAX_CORNER_RADIUS) {
        throw new RangeError("native UI corner radius exceeds its protocol bound");
    }
    // Older display modules applied the window-decoration radius to tiny
    // controls. Accept those snapshots safely, but never let their geometry
    // escape its own half-size bound.
    return Math.min(
        radius,
        Math.floor(Math.min(geometry.width, geometry.height) / 2),
    );
}

export function decodeRiceNativeUiSnapshot(input) {
    const bytes = asBytes(input, "native UI snapshot");
    if (
        bytes.byteLength < SNAPSHOT_HEADER_BYTES ||
        bytes.byteLength > MAX_SNAPSHOT_BYTES
    ) {
        throw new RangeError("native UI snapshot has an invalid size");
    }
    if (
        bytes[0] !== 0x52 ||
        bytes[1] !== 0x55 ||
        bytes[2] !== 0x49 ||
        bytes[3] !== 0x44
    ) {
        throw new TypeError("native UI snapshot has invalid magic");
    }
    const view = new DataView(
        bytes.buffer,
        bytes.byteOffset,
        bytes.byteLength,
    );
    if (
        ![SNAPSHOT_VERSION,14].includes(view.getUint16(4, true)) ||
        view.getUint16(6, true) !== SNAPSHOT_HEADER_BYTES ||
        view.getUint32(8, true) !== bytes.byteLength
    ) {
        throw new RangeError("native UI snapshot header is incompatible");
    }
    const directionCode = view.getUint32(100, true);
    if (directionCode > 1) {
        throw new RangeError("native UI snapshot direction is invalid");
    }
    const direction = directionCode === 1 ? "rtl" : "ltr";
    const commandCount = view.getUint32(20, true);
    if (commandCount > MAX_COMMANDS) {
        throw new RangeError("native UI snapshot command limit exceeded");
    }

    const dialogWidget = view.getBigUint64(48, true);
    const dialogTitleBytes = view.getUint32(72, true);
    const pointerCursor = view.getUint32(76, true);
    const popupCommandStart = view.getUint32(96, true);
    if (
        pointerCursor > 5 ||
        dialogTitleBytes > bytes.byteLength - SNAPSHOT_HEADER_BYTES ||
        (dialogWidget === 0n && dialogTitleBytes !== 0) ||
        (popupCommandStart !== 0xffff_ffff && popupCommandStart > commandCount)
    ) {
        throw new RangeError("native UI dialog metadata is invalid");
    }
    const decoder = new TextDecoder("utf-8", { fatal: true });
    const dialogTitleEnd = SNAPSHOT_HEADER_BYTES + dialogTitleBytes;
    const dialog = dialogWidget === 0n
        ? null
        : Object.freeze({
            widget: dialogWidget,
            rect: rect(view, 56),
            title: decoder.decode(bytes.subarray(
                SNAPSHOT_HEADER_BYTES,
                dialogTitleEnd,
            )) || t("window.dialog"),
        });
    const popup = popupCommandStart === 0xffff_ffff
        ? null
        : Object.freeze({
            rect: rect(view, 80),
            commandStart: popupCommandStart,
        });
    const commands = [];
    const webScenes = new Map();
    let offset = dialogTitleEnd;
    for (let index = 0; index < commandCount; index += 1) {
        if (offset + 4 > bytes.byteLength) {
            throw new RangeError("native UI command header is truncated");
        }
        const tag = view.getUint8(offset);
        const flags = view.getUint8(offset + 1);
        const recordBytes = view.getUint16(offset + 2, true);
        const end = offset + recordBytes;
        if (
            (
                tag === 3
                    ? (flags & 0x03) > 2 ||
                        ((flags >> 2) & 0x03) > 2 ||
                        ((flags & 0x20) !== 0 && (flags & 0x10) === 0)
                    : tag === 4
                        ? (flags & ~0x0f) !== 0
                    : tag === 7
                        ? (flags & ~0x01) !== 0
                    : tag === 13
                        ? (flags & ~0x7f) !== 0 ||
                            ((flags & 0x20) !== 0 && (flags & 0x08) !== 0) ||
                            ((flags & 0x40) !== 0 &&
                                (flags & (0x08 | 0x20)) !== 0)
                    : tag === 30 ? flags>6
                    : tag === 29
                        ? (flags & ~127)!==0 || (flags&3)>2 || ((flags>>2)&3)>2
                    : tag === 28 || tag === 32
                        ? (flags & ~12) !== 0 || ((flags >> 2) & 3) === 3
                    : tag === 16 || tag === 27 || tag === 31
                        ? flags > 2
                    : tag === 19
                        ? flags > 1
                    : flags !== 0
            ) ||
            recordBytes < 4 ||
            end > bytes.byteLength
        ) {
            throw new RangeError(
                `native UI command record is invalid: index=${index} ` +
                `tag=${tag} flags=${flags} bytes=${recordBytes} ` +
                `offset=${offset} snapshot=${bytes.byteLength}`,
            );
        }
        const geometry = recordBytes >= 20 ? rect(view, offset + 4) : null;
        let command;
        if(tag===34 && recordBytes>=22){const length=view.getUint16(offset+20,true),selector=decoder.decode(bytes.subarray(offset+22,end));if(!length||length>320||recordBytes!==22+length||!/^[a-zA-Z0-9_.:+/@*-]+$/.test(selector))throw RangeError('Invalid icon override selector');command={type:'icon-override',rect:geometry,selector};
        } else if(tag===33 && recordBytes===22) {
            const role=['button','tab','menu','menuItem','dock','notification','separator','runningIndicator','input','checkbox','switch','slider','scrollbar','window'][bytes[offset+20]],state=['normal','hover','pressed','focused','disabled','selected'][bytes[offset+21]];
            if(!role||!state)throw RangeError('Invalid native themed surface');
            command={type:'theme-surface',rect:geometry,surfaceRole:role,surfaceState:state};
        } else if (tag === 1 && recordBytes === 26) {
            command = {
                type: "fill",
                rect: geometry,
                color: color(bytes, offset + 20),
                radius: validateRadius(
                    view.getUint16(offset + 24, true),
                    geometry,
                ),
            };
        } else if (tag === 2 && recordBytes === 28) {
            command = {
                type: "border",
                rect: geometry,
                color: color(bytes, offset + 20),
                width: view.getUint16(offset + 24, true),
                radius: validateRadius(
                    view.getUint16(offset + 26, true),
                    geometry,
                ),
            };
        } else if (tag === 23 && recordBytes === 24) {
            if (
                geometry.width === 0 || geometry.height === 0 ||
                (geometry.width !== 1 && geometry.height !== 1)
            ) {
                throw new RangeError("native UI dotted-line geometry is invalid");
            }
            command = {
                type: "dotted-line",
                rect: geometry,
                color: color(bytes, offset + 20),
            };
        } else if (tag === 3 && recordBytes >= 30) {
            const textBytes = view.getUint32(offset + 26, true);
            const hasCaretOffset = (flags & 0x20) !== 0;
            const textOffset = offset + (hasCaretOffset ? 34 : 30);
            if (recordBytes !== (hasCaretOffset ? 34 : 30) + textBytes) {
                throw new RangeError("native UI text record length is invalid");
            }
            command = {
                type: "text",
                rect: geometry,
                color: color(bytes, offset + 20),
                sizePx: view.getUint16(offset + 24, true),
                align: ["start", "center", "end"][flags & 0x03],
                role: ["label", "editor", "markdown"][(flags >> 2) & 0x03],
                caret: (flags & 0x10) !== 0,
                caretOffset: hasCaretOffset
                    ? view.getUint32(offset + 30, true)
                    : null,
                brand: (flags & 0x40) !== 0,
                gradient: (flags & 0x80) !== 0,
                text: decoder.decode(bytes.subarray(textOffset, end)),
            };
            // Static labels can use locale direction without changing the existing
            // editing model. Carets, editors and brand text stay explicitly LTR.
            command.direction = command.role === "label" && !command.caret &&
                command.caretOffset === null && !command.brand ? direction : "ltr";
        } else if (tag === 30 && recordBytes===26) {
            const width=view.getUint16(offset+24,true);if(width<1||width>12)throw new RangeError("Invalid stroke width");
            command={type:"stroke-line",rect:geometry,color:color(bytes,offset+20),width,pattern:flags};
        } else if (tag === 29 && recordBytes>=32) {
            const sizePx=view.getUint16(offset+24,true),fontBytes=view.getUint16(offset+26,true),textBytes=view.getUint32(offset+28,true);
            const headerBytes=flags&64?34:32,zoom=flags&64?view.getUint16(offset+32,true):100;
            if(fontBytes>1024||!sizePx||zoom<25||zoom>400||recordBytes!==headerBytes+fontBytes+textBytes)throw new RangeError("Invalid grid text layout record");
            const font=fontBytes?JSON.parse(decoder.decode(bytes.subarray(offset+headerBytes,offset+headerBytes+fontBytes))):null;
            if(fontBytes && (!font||typeof font!=="object"||Array.isArray(font)||Object.keys(font).length!==5||typeof font.family!=="string"||!font.family||new TextEncoder().encode(font.family).length>128||/[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(font.family)||!Number.isFinite(font.size_pt)||font.size_pt<1||font.size_pt>409||!Number.isInteger(font.weight)||font.weight<100||font.weight>900||font.weight%100||typeof font.italic!=="boolean"||typeof font.underline!=="boolean"))throw new RangeError("Invalid grid text font");
            command={type:"text",documentText:true,rect:geometry,color:color(bytes,offset+20),sizePx:(font?font.size_pt*4/3:sizePx)*zoom/100,lineHeightPx:Math.max(1,Math.round((font?font.size_pt*4/3:sizePx)*1.25))*zoom/100,align:["start","center","end"][flags&3],vertical:["top","center","bottom"][(flags>>2)&3],wrap:Boolean(flags&16),strike:Boolean(flags&32),role:"label",caret:false,caretOffset:null,brand:false,gradient:false,direction,text:decoder.decode(bytes.subarray(offset+headerBytes+fontBytes,end)),...(font?{font:{family:font.family,weight:font.weight,italic:font.italic,underline:font.underline}}:{})};
        } else if ([27,28,31,32].includes(tag) && recordBytes >= ((tag === 28 || tag === 32 ? 50 : 34) + (tag >= 31 ? 6 : 0))) {
            // Fractional records extend the point-size field; legacy tags retain
            // their original integer units and byte offsets.
            const fractional = tag === 31 || tag === 32, edit = tag === 28 || tag === 32;
            const shift = fractional ? 6 : 0, baseHeaderBytes = (edit ? 50 : 34) + shift;
            const sizePt = fractional ? view.getFloat64(offset + 24, true) : view.getUint16(offset + 24, true);
            const weight = view.getUint16(offset + 26 + shift, true);
            const fontFlags = bytes[offset + 28 + shift], familyBytes = bytes[offset + 29 + shift];
            const textBytes = view.getUint32(offset + 30 + shift, true);
            const headerBytes=baseHeaderBytes+(fontFlags&4?2:0),zoom=fontFlags&4?view.getUint16(offset+baseHeaderBytes,true):100;
            if ((edit ? (flags & ~12) !== 0 || ((flags >> 2) & 3) === 3 : flags > 2) || fontFlags > 7 || zoom<25 || zoom>400 || !Number.isFinite(sizePt) || sizePt < 1 || sizePt > 409
                || weight < 100 || weight > 900 || weight % 100 !== 0
                || !familyBytes || familyBytes > 128 || recordBytes !== headerBytes + familyBytes + textBytes) {
                throw new RangeError("native styled text record is invalid");
            }
            const family = decoder.decode(bytes.subarray(offset + headerBytes, offset + headerBytes + familyBytes));
            if (/[\u0000-\u001f\u007f-\u009f]/u.test(family)) throw new RangeError("native styled text family is invalid");
            command = { type: "text", rect: geometry, color: color(bytes, offset + 20),
                sizePx: sizePt * 4 / 3 * zoom / 100, align: ["start", "center", "end"][flags & 3], role: "label",
                caret: false, caretOffset: null, brand: false, gradient: false, direction,
                font: Object.freeze({ family, weight, italic: (fontFlags & 1) !== 0, underline: (fontFlags & 2) !== 0 }),
                text: decoder.decode(bytes.subarray(offset + headerBytes + familyBytes, end)) };
            if (edit) {
                if (flags & 12) command.vertical = [undefined,"top","bottom"][(flags >> 2) & 3];
                const caret = view.getUint32(offset + 34 + shift, true), start = view.getUint32(offset + 38 + shift, true), finish = view.getUint32(offset + 42 + shift, true), length = Array.from(command.text).length;
                if ((caret !== 0xffffffff && caret > length) || ((start !== 0xffffffff || finish !== 0xffffffff) && (start > finish || finish > length))) throw new RangeError("Invalid document text selection");
                Object.assign(command, { documentEditor: true, lineHeightPx:Math.max(1,Math.round(sizePt*4/3*1.25))*zoom/100, caret: caret !== 0xffffffff, caretOffset: caret === 0xffffffff ? null : caret, direction:"ltr", selection: start === 0xffffffff ? null : { start, end:finish, color:color(bytes,offset+46+shift) } });
            }
        } else if (tag === 18 && recordBytes >= 40) {
            const textBytes = view.getUint32(offset + 36, true);
            if (recordBytes !== 40 + textBytes) {
                throw new RangeError("native UI group-box record length is invalid");
            }
            command = {
                type: "group-box-frame",
                rect: geometry,
                color: color(bytes, offset + 20),
                width: view.getUint16(offset + 24, true),
                sizePx: view.getUint16(offset + 26, true),
                legendX: view.getInt32(offset + 28, true),
                legendY: view.getInt32(offset + 32, true),
                legend: decoder.decode(bytes.subarray(offset + 40, end)),
            };
        } else if (tag === 19 && recordBytes === 34) {
            command = {
                type: "balloon-frame",
                rect: geometry,
                fill: color(bytes, offset + 20),
                border: color(bytes, offset + 24),
                radius: validateRadius(
                    view.getUint16(offset + 28, true),
                    geometry,
                ),
                caretX: view.getInt32(offset + 30, true),
                caretAtTop: (flags & 0x01) !== 0,
            };
        } else if (tag === 4 && recordBytes === 26) {
            command = {
                type: "icon",
                rect: geometry,
                color: color(bytes, offset + 20),
                icon: view.getUint16(offset + 24, true),
                hovered: (flags & 0x01) !== 0,
                animated: (flags & 0x02) !== 0,
                shortcut: (flags & 0x04) !== 0,
                scopedColor: (flags & 0x08) !== 0,
            };
        } else if (tag === 5 && recordBytes === 20) {
            command = { type: "push-clip", rect: geometry };
        } else if (tag === 6 && recordBytes === 4) {
            command = { type: "pop-clip" };
        } else if (tag === 7 && recordBytes === 64) {
            if (flags & ~1) throw new RangeError('Invalid native scene flags');
            const surface = view.getBigUint64(offset + 20, true);
            const totalBytes = view.getUint32(offset + 44, true);
            const scene = {
                type: "web-scene",
                vectorScene: Boolean(flags & 1),
                rect: geometry,
                surface,
                epoch: view.getBigUint64(offset + 28, true),
                viewportWidth: view.getUint32(offset + 36, true),
                viewportHeight: view.getUint32(offset + 40, true),
                displayList: new Uint8Array(totalBytes),
                damageRect: view.getUint32(offset + 56, true) === 0xffff_ffff
                    ? null
                    : Object.freeze({
                        x: view.getUint32(offset + 48, true),
                        y: view.getUint32(offset + 52, true),
                        width: view.getUint32(offset + 56, true),
                        height: view.getUint32(offset + 60, true),
                    }),
                written: 0,
            };
            webScenes.set(surface, scene);
            if (totalBytes === 0) {
                command = scene;
                webScenes.delete(surface);
            } else {
                command = null;
            }
        } else if (tag === 8 && recordBytes >= 16) {
            const surface = view.getBigUint64(offset + 4, true);
            const scene = webScenes.get(surface);
            const chunkOffset = view.getUint32(offset + 12, true);
            const chunk = bytes.subarray(offset + 16, end);
            if (
                !scene ||
                chunkOffset !== scene.written ||
                chunkOffset + chunk.byteLength > scene.displayList.byteLength
            ) {
                throw new RangeError("native web-scene chunk is invalid");
            }
            scene.displayList.set(chunk, chunkOffset);
            scene.written += chunk.byteLength;
            if (scene.written === scene.displayList.byteLength) {
                command = scene;
                webScenes.delete(surface);
            } else {
                command = null;
            }
        } else if (tag === 9 && (recordBytes === 36 || recordBytes === 48)) {
            const request = view.getBigUint64(offset + 20, true);
            const quarterTurns = view.getUint8(offset + 28);
            const scaleMode = view.getUint8(offset + 29);
            const scalePermille = view.getUint16(offset + 30, true);
            const panX = view.getInt16(offset + 32, true);
            const panY = view.getInt16(offset + 34, true);
            if (request === 0n) {
                throw new RangeError("native media request ID is invalid");
            }
            if (
                quarterTurns > 3 ||
                scaleMode > 1 ||
                scalePermille < 10 ||
                scalePermille > 64_000
            ) {
                throw new RangeError("native media transform is invalid");
            }
            command = {
                type: "media",
                rect: geometry,
                request,
                quarterTurns,
                scaleMode: scaleMode === 0 ? "fit" : "actual",
                scale: scalePermille / 1000,
                panX,
                panY,
                ...(recordBytes === 48 ? {imageEdit: Array.from({length: 6}, (_, i) => view.getUint16(offset + 36 + i * 2, true))} : {}),
            };
        } else if (tag === 20 && recordBytes >= 24) {
            const dataBytes = view.getUint32(offset + 20, true);
            if (recordBytes !== 24 + dataBytes) {
                throw new RangeError("native inline-image record length is invalid");
            }
            const dataUrl = decoder.decode(bytes.subarray(offset + 24, end));
            if (!/^data:image\/(?:png|gif|jpe?g|webp|svg\+xml|x-icon|vnd\.microsoft\.icon);base64,/i.test(dataUrl)) {
                throw new RangeError("native inline-image payload is invalid");
            }
            command = {
                type: "inline-image",
                rect: geometry,
                dataUrl,
            };
        } else if ((tag === 10 || tag === 11) && recordBytes === 24) {
            command = {
                type: tag === 10 ? "spinner" : "activity",
                rect: geometry,
                color: color(bytes, offset + 20),
            };
        } else if (tag === 26 && recordBytes === 27) {
            command = {type: "color-picker", rect: geometry, color: color(bytes, offset + 20), hue: bytes[offset + 24] !== 0, hueDegrees: view.getUint16(offset + 25, true)};
        } else if (tag === 25 && recordBytes === 28) {
            command = { type: "terminal-grid", rect: geometry, session: view.getBigUint64(offset + 20, true) };
        } else if (tag === 24 && recordBytes >= 24) {
            const length = view.getUint32(offset + 20, true);
            if (length !== recordBytes - 24) throw new RangeError("invalid terminal row record");
            command = { type: "terminal-row", rect: geometry,
                row: JSON.parse(decoder.decode(bytes.subarray(offset + 24, offset + recordBytes))) };
        } else if (tag === 12 && recordBytes === 28) {
            const session = view.getBigUint64(offset + 20, true);
            if (session === 0n) {
                throw new RangeError("native terminal portal session is invalid");
            }
            command = {
                type: "terminal-portal",
                rect: geometry,
                session,
            };
        } else if (tag === 17 && recordBytes === 84) {
            const identity = decoder.decode(
                bytes.subarray(offset + 20, offset + 84),
            ).toLowerCase();
            if (!/^[0-9a-f]{64}$/.test(identity)) {
                throw new RangeError("native host surface identity is invalid");
            }
            command = {
                type: "host-portal",
                rect: geometry,
                identity,
            };
        } else if (
            tag === 13 &&
            (recordBytes === 20 || recordBytes === 24 || recordBytes === 26)
        ) {
            command = {
                type: "glass",
                rect: geometry,
                tone: flags & 0x03,
                ...(recordBytes >= 24
                    ? { tint: color(bytes, offset + 20) }
                    : {}),
                ...(recordBytes === 26
                    ? { radius: view.getUint16(offset + 24, true) }
                    : {}),
                hovered: (flags & 0x04) !== 0,
                control: (flags & 0x08) !== 0,
                active: (flags & 0x10) !== 0,
                ...((flags & 0x20) !== 0 ? { structural: true } : {}),
                ...((flags & 0x40) !== 0 ? { popup: true } : {}),
            };
        } else if (tag === 21 && recordBytes === 25) {
            const shape = ["default", "wedged", "pill", "chamfered"][
                view.getUint8(offset + 24)
            ];
            if (!shape || shape === "default") {
                throw new RangeError("native shaped fill geometry is invalid");
            }
            command = {
                type: "shaped-fill",
                rect: geometry,
                color: color(bytes, offset + 20),
                shape,
            };
        } else if (tag === 22 && recordBytes === 27) {
            const shape = ["default", "wedged", "pill", "chamfered"][
                view.getUint8(offset + 26)
            ];
            if (!shape || shape === "default") {
                throw new RangeError("native shaped border geometry is invalid");
            }
            command = {
                type: "shaped-border",
                rect: geometry,
                color: color(bytes, offset + 20),
                width: view.getUint16(offset + 24, true),
                shape,
            };
        } else if (tag === 14 && recordBytes === 20) {
            command = { type: "scenic-backdrop", rect: geometry };
        } else if (tag === 15 && recordBytes === 20) {
            command = { type: "welcome-journey", rect: geometry };
        } else if (tag === 16 && recordBytes >= 38) {
            const start = view.getUint32(offset + 26, true);
            const endOffset = view.getUint32(offset + 30, true);
            const textBytes = view.getUint32(offset + 34, true);
            if (recordBytes !== 38 + textBytes || start >= endOffset) {
                throw new RangeError("native UI text selection record is invalid");
            }
            command = {
                type: "text-selection",
                direction: "ltr",
                rect: geometry,
                color: color(bytes, offset + 20),
                sizePx: view.getUint16(offset + 24, true),
                align: ["start", "center", "end"][flags],
                start,
                end: endOffset,
                text: decoder.decode(bytes.subarray(offset + 38, end)),
            };
        } else {
            throw new RangeError(`unknown native UI command ${tag}`);
        }
        if (command) {
            delete command.written;
            commands.push(Object.freeze(command));
        }
        offset = end;
    }
    if (offset !== bytes.byteLength || webScenes.size !== 0) {
        throw new RangeError("native UI snapshot has trailing bytes");
    }
    return Object.freeze({
        direction,
        width: view.getUint32(12, true),
        height: view.getUint32(16, true),
        app: view.getBigUint64(24, true),
        window: view.getBigUint64(32, true),
        focusedWidget: view.getBigUint64(40, true),
        cursor: ["default", "col-resize", "row-resize", "pointer", "text", "crosshair"][pointerCursor],
        dialog,
        popup,
        commands: Object.freeze(commands),
    });
}

/**
 * Bounded adapter for the native UI exports in the rice-display Wasm instance.
 * RAPP frames remain opaque here except for common envelope validation.
 */
export class RiceNativeUiBridge {
    constructor(displayCore, options = {}) {
        const exports_ = displayCore?.exports;
        const memory = displayCore?.memory;
        if (!exports_ || !(memory instanceof WebAssembly.Memory)) {
            throw new TypeError("native UI bridge requires a RiceDisplayCore");
        }
        this.exports = exports_;
        this.memory = memory;
        this.supportsGridDamage = typeof exports_.rice_native_ui_grid_damage_version === "function"
            && exports_.rice_native_ui_grid_damage_version() === 1;
        this.supportsGridFillEndpoint = typeof exports_.rice_native_ui_grid_fill_endpoint_version === "function"
            && exports_.rice_native_ui_grid_fill_endpoint_version() === 1;
        this.supportsGridTextStyle = typeof exports_.rice_native_ui_grid_text_style_version === "function"
            && exports_.rice_native_ui_grid_text_style_version() === 1;
        this.supportsWidgetSize = typeof exports_.rice_native_ui_widget_size_version === "function"
            && exports_.rice_native_ui_widget_size_version() === 1;
        this.supportsGridLargeFrame = typeof exports_.rice_native_ui_grid_large_frame_version === "function"
            && exports_.rice_native_ui_grid_large_frame_version() === 1;
        this.supportsGridFilterRow = typeof exports_.rice_native_ui_grid_filter_row_version === "function"
            && exports_.rice_native_ui_grid_filter_row_version() === 1;
        this.supportsGridPaint = typeof exports_.rice_native_ui_grid_paint_version === "function" && exports_.rice_native_ui_grid_paint_version() === 1;
        this.supportsGridFractionalFont = typeof exports_.rice_native_ui_grid_fractional_font_version === "function" && exports_.rice_native_ui_grid_fractional_font_version() === 1;
        this.supportsSheetDataIcons = typeof exports_.rice_native_ui_sheet_data_icons_version === "function" && exports_.rice_native_ui_sheet_data_icons_version() === 1;
        this.supportsContentShadows = typeof exports_.rice_native_ui_content_shadows_version === "function" && exports_.rice_native_ui_content_shadows_version() === 1;
        this.supportsDocumentAlignment = typeof exports_.rice_native_ui_document_alignment_version === "function" && exports_.rice_native_ui_document_alignment_version() === 1;
        this.supportsGridMerges = typeof exports_.rice_native_ui_grid_merges_version === "function" && exports_.rice_native_ui_grid_merges_version() === 1;
        this.supportsZoomIcons = typeof exports_.rice_native_ui_zoom_icons_version === "function" && exports_.rice_native_ui_zoom_icons_version() === 1;
        this.supportsGridDocumentCanvas = typeof exports_.rice_native_ui_grid_document_canvas_version === "function" && exports_.rice_native_ui_grid_document_canvas_version() === 1;
        this.supportsGridZoom = typeof exports_.rice_native_ui_grid_zoom_version === "function" && exports_.rice_native_ui_grid_zoom_version() === 1;
        this.supportsTextFieldDocumentFont = typeof exports_.rice_native_ui_document_font_version === "function" && exports_.rice_native_ui_document_font_version() === 1;
        this.supportsTextFieldSelection = typeof exports_.rice_native_ui_text_field_selection_version === "function"
            && exports_.rice_native_ui_text_field_selection_version() === 1;
        this.supportsGridReferences = typeof exports_.rice_native_ui_grid_references_version === "function"
            && exports_.rice_native_ui_grid_references_version() === 1;
        this.onRenderEvents = null;
        this.documentEditorPoints = new DocumentEditorPointRequests(this);
        this.gridFocus = new NativeGridFocus();
        this.supportsMeasuredEditor = typeof exports_.rice_native_ui_pointer_measured === 'function'
            && typeof exports_.rice_native_ui_editor_epoch === 'function';
        this.editorPointer = this.supportsMeasuredEditor ? new RiceNativeEditorPointer(options.document) : null;
        this.supportsMeasuredEditor &&= Boolean(this.editorPointer?.renderer);
        this.webSceneDisplayLists = new Map();
        this.renderedSnapshots = new Map();
        this.renderedSnapshotBytes = 0;
        this.renderMutationSerial = 0;
        for (const [name, arity] of [
            ["rice_io_reserve", 1],
            ["rice_io_capacity", 0],
            ["rice_native_ui_init", 0],
            ["rice_native_ui_apply", 1],
            ["rice_native_ui_set_theme", 1],
            ["rice_native_ui_render", 6],
            ["rice_native_ui_snapshot_pending", 0],
            ["rice_native_ui_snapshot_drain", 1],
            ["rice_native_ui_pointer", 9],
            ["rice_native_ui_activate", 6],
            ["rice_native_ui_key", 8],
            ["rice_native_ui_selection_prepare", 4],
            ["rice_native_ui_selection_pending", 0],
            ["rice_native_ui_selection_drain", 1],
            ["rice_native_ui_event_pending", 0],
            ["rice_native_ui_event_drain", 1],
        ]) {
            requiredExport(exports_, name, arity);
        }
        if (
            exports_.rice_native_ui_hit_test !== undefined &&
            (
                typeof exports_.rice_native_ui_hit_test !== "function" ||
                exports_.rice_native_ui_hit_test.length !== 6
            )
        ) {
            throw new TypeError(
                "rice-display.wasm rice_native_ui_hit_test must take 6 arguments",
            );
        }
        if (!options.restored && (this.exports.rice_native_ui_init() >>> 0) !== 1) {
            throw new Error("native UI host initialization failed");
        }
        if (options.checkpoint) this.restoreCheckpoint(options.checkpoint);
    }

    captureCheckpoint() {
        return {
            schema: "rice.native-ui-bridge.checkpoint/v1",
            gridFocus: this.gridFocus.capture(),
            webSceneDisplayLists: [...this.webSceneDisplayLists].map(
                ([key, record]) => ({
                    key,
                    epoch: record.epoch.toString(),
                    displayList: record.displayList.slice(),
                }),
            ),
        };
    }

    restoreCheckpoint(snapshot) {
        this.invalidateRender();
        if (
            snapshot?.schema !== "rice.native-ui-bridge.checkpoint/v1" ||
            !Array.isArray(snapshot.webSceneDisplayLists) ||
            snapshot.webSceneDisplayLists.length > 64
        ) throw new TypeError("native UI bridge checkpoint is invalid");
        const restoredGridFocus = new NativeGridFocus();
        restoredGridFocus.restore(snapshot.gridFocus);
        let sceneBytes = 0;
        for (const record of snapshot.webSceneDisplayLists) {
            if (!(record?.displayList instanceof Uint8Array) || record.displayList.length > 8 * 1024 * 1024 ||
                (sceneBytes += record.displayList.length) > 64 * 1024 * 1024) {
                throw new TypeError("native web-scene checkpoint exceeds its byte budget");
            }
        }
        this.webSceneDisplayLists = new Map(
            snapshot.webSceneDisplayLists.map(record => {
                if (!(record?.displayList instanceof Uint8Array)) {
                    throw new TypeError("native web-scene checkpoint is invalid");
                }
                return [String(record.key), {
                    epoch: BigInt(record.epoch),
                    displayList: record.displayList.slice(),
                }];
            }),
        );
        this.gridFocus = restoredGridFocus;
        return true;
    }

    measureControlTree(nodes, width=0, owner=null) {
        if(!Array.isArray(nodes)||!nodes.length||nodes.length>2048||!Number.isInteger(width)||(width!==0&&(width<96||width>4096)))throw RangeError('Native measurement needs 1..2048 controls and width 0 or 96..4096');
        const query=this.exports.rice_native_ui_measure_tree;if(typeof query!=='function')throw Error('Native host lacks detached control measurement');
        let size=64;const encoder=new TextEncoder(),allowed=new Set(['id','parent','index','kind','state','text','icon']);
        for(const node of nodes){
            if(!node||typeof node!=='object'||Array.isArray(node)||Object.keys(node).some(key=>!allowed.has(key))||typeof node.id!=='string'||! /^[1-9][0-9]{0,19}$/.test(node.id)||node.parent!==null&&(typeof node.parent!=='string'||! /^[1-9][0-9]{0,19}$/.test(node.parent))||!Number.isInteger(node.index)||node.index<0||node.index>2048||!Number.isInteger(node.kind)||node.kind<1||node.kind>65535||!Number.isInteger(node.state)||node.state<0||node.state>0xffff_ffff||node.text!==null&&typeof node.text!=='string'||node.icon!==null&&(!Number.isInteger(node.icon)||node.icon<1||node.icon>65535))throw TypeError('Invalid detached native control descriptor');
            if(node.text!==null&&node.text.length>65536)throw RangeError('Native measurement text exceeds 64 KiB');
            size+=64+(node.text===null?0:encoder.encode(node.text).length);
            if(size>256*1024)throw RangeError('Native measurement exceeds 256 KiB');
        }
        const frame=encodeUiPatch({app:1n,window:1n,sequence:1n,patches:nodes.map(node=>({type:'create',...node,id:BigInt(node.id),parent:node.parent===null?null:BigInt(node.parent)}))});
        if(frame.byteLength>256*1024)throw RangeError('Native measurement exceeds 256 KiB');
        this.#stage(frame);
        const [appLow,appHigh]=owner?idHalves(owner,'measurement owner'):[0,0];
        const packed=BigInt(query(frame.byteLength,width,appLow,appHigh,owner?1:0,0));
        if(packed===0n)throw Error('Native host rejected the detached control measurement');
        return {width:Number(packed&0xffff_ffffn),height:Number(packed>>32n)};
    }

    apply(message) {
        const frame = validateRiceNativeAppFrame(message);
        const header = new DataView(frame.buffer, frame.byteOffset, frame.byteLength);
        this.invalidateRender(header.getBigUint64(12, true), header.getBigUint64(20, true));
        const pointer = this.#stage(frame);
        void pointer;
        const consumed = this.exports.rice_native_ui_apply(
            frame.byteLength,
        ) >>> 0;
        if (consumed !== frame.byteLength) {
            throw new Error("native UI host rejected the RAPP patch");
        }
        this.gridFocus.observe(frame);
        return this.supportsGridDamage ? this.drainEvents() : [];
    }

    removeApp(app) {
        const id = BigInt(app);
        if (this.exports.rice_native_ui_remove_app(Number(id & 0xffff_ffffn), Number(id >> 32n)) !== 1) {
            throw new Error("native UI host could not release application");
        }
        this.purgePresentation(app,{nativeReleased:true});
    }

    // Caller must first replace private native widgets/scenes with its locked
    // presentation. This releases JS/checkpoint ownership without removing the
    // native app, its window, or its monotonic patch sequence.
    purgePresentation(app,{nativeReleased=false}={}) {
        const id=checkedId(app,'presentation owner');
        this.invalidateRender(id);
        for (const key of this.dialogWorkAreas?.keys() || []) if (key.startsWith(`${id}:`)) this.dialogWorkAreas.delete(key);
        this.documentEditorPoints.removeApp(id);
        this.editorPointer?.removeApp(id);
        this.gridFocus.removeApp(id);
        for (const key of this.webSceneDisplayLists.keys()) {
            if (!key.startsWith(`${id}:`))continue;
            // Live references require retransmission if an ordinary caller
            // purges while retaining a scene. Removed surfaces need no native
            // release; forget_scene accepts those as an idempotent operation.
            const ids=key.split(':').flatMap(value=>idHalves(BigInt(value),'scene owner ID'));
            if(!nativeReleased && this.exports.rice_native_ui_forget_scene?.(...ids)!==1) {
                throw new Error('native UI could not release retained scene reference');
            }
            this.webSceneDisplayLists.delete(key);
        }
        return true;
    }

    setTheme(theme) {
        this.invalidateRender();
        const groups = [
            [theme?.taskbar, 7],
            [theme?.menu, 6],
            [theme?.frame, 8],
        ];
        for (const [colors, expected] of groups) {
            if (
                !Array.isArray(colors) ||
                colors.length !== expected ||
                colors.some(value =>
                    !Number.isInteger(value) ||
                    value < 0 ||
                    value > 0x00ff_ffff
                )
            ) {
                throw new RangeError("invalid RiceWM native theme RGB groups");
            }
        }
        if (
            !Number.isInteger(theme?.icon) ||
            theme.icon < 0 ||
            theme.icon > 0x00ff_ffff
        ) {
            throw new RangeError("invalid RiceWM native theme icon color");
        }
        if (
            !Number.isInteger(theme?.iconHover) ||
            theme.iconHover < 0 ||
            theme.iconHover > 0x00ff_ffff
        ) {
            throw new RangeError("invalid RiceWM native theme icon hover color");
        }
        const redmond98 = theme?.chromeProfile === "redmond-98";
        const familiar = theme?.file === "classic.theme" ||
            theme?.chromeProfile === "familiar";
        const defaultRadius = familiar || redmond98
            ? 0
            : MODERN_CORNER_RADIUS;
        const radius = theme?.cornerRadius ?? defaultRadius;
        if (
            !Number.isInteger(radius) ||
            radius < 0 ||
            radius > MAX_CORNER_RADIUS
        ) {
            throw new RangeError("invalid RiceWM native theme corner radius");
        }
        const custom=theme?.bundle||{};
        const extended=Object.keys(custom.typography||{}).length||Object.keys(custom.metrics||{}).length;
        if(extended && this.exports.rice_native_ui_theme_version?.()!==2)throw Error('Desktop typography and metrics require the updated native display engine');
        const record = new Uint8Array(extended?126:THEME_RECORD_BYTES);
        record.set([0x52, 0x54, 0x55, 0x49]);
        const view = new DataView(record.buffer);
        view.setUint16(4, extended?2:1, true);
        view.setUint16(6, record.length, true);
        // Preserve the 96-byte ABI: the bounded radius field has an unused
        // high bit which Rust strips before validating the metric.
        view.setUint16(8, radius | (redmond98 ? REDMOND98_RADIUS_MARKER : 0) | (theme?.bundle?.surface?.frame === "bracket" ? 1 << 14 : 0), true);
        const backdropMaterial = ["backdrop-blur", "frosted-ice", "liquid-glass"].includes(
            theme?.material,
        ) || [
            "glass.theme",
            "frosted-ice.theme",
            "emerald.theme",
            "orchard.theme",
        ].includes(theme?.file);
        const frostedEnabled = theme?.frostedIce?.enabled === true ||
            backdropMaterial;
        const frostedWidgets = theme?.frostedIce?.widgets;
        let flags = backdropMaterial ? 1 : 0;
        if (frostedEnabled && frostedWidgets?.buttons !== false) flags |= 1 << 1;
        if (frostedEnabled && frostedWidgets?.switches !== false) flags |= 1 << 2;
        if (frostedEnabled && frostedWidgets?.sliders !== false) flags |= 1 << 3;
        if (frostedEnabled && frostedWidgets?.scrollbars !== false) flags |= 1 << 4;
        const chromeProfile = theme?.chromeProfile || "standard";
        // The stable `classic.theme` filename remains compatible, while the
        // profile also lets bundled and custom retro themes opt into the same
        // compact raised metrics without filename coupling.
        if (familiar) flags |= 1 << 5;
        if (chromeProfile === "yellow-jacket") flags |= 1 << 6;
        if (chromeProfile === "command-deck") flags |= 1 << 7;
        if (chromeProfile === "orchard") flags |= 1 << 8;
        if (chromeProfile === "neon-grid") flags |= 1 << 9;
        const buttonStyle = String(theme?.buttonStyle || "").toLowerCase();
        if (buttonStyle === "default") flags |= 1 << 13;
        if (buttonStyle === "wedged") flags |= 1 << 10;
        if (buttonStyle === "pill") flags |= 1 << 11;
        if (buttonStyle === "chamfered") flags |= 1 << 12;
        if (theme?.sidebarMaterial === "glass") flags |= 1 << 14;
        if (theme?.menuMaterial === "glass") flags |= 1 << 15;
        view.setUint16(10, flags, true);
        let offset = 12;
        for (const colors of groups.map(([values]) => values)) {
            for (const value of colors) {
                view.setUint32(offset, value, true);
                offset += 4;
            }
        }
        if(extended){
            const fields=[['typography','bodyPx'],['typography','smallPx'],['typography','titlePx'],['typography','displayPx'],['typography','lineHeightPercent'],['metrics','controlHeight'],['metrics','menuRowHeight'],['metrics','scrollbarWidth'],['metrics','focusWidth'],['metrics','borderWidth'],['metrics','controlPaddingX'],['metrics','controlPaddingY'],['metrics','cornerRadius']];
            let mask=0;fields.forEach(([group,key],i)=>{if(custom[group]?.[key]!==undefined){mask|=1<<i;view.setUint16(100+i*2,custom[group][key],true);}});view.setUint32(96,mask,true);
        }
        this.#stage(record);
        if ((this.exports.rice_native_ui_set_theme(record.byteLength) >>> 0) !== 1) {
            throw new Error("native UI host rejected the RiceWM theme");
        }
    }

    invalidateRender(app = null, window = null) {
        this.renderMutationSerial++;
        for (const [key, value] of this.renderedSnapshots) {
            if (app !== null && (window === null ? !key.startsWith(`${app}:`) : key !== `${app}:${window}`)) continue;
            this.renderedSnapshotBytes -= value.bytes;
            this.renderedSnapshots.delete(key);
        }
    }

    setFormFactor(compact) {
        const value = Number(Boolean(compact));
        if (value !== this.renderedFormFactor) this.invalidateRender();
        this.exports.rice_native_ui_form_factor?.(value);
        this.renderedFormFactor = value;
    }

    setDialogWorkArea(app, window, area) {
        const setter = this.exports.rice_native_ui_dialog_work_area;
        if (typeof setter !== 'function') return false;
        const ids = [...idHalves(app, "application ID"), ...idHalves(window, "window ID")];
        const values = [area.x, area.y, area.width, area.height];
        if (!values.every(Number.isInteger) || Math.abs(area.x) > 16384 || Math.abs(area.y) > 16384 ||
            area.width < 1 || area.height < 1 || area.width > 16384 || area.height > 16384) {
            return false;
        }
        this.dialogWorkAreas ??= new Map();
        const key = `${app}:${window}`, signature = values.join(',');
        if (this.dialogWorkAreas.get(key) === signature) return true;
        if ((setter(...ids, ...values) >>> 0) !== 1) return false;
        this.dialogWorkAreas.set(key, signature);
        this.invalidateRender(app, window);
        return true;
    }

    render(app, window, width, height) {
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const cacheKey = `${app}:${window}`;
        const cached = this.renderedSnapshots.get(cacheKey);
        if (cached && cached.snapshot.width === width && cached.snapshot.height === height &&
            (!this.supportsMeasuredEditor || cached.epoch === this.exports.rice_native_ui_editor_epoch(appLow, appHigh, windowLow, windowHigh))) {
            this.editorPointer?.retain(cached.snapshot, cached.epoch);
            this.documentEditorPoints.observe(cached.snapshot);
            return embedBowlMedia(cached.snapshot, this.ownsWebMediaSession);
        }
        this.invalidateRender(app, window);
        const renderSerial = this.renderMutationSerial;
        const expected = this.exports.rice_native_ui_render(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
            width,
            height,
        ) >>> 0;
        if (
            expected === RESULT_ERROR ||
            expected < SNAPSHOT_HEADER_BYTES ||
            expected > MAX_SNAPSHOT_BYTES ||
            (this.exports.rice_native_ui_snapshot_pending() >>> 0) !== expected
        ) {
            throw new Error(`native UI render failed for application ${app}, window ${window}`);
        }
        const pointer = this.#reserve(expected);
        const written = this.exports.rice_native_ui_snapshot_drain(
            expected,
        ) >>> 0;
        if (written !== expected) {
            throw new Error("native UI snapshot drain failed");
        }
        let snapshot = decodeRiceNativeUiSnapshot(
            new Uint8Array(this.memory.buffer, pointer, written).slice(),
        );
        const renderEpoch = this.exports.rice_native_ui_editor_epoch?.(appLow, appHigh, windowLow, windowHigh);
        if(this.gridFocus.has(snapshot.app,snapshot.window,snapshot.focusedWidget))snapshot=Object.freeze({...snapshot,focusedWidgetKind:45});
        this.editorPointer?.retain(snapshot,
            this.exports.rice_native_ui_editor_epoch?.(appLow, appHigh, windowLow, windowHigh));
        let changed = false;
        const commands = snapshot.commands.map(command => {
            if (command.type !== "web-scene" || command.epoch === 0n) {
                return command;
            }
            const key = `${snapshot.app}:${snapshot.window}:${command.surface}`;
            if (command.displayList.byteLength > 0) {
                this.webSceneDisplayLists.delete(key);
                this.webSceneDisplayLists.set(key, {
                    epoch: command.epoch,
                    displayList: command.displayList,
                });
                return command;
            }
            const retained = this.webSceneDisplayLists.get(key);
            if (!retained || retained.epoch !== command.epoch) {
                throw new Error(
                    "native UI web-scene reference has no retained display list",
                );
            }
            changed = true;
            return Object.freeze({
                ...command,
                displayList: retained.displayList,
            });
        });
        // Resolve the complete snapshot before evicting: later commands may
        // reference a resident entry displaced by an earlier new scene. Tell
        // native emission to resend evicted bytes next time, leaving its owned
        // scene intact. Silent JS-only eviction strands live native references.
        let sceneBytes = [...this.webSceneDisplayLists.values()].reduce((sum, record) => sum + record.displayList.byteLength, 0);
        while (this.webSceneDisplayLists.size > 64 || sceneBytes > 64 * 1024 * 1024) {
            const key = this.webSceneDisplayLists.keys().next().value;
            const ids = key.split(':').flatMap(value => idHalves(BigInt(value), 'scene owner ID'));
            if (typeof this.exports.rice_native_ui_forget_scene !== 'function' ||
                this.exports.rice_native_ui_forget_scene(...ids) !== 1) {
                throw new Error('Native scene cache eviction requires a synchronized display runtime');
            }
            sceneBytes -= this.webSceneDisplayLists.get(key).displayList.byteLength;
            this.webSceneDisplayLists.delete(key);
        }
        if (this.supportsWidgetSize && typeof this.onRenderEvents === "function") {
            const events = this.drainEvents();
            if (events.length) this.onRenderEvents(events);
        }
        this.documentEditorPoints.observe(snapshot);
        const resolved = changed
            ? Object.freeze({ ...snapshot, commands: Object.freeze(commands) })
            : snapshot;
        // Unrelated status/clock updates must not lay out every open document.
        // Bound retained snapshots independently of the native web-scene cache.
        const bytes = written + commands.reduce((size, command) => size + (command.displayList?.byteLength || 0), 0);
        if (bytes <= 16 * 1024 * 1024 && this.renderMutationSerial === renderSerial) {
            while (this.renderedSnapshots.size >= 16 || this.renderedSnapshotBytes + bytes > 16 * 1024 * 1024) {
                const oldest = this.renderedSnapshots.keys().next().value;
                this.renderedSnapshotBytes -= this.renderedSnapshots.get(oldest).bytes;
                this.renderedSnapshots.delete(oldest);
            }
            this.renderedSnapshots.set(cacheKey, {snapshot: resolved, bytes,
                epoch: renderEpoch});
            this.renderedSnapshotBytes += bytes;
        }
        return embedBowlMedia(resolved, this.ownsWebMediaSession);
    }

    async prepareEditorPointer(app, window) {
        if (!this.editorPointer) return false;
        let changed = false;
        const key = `${app}:${window}`;
        const ids = [...idHalves(app, 'application ID'), ...idHalves(window, 'window ID')];
        for (let attempt = 0; attempt < 4; attempt++) {
            let retained = this.editorPointer.snapshots.get(key);
            if (!retained) return changed;
            if (this.exports.rice_native_ui_editor_epoch(...ids) !== retained.epoch) {
                this.render(app, window, retained.snapshot.width, retained.snapshot.height);
                retained = this.editorPointer.snapshots.get(key); changed = true;
            }
            changed = await this.editorPointer.prepare(app, window) || changed;
            if (this.exports.rice_native_ui_editor_epoch(...ids) === retained.epoch) return changed;
        }
        throw new Error('Native editor layout changed repeatedly while preparing pointer input');
    }

    pointer(app, window, kind, x, y, button = 0, modifiers = 0) {
        this.invalidateRender(app, window);
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const kindValue = typeof kind === "number" ? kind : POINTER_KINDS[kind];
        if (!Number.isInteger(kindValue) || kindValue < 0 || kindValue > 5) {
            throw new RangeError("unknown native UI pointer event");
        }
        this.documentEditorPoints.pointer(app, window, kindValue, x, y, button, modifiers);
        const sample = this.editorPointer?.sample(this, app, window, kindValue, x, y, button);
        if (kindValue === POINTER_KINDS.down && button === 6 && sample?.multiline) {
            // DOM double-click is a complete semantic gesture. Native multiline
            // editors select words through a measured primary press carrying
            // this modifier; preserve button6 for non-editor application hits.
            const down = this.pointer(app, window, "down", x, y, 1, modifiers | 0x8000);
            const up = this.pointer(app, window, "up", x, y, 1, modifiers);
            const events = [...down, ...up];
            Object.defineProperty(events, "visualChanged", { value: down.visualChanged || up.visualChanged });
            Object.defineProperty(events, "motionContinuous", { value: false });
            return events;
        }
        if (sample) this.#stage(sample.bytes);
        const pointer = sample ? this.exports.rice_native_ui_pointer_measured : this.exports.rice_native_ui_pointer;
        const result = pointer(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
            kindValue,
            x,
            y,
            button,
            modifiers,
            ...(sample ? [sample.bytes.byteLength] : []),
        ) >>> 0;
        if (result === RESULT_ERROR) {
            throw new Error("native UI pointer event failed");
        }
        this.editorPointer?.accepted(app, window, kindValue, button, sample?.target);
        const events = this.drainEvents();
        Object.defineProperty(events, "visualChanged", {
            value: typeof this.exports.rice_native_ui_pointer_visual_changed === "function"
                ? (this.exports.rice_native_ui_pointer_visual_changed() >>> 0) === 1
                : true,
        });
        Object.defineProperty(events, "motionContinuous", {
            value: typeof this.exports.rice_native_ui_pointer_motion_continuous === "function"
                ? (this.exports.rice_native_ui_pointer_motion_continuous() >>> 0) === 1
                : true,
        });
        return events;
    }

    releaseDocumentEditorPointer(app, window, x, y, button, modifiers = 0) {
        return this.documentEditorPoints.releaseSuppressed(app, window, x, y, button, modifiers);
    }

    widgetActions(app, window, target) {
        const query = this.exports.rice_native_ui_widget_actions;
        if (typeof query !== "function") return null;
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const [targetLow, targetHigh] = idHalves(target, "widget ID");
        const flags = query(appLow, appHigh, windowLow, windowHigh, targetLow, targetHigh) >>> 0;
        return {activatable: Boolean(flags & 1), editable: Boolean(flags & 2), focusable: Boolean(flags & 4)};
    }

    activate(app, window, target) {
        this.invalidateRender(app, window);
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const [targetLow, targetHigh] = idHalves(target, "widget ID");
        const result = this.exports.rice_native_ui_activate(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
            targetLow,
            targetHigh,
        ) >>> 0;
        if (result === RESULT_ERROR || (result !== 1 && !this.widgetActions(app, window, target)?.activatable)) {
            throw new Error("native UI semantic activation failed");
        }
        return this.drainEvents();
    }

    hitTest(app, window, x, y) {
        const hitTest = this.exports.rice_native_ui_hit_test;
        if (typeof hitTest !== "function") return null;
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const result = BigInt(hitTest(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
            Math.trunc(Number(x) || 0),
            Math.trunc(Number(y) || 0),
        ));
        return result === 0n ? null : result;
    }

    scrollOwnerAtPoint(app, window, x, y) {
        const query=this.exports.rice_native_ui_scroll_owner;
        if(typeof query!=="function")return null;
        const [appLow,appHigh]=idHalves(app,"application ID"),[windowLow,windowHigh]=idHalves(window,"window ID");
        const owner=BigInt(query(appLow,appHigh,windowLow,windowHigh,x,y));
        return owner===0n?null:owner;
    }

    key(
        app,
        window,
        pressed,
        keyCode,
        modifiers = 0,
        text = "",
    ) {
        this.invalidateRender(app, window);
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const encoded = new TextEncoder().encode(text);
        if (encoded.byteLength > 16 * 1024) {
            throw new RangeError("native UI key text exceeds the RAPP limit");
        }
        if (encoded.byteLength > 0) this.#stage(encoded);
        const result = this.exports.rice_native_ui_key(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
            pressed ? 1 : 0,
            keyCode,
            modifiers,
            encoded.byteLength,
        ) >>> 0;
        if (result === RESULT_ERROR) {
            throw new Error("native UI key event failed");
        }
        return this.drainEvents();
    }

    selectedText(app, window) {
        const [appLow, appHigh] = idHalves(app, "application ID");
        const [windowLow, windowHigh] = idHalves(window, "window ID");
        const expected = this.exports.rice_native_ui_selection_prepare(
            appLow,
            appHigh,
            windowLow,
            windowHigh,
        ) >>> 0;
        if (
            expected === RESULT_ERROR ||
            (this.exports.rice_native_ui_selection_pending() >>> 0) !== expected
        ) {
            throw new Error("native UI text selection query failed");
        }
        if (expected === 0) return "";
        const pointer = this.#reserve(expected);
        const written = this.exports.rice_native_ui_selection_drain(expected) >>> 0;
        if (written !== expected) {
            throw new Error("native UI text selection drain failed");
        }
        return new TextDecoder("utf-8", { fatal: true }).decode(
            new Uint8Array(this.memory.buffer, pointer, written).slice(),
        );
    }

    drainEvents() {
        const output = [];
        for (;;) {
            const pending =
                this.exports.rice_native_ui_event_pending() >>> 0;
            if (pending === 0) return output;
            if (
                pending === RESULT_ERROR ||
                pending > RICE_NATIVE_APP_MAX_MESSAGE_BYTES
            ) {
                throw new Error("native UI event queue is invalid");
            }
            const pointer = this.#reserve(pending);
            const written =
                this.exports.rice_native_ui_event_drain(pending) >>> 0;
            if (written !== pending) {
                throw new Error("native UI event drain failed");
            }
            output.push(validateRiceNativeAppFrame(
                new Uint8Array(this.memory.buffer, pointer, written).slice(),
            ));
        }
    }

    #stage(bytes) {
        const pointer = this.#reserve(bytes.byteLength);
        new Uint8Array(
            this.memory.buffer,
            pointer,
            bytes.byteLength,
        ).set(bytes);
        return pointer;
    }

    #reserve(capacity) {
        if (!Number.isInteger(capacity) || capacity < 1) {
            throw new RangeError("native UI staging capacity is invalid");
        }
        const pointer = this.exports.rice_io_reserve(capacity) >>> 0;
        const actual = this.exports.rice_io_capacity() >>> 0;
        if (
            pointer === 0 ||
            actual < capacity ||
            pointer + capacity > this.memory.buffer.byteLength
        ) {
            throw new RangeError("native UI staging allocation failed");
        }
        return pointer;
    }
}
