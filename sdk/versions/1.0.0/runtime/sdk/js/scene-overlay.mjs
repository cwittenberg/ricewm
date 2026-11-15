import { Stack } from './controls.mjs';
import { SceneSurface, retainedScene } from './scene-surface.mjs';
import { Message } from './messages.mjs';
import { State } from './control.mjs';
function checkedBounds(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !['x','y','width','height'].includes(key)) ||
        !['x','y','width','height'].every(key => Number.isFinite(value[key]) && Math.abs(value[key]) <= 16) || value.width <= 0 || value.height <= 0) throw new RangeError('Scene overlay requires bounded normalized scene coordinates');
    return Object.freeze({...value});
}
/** Native child widgets anchored to the exact fitted/zoomed SceneSurface.
 * Append after surface in the same Stack. Bounds are fractions of the full
 * scene, including its offscreen region when panned; the parent clips input. */
export class SceneOverlay extends Stack {
    static constructorOptions = Object.freeze(["surface", "bounds"]);
    constructor({surface,bounds,...properties},children=[]) {
        if (!(surface instanceof SceneSurface)) throw new TypeError('SceneOverlay requires a SceneSurface');
        super(properties,children); this.surface=surface; this.bounds=checkedBounds(bounds);
    }
    setBounds(bounds) { this.bounds=checkedBounds(bounds);this.application?.invalidate(this);return this; }
    toWidget() { return Object.freeze({...super.toWidget(),text:`panel:scene-overlay:${this.surface.id}:${['x','y','width','height'].map(key=>Math.round(this.bounds[key]*1000000)).join(':')}:flush`}); }
}

/** Native pointer targets above embedded editors. The caller paints its own
 * handles and owns gestures/history; these transparent surfaces retain native
 * capture and convert captured movement back to the source scene's coordinates. */
export class SceneHandleOverlay extends SceneOverlay {
    static family='composite';
    static constructorOptions=Object.freeze(['surface','handles','hitSize','scale','onPointer']);
    static authoring=Object.freeze({
        topics:Object.freeze(['visual','documents','editing']),
        constructor:'new SceneHandleOverlay({surface,handles:[{id:"north",point:[.5,.2]}],scale:1,hitSize:16,onPointer(handle,event)})',
        methods:'Append after scene editors in the same Stack. setHandles(handles), setScale(pixelsPerSceneUnit), setHitSize(screenPixels). At most 32 unique caller identities, with normalized scene points; hitSize is 1..64 screen pixels. Transparent native targets preserve pointer capture above embedded editors. Callback event.detail uses the original SceneSurface viewport coordinates, including captured movement outside the target. Caller paints handles, routes input and owns commits/cancellation/history. Stable identities retain native widgets through geometry updates.',
    });
    constructor({surface,handles=[],hitSize=16,scale=1,onPointer=()=>{},name='scene-handles',...properties}={}) {
        if(!(surface instanceof SceneSurface)||typeof onPointer!=='function')throw TypeError('Scene handles need a surface and pointer callback');
        super({state:State.EXPANSIVE,...properties,name,surface,bounds:{x:0,y:0,width:1,height:1}});
        this.surface=surface;this.onPointer=onPointer;this.records=new Map();this.serial=0;
        this.hitSize=this.checkedHitSize(hitSize);this.scale=this.checkedScale(scale);this.handles=[];
        // Every native anchored overlay requires a sibling source surface.
        // This transparent, passive anchor gives nested targets that same
        // checked relationship without changing native layout admission.
        this.anchor=this.append(new SceneSurface({enabled:false,backgroundColor:'transparent',state:State.EXPANSIVE}));
        this.anchorSize=null;
        this.stopSize=surface.on(Message.CHANGE,event=>{if(event.detail?.property==='frameSize')this.setHandles(this.handles);});
        try{this.setHandles(handles);}catch(error){this.stopSize();throw error;}
    }
    checkedScale(value){if(!Number.isFinite(value)||value<=0||value>1024)throw RangeError('Invalid scene handle scale');return value;}
    checkedHitSize(value){if(!Number.isFinite(value)||value<1||value>64)throw RangeError('Invalid scene handle hit size');return value;}
    setScale(value){value=this.checkedScale(value);if(value!==this.scale){const previous=this.scale;this.scale=value;try{this.setHandles(this.handles);}catch(error){this.scale=previous;throw error;}}return this;}
    setHitSize(value){value=this.checkedHitSize(value);const previous=this.hitSize;this.hitSize=value;try{this.setHandles(this.handles);}catch(error){this.hitSize=previous;throw error;}return this;}
    setHandles(handles){
        if(!Array.isArray(handles)||handles.length>32)throw RangeError('Scene handles exceed their bound');
        const ids=new Set(),frame=retainedScene(this.surface);
        const checked=handles.map(handle=>{
            if(!handle||Object.keys(handle).some(key=>!['id','point'].includes(key))||typeof handle.id!=='string'||!handle.id||handle.id.length>64||ids.has(handle.id)||
                !Array.isArray(handle.point)||handle.point.length!==2||!handle.point.every(value=>Number.isFinite(value)&&Math.abs(value)<=15))throw TypeError('Invalid scene handle');
            ids.add(handle.id);const value=Object.freeze({id:handle.id,point:Object.freeze([...handle.point])});
            const width=frame?this.hitSize/(this.scale*frame.width):1,height=frame?this.hitSize/(this.scale*frame.height):1;
            const bounds=checkedBounds({x:value.point[0]-width/2,y:value.point[1]-height/2,width,height});
            return {value,bounds};
        });
        if(frame&&(!this.anchorSize||this.anchorSize.width!==frame.width||this.anchorSize.height!==frame.height)){
            this.anchor.submit({width:frame.width,height:frame.height,displayList:new Uint8Array()});
            this.anchorSize={width:frame.width,height:frame.height};
        }
        this.visible=Boolean(frame&&checked.length);
        for(const[id,record]of this.records)if(!ids.has(id)){this.remove(record.overlay,{destroy:true});this.records.delete(id);}
        for(const{value,bounds}of checked){
            let record=this.records.get(value.id);
            if(!record){
                const input=new SceneSurface({name:`${(this.name||'scene-handles').slice(0,95)}-handle-${++this.serial}`,backgroundColor:'transparent',state:State.EXPANSIVE});
                // Explicit transparent no-op, rather than the retained-reference
                // sentinel; no document artwork is duplicated by the hit layer.
                input.submit({width:16,height:16,displayList:new Uint8Array()});
                const overlay=new SceneOverlay({surface:this.anchor,bounds,visible:Boolean(frame)},[input]);
                record={input,overlay,value};this.records.set(value.id,record);this.append(overlay);
                input.on(Message.POINTER,event=>{
                    const current=retainedScene(this.surface);if(!current)return;
                    const d=event.detail,b=record.overlay.bounds;
                    const detail=Object.freeze({...d,x:(b.x+d.x/16*b.width)*current.width,y:(b.y+d.y/16*b.height)*current.height});
                    return this.onPointer(record.value,Object.freeze({...event,detail,preventDefault:()=>event.preventDefault(),stopPropagation:()=>event.stopPropagation()}));
                });
                input.on(Message.KEY,event=>{
                    if(this.surface.sendMessage(Message.KEY,event.detail)){event.preventDefault();event.stopPropagation();}
                });
            }
            record.value=value;record.overlay.setBounds(bounds);record.overlay.visible=Boolean(frame);
        }
        this.handles=Object.freeze(checked.map(item=>item.value));return this;
    }
    destroy(){this.stopSize?.();this.records.clear();return super.destroy();}
}
