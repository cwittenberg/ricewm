// Outer shadows of actual composited content alpha. No shape or app knowledge.
// A three-box separable Gaussian approximation keeps work linear in pixels.
const MAX_PIXELS = 1024 * 1024, MAX_AXIS = 4096, MAX_CACHE_BYTES = 16 * 1024 * 1024;
const cache = new Map();
let cacheBytes = 0;
function surface(context, width, height) {
    const canvas = typeof OffscreenCanvas === 'function' ? new OffscreenCanvas(width, height)
        : context.canvas.ownerDocument.createElement('canvas');
    canvas.width = width; canvas.height = height; return canvas;
}
function sampling(transform) {
    const {a,b,c,d} = transform, sum = a*a+b*b+c*c+d*d, determinant = a*d-b*c;
    return Math.sqrt((sum+Math.sqrt(Math.max(0,sum*sum-4*determinant*determinant)))/2);
}
function capture(parent, bounds, pad, pixelBudget) {
    const x = bounds[0]-pad, y = bounds[1]-pad;
    const width = bounds[2]+2*pad, height = bounds[3]+2*pad;
    if (![x,y,width,height].every(Number.isFinite) || width <= 0 || height <= 0) throw new RangeError('Invalid shadow extent');
    const scale = Math.max(Number.MIN_VALUE, Math.min(sampling(parent.getTransform()) || 1,
        (MAX_AXIS-1)/width, (MAX_AXIS-1)/height, Math.sqrt(pixelBudget/(width*height))*.999));
    const pixelHeight = Math.min(pixelBudget, Math.max(1, Math.ceil(height*scale)));
    const pixelWidth = Math.min(Math.max(1, Math.ceil(width*scale)), Math.floor(pixelBudget/pixelHeight));
    const canvas = surface(parent, pixelWidth, pixelHeight), context = canvas.getContext('2d');
    const sx = pixelWidth/width, sy = pixelHeight/height;
    context.setTransform(sx, 0, 0, sy, -x*sx, -y*sy);
    return { parent, canvas, context, x, y, width, height, sx, sy };
}
export function beginContentShadow(parent, bounds, offset, blur, color, pixelBudget = MAX_PIXELS, transform = null) {
    // Budget counts every source/transformed/mask canvas, not only the first
    // capture. The separate bounded LRU includes retained mask canvases.
    const surfaces = 3;
    if (!Number.isInteger(pixelBudget) || pixelBudget < surfaces || pixelBudget > MAX_PIXELS) throw new RangeError('Invalid content shadow pixel budget');
    if (transform && (!Array.isArray(transform) || transform.length !== 6 || transform.some(v=>!Number.isFinite(v)) ||
        transform.slice(0,4).some(v=>Math.abs(v)>1024) || transform.slice(4).some(v=>Math.abs(v)>1e6) ||
        transform[0]*transform[3]===transform[1]*transform[2])) throw new RangeError('Invalid shadow affine');
    const budget = Math.floor(pixelBudget/surfaces);
    if (!Number.isFinite(blur) || blur < 0 || blur > 1e6) throw new RangeError('Invalid shadow sigma');
    const layer = capture(parent, bounds, 1, budget);
    return { ...layer, bounds, offset, blur, color, transform, budget };
}
function transformedAlpha(layer) {
    const {parent,x,y,width,height,budget,blur,color}=layer;
    const m=layer.transform??[1,0,0,1,0,0];
    // Compute extents from vectors rather than subtracting translated corners:
    // a valid tiny scale must not become zero width through cancellation.
    const ax=m[0]*width,cx=m[2]*height,by=m[1]*width,dy=m[3]*height;
    const left=m[0]*x+m[2]*y+m[4]+Math.min(0,ax)+Math.min(0,cx);
    const top=m[1]*x+m[3]*y+m[5]+Math.min(0,by)+Math.min(0,dy);
    const extent=[left,top,Math.abs(ax)+Math.abs(cx),Math.abs(by)+Math.abs(dy)];
    const result=capture(parent,layer.transform?extent:layer.bounds,Math.ceil(4*blur)+1,budget);
    result.context.transform(...m);
    result.context.drawImage(layer.canvas,x,y,width,height);
    return {...result,blur,color};
}
function boxPass(source, target, width, height, radius, horizontal) {
    const length = horizontal ? width : height, lines = horizontal ? height : width;
    const stride = horizontal ? 1 : width, denominator = radius*2+1;
    for (let line=0; line<lines; line++) {
        const base = horizontal ? line*width : line;
        let sum=0;
        for (let i=0; i<=Math.min(radius,length-1); i++) sum+=source[base+i*stride];
        for (let i=0; i<length; i++) {
            target[base+i*stride]=sum/denominator;
            if (i-radius>=0) sum-=source[base+(i-radius)*stride];
            if (i+radius+1<length) sum+=source[base+(i+radius+1)*stride];
        }
    }
}
function radii(sigma) {
    let low=Math.floor(Math.sqrt(4*sigma*sigma+1)); if(low%2===0) low--;
    low=Math.max(1,low); const high=low+2;
    const count=Math.max(0,Math.min(3,Math.round((12*sigma*sigma-3*low*low-12*low-9)/(-4*low-4))));
    return Array.from({length:3},(_,i)=>((i<count?low:high)-1)/2);
}
function equal(a,b) { return a.length===b.length && a.every((value,i)=>value===b[i]); }
function alphaMask(layer) {
    const {canvas,context,blur,color,sx,sy}=layer, {width,height}=canvas;
    const rgba=context.getImageData(0,0,width,height).data, alpha=new Uint8ClampedArray(width*height);
    let hash=2166136261;
    for(let i=0;i<alpha.length;i++){alpha[i]=rgba[i*4+3];hash=Math.imul(hash^alpha[i],16777619);}
    const key=`${width}:${height}:${blur*sx}:${blur*sy}:${color}:${hash>>>0}`;
    const found=cache.get(key);
    if(found && equal(found.alpha,alpha)){cache.delete(key);cache.set(key,found);return found.canvas;}
    let source=alpha.slice(),target=new Uint8ClampedArray(alpha.length);
    const rx=radii(blur*sx),ry=radii(blur*sy);
    for(let i=0;i<3;i++){
        boxPass(source,target,width,height,rx[i],true);[source,target]=[target,source];
        boxPass(source,target,width,height,ry[i],false);[source,target]=[target,source];
    }
    const mask=surface(context,width,height),maskContext=mask.getContext('2d'),image=maskContext.createImageData(width,height);
    const red=color&255,green=(color>>>8)&255,blue=(color>>>16)&255,opacity=(color>>>24)/255;
    for(let i=0;i<source.length;i++){const p=i*4;image.data[p]=red;image.data[p+1]=green;image.data[p+2]=blue;image.data[p+3]=source[i]*opacity;}
    maskContext.putImageData(image,0,0);
    if(found){cacheBytes-=found.bytes;found.canvas.width=0;found.canvas.height=0;cache.delete(key);}
    const bytes=alpha.byteLength+width*height*4;cache.set(key,{alpha,canvas:mask,bytes});cacheBytes+=bytes;
    while(cacheBytes>MAX_CACHE_BYTES || cache.size>16){const first=cache.keys().next().value,entry=cache.get(first);cache.delete(first);cacheBytes-=entry.bytes;entry.canvas.width=0;entry.canvas.height=0;}
    return mask;
}
export function endContentShadow(layer) {
    const {parent,canvas,x,y,width,height,offset,color}=layer;
    let shadow=layer;
    try {
        if(color>>>24) {
            shadow=transformedAlpha(layer);
            parent.drawImage(alphaMask(shadow),shadow.x+offset[0],shadow.y+offset[1],shadow.width,shadow.height);
        }
        parent.drawImage(canvas,x,y,width,height);
    } finally {
        canvas.width=0;canvas.height=0;
        if(shadow!==layer){shadow.canvas.width=0;shadow.canvas.height=0;}
    }
    return parent;
}
