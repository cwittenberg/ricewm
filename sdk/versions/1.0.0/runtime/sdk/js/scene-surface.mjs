import { SurfaceControl } from './control.mjs';
import { Message } from './messages.mjs';

export const MAX_SCENE_BYTES = 8 * 1024 * 1024 - 56;
const retained = new WeakMap();
const VECTOR_OPS = new Set([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 25, 26, 29, 30]);

/** Copies the bounded vector envelope. RiceUI also validates every command's
 * payload and resources at its authoritative native ingress. */
export function copySceneFrame({ width, height, displayList } = {}) {
    for (const value of [width, height]) {
        if (!Number.isInteger(value) || value < 1 || value > 16384) throw new RangeError('Invalid scene viewport');
    }
    if (!(displayList instanceof Uint8Array)) throw new TypeError('Scene displayList must be Uint8Array');
    if (displayList.length > MAX_SCENE_BYTES) throw new RangeError('Scene exceeds its byte budget');
    // Native snapshots reserve a zero-byte payload for a retained reference.
    // Clear with an explicit transparent no-op rectangle, not that sentinel.
    const bytes = displayList.length ? displayList.slice() : new Uint8Array(28);
    if (!displayList.length) { bytes[0] = 5; bytes[4] = 28; }
    const view = new DataView(bytes.buffer);
    let stacking = 0, clips = 0, shadows = 0, shadowCount = 0, count = 0;
    const scopes = [], images = new Set();
    for (let offset = 0; offset < bytes.length;) {
        if (offset + 8 > bytes.length) throw new RangeError('Truncated scene command');
        const op = bytes[offset], compact = view.getUint16(offset + 2, true), extended = view.getUint32(offset + 4, true);
        const length = compact ? compact * 4 : extended;
        if (!VECTOR_OPS.has(op)) throw new RangeError('Scene command is not a vector command');
        if (bytes[offset + 1] || (compact && extended) || length < 8 || length % 4 || offset + length > bytes.length) {
            throw new RangeError('Invalid scene command envelope');
        }
        if (++count > 262144) throw new RangeError('Too many scene commands');
        if (op === 14) {
            if (length < 24) throw new RangeError('Invalid scene image resource');
            const id = view.getUint32(offset + 8, true), width = view.getUint32(offset + 12, true), height = view.getUint32(offset + 16, true);
            const size = view.getUint32(offset + 20, true);
            if (!id || !width || !height || width > 16384 || height > 16384 || size !== width * height * 4 || length !== size + 24 || images.has(id)) {
                throw new RangeError('Invalid or duplicate scene image resource');
            }
            images.add(id);
        }
        if (op === 7 && (length !== 52 || !images.has(view.getUint32(offset + 40, true)))) {
            throw new RangeError('Scene image must be defined before use');
        }
        if (op === 29) {
            if (length !== 40 && length !== 64) throw new RangeError('Invalid content shadow command');
            const values = Array.from({ length: 7 }, (_, i) => view.getFloat32(offset + 8 + i * 4, true));
            if (values.some(value => !Number.isFinite(value)) || [0, 1, 4, 5].some(i => Math.abs(values[i]) > 1e6) ||
                [2, 3].some(i => values[i] <= 0 || values[i] > 16384) || values[6] < 0 || values[6] > 1e6) {
                throw new RangeError('Invalid content shadow bounds');
            }
            if (length === 64) {
                const m = Array.from({length: 6}, (_, i) => view.getFloat32(offset + 40 + i * 4, true));
                if (m.some(v => !Number.isFinite(v)) || m.slice(0, 4).some(v => Math.abs(v) > 1024) ||
                    m.slice(4).some(v => Math.abs(v) > 1e6) || m[0]*m[3] === m[1]*m[2]) throw new RangeError('Invalid shadow affine');
            }
            if (++shadows > 4 || ++shadowCount > 128) throw new RangeError('Too many nested or total content shadows');
            scopes.push('shadow');
        }
        if (op === 30 && (length !== 8 || --shadows < 0 || scopes.pop() !== 'shadow')) throw new RangeError('Invalid scene shadow stack');
        if (op === 1) stacking++;
        if (op === 2) stacking--;
        if (op === 3 || op === 20) clips++;
        if (op === 4) clips--;
        if (op === 1) scopes.push('transform');
        if (op === 3 || op === 20) scopes.push('clip');
        if ((op === 2 && scopes.pop() !== 'transform') || (op === 4 && scopes.pop() !== 'clip')) {
            throw new RangeError('Invalid scene stack nesting');
        }
        if (stacking < 0 || clips < 0 || stacking > 256 || clips > 256) throw new RangeError('Invalid scene stack');
        offset += length;
    }
    if (stacking || clips || shadows) throw new RangeError('Unbalanced scene stack');
    return Object.freeze({ width, height, displayList: bytes });
}

