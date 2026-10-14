import {widgetSurfacePreferences} from './rice-widget-surface-preferences.mjs';
/** Real Qt Quick surfaces, composited behind application windows by Rice. */
const cssColor=(value,fallback)=>Number.isInteger(value)?'#'+(value&0xffffff).toString(16).padStart(6,'0'):fallback;
const widgetPalette=state=>{const p=state?.appearance||{};return {background:cssColor(p.background,'#232629'),text:cssColor(p.text,'#eff0f1'),accent:cssColor(p.accent,'#3daee9'),icon:cssColor(p.icon,cssColor(p.text,'#eff0f1')),iconVariant:p.iconVariant||'monochrome',iconOverrides:p.iconOverrides||{},themeIcons:p.themeIcons||{},iconThemeName:p.iconThemeName||''};};
export class RiceQmlWidgetAdapter {
    constructor(host){this.host=host;this.surfaces=new Map();this.draining=new Set();this.generation=0;this.backdrop=null;this.backdropKey='';this.closed=false;this.listen=event=>this.message(event);globalThis.window?.addEventListener('message',this.listen);
        this.flushLifecycle=()=>{
            for(const surface of this.surfaces.values())if(globalThis.document?.hidden||surface.pendingConfig)void this.flushConfig(surface).catch(error=>this.host.request('log',{id:surface.id,message:'Native configuration lifecycle flush failed: '+error.message}).catch(()=>{}));
            for(const surface of this.draining)void this.disposeSurface(surface).then(()=>this.releaseListeners());
        };
        globalThis.document?.addEventListener('visibilitychange',this.flushLifecycle);
    }
    update(state){
        const ids=new Set((state?.instances||[]).map(i=>i.id));
        for(const [id,surface] of this.surfaces){
            if(!ids.has(id)){this.disposeSurface(surface);this.surfaces.delete(id);continue;}
            const instance=state.instances.find(i=>i.id===id);
            this.syncSurfacePreferences(surface,instance.surface);
            if(surface.active!==instance.visible){
                surface.active=instance.visible;this.send(surface,{type:'active',active:instance.visible});if(!surface.active)void this.flushConfig(surface).catch(error=>this.host.request('log',{id:surface.id,message:'Native configuration flush failed: '+error.message}).catch(()=>{}));
                if(surface.wantsCatalog){if(instance.visible)void this.subscribeCatalog(surface);else void this.host.request('kicker-catalog-unsubscribe',{instance:id}).catch(()=>{});}
                if(surface.wantsTasks){if(instance.visible)void this.subscribeTasks(surface);else void this.host.request('desktop-model-unsubscribe',{instance:id}).catch(()=>{});}
            }
            const catalog=state.applicationCatalogs?.[id];if(catalog&&surface.active&&surface.wantsCatalog)this.deliverCatalog(surface,catalog);
            const model=state.desktopModels?.[id];if(model&&surface.active&&surface.wantsTasks)this.deliverTasks(surface,model);
        }
    }
    syncSurfacePreferences(surface,value){
        surface.preferences=widgetSurfacePreferences(value);
        if(!surface.loaded)return;
        const key=JSON.stringify(surface.preferences);
        if(key!==surface.surfaceKey){surface.surfaceKey=key;this.send(surface,{type:'surface-preferences',surface:surface.preferences});}
    }
    deliverCatalog(surface,catalog){
        const key=catalog.revision??catalog.error;if(surface.catalogKey===key)return;surface.catalogKey=key;
        this.send(surface,{type:'catalog-state',...catalog});
    }
    async subscribeCatalog(surface){
        if(!surface.active)return;
        try{const catalog=await this.host.request('kicker-catalog-subscribe',{instance:surface.id});if(this.surfaces.get(surface.id)===surface&&surface.active)this.deliverCatalog(surface,catalog);}
        catch(error){this.send(surface,{type:'catalog-state',error:error.message});}
    }
    deliverTasks(surface,model){
        const key=model.epoch+':'+(model.event?.revision??model.event?.message);
        if(surface.tasksKey===key)return;surface.tasksKey=key;
        this.send(surface,{type:'desktop-model',...model});
    }
    async subscribeTasks(surface){
        if(!surface.active)return;
        try{const model=await this.host.request('desktop-model-subscribe',{instance:surface.id});if(this.surfaces.get(surface.id)===surface&&surface.active)this.deliverTasks(surface,model);}
        catch(error){this.send(surface,{type:'desktop-model',event:{kind:'error',message:error.message}});}
    }
    send(surface,message){if(surface.ready)surface.frame.contentWindow.postMessage({channel:'rice-qt-host',...message},location.origin);}
    startupProgress(surface,phase){
        clearTimeout(surface.startupTimer);
        surface.startupPhase=phase;surface.startupNote='';
        const started=performance.now();
        void this.host.request('log',{id:surface.id,message:'Startup: '+phase}).catch(()=>{});
        if(phase==='engine initialized')return;
        surface.startupTimer=setTimeout(()=>{
            if(this.surfaces.get(surface.id)!==surface||surface.loaded||surface.error)return;
            surface.startupNote='Still starting · See widget logs';
            void this.host.request('log',{id:surface.id,message:'Startup is still waiting at '+phase+' after '+Math.round((performance.now()-started)/1000)+' seconds; the engine has not been restarted.'}).catch(()=>{});
            this.host.invalidate();
        },45000);
    }
    async message(event){
        if(event.origin!==location.origin||event.data?.channel!=='rice-qt-widget')return;
        const surface=[...this.surfaces.values(),...this.draining].find(s=>s.frame.contentWindow===event.source);if(!surface)return;
        const {type,value}=event.data;
        if(['log','error','status','loaded'].includes(type))void this.host.request('log',{id:surface.id,message:type+': '+String(value??'')}).catch(()=>{});
        if(type==='frame-ready'){
            surface.ready=true;
            this.startupProgress(surface,'reading package files');
            try{const [packageResult,configResult,identityResult,settingsResult]=await Promise.allSettled([this.host.request('files',{package:surface.package.id}),this.host.request('config-read',{instance:surface.id}),this.host.request('identity',{id:surface.id}),this.host.request('kde-settings-capabilities',{instance:surface.id})]);if(configResult.status==='fulfilled')surface.config=configResult.value;if(packageResult.status==='rejected')throw packageResult.reason;if(configResult.status==='rejected')throw configResult.reason;if(identityResult.status==='rejected')throw identityResult.reason;const files=packageResult.value,config=configResult.value,settingsCapabilities=settingsResult.status==='fulfilled'?settingsResult.value:[];if(settingsResult.status==='rejected')void this.host.request('log',{id:surface.id,message:'Settings capabilities unavailable: '+settingsResult.reason.message}).catch(()=>{});if(this.surfaces.get(surface.id)!==surface){await this.host.request('config-close',{lease:config.lease});return;}this.startupProgress(surface,'starting Qt');this.send(surface,{type:'start',files,configFiles:config.files,entry:surface.package.entry,metadata:{...surface.package.metadata,id:surface.package.id,name:surface.package.name,appletId:identityResult.value},requiresPackageIdentity:Boolean(surface.package.rootPackage),settings:surface.settings,settingsCapabilities,surface:surface.preferences,palette:surface.themePalette,width:surface.rect.width,height:surface.rect.height});}
            catch(error){surface.error=error.message;this.host.invalidate();}
        }else if(type==='config-ready'){surface.configSupported=value===true;
        }else if(type==='config-write'){
            let snapshot;try{snapshot=JSON.parse(value);}catch{return;}
            if(!Number.isSafeInteger(snapshot.request))return;surface.pendingConfig=snapshot;
            const pending=surface.configFlushes?.get(snapshot.request);
            surface.configWrites=(surface.configWrites||Promise.resolve()).catch(()=>{}).then(async()=>{
                if(!surface.config?.lease)throw Error('Widget config was not restored');
                const result=await this.host.request('config-write',{lease:surface.config.lease,revision:surface.config.revision,files:snapshot.files});
                surface.config.revision=result.revision;if(surface.pendingConfig===snapshot)surface.pendingConfig=null;
                this.send(surface,{type:'config-ack',request:snapshot.request,revision:result.revision});
            });
            try{await surface.configWrites;pending?.resolve();}
            catch(error){pending?.reject(error);this.send(surface,{type:'config-ack',request:snapshot.request,error:error.message});void this.host.request('log',{id:surface.id,message:'Native configuration save failed: '+error.message}).catch(()=>{});}
            finally{if(pending){clearTimeout(pending.timer);surface.configFlushes.delete(snapshot.request);}}
        }else if(type==='config-error'){
            const error=JSON.parse(value),pending=surface.configFlushes?.get(error.request);
            if(pending){clearTimeout(pending.timer);surface.configFlushes.delete(error.request);pending.reject(Error(error.message));}
            void this.host.request('log',{id:surface.id,message:'Native configuration capture failed: '+error.message}).catch(()=>{});
        }else if(type==='internal-action'){
            try{
                const action=JSON.parse(value);
                if(action.name!=='configure')throw Error('Unsupported widget action: '+String(action.name));
                await this.host.request('preferences',{id:surface.id});
            }catch(error){void this.host.request('log',{id:surface.id,message:'Widget action failed: '+error.message}).catch(()=>{});}
        }else if(type==='surface'){
            const bounds=JSON.parse(value);
            if(['x','y','width','height','popupX','popupY','popupWidth','popupHeight'].some(key=>!Number.isFinite(bounds[key]))||bounds.width<=0||bounds.height<=0||bounds.width>32768||bounds.height>32768)throw Error('Invalid widget surface bounds');
            surface.bounds=bounds;surface.frame.style.width=bounds.width+'px';surface.frame.style.height=bounds.height+'px';this.host.invalidate();
        }else if(type==='catalog'){
            const request=JSON.parse(value);
            if(request.kind==='subscribe'){surface.wantsCatalog=true;await this.subscribeCatalog(surface);}
            else if(request.kind==='launch'){
                let error='';try{await this.host.request('kicker-catalog-launch',{...request,instance:surface.id});}catch(failure){error=failure.message;}
                if(this.surfaces.get(surface.id)===surface)this.send(surface,{type:'catalog-ack',request:request.request,error});
            }
        }else if(type==='sensors'){
            let response;
            try{response=await this.host.request('sensors-state',{instance:surface.id});}
            catch(error){response={error:error.message};}
            if(this.surfaces.get(surface.id)===surface)this.send(surface,{type:'sensors-state',...response});
        }else if(type==='process'){
            const request=JSON.parse(value);
            if(!Number.isSafeInteger(request.request)||request.request<1)return;
            if(request.kind==='cancel'){
                await this.host.request('process-cancel',{instance:surface.id,request:request.request}).catch(()=>{});
            }else if(request.kind==='execute'){
                let result,error;
                try{result=await this.host.request('process-execute',{instance:surface.id,request:request.request,command:request.command});}
                catch(failure){error=failure.message;void this.host.request('log',{id:surface.id,message:'Command execution failed: '+error}).catch(()=>{});}
                if(this.surfaces.get(surface.id)===surface)this.send(surface,{type:'process-result',request:request.request,result,error});
            }
        }else if(type==='settings'){
            const request=JSON.parse(value);let result,error;
            try{
                if(!Number.isSafeInteger(request.request)||request.request<1)throw Error('Invalid settings request');
                if(request.kind==='open')result=await this.host.request('kde-settings-open',{instance:surface.id,module:request.module,arguments:request.arguments});
                else if(request.kind==='capabilities')result=await this.host.request('kde-settings-capabilities',{instance:surface.id,modules:request.modules});
                else throw Error('Unknown settings request');
            }catch(failure){error=failure.message;void this.host.request('log',{id:surface.id,message:'Settings request failed: '+error}).catch(()=>{});}
            if(this.surfaces.get(surface.id)===surface)this.send(surface,{type:'settings-result',kind:request.kind,request:request.request,result,error});
        }else if(type==='tasks'){
            const request=JSON.parse(value);
            if(request.kind==='subscribe'){surface.wantsTasks=true;await this.subscribeTasks(surface);}
            else if(request.kind==='unsubscribe'){surface.wantsTasks=false;await this.host.request('desktop-model-unsubscribe',{instance:surface.id});}
            else if(request.kind==='activate'){
                let error='';try{await this.host.request('desktop-model-action',{instance:surface.id,operation:'activate',id:request.id});}catch(failure){error=failure.message;}
                this.send(surface,{type:'desktop-model-ack',request:request.request,error});
            }
        }else if(type==='startup'&&['loading Qt runtime','loading package QML'].includes(value)){this.startupProgress(surface,value);}
        else if(type==='loaded'){surface.loaded=true;this.startupProgress(surface,'engine initialized');surface.geometry='';surface.palette='';this.send(surface,{type:'active',active:surface.active});if(this.backdrop)this.send(surface,{type:'backdrop',bytes:this.backdrop,generation:this.generation});this.host.invalidate();}
        else if(type==='size'){const size=JSON.parse(value),instance=this.host.state?.instances.find(i=>i.id===surface.id);if(instance?.autoSize&&size.width>=48&&size.height>=48)try{await this.host.request('measure',{id:surface.id,rect:{...instance.rect,width:Math.ceil(size.width),height:Math.ceil(size.height)}});}catch(error){surface.error=error.message;}}
        else if(type==='frame')this.host.invalidate(surface.rect,surface.id);
        else if(type==='calendar'){
            const request=JSON.parse(value);
            if(!surface.active)return;
            if(!Number.isSafeInteger(request.instance)||!Number.isSafeInteger(request.request))return;
            let response;
            try{response={events:await this.host.request('calendar-state',{instance:surface.id,start:request.start,end:request.end})};}
            catch(error){response={error:error.message};}
            if(this.surfaces.get(surface.id)===surface&&surface.active)this.send(surface,{type:'calendar-state',instance:request.instance,request:request.request,...response});
        }
        else if(type==='media'){
            const request=JSON.parse(value);
            if(!surface.active)return;
            if(request.kind==='state'&&surface.mediaPending)return;
            if(request.kind==='state')surface.mediaPending=true;
            try{
                if(request.kind==='control')await this.host.request('media-control',{instance:surface.id,session:request.session,command:request.command,value:request.value});
                else if(request.kind!=='state')throw Error('Unknown media request');
                const players=await this.host.request('media-state',{instance:surface.id});
                if(this.surfaces.get(surface.id)===surface)this.send(surface,{type:'media-state',request:request.request,players});
            }catch(error){
                const message='Media request: '+error.message;
                if(surface.mediaError!==message){surface.mediaError=message;void this.host.request('log',{id:surface.id,message}).catch(()=>{});}
            }finally{if(request.kind==='state')surface.mediaPending=false;}
        }
        else if(type==='actions'){
            const result=JSON.parse(value),pending=surface.actionRequests?.get(result.request);
            if(pending){clearTimeout(pending.timer);surface.actionRequests.delete(result.request);pending.resolve(result.actions);}
        }
        else if(type==='error'||type==='status'&&value==='error'){surface.error=surface.error||String(value==='error'?'This widget could not load. Open its settings for compatibility details.':value||'Widget could not load');this.host.invalidate();}
        else if(type==='network'){const request=JSON.parse(value);try{const result=await this.host.request('fetch',{...request,instance:surface.id});this.send(surface,{type:'network-response',id:request.id,...result});}catch(error){this.send(surface,{type:'network-response',id:request.id,error:error.message});}}
        else if(type==='configuration'){try{await this.host.request('update',{id:surface.id,settings:JSON.parse(value)});}catch(error){surface.error=error.message;}}
        // Upstream services also log recoverable diagnostics containing
        // "Error:". Loader/construction failures arrive through status/error;
        // only explicit engine or Wasm fatal messages can blank a loaded view.
        else if(type==='log'&&/QQmlApplicationEngine failed to load component|(?:WebAssembly\.)?RuntimeError: (?:memory access out of bounds|unreachable)|Aborted\(/.test(value)){surface.error=String(value).slice(0,240);this.host.invalidate();}
    }
    prepare(context,viewport,state){
        this.update(state);
        if(!state.instances.some(i=>i.visible&&state.packages.some(p=>p.id===i.package&&p.schema==='plasma.applet/v1')))return;
        const now=performance.now();if(this.capturing||now<(this.nextBackdrop||0))return;this.nextBackdrop=now+500;
        const document=context.canvas.ownerDocument||globalThis.document;if(!document)return;
        this.sample ||= document.createElement('canvas');this.sample.width=64;this.sample.height=32;
        const sample=this.sample.getContext('2d',{willReadFrequently:true});sample.drawImage(context.canvas,0,0,64,32);
        const pixels=sample.getImageData(0,0,64,32).data;let hash=2166136261;for(const value of pixels)hash=Math.imul(hash^value,16777619);
        const key=viewport.width+':'+viewport.height+':'+hash;if(key===this.backdropKey)return;
        this.backdropKey=key;const capture=document.createElement('canvas');capture.width=viewport.width;capture.height=viewport.height;capture.getContext('2d').drawImage(context.canvas,0,0);
        this.capturing=true;capture.toBlob(async blob=>{try{if(!blob||this.closed)return;this.backdrop=new Uint8Array(await blob.arrayBuffer());this.generation++;for(const surface of this.surfaces.values())if(surface.loaded)this.send(surface,{type:'backdrop',bytes:this.backdrop,generation:this.generation});}finally{this.capturing=false;}},'image/png');
    }
    paint(context,rect,pkg,settings,{instance,state,viewport}){
        if(!this.host.request)return;
        let surface=this.surfaces.get(instance.id);
        const values={...Object.fromEntries(pkg.fields.map(f=>[f.name,f.default])),...settings};
        if(!surface){
            const frame=document.createElement('iframe');frame.title=pkg.name+' widget runtime';frame.tabIndex=-1;frame.setAttribute('aria-hidden','true');frame.style.cssText='position:fixed;left:-20000px;top:0;border:0;pointer-events:none;';frame.src=new URL('./rice-qml-widget-frame.html?v=2',import.meta.url).href;
            surface={id:instance.id,package:pkg,frame,rect,settings:values,active:true};this.surfaces.set(instance.id,surface);this.startupProgress(surface,'loading widget frame');frame.style.width=rect.width+'px';frame.style.height=rect.height+'px';document.body.append(frame);
        }
        surface.rect=rect;
        this.syncSurfacePreferences(surface,instance.surface);
        surface.themePalette=widgetPalette(state);
        if(surface.loaded){
            const geometry=JSON.stringify([rect,viewport]);if(geometry!==surface.geometry){surface.geometry=geometry;if(!surface.bounds?.popup){surface.frame.style.width=rect.width+'px';surface.frame.style.height=rect.height+'px';}this.send(surface,{type:'geometry',...rect,desktopWidth:viewport.width,desktopHeight:viewport.height});}
            const serialized=JSON.stringify(values);if(serialized!==surface.configuration){surface.configuration=serialized;this.send(surface,{type:'configure',settings:values});}
            const palette=surface.themePalette;const paletteKey=JSON.stringify(palette);if(paletteKey!==surface.palette){surface.palette=paletteKey;this.send(surface,{type:'palette',palette});}
            const canvas=surface.frame.contentWindow?.riceWidgetFrame?.canvas;
            if(canvas){const bounds=surface.bounds||{x:0,y:0,width:rect.width,height:rect.height},sx=canvas.width/bounds.width,sy=canvas.height/bounds.height;
                context.drawImage(canvas,-bounds.x*sx,-bounds.y*sy,rect.width*sx,rect.height*sy,rect.x,rect.y,rect.width,rect.height);}

        }
        if(surface.reportedError!==surface.error){surface.reportedError=surface.error;void this.host.request('status',{id:surface.id,error:surface.error||null}).catch(()=>{});}
        if(surface.error||!surface.loaded){context.fillStyle=cssColor(state.appearance?.background,'#232629');context.fillRect(rect.x,rect.y,rect.width,rect.height);context.fillStyle=cssColor(state.appearance?.text,'#ffffff');context.font='13px system-ui';context.fillText(surface.error||surface.startupNote||'Starting '+pkg.name+'…',rect.x+12,rect.y+28,rect.width-24);}
    }
    popupRect(surface){const b=surface.bounds;return b?.popup?{x:surface.rect.x+b.popupX,y:surface.rect.y+b.popupY,width:b.popupWidth,height:b.popupHeight}:null;}
    popupHit(x,y){
        for(const instance of [...(this.host.state?.instances||[])].reverse()){
            const surface=this.surfaces.get(instance.id),r=surface&&this.popupRect(surface);
            if(instance.visible&&surface?.active&&r&&x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height)return instance;
        }
        return null;
    }
    paintOverlays(context){
        if(this.host.state?.editing)return;
        for(const surface of this.surfaces.values()){
            if(!surface.active||!surface.loaded||surface.error)continue;
            const r=this.popupRect(surface),b=surface.bounds,canvas=surface.frame.contentWindow?.riceWidgetFrame?.canvas;
            if(!r||!canvas)continue;
            const sx=canvas.width/b.width,sy=canvas.height/b.height;
            context.drawImage(canvas,(b.popupX-b.x)*sx,(b.popupY-b.y)*sy,b.popupWidth*sx,b.popupHeight*sy,r.x,r.y,r.width,r.height);
        }
    }
    deactivateOutside(x,y,force=false){
        const contains=r=>r&&x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height;
        for(const surface of this.surfaces.values())if(surface.active&&surface.bounds?.popup&&(force||!contains(surface.rect)&&!contains(this.popupRect(surface))))this.send(surface,{type:'deactivate'});
    }
    pointer(instance,kind,x,y,button,buttons){const s=this.surfaces.get(instance.id);if(!s?.loaded)return false;this.send(s,{type:'pointer',kind,x:x-instance.rect.x,y:y-instance.rect.y,button:button===0?1:button===2?2:button===1?4:0,buttons:buttons&1|(buttons&2)|(buttons&4)});return true;}
    actions(instance){
        const surface=this.surfaces.get(instance.id);if(!surface?.loaded||surface.error)return Promise.resolve([]);
        surface.actionRequests ||= new Map();const request=(surface.actionSequence||0)+1;surface.actionSequence=request;
        return new Promise(resolve=>{const timer=setTimeout(()=>{surface.actionRequests.delete(request);resolve([]);},2000);surface.actionRequests.set(request,{resolve,timer});this.send(surface,{type:'actions',request});});
    }
    triggerAction(instance,id){const surface=this.surfaces.get(instance.id);if(surface?.loaded&&surface.active)this.send(surface,{type:'action',id});}
    key(instance,event){const s=this.surfaces.get(instance.id);if(!s?.loaded)return false;const special={Escape:0x1000000,Tab:0x1000001,Backspace:0x1000003,Enter:0x1000004,Delete:0x1000007,Home:0x1000010,End:0x1000011,ArrowLeft:0x1000012,ArrowUp:0x1000013,ArrowRight:0x1000014,ArrowDown:0x1000015};const key=special[event.key]||(event.key.length===1?event.key.toUpperCase().codePointAt(0):0);if(!key)return false;this.send(s,{type:'key',pressed:event.type!=='keyup',key,text:event.key.length===1?event.key:'',modifiers:(event.shiftKey?0x2000000:0)|(event.ctrlKey?0x4000000:0)|(event.altKey?0x8000000:0)|(event.metaKey?0x10000000:0)});return true;}
    flushConfig(surface){
        if(!surface.loaded||!surface.configSupported||!surface.config)return surface.configWrites||Promise.resolve();
        surface.configFlushes ||= new Map();const request=surface.configFlushSequence=(surface.configFlushSequence||0)+1;
        return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{surface.configFlushes.delete(request);reject(Error('Native configuration flush timed out'));},10000);surface.configFlushes.set(request,{resolve,reject,timer});this.send(surface,{type:'config-flush',request});});
    }
    disposeSurface(surface){
        if(surface.disposal)return surface.disposal;
        this.draining.add(surface);surface.active=false;this.send(surface,{type:'active',active:false});
        if(surface.wantsCatalog)void this.host.request('kicker-catalog-unsubscribe',{instance:surface.id}).catch(()=>{});
        if(surface.wantsTasks)void this.host.request('desktop-model-unsubscribe',{instance:surface.id}).catch(()=>{});
        clearTimeout(surface.startupTimer);for(const pending of surface.actionRequests?.values()||[]){clearTimeout(pending.timer);pending.resolve([]);}surface.actionRequests?.clear();
        surface.disposal=this.flushConfig(surface).then(async()=>{
            await surface.configWrites;
            if(surface.config?.lease)await this.host.request('config-close',{lease:surface.config.lease}).catch(()=>{});
            surface.frame.remove();this.draining.delete(surface);
        }).catch(error=>{surface.disposal=null;void this.host.request('log',{id:surface.id,message:'Native configuration shutdown failed; retained for retry: '+error.message}).catch(()=>{});});return surface.disposal;
    }
    releaseListeners(){if(this.closed&&!this.draining.size){globalThis.window?.removeEventListener('message',this.listen);globalThis.document?.removeEventListener('visibilitychange',this.flushLifecycle);}}
    close(){this.closed=true;const pending=[...new Set([...this.surfaces.values(),...this.draining])].map(s=>this.disposeSurface(s));this.surfaces.clear();this.backdrop=null;return Promise.all(pending).finally(()=>this.releaseListeners());}
}
