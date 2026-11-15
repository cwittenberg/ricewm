import {shapedTextPresentation} from './shaped-text-presentation.mjs';
import {Control} from './control.mjs';
import {Message} from './messages.mjs';
import {ShapedTextLayout} from './shaped-text-layout.mjs';
import {documentEditorShapedReference,documentEditorPresentation} from './document-editor.mjs';

const active=new WeakMap(),latest=new WeakMap();
const context=control=>{
    const app=control?.application,window=app?.controlWindows.get(control.id);
    if(!(control instanceof Control)||control.constructor.kind!==21||control.destroyed||!window||typeof app.editorLayoutHost!=='function')throw Error('Shaped layout needs an attached native document TextEditor');
    return {app,window,widget:String(control.id)};
};

/** Prepare source-bound interaction geometry without changing the current
 * editor. The caller can then install its matching value/vector frame and
 * activate the small reference in one synchronous presentation update. */
export async function prepareShapedEditorLayout(control,{text,map,presentation=null}={}) {
    const owner=context(control),layout=new ShapedTextLayout(text,map);presentation=shapedTextPresentation(presentation);
    const record={layout,token:null,disposed:false,revision:0,caret:0,anchor:0,selectionWork:null};latest.set(control,record);
    const request=parameters=>owner.app.editorLayoutHost({app:String(owner.app.id),window:String(owner.window.id),...parameters,widget:owner.widget});
    const release=()=>record.token?Promise.resolve().then(()=>request({action:'release',token:record.token})):Promise.resolve();
    const attached=()=>!record.disposed&&!control.destroyed&&control.application===owner.app&&owner.app.controlWindows.get(control.id)===owner.window;
    const current=()=>attached()&&active.get(control)===record;
    const candidate=()=>attached()&&latest.get(control)===record;
    const reference=(caret,anchor)=>documentEditorShapedReference({token:record.token,caret,anchor,revision:record.revision+1});
    const select=(caret,anchor=caret)=>{
        if(!current()||control.value!==text)throw Error('Shaped editor layout is stale');
        const c=layout.caret(caret),a=layout.caret(anchor),ref=reference(caret,anchor);
        documentEditorPresentation({...control,id:control.id,values:control.values,documentShapedReference:ref},`editor:plain:${control.values.wrap?'word':'none'}:nolines:${c.scalar}:${a.scalar}\n${text}`);
        record.caret=caret;record.anchor=anchor;record.revision=ref.revision;
        control.documentShapedReference=ref;control.setSelection(a.scalar,c.scalar);owner.app.invalidate(control);
    };
    try{
        const {lines,clusters,stops,...header}=layout.map;
        const begun=await request({action:'begin',text,header,presentation});record.token=begun?.token;
        reference(0,0);
        for(const[kind,rows]of Object.entries({lines,clusters,stops}))for(let offset=0;offset<rows.length;offset+=128){
            if(!candidate())throw Error('Shaped editor upload was superseded');
            const page=rows.slice(offset,offset+128),reply=await request({action:'append',token:record.token,kind,offset,rows:page});
            if(reply?.next!==offset+page.length)throw Error('Shaped editor page acknowledgement changed');
        }
        const committed=await request({action:'commit',token:record.token});
        if(!candidate()||committed?.ready!==true)throw Error('Shaped editor upload was superseded');
    }catch(error){
        if(latest.get(control)===record)latest.delete(control);
        record.disposed=true;await release().catch(()=>{});throw error;
    }
    const state=async()=>{
        const reply=await request({action:'state',token:record.token});
        if(!attached())throw Error('Shaped editor layout is stale');return reply;
    };
    const synchronize=()=>{
        const revision=record.revision;
        const pending=state().then(reply=>{
            if(!current()||record.revision!==revision||control.value!==text||!reply?.selection)return;
            const {caret,anchor}=reply.selection,c=layout.caret(caret),a=layout.caret(anchor),selection=control.selection;
            if(c.scalar===selection.caret&&a.scalar===selection.anchor)select(caret,anchor);
        });
        record.selectionWork=pending;
        pending.finally(()=>{if(record.selectionWork===pending)record.selectionWork=null;}).catch(()=>{});
        owner.app.observeHostWork?.(pending.catch(()=>{}),owner.window.id);return pending;
    };
    let stopDestroy,stopSelection;
    const handle=Object.freeze({layout,token:record.token,select,state,synchronize,
        get selection(){return Object.freeze({caret:record.caret,anchor:record.anchor});},
        get pending(){return record.selectionWork;},
        activate({caret=null,anchor=null}={}){
            if(!candidate()||control.value!==text||!control.values.documentFont)throw Error('Shaped editor source or font changed before activation');
            caret??=layout.stopAtScalar(control.selection.caret);anchor??=layout.stopAtScalar(control.selection.anchor);
            if(caret===null||anchor===null)throw Error('Editor selection has no native caret stop');
            layout.caret(caret);layout.caret(anchor);
            const ref=reference(caret,anchor);
            documentEditorPresentation({...control,id:control.id,values:control.values,documentShapedReference:ref},control.editorController().presentation());
            const previous=active.get(control);active.set(control,record);
            select(caret,anchor);control.shapedInteraction=handle;
            if(previous&&previous!==record)void previous.handle.dispose().catch(()=>{});
            return handle;
        },
        async dispose(){
            if(record.disposed)return;record.disposed=true;stopDestroy?.();stopSelection?.();
            if(latest.get(control)===record)latest.delete(control);
            if(active.get(control)===record){active.delete(control);control.documentShapedReference=null;control.shapedInteraction=null;owner.app.invalidate(control);}
            await release();
        },
    });record.handle=handle;
    stopDestroy=control.on(Message.DESTROY,()=>{void handle.dispose().catch(()=>{});});
    stopSelection=control.on(Message.TEXT_SELECTION,()=>{if(current())void synchronize().catch(()=>{});});
    return handle;
}

/** Publish only after source-bound validation. Glyph vectors remain in the
 * caller's SceneSurface; failed uploads preserve the previous active map. */
export async function openShapedEditorLayout(control,options={}) {
    const text=options.text??control?.value;
    if(control?.value!==text||!control.values.documentFont)throw Error('Shaped layout source or font differs from the editor value');
    const handle=await prepareShapedEditorLayout(control,{...options,text});
    try{return handle.activate({caret:options.caret??null,anchor:options.anchor??options.caret??null});}
    catch(error){await handle.dispose().catch(()=>{});throw error;}
}
