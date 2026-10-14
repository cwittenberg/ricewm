import { Button, CheckBox, Column, Container, Dialog, Label, List, Table, Menu, MenuItem, Message, Row, ScrollView, Stack, State, TextField } from '@ricewm/app';
import { checkedSheetRange } from './command-contract.mjs';
import { FUNCTION_CATALOG, FUNCTION_CATEGORIES, functionCatalogMatches, shortFunctionDescription } from './function-catalog.mjs';

export function cellAddress(row, column) {
    let name = '', n = column + 1;
    while (n) { name = String.fromCharCode(65 + (n - 1) % 26) + name; n = Math.floor((n - 1) / 26); }
    return `${name}${row + 1}`;
}
export function rangeAddress(range) {
    const first = cellAddress(range.firstRow, range.firstColumn), last = cellAddress(range.lastRow, range.lastColumn);
    return first === last ? first : `${first}:${last}`;
}
export function selectionRange(projection) {
    if (projection?.selection.range) return projection.selection.range;
    const active = projection?.frame?.active;
    return active ? { firstRow: active[0], lastRow: active[0], firstColumn: active[1], lastColumn: active[1] } : null;
}
export function betweenCells(first, last) {
    return checkedSheetRange({ firstRow: Math.min(first.row, last.row), lastRow: Math.max(first.row, last.row),
        firstColumn: Math.min(first.column, last.column), lastColumn: Math.max(first.column, last.column) });
}
export function fillSelection(projection, direction) {
    const target = selectionRange(projection);
    if (!target) return null;
    if (direction === 'down' && target.firstRow === target.lastRow || direction === 'right' && target.firstColumn === target.lastColumn) return null;
    const source = { ...target, ...(direction === 'down' ? { lastRow: target.firstRow } : { lastColumn: target.firstColumn }) };
    return { type: 'fill-range', sheetId: projection.activeSheetId, source, target };
}
export function autoSumCommand(projection) {
    const range = selectionRange(projection), active = projection?.frame?.active;
    if (!range || !active) return null;
    let target, source;
    if (range.firstRow !== range.lastRow || range.firstColumn !== range.lastColumn) {
        if (range.lastRow === 1048575) return null;
        target = { row: range.lastRow + 1, column: range.firstColumn }; source = range;
    } else if (active[0] > 0) {
        target = { row: active[0], column: active[1] };
        source = { firstRow: 0, lastRow: active[0] - 1, firstColumn: active[1], lastColumn: active[1] };
    } else if (active[1] > 0) {
        target = { row: active[0], column: active[1] };
        source = { firstRow: active[0], lastRow: active[0], firstColumn: 0, lastColumn: active[1] - 1 };
    } else return null;
    return { type: 'edit-cell', sheetId: projection.activeSheetId, ...target, input: `=SUM(${rangeAddress(source)})` };
}
export function createFunctionDialog({ projection, initial = 'SUM', category = null, apply, onClose }) {
    if (initial === 'VLOOKUP' && category===null) return createVLookupDialog({ projection, apply, onClose });
    const active = projection.frame.active, suggestion = autoSumCommand(projection);
    const initialMatches=functionCatalogMatches('',category);
    let selected = initialMatches.find(value => value.name === initial) || initialMatches[0], acting = false, closed = false;
    let matches = [];
    const search = new TextField({ name: 'function-search', value: '', focused: true, accessibleName: 'Search function names, categories and descriptions' });
    const categoryMenu=new Menu({name:'function-category',text:category||'All functions',accessibleName:'Function category',state:State.START_ALIGNED|State.TRAILING_ICON});
    const results = new Table({ name: 'function-results',columnWidths:[260,740] });
    const resultsScroll=new ScrollView({name:'function-results-scroll',text:'viewport:210|0/0',state:State.EXPANSIVE},[results]);
    const count = new Label({ name: 'function-count', text: '' });
    const args = new TextField({ name: 'function-arguments', value: '' });
    const formula = new TextField({ name: 'function-formula', value: '', readOnly: true });
    const destination = new TextField({ name: 'function-destination', value: cellAddress(suggestion?.row ?? active[0], suggestion?.column ?? active[1]) });
    const help = new Label({ name: 'function-help', text: '' }), status = new Label({ name: 'function-status', text: '' });
    const insert = new Button({ name: 'function-insert', text: 'Insert formula' }), cancel = new Button({ name: 'function-cancel', text: 'Cancel' });
    const refresh = () => formula.setProperty('value', `=${selected.name}(${args.getProperty('value')})`);
    const controls = () => [insert, cancel, args, destination, search, categoryMenu, ...results.children.flatMap(row=>row.children)];
    const update = () => {
        for (const control of controls()) control.setProperty('enabled', !acting && !closed);
        insert.setProperty('enabled',!acting&&!closed&&matches.includes(selected));
        for(const row of results.children)for(const button of row.children)button.setProperty('checked',row.functionName===selected.name);
    };
    const focusResult = value => {
        search.setProperty('focused',false);args.setProperty('focused',false);destination.setProperty('focused',false);
        for(const row of results.children)for(const button of row.children)button.setProperty('focused',row.functionName===value.name&&button===row.children[0]);
    };
    const choose = (value,focus=false) => {
        if (acting || closed) return;
        selected = value;
        if(focus)focusResult(value);
        help.text = `${value.category} · ${value.signature}\n${value.description}`;
        const defaults = { IF: 'A1>0,"Yes","No"', VLOOKUP: 'A1,$D$1:$F$100,2,FALSE' };
        args.setProperty('value', defaults[value.name] ??
            (['SUM', 'AVERAGE', 'MIN', 'MAX', 'COUNT'].includes(value.name) ? suggestion?.input.slice(5, -1) || '' : ''));
        refresh(); update();
    };
    const showMatches = () => {
        if (acting || closed) return;
        for (const row of [...results.children]) results.remove(row, { destroy: true });
        try {
            matches = functionCatalogMatches(search.getProperty('value'),category);
            for (const value of matches) {
                const name=new Button({name:`function-item-${value.name.toLowerCase()}`,text:value.name,state:State.START_ALIGNED});
                const description=new Button({name:`function-description-${value.name.toLowerCase()}`,text:shortFunctionDescription(value),state:State.START_ALIGNED});
                const row=new Row({text:'panel:fixed:flush'},[name,description]);row.functionName=value.name;
                for(const button of row.children){button.on(Message.ACTIVATE,()=>choose(value,true));button.on(Message.KEY,event=>{escape(event);if(!event.detail?.pressed||acting||closed)return;const key=event.detail.key||({111:'ArrowUp',116:'ArrowDown',110:'Home',115:'End'})[event.detail.keyCode];if(['ArrowUp','ArrowDown','Home','End'].includes(key)){event.preventDefault();const index=matches.indexOf(value),next=key==='Home'?0:key==='End'?matches.length-1:Math.max(0,Math.min(matches.length-1,index+(key==='ArrowDown'?1:-1)));choose(matches[next],true);const target=results.children[next]?.children[0];if(target){for(const sibling of results.children)for(const control of sibling.children)control.setProperty('focused',control===target);resultsScroll.scrollTo(target);}}});}
                results.append(row);
            }
            count.text = matches.length ? `${matches.length} function${matches.length===1?'':'s'}` : 'No matching functions. Try a name or category.';
        } catch (error) { matches = []; count.text = String(error.message); }
        resultsScroll.setProperty('text','viewport:210|0/0');update();
    };
    const close = () => { if (!acting && !closed) { closed = true; onClose(); } };
    const dialog = new Dialog({ name: 'function-dialog', text: 'Insert function', modal: true }, [
        new Column({text:'panel:flush:tight-stack'},[categoryMenu,new Label({ text: 'Find a function' }), search,
        new Stack({text:'panel:fixed-height:210:flush'},[resultsScroll]),count,help,
        new Column({text:'panel:flush:tight-stack'}, [new Label({ text: 'Arguments (Excel formula syntax)' }), args,
            new Label({ text: 'Formula' }), formula, new Label({ text: 'Destination cell' }), destination]),
        status, new Row({ text: 'panel:actions' }, [cancel, insert])]),
    ]);
    args.on(Message.VALUE_CHANGED, refresh); cancel.on(Message.ACTIVATE, close);
    for(const value of [null,...FUNCTION_CATEGORIES]){
        const item=new MenuItem({name:'function-category-'+(value||'all').toLowerCase().replaceAll(' ','-'),text:value||'All functions',checked:value===category});
        item.on(Message.ACTIVATE,()=>{if(acting||closed)return;category=value;categoryMenu.text=value||'All functions';for(const sibling of categoryMenu.children)sibling.setProperty('checked',sibling===item);showMatches();if(matches.length)choose(matches[0]);});categoryMenu.append(item);
    }
    const escape = event => { if (event.detail?.pressed && (event.detail.key === 'Escape' || event.detail.keyCode === 9)) close(); };
    for (const control of [dialog, args, destination, cancel, insert, search]) control.on(Message.KEY, escape);
    search.on(Message.VALUE_CHANGED, () => {showMatches();if(matches.length)choose(matches[0]);});
    search.on(Message.KEY, event => {
        if (!event.detail?.pressed || acting || closed) return;
        if (event.detail.key === 'Enter' || event.detail.keyCode === 36) { event.preventDefault(); if (matches.length) choose(matches[0]); search.setProperty('focused',false);for(const row of results.children)for(const button of row.children)button.setProperty('focused',false);args.setProperty('focused', true); }
        else if(event.detail.key==='ArrowDown'||event.detail.keyCode===116){event.preventDefault();const target=results.children[0]?.children[0];if(target){choose(matches[0]);search.setProperty('focused',false);target.setProperty('focused',true);resultsScroll.scrollTo(target);}}
    });
    dialog.on(Message.CREATED,()=>{const target=results.children.find(row=>row.functionName===selected.name)?.children[0];if(target)resultsScroll.scrollTo(target);});
    dialog.on(Message.DESTROY, () => { closed = true; });
    insert.on(Message.ACTIVATE, async () => {
        if (acting || closed) return;
        const match = /^\$?([A-Z]{1,3})\$?([1-9][0-9]{0,6})$/iu.exec(destination.getProperty('value').trim());
        if (!match) { status.text = 'Enter a destination cell such as B12.'; return; }
        let column = 0; for (const letter of match[1].toUpperCase()) column = column * 26 + letter.charCodeAt(0) - 64;
        const row = Number(match[2]) - 1; column--;
        if (row >= 1048576 || column >= 16384) { status.text = 'Destination is outside the worksheet.'; return; }
        acting = true; update();
        try {
            const applied = await apply({ type: 'edit-cell', sheetId: projection.activeSheetId, row, column, input: formula.getProperty('value') });
            if (closed) return;
            if (applied) { acting = false; close(); } else status.text = 'The formula was not inserted. Check the formula and try again.';
        } catch (error) { if (!closed) status.text = `Formula insertion failed: ${String(error.message || error).slice(0, 160)}`; }
        finally { acting = false; if (!closed) update(); }
    });
    showMatches(); choose(selected);
    return dialog;
}

