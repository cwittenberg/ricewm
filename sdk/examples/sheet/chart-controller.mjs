import { Stack, openTypographyEngine, createCancellationScope } from '@ricewm/app';
import { openChartProvider } from './chart-provider.mjs';
import { createChartEditor, createChartContextMenu } from './chart-editor.mjs';

/** One presentation controller per workbook window. Cell resolution, chart
 * projection, anchors and mutations remain inside the spreadsheet actor. */
export function createSheetCharts({parent,query,services,onCommand,onSelection,onEdit,onError}) {
    let provider=openChartProvider({query}),layer=null,closed=false,generation=0,selected=null,opening=null,typography=null;
    const lifetime=createCancellationScope(),editors=new Map();
    const shapeText=async request=>{
        if(closed)throw Error('Chart window is closed');
        opening ||= openTypographyEngine({services,signal:lifetime.signal}).then(value=>{typography=value;return value;});
        return (await opening).shape(request);
    };
    function ensureLayer(){if(!layer){layer=new Stack({name:'sheet-charts',text:'panel:overlay:flush'});parent.append(layer);}return layer;}
    function select(id,part=null){selected=id;for(const [key,record] of editors)record.editor.setSelected(key===id,part);onSelection(id,part);}
    function remove(id){const record=editors.get(id);if(record){layer.remove(record.editor.control,{destroy:true});editors.delete(id);}if(selected===id)select(null);}
    function disposeEditors(){for(const id of [...editors.keys()])remove(id);if(layer){parent.remove(layer,{destroy:true});layer=null;}}
    function fail(error){if(!closed)onError(error);}
    return {
        get selected(){return selected;},
        get ids(){return [...editors.keys()];},
        select,
        get(id){return provider.get(id);},
        projection(id){return editors.get(id)?.projection??null;},
        seriesColors(id){return Object.fromEntries((editors.get(id)?.projection.series||[]).map(s=>[s.id,s.color]));},
        async refresh(projection,viewport){
            const ticket=++generation;
            if(closed||!projection?.capabilities?.includes('create-chart')||!viewport.pixelWidth){disposeEditors();return;}
            const revision=projection.revision,sheetId=projection.activeSheetId;
            const view={...viewport,followSelection:false},visible=new Map();let offset=0;
            do {
                const page=await provider.list(sheetId,{offset,limit:16,viewport:view});
                if(closed||ticket!==generation)return;
                if(page.revision!==revision)throw Error('Chart layout belongs to another workbook revision');
                for(const entry of page.result.charts){const b=entry.bounds;if(b&&b.width>0&&b.height>0&&b.x<view.pixelWidth&&b.y<view.pixelHeight&&b.x+b.width>48&&b.y+b.height>24)visible.set(entry.id,entry);}
                offset=page.result.nextOffset;
            }while(offset!==null);
            for(const id of [...editors.keys()])if(!visible.has(id))remove(id);
            for(const [id,entry]of visible){
                const result=await provider.projection(id);
                if(closed||ticket!==generation)return;
                if(result.revision!==revision)throw Error('Chart results belong to another workbook revision');
                const bounds=entry.bounds,viewSize={width:view.pixelWidth,height:view.pixelHeight},documentZoom=view.zoomPercent||100;
                const identity=JSON.stringify([bounds,viewSize,documentZoom,result.token]);
                const prior=editors.get(id);
                if(prior){
                    prior.viewport=view;prior.revision=revision;prior.projection=result.projection;
                    if(prior.identity!==identity){prior.identity=identity;void prior.editor.present({projection:result.projection,bounds,viewport:viewSize,documentZoom,clipInsets:{left:projection.frame.row_header,top:projection.frame.column_header}}).catch(fail);}
                    continue;
                }
                ensureLayer();
                const record={viewport:view,revision,identity,projection:result.projection,editor:null};
                const editor=createChartEditor({projection:result.projection,bounds,viewport:viewSize,documentZoom,shapeText,clipInsets:{left:projection.frame.row_header,top:projection.frame.column_header},
                    selected:selected===id,onError:fail,
                    onSelection:event=>select(event.part?id:null,event.part),
                    onEdit:event=>onEdit(id,event?.part?.kind==='legend'?'legend':'edit-chart',event?.part),onDelete:()=>onCommand({type:'delete-chart',id},record.revision),
                    onCommit:event=>onCommand({type:'move-chart',id,chartRevision:event.chartRevision,viewport:record.viewport,bounds:event.after},record.revision),
                });
                record.editor=editor;editors.set(id,record);layer.append(editor.control);
                createChartContextMenu({onAction:name=>name==='delete-chart'?void onCommand({type:'delete-chart',id},record.revision):onEdit(id,name)}).attachTo(editor.control);
                void editor.ready.catch(fail);
            }
        },
        reset(){generation++;provider.close();provider=openChartProvider({query});disposeEditors();select(null);},
        async close(){if(closed)return;closed=true;generation++;provider.close();disposeEditors();lifetime.abort();await typography?.close();},
    };
}
