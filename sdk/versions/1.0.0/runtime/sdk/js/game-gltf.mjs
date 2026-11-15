// Bounded glTF 2.0 triangle/animation subset. Dependencies come from the caller;
// decoding a document never follows a URI or opens a network connection.
const integer=(n,min,max)=>Number.isSafeInteger(n)&&n>=min&&n<=max;
const array=(value,size)=>Array.isArray(value)&&value.length===size&&value.every(Number.isFinite);
const identity=()=>[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1];
const multiply=(a,b)=>Array.from({length:16},(_,i)=>{const row=i%4,col=Math.floor(i/4);let sum=0;for(let k=0;k<4;k++)sum+=a[k*4+row]*b[col*4+k];return sum;});
function nodeMatrix(node){
  if(node.matrix!==undefined){if(!array(node.matrix,16)||['translation','rotation','scale'].some(k=>node[k]!==undefined))throw Error('Invalid glTF node matrix');return node.matrix;}
  const t=node.translation??[0,0,0],s=node.scale??[1,1,1],q=node.rotation??[0,0,0,1];
  if(!array(t,3)||!array(s,3)||!array(q,4)||Math.abs(Math.hypot(...q)-1)>0.001)throw Error('Invalid glTF node transform');
  const [x,y,z,w]=q;
  return [(1-2*y*y-2*z*z)*s[0],(2*x*y+2*z*w)*s[0],(2*x*z-2*y*w)*s[0],0,(2*x*y-2*z*w)*s[1],(1-2*x*x-2*z*z)*s[1],(2*y*z+2*x*w)*s[1],0,(2*x*z+2*y*w)*s[2],(2*y*z-2*x*w)*s[2],(1-2*x*x-2*y*y)*s[2],0,...t,1];
}
function parseGLTF(document,{buffers=[],maximumTriangles=8192,profile="cpu",time=0,animation=null,loop=true,rigged=false,poseOnly=false,cache=null}={}){
  if(!document||document.asset?.version!=='2.0'||!(["cpu","gpu"].includes(profile))||!integer(maximumTriangles,1,profile==="gpu"?65536:16384)||!Array.isArray(buffers)||buffers.length>32||buffers.some(b=>!(b instanceof Uint8Array))||buffers.reduce((n,b)=>n+b.length,0)>32*1024*1024)throw Error('Invalid bounded glTF document/buffers');
  for(const key of ['accessors','bufferViews','nodes','meshes','materials','images','textures','scenes','skins','animations'])if(document[key]!==undefined&&(!Array.isArray(document[key])||document[key].length>16384))throw Error('glTF table exceeds limit');
  if((document.extensionsRequired??[]).length)throw Error('Required glTF extensions are unsupported');
  if(!Number.isFinite(time)||time<0||typeof loop!=='boolean'||(document.skins??[]).length>128||(document.animations??[]).length>128)throw Error('Invalid bounded glTF animation');
  if(!Array.isArray(document.buffers)||document.buffers.length!==buffers.length||document.buffers.some((b,i)=>!integer(b.byteLength,1,32*1024*1024)||b.byteLength>buffers[i].length||buffers[i].length-b.byteLength>3))throw Error('Missing or mismatched glTF buffers');
  const viewBytes=index=>{
    const view=document.bufferViews?.[index];
    if(!view||!integer(view.buffer,0,buffers.length-1)||!integer(view.byteOffset??0,0,buffers[view.buffer].length)||!integer(view.byteLength,1,buffers[view.buffer].length)||(view.byteOffset??0)+view.byteLength>document.buffers[view.buffer].byteLength)throw Error('glTF bufferView outside buffer');
    return {bytes:buffers[view.buffer].subarray(view.byteOffset??0,(view.byteOffset??0)+view.byteLength),view};
  };
  const accessor=(index,types,components)=>{
    const acc=document.accessors?.[index],spec={5121:[1,'getUint8',255],5123:[2,'getUint16',65535],5125:[4,'getUint32',4294967295],5126:[4,'getFloat32',1]}[acc?.componentType];
    if(!acc||!types.includes(acc.type)||!spec||!integer(acc.count,1,profile==='gpu'?196608:acc.type==='SCALAR'?49152:16384)||acc.sparse||!integer(acc.bufferView,0,(document.bufferViews?.length??0)-1))throw Error('Unsupported glTF accessor');
    const cacheKey=index+':'+types.join(',')+':'+components;if(cache?.accessors.has(cacheKey))return cache.accessors.get(cacheKey);
    const {bytes,view}=viewBytes(acc.bufferView),stride=view.byteStride??spec[0]*components,offset=acc.byteOffset??0;
    if(!integer(stride,spec[0]*components,252)||stride%spec[0]||!integer(offset,0,bytes.length)||offset%spec[0]||offset+(acc.count-1)*stride+spec[0]*components>bytes.length)throw Error('glTF accessor escapes bufferView');
    const data=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),result=[];
    for(let i=0;i<acc.count;i++){
      const values=Array.from({length:components},(_,j)=>data[spec[1]](offset+i*stride+j*spec[0],true)/(acc.normalized?spec[2]:1));
      if(!values.every(Number.isFinite))throw Error('Nonfinite glTF accessor');result.push(values);
    }
    cache?.accessors.set(cacheKey,result);return result;
  };
  const nodes=(document.nodes??[]).map(node=>({...node})),clips=(document.animations??[]).map((clip,index)=>({index,name:clip.name??'clip-'+index}));
  if(animation!==null){
    const clipIndex=typeof animation==='string'?clips.find(c=>c.name===animation)?.index:animation,clip=document.animations?.[clipIndex];
    if(!clip||!Array.isArray(clip.channels)||clip.channels.length>1024||!Array.isArray(clip.samplers)||clip.samplers.length>1024)throw Error('Invalid glTF animation clip');
    let totalKeys=0;
    const tracks=cache?.tracks.get(clipIndex)??clip.channels.map(channel=>{
      const target=channel.target,sampler=clip.samplers[channel.sampler],path=target?.path;
      if(!integer(target?.node,0,nodes.length-1)||!['translation','rotation','scale'].includes(path)||!sampler||!['LINEAR','STEP'].includes(sampler.interpolation??'LINEAR')||nodes[target.node].matrix)throw Error('Unsupported glTF animation channel; only LINEAR/STEP node TRS supported');
      if(document.accessors?.[sampler.input]?.componentType!==5126||document.accessors?.[sampler.output]?.componentType!==5126)throw Error('Animation keyframes must be float');
      const times=accessor(sampler.input,['SCALAR'],1).map(v=>v[0]),values=accessor(sampler.output,[path==='rotation'?'VEC4':'VEC3'],path==='rotation'?4:3);
      totalKeys+=times.length;if(totalKeys>262144)throw Error('Animation keyframe budget exceeded');
      if(values.length!==times.length||times.some((v,i)=>v<0||i>0&&v<=times[i-1]))throw Error('Invalid glTF keyframe times');
      return {target,path,times,values,interpolation:sampler.interpolation??'LINEAR'};
    });
    cache?.tracks.set(clipIndex,tracks);
    const duration=Math.max(0,...tracks.map(t=>t.times.at(-1))),sampleTime=loop&&duration>0?time%duration:Math.min(time,duration),targets=new Set();
    for(const track of tracks){
      const key=track.target.node+':'+track.path;if(targets.has(key))throw Error('Duplicate glTF animation target');targets.add(key);
      let index=0;while(index+1<track.times.length&&track.times[index+1]<=sampleTime)index++;
      const next=Math.min(index+1,track.times.length-1),a=track.values[index],b=track.values[next];
      const factor=track.interpolation==='STEP'||next===index?0:Math.max(0,(sampleTime-track.times[index])/(track.times[next]-track.times[index]));
      let value;
      if(track.path==='rotation'){
        if(Math.abs(Math.hypot(...a)-1)>0.001||Math.abs(Math.hypot(...b)-1)>0.001)throw Error('Invalid animation quaternion');
        let cosine=a.reduce((sum,v,i)=>sum+v*b[i],0),sign=cosine<0?-1:1;cosine=Math.min(1,Math.abs(cosine));
        const angle=Math.acos(cosine),sine=Math.sin(angle),wa=sine>0.0001?Math.sin((1-factor)*angle)/sine:1-factor,wb=sine>0.0001?Math.sin(factor*angle)/sine:factor;
        value=a.map((v,i)=>v*wa+b[i]*sign*wb);const length=Math.hypot(...value);value=value.map(v=>v/length);
      }else value=a.map((v,i)=>v+(b[i]-v)*factor);
      nodes[track.target.node][track.path]=value;
    }
  }
  const textureKey=map=>{if(!map)return undefined;if((map.texCoord??0)!==0||map.extensions)throw Error('Only untransformed TEXCOORD_0 is supported');const source=document.textures?.[map.index]?.source;if(!integer(source,0,(document.images?.length??0)-1))throw Error('Missing glTF physical texture');return 'image-'+source;};
  const materials=Object.create(null),encodedImages=[];
  for(const [i,material]of (poseOnly?[]:document.materials??[]).entries()){
    if(i>=128)throw Error('glTF material budget exceeded');
    if(material.extensions&&Object.keys(material.extensions).length)throw Error('glTF material extensions are unsupported');
    const pbr=material.pbrMetallicRoughness??{};if([pbr.roughnessFactor??1,pbr.metallicFactor??1].some(v=>!Number.isFinite(v)||v<0||v>1)||!array(material.emissiveFactor??[0,0,0],3)||(material.emissiveFactor??[]).some(v=>v<0||v>1))throw Error('Invalid glTF physical material');const color=pbr.baseColorFactor??[1,1,1,1],map=pbr.baseColorTexture;
    if(!array(color,4)||color.some(v=>v<0||v>1)||material.alphaMode==='BLEND')throw Error('Unsupported glTF material color/blending');
    if(map&&((map.texCoord??0)!==0||map.extensions))throw Error('Only untransformed TEXCOORD_0 is supported');
    const source=map?document.textures?.[map.index]?.source:undefined;
    if(map&&!integer(source,0,(document.images?.length??0)-1))throw Error('Missing glTF texture');
    const texture=map?document.textures?.[map.index]:null,sampler=texture?.sampler===undefined?{}:document.samplers?.[texture.sampler];
    if(map&&(!sampler||![10497,33071,33648].includes(sampler.wrapS??10497)||![10497,33071,33648].includes(sampler.wrapT??10497)))throw Error('Unsupported glTF sampler');
    const alphaCutoff=material.alphaMode==='MASK'?(material.alphaCutoff??0.5):0;
    if(!Number.isFinite(alphaCutoff)||alphaCutoff<0||alphaCutoff>1)throw Error('Invalid glTF alpha cutoff');
    materials['material-'+i]={color:[...color],alphaCutoff,roughness:pbr.roughnessFactor??1,metalness:pbr.metallicFactor??1,normalMap:textureKey(material.normalTexture),roughnessMap:textureKey(pbr.metallicRoughnessTexture),metalnessMap:textureKey(pbr.metallicRoughnessTexture),aoMap:textureKey(material.occlusionTexture),emissiveMap:textureKey(material.emissiveTexture),emissive:material.emissiveFactor??[0,0,0],...(map?{texture:'image-'+source,wrapS:sampler.wrapS??10497,wrapT:sampler.wrapT??10497}:{})};
  }
  for(const [i,image]of (poseOnly?[]:document.images??[]).entries()){
    if(i>=128)throw Error('glTF image budget exceeded');
    if(image.bufferView!==undefined){
      if(!['image/png','image/jpeg'].includes(image.mimeType))throw Error('Unsupported glTF image encoding');
      const {bytes}=viewBytes(image.bufferView);if(bytes.length>8*1024*1024)throw Error('glTF image exceeds 8 MiB');
      encodedImages.push({key:'image-'+i,mimeType:image.mimeType,bytes:bytes.slice()});
    }else if(typeof image.uri==='string')encodedImages.push({key:'image-'+i,uri:image.uri});
    else throw Error('Missing glTF image');
  }
  const triangles=[],matrices=new Map(),jobs=[],rigs=[],poses=Object.create(null);
  let visited=0;
  const visit=(index,parent,ancestors)=>{
    if(!integer(index,0,nodes.length-1)||ancestors.has(index)||matrices.has(index)||++visited>4096)throw Error('Invalid/cyclic glTF scene graph');
    const node=nodes[index];
    const matrix=multiply(parent,nodeMatrix(node)),next=new Set(ancestors).add(index);
    if(!matrix.every(Number.isFinite))throw Error('Nonfinite glTF transform');
    matrices.set(index,matrix);if(node.mesh!==undefined)jobs.push({node,matrix,index});
    if(node.children!==undefined&&(!Array.isArray(node.children)||node.children.length>nodes.length))throw Error('Invalid glTF children');
    for(const child of node.children??[])visit(child,matrix,next);
  };
  const scene=document.scenes?.[document.scene??0];
  if(!scene||!Array.isArray(scene.nodes)||scene.nodes.length>nodes.length)throw Error('glTF needs an explicit scene');
  for(const node of scene.nodes)visit(node,identity(),new Set());
  for(const {node,matrix,index:nodeIndex}of jobs){
      let skinMatrices;
      if(node.skin!==undefined){
        const skin=document.skins?.[node.skin];if(!skin||!Array.isArray(skin.joints)||!skin.joints.length||skin.joints.length>256||new Set(skin.joints).size!==skin.joints.length)throw Error('Invalid glTF skin joints');
        if(skin.inverseBindMatrices!==undefined&&document.accessors?.[skin.inverseBindMatrices]?.componentType!==5126)throw Error('Inverse bind matrices must be float');
        const inverse=skin.inverseBindMatrices===undefined?skin.joints.map(identity):accessor(skin.inverseBindMatrices,['MAT4'],16);
        if(inverse.length!==skin.joints.length)throw Error('Inverse bind matrix count mismatch');
        skinMatrices=skin.joints.map((joint,i)=>{if(!matrices.has(joint))throw Error('Skin joint outside active scene');return multiply(matrices.get(joint),inverse[i]);});
      }
      const rigId='node-'+nodeIndex,rigMatrices=skinMatrices??[matrix],rigParts=[];
      if(rigged)poses[rigId]=new Float32Array(rigMatrices.flat());
      if(poseOnly)continue;
      const mesh=document.meshes?.[node.mesh];if(!Array.isArray(mesh?.primitives)||mesh.primitives.length>128)throw Error('Invalid glTF mesh');
      for(const primitive of mesh.primitives){
        if((primitive.mode??4)!==4||primitive.targets||primitive.extensions)throw Error('Only uncompressed glTF triangles without morph targets are supported');
        const posAcc=document.accessors?.[primitive.attributes?.POSITION];
        if(posAcc?.componentType!==5126||posAcc.normalized)throw Error('glTF positions must be float VEC3');
        const positions=accessor(primitive.attributes.POSITION,['VEC3'],3),uv=primitive.attributes.TEXCOORD_0!==undefined?accessor(primitive.attributes.TEXCOORD_0,['VEC2'],2):null;
        if(uv&&uv.length!==positions.length)throw Error('glTF UV count mismatch');
        const normals=primitive.attributes.NORMAL!==undefined?accessor(primitive.attributes.NORMAL,['VEC3'],3):null;if(normals&&normals.length!==positions.length)throw Error('glTF normal count mismatch');
        let joints,weights;
        if(skinMatrices){
          const attributes=primitive.attributes;
          if(![5121,5123].includes(document.accessors?.[attributes.JOINTS_0]?.componentType)||document.accessors?.[attributes.JOINTS_0]?.normalized||attributes.JOINTS_1!==undefined||attributes.WEIGHTS_1!==undefined)throw Error('Skin requires at most four integer joint influences');
          joints=accessor(attributes.JOINTS_0,['VEC4'],4);weights=accessor(attributes.WEIGHTS_0,['VEC4'],4);
          if(joints.length!==positions.length||weights.length!==positions.length||joints.some(j=>j.some(v=>!integer(v,0,skinMatrices.length-1)))||weights.some(w=>w.some(v=>v<0||v>1)||Math.hypot(...w)===0))throw Error('Invalid skin joint/weight data');
        }
        if(primitive.indices!==undefined&&![5121,5123,5125].includes(document.accessors?.[primitive.indices]?.componentType))throw Error('Invalid glTF index type');
        const indices=primitive.indices===undefined?positions.map((_,i)=>i):accessor(primitive.indices,['SCALAR'],1).map(v=>v[0]);
        if(indices.length%3||triangles.length+indices.length/3>maximumTriangles)throw Error('glTF exceeds triangle budget');
        const material=primitive.material===undefined?'':'material-'+primitive.material;
        if(material&&!Object.hasOwn(materials,material))throw Error('Missing glTF material');
        if(['texture','normalMap','roughnessMap','metalnessMap','emissiveMap','aoMap'].some(key=>materials[material]?.[key])&&!uv)throw Error('Textured glTF primitive needs TEXCOORD_0');
        if(rigged){
          const points=[],directions=[],coordinates=[],influences=[],factors=[];
          for(const at of indices){
            if(!integer(at,0,positions.length-1))throw Error('glTF index outside vertices');
            points.push(...positions[at]);if(normals)directions.push(...normals[at]);if(uv)coordinates.push(...uv[at]);
            influences.push(...(joints?.[at]??[0,0,0,0]));const weight=weights?.[at]??[1,0,0,0],total=weight.reduce((n,v)=>n+v,0);factors.push(...weight.map(v=>v/total));
          }
          rigParts.push({material,positions:new Float32Array(points),...(normals?{normals:new Float32Array(directions)}:{}),...(uv?{uvs:new Float32Array(coordinates)}:{}),skinIndices:new Uint16Array(influences),skinWeights:new Float32Array(factors)});
        }
        for(let i=0;i<indices.length;i+=3)triangles.push({material,corners:indices.slice(i,i+3).map(at=>{
          if(!integer(at,0,positions.length-1))throw Error('glTF index outside vertices');
          const p=positions[at],transform=m=>[0,1,2].map(row=>m[row]*p[0]+m[4+row]*p[1]+m[8+row]*p[2]+m[12+row]);
          let point;
          if(skinMatrices){const total=weights[at].reduce((sum,w)=>sum+w,0);point=[0,0,0];for(let j=0;j<4;j++){const position=transform(skinMatrices[joints[at][j]]);for(let axis=0;axis<3;axis++)point[axis]+=position[axis]*weights[at][j]/total;}}
          else point=transform(matrix);
          if(!point.every(Number.isFinite))throw Error('Nonfinite glTF position');
          let normal;if(normals){
            const apply=m=>{const a=m[0],b=m[4],c=m[8],d=m[1],e=m[5],f=m[9],g=m[2],h=m[6],i=m[10],n=normals[at],det=a*(e*i-f*h)-b*(d*i-f*g)+c*(d*h-e*g);if(Math.abs(det)<1e-12)throw Error('Singular glTF normal transform');return [((e*i-f*h)*n[0]+(f*g-d*i)*n[1]+(d*h-e*g)*n[2])/det,((c*h-b*i)*n[0]+(a*i-c*g)*n[1]+(b*g-a*h)*n[2])/det,((b*f-c*e)*n[0]+(c*d-a*f)*n[1]+(a*e-b*d)*n[2])/det];};
            if(skinMatrices){const total=weights[at].reduce((sum,w)=>sum+w,0);normal=[0,0,0];for(let j=0;j<4;j++){const n=apply(skinMatrices[joints[at][j]]);for(let axis=0;axis<3;axis++)normal[axis]+=n[axis]*weights[at][j]/total;}}else normal=apply(matrix);const length=Math.hypot(...normal);if(!length)throw Error('Zero glTF normal');normal=normal.map(v=>v/length);
          }
          return {point,uv:uv?[...uv[at]]:[0,0],...(normal?{normal}:{})};
        })});
      }
      if(rigged)rigs.push({id:rigId,jointCount:rigMatrices.length,parts:rigParts});
  }
  if(!poseOnly&&!triangles.length)throw Error('glTF scene has no triangles');
  return {triangles,materials,encodedImages,clips,...(rigged?{rigs,pose:{skins:poses,nodes:Object.fromEntries([...matrices].map(([i,m])=>[i,new Float32Array(m)]))}}:{})};
}
export function parseGameGLTF(document,options){return parseGLTF(document,options);}
/** Compile immutable, bounded glTF skin data once; sample only its skeleton on
 * subsequent frames. GPU vertex skinning remains in the trusted host. */
