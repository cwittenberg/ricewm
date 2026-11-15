import {SceneTextEditor} from './scene-text-editor.mjs';
import {SceneSurface,copySceneFrame} from './scene-surface.mjs';
import {shapedTextPresentation} from './shaped-text-presentation.mjs';
import {prepareShapedEditorLayout} from './shaped-editor-layout.mjs';
import {documentEditorShapedReference} from './document-editor.mjs';
import {Message} from './messages.mjs';
import {State} from './control.mjs';

/** Native text input over caller-rendered glyph vectors. The caller supplies
 * the same source-bound shaper map and transform used for its SceneSurface.
 * Input/focus/clipboard remain native; source edits and history remain caller
 * owned. No browser text painting or measurement is used in this mode. */
export class ShapedSceneTextEditor extends SceneTextEditor {
    static supportsShapedText=true;
    static constructorOptions=Object.freeze([...SceneTextEditor.constructorOptions,'renderText']);
    static authoring=Object.freeze({topics:['documents','visual'],constructor:'new ShapedSceneTextEditor({surface,renderText:async({value,edit,selection})=>({frame,map,presentation}),onChange,onCommit,onError,tabBehavior?:"insert"|"navigate",onEditKey?:(event)=>boolean,name})',methods:'await open({id,value,bounds,font,paint:null,layout?,zoom?,selection?,point?}), refresh(config), refreshText(), setSelectionAt({x,y,phase?,extend?,select?}), setSelection(anchor,caret), runAction(asyncCallback), close(); runAction retains the order of queued keys and source actions. waiting includes native shaping/upload, point-selection synchronization and exit. tabBehavior insert keeps literal Tab in native text; navigate retains the caller navigation/commit behavior. onEditKey runs after queued reflow and can consume a caller-owned source action such as list indentation. Keys queued during navigation follow its next opened target. renderText returns actual glyph vectors and their native editing map. presentation={width,height,transform} is the exact glyph transform and SceneSurface viewport. Native input is retained; queued keys wait for matching glyph/caret geometry. The caller owns rich run projections, source transactions and history.'});
    constructor({renderText,onChange=()=>{},...options}={}){
        if(typeof renderText!=='function'||typeof onChange!=='function')throw TypeError('Shaped scene text needs rendering and change callbacks');
        let owner;
        super({...options,onChange:(value,edit)=>{
            const changed=onChange(value,edit);
            return owner?.refreshText({value,edit,changed});
        }});
        owner=this;this.renderText=renderText;this.layoutWork=null;this.actionWork=null;this.layoutGeneration=0;this.shapedHandle=null;this.preflightReference=null;this.layoutKeys=[];
        this.glyphSurfaces=[0,1].map(index=>this.append(new SceneSurface({name:this.name+'-glyphs-'+index,enabled:false,visible:false,backgroundColor:'transparent',state:State.EXPANSIVE})));this.glyphSurface=null;
        this.editor.moveTo(this,{index:this.children.length-1});
        const route=this.editor.routeKey;
        this.editor.routeKey=event=>{
            if(this.layoutWork||this.actionWork){
                if(this.layoutKeys.length<512&&this.layoutKeys.reduce((n,k)=>n+(k.text?.length||0),0)+(event.detail.text?.length||0)<=16000)this.layoutKeys.push({...event.detail});
                event.preventDefault();return true;
            }
            return route(event);
        };
    }
    get waiting(){return this.layoutWork||this.actionWork||this.shapedHandle?.pending||super.waiting;}
    setSelection(anchor,caret=anchor){
        if(!this.active||!this.shapedHandle||this.shapedHandle.layout.text!==this.value)return false;
        const a=this.shapedHandle.layout.stopAtScalar(anchor),c=this.shapedHandle.layout.stopAtScalar(caret,{affinity:'upstream'});
        if(a===null||c===null)throw RangeError('Scene selection needs native grapheme stops');
        this.shapedHandle.select(c,a);return true;
    }
    drainInput(){
        while(this.active&&!this.layoutWork&&!this.actionWork&&this.layoutKeys.length){
            const item=this.layoutKeys.shift();
            if(!item.action){this.editor.sendMessage(Message.KEY,item,{cancelable:true});continue;}
            const pending=Promise.resolve().then(()=>this.active&&item.generation===this.generation?item.action():false);
            this.actionWork=pending;
            pending.then(item.resolve,item.reject);
            pending.finally(()=>{if(this.actionWork===pending)this.actionWork=null;this.drainInput();}).catch(()=>{});
        }
    }
    runAction(action){
        if(typeof action!=='function')throw TypeError('Scene text action must be a function');
        if(!this.active)return Promise.resolve(false);
        const selection=this.shapedHandle?.pending;
        if(selection&&!this.layoutWork)this.work(()=>selection);
        if(this.layoutKeys.length>=512)return Promise.reject(RangeError('Scene text action queue is full'));
        const pending=new Promise((resolve,reject)=>{this.layoutKeys.push({action,resolve,reject,generation:this.generation});});
        this.drainInput();this.application?.observeHostWork?.(pending.catch(()=>{}),this.application.controlWindows.get(this.editor.id)?.id);return pending;
    }
    checked(config,withValue=false){return super.checked(config,withValue,this.preflightReference||this.editor.documentShapedReference||null);}
    work(operation){
        const previous=this.layoutWork;
        const pending=Promise.resolve(previous).catch(()=>{}).then(operation);
        this.layoutWork=pending;
        pending.finally(()=>{
            if(this.layoutWork!==pending)return;this.layoutWork=null;
            this.drainInput();
        }).catch(()=>{});
        this.application?.observeHostWork?.(pending.catch(()=>{}),this.application.controlWindows.get(this.editor.id)?.id);
        return pending;
    }
    async prepared(value,edit,selection){
        const rendered=await this.renderText({value,edit,selection});
        const frame=copySceneFrame(rendered?.frame),presentation=shapedTextPresentation(rendered.presentation??{width:frame.width,height:frame.height,transform:[1,0,0,1,0,0]});
        if(presentation.width!==frame.width||presentation.height!==frame.height)throw RangeError('Shaped editor glyph and caret viewports differ');
        const surface=this.glyphSurfaces.find(surface=>surface!==this.glyphSurface),epoch=surface.submit(frame);
        const bound={...presentation,surface:String(surface.id),epoch:String(epoch)};
        const handle=await prepareShapedEditorLayout(this.editor,{text:value,map:rendered.map,presentation:bound});
        return {frame,handle,surface};
    }
    open(config){
        // Validate appearance before starting work; rejected replacements must
        // retain the currently visible edit and its source-bound geometry.
        this.checked({...config,paint:null},true);
        const generation=++this.layoutGeneration,value=config.value,selection=config.selection||{caret:Array.from(value).length,anchor:Array.from(value).length};
        return this.work(async()=>{
            if(generation!==this.layoutGeneration||this.destroyed)return false;
            let prepared;
            try{
                prepared=await this.prepared(value,null,selection);
                if(generation!==this.layoutGeneration||this.destroyed){await prepared.handle.dispose();return false;}
                const {handle,frame}=prepared,caret=handle.layout.stopAtScalar(selection.caret),anchor=handle.layout.stopAtScalar(selection.anchor);
                if(caret===null||anchor===null)throw RangeError('Scene selection has no native grapheme stop');
                this.preflightReference=documentEditorShapedReference({token:handle.token,caret,anchor,revision:1});
                this.checked({...config,paint:null},true);
                super.open({...config,paint:null});handle.activate({caret,anchor});this.shapedHandle=handle;
                for(const surface of this.glyphSurfaces)surface.visible=surface===prepared.surface;this.glyphSurface=prepared.surface;
                return this;
            }catch(error){await prepared?.handle.dispose().catch(()=>{});if(generation!==this.layoutGeneration||this.destroyed)return false;throw error;}
            finally{this.preflightReference=null;}
        });
    }
    refreshText({value=this.value,edit=null,changed=null}={}){
        if(!this.active)return Promise.resolve(false);
        const generation=++this.layoutGeneration,selection=this.editor.selection;
        return this.work(async()=>{
            if(generation!==this.layoutGeneration||this.destroyed)return false;
            let prepared;
            try{
                await changed;prepared=await this.prepared(value,edit,selection);
                if(generation!==this.layoutGeneration||!this.active||this.value!==value){await prepared.handle.dispose();return false;}
                const {handle,frame}=prepared,caret=handle.layout.stopAtScalar(selection.caret),anchor=handle.layout.stopAtScalar(selection.anchor);
                if(caret===null||anchor===null)throw RangeError('Edited selection has no native grapheme stop');
                handle.activate({caret,anchor});this.shapedHandle=handle;for(const surface of this.glyphSurfaces)surface.visible=surface===prepared.surface;this.glyphSurface=prepared.surface;
                return true;
            }catch(error){await prepared?.handle.dispose().catch(()=>{});if(generation===this.layoutGeneration)this.onError(error);return false;}
        });
    }
    refresh(config){
        this.checked({...config,paint:null,value:this.value},true);
        return super.refresh({...config,paint:null});
    }
    close(options){
        this.layoutGeneration++;for(const item of this.layoutKeys)item.resolve?.(false);this.layoutKeys=[];for(const surface of this.glyphSurfaces)surface.visible=false;this.glyphSurface=null;
        const handle=this.shapedHandle;this.shapedHandle=null;void handle?.dispose().catch(()=>{});
        return super.close(options);
    }
}
