import {t} from './rice-i18n.mjs';
/** Placeable original GLava surfaces using the shared widget shell and audio bus. */
export class RiceGlavaWidgetAdapter {
 constructor(host){this.host=host;this.surfaces=new Map();this.closed=false;this.listen=e=>this.message(e);globalThis.window?.addEventListener('message',this.listen);}
 send(surface,message){surface.frame.contentWindow?.postMessage({channel:'rice-glava-host',...message},location.origin);}
 releaseAudio(surface){surface.tap?.close();surface.tap=null;surface.waitForAudio?.();surface.waitForAudio=null;surface.audio=null;surface.idle=false;}
 wake(surface){if(!surface.active)return;surface.waitForAudio?.();surface.waitForAudio=null;surface.idle=false;this.send(surface,{type:'wake'});}
 audioChanged(){for(const surface of this.surfaces.values()){this.releaseAudio(surface);if(surface.active)this.wake(surface);}}
 destroy(surface){this.releaseAudio(surface);surface.frame.contentWindow?.riceGlavaFrame?.close();surface.frame.remove();this.surfaces.delete(surface.id);}
 update(state){
  for(const surface of this.surfaces.values()){
   const instance=state?.instances?.find(i=>i.id===surface.id);
   if(!instance){this.destroy(surface);continue;}
   if(surface.active!==instance.visible){surface.active=instance.visible;if(!surface.active)this.releaseAudio(surface);this.send(surface,{type:'active',active:surface.active});}
  }
 }
 async message(event){
  if(this.closed||event.origin!==location.origin||event.data?.channel!=='rice-glava-widget')return;
  const surface=[...this.surfaces.values()].find(s=>s.frame.contentWindow===event.source);if(!surface)return;
  const {type,value}=event.data;
  if(type==='loaded'){if(surface.error){surface.error=null;this.report(surface);}surface.loaded=true;surface.palette='';surface.frameRate=null;surface.frames=value.frames;this.send(surface,{type:'geometry',width:surface.rect.width,height:surface.rect.height});this.send(surface,{type:'active',active:surface.active});this.host.invalidate();}
  else if(type==='frame')this.host.invalidate(surface.rect,surface.id);
  else if(type==='idle'){
   surface.tap?.close();surface.tap=null;surface.idle=true;
   surface.waitForAudio?.();surface.waitForAudio=null;
   if(surface.active&&surface.audio?.watchPcmActivity)surface.waitForAudio=surface.audio.watchPcmActivity(()=>this.wake(surface),{since:surface.activityRevision});
  }else if(type==='pcm'){
   try{
    if(value.frames!==surface.frames||value.rate!==22050)throw Error('Invalid GLava audio request');
    const audio=this.host.renderer?.visualizerAudio||this.host.audio;
    if(surface.audio!==audio){this.releaseAudio(surface);surface.audio=audio;}
    surface.waitForAudio?.();surface.waitForAudio=null;surface.idle=false;
    surface.activityRevision=surface.audio?.pcmActivityRevision;
    if(surface.active&&surface.audio&&!surface.tap)surface.tap=surface.audio.createPcmTap({frames:value.frames,rate:value.rate});
    const pcm=surface.active&&surface.tap?surface.tap.sample():{left:new Float32Array(value.frames),right:new Float32Array(value.frames)};
    this.send(surface,{type:'pcm',left:pcm.left,right:pcm.right});
   }catch(error){surface.error=error.message;this.send(surface,{type:'active',active:false});this.releaseAudio(surface);this.report(surface);}
  }else if(type==='error'){surface.error=String(value);this.releaseAudio(surface);this.report(surface);}
  if(type==='log'||type==='error')void this.host.request('log',{id:surface.id,message:String(value)}).catch(()=>{});
 }
 report(surface){void this.host.request('status',{id:surface.id,error:surface.error}).catch(()=>{});this.host.invalidate();}
 paint(context,rect,pkg,settings,{instance,state}){
  if(this.host.renderer?.safeGraphics){
   context.fillStyle=Number.isInteger(state.appearance?.text)?'#'+(state.appearance.text&0xffffff).toString(16).padStart(6,'0'):'#eff0f1';
   context.font='13px system-ui';context.fillText(t('Visualizer paused in safe graphics mode'),rect.x+12,rect.y+24,rect.width-24);return;
  }
  let surface=this.surfaces.get(instance.id);
  if(!surface){
   const frame=document.createElement('iframe');frame.title=pkg.name+' widget runtime';frame.tabIndex=-1;frame.setAttribute('aria-hidden','true');frame.style.cssText='position:fixed;left:-20000px;top:0;border:0;pointer-events:none;';frame.src=new URL('./rice-glava-widget-frame.html',import.meta.url).href;
   surface={id:instance.id,frame,rect,active:instance.visible,loaded:false};this.surfaces.set(instance.id,surface);document.body.append(frame);
  }
  if(surface.rect.width!==rect.width||surface.rect.height!==rect.height)this.send(surface,{type:'geometry',width:rect.width,height:rect.height});surface.rect=rect;
  if(surface.loaded){
   const frameRate=this.host.presentation?Math.min(30,settings?.frameRate??30):settings?.frameRate??30;
   if(surface.frameRate!==frameRate){surface.frameRate=frameRate;this.send(surface,{type:'settings',frameRate});}
   const color=(value,fallback)=>Number.isInteger(value)?value&0xffffff:fallback;
   const palette={accent:color(state.appearance?.accent,0x3daee9),text:color(state.appearance?.text,0xeff0f1)},key=JSON.stringify(palette);
   if(surface.palette!==key){surface.palette=key;this.send(surface,{type:'palette',palette});}
  }
  if(surface.loaded&&!surface.error){const canvas=surface.frame.contentWindow?.riceGlavaFrame?.canvas;if(canvas)context.drawImage(canvas,rect.x,rect.y,rect.width,rect.height);}
  else{context.fillStyle=Number.isInteger(state.appearance?.text)?'#'+(state.appearance.text&0xffffff).toString(16).padStart(6,'0'):'#eff0f1';context.font='13px system-ui';context.fillText(surface.error||'Starting '+pkg.name+'…',rect.x+12,rect.y+24,rect.width-24);}
 }
 close(){this.closed=true;globalThis.window?.removeEventListener('message',this.listen);for(const surface of [...this.surfaces.values()])this.destroy(surface);}
}
