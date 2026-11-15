import {riceGnomePanelCanvas} from './rice-gnome-panel-frames.mjs';

// Extension-owned chrome is a compositor plane, not an application. This keeps
// real app focus and window inventories intact while retaining upstream actors.
export function readRiceGnomeOverlayFrames(values,previous=new Map()) {
    if(!Array.isArray(values)||values.length>16)throw TypeError('Invalid GNOME overlay frames');
    const result=new Map();let bytes=0;
    for(const value of values){
        const {uuid,generation,width,height,pixels,revision}=value||{},key=value?.key;
        if(typeof uuid!=='string'||!uuid||uuid.length>256||typeof key!=='string'||!key||key.length>320||result.has(key)||
            !Number.isSafeInteger(generation)||generation<1||!Number.isSafeInteger(revision)||revision<1||
            !Number.isInteger(width)||width<1||width>2048||!Number.isInteger(height)||height<1||height>2048||
            !(pixels instanceof Uint32Array)||pixels.length!==width*height)throw TypeError('Invalid GNOME overlay frame');
        bytes+=pixels.byteLength;if(bytes>16*1024*1024)throw RangeError('GNOME overlay frame budget exceeded');
        if(!Array.isArray(value.hits)||value.hits.length>2048||value.hits.some(hit=>!Number.isSafeInteger(hit.actorId)||hit.actorId<1||
            !['x','y','width','height'].every(k=>Number.isFinite(hit[k]))||hit.x<0||hit.y<0||hit.width<=0||hit.height<=0||hit.x+hit.width>width||hit.y+hit.height>height))throw TypeError('Invalid GNOME overlay input region');
        const old=previous.get(key);
        result.set(key,{...value,surface:old?.uuid===uuid&&old.generation===generation&&old.revision===revision&&old.width===width&&old.height===height?old.surface:null});
    }
    return result;
}
export function paintRiceGnomeOverlayFrames(context,frames,factory,width,height){
    for(const frame of frames.values())context.drawImage(riceGnomePanelCanvas(frame,factory),0,0,width,height);
}

export function riceGnomeOverlayInputTarget(frames,event,viewport){
    if(!['pointer-button','pointer-motion'].includes(event?.input)||![event.x,event.y,viewport.width,viewport.height].every(Number.isFinite)||viewport.width<=0||viewport.height<=0)return null;
    for(const frame of [...frames.values()].reverse()){
        const x=event.x*frame.width/viewport.width,y=event.y*frame.height/viewport.height;
        const hit=[...(frame.hits||[])].reverse().find(h=>x>=h.x&&y>=h.y&&x<h.x+h.width&&y<h.y+h.height);
        if(hit)return {uuid:frame.uuid,generation:frame.generation,actorId:hit.actorId};
    }
    return null;
}