export function createGameRig(document,{buffers=[],maximumTriangles=65536}={}){
  if(document instanceof Uint8Array){if(buffers.length)throw Error('GLB rig buffers must be embedded');const unpacked=unpackGameGLB(document);return createGameRig(unpacked.document,{buffers:unpacked.buffers,maximumTriangles});}
  if(!document||JSON.stringify(document).length>1024*1024||!Array.isArray(buffers)||buffers.some(b=>!(b instanceof Uint8Array))||buffers.reduce((n,b)=>n+b.length,0)>32*1024*1024)throw Error('Invalid bounded game rig source');
  const source=JSON.parse(JSON.stringify(document)),retained=buffers.map(b=>b.slice()),options={buffers:retained,maximumTriangles,profile:'gpu',rigged:true,cache:{accessors:new Map(),tracks:new Map()}};
  const compiled=parseGLTF(source,options);
  const bounds={min:[Infinity,Infinity,Infinity],max:[-Infinity,-Infinity,-Infinity]};for(const t of compiled.triangles)for(const c of t.corners)for(let i=0;i<3;i++){bounds.min[i]=Math.min(bounds.min[i],c.point[i]);bounds.max[i]=Math.max(bounds.max[i],c.point[i]);}
  return Object.freeze({bounds,rigs:compiled.rigs,materials:compiled.materials,encodedImages:compiled.encodedImages,clips:compiled.clips,nodes:(source.nodes??[]).map((node,index)=>({index,name:node.name??'node-'+index})),pose:({animation=null,time=0,loop=true}={})=>parseGLTF(source,{...options,animation,time,loop,poseOnly:true}).pose});
}
createGameRig.controlFactory=Object.freeze({topics:['games'],constructor:'createGameRig(documentOrGlb,{buffers:[Uint8Array],maximumTriangles:65536})',methods:'Compile supplied glTF 2.0 skeletal geometry once; no URL fetching. Returns bounds:{min,max}, rigs:[{id,jointCount,parts:[{material,positions,normals,uvs,skinIndices:Uint16Array,skinWeights:Float32Array}]}], materials, encodedImages, clips and named nodes. Upload each rig as a mesh with jointCount and its parts. pose({animation:clipName,time,loop:true}) samples bounded LINEAR/STEP node tracks and quaternion slerp without rebuilding geometry; returns {skins:{[rig.id]:Float32Array of affine skin matrices},nodes:{[nodeIndex]:Float32Array world matrix}}. Pass skins[rig.id] as skinMatrices on that rendered object; different actors can use independent poses of one uploaded mesh. Use node world matrices to attach held equipment. Up to four normalized influences and 256 joints per rig, immutable retained source/buffers, same glTF format/triangle bounds as parseGameGLTF. CPU parse/raster defaults remain unchanged. Use actual walk/run/idle clips tied to movement and gameplay; avoid rigid toy joints, copied static poses and floating heads.'});
parseGameGLTF.controlFactory=Object.freeze({topics:['games'],constructor:'parseGameGLTF(document,{buffers:[new Uint8Array(buffer)],maximumTriangles:8192,profile:"cpu",animation:null,time:0,loop:true})',methods:'glTF 2.0 scene graph, matrix/TRS/quaternion transforms, TRIANGLES, UV0, diffuse baseColor factors/textures and embedded PNG/JPEG slices. animation is clip name/index or null for bind pose; time is seconds. Supports LINEAR/STEP translation/scale, quaternion slerp, skeletal skinning with four joint weights and inverse bind matrices, at most 256 joints per skin. clips lists names/indexes. All external buffers must be supplied explicitly in document order; encodedImages have {key,bytes,mimeType} or {key,uri}. Decode via openImageEngine. Use bakeGameModelAtlas for directional animated units; sample small scenes live only after measuring. No fetching. Required extensions, Draco/compression, CUBICSPLINE, morphs and translucent blend materials fail explicitly; Retains authored transformed normals and physical texture references; the CPU rasterizer ignores physical maps, while openGameRenderer uses them. 8192 default output triangles (explicit CPU limit up to 16384; profile:"gpu" permits up to 65536 with 196608 accessor elements), 32 MiB buffers, 128 materials/images, bounded scene graph. Keep source notices and all dependencies.'});

