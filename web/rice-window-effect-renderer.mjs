import {themeColor} from './rice-theme-customization.mjs';
import {rgbaColor} from './rice-native-canvas-primitives.mjs';
import {windowEffectsOutset,sampleWindowEffect,windowEffectPoint} from './rice-window-effects.mjs';
const css=color=>rgbaColor(themeColor(color,[0,0,0,0]));
function ink(context,paint,rect){
    if(!paint.gradient)return css(paint.color);
    const g=paint.gradient,a=(g.angle??90)*Math.PI/180,dx=Math.cos(a)*rect.width/2,dy=Math.sin(a)*rect.height/2,cx=rect.width/2,cy=rect.height/2;
    const gradient=context.createLinearGradient(cx-dx,cy-dy,cx+dx,cy+dy);
    for(const stop of g.stops)gradient.addColorStop(stop.at,css(stop.color));
    return gradient;
}
/** Cache static halos independently of window position/content; bounded to 32 MiB. */
export class RiceWindowEffectRenderer {
    constructor(){this.cache=new Map();this.bytes=0;}
    clear(){this.cache.clear();this.bytes=0;}
    paint(context,frame,style,shapeKey,appendPath,{now=0,reducedMotion=true}={}){
        const pad=windowEffectsOutset({windowEffects:{active:style}});
        if(!pad&&!(style.layers?.length))return;
        const width=Math.ceil(frame.width+2*pad),height=Math.ceil(frame.height+2*pad);
        const {animation,...staticStyle}=style;
        const key=JSON.stringify([frame.width,frame.height,staticStyle,pad,shapeKey]);
        let surface=this.cache.get(key);
        if(surface){this.cache.delete(key);this.cache.set(key,surface);}
        else{
            // Large windows use a bounded raster; geometry and input remain exact.
            const scale=Math.min(1,2048/width,2048/height);
            surface=new OffscreenCanvas(Math.max(1,Math.ceil(width*scale)),Math.max(1,Math.ceil(height*scale)));
            const c=surface.getContext('2d');c.scale(scale,scale);c.translate(pad,pad);
            for(const layer of style.layers??[]){
                c.save();c.translate(layer.x??0,layer.y??0);c.filter=`blur(${(layer.blur??0)*scale}px)`;
                c.fillStyle=c.strokeStyle=ink(c,layer,frame);c.beginPath();appendPath(c);c.fill();
                if(layer.spread){c.lineWidth=layer.spread*2;c.stroke();}c.restore();
            }
            if(style.rim&&(style.rim.width??1)>0){c.strokeStyle=ink(c,style.rim,frame);c.lineWidth=(style.rim.width??1)*2;c.beginPath();appendPath(c);c.stroke();}
            // Remove the complete interior, including blurred pixels. Effects
            // never tint content or leak into a translucent window's backdrop.
            c.save();c.globalCompositeOperation='destination-out';c.fillStyle='#000';c.beginPath();appendPath(c);c.fill();c.restore();
            const cost=surface.width*surface.height*4;
            while(this.cache.size&&(this.bytes+cost>32*1024*1024||this.cache.size>=16)){
                const first=this.cache.keys().next().value,old=this.cache.get(first);this.bytes-=old.width*old.height*4;this.cache.delete(first);
            }
            this.cache.set(key,surface);this.bytes+=cost;
        }
        const sample=sampleWindowEffect(style,now,reducedMotion);
        context.save();
        try{
            context.globalAlpha*=sample.alpha;
            context.drawImage(surface,frame.x-pad,frame.y-pad,width,height);
            const effect=sample.effect??animation?.preset;
            if(!sample.animated||effect==='breathe'||effect==='none')return;
            context.translate(frame.x,frame.y);
            context.beginPath();context.rect(-pad,-pad,width,height);appendPath(context);context.clip('evenodd');
            const radius=typeof shapeKey?.[0]==='number'?shapeKey[0]:8;
            const point=t=>{const p=windowEffectPoint(frame.width+8,frame.height+8,radius+4,t);return {x:p.x-4,y:p.y-4};};
            const tint=themeColor(sample.color??animation.color??'#dfc6ff',[223,198,255,255]);
            context.globalAlpha*=sample.intensity;
            if(effect==='flow'){
                const p=point(sample.phase),g=context.createRadialGradient(p.x,p.y,0,p.x,p.y,64);
                g.addColorStop(0,rgbaColor(tint));g.addColorStop(0.2,rgbaColor([...tint.slice(0,3),Math.round(tint[3]*0.55)]));g.addColorStop(1,rgbaColor([...tint.slice(0,3),0]));
                context.fillStyle=g;context.fillRect(p.x-64,p.y-64,128,128);
            }else if(effect==='beam'){
                // Taper a trailing beam along the silhouette, with a bright moving head.
                context.lineCap='round';context.lineJoin='round';context.strokeStyle=rgbaColor(tint);context.shadowColor=rgbaColor(tint);context.shadowBlur=12;
                const baseAlpha=context.globalAlpha;
                for(let i=0;i<32;i++){
                    const a=point(sample.phase-(32-i)/200),b=point(sample.phase-(31-i)/200);
                    context.globalAlpha=baseAlpha*((i+1)/32)**2;context.lineWidth=2+2*(i+1)/32;
                    context.beginPath();context.moveTo(a.x,a.y);context.lineTo(b.x,b.y);context.stroke();
                }
                const head=point(sample.phase);context.globalAlpha=baseAlpha;context.fillStyle='#fff';context.beginPath();context.arc(head.x,head.y,2,0,Math.PI*2);context.fill();
            }else{
                paintCinematicMotion(context,{effect,phase:sample.phase,tint,accent:themeColor(animation.secondaryColor??'#7adfff',[122,223,255,255]),point,intensity:sample.intensity});
            }
        }finally{context.restore();}
    }
}

