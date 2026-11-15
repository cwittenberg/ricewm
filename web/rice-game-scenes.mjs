import {createGameSceneStrategy,createCpuSceneStrategy} from './rice-game-renderer-strategies.mjs';
import {renderRiceWebScene} from './rice-native-web-scene-renderer.mjs';

const MAX_SURFACES=128,MAX_BITMAP_BYTES=64*1024*1024;
function rasterSize(scene,context,allScenes){
 let scale=1;
 if(allScenes&&scene.rect){const m=context?.getTransform?.(),density=m?Math.max(Math.hypot(m.a,m.b),Math.hypot(m.c,m.d)):1;scale=Math.min(scene.rect.width/scene.viewportWidth,scene.rect.height/scene.viewportHeight)*(Number.isFinite(density)?density:1);}
 scale=Math.min(scale,Math.sqrt(2073600/(scene.viewportWidth*scene.viewportHeight)),4096/scene.viewportWidth,4096/scene.viewportHeight);
 return [Math.max(1,Math.floor(scene.viewportWidth*scale)),Math.max(1,Math.floor(scene.viewportHeight*scale))];
}
const keyOf=scene=>`${scene.cacheNamespace}:${scene.surface}`;
/** Private compositor adapter. Only host-attributed game owners enter it;
 * the same Strategy can select a backend for all validated SDK vector scenes. */
