/** Reference-field transport syntax only. Names, formulas and cell values are
 * workbook-engine operations and are not evaluated in this module. */
import {checkedSheetCommand,checkedCapabilities} from './command-contract.mjs';
import {parseSheetRanges,formatSheetRanges} from './ranges.mjs';
import {checkedQueryRequest,checkedQueryResult} from './query-contract.mjs';
const fail=()=>{throw Object.assign(new TypeError("Enter a worksheet cell or range, such as A1:C20 or 'Quarter 1'!A1."),{code:'INVALID_NAVIGATION_REFERENCE'});};
function metadata(sheets,activeSheetId){
    if(!Array.isArray(sheets)||!sheets.length||sheets.length>65536||!sheets.some(sheet=>sheet?.id===activeSheetId))fail();
    const ids=new Set();
    for(const sheet of sheets){if(typeof sheet?.id!=='string'||!sheet.id||ids.has(sheet.id)||typeof sheet.name!=='string'||!sheet.name.length||sheet.name.length>256||/[\u0000-\u001f\ud800-\udfff]/u.test(sheet.name))fail();ids.add(sheet.id);}
}
function split(reference){
    let value=reference.trim().replace(/^=/u,'').trim();
    if(value.startsWith("'")){
        let name='';
        for(let index=1;index<value.length;index++){
            if(value[index]!=="'"){name+=value[index];continue;}
            if(value[index+1]==="'"){name+="'";index++;continue;}
            if(value[index+1]!=='!')fail();
            return [name,value.slice(index+2).trim()];
        }
        fail();
    }
    const bang=value.indexOf('!');if(bang<0)return [null,value];
    const name=value.slice(0,bang);if(!name||/[\s'!\[\]:,]/u.test(name))fail();
    return [name,value.slice(bang+1).trim()];
}
function range(body){
    const parts=body.split(':').map(part=>part.trim());if(parts.length<1||parts.length>2)fail();
    const kind=part=>/^\$?[A-Za-z]{1,3}\$?[1-9]\d{0,6}$/u.test(part)?'cell':/^\$?[A-Za-z]{1,3}$/u.test(part)?'column':/^\$?[1-9]\d{0,6}$/u.test(part)?'row':null;
    const type=kind(parts[0]);if(!type||parts.length===1&&type!=='cell'||parts.some(part=>kind(part)!==type))fail();
    const ends=parts.map(part=>parseSheetRanges(type==='cell'?part:`${part}:${part}`)[0]),a=ends[0],b=ends[1]||a;
    return {firstRow:Math.min(a.firstRow,b.firstRow),lastRow:Math.max(a.lastRow,b.lastRow),firstColumn:Math.min(a.firstColumn,b.firstColumn),lastColumn:Math.max(a.lastColumn,b.lastColumn)};
}
export function sheetNavigationCommand(reference,{sheets,activeSheetId,capabilities=[]}={}){
    const available=checkedCapabilities(capabilities);
    if(available.includes('navigate-reference')){
        metadata(sheets,activeSheetId);
        return checkedSheetCommand({type:'navigate-reference',sheetId:activeSheetId,reference});
    }
    if(typeof reference!=='string'||reference.length>8192||/[\u0000-\u001f\ud800-\udfff\[\]]/u.test(reference))fail();
    metadata(sheets,activeSheetId);const [name,body]=split(reference);
    const candidates=name===null?sheets.filter(sheet=>sheet.id===activeSheetId):sheets.filter(sheet=>sheet.name.toLowerCase()===name.toLowerCase());
    if(candidates.length!==1)fail();
    return checkedSheetCommand({type:available.includes('navigate-range')?'navigate-range':'select-range',sheetId:candidates[0].id,...range(body)});
}
/** Validate native destination pages without interpreting references/formulas. */
export function checkedSheetNavigationPage(response,request,expectedRevision){
    const fail=()=>{throw Object.assign(new TypeError('The destination list changed or is invalid. Reopen Go To.'),{code:'INVALID_NAVIGATION_PAGE'});};
    const checked=checkedQueryRequest(request),envelope=checkedQueryResult(response),page=envelope.result;
    const plain=value=>value!==null&&typeof value==='object'&&Object.getPrototypeOf(value)===Object.prototype;
    const exact=(value,keys)=>plain(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
    const text=(value,max)=>typeof value==='string'&&value.length>0&&new TextEncoder().encode(value).length<=max&&!/[\p{Cc}\ud800-\udfff]/u.test(value);
    if(checked.action!=='navigation-destinations'||!Number.isSafeInteger(expectedRevision)||envelope.revision!==expectedRevision||!exact(page,['offset','nextOffset','totalCandidates','items'])||page.offset!==checked.offset||!Number.isSafeInteger(page.totalCandidates)||page.totalCandidates<0||page.totalCandidates>5120||!Array.isArray(page.items)||page.items.length>checked.limit||page.items.length>Math.max(0,page.totalCandidates-checked.offset))fail();
    if(page.nextOffset!==null&&(!Number.isSafeInteger(page.nextOffset)||page.nextOffset<=checked.offset||page.nextOffset>=page.totalCandidates||page.nextOffset-checked.offset>128||page.nextOffset-checked.offset<page.items.length))fail();
    if(page.nextOffset===null&&page.totalCandidates-checked.offset>128)fail();
    const items=page.items.map(item=>{
        if(!exact(item,['kind','label','reference','scopeSheetId'])||!['name','table'].includes(item.kind)||!text(item.label,2048)||!text(item.reference,8192)||item.scopeSheetId!==null&&!text(item.scopeSheetId,128)||item.kind==='table'&&item.scopeSheetId!==null)fail();
        return Object.freeze({...item});
    });
    return Object.freeze({...page,items:Object.freeze(items)});
}
export function formatSheetNavigationReference(command,sheets){
    const checked=checkedSheetCommand(command);if(!['select-range','navigate-range'].includes(checked.type)||!Array.isArray(sheets))fail();
    const sheet=sheets.find(sheet=>sheet.id===checked.sheetId);if(!sheet)fail();
    metadata(sheets,sheet.id);const {type,sheetId,...selected}=checked;
    return `'${sheet.name.replaceAll("'","''")}'!${formatSheetRanges([selected],{absolute:false})}`;
}
