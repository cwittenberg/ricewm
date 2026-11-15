import {failure} from './contract-values.mjs';

/** Read original native capabilities; source adapters never grant editability. */
export async function nativeRangeObject(backend,slide,object,alive=()=>{}){
    let startObject=0,limit=64;
    do{
        let reply;
        try{reply=await backend.exchange('scene',{slide,startObject,limit},backend.revision);}
        catch(error){alive();if(error.code!=='RESPONSE_TOO_LARGE'||limit===1)throw error;limit=Math.max(1,Math.floor(limit/2));continue;}
        alive();const scene=reply.result.scene,target=scene.objects.find(item=>item.id===object);
        if(target)return {target,editable:scene.canSetBackground===true};
        startObject=scene.nextObject;
    }while(startObject!==null);
    return {target:null,editable:false};
}

export async function nativeRangeText(backend,slide,command,alive=()=>{}){
    const {target,editable}=await nativeRangeObject(backend,slide,command.object,alive);
    const reject=()=>{throw failure('UNSUPPORTED_OPERATION','Select source-backed editable text or a table cell');};
    if(!editable||!target)reject();
    if(command.cell===undefined){if(!target.text?.editable)reject();return target.text;}
    const {row,column}=command.cell,table=target.table;
    if(!table?.canEditCells||row>=table.rows||column>=table.columns)reject();
    const reply=await backend.exchange('table-cells',{slide:command.slide,object:command.object,startCell:row*table.columns+column,limit:1},backend.revision);alive();
    const cell=reply.result.cells[0];
    if(cell?.row!==row||cell.column!==column||cell.covered||!cell.editable||!cell.text?.editable)reject();
    return cell.text;
}
