import {Control, SurfaceControl, State} from './control.mjs';
import {Message} from './messages.mjs';
import {desktopEventTopics} from './desktop.mjs';

class TerminalRow extends Control { static kind = 64; }
const encoder = new TextEncoder();
export function nativeTerminalKey({keyCode: code, modifiers = 0, text = ''}, application = false) {
    const shift = !!(modifiers & 1), ctrl = !!(modifiers & 4), alt = !!(modifiers & 8), modifier = 1 + +shift + 2 * +alt + 4 * +ctrl;
    const arrow = {111: 'A', 116: 'B', 114: 'C', 113: 'D', 110: 'H', 115: 'F'}[code];
    if (arrow) return encoder.encode(modifier > 1 ? `\x1b[1;${modifier}${arrow}` : `\x1b${application ? 'O' : '['}${arrow}`);
    const tilde = {118: 2, 119: 3, 112: 5, 117: 6, 71: 15, 72: 17, 73: 18, 74: 19, 75: 20, 76: 21, 95: 23, 96: 24}[code];
    if (tilde) return encoder.encode(`\x1b[${tilde}${modifier > 1 ? ';' + modifier : ''}~`);
    if (code >= 67 && code <= 70) return encoder.encode(modifier > 1 ? `\x1b[1;${modifier}${String.fromCharCode(80 + code - 67)}` : `\x1bO${String.fromCharCode(80 + code - 67)}`);
    let value = {9: '\x1b', 22: ctrl ? '\x08' : '\x7f', 23: shift ? '\x1b[Z' : '\t', 36: '\r', 104: '\r'}[code];
    if (value === undefined) {
        const fallback = {24:'q',25:'w',26:'e',27:'r',28:'t',29:'y',30:'u',31:'i',32:'o',33:'p',38:'a',39:'s',40:'d',41:'f',42:'g',43:'h',44:'j',45:'k',46:'l',52:'z',53:'x',54:'c',55:'v',56:'b',57:'n',58:'m',65:' ',34:'[',35:']',51:'\\'}[code] || '';
        value = text || (ctrl ? fallback : ''); if (!value) return null;
        if (ctrl) { const first = value.codePointAt(0); if (first >= 97 && first <= 122) value = String.fromCharCode(first - 96); else if (first >= 64 && first <= 95) value = String.fromCharCode(first & 31); else if (first === 32) value = '\0'; else return null; }
    }
    return encoder.encode((alt ? '\x1b' : '') + value);
}

