/** Static Wavefront geometry and CPU textured rendering for small native scenes.
 * No URL is fetched or material script executed by either parser. */
import { GameSpriteAtlas } from './game-sprites.mjs';
const finite = values => values.every(Number.isFinite);
const boundedText = text => {
  if (typeof text !== 'string' || text.length > 4 * 1024 * 1024) throw RangeError('Model text exceeds 4 MiB');
};
const localPath = path => typeof path === 'string' && /^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9_. /-]{1,255}$/.test(path);
export async function readGameAsset({services,path,maximumBytes=8*1024*1024,signal}={}){
  if(!localPath(path)||!services?.assets||!Number.isSafeInteger(maximumBytes)||maximumBytes<1||maximumBytes>32*1024*1024)throw RangeError('Invalid packaged game asset request');
  const alive=()=>{if(signal?.aborted)throw signal.reason??Error('Asset load cancelled');};alive();
  const stat=await services.assets.stat({path});alive();
  if(!Number.isSafeInteger(stat?.size)||stat.size<1||stat.size>maximumBytes)throw RangeError('Game asset exceeds byte budget');
  const bytes=new Uint8Array(stat.size);
  for(let offset=0;offset<bytes.length;offset+=262144){
    alive();const length=Math.min(262144,bytes.length-offset),chunk=await services.assets.read({path,offset,length});alive();
    if(!(chunk instanceof Uint8Array)||chunk.length!==length)throw Error('Incomplete packaged game asset');bytes.set(chunk,offset);
  }
  return bytes;
}
readGameAsset.controlFactory=Object.freeze({topics:['games'],constructor:'await readGameAsset({services,path:"assets/unit.glb",maximumBytes:8388608,signal})',methods:'Read an exact approved package asset as Uint8Array using assets.stat and <=256 KiB assets.read ranges. Requires assets.stat/read permissions. Default 8 MiB, explicit maximum up to 32 MiB. Rejects missing/truncated/oversized bytes and honours AbortSignal between chunks. No VFS/network reads; use this for real imported textures and model buffers rather than assuming assets.read returns a whole large file.'});
export function parseGameMTL(text) {
  boundedText(text);
  const materials = Object.create(null); let material;
  for (const line of text.split(/\r?\n/)) {
    const [key, ...args] = line.replace(/#.*/, '').trim().split(/\s+/);
    if (key === 'newmtl') {
      const name = args.join(' ');
      if (!name || name.length > 128 || Object.hasOwn(materials, name) || Object.keys(materials).length >= 128) throw RangeError('Invalid model material');
      materials[name] = material = { color: [1, 1, 1, 1] };
    } else if (key === 'Kd' && material) {
      const rgb = args.map(Number);
      if (rgb.length !== 3 || !finite(rgb) || rgb.some(v => v < 0 || v > 1)) throw RangeError('Invalid material color');
      material.color = [...rgb, material.color[3]];
    } else if (['d', 'Tr'].includes(key) && material) {
      const alpha = Number(args[0]);
      if (args.length !== 1 || !Number.isFinite(alpha) || alpha < 0 || alpha > 1) throw RangeError('Invalid material alpha');
      material.color[3] = key === 'Tr' ? 1 - alpha : alpha;
    } else if (key === 'map_Kd' && material) {
      const path = args.join(' ');
      if (!localPath(path) || path.startsWith('-')) throw Error('Texture maps require a package-local path without MTL options');
      material.texture = path;
    }
  }
  return materials;
}
parseGameMTL.controlFactory = Object.freeze({ topics: ['games'], constructor: 'parseGameMTL(text)', methods: 'Parse at most 128 static Wavefront materials: Kd diffuse RGBA, d/Tr opacity, map_Kd package-local texture path. Returns material map; no fetching. No PBR/normal/roughness shaders. Import every map separately and retain original notices.' });

export function parseGameOBJ(text, { materials = {}, maximumTriangles = 4096, profile = "cpu" } = {}) {
  boundedText(text);
  if (!Number.isInteger(maximumTriangles) || maximumTriangles < 1 || maximumTriangles > (profile==="gpu"?65536:4096) || !["cpu","gpu"].includes(profile)) throw RangeError('Invalid triangle budget');
  const positions = [], texcoords = [], normals = [], triangles = [], libraries = []; let material = '';
  const index = (raw, list) => {
    const n = Number(raw), result = n > 0 ? n - 1 : list.length + n;
    if (!/^-?[1-9]\d*$/.test(raw || '') || !Number.isInteger(n) || result < 0 || result >= list.length) throw RangeError('OBJ index outside declared data');
    return result;
  };
  for (const line of text.split(/\r?\n/)) {
    const [key, ...args] = line.replace(/#.*/, '').trim().split(/\s+/);
    if (['v', 'vt', 'vn'].includes(key)) {
      const values = args.map(Number), size = key === 'vt' ? 2 : 3;
      if (values.length !== size || !finite(values) || values.some(v => Math.abs(v) > 1e6)) throw RangeError('Invalid OBJ coordinate');
      const list = key === 'v' ? positions : key === 'vt' ? texcoords : normals;
      if (list.length >= (profile==="gpu"?196608:16384)) throw RangeError('OBJ exceeds vertex budget');
      list.push(values);
    } else if (key === 'mtllib') {
      const path = args.join(' ');
      if (!localPath(path) || libraries.length >= 16) throw Error('OBJ material library must be package-local');
      libraries.push(path);
    } else if (key === 'usemtl') {
      material = args.join(' ');
      if (!Object.hasOwn(materials, material)) throw Error('Missing OBJ material: ' + material);
    } else if (key === 'f') {
      if (args.length < 3 || args.length > 8) throw RangeError('OBJ needs convex faces with 3..8 corners');
      const corners = args.map(raw => {
        const parts = raw.split('/');
        if (parts.length > 3) throw RangeError('Invalid OBJ face');
        const point = positions[index(parts[0], positions)];
        const uv = parts[1] ? texcoords[index(parts[1], texcoords)] : [0, 0];
        const normal=parts[2]?normals[index(parts[2],normals)]:null;
        return { point: [...point], uv: [uv[0], 1 - uv[1]],...(normal?{normal:[...normal]}:{}) };
      });
      for (let i = 1; i < corners.length - 1; i++) {
        if (triangles.length >= maximumTriangles) throw RangeError('OBJ exceeds triangle budget');
        triangles.push({ corners: [corners[0], corners[i], corners[i + 1]], material });
      }
    } else if (['call', 'csh', 'surf', 'curv'].includes(key)) throw Error('Executable/freeform OBJ content is unsupported');
  }
  if (!triangles.length) throw Error('OBJ contains no faces');
  return { triangles, materials, libraries };
}
parseGameOBJ.controlFactory = Object.freeze({ topics: ['games'], constructor: 'parseGameOBJ(text,{materials:parseGameMTL(mtlText),maximumTriangles:4096})', methods: 'Bounded static OBJ v/vt/vn, positive/negative face indices, convex face fan triangulation and named MTL materials. Returns {triangles:[{corners:[{point:[x,y,z],uv:[u,v]}],material}],materials,libraries}. Y-up, UV V flipped once from OBJ convention. Missing materials fail; libraries are references only, never fetched. Default CPU profile: at most 16384 vertices/4096 triangles. Explicit profile:"gpu" allows 196608 vertices and maximumTriangles up to 65536 for openGameRenderer; preserves authored corner normals. CPU rasterization retains its independent 16384 triangle ceiling. No GLB/glTF, FBX, skinning, skeletal animation, concave polygon triangulation or executable OBJ commands. Author frame animation with GameSpriteAtlas or transform static models.' });

/** Z-buffer, near clipping and perspective-correct UV interpolation. The returned
 * small RGBA frame uses the existing native image command, not browser WebGL. */
export function rasterizeGameModel(model, {
  width = 320, height = 180, camera = {}, textures = {},
  light = [-0.4, 0.8, -0.6], ambient = 0.45, cull = false, captureSurface = false,
} = {}) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width < 1 || height < 1 || width * height > 262144 || width > 1024 || height > 1024 || !Array.isArray(model?.triangles) || model.triangles.length > 16384 || !Array.isArray(light) || light.length !== 3 || !finite(light) || !Math.hypot(...light) || !Number.isFinite(ambient) || ambient < 0 || ambient > 1 || (typeof cull !== 'boolean'||typeof captureSurface !== 'boolean')) throw RangeError('Invalid bounded model render');
  const { x = 0, y = 0, z = -4, yaw = 0, pitch = 0, near = 0.1, focalLength = height, orthographic = false, scale = 32 } = camera;
  if (!finite([x,y,z,yaw,pitch,near,focalLength,scale]) || near <= 0 || focalLength <= 0 || scale <= 0 || typeof orthographic !== 'boolean') throw RangeError('Invalid model camera');
  let texturePixels = 0;
  for (const texture of Object.values(textures)) {
    if (!Number.isInteger(texture?.width) || !Number.isInteger(texture?.height) || texture.width < 1 || texture.height < 1 || !(texture.rgba instanceof Uint8Array) || texture.rgba.length !== texture.width * texture.height * 4) throw RangeError('Invalid decoded model texture');
    texturePixels += texture.width * texture.height;
    if (texturePixels > 1048576) throw RangeError('Model texture budget exceeds 1 megapixel');
  }
  const surface=captureSurface?{albedo:new Uint8Array(width*height*4),normal:new Int8Array(width*height*3),position:new Float32Array(width*height*3),bounds:{minimum:[Infinity,Infinity,Infinity],maximum:[-Infinity,-Infinity,-Infinity]}}:null;
  const rgba = new Uint8Array(width * height * 4), depth = new Float64Array(width * height).fill(Infinity);
  const direction = light.map(v => v / Math.hypot(...light)), cy = Math.cos(yaw), sy = Math.sin(yaw), cp = Math.cos(pitch), sp = Math.sin(pitch);
  const transform = p => {
    const dx = p[0]-x, dy = p[1]-y, dz = p[2]-z, rx = dx*cy-dz*sy, rz = dx*sy+dz*cy;
    return [rx,dy*cp-rz*sp,dy*sp+rz*cp];
  };
  const edge = (a,b,px,py) => (px-a[0])*(b[1]-a[1])-(py-a[1])*(b[0]-a[0]);
  // Bound total pixel candidates as well as triangles: no extreme overdraw loop.
  let candidates = 0, painted = 0;
  for (const triangle of model.triangles) {
    if (!Array.isArray(triangle.corners) || triangle.corners.length !== 3) throw RangeError('Invalid model triangle');
    const corners = triangle.corners;
    for (const v of corners) if (!Array.isArray(v.point) || v.point.length !== 3 || !finite(v.point) || !Array.isArray(v.uv) || v.uv.length !== 2 || !finite(v.uv)) throw RangeError('Invalid model corner');
    if(surface)for(const corner of corners)for(let j=0;j<3;j++){surface.bounds.minimum[j]=Math.min(surface.bounds.minimum[j],corner.point[j]);surface.bounds.maximum[j]=Math.max(surface.bounds.maximum[j],corner.point[j]);}
    const a=corners[0].point,b=corners[1].point,c=corners[2].point;
    const ab=b.map((v,i)=>v-a[i]),ac=c.map((v,i)=>v-a[i]);
    const n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]],length=Math.hypot(...n);
    if (!length) continue;
    if (cull && n.reduce((v,k,i)=>v+k*(a[i]-[x,y,z][i]),0)>=0) continue;
    const intensity=ambient+(1-ambient)*Math.max(0,n.reduce((v,k,i)=>v+k*direction[i],0)/length);
    const material=model.materials?.[triangle.material] ?? {color:[1,1,1,1]},color=material.color??[1,1,1,1];
    if (!Array.isArray(color) || color.length!==4 || !finite(color) || color.some(v=>v<0||v>1)) throw RangeError('Invalid model color');
    const texture=material.texture && Object.hasOwn(textures,material.texture) ? textures[material.texture] : null;
    if (material.texture && !texture) throw Error('Missing decoded model texture: '+material.texture);
    const alphaCutoff=material.alphaCutoff??0.5;
    if(!Number.isFinite(alphaCutoff)||alphaCutoff<0||alphaCutoff>1)throw RangeError('Invalid model alpha cutoff');
    const wrap=(value,mode=10497)=>mode===33071?Math.min(1-Number.EPSILON,Math.max(0,value)):mode===33648?1-Math.abs(((value%2+2)%2)-1):(value%1+1)%1;
    // Clip in camera space, carrying UV through the same interpolation.
    const original=corners.map(v=>[...transform(v.point),...v.uv,...(captureSurface?v.point:[])]),polygon=[];
    for(let i=0;i<3;i++){
      const v=original[i],w=original[(i+1)%3],inside=v[2]>=near;
      if(inside)polygon.push(v);
      if(inside!==(w[2]>=near)){const t=(near-v[2])/(w[2]-v[2]);polygon.push(v.map((value,j)=>value+(w[j]-value)*t));}
    }
    for(let k=1;k<polygon.length-1;k++){
      const vertices=[polygon[0],polygon[k],polygon[k+1]].map(v=>{
        const q=orthographic?1:1/v[2],factor=orthographic?scale:focalLength*q;
        return [width/2+v[0]*factor,height/2-v[1]*factor,v[2],v[3],v[4],q,...v.slice(5)];
      });
      const [v0,v1,v2]=vertices,area=edge(v0,v1,v2[0],v2[1]);
      if(!Number.isFinite(area)||Math.abs(area)<1e-8)continue;
      const left=Math.max(0,Math.floor(Math.min(...vertices.map(v=>v[0])))),right=Math.min(width-1,Math.ceil(Math.max(...vertices.map(v=>v[0]))));
      const top=Math.max(0,Math.floor(Math.min(...vertices.map(v=>v[1])))),bottom=Math.min(height-1,Math.ceil(Math.max(...vertices.map(v=>v[1]))));
      candidates+=Math.max(0,right-left+1)*Math.max(0,bottom-top+1);
      if(candidates>8*1024*1024)throw RangeError('Model exceeded pixel work budget; reduce geometry/resolution');
      // Scalar interpolation keeps the bounded raster loop allocation-free.
      const q0=v0[5],q1=v1[5],q2=v2[5],u0=v0[3]*q0,u1=v1[3]*q1,u2=v2[3]*q2,t0=v0[4]*q0,t1=v1[4]*q1,t2=v2[4]*q2;
      const nx=Math.round(n[0]/length*127),ny=Math.round(n[1]/length*127),nz=Math.round(n[2]/length*127);
      for(let py=top;py<=bottom;py++)for(let px=left;px<=right;px++){
        const w0=edge(v1,v2,px+0.5,py+0.5)/area,w1=edge(v2,v0,px+0.5,py+0.5)/area,w2=edge(v0,v1,px+0.5,py+0.5)/area;
        if(w0< -1e-9||w1< -1e-9||w2< -1e-9)continue;
        const q=w0*q0+w1*q1+w2*q2,zValue=orthographic?w0*v0[2]+w1*v1[2]+w2*v2[2]:1/q;
        const pixel=py*width+px;if(zValue>=depth[pixel])continue;
        let sr=255,sg=255,sb=255,sa=255;
        if(texture){
          const u=(w0*u0+w1*u1+w2*u2)/q,v=(w0*t0+w1*t1+w2*t2)/q;
          const tx=Math.min(texture.width-1,Math.floor(wrap(u,material.wrapS)*texture.width)),ty=Math.min(texture.height-1,Math.floor(wrap(v,material.wrapT)*texture.height)),offset=(ty*texture.width+tx)*4;
          sr=texture.rgba[offset];sg=texture.rgba[offset+1];sb=texture.rgba[offset+2];sa=texture.rgba[offset+3];
        }
        if(sa*color[3]<alphaCutoff*255)continue;
        const at=pixel*4,pt=pixel*3;
        depth[pixel]=zValue;rgba[at]=Math.round(sr*color[0]*intensity);rgba[at+1]=Math.round(sg*color[1]*intensity);rgba[at+2]=Math.round(sb*color[2]*intensity);rgba[at+3]=255;painted++;
        if(surface){
          surface.albedo[at]=Math.round(sr*color[0]);surface.albedo[at+1]=Math.round(sg*color[1]);surface.albedo[at+2]=Math.round(sb*color[2]);surface.albedo[at+3]=255;
          surface.normal[pt]=nx;surface.normal[pt+1]=ny;surface.normal[pt+2]=nz;
          for(let j=0;j<3;j++)surface.position[pt+j]=(w0*v0[6+j]*q0+w1*v1[6+j]*q1+w2*v2[6+j]*q2)/q;
        }
      }
    }
  }
  return {width,height,rgba,painted,...(surface?{surface}:{})};
}
rasterizeGameModel.controlFactory = Object.freeze({topics:['games'],constructor:'rasterizeGameModel(model,{width:320,height:180,camera:{x:0,y:0,z:-4,yaw:0,pitch:0,orthographic:false,scale:32},textures:{"stone.png":{width,height,rgba}},light:[-0.4,0.8,-0.6],ambient:0.45,cull:false})',methods:'Returns {width,height,rgba,painted}; captureSurface:true also retains {surface:{albedo:Uint8Array,normal:Int8Array,position:Float32Array}} at the same bounded dimensions. Feed this to GameModelLayer for dynamic normal-based lights and solid fog shading without rerasterizing geometry. CPU depth buffer, near clipping, perspective-correct UVs, repeat/nearest diffuse textures, directional flat light and alpha cutouts. Orthographic yaw/pitch camera supports isometric 3D props. Draw with SceneBuilder.defineImage(id,width,height,rgba).image(id,rect), or bake reusable sprite frames once rather than rasterizing per frame. Hard limits: 16384 triangles, 262144 output pixels, 1 megapixel total decoded textures, 8 million bounding-box pixel candidates. Missing texture fails explicitly. No PBR, translucent blending or GPU engine. parseGameGLTF/GLB supplies sampled/skinned model geometry; bake larger scenes into directional sprites before gameplay.'});

