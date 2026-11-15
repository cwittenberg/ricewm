/** Presentation of a native-owned selection plan; no cell matching or rectangle
 * algebra runs here. Commit always reparses the original native command. */
import {checkedSheetSelectionAreas} from './command-contract.mjs';
const fail=()=>{throw Object.assign(new TypeError('Invalid native selection preview'),{code:'INVALID_SELECTION_PREVIEW'});};
export function checkedSheetSelectionPreview(response,request,revision){
    const value=response?.result;
    if(!Number.isSafeInteger(revision)||revision<0||response?.revision!==revision||typeof request?.sheetId!=='string'||!request.sheetId.length||request.sheetId.length>128||!value||Object.getPrototypeOf(value)!==Object.prototype||Object.keys(value).length!==4||!['sheetId','ranges','active','selectionApplied'].every(k=>Object.hasOwn(value,k))||value.sheetId!==request.sheetId||value.selectionApplied!==false)fail();
    let selection;try{selection=checkedSheetSelectionAreas(value.ranges,value.active);}catch{fail();}
    return Object.freeze({sheetId:value.sheetId,...selection,selectionApplied:false});
}
export function sheetSelectionPreviewFrame(frame,plan){
    const {ranges,active}=checkedSheetSelectionAreas(plan.ranges,plan.active);
    const primary=ranges.find(r=>r.firstRow<=active.row&&active.row<=r.lastRow&&r.firstColumn<=active.column&&active.column<=r.lastColumn);
    const rows=frame.rows.flatMap((r,i)=>r.index>=primary.firstRow&&r.index<=primary.lastRow?[i]:[]),columns=frame.columns.flatMap((c,i)=>c.index>=primary.firstColumn&&c.index<=primary.lastColumn?[i]:[]);
    const {selection_areas:_,fill_endpoint:oldEndpoint,...base}=frame;
    return {...base,active:[active.row,active.column],selection:rows.length&&columns.length?{first_row:rows[0],last_row:rows.at(-1),first_column:columns[0],last_column:columns.at(-1)}:null,
        ...(ranges.length>1?{selection_areas:ranges.map(r=>({first_row:r.firstRow,last_row:r.lastRow,first_column:r.firstColumn,last_column:r.lastColumn}))}:oldEndpoint?{fill_endpoint:[primary.lastRow,primary.lastColumn]}:{})};
}