/** A retained vector surface owned and painted by RiceUI. No DOM or browser
 * navigation capability is exposed. Pointer events use viewport coordinates. */
export class SceneSurface extends SurfaceControl {
    static kind = 65;
    static properties = Object.freeze({ cursor: {
        default: 'default', validate(value) {
            if (!['default', 'pointer', 'crosshair', 'col-resize', 'row-resize'].includes(value)) throw new TypeError('Unsupported scene cursor');
            return value;
        },
    }, presentation: {
        default: 'surface',
        validate(value) {
            if (!['surface', 'page'].includes(value)) throw new TypeError('Scene presentation must be surface or page');
            return value;
        },
    }, pageZoom: {
        default: 'fit', validate(value) {
            if (value !== 'fit' && (!Number.isInteger(value) || value < 10 || value > 400)) throw new RangeError('Page zoom must be fit or 10–400 percent');
            return value;
        },
    }, pagePanX: { default: .5, validate: pagePan }, pagePanY: { default: .5, validate: pagePan }, backgroundColor: {
        default: null,
        validate(value) {
            if (value !== null && value !== 'transparent' && (typeof value !== 'string' || !/^#[0-9a-fA-F]{6}$/.test(value))) {
                throw new TypeError('Scene backgroundColor must be null, transparent or #RRGGBB');
            }
            return value?.toLowerCase() ?? null;
        },
    } });
    toWidget() {
        const widget = super.toWidget();
        const cursor = this.values.cursor === 'default' ? '' : `:cursor:${this.values.cursor}`;
        if (this.values.presentation === 'page') {
            const frame = retained.get(this);
            const zoom = this.values.pageZoom === 'fit' ? '' : `:zoom:${this.values.pageZoom}:pan:${Math.round(this.values.pagePanX * 1_000_000)}:${Math.round(this.values.pagePanY * 1_000_000)}`;
            return Object.freeze({ ...widget, text: `scene:page:${frame?.width || 1}:${frame?.height || 1}:bg:${this.values.backgroundColor || '#ffffff'}${zoom}${cursor}` });
        }
        if (this.values.backgroundColor === null && !cursor) return widget;
        return Object.freeze({ ...widget, text: `${this.values.backgroundColor === null ? 'scene' : `scene:bg:${this.values.backgroundColor}`}${cursor}` });
    }
    append() { throw new TypeError('SceneSurface is a leaf; submit a vector frame'); }
    submit(frame) {
        if (this.destroyed) throw new Error('SceneSurface is destroyed');
        const next = copySceneFrame(frame);
        const previous = retained.get(this);
        const epoch = (previous?.epoch || 0n) + 1n;
        if (epoch > 0xffffffffffffffffn) throw new RangeError('Scene epoch exhausted');
        const value = Object.freeze({ ...next, epoch });
        this.application?.queueScene(this, value);
        retained.set(this, value);
        if (this.values.presentation === 'page' && (previous?.width !== value.width || previous?.height !== value.height)) {
            this.application?.invalidate(this);
        }
        if (previous?.width !== value.width || previous?.height !== value.height) this.emit(Message.CHANGE, { property: 'frameSize', value: { width: value.width, height: value.height } });
        return epoch;
    }
    destroy() { retained.delete(this); return super.destroy(); }
}

// Internal retained value; public submissions and wire encoding both copy bytes.
export function retainedScene(control) { return retained.get(control) || null; }

function pagePan(value) {
    if (!Number.isFinite(value) || value < 0 || value > 1) throw new RangeError('Page pan must be between zero and one');
    return value;
}
