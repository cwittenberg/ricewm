/** Portable authoring descriptor. Source is data until qualified and compiled. */
export const EFFECT_SCHEMA='rice.effect/v1';
export const EFFECT_TARGETS=Object.freeze(['background','window-frame','shell','widget','world-material']);
export const EFFECT_LIMITS=Object.freeze({sourceBytes:65536,functions:32,iterations:64,work:8192,passes:4,layers:3,pixels:2073600,textureBytes:64*1024*1024});
const record=v=>v&&typeof v==='object'&&!Array.isArray(v);
const keys=(v,allowed,label)=>{if(!record(v)||Object.keys(v).some(k=>!allowed.includes(k)))throw Error(`Unsupported ${label} fields`);};
const text=(v,max)=>typeof v==='string'&&v.trim()&&v.length<=max;
const finite=(v,min,max)=>Number.isFinite(v)&&v>=min&&v<=max;
export function parseRiceEffect(input){
 const v=typeof input==='string'?JSON.parse(input):structuredClone(input);
 keys(v,['schema','id','name','description','targets','implementations','parameters','values','animated','frameRate','scale','fallback','passes','inputs'],'effect');
 if(v.schema!==EFFECT_SCHEMA||!text(v.id,80)||! /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/.test(v.id)||!text(v.name,96))throw Error('Invalid effect identity');
 if(v.description!==undefined&&(typeof v.description!=='string'||v.description.length>2048))throw Error('Effect description is too long');
 if(!Array.isArray(v.targets)||!v.targets.length||v.targets.length>5||new Set(v.targets).size!==v.targets.length||v.targets.some(t=>!EFFECT_TARGETS.includes(t)))throw Error('Invalid effect targets');
 keys(v.implementations,['glsl','wgsl'],'shader implementations');
 if(!Object.keys(v.implementations).length)throw Error('Provide GLSL or WGSL source');
 for(const source of Object.values(v.implementations))if(typeof source!=='string'||!source.trim()||new TextEncoder().encode(source).length>EFFECT_LIMITS.sourceBytes)throw Error('Shader source must be between 1 byte and 64 KiB');
 v.parameters??=[];v.values??=[];
 if(!Array.isArray(v.parameters)||v.parameters.length>4||!Array.isArray(v.values)||v.values.length>4)throw Error('Effects support up to four parameters');
 const seen=new Set();
 v.parameters.forEach((p,i)=>{keys(p,['name','label','min','max','step','default'],'parameter');if(!text(p.name,32)||seen.has(p.name)||!text(p.label,80)||!finite(p.min,-10000,10000)||!finite(p.max,p.min+Number.EPSILON,10000)||!finite(p.default,p.min,p.max)||p.step!==undefined&&!finite(p.step,0.000001,p.max-p.min))throw Error('Invalid effect parameter');seen.add(p.name);v.values[i]??=p.default;if(!finite(v.values[i],p.min,p.max))throw Error(`Invalid value for ${p.label}`);});
 if(v.values.length>v.parameters.length)throw Error('Effect values have no matching parameter');
 v.animated??=false;v.frameRate??=30;v.scale??=0.5;v.passes??=1;v.inputs??=[];
 if(typeof v.animated!=='boolean'||!Number.isInteger(v.frameRate)||!finite(v.frameRate,15,60)||!finite(v.scale,0.25,1)||!Number.isInteger(v.passes)||!finite(v.passes,1,EFFECT_LIMITS.passes))throw Error('Invalid effect rendering budget');
 if(!Array.isArray(v.inputs)||new Set(v.inputs).size!==v.inputs.length||v.inputs.some(i=>!['pointer','audio','session'].includes(i)))throw Error('Unsupported effect input');
 if(typeof v.fallback!=='string'||!/^#[a-f\d]{6}$/i.test(v.fallback))throw Error('Provide a six-digit fallback color');
 return v;
}
export const RICE_EFFECT_TEMPLATES=Object.freeze([
 {id:'aurora',name:'Aurora',description:'A layered, animated color field. Change the source to make it your own.',glsl:`vec4 riceEffect(vec2 uv, float time, vec4 params) {
   float wave=sin(uv.x*params.x+time*0.4)*0.18;
   float glow=exp(-abs(uv.y-0.5-wave)*params.y);
   vec3 color=mix(vec3(0.02,0.04,0.12),vec3(0.18,0.8,0.65),glow);
   return vec4(color,1.0);
 }`,wgsl:`fn riceEffect(uv: vec2f, time: f32, params: vec4f) -> vec4f {
   let wave=sin(uv.x*params.x+time*0.4)*0.18;
   let glow=exp(-abs(uv.y-0.5-wave)*params.y);
   let color=mix(vec3f(0.02,0.04,0.12),vec3f(0.18,0.8,0.65),glow);
   return vec4f(color,1.0);
 }`,parameters:[{name:'frequency',label:'Wave frequency',min:1,max:20,default:8,step:0.1},{name:'glow',label:'Glow falloff',min:1,max:40,default:12,step:0.1}]},
 {id:'sculpture',name:'3D sculpture',description:'A shaded sphere with animated surface detail, written entirely in shader code.',glsl:`vec4 riceEffect(vec2 uv, float time, vec4 params) {
   vec2 p=(uv-0.5)*2.0; float r=dot(p,p);
   if(r>0.65) return vec4(0.03,0.04,0.08,1.0);
   vec3 n=normalize(vec3(p,sqrt(0.65-r)));
   float light=max(0.0,dot(n,normalize(vec3(-0.4,0.6,1.0))));
   float bands=0.5+0.5*sin(n.y*params.x+time);
   return vec4(mix(vec3(0.15,0.1,0.5),vec3(0.9,0.4,0.2),bands)*(0.2+light),1.0);
 }`,wgsl:`fn riceEffect(uv: vec2f, time: f32, params: vec4f) -> vec4f {
   let p=(uv-0.5)*2.0; let r=dot(p,p);
   if(r>0.65) { return vec4f(0.03,0.04,0.08,1.0); }
   let n=normalize(vec3f(p,sqrt(0.65-r)));
   let light=max(0.0,dot(n,normalize(vec3f(-0.4,0.6,1.0))));
   let bands=0.5+0.5*sin(n.y*params.x+time);
   return vec4f(mix(vec3f(0.15,0.1,0.5),vec3f(0.9,0.4,0.2),bands)*(0.2+light),1.0);
 }`,parameters:[{name:'bands',label:'Surface bands',min:1,max:40,default:12,step:0.1}]},
 {id:'contours',name:'Contour lines',description:'A reusable translucent treatment for frames, widgets and shell surfaces.',glsl:`vec4 riceEffect(vec2 uv, float time, vec4 params) {
   float line=smoothstep(0.85,1.0,sin((uv.x+uv.y)*params.x+time));
   return vec4(0.2,0.7,1.0,line*params.y);
 }`,wgsl:`fn riceEffect(uv: vec2f, time: f32, params: vec4f) -> vec4f {
   let line=smoothstep(0.85,1.0,sin((uv.x+uv.y)*params.x+time));
   return vec4f(0.2,0.7,1.0,line*params.y);
 }`,parameters:[{name:'density',label:'Line density',min:1,max:80,default:30,step:1},{name:'opacity',label:'Opacity',min:0,max:1,default:0.35,step:0.01}]}
]);
export function riceEffectTemplate(template='aurora',id=crypto.randomUUID()){
 const t=RICE_EFFECT_TEMPLATES.find(t=>t.id===template);if(!t)throw Error('Unknown effect template');
 return parseRiceEffect({schema:EFFECT_SCHEMA,id,name:t.name,description:t.description,targets:template==='contours'?['window-frame','shell','widget']:['background'],implementations:{glsl:t.glsl,wgsl:t.wgsl},parameters:t.parameters,animated:true,fallback:'#101525'});
}
