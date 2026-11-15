import {t} from './rice-i18n.mjs';
import {overviewTheme} from './rice-overview-theme.mjs';

const identity = item => `${item.kind}:${item.id}`;
const inside = (rect, x, y) => rect && x >= rect.x && y >= rect.y && x < rect.x + rect.width && y < rect.y + rect.height;
const wrap = (value, count) => ((value % count) + count) % count;
export function carouselContains(points, x, y) {
    let hit = false;
    for (let i=0,j=points.length-1;i<points.length;j=i++) {
        const a=points[i],b=points[j];
        if ((a.y>y)!==(b.y>y) && x<(b.x-a.x)*(y-a.y)/(b.y-a.y)+a.x) hit=!hit;
    }
    return hit;
}

function traceSurface(context, points) {
    context.beginPath();points.forEach((point,index)=>index?context.lineTo(point.x,point.y):context.moveTo(point.x,point.y));context.closePath();
}

// Paint one depth-sorted mesh facet. Texture coordinates stay attached to the
// outside of the cylinder: looking through its back naturally reverses the app.
function projectFacet(context, surface, facet) {
    const {left,right,u0,u1}=facet;
    const width=(u1-u0)*surface.width;
    const top={x:(left.top.x+right.top.x)/2,y:(left.top.y+right.top.y)/2};
    const bottom={x:(left.bottom.x+right.bottom.x)/2,y:(left.bottom.y+right.bottom.y)/2};
    const dx=(right.top.x-left.top.x)/width,dy=(right.top.y-left.top.y)/width;
    context.save();
    // Slightly overlap interior seams. All facets are opaque, and closer
    // surfaces overwrite farther ones, including a window's own silhouette.
    const overlap=Math.min(2,1.2/Math.max(.01,Math.hypot(right.top.x-left.top.x,right.top.y-left.top.y)));
    const expand=(a,b,t)=>({x:a.x+(b.x-a.x)*t,y:a.y+(b.y-a.y)*t});
    const before=Math.min(overlap,u0/(u1-u0)),after=Math.min(overlap,(1-u1)/(u1-u0));
    const outline=[expand(left.top,right.top,-before),expand(left.top,right.top,1+after),
        expand(left.bottom,right.bottom,1+after),expand(left.bottom,right.bottom,-before)];
    traceSurface(context,outline);context.clip();
    context.transform(dx,dy,(bottom.x-top.x)/surface.height,(bottom.y-top.y)/surface.height,
        top.x-dx*width/2,top.y-dy*width/2);
    const sourceLeft=u0*surface.width-width*before,sourceRight=u1*surface.width+width*after;
    context.drawImage(surface,sourceLeft,0,sourceRight-sourceLeft,surface.height,-width*before,0,sourceRight-sourceLeft,surface.height);
    context.restore();
}

const clamp = value => Math.max(0, Math.min(1, value));
const easeOut = value => 1 - (1 - clamp(value)) ** 3;
const mix = (a, b, amount) => a + (b - a) * amount;

// Unwrap the selected cylindrical surface, turn it towards the camera, then
// land every texture coordinate exactly on the live window's desktop frame.
export function carouselActivationColumns(entry, projection, target, amount) {
    const {radius,width,height}=entry,{depth,camera,cosPitch,sinPitch,centerX,centerY}=projection;
    const bend=1-amount,angle=entry.angle*bend,cos=Math.cos(angle),sin=Math.sin(angle);
    return entry.columns.map((_,index)=>{
        const u=index/(entry.columns.length-1),local=(u-.5)*width,arc=local*bend/radius;
        const x=bend>1e-6?Math.sin(arc)*radius/bend:local;
        const z=radius+(bend>1e-6?(Math.cos(arc)-1)*radius/bend:0);
        const worldX=x*cos+z*sin,worldZ=z*cos-x*sin;
        const project=(y,destinationY)=>{
            const distance=camera-worldZ*cosPitch+y*sinPitch;
            return {x:mix(centerX+worldX*depth/distance,target.x+u*target.width,amount),
                y:mix(centerY+(y*cosPitch+worldZ*sinPitch)*depth/distance,destinationY,amount),distance};
        };
        return {top:project(-height/2,target.y),bottom:project(height/2,target.y+target.height)};
    });
}

