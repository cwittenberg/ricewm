/** Native presentation of engine-owned distinct/search pages. These components
 * never scan worksheet cells, search local result arrays, or sort/filter data.
 * Owners append controller.dialog and remove it in onClose. Destroy also retires
 * pending responses. Providers own calculation/query cancellation and revisions.
 */
import { Button, CheckBox, Column, Container, Dialog, Label, List, Menu, MenuItem, Message, Popover, Row, ScrollView, State, TextField } from '@ricewm/app';

const LIMIT = 64, MAX_SELECTION = 256, MAX_BYTES = 32768;
const encoder = new TextEncoder();
const integer = value => Number.isSafeInteger(value) && value >= 0;
const object = value => value && typeof value === 'object' && !Array.isArray(value);
const plainText = value => typeof value === 'string' && !/[\ud800-\udfff]/u.test(value);
const length = value => encoder.encode(JSON.stringify(value)).length;
const label = (value, maximum = 160) => Array.from(String(value).replace(/[\u0000-\u001f\u007f]/g, ' ')).slice(0, maximum).join('');
const failure = message => { throw new TypeError(message); };

function typedValue(value) {
    if (!object(value) || !['blank', 'number', 'text', 'boolean', 'error'].includes(value.kind)) failure('Invalid distinct value');
    const hasValue = value.kind !== 'blank';
    if (Object.keys(value).length !== (hasValue ? 2 : 1) || (hasValue && !Object.hasOwn(value, 'value'))) failure('Invalid distinct value fields');
    if (value.kind === 'blank') return Object.freeze({ kind: 'blank' });
    if (value.kind === 'number' && (typeof value.value !== 'number' || !Number.isFinite(value.value))) failure('Invalid numeric filter value');
    if (value.kind === 'boolean' && typeof value.value !== 'boolean') failure('Invalid boolean filter value');
    if (['text', 'error'].includes(value.kind) && (!plainText(value.value) || encoder.encode(value.value).length > 16384)) failure('Filter value exceeds text limits');
    return Object.freeze({ kind: value.kind, value: Object.is(value.value, -0) ? 0 : value.value });
}
const identity = value => JSON.stringify(value);
function valueLabel(value) {
    if (value.kind === 'blank') return '(Blanks)';
    if (value.kind === 'text') return value.value === '' ? '(Empty text)' : `“${label(value.value)}”`;
    if (value.kind === 'boolean') return value.value ? 'TRUE' : 'FALSE';
    return label(value.value);
}
function selectedCriterion(values, displayValues = false) {
    if (values.length > MAX_SELECTION) failure('Choose at most 256 values');
    if (displayValues && values.some(value => !['text', 'blank'].includes(value.kind))) failure('Displayed filters require text labels or blanks');
    const criterion = displayValues ? { type: 'displayValues', values: values.filter(value => value.kind === 'text').map(value => value.value), blank: values.some(value => value.kind === 'blank') } : { type: 'selected', values };
    if (length(criterion) >= MAX_BYTES) failure('Selected values exceed the 32 KiB filter limit');
    return criterion;
}
function pageResult(value, requestedOffset, entry) {
    const forwardOnly = value?.forwardOnly === true, finalTotal = value?.finalTotal !== false;
    if (!object(value) || !Array.isArray(value.entries) || value.entries.length > LIMIT ||
        !integer(value.total) || !integer(value.revision) ||
        (value.offset !== undefined && value.offset !== requestedOffset) || requestedOffset > value.total ||
        value.entries.length > value.total - requestedOffset ||
        !(value.next === null || (integer(value.next) && value.next > requestedOffset && value.next <= value.total)) ||
        (!forwardOnly && !finalTotal) || (value.next === null && !finalTotal) ||
        (value.next === null && requestedOffset + value.entries.length !== value.total) ||
        (value.next !== null && value.next !== requestedOffset + value.entries.length) ||
        (value.next !== null && value.entries.length === 0) || length(value) > MAX_BYTES) failure('Invalid or oversized engine result page');
    return Object.freeze({ offset: requestedOffset, next: value.next, total: value.total, revision: value.revision, forwardOnly, finalTotal,
        entries: Object.freeze(value.entries.map(entry)) });
}
function distinctPage(value, offset) {
    const seen = new Set();
    return pageResult(value, offset, entry => {
        if (!object(entry) || !integer(entry.count) || entry.count < 1) failure('Invalid distinct count');
        const value = typedValue(entry.value), key = identity(value);
        if (seen.has(key)) failure('Duplicate typed value in engine page');
        seen.add(key);
        return Object.freeze({ value, count: entry.count });
    });
}
function findPage(value, offset) {
    return pageResult(value, offset, entry => {
        if (!object(entry) || !plainText(entry.sheetId) || !entry.sheetId.length || entry.sheetId.length > 128 ||
            !integer(entry.row) || entry.row >= 1048576 || !integer(entry.column) || entry.column >= 16384 ||
            !plainText(entry.text) || encoder.encode(entry.text).length > 16384) failure('Invalid engine search result');
        return Object.freeze({ sheetId: entry.sheetId, row: entry.row, column: entry.column, text: entry.text });
    });
}

