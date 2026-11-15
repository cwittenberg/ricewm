import * as N from './vendor/three/three.tsl.js';

const coverage=N.wgslFn(`
fn riceBoxCoverage(p:vec2f, box:vec4f, sigma:f32)->f32 {
 if(box.z<=0.0||box.w<=0.0){return 0.0;}
 if(sigma<=0.0){return select(0.0,1.0,all(p>=box.xy)&&all(p<box.xy+box.zw));}
 let scale=sigma*1.4142135623730951;
 let x=clamp((riceErf((box.x+box.z-p.x)/scale)-riceErf((box.x-p.x)/scale))*0.5,0.0,1.0);
 let y=clamp((riceErf((box.y+box.w-p.y)/scale)-riceErf((box.y-p.y)/scale))*0.5,0.0,1.0);
 return x*y;
}fn riceErf(value:f32)->f32 {
 let x=abs(value);let t=1.0/(1.0+0.3275911*x);
 return sign(value)*(1.0-(((((1.061405429*t-1.453152027)*t+1.421413741)*t-0.284496736)*t+0.254829592)*t)*exp(-x*x));
}
`);

/** Native BoxShadow semantics with fragment-side Gaussian integration. */
export function createGameGpuShadow(T){
 const bounds=N.uniform(new T.Vector4()),box=N.uniform(new T.Vector4()),sigma=N.uniform(0),inset=N.uniform(0),color=N.uniform(new T.Vector4());
 const p=N.uv().mul(bounds.zw).add(bounds.xy),amount=coverage({p,box,sigma});
 const alpha=N.select(inset.greaterThan(.5),N.float(1).sub(amount),amount);
 return {fragment:N.vec4(color.rgb,color.a.mul(alpha)),update(d){
  const {width,height,offset,blur,spread,rgba,alpha=1,inside}=d;
  const padding=blur*4,extent=inside?[0,0,width,height]:[offset[0]-spread-padding,offset[1]-spread-padding,width+spread*2+padding*2,height+spread*2+padding*2];
  bounds.value.set(...extent);const amount=inside?-spread:spread;
  box.value.set(offset[0]-amount,offset[1]-amount,width+amount*2,height+amount*2);sigma.value=blur;inset.value=inside?1:0;
  color.value.set((rgba&255)/255,((rgba>>>8)&255)/255,((rgba>>>16)&255)/255,(rgba>>>24)/255*alpha);
  return extent;
 }};
}
