/** Find transport only. Wildcard matching, cell text, ordering and wrap belong
 * to the workbook engine. No browser formula/value matching implementation. */
import {checkedSheetRange} from './command-contract.mjs';
import {checkedQueryResult} from './query-contract.mjs';
const encoder=new TextEncoder(),plain=value=>value!==null&&typeof value==='object'&&Object.getPrototypeOf(value)===Object.prototype;
const exact=(value,fields)=>plain(value)&&Object.keys(value).length===fields.length&&fields.every(field=>Object.hasOwn(value,field));
const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_FIND'});};
const sheet=value=>typeof value==='string'&&value.length>0&&value.length<=128&&!/[\p{Cc}\ud800-\udfff]/u.test(value);
export const SHEET_FIND_DEFAULTS=Object.freeze({within:'sheet',text:'',matchCase:false,wholeCell:false,lookIn:'formulas',order:'rows'});
const common=(value,empty)=>['sheet','workbook','selection'].includes(value.within)&&typeof value.text==='string'&&(empty||value.text.length>0)&&encoder.encode(value.text).length<=1024&&!/[\u0000\ud800-\udfff]/u.test(value.text)&&typeof value.matchCase==='boolean'&&typeof value.wholeCell==='boolean'&&['formulas','values'].includes(value.lookIn)&&['rows','columns'].includes(value.order);
export function checkedSheetFindPreferences(value){
    if(!exact(value,['within','text','matchCase','wholeCell','lookIn','order'])||!common(value,true))fail('Invalid Find preferences');
    return Object.freeze({...value});
}
export function checkedSheetFindOptions(value){
    if(!exact(value,['sheetId','within','range','text','matchCase','wholeCell','lookIn','order'])||!sheet(value.sheetId)||!common(value,false)||(value.within==='selection')!==(value.range!==null))fail('Invalid Find options');
    return Object.freeze({...value,range:value.range===null?null:checkedSheetRange(value.range)});
}
export function checkedSheetFindTarget(value){
    if(value===null)return null;
    if(!exact(value,['sheetId','row','column'])||!sheet(value.sheetId)||!Number.isSafeInteger(value.row)||value.row<0||value.row>=1048576||!Number.isSafeInteger(value.column)||value.column<0||value.column>=16384)fail('Invalid Find position');
    return Object.freeze({...value});
}
export function checkedSheetReplacementText(value){
    if(typeof value!=='string'||encoder.encode(value).length>16384||encoder.encode(JSON.stringify(value)).length>24576||/[\u0000\ud800-\udfff]/u.test(value))fail('Replacement exceeds the cell input profile');
    return value;
}
export function checkedSheetReplaceRequest(value){
    if(!exact(value,['options','replacement','target']))fail('Invalid Replace fields');
    const options=checkedSheetFindOptions(value.options),replacement=checkedSheetReplacementText(value.replacement),target=checkedSheetFindTarget(value.target);
    if(options.lookIn!=='formulas'||target&&(options.within!=='workbook'&&target.sheetId!==options.sheetId||options.range&&(target.row<options.range.firstRow||target.row>options.range.lastRow||target.column<options.range.firstColumn||target.column>options.range.lastColumn)))fail('Replace requires Formulas and a target inside its scope');
    return Object.freeze({options,replacement,target});
}
export function checkedSheetReplacePreview(response,request,expectedRevision){
    const checked=checkedSheetReplaceRequest(request),envelope=checkedQueryResult(response),result=envelope.result;
    if(envelope.revision!==expectedRevision||!Number.isSafeInteger(expectedRevision)||!exact(result,['matchedCells','changedCells','occurrences'])||Object.values(result).some(value=>!Number.isSafeInteger(value)||value<0)||result.matchedCells>2000000||result.changedCells>10000||result.changedCells>result.matchedCells||result.occurrences>0xffffffff||result.occurrences<result.matchedCells||result.matchedCells===0&&result.occurrences!==0||checked.target!==null&&result.matchedCells>1)fail('Replace preview changed or is invalid');
    return Object.freeze({...result});
}
export function checkedSheetFindPage(response,{options,after,limit},expectedRevision){
    const scope=checkedSheetFindOptions(options);checkedSheetFindTarget(after);
    if(!Number.isSafeInteger(limit)||limit<1||limit>32)fail('Invalid Find page limit');
    const envelope=checkedQueryResult(response),page=envelope.result;
    if(envelope.revision!==expectedRevision||!Number.isSafeInteger(expectedRevision)||!exact(page,['items','next','total'])||!Number.isSafeInteger(page.total)||page.total<0||page.total>2000000||!Array.isArray(page.items)||page.items.length>limit||page.items.length>page.total)fail('Find results changed or are invalid. Search again.');
    const seen=new Set();
    const items=page.items.map(item=>{
        if(!exact(item,['sheetId','row','column','preview','truncated'])||typeof item.preview!=='string'||Array.from(item.preview).length>160||encoder.encode(item.preview).length>640||/[\ud800-\udfff]/u.test(item.preview)||typeof item.truncated!=='boolean')fail('Invalid Find result');
        const {preview,truncated,...target}=item,checked=checkedSheetFindTarget(target),identity=JSON.stringify([checked.sheetId,checked.row,checked.column]);
        if(seen.has(identity)||scope.within!=='workbook'&&checked.sheetId!==scope.sheetId||scope.range&&(checked.row<scope.range.firstRow||checked.row>scope.range.lastRow||checked.column<scope.range.firstColumn||checked.column>scope.range.lastColumn))fail('Find result is duplicated or outside its scope');
        seen.add(identity);return Object.freeze({...checked,preview,truncated});
    });
    const next=checkedSheetFindTarget(page.next),last=items.at(-1);
    if(next&&(!last||next.sheetId!==last.sheetId||next.row!==last.row||next.column!==last.column||after&&next.sheetId===after.sheetId&&next.row===after.row&&next.column===after.column))fail('Find page did not advance');
    return Object.freeze({...page,next,items:Object.freeze(items)});
}

