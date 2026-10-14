import { Button, Dialog, Label, Menu, MenuItem, Message, Row, State, TableSizePicker, TextField } from '@ricewm/app';

/** Table creation UI only; the caller supplies one authoritative insert action. */
export function createTableTools({ root, insert, notice = () => {}, icon = 41 }) {
    if (!root || typeof insert !== 'function') throw new TypeError('Table tools require a native root and insert callback');
    let enabled = false, dialog = null, inserting = false;
    const picker = new TableSizePicker();
    const more = new MenuItem({ name: 'insert-table-options', text: 'Insert Table…', icon });
    const control = new Menu({ name: 'insert-table', text: '', icon, state: State.DISPLAY,
        accessibleName: 'Table', tooltip: 'Table', enabled: false }, [picker, more]);
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    const commit = size => {
        if (!enabled || inserting) return;
        close();
        // Native popup dismissal focuses this menu. Keys already queued for it
        // must reach the shell's pending-text buffer while creation is in flight.
        // Internal availability still rejects a second insertion during that work.
        inserting = true;
        const finish = () => { inserting = false; control.setProperty('enabled', enabled); };
        try { Promise.resolve(insert(size)).catch(error => notice(error.message || 'Could not insert table')).finally(finish); }
        catch (error) { finish(); notice(error.message || 'Could not insert table'); }
    };
    picker.on(Message.VALUE_CHANGED, event => commit(event.detail.value));
    control.on(Message.POINTER, event => { if (event.detail?.kind === 'down') picker.setPreview(null); });
    more.on(Message.ACTIVATE, () => {
        if (!enabled || dialog) return;
        const columns = new TextField({ name: 'table-columns', value: '3', accessibleName: 'Number of columns' });
        const rows = new TextField({ name: 'table-rows', value: '2', accessibleName: 'Number of rows' });
        const error = new Label({ name: 'table-size-error', text: '' });
        const cancel = new Button({ name: 'table-size-cancel', text: 'Cancel' });
        const apply = new Button({ name: 'table-size-insert', text: 'Insert', accent: true, icon });
        const submit = () => {
            const numbers = [rows, columns].map(field => /^\d+$/u.test(field.getProperty('value').trim()) ? Number(field.getProperty('value').trim()) : NaN);
            if (!Number.isSafeInteger(numbers[0]) || numbers[0] < 1 || numbers[0] > 128 || !Number.isSafeInteger(numbers[1]) || numbers[1] < 1 || numbers[1] > 64 || numbers[0] * numbers[1] > 4096) {
                error.text = 'Use 1–128 rows and 1–64 columns, up to 4096 cells.'; return;
            }
            commit({ rows: numbers[0], columns: numbers[1] });
        };
        cancel.on(Message.ACTIVATE, close); apply.on(Message.ACTIVATE, submit);
        dialog = new Dialog({ name: 'insert-table-dialog', text: 'Insert Table', modal: true }, [
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Number of columns' }), columns]),
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Number of rows' }), rows]), error,
            new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        for (const target of [dialog, rows, columns]) target.on(Message.KEY, event => {
            if (!event.detail?.pressed) return;
            if (event.detail.keyCode === 9) { event.preventDefault(); close(); }
            else if ([36, 104].includes(event.detail.keyCode)) { event.preventDefault(); submit(); }
        });
        root.append(dialog); columns.setProperty('focused', true);
    });
    return Object.freeze({ control, picker,
        setEnabled(value) { enabled = Boolean(value); control.setProperty('enabled', enabled || inserting); if (!enabled) close(); },
        close,
    });
}