/** Shared query lifecycle (a bounded presentation controller, not a data model).
 * New searches supersede earlier pages; a changed revision during pagination
 * requests an explicit refresh instead of silently mixing different workbooks.
 */
function pagedDialog({ prefix, title, query, validate, makeRequest, render, onClose, options = [], allowEmpty = true, anchor = null }) {
    if (typeof query !== 'function' || typeof onClose !== 'function') failure('Dialog needs query and onClose callbacks');
    let closed = false, loading = false, acting = false, serial = 0, timer = null, page = null, revision = null;
    let history = [], changed = () => {};
    const search = new TextField({ name: `${prefix}-search`, value: '', focused: Boolean(anchor), accessibleName: 'Search text' });
    const submit = new Button({ name: `${prefix}-search-submit`, text: 'Search' });
    const status = new Label({ name: `${prefix}-status`, text: 'Ready' });
    const rows = new List({ name: `${prefix}-results` });
    const previous = new Button({ name: `${prefix}-previous`, text: 'Back', enabled: false });
    const next = new Button({ name: `${prefix}-next`, text: 'Next', enabled: false });
    const cancel = new Button({ name: `${prefix}-cancel`, text: 'Cancel' });
    const actions = new Row({ text: 'panel:actions' }, [previous, next, cancel]);
    const popup = anchor && Number.isInteger(anchor.x) && Number.isInteger(anchor.y) && anchor.x >= 0 && anchor.y >= 0 && anchor.x <= 65536 && anchor.y <= 65536;
    const Surface = popup ? Popover : Dialog;
    const titleLabel=popup?new Label({text:title}):null;
    const dialog = new Surface({ name: `${prefix}-dialog`, text: popup ? `panel:anchored-popup:${anchor.x}:${anchor.y}:360:520` : title, ...(popup ? {} : {open: true, modal: true}) }, [
        ...(titleLabel ? [titleLabel] : []),
        new Row({ text: 'panel:fixed:flush' }, [search, submit]),
        ...(options.length ? [new Column({ text: 'panel:fixed:flush:tight-stack' }, options)] : []),
        new ScrollView({text:popup?'0/0':null,...(popup?{state:State.EXPANSIVE}:{})}, [rows]), new Container({text:'panel:fixed:flush'},[new Column({text:'panel:fixed:flush:tight-stack'},[status,actions])]),
    ]);
    function clearRows() { for (const child of [...rows.children]) rows.remove(child, { destroy: true }); }
    function update() {
        search.setProperty('enabled', !closed && !acting); submit.setProperty('enabled', !closed && !acting);
        for (const option of options) option.setProperty('enabled', !closed && !acting);
        previous.setProperty('enabled', !closed && !loading && !acting && history.length > 0);
        next.setProperty('enabled', !closed && !loading && !acting && page?.next !== null && page !== null);
        cancel.setProperty('enabled', !closed && !acting);
        for (const row of rows.children) row.setProperty('enabled', !closed && !loading && !acting);
        dialog.setProperty('busy', loading || acting); changed();
    }
    function retire() { if (closed) return false; closed = true; serial += 1; clearTimeout(timer); timer = null; return true; }
    function close() { if (retire()) onClose(); }
    dialog.on(Message.DESTROY, retire);
    dialog.on(Message.POINTER, event => { if (event.detail?.kind === 'cancel' && !acting) close(); });
    cancel.on(Message.ACTIVATE, () => { if (!acting) close(); });
    dialog.on(Message.KEY, event => {
        if (event.detail?.pressed && (event.detail.key === 'Escape' || event.detail.keyCode === 9) && !acting) close();
    });
    async function run(offset = 0, direction = 'reset') {
        if (closed || acting) return;
        clearTimeout(timer); timer = null;
        const mine = ++serial, oldPage = page;
        loading = true;
        if (direction === 'reset') { history = []; revision = null; page = null; clearRows(); }
        status.text = 'Searching…'; update();
        try {
            const literal = search.getProperty('value');
            if (!plainText(literal) || encoder.encode(literal).length > 1024) failure('Search text must be valid Unicode and at most 1 KiB');
            if (!allowEmpty && !literal.length) { page = null; clearRows(); status.text = 'Enter text to find in this workbook.'; return; }
            const requested = makeRequest({ offset, limit: LIMIT, literal });
            const response = validate(await query(requested), offset);
            if (closed || mine !== serial) return;
            if (direction !== 'reset' && response.revision !== revision) failure('Workbook changed. Search again to refresh the results.');
            if (direction === 'next' && oldPage && !response.forwardOnly) history.push(oldPage.offset);
            if (direction === 'previous') history.pop();
            page = response; revision = response.revision;
            clearRows(); render(response.entries, rows, controller);
            status.text = response.total ? (response.finalTotal ? `${offset + 1}–${offset + response.entries.length} of ${response.total}`
                : `${offset + 1}–${offset + response.entries.length} · ${response.total} found so far`) : 'No matching values or results.';
        } catch (cause) {
            if (closed || mine !== serial) return;
            page = null; history = []; revision = null; clearRows(); status.text = `Search failed: ${label(cause?.message || cause, 240)}`;
        } finally { if (!closed && mine === serial) { loading = false; update(); } }
    }
    function refresh() { return run(); }
    search.on(Message.VALUE_CHANGED, () => {
        if (closed || acting) return;
        serial += 1; loading = true; page = null; history = []; clearRows(); clearTimeout(timer);
        status.text = 'Waiting for search…'; update(); timer = setTimeout(refresh, 150);
    });
    search.on(Message.KEY, event => { if (event.detail?.pressed && (event.detail.key === 'Enter' || event.detail.keyCode === 36)) void refresh(); });
    submit.on(Message.ACTIVATE, () => { void refresh(); });
    previous.on(Message.ACTIVATE, () => { if (!loading && !acting && history.length) void run(history.at(-1), 'previous'); });
    next.on(Message.ACTIVATE, () => { if (!loading && !acting && page?.next !== null && page) void run(page.next, 'next'); });
    const controller = {
        dialog, actions, rows, titleLabel, refresh, close,
        get busy() { return closed || loading || acting; },
        get closed() { return closed; },
        onChange(callback) { changed = callback; update(); },
        report(message) { status.text = label(message, 240); },
        async action(callback) {
            if (closed || loading || acting) return;
            acting = true; serial += 1; clearTimeout(timer); update();
            // Cancel stays disabled once mutation/selection is submitted. Closing
            // the owning window retires UI replies; it cannot undo an engine commit.
            try { await callback(); if (!closed) close(); }
            catch (cause) { if (!closed) status.text = `Request failed: ${label(cause?.message || cause, 240)}`; }
            finally { if (!closed) { acting = false; update(); } }
        },
    };
    queueMicrotask(refresh);
    return controller;
}