export class RiceGameScenes {
 constructor({open=null,allScenes=false,onDamage=()=>{},onStatus=()=>{},onFault=()=>{}}={}){
  this.open=open;this.allScenes=allScenes;this.liveApps=new Set();this.acceleration='auto';this.clock=0;this.bitmapBytes=0;
  this.onDamage=onDamage;this.onStatus=onStatus;this.onFault=onFault;this.owners=new Set();this.surfaces=new Set();this.entries=new Map();this.closed=false;this.onRelease=new Set();
 }
 reconcile(snapshot){
  const live=new Set((snapshot?.liveApps??[]).map(String));
  this.liveApps=live;
  this.owners=new Set((snapshot?.gameApps??[]).map(String).filter(owner=>live.has(owner)));
  this.surfaces=new Set((snapshot?.gameSurfaces??[]).filter(value=>live.has(String(value.app))).map(value=>`${value.app}:${value.surface}`));
  for(const [key,entry]of this.entries){
   if(live.has(entry.owner))entry.seenLive=true;
   if(!live.has(entry.owner)&&entry.seenLive||!entry.explicit&&!this.owners.has(entry.owner)&&!this.surfaces.has(key)&&!(this.allScenes&&live.has(entry.owner))){this.entries.delete(key);this.release(entry);}
  }
 }
 render(context,scene,mediaSurface){
  // Native controls without a submitted frame still emit an empty scene.
  // Their backgrounds are already painted by the compositor. Reserving a
  // renderer for each one can evict real chart surfaces from the bounded pool.
  if(scene.displayList?.length===0)return renderRiceWebScene(context,scene,mediaSurface);
  if(this.closed||!scene.vectorScene||!this.owners.has(String(scene.cacheNamespace))&&!this.surfaces.has(keyOf(scene))&&!this.entries.get(keyOf(scene))?.explicit&&!(this.allScenes&&this.liveApps.has(String(scene.cacheNamespace))))return renderRiceWebScene(context,scene,mediaSurface);
  const [rasterWidth,rasterHeight]=rasterSize(scene,context,this.allScenes);scene={...scene,rasterWidth,rasterHeight};
  const key=keyOf(scene);let entry=this.entries.get(key);
  if(!entry){
   if(this.entries.size>=MAX_SURFACES){const old=[...this.entries.values()].filter(e=>!e.busy&&!e.explicit).sort((a,b)=>a.last-b.last)[0];if(!old){this.drawFault(context,scene);return;}this.entries.delete(old.key);this.release(old);}
   entry=this.createEntry(key,String(scene.cacheNamespace),scene);
  }
  entry.last=++this.clock;
  if(entry.status==='cpu')return entry.renderer.paint(context,scene,mediaSurface);
  if(entry.epoch!==scene.epoch||entry.width!==scene.viewportWidth||entry.height!==scene.viewportHeight||rasterWidth>(entry.lastFrame?.rasterWidth||0)||rasterHeight>(entry.lastFrame?.rasterHeight||0)){
   // Native ingress has validated these immutable self-contained bytes. Keep
   // only the newest frame while GPU work is pending; never form a queue.
   entry.epoch=scene.epoch;entry.width=scene.viewportWidth;entry.height=scene.viewportHeight;
   // Keep dimensions as a pair. Combining the largest width and height from
   // different layouts creates an oversized, distorted drawing buffer.
   entry.submittedFrames++;if(entry.pending)entry.coalescedFrames++;
   entry.lastFrame={width:scene.viewportWidth,height:scene.viewportHeight,rasterWidth,rasterHeight,displayList:scene.displayList};entry.pending=entry.lastFrame;
   void this.pump(entry);
  }
  if(entry.renderer){
   entry.renderer.paint(context,scene,mediaSurface);
  }else if(entry.error){
   // Fail this owned game surface visibly. A bad frame must never crash the
   // desktop or silently choose CPU when a working GPU has been admitted.
   this.drawFault(context,scene);
  }
 }
 createEntry(key,owner,scene){
  const entry={key,owner,abort:new AbortController(),renderer:null,pending:null,lastFrame:null,busy:false,status:'preparing',epoch:null,error:null,work:Promise.resolve(),captures:0,submittedFrames:0,completedFrames:0,coalescedFrames:0,explicit:false,explicitOwners:new Set(),seenLive:this.owners.has(owner)||this.surfaces.has(key)};
  this.entries.set(key,entry);
  const unavailable=this.open?.unavailable?.();
  if(this.acceleration==='software'||unavailable){
   entry.renderer=createCpuSceneStrategy(this.acceleration==='software'?'disabled-by-setting':typeof unavailable==='string'?unavailable:'WebGPU is unavailable');entry.status='cpu';entry.fallbackReason=entry.renderer.inspect().fallbackReason;entry.ready=Promise.resolve();
  }else entry.ready=this.start(entry,scene);
  return entry;
 }
 async bindExternalSurface({app,surface,width=640,height=360},lease){
  if(this.closed)throw Error('Game compositor closed');const owner=String(app),key=owner+':'+String(surface);
  if(!/^[1-9][0-9]{0,19}$/.test(owner)||BigInt(owner)>0xffffffffffffffffn||!/^[1-9][0-9]{0,19}$/.test(String(surface))||BigInt(surface)>0xffffffffffffffffn||!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>16384||height>16384)throw Error('Invalid game surface owner');
  let entry=this.entries.get(key);if(!entry){if(this.entries.size>=MAX_SURFACES)throw RangeError('Game surface budget exceeded');entry=this.createEntry(key,owner,{viewportWidth:width,viewportHeight:height});}
  entry.explicitOwners.add(lease);entry.explicit=true;await entry.ready;if(entry.abort.signal.aborted||entry.status!=='webgpu'){this.releaseExternalSurface(key,lease);throw Error(entry.error?.message||'Owned GPU game surface is unavailable');}return key;
 }
 releaseExternalSurface(key,lease){const entry=this.entries.get(key);if(!entry)return;entry.explicitOwners.delete(lease);entry.explicit=entry.explicitOwners.size>0;if(!entry.explicit&&!this.owners.has(entry.owner)&&!this.surfaces.has(key)){this.entries.delete(key);this.release(entry);}}
 setExternalImage(key,id,bitmap){
  const entry=this.entries.get(key);if(!entry||entry.abort.signal.aborted||entry.status!=='webgpu'){bitmap?.close();throw Error('Game image owner is no longer available');}
  try{entry.renderer.bindExternalImage(id,bitmap);}catch(error){bitmap?.close();throw error;}
  if(entry.lastFrame){entry.pending=entry.lastFrame;void this.pump(entry);}
 }
 removeExternalImage(key,id){const entry=this.entries.get(key);entry?.renderer?.unbindExternalImage?.(id);if(entry?.lastFrame){entry.pending=entry.lastFrame;void this.pump(entry);}}
 drawFault(context,scene){
  context.save();try{context.beginPath();context.rect(scene.rect.x,scene.rect.y,scene.rect.width,scene.rect.height);context.clip();context.fillStyle='#18151a';context.fillRect(scene.rect.x,scene.rect.y,scene.rect.width,scene.rect.height);context.fillStyle='#e9dfe3';context.font='13px "RiceWM Sans", sans-serif';context.fillText('Rendering stopped. See app diagnostics.',scene.rect.x+12,scene.rect.y+24,Math.max(1,scene.rect.width-24));}finally{context.restore();}
 }
 async start(entry,scene){
  try{
   // Render at native game resolution, with a bounded host-owned surface. The
   // native compositor applies the same fit transform used for pointer input.
   const [width,height]=scene.rasterWidth?[scene.rasterWidth,scene.rasterHeight]:rasterSize(scene,null,this.allScenes);
   const renderer=await createGameSceneStrategy({acceleration:this.acceleration,width,height,signal:entry.abort.signal,onLost:error=>this.fail(entry,error)},{open:this.open});
   if(entry.abort.signal.aborted){renderer.close();return;}entry.renderer=renderer;entry.status=renderer.inspect().backend;entry.fallbackReason=renderer.inspect().fallbackReason;if(entry.status==='cpu')entry.pending=null;this.status(entry);this.onDamage(entry.owner);void this.pump(entry);
  }catch(error){
   if(entry.abort.signal.aborted)return;
   this.fail(entry,error);
  }
 }
 async pump(entry){
  if(entry.busy||entry.status!=='webgpu'||!entry.pending||entry.abort.signal.aborted)return;
  entry.busy=true;const frame=entry.pending;entry.pending=null;
  try{
   await this.withRenderer(entry,renderer=>{const [width,height]=frame.rasterWidth?[frame.rasterWidth,frame.rasterHeight]:rasterSize({viewportWidth:frame.width,viewportHeight:frame.height},null,false),bytes=width*height*4;
    if(this.bitmapBytes-(entry.bitmapBytes||0)+bytes>MAX_BITMAP_BYTES)throw RangeError('SDK scene bitmap budget exceeded');
    this.bitmapBytes+=bytes-(entry.bitmapBytes||0);entry.bitmapBytes=bytes;renderer.resize?.(width,height);return renderer.present(frame);});if(entry.abort.signal.aborted)return;
   entry.completedFrames++;this.onDamage(entry.owner);
  }catch(error){if(!entry.abort.signal.aborted)this.fail(entry,error);}
  finally{entry.busy=false;if(entry.status==='webgpu'&&entry.pending)void this.pump(entry);}
 }
 withRenderer(entry,operation){
  const task=entry.work.then(()=>{if(entry.abort.signal.aborted||entry.status!=='webgpu'||!entry.renderer)throw Error('Game renderer is no longer available');return operation(entry.renderer);});entry.work=task.catch(()=>{});return task;
 }
 async captureScenes(owner,commands){
  owner=String(owner);if(!/^[1-9][0-9]{0,19}$/.test(owner)||!Array.isArray(commands)||commands.length>65536)throw Error('Invalid game capture owner');
  const captures=new Map();let capturedBytes=0;
  try{for(const scene of commands){
   if(scene.type!=='web-scene'||!scene.vectorScene||scene.displayList?.length===0)continue;const key=owner+':'+scene.surface;
   let entry=this.entries.get(key);if(!entry&&(this.owners.has(owner)||this.surfaces.has(key)||this.allScenes&&this.liveApps.has(owner))){if(this.entries.size>=MAX_SURFACES)throw Error('Game capture surface budget exceeded');entry=this.createEntry(key,owner,scene);}
   if(!entry)continue;await entry.ready;if(entry.status==='cpu')continue;if(entry.status!=='webgpu')throw Error(entry.error?.message||'Game GPU capture is unavailable');
   const bitmap=await this.withRenderer(entry,renderer=>renderer.captureFrame({width:scene.viewportWidth,height:scene.viewportHeight,displayList:scene.displayList}));
   if(entry.abort.signal.aborted){bitmap.close();throw Error('Game capture owner closed');}
   capturedBytes+=(bitmap.width||0)*(bitmap.height||0)*4;if(capturedBytes>MAX_BITMAP_BYTES){bitmap.close();throw RangeError('SDK scene capture bitmap budget exceeded');}
   captures.get(String(scene.surface))?.close();captures.set(String(scene.surface),bitmap);entry.captures++;
  }return captures;}catch(error){for(const bitmap of captures.values())bitmap.close();throw error;}
 }
 status(entry){this.onStatus({owner:entry.owner,surface:entry.key,backend:entry.status,fallbackReason:entry.fallbackReason??null,...entry.renderer?.inspect()});}
 fail(entry,error){if(entry.abort.signal.aborted||entry.status==='failed')return;entry.status='failed';this.bitmapBytes-=entry.bitmapBytes||0;entry.bitmapBytes=0;entry.error=error;entry.pending=null;const renderer=entry.renderer;entry.renderer=null;
  try{renderer?.close();}catch(cleanupError){entry.cleanupError=String(cleanupError?.message||cleanupError);}finally{
   // A renderer unusable for this surface must not blank an otherwise usable
   // application. Preserve its exact diagnostic and use the existing fallback.
   if(this.allScenes&&!entry.explicit){entry.gpuError=String(error?.message||error);entry.renderer=createCpuSceneStrategy(error?.code==='WEBGPU_DEVICE_LOST'?'WebGPU device lost':'WebGPU rendering failed: '+entry.gpuError);entry.status='cpu';entry.error=null;entry.fallbackReason=entry.renderer.inspect().fallbackReason;}else this.onFault(error);
   this.status(entry);this.onDamage(entry.owner);
  }}
 inspect(){return [...this.entries.values()].map(entry=>({owner:entry.owner,surface:entry.key,backend:entry.status,fallbackReason:entry.fallbackReason??null,error:entry.error?.message??null,gpuError:entry.gpuError??null,cleanupError:entry.cleanupError??null,explicitCaptures:entry.captures,retainedBitmapBytes:entry.bitmapBytes||0,submittedFrames:entry.submittedFrames,completedFrames:entry.completedFrames,coalescedFrames:entry.coalescedFrames,framePending:entry.busy||!!entry.pending,frameBytes:entry.lastFrame?.displayList.byteLength||0,...entry.renderer?.inspect()}));}
 release(entry){if(entry.abort.signal.aborted)return;this.bitmapBytes-=entry.bitmapBytes||0;entry.bitmapBytes=0;entry.abort.abort();entry.pending=null;entry.lastFrame=null;entry.renderer?.close();entry.renderer=null;for(const listener of this.onRelease)listener(entry.key);}
 setAccelerationMode(mode){mode=['auto','hardware','software'].includes(mode)?mode:'auto';if(this.acceleration===mode)return;const changed=(this.acceleration==='software')!==(mode==='software');this.acceleration=mode;if(!changed)return;const pinned=[...this.entries.values()].some(e=>e.explicit);for(const [key,entry]of this.entries)if(!entry.explicit){this.entries.delete(key);this.release(entry);}if(!pinned)this.open?.reset?.();}
 close(){if(this.closed)return;this.closed=true;for(const e of this.entries.values())this.release(e);this.entries.clear();this.owners.clear();this.surfaces.clear();this.onRelease.clear();this.liveApps.clear();this.open?.close?.();}
}
