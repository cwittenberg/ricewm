import {renderRiceWebScene} from './rice-native-web-scene-renderer.mjs';

/** Internal renderer Strategy. Both implementations present into the existing
 * native command's clip and opacity; no DOM or window behavior is replaced. */
class CpuGameSceneStrategy {
 constructor(reason){this.reason=reason;this.closed=false;}
 async present(){}
 paint(context,scene,mediaSurface){if(!this.closed)renderRiceWebScene(context,scene,mediaSurface);}
 inspect(){return {backend:'cpu',fallbackReason:this.reason,applicationReadbacks:0};}
 close(){this.closed=true;}
}
export const createCpuSceneStrategy=reason=>new CpuGameSceneStrategy(reason);
class WebGpuGameSceneStrategy {
 constructor(scene){this.scene=scene;this.bitmap=null;this.closed=false;}
 async present(frame){
  if(this.closed)throw Error('Game renderer closed');
  await this.scene.render(frame);if(this.closed)return;
  const bitmap=this.scene.capture();this.bitmap?.close();this.bitmap=bitmap;
 }
 resize(width,height){if(this.scene.resize)this.scene.resize(width,height);else this.scene.gpu?.resize(width,height);}
 async captureFrame(frame){if(this.closed)throw Error('Game renderer closed');await this.scene.render(frame);if(this.closed)throw Error('Game renderer closed');return this.scene.capture();}
 bindExternalImage(id,bitmap){this.scene.bindExternalImage(id,bitmap);}
 unbindExternalImage(id){this.scene.unbindExternalImage(id);}
 paint(context,scene){
  if(this.closed||!this.bitmap)return;
  const scale=Math.min(scene.rect.width/scene.viewportWidth,scene.rect.height/scene.viewportHeight),width=scene.viewportWidth*scale,height=scene.viewportHeight*scale;
  context.save();try{context.beginPath();context.rect(scene.rect.x,scene.rect.y,scene.rect.width,scene.rect.height);context.clip();context.drawImage(this.bitmap,scene.rect.x+(scene.rect.width-width)/2,scene.rect.y+(scene.rect.height-height)/2,width,height);}finally{context.restore();}
 }
 inspect(){return {...this.scene.inspect(),backend:'webgpu',fallbackReason:null};}
 close(){if(this.closed)return;this.closed=true;this.scene.close();this.bitmap?.close();this.bitmap=null;}
}

/** Factory performs capability selection once. Runtime frame/shader/quota
 * errors remain owned failures; they never masquerade as unavailable WebGPU. */
export async function createGameSceneStrategy(options,{open}={}){
 if(options.acceleration==='software')return new CpuGameSceneStrategy('disabled-by-setting');
 const factory=open??(async o=>{
  const {createGameWebGpuRenderer}=await import('./rice-game-webgpu-device.mjs');
  const gpu=await createGameWebGpuRenderer(o);
  try{const {RiceGameWebGpuScene}=await import('./rice-game-webgpu-scene.mjs');return new RiceGameWebGpuScene(gpu);}catch(error){gpu.close();throw error;}
 });
 try{return new WebGpuGameSceneStrategy(await factory(options));}
 catch(error){if(error?.code!=='WEBGPU_UNAVAILABLE')throw error;return new CpuGameSceneStrategy(error.message);}
}
