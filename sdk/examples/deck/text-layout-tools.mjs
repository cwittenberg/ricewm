import { Button, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';

/** Text-body geometry is projected from the source; only explicit choices edit it. */
export function createTextLayoutTools({ root, canvas, notice }) {
    let targets = [], enabled = false, dialog = null;
    const vertical = new Menu({ name: 'text-vertical-alignment', text: '', icon: 190, state: State.DISPLAY,
        tooltip: 'Align text vertically', accessibleName: 'Align text vertically', enabled: false });
    for (const [value, label] of [['top', 'Top'], ['middle', 'Middle'], ['bottom', 'Bottom']]) {
        const item = vertical.append(new MenuItem({ name: `text-vertical-${value}`, text: label }));
        item.on(Message.ACTIVATE, () => { if (enabled) void canvas.textLayout({ vertical: value }); });
    }
    const margins = new Button({ name: 'text-box-margins', text: '', icon: 189, state: State.DISPLAY,
        tooltip: 'Text box margins', accessibleName: 'Text box margins', enabled: false });
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    margins.on(Message.ACTIVATE, () => {
        if (!enabled || dialog) return;
        const fields = new Map(), error = new Label({ name: 'text-margins-error', text: '' });
        const common = key => targets.every(object => object.text.layout[key] === targets[0].text.layout[key]) ? targets[0].text.layout[key] : null;
        const rows = ['left', 'top', 'right', 'bottom'].map(key => {
            const current = common(key);
            const field = new TextField({ name: `text-margin-${key}`, value: current === null ? '' : String(Math.round(current * 72 / 96 * 100) / 100), placeholder: 'Mixed' });
            fields.set(key, { field, initial: field.getProperty('value') });
            return new Row({}, [new Label({ text: `${key[0].toUpperCase() + key.slice(1)} (pt)` }), field]);
        });
        const apply = new Button({ name: 'apply-text-margins', text: 'Apply', accent: true });
        const cancel = new Button({ text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        apply.on(Message.ACTIVATE, () => {
            if (!enabled) { close(); return; }
            const layout = {};
            for (const [key, { field, initial }] of fields) {
                const value = field.getProperty('value').trim();
                if (value === initial) continue;
                if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value) || Number(value) > 750000) {
                    error.text = 'Enter a non-negative margin in points.'; return;
                }
                layout[key] = Number(value) * 96 / 72;
            }
            close();
            if (Object.keys(layout).length) Promise.resolve(canvas.textLayout(layout)).catch(error => notice(error.message));
        });
        dialog = new Dialog({ name: 'text-margins-dialog', text: 'Text box margins', modal: true }, [
            new Label({ text: 'Inset text from each edge. Unchanged or mixed margins are preserved.' }),
            ...rows, error, new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    });
    return Object.freeze({ controls: [vertical, margins], setState({ selected, usable }) {
        targets = selected.filter(object => object.text?.editable && object.text.layout);
        enabled = usable && selected.length > 0 && targets.length === selected.length;
        for (const control of [vertical, margins]) control.setProperty('enabled', enabled);
        for (const item of vertical.children.filter(item => item.name?.startsWith('text-vertical-'))) {
            item.setProperty('checked', targets.length > 0 && targets.every(object => object.text.layout.vertical === item.name.slice('text-vertical-'.length)));
        }
    } });
}
