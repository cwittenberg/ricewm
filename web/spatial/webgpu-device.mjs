import {sharedSpatialGpuBudget} from './gpu-budget.mjs';

// This is an owned candidate/device boundary, not a renderer or backend selector.
// All managed buffer/texture storage must use the checked factories below. The
// exposed device is for pipelines, bindings, commands and submission; it must
// not be borrowed from the ordinary desktop or used to bypass these factories.
const MAX_RESOURCES = 2048;
const FORMATS = new Map([
    ...['r8unorm','r8snorm','r8uint','r8sint'].map(f=>[f,1]),
    ...['rg8unorm','rg8snorm','rg8uint','rg8sint','r16uint','r16sint','r16float'].map(f=>[f,2]),
    ...['rgba8unorm','rgba8unorm-srgb','rgba8snorm','rgba8uint','rgba8sint',
        'bgra8unorm','bgra8unorm-srgb','rg16uint','rg16sint','rg16float',
        'r32uint','r32sint','r32float','rgb10a2uint','rgb10a2unorm','rg11b10ufloat',
        'depth32float','depth24plus','stencil8'].map(f=>[f,4]),
    ...['rgba16uint','rgba16sint','rgba16float','rg32uint','rg32sint','rg32float',
        'depth24plus-stencil8','depth32float-stencil8'].map(f=>[f,8]),
    ...['rgba32uint','rgba32sint','rgba32float'].map(f=>[f,16]),
    ['depth16unorm',2],
]);
const integer = (n, min=0) => {
    if(!Number.isSafeInteger(n)||n<min)throw RangeError('Invalid WebGPU storage size');
    return n;
};
const product = (...values) => integer(values.reduce((a,b)=>a*integer(b),1));
const cancelled = message => Object.assign(new Error(message),{name:'AbortError'});
const message = error => String(error?.message||error||'WebGPU candidate closed');

/** Application-visible payload accounting, with conservative packed depth
 * allowances. Array layers remain constant through mips; volume depth shrinks.
 * Unsupported/compressed formats fail explicitly instead of undercounting.
 * This does not measure opaque driver/compositor allocation or temporary copies. */
export function spatialWebGpuTextureBytes(descriptor) {
    const {format,dimension='2d',mipLevelCount=1,sampleCount=1}=descriptor||{};
    const bpp=FORMATS.get(format);if(!bpp)throw RangeError('Unsupported spatial WebGPU texture format');
    if(!['1d','2d','3d'].includes(dimension))throw RangeError('Invalid WebGPU texture dimension');
    const extent=descriptor.size;
    let width,height,depth;
    if(Array.isArray(extent)||ArrayBuffer.isView(extent)) {
        if(extent.length<1||extent.length>3)throw RangeError('Invalid WebGPU texture extent');
        [width,height=1,depth=1]=extent;
    }else if(extent&&typeof extent==='object') {
        ({width,height=1,depthOrArrayLayers:depth=1}=extent);
    }else throw RangeError('Invalid WebGPU texture extent');
    integer(width,1);integer(height,1);integer(depth,1);integer(mipLevelCount,1);
    if(dimension==='1d'&&(height!==1||depth!==1))throw RangeError('Invalid 1D texture extent');
    if(mipLevelCount>1+Math.floor(Math.log2(Math.max(width,height,dimension==='3d'?depth:1))))throw RangeError('Too many texture mip levels');
    if(![1,4].includes(sampleCount)||sampleCount!==1&&(dimension!=='2d'||mipLevelCount!==1||depth!==1))throw RangeError('Invalid texture sample count');
    let bytes=0;
    for(let level=0;level<mipLevelCount;level++) {
        const w=Math.max(1,Math.floor(width/2**level)),h=Math.max(1,Math.floor(height/2**level));
        const d=dimension==='3d'?Math.max(1,Math.floor(depth/2**level)):depth;
        bytes=integer(bytes+product(w,h,d,sampleCount,bpp));
    }
    return {bytes,width,height,depth,dimension};
}

