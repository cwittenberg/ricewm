import {riceIconLabel} from "./rice-icon-labels.mjs";
import {t,riceLocale} from "./rice-i18n.mjs";
import {rgbaColor,readableInk} from "./rice-native-canvas-primitives.mjs";
import {parseTopbarLayout, TOPBAR_DEFAULTS, topbarRows, previewTopbarRows} from './rice-topbar-layout.mjs';
import {riceTopbarSymbolicIconName, RICE_ICON_CATALOG} from './rice-native-icon-renderer.mjs';
const contains=(r,p)=>r.width>0 && p.x>=r.x && p.x<r.x+r.width && p.y>=r.y && p.y<r.y+r.height;
const copy=value=>JSON.parse(JSON.stringify(value));
const builtinTitles=()=>({"@clock":t("shell.topbar.calendar"),"@screenshot":t("shell.topbar.screenshot"),"@search":t("shell.topbar.search"),"@mail":t("shell.topbar.mail"),"@network":t("shell.topbar.network"),"@audio":t("shell.topbar.sound"),"@battery":t("shell.topbar.battery"),"@session":t("shell.topbar.session"),"@applications":"Rice Hub"});

export class RiceTopbarController {
    constructor({renderer,activate=()=>{},save=async()=>{},saveTransparency=async()=>{},toggle,toggleMenu}) {
        Object.assign(this,{renderer,activate,save,saveTransparency,toggle,toggleMenu});
        this.value=copy(TOPBAR_DEFAULTS);this.rows=[];this.hover=null;this.press=null;this.drag=null;
        this.transparent=false;
        this.layoutListeners=new Set();
        this.activities=new Map();this.inhibitions=new Map();
        this.keyHandler=event=>{if(event.key==='Escape'){this.cancel();this.dialog?.close();}};
        globalThis.addEventListener?.('keydown',this.keyHandler);
        this.blurHandler=()=>this.cancel();
        globalThis.addEventListener?.('blur',this.blurHandler);
    }
    refreshTransparency() {
        const transparent=this.renderer.nativeCanvasWindow.snapshot?.shell?.topBarTransparent;
        if(typeof transparent==='boolean' && transparent!==this.lastTransparency){
            const changed=this.transparent!==transparent;
            this.lastTransparency=transparent;this.transparent=transparent;return changed;
        }
        return false;
    }
    refresh() {
        this.refreshTransparency();
        const raw=this.renderer.nativeCanvasWindow.snapshot?.shell?.topBarLayout;
        if(raw && raw!==this.lastRaw){this.lastRaw=raw;this.value=parseTopbarLayout(raw)||copy(TOPBAR_DEFAULTS);}
        this.rows=topbarRows(this.renderer).map(item=>({...item,title:builtinTitles()[item.key]||item.title}));
        if(this.layoutListeners.size){
            const layoutKey=JSON.stringify(this.rows.map(({key,x,y,width,height})=>[key,x,y,width,height]));
            if(layoutKey!==this.layoutKey){this.layoutKey=layoutKey;for(const listener of this.layoutListeners)listener();}
        }
        if(!this.rows.length)this.hideTip();
        else if(this.tip && this.tipOwnedKey)this.tip.textContent=this.tooltipTitle(this.rows.find(row=>row.key===this.tipOwnedKey)||{key:this.tipOwnedKey,title:''});
        if(this.dialog && this.applyTheme(this.dialog))this.repaintPreviews?.();
        if(this.dialog && this.dialogLocale!==riceLocale()){this.dialogLocale=riceLocale();for(const update of [...(this.dialogLabels||[]),...(this.rowLabels||[])])update();this.renderStatus();}
    }
    inhibit(key,reason){
        const token=Symbol(key),leases=this.inhibitions.get(key)||new Map();leases.set(token,reason);this.inhibitions.set(key,leases);this.cancel();
        return ()=>{if(!leases.delete(token))return;if(!leases.size)this.inhibitions.delete(key);this.redraw();};
    }
    disabledReason(item){return this.inhibitions.get(item?.key)?.values().next().value||'';}
    subscribeLayout(listener){this.layoutListeners.add(listener);return ()=>this.layoutListeners.delete(listener);}
    setActivity(key,active,{theme=null,reduceMotion=false}={}){
        const previous=this.activities.get(key);
        if(!active&&!previous)return;
        if(active&&previous&&previous.reduceMotion===reduceMotion){previous.theme=theme;return;}
        if(active)this.activities.set(key,{started:previous?.started??performance.now(),theme,reduceMotion});
        else this.activities.delete(key);
        this.redrawActivity();
        clearTimeout(this.activityTimer);this.activityTimer=null;
        if([...this.activities.values()].some(value=>!value.reduceMotion))this.activityTimer=setTimeout(()=>this.animateActivity(),50);
    }
    redrawActivity(){
        const rows=this.rows.filter(row=>this.activities.has(row.key)&&row.width>0);
        if(rows.length&&Array.isArray(this.renderer.damageRects))this.renderer.damageRects.push(...rows.map(({x,y,width,height})=>({x,y,width,height})));
        this.redraw();
    }
    animateActivity(){
        this.activityTimer=null;if(!this.activities.size)return;
        if(!globalThis.document?.hidden)this.redrawActivity();
        this.activityTimer=setTimeout(()=>this.animateActivity(),50);
    }
    redraw(){this.renderer.damaged=true;this.renderer.schedule();}
    itemAt(point){return this.rows.find(item=>contains(item,point));}
    panelAt(point){return [...this.renderer.windows.values()].find(panel=>panel.viewable &&
        (panel.id===0xe0000008 || (panel.id>=0xe0000100 && panel.id<0xe0000120 && (panel.id-0xe0000100)%2===0)) && contains(panel,point));}
    get rect(){return this.rows.find(r=>r.key==='@search')||null;}
    get menuRect(){return this.rows.find(r=>r.key==='@applications')||null;}
    paint(context,{panel,foregroundOnly=false}) {
        this.refresh();
        const icons=this.renderer.nativeCanvasWindow.iconRenderer;
        const chrome=this.renderer.nativeCanvasWindow.snapshot?.chrome||{};
        let background=panel.backgroundPixel&0xffffff;
        // The retained panel uses the active theme; its X11 backgroundPixel is
        // only the allocation default and may still be the old dark color.
        try {
            const pixel=panel.surfaceContext?.getImageData(0,0,1,1).data;
            if(pixel)background=(pixel[0]<<16)|(pixel[1]<<8)|pixel[2];
        } catch { /* A new panel can paint before its retained surface arrives. */ }
        if(this.transparent && panel.topbarBackdrop){
            const pixel=panel.topbarBackdrop.getImageData(0,0,1,1).data;
            background=(pixel[0]<<16)|(pixel[1]<<8)|pixel[2];
        }
        let color=chrome.icon||chrome.text||[242,242,242,255];
        // Panel and window bodies can use different surfaces within one theme.
        const luminance=rgb=>rgb.slice(0,3).reduce((sum,c,i)=>{const v=c/255;return sum+(v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4)*[0.2126,0.7152,0.0722][i];},0);
        const panelLight=luminance([(background>>16)&255,(background>>8)&255,background&255]),inkLight=luminance(color);
        if((Math.max(panelLight,inkLight)+0.05)/(Math.min(panelLight,inkLight)+0.05)<3)color=panelLight>0.4?[10,8,13,255]:[255,255,255,255];
        const hoverBackground=chrome.selection||chrome.control||[65,65,65,255];
        const radius=chrome.buttonStyle==='Pill'?16:Math.min(8,Math.max(0,Number(chrome.radius)||0));
        const panelColor=color;
        for(const item of this.rows.filter(r=>r.width>0)) {
            let color=panelColor;
            if(this.transparent && panel.topbarBackdrop){
                const paper=panel.topbarBackdrop.getImageData(
                    Math.min(panel.width-1,Math.max(0,Math.floor(item.x-panel.x+item.width/2))),
                    Math.min(panel.height-1,Math.max(0,Math.floor(item.y-panel.y+item.height/2))),1,1).data;
                color=readableInk(chrome.icon||chrome.text||[242,242,242,255],paper);
            }
            const disabled=!!this.disabledReason(item);if(disabled)color=[145,145,145,255];
            const hovered=!disabled && this.hover?.key===item.key && !this.drag;
            if(item.key==='@clock')continue;
            if(item.key==='@mail'){this.renderer._renderMailIndicator(panel,{...item,y:item.y+(item.height-24)/2,height:24},item.icon,{background:hovered?hoverBackground:null,foreground:hovered?(chrome.selectionText||color):color,hovered,radius});continue;}
            if(item.key==='@battery'){
                context.save();
                if(hovered){context.fillStyle=rgbaColor(hoverBackground);context.beginPath();context.roundRect(item.x,item.y+2,item.width,item.height-4,radius);context.fill();}
                context.strokeStyle=rgbaColor(hovered?(chrome.selectionText||color):color);context.lineWidth=1.4;
                if(item.icon==='battery'){context.strokeRect(item.x+4,item.y+11,14,10);context.fillStyle=context.strokeStyle;context.fillRect(item.x+19,item.y+14,2,4);}else{icons.renderNamed({icon:item.icon,rect:{x:item.x+4,y:item.y+(item.height-16)/2,width:16,height:16},color});context.fillStyle=context.strokeStyle;}
                context.font='10px system-ui';context.textAlign='left';context.textBaseline='middle';context.fillText(this.renderer.systemControlsLayout.batteryPercentage+'%',item.x+25,item.y+item.height/2);context.restore();
                continue;
            }
            const name=riceTopbarSymbolicIconName(item.icon);
            const known=name && (icons.fontReady || name==='brain' || name==='grain' || name==='clock');
            // Extension-owned file artwork remains in its retained surface.
            if(!known && !item.key.startsWith('@'))continue;
            const rect={x:item.x+(item.key.startsWith("@")?(item.width-16)/2:8),y:item.y+(item.height-16)/2,width:16,height:16};
            const activity=disabled?null:this.activities.get(item.key);
            const itemHoverBackground=activity?(chrome.control||hoverBackground):hoverBackground;
            context.save();
            if(activity||disabled){
                // Replace the retained extension glyph before painting its animated color.
                const cell={x:item.x+4,y:item.y+2,width:24,height:item.height-4};
                if(foregroundOnly)context.clearRect(cell.x,cell.y,cell.width,cell.height);
                else if(this.transparent&&panel.topbarBackdrop)context.drawImage(panel.topbarBackdrop.canvas,cell.x-panel.x,cell.y-panel.y,cell.width,cell.height,cell.x,cell.y,cell.width,cell.height);
                else{context.fillStyle='#'+background.toString(16).padStart(6,'0');context.fillRect(cell.x,cell.y,cell.width,cell.height);}
            }
            if(hovered){context.fillStyle=rgbaColor(itemHoverBackground);context.beginPath();const labeled=!item.key.startsWith('@')&&item.width>32;context.roundRect(item.x+(labeled?4:0),item.y+2,labeled?24:item.width,item.height-4,radius);context.fill();}
            const previousColor=icons.themeColor,previousHoverColor=icons.themeHoverColor;
            if(activity){
                const theme=typeof activity.theme==='function'?activity.theme():activity.theme;
                const accent=Number.isInteger(theme?.taskbar?.[6])?[(theme.taskbar[6]>>16)&255,(theme.taskbar[6]>>8)&255,theme.taskbar[6]&255,255]:chrome.selection||panelColor;
                const reduced=activity.reduceMotion||this.renderer.nativeCanvasWindow.snapshot?.shell?.motionEnabled===false||globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
                const beam=reduced?0:(1-Math.cos((performance.now()-activity.started)*Math.PI/800))/2;
                color=accent.map((value,index)=>index===3?255:Math.round(value+(panelColor[index]-value)*beam));
                icons.themeColor=color;icons.themeHoverColor=color;
            }
            try{icons.renderNamed({icon:name,rect,color:activity?color:hovered?(chrome.icon||color):color,hovered,hoverBackground:itemHoverBackground,monochrome:true});}
            finally{icons.themeColor=previousColor;icons.themeHoverColor=previousHoverColor;}
            context.restore();
        }
        if(this.drag && this.press && !foregroundOnly){
            const surface=this.previewSurface ||= this.renderer.canvasFactory(panel.width,panel.height);
            if(surface.width!==panel.width)surface.width=panel.width;
            if(surface.height!==panel.height)surface.height=panel.height;
            const target=surface.getContext('2d');
            target.clearRect(0,0,panel.width,panel.height);
            if(this.transparent && panel.topbarBackdrop){
                this.renderer._renderTransparentTopbarArtwork(target,panel);
            }else target.drawImage(context.canvas,panel.x,panel.y,panel.width,panel.height,0,0,panel.width,panel.height);
            const preview=previewTopbarRows(this.rows,this.press.item.key,this.drag.before,this.drag.position,panel);
            this.previewRows=preview;
            context.save();context.globalAlpha=1;
            if(this.transparent && panel.topbarBackdrop)context.drawImage(panel.topbarBackdrop.canvas,panel.x,panel.y);
            else {context.fillStyle='#'+background.toString(16).padStart(6,'0');context.fillRect(panel.x,panel.y,panel.width,panel.height);}
            for(const item of preview.filter(row=>row.width>0)){
                const original=this.rows.find(row=>row.key===item.key);
                context.drawImage(surface,original.x-panel.x,0,original.width,panel.height,item.x,panel.y,item.width,panel.height);
            }
            const selected=preview.find(row=>row.key===this.press.item.key && row.width>0);
            if(selected){
                context.strokeStyle=rgbaColor(this.renderer.nativeCanvasWindow.snapshot?.chrome?.selection||[112,185,255,255]);context.lineWidth=2;
                context.strokeRect(selected.x+1,panel.y+2,selected.width-2,panel.height-4);
            }
            context.restore();
        }else this.previewRows=null;
    }
    applyTheme(element) {
        const chrome=this.renderer.nativeCanvasWindow.snapshot?.chrome||{};
        const fallback={body:[36,36,36,255],text:[240,240,240,255],active:[65,65,65,255],control:[52,52,52,255],border:[110,110,110,255],selection:[90,150,230,255],selectionText:[255,255,255,255]};
        const signature=JSON.stringify([chrome.body,chrome.text,chrome.controlText,chrome.active,chrome.control,chrome.border,chrome.selection,chrome.radius,chrome.material,chrome.buttonStyle,chrome.windowShape]);
        if(element.dataset.riceThemeSignature===signature)return false;
        element.dataset.riceThemeSignature=signature;
        for(const key of Object.keys(fallback))element.style.setProperty('--rice-topbar-'+key,rgbaColor(chrome[key]||fallback[key]));
        const body=chrome.body||fallback.body;
        element.style.setProperty('--rice-topbar-title-text',rgbaColor(readableInk(chrome.text||fallback.text,chrome.active||fallback.active)));
        element.style.setProperty('--rice-topbar-text',rgbaColor(readableInk(chrome.controlText||chrome.text||fallback.text,body)));
        element.style.setProperty('--rice-topbar-control-text',rgbaColor(readableInk(chrome.controlText||chrome.text||fallback.text,chrome.control||fallback.control)));
        element.style.setProperty('--rice-topbar-selectionText',rgbaColor(readableInk(chrome.selectionText||fallback.selectionText,chrome.selection||fallback.selection)));
        element.style.setProperty('--rice-topbar-radius',Math.max(0,Number(chrome.radius)||0)+'px');
        element.style.setProperty('--rice-topbar-button-radius',chrome.buttonStyle==='Pill'?'999px':Math.min(6,Math.max(0,Number(chrome.radius)||0))+'px');
        element.style.setProperty('--rice-topbar-shadow',chrome.redmond98||chrome.familiar?'inset 1px 1px #fff8,inset -1px -1px #0008':'none');
        element.style.backdropFilter=chrome.material==='frosted-ice'?'blur(24px) saturate(1.2)':'none';
        element.dataset.chromeProfile=chrome.chromeProfile||'standard';
        return true;
    }

