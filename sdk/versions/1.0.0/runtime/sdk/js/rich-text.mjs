import { Column, Row, Button, TextEditor, TextField } from './controls.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { CommandHistory } from './command-history.mjs';
import { validateRichText, richTextPlainText } from './visual-document.mjs';

const scalars = (text) => Array.from(text);
function characters(value) {
    const out = [];
    for (const [i, block] of value.entries()) {
        if (i) out.push({ text: '\n', block: { ...block, runs: undefined } });
        for (const run of block.runs)
            for (const text of scalars(run.text))
                out.push({ ...run, text, block: { ...block, runs: undefined } });
    }
    return out;
}
function blocks(chars, emptyType = 'paragraph') {
    const result = [];
    let block = { type: emptyType, runs: [] };
    for (const character of chars) {
        if (character.text === '\n') {
            result.push(block);
            block = { ...character.block, runs: [] };
            continue;
        }
        if (!block.runs.length) block = { ...character.block, runs: [] };
        const { text, block: _, ...style } = character,
            last = block.runs.at(-1);
        if (
            last &&
            JSON.stringify({ ...last, text: undefined }) === JSON.stringify({ ...style, text: undefined })
        )
            last.text += text;
        else block.runs.push({ ...style, text });
    }
    result.push(block);
    return result.map((b) => Object.fromEntries(Object.entries(b).filter(([, v]) => v !== undefined)));
}
export function replaceRichText(value, text, typingStyle = null) {
    value = validateRichText(value);
    if (typeof text !== 'string' || text.length > 32768) throw RangeError('Rich text exceeds budget');
    const old = characters(value),
        next = scalars(text);
    let prefix = 0,
        suffix = 0;
    while (prefix < old.length && prefix < next.length && old[prefix].text === next[prefix]) prefix++;
    while (
        suffix < old.length - prefix &&
        suffix < next.length - prefix &&
        old[old.length - 1 - suffix].text === next[next.length - 1 - suffix]
    )
        suffix++;
    const inherited = {
        ...(old[Math.max(0, prefix - 1)] || { block: { type: 'paragraph' } }),
        ...typingStyle,
    };
    if (inherited.link === null) delete inherited.link;
    const inserted = next.slice(prefix, next.length - suffix).map((text) => ({ ...inherited, text }));
    return validateRichText(
        blocks([...old.slice(0, prefix), ...inserted, ...(suffix ? old.slice(old.length - suffix) : [])]),
    );
}
export function formatRichText(value, { anchor = 0, caret = 0 }, patch) {
    value = validateRichText(value);
    if (
        !patch ||
        Object.keys(patch).some(
            (k) => !['bold', 'italic', 'underline', 'link', 'type', 'checked'].includes(k),
        )
    )
        throw TypeError('Unsupported rich text format');
    const chars = characters(value),
        start = Math.min(anchor, caret),
        end = Math.max(anchor, caret);
    if (![anchor, caret].every((v) => Number.isInteger(v) && v >= 0 && v <= chars.length))
        throw RangeError('Invalid rich text selection');
    if (patch.type !== undefined || patch.checked !== undefined) {
        let position = 0;
        const next = value.map((block) => {
            const length = scalars(block.runs.map((r) => r.text).join('')).length,
                selected = position <= end && position + length >= start;
            position += length + 1;
            return selected
                ? {
                      ...block,
                      ...(patch.type !== undefined ? { type: patch.type } : {}),
                      ...(patch.checked !== undefined ? { checked: patch.checked } : {}),
                  }
                : block;
        });
        return validateRichText(next);
    }
    for (let i = start; i < end; i++) {
        Object.assign(chars[i], patch);
        if (chars[i].link === null) delete chars[i].link;
    }
    return validateRichText(blocks(chars));
}
export function richTextMarkdown(value) {
    return validateRichText(value)
        .map((block) => {
            const text = block.runs
                .map((run) => {
                    let text = run.text.replace(/[\\`*_\[\]]/g, '\\$&');
                    if (run.bold) text = '**' + text + '**';
                    if (run.italic) text = '*' + text + '*';
                    if (run.link) text = '[' + text + '](' + run.link + ')';
                    return text;
                })
                .join('');
            return (
                (block.type === 'heading'
                    ? '# '
                    : block.type === 'checklist'
                      ? `- [${block.checked ? 'x' : ' '}] `
                      : block.type === 'bullet'
                        ? '- '
                        : block.type === 'code'
                          ? '    '
                          : '') + text
            );
        })
        .join('\n');
}

/** Existing native rich-editor paint and measured selection consume these
 * Unicode scalar ranges; no HTML editor or second caret model is introduced. */
export function richTextLanguage(value) {
    let offset = 0;
    const ranges = [];
    for (const block of validateRichText(value)) {
        const start = offset;
        for (const run of block.runs) {
            const end = offset + scalars(run.text).length;
            for (const [key, token] of [
                ['bold', 'b'],
                ['italic', 'i'],
                ['underline', 'u'],
            ])
                if (run[key]) ranges.push(`${token}${offset}-${end}`);
            if (run.link) ranges.push(`u${offset}-${end}`);
            offset = end;
        }
        if (block.type === 'heading') ranges.push(`b${start}-${offset}`, `f${start}-${offset},20`);
        if (block.type === 'bullet') ranges.push(`B${start}-${offset}`);
        if (block.type === 'checklist') ranges.push(`K${start}-${offset},${block.checked ? 1 : 0}`);
        offset++;
    }
    if (ranges.length > 256) throw RangeError('Rich text presentation exceeds 256 styled ranges');
    return 'rich' + ranges.map((r) => '~' + r).join('');
}
class RichTextInput extends TextEditor {
    toWidget() {
        const widget = super.toWidget(),
            language = richTextLanguage(this.richValue?.() || [{ type: 'paragraph', runs: [] }]);
        return Object.freeze({
            ...widget,
            text: widget.text.replace(/^editor:plain:/, `editor:${language}:`),
        });
    }
}

/** Native Unicode selection edits a validated structured rich-text model. */
export class RichTextEditor extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['value', 'onChange', 'history', 'height']);
    static authoring = Object.freeze({
        constructor:
            'new RichTextEditor({value:[{type:"paragraph",runs:[{text:"Hello",bold:true}]}],onChange(blocks)})',
        methods:
            'format({bold,italic,underline,link}), setBlock(type,{checked}), undo(), redo(); editor exposes native TextEditor selection. Styled native editing uses the shared rich-editor renderer and Unicode scalar caret/selection. Validated blocks retain headings, checklists, safe links and inline marks.',
    });
    constructor({
        value = [{ type: 'paragraph', runs: [] }],
        onChange,
        history,
        height = 240,
        ...options
    } = {}) {
        if (!Number.isInteger(height) || height < 120 || height > 4096)
            throw RangeError('Rich editor height must be 120..4096');
        super({ ...options, text: `panel:fixed-height:${height}:gap=8` });
        this.history = history ?? new CommandHistory({ value, validate: validateRichText });
        this.ownsHistory = !history;
        this.onChange = onChange;
        this.typingStyle = {};
        this.editor = new RichTextInput({
            name: 'rich-text-input',
            value: richTextPlainText(this.history.value),
            state: State.ENABLED | State.EXPANSIVE,
        });
        this.editor.richValue = () => this.history.value;
        const actions = [
            ['Bold', () => this.toggle('bold')],
            ['Italic', () => this.toggle('italic')],
            ['Underline', () => this.toggle('underline')],
            ['Heading', () => this.setBlock('heading')],
            ['Paragraph', () => this.setBlock('paragraph')],
            ['Checklist', () => this.setBlock('checklist', { checked: false })],
            ['Check', () => this.setBlock('checklist', { checked: true })],
            ['Undo', () => this.undo()],
            ['Redo', () => this.redo()],
        ];
        this.append(
            new Row(
                {},
                actions.map(([text, run]) => {
                    const button = new Button({ text, name: 'rich-' + text.toLowerCase() });
                    button.on(Message.ACTIVATE, run);
                    return button;
                }),
            ),
        );
        this.linkField = new TextField({ name: 'rich-link-url', accessibleName: 'Link URL' });
        const link = new Button({ text: 'Apply link' });
        link.on(Message.ACTIVATE, () => this.format({ link: this.linkField.value }));
        this.append(new Row({}, [this.linkField, link]));
        this.append(this.editor);
        this.editor.on(Message.VALUE_CHANGED, () => {
            if (this.synchronizing) return;
            this.history.transact('Edit rich text', () =>
                replaceRichText(this.history.value, this.editor.value, this.typingStyle),
            );
        });
        this.editor.on(Message.KEY, (event) => {
            if (event.detail?.pressed === false || !(event.detail?.modifiers & 4)) return;
            const mark = { 56: 'bold', 31: 'italic', 30: 'underline' }[event.detail.keyCode];
            if (mark) {
                this.toggle(mark);
                event.preventDefault();
            }
        });
        this.stop = this.history.subscribe(() => {
            this.synchronizing = true;
            try {
                const text = richTextPlainText(this.history.value);
                if (this.editor.value !== text) this.editor.value = text;
                this.editor.application?.invalidate(this.editor);
            } finally {
                this.synchronizing = false;
            }
            this.onChange?.(structuredClone(this.history.value));
        });
    }
    get value() {
        return this.history.value;
    }
    selection() {
        return this.editor.selection || { anchor: 0, caret: 0 };
    }
    format(patch) {
        const selection = this.selection();
        if (
            selection.anchor === selection.caret &&
            !Object.hasOwn(patch, 'type') &&
            !Object.hasOwn(patch, 'checked')
        ) {
            formatRichText([{ type: 'paragraph', runs: [{ text: 'x' }] }], { anchor: 0, caret: 1 }, patch);
            Object.assign(this.typingStyle, patch);
            this.editor.setProperty('focused', true);
            return true;
        }
        return this.history.transact('Format text', () =>
            formatRichText(this.history.value, selection, patch),
        );
    }
    toggle(mark) {
        const chars = characters(this.value),
            selection = this.selection(),
            start = Math.min(selection.anchor, selection.caret),
            end = Math.max(selection.anchor, selection.caret);
        return this.format({
            [mark]: start === end ? !this.typingStyle[mark] : !chars.slice(start, end).every((c) => c[mark]),
        });
    }
    setBlock(type, options = {}) {
        return this.format({ type, ...options });
    }
    undo() {
        return this.history.undo();
    }
    redo() {
        return this.history.redo();
    }
    destroy() {
        this.stop?.();
        if (this.ownsHistory) this.history.destroy();
        super.destroy();
    }
}
