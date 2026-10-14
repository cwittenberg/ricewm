import { Button, Dialog, Label, Message, Row, TextField } from '@ricewm/app';
import { checkedSheetCommand } from './command-contract.mjs';
import { selectionRange } from './editor-actions.mjs';

/** Native presentation helpers only: the engine owns sheets, references and undo. */
export function structureCommand(projection, action, value) {
    if (!projection) return null;
    const sheetId = projection.activeSheetId, index = projection.sheets.findIndex(sheet => sheet.id === sheetId);
    if (action === 'create-sheet') return { type: action };
    if (action === 'rename-sheet') return checkedSheetCommand({ type: action, sheetId, name: value });
    if (action === 'duplicate-sheet' || action === 'delete-sheet') return { type: action, sheetId };
    if (action === 'move-sheet' || action === 'move-sheet-left' || action === 'move-sheet-right') {
        const destination = action === 'move-sheet' ? Number(value) - 1 : index + (action.endsWith('left') ? -1 : 1);
        return Number.isSafeInteger(destination) && destination >= 0 && destination < projection.sheets.length
            ? { type: 'move-sheet', sheetId, index: destination } : null;
    }
    const range = selectionRange(projection);
    if (!range || !['insert-rows', 'delete-rows', 'insert-columns', 'delete-columns'].includes(action)) return null;
    const rows = action.endsWith('rows'), first = rows ? range.firstRow : range.firstColumn,
        last = rows ? range.lastRow : range.lastColumn;
    return checkedSheetCommand({ type: action, sheetId, index: first, count: last - first + 1 });
}

export function createStructureDialog({ projection, action, apply, onClose }) {
    const current = projection.sheets.find(sheet => sheet.id === projection.activeSheetId);
    const renaming = action === 'rename-sheet', moving = action === 'move-sheet';
    const rows = action === 'delete-rows', columns = action === 'delete-columns';
    const title = renaming ? 'Rename sheet' : moving ? 'Move sheet' : rows ? 'Delete rows' : columns ? 'Delete columns' : 'Delete sheet';
    const command = renaming || moving ? null : structureCommand(projection, action);
    const input = renaming || moving ? new TextField({ name: 'structure-value', focused: true,
        value: renaming ? current.name : String(projection.sheets.findIndex(sheet => sheet.id === current.id) + 1) }) : null;
    const explanation = renaming ? 'Sheet name (1–31 characters)' : moving ? `Tab position (1–${projection.sheets.length})` : rows || columns
        ? `Delete ${command.count} ${rows ? 'row' : 'column'}${command.count === 1 ? '' : 's'} starting at ${command.index + 1} from “${current.name}”? This includes hidden rows or columns. Cells and formulas will shift. You can undo this operation.`
        : `Delete “${current.name}” and its contents? Formulas referencing this sheet may become #REF!. You can undo this operation.`;
    const status = new Label({ name: 'structure-status', text: '' });
    const accept = new Button({ name: 'structure-apply', text: renaming ? 'Rename' : moving ? 'Move' : 'Delete' });
    const cancel = new Button({ name: 'structure-cancel', text: 'Cancel', focused: !input });
    let acting = false, closed = false;
    const close = () => { if (!acting && !closed) { closed = true; onClose(); } };
    const dialog = new Dialog({ name: 'structure-dialog', text: title, modal: true }, [
        new Label({ text: explanation }), ...(input ? [input] : []), status,
        new Row({ text: 'panel:actions' }, [cancel, accept]),
    ]);
    const controls = [cancel, accept, ...(input ? [input] : [])];
    async function submit() {
        if (acting || closed) return;
        let checked;
        try {
            const value = input?.getProperty('value');
            if (moving && !/^[1-9][0-9]{0,2}$/u.test(value)) throw Error('Enter a tab position within this workbook.');
            checked = command || structureCommand(projection, action, value);
            if (!checked) throw Error('Enter a tab position within this workbook.');
            checked = checkedSheetCommand(checked);
        } catch (error) { status.text = String(error.message || error).slice(0, 240); return; }
        acting = true; for (const control of controls) control.setProperty('enabled', false);
        try {
            const applied = await apply(checked);
            if (closed) return;
            if (applied) { acting = false; close(); }
            else status.text = 'The operation did not complete. Your workbook remains open.';
        } catch (error) { if (!closed) status.text = String(error.message || error).slice(0, 240); }
        finally { acting = false; if (!closed) for (const control of controls) control.setProperty('enabled', true); }
    }
    accept.on(Message.ACTIVATE, submit); cancel.on(Message.ACTIVATE, close);
    for (const control of [dialog, ...controls]) control.on(Message.KEY, event => {
        if (!event.detail?.pressed) return;
        if (event.detail.key === 'Escape' || event.detail.keyCode === 9) { event.preventDefault(); close(); }
        // Destructive confirmation defaults to Cancel; Enter in the naming field
        // is convenient, but an arbitrary Enter never deletes a worksheet.
        if (control === input && (event.detail.key === 'Enter' || event.detail.keyCode === 36)) { event.preventDefault(); void submit(); }
    });
    dialog.on(Message.DESTROY, () => { closed = true; });
    return dialog;
}
