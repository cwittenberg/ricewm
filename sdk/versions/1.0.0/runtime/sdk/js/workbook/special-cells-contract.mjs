/** Transport-only native Go To Special plans. No worksheet matching in JS. */
import {checkedSheetRange} from './command-contract.mjs';
import {checkedQueryResult} from './query-contract.mjs';
const capabilities=Object.freeze({constants:'special-cells',formulas:'special-cells',blanks:'special-cells',visible:'select-special-cells',notes:'special-cell-metadata','conditional-formats':'special-cell-metadata','same-conditional-formats':'special-cell-metadata','data-validation':'special-cell-metadata','same-data-validation':'special-cell-metadata','current-region':'special-cell-geometry','current-array':'special-cell-geometry','last-cell':'special-cell-geometry','row-differences':'special-cell-differences','column-differences':'special-cell-differences',precedents:'special-cell-references','all-precedents':'special-cell-references',dependents:'special-cell-references','all-dependents':'special-cell-references'});
export const SHEET_SPECIAL_CELL_KINDS=Object.freeze(Object.keys(capabilities));
export const sheetSpecialCellKinds=(advertised=[])=>Object.freeze(SHEET_SPECIAL_CELL_KINDS.filter(kind=>advertised.includes(capabilities[kind])));
export const SHEET_SPECIAL_VALUE_TYPES=Object.freeze(['numbers','text','logicals','errors']);
const fail=()=>{throw Object.assign(new TypeError('Invalid native special-cell plan or scope'),{code:'INVALID_SPECIAL_CELLS'});};
const plain=v=>v!==null&&typeof v==='object'&&Object.getPrototypeOf(v)===Object.prototype;
const exact=(v,keys)=>plain(v)&&Object.keys(v).length===keys.length&&keys.every(k=>Object.hasOwn(v,k));
const compare=(a,b)=>a.firstRow-b.firstRow||a.firstColumn-b.firstColumn||a.lastRow-b.lastRow||a.lastColumn-b.lastColumn;
const overlap=(a,b)=>a.firstRow<=b.lastRow&&b.firstRow<=a.lastRow&&a.firstColumn<=b.lastColumn&&b.firstColumn<=a.lastColumn;
function range(value){try{return checkedSheetRange(value);}catch{fail();}}
export function checkedSheetSpecialCellsRequest(value){
    if(!exact(value,['sheetId','ranges','kind','types'])||typeof value.sheetId!=='string'||!value.sheetId.length||value.sheetId.length>128||/[\p{Cc}\ud800-\udfff]/u.test(value.sheetId)
      ||!SHEET_SPECIAL_CELL_KINDS.includes(value.kind)||!Array.isArray(value.ranges)||!value.ranges.length||value.ranges.length>512
      ||!Array.isArray(value.types)||value.types.length>4||value.types.some((v,i)=>!SHEET_SPECIAL_VALUE_TYPES.includes(v)||value.types.indexOf(v)!==i)
      ||(!['constants','formulas'].includes(value.kind))!==(value.types.length===0))fail();
    const ranges=value.ranges.map(range);
    if(['current-region','current-array','last-cell','precedents','all-precedents','dependents','all-dependents'].includes(value.kind)&&(ranges.length!==1||ranges[0].firstRow!==0||ranges[0].lastRow!==1048575||ranges[0].firstColumn!==0||ranges[0].lastColumn!==16383))fail();
    return Object.freeze({sheetId:value.sheetId,ranges:Object.freeze(ranges),kind:value.kind,types:Object.freeze([...value.types])});
}
// Geometry admission only: every returned rectangle must be inside the union
// of the explicitly requested scopes, including scopes that touch or overlap.
function covered(area,scopes,work){
    let remaining=[area];
    for(const b of scopes){
        const next=[];
        for(const a of remaining){
            if(--work.left<0)fail();
            if(!overlap(a,b)){next.push(a);continue;}
            const firstRow=Math.max(a.firstRow,b.firstRow),lastRow=Math.min(a.lastRow,b.lastRow),firstColumn=Math.max(a.firstColumn,b.firstColumn),lastColumn=Math.min(a.lastColumn,b.lastColumn);
            if(a.firstRow<firstRow)next.push({...a,lastRow:firstRow-1});
            if(lastRow<a.lastRow)next.push({...a,firstRow:lastRow+1});
            if(a.firstColumn<firstColumn)next.push({firstRow,lastRow,firstColumn:a.firstColumn,lastColumn:firstColumn-1});
            if(lastColumn<a.lastColumn)next.push({firstRow,lastRow,firstColumn:lastColumn+1,lastColumn:a.lastColumn});
        }
        remaining=next;if(!remaining.length)return true;
        if(remaining.length>32768)fail();
    }
    return false;
}
export function checkedSheetSpecialCellsPlan(response,request,expectedRevision){
    if(!exact(request,['action','sheetId','ranges','kind','types'])||request.action!=='special-cells')fail();
    const {action,...fields}=request,checked=checkedSheetSpecialCellsRequest(fields),envelope=checkedQueryResult(response),result=envelope.result;
    if(!Number.isSafeInteger(expectedRevision)||expectedRevision<0||envelope.revision!==expectedRevision
      ||!exact(result,['sheetId','kind','types','ranges','cellCount','selectionApplied'])||result.sheetId!==checked.sheetId||result.kind!==checked.kind
      ||!Array.isArray(result.types)||result.types.length!==checked.types.length||result.types.some((t,i)=>t!==checked.types[i])
      ||result.selectionApplied!==false||!Array.isArray(result.ranges)||result.ranges.length>512||!Number.isSafeInteger(result.cellCount)||result.cellCount<0)fail();
    const ranges=result.ranges.map(range);let count=0;const work={left:2000000};
    for(let i=0;i<ranges.length;i++){
        const r=ranges[i];
        if(i&&compare(ranges[i-1],r)>=0||ranges.slice(0,i).some(old=>overlap(old,r))||!covered(r,checked.ranges,work))fail();
        count+=(r.lastRow-r.firstRow+1)*(r.lastColumn-r.firstColumn+1);
    }
    if(count!==result.cellCount)fail();
    return Object.freeze({...result,types:checked.types,ranges:Object.freeze(ranges)});
}
