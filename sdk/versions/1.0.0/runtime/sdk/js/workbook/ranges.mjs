/** Logical single-worksheet references for range fields and automation. Formula
 * parsing and named-reference evaluation remain workbook-engine operations. */
import { checkedSheetRange } from './command-contract.mjs';

function invalid(){throw Object.assign(new TypeError('Enter a valid worksheet range, such as $A$1:$C$20.'),{code:'INVALID_SHEET_RANGE'});}
export function sheetCellAddress(row,column,{absolute=false}={}){
    checkedSheetRange({firstRow:row,lastRow:row,firstColumn:column,lastColumn:column});
    let letters='',number=column+1;
    while(number){letters=String.fromCharCode(65+(number-1)%26)+letters;number=Math.floor((number-1)/26);}
    return `${absolute?'$':''}${letters}${absolute?'$':''}${row+1}`;
}
export function formatSheetRanges(ranges,{absolute=true,equals=false}={}){
    if(!Array.isArray(ranges)||!ranges.length||ranges.length>64)invalid();
    return (equals?'=':'')+ranges.map(value=>{
        const r=checkedSheetRange(value),first=sheetCellAddress(r.firstRow,r.firstColumn,{absolute}),last=sheetCellAddress(r.lastRow,r.lastColumn,{absolute});
        return first===last?first:`${first}:${last}`;
    }).join(',');
}
export function parseSheetRanges(value){
    if(typeof value!=='string'||value.length>8192||/[\u0000-\u001f\ud800-\udfff]/u.test(value))invalid();
    const source=value.trim().replace(/^=/u,'').replace(/\s*:\s*/gu,':');
    if(/(?:^|,)\s*(?:,|$)/u.test(source))invalid();
    const references=source.split(/[\s,]+/u);
    if(!source||references.length>64||references.some(ref=>!ref))invalid();
    const column=letters=>{let n=0;for(const c of letters.toUpperCase())n=n*26+c.charCodeAt(0)-64;return n-1;};
    const cell=ref=>{const match=/^\$?([A-Za-z]{1,3})\$?([1-9]\d{0,6})$/u.exec(ref);return match?{row:Number(match[2])-1,column:column(match[1])}:null;};
    return Object.freeze(references.map(reference=>{
        const parts=reference.split(':');if(parts.length>2||parts.some(part=>!part))invalid();
        const first=cell(parts[0]),last=cell(parts[1]||parts[0]);
        let range;
        if(first&&last)range={firstRow:first.row,lastRow:last.row,firstColumn:first.column,lastColumn:last.column};
        else if(parts.length===2&&parts.every(p=>/^\$?[A-Za-z]{1,3}$/u.test(p)))range={firstRow:0,lastRow:1048575,firstColumn:column(parts[0].replace('$','')),lastColumn:column(parts[1].replace('$',''))};
        else if(parts.length===2&&parts.every(p=>/^\$?[1-9]\d{0,6}$/u.test(p)))range={firstRow:Number(parts[0].replace('$',''))-1,lastRow:Number(parts[1].replace('$',''))-1,firstColumn:0,lastColumn:16383};
        else invalid();
        return checkedSheetRange(range);
    }));
}