/** UI session state only: authored search inputs, never worksheet cell results. */
export const SHEET_FIND_HISTORY_LIMIT=10;
export function checkedSheetFindHistory(values,{replacement=false}={}){
    if(!Array.isArray(values)||values.length>SHEET_FIND_HISTORY_LIMIT||new Set(values).size!==values.length)fail('Invalid recent search history');
    for(const value of values){
        if(replacement)checkedSheetReplacementText(value);
        else if(typeof value!=='string'||!value.length)fail('Recent searches require nonempty text');
        else checkedSheetFindPreferences({...SHEET_FIND_DEFAULTS,text:value});
    }
    return Object.freeze([...values]);
}
export function rememberSheetFindText(history,value,{replacement=false}={}){
    checkedSheetFindHistory(history,{replacement});
    if(!replacement&&value==='')return Object.freeze([...history]);
    checkedSheetFindHistory([value],{replacement});
    return Object.freeze([value,...history.filter(item=>item!==value)].slice(0,SHEET_FIND_HISTORY_LIMIT));
}
export function checkedSheetFindUiState(value){
    if(!exact(value,['preferences','replacement','optionsExpanded'])||typeof value.optionsExpanded!=='boolean')fail('Invalid Find UI state');
    return Object.freeze({preferences:checkedSheetFindPreferences(value.preferences),replacement:checkedSheetReplacementText(value.replacement),optionsExpanded:value.optionsExpanded});
}
export function checkedSheetFindHistoryRequest(value){
    if(!exact(value,['kind','offset','limit'])||!['find','replace'].includes(value.kind)||!Number.isSafeInteger(value.offset)||value.offset<0||value.offset>SHEET_FIND_HISTORY_LIMIT||!Number.isSafeInteger(value.limit)||value.limit<1||value.limit>SHEET_FIND_HISTORY_LIMIT)fail('Invalid Find history page');
    return Object.freeze({...value});
}
/** Excel repeats Find with Shift+F4 / Ctrl+Shift+F4. F3 has other Excel duties. */
export function sheetFindRepeatDirection(detail){
    if(!detail||detail.pressed===false||(detail.key?detail.key!=='F4':detail.keyCode!==70))return null;
    return detail.modifiers===1?'forward':detail.modifiers===5?'backward':null;
}

