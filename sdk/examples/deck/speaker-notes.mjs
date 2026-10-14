import { Column, Label, Message, State, TextEditor } from '@ricewm/app';
import { validNotesText } from './notes-contract.mjs';

// Same reserved envelope space as the shared SDK TextEditor.
const EDITOR_BYTES = 16 * 1024 - 128;
const bytes = value => new TextEncoder().encode(value).length;
function prefix(value) {
    let length = 0, result = '';
    for (const scalar of value) {
        length += bytes(scalar);
        if (length > EDITOR_BYTES) break;
        result += scalar;
    }
    return result;
}

/** Retained native notes editor; the source document owns committed content.
 * Pending input is pinned to a slide, and late replies never replace the caret
 * or characters entered while a save is in flight. */
export function createSpeakerNotes({ save, changed, notice }) {
    let slide = null, original = '', value = '', editable = false, ready = false;
    let opened = false, displayed = true, closed = false, timer = null, pending = null;
    const heading = new Label({ text: 'Speaker notes' });
    const help = new Label({ name: 'speaker-notes-hint', text: 'Click to add notes' });
    const editor = new TextEditor({ name: 'speaker-notes-editor', accessibleName: 'Speaker notes',
        value: '', rows: 4, enabled: false, readOnly: true, state: State.EXPANSIVE });
    const control = new Column({ name: 'speaker-notes-pane', text: 'panel:fixed-height:148:gap=2', visible: false }, [heading, editor, help]);
    const dirty = () => Boolean(slide && value !== original);
    function cancel() { if (timer !== null) clearTimeout(timer); timer = null; }
    function schedule() {
        cancel();
        if (!closed && ready && editable && dirty() && validNotesText(value)) {
            timer = setTimeout(() => { timer = null; void flush(); }, 150);
        }
    }
    function refresh() {
        control.setProperty('visible', opened && displayed);
        editor.setProperty('enabled', Boolean(!closed && ready && editable));
        editor.setProperty('readOnly', !editable || !ready);
    }
    async function flush() {
        cancel();
        if (pending) { if (!await pending) return false; if (dirty()) return flush(); return !closed; }
        if (!dirty()) return !closed;
        if (closed || !ready || !editable || !validNotesText(value)) return false;
        const target = slide, text = value;
        pending = Promise.resolve().then(() => save({ type: 'set-speaker-notes', slide: target, text })).then(success => {
            if (!closed && slide === target && success === true) { original = text; changed(); return true; }
            return false;
        }).catch(error => { if (!closed) notice(`Could not save notes: ${String(error.message).slice(0, 180)}`); return false; });
        let success;
        try { success = await pending; }
        finally { pending = null; }
        if (success && dirty()) return flush();
        return success;
    }
    editor.on(Message.VALUE_CHANGED, event => {
        if (closed || !ready || !editable || !slide) return;
        value = String(event.detail?.value ?? editor.getProperty('value'));
        help.text = value ? '' : 'Click to add notes'; help.setProperty('visible', !value);
        changed(); schedule();
    });
    return Object.freeze({ control, editor,
        get opened() { return opened; }, get dirty() { return dirty(); },
        get committing() { return Boolean(pending); }, get slide() { return slide; },
        bind(snapshot, { preserveDraft = false } = {}) {
            const next = snapshot?.slide || null;
            const retained = dirty() && preserveDraft && next === slide;
            if (dirty() && !retained) throw Error('Speaker notes must be saved before replacing their slide');
            if (next !== slide) editor.setProperty('focused', false);
            slide = next; original = snapshot?.text || ''; if (!retained) value = original;
            const tooLong = bytes(value) > EDITOR_BYTES;
            editable = Boolean(snapshot?.editable && !tooLong);
            editor.setProperty('value', tooLong ? prefix(value) : value);
            help.text = tooLong ? 'These notes exceed the editor limit. The original text is preserved.' : snapshot?.reason || (!value ? 'Click to add notes' : '');
            help.setProperty('visible', Boolean(help.text)); refresh();
        },
        setState({ enabled, visible = true }) { ready = Boolean(enabled); displayed = visible; refresh(); schedule(); },
        setOpen(next) { opened = Boolean(next); if (!opened) editor.setProperty('focused', false); refresh(); },
        focus() { if (opened && displayed && ready && editable) editor.setProperty('focused', true); },
        flush,
        close() { closed = true; cancel(); editor.setProperty('focused', false); },
    });
}
