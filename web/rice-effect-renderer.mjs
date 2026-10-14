import {parseRiceEffect,EFFECT_LIMITS} from './rice-effect-contract.mjs';
import {qualifyRiceEffect} from './rice-shader-validator.mjs';
import {createRiceVisualEffectWebGl2Context,riceWebGlRendererIsSoftware} from './rice-visual-effect-capability.mjs';
const VERTEX_GLSL=`#version 300 es
precision highp float;
out vec2 riceUv;
void main(){vec2 p=vec2(float((gl_VertexID<<1)&2),float(gl_VertexID&2));riceUv=vec2(p.x,1.0-p.y);gl_Position=vec4(p*2.0-1.0,0.0,1.0);}`;
const WGSL_HEADER=`struct RiceUniforms { size:vec2f, time:f32, phase:f32, params:vec4f, signals:vec4f };
@group(0) @binding(0) var<uniform> riceUniforms:RiceUniforms;
@group(0) @binding(1) var riceTexture:texture_2d<f32>;
@group(0) @binding(2) var riceSampler:sampler;
fn riceInput(uv:vec2f)->vec4f{return textureSampleLevel(riceTexture,riceSampler,uv,0.0);}
fn riceSignals()->vec4f{return riceUniforms.signals;}
@vertex fn riceVertex(@builtin(vertex_index) index:u32)->@builtin(position) vec4f{
 let p=vec2f(f32((index<<1u)&2u),f32(index&2u));return vec4f(p*2.0-1.0,0.0,1.0);
}
`;
export function riceEffectShaderSource(effect,language,qualification){
 const source=effect.implementations[language],functions=qualification?.functions||[];
 const call=functions.includes('riceEffect')?'riceEffect(uv, time, params)':'riceMaterial(vec3'+(language==='wgsl'?'f':'')+'(uv, 0.0), vec3'+(language==='wgsl'?'f':'')+'(0.0,0.0,1.0), uv, time, params)';
 if(language==='wgsl')return WGSL_HEADER+source+`\n@fragment fn riceFragment(@builtin(position) position:vec4f)->@location(0) vec4f{
 let uv=position.xy/riceUniforms.size;let time=riceUniforms.time;let params=riceUniforms.params;
 let color=clamp(${call},vec4f(0.0),vec4f(1.0));return vec4f(color.rgb*color.a,color.a);}`;
 return `#version 300 es
precision highp float;
in vec2 riceUv;out vec4 riceColor;
uniform float riceTime;uniform vec4 riceParams;uniform vec4 riceSignalValues;uniform sampler2D riceTexture;
vec4 riceInput(vec2 uv){return texture(riceTexture,vec2(uv.x,1.0-uv.y));}
vec4 riceSignals(){return riceSignalValues;}
#line 1
${source}
void main(){vec2 uv=riceUv;float time=riceTime;vec4 params=riceParams;riceColor=clamp(${call},0.0,1.0);}`;
}
const values=e=>new Float32Array([...e.values,0,0,0,0].slice(0,4));
/** Compositor-owned resource pool; every candidate compiles before it can commit. */
export class RiceEffectRenderer {
 constructor({canvasFactory=(w,h)=>{const c=typeof OffscreenCanvas==='function'?new OffscreenCanvas(w,h):document.createElement('canvas');c.width=w;c.height=h;return c;},gpu=globalThis.navigator?.gpu,mode='auto',onStatus=()=>{},changed=()=>{},now=()=>performance.now(),setTimer=setTimeout,clearTimer=clearTimeout}={}){
  Object.assign(this,{canvasFactory,gpu,mode,onStatus,changed,now,setTimer,clearTimer});this.prepared=new Map();this.layers=[];this.devicePromise=null;this.timer=null;this.sequence=0;this.closed=false;this.paused=false;this.lastFrame=-Infinity;this.stats={frames:0,compiles:0,textureBytes:0,backend:'none',gpuTimeMs:null};
 }
 async device(){
  if(!this.gpu||this.mode==='disabled'||this.mode==='software')return null;
  if(!this.devicePromise)this.devicePromise=(async()=>{
   const adapter=await this.gpu.requestAdapter({powerPreference:'high-performance'});if(!adapter)return null;
   const software=adapter.isFallbackAdapter||riceWebGlRendererIsSoftware(Object.values(adapter.info||{}).join(' '))||riceWebGlRendererIsSoftware(adapter.info?.description+' '+adapter.info?.architecture);
   if(this.mode==='hardware'&&software)return null;
   const device=await adapter.requestDevice();this.adapterClass=software?'software':'hardware';
   device.lost.then(info=>{if(this.closed)return;this.devicePromise=null;this.failed=String(info.message||'GPU device lost');this.pause(true);this.onStatus({status:'unavailable',reason:this.failed});this.changed();});
   device.addEventListener?.('uncapturederror',event=>{this.failed??=event.error?.message||'GPU execution error';this.pause(true);this.onStatus({status:'unavailable',reason:this.failed});this.changed();});
   if(this.closed){device.destroy();return null;}this.gpuDevice=device;return device;
  })().catch(error=>{this.onStatus({status:'backend-fallback',reason:error.message});return null;});
  return this.devicePromise;
 }
 async prepare(input,{backend='auto'}={}){
  if(this.closed)throw Error('Effect renderer is closed');
  const effect=parseRiceEffect(input),qualification=await qualifyRiceEffect(effect),generation=this.sequence;
  let surface,error;
  if(backend!=='webgl2'&&effect.implementations.wgsl){const device=await this.device();if(device)try{surface=await this.prepareGpu(effect,qualification.wgsl,device);}catch(e){error=e;}}
  if(!surface&&backend!=='webgpu'&&effect.implementations.glsl)try{surface=this.prepareGl(effect,qualification.glsl);}catch(e){error=e;}
  if(!surface)throw error||Error(`No compatible shader backend (${backend}). Keep the fallback color or provide another implementation.`);
  if(this.closed||generation!==this.sequence){surface.dispose();throw Error('Effect preparation was cancelled');}
  // Stale preparations cannot accumulate GPU resources indefinitely.
  for(const [token,p] of this.prepared)if(this.now()-p.created>60000){p.surface.dispose();this.prepared.delete(token);}
  if(this.prepared.size>=8){surface.dispose();throw Error('Too many prepared effects; finish or cancel the current preview');}
  const token=crypto.randomUUID();this.prepared.set(token,{surface,effect,created:this.now()});this.stats.compiles++;
  return {token,backend:surface.backend,adapterClass:surface.adapterClass,qualification,sourceRevision:token};
 }
 prepareGl(effect,qualification){
  const capability=createRiceVisualEffectWebGl2Context(this.canvasFactory,1,1,this.mode,{alpha:true,premultipliedAlpha:false,preserveDrawingBuffer:true,depth:false,antialias:false});
  if(!capability)throw Error('WebGL2 is unavailable under the selected acceleration policy');
  const {gl,canvas}=capability,shaders=[];let program;
  const compile=(type,source)=>{const shader=gl.createShader(type);shaders.push(shader);gl.shaderSource(shader,source);gl.compileShader(shader);if(!gl.getShaderParameter(shader,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(shader)||'Shader compilation failed');return shader;};
  try{program=gl.createProgram();gl.attachShader(program,compile(gl.VERTEX_SHADER,VERTEX_GLSL));gl.attachShader(program,compile(gl.FRAGMENT_SHADER,riceEffectShaderSource(effect,'glsl',qualification)));gl.linkProgram(program);if(!gl.getProgramParameter(program,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(program)||'Effect pipeline failed');}
  catch(error){if(program)gl.deleteProgram(program);gl.getExtension('WEBGL_lose_context')?.loseContext();throw error;}
  finally{for(const shader of shaders)gl.deleteShader(shader);}
  const locations=Object.fromEntries(['riceTime','riceParams','riceSignalValues','riceTexture'].map(n=>[n,gl.getUniformLocation(program,n)]));
  let textures=[],framebuffers=[],size='',disposed=false;
  const release=()=>{textures.forEach(t=>gl.deleteTexture(t));framebuffers.forEach(f=>gl.deleteFramebuffer(f));textures=[];framebuffers=[];};
  canvas.addEventListener?.('webglcontextlost',event=>{event.preventDefault();if(disposed)return;this.failed='WebGL context lost';this.pause(true);this.onStatus({status:'unavailable',reason:this.failed});this.changed();});
  return {backend:'webgl2',adapterClass:capability.adapterClass,canvas,draw:(width,height,time,signals)=>{
   if(disposed||gl.isContextLost())return false;
   if(size!==`${width}:${height}`){release();canvas.width=width;canvas.height=height;size=`${width}:${height}`;
    for(let i=0;i<2;i++){const texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D,texture);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_S,gl.CLAMP_TO_EDGE);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_WRAP_T,gl.CLAMP_TO_EDGE);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA8,width,height,0,gl.RGBA,gl.UNSIGNED_BYTE,null);const f=gl.createFramebuffer();gl.bindFramebuffer(gl.FRAMEBUFFER,f);gl.framebufferTexture2D(gl.FRAMEBUFFER,gl.COLOR_ATTACHMENT0,gl.TEXTURE_2D,texture,0);if(gl.checkFramebufferStatus(gl.FRAMEBUFFER)!==gl.FRAMEBUFFER_COMPLETE)throw Error('Effect render target allocation failed');textures.push(texture);framebuffers.push(f);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);}
   }
   gl.useProgram(program);gl.viewport(0,0,width,height);gl.uniform1f(locations.riceTime,time);gl.uniform4fv(locations.riceParams,values(effect));gl.uniform4fv(locations.riceSignalValues,new Float32Array(signals));gl.uniform1i(locations.riceTexture,0);
   gl.bindFramebuffer(gl.FRAMEBUFFER,framebuffers[1]);gl.clearColor(0,0,0,0);gl.clear(gl.COLOR_BUFFER_BIT);
   for(let pass=0;pass<effect.passes;pass++){gl.bindFramebuffer(gl.FRAMEBUFFER,pass===effect.passes-1?null:framebuffers[pass%2]);gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D,textures[(pass+1)%2]);gl.drawArrays(gl.TRIANGLES,0,3);}
   return true;
  },dispose:()=>{if(disposed)return;disposed=true;release();gl.deleteProgram(program);gl.getExtension('WEBGL_lose_context')?.loseContext();}};
 }
 async prepareGpu(effect,qualification,device){
  const canvas=typeof OffscreenCanvas==='function'?new OffscreenCanvas(1,1):this.canvasFactory(1,1),context=canvas.getContext('webgpu');if(!context||typeof canvas.transferToImageBitmap!=='function')throw Error('A transferable WebGPU canvas is unavailable');
  const format=this.gpu.getPreferredCanvasFormat(),usage=globalThis.GPUTextureUsage,bufferUsage=globalThis.GPUBufferUsage;
  let uniform,pipeline,layout,sampler;
  device.pushErrorScope('validation');
  try{
   const module=device.createShaderModule({label:effect.name,code:riceEffectShaderSource(effect,'wgsl',qualification)});
   const info=await module.getCompilationInfo();const errors=info.messages.filter(m=>m.type==='error');if(errors.length)throw Error(errors.map(m=>`Line ${Math.max(1,m.lineNum-WGSL_HEADER.split('\n').length+1)}: ${m.message}`).join('\n'));
   layout=device.createBindGroupLayout({entries:[{binding:0,visibility:2,buffer:{type:'uniform'}},{binding:1,visibility:2,texture:{sampleType:'float'}},{binding:2,visibility:2,sampler:{type:'filtering'}}]});
   pipeline=await device.createRenderPipelineAsync({layout:device.createPipelineLayout({bindGroupLayouts:[layout]}),vertex:{module,entryPoint:'riceVertex'},fragment:{module,entryPoint:'riceFragment',targets:[{format}]},primitive:{topology:'triangle-list'}});
   uniform=device.createBuffer({size:48,usage:bufferUsage.UNIFORM|bufferUsage.COPY_DST});sampler=device.createSampler({magFilter:'linear',minFilter:'linear'});
  }catch(e){uniform?.destroy();throw e;}finally{const error=await device.popErrorScope();if(error){uniform?.destroy();throw Error(error.message);}}
  context.configure({device,format,alphaMode:'premultiplied'});
  let textures=[],groups=[],size='',disposed=false,bitmap=null;
  const release=()=>{textures.forEach(t=>t.destroy());textures=[];groups=[];};
  return {backend:'webgpu',adapterClass:this.adapterClass,canvas,get image(){return bitmap||canvas;},draw:(width,height,time,signals)=>{
   if(disposed||this.failed)return false;
   if(size!==`${width}:${height}`){release();canvas.width=width;canvas.height=height;size=`${width}:${height}`;for(let i=0;i<2;i++)textures.push(device.createTexture({size:[width,height],format,usage:usage.RENDER_ATTACHMENT|usage.TEXTURE_BINDING}));groups=textures.map(t=>device.createBindGroup({layout,entries:[{binding:0,resource:{buffer:uniform}},{binding:1,resource:t.createView()},{binding:2,resource:sampler}]}));}
   device.queue.writeBuffer(uniform,0,new Float32Array([width,height,time,0,...values(effect),...signals]));
   const encoder=device.createCommandEncoder();
   const clear=encoder.beginRenderPass({colorAttachments:[{view:textures[1].createView(),clearValue:{r:0,g:0,b:0,a:0},loadOp:'clear',storeOp:'store'}]});clear.end();
   for(let index=0;index<effect.passes;index++){const pass=encoder.beginRenderPass({colorAttachments:[{view:index===effect.passes-1?context.getCurrentTexture().createView():textures[index%2].createView(),clearValue:{r:0,g:0,b:0,a:0},loadOp:'clear',storeOp:'store'}]});pass.setPipeline(pipeline);pass.setBindGroup(0,groups[(index+1)%2]);pass.draw(3);pass.end();}
   device.queue.submit([encoder.finish()]);bitmap?.close();bitmap=canvas.transferToImageBitmap();return true;
  },dispose:()=>{if(disposed)return;disposed=true;bitmap?.close();release();uniform.destroy();context.unconfigure();}};
 }
 commit(token,{owner,target='background',instance=null}={}){
  const candidate=this.prepared.get(token);if(!candidate)throw Error('Prepared effect expired');
  if(typeof owner!=='string'||!owner||!candidate.effect.targets.includes(target)||target==='widget'&&typeof instance!=='string')throw Error('Effect target is not declared');
  const index=this.layers.findIndex(l=>l.owner===owner&&l.target===target&&l.instance===instance);
  if(index<0&&this.layers.length>=EFFECT_LIMITS.layers)throw Error('At most three custom effect layers can be active');
  const layer={...candidate,owner,target,instance,token,started:this.now(),rendered:null};
  this.prepared.delete(token);if(index>=0){this.layers[index].surface.dispose();this.layers[index]=layer;}else this.layers.push(layer);
  this.mutationVersion=(this.mutationVersion||0)+1;this.failed=null;this.changed();this.arm();return this.status();
 }
 discard(token){const item=this.prepared.get(token);item?.surface.dispose();this.prepared.delete(token);return true;}
 async replace(owner,bindings){
  if(typeof owner!=='string'||!owner||!Array.isArray(bindings)||bindings.length>EFFECT_LIMITS.layers)throw Error('Invalid effect layer transaction');
  const version=this.mutationVersion||0,active=bindings.filter(b=>b.enabled!==false),other=this.layers.filter(l=>l.owner!==owner),candidates=[];
  if(other.length+active.length>EFFECT_LIMITS.layers)throw Error('At most three custom effect layers can be active; disable another recipe first');
  try{
   for(const b of active){const effect=parseRiceEffect(b.effect);if(!effect.targets.includes(b.target)||b.target==='world-material'||b.target==='widget'&&typeof b.instance!=='string')throw Error('Invalid desktop effect target');const p=await this.prepare(effect,{backend:b.backend||'auto'});candidates.push({...b,token:p.token});}
   const next=candidates.map(b=>({...this.prepared.get(b.token),owner,target:b.target,instance:b.instance||null,token:b.token,started:this.now(),rendered:null}));
   // No await after the generation check: the replacement is one synchronous
   // compositor operation, so failed compilation leaves every old layer intact.
   if(this.closed||version!==(this.mutationVersion||0))throw Error('Effect layers changed during compilation; run the recipe again');
   if(next.some(l=>!l.surface))throw Error('Prepared effect expired');
   const previous=this.layers.filter(l=>l.owner===owner);this.layers=[...other,...next];
   for(const b of candidates)this.prepared.delete(b.token);for(const l of previous)l.surface.dispose();
   this.mutationVersion=(this.mutationVersion||0)+1;this.failed=null;this.changed();this.arm();return this.status();
  }catch(error){for(const b of candidates)this.discard(b.token);throw error;}
 }
 remove(owner){this.mutationVersion=(this.mutationVersion||0)+1;for(const l of this.layers.filter(l=>l.owner===owner))l.surface.dispose();this.layers=this.layers.filter(l=>l.owner!==owner);this.changed();this.arm();return this.status();}
 pause(paused){const next=Boolean(paused);if(this.paused!==next){this.paused=next;this.arm();}return this.status();}
 setMode(mode){
  if(this.mode===mode)return;this.mode=mode;this.sequence++;this.mutationVersion=(this.mutationVersion||0)+1;
  for(const p of this.prepared.values())p.surface.dispose();this.prepared.clear();
  if(mode==='disabled'||this.layers.some(l=>mode==='hardware'&&l.surface.adapterClass==='software'||mode==='software'&&l.surface.adapterClass!=='software')){
   for(const l of this.layers)l.surface.dispose();this.failed=mode==='disabled'?'Custom effects are disabled by the acceleration policy':'Acceleration policy changed; run the recipe again to compile a compatible backend';this.pause(true);this.onStatus({status:'paused',reason:this.failed});this.changed();
  }else if(this.failed){this.onStatus({status:'paused',reason:'Run the recipe again to restore effects under the new acceleration policy'});}
 }
 setInputs({pointer,audio,session}={}){
  const clamp=v=>Number.isFinite(v)?Math.max(0,Math.min(1,v)):0,next=[...(this.signals||[0,0,0,0])];
  if(pointer){next[0]=clamp(pointer[0]);next[1]=clamp(pointer[1]);}if(audio!==undefined)next[2]=clamp(audio);if(session!==undefined)next[3]=clamp(session);
  const previous=this.signals||[0,0,0,0];this.signals=next;
  if(!this.paused&&this.layers.some(l=>l.effect.inputs.some(input=>input==='pointer'&&(next[0]!==previous[0]||next[1]!==previous[1])||input==='audio'&&next[2]!==previous[2]||input==='session'&&next[3]!==previous[3])))this.changed();
 }
 arm(){
  if(this.timer!==null){this.clearTimer(this.timer);this.timer=null;}
  const dynamic=this.layers.filter(l=>l.effect.animated||l.effect.inputs.some(i=>i==='audio'||i==='session'));
  if(this.closed||this.paused||!dynamic.length)return;
  const fps=Math.min(30,...dynamic.map(l=>l.effect.frameRate));
  this.timer=this.setTimer(()=>{this.timer=null;if(!this.paused&&!this.closed){this.changed();this.arm();}},1000/fps);
 }
 render(context,{width,height,target='background',rects=null,signals=this.signals||[0,0,0,0],reducedMotion=false}={}){
  const now=this.now();
  for(const l of this.layers.filter(l=>l.target===target)){
   const list=rects||[{x:0,y:0,width,height}],scale=l.effect.scale,ratio=Math.min(scale,Math.sqrt(EFFECT_LIMITS.pixels/Math.max(1,width*height)),Math.sqrt(EFFECT_LIMITS.textureBytes/Math.max(1,this.layers.length*16*width*height)));
   const w=Math.max(1,Math.floor(width*ratio)),h=Math.max(1,Math.floor(height*ratio));
   const inputSignals=[...(l.effect.inputs.includes('pointer')?signals.slice(0,2):[0,0]),l.effect.inputs.includes('audio')?signals[2]:0,l.effect.inputs.includes('session')?signals[3]:0].map(v=>Number.isFinite(v)?Math.max(0,Math.min(1,v)):0);
   const key=`${w}:${h}:${reducedMotion?0:l.effect.animated&&!this.paused?Math.floor(now/(1000/l.effect.frameRate)):0}:${inputSignals.join(',')}`;
   let ready=!this.failed;
   if(ready&&l.rendered!==key)try{ready=l.surface.draw(w,h,reducedMotion||!l.effect.animated?0:(now-l.started)/1000,inputSignals);if(ready){l.rendered=key;this.stats.frames++;this.stats.backend=l.surface.backend;}}catch(error){ready=false;this.failed=error.message;this.pause(true);this.onStatus({status:'unavailable',reason:error.message});}
   ready=ready&&l.surface.ready!==false;
   context.save();if(['shell','window-frame'].includes(target))context.globalAlpha*=0.3;for(const rect of list){if(l.instance&&rect.id!==l.instance)continue;context.save();context.beginPath();context.rect(rect.x,rect.y,rect.width,rect.height);context.clip();if(ready)context.drawImage(l.surface.image||l.surface.canvas,rect.x,rect.y,rect.width,rect.height);else if(target==='background'){context.fillStyle=l.effect.fallback;context.fillRect(rect.x,rect.y,rect.width,rect.height);}context.restore();}context.restore();
  }
  this.stats.textureBytes=this.layers.reduce((sum,l)=>sum+(l.surface.canvas.width*l.surface.canvas.height*16),0);
 }
 status(){return {layers:this.layers.map(l=>({owner:l.owner,generation:l.token,target:l.target,instance:l.instance,id:l.effect.id,name:l.effect.name,backend:l.surface.backend,adapterClass:l.surface.adapterClass})),paused:this.paused,error:this.failed||null,...this.stats};}
 async action(action,data={}){if(action==='replace')return this.replace(data.owner,data.bindings);if(action==='prepare')return this.prepare(data.effect,{backend:data.backend});if(action==='commit')return this.commit(data.token,data);if(action==='discard')return this.discard(data.token);if(action==='remove')return this.remove(data.owner);if(action==='pause')return this.pause(data.paused);if(action==='status')return this.status();throw Error('Unknown effect renderer action');}
 dispose(){if(this.closed)return;this.closed=true;this.sequence++;if(this.timer!==null)this.clearTimer(this.timer);for(const p of this.prepared.values())p.surface.dispose();for(const l of this.layers)l.surface.dispose();this.prepared.clear();this.layers=[];this.gpuDevice?.destroy();}
}
