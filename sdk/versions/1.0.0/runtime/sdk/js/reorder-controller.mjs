import {Button} from './controls.mjs';
import {Message} from './messages.mjs';

let serial=0;
const owners=new WeakMap();
const parse=value=>{
    if(typeof value!=='string'||!value.startsWith('rice-reorder:')||value.length>1024)return null;
    try{const data=JSON.parse(value.slice(13));
        if(!data||Object.keys(data).sort().join(',')!=='phase,side,token,x,y'||!['move','up'].includes(data.phase)||
            ![data.x,data.y].every(value=>Number.isInteger(value)&&value>=-2147483648&&value<=2147483647)||
            !(data.token===null&&data.side===null||typeof data.token==='string'&&/^[0-9]{1,5}$/.test(data.token)&&Number(data.token)<=65535&&['before','after'].includes(data.side)))return null;
        return Object.freeze(data);
    }catch{return null;}
};

/** A native Button with ordinary activation and an opt-in captured grip. */
export class ReorderButton extends Button {
    static authoring=Object.freeze({topics:Object.freeze(['visual','navigation','product','documents']),constructor:'new ReorderButton({name,accessibleName},[content])',methods:'Native Button input for ReorderController. With no controller it behaves as Button. Attached grips retain native capture, focus and keyboard activation. Keep preview artwork/labels in child controls; the controller owns the native grip presentation until detached.'});
    setReorderMarker(group,token){
        if(typeof group!=='string'||!/^rw-reorder\.[hv]\.r[1-9][0-9]*$/.test(group)||!Number.isInteger(token)||token<0||token>65535)throw TypeError('Invalid reorder marker');
        this.reorderMarker=`drag-handle:${group}:flush:${token}`;this.detachedRelease=false;this.application?.invalidate(this);
    }
    releaseReorderMarker(defer=false){this.detachedRelease=Boolean(defer&&this.reorderMarker);if(!this.detachedRelease){this.reorderMarker=null;this.application?.invalidate(this);}}
    toWidget(){const widget=super.toWidget();return this.reorderMarker?Object.freeze({...widget,text:this.reorderMarker}):widget;}
    dispatchHostEvent(event){
        if(event?.type==='pointer'&&event.kind==='down'&&this.detachedRelease)this.releaseReorderMarker(false);
        if(event?.type==='value-changed'&&this.reorderMarker){
            const detail=parse(event.value);
            if(detail){if(detail.phase==='up'&&this.detachedRelease)this.releaseReorderMarker(false);return this.sendMessage(Message.REORDER,detail);}
            // An older host's grip feedback cannot invent an insertion side.
            if(typeof event.value==='string'&&/^(preview|drop):/.test(event.value))return false;
        }
        return super.dispatchHostEvent(event);
    }
}

