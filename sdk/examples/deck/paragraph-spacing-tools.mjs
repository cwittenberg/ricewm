import { Button, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';

const keys = ['lineHeight', 'spaceBefore', 'spaceAfter'];
const defaults = key => ({ kind: key === 'lineHeight' ? 'multiple' : 'exact', value: key === 'lineHeight' ? 1 : 0 });
const equal = (a, b) => a.kind === b.kind && Math.abs(a.value - b.value) < 1e-8;
const display = value => String(Math.round(value * 10000) / 10000);

/** Compare resolved paragraph values without materializing inherited formatting. */
export function paragraphSpacingSummary(selected) {
    const paragraphs = selected.flatMap(object => object.text?.paragraphs?.length ? object.text.paragraphs : [{}]);
    return Object.fromEntries(keys.map(key => {
        const values = paragraphs.map(paragraph => key === 'lineHeight' && paragraph[key] === null ? null : paragraph[key] ?? defaults(key));
        return [key, values.length && values.every(value => value && values[0] && equal(value, values[0])) ? { ...values[0] } : null];
    }));
}

export function createParagraphSpacingTools({ root, canvas, notice }) {
    let targets = [], enabled = false, dialog = null, dialogTargets = '';
    const control = new Menu({ name: 'paragraph-spacing', text: '', icon: 515, state: State.DISPLAY,
        tooltip: 'Line and paragraph spacing', accessibleName: 'Line and paragraph spacing', enabled: false });
    const applySpacing = async spacing => { try { await canvas.paragraphSpacing(spacing); } catch (error) { notice(error.message); } };
    const presets = [1, 1.15, 1.5, 2, 2.5, 3].map(value => {
        const item = control.append(new MenuItem({ name: `paragraph-spacing-${String(value).replace('.', '-')}`, text: Number.isInteger(value) ? `${value}.0` : String(value) }));
        item.on(Message.ACTIVATE, () => { if (enabled) void applySpacing({ lineHeight: { kind: 'multiple', value } }); });
        return { item, value };
    });
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; dialogTargets = ''; };
    const options = control.append(new MenuItem({ name: 'paragraph-spacing-options', text: 'Spacing options…' }));
    options.on(Message.ACTIVATE, () => {
        if (!enabled || dialog) return;
        const summary = paragraphSpacingSummary(targets);
        dialogTargets = targets.map(object => object.id).join('\n');
        let mode = summary.lineHeight?.kind ?? null;
        const initialMode = mode;
        const modeControl = new Menu({ name: 'paragraph-line-mode', text: mode === 'exact' ? 'Exactly' : mode === 'multiple' ? 'Multiple' : mode === 'normal' ? 'Normal' : 'Mixed',
            icon: 74, state: State.START_ALIGNED | State.TRAILING_ICON, accessibleName: 'Line spacing mode' });
        const atLabel = new Label({ text: mode === 'exact' ? 'At (pt)' : mode === 'multiple' ? 'At (multiple)' : 'At' });
        const line = new TextField({ name: 'paragraph-line-value', value: summary.lineHeight && mode !== 'normal' ? display(summary.lineHeight.value * (mode === 'exact' ? 72 / 96 : 1)) : '', placeholder: mode === 'normal' ? 'Font metrics' : 'Mixed', enabled: mode !== 'normal' });
        const initialLine = line.getProperty('value');
        const modes = [['multiple', 'Multiple'], ['exact', 'Exactly'], ...(canvas.supportsNormalLineSpacing ? [['normal', 'Normal']] : [])];
        for (const [kind, label] of modes) {
            const item = modeControl.append(new MenuItem({ text: label, name: `paragraph-line-mode-${kind}` }));
            item.on(Message.ACTIVATE, () => {
                if (mode === kind) return;
                mode = kind; modeControl.text = label; atLabel.text = kind === 'exact' ? 'At (pt)' : kind === 'multiple' ? 'At (multiple)' : 'At';
                line.setProperty('value', kind === 'exact' ? '12' : kind === 'multiple' ? '1' : '');
                line.setProperty('enabled', kind !== 'normal'); line.setProperty('placeholder', kind === 'normal' ? 'Font metrics' : 'Mixed');
            });
        }
        const fields = new Map();
        const rows = [['spaceBefore', 'Before (pt)', 'paragraph-before'], ['spaceAfter', 'After (pt)', 'paragraph-after']].map(([key, label, name]) => {
            const current = summary[key];
            // Percentage margins from PPTX cannot be expressed as points without
            // losing their relative semantics. Retain them until explicitly edited.
            const initial = current ? current.kind === 'exact' ? display(current.value * 72 / 96) : `${display(current.value)} lines` : '';
            const field = new TextField({ name, value: initial, placeholder: 'Mixed' });
            fields.set(key, { field, initial });
            return new Row({ text: 'panel:form-control' }, [new Label({ text: label }), field]);
        });
        const error = new Label({ name: 'paragraph-spacing-error', text: '' });
        const apply = new Button({ name: 'apply-paragraph-spacing', text: 'Apply', accent: true });
        const cancel = new Button({ name: 'cancel-paragraph-spacing', text: 'Cancel' });
        cancel.on(Message.ACTIVATE, close);
        const number = (text, maximum) => /^(?:\d+(?:\.\d*)?|\.\d+)$/u.test(text) && Number(text) <= maximum ? Number(text) : null;
        apply.on(Message.ACTIVATE, () => {
            if (!enabled) { close(); return; }
            const spacing = {}, value = line.getProperty('value').trim();
            if (mode !== initialMode || value !== initialLine) {
                if (!mode) { error.text = 'Choose Multiple or Exactly for line spacing.'; return; }
                const amount = mode === 'normal' ? 1 : number(value, mode === 'exact' ? 49152 : 100);
                if (amount === null) { error.text = mode === 'exact' ? 'Enter line spacing from 0 to 49152 points.' : 'Enter a line-spacing multiple from 0 to 100.'; return; }
                spacing.lineHeight = { kind: mode, value: amount * (mode === 'exact' ? 96 / 72 : 1) };
            }
            for (const [key, { field, initial }] of fields) {
                const value = field.getProperty('value').trim();
                if (value === initial) continue;
                const amount = number(value, 49152);
                if (amount === null) { error.text = 'Enter paragraph spacing from 0 to 49152 points.'; return; }
                spacing[key] = { kind: 'exact', value: amount * 96 / 72 };
            }
            close();
            if (Object.keys(spacing).length) void applySpacing(spacing);
        });
        dialog = new Dialog({ name: 'paragraph-spacing-dialog', text: 'Paragraph spacing', modal: true }, [
            new Label({ text: 'All paragraphs in the selected shapes. Unchanged values are preserved.' }),
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Line spacing' }), modeControl]), new Row({ text: 'panel:form-control' }, [atLabel, line]),
            ...rows, error, new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    });
    return Object.freeze({ control, setState({ selected, usable }) {
        targets = selected.filter(object => object.text?.editable);
        enabled = usable && selected.length > 0 && targets.length === selected.length;
        control.setProperty('enabled', enabled);
        const summary = paragraphSpacingSummary(targets);
        presets.forEach(({ item, value }) => item.setProperty('checked', Boolean(summary.lineHeight && equal(summary.lineHeight, { kind: 'multiple', value }))));
        if (dialog && dialogTargets !== targets.map(object => object.id).join('\n')) close();
    } });
}