const fract=n=>n-Math.floor(n);
const noise=n=>fract(Math.sin(n*127.1+311.7)*43758.5453123);
const blend=(a,b,t)=>a.map((v,i)=>Math.round(v+(b[i]-v)*t));
/** All motion is deterministic, bounded and drawn outside the content clip. */
function paintCinematicMotion(c,{effect,phase,tint,accent,point,intensity}){
    const baseAlpha=c.globalAlpha;
    const at=(t,distance=0)=>{
        const p=point(t),a=point(t-0.0001),b=point(t+0.0001),length=Math.hypot(b.x-a.x,b.y-a.y)||1;
        const nx=(b.y-a.y)/length,ny=-(b.x-a.x)/length;
        return {...p,x:p.x+nx*distance,y:p.y+ny*distance,nx,ny};
    };
    const bloom=(p,radius,color,alpha)=>{
        const g=c.createRadialGradient(p.x,p.y,0,p.x,p.y,radius);
        g.addColorStop(0,rgbaColor(color));g.addColorStop(0.18,rgbaColor([...color.slice(0,3),Math.round(color[3]*0.55)]));g.addColorStop(1,rgbaColor([...color.slice(0,3),0]));
        c.globalAlpha=baseAlpha*alpha;c.fillStyle=g;c.fillRect(p.x-radius,p.y-radius,radius*2,radius*2);
    };
    const line=(points,color,width,alpha)=>{
        c.beginPath();points.forEach((p,i)=>i?c.lineTo(p.x,p.y):c.moveTo(p.x,p.y));
        c.lineWidth=width;c.strokeStyle=rgbaColor(color);c.globalAlpha=baseAlpha*alpha;c.stroke();
    };
    const trail=(head,length,color,strength=1)=>{
        const count=28;c.lineCap='butt';
        for(let i=0;i<count;i++){
            const t=(i+1)/count,p=point(head-length+i/count*length),q=point(head-length+t*length),power=t*t*strength;
            line([p,q],color,11,0.075*power);line([p,q],color,4,0.55*power);line([p,q],blend(color,[255,255,255,color[3]],t*0.7),1.1,power);
        }
        c.lineCap='round';const p=point(head);bloom(p,30,color,0.75*strength);
        c.globalAlpha=baseAlpha*strength;c.fillStyle=rgbaColor(blend(color,[255,255,255,color[3]],0.85));c.beginPath();c.arc(p.x,p.y,1.8,0,Math.PI*2);c.fill();
    };
    c.save();
    try{
        c.globalCompositeOperation='lighter';c.lineCap='round';c.lineJoin='round';
        if(effect==='comet'){
            trail(phase,0.24,tint);
            for(let i=0;i<9;i++){
                const age=(i+1)/10,p=at(phase-age*0.23,2+noise(i+11)*age*18);
                c.globalAlpha=baseAlpha*(1-age)**2;c.fillStyle=rgbaColor(tint);c.beginPath();c.arc(p.x,p.y,0.5+noise(i+8),0,Math.PI*2);c.fill();
            }
        }else if(effect==='orbit'){
            trail(phase,0.13,tint,0.95);trail(0.5-phase,-0.13,accent,0.8);
        }else if(effect==='aurora'){
            // Overlapping pools form broad continuous curtains, rather than a
            // necklace of separate lights. The second curtain moves oppositely.
            for(let i=0;i<8;i++){
                const t=i/7,offset=(t-0.5)*0.22,wave=0.5+0.5*Math.sin(phase*Math.PI*2+t*3),envelope=Math.sin(Math.PI*t)**2;
                bloom(at(phase+offset,10),64+wave*12,blend(tint,accent,t),0.18*envelope);
                bloom(at(0.5-phase-offset,8),56+wave*10,blend(accent,tint,t),0.13*envelope);
            }
        }else if(effect==='prism'){
            const points=Array.from({length:128},(_,i)=>point(i/128));
            const xs=points.map(p=>p.x),ys=points.map(p=>p.y),left=Math.min(...xs),right=Math.max(...xs),top=Math.min(...ys),bottom=Math.max(...ys);
            const cx=(left+right)/2,cy=(top+bottom)/2,angle=phase*Math.PI*2,extent=Math.hypot(right-left,bottom-top)/2,dx=Math.cos(angle)*extent,dy=Math.sin(angle)*extent;
            const g=c.createLinearGradient(cx-dx,cy-dy,cx+dx,cy+dy);
            for(const [position,color,alpha]of [[0,tint,0],[0.2,tint,0.05],[0.43,tint,0.9],[0.57,accent,0.9],[0.8,accent,0.05],[1,accent,0]])g.addColorStop(position,rgbaColor([...color.slice(0,3),Math.round(color[3]*alpha)]));
            c.beginPath();points.forEach((p,i)=>i?c.lineTo(p.x,p.y):c.moveTo(p.x,p.y));c.closePath();c.strokeStyle=g;
            c.lineWidth=13;c.globalAlpha=baseAlpha*0.13;c.stroke();c.lineWidth=3;c.globalAlpha=baseAlpha*0.9;c.stroke();
            bloom(point(phase),40,tint,0.45);bloom(point(0.5-phase),40,accent,0.4);
        }else if(effect==='stardust'){
            for(let i=0;i<24;i++){
                const age=fract(phase+noise(i+41)),alpha=Math.sin(Math.PI*age)**2,color=blend(tint,accent,noise(i+31)),p=at(noise(i+19),3+44*age*age);
                p.x+=p.ny*Math.sin(age*Math.PI)*12;p.y-=p.nx*Math.sin(age*Math.PI)*12;
                bloom(p,4+4*noise(i+91),color,alpha*0.6);
                const tail={x:p.x-p.nx*age*7,y:p.y-p.ny*age*7};
                line([tail,p],blend(color,[255,255,255,color[3]],0.5),0.7+noise(i+61),alpha);
            }
        }else if(effect==='electric'){
            // One coherent discharge geometry per burst. Fractal branches persist
            // through a fast strike, decay and smaller restrike instead of jittering.
            const burst=Math.floor(phase*7),age=fract(phase*7),envelope=Math.min(1,Math.exp(-age*5)*(0.3+0.7*Math.min(1,age/0.06))+0.5*Math.exp(-(((age-0.5)/0.08)**2)));
            const seed=burst*83+17,start=noise(seed),span=0.10+noise(seed+1)*0.11,offsets=Array(33).fill(0);
            const divide=(a,b,amplitude,depth)=>{
                if(b-a<2)return;const m=(a+b)>>1;
                offsets[m]=(offsets[a]+offsets[b])/2+(noise(seed+m*13+depth*7)-0.5)*amplitude;
                divide(a,m,amplitude*0.56,depth+1);divide(m,b,amplitude*0.56,depth+1);
            };
            divide(0,32,28+intensity*12,0);
            const points=offsets.map((offset,i)=>at(start+span*i/32,Math.sin(i/32*Math.PI)*18+offset));
            const color=blend(tint,accent,0.35);
            const bolt=(points,strength,width)=>{
                c.shadowColor=rgbaColor(color);c.shadowBlur=14;
                line(points,color,width*5,envelope*strength*0.14);c.shadowBlur=0;
                line(points,color,width*2.1,envelope*strength*0.9);
                line(points,blend(color,[255,255,255,color[3]],0.9),width*0.65,envelope*strength);
            };
            bolt(points,1,1.4);
            for(let branch=0;branch<3;branch++){
                const index=8+branch*7,root=points[index],distance=15+noise(seed+branch+9)*23,branchPoints=[root];
                for(let j=1;j<=8;j++){
                    const t=j/8,p=at(start+span*index/32+t*(noise(seed+branch+12)-0.5)*0.05,distance*t+Math.sin(t*Math.PI)*(noise(seed+j+branch*9)-0.5)*12);
                    branchPoints.push({x:root.x+(p.x-points[index].x)*t,y:root.y+(p.y-points[index].y)*t});
                }
                bolt(branchPoints,0.55,0.8);
            }
            bloom(points[0],17,color,envelope*0.4);bloom(points[32],17,color,envelope*0.4);
        }
    }finally{c.restore();}
}