/** Four native argument controls for the dedicated lookup command. */
export function createVLookupDialog({ projection, apply, onClose }) {
    const active = projection.frame.active;
    let acting = false, closed = false;
    const lookup = new TextField({ name: 'vlookup-value', value: '', focused: true, accessibleName: 'Lookup value: a cell reference, number or quoted text' });
    const table = new TextField({ name: 'vlookup-table', value: '', accessibleName: 'Table array, for example $D$2:$F$100' });
    const column = new TextField({ name: 'vlookup-column', value: '2', accessibleName: 'Return column number within the table' });
    const approximate = new CheckBox({ name: 'vlookup-approximate', text: 'Approximate match (TRUE)', checked: false });
    const formula = new TextField({ name: 'function-formula', value: '', readOnly: true, accessibleName: 'Formula preview' });
    const destination = new TextField({ name: 'function-destination', value: cellAddress(active[0], active[1]), accessibleName: 'Destination cell' });
    const status = new Label({ name: 'function-status', text: '' });
    const insert = new Button({ name: 'function-insert', text: 'Insert formula', accent: true });
    const cancel = new Button({ name: 'function-cancel', text: 'Cancel' });
    const fields = [lookup, table, column, approximate, destination, insert, cancel];
    const refresh = () => {
        formula.setProperty('value', `=VLOOKUP(${lookup.getProperty('value').trim()},${table.getProperty('value').trim()},${column.getProperty('value').trim()},${approximate.getProperty('checked') ? 'TRUE' : 'FALSE'})`);
        for (const field of fields) field.setProperty('enabled', !acting && !closed);
    };
    const close = () => { if (!acting && !closed) { closed = true; onClose(); } };
    const row = (text, field) => new Column({ text: 'panel:flush:tight-stack' }, [new Label({ text }), field]);
    const body = new Column({text:'panel:flush:tight-stack'}, [
        new Label({ name: 'function-help', text: 'Search the first column; return a value from the same row.' }),
        row('Lookup value', lookup),
        row('Table array', table),
        row('Column index', column),
        approximate,
        new Label({ text: 'Approximate: sort the first column ascending.' }),
        row('Formula', formula), row('Destination cell', destination), status,
    ]);
    const dialog = new Dialog({ name: 'function-dialog', text: 'VLOOKUP arguments', modal: true }, [
        new ScrollView({name:'function-arguments-scroll',text:'viewport:350|0/0',state:State.EXPANSIVE},[body]),
        new Container({text:'panel:fixed:flush'}, [new Row({ text: 'panel:actions' }, [cancel, insert])]),
    ]);
    for (const field of [lookup, table, column]) field.on(Message.VALUE_CHANGED, refresh);
    approximate.on(Message.ACTIVATE, refresh);
    cancel.on(Message.ACTIVATE, close);
    dialog.on(Message.DESTROY, () => { closed = true; });
    const escape = event => { if (event.detail?.pressed && (event.detail.key === 'Escape' || event.detail.keyCode === 9)) { close(); event.preventDefault(); } };
    for (const field of [dialog, ...fields]) field.on(Message.KEY, escape);
    insert.on(Message.ACTIVATE, async () => {
        if (acting || closed) return;
        if (!lookup.getProperty('value').trim()) { status.text = 'Enter the value or cell to look up.'; lookup.setProperty('focused', true); return; }
        if (!table.getProperty('value').trim()) { status.text = 'Enter the table range or named range.'; table.setProperty('focused', true); return; }
        const index = column.getProperty('value').trim();
        if (!/^[1-9][0-9]{0,4}$/u.test(index) || Number(index) > 16384) { status.text = 'Enter a column number from 1 to 16384.'; column.setProperty('focused', true); return; }
        const match = /^\$?([A-Z]{1,3})\$?([1-9][0-9]{0,6})$/iu.exec(destination.getProperty('value').trim());
        let targetColumn = 0;
        if (match) for (const letter of match[1].toUpperCase()) targetColumn = targetColumn * 26 + letter.charCodeAt(0) - 64;
        const targetRow = match ? Number(match[2]) - 1 : -1;
        if (!match || targetRow >= 1048576 || targetColumn > 16384) { status.text = 'Enter a destination cell within the worksheet, such as B12.'; destination.setProperty('focused', true); return; }
        acting = true; refresh();
        try {
            const result = await apply({ type: 'edit-cell', sheetId: projection.activeSheetId, row: targetRow, column: targetColumn - 1, input: formula.getProperty('value') });
            if (!closed) { if (result) { acting = false; close(); } else status.text = 'The formula was not inserted. Check the arguments and try again.'; }
        } catch (error) { if (!closed) status.text = `Formula insertion failed: ${String(error.message || error).slice(0, 160)}`; }
        finally { acting = false; if (!closed) refresh(); }
    });
    refresh(); return dialog;
}
