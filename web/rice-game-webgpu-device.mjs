import {createSpatialWebGpuDevice} from './spatial/webgpu-device.mjs';
import {manageGameWebGpuBindings} from './rice-game-webgpu-bindings.mjs';

/** Trusted game adapter for the pinned Three renderer. Existing desktop
 * renderers keep their own devices, clocks and allocation owners. */
export async function createGameWebGpuRenderer({
    width=640,height=360,gpu=globalThis.navigator?.gpu,signal,budget,
    canvasFactory=(w,h)=>new OffscreenCanvas(w,h),allowSoftware=false,isolateFrameErrors=false,onLost=()=>{},
}={}) {
    if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width*height>2073600)
        throw RangeError('Game GPU viewport exceeds its pixel budget');
    let owner,renderer,canvas,closed=false,failure=null,frames=0,frameScope=false;
    let completionWaitMs=0,lastCompletionWaitMs=0;
    const fail=error=>{if(closed||failure)return;failure=error;try{onLost(error);}finally{close();}};
    const close=()=>{
        if(closed)return;closed=true;
        try{renderer?.dispose();}finally{owner?.close();}
    };
    const active=()=>{if(closed)throw failure||Error('Game GPU renderer closed');signal?.throwIfAborted?.();};
    try {
        owner=await createSpatialWebGpuDevice({gpu,budget,label:'riceapp-game-webgpu',signal,allowSoftware,onLost:error=>fail(Object.assign(error,{code:'WEBGPU_DEVICE_LOST'}))});
        active();canvas=canvasFactory(width,height);canvas.width=width;canvas.height=height;
        const nativeContext=canvas.getContext('webgpu');
        if(!nativeContext)throw Object.assign(Error('WebGPU game canvas unavailable'),{code:'WEBGPU_UNAVAILABLE'});
        const T=await import('./vendor/three/three-webgpu.js');active();
        // GPUDevice proxies are not WebIDL GPUDevice instances. The trusted
        // library uses checked factories; canvas configuration receives the
        // original device, with drawing storage accounted by the same owner.
        const context={canvas,
            configure:options=>nativeContext.configure({...options,device:owner.device}),
            unconfigure:()=>nativeContext.unconfigure(),
            getCurrentTexture:()=>nativeContext.getCurrentTexture(),
            getConfiguration:()=>nativeContext.getConfiguration?.(),
        };
        renderer=new T.WebGPURenderer({canvas,context,device:owner.trackedDevice(),antialias:true,samples:4,alpha:true,stencil:true});
        // Pinned 0.180.0 starts a second RAF clock and installs an automatic GL
        // fallback. The SDK owns scheduling and the explicit fallback decision.
        // Keep these two integration points private and covered by real API tests.
        renderer._getFallback=null;
        let animation=renderer._animation;
        const ownedClock={requestAnimationFrame:()=>0,cancelAnimationFrame:()=>{}};
        Object.defineProperty(renderer,'_animation',{configurable:true,
            get:()=>animation,set:value=>{animation=value;value?.setContext(ownedClock);}});
        renderer.onDeviceLost=info=>{if(!closed)fail(Object.assign(Error('Game WebGPU device lost: '+String(info?.message||info?.reason||'unknown')),{code:'WEBGPU_DEVICE_LOST'}));};
        await renderer.init();
        manageGameWebGpuBindings(renderer);
        renderer._animation.stop();
        active();
        if(renderer.backend?.isWebGPUBackend!==true)throw Error('Game renderer did not select WebGPU');
        renderer.setSize(width,height,false);renderer.setPixelRatio(1);
        renderer.info.autoReset=false;renderer.sortObjects=false;
        renderer.outputColorSpace=T.SRGBColorSpace;
        owner.resizeDrawingBuffer(width,height);
        await owner.checkAllocations();
        const statistics=()=>({backend:'webgpu',adapterClass:owner.capabilities.adapterClass,
            software:owner.capabilities.adapterClass==='known-software',frames,
            applicationReadbacks:0,gpuStorage:owner.snapshot(),
            completionWaitMs,lastCompletionWaitMs,
            drawCalls:renderer.info.render.drawCalls,renderPasses:renderer.info.render.frameCalls,triangles:renderer.info.render.triangles});
        return Object.freeze({T,renderer,canvas,owner,
            beginFrame(){active();if(frameScope)throw Error('Game GPU frame already pending');owner.device.pushErrorScope('validation');frameScope=true;renderer.info.reset();renderer._nodes.nodeFrame.update();renderer.info.frame=renderer._nodes.nodeFrame.frameId;},
            async completeFrame(){
                active();const error=frameScope?owner.device.popErrorScope():Promise.resolve(null);frameScope=false;
                await owner.checkAllocations();const validation=await error;
                if(validation){const failure=Error('Game GPU frame validation failed: '+String(validation.message||validation));if(!isolateFrameErrors)fail(failure);throw failure;}
                active();
                // A validation scope is not a completion fence. Handing an
                // unfinished bitmap to Canvas2D can block its drawImage() on
                // the desktop/input thread. Wait asynchronously in this owned
                // lane instead; pending native frames already coalesce there.
                // This is elapsed queue waiting, not GPU utilization timing.
                const started=performance.now();
                await owner.device.queue.onSubmittedWorkDone();active();
                lastCompletionWaitMs=Math.max(0,performance.now()-started);completionWaitMs+=lastCompletionWaitMs;frames++;
            },
            async abortFrame(){if(!frameScope)return;frameScope=false;await owner.device.popErrorScope();},
            resize(w,h){active();if(!Number.isInteger(w)||!Number.isInteger(h)||w<1||h<1||w*h>2073600)throw RangeError('SDK GPU viewport exceeds its pixel budget');if(canvas.width===w&&canvas.height===h)return;owner.resizeDrawingBuffer(w,h);renderer.setSize(w,h,false);},
            capture(){active();if(typeof canvas.transferToImageBitmap!=='function')throw Error('Game GPU surface cannot be transferred');return canvas.transferToImageBitmap();},
            inspect:statistics,close,
        });
    }catch(error){close();throw error;}
}
