import { textClipRect } from './text-overflow.mjs';
import { shapeById } from './shape-catalog.mjs';
import { cellKey } from './table-values.mjs';

const identity = [1, 0, 0, 1, 0, 0];
const frameCache = new WeakMap(), imageCache = new WeakMap();
const finite = value => { if (!Number.isFinite(value)) throw new RangeError('Nonfinite ink coordinate'); return value; };
const point = (m, x, y) => [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
const multiply = (a, b) => [a[0]*b[0]+a[2]*b[1], a[1]*b[0]+a[3]*b[1], a[0]*b[2]+a[2]*b[3], a[1]*b[2]+a[3]*b[3], a[0]*b[4]+a[2]*b[5]+a[4], a[1]*b[4]+a[3]*b[5]+a[5]];
const rect = b => [b.x, b.y, b.width, b.height];
const alpha = paint => paint && paint.opacity > 0;
const rgba = color => color >>> 24;
function union(a, b) {
    if (!a) return b?.slice() ?? null;
    if (!b) return a.slice();
    const x = Math.min(a[0], b[0]), y = Math.min(a[1], b[1]);
    return [x, y, Math.max(a[0]+a[2], b[0]+b[2])-x, Math.max(a[1]+a[3], b[1]+b[3])-y];
}
function intersect(a, b) {
    if (!a || !b) return a?.slice() ?? null;
    const x = Math.max(a[0], b[0]), y = Math.max(a[1], b[1]);
    return [x, y, Math.max(0, Math.min(a[0]+a[2], b[0]+b[2])-x), Math.max(0, Math.min(a[1]+a[3], b[1]+b[3])-y)];
}
function hull(points) {
    let bounds = null;
    for (const [x, y] of points) bounds = union(bounds, [finite(x), finite(y), 0, 0]);
    return bounds;
}
function transformRect(b, m) {
    if (!b) return null;
    const [x,y,w,h] = b;
    return hull([[x,y],[x+w,y],[x+w,y+h],[x,y+h]].map(([x,y]) => point(m,x,y)));
}
function expand(b, radius, m = identity) {
    if (!b) return null;
    const x = radius*Math.hypot(m[0],m[2]), y = radius*Math.hypot(m[1],m[3]);
    return [b[0]-x,b[1]-y,b[2]+2*x,b[3]+2*y];
}
const nonempty = b => b && b.every(Number.isFinite) && b[2] > 0 && b[3] > 0 ? b : null;
function pathHull(segments, m = identity) {
    const points = []; let drawn = false;
    for (const [verb,...values] of segments) {
        const count = { M:2,L:2,Q:4,C:6,Z:0 }[verb];
        if (count === undefined || values.length !== count) throw new RangeError('Invalid ink path');
        if (verb !== 'M' && verb !== 'Z') drawn = true;
        for (let i=0;i<count;i+=2) points.push(point(m,finite(values[i]),finite(values[i+1])));
    }
    return drawn ? hull(points) : null;
}
function imageInk(image, destination, uv = [0,0,1,1], smoothing = true) {
    if (!image || !destination || destination[2] <= 0 || destination[3] <= 0) return null;
    const { width, height, rgba: pixels } = image;
    if (!(pixels instanceof Uint8Array) || !Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || pixels.length !== width*height*4) throw new RangeError('Invalid ink image');
    let extent = imageCache.get(pixels);
    if (!extent) {
        let left=width,top=height,right=0,bottom=0;
        for(let y=0;y<height;y++) for(let x=0;x<width;x++) if(pixels[(y*width+x)*4+3]) {
            left=Math.min(left,x);top=Math.min(top,y);right=Math.max(right,x+1);bottom=Math.max(bottom,y+1);
        }
        extent = { width,height,bounds:right>left&&bottom>top?[left,top,right-left,bottom-top]:null };
        imageCache.set(pixels,extent);
    }
    if (extent.width !== width || extent.height !== height) throw new RangeError('Image dimensions changed for retained pixels');
    if (!extent.bounds) return null;
    if (uv.length !== 4 || uv.some(v=>!Number.isFinite(v)) || uv[2] <= uv[0] || uv[3] <= uv[1]) throw new RangeError('Invalid ink image crop');
    // Bilinear sampling can include an adjacent transparent texel. This expands
    // only capture bounds; the real image alpha remains the shadow mask.
    const bounds=intersect(expand(extent.bounds,smoothing?1:0),[uv[0]*width,uv[1]*height,(uv[2]-uv[0])*width,(uv[3]-uv[1])*height]);
    if (!nonempty(bounds)) return null;
    const sx=destination[2]/((uv[2]-uv[0])*width),sy=destination[3]/((uv[3]-uv[1])*height);
    return [destination[0]+(bounds[0]-uv[0]*width)*sx,destination[1]+(bounds[1]-uv[1]*height)*sy,bounds[2]*sx,bounds[3]*sy];
}

/** Conservative source-ink AABB of a retained native vector display list.
 * Resource definitions do not paint. Glyph paths use their actual outlines,
 * including negative bearings/italic overhang, not line or text-box metrics.
 * Curves use their control-point hull; transformed clips use conservative AABBs.
 * Unknown paint commands reject rather than silently underestimating capture.
 * Cached input bytes and image pixel arrays must be treated as immutable. */
export function displayListInkBounds(bytes, { transform = identity, clip = null } = {}) {
    if (!(bytes instanceof Uint8Array) || bytes.length > 8*1024*1024) throw new RangeError('Invalid ink display list');
    if (!Array.isArray(transform) || transform.length !== 6 || transform.some(v=>!Number.isFinite(v))) throw new RangeError('Invalid ink transform');
    const cached = transform === identity && clip === null;
    if (cached && frameCache.has(bytes)) return frameCache.get(bytes)?.slice() ?? null;
    const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),paths=new Map(),images=new Map(),stack=[];
    let matrix=transform.slice(),clipping=clip,opacity=1,bounds=null,count=0;
    function paint(b) { if(opacity>0) {const visible=nonempty(intersect(b,clipping));if(visible)bounds=union(bounds,visible);} }
    for(let at=0;at<bytes.length;) {
        if(at+8>bytes.length)throw new RangeError('Truncated ink command');
        const op=view.getUint8(at),compact=view.getUint16(at+2,true),extended=view.getUint32(at+4,true),length=compact?compact*4:extended;
        if(view.getUint8(at+1)||compact&&extended||length<8||length%4||at+length>bytes.length||++count>262144)throw new RangeError('Invalid ink command envelope');
        const size=(...allowed)=>{if(!allowed.includes(length))throw new RangeError('Invalid ink command size');};
        const u=n=>{if(n+4>length)throw new RangeError('Truncated ink field');return view.getUint32(at+n,true);};
        const f=n=>{if(n+4>length)throw new RangeError('Truncated ink field');return finite(view.getFloat32(at+n,true));};
        const floats=(n,c)=>Array.from({length:c},(_,i)=>f(n+i*4));
        if(op===1) {size(36,40);stack.push({kind:'transform',matrix,clipping,opacity});matrix=multiply(matrix,floats(8,6));opacity*=f(32);}
        else if(op===2||op===4) {size(8);const saved=stack.pop();if(saved?.kind!==(op===2?'transform':'clip'))throw new RangeError('Unbalanced ink scope');({matrix,clipping,opacity}=saved);}
        else if(op===3) {size(40);stack.push({kind:'clip',matrix,clipping,opacity});const next=transformRect(floats(8,4),matrix);clipping=clipping?intersect(clipping,next):next;}
        else if(op===19) {
            const id=u(8),n=u(12);if(!id||n<1||n>16384||length!==16+n*28||paths.has(id))throw new RangeError('Invalid ink path resource');
            const verbs=['M','L','Q','C','Z'],counts=[2,2,4,6,0],segments=[];
            for(let i=0;i<n;i++){const offset=16+i*28,verb=view.getUint8(at+offset);if(verb>4)throw new RangeError('Invalid ink path verb');segments.push([verbs[verb],...floats(offset+4,counts[verb])]);}
            paths.set(id,segments);
        } else if(op===20||op===21||op===22) {
            size(op===20?40:op===21?44:52);const path=paths.get(u(8));if(!path)throw new RangeError('Undefined ink path');
            const m=multiply(matrix,floats(12,6));let b=pathHull(path,m);
            if(op===20) {stack.push({kind:'clip',matrix,clipping,opacity});clipping=clipping?intersect(clipping,b??[0,0,0,0]):b??[0,0,0,0];}
            else if(rgba(u(36))) {
                if(op===22){const width=f(40),miter=f(44),cap=view.getUint8(at+48),join=view.getUint8(at+49);if(width<0||miter<0||cap>2||join>2)throw new RangeError('Invalid ink stroke');if(width===0){at+=length;continue;}b=expand(b,width/2*Math.max(cap===2?Math.SQRT2:1,join===1?miter:1),m);}
                paint(b);
            }
        } else if(op===14) {
            const id=u(8),width=u(12),height=u(16),n=u(20);if(!id||images.has(id)||!width||!height||n!==width*height*4||length!==24+n)throw new RangeError('Invalid ink image resource');
            images.set(id,{width,height,rgba:bytes.subarray(at+24,at+length)});
        } else if(op===7) {size(52);const image=images.get(u(40));if(!image)throw new RangeError('Undefined ink image');if(f(44)>0)paint(transformRect(imageInk(image,floats(8,4),floats(24,4),view.getUint8(at+48)!==0),matrix));}
        else if(op===5) {size(28);if(rgba(u(24)))paint(transformRect(floats(8,4),matrix));}
        else if(op===11) {size(36);if(rgba(u(28))&&f(24)>0)paint(expand(hull([point(matrix,f(8),f(12)),point(matrix,f(16),f(20))]),f(24)/2,matrix));}
        else if(op===13) {
            size(44);if(rgba(u(36))){const [x,y,w,h]=floats(8,4),center=point(matrix,x+w/2,y+h/2),rx=Math.hypot(matrix[0]*w/2,matrix[2]*h/2),ry=Math.hypot(matrix[1]*w/2,matrix[3]*h/2);let b=[center[0]-rx,center[1]-ry,2*rx,2*ry];if(!view.getUint8(at+40))b=f(32)>0?expand(b,f(32)/2,matrix):null;paint(b);}
        } else if(op===6||op===25||op===26) {
            const field=op===6?40:op===25?48:44;if(length<field+4)throw new RangeError('Truncated ink gradient');const n=view.getUint16(at+field,true);if(n<2||length!==field+4+n*8)throw new RangeError('Invalid ink gradient');
            if(Array.from({length:n},(_,i)=>rgba(u(field+8+i*8))).some(Boolean))paint(transformRect(floats(8,4),matrix));
        } else if(op===9) {size(72);if([0,1,2,3].some(i=>f(24+i*4)>0&&rgba(u(40+i*4))))paint(transformRect(floats(8,4),matrix));}
        else if(op===12) {
            if(length<44)throw new RangeError('Truncated ink polygon');const n=view.getUint16(at+16,true),flags=view.getUint8(at+18);if(n<3||length!==20+n*8)throw new RangeError('Invalid ink polygon');
            if(rgba(u(8))){let b=hull(Array.from({length:n},(_,i)=>point(matrix,f(20+i*8),f(24+i*8))));if(!(flags&1))b=f(12)>0?expand(b,f(12)*5,matrix):null;paint(b);}
        } else if(op===15) { /* Hit regions do not paint. */ }
        else throw new RangeError('Unsupported ink paint opcode '+op);
        at+=length;
    }
    if(stack.length)throw new RangeError('Unbalanced ink scope');
    bounds=nonempty(bounds);if(cached)frameCache.set(bytes,bounds?.slice()??null);return bounds;
}

