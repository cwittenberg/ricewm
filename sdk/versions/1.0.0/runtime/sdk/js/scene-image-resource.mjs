// Internal immutable image snapshots. Native submissions remain complete,
// independently validated frames; no persistent resource IDs cross that boundary.
const resources = new WeakMap(), definitions = new WeakMap();
const CACHE_BYTES = 4 * 1024 * 1024, CACHE_ENTRIES = 64;
export function createSceneImageResource(width, height, rgba) {
    const resource = Object.freeze({});
    resources.set(resource, {width, height, rgba: rgba.slice(), regions: new Map(), regionBytes: 0});
    return resource;
}
export function copySceneImagePixels(resource) { return resources.get(resource).rgba.slice(); }
export function appendSceneImageResource(scene, id, resource) {
    if (!Number.isInteger(id) || id < 1 || id > 0xffffffff || !resources.has(resource)) throw RangeError('Invalid scene image resource');
    const image = resources.get(resource), size = 24 + image.rgba.length;
    if (scene.bytes + size > scene.maximumBytes) throw RangeError('Scene exceeds builder budget');
    const marker = new Uint8Array();
    definitions.set(marker, {id, image});
    scene.commands.push(marker); scene.bytes += size;
}
function imageCommand(id, width, height, rgba) {
    const bytes = new Uint8Array(24 + rgba.length), view = new DataView(bytes.buffer);
    bytes[0] = 14; view.setUint32(4, bytes.length, true); view.setUint32(8, id, true);
    view.setUint32(12, width, true); view.setUint32(16, height, true); view.setUint32(20, rgba.length, true);
    bytes.set(rgba, 24); return bytes;
}
function regionPixels(image, region) {
    const key = region.join(','), resident = image.regions.get(key);
    if (resident) { image.regions.delete(key); image.regions.set(key, resident); return resident; }
    const [x,y,width,height] = region, pixels = new Uint8Array(width*height*4);
    for (let row=0;row<height;row++) pixels.set(image.rgba.subarray(((y+row)*image.width+x)*4,((y+row)*image.width+x+width)*4),row*width*4);
    image.regions.set(key,pixels); image.regionBytes += pixels.length;
    while (image.regionBytes > CACHE_BYTES || image.regions.size > CACHE_ENTRIES) {
        const oldest=image.regions.keys().next().value;
        image.regionBytes -= image.regions.get(oldest).length; image.regions.delete(oldest);
    }
    return pixels;
}
export function finalizeSceneImageCommands(commands) {
    if (!commands.some(command=>definitions.has(command))) return {commands, bytes:commands.reduce((n,c)=>n+c.length,0)};
    const ids=new Set(), counts=new Map(), groups=new Map(), draws=[];
    const define=id=>{ids.add(id);counts.set(id,(counts.get(id)||0)+1);};
    for (const command of commands) {
        const definition=definitions.get(command);
        if (definition) { define(definition.id); groups.set(definition.id,{...definition,regions:new Map()}); continue; }
        const view=new DataView(command.buffer,command.byteOffset,command.byteLength);
        for (let offset=0;offset<command.length;) {
            if(offset+8>command.length)throw RangeError('Truncated scene command');
            const compact=view.getUint16(offset+2,true),length=compact?compact*4:view.getUint32(offset+4,true);
            if(length<8||offset+length>command.length)throw RangeError('Invalid scene command');
            if(command[offset]===14)define(view.getUint32(offset+8,true));
            if(command[offset]===7&&length===52)draws.push({command,offset,id:view.getUint32(offset+40,true),smoothing:view.getUint8(offset+48)===1,uv:[0,1,2,3].map(i=>view.getFloat32(offset+24+i*4,true))});
            offset+=length;
        }
    }
    for(const draw of draws) {
        const group=groups.get(draw.id);if(!group||counts.get(draw.id)!==1)continue;
        // Rebased float UVs can cross a nearest-neighbour texel boundary.
        // Explicit pixel art keeps its original texture coordinates exactly.
        if(!draw.smoothing)group.uncropped=true;
        const {image}=group,[u0,v0,u1,v1]=draw.uv;
        // Retain the neighbouring texels too: filtered sampling must match the
        // original atlas, including sprites without authored gutters.
        const x=Math.max(0,Math.floor(u0*image.width)-1),y=Math.max(0,Math.floor(v0*image.height)-1);
        const right=Math.min(image.width,Math.ceil(u1*image.width)+1),bottom=Math.min(image.height,Math.ceil(v1*image.height)+1);
        const rect=[x,y,right-x,bottom-y],key=rect.join(',');
        if(rect[2]<=0||rect[3]<=0)continue;
        if(!group.regions.has(key))group.regions.set(key,{rect,draws:[]});group.regions.get(key).draws.push(draw);
    }
    let nextId=0xffffffff;
    const replacements=new Map(), patches=new Map();
    for(const [id,group]of groups) {
        const {image}=group,fullBytes=24+image.rgba.length;
        const cropBytes=[...group.regions.values()].reduce((n,r)=>n+24+r.rect[2]*r.rect[3]*4,0);
        if(counts.get(id)!==1||group.uncropped||!group.regions.size||cropBytes>=fullBytes) { replacements.set(id,[imageCommand(id,image.width,image.height,image.rgba)]);continue; }
        const chunks=[];
        for(const region of group.regions.values()) {
            while(ids.has(nextId)&&nextId>0)nextId--;
            if(!nextId)throw RangeError('Scene image IDs exhausted');
            const croppedId=nextId--;ids.add(croppedId);const [x,y,width,height]=region.rect;
            chunks.push(imageCommand(croppedId,width,height,regionPixels(image,region.rect)));
            for(const draw of region.draws) {
                let copy=patches.get(draw.command);if(!copy){copy=draw.command.slice();patches.set(draw.command,copy);}
                const view=new DataView(copy.buffer,copy.byteOffset,copy.byteLength),uv=draw.uv;
                view.setUint32(draw.offset+40,croppedId,true);
                for(let i=0;i<4;i++)view.setFloat32(draw.offset+24+i*4,(uv[i]*(i%2?image.height:image.width)-(i%2?y:x))/(i%2?height:width),true);
            }
        }
        replacements.set(id,chunks);
    }
    const output=[];
    for(const command of commands){const definition=definitions.get(command);if(definition)output.push(...replacements.get(definition.id));else output.push(patches.get(command)||command);}
    return {commands:output,bytes:output.reduce((n,c)=>n+c.length,0)};
}
