/** Permissioned text clipboard transport. The workbook owns TSV parsing,
 * values, formulas and history. This module only validates and moves bytes. */
import {Clipboard} from '../clipboard.mjs';
import {checkedQueryRequest} from './query-contract.mjs';
import {SHEET_PASTE_TEXT_MAX_BYTES} from './command-contract.mjs';
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true});
const MIME=['text/plain;charset=utf-8','text/plain'];
const granted=method=>typeof method==='function'&&method.granted!==false;
const error=message=>Object.assign(new TypeError(message),{code:'INVALID_CLIPBOARD'});
const plain=value=>value&&typeof value==='object'&&!Array.isArray(value);

/** Read an immutable native clipboard snapshot a page at a time. Revision,
 * generation, UTF-8 byte offsets and total size must agree across every page. */
export async function readWorkbookClipboardText(query,{generation,revision,alive=()=>true}={}){
    if(typeof query!=='function'||!Number.isSafeInteger(revision)||revision<0)throw error('Invalid clipboard export context');
    let offset=0,total=null;const parts=[];
    for(let page=0;page<130;page++){
        if(!alive())throw error('Clipboard owner closed');
        const response=await query(checkedQueryRequest({action:'clipboard-text',generation,offset,limit:8192}),{expectedRevision:revision});
        if(!alive())throw error('Clipboard owner closed');
        const value=response?.result;
        if(response?.revision!==revision||!plain(value)||Object.keys(value).length!==5
            ||!['generation','offset','nextOffset','totalBytes','text'].every(k=>Object.hasOwn(value,k))
            ||value.generation!==generation||value.offset!==offset||typeof value.text!=='string'||/[\ud800-\udfff]/u.test(value.text)
            ||!Number.isSafeInteger(value.totalBytes)||value.totalBytes<0||value.totalBytes>1024*1024)throw error('Invalid native clipboard text page');
        if(total===null)total=value.totalBytes;
        if(value.totalBytes!==total)throw error('Clipboard size changed during export');
        const bytes=encoder.encode(value.text).length,end=offset+bytes;
        if(bytes>8192||end>total||value.nextOffset!==(end===total?null:end)
            ||end<total&&bytes<8189)throw error('Clipboard text page did not advance consistently');
        parts.push(value.text);if(end===total)return parts.join('');offset=end;
    }
    throw error('Clipboard export exceeded its page budget');
}

/** Use the normal SDK services and their declared grants. A denied request is
 * propagated; another clipboard method is not used to retry that denial. */
export function createWorkbookTextClipboard({services}={}){
    const service=services?.clipboard;
    const formats=service?new Clipboard({services}):null;
    const readFormats=granted(service?.readFormats),writeFormats=granted(service?.writeFormats);
    const canRead=readFormats||granted(service?.readText),canWrite=writeFormats||granted(service?.writeText);
    return Object.freeze({canRead,canWrite,
        async read(){
            if(!canRead)throw error('Clipboard reading is not granted');
            let text;
            if(readFormats){
                const value=await formats.read(MIME);
                if(value===null)return null;
                if(!plain(value)||!MIME.includes(value.mimeType)||!(value.data instanceof Uint8Array)||value.truncated===true)throw error('Clipboard does not contain complete plain text');
                if(value.data.length>SHEET_PASTE_TEXT_MAX_BYTES)throw error('Clipboard text exceeds the current 24 KiB paste profile');
                text=decoder.decode(value.data);
            }else{text=await service.readText({});if(text===null)return null;}
            if(typeof text!=='string'||/[\ud800-\udfff]/u.test(text)||encoder.encode(text).length>SHEET_PASTE_TEXT_MAX_BYTES)throw error('Clipboard text exceeds the paste profile or is not valid Unicode');
            return text;
        },
        async write(text){
            if(!canWrite)throw error('Clipboard writing is not granted');
            if(typeof text!=='string'||/[\ud800-\udfff]/u.test(text)||encoder.encode(text).length>1024*1024)throw error('Clipboard text exceeds the export profile or is not valid Unicode');
            if(writeFormats)return formats.write([{mimeType:'text/plain',data:encoder.encode(text)}]);
            if(await service.writeText({text})!==true)throw error('Clipboard write was not acknowledged');
            return true;
        },
    });
}
