import { Message } from './messages.mjs';

// Reserve space for the native envelope within the 16 KiB widget-text limit.
export const MAX_EDITOR_VALUE_BYTES = 16 * 1024 - 128;
const encoder = new TextEncoder();
export function textEditorValue(value) {
    const text = String(value ?? '');
    if (encoder.encode(text).length > MAX_EDITOR_VALUE_BYTES) throw new RangeError('TextEditor value exceeds native text budget');
    for (const scalar of text) if (scalar.codePointAt(0) >= 0xd800 && scalar.codePointAt(0) <= 0xdfff) throw new TypeError('TextEditor value contains an unpaired surrogate');
    return text;
}
const category = value => /\s/u.test(value) ? 0 : /[\p{L}\p{N}\p{M}_]/u.test(value) ? 1 : 2;
function word(chars, caret, direction) {
    if (direction < 0) {
        while (caret > 0 && category(chars[caret - 1]) === 0) caret--;
        const kind = caret > 0 ? category(chars[caret - 1]) : -1;
        while (caret > 0 && category(chars[caret - 1]) === kind) caret--;
    } else {
        const kind = caret < chars.length ? category(chars[caret]) : -1;
        while (caret < chars.length && category(chars[caret]) === kind) caret++;
        while (caret < chars.length && category(chars[caret]) === 0) caret++;
    }
    return caret;
}
const lineStart = (chars, caret) => caret === 0 ? 0 : chars.lastIndexOf('\n', caret - 1) + 1;
function lineEnd(chars, caret) { const end = chars.indexOf('\n', caret); return end < 0 ? chars.length : end; }

/** Plain SDK editor state. Rust rich editors retain their own editing model.
 * Indices are Unicode scalar counts, matching native TextSelection events. */
export class TextEditorController {
    constructor(control) { this.control = control; this.value = null; this.caret = 0; this.anchor = 0; this.column = null; }
    synchronize() {
        const value = this.control.getProperty('value');
        if (value !== this.value) {
            this.value = value; this.caret = Array.from(value).length; this.anchor = this.caret; this.column = null;
        }
        return Array.from(value);
    }
    presentation() {
        this.synchronize();
        return `editor:plain:${this.control.getProperty('wrap') ? 'word' : 'none'}:nolines:${this.caret}:${this.anchor}\n${this.value}`;
    }
    selection(caret, anchor = caret) {
        this.caret = caret; this.anchor = anchor;
        this.control.application?.invalidate(this.control);
    }
    replace(chars, start, end, inserted) {
        const next = [...chars.slice(0, start), ...inserted, ...chars.slice(end)].join('');
        try { textEditorValue(next); } catch (error) { if (error instanceof RangeError) return false; throw error; }
        this.value = next; this.column = null;
        this.selection(start + inserted.length);
        // Use the same public semantic event as host-edited TextField controls.
        this.control.sendMessage(Message.VALUE_CHANGED, { value: next });
        return true;
    }
    handle(event) {
        const control = this.control;
        if (!control.getProperty('enabled') || !control.getProperty('visible')) return false;
        const chars = this.synchronize(), detail = event.detail || {};
        if (event.type === Message.TEXT_SELECTION) {
            if (![detail.anchor, detail.caret].every(value => Number.isSafeInteger(value) && value >= 0 && value <= chars.length)) return false;
            // Native editor pointer events carry selection rather than separate
            // focus messages. Keep semantic focus exclusive before producing
            // patches, so an earlier TextField focus flag cannot reclaim it.
            const root=control.application?.controlWindows.get(control.id)?.root;
            if(root){
                const blur=peer=>{if(peer!==control&&peer.getProperty('focused'))peer.sendMessage(Message.BLUR);for(const child of [...peer.children])blur(child);};
                blur(root);if(control.destroyed||!root.contains(control))return false;
                if(!control.getProperty('focused'))control.sendMessage(Message.FOCUS);
            }
            this.column = null; this.selection(detail.caret, detail.anchor); return true;
        }
        if (event.type !== Message.KEY || !detail.pressed) return false;
        const key = detail.keyCode, modifiers = Number(detail.modifiers) || 0;
        if(key===23&&!control.getProperty('acceptTab'))return false;
        const altGraph = Boolean(modifiers & 128);
        const shift = Boolean(modifiers & 1), ctrl = Boolean(modifiers & 4) && !altGraph;
        if ((modifiers & 64) || ((modifiers & 8) && !altGraph)) return false;
        if (ctrl && key === 38) { this.column = null; this.selection(chars.length, 0); return true; }
        const start = Math.min(this.anchor, this.caret), end = Math.max(this.anchor, this.caret);
        const arrows = { 113: -1, 114: 1 }, vertical = { 111: -1, 116: 1 };
        if (key in arrows || key in vertical || key === 110 || key === 115) {
            let caret = this.caret;
            if (key in arrows) {
                const direction = arrows[key];
                caret = !shift && start !== end ? (direction < 0 ? start : end)
                    : ctrl ? word(chars, caret, direction) : Math.max(0, Math.min(chars.length, caret + direction));
                this.column = null;
            } else if (key in vertical) {
                const beginning = lineStart(chars, caret); this.column ??= caret - beginning;
                if (vertical[key] < 0) caret = beginning ? Math.min(beginning - 1, lineStart(chars, beginning - 1) + this.column) : 0;
                else { const ending = lineEnd(chars, caret); caret = ending < chars.length ? Math.min(lineEnd(chars, ending + 1), ending + 1 + this.column) : chars.length; }
            } else { caret = key === 110 ? (ctrl ? 0 : lineStart(chars, caret)) : (ctrl ? chars.length : lineEnd(chars, caret)); this.column = null; }
            this.selection(caret, shift ? this.anchor : caret); return true;
        }
        if (control.getProperty('readOnly')) return false;
        if (key === 22 || key === 119) {
            const a = start !== end ? start : key === 22 ? (ctrl ? word(chars, start, -1) : Math.max(0, start - 1)) : start;
            const b = start !== end ? end : key === 119 ? (ctrl ? word(chars, end, 1) : Math.min(chars.length, end + 1)) : end;
            if (a !== b) this.replace(chars, a, b, []);
            return true;
        }
        if (ctrl) return false;
        const input = key === 36 || key === 104 ? '\n' : key === 23 ? '\t' : typeof detail.text === 'string' ? detail.text : '';
        const inserted = Array.from(input.replace(/\r\n?/gu, '\n')).filter(char => char === '\n' || char === '\t' || !/[\u0000-\u001f\u007f-\u009f]/u.test(char));
        if (!inserted.length) return false;
        return this.replace(chars, start, end, inserted);
    }
}
