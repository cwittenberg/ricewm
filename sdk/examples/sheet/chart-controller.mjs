import { Stack, openTypographyFontBank, createCancellationScope } from '@ricewm/app';
import { openChartProvider } from './chart-provider.mjs';
import { createChartEditor, createChartContextMenu } from './chart-editor.mjs';

/** One presentation controller per workbook window. Cell resolution, chart
 * projection, anchors and mutations remain inside the spreadsheet actor. */
export function createSheetCharts({parent,query,services,shapeText:sharedShapeText,onCommand,onSelection,onEdit,onError}) {
    if(sharedShapeText!==undefined&&typeof sharedShapeText!=='function')throw TypeError('Chart text shaper must be callable');
    let provider=openChartProvider({query}),layer=null,closed=false,generation=0,selected=null,selectedPart=null,sheet=null,opening=null,typography=null;
    const lifetime=createCancellationScope(),editors=new Map();
    const shapeText=async request=>{
        if(closed)throw Error('Chart window is closed');
        if(sharedShapeText)return sharedShapeText(request);
        opening ||= openTypographyFontBank({services,signal:lifetime.signal}).then(value=>{typography=value;return value;});
        return (await opening).shape(request);
    };
    function ensureLayer(){if(!layer){layer=new Stack({name:'sheet-charts',text:'panel:overlay:flush'});parent.append(layer);}return layer;}
    function select(id,part=null){selected=id;selectedPart=part?{...part}:null;for(const [key,record] of editors)record.editor.setSelected(key===id,selectedPart);onSelection(id,selectedPart);}
    // A viewport editor is a disposable presentation of a document object.
    // Culling it must preserve the selected object and its selected part.
    function remove(id){const record=editors.get(id);if(record){layer.remove(record.editor.control,{destroy:true});editors.delete(id);}}
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
            if(closed)return;
            const nextSheet=projection?.activeSheetId??null;
            if(nextSheet!==sheet){sheet=nextSheet;disposeEditors();if(selected!==null)select(null);}
            if(!projection?.capabilities?.includes('create-chart')){disposeEditors();if(selected!==null)select(null);return;}
            if(!viewport.pixelWidth){disposeEditors();return;}
            const revision=projection.revision,sheetId=projection.activeSheetId;
            const view={...viewport,followSelection:false},visible=new Map(),inventory=new Set();let offset=0,total=null;
            do {
                const page=await provider.list(sheetId,{offset,limit:16,viewport:view});
                if(closed||ticket!==generation)return;
                if(page.revision!==revision)throw Error('Chart layout belongs to another workbook revision');
                if(total!==null&&total!==page.result.total)throw Error('Chart inventory changed while paging');total=page.result.total;
                for(const entry of page.result.charts){
                    if(inventory.has(entry.id))throw Error('Duplicate chart identity across inventory pages');inventory.add(entry.id);
                    const b=entry.bounds;if(b&&b.width>0&&b.height>0&&b.x<view.pixelWidth&&b.y<view.pixelHeight&&b.x+b.width>48&&b.y+b.height>24)visible.set(entry.id,entry);
                }
                offset=page.result.nextOffset;
            }while(offset!==null);
            if(inventory.size!==total)throw Error('Chart inventory is incomplete');
            if(selected!==null&&!inventory.has(selected))select(null);
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
                if(selected===id)editor.setSelected(true,selectedPart);
                createChartContextMenu({onAction:name=>name==='delete-chart'?void onCommand({type:'delete-chart',id},record.revision):onEdit(id,name)}).attachTo(editor.control);
                void editor.ready.catch(fail);
            }
        },
        reset(){generation++;provider.close();provider=openChartProvider({query});disposeEditors();sheet=null;select(null);},
        async close(){if(closed)return;closed=true;generation++;provider.close();disposeEditors();lifetime.abort();await typography?.close();},
    };
}