/** Bake sampled model poses into directional animation once, outside gameplay. */
export async function bakeGameModelAtlas({sample,textures={},directions=8,frames=8,duration=1,width=48,height=64,target=[0,0,0],distance=6,elevation=3,scale=20,azimuth=Math.PI/4,signal}={}){
  if(typeof sample!=='function'||!Number.isInteger(directions)||directions<1||directions>16||!Number.isInteger(frames)||frames<1||frames>32||directions*frames>128||!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width*height*directions*frames>1048576||width*frames>2048||height*directions>2048||!Array.isArray(target)||target.length!==3||!finite([...target,duration,distance,elevation,scale,azimuth])||duration<=0||duration>60||distance<=0||scale<=0)throw RangeError('Invalid bounded directional atlas bake');
  const atlasWidth=width*frames,atlasHeight=height*directions,rgba=new Uint8Array(atlasWidth*atlasHeight*4),rects={},animations={};
  for(let direction=0;direction<directions;direction++){
    const yaw=azimuth+direction*Math.PI*2/directions,sequence=[];
    for(let frame=0;frame<frames;frame++){
      if(signal?.aborted)throw signal.reason??Error('Model bake cancelled');
      const pose=await sample(frame*duration/frames),image=rasterizeGameModel(pose,{width,height,textures,camera:{x:target[0]-Math.sin(yaw)*distance,y:target[1]+elevation,z:target[2]-Math.cos(yaw)*distance,yaw,pitch:-Math.atan2(elevation,distance),orthographic:true,scale}});
      for(let row=0;row<height;row++)rgba.set(image.rgba.subarray(row*width*4,(row+1)*width*4),((direction*height+row)*atlasWidth+frame*width)*4);
      const name='view-'+direction+'-frame-'+frame;rects[name]={x:frame*width,y:direction*height,width,height,pivotX:0.5,pivotY:1};sequence.push({frame:name,duration:duration/frames});
      // Let app cancellation/close and progress rendering run between poses.
      await new Promise(resolve=>setTimeout(resolve,0));
    }
    animations['view-'+direction]=sequence;
  }
  return new GameSpriteAtlas({width:atlasWidth,height:atlasHeight,rgba,frames:rects,animations});
}
bakeGameModelAtlas.controlFactory=Object.freeze({topics:['games'],constructor:'await bakeGameModelAtlas({sample:time=>parseGameGLB(bytes,{animation:"Walk",time}),textures,directions:8,frames:8,duration:1,width:48,height:64,target:[0,0.8,0],scale:20,signal})',methods:'Bake diffuse textured model poses from an explicit sample(timeSeconds) callback into a GameSpriteAtlas with view-N-frame-M frames and view-N animations. Eight orthographic orbit angles by default, up to 128 total frames/1 megapixel. Uses actual sampled/skinned glTF geometry through parseGameGLTF/GLB; reuse decoded textures. Bake once in app-owned initialization, yield between frames, cancel with AbortSignal, and retain/reuse the atlas in gameplay rather than rasterizing many 3D models each frame. Tune target/scale/dimensions to keep the complete model and weapons inside every frame. Build separate walk/attack/death atlases when needed. This is a bounded sprite baking path, not a GPU renderer.'});