export function createFilterValuesDialog({ query, apply, onClose, selected = null, excluded = null,
    criterion: restoredCriterion = null, supportsExclusion = false, typedFilters = false,
    title = 'Filter values', sort = null, displayValues = false, anchor = null } = {}) {
    if (typeof apply !== 'function' || (selected !== null && !Array.isArray(selected)) || (excluded !== null && !Array.isArray(excluded))) failure('Invalid filter dialog configuration');
    if ((selected || excluded)?.length > MAX_SELECTION) failure('Choose at most 256 values');
    const exclusionAvailable = supportsExclusion && displayValues;
    const keyFor = value => displayValues && value.kind === 'text' ? JSON.stringify({ kind: 'text', value: value.value.toLowerCase() }) : identity(value);
    const criterion = values => selectedCriterion(values, displayValues);
    const selection = new Map(); let all = selected === null && excluded === null, dirty = false;
    let exclude = exclusionAvailable && selected === null;
    let condition = typedFilters && restoredCriterion && !['displayValues','displayExcluded'].includes(restoredCriterion.type) ? {...restoredCriterion} : null;
    for (const item of (exclude ? excluded : selected) || []) { const value = typedValue(item); selection.set(keyFor(value), value); }
    criterion([...selection.values()]);
    const summary = new Label({ name: 'filter-selection', text: '' });
    const selectAll = new CheckBox({name:'filter-select-all',text:'(Select All)',checked:exclude && !selection.size,visible:exclusionAvailable});
    const argument = new TextField({name:'filter-condition-value',value:String(condition?.value ?? condition?.literal ?? ''),accessibleName:'Filter condition value'});
    const numberLabels={equal:'Equals number',notEqual:'Does not equal',less:'Less than',lessEqual:'Less than or equal',greater:'Greater than',greaterEqual:'Greater than or equal'};
    const conditionLabel=value=>!value?'Checked values':value.type==='number'?numberLabels[value.comparison]:value.type==='text'?(value.exact?'Equals text':'Contains text'):(value.blank?'Blanks':'Nonblanks');
    const conditionMenu = new Menu({name:'filter-condition',text:conditionLabel(condition),icon:162});
    const conditionRow = new Row({text:'panel:fixed:flush',visible:typedFilters},[conditionMenu,argument]);
    let applyButton, clear, controller; const sorting = [];
    function chooseCondition(value, text) {
        condition=value;conditionMenu.text=text;dirty=true;
        argument.setProperty('value',String(value?.value ?? value?.literal ?? ''));
        argument.setProperty('visible',Boolean(value && value.type!=='blank'));describe();
    }
    if(typedFilters) {
        const item=(name,text,value)=>{const control=new MenuItem({name,text});control.on(Message.ACTIVATE,()=>chooseCondition(value,text));conditionMenu.append(control);};
        item('filter-condition-values','Checked values',null);
        for(const [comparison,text] of [['equal','Equals number'],['notEqual','Does not equal'],['less','Less than'],['lessEqual','Less than or equal'],['greater','Greater than'],['greaterEqual','Greater than or equal']])
            item(`filter-number-${comparison}`,text,{type:'number',comparison,value:0});
        item('filter-text-contains','Contains text',{type:'text',literal:'',caseSensitive:false,exact:false});
        item('filter-text-equals','Equals text',{type:'text',literal:'',caseSensitive:false,exact:true});
        item('filter-only-blanks','Blanks',{type:'blank',blank:true});item('filter-not-blanks','Nonblanks',{type:'blank',blank:false});
    }
    argument.setProperty('visible',Boolean(condition && condition.type!=='blank'));
    argument.on(Message.VALUE_CHANGED,()=>{dirty=true;describe();});
    controller = pagedDialog({ prefix: 'filter', title, query, onClose, anchor,
        options: [conditionRow, selectAll, summary], validate: distinctPage, makeRequest: request => request,
        render(entries, rows, pager) {
            for (const [index, entry] of entries.entries()) {
                if (displayValues && !['text', 'blank'].includes(entry.value.kind)) failure('Engine returned a typed value in displayed filter mode');
                const key = keyFor(entry.value), check = new CheckBox({ name: `filter-value-${index}`,
                    text: `${displayValues && entry.value.kind === 'text' ? (entry.value.value === '' ? '(Empty display)' : label(entry.value.value)) : valueLabel(entry.value)} (${entry.count})`,
                    checked: exclude ? !selection.has(key) : selection.has(key) });
                check.on(Message.ACTIVATE, () => {
                    if (pager.busy || condition) return;
                    const wasChecked = exclude ? !selection.has(key) : selection.has(key), wanted = check.getProperty('checked');
                    const next = new Map(selection);
                    if (exclude ? !wanted : wanted) next.set(key, entry.value); else next.delete(key);
                    try { criterion([...next.values()]); }
                    catch (cause) { check.setProperty('checked', wasChecked); pager.report(cause.message); return; }
                    all = false; dirty=true;selection.clear(); for (const [key, value] of next) selection.set(key, value);
                    describe();
                });
                rows.append(check);
            }
        },
    });
    function describe() {
        if(!controller)return;
        summary.text = condition ? 'Filter by the selected condition.' : all ? 'All values are selected.' : exclude ? `All except ${selection.size} values` : `${selection.size} values selected${selection.size ? '' : ' — applying will hide every row'}`;
        applyButton?.setProperty('enabled', !controller.busy && (dirty || !all || Boolean(condition)));
        clear?.setProperty('enabled', !controller.busy);
        selectAll.setProperty('checked',exclude&&!selection.size&&!condition);
        selectAll.setProperty('state',(selectAll.getProperty('state')&~State.INDETERMINATE)|(!condition&&selection.size>0?State.INDETERMINATE:0));
        conditionMenu.text=conditionLabel(condition);
        selectAll.setProperty('enabled',!controller.busy&&!condition);
        controller.rows.setProperty('enabled',!controller.busy&&!condition);
        argument.setProperty('enabled',!controller.busy);
        for (const button of sorting) button.setProperty('enabled', !controller.busy);
    }
    selectAll.on(Message.ACTIVATE,()=>{
        if(controller.busy)return;
        exclude=selectAll.getProperty('checked');selection.clear();all=exclude;dirty=true;condition=null;
        for(const row of controller.rows.children)row.setProperty('checked',exclude);describe();
    });
    clear = new Button({ name: 'filter-all', text: 'Clear filter', icon:163 });
    applyButton = new Button({ name: 'filter-apply', text: 'OK', accent: true, enabled: false });
    clear.on(Message.ACTIVATE, () => { void controller.action(() => apply(null)); });
    applyButton.on(Message.ACTIVATE, () => { void controller.action(() => {
        let predicate;
        if(condition) {
            if(condition.type==='number') {const value=Number(argument.getProperty('value'));if(!argument.getProperty('value').trim()||!Number.isFinite(value))failure('Enter a valid number');predicate={...condition,value};}
            else if(condition.type==='text')predicate={...condition,literal:argument.getProperty('value')};
            else predicate=condition;
        } else if(all || exclude&&!selection.size) predicate=null;
        else {predicate=criterion([...selection.values()]);if(exclude)predicate={...predicate,type:'displayExcluded'};}
        return apply(predicate);
    }); });
    const cancel = controller.actions.children.at(-1);
    controller.actions.remove(cancel); controller.actions.append(applyButton); controller.actions.append(cancel);
    const headerActions=[];
    if (typeof sort === 'function') {
        for (const direction of ['ascending', 'descending']) {
            const button = new Button({ name: `filter-sort-${direction}`, text: direction==='ascending'?'Sort A to Z…':'Sort Z to A…',icon:direction==='ascending'?164:165, enabled: false });
            button.on(Message.ACTIVATE, () => {
                if (controller.busy) return;
                void Promise.resolve().then(() => sort(direction)).catch(error => { if (!controller.closed) controller.report(String(error.message || error)); });
            });
            sorting.push(button);
        }
    }
    if(sorting.length)headerActions.push(new Row({text:'panel:fixed:flush'},sorting));
    if(controller.titleLabel)controller.dialog.remove(controller.titleLabel);
    headerActions.push(new Row({text:'panel:fixed:flush'},[...(controller.titleLabel?[controller.titleLabel]:[]),clear]));
    const contents=[...controller.dialog.children];for(const child of contents)controller.dialog.remove(child);
    controller.dialog.append(new Column({text:'panel:fixed:flush:tight-stack'},headerActions));for(const child of contents)controller.dialog.append(child);
    controller.onChange(describe);
    return Object.freeze({ dialog: controller.dialog, refresh: controller.refresh, close: controller.close });
}

