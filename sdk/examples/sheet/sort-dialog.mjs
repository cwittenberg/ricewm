import { Button, CheckBox, Column, Container, Dialog, Label, Message, Row, ScrollView, TextField } from '@ricewm/app';
import { checkedSheetCommand, checkedSheetRange } from './command-contract.mjs';

export function sortColumnName(column) {
    let name = '', n = column + 1;
    while (n) { name = String.fromCharCode(65 + (n - 1) % 26) + name; n = Math.floor((n - 1) / 26); }
    return name;
}
export function sortColumnIndex(text) {
    if (typeof text !== 'string' || !/^\$?[A-Z]{1,3}$/iu.test(text.trim())) throw Error('Enter a column letter, such as A or BC.');
    let value = 0;
    for (const letter of text.trim().replace('$', '').toUpperCase()) value = value * 26 + letter.charCodeAt(0) - 64;
    if (value > 16384) throw Error('Sort column is outside the worksheet.');
    return value - 1;
}
export function sortRangeText(range) {
    return `${sortColumnName(range.firstColumn)}${range.firstRow + 1}:${sortColumnName(range.lastColumn)}${range.lastRow + 1}`;
}
export function parseSortRange(text) {
    if (typeof text !== 'string') throw Error('Enter a range such as A1:D20.');
    const match = /^\s*(\$?[A-Z]{1,3})\$?([1-9][0-9]{0,6})(?::(\$?[A-Z]{1,3})\$?([1-9][0-9]{0,6}))?\s*$/iu.exec(text);
    if (!match) throw Error('Enter a range on this sheet, such as A1:D20.');
    return checkedSheetRange({ firstRow: Number(match[2]) - 1, lastRow: Number(match[4] || match[2]) - 1,
        firstColumn: sortColumnIndex(match[1]), lastColumn: sortColumnIndex(match[3] || match[1]) });
}

/** Sorts no cells: this native dialog only describes one bounded engine command. */
export function createSortDialog({ sheetId, range, column = range.firstColumn, direction = 'ascending', header = false,
    isCurrent = () => true, apply, onClose }) {
    const initial = checkedSheetCommand({ type: 'sort-range', sheetId, range, header, keys: [{ column, direction }], caseSensitive: false });
    let closed = false, acting = false, levels = [{ column: sortColumnName(column), direction }];
    const rangeInput = new TextField({ name: 'sort-range', value: sortRangeText(initial.range), focused: true });
    const headings = new CheckBox({ name: 'sort-header', text: 'First row is a header', checked: header });
    const matchCase = new CheckBox({ name: 'sort-case', text: 'Match case', checked: false });
    const rows = new Column({ name: 'sort-keys' });
    const add = new Button({ name: 'sort-add', text: 'Add level' });
    const status = new Label({ name: 'sort-status', text: '' });
    const accept = new Button({ name: 'sort-apply', text: 'Sort', accent: true });
    const cancel = new Button({ name: 'sort-cancel', text: 'Cancel' });
    let rowControls = [];
    const controls = () => [rangeInput, headings, matchCase, add, accept, cancel, ...rowControls];
    const close = () => { if (!closed && !acting) { closed = true; onClose(); } };
    const escape = event => {
        if (event.detail?.pressed && (event.detail.key === 'Escape' || event.detail.keyCode === 9)) { event.preventDefault(); close(); }
    };
    const update = () => {
        for (const control of controls()) control.setProperty('enabled', !acting && !closed);
        add.setProperty('enabled', !acting && !closed && levels.length < 8);
        for (const control of rowControls) if (control.name?.startsWith('sort-remove-')) control.setProperty('enabled', !acting && !closed && levels.length > 1);
    };
    function renderKeys() {
        for (const row of [...rows.children]) rows.remove(row, { destroy: true });
        rowControls = [];
        levels.forEach((level, index) => {
            const input = new TextField({ name: `sort-column-${index}`, value: level.column, accessibleName: `Sort column ${index + 1}` });
            const order = new Button({ name: `sort-direction-${index}`, text: level.direction === 'ascending' ? 'Ascending' : 'Descending' });
            const remove = new Button({ name: `sort-remove-${index}`, text: 'Remove', enabled: levels.length > 1 });
            input.on(Message.VALUE_CHANGED, () => { if (!acting && !closed) level.column = input.getProperty('value'); });
            order.on(Message.ACTIVATE, () => { if (acting || closed) return; level.direction = level.direction === 'ascending' ? 'descending' : 'ascending'; order.text = level.direction === 'ascending' ? 'Ascending' : 'Descending'; });
            remove.on(Message.ACTIVATE, () => { if (acting || closed || levels.length < 2) return; levels.splice(index, 1); renderKeys(); });
            for (const control of [input, order, remove]) { control.on(Message.KEY, escape); rowControls.push(control); }
            rows.append(new Row({ text: 'panel:fixed' }, [new Label({ text: index ? 'Then by column' : 'Sort by column' }),
                new Container({ text: 'panel:fixed:width=96' }, [input]), order, remove]));
        });
        update();
    }
    add.on(Message.ACTIVATE, () => {
        if (acting || closed || levels.length >= 8) return;
        try {
            const selected = parseSortRange(rangeInput.getProperty('value')), used = new Set(levels.map(level => sortColumnIndex(level.column)));
            let next = selected.firstColumn; while (used.has(next) && next <= selected.lastColumn) next += 1;
            if (next > selected.lastColumn) throw Error('Every column in this range already has a sort level.');
            levels.push({ column: sortColumnName(next), direction: 'ascending' }); renderKeys(); status.text = '';
        } catch (error) { status.text = String(error.message || error).slice(0, 240); }
    });
    const dialog = new Dialog({ name: 'sort-dialog', text: 'Sort range', modal: true }, [
        new Label({ text: 'Range on the current sheet (all columns that must stay together)' }), rangeInput,
        new Row({ text: 'panel:fixed' }, [headings, matchCase]), new ScrollView({}, [rows]), add,
        new Label({ text: 'Ascending: numbers/dates, text, logical values, then errors. Blanks stay last in either direction; ties keep their original order. Text order is not locale-specific.' }),
        new Label({ text: 'Clear filters and unhide rows before sorting.' }),
        new Label({ text: 'Cell contents and styles move together within this rectangle. Row heights stay in place. Cells outside the range keep their written references.' }),
        status, new Row({ text: 'panel:actions' }, [cancel, accept]),
    ]);
    for (const control of [dialog, rangeInput, headings, matchCase, add, accept, cancel]) control.on(Message.KEY, escape);
    cancel.on(Message.ACTIVATE, close); dialog.on(Message.DESTROY, () => { closed = true; });
    accept.on(Message.ACTIVATE, async () => {
        if (closed || acting) return;
        let command;
        try {
            if (!isCurrent()) throw Error('Workbook changed. Close and reopen Sort before applying.');
            command = checkedSheetCommand({ type: 'sort-range', sheetId, range: parseSortRange(rangeInput.getProperty('value')),
                header: headings.getProperty('checked'), caseSensitive: matchCase.getProperty('checked'),
                keys: levels.map(level => ({ column: sortColumnIndex(level.column), direction: level.direction })) });
        } catch (error) { status.text = String(error.message || error).slice(0, 240); return; }
        acting = true; update();
        try {
            const applied = await apply(command);
            if (closed) return;
            if (applied) { acting = false; close(); } else status.text = 'The range was not sorted. Your workbook remains open.';
        } catch (error) { if (!closed) status.text = String(error.message || error).slice(0, 240); }
        finally { acting = false; if (!closed) update(); }
    });
    renderKeys(); return dialog;
}