/** CPU composition of depth-sorted model sprites with retained surface normals.
 * Fog darkens RGB rather than making remembered objects transparent. */
export class GameModelLayer {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameModelLayer({width:640,height:360})',methods:'clear(); draw({image,frame,x,y,scale,pivotX,pivotY,origin:[column,height,row],brightness:1,light:[-0.6,1,-0.45],ambient:0.38,pointLights:[{position:[column,height,row],color:[0.5,0.8,1],radius:6,intensity:1}]}); render(scene,imageId). Supply rasterizeGameModel(...,{captureSurface:true}) to relight actual retained per-pixel normals, local positions and diffuse albedo. Draw sprites in world depth order, then render native RGBA layers; render(scene,imageId,{rect:[x,y,width,height]}) maps a layer to a logical viewport region. Split a high-density viewport into strips that each remain within the pixel budget. Missing surface data uses the baked RGBA unchanged except brightness. fogAt:(column,row)=>0..1 applies visibility to each retained surface position: 0 hides, intermediate values darken RGB while preserving solidity. brightness dims RGB for remembered fog without ghost transparency; skip unexplored objects before drawing. Point lights affect only provided surfaces and never reveal hidden objects. Optional atlas frame uses pixel x/y/width/height and 0..1 pivots. At most 128 sprites, 8 point lights per sprite, 262144 output pixels, 1 megapixel input image, 8 million destination candidates per clear. No shadow maps, PBR, GPU lighting or automatic world visibility.'});
  constructor({width=640,height=360}={}){
    if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>1024||height>1024||width*height>262144)throw RangeError('Invalid model layer dimensions');
    this.width=width;this.height=height;this.rgba=new Uint8Array(width*height*4);this.lightHits=new Uint32Array(8);this.clear();
  }
  clear(){this.rgba.fill(0);this.lightHits.fill(0);this.sprites=0;this.candidates=0;this.foggedPixels=0;this.litPixels=0;return this;}
  draw({image,frame=null,x,y,scale=1,pivotX=frame?.pivotX??.5,pivotY=frame?.pivotY??1,origin=[0,0,0],brightness=1,light=[-.6,1,-.45],ambient=.38,pointLights=[],fogAt=null}={}){
    if(!Number.isInteger(image?.width)||!Number.isInteger(image?.height)||image.width<1||image.height<1||image.width*image.height>1048576||!(image.rgba instanceof Uint8Array)||image.rgba.length!==image.width*image.height*4||!finite([x,y,scale,pivotX,pivotY,brightness,ambient])||scale<=0||scale>16||pivotX<0||pivotX>1||pivotY<0||pivotY>1||brightness<0||brightness>1||ambient<0||ambient>1||!Array.isArray(origin)||origin.length!==3||!finite(origin)||!Array.isArray(light)||light.length!==3||!finite(light)||!Math.hypot(...light)||!Array.isArray(pointLights)||pointLights.length>8)throw RangeError('Invalid model layer sprite');
    if(fogAt!==null&&typeof fogAt!=='function')throw TypeError('Model fog requires a world visibility callback');
    const rect=frame??{x:0,y:0,width:image.width,height:image.height};
    if(![rect.x,rect.y,rect.width,rect.height].every(Number.isInteger)||rect.x<0||rect.y<0||rect.width<1||rect.height<1||rect.x+rect.width>image.width||rect.y+rect.height>image.height||rect.offsetX||rect.offsetY)throw RangeError('Invalid model layer frame');
    for(const p of pointLights)if(!Array.isArray(p.position)||p.position.length!==3||!finite(p.position)||!Array.isArray(p.color)||p.color.length!==3||!finite(p.color)||p.color.some(c=>c<0||c>1)||!Number.isFinite(p.radius)||p.radius<=0||p.radius>64||!Number.isFinite(p.intensity)||p.intensity<0||p.intensity>4)throw RangeError('Invalid model point light');
    const surface=image.surface,pixels=image.width*image.height;
    if(surface&&(!(surface.albedo instanceof Uint8Array)||surface.albedo.length!==pixels*4||!(surface.normal instanceof Int8Array)||surface.normal.length!==pixels*3||!(surface.position instanceof Float32Array)||surface.position.length!==pixels*3))throw RangeError('Invalid retained model surface');
    const left=x-rect.width*pivotX*scale,top=y-rect.height*pivotY*scale;
    const x0=Math.max(0,Math.floor(left)),y0=Math.max(0,Math.floor(top)),x1=Math.min(this.width,Math.ceil(left+rect.width*scale)),y1=Math.min(this.height,Math.ceil(top+rect.height*scale));
    if(++this.sprites>128||(this.candidates+=Math.max(0,x1-x0)*Math.max(0,y1-y0))>8*1024*1024)throw RangeError('Model layer exceeds pixel work budget');
    const norm=Math.hypot(...light),sunX=light[0]/norm/127,sunY=light[1]/norm/127,sunZ=light[2]/norm/127,source=surface?.albedo??image.rgba;
    const positions=surface?.position,normals=surface?.normal;
    // Reject lights outside the complete model bounds once per sprite. No
    // per-pixel temporary arrays or square roots for distant scene lights.
    const prepared=[];
    if(surface)for(let i=0;i<pointLights.length;i++){
      const p=pointLights[i],lx=p.position[0]-origin[0],ly=p.position[1]-origin[1],lz=p.position[2]-origin[2],bounds=surface.bounds;
      if(Array.isArray(bounds?.minimum)&&Array.isArray(bounds?.maximum)&&bounds.minimum.length===3&&bounds.maximum.length===3&&finite(bounds.minimum)&&finite(bounds.maximum)){
        const dx=Math.max(bounds.minimum[0]-lx,0,lx-bounds.maximum[0]),dy=Math.max(bounds.minimum[1]-ly,0,ly-bounds.maximum[1]),dz=Math.max(bounds.minimum[2]-lz,0,lz-bounds.maximum[2]);if(dx*dx+dy*dy+dz*dz>=p.radius*p.radius)continue;
      }
      prepared.push([lx,ly,lz,p.radius,p.intensity,p.color[0],p.color[1],p.color[2],i]);
    }
    const rootFog=!surface&&fogAt?fogAt(origin[0],origin[2]):1;
    if(!Number.isFinite(rootFog)||rootFog<0||rootFog>1)throw RangeError('Invalid model fog brightness');
    const right=rect.x+rect.width-1,bottom=rect.y+rect.height-1,inverseScale=1/scale;
    for(let py=y0;py<y1;py++){
      const fy=rect.y+(py+.5-top)*inverseScale-.5,sy=Math.max(rect.y,Math.min(bottom,Math.round(fy))),by=Math.floor(fy),ty=fy-by,ya=Math.max(rect.y,Math.min(bottom,by))*image.width,yb=Math.max(rect.y,Math.min(bottom,by+1))*image.width;
      for(let px=x0;px<x1;px++){
        const fx=rect.x+(px+.5-left)*inverseScale-.5,sx=Math.max(rect.x,Math.min(right,Math.round(fx))),pixel=sy*image.width+sx,offset=pixel*4;
        if(!source[offset+3])continue;
        const index=pixel*3,fog=surface&&fogAt?fogAt(origin[0]+positions[index],origin[2]+positions[index+2]):rootFog,dim=brightness*fog;
        if(!Number.isFinite(fog)||fog<0||fog>1)throw RangeError('Invalid model fog brightness');if(dim<1)this.foggedPixels++;if(dim===0)continue;
        const nx=normals?normals[index]:0,ny=normals?normals[index+1]:0,nz=normals?normals[index+2]:0,shade=surface?ambient+(1-ambient)*Math.max(0,nx*sunX+ny*sunY+nz*sunZ):1;
        let red=shade,green=shade,blue=shade;
        for(let j=0;j<prepared.length;j++){
          const p=prepared[j],dx=p[0]-positions[index],dy=p[1]-positions[index+1],dz=p[2]-positions[index+2],squared=dx*dx+dy*dy+dz*dz;
          if(squared>=p[3]*p[3])continue;
          const distance=Math.sqrt(squared),facing=(nx*dx+ny*dy+nz*dz)/(127*Math.max(.001,distance));if(facing<=0)continue;
          const falloff=1-distance/p[3],gain=p[4]*falloff*falloff*facing;
          this.litPixels++;this.lightHits[p[8]]++;red+=gain*p[5];green+=gain*p[6];blue+=gain*p[7];
        }
        // Bilinear premultiplied sampling, scalar hot path. Transparent gutters
        // cannot bleed colors into the neighbouring sprite or its silhouette.
        const bx=Math.floor(fx),tx=fx-bx,xa=Math.max(rect.x,Math.min(right,bx)),xb=Math.max(rect.x,Math.min(right,bx+1)),a=(ya+xa)*4,b=(ya+xb)*4,c=(yb+xa)*4,d=(yb+xb)*4;
        const wa=(1-tx)*(1-ty)*source[a+3]/255,wb=tx*(1-ty)*source[b+3]/255,wc=(1-tx)*ty*source[c+3]/255,wd=tx*ty*source[d+3]/255,alpha=wa+wb+wc+wd;if(alpha<=0)continue;
        const r=(source[a]*wa+source[b]*wb+source[c]*wc+source[d]*wd)/alpha,g=(source[a+1]*wa+source[b+1]*wb+source[c+1]*wc+source[d+1]*wd)/alpha,bv=(source[a+2]*wa+source[b+2]*wb+source[c+2]*wc+source[d+2]*wd)/alpha;
        const at=(py*this.width+px)*4;
        if(alpha>.999){this.rgba[at]=Math.min(255,r*red)*dim;this.rgba[at+1]=Math.min(255,g*green)*dim;this.rgba[at+2]=Math.min(255,bv*blue)*dim;this.rgba[at+3]=255;}
        else{const old=this.rgba[at+3]/255*(1-alpha),out=alpha+old;this.rgba[at]=Math.round((Math.min(255,r*red)*dim*alpha+this.rgba[at]*old)/out);this.rgba[at+1]=Math.round((Math.min(255,g*green)*dim*alpha+this.rgba[at+1]*old)/out);this.rgba[at+2]=Math.round((Math.min(255,bv*blue)*dim*alpha+this.rgba[at+2]*old)/out);this.rgba[at+3]=Math.round(out*255);}
      }
    }
    return this;
  }
  render(scene,imageId,{rect=[0,0,this.width,this.height]}={}){scene.defineImage(imageId,this.width,this.height,this.rgba).image(imageId,rect,{smoothing:true});return scene;}
}
