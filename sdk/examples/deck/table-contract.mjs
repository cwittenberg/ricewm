import { exact, failure } from './contract-values.mjs';
import { checkedStatus } from './status-contract.mjs';
import { validSceneText } from './scene-contract.mjs';
import { validTextStyle, validTextLayout } from './authoring-contract.mjs';
import { bounded, count, index, paint, border } from './table-values.mjs';
const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const color = value => value === null || typeof value === 'string' && /^#[a-f0-9]{6}$/iu.test(value);
const stroke = value => value === null || exact(value,['color','width']) && color(value.color) && value.color !== null && bounded(value.width) && value.width >= 0;
const sides = ['top','right','bottom','left'];
export const TABLE_COMMANDS = new Set(['insert-table','resize-table','set-table-cell-text','set-table-cell-style','set-table-style','insert-table-rows','delete-table-rows','insert-table-columns','delete-table-columns']);
export function checkedTableCommand(command) {
    const bad = () => { throw failure('INVALID_REQUEST','Invalid slide table command'); };
    if (!id(command?.slide)) bad();
    if (command.type === 'insert-table') {
        if (!exact(command,['type','slide','rows','columns','bounds']) || !count(command.rows,128) || !count(command.columns,64) || command.rows * command.columns > 4096 ||
            !exact(command.bounds,['x','y','width','height']) || !Object.values(command.bounds).every(bounded) || command.bounds.width <= 0 || command.bounds.height <= 0) bad();
    } else {
        if (!id(command.object)) bad();
        const base = ['type','slide','object'];
        if (command.type === 'resize-table') {
            const tracks = (values,max) => Array.isArray(values) && count(values.length,max) && values.every(v=>bounded(v)&&v>=1) && values.reduce((sum,v)=>sum+v,0)<=1e6;
            if(!exact(command,[...base,'columnWidths','rowHeights']) || !tracks(command.columnWidths,64) || !tracks(command.rowHeights,128)) bad();
        } else if (command.type === 'set-table-cell-text' || command.type === 'set-table-cell-style') {
            const isText = command.type === 'set-table-cell-text';
            if (!exact(command,[...base,'row','column',isText?'text':'style']) || !index(command.row,128) || !index(command.column,64)) bad();
            if (isText) {
                if (typeof command.text !== 'string' || new TextEncoder().encode(command.text).length > 16384 || /[\ud800-\udfff]/u.test(command.text)) bad();
            } else {
                const style = command.style;
                if (!exact(style,['text','fill','borders','textLayout'],[]) || !Object.keys(style).length ||
                    style.text !== undefined && !validTextStyle(style.text) || style.fill !== undefined && !color(style.fill) ||
                    style.textLayout !== undefined && !validTextLayout(style.textLayout) ||
                    style.borders !== undefined && (!exact(style.borders,sides,[]) || !Object.keys(style.borders).length || !Object.values(style.borders).every(stroke))) bad();
            }
        } else if (command.type === 'set-table-style') {
            const s = command.style;
            if (!exact(command,[...base,'style']) || !exact(s,['preset','headerRow','bandedRows']) || !/^(plain|accent-[1-6])$/u.test(s.preset) || typeof s.headerRow !== 'boolean' || typeof s.bandedRows !== 'boolean') bad();
        } else if (TABLE_COMMANDS.has(command.type)) {
            const maximum = command.type.endsWith('rows') ? 128 : 64;
            if (!exact(command,[...base,'at','count']) || !Number.isSafeInteger(command.at) || command.at < 0 || command.at > maximum || !count(command.count,maximum)) bad();
        } else bad();
    }
    return structuredClone(command);
}
export function tableArguments(args) {
    if (!exact(args,['slide','object','startCell','limit'],['slide','object']) || !id(args.slide) || !id(args.object)) throw failure('INVALID_REQUEST','Invalid table identity');
    const result = { ...args, startCell: args.startCell ?? 0, limit: args.limit ?? 32 };
    if (!index(result.startCell,4096) || !count(result.limit,64)) throw failure('INVALID_REQUEST','Invalid table cell page');
    return result;
}
export function checkedTableReply(response,args,expected) {
    checkedStatus(response); const r = response.result;
    const invalid = () => { throw failure('INVALID_RESPONSE','Invalid bounded table cells'); };
    if (response.revision !== expected || r.slide !== args.slide || r.object !== args.object || !count(r.totalCells,4096) ||
        !Array.isArray(r.cells) || !r.cells.length || r.cells.length > args.limit ||
        (r.nextCell !== null && r.nextCell !== args.startCell + r.cells.length)) invalid();
    for (const cell of r.cells) {
        if (!cell || !index(cell.row,128) || !index(cell.column,64) || !count(cell.rowSpan,128) || !count(cell.columnSpan,64) ||
            typeof cell.covered !== 'boolean' || typeof cell.editable !== 'boolean' || !exact(cell.bounds,['x','y','width','height']) ||
            !Object.values(cell.bounds).every(bounded) || cell.bounds.width < 0 || cell.bounds.height < 0 || !paint(cell.fill) ||
            !exact(cell.borders,sides) || !Object.values(cell.borders).every(border) || !validSceneText(cell.text)) invalid();
    }
    return response;
}
