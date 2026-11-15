/** Opaque native clipboard ownership, never a second cell/formula model. */
import {Clipboard} from '../clipboard.mjs';
import {createWorkbookTextClipboard} from './clipboard-text.mjs';
export const WORKBOOK_CLIPBOARD_OWNER_MIME='application/vnd.ricewm.sheet-clipboard-owner+json';
const schema='rice.sheet.clipboard-owner/v1',encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const uuid=value=>typeof value==='string'&&/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/u.test(value);
const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_CLIPBOARD_OWNER'});};
export function checkedWorkbookClipboardOwner(value){
    if(!value||Object.getPrototypeOf(value)!==Object.prototype||Object.keys(value).length!==4
        ||!['schema','documentId','token','generation'].every(key=>Object.hasOwn(value,key))
        ||value.schema!==schema||!uuid(value.documentId)||!uuid(value.token)
        ||!Number.isSafeInteger(value.generation)||value.generation<1)fail('Invalid native clipboard ownership');
    return Object.freeze({...value});
}
export function createWorkbookOwnedClipboard({services}={}){
    const granted=method=>typeof method==='function'&&method.granted!==false;
    const formats=services?.clipboard?new Clipboard({services}):null,text=createWorkbookTextClipboard({services});
    const canRead=granted(services?.clipboard?.readFormats),canWrite=granted(services?.clipboard?.writeFormats);
    return Object.freeze({canRead,canWrite,
        async write(owner,plainText){
            if(!canWrite)fail('Native clipboard writing is not granted');
            checkedWorkbookClipboardOwner(owner);
            if(typeof plainText!=='string'||/[\ud800-\udfff]/u.test(plainText)||encoder.encode(plainText).length>1024*1024)fail('Native clipboard text exceeds its export profile');
            return formats.write([{mimeType:WORKBOOK_CLIPBOARD_OWNER_MIME,data:encoder.encode(JSON.stringify(owner))},{mimeType:'text/plain',data:encoder.encode(plainText)}]);
        },
        async read(){
            if(!canRead)fail('Native clipboard reading is not granted');
            const result=await formats.read([WORKBOOK_CLIPBOARD_OWNER_MIME,'text/plain;charset=utf-8','text/plain']);
            if(result===null)return null;
            if(!(result?.data instanceof Uint8Array)||result.truncated===true)fail('Clipboard representation is incomplete');
            if(result.mimeType===WORKBOOK_CLIPBOARD_OWNER_MIME){
                if(result.data.length>1024)fail('Native clipboard identity exceeds its bound');
                return {kind:'owner',owner:checkedWorkbookClipboardOwner(JSON.parse(decoder.decode(result.data)))};
            }
            if(!['text/plain;charset=utf-8','text/plain'].includes(result.mimeType)||result.data.length>24*1024)fail('Clipboard is not bounded plain text');
            return {kind:'text',text:decoder.decode(result.data)};
        },
        // Foreign ownership can negotiate the plain companion. A denied read
        // never reaches this method; no retries through another host endpoint.
        readText:()=>text.read(),
    });
}
