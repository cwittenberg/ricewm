/** Shared admission for spatial GL storage across live/candidate worlds and
 * private previews. Bytes describe application-visible storage plus an explicit
 * drawing-buffer allowance, not opaque driver or browser compositor memory. */
export const SPATIAL_GPU_STORAGE_LIMIT = 256 * 1024 * 1024;
const size = n => {
    if (!Number.isSafeInteger(n) || n < 0) throw RangeError('Invalid graphics storage size');
    return n;
};

export class SpatialGpuBudget {
    constructor(limit = SPATIAL_GPU_STORAGE_LIMIT) {
        this.limit = size(limit); this.bytes = 0; this.peakBytes = 0; this.owners = new Set();
    }
    open(label = 'spatial') {
        if (this.owners.size >= 16) throw RangeError('Too many spatial graphics contexts');
        const records = new Map(), owner = {label:String(label).slice(0,80), records, bytes:0};
        this.owners.add(owner); let closed = false;
        return {
            set:(key, bytes, kind = 'storage') => {
                if (closed) throw Error('Graphics admission owner is closed');
                size(bytes); const before = records.get(key)?.bytes || 0, next = this.bytes - before + bytes;
                if (!Number.isSafeInteger(next) || next > this.limit) throw RangeError('Combined spatial graphics storage budget exceeded');
                if (bytes) records.set(key,{bytes,kind}); else records.delete(key);
                owner.bytes += bytes - before; this.bytes = next; this.peakBytes = Math.max(this.peakBytes,next);
            },
            get:key => records.get(key)?.bytes || 0,
            close:() => { if (closed) return; closed = true; this.bytes -= owner.bytes; owner.bytes = 0; records.clear(); this.owners.delete(owner); },
            snapshot:() => ({label:owner.label,bytes:owner.bytes,closed,resources:records.size}),
        };
    }
    snapshot() {
        let drawingBufferAllowanceBytes=0;
        for(const owner of this.owners) for(const record of owner.records.values()) if(record.kind==='drawing-buffer-allowance') drawingBufferAllowanceBytes+=record.bytes;
        return {limit:this.limit,bytes:this.bytes,storageBytes:this.bytes-drawingBufferAllowanceBytes,drawingBufferAllowanceBytes,peakBytes:this.peakBytes,contexts:this.owners.size};
    }
}

export const sharedSpatialGpuBudget = new SpatialGpuBudget();

/** Wrap this context only; unrelated applications and globals are untouched.
 * Allocation admission happens before the driver call. Data updates and draws
 * do not reserve new storage. Methods without bookkeeping bind once and cache. */
