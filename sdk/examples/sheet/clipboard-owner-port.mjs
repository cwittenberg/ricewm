import {WorkbookContracts} from '@ricewm/app';
const {checkedSheetCommand,checkedWorkbookClipboardOwner,createWorkbookOwnedClipboard,readWorkbookClipboardText}=WorkbookContracts;
/** System identity selects an existing native snapshot; it never supplies cell
 * data or permission to delete a source. The engine rechecks the generation. */
export function createSheetOwnedClipboardPort({services,command,query,revision,identity,alive=()=>true}){
    const transport=createWorkbookOwnedClipboard({services});let ownership=null,failedOwnership=null;
    const observe=(projection,{systemWriteFailed=false}={})=>{
        if(!projection)return;
        const slot=projection.clipboard,documentId=identity();
        if(ownership&&(ownership.owner.documentId!==documentId||slot?.generation!==ownership.owner.generation||slot?.mode!==ownership.mode))ownership=null;
        if(failedOwnership&&(failedOwnership.documentId!==documentId||slot?.generation!==failedOwnership.generation))failedOwnership=null;
        if(systemWriteFailed&&slot)failedOwnership={documentId,generation:slot.generation};
    };
    const current=(expected,documentId)=>{
        if(!alive())throw Error('Document is closed');
        if(!Number.isSafeInteger(expected)||expected!==revision()||documentId!==identity())throw Error('Workbook changed before the clipboard operation');
    };
    const copy=cut=>async(data,{expectedRevision}={})=>{
        const documentId=identity();current(expectedRevision,documentId);
        if(Object.keys(data).length!==2||!Object.hasOwn(data,'sheetId')||!Object.hasOwn(data,'range'))throw TypeError('Copy/Cut requires sheetId and range');
        const projection=await command(checkedSheetCommand({type:cut?'cut-range':'copy-range',...data}),{expectedRevision});ownership=null;failedOwnership=null;
        try{
            current(projection.revision,documentId);
            if(projection.clipboard?.mode!==(cut?'cut':'copy'))throw Error('Native Copy/Cut did not return the expected clipboard identity');
            const owner=checkedWorkbookClipboardOwner({schema:'rice.sheet.clipboard-owner/v1',documentId,token:crypto.randomUUID(),generation:projection.clipboard.generation});
            const text=await readWorkbookClipboardText(query,{generation:owner.generation,revision:projection.revision,alive});
            current(projection.revision,documentId);await transport.write(owner,text);
            ownership={owner,mode:projection.clipboard.mode};return {applied:true,written:true,projection};
        }catch(error){failedOwnership={documentId,generation:projection.clipboard?.generation};return {applied:true,written:false,projection,error:String(error.message||error).slice(0,240)};}
    };
    return Object.freeze({
        observe,
        copy:transport.canWrite?copy(false):undefined,cut:transport.canWrite?copy(true):undefined,
        paste:transport.canRead?async(data,{expectedRevision}={})=>{
            const documentId=identity();current(expectedRevision,documentId);
            if(failedOwnership?.documentId===documentId)throw Error('System Copy/Cut failed. Retry Copy, or use internal Paste Special or explicit Paste Text.');
            if(Object.keys(data).length!==3||!Object.hasOwn(data,'sheetId')||!Object.hasOwn(data,'target')||!['all','values','formulas'].includes(data.mode))throw TypeError('Paste requires sheetId, target and mode');
            // Native validation happens before the first host read.
            const template=checkedSheetCommand({type:'paste-special',sheetId:data.sheetId,target:data.target,options:{mode:data.mode,operation:'none',transpose:false,skipBlanks:false,link:false}});
            const representation=await transport.read();current(expectedRevision,documentId);
            if(representation===null)return {applied:false,projection:null};
            const owner=representation.kind==='owner'?representation.owner:null;
            const same=owner&&ownership&&owner.documentId===documentId&&owner.token===ownership.owner.token&&owner.generation===ownership.owner.generation;
            if(owner?.documentId===documentId&&!same)throw Error('Native clipboard identity expired. Copy again, or choose explicit Paste Text.');
            let checked;
            if(same){
                if(ownership.mode==='cut'&&data.mode!=='all')throw Error('The current Cut transaction requires Paste All');
                checked=checkedSheetCommand({...template,clipboardGeneration:owner.generation});
            }else{
                const text=representation.kind==='text'?representation.text:await transport.readText();current(expectedRevision,documentId);
                if(text===null)return {applied:false,projection:null};
                checked=checkedSheetCommand({type:'paste-text',sheetId:data.sheetId,target:data.target,text,mode:data.mode==='values'?'values':'all'});
            }
            const projection=await command(checked,{expectedRevision});observe(projection);
            return {applied:true,projection};
        }:undefined,
    });
}