    hideTip(){clearTimeout(this.tipTimer);this.tipTimer=null;this.tip?.remove();this.tip=null;}
    tooltipTitle(item){return (this.disabledReason(item)?`${builtinTitles()[item.key]||item.title} · ${this.disabledReason(item)}`:(builtinTitles()[item.key]||item.title)+({'@search':' · Ctrl+Space','@applications':' · Super','@mail':' · Ctrl+Alt+I'}[item.key]||''));}
    showTip(item,text=null) {
        this.hideTip();if(!globalThis.document || (!this.value.tooltips && !this.drag))return;
        const output=this.renderer.outputContextAt?.(item);
        const document=output?.document||globalThis.document;
        const view=document.defaultView||globalThis;
        this.tipOwnedKey=text===null?item.key:null;
        const tip=document.createElement('div');tip.role='tooltip';tip.textContent=text===null?this.tooltipTitle(item):text;tip.dataset.riceTopbarTooltip='';
        Object.assign(tip.style,{position:'fixed',zIndex:'2147483647',pointerEvents:'none',padding:'7px 10px',border:'1px solid var(--rice-topbar-border)',borderRadius:'var(--rice-topbar-button-radius)',background:'var(--rice-topbar-body)',color:'var(--rice-topbar-text)',font:'12px system-ui',boxShadow:'0 3px 12px #0007',maxWidth:'320px'});
        this.applyTheme(tip);tip.setAttribute('popover','manual');tip.style.margin='0';tip.style.inset='auto';document.body.append(tip);tip.showPopover?.();this.tip=tip;
        const canvas=output?.canvas||this.renderer.canvas;const bounds=canvas?.getBoundingClientRect?.()||{left:0,top:0,width:view.innerWidth,height:view.innerHeight};
        const scale=bounds.width/(canvas?.width||bounds.width);
        const local=output?output.toClient(item):{x:bounds.left+item.x*scale,y:bounds.top+item.y*scale,width:item.width*scale,height:item.height*scale};
        tip.style.left=Math.max(4,Math.min(view.innerWidth-tip.offsetWidth-4,local.x+local.width/2-tip.offsetWidth/2))+'px';
        tip.style.top=(local.y+local.height+6)+'px';
    }
    cancel(){clearTimeout(this.holdTimer);this.press=null;this.drag=null;if(this.renderer.canvas?.style)this.renderer.canvas.style.cursor='';this.hideTip();this.redraw();}
    handle(event) {
        this.refresh();
        if(event.input==='pointer-button'&&(this.disabledReason(this.itemAt(event))||this.disabledReason(this.press?.item))){this.cancel();return true;}
        if(event.input==='pointer-button' && event.pressed && Number(event.button)===6 &&
            !Number.isFinite(event.scrollDeltaX) && this.panelAt(event)){
            const transparent=this.transparent=!this.transparent;this.cancel();
            // Save through the same preference used by themes and recipe capture.
            const task=(this.transparencyTransition||Promise.resolve()).catch(()=>{}).then(()=>this.saveTransparency(transparent));
            this.transparencyTransition=task;
            void task.catch(()=>{if(this.transparencyTransition===task){this.transparent=this.lastTransparency??false;this.redraw();}});
            return true;
        }
        if(event.input==='pointer-motion'){
            const item=this.itemAt(event);
            if(!this.drag && item?.key!==this.hover?.key){this.hideTip();this.hover=item||null;if(item)this.tipTimer=setTimeout(()=>this.showTip(item),500);this.redraw();}
            if(this.press){
                if(!this.drag && Math.hypot(event.x-this.press.x,event.y-this.press.y)>7){clearTimeout(this.holdTimer);this.press.cancelled=true;}
                if(this.drag){
                    const candidates=this.rows.filter(r=>r.width>0&&r.key!==this.press.item.key).sort((a,b)=>a.x-b.x);
                    const target=candidates.find(r=>event.x<r.x+r.width/2)||null;
                    this.drag={x:event.x,before:target?.key||null,position:target?.position??2,edge:target?.x??(candidates.at(-1)?.x+candidates.at(-1)?.width+4||event.x)};
                    this.redraw();
                }
                return true;
            }
            return false;
        }
        if(event.input==='pointer-button' && Number(event.button)===1){
            if(this.disabledReason(this.itemAt(event))||this.disabledReason(this.press?.item)){this.cancel();return true;}
            if(event.pressed){
                const item=this.itemAt(event);if(!item)return false;
                this.hideTip();this.press={item,x:event.x,y:event.y,cancelled:false};
                this.holdTimer=setTimeout(()=>{if(!this.press||this.press.cancelled)return;this.drag={x:event.x,before:item.key,position:item.position,edge:item.x};if(this.renderer.canvas?.style)this.renderer.canvas.style.cursor='grabbing';this.showTip(item,t('shell.topbar.drag-help'));this.redraw();},450);
                return true;
            }
            if(!this.press)return false;
            clearTimeout(this.holdTimer);
            const press=this.press,drag=this.drag;
            this.press=null;this.drag=null;if(this.renderer.canvas?.style)this.renderer.canvas.style.cursor='';this.hideTip();
            if(drag){
                if(drag.before===press.item.key){this.redraw();return true;}
                if(Math.abs(event.y-press.item.y)>press.item.height+50){this.redraw();return true;}
                const value=this.materialize();const moving=value.items.find(r=>r.key===press.item.key);moving.position=drag.position;
                value.items=value.items.filter(r=>r.key!==moving.key);const at=value.items.findIndex(r=>r.key===drag.before);
                value.items.splice(at<0?value.items.length:at,0,moving);void this.commit(value).catch(()=>{});
            }else if(!press.cancelled && this.itemAt(event)?.key===press.item.key){
                if(press.item.key==='@clock')this.toggleMenu?this.toggleMenu('notifications'):this.renderer.toggleCalendar(true,press.item);
                else if(press.item.key==='@search')this.toggle?.();
                else if(press.item.key==='@applications')this.toggleMenu?.();
                else void Promise.resolve(this.activate(press.item.key)).catch(error=>this.showTip(press.item,error.message));
            }
            this.redraw();return true;
        }
        return false;
    }
    materialize(){
        const value=copy(this.value),known=new Set(value.items.map(i=>i.key));
        for(const item of this.rows)if(!known.has(item.key))value.items.push({key:item.key,visible:true,position:item.position,icon:''});
        return value;
    }
    async commit(value){
        const checked=parseTopbarLayout(value);if(!checked)throw Error(t('shell.topbar.invalid'));
        const task=(this.saveTransition||Promise.resolve()).catch(()=>{}).then(()=>this.save(checked));
        this.saveTransition=task;
        try{await task;this.value=checked;this.redraw();this.setStatus('shell.topbar.saved');}
        catch(error){if(this.status)this.setStatus('shell.topbar.save-error',{error:error.message});else this.showTip(this.rows.find(r=>r.width>0)||{x:0,y:0,width:100,height:32},t('shell.topbar.save-tip-error',{error:error.message}));throw error;}
    }
    setStatus(key,args={}){this.statusMessage={key,args};this.renderStatus();}
    renderStatus(){if(this.status && this.statusMessage)this.status.textContent=t(this.statusMessage.key,this.statusMessage.args);}
    openSettings(){
        this.cancel();if(!globalThis.document)return;
        const document=this.renderer.outputContextAt?.()?.document||globalThis.document;
        this.dialogLabels=[];this.rowLabels=[];this.statusMessage=null;this.dialogLocale=riceLocale();
        const label=(node,render,property='textContent',rows=false)=>{const update=()=>{node[property]=render();};(rows?this.rowLabels:this.dialogLabels).push(update);update();};
        this.dialog?.remove();
        const dialog=document.createElement('dialog');this.dialog=dialog;dialog.dataset.riceKeyboardOwner='dom';dialog.dataset.riceTopbarSettings='';
        Object.assign(dialog.style,{width:'min(760px,94vw)',maxHeight:'86vh',boxSizing:'border-box',overflow:'hidden',display:'flex',flexDirection:'column',padding:'20px',border:'1px solid var(--rice-topbar-border)',borderRadius:'var(--rice-topbar-radius)',background:'var(--rice-topbar-body)',color:'var(--rice-topbar-text)',font:'14px system-ui'});
        this.applyTheme(dialog);
        const title=document.createElement('h2');label(title,()=>t('shell.topbar.title'));Object.assign(title.style,{margin:'-20px -20px 14px',padding:'10px 16px',fontSize:'16px',background:'var(--rice-topbar-active)',color:'var(--rice-topbar-title-text)'});title.style.flexShrink='0';dialog.append(title);
        const help=document.createElement('p');label(help,()=>t('shell.topbar.help'));help.style.lineHeight='1.45';help.style.margin='0 0 14px';dialog.append(help);
        const value=this.materialize();
        const tipLabel=document.createElement('label'),tips=document.createElement('input');tips.type='checkbox';tips.checked=value.tooltips;const tipText=document.createElement('span');label(tipText,()=>t('shell.topbar.tooltips'));tipLabel.append(tips,' ',tipText);dialog.append(tipLabel);
        const list=document.createElement('div');list.dataset.riceTopbarList='';Object.assign(list.style,{marginTop:'14px',overflowY:'auto',minHeight:'0',flex:'1 1 auto',paddingRight:'6px',overscrollBehavior:'contain',scrollbarColor:'var(--rice-topbar-border) var(--rice-topbar-body)'});dialog.append(list);
        const status=document.createElement('p');status.role='status';status.style.margin='8px 0';status.style.minHeight='18px';status.style.fontSize='12px';dialog.append(status);this.status=status;
        const save=()=>{this.setStatus('day.ui.23e39291d613');void this.commit(copy(value)).catch(()=>{});};tips.onchange=()=>{value.tooltips=tips.checked;this.hideTip();save();};
        const names=new Map([...Object.entries(builtinTitles()),...this.rows.map(r=>[r.key,r.title])]);
        const draw=()=>{
            list.replaceChildren();this.rowLabels=[];
            const painters=[];this.repaintPreviews=()=>painters.forEach(paint=>paint());
            value.items.forEach((item,index)=>{
                const row=document.createElement('div');row.dataset.topbarItem=item.key;Object.assign(row.style,{display:'grid',gridTemplateColumns:'20px 22px minmax(110px,1fr) 90px 118px 30px 30px',gap:'7px',alignItems:'center',padding:'8px 0'});
                const visible=document.createElement('input');visible.type='checkbox';visible.checked=item.visible;label(visible,()=>t('shell.topbar.show',{name:builtinTitles()[item.key]||names.get(item.key)||item.key}),'ariaLabel',true);visible.onchange=()=>{item.visible=visible.checked;save();};
                const nameLabel=document.createElement('span');label(nameLabel,()=>builtinTitles()[item.key]||names.get(item.key)||item.key.split('@')[0].replace(/[-_]/g,' '),'textContent',true);
                const preview=document.createElement('canvas');preview.width=20;preview.height=20;preview.style.width='20px';preview.style.height='20px';preview.setAttribute('aria-hidden','true');
                const paintPreview=()=>{const icons=this.renderer.nativeCanvasWindow.iconRenderer,previous=icons.context;const context=preview.getContext('2d');context.clearRect(0,0,20,20);icons.context=context;try{const icon=riceTopbarSymbolicIconName(item.icon||this.rows.find(r=>r.key===item.key)?.icon)||'settings';icons.renderNamed({icon,rect:{x:2,y:2,width:16,height:16},color:this.renderer.nativeCanvasWindow.snapshot?.chrome?.icon||[235,238,244,255],monochrome:true});}finally{icons.context=previous;}};painters.push(paintPreview);paintPreview();
                const position=document.createElement('select');label(position,()=>t('shell.topbar.position',{name:nameLabel.textContent}),'ariaLabel',true);for(const [n,key]of [[0,'settings.left'],[1,'settings.center'],[2,'settings.right']]){const option=document.createElement('option');option.value=n;label(option,()=>t(key),'textContent',true);position.append(option);}position.value=item.position;position.onchange=()=>{item.position=Number(position.value);save();};
                const icon=document.createElement('select');label(icon,()=>t('shell.topbar.icon',{name:nameLabel.textContent}),'ariaLabel',true);for(const name of ['',...RICE_ICON_CATALOG]){const option=document.createElement('option');option.value=name;label(option,()=>name?riceIconLabel(name):t('shell.topbar.default-icon'),'textContent',true);icon.append(option);}icon.value=item.icon;icon.disabled=item.key==='@clock';icon.onchange=()=>{item.icon=icon.value;paintPreview();save();};
                const up=document.createElement('button'),down=document.createElement('button');up.textContent='←';down.textContent='→';for(const [button,key]of [[up,'shell.topbar.earlier'],[down,'shell.topbar.later']]){label(button,()=>t(key,{name:nameLabel.textContent}),'title',true);label(button,()=>button.title,'ariaLabel',true);}const previous=value.items.findLastIndex((other,i)=>i<index&&other.position===item.position);const next=value.items.findIndex((other,i)=>i>index&&other.position===item.position);up.disabled=previous<0;down.disabled=next<0;
                const move=delta=>{const target=delta<0?previous:next;value.items.splice(index,1);value.items.splice(target,0,item);save();draw();};up.onclick=()=>move(-1);down.onclick=()=>move(1);
                row.append(visible,preview,nameLabel,position,icon,up,down);list.append(row);
            });
        };draw();
        const reset=document.createElement('button');label(reset,()=>t('shell.topbar.restore'));reset.onclick=async()=>{await this.commit(copy(TOPBAR_DEFAULTS));dialog.close();};
        const close=document.createElement('button');label(close,()=>t('common.done'));close.style.marginLeft='12px';close.onclick=()=>dialog.close();const footer=document.createElement('div');footer.style.flexShrink='0';footer.append(reset,close);dialog.append(footer);
        dialog.addEventListener('close',()=>{dialog.remove();this.status=null;this.dialog=null;this.repaintPreviews=null;this.dialogLabels=[];this.rowLabels=[];this.statusMessage=null;});const style=document.createElement('style');style.textContent=`
[data-rice-topbar-settings]::backdrop{background:rgba(0,0,0,.4)}
[data-rice-topbar-settings] button,[data-rice-topbar-settings] select{background:var(--rice-topbar-control);color:var(--rice-topbar-control-text);border:1px solid var(--rice-topbar-border);border-radius:var(--rice-topbar-button-radius);box-shadow:var(--rice-topbar-shadow);padding:6px;font:12px system-ui;min-height:30px}
[data-rice-topbar-settings] button:not(:disabled):hover{background:var(--rice-topbar-selection);color:var(--rice-topbar-selectionText)}
[data-rice-topbar-settings] button:focus-visible,[data-rice-topbar-settings] select:focus-visible,[data-rice-topbar-settings] input:focus-visible{outline:2px solid var(--rice-topbar-selection);outline-offset:2px}
[data-rice-topbar-settings] input{accent-color:var(--rice-topbar-selection)}
[data-rice-topbar-settings] button:disabled,[data-rice-topbar-settings] select:disabled{opacity:.7}
[data-rice-topbar-settings] [data-topbar-item]{border-bottom:1px solid var(--rice-topbar-border)}
@media(max-width:650px){[data-rice-topbar-settings] [data-topbar-item]{grid-template-columns:20px 22px 1fr 30px 30px!important}[data-rice-topbar-settings] [data-topbar-item] select{grid-row:2;grid-column:3 / 6}[data-rice-topbar-settings] [data-topbar-item] select+select{grid-row:3}[data-rice-topbar-settings] [data-topbar-item] button{grid-row:1}}
`;dialog.append(style);document.body.append(dialog);dialog.showModal();tips.focus({preventScroll:true});
    }
    close(){clearTimeout(this.activityTimer);this.activityTimer=null;this.activities.clear();this.layoutListeners.clear();this.cancel();this.dialog?.close();globalThis.removeEventListener?.('keydown',this.keyHandler);globalThis.removeEventListener?.('blur',this.blurHandler);}
}