/** Result order is computed globally by the advertised native query. Labels
 * are bounded previews; the browser must never sort them into a new order. */
export const SHEET_FIND_RESULT_COLUMNS=Object.freeze(['order','book','sheet','cell','value','formula']);
export const SHEET_FIND_RESULT_SORT=Object.freeze({column:'order',direction:'ascending'});
export function checkedSheetFindResultSort(value){
    if(!exact(value,['column','direction'])||!SHEET_FIND_RESULT_COLUMNS.includes(value.column)||!['ascending','descending'].includes(value.direction))fail('Invalid Find result sort');
    return Object.freeze({...value});
}
function checkedResultPreview(value){
    if(!exact(value,['text','truncated'])||typeof value.text!=='string'||Array.from(value.text).length>160||encoder.encode(value.text).length>640||/[\ud800-\udfff]/u.test(value.text)||typeof value.truncated!=='boolean')fail('Invalid Find result preview');
    return Object.freeze({...value});
}
export function checkedSheetFindResultsPage(response,request,expectedRevision){
    const scope=checkedSheetFindOptions(request.options),sort=checkedSheetFindResultSort(request.sort),after=checkedSheetFindTarget(request.after),limit=request.limit;
    if(!Number.isSafeInteger(limit)||limit<1||limit>16)fail('Invalid Find result page limit');
    const envelope=checkedQueryResult(response),page=envelope.result;
    if(!Number.isSafeInteger(expectedRevision)||expectedRevision<0||envelope.revision!==expectedRevision||!exact(page,['book','sheets','sort','items','next','total'])||!Number.isSafeInteger(page.total)||page.total<0||page.total>2000000||!Array.isArray(page.items)||page.items.length>limit||page.items.length>page.total||!Array.isArray(page.sheets)||page.sheets.length>limit)fail('Find results changed or are invalid');
    const echoed=checkedSheetFindResultSort(page.sort);if(echoed.column!==sort.column||echoed.direction!==sort.direction)fail('Find results used a different sort');
    const book=checkedResultPreview(page.book),sheetIds=new Set();
    const sheets=page.sheets.map(item=>{
        if(!exact(item,['id','name'])||!sheet(item.id)||sheetIds.has(item.id))fail('Invalid Find result sheet');
        sheetIds.add(item.id);return Object.freeze({id:item.id,name:checkedResultPreview(item.name)});
    });
    const seen=new Set(),usedSheets=new Set();
    const items=page.items.map(item=>{
        if(!exact(item,['sheetId','row','column','cell','value','formula']))fail('Invalid Find result item');
        const {cell,value,formula,...position}=item,target=checkedSheetFindTarget(position),id=JSON.stringify([target.sheetId,target.row,target.column]);
        if(!sheetIds.has(target.sheetId)||seen.has(id)||scope.within!=='workbook'&&target.sheetId!==scope.sheetId||scope.range&&(target.row<scope.range.firstRow||target.row>scope.range.lastRow||target.column<scope.range.firstColumn||target.column>scope.range.lastColumn))fail('Find result is repeated or outside its scope');
        let column='',n=target.column+1;while(n){column=String.fromCharCode(65+(n-1)%26)+column;n=Math.floor((n-1)/26);}
        if(cell!==`${column}${target.row+1}`)fail('Find result address does not match its coordinates');
        seen.add(id);usedSheets.add(target.sheetId);return Object.freeze({...target,cell,value:checkedResultPreview(value),formula:checkedResultPreview(formula)});
    });
    if(usedSheets.size!==sheetIds.size)fail('Unreferenced Find sheet metadata');
    const next=checkedSheetFindTarget(page.next),last=items.at(-1);
    if(next&&(!last||next.sheetId!==last.sheetId||next.row!==last.row||next.column!==last.column||after&&next.sheetId===after.sheetId&&next.row===after.row&&next.column===after.column))fail('Find result cursor did not advance');
    return Object.freeze({book,sheets:Object.freeze(sheets),sort:echoed,items:Object.freeze(items),next,total:page.total});
}
