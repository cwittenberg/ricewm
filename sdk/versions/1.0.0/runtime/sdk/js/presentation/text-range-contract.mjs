import {exact, failure} from './contract-values.mjs';
import {validTextStyle} from './authoring-contract.mjs';
import {cellKey} from './table-values.mjs';
import {validParagraphFormat} from './paragraph-format.mjs';

export const TEXT_RANGE_COMMANDS = new Set(['replace-text-range','set-text-range-style','set-paragraph-format']);
const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const xmlText = value => typeof value === 'string' && new TextEncoder().encode(value).length <= 32768 &&
    !/[\u0000-\u0008\u000b-\u001f\u007f\ud800-\udfff\ufffe\uffff]/u.test(value);

/** Revision-pinned UTF-8 ranges. Paragraph and soft breaks remain distinct. */
export function checkedTextRangeCommand(command) {
    const fail = () => {throw failure('INVALID_REQUEST','Invalid presentation text range command');};
    const base = ['type','slide','object','range'];
    if(command?.cell!==undefined){
        if(!exact(command.cell,['row','column'])||!Number.isInteger(command.cell.row)||command.cell.row<0||command.cell.row>=128||
            !Number.isInteger(command.cell.column)||command.cell.column<0||command.cell.column>=64)fail();
        base.push('cell');
    }
    if (!id(command?.slide) || !id(command?.object) || !exact(command.range,['start','end']) ||
        ![command.range.start,command.range.end].every(n=>Number.isSafeInteger(n)&&n>=0&&n<=262144) || command.range.start>command.range.end) fail();
    if(command.type==='set-paragraph-format'){
        if(!exact(command,[...base,'format'])||!validParagraphFormat(command.format))fail();
    } else if (command.type === 'set-text-range-style') {
        if (!exact(command,[...base,'style']) || command.range.start===command.range.end&&Object.keys(command.style||{}).join(',')!=='align' || !validTextStyle(command.style)) fail();
    } else if (command.type === 'replace-text-range') {
        if (!exact(command,[...base,'affinity','insert']) || !['before','after'].includes(command.affinity) ||
            !Array.isArray(command.insert) || command.insert.length>4096) fail();
        let bytes=0;
        for (const token of command.insert) {
            if (token?.kind==='text') {
                if (!(exact(token,['kind','text'])||exact(token,['kind','text','style'])&&validTextStyle(token.style)&&token.style.align===undefined) || !xmlText(token.text) || /[\r\n]/u.test(token.text)) fail();
                bytes+=new TextEncoder().encode(token.text).length;
            } else if (['softBreak','paragraphBreak'].includes(token?.kind)) {
                if (!(exact(token,['kind'])||token.kind==='softBreak'&&exact(token,['kind','style'])&&validTextStyle(token.style)&&token.style.align===undefined)) fail(); bytes++;
            } else fail();
        }
        if (bytes>32768) fail();
    } else fail();
    return structuredClone(command);
}

/** Derived lookup identity only. Commands retain the real object and cell. */
export const textRangeTargetKey=command=>command.cell===undefined?command.object:cellKey(command.object,command.cell.row,command.cell.column);
textRangeTargetKey.controlFactory=Object.freeze({topics:['documents','workflow'],constructor:'textRangeTargetKey({object:"existing table ID",cell:{row:0,column:0}})',methods:'Return the derived lookup key used by inspectPptxTextRanges/inspectOdpTextRanges. Omit cell for ordinary shape text. Source commands retain the actual object and optional zero-based cell:{row,column}; never send the derived cell key as a document object ID. The caller still validates the command, native editability, exact text and revision.'});

export function textInsertTokens(value, {breakKind='paragraphBreak',style=null}={}) {
    if (typeof value!=='string') throw failure('INVALID_REQUEST','Invalid text insertion');
    value=value.replaceAll('\r\n','\n').replaceAll('\r','\n');
    if (!xmlText(value) || !['paragraphBreak','softBreak'].includes(breakKind)||style!==null&&(!validTextStyle(style)||style.align!==undefined)) throw failure('INVALID_REQUEST','Invalid text insertion');
    const result=[];
    for (const [i,text] of value.split('\n').entries()) {
        if (i) result.push({kind:breakKind,...(style&&breakKind==='softBreak'?{style:structuredClone(style)}:{})});
        if (text) result.push({kind:'text',text,...(style?{style:structuredClone(style)}:{})});
    }
    return result;
}