export function trackSpatialGlContext(raw, {canvas=raw.canvas,label='world',budget=sharedSpatialGpuBudget}={}) {
    const lease=budget.open(label), records=new Map(), functions=new Map(), bufferBindings=new Map(), textureBindings=new Map(), elementBindings=new Map();
    let unit=raw.TEXTURE0, vao=null, renderbuffer=null, closed=false;
    const drawingKey={}, textureTarget=t=>t>=raw.TEXTURE_CUBE_MAP_POSITIVE_X&&t<=raw.TEXTURE_CUBE_MAP_NEGATIVE_Z?raw.TEXTURE_CUBE_MAP:t;
    const textureAt=t=>textureBindings.get(`${unit}:${textureTarget(t)}`), bytesOf=r=>r?.levels?[...r.levels.values()].reduce((n,v)=>n+v.bytes,0):r?.bytes||0;
    const release=object=>{if(!object)return;if(!closed)lease.set(object,0);records.delete(object);};
    const replace=(object,record,run)=>{
        if(!object)return run(); if(closed)throw Error('Graphics context admission is closed');
        const before=records.get(object), oldBytes=bytesOf(before); lease.set(object,bytesOf(record));
        try {const result=run();records.set(object,record);return result;} catch(error){lease.set(object,oldBytes);throw error;}
    };
    const pixels=(format,type)=>{
        const sized=new Map([[raw.R8,1],[raw.RG8,2],[raw.RGB8,3],[raw.RGBA8,4],[raw.SRGB8,3],[raw.SRGB8_ALPHA8,4],[raw.R16F,2],[raw.RG16F,4],[raw.RGB16F,6],[raw.RGBA16F,8],[raw.R32F,4],[raw.RG32F,8],[raw.RGB32F,12],[raw.RGBA32F,16],[raw.DEPTH_COMPONENT16,2],[raw.DEPTH_COMPONENT24,4],[raw.DEPTH_COMPONENT32F,4],[raw.DEPTH24_STENCIL8,4],[raw.DEPTH32F_STENCIL8,8],[raw.STENCIL_INDEX8,1]].filter(([key])=>key!==undefined));
        if(sized.has(format))return sized.get(format);
        const components=new Map([[raw.RED,1],[raw.RG,2],[raw.RGB,3],[raw.RGBA,4],[raw.DEPTH_COMPONENT,1]].filter(([key])=>key!==undefined)).get(format);
        const element=new Map([[raw.UNSIGNED_BYTE,1],[raw.BYTE,1],[raw.UNSIGNED_SHORT,2],[raw.SHORT,2],[raw.HALF_FLOAT,2],[raw.UNSIGNED_INT,4],[raw.INT,4],[raw.FLOAT,4]].filter(([key])=>key!==undefined)).get(type);
        const packed=new Map([[raw.UNSIGNED_SHORT_5_6_5,2],[raw.UNSIGNED_SHORT_4_4_4_4,2],[raw.UNSIGNED_SHORT_5_5_5_1,2],[raw.UNSIGNED_INT_2_10_10_10_REV,4],[raw.UNSIGNED_INT_10F_11F_11F_REV,4],[raw.UNSIGNED_INT_5_9_9_9_REV,4],[raw.UNSIGNED_INT_24_8,4],[raw.FLOAT_32_UNSIGNED_INT_24_8_REV,8]].filter(([key])=>key!==undefined));
        if(packed.has(type))return packed.get(type); if(components&&element)return components*element;
        throw RangeError('Unsupported spatial graphics storage format');
    };
    const image=(method,args)=>{
        const [target,level,internal]=args;let w,h,d=1,type;
        if(method==='texImage3D'){[w,h,d]=args.slice(3,6);type=args[8];}
        else if(args.length>=9){[w,h]=args.slice(3,5);type=args[7];}
        else {const source=args[5];w=source?.videoWidth??source?.naturalWidth??source?.width;h=source?.videoHeight??source?.naturalHeight??source?.height;type=args[4];}
        size(w);size(h);size(d);size(level);const bpp=pixels(internal,type), object=textureAt(target), previous=records.get(object), levels=new Map(previous?.levels);
        levels.set(`${target}:${level}`,{target,level,w,h,d,bpp,bytes:size(w*h*d*bpp)});
        return replace(object,{levels},()=>raw[method](...args));
    };
    const immutable=(method,args)=>{
        const [target,count,format,w,h]=args,d=method==='texStorage3D'?args[5]:1;size(count);size(w);size(h);size(d);
        const object=textureAt(target),levels=new Map(),bpp=pixels(format);
        const faces=target===raw.TEXTURE_CUBE_MAP?Array.from({length:6},(_,i)=>raw.TEXTURE_CUBE_MAP_POSITIVE_X+i):[target];
        for(const face of faces)for(let level=0;level<count;level++) {const lw=Math.max(1,Math.floor(w/2**level)),lh=Math.max(1,Math.floor(h/2**level)),ld=target===raw.TEXTURE_2D_ARRAY?d:Math.max(1,Math.floor(d/2**level));levels.set(`${face}:${level}`,{target:face,level,w:lw,h:lh,d:ld,bpp,bytes:size(lw*lh*ld*bpp)});}
        return replace(object,{levels},()=>raw[method](...args));
    };
    const hooks={
        bindBuffer:(target,object)=>{if(target===raw.ELEMENT_ARRAY_BUFFER)elementBindings.set(vao,object);else bufferBindings.set(target,object);return raw.bindBuffer(target,object);},
        bindVertexArray:object=>{vao=object;return raw.bindVertexArray(object);},
        bufferData:(target,data,...rest)=>{const object=target===raw.ELEMENT_ARRAY_BUFFER?elementBindings.get(vao):bufferBindings.get(target);let bytes=typeof data==='number'?data:data?.byteLength;if(typeof data!=='number'&&rest.length>=2){const offset=rest[1]||0,length=rest[2]||0,element=data.BYTES_PER_ELEMENT||1;bytes=(length||data.length-offset)*element;}return replace(object,{bytes:size(bytes)},()=>raw.bufferData(target,data,...rest));},
        activeTexture:value=>{unit=value;return raw.activeTexture(value);},
        bindTexture:(target,object)=>{textureBindings.set(`${unit}:${target}`,object);return raw.bindTexture(target,object);},
        texImage2D:(...args)=>image('texImage2D',args),texImage3D:(...args)=>image('texImage3D',args),
        texStorage2D:(...args)=>immutable('texStorage2D',args),texStorage3D:(...args)=>immutable('texStorage3D',args),
        generateMipmap:target=>{
            const object=textureAt(target),levels=new Map(records.get(object)?.levels),bases=[...levels.values()].filter(v=>v.level===0);
            for(const base of bases){const count=Math.floor(Math.log2(Math.max(base.w,base.h,target===raw.TEXTURE_3D?base.d:1)))+1;for(let level=1;level<count;level++){const w=Math.max(1,base.w>>level),h=Math.max(1,base.h>>level),d=target===raw.TEXTURE_3D?Math.max(1,base.d>>level):base.d;levels.set(`${base.target}:${level}`,{...base,level,w,h,d,bytes:size(w*h*d*base.bpp)});}}
            return replace(object,{levels},()=>raw.generateMipmap(target));
        },
        bindRenderbuffer:(target,object)=>{renderbuffer=object;return raw.bindRenderbuffer(target,object);},
        renderbufferStorage:(target,format,w,h)=>replace(renderbuffer,{bytes:size(size(w)*size(h)*pixels(format))},()=>raw.renderbufferStorage(target,format,w,h)),
        renderbufferStorageMultisample:(target,samples,format,w,h)=>replace(renderbuffer,{bytes:size(Math.max(1,size(samples))*size(w)*size(h)*pixels(format))},()=>raw.renderbufferStorageMultisample(target,samples,format,w,h)),
    };
    for(const name of ['deleteBuffer','deleteTexture','deleteRenderbuffer'])hooks[name]=object=>{const result=raw[name](object);release(object);return result;};
    const gpu={
        resizeDrawingBuffer:(w,h)=>{if(closed)throw Error('Graphics context admission is closed');lease.set(drawingKey,size(size(w)*size(h)*16),'drawing-buffer-allowance');},
        snapshot:()=>({...lease.snapshot(),combined:budget.snapshot()}),
        close:()=>{if(closed)return;closed=true;records.clear();bufferBindings.clear();textureBindings.clear();elementBindings.clear();lease.close();},
    };
    try {gpu.resizeDrawingBuffer(canvas.width,canvas.height);}catch(error){gpu.close();throw error;}
    const gl=new Proxy(raw,{
        get:(target,key)=>{if(functions.has(key))return functions.get(key);const value=Reflect.get(target,key,target);if(typeof value!=='function')return value;const fn=hooks[key]||value.bind(target);functions.set(key,fn);return fn;},
        // Diagnostic decorators wrap the tracked method, never replace the raw
        // driver method that it calls. Forwarding these writes creates recursion.
        set:(_target,key,value)=>{functions.set(key,value);return true;},
    });
    return {gl,gpu};
}
