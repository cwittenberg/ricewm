import * as N from './vendor/three/three.tsl.js';

const radialPosition=N.wgslFn(`
fn riceRadialPosition(p:vec2f, a:vec3f, b:vec3f)->f32 {
 let q=p-a.xy;let d=b.xy-a.xy;let r=b.z-a.z;
 let aa=dot(d,d)-r*r;let bb=-2.0*(dot(q,d)+a.z*r);let cc=dot(q,q)-a.z*a.z;
 if(abs(aa)<0.000001){
  if(abs(bb)<0.000001){return -1e20;}
  let t=-cc/bb;if(a.z+t*r>=0.0){return t;}return -1e20;
 }
 let discriminant=bb*bb-4.0*aa*cc;if(discriminant<0.0){return -1e20;}
 let root=sqrt(discriminant);let t0=(-bb-root)/(2.0*aa);let t1=(-bb+root)/(2.0*aa);
 let v0=a.z+t0*r>=0.0;let v1=a.z+t1*r>=0.0;
 if(v0&&v1){return max(t0,t1);}if(v0){return t0;}if(v1){return t1;}return -1e20;
}`);
const sweepPosition=N.wgslFn(`
fn riceSweepPosition(p:vec2f, a:vec4f, extend:f32)->f32 {
 let tau=6.283185307179586;var angle=atan2(p.y-a.y,p.x-a.x)-a.z;
 angle=angle-floor(angle/tau)*tau;let span=max(0.0000001,a.w-a.z);let t=angle/span;
 if(span<tau&&extend>0.5){
  if(extend>1.5){return 1.0-abs((t-floor(t/2.0)*2.0)-1.0);}
  return fract(t);
 }return t;
}`);

/** Fixed shader family; changing gradient positions/colours updates uniforms
 * without generating a new shader on each game animation frame. */
export function createGameGpuGradient(T,kind,count){
    if(![6,25,26].includes(kind)||!Number.isInteger(count)||count<2||count>64)throw RangeError('Invalid bounded game gradient');
    const bounds=N.uniform(new T.Vector4()),a=N.uniform(new T.Vector4()),b=N.uniform(new T.Vector4()),extend=N.uniform(0),opacity=N.uniform(1);
    const colors=Array.from({length:count},()=>N.uniform(new T.Vector4())),positions=Array.from({length:count},()=>N.uniform(0));
    const point=N.uv().mul(bounds.zw).add(bounds.xy);let t;
    if(kind===6){const direction=b.xy.sub(a.xy);t=point.sub(a.xy).dot(direction).div(direction.dot(direction).max(.0000001));}
    else if(kind===25)t=radialPosition({p:point,a:a.xyz,b:b.xyz});
    else t=sweepPosition({p:point,a,extend});
    let result=colors[0];
    for(let i=1;i<count;i++){
        const delta=positions[i].sub(positions[i-1]);
        const weight=N.select(delta.greaterThan(0),t.sub(positions[i-1]).div(delta.max(.0000001)).clamp(0,1),N.step(positions[i],t));
        result=N.mix(result,colors[i],weight);
    }
    let fragment=N.vec4(result.rgb.div(result.a.max(.0000001)),result.a.mul(opacity));
    if(kind===25)fragment=N.select(t.greaterThan(-1e10),fragment,N.vec4(0));
    return {fragment,update(descriptor){
        const {rect,from,to,stops,alpha=1,extension=0}=descriptor;
        if(stops.length!==count||![...rect,...from,...to,alpha,extension].every(Number.isFinite))throw Error('Invalid game gradient uniforms');
        bounds.value.set(...rect);a.value.set(from[0],from[1],from[2]??0,from[3]??0);b.value.set(to[0]??0,to[1]??0,to[2]??0,to[3]??0);opacity.value=alpha;extend.value=extension;
        for(let i=0;i<count;i++){
            const [at,color]=stops[i],alpha=(color>>>24)/255;positions[i].value=at;
            colors[i].value.set((color&255)/255*alpha,((color>>>8)&255)/255*alpha,((color>>>16)&255)/255*alpha,alpha);
        }
    }};
}
