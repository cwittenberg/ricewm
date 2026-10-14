export {checkedSelectionValidation} from './validation-contract.mjs';
import { checkedDataToolOptions } from './data-tools-contract.mjs';
import { checkedChart, checkedChartBounds } from './chart-contract.mjs';
import { checkedViewport } from './viewport-contract.mjs';
/** Semantic commands use zero-based logical worksheet coordinates, never visible
 * row slots. A backend adapting a one-based engine must translate at its boundary.
 * Cell input is uninterpreted text/formula; calculation belongs to the engine.
 */
export const SHEET_COMMANDS = Object.freeze(['select-validation-value','create-defined-name','update-defined-name','delete-defined-name','select-sheet', 'select-cell', 'select-range', 'edit-cell',
    'merge-cells', 'unmerge-cells', 'fill-range', 'copy-range', 'paste-range', 'clear-range', 'format-range', 'format-cells', 'paint-format', 'apply-cell-style', 'delete-cell-style', 'undo', 'redo',
    'create-sheet', 'rename-sheet', 'delete-sheet', 'duplicate-sheet', 'move-sheet',
    'insert-rows', 'delete-rows', 'insert-columns', 'delete-columns', 'sort-range', 'remove-duplicates', 'text-to-columns', 'resize-rows', 'resize-columns', 'toggle-autofilter', 'fill-down-adjacent', 'create-chart', 'update-chart', 'delete-chart', 'move-chart']);
export const SHEET_INPUT_MAX_BYTES = 16 * 1024;
const fail = message => { throw Object.assign(new TypeError(message), { code: 'INVALID_COMMAND' }); };
// Match the engine's finite decimal point profile without rounding the value.
const fontPoints = value => typeof value === 'string' && value.length <= 64
    && /^[+-]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][+-]?[0-9]+)?$/.test(value)
    && Number.isFinite(Number(value)) && Number(value) >= 1 && Number(value) <= 409;

const exact = (value, keys) => value && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key));
const sheet = value => typeof value === 'string' && value.length > 0 && value.length <= 128;
const coordinate = (value, maximum) => Number.isSafeInteger(value) && value >= 0 && value < maximum;
const cells = range => (range.lastRow - range.firstRow + 1) * (range.lastColumn - range.firstColumn + 1);

export function checkedSheetRange(value) {
    if (!exact(value, ['firstRow', 'lastRow', 'firstColumn', 'lastColumn']) ||
        !coordinate(value.firstRow, 1048576) || !coordinate(value.lastRow, 1048576) ||
        !coordinate(value.firstColumn, 16384) || !coordinate(value.lastColumn, 16384) ||
        value.firstRow > value.lastRow || value.firstColumn > value.lastColumn) fail('Invalid logical Sheet range');
    return Object.freeze({ firstRow: value.firstRow, lastRow: value.lastRow,
        firstColumn: value.firstColumn, lastColumn: value.lastColumn });
}

export function checkedCapabilities(value = []) {
    if (!Array.isArray(value) || value.length > SHEET_COMMANDS.length+1 || new Set(value).size !== value.length ||
        value.some(name => name!=='data-validation'&&!SHEET_COMMANDS.includes(name))) fail('Invalid Sheet command capabilities');
    return Object.freeze([...value]);
}

