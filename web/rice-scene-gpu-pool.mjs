import {riceWebGpuPresentationNeedsResizeWorkaround} from './rice-visual-effect-capability.mjs';

/** One owned GPU/device and serialized drawing lane for SDK vector surfaces.
 * App identity, input, native validation and the public SDK stay unchanged.
 * Retained bitmaps belong to their surface; compiled models have a bounded LRU.
 */
export class RiceSceneGpuPool {
 constructor({openDevice=null,createScene=null,maxModels=8,scope=globalThis,...deviceOptions}={}){
  this.scope=scope;this.openDevice=openDevice;this.createScene=createScene;this.deviceOptions=deviceOptions;
  this.maxModels=maxModels;this.views=new Set();this.pending=null;this.gpu=null;this.Scene=null;
  this.work=Promise.resolve();this.generation=0;this.clock=0;this.closed=false;this.active=null;
  this.open=options=>this.acquire(options);this.open.close=()=>this.close();this.open.reset=()=>this.reset();
  this.open.unavailable=()=>this.openDevice?null:riceWebGpuPresentationNeedsResizeWorkaround(this.scope)?'WebGPU viewport resizing is unavailable on this browser':!(this.deviceOptions.gpu??this.scope.navigator?.gpu)?.requestAdapter?'WebGPU is unavailable':null;
 }
 async prepare(){
  if(this.closed)throw Error('SDK scene renderer closed');
  if(!this.openDevice&&riceWebGpuPresentationNeedsResizeWorkaround(this.scope))throw Object.assign(Error('WebGPU viewport resizing is unavailable on this browser'),{code:'WEBGPU_UNAVAILABLE'});
  if(!this.pending){
   const generation=this.generation;
   this.pending=(async()=>{
    const open=this.openDevice??(await import('./rice-game-webgpu-device.mjs')).createGameWebGpuRenderer;
    const gpu=await open({...this.deviceOptions,width:1,height:1,isolateFrameErrors:true,onLost:error=>{if(this.generation===generation)this.lost(error);}});
    try{
     const Scene=this.createScene??((await import('./rice-game-webgpu-scene.mjs')).RiceGameWebGpuScene);
     if(this.closed||generation!==this.generation)throw Error('SDK scene renderer superseded');
     this.gpu=gpu;this.Scene=Scene;return gpu;
    }catch(error){gpu.close();throw error;}
   })();
  }
  return this.pending;
 }
 async acquire(options){
  const gpu=await this.prepare();if(this.closed||this.gpu!==gpu)throw Error('SDK scene renderer superseded');options.signal?.throwIfAborted?.();
  const view={options,model:null,bitmap:null,closed:false,last:0,stats:null};this.views.add(view);
  const active=()=>{if(view.closed||this.closed)throw Error('SDK scene surface closed');options.signal?.throwIfAborted?.();};
  return {
   render:frame=>{
    const task=this.work.then(async()=>{
     active();const model=this.model(view);this.active=view;
     try{
      this.gpu.resize(options.width,options.height);
      await model.render(frame);active();
      // Capture before releasing the shared lane. A different surface must
      // never overwrite this canvas between render() and capture().
      view.bitmap?.close();view.bitmap=this.gpu.capture();view.stats=model.inspect();
     }finally{this.active=null;}
    });
    this.work=task.catch(()=>{});return task;
   },
   resize:(width,height)=>{active();options.width=width;options.height=height;},
   capture:()=>{active();if(!view.bitmap)throw Error('SDK scene has no completed GPU frame');const bitmap=view.bitmap;view.bitmap=null;return bitmap;},
   bindExternalImage:(id,bitmap)=>{active();this.model(view).bindExternalImage(id,bitmap);},
   unbindExternalImage:id=>view.model?.unbindExternalImage(id),
   inspect:()=>({...view.stats,backend:'webgpu',applicationReadbacks:0,sharedDevice:true,gpuStorage:this.gpu?.owner.snapshot()}),
   close:()=>this.release(view),
  };
 }
 model(view){
  view.last=++this.clock;if(view.model)return view.model;
  const models=[...this.views].filter(v=>v.model);
  // External image leases and in-flight work cannot be evicted. The ordinary
  // allocation guard remains authoritative when pinned live games fill it.
  for(const old of models.sort((a,b)=>a.last-b.last)){
   const resources=this.gpu.owner.snapshot().resources;
   if(models.filter(v=>v.model).length<this.maxModels&&resources<1024)break;
   if(old===this.active||old.model.externalImages?.size)continue;
   old.model.close();old.model=null;
  }
  view.model=this.createScene?this.createScene(this.gpu):new this.Scene(this.gpu,{ownsGpu:false});
  return view.model;
 }
 release(view){if(view.closed)return;view.closed=true;this.views.delete(view);view.bitmap?.close();view.bitmap=null;view.model?.close();view.model=null;}
 lost(error){for(const view of [...this.views])view.options.onLost?.(error);this.reset();}
 reset(){
  this.generation++;for(const view of [...this.views])this.release(view);
  this.gpu?.close();this.gpu=null;this.pending=null;this.Scene=null;
 }
 close(){if(this.closed)return;this.closed=true;this.reset();}
}
