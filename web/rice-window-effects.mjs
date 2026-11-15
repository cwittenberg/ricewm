/** Portable, opt-in window halos. No scripts, URLs or input surfaces. */
export const WINDOW_EFFECT_DEFAULT_DURATIONS=Object.freeze({none:2600,breathe:2600,flow:2600,electric:3200,beam:2600,custom:2600,comet:2600,prism:6000,stardust:5200});
export const windowEffectDefaultDuration=preset=>WINDOW_EFFECT_DEFAULT_DURATIONS[preset]??({aurora:7200,orbit:4800}[preset])??2600;
export const WINDOW_EFFECTS_SCHEMA = Object.freeze({
    writeTo:'bundle.windowEffects', applyTo:['all','active','per-state'], animationApplyTo:['active','all'], states:['active','inactive'], maxLayers:6,
    layer:{color:'#RRGGBB, #RRGGBBAA or transparent (or use gradient)',gradient:'{angle:-360..360,stops:2..8 [{at:0..1,color}]}',blur:{min:0,max:128},spread:{min:0,max:24},x:{min:-64,max:64},y:{min:-64,max:64}},
    rim:{color:'color (or use gradient)',gradient:'same as layer',width:{min:0,max:12}},
    animationDefaults:{durationByPreset:WINDOW_EFFECT_DEFAULT_DURATIONS,secondaryColor:'#7adfff'},
    animation:{preset:['none','breathe','flow','electric','beam','custom','comet','prism','stardust'],duration:{min:400,max:20000,default:2600},intensity:{min:0,max:1,default:0.65},direction:['clockwise','counterclockwise'],phase:{min:0,max:1,default:0},color:'optional highlight color',secondaryColor:'optional accent highlight color; default #7adfff',effect:['none','beam','flow','electric','comet','prism','stardust'],keyframes:{min:2,max:32,at:'strictly increasing 0..1; first 0, last 1',alpha:'0..1, default 1',phase:'0..8 rim laps, default at',intensity:'0..1, default animation intensity',color:'optional highlight color',easing:['linear','ease-in','ease-out','ease-in-out','hold']}},
    semantics:'applyTo:all uses the active style on every eligible window, active limits glow to the focused window, and per-state (the default when omitted) keeps separate active/inactive styles. animationApplyTo:active keeps inactive halos static; all (the default when omitted) animates each window with its own stable phase. Beam is a traveling luminous streak. Custom sequences interpolate bounded keyframes over duration, repeating each cycle; each segment uses its starting keyframe easing. Alpha changes the whole halo, phase moves the highlight around the rim, intensity and color change the highlight. Effect selects any moving preset or none. Comet has a long luminous tail, Prism has sweeping two-color bands, Stardust has drifting particles, and Electric has intermittent branching discharges. secondaryColor controls the accent color. Omitted keyframe properties use the documented defaults; no executable code or URLs are allowed. Legacy presets remain supported. Each supplied state replaces its legacy shadow. Omitted states retain existing behavior. Empty layers clear the halo. Effects follow the complete window silhouette, outside content and hit targets; maximized/fullscreen windows omit them. The active_shadow setting controls only the legacy shadow. Use sdk.screenshot or visual.capture on a real app; theme.visual excludes applications.',
});
const object=v=>v&&typeof v==='object'&&!Array.isArray(v);
const color=v=>typeof v==='string'&&/^(#[\da-f]{6}(?:[\da-f]{2})?|transparent)$/i.test(v);
const bounded=(v,min,max)=>Number.isFinite(v)&&v>=min&&v<=max;
function paint(value,keys){
    if(!object(value)||Object.keys(value).some(k=>!keys.includes(k))||((value.color!==undefined)===(value.gradient!==undefined)))throw TypeError('Window effect needs one color or gradient');
    if(value.color!==undefined&&!color(value.color))throw TypeError('Invalid window effect color');
    if(value.gradient!==undefined){const g=value.gradient;
        if(!object(g)||Object.keys(g).some(k=>!['angle','stops'].includes(k))||!bounded(g.angle??90,-360,360)||!Array.isArray(g.stops)||g.stops.length<2||g.stops.length>8||g.stops.some((s,i)=>!object(s)||Object.keys(s).some(k=>!['at','color'].includes(k))||!bounded(s.at,0,1)||!color(s.color)||(i>0&&s.at<g.stops[i-1].at)))throw TypeError('Invalid window effect gradient');
    }
}
export function validateWindowEffects(value){
    if(!object(value)||Object.keys(value).some(k=>!['applyTo','animationApplyTo'].includes(k)&&!WINDOW_EFFECTS_SCHEMA.states.includes(k))||(value.applyTo!==undefined&&!WINDOW_EFFECTS_SCHEMA.applyTo.includes(value.applyTo))||(value.animationApplyTo!==undefined&&!WINDOW_EFFECTS_SCHEMA.animationApplyTo.includes(value.animationApplyTo)))throw TypeError('Invalid window effect state');
    for(const name of WINDOW_EFFECTS_SCHEMA.states){
        const state=value[name];if(state===undefined)continue;
        if(!object(state)||Object.keys(state).some(k=>!['layers','rim','animation'].includes(k))||!Array.isArray(state.layers??[])||(state.layers?.length??0)>6)throw TypeError('Window effects support up to six layers');
        for(const layer of state.layers??[]){
            paint(layer,['color','gradient','blur','spread','x','y']);
            for(const [key,rule]of Object.entries(WINDOW_EFFECTS_SCHEMA.layer))if(object(rule)&&!bounded(layer[key]??0,rule.min,rule.max))throw TypeError('Invalid window effect '+key);
        }
        if(state.rim!==undefined){paint(state.rim,['color','gradient','width']);if(!bounded(state.rim.width??1,0,12))throw TypeError('Invalid window effect rim');}
        if(state.animation!==undefined){const a=state.animation,schema=WINDOW_EFFECTS_SCHEMA.animation;
            if(!object(a)||Object.keys(a).some(k=>!Object.hasOwn(schema,k))||!schema.preset.includes(a.preset)&&!['aurora','orbit'].includes(a.preset)||!bounded(a.duration??2600,400,20000)||!bounded(a.intensity??0.65,0,1)||!bounded(a.phase??0,0,1)||(a.direction!==undefined&&!schema.direction.includes(a.direction))||(a.color!==undefined&&!color(a.color))||(a.secondaryColor!==undefined&&!color(a.secondaryColor)))throw TypeError('Invalid window effect animation');
            if(a.effect!==undefined&&!schema.effect.includes(a.effect)&&!['aurora','orbit'].includes(a.effect))throw TypeError('Invalid custom window effect');
            if(a.preset==='custom'||a.keyframes!==undefined){
                const frames=a.keyframes;
                if(!Array.isArray(frames)||frames.length<2||frames.length>32||frames[0]?.at!==0||frames.at(-1)?.at!==1||frames.some((f,i)=>!object(f)||Object.keys(f).some(k=>!['at','alpha','phase','intensity','color','easing'].includes(k))||!bounded(f.at,0,1)||(i>0&&f.at<=frames[i-1].at)||(f.alpha!==undefined&&!bounded(f.alpha,0,1))||(f.phase!==undefined&&!bounded(f.phase,0,8))||(f.intensity!==undefined&&!bounded(f.intensity,0,1))||(f.color!==undefined&&!color(f.color))||(f.easing!==undefined&&!schema.keyframes.easing.includes(f.easing))))throw TypeError('Custom glow needs 2–32 ordered keyframes from at:0 to at:1');
            }
        }
    }
    return structuredClone(value);
}
/** Identity comes from the window owner and window handle, never its title, position or focus. */
export function windowEffectPhase(identity){let hash=2166136261;for(const c of String(identity))hash=Math.imul(hash^c.codePointAt(0),16777619);return (hash>>>0)/4294967296;}
export function windowEffectStyle(chrome,active=true,identity){
    const effects=chrome?.windowEffects;
    const style=effects?.applyTo==='all'?(effects.active??effects.inactive):effects?.applyTo==='active'?(active?effects.active:{layers:[]}):effects?.[active?'active':'inactive'];
    if(!style?.animation)return style;
    if(effects.animationApplyTo==='active'&&!active){const {animation,...staticStyle}=style;return staticStyle;}
    if(identity===undefined)return style;
    return {...style,animation:{...style.animation,phase:((style.animation.phase??0)+windowEffectPhase(identity))%1}};
}
export function windowEffectsOutset(chrome){
    let size=0;
    for(const state of [chrome?.windowEffects?.active,chrome?.windowEffects?.inactive]){
        if(!state)continue;
        size=Math.max(size,state.rim?.width??0);
        for(const layer of state.layers??[])size=Math.max(size,Math.ceil((layer.blur??0)*3+(layer.spread??0)+Math.max(Math.abs(layer.x??0),Math.abs(layer.y??0))+2));
        if(windowEffectAnimated(state)&&['flow','electric','beam','custom','comet','aurora','orbit','prism','stardust'].includes(state.animation.preset))size=Math.max(size,['flow','beam'].includes(state.animation.preset)?72:96);
    }
    return size;
}
export function windowEffectAnimated(style){return !!(style?.animation&&style.animation.preset!=='none'&&(style.animation.intensity??0.65)>0&&((style.layers?.length??0)>0||(style.rim&&(style.rim.width??1)>0)));}
export function windowEffectMotionOff(shell,preview=false){return preview||shell?.motionEnabled===false||globalThis.document?.hidden===true||globalThis.matchMedia?.('(prefers-reduced-motion: reduce)').matches===true;}
/** Deterministic samples make screenshots reproducible and motion testable. */
export function sampleWindowEffect(style,now=0,reduced=false){
    if(reduced||!windowEffectAnimated(style))return {animated:false,alpha:1,phase:0,intensity:0};
    const a=style.animation,intensity=a.intensity??0.65;
    const phase=(((now/(a.duration??windowEffectDefaultDuration(a.preset))+(a.phase??0))%1)+1)%1;
    if(a.preset==='custom'){
        const frames=a.keyframes;let index=0;while(index<frames.length-2&&phase>=frames[index+1].at)index++;
        const from=frames[index],to=frames[index+1];let t=(phase-from.at)/(to.at-from.at);
        switch(from.easing){case 'hold':t=0;break;case 'ease-in':t*=t;break;case 'ease-out':t=1-(1-t)**2;break;case 'ease-in-out':t=t*t*(3-2*t);break;}
        const mix=(x,y)=>x+(y-x)*t;
        let travel=mix(from.phase??from.at,to.phase??to.at)%1;if(a.direction==='counterclockwise')travel=1-travel;
        const rgba=v=>v==='transparent'?[0,0,0,0]:[0,2,4,6].map((n,i)=>i===3&&v.length===7?255:parseInt(v.slice(n+1,n+3),16));
        const left=rgba(from.color??a.color??'#dfc6ff'),right=rgba(to.color??a.color??'#dfc6ff');
        return {animated:true,alpha:mix(from.alpha??1,to.alpha??1),phase:travel,intensity:mix(from.intensity??intensity,to.intensity??intensity),effect:a.effect??'beam',color:'#'+left.map((v,i)=>Math.round(mix(v,right[i])).toString(16).padStart(2,'0')).join('')};
    }
    const wave=0.5-0.5*Math.cos(phase*Math.PI*2);
    const alpha=a.preset==='breathe'?1-intensity*0.55*wave:1;
    return {animated:true,alpha,phase:a.direction==='counterclockwise'?1-phase:phase,intensity};
}
/** Constant-speed traversal of a rounded rectangular rim, starting at top left. */
export function windowEffectPoint(width,height,radius,progress){
    const r=Math.max(0,Math.min(radius,width/2,height/2)),w=width-2*r,h=height-2*r,arc=Math.PI*r/2;
    const lengths=[w,arc,h,arc,w,arc,h,arc];let d=(((progress%1)+1)%1)*(2*w+2*h+4*arc);
    for(let i=0;i<lengths.length;i++){
        if(d>lengths[i]&&i<7){d-=lengths[i];continue;}
        if(i===0)return {x:r+d,y:0};if(i===2)return {x:width,y:r+d};if(i===4)return {x:width-r-d,y:height};if(i===6)return {x:0,y:height-r-d};
        const corner=(i-1)/2,angle=-Math.PI/2+corner*Math.PI/2+(r?d/r:0),cx=corner<2?width-r:r,cy=corner===0||corner===3?r:height-r;
        return {x:cx+Math.cos(angle)*r,y:cy+Math.sin(angle)*r};
    }
}
