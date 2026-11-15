import {copyRiceBusPayload} from '@ricewm/app';

const cellSummary=table=>({...table,cells:[],cellPreview:{startCell:0,count:0,totalCells:table.rows*table.columns,truncated:Boolean(table.cells?.length)}});

/** Keep the source model in the renderer and revision owner. The default AI
 * context carries one copy of cells; targeted reads use table-cells paging. */
export function deckCopilotContext(snapshot){
    const context={...snapshot,selection:snapshot.selection.map(object=>object.table?{...object,table:cellSummary(object.table)}:object)};
    try{return copyRiceBusPayload(context);}
    catch(error){
        if(!(error instanceof RangeError)||!/^RiceBus payload (?:is too complex|exceeds 256 KiB)$/.test(error.message))throw error;
        if(!context.scene?.objects.some(object=>object.table?.cells?.length))throw error;
        // Preserve identities, geometry, design/options and original native
        // permissions. Omitted cell details remain available through the same
        // owner's bounded query; this never changes action validation inputs.
        const scene={...context.scene,objects:context.scene.objects.map(object=>object.table?{...object,table:cellSummary(object.table)}:object)};
        return copyRiceBusPayload({...context,scene});
    }
}