function unpackGameGLB(bytes){
  if(!(bytes instanceof Uint8Array)||bytes.length<20||bytes.length>32*1024*1024)throw Error('Invalid bounded GLB');
  const data=new DataView(bytes.buffer,bytes.byteOffset,bytes.length);
  if(data.getUint32(0,true)!==0x46546c67||data.getUint32(4,true)!==2||data.getUint32(8,true)!==bytes.length)throw Error('Invalid GLB header');
  let document,bin;
  for(let offset=12;offset<bytes.length;){
    if(offset+8>bytes.length)throw Error('Truncated GLB chunk');
    const size=data.getUint32(offset,true),type=data.getUint32(offset+4,true);offset+=8;
    if(size%4||offset+size>bytes.length)throw Error('Invalid GLB chunk bounds');
    const chunk=bytes.subarray(offset,offset+size);
    if(document===undefined){if(type!==0x4e4f534a)throw Error('GLB JSON must be first');document=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(chunk));}
    else if(type===0x004e4942){if(bin)throw Error('Duplicate GLB BIN');bin=chunk;}
    offset+=size;
  }
  if(document?.buffers?.some(b=>b.uri!==undefined))throw Error('GLB external buffers require explicit parseGameGLTF loading');
  return {document,buffers:bin?[bin]:[]};
}
export function parseGameGLB(bytes,options){const {document,buffers}=unpackGameGLB(bytes);return parseGameGLTF(document,{...options,buffers});}
parseGameGLB.controlFactory=Object.freeze({topics:['games'],constructor:'parseGameGLB(bytes,{maximumTriangles:8192,profile:"cpu",animation:null,time:0,loop:true})',methods:'Parse a bounded GLB 2 binary with embedded JSON/BIN into the same posed triangle model as parseGameGLTF. Embedded PNG/JPEG payloads are returned in encodedImages for openImageEngine. Never fetch dependencies during parse. Rejects external buffers, malformed chunks and unsupported compressed features. Use parseGameGLTF for explicitly packaged external buffers.'});
