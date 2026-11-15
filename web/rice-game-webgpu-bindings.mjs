/** Pinned Three 0.180.0 does not retire uniform buffers with render objects.
 * Keep their lifetime explicit on this owned renderer. Shared bindings are
 * reference counted; geometry/storage buffers keep Three's existing ownership.
 */
export function manageGameWebGpuBindings(renderer){
 const bindings=renderer._bindings,backend=renderer.backend,objects=new WeakMap(),groups=new Map(),uniforms=new Map();
 const get=bindings.getForRender;
 bindings.getForRender=function(object){
  let record=objects.get(object);
  if(!record){
   record={groups:new Set(),uniforms:new Set(),closed:false};objects.set(object,record);
   const dispose=object.onDispose;
   object.onDispose=()=>{
    if(record.closed)return;record.closed=true;
    try{dispose.call(object);}finally{
     for(const group of record.groups){const count=groups.get(group)-1;if(count)groups.set(group,count);else{groups.delete(group);bindings.delete(group);backend.delete(group);}}
     for(const uniform of record.uniforms){const count=uniforms.get(uniform)-1;if(count)uniforms.set(uniform,count);else{uniforms.delete(uniform);backend.get(uniform).buffer?.destroy();backend.delete(uniform);}}
     record.groups.clear();record.uniforms.clear();
    }
   };
  }
  try{return get.call(this,object);}finally{
   // Even a partially allocated bind group belongs to the failed render object.
   for(const group of object._bindings||[]){
    if(!record.groups.has(group)){record.groups.add(group);groups.set(group,(groups.get(group)||0)+1);}
    for(const uniform of group.bindings)if(uniform.isUniformBuffer&&!record.uniforms.has(uniform)){record.uniforms.add(uniform);uniforms.set(uniform,(uniforms.get(uniform)||0)+1);}
   }
  }
 };
 // An admission failure can leave a Three attribute without a native buffer.
 // Its cleanup must still reach all the other successfully allocated resources.
 const destroy=backend.destroyAttribute;
 backend.destroyAttribute=function(attribute){const key=this.attributeUtils._getBufferAttribute(attribute);if(this.get(key).buffer)return destroy.call(this,attribute);this.delete(attribute);};
}
