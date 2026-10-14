// Exercise the actual RiceWM Wasm display core. No browser host or account.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {deflateSync} from 'node:zlib';

const [wasmPath, output] = process.argv.slice(2);
if (!wasmPath || !output) throw Error('usage: node examples/display-smoke.mjs <rice_display.wasm> <new-output.png>');
const {instance} = await WebAssembly.instantiate(await readFile(wasmPath), {});
const api = instance.exports, width = 960, height = 640;
assert.equal(api.rice_init(width, height), 1);
const client = api.rice_client_open();
assert.ok(client);
function stage(bytes) {
    const pointer = api.rice_io_reserve(bytes.length);
    new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
    return pointer;
}
function feed(bytes) {
    assert.equal(api.rice_client_feed(client, stage(bytes), bytes.length), bytes.length);
}
function drain() {
    const size = api.rice_client_pending(client), pointer = api.rice_io_reserve(size);
    assert.equal(api.rice_client_drain(client, pointer, size), size);
    return Buffer.from(new Uint8Array(api.memory.buffer, pointer, size));
}
function request(opcode, detail, body) {
    const bytes = Buffer.alloc(4 + body.length);
    bytes[0] = opcode; bytes[1] = detail; bytes.writeUInt16LE(bytes.length / 4, 2);body.copy(bytes, 4);
    feed(bytes);
}
feed(Buffer.from([108,0,11,0,0,0,0,0,0,0,0,0]));
const setup = drain(); assert.equal(setup[0], 1);
const resourceBase = setup.readUInt32LE(12);
for (let index = 0; index < 2; index++) {
    const id = resourceBase + 1 + index;
    const body = Buffer.alloc(36);
    body.writeUInt32LE(id, 0); body.writeUInt32LE(0x100, 4);
    body.writeInt16LE(70 + index * 300, 8);body.writeInt16LE(80 + index * 100, 10);
    body.writeUInt16LE(360, 12);body.writeUInt16LE(240, 14);
    body.writeUInt16LE(0, 16);body.writeUInt16LE(1, 18);body.writeUInt32LE(0x102, 20);
    body.writeUInt32LE((1 << 1) | (1 << 11), 24);
    body.writeUInt32LE(index ? 0xeee4d4 : 0xd6e6f2, 28);body.writeUInt32LE(0x28000f, 32);
    request(1, 24, body);
    const title = Buffer.from(index ? 'RiceWM window two' : 'RiceWM window one');
    const property = Buffer.alloc(20 + ((title.length + 3) & ~3));
    property.writeUInt32LE(id, 0);property.writeUInt32LE(39, 4);property.writeUInt32LE(31, 8);
    property[12] = 8;property.writeUInt32LE(title.length, 16);title.copy(property, 20);
    request(18, 0, property);
    const map = Buffer.alloc(4);map.writeUInt32LE(id);request(8, 0, map);
}
const windows = new Map();
function updateScene() {
    const size = api.rice_scene_pending(), pointer = api.rice_io_reserve(size);
    assert.equal(api.rice_scene_drain(pointer, size), size);
    const data = Buffer.from(new Uint8Array(api.memory.buffer, pointer, size));
    for (let offset = 0; offset < size; offset += 32) {
        const op = data[offset], id = data.readUInt32LE(offset + 4);
        if (op === 0) windows.clear();
        if (op === 2) windows.delete(id);
        if (op === 1) windows.set(id, {
            id, flags:data[offset + 1], parent:data.readUInt32LE(offset + 8),
            x:data.readInt32LE(offset + 12),y:data.readInt32LE(offset + 16),
            width:data.readUInt16LE(offset + 20),height:data.readUInt16LE(offset + 22),
            stack:data.readUInt16LE(offset + 26) & 0x3fff,
        });
    }
}
updateScene();
for (let index = 0; index < 2; index++) assert.ok(windows.get(resourceBase + 1 + index)?.flags & 1);
const clientWindow = windows.get(resourceBase + 2), frame = windows.get(clientWindow.parent);
assert.ok(frame && frame.id !== 0x100, 'actual window manager must reparent the client into a frame');
const old = {x:frame.x,y:frame.y};
const x = frame.x + Math.floor(frame.width / 2), y = frame.y + 10;
api.rice_input_pointer(0,0,x,y,0);api.rice_input_pointer(1,1,x,y,0);
api.rice_input_pointer(0,0,x+40,y+25,256);api.rice_input_pointer(2,1,x+40,y+25,0);
updateScene();
const moved = windows.get(frame.id);
assert.ok(moved.x !== old.x || moved.y !== old.y, 'title-bar drag must move the managed frame');
const rgba = Buffer.alloc(width*height*4);
for (let i=0;i<rgba.length;i+=4) {rgba[i]=32;rgba[i+1]=45;rgba[i+2]=64;rgba[i+3]=255;}
function paint(parent, ox, oy, clip, visited = new Set()) {
    const children = [...windows.values()].filter(w => w.parent === parent && w.id !== parent && w.flags & 1).sort((a,b)=>a.stack-b.stack);
    for (const window of children) {
        if (visited.has(window.id) || window.flags & 4) continue;
        visited.add(window.id);
        const x=ox+window.x,y=oy+window.y, count=window.width*window.height*4;
        if (!count) continue;
        const pointer=api.rice_io_reserve(count);
        const read=api.rice_surface_read(window.id,0,0,window.width,window.height,pointer,count);
        if (read !== count) throw Error('surface read failed');
        const pixels=Buffer.from(new Uint8Array(api.memory.buffer,pointer,count));
        const bounds={left:Math.max(0,clip.left,x),top:Math.max(0,clip.top,y),right:Math.min(width,clip.right,x+window.width),bottom:Math.min(height,clip.bottom,y+window.height)};
        for(let py=bounds.top;py<bounds.bottom;py++)for(let px=bounds.left;px<bounds.right;px++) {
            const src=((py-y)*window.width+px-x)*4,dst=(py*width+px)*4,a=pixels[src+3]/255;
            for(let c=0;c<3;c++)rgba[dst+c]=Math.round(pixels[src+c]*a+rgba[dst+c]*(1-a));
        }
        paint(window.id,x,y,bounds,visited);
    }
}
paint(0x100,0,0,{left:0,top:0,right:width,bottom:height});
function crc32(bytes) {let crc=0xffffffff;for(const byte of bytes){crc^=byte;for(let bit=0;bit<8;bit++)crc=(crc>>>1)^(0xedb88320&-(crc&1));}return (crc^0xffffffff)>>>0;}
function chunk(type,bytes) {const tag=Buffer.from(type),length=Buffer.alloc(4),crc=Buffer.alloc(4);length.writeUInt32BE(bytes.length);crc.writeUInt32BE(crc32(Buffer.concat([tag,bytes])));return Buffer.concat([length,tag,bytes,crc]);}
const header=Buffer.alloc(13);header.writeUInt32BE(width);header.writeUInt32BE(height,4);header[8]=8;header[9]=6;
const rows=Buffer.alloc(height*(width*4+1));for(let y=0;y<height;y++)rgba.copy(rows,y*(width*4+1)+1,y*width*4,(y+1)*width*4);
await writeFile(output,Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',header),chunk('IDAT',deflateSync(rows)),chunk('IEND',Buffer.alloc(0))]),{flag:'wx'});
assert.equal(api.rice_client_close(client),1);
console.log('PASS: Wasm X11 setup, two managed windows, title-bar drag and raster output:',output);
