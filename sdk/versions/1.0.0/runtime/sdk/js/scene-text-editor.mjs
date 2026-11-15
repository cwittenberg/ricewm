import {TextEditor} from './controls.mjs';
import {SceneOverlay} from './scene-overlay.mjs';
import {Message} from './messages.mjs';
import {State} from './control.mjs';
import {documentEditorPresentation} from './document-editor.mjs';

class SceneTextInput extends TextEditor {
    windowProc(event) {
        if(event.type===Message.KEY&&event.detail?.pressed&&this.routeKey?.(event))return true;
        return super.windowProc(event);
    }
}

/** A retained native text editor anchored to an existing scene. Callers own
 * source data, history and commits. Supply the resolved, opaque document paint;
 * this control never guesses a color from the current desktop theme. */
export class SceneTextEditor extends SceneOverlay {
    static family='composite';
    static constructorOptions=Object.freeze(['surface','onChange','onCommit','onNavigate','onError','tabBehavior','onEditKey']);
    static authoring=Object.freeze({
        constructor:'new SceneTextEditor({surface,onChange(value),onCommit?,onNavigate?(backwards),onError?,tabBehavior?:"insert"|"navigate",onEditKey?:(event)=>boolean,name})',
        methods:'Append after surface in the same Stack. open({id,value,bounds,font,paint,layout?,zoom?,selection?,point?}), refresh({id,bounds,font,paint,layout?,zoom?}), setSelectionAt({x,y,phase?,extend?,select?}), setSelection(anchor,caret), close(), input(keys), selectWord(); editor exposes native selection. Pointer requests remain revisioned even at the same coordinates; select:"word" waits for the native point selection. Bounds and points are normalized scene coordinates. Escape/Ctrl+Enter commit. tabBehavior defaults to navigate for compatibility; insert retains native text input and a literal source Tab. onEditKey may return true to consume a caller-owned source action before ordinary editing. A rejected/failed commit retains draft, selection and focus. Caller supplies source-resolved opaque background/foreground, geometry, font, layout and zoom; arbitrary rotated native widgets and transparent backgrounds are not supported. The control never owns document history.',
    });
    constructor({surface,onChange=()=>{},onCommit=()=>true,onNavigate=null,onError=()=>{},tabBehavior='navigate',onEditKey=null,name='scene-text-editor',...properties}={}) {
        for(const callback of [onChange,onCommit,onError])if(typeof callback!=='function')throw TypeError('SceneTextEditor callbacks must be functions');
        if(onNavigate!==null&&typeof onNavigate!=='function')throw TypeError('SceneTextEditor navigation must be a function');
        if(!['insert','navigate'].includes(tabBehavior)||onEditKey!==null&&typeof onEditKey!=='function')throw TypeError('Invalid scene text keyboard policy');
        // Native Tab traversal must defer to this control's commit/navigation
        // owner rather than moving focus before its KEY handler can run.
        const editor=new SceneTextInput({name:name+'-input',accessibleName:'Document text',value:'',rows:1,wrap:true,acceptTab:true,state:State.EXPANSIVE});
        super({...properties,name,surface,bounds:{x:0,y:0,width:1,height:1},visible:false},[editor]);
        Object.assign(this,{editor,onChange,onCommit,onNavigate,onError,tabBehavior,onEditKey});
        this.opened=false;this.identity=null;this.transition=null;this.navigating=false;this.generation=0;this.queued=[];this.wordAfterPoint=false;this.pointRevision=0;
        editor.on(Message.VALUE_CHANGED,event=>{if(this.opened)return this.onChange(String(event.detail?.value??editor.value),event.detail?.edit||null);});
        editor.on(Message.TEXT_SELECTION,()=>{if(this.wordAfterPoint){this.wordAfterPoint=false;this.selectWord();}});
        editor.routeKey=event=>{
            const key=event.detail;
            if(this.transition){
                if(this.queued.length<512&&this.queued.reduce((n,k)=>n+(k.text?.length||0),0)+(key.text?.length||0)<=16000)this.queued.push({...key});
                event.preventDefault();return true;
            }
            if(key.key==='Escape'||key.keyCode===9||((key.modifiers&4)&&(key.key==='Enter'||key.keyCode===36))){
                event.preventDefault();void this.finish(false);return true;
            }
            if(this.onEditKey?.(event)===true){event.preventDefault();return true;}
            if((key.key==='Tab'||key.keyCode===23)&&this.tabBehavior==='navigate'){event.preventDefault();void this.finish(true,Boolean(key.modifiers&1));return true;}
            return false;
        };
    }
    get active(){return this.opened;}
    get key(){return this.identity;}
    get value(){return this.editor.value;}
    get waiting(){return this.transition;}
    checked(config,withValue=false,shapedReference=null) {
        if(!config||typeof config.id!=='string'||!config.id||config.id.length>4096)throw TypeError('Scene text needs a bounded caller identity');
        const b=config.bounds;
        if(!b||Object.keys(b).some(k=>!['x','y','width','height'].includes(k))||!['x','y','width','height'].every(k=>Number.isFinite(b[k])&&Math.abs(b[k])<=16)||b.width<=0||b.height<=0)throw RangeError('Scene text needs normalized scene bounds');
        const values={documentFont:config.font,documentPaint:config.paint,documentLayout:config.layout??{},documentZoom:config.zoom??100,wrap:config.wrap!==false};
        if(!values.documentFont||!values.documentPaint&&this.constructor.supportsShapedText!==true)throw TypeError('Scene text requires resolved font and document paint');
        if(withValue)values.value=config.value;
        for(const [name,value]of Object.entries(values))values[name]=this.editor.propertyDescriptors[name].validate(value);
        if(withValue&&config.selection){
            const n=Array.from(values.value).length;
            if(![config.selection.anchor,config.selection.caret].every(v=>Number.isSafeInteger(v)&&v>=0&&v<=n))throw RangeError('Scene text selection exceeds its value');
        }
        if(withValue&&config.point){const {x,y,phase='click',extend=false,select}=config.point;if(![x,y].every(n=>Number.isFinite(n)&&Math.abs(n)<=16)||!['click','down','move','up'].includes(phase)||typeof extend!=='boolean'||select!==undefined&&select!=='word')throw RangeError('Invalid scene text selection point');}
        if(withValue){
            const p=config.point,point=p?{x:p.x,y:p.y,phase:p.phase||'click',extend:Boolean(p.extend),origin:p.phase==='down'?{x:p.x,y:p.y,extend:Boolean(p.extend)}:null,revision:Math.max(this.pointRevision,this.editor.documentSelectionPoint?.revision||0)+1}:null;
            documentEditorPresentation({id:this.editor.id||'18446744073709551615',values:{...this.editor.values,...values},documentSelectionPoint:point,documentShapedReference:shapedReference},`editor:plain:word:nolines:0:0\n${values.value}`);
        }
        return values;
    }
    appearance(config,values) {
        this.setBounds(config.bounds);
        for(const [name,value]of Object.entries(values))this.editor.setProperty(name,value);
    }
    open(config) {
        const values=this.checked(config,true);
        this.generation++;if(!this.navigating)this.queued=[];this.identity=config.id;this.wordAfterPoint=false;
        this.pointRevision=Math.max(this.pointRevision,this.editor.documentSelectionPoint?.revision||0);this.editor.documentSelectionPoint=null;
        this.appearance(config,values);this.opened=true;this.visible=true;
        this.editor.enabled=true;this.editor.setProperty('readOnly',false);
        if(config.selection)this.editor.setSelection(config.selection.anchor,config.selection.caret);
        this.surface.setProperty('focused',false);this.editor.setProperty('focused',true);
        if(config.point)this.setSelectionAt(config.point);
        return this;
    }
    setSelectionAt(point){
        if(!this.opened)return false;
        const {select,...coordinates}=point;
        if(select!==undefined&&select!=='word')throw TypeError('Scene text selection supports select:"word"');
        this.editor.setSelectionAt(coordinates);this.wordAfterPoint=select==='word';
        this.pointRevision=Math.max(this.pointRevision+1,this.editor.documentSelectionPoint.revision);
        this.editor.documentSelectionPoint=Object.freeze({...this.editor.documentSelectionPoint,revision:this.pointRevision});
        return true;
    }
    setSelection(anchor,caret=anchor){
        if(!this.opened)return false;
        return this.editor.setSelection(anchor,caret);
    }
    refresh(config) {
        if(!this.opened||config.id!==this.identity)return false;
        this.appearance(config,this.checked(config));return true;
    }
    selectWord() {
        const value=this.value,chars=Array.from(value),at=this.editor.selection.caret;
        if(!chars.length)return;
        const offset=chars.slice(0,Math.min(at,chars.length-1)).join('').length;
        let first=0,last=value.length,found=false;
        try{
            if(typeof globalThis.Intl?.Segmenter==='function')for(const span of new Intl.Segmenter(undefined,{granularity:'word'}).segment(value))if(offset>=span.index&&offset<span.index+span.segment.length){first=span.index;last=first+span.segment.length;found=true;break;}
        }catch{/* Restricted/older realms keep bounded Unicode word selection. */}
        if(!found){
            const part=c=>/\s/u.test(c)?0:/[\p{L}\p{N}\p{M}_]/u.test(c)?1:2;
            let a=Math.min(at,chars.length-1),b=a+1;const kind=part(chars[a]);
            while(a>0&&part(chars[a-1])===kind)a--;while(b<chars.length&&part(chars[b])===kind)b++;
            first=chars.slice(0,a).join('').length;last=chars.slice(0,b).join('').length;
        }
        this.setSelection(Array.from(value.slice(0,first)).length,Array.from(value.slice(0,last)).length);
    }
    input(keys) {for(const key of [...keys])this.editor.sendMessage(Message.KEY,key,{cancelable:true});}
    finish(navigate=false,backwards=false) {
        if(this.transition)return this.transition;
        if(!this.opened)return Promise.resolve(true);
        const generation=this.generation;
        this.navigating=Boolean(navigate&&this.onNavigate);
        this.transition=Promise.resolve().then(async()=>{
            const ok=await(navigate&&this.onNavigate?this.onNavigate(backwards):this.onCommit());
            if(ok===false){if(this.opened)this.editor.setProperty('focused',true);return false;}
            if(generation===this.generation){
                const queued=this.queued;
                this.close({restoreFocus:true});
                // Normal completion forwards input received while awaiting the
                // caller. Explicit close/replacement still discards stale keys.
                this.queued=queued;
            }
            return true;
        }).catch(error=>{if(this.opened)this.editor.setProperty('focused',true);this.onError(error);return false;}).finally(()=>{
            this.transition=null;this.navigating=false;const keys=this.queued;this.queued=[];
            const target=this.opened?this.editor:this.surface;
            if(!target.destroyed)for(const key of keys)target.sendMessage(Message.KEY,key,{cancelable:true});
        });
        return this.transition;
    }
    close({restoreFocus=false}={}) {this.generation++;this.opened=false;this.identity=null;this.wordAfterPoint=false;this.queued=[];this.editor.setProperty('focused',false);this.visible=false;if(restoreFocus&&!this.surface.destroyed)this.surface.setProperty('focused',true);return this;}
}
