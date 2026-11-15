import {WorkbookContracts} from '@ricewm/app';
const {checkedSheetCommand,createWorkbookTextClipboard,readWorkbookClipboardText}=WorkbookContracts;

/** Called under the document owner's exclusive operation. No cells, formulas
 * or TSV parsing live here; native commands own the complete transaction. */
export function createSheetTextClipboardPort({services,command,query,revision,alive=()=>true}){
    const transport=createWorkbookTextClipboard({services});
    const current=expected=>{
        if(!alive())throw Error('Document is closed');
        if(!Number.isSafeInteger(expected)||revision()!==expected)throw Error('Workbook changed before the clipboard operation');
    };
    const paste=mode=>async(data,{expectedRevision}={})=>{
        current(expectedRevision);
        const checked=checkedSheetCommand({type:'paste-text',...data,text:'',mode});
        // Validate shape before reading any host data. Callers cannot inject
        // text or choose formula interpretation through this external action.
        if(Object.keys(data).some(key=>!['sheetId','target'].includes(key)))throw TypeError('Paste clipboard requires sheetId and target');
        const text=await transport.read();
        current(expectedRevision);
        if(text===null)return {applied:false,projection:null};
        const projection=await command(checkedSheetCommand({...checked,text}),{expectedRevision});
        return {applied:true,projection};
    };
    return Object.freeze({
        'copy-text':transport.canWrite?async(data,{expectedRevision}={})=>{
            current(expectedRevision);
            if(Object.keys(data).some(key=>!['sheetId','range'].includes(key)))throw TypeError('Copy clipboard requires sheetId and range');
            const projection=await command(checkedSheetCommand({type:'copy-range',...data}),{expectedRevision});
            // Copy has already changed the internal clipboard identity. Even
            // if export/host write fails, return its projection to the UI.
            try{
                current(projection.revision);
                if(projection.clipboard?.mode!=='copy')throw Error('Native Copy did not return a clipboard identity');
                const text=await readWorkbookClipboardText(query,{generation:projection.clipboard.generation,revision:projection.revision,alive});
                current(projection.revision);
                await transport.write(text);
                return {applied:true,written:true,projection};
            }catch(error){return {applied:true,written:false,projection,error:String(error.message||error).slice(0,240)};}
        }:undefined,
        'paste-text':transport.canRead?paste('all'):undefined,
        'paste-values':transport.canRead?paste('values'):undefined,
    });
}
