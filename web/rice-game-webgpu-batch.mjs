import * as N from './vendor/three/three.tsl.js';

/** Contiguous sprite/rectangle batch. Per-instance attributes carry the same
 * native transform, encoded colour/opacity and atlas UVs as individual draws. */
export class GameWebGpuQuadBatch {
 constructor(T){this.T=T;this.mesh=null;this.capacity=0;this.material=null;this.sample=null;this.textured=null;this.count=0;}
 prepare(texture,clip,scene,index){
  const T=this.T,textured=Boolean(texture);
  if(!this.material||this.textured!==textured){
   this.material?.dispose();this.textured=textured;
   const tint=N.attribute('riceGameTint','vec4'),region=N.attribute('riceGameUv','vec4'),material=new T.MeshBasicNodeMaterial({transparent:true,depthTest:false,depthWrite:false,side:T.DoubleSide,forceSinglePass:true,toneMapped:false});
   if(textured){this.sample=N.texture(texture,N.uv().mul(region.zw.sub(region.xy)).add(region.xy));material.fragmentNode=N.vec4(this.sample.rgb.mul(tint.rgb),this.sample.a.mul(tint.a));}
   else{this.sample=null;material.fragmentNode=tint;}
   material.stencilWrite=true;material.stencilFunc=T.EqualStencilFunc;material.stencilFuncMask=255;material.stencilFail=T.KeepStencilOp;material.stencilZFail=T.KeepStencilOp;material.stencilZPass=T.KeepStencilOp;this.material=material;
  }
  if(this.sample)this.sample.value=texture;this.material.stencilRef=clip;
  this.texture=texture;this.clip=clip;this.scene=scene;this.index=index;this.count=0;
  if(this.mesh){this.mesh.material=this.material;if(this.mesh.parent!==scene)scene.add(this.mesh);this.mesh.visible=true;this.mesh.userData.gameDrawIndex=index;}
 }
 grow(){
  const T=this.T,capacity=Math.min(8192,Math.max(16,this.capacity*2)),old=this.mesh;
  if(capacity<=this.capacity)throw RangeError('Game quad batch exceeds its instance budget');
  const geometry=new T.BufferGeometry();geometry.setAttribute('position',new T.Float32BufferAttribute([0,0,0,1,0,0,0,1,0,0,1,0,1,0,0,1,1,0],3));geometry.setAttribute('uv',new T.Float32BufferAttribute([0,0,1,0,0,1,0,1,1,0,1,1],2));
  const tint=new T.InstancedBufferAttribute(new Float32Array(capacity*4),4),region=new T.InstancedBufferAttribute(new Float32Array(capacity*4),4);tint.setUsage(T.DynamicDrawUsage);region.setUsage(T.DynamicDrawUsage);geometry.setAttribute('riceGameTint',tint);geometry.setAttribute('riceGameUv',region);
  const mesh=new T.InstancedMesh(geometry,this.material,capacity);mesh.instanceMatrix.setUsage(T.DynamicDrawUsage);mesh.frustumCulled=false;mesh.matrixAutoUpdate=false;mesh.userData.gameDrawIndex=this.index;mesh.visible=true;
  if(old){mesh.instanceMatrix.array.set(old.instanceMatrix.array);tint.array.set(old.geometry.attributes.riceGameTint.array);region.array.set(old.geometry.attributes.riceGameUv.array);old.removeFromParent();old.geometry.dispose();old.dispose();this.material.needsUpdate=true;}
  this.capacity=capacity;this.mesh=mesh;this.scene.add(mesh);
 }
 append(matrix,color,alpha,uv){
  if(this.count>=this.capacity)this.grow();const i=this.count++,m=this.mesh.instanceMatrix.array,o=i*16;
  m.set([matrix[0],matrix[1],0,0,matrix[2],matrix[3],0,0,0,0,1,0,matrix[4],matrix[5],0,1],o);
  this.mesh.geometry.attributes.riceGameTint.array.set([(color&255)/255,((color>>>8)&255)/255,((color>>>16)&255)/255,((color>>>24)/255)*alpha],i*4);this.mesh.geometry.attributes.riceGameUv.array.set(uv,i*4);return this.mesh;
 }
 finish(){if(!this.mesh)return;this.mesh.count=this.count;this.mesh.instanceMatrix.needsUpdate=true;this.mesh.geometry.attributes.riceGameTint.needsUpdate=true;this.mesh.geometry.attributes.riceGameUv.needsUpdate=true;}
 hide(){if(this.mesh)this.mesh.visible=false;}
 close(){if(this.mesh){this.mesh.removeFromParent();this.mesh.geometry.dispose();this.mesh.dispose();}this.material?.dispose();this.mesh=null;this.material=null;}
}