export class RiceOverviewCarousel {
    constructor(renderer) {
        this.renderer = renderer;
        this.position = 0; this.target = 0; this.entries = []; this.facets = []; this.items = [];
        this.textures = new Map(); this.backdrop = null; this.press = null; this.lastFrame = null;
        this.scope = null; this.scopeDefault = null; this.signature = '';
    }
    get enabled() { return this.renderer.nativeCanvasWindow.snapshot?.shell?.activitiesCarousel === true; }
    get allWorkspaces() {
        const value = this.renderer.nativeCanvasWindow.snapshot?.shell?.activitiesAllWorkspaces === true;
        if (this.scopeDefault !== value) { this.scopeDefault = value; this.scope = null; }
        return this.scope ?? value;
    }
    get reduced() { const shell=this.renderer.nativeCanvasWindow.snapshot?.shell;return shell?.reduceMotion || shell?.motionEnabled === false; }
    damage() { this.renderer.damaged = true; this.renderer.schedule(); }
    reset() { this.press = null; this.signature = ''; this.textures.clear(); this.backdrop=null; this.blurredBackdrop=null; this.lastFrame = null; this.wheel=0; this.scope=null; this.selectedKey=null; this.activation=null; }
    select(index, animate = true) {
        if (this.activation || !this.items.length) return null;
        this.target = index;
        if (!animate || this.reduced) this.position = index;
        const item = this.items[wrap(Math.round(index),this.items.length)];
        this.renderer.overviewHover = identity(item); this.renderer.overviewPointerActive = false;
        this.damage(); return item;
    }
    move(direction) { return this.select(Math.round(this.target) + (['left','up'].includes(direction) ? -1 : 1)); }
    layout(items, root, top, filterTop = 46) {
        if (this.activation) return this.entries;
        this.root={width:root.width,height:root.height};
        top=Math.max(top,filterTop+50);
        const renderer=this.renderer;
        this.area=renderer.overviewInteraction?.searchArea(root)||{x:0,y:0,width:root.width,height:root.height};
        const area=this.area;
        this.scopeRect={x:area.x+area.width/2-88,y:area.y+top+4,width:176,height:28};
        this.previousRect={x:area.x+area.width/2-90,y:area.y+area.height-114,width:42,height:36};
        this.nextRect={...this.previousRect,x:area.x+area.width/2+48};
        const query=renderer.overviewSearchQuery;
        const filterWidth=Math.min(460,Math.max(100,area.width-48));
        this.filterRect=query?{x:area.x+(area.width-filterWidth)/2,y:area.y+filterTop,width:filterWidth,height:36}:null;
        this.clearFilterRect=this.filterRect?{x:this.filterRect.x+filterWidth-36,y:this.filterRect.y,width:36,height:36}:null;
        this.items=items;
        const signature=items.map(identity).join('|');
        if (signature!==this.signature) {
            const old=this.selectedKey;
            this.signature=signature;
            const selected=Math.max(0,items.findIndex(item=>identity(item)===old || (!old && item.active)));
            this.position=this.target=selected;
            for (const id of this.textures.keys()) if (!items.some(item=>identity(item)===id)) this.textures.delete(id);
        }
        if (!items.length) { this.entries=[]; this.facets=[]; return []; }
        const now=renderer.now(),dt=Math.min(40,Math.max(0,now-(this.lastFrame??now-16)));this.lastFrame=now;
        if (!this.press?.dragging) {
            this.target=Math.round(this.target);
            this.position=this.reduced?this.target:this.position+(this.target-this.position)*(1-Math.exp(-dt/85));
            if (Math.abs(this.target-this.position)<.001) this.position=this.target; else this.damage();
        }
        const selected=items[wrap(Math.round(this.target),items.length)];
        this.selectedKey=identity(selected);renderer.overviewHover=this.selectedKey;
        const count=items.length,step=Math.PI*2/count;
        const radius=area.width*.54,depth=Math.max(900,area.width*1.25),camera=depth+radius;
        const pitch=Math.min(.20,Math.max(.06,(area.height-top-220)/(area.width*2.1))),cosPitch=Math.cos(pitch),sinPitch=Math.sin(pitch);
        const centerX=area.x+area.width/2,centerY=area.y+top+(area.height-top-160)*.49-radius*sinPitch*.45;
        const project=(x,y,z)=>{
            const distance=camera-z*cosPitch+y*sinPitch;
            return {x:centerX+x*depth/distance,y:centerY+(y*cosPitch+z*sinPitch)*depth/distance,distance};
        };
        this.projection={radius,depth,camera,cosPitch,sinPitch,centerX,centerY};
        this.entries=items.map((item,index)=>{
            // One cylinder around a fixed vertical axis owns every vertex and
            // outward normal. Rotation never turns the rear windows to the viewer.
            const offset=wrap(index-this.position+items.length/2,items.length)-items.length/2;
            const angle=offset*step,cos=Math.cos(angle),sin=Math.sin(angle);
            const ratio=Math.max(.45,Math.min(2.2,item.source.width/Math.max(1,item.source.height)));
            const width=Math.min(area.width*.40,(area.height-top-210)*.70*ratio,radius*step*.88),height=width/ratio;
            const x=sin*radius,z=cos*radius,segments=64,span=width/radius;
            let columns=Array.from({length:segments+1},(_,column)=>{
                const arc=angle+(column/segments-.5)*span;
                const normal={x:Math.sin(arc),z:Math.cos(arc)};
                const worldX=normal.x*radius,worldZ=normal.z*radius;
                return {top:project(worldX,-height/2,worldZ),bottom:project(worldX,height/2,worldZ),
                    world:{x:worldX,z:worldZ},normal};
            });
            const corners=[...columns.map(column=>column.top),...columns.map(column=>column.bottom).reverse()];
            const xs=corners.map(p=>p.x),ys=corners.map(p=>p.y);
            const preview={x:Math.min(...xs),y:Math.min(...ys),width:Math.max(...xs)-Math.min(...xs),height:Math.max(...ys)-Math.min(...ys)};
            return {item,preview,hit:preview,corners,columns,radius,width,height,angle,normal:{x:sin,z:cos},center:{x,z},z,selected:identity(item)===this.selectedKey,opacity:.48+.52*(cos+1)/2};
        }).sort((a,b)=>a.z-b.z);
        this.facets=this.entries.flatMap(entry=>entry.columns.slice(0,-1).map((left,index)=>{
            const right=entry.columns[index+1],u0=index/(entry.columns.length-1),u1=(index+1)/(entry.columns.length-1);
            const cosine=(left.normal.z+right.normal.z)/2;
            return {entry,left,right,u0,u1,
                outline:[left.top,right.top,right.bottom,left.bottom],
                distance:(left.top.distance+left.bottom.distance+right.top.distance+right.bottom.distance)/4,
                back:camera*cosPitch*cosine-radius<0};
        })).sort((a,b)=>b.distance-a.distance);
        return this.entries;
    }
    hit(x,y) { return this.facets.findLast(facet=>carouselContains(facet.outline,x,y))?.entry.item || null; }
    texture(item, colors, back = false) {
        const renderer=this.renderer,id=identity(item);
        // Native command arrays and retained X11 revisions are stable between
        // frames. Rotation must not re-render all applications every frame.
        const native=item.snapshot||renderer.nativeCanvasWindow.snapshots.find(snapshot=>Number(snapshot.taskbarId)===Number(item.id));
        const revision=item.kind==='native'?native:renderer.sceneBackdropRevision;
        const finish=record=>{
            if(!back)return record.surface;
            if(!record.back){
                record.back=renderer.canvasFactory(record.surface.width,record.surface.height);
                record.back.width=record.surface.width;record.back.height=record.surface.height;
                const context=record.back.getContext('2d',{alpha:false});
                context.drawImage(record.surface,0,0);
                context.fillStyle=colors.background(.32);context.fillRect(0,0,record.back.width,record.back.height);
            }
            return record.back;
        };
        const cached=this.textures.get(id);
        if (cached && cached.revision===revision && cached.assetsRevision===renderer.overviewPreviewRevision) return finish(cached);
        const scale=Math.min(1,900/Math.max(1,item.source.width),600/Math.max(1,item.source.height));
        const surface=renderer.canvasFactory(Math.max(1,Math.round(item.source.width*scale)),Math.max(1,Math.round(item.source.height*scale)));
        surface.width=Math.max(1,Math.round(item.source.width*scale));surface.height=Math.max(1,Math.round(item.source.height*scale));
        const context=surface.getContext('2d',{alpha:false});const rect={x:0,y:0,width:surface.width,height:surface.height};
        context.fillStyle=colors.surface;context.fillRect(0,0,rect.width,rect.height);
        const painted=item.kind==='native'?renderer.nativeCanvasWindow.renderSnapshotPreview(native,context,rect):renderer._renderX11TaskPreview(item.id,context,rect);
        if (!painted) { context.fillStyle=colors.ink;context.font='20px sans-serif';context.textAlign='center';context.fillText(item.title,rect.width/2,rect.height/2,rect.width-24); }
        const record={revision,assetsRevision:renderer.overviewPreviewRevision,surface};
        this.textures.set(id,record);return finish(record);
    }
    beginActivation(item) {
        if (this.activation || !this.enabled || !this.renderer.overviewActive || this.renderer.overviewProgress===0 || this.reduced) return false;
        const entry=this.entries.find(entry=>identity(entry.item)===identity(item));
        if (!entry || !this.root) return false;
        const colors=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome);
        const surface=this.texture(entry.item,colors),back=this.texture(entry.item,colors,true);
        const others=this.renderer.canvasFactory(this.root.width,this.root.height);
        others.width=this.root.width;others.height=this.root.height;
        this.paintScene(others.getContext('2d'),this.root,1,identity(item));
        this.activation={entry,surface,back,others,backdrop:this.blurredBackdrop||this.backdrop,projection:this.projection,
            target:{...item.source,...entry.item.source},startedAt:this.renderer.now(),duration:220};
        this.press=null;this.damage();return true;
    }
    activationAmount() { return clamp((this.renderer.now()-this.activation.startedAt)/this.activation.duration); }
    paintActivation(context) {
        const flight=this.activation,t=this.activationAmount(),amount=easeOut(t);
        const live=flight.entry.item.kind==='native'
            ? this.renderer.nativeCanvasWindow.snapshots.find(snapshot=>Number(snapshot.taskbarId)===Number(flight.entry.item.id))?.frame
            : this.renderer.windows.get(flight.entry.item.id);
        if (live && [live.x,live.y,live.width,live.height].every(Number.isFinite)) {
            const border=flight.entry.item.kind==='x11'?live.borderWidth||0:0;
            flight.target={x:live.x-border,y:live.y-border,width:live.width+border*2,height:live.height+border*2};
        }
        context.save();context.globalAlpha=(1-amount)**2;
        context.drawImage(flight.others,0,0);context.restore();
        // Sixteen facets stay below a pixel of curvature error at desktop
        // sizes while keeping activation rasterization short on software GPUs.
        const columns=carouselActivationColumns(flight.entry,flight.projection,flight.target,amount).filter((_,index)=>index%4===0);
        const facets=columns.slice(0,-1).map((left,index)=>{
            const right=columns[index+1],dx=right.top.x-left.top.x,dy=right.top.y-left.top.y;
            return {left,right,u0:index/(columns.length-1),u1:(index+1)/(columns.length-1),
                back:dx*(left.bottom.y-left.top.y)-dy*(left.bottom.x-left.top.x)<0,
                distance:(left.top.distance+left.bottom.distance+right.top.distance+right.bottom.distance)/4};
        }).sort((a,b)=>b.distance-a.distance);
        // The last few frames reveal the live surface at identical geometry.
        context.save();context.globalAlpha=1-clamp((t-.84)/.16);
        const outline=[...columns.map(c=>c.top),...columns.map(c=>c.bottom).reverse()];
        traceSurface(context,outline);context.fillStyle=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome).surface;
        context.shadowColor='rgba(0,0,0,.25)';context.shadowBlur=18*(1-amount);context.shadowOffsetY=8*(1-amount);context.fill();
        context.shadowColor='transparent';
        for(const facet of facets)projectFacet(context,facet.back?flight.back:flight.surface,facet);
        context.restore();
    }
    paintBackdrop(context,root,progress) {
        if (this.activation) {
            const flight=this.activation,t=this.activationAmount(),amount=easeOut(t);
            context.save();context.globalAlpha=1-easeOut(clamp((t-.55)/.45));
            // Reuse the already blurred wallpaper. A full-screen blur on every
            // activation frame forces expensive software rasterization.
            if(flight.backdrop)context.drawImage(flight.backdrop,0,0,root.width,root.height);
            else {context.fillStyle=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome).surface;context.fillRect(0,0,root.width,root.height);}
            context.filter='none';context.fillStyle=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome).background(.22*(1-amount));
            context.fillRect(0,0,root.width,root.height);context.restore();return;
        }
        const scale=Math.min(.6,1280/root.width),width=Math.max(1,Math.round(root.width*scale)),height=Math.max(1,Math.round(root.height*scale));
        if(!this.backdrop||this.backdrop.width!==width||this.backdrop.height!==height){this.backdrop=this.renderer.canvasFactory(width,height);this.backdrop.width=width;this.backdrop.height=height;}
        const background=this.backdrop.getContext('2d',{alpha:false});
        background.drawImage(context.canvas,0,0,width,height);
        if(!this.blurredBackdrop||this.blurredBackdrop.width!==width||this.blurredBackdrop.height!==height){this.blurredBackdrop=this.renderer.canvasFactory(width,height);this.blurredBackdrop.width=width;this.blurredBackdrop.height=height;}
        const blurred=this.blurredBackdrop.getContext('2d',{alpha:false});
        blurred.filter=`blur(${7*scale}px)`;blurred.drawImage(this.backdrop,-8*scale,-8*scale,width+16*scale,height+16*scale);blurred.filter='none';
        context.save();context.globalAlpha=progress;
        context.drawImage(this.blurredBackdrop,0,0,root.width,root.height);
        const colors=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome);
        context.fillStyle=colors.background(.22);context.fillRect(0,0,root.width,root.height);context.restore();
    }
    paint(context, root, progress) {
        if (this.activation) { this.paintActivation(context);return; }
        this.paintScene(context,root,progress);
    }
    paintScene(context, root, progress, omit = null) {
        const colors=overviewTheme(this.renderer.nativeCanvasWindow.snapshot?.chrome),area=this.area;
        context.save();context.globalAlpha*=progress;
        for (const entry of this.entries) {
            if(identity(entry.item)===omit)continue;
            context.save();traceSurface(context,entry.corners);
            context.fillStyle=colors.surface;context.shadowColor='rgba(0,0,0,.28)';context.shadowBlur=18;context.shadowOffsetY=10;
            context.fill();context.restore();
        }
        // Painter order belongs to individual world-space facets, not window
        // centers. Side surfaces may turn away and occlude their own back edge.
        for (const facet of this.facets) {
            const {entry,left,right}=facet;
            if(identity(entry.item)===omit)continue;
            projectFacet(context,this.texture(entry.item,colors,facet.back),facet);
            context.save();
            context.strokeStyle=entry.selected?colors.accent:colors.line;context.lineWidth=entry.selected?1.7:.8;
            context.beginPath();context.moveTo(left.top.x,left.top.y);context.lineTo(right.top.x,right.top.y);
            context.moveTo(left.bottom.x,left.bottom.y);context.lineTo(right.bottom.x,right.bottom.y);
            if(facet.u0===0){context.moveTo(left.top.x,left.top.y);context.lineTo(left.bottom.x,left.bottom.y);}
            if(facet.u1===1){context.moveTo(right.top.x,right.top.y);context.lineTo(right.bottom.x,right.bottom.y);}
            context.stroke();context.restore();
        }
        const selected=this.items.find(item=>identity(item)===this.selectedKey);
        context.textAlign='center';context.textBaseline='middle';
        const button=(rect,label,active=false)=>{this.renderer._roundedRect(context,rect,colors.radius);context.fillStyle=active?colors.accentWash:colors.control;context.fill();context.strokeStyle=active?colors.accent:colors.line;context.lineWidth=1;context.stroke();context.fillStyle=colors.ink;context.font='500 13px "RiceWM Sans", sans-serif';context.fillText(label,rect.x+rect.width/2,rect.y+rect.height/2,rect.width-12);};
        if(this.filterRect){
            this.renderer._roundedRect(context,this.filterRect,colors.radius);context.fillStyle=colors.control;context.fill();
            context.strokeStyle=colors.accent;context.stroke();
            context.save();context.beginPath();context.rect(this.filterRect.x+12,this.filterRect.y,this.filterRect.width-52,this.filterRect.height);context.clip();
            context.textAlign='left';context.fillStyle=colors.ink;context.font='14px "RiceWM Sans", sans-serif';
            context.fillText(this.renderer.overviewSearchQuery,this.filterRect.x+14,this.filterRect.y+18);context.restore();
            button(this.clearFilterRect,'×');
        }
        button(this.scopeRect,`${this.allWorkspaces?'✓ ':''}${t('activities.carousel.all')}`,this.allWorkspaces);
        context.fillStyle=colors.ink;context.font='500 14px "RiceWM Sans", sans-serif';
        context.fillText(selected?.title||t(this.renderer.overviewSearchQuery?'activities.carousel.no-matches':'activities.carousel.empty'),area.x+area.width/2,area.y+area.height-174,area.width-60);
        context.fillStyle=colors.muted;context.font='12px "RiceWM Sans", sans-serif';
        const detail=selected?t('activities.carousel.workspace',{workspace:(selected.workspace??0)+1,index:wrap(Math.round(this.target),this.items.length)+1,count:this.items.length}):t(this.renderer.overviewSearchQuery?'activities.carousel.clear-filter-hint':'activities.carousel.empty-hint');
        context.fillText(detail,area.x+area.width/2,area.y+area.height-147,area.width-40);
        button(this.previousRect,'‹');button(this.nextRect,'›');
        context.fillStyle=colors.ink;context.fillText(this.items.length?`${wrap(Math.round(this.target),this.items.length)+1} / ${this.items.length}`:'0 / 0',area.x+area.width/2,area.y+area.height-96);
        context.fillStyle=colors.muted;context.font='12px "RiceWM Sans", sans-serif';context.fillText(t('activities.carousel.hint'),area.x+area.width/2,area.y+area.height-51,area.width-32);
        context.restore();
    }
    handle(event) {
        if (!this.enabled || !this.renderer.overviewActive) return null;
        if (this.activation) return {};
        const {x,y}=event;
        if (event.recovered || event.input==='pointer-cancel') { this.press=null;this.select(Math.round(this.position));return {}; }
        if (event.input==='pointer-axis' || event.input==='pointer-button'&&[4,5].includes(Number(event.button))) {
            if (event.input==='pointer-button'&&!event.pressed) return {};
            const dx=Number(event.scrollDeltaX)||0,dy=Number(event.scrollDeltaY)||0;
            // The normalizer emits both an axis event and vertical wheel buttons.
            // Horizontal packets belong to the axis; vertical packets to buttons.
            if (event.input==='pointer-axis'&&Math.abs(dx)<=Math.abs(dy) || event.input==='pointer-button'&&Math.abs(dx)>Math.abs(dy)) return {};
            const delta=Math.abs(dx)>Math.abs(dy)?dx:dy||Number(event.scrollDelta)||(event.scrollDelta===0?0:event.button===4?-80:80);
            if (!Number.isFinite(delta)||delta===0 || this.press) return {};
            const time=this.renderer.now();
            if (time-(this.lastWheel??-Infinity)>200 || Math.sign(delta)!==Math.sign(this.wheel||delta)) this.wheel=0;
            this.lastWheel=time;
            this.wheel=(this.wheel||0)+delta;
            if (Math.abs(this.wheel)>=45) { const steps=Math.sign(this.wheel)*Math.max(1,Math.floor(Math.abs(this.wheel)/120));this.wheel=0;this.select(Math.round(this.target)+Math.max(-3,Math.min(3,steps))); }
            return {};
        }
        if (event.input==='pointer-motion') {
            if (this.press && Math.abs(x-this.press.x)>6) this.press.dragging=true;
            if (this.press?.dragging) { this.position=this.target=this.press.position-(x-this.press.x)/Math.max(80,this.area.width*.15);this.damage(); }
            return {};
        }
        if (event.input==='pointer-button'&&Number(event.button)===1) {
            if (event.pressed) { this.press={x,y,position:this.position,item:this.hit(x,y),scope:inside(this.scopeRect,x,y),clear:inside(this.clearFilterRect,x,y),filter:inside(this.filterRect,x,y),previous:inside(this.previousRect,x,y),next:inside(this.nextRect,x,y)};return {}; }
            const start=this.press;this.press=null;if (!start) return {};
            if (start.dragging) { this.select(Math.round(this.position));return {}; }
            if (start.clear&&inside(this.clearFilterRect,x,y)) { this.renderer.setOverviewSearchQuery('');return {query:''}; }
            if (start.filter) return {};
            if (start.scope&&inside(this.scopeRect,x,y)) { this.scope=!this.allWorkspaces;this.signature='';this.renderer.overviewItems=null;this.damage();return {}; }
            if (start.previous&&inside(this.previousRect,x,y)) {this.move('left');return {};}
            if (start.next&&inside(this.nextRect,x,y)) {this.move('right');return {};}
            const item=this.hit(x,y);
            if (start.item&&item&&identity(start.item)===identity(item)) {this.beginActivation(item);return {activate:item};}
            return {};
        }
        return null;
    }
}