/** Embedded Rice Terminal2 presentation using the existing native terminal grid. */
export class NativeTerminal extends SurfaceControl {
    static kind = 63;
    static controlAvailability = 'approved applications; requires protected terminal service';
    static constructorOptions = Object.freeze(['services', 'bus', 'onState', 'onError', 'passThroughKey']);
    static authoring = Object.freeze({constructor: 'new NativeTerminal({services,bus,onState,onError,...properties})', methods: 'await open({directory,columns:100,rows:12}), write(Uint8Array), resize(columns,rows), close(). Existing Rice Terminal2 headless session and native grid; declare terminal.spawn/write/resize/close and desktopEventTopics(appId).terminalEvent subscription. directory is a user-selected workspace grant. Session input, frames and exit are scoped to one approved worker. No DOM terminal or host shell.'});
    constructor({services, bus, onState = () => {}, onError = () => {}, passThroughKey = () => false, ...properties} = {}) {
        if (!services?.terminal || !bus) throw TypeError('NativeTerminal requires terminal services and the app bus');
        super({text: '1:13:0', state: State.ENABLED | State.EXPANSIVE, ...properties});
        Object.assign(this, {services, bus, onState, onError, passThroughKey}); this.session = null; this.nativeSession = null; this.frame = null; this.rowFrames = []; this.closed = false; this.pending = []; this.pendingBytes = 0; this.writeQueue = Promise.resolve(); this.resizeQueue = Promise.resolve();
        this.on(Message.SIZE_CHANGED, event => { const d = event.detail || {}; if (d.width > 0 && d.height > 0) { this.measuredSize = [Math.max(2, Math.floor(d.width / 8)), Math.max(1, Math.floor(d.height / 18))]; void this.resize(...this.measuredSize).catch(this.onError); } });
        this.on(Message.BLUR, () => this.cancelPointer());
        this.on(Message.CHANGE, event => { if (['enabled','visible'].includes(event.detail?.property) && event.detail.value === false) this.cancelPointer(); });
        this.on(Message.DESTROY, () => void this.close().catch(onError));
    }
    inspectControl() { return {kind: 'terminal', clipboard: 'application', state: this.stateName || 'closed', selectionActive: !!this.selecting}; }
    async open({directory, columns = 100, rows = 12, command}) {
        if (this.session || this.opening || this.closed || !this.application) throw Error('Terminal is already open, closed or unattached');
        const receive = event => {
            const data = event.payload; if (this.closed || !data?.terminal || !data.event) return;
            if (!this.session) {
                this.pendingBytes += typeof data.event.payload === 'string' ? data.event.payload.length * 2 : 1024;
                if (this.pending.length >= 128 || this.pendingBytes > 8 * 1024 * 1024) { this.onError(Error('Terminal startup output exceeded its budget')); void this.close().catch(this.onError); return; }
                this.pending.push(data); return;
            }
            if (data.terminal === this.session) { try { this.accept(data.event); } catch (error) { this.onError(error); } }
        };
        this.stop = this.bus.subscribe(desktopEventTopics(this.application.manifest.id).terminalEvent, receive);
        this.stateName = 'starting'; this.onState('starting');
        this.opening = this.services.terminal.spawn({kind: 'terminal2', directory, columns, rows, ...(command === undefined ? {} : {command})});
        try {
            const result = await this.opening; this.session = result.terminal; this.nativeSession = result.nativeSession;
            if (this.closed) { await this.services.terminal.close({session: this.session}); return; }
            if (typeof this.session !== 'string' || !/^[1-9][0-9]{0,19}$/u.test(this.nativeSession || '')) throw Error('Invalid native terminal receipt');
            this.text = `${this.nativeSession}:13:0`;
            for (const data of this.pending) if (data.terminal === this.session) this.accept(data.event);
            this.pending = []; this.pendingBytes = 0; return result;
        } catch (error) { this.stop?.(); this.stateName = 'failed'; this.onState('failed'); throw error; }
        finally { this.opening = null; }
    }
    accept(event) {
        if (event.kind === 'grid-chunk') {
            if (!Number.isSafeInteger(event.frame) || event.frame < 1 || !Number.isInteger(event.index) || !Number.isInteger(event.total) || event.total < 1 || event.total > 86 || typeof event.payload !== 'string' || event.payload.length > 48 * 1024 || event.index < 0 || event.index >= event.total) throw Error('Invalid terminal grid chunk');
            if (event.index === 0) this.chunks = {frame: event.frame, total: event.total, parts: []};
            const chunks = this.chunks;
            if (!chunks || chunks.frame !== event.frame || chunks.total !== event.total || chunks.parts.length !== event.index) throw Error('Terminal grid chunk order changed');
            chunks.parts.push(event.payload);
            if (chunks.parts.length === chunks.total) { this.chunks = null; this.accept({kind: 'grid', payload: chunks.parts.join('')}); }
            return;
        }
        if (event.kind === 'ready' || event.kind === 'exited') { this.stateName = event.kind; this.onState(event.kind, event); if (event.kind === 'ready' && this.measuredSize) void this.resize(...this.measuredSize).catch(this.onError); return; }
        if (event.kind !== 'grid') return;
        if (typeof event.payload !== 'string' || event.payload.length > 4 * 1024 * 1024) throw Error('Terminal frame exceeds budget');
        const frame = JSON.parse(event.payload);
        if (!Number.isInteger(frame.columns) || frame.columns < 2 || frame.columns > 512 || !Number.isInteger(frame.rows) || frame.rows < 1 || frame.rows > 256 || !Array.isArray(frame.lines) || frame.lines.length !== frame.rows) throw Error('Invalid terminal frame');
        if ((!this.frame || this.frame.columns !== frame.columns || this.frame.rows !== frame.rows) && frame.lines.some(line => line === null)) throw Error('Terminal resize requires complete rows');
        for (let index = 0; index < frame.rows; index++) {
            const line = frame.lines[index]; if (line === null) continue;
            if (!line || !Array.isArray(line.cells) || line.cells.length > frame.columns) throw Error('Invalid native terminal row');
            if (this.selection && JSON.stringify(this.rowFrames[index]?.cells) !== JSON.stringify(line.cells)) { this.selection = null; this.selecting = false; }
            this.rowFrames[index] = line;
            const text = JSON.stringify({...line, font_size: 13, transparency: 0, icon_glyphs: true});
            if (!this.children[index]) this.append(new TerminalRow({text})); else this.children[index].text = text;
        }
        while (this.children.length > frame.rows) this.remove(this.children.at(-1), {destroy: true});
        this.rowFrames.length = frame.rows; this.frame = {...frame, lines: []}; this.paintSelection();
    }
    write(bytes) {
        if (!(bytes instanceof Uint8Array) || bytes.length > 65536 || !this.session || this.closed || this.stateName !== 'ready') return Promise.reject(Error('Terminal is not ready for input'));
        const session = this.session, copy = bytes.slice();
        this.writeQueue = this.writeQueue.catch(() => {}).then(async () => { if (this.closed || this.session !== session) throw Error('Terminal closed'); const result = await this.services.terminal.write({session, bytes: copy}); if (result.written !== copy.length) throw Error('Incomplete terminal input'); }); return this.writeQueue;
    }
    resize(columns, rows) {
        if (!this.session || this.closed || this.stateName !== 'ready') return Promise.resolve(false);
        columns = Math.max(2, Math.min(512, columns)); rows = Math.max(1, Math.min(256, rows));
        const size = `${columns}:${rows}`; if (this.requestedSize === size) return this.resizeQueue; this.requestedSize = size;
        this.resizeQueue = this.resizeQueue.catch(() => {}).then(() => this.closed ? false : this.services.terminal.resize({session: this.session, columns, rows})); return this.resizeQueue;
    }
    paintSelection() {
        if (!this.frame) return;
        let [start, end] = this.selection || [];
        if (start > end) [start, end] = [end, start];
        for (let row = 0; row < this.rowFrames.length; row++) {
            const columns = this.frame.columns, from = Math.max(0, start - row * columns), to = Math.min(columns, end - row * columns);
            const selection = Number.isFinite(from) && to > from ? [from, to] : null;
            this.children[row].text = JSON.stringify({...this.rowFrames[row], selection, font_size: 13, transparency: 0, icon_glyphs: true});
        }
    }
    selectedText() {
        if (!this.selection || !this.frame) return '';
        const [start, end] = [...this.selection].sort((a,b) => a-b), lines = [], columns = this.frame.columns;
        for (let row = Math.floor(start / columns); row < this.rowFrames.length && row * columns < end; row++) {
            const from = Math.max(0, start - row * columns), to = Math.min(columns, end - row * columns);
            let text = '', cursor = from;
            for (const [column, width, value] of this.rowFrames[row].cells) { if (column >= to || column + width <= from) continue; if (column > cursor) text += ' '.repeat(column - cursor); text += value; cursor = column + width; }
            lines.push(text.replace(/ +$/u, ''));
        }
        return lines.join('\n');
    }
    async paste() {
        const session = this.session, text = await this.services.clipboard.readText();
        if (this.closed || session !== this.session || this.stateName !== 'ready') throw Error('Terminal changed while reading clipboard');
        if (typeof text !== 'string') return;
        const value = text.replace(/\r?\n/gu, '\r').replaceAll('\x1b', '');
        const bytes = encoder.encode(this.frame?.bracketed_paste ? `\x1b[200~${value}\x1b[201~` : value);
        if (bytes.length > 65536) throw RangeError('Paste exceeds 64 KiB terminal input budget');
        return this.write(bytes);
    }
    cancelPointer() {
        this.selecting = false;
        if (this.mouseButton !== undefined && this.frame && this.stateName === 'ready' && !this.closed) { const bytes = this.mouse({kind: 'up', button: this.mouseButton + 1, ...(this.lastMousePoint || {x:0,y:0}), modifiers:0}); if (bytes) void this.write(bytes).catch(this.onError); }
        this.mouseButton = undefined;
    }
    mouse(detail) {
        this.lastMousePoint = {x:detail.x, y:detail.y};
        const column = Math.max(1, Math.min(this.frame.columns, Math.floor(detail.x / 8) + 1)), row = Math.max(1, Math.min(this.frame.rows, Math.floor(detail.y / 18) + 1));
        const down = detail.kind === 'down' || detail.kind === 'wheel', up = detail.kind === 'up' || detail.kind === 'leave';
        if (up && this.mouseButton === undefined) return null;
        if (this.frame.mouse_x10 && !down) { if (up) this.mouseButton = undefined; return null; }
        if (!down && !up && !(this.frame.mouse_motion === 2 || this.frame.mouse_motion === 1 && this.mouseButton !== undefined)) return null;
        let button = detail.kind === 'wheel' ? detail.button === 4 ? 64 : 65 : down ? Math.max(0, detail.button - 1) : this.mouseButton ?? 3;
        if (down && detail.kind !== 'wheel') this.mouseButton = button;
        if (up) this.mouseButton = undefined;
        if (!down && !up) button |= 32;
        const modifiers = detail.modifiers || 0; button |= (modifiers & 1 ? 4 : 0) | (modifiers & 8 ? 8 : 0) | (modifiers & 4 ? 16 : 0);
        if (this.frame.mouse_sgr) return encoder.encode(`\x1b[<${button};${column};${row}${up ? 'm' : 'M'}`);
        if (up) button = 3 | (button & 28);
        return new Uint8Array([27,91,77,32 + button,32 + Math.min(223,column),32 + Math.min(223,row)]);
    }
    scroll(lines) {
        if (!this.session || this.closed || this.stateName !== 'ready') return Promise.resolve(false);
        this.selection = null; this.selecting = false; this.paintSelection();
        return this.services.terminal.scroll({session: this.session, lines});
    }
    windowProc(event) {
        const d = event.detail || {};
        if (event.type === Message.KEY && d.pressed && this.stateName === 'ready') {
            if (this.passThroughKey(d)) { this.cancelPointer(); return undefined; }
            if ((d.modifiers & 77) === 5 && [54,55].includes(d.keyCode) && this.services.clipboard) { event.preventDefault(); event.stopPropagation(); return (d.keyCode === 55 ? this.paste() : this.services.clipboard.writeText(this.selectedText())).catch(this.onError); }
            if ((d.modifiers & 77) === 1 && [112,117].includes(d.keyCode)) { event.preventDefault(); event.stopPropagation(); return this.scroll((d.keyCode === 112 ? 1 : -1) * (this.frame?.rows || 12)).catch(this.onError); }
            event.preventDefault(); event.stopPropagation(); const bytes = nativeTerminalKey(d, this.frame?.application_cursor); if (bytes) return this.write(bytes).catch(this.onError); return true;
        }
        if (event.type === Message.POINTER && d.kind === 'leave') this.selecting = false;
        if (event.type === Message.POINTER && this.frame && this.stateName === 'ready') {
            if (this.frame.mouse && (this.mouseButton !== undefined || !(d.modifiers & 1) && !this.selecting)) { const bytes = this.mouse(d); event.preventDefault(); return bytes ? this.write(bytes).catch(this.onError) : true; }
            if (d.kind === 'wheel' && [4,5].includes(d.button)) { event.preventDefault(); return this.scroll(d.button === 4 ? 3 : -3).catch(this.onError); }
            if (d.button === 1 && d.kind === 'down' || this.selecting && ['move','up'].includes(d.kind)) {
                const col = Math.max(0, Math.min(this.frame.columns, Math.floor(d.x / 8))), row = Math.max(0, Math.min(this.frame.rows - 1, Math.floor(d.y / 18))), point = row * this.frame.columns + col;
                if (d.kind === 'down') { this.selection = [point, point]; this.selecting = true; } else { this.selection[1] = point; if (d.kind === 'up') this.selecting = false; }
                this.paintSelection(); event.preventDefault(); return true;
            }
        }
        return super.windowProc(event);
    }
    async close() {
        if (this.closed) return; this.closed = true; this.stop?.(); this.pending = []; this.pendingBytes = 0;
        if (this.session) await this.services.terminal.close({session: this.session}); this.stateName = 'closed';
    }
}