/** Source paint bounds before object.transform, excluding selection, guides and
 * shadows. Returns null when the rendered object contributes no alpha. Group
 * callers must union transformed descendant bounds themselves. */
export function objectInkBounds(scene, object, { textFrames = new Map(), imageFrames = new Map(), placeholderGuides = false } = {}) {
    if(!object?.bounds||object.kind==='group')return null;
    const b=rect(object.bounds);let result=null;
    const add=value=>{if(nonempty(value))result=union(result,value);};
    const text=(frame,clip)=>{if(frame&&(!frame.placeholder||placeholderGuides))add(intersect(displayListInkBounds(frame.displayList),clip));};
    if(object.geometry==='line') {
        if(!alpha(object.stroke)||object.stroke.width<=0)return null;
        if(!object.line)return expand(hull([[b[0],b[1]],[b[0]+b[2],b[1]+b[3]]]),object.stroke.width/2);
        const [a,c,d,e,x,y]=object.transform??identity,det=a*e-c*d;
        if(!det)throw new RangeError('Singular line ink transform');
        const inverse=[e/det,-c/det,-d/det,a/det,(d*y-e*x)/det,(c*x-a*y)/det];
        const points=[object.line.start,object.line.end].map(p=>point(inverse,p.x,p.y));
        return expand(hull(points),object.stroke.width/2,inverse);
    }
    if(object.table) {
        for(const cell of object.table.cells||[]) {
            if(cell.covered)continue;const cb=rect(cell.bounds),[x,y,w,h]=cb;
            if(alpha(cell.fill))add(cb);
            const edges={top:[[x,y],[x+w,y]],right:[[x+w,y],[x+w,y+h]],bottom:[[x+w,y+h],[x,y+h]],left:[[x,y+h],[x,y]]};
            for(const [side,points] of Object.entries(edges)){const stroke=cell.borders?.[side];if(alpha(stroke)&&stroke.width>0)add(expand(hull(points),stroke.width/2));}
            text(textFrames.get(cellKey(object.id,cell.row,cell.column)),cb);
        }
    } else {
        let paths=object.paths;
        if(!paths?.length&&object.geometry!=='rect'&&object.geometry!=='ellipse') {
            const source=shapeById(object.geometry)?.path;
            if(source?.length)paths=[{fill:true,stroke:true,commands:source.map(([verb,...values])=>[verb,...values.map((v,i)=>v/1000*(i%2?b[3]:b[2])+(i%2?b[1]:b[0]))])}];
        }
        if(paths?.length)for(const path of paths){const pb=pathHull(path.commands);if(path.fill&&alpha(object.fill))add(pb);if(path.stroke&&alpha(object.stroke)&&object.stroke.width>0)add(expand(pb,object.stroke.width/2));}
        else if(['rect','ellipse'].includes(object.geometry)) {if(alpha(object.fill))add(b);if(alpha(object.stroke)&&object.stroke.width>0)add(expand(b,object.stroke.width/2));}
        const image=imageFrames.get(object.id);
        if(image&&object.image){const {left=0,top=0,right=0,bottom=0}=object.image.crop||{};add(imageInk(image,b,[left,top,1-right,1-bottom]));}
        const frame=textFrames.get(object.id);
        if(frame)text(frame,textClipRect(scene,object));
    }
    return nonempty(result);
}