/** Caller-owned stable order and one transaction per completed drag. */
export class ReorderController {
    static controlFactory=Object.freeze({topics:['visual','navigation','product','documents'],constructor:'new ReorderController({items:["a","b"],axis:"horizontal",threshold:6,onPreview,onCommit,onCancel})',methods:'attach(reorderButton,id), detach(reorderButton), setItems(ids), setAxis("horizontal"|"vertical"), setEnabled(boolean), cancel(), dispose(); dragging, sourceId, pending. sourceId lets a virtualized owner retain its captured input while scrolling. onPreview receives {id,before,index,sourceIndex,point}; null clears feedback. onCommit receives one completed non-noop move; callbacks own data, errors and undo. Plain clicks retain activation. Escape/blur, changed order and removal cancel. Native hit ownership and captured pointer coordinates compute before/after from actual hit geometry; no DOM or global pointer listener is needed.'});
    constructor({items=[],axis='horizontal',threshold=6,onPreview=()=>{},onCommit=()=>{},onCancel=()=>{}}={}){
        if(!Number.isFinite(threshold)||threshold<0||threshold>64||![onPreview,onCommit,onCancel].every(value=>typeof value==='function'))throw TypeError('Invalid reorder controller');
        this.serial=++serial;this.records=new Map();this.active=null;this.pending=false;this.enabled=true;this.disposed=false;
        this.threshold=threshold;this.onPreview=onPreview;this.onCommit=onCommit;this.onCancel=onCancel;this.items=[];this.setAxis(axis);this.setItems(items);
    }
    get dragging(){return Boolean(this.active?.moved);}
    get sourceId(){return this.active?.record.id??null;}
    marker(record){record.control.setReorderMarker(`rw-reorder.${this.axis==='horizontal'?'h':'v'}.r${this.serial}`,this.items.indexOf(record.id));}
    setItems(items){
        if(!Array.isArray(items)||items.length>65536||items.some(id=>typeof id!=='string'||!id||id.length>256)||new Set(items).size!==items.length)throw TypeError('Invalid reorder identities');
        if(this.items.length===items.length&&this.items.every((id,index)=>id===items[index]))return false;
        this.cancel();this.items=[...items];
        for(const record of [...this.records.values()])if(!this.items.includes(record.id))this.detach(record.control);else this.marker(record);
        return true;
    }
    setAxis(axis){if(!['horizontal','vertical'].includes(axis))throw TypeError('Invalid reorder axis');if(this.axis===axis)return false;this.cancel();this.axis=axis;for(const record of this.records.values())this.marker(record);return true;}
    setEnabled(enabled){if(typeof enabled!=='boolean')throw TypeError('Reorder readiness requires a boolean');this.enabled=enabled;if(!enabled)this.cancel();}
    attach(control,id){
        if(this.disposed||!(control instanceof ReorderButton)||control.destroyed||!this.items.includes(id)||owners.has(control))throw TypeError('Reorder attachment requires an owned input and known identity');
        const record={control,id,stops:[]};this.records.set(control,record);owners.set(control,this);this.marker(record);
        record.stops.push(control.on(Message.POINTER,event=>{
            const d=event.detail;
            if(d?.kind==='down'){
                this.cancel();
                if(d.button===1&&this.enabled&&!this.pending&&!this.disposed)this.active={record,x:d.x,y:d.y,moved:false,preview:null};
            }else if(d?.kind==='leave'&&this.active?.record===record)this.cancel();
        }),control.on(Message.REORDER,event=>this.receive(record,event.detail)),control.on(Message.BLUR,()=>{if(this.active?.record===record)this.cancel();}),
        control.on(Message.KEY,event=>{if(this.active&&event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){this.cancel();event.preventDefault();event.stopPropagation();}}),
        control.on(Message.DESTROY,()=>this.detach(control)));
        return()=>this.detach(control);
    }
    receive(record,data){
        const active=this.active;if(!active||active.record!==record||!this.enabled||this.disposed||this.pending)return false;
        if(![data?.x,data?.y].every(Number.isFinite))return false;
        if(!active.moved&&Math.hypot(data.x-active.x,data.y-active.y)>=this.threshold)active.moved=true;
        if(active.moved){
            let slot=data.token===null?this.items.length:Number(data.token)+(data.side==='after'?1:0);
            if(!Number.isInteger(slot)||slot<0||slot>this.items.length){this.cancel();return false;}
            const sourceIndex=this.items.indexOf(record.id),before=this.items[slot]??null,index=slot>sourceIndex?slot-1:slot;
            active.preview=Object.freeze({id:record.id,before,index,sourceIndex,point:Object.freeze({x:data.x,y:data.y})});
            this.onPreview(active.preview);
        }
        if(data.phase!=='up')return true;
        this.active=null;this.onPreview(null);
        if(!active.moved){record.control.emit(Message.ACTIVATE);return true;}
        const move=active.preview;if(!move||move.sourceIndex===move.index)return false;
        this.pending=true;
        return Promise.resolve().then(()=>this.onCommit(move)).finally(()=>{this.pending=false;});
    }
    cancel(){const active=this.active;this.active=null;if(active){this.onPreview(null);this.onCancel();}return Boolean(active);}
    detach(control){const record=this.records.get(control);if(!record)return false;const captured=this.active?.record===record;if(captured)this.cancel();this.records.delete(control);owners.delete(control);for(const stop of record.stops)stop();if(!control.destroyed)control.releaseReorderMarker(captured);return true;}
    dispose(){if(this.disposed)return;this.disposed=true;for(const control of [...this.records.keys()])this.detach(control);this.cancel();}
}