export function checkedSheetCommand(value) {
    const type = value?.type;
    if (!SHEET_COMMANDS.includes(type)) throw Object.assign(new TypeError('Unsupported Sheet command'), { code: 'UNSUPPORTED_COMMAND' });
    if(type==='select-validation-value'){
        if(!exact(value,['type','sheetId','row','column','ruleIndex','itemIndex'])||!sheet(value.sheetId)||
            !coordinate(value.row,1048576)||!coordinate(value.column,16384)||!coordinate(value.ruleIndex,1024)||!coordinate(value.itemIndex,10000))fail('Invalid validation list choice');
        return Object.freeze({...value});
    }
    if (['create-defined-name','update-defined-name','delete-defined-name'].includes(type)) {
        const keys=['type','name','scopeSheetId',...(type==='delete-defined-name'?[]:['formula']),...(type==='update-defined-name'?['newName','newScopeSheetId']:[])];
        const name=word=>typeof word==='string'&&[...word].length>=1&&[...word].length<=255&&!/[\p{Cc}\ud800-\udfff]/u.test(word);
        const scope=id=>id===null||sheet(id);
        if(!exact(value,keys)||!name(value.name)||!scope(value.scopeSheetId)||
            type!=='delete-defined-name'&&(typeof value.formula!=='string'||!value.formula.length||new TextEncoder().encode(value.formula).length>16384||/[\u0000\ud800-\udfff]/u.test(value.formula))||
            type==='update-defined-name'&&(!name(value.newName)||!scope(value.newScopeSheetId)))fail('Invalid defined-name command');
        return Object.freeze({...value});
    }
    if(['remove-duplicates','text-to-columns'].includes(type)){const {type:_,...options}=value;return Object.freeze({type,...checkedDataToolOptions(options,type,{fail})});}
    if (type === 'create-chart' || type === 'update-chart') {
        if (!exact(value,['type','chart'])) fail('Invalid chart command');
        return Object.freeze({type,chart:checkedChart(value.chart,{draft:type==='create-chart'})});
    }
    if (type === 'delete-chart' || type === 'move-chart') {
        if (!exact(value,['type','id',...(type==='move-chart'?['chartRevision','viewport','bounds']:[])]) || !Number.isSafeInteger(value.id) || value.id < 1 || value.id > 0xffffffff) fail('Invalid chart identity');
        if (type==='delete-chart') return Object.freeze({type,id:value.id});
        if (!Number.isSafeInteger(value.chartRevision) || value.chartRevision < 0) fail('Invalid chart revision');
        return Object.freeze({type,id:value.id,chartRevision:value.chartRevision,viewport:checkedViewport(value.viewport),bounds:checkedChartBounds(value.bounds)});
    }
    if (type === 'undo' || type === 'redo' || type === 'create-sheet') {
        if (!exact(value, ['type'])) fail('Command cannot contain cell arguments');
        return Object.freeze({ type });
    }
    if (type === 'rename-sheet') {
        if (!exact(value, ['type', 'sheetId', 'name']) || !sheet(value.sheetId) || typeof value.name !== 'string' ||
            !value.name.length || [...value.name].length > 31 || /[\p{Cc}\ud800-\udfff\[\]:*?/\\]/u.test(value.name) ||
            value.name.startsWith("'") || value.name.endsWith("'")) fail('Sheet names need 1–31 characters and cannot contain controls, []:*?/\\ or leading/trailing apostrophes.');
        return Object.freeze({ type, sheetId: value.sheetId, name: value.name });
    }
    if (type === 'move-sheet') {
        if (!exact(value, ['type', 'sheetId', 'index']) || !sheet(value.sheetId) || !coordinate(value.index, 256)) fail('Invalid sheet tab position');
        return Object.freeze({ type, sheetId: value.sheetId, index: value.index });
    }
    if (type === 'resize-rows' || type === 'resize-columns') {
        const rows = type === 'resize-rows', first = rows ? 'firstRow' : 'firstColumn', last = rows ? 'lastRow' : 'lastColumn';
        const dimension = rows ? 'heightPt' : 'widthPx', maximum = rows ? 1048576 : 16384;
        if (!exact(value, ['type', 'sheetId', first, last, dimension]) || !sheet(value.sheetId) ||
            !coordinate(value[first], maximum) || !coordinate(value[last], maximum) || value[last] < value[first] ||
            value[last] - value[first] >= 4096 || !Number.isFinite(value[dimension]) || value[dimension] < 1 || value[dimension] > (rows ? 409 : 4096)) fail('Resize requires 1–4096 tracks and a positive size within the supported bounds');
        return Object.freeze({ type, sheetId: value.sheetId, [first]: value[first], [last]: value[last], [dimension]: value[dimension] });
    }
    if (['insert-rows', 'delete-rows', 'insert-columns', 'delete-columns'].includes(type)) {
        const maximum = type.endsWith('rows') ? 1048576 : 16384;
        if (!exact(value, ['type', 'sheetId', 'index', 'count']) || !sheet(value.sheetId) || !coordinate(value.index, maximum) ||
            !Number.isSafeInteger(value.count) || value.count < 1 || value.count > 4096 || value.index + value.count > maximum) fail('Select between 1 and 4096 rows or columns within worksheet bounds');
        return Object.freeze({ type, sheetId: value.sheetId, index: value.index, count: value.count });
    }
    if (['select-sheet', 'delete-sheet', 'duplicate-sheet'].includes(type)) {
        if (!exact(value, ['type', 'sheetId']) || !sheet(value.sheetId)) fail('Invalid Sheet selection command');
        return Object.freeze({ type, sheetId: value.sheetId });
    }
    if (type === 'toggle-autofilter' || type === 'fill-down-adjacent') {
        const hasRange = value.range !== undefined;
        if (!exact(value, ['type','sheetId',...(hasRange?['range']:[])]) || !sheet(value.sheetId) || type === 'fill-down-adjacent' && !hasRange) fail('Invalid worksheet range command');
        return Object.freeze({type,sheetId:value.sheetId,...(hasRange?{range:checkedSheetRange(value.range)}:{})});
    }
    if (type === 'select-range') {
        if (!exact(value, ['type', 'sheetId', 'firstRow', 'lastRow', 'firstColumn', 'lastColumn']) || !sheet(value.sheetId)) fail('Invalid range selection');
        const { type: _, sheetId, ...range } = value;
        return Object.freeze({ type, sheetId, ...checkedSheetRange(range) });
    }
    if (type === 'sort-range') {
        if (!exact(value, ['type', 'sheetId', 'range', 'header', 'keys', 'caseSensitive']) || !sheet(value.sheetId) ||
            typeof value.header !== 'boolean' || typeof value.caseSensitive !== 'boolean' || !Array.isArray(value.keys) ||
            value.keys.length < 1 || value.keys.length > 8) fail('Sort needs a range, header option and 1–8 column keys');
        const range = checkedSheetRange(value.range);
        if (cells(range) > 10000 || range.lastRow - range.firstRow + 1 <= Number(value.header)) fail('Sort range needs data rows and at most 10000 cells');
        const seen = new Set(), keys = value.keys.map(key => {
            if (!exact(key, ['column', 'direction']) || !coordinate(key.column, 16384) || key.column < range.firstColumn ||
                key.column > range.lastColumn || !['ascending', 'descending'].includes(key.direction) || seen.has(key.column)) fail('Choose distinct sort columns inside the range');
            seen.add(key.column); return Object.freeze({ column: key.column, direction: key.direction });
        });
        if (seen.size !== keys.length) fail('Sort keys must not contain missing entries');
        return Object.freeze({ type, sheetId: value.sheetId, range, header: value.header, keys: Object.freeze(keys), caseSensitive: value.caseSensitive });
    }
    if (type === 'fill-range') {
        if (!exact(value, ['type', 'sheetId', 'source', 'target']) || !sheet(value.sheetId)) fail('Invalid range fill');
        const source = checkedSheetRange(value.source), target = checkedSheetRange(value.target);
        if (cells(target) > 10000 || target.firstRow > source.firstRow || target.lastRow < source.lastRow ||
            target.firstColumn > source.firstColumn || target.lastColumn < source.lastColumn ||
            !(target.firstRow === source.firstRow && target.lastRow === source.lastRow ||
                target.firstColumn === source.firstColumn && target.lastColumn === source.lastColumn)) fail('Fill must extend the source along one axis within 10000 cells');
        return Object.freeze({ type, sheetId: value.sheetId, source, target });
    }
    if (type === 'delete-cell-style') {
        if (!exact(value,['type','name']) || typeof value.name !== 'string' || !value.name.length || new TextEncoder().encode(value.name).length > 128 || /[\p{Cc}\ud800-\udfff]/u.test(value.name)) fail('Invalid cell style');
        return Object.freeze({type,name:value.name});
    }
    if (type === 'apply-cell-style') {
        if (!exact(value,['type','sheetId','range','name']) || !sheet(value.sheetId) || typeof value.name !== 'string' || !value.name.length || new TextEncoder().encode(value.name).length > 128 || /[\p{Cc}\ud800-\udfff]/u.test(value.name)) fail('Invalid cell style');
        const range=checkedSheetRange(value.range);if(cells(range)>10000)fail('Style range exceeds 10000 cells');
        return Object.freeze({type,sheetId:value.sheetId,range,name:value.name});
    }
    if (type === 'format-cells') {
        const optional=['changes','borders'].filter(key=>Object.hasOwn(value,key));
        if (!exact(value,['type','sheetId','range',...optional]) || !sheet(value.sheetId))fail('Invalid cell format command');
        const range=checkedSheetRange(value.range),changes=value.changes||[],borders=value.borders||[];
        if(cells(range)>10000 || !Array.isArray(changes) || !Array.isArray(borders) || changes.length>16 || borders.length>10 || !changes.length&&!borders.length)fail('Invalid format transaction bounds');
        const seen=new Set(),checkedChanges=changes.map(change=>{
            if(!exact(change,['path','value']) || seen.has(change.path) || typeof change.value!=='string' || new TextEncoder().encode(change.value).length>128 || /[\p{Cc}\ud800-\udfff]/u.test(change.value))fail('Invalid format property');
            seen.add(change.path);const v=change.value;
            const valid=['font.b','font.i','font.u','font.strike','alignment.wrap_text'].includes(change.path)?['true','false'].includes(v)
                : change.path==='font.size'?fontPoints(v)
                : ['font.color','fill.color'].includes(change.path)?v==='none'||/^#[0-9a-f]{6}$/i.test(v)
                : change.path==='alignment.horizontal'?['general','left','center','right'].includes(v)
                : change.path==='alignment.vertical'?['top','center','bottom'].includes(v)
                : ['font.name','num_fmt'].includes(change.path)&&Boolean(v.trim());
            if(!valid)fail('Unsupported cell format');return Object.freeze({...change});
        });
        const checkedBorders=borders.map(border=>{
            if(!exact(border,['edge','style','color']) || !['all','none','outline','inside','top','right','bottom','left','inside-horizontal','inside-vertical'].includes(border.edge)
                || !['none','thin','medium','thick','double','dotted','mediumDashed','mediumDashDot','mediumDashDotDot','slantDashDot','dashed','dashDot','dashDotDot','hair'].includes(border.style)
                || typeof border.color!=='string' || !/^#[0-9a-f]{6}$/i.test(border.color))fail('Invalid cell border');
            return Object.freeze({...border});
        });
        return Object.freeze({type,sheetId:value.sheetId,range,changes:Object.freeze(checkedChanges),borders:Object.freeze(checkedBorders)});
    }
    if (type === 'paint-format') {
        if (!exact(value, ['type', 'sourceSheetId', 'sourceRange', 'sheetId', 'range']) || !sheet(value.sourceSheetId) || !sheet(value.sheetId)) fail('Invalid Format Painter command');
        const sourceRange = checkedSheetRange(value.sourceRange), range = checkedSheetRange(value.range);
        if (cells(sourceRange) > 10000 || cells(range) > 10000) fail('Format Painter selection exceeds 10000 cells');
        return Object.freeze({ type, sourceSheetId: value.sourceSheetId, sourceRange, sheetId: value.sheetId, range });
    }
    if (['merge-cells', 'unmerge-cells', 'copy-range', 'clear-range', 'format-range'].includes(type)) {
        const formatting = type === 'format-range';
        if (!exact(value, ['type', 'sheetId', 'range', ...(formatting ? ['path', 'value'] : [])]) || !sheet(value.sheetId)) fail('Invalid range command');
        if (formatting && (typeof value.value !== 'string' || !value.value.length || value.value.length > 128 || /[\ud800-\udfff]/u.test(value.value) ||
            !['num_fmt', 'alignment.horizontal', 'font.b', 'font.i', 'font.u', 'font.size'].includes(value.path) ||
            ['font.b', 'font.i', 'font.u'].includes(value.path) && !['true', 'false'].includes(value.value) ||
            value.path === 'font.size' && !fontPoints(value.value) ||
            value.path === 'alignment.horizontal' && !['general', 'left', 'center', 'right'].includes(value.value))) fail('Invalid range format');
        const range = checkedSheetRange(value.range);
        if (cells(range) > (type === 'copy-range' ? 4096 : 10000)) fail('Range exceeds the engine operation limit');
        return Object.freeze({ type, sheetId: value.sheetId, range,
            ...(formatting ? { path: value.path, value: value.value } : {}) });
    }
    if (type === 'paste-range') {
        if (!exact(value, ['type', 'sheetId', 'row', 'column', 'mode']) || !sheet(value.sheetId) ||
            !coordinate(value.row, 1048576) || !coordinate(value.column, 16384) || !['all', 'values', 'formulas'].includes(value.mode)) fail('Invalid range paste');
        return Object.freeze({ type, sheetId: value.sheetId, row: value.row, column: value.column, mode: value.mode });
    }
    const warning=type==='edit-cell'&&Object.hasOwn(value,'allowValidationWarning');
    const fields = ['type', 'sheetId', 'row', 'column', ...(type === 'edit-cell' ? ['input'] : []),...(warning?['allowValidationWarning']:[])];
    if (!exact(value, fields) || !sheet(value.sheetId) || !coordinate(value.row, 1048576) || !coordinate(value.column, 16384)) {
        fail('Cell coordinates must be zero-based logical worksheet indices');
    }
    if (type === 'edit-cell' && (typeof value.input !== 'string' || /[\ud800-\udfff]/u.test(value.input) ||
        new TextEncoder().encode(value.input).byteLength > SHEET_INPUT_MAX_BYTES)) fail('Cell input exceeds native text limits or contains invalid Unicode');
    if(warning&&typeof value.allowValidationWarning!=='boolean')fail('Invalid validation warning choice');
    return Object.freeze({ type, sheetId: value.sheetId, row: value.row, column: value.column,
        ...(type === 'edit-cell' ? { input: value.input } : {}),...(warning?{allowValidationWarning:value.allowValidationWarning}:{}) });
}