function knownSoftware(adapter) {
    const info=adapter?.info||{},identity=[info.vendor,info.architecture,info.device,info.description,info.driver,info.type].join(' ').toLowerCase();
    return adapter?.isFallbackAdapter===true||info.isFallbackAdapter===true||String(info.type||'').toUpperCase()==='CPU'||/\b(?:swiftshader|subzero|llvmpipe|lavapipe)\b/.test(identity);
}

/** Creates one dedicated device. Signals/revision predicates are trusted engine
 * inputs, never recipe callbacks. Unknown adapter identity is not hardware proof.
 * Required limits are minimum numeric capacities (not alignment requirements).
 * Optional timestamp-query support is reported, never required implicitly. */
export async function createSpatialWebGpuDevice({gpu=globalThis.navigator?.gpu,
    budget=sharedSpatialGpuBudget,label='webgpu-world',signal,isCurrent=()=>true,
    requiredFeatures=[],minimumLimits={},allowSoftware=false,onLost=()=>{}}={}) {
    if(!gpu?.requestAdapter)throw Object.assign(Error('WebGPU is unavailable'),{code:'WEBGPU_UNAVAILABLE'});
    if(!Array.isArray(requiredFeatures)||requiredFeatures.some(f=>typeof f!=='string'||!f))throw TypeError('Invalid WebGPU feature requirements');
    const features=[...new Set(requiredFeatures)];
    const limits={...minimumLimits};for(const [key,value]of Object.entries(limits))if(!/^max[A-Z]/.test(key)||!Number.isSafeInteger(value)||value<1)throw TypeError('Invalid WebGPU minimum limit');
    if(signal?.aborted)throw cancelled('WebGPU candidate aborted');
    const lease=budget.open(label),records=new Set(),drawingKey={},interruptions=new Set(),pendingChecks=new Set();
    let trackedDevice=null;
    let device=null,state='preparing',reason=null,capabilities=null;
    const cleanupErrors=[];
    const noteCleanup=error=>{if(cleanupErrors.length<8)cleanupErrors.push(message(error));};
    const safeDestroy=resource=>{try{resource?.destroy();}catch(error){noteCleanup(error);}};
    const release=record=>{
        if(!records.delete(record))return;
        if(record.nativeDestroy){try{record.nativeDestroy();}catch(error){noteCleanup(error);}}else safeDestroy(record.resource);if(state!=='closed')lease.set(record,0);
    };
    const stop=(error,{lost=false}={})=>{
        if(state==='closed')return;
        state='closed';reason=error;signal?.removeEventListener('abort',abort);
        device?.removeEventListener?.('uncapturederror',uncaptured);
        // Destroy owned objects before retiring accounting; no old/live owner is touched.
        for(const record of [...records])release(record);records.clear();
        safeDestroy(device);lease.close();for(const interrupted of interruptions)interrupted();
        if(lost)try{Promise.resolve(onLost(error)).catch(noteCleanup);}catch(callbackError){noteCleanup(callbackError);}
    };
    const abort=()=>stop(cancelled('WebGPU candidate aborted'));
    const uncaptured=event=>{event.preventDefault?.();stop(Error(`WebGPU uncaptured error: ${message(event.error)}`),{lost:true});};
    signal?.addEventListener('abort',abort,{once:true});
    const current=()=>{
        if(state==='closed')throw reason;
        let valid;try{valid=isCurrent();}catch(error){stop(error);throw error;}
        if(typeof valid!=='boolean'){const error=TypeError('WebGPU revision predicate must return a boolean synchronously');stop(error);throw error;}
        if(!valid){const error=cancelled('WebGPU candidate superseded');stop(error);throw error;}
    };
    // Browser adapter/device/error-scope promises cannot themselves be cancelled.
    // Reject promptly, retaining only a late disposer for an acquired device.
    const wait=(value,onLate=()=>{})=>new Promise((resolve,reject)=>{
        let ended=false;
        const interrupted=()=>{if(ended)return;ended=true;interruptions.delete(interrupted);reject(reason||cancelled('WebGPU candidate closed'));};
        interruptions.add(interrupted);if(state==='closed')interrupted();
        Promise.resolve(value).then(result=>{
            interruptions.delete(interrupted);
            if(ended){onLate(result);return;}ended=true;resolve(result);
        },error=>{
            interruptions.delete(interrupted);
            if(!ended){ended=true;reject(error);}
        });
    });
    // Both asynchronous callers and trusted synchronous graphics libraries use
    // one reservation ledger, descriptor validation and driver error scopes.
    // The synchronous facade never exposes an unaccounted allocation path.
    const beginAllocation=(method,descriptor,bytes,kind,{tracked=false}={})=>{
        current();if(state!=='ready')throw Error('WebGPU device is not ready');
        if(records.size>=MAX_RESOURCES)throw RangeError('Too many managed WebGPU resources');
        const record={resource:null,bytes,pending:true,kind};lease.set(record,bytes,kind);records.add(record);
        let operationError,pushed=0;const scopes=[];
        try {
            try {
                for(const filter of ['internal','out-of-memory','validation']){device.pushErrorScope(filter);pushed++;}
                record.resource=device[method](descriptor);
                if(!record.resource)throw Error('WebGPU returned no resource');
                if(tracked){
                    record.nativeDestroy=record.resource.destroy.bind(record.resource);
                    Object.defineProperty(record.resource,'destroy',{configurable:true,value:()=>release(record)});
                }
            }catch(error){operationError=error;}
            finally {while(pushed-- > 0)try{scopes.push(Promise.resolve(device.popErrorScope()));}catch(error){scopes.push(Promise.reject(error));}}
        }catch(error){release(record);throw error;}
        const ready=(async()=>{
            try{
                const results=await wait(Promise.allSettled(scopes));
                if(operationError)throw operationError;
                const failure=results.find(result=>result.status==='rejected'||result.value);
                if(failure)throw Error(`WebGPU allocation failed: ${message(failure.reason||failure.value)}`);
                current();record.pending=false;
                return Object.freeze({resource:record.resource,bytes,release:()=>release(record)});
            }catch(error){release(record);throw error;}
        })();
        return {record,ready,operationError};
    };
    const allocate=async(...args)=>beginAllocation(...args).ready;
    const bufferInput=descriptor=>{
        current();const copy={...descriptor},bytes=integer(copy.size);
        if(bytes>device.limits.maxBufferSize)throw RangeError('WebGPU buffer size exceeds device limit');
        return [copy,bytes,(copy.usage&3)?'staging':'storage'];
    };
    const textureInput=descriptor=>{
        current();const copy={...descriptor},s=spatialWebGpuTextureBytes(copy);
        copy.size={width:s.width,height:s.height,depthOrArrayLayers:s.depth};
        const max=device.limits[`maxTextureDimension${s.dimension.toUpperCase()}`];
        if(!Number.isFinite(max)||s.width>max||s.height>max||s.dimension==='3d'&&s.depth>max||s.dimension==='2d'&&s.depth>device.limits.maxTextureArrayLayers)throw RangeError('WebGPU texture extent exceeds device limit');
        return [copy,s.bytes,'storage'];
    };
    const trackedAllocation=(method,input)=>{
        const allocation=beginAllocation(method,...input,{tracked:true});
        const checked=allocation.ready.then(()=>{},error=>{stop(error,{lost:true});throw error;});
        pendingChecks.add(checked);
        // Keep rejection observed even if the renderer closes before its next
        // explicit checkpoint; the owner transitions to failed immediately.
        checked.catch(()=>{}).finally(()=>pendingChecks.delete(checked));
        if(allocation.operationError)throw allocation.operationError;
        return allocation.record.resource;
    };
    try {
        current();let adapter=null,lastFailure='No compatible WebGPU adapter';
        for(const preference of [{powerPreference:'high-performance'},undefined,{featureLevel:'compatibility'}]) {
            current();let candidate;
            try{candidate=await wait(gpu.requestAdapter(preference));}catch(error){current();lastFailure=message(error);continue;}
            current();if(!candidate)continue;
            if(knownSoftware(candidate)&&!allowSoftware){lastFailure='Only a known software WebGPU adapter is available';continue;}
            const missing=features.find(f=>!candidate.features?.has(f));
            if(missing){lastFailure=`WebGPU feature unavailable: ${missing}`;continue;}
            const short=Object.entries(limits).find(([key,value])=>!Number.isFinite(candidate.limits?.[key])||candidate.limits[key]<value);
            if(short){lastFailure=`WebGPU limit unavailable: ${short[0]}`;continue;}
            adapter=candidate;break;
        }
        if(!adapter)throw Object.assign(Error(lastFailure),{code:'WEBGPU_UNAVAILABLE'});
        try{device=await wait(adapter.requestDevice({requiredFeatures:features,requiredLimits:limits}),safeDestroy);}catch(error){current();throw Object.assign(Error('WebGPU device unavailable: '+message(error),{cause:error}),{code:'WEBGPU_UNAVAILABLE'});}
        // Abort can run after the wait resolves but before this continuation.
        if(state==='closed'){safeDestroy(device);throw reason;}
        current();
        if(!device?.lost?.then||!device.pushErrorScope||!device.popErrorScope)throw Error('Incomplete WebGPU device lifecycle API');
        for(const key of ['maxBufferSize','maxTextureDimension1D','maxTextureDimension2D','maxTextureDimension3D','maxTextureArrayLayers'])
            if(!Number.isSafeInteger(device.limits?.[key])||device.limits[key]<1)throw Error(`Incomplete WebGPU device limit: ${key}`);
        for(const feature of features)if(!device.features?.has(feature))throw Error(`Requested WebGPU device feature missing: ${feature}`);
        for(const [key,value]of Object.entries(limits))if(!Number.isFinite(device.limits[key])||device.limits[key]<value)throw Error(`Requested WebGPU device limit missing: ${key}`);
        device.addEventListener?.('uncapturederror',uncaptured);
        device.lost.then(info=>{if(state!=='closed')stop(Error(`WebGPU device lost: ${message(info?.message||info?.reason)}`),{lost:true});},error=>{if(state!=='closed')stop(error,{lost:true});});
        capabilities=Object.freeze({adapterClass:knownSoftware(adapter)?'known-software':'not-known-software',
            features:Object.freeze([...device.features].sort()),requiredLimits:Object.freeze({...limits})});
        await wait(Promise.resolve());current();
        state='ready';
        return Object.freeze({device,capabilities,
            createBuffer:descriptor=>allocate('createBuffer',...bufferInput(descriptor)),
            createTexture:descriptor=>allocate('createTexture',...textureInput(descriptor)),
            // Trusted library adapter. Native resources retain their browser
            // identity; destroy() releases the same checked owner reservation.
            trackedDevice:()=>{
                current();if(trackedDevice)return trackedDevice;
                const functions=new Map();
                trackedDevice=new Proxy(device,{get(target,key){
                    if(key==='createBuffer')return descriptor=>trackedAllocation('createBuffer',bufferInput(descriptor));
                    if(key==='createTexture')return descriptor=>trackedAllocation('createTexture',textureInput(descriptor));
                    if(key==='destroy')return ()=>stop(cancelled('WebGPU device released'));
                    const value=Reflect.get(target,key,target);
                    if(typeof value!=='function')return value;
                    if(!functions.has(key))functions.set(key,value.bind(target));
                    return functions.get(key);
                }});return trackedDevice;
            },
            checkAllocations:async()=>{current();await wait(Promise.all([...pendingChecks]));current();},
            resizeDrawingBuffer:(width,height)=>{
                current();integer(width);integer(height);
                if(width>device.limits.maxTextureDimension2D||height>device.limits.maxTextureDimension2D)throw RangeError('WebGPU drawing extent exceeds device limit');
                lease.set(drawingKey,product(width,height,16),'drawing-buffer-allowance');
            },
            snapshot:()=>({...lease.snapshot(),state,reason:reason?message(reason):null,
                pendingAllocations:[...records].filter(r=>r.pending).length,managedResources:records.size,
                stagingBytes:[...records].filter(r=>r.kind==='staging').reduce((n,r)=>n+r.bytes,0),
                drawingBufferAllowanceBytes:lease.get(drawingKey),
                cleanupErrors:[...cleanupErrors],combined:budget.snapshot()}),
            close:()=>stop(cancelled('WebGPU device released')),
        });
    }catch(error){stop(error);throw error;}
}