export function createFindDialog({ query, select, onClose, title = 'Find in workbook' } = {}) {
    if (typeof select !== 'function') failure('Find dialog needs a selection callback');
    const caseSensitive = new CheckBox({ name: 'find-case', text: 'Match case', checked: false });
    const formulas = new CheckBox({ name: 'find-formulas', text: 'Search formulas', checked: false });
    const controller = pagedDialog({ prefix: 'find', title, query, onClose, allowEmpty: false, options: [caseSensitive, formulas], validate: findPage,
        makeRequest: request => ({ ...request, caseSensitive: caseSensitive.getProperty('checked'), formulas: formulas.getProperty('checked') }),
        render(entries, rows, pager) {
            for (const [index, entry] of entries.entries()) {
                let column = '', n = entry.column + 1;
                while (n) { column = String.fromCharCode(65 + (n - 1) % 26) + column; n = Math.floor((n - 1) / 26); }
                const result = new Button({ name: `find-result-${index}`, text: `${label(entry.sheetId, 64)}!${column}${entry.row + 1} · ${label(entry.text)}` });
                result.on(Message.ACTIVATE, () => { void pager.action(() => select({ sheetId: entry.sheetId, row: entry.row, column: entry.column })); });
                rows.append(result);
            }
        },
    });
    for (const option of [caseSensitive, formulas]) option.on(Message.ACTIVATE, () => { void controller.refresh(); });
    return Object.freeze({ dialog: controller.dialog, refresh: controller.refresh, close: controller.close });
}
