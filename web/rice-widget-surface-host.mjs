import {t} from './rice-i18n.mjs';
import {riceLocale} from './rice-i18n.mjs';
import {RiceGlavaWidgetAdapter} from './rice-glava-widget-adapter.mjs';
import {RiceQmlWidgetAdapter} from './rice-qml-widget-adapter.mjs';
import {widgetRect} from './rice-widget-controller.mjs';
const themeColor=(value,fallback)=>Number.isInteger(value)?'#'+(value&0xffffff).toString(16).padStart(6,'0'):fallback;

/** Compositor-owned desktop plane. Adapters render inside bounded instance rectangles. */
export class RiceWidgetSurfaceHost {
    constructor(renderer) {
        this.renderer=renderer;this.state=null;this.adapters=new Map();this.drag=null;this.audio=null;this.timer=null;this.disposed=false;
        this.adapters.set('rice.widget/v1',{paint:paintRiceWidget});
        this.adapters.set('plasma.applet/v1',new RiceQmlWidgetAdapter(this));
        this.adapters.set('glava.visualizer/v1',new RiceGlavaWidgetAdapter(this));
    }
    register(schema,adapter){if(this.adapters.has(schema)||typeof adapter?.paint!=='function')throw Error('Invalid widget adapter');this.adapters.set(schema,adapter);return()=>{this.adapters.delete(schema);adapter.close?.();};}
    clearHover(){if(this.hovered){const item=this.hovered,pkg=this.state?.packages.find(p=>p.id===item.package);this.adapters.get(pkg?.schema)?.pointer?.(item,3,0,0,-1,0);this.hovered=null;}}
    reconcileHover(){
        if(!this.hovered||!this.hoverPoint)return;
        const previous=this.hovered,point=this.hoverPoint;
        const current=!this.state?.editing&&!this.renderer.shellOverlayActive&&!this.blockedAt(point)?this.hit(point.x,point.y):null;
        if(current?.id!==previous.id){this.clearHover();return;}
        this.hovered=current;
        if(['x','y','width','height'].some(key=>current.rect[key]!==previous.rect[key])){
            const pkg=this.state.packages.find(p=>p.id===current.package);
            this.adapters.get(pkg?.schema)?.pointer?.(current,0,point.x,point.y,-1,0);
        }
    }
    update(state){if(state?.editing||!state?.instances.some(i=>i.id===this.hovered?.id&&i.visible))this.clearHover();this.state=state;this.reconcileHover();if(this.menuInstance&&!state?.instances.some(i=>i.id===this.menuInstance&&i.visible))this.closeMenu?.();this.updateProducers();if(this.drag&&!state?.editing)this.drag=null;this.syncToolbar?.();}
    presentationState(){const p=this.presentation;return !p?this.state:this.state&&{...this.state,instances:this.state.instances.map(i=>({...i,visible:i.visible&&p.visible.has(i.id),surface:p.surfacePreferences?.(i)??i.surface}))};}
    updateProducers(){const state=this.presentationState();for(const adapter of this.adapters.values())adapter.update?.(state);this.arm();this.onProducerVisibilityChange?.(state);}
    acquirePresentation(onDamage,surfacePreferences){
        if(this.disposed)throw Error('Widget host is closed');
        if(this.presentation)throw Error('Widget presentation already owned');
        if(this.interacting){const item=this.interacting,pkg=this.state?.packages.find(p=>p.id===item.package);this.adapters.get(pkg?.schema)?.pointer?.(item,2,item.rect.x-1,item.rect.y-1,0,0);}
        if(this.drag)this.renderer.canvas?.releasePointerCapture?.(this.drag.pointer);
        this.clearHover();this.closeMenu?.();this.drag=null;this.interacting=null;this.focused=null;
        for(const adapter of this.adapters.values())adapter.deactivateOutside?.(0,0,true);
        const lease={visible:new Set(),onDamage,surfacePreferences,closed:false};this.presentation=lease;this.updateProducers();this.syncToolbar?.();
        return {setVisible:ids=>{if(lease.closed)return;const next=new Set(ids);if(next.size===lease.visible.size&&[...next].every(id=>lease.visible.has(id)))return;lease.visible=next;this.updateProducers();},
            release:()=>{if(lease.closed)return;lease.closed=true;if(this.presentation===lease){this.presentation=null;this.updateProducers();this.syncToolbar?.();this.invalidate();}}};
    }
    arm(){
        const visible=this.presentationState()?.instances?.filter(i=>i.visible)||[];
        if(!visible.length||this.disposed){clearTimeout(this.timer);this.timer=null;this.audio?.stopAnalysis?.();return;}
        const packages=this.state.packages||[];
        const nodes=visible.flatMap(i=>packages.find(p=>p.id===i.package)?.nodes||[]);
        if(!nodes.some(n=>n.type==='spectrum'))this.audio?.stopAnalysis?.();
        const delay=nodes.some(n=>n.type==='spectrum')&&this.audio?50:nodes.some(n=>n.type==='clock')?1000:null;
        if(this.timer && delay===this.timerDelay)return;
        clearTimeout(this.timer);this.timer=null;this.timerDelay=delay;
        if(delay!==null)this.timer=setTimeout(()=>{this.timer=null;this.invalidate();this.arm();},delay);
    }
    invalidate(rect,id){if(!this.disposed){
        if(this.presentation){this.presentation.onDamage?.(id);return;}
        if(rect&&this.renderer._markNativeDamage)this.renderer._markNativeDamage([{...rect}]);
        else this.renderer.damaged=true;
        this.renderer.schedule();
    }}
    setAudio(source){if(this.audio===source)return;this.audio?.stopAnalysis?.();this.audio=source;for(const adapter of this.adapters.values())adapter.audioChanged?.();this.arm();}
    paint(context,viewport){
        const state=this.state;if(!state||this.presentation)return;
        if(this.hovered&&(this.renderer.shellOverlayActive||this.hoverPoint&&this.blockedAt(this.hoverPoint)))this.clearHover();
        for(const adapter of this.adapters.values())adapter.prepare?.(context,viewport,state);
        for(const instance of state.instances||[]){
            if(!instance.visible)continue;
            const p=state.packages.find(p=>p.id===instance.package);const adapter=this.adapters.get(p?.schema);if(!adapter)continue;
            const rect=widgetRect(this.drag?.id===instance.id?this.drag.rect:instance.rect,viewport);
            context.save();
            context.beginPath();context.rect(rect.x,rect.y,rect.width,rect.height);context.clip();
            this.renderer.customEffects?.render(context,{...viewport,target:'widget',rects:[{...rect,id:instance.id}],reducedMotion:this.renderer.nativeCanvasWindow.snapshot?.shell?.motionEnabled===false});
            try{adapter.paint(context,rect,p,instance.settings,{audio:this.audio,now:new Date(),instance,state,viewport});}
            catch(error){context.fillStyle='#ffcccc';context.font='13px sans-serif';context.fillText(t('Widget could not render'),rect.x+8,rect.y+24);}
            context.restore();
            if(state.editing){context.save();context.strokeStyle=themeColor(state.selected===instance.id?state.appearance?.accent:state.appearance?.line,'#8c99ab');context.lineWidth=2;context.strokeRect(rect.x+1,rect.y+1,rect.width-2,rect.height-2);context.fillStyle=themeColor(state.appearance?.accent,'#8c99ab');context.fillRect(rect.x+rect.width-12,rect.y+rect.height-12,12,12);context.restore();}
        }
        for(const adapter of this.adapters.values())adapter.paintOverlays?.(context,viewport,state);
    }
    hit(x,y){if(!this.state?.editing)for(const adapter of this.adapters.values()){const item=adapter.popupHit?.(x,y);if(item)return item;}return [...(this.state?.instances||[])].reverse().find(i=>i.visible&&x>=i.rect.x&&y>=i.rect.y&&x<i.rect.x+i.rect.width&&y<i.rect.y+i.rect.height)||null;}
    blockedAt({x,y}){
        const contains=r=>r&&x>=r.x&&y>=r.y&&x<r.x+r.width&&y<r.y+r.height;
        return (this.renderer.nativeCanvasWindow?.snapshots||[]).some(w=>w.role!=='desktop'&&w.layerVisible!==false&&contains(w.frame))||
            [...this.renderer.windows.values()].some(w=>w.viewable&&w.parent!==w.id&&!w.desktopLayer&&contains(w));
    }
    attachInput({canvas=this.renderer.canvas,request,onError=()=>{},window_=globalThis.window}={}){
        if(!window_||this.detachInput)return;
        const point=e=>{const r=canvas.getBoundingClientRect();return {x:(e.clientX-r.left)*canvas.width/r.width,y:(e.clientY-r.top)*canvas.height/r.height};};
        this.request=request;
        const send=(action,data)=>Promise.resolve(request(action,data)).catch(onError);
        const stop=e=>{e.preventDefault();e.stopImmediatePropagation();};
        const toolbar=window_.document.createElement('div');toolbar.setAttribute('role','toolbar');toolbar.setAttribute('aria-label','Edit desktop widgets');
        toolbar.style.cssText='position:fixed;left:50%;top:68px;transform:translateX(-50%);z-index:90000;max-width:90vw;padding:10px 14px;border:1px solid var(--widget-edge);border-radius:var(--widget-radius);background:var(--widget-background);color:var(--widget-text);font:13px var(--widget-font);box-shadow:0 8px 30px #0006;';
        const hint=window_.document.createElement('span');hint.textContent='Drag to move · Corner to resize · Arrows for precision ';
        const done=window_.document.createElement('button');done.textContent='Done';done.style.cssText='margin-left:12px;padding:7px 14px;border-radius:var(--widget-radius);border:1px solid var(--widget-edge);background:var(--widget-selected);color:var(--widget-text);font:inherit;cursor:pointer';done.onclick=()=>void send('edit',{enabled:false});
        toolbar.append(hint,done);window_.document.body.append(toolbar);
        this.syncToolbar=()=>{
            toolbar.hidden=Boolean(this.presentation)||!this.state?.editing;
            const appearance=this.state?.appearance||{};
            for(const [key,value] of Object.entries({edge:themeColor(appearance.accent,'#687386'),background:themeColor(appearance.background,'#20242b'),text:themeColor(appearance.text,'#ffffff'),selected:themeColor(appearance.selected,'#394a63'),radius:Math.max(0,Math.min(24,appearance.radius??8))+'px',font:appearance.font==='monospace'?'ui-monospace,monospace':'system-ui'}))toolbar.style.setProperty('--widget-'+key,value);
        };this.syncToolbar();
        const adapterFor=i=>this.adapters.get(this.state.packages.find(p=>p.id===i?.package)?.schema);
        const blockedAt=at=>this.blockedAt(at);
        let menu=null,menuGeneration=0;
        this.closeMenu=()=>{menuGeneration++;menu?.remove();menu=null;this.menuInstance=null;};
        const openMenu=async(item,x,y)=>{
            this.closeMenu();this.menuInstance=item.id;const generation=menuGeneration;
            const adapter=adapterFor(item),actions=await (adapter?.actions?.(item)||[]);
            if(generation!==menuGeneration||!this.state?.instances.some(i=>i.id===item.id&&i.visible))return;
            const appearance=this.state.appearance||{},doc=window_.document;
            menu=doc.createElement('div');menu.setAttribute('role','menu');menu.setAttribute('aria-label','Widget actions');
            menu.style.cssText=`position:fixed;z-index:90001;min-width:180px;max-width:min(320px,95vw);max-height:80vh;overflow:auto;padding:6px;border:1px solid ${themeColor(appearance.line,'#596273')};border-radius:${Math.max(0,Math.min(24,appearance.radius??8))}px;background:${themeColor(appearance.background,'#20242b')};color:${themeColor(appearance.text,'#ffffff')};font:14px system-ui;box-shadow:0 8px 30px #0006;`;
            const append=(text,activate,action={enabled:true})=>{
                const button=doc.createElement('button');button.type='button';button.textContent=text;button.disabled=!action.enabled;
                button.setAttribute('role',action.checkable?'menuitemcheckbox':'menuitem');if(action.checkable)button.setAttribute('aria-checked',String(Boolean(action.checked)));
                button.style.cssText='display:block;width:100%;border:0;background:transparent;color:inherit;font:inherit;text-align:left;padding:9px 12px;cursor:pointer;overflow-wrap:anywhere;';
                if(action.checkable)button.textContent=(action.checked?'✓ ':'○ ')+text;
                if(button.disabled)button.style.opacity='.45';
                const highlight=()=>{button.style.background=themeColor(appearance.selected,'#394a63');};const clear=()=>{button.style.background='transparent';};
                button.onfocus=highlight;button.onblur=clear;button.onmouseenter=highlight;button.onmouseleave=clear;
                button.onclick=()=>{this.closeMenu();canvas.focus();activate();};menu.append(button);
            };
            const separator=()=>{const line=doc.createElement('hr');line.setAttribute('role','separator');line.style.cssText='border:0;border-top:1px solid currentColor;opacity:.25;margin:5px;';menu.append(line);};
            for(const action of actions.slice(0,128)){if(action.separator)separator();else append(action.text,()=>adapter.triggerAction(item,action.id),action);}
            if(actions.length)separator();
            append('Edit placement',()=>void send('edit',{enabled:true,id:item.id}));
            append('Widget settings',()=>void send('preferences',{id:item.id}));
            menu.onkeydown=e=>{const buttons=[...menu.querySelectorAll('button:not(:disabled)')],index=buttons.indexOf(doc.activeElement);if(e.key==='Escape'||e.key==='Tab'){this.closeMenu();canvas.focus();stop(e);}else if(['ArrowDown','ArrowUp','Home','End'].includes(e.key)){const next=e.key==='Home'?0:e.key==='End'?buttons.length-1:(index+(e.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length;buttons[next]?.focus();stop(e);}};
            doc.body.append(menu);const bounds=menu.getBoundingClientRect();menu.style.left=Math.max(4,Math.min(x,window_.innerWidth-bounds.width-4))+'px';menu.style.top=Math.max(4,Math.min(y,window_.innerHeight-bounds.height-4))+'px';menu.querySelector('button:not(:disabled)')?.focus();
        };
        const down=e=>{if(this.presentation)return;
            const outsidePoint=point(e),foreignTarget=e.target!==canvas&&e.target!==canvas.parentElement;
            for(const adapter of this.adapters.values())adapter.deactivateOutside?.(outsidePoint.x,outsidePoint.y,foreignTarget||this.renderer.shellOverlayActive||blockedAt(outsidePoint));

            if(!menu?.contains(e.target))this.closeMenu();
            this.focused=null;
            if((e.target!==canvas&&e.target!==canvas.parentElement)||!this.state||![0,2].includes(e.button)||this.renderer.shellOverlayActive)return;
            const at=point(e),item=this.hit(at.x,at.y);if(!item)return;
            // Desktop widgets never take input through an application or shell window.
            if(blockedAt(at))return;
            if(e.button===2){stop(e);void openMenu(item,e.clientX,e.clientY).catch(onError);return;}
            if(!this.state.editing){const pkg=this.state.packages.find(p=>p.id===item.package);const adapter=this.adapters.get(pkg?.schema);if(adapter?.pointer?.(item,1,at.x,at.y,e.button,e.buttons)){this.interacting=item;this.focused=item;canvas.focus();stop(e);}return;}
            stop(e);canvas.focus();canvas.setPointerCapture?.(e.pointerId);
            this.state.selected=item.id;void send('edit',{enabled:true,id:item.id});
            this.drag={id:item.id,pointer:e.pointerId,start:at,original:{...item.rect},rect:{...item.rect},resize:at.x>item.rect.x+item.rect.width-20&&at.y>item.rect.y+item.rect.height-20};this.invalidate();
        };
        const move=e=>{if(this.presentation)return;
            if(!this.state?.editing&&!this.interacting){
                const at=point(e),item=!this.renderer.shellOverlayActive&&!blockedAt(at)&&(e.target===canvas||e.target===canvas.parentElement)?this.hit(at.x,at.y):null;
                if(this.hovered&&this.hovered.id!==item?.id)this.clearHover();
                this.hoverPoint=at;
                this.hovered=item;
                if(item)adapterFor(item)?.pointer?.(item,0,at.x,at.y,-1,0);
            }
            if(this.interacting){const p=point(e);adapterFor(this.interacting)?.pointer(this.interacting,0,p.x,p.y,e.button,e.buttons);stop(e);return;}if(!this.drag||e.pointerId!==this.drag.pointer)return;stop(e);const p=point(e),d=this.drag,dx=p.x-d.start.x,dy=p.y-d.start.y;
            const snap=n=>e.altKey?n:Math.round(n/8)*8;
            d.rect=widgetRect(d.resize?{...d.original,width:snap(d.original.width+dx),height:snap(d.original.height+dy)}:{...d.original,x:snap(d.original.x+dx),y:snap(d.original.y+dy)},canvas);this.invalidate();};
        const up=e=>{if(this.interacting){const p=point(e);adapterFor(this.interacting)?.pointer(this.interacting,2,p.x,p.y,e.button,0);this.interacting=null;stop(e);return;}if(!this.drag||e.pointerId!==this.drag.pointer)return;stop(e);const d=this.drag;this.drag=null;canvas.releasePointerCapture?.(e.pointerId);if(e.type!=='pointercancel')void send('update',{id:d.id,rect:d.rect});this.invalidate();};
        const key=e=>{if(this.presentation)return;
            if(!this.state?.editing&&this.focused&&!this.renderer.shellOverlayActive&&(e.target===canvas||e.target===canvas.parentElement)){const i=this.state?.instances.find(i=>i.id===this.focused.id&&i.visible);if(i&&adapterFor(i)?.key?.(i,e))stop(e);return;}
            if(e.type==='keyup')return;
            if(!this.state?.editing||(e.target!==canvas&&e.target!==canvas.parentElement))return;
            if(e.key==='Escape'){stop(e);if(this.drag){this.drag=null;this.invalidate();}else void send('edit',{enabled:false});return;}
            if(e.key==='Enter'){stop(e);void send('edit',{enabled:false});return;}
            const i=this.state.instances.find(i=>i.id===this.state.selected&&i.visible);if(!i)return;
            const vectors={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]},v=vectors[e.key];if(!v)return;stop(e);
            const delta=e.shiftKey?16:1,r={...i.rect};if(e.altKey){r.width+=v[0]*delta;r.height+=v[1]*delta;}else{r.x+=v[0]*delta;r.y+=v[1]*delta;}
            void send('update',{id:i.id,rect:widgetRect(r,canvas)});
        };
        const context=e=>{const at=point(e);if((e.target===canvas||e.target===canvas.parentElement)&&!this.renderer.shellOverlayActive&&!blockedAt(at)&&this.hit(at.x,at.y))stop(e);};
        const leave=e=>{if(e.type==='blur')for(const adapter of this.adapters.values())adapter.deactivateOutside?.(0,0,true);if(e.type==='blur'||e.type==='pointercancel'||e.relatedTarget==null)this.clearHover();};
        const events=[['blur',leave],['pointerout',leave],['pointercancel',leave],['pointerdown',down],['pointermove',move],['pointerup',up],['pointercancel',up],['keydown',key],['keyup',key],['contextmenu',context]];
        for(const [name,fn] of events)window_.addEventListener(name,fn,true);
        this.detachInput=()=>{this.clearHover();this.closeMenu();toolbar.remove();this.syncToolbar=null;for(const [name,fn] of events)window_.removeEventListener(name,fn,true);};
    }
    close(){this.disposed=true;if(this.presentation)this.presentation.closed=true;this.presentation=null;this.onProducerVisibilityChange=null;clearTimeout(this.timer);this.detachInput?.();for(const a of this.adapters.values())a.close?.();this.audio?.stopAnalysis?.();this.audio=null;this.state=null;}
}
function paintRiceWidget(ctx,r,p,s={}, {audio,now}){
    ctx.globalAlpha=s.opacity??1;
    if(s.glass!==false){
        // Sample only the already-painted desktop plane, before window composition.
        ctx.save();ctx.beginPath();ctx.roundRect(r.x,r.y,r.width,r.height,16);ctx.clip();
        ctx.filter='blur(12px)';ctx.drawImage(ctx.canvas,r.x,r.y,r.width,r.height,r.x,r.y,r.width,r.height);ctx.filter='none';
        ctx.fillStyle='#15233370';ctx.fillRect(r.x,r.y,r.width,r.height);ctx.restore();
        ctx.strokeStyle='#ffffff55';ctx.lineWidth=1;ctx.beginPath();ctx.roundRect(r.x+.5,r.y+.5,r.width-1,r.height-1,16);ctx.stroke();
    }
    const h=(r.height-24)/p.nodes.length;
    p.nodes.forEach((node,index)=>{
        const y=r.y+12+index*h;
        ctx.fillStyle=node.color||s.color||'#ffffff';ctx.textAlign='center';ctx.textBaseline='middle';
        if(node.type==='text'){
            ctx.font=`${node.size||20}px sans-serif`;
            const lines=(s.text??node.text??'').split('\n').slice(0,16);lines.forEach((line,n)=>ctx.fillText(line,r.x+r.width/2,y+h/2+(n-(lines.length-1)/2)*24,r.width-24));
        } else if(node.type==='clock'){
            ctx.font=`500 ${Math.min(64,h*.48)}px sans-serif`;
            ctx.fillText(now.toLocaleTimeString(riceLocale(),{hour:'2-digit',minute:'2-digit',hour12:s.hour12??false}),r.x+r.width/2,y+h*.42,r.width-24);
            if(s.showDate!==false){ctx.font='14px sans-serif';ctx.fillText(now.toLocaleDateString(riceLocale(),{weekday:'long',month:'long',day:'numeric'}),r.x+r.width/2,y+h*.78,r.width-24);}
        }else if(node.type==='spectrum'){
            const data=audio?.sample?.();
            if(!data||!data.active){ctx.font='14px sans-serif';ctx.fillText(t('Waiting for Rice Player audio'),r.x+r.width/2,y+h/2,r.width-24);return;}
            const bins=data.frequency, count=Math.min(48,bins.length),w=(r.width-32)/count;
            if(s.style==='radial'){
                const base=Math.min(r.width,h)*.22;ctx.strokeStyle=s.color||'#8ee4ff';ctx.lineWidth=3;
                for(let i=0;i<count;i++){const angle=i/count*Math.PI*2,v=bins[i]/255*base;ctx.beginPath();ctx.moveTo(r.x+r.width/2+Math.cos(angle)*base,y+h/2+Math.sin(angle)*base);ctx.lineTo(r.x+r.width/2+Math.cos(angle)*(base+v),y+h/2+Math.sin(angle)*(base+v));ctx.stroke();}
            }else if(s.style==='wave'){
                ctx.strokeStyle=s.color||'#8ee4ff';ctx.lineWidth=2;ctx.beginPath();data.waveform.forEach((v,i)=>{const x=r.x+16+i/(data.waveform.length-1)*(r.width-32),yy=y+h/2+(v-128)/128*h*.4;i?ctx.lineTo(x,yy):ctx.moveTo(x,yy);});ctx.stroke();
            }else for(let i=0;i<count;i++){const bar=Math.max(1,bins[i]/255*(h-12));ctx.fillRect(r.x+16+i*w,y+h-bar,Math.max(1,w-3),bar);}
        }
    });
}
