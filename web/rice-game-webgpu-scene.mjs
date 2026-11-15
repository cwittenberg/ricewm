import {GameWebGpuQuadBatch} from './rice-game-webgpu-batch.mjs';
import {gaussianBlur} from './vendor/three/addons/tsl/display/GaussianBlurNode.js';
import {createGameGpuShadow} from './rice-game-webgpu-shadow.mjs';
import {createGameGpuGradient} from './rice-game-webgpu-gradients.mjs';
import {createGameWebGpuRenderer} from './rice-game-webgpu-device.mjs';
import * as N from './vendor/three/three.tsl.js';
import {SVGLoader} from './vendor/three/addons/loaders/SVGLoader.js';

const IDENTITY=[1,0,0,1,0,0],MAX_DRAWS=8192,MAX_GEOMETRIES=512;
const multiply=(a,b)=>[a[0]*b[0]+a[2]*b[1],a[1]*b[0]+a[3]*b[1],a[0]*b[2]+a[2]*b[3],a[1]*b[2]+a[3]*b[3],a[0]*b[4]+a[2]*b[5]+a[4],a[1]*b[4]+a[3]*b[5]+a[5]];
function same(a,b){
 if(a?.byteLength!==b.byteLength)return false;
 if(!(a.byteOffset%4)&&!(b.byteOffset%4)&&!(b.byteLength%4)){
  const left=new Uint32Array(a.buffer,a.byteOffset,a.byteLength/4),right=new Uint32Array(b.buffer,b.byteOffset,b.byteLength/4);
  for(let i=0;i<right.length;i++)if(left[i]!==right[i])return false;return true;
 }
 for(let i=0;i<b.length;i++)if(a[i]!==b[i])return false;return true;
}
function setMatrix(object,m){object.matrix.set(m[0],m[2],0,m[4],m[1],m[3],0,m[5],0,0,1,0,0,0,0,1);object.matrixWorldNeedsUpdate=true;}

/** Owned retained 2D scene. Simulation and input remain in the SDK worker;
 * this module only turns bounded drawing commands into GPU work. */
export class RiceGameWebGpuScene {
    static async open(options){const gpu=await createGameWebGpuRenderer(options);try{return new RiceGameWebGpuScene(gpu);}catch(error){gpu.close();throw error;}}
    constructor(gpu,{ownsGpu=true}={}){
        this.gpu=gpu;this.ownsGpu=ownsGpu;this.T=gpu.T;const T=this.T;
        // Match native Canvas2D's encoded-sRGB UI composition. 3D retains its
        // physical linear-light pipeline; this setting belongs to this 2D owner.
        gpu.renderer.outputColorSpace=T.LinearSRGBColorSpace;
        this.scene=new T.Scene();this.camera=new T.OrthographicCamera(0,640,0,360,-100,100);this.camera.position.z=10;
        this.pool=[];this.batches=[];this.batchIndex=0;this.lastBatch=null;this.paths=new Map();this.images=new Map();this.externalImages=new Map();this.geometries=new Map();this.sequence=0;this.draws=0;this.closed=false;
        this.rootLayer={scene:this.scene,bounds:null};this.currentLayer=this.rootLayer;this.layers=[];this.layerOrder=[];this.layerIndex=0;this.shadows=[];this.shadowIndex=0;this.glyphs=new Map();this.glyphMeasure=null;
        this.stats={textureUploads:0,textureUploadBytes:0,pathCompilations:0,fontRasterizations:0,externalImageFrames:0,frames:0};
    }
    geometry(key,make){
        let entry=this.geometries.get(key);
        if(!entry){entry={geometry:make(),epoch:this.sequence};if(!entry.geometry)throw Error('Game GPU geometry is empty');this.geometries.set(key,entry);}
        entry.epoch=this.sequence;return entry.geometry;
    }
    quad(uv=[0,0,1,1]){
        const T=this.T;return this.geometry('quad:'+uv.join(','),()=>{
            const g=new T.BufferGeometry();g.setAttribute('position',new T.Float32BufferAttribute([0,0,0,1,0,0,0,1,0,0,1,0,1,0,0,1,1,0],3));
            g.userData.gameQuadUv=uv;const [a,b,c,d]=uv;g.setAttribute('uv',new T.Float32BufferAttribute([a,b,c,b,a,d,a,d,c,b,c,d],2));return g;
        });
    }
    includeBounds(geometry,matrix){
        if(!geometry.boundingBox)geometry.computeBoundingBox();const box=geometry.boundingBox;
        if(!box||!Number.isFinite(box.min.x)||!Number.isFinite(box.max.x))return;
        const b=this.currentLayer.bounds||{minX:Infinity,minY:Infinity,maxX:-Infinity,maxY:-Infinity};
        for(const [x,y]of [[box.min.x,box.min.y],[box.min.x,box.max.y],[box.max.x,box.min.y],[box.max.x,box.max.y]]){const px=matrix[0]*x+matrix[2]*y+matrix[4],py=matrix[1]*x+matrix[3]*y+matrix[5];b.minX=Math.min(b.minX,px);b.minY=Math.min(b.minY,py);b.maxX=Math.max(b.maxX,px);b.maxY=Math.max(b.maxY,py);}this.currentLayer.bounds=b;
    }
    paint(geometry,matrix,color,alpha,clip,{texture=null,mask=null,group=null,gradient=null,shadow=null,nodeMaterial=null}={}){
        if(this.draws>=MAX_DRAWS)throw RangeError('Game GPU scene exceeds its draw budget');
        const T=this.T,index=this.draws++;
        if(geometry.userData.gameQuadUv&&!mask&&!group&&!gradient&&!shadow&&!nodeMaterial){
            let batch=this.lastBatch;
            if(!batch||batch.scene!==this.currentLayer.scene||batch.texture!==texture||batch.clip!==clip){batch=this.batches[this.batchIndex]??=new GameWebGpuQuadBatch(T);this.batchIndex++;batch.prepare(texture,clip,this.currentLayer.scene,index);this.lastBatch=batch;}
            const mesh=batch.append(matrix,color,alpha,geometry.userData.gameQuadUv);if((color>>>24)>0&&alpha>0)this.includeBounds(geometry,matrix);return mesh;
        }
        this.lastBatch=null;let object=this.pool[index];
        if(!object){const material=new T.MeshBasicMaterial({transparent:true,depthTest:false,depthWrite:false,side:T.DoubleSide,forceSinglePass:true,toneMapped:false});object=new T.Mesh(geometry,material);object.matrixAutoUpdate=false;object.frustumCulled=false;object.userData.baseMaterial=material;object.userData.groupMaterial=null;object.userData.gradientMaterial=null;object.userData.shadowMaterial=null;this.pool[index]=object;}
        if(object.parent!==this.currentLayer.scene)this.currentLayer.scene.add(object);
        object.userData.gameDrawIndex=index;
        const geometryChanged=object.geometry!==geometry;object.visible=true;object.geometry=geometry;setMatrix(object,matrix);
        if(nodeMaterial){object.material=nodeMaterial;}else if(shadow){
            let entry=object.userData.shadowMaterial;
            if(!entry){const style=createGameGpuShadow(T),material=new T.MeshBasicNodeMaterial({transparent:true,depthTest:false,depthWrite:false,side:T.DoubleSide,forceSinglePass:true,toneMapped:false});material.fragmentNode=style.fragment;entry={style,material};object.userData.shadowMaterial=entry;}
            entry.style.update(shadow);object.material=entry.material;
        }else if(gradient){
            const key=gradient.kind+':'+gradient.stops.length;let entry=object.userData.gradientMaterial;
            if(entry?.key!==key){entry?.material.dispose();const style=createGameGpuGradient(T,gradient.kind,gradient.stops.length),material=new T.MeshBasicNodeMaterial({transparent:true,depthTest:false,depthWrite:false,side:T.DoubleSide,forceSinglePass:true,toneMapped:false});material.fragmentNode=style.fragment;entry={key,material,style};object.userData.gradientMaterial=entry;}
            entry.style.update(gradient);object.material=entry.material;
        }else if(group){
            let entry=object.userData.groupMaterial;
            // A retired target may later be recreated for this group slot. The
            // texture node must follow that lifetime, rather than the slot id.
            if(!entry){
                const material=new T.MeshBasicNodeMaterial({transparent:true,depthTest:false,depthWrite:false,side:T.DoubleSide,forceSinglePass:true,toneMapped:false});
                const sample=N.texture(group.target.texture),opacity=N.uniform(1);
                material.fragmentNode=N.vec4(sample.rgb.div(N.max(sample.a,N.float(.0000001))),sample.a.mul(opacity));
                entry={material,sample,opacity};object.userData.groupMaterial=entry;
            }
            entry.sample.value=group.target.texture;entry.opacity.value=group.opacityNode.value;object.material=entry.material;
        }else object.material=object.userData.baseMaterial;
        const material=object.material;
        if(geometryChanged)material.needsUpdate=true;
        if(Boolean(material.map)!==Boolean(texture))material.needsUpdate=true;
        material.map=texture;material.color.setRGB((color&255)/255,((color>>>8)&255)/255,((color>>>16)&255)/255,T.LinearSRGBColorSpace);
        material.opacity=((color>>>24)/255)*alpha;material.colorWrite=!mask;
        material.stencilWrite=true;material.stencilFunc=T.EqualStencilFunc;material.stencilRef=clip;material.stencilFuncMask=255;
        material.stencilFail=T.KeepStencilOp;material.stencilZFail=T.KeepStencilOp;
        material.stencilZPass=mask==='push'?T.IncrementStencilOp:mask==='pop'?T.DecrementStencilOp:T.KeepStencilOp;
        if(!mask&&material.opacity>0)this.includeBounds(geometry,matrix);
        return object;
    }
    startLayer(opacity){
        const index=this.layerIndex++;if(index>=128)throw RangeError('Game GPU opacity group budget exceeded');
        let layer=this.layers[index];if(!layer){layer={index,scene:new this.T.Scene(),camera:new this.T.OrthographicCamera(0,1,0,1,-100,100),target:null,opacityNode:N.uniform(opacity)};layer.camera.position.z=10;this.layers[index]=layer;}
        layer.bounds=null;layer.opacityNode.value=opacity;layer.epoch=this.sequence;return layer;
    }
    allocateLayer(layer,bounds,{clamp=true,scaleX=this.gpu.canvas.width/this.logicalWidth,scaleY=this.gpu.canvas.height/this.logicalHeight}={}){
        const sx=scaleX,sy=scaleY;
        const x=clamp?Math.max(0,Math.floor(bounds.minX*sx)/sx):Math.floor(bounds.minX*sx)/sx,y=clamp?Math.max(0,Math.floor(bounds.minY*sy)/sy):Math.floor(bounds.minY*sy)/sy;
        const right=clamp?Math.min(this.logicalWidth,Math.ceil(bounds.maxX*sx)/sx):Math.ceil(bounds.maxX*sx)/sx,bottom=clamp?Math.min(this.logicalHeight,Math.ceil(bounds.maxY*sy)/sy):Math.ceil(bounds.maxY*sy)/sy;
        if(right<=x||bottom<=y)return false;
        const rawWidth=right-x,rawHeight=bottom-y,downsample=Math.min(1,Math.sqrt(2073600/(rawWidth*sx*rawHeight*sy)),4096/(rawWidth*sx),4096/(rawHeight*sy));
        const widthScale=sx*downsample,heightScale=sy*downsample;
        const w=Math.max(1,Math.ceil(rawWidth*widthScale-1e-7)),h=Math.max(1,Math.ceil(rawHeight*heightScale-1e-7));
        if(!layer.target){layer.target=new this.T.RenderTarget(w,h,{type:this.T.HalfFloatType,depthBuffer:true,stencilBuffer:true,samples:4});layer.target.texture.colorSpace=this.T.LinearSRGBColorSpace;}
        else if(layer.target.width<w||layer.target.height<h){
            const growWidth=Math.max(w,layer.target.width),growHeight=Math.max(h,layer.target.height);
            layer.target.setSize(growWidth*growHeight<=2073600?growWidth:w,growWidth*growHeight<=2073600?growHeight:h);
        }
        const logicalWidth=clamp?layer.target.width/widthScale:rawWidth,logicalHeight=clamp?layer.target.height/heightScale:rawHeight;
        layer.rect=[x,y,logicalWidth,logicalHeight];layer.pixelScale=[layer.target.width/logicalWidth,layer.target.height/logicalHeight];Object.assign(layer.camera,{left:x,right:x+logicalWidth,top:y,bottom:y+logicalHeight});layer.camera.updateProjectionMatrix();this.layerOrder.push(layer);return true;
    }
    finishLayer(layer,parent,clip){
        this.currentLayer=parent;if(!layer.bounds||layer.opacityNode.value===0||!this.allocateLayer(layer,layer.bounds))return;
        this.rect(layer.rect,IDENTITY,0xffffffff,1,clip,{group:layer});
    }
    finishShadow(scope){
        const {source,parent,matrix,clip,alpha,box,offset,blur,color,affine}=scope;this.currentLayer=parent;
        // Native shadows capture local content inside declared bounds, before
        // applying the optional shadow affine and the parent's transform.
        if(!this.allocateLayer(source,{minX:box[0]-1,minY:box[1]-1,maxX:box[0]+box[2]+1,maxY:box[1]+box[3]+1},{clamp:false}))return;
        if(color>>>24){
            const [x,y,w,h]=source.rect,corners=[[x,y],[x+w,y],[x,y+h],[x+w,y+h]].map(([x,y])=>[affine[0]*x+affine[2]*y+affine[4],affine[1]*x+affine[3]*y+affine[5]]),pad=Math.ceil(blur*4)+1;
            const mask=this.startLayer(1);mask.bounds={minX:Math.min(...corners.map(p=>p[0]))-pad,minY:Math.min(...corners.map(p=>p[1]))-pad,maxX:Math.max(...corners.map(p=>p[0]))+pad,maxY:Math.max(...corners.map(p=>p[1]))+pad};
            if(this.allocateLayer(mask,mask.bounds,{clamp:false})){
                this.currentLayer=mask;this.rect(source.rect,affine,0xffffffff,1,0,{group:source});this.currentLayer=parent;
                const index=this.shadowIndex++;let effect=this.shadows[index];
                if(!effect){const sample=N.texture(mask.target.texture),radius=N.uniform(new this.T.Vector2()),tint=N.uniform(new this.T.Vector4()),blurNode=gaussianBlur(sample,radius,4),material=new this.T.MeshBasicNodeMaterial({transparent:true,depthTest:false,depthWrite:false,side:this.T.DoubleSide,forceSinglePass:true,toneMapped:false});material.fragmentNode=N.vec4(tint.rgb,blurNode.a.mul(tint.a));effect={sample,radius,tint,blurNode,material};this.shadows[index]=effect;}
                effect.sample.value=mask.target.texture;effect.radius.value.set(blur*mask.pixelScale[0]/(11/3),blur*mask.pixelScale[1]/(11/3));effect.tint.value.set((color&255)/255,((color>>>8)&255)/255,((color>>>16)&255)/255,(color>>>24)/255*alpha);effect.epoch=this.sequence;effect.released=false;
                this.rect([mask.rect[0]+offset[0],mask.rect[1]+offset[1],mask.rect[2],mask.rect[3]],matrix,0xffffffff,1,clip,{nodeMaterial:effect.material});
            }
        }
        source.opacityNode.value=alpha;this.rect(source.rect,matrix,0xffffffff,1,clip,{group:source});
    }
    rect(rect,matrix,color,alpha,clip,options){return this.paint(this.quad(),multiply(matrix,[rect[2],0,0,rect[3],rect[0],rect[1]]),color,alpha,clip,options);}
    path(id,view,offset,length){
        const definition=new Uint8Array(view.buffer,view.byteOffset+offset,length),previous=this.paths.get(id);
        if(previous&&same(previous.definition,definition)){previous.epoch=this.sequence;return previous;}
        const path=new this.T.ShapePath(),count=view.getUint32(offset+12,true);
        for(let i=0;i<count;i++){
            const at=offset+16+i*28,verb=view.getUint8(at),v=Array.from({length:6},(_,n)=>view.getFloat32(at+4+n*4,true));
            if(verb===0)path.moveTo(v[0],v[1]);else if(verb===1)path.lineTo(v[0],v[1]);else if(verb===2)path.quadraticCurveTo(...v.slice(0,4));else if(verb===3)path.bezierCurveTo(...v);else if(verb===4)path.currentPath.closePath();else throw Error('Invalid game GPU path verb');
        }
        if(previous){previous.fill?.dispose();previous.evenOdd?.dispose();for(const g of previous.strokes.values())g.dispose();}
        const entry={definition:definition.slice(),path,fill:null,evenOdd:null,strokes:new Map(),epoch:this.sequence};this.paths.set(id,entry);this.stats.pathCompilations++;return entry;
    }
    fill(path,evenOdd=false){
        // Native glyphs and compound paths can place holes before their outer
        // contour. Classify by the actual fill rule, rather than path order.
        const key=evenOdd?'evenOdd':'fill';
        if(!path[key]){path.path.userData={style:{fillRule:evenOdd?'evenodd':'nonzero'}};path[key]=new this.T.ShapeGeometry(SVGLoader.createShapes(path.path),20);}
        return path[key];
    }
    roundClip(rect,radius){
        const [x,y,w,h]=rect,r=Math.max(0,Math.min(radius,32,Math.floor(Math.min(w,h)/2)));
        if(!r)return {geometry:this.quad(),matrix:[w,0,0,h,x,y]};
        const geometry=this.geometry(['round-clip',w,h,r].join(':'),()=>{
            const p=new this.T.Shape();p.moveTo(r,0);p.lineTo(w-r,0);p.absarc(w-r,r,r,-Math.PI/2,0);p.lineTo(w,h-r);p.absarc(w-r,h-r,r,0,Math.PI/2);p.lineTo(r,h);p.absarc(r,h-r,r,Math.PI/2,Math.PI);p.lineTo(0,r);p.absarc(r,r,r,Math.PI,Math.PI*1.5);p.closePath();return new this.T.ShapeGeometry(p,16);
        });return {geometry,matrix:[1,0,0,1,x,y]};
    }
    stroke(path,width,cap=0,join=0,miter=4){
        const key=[width,cap,join,miter].join(',');let geometry=path.strokes.get(key);if(geometry)return geometry;
        const positions=[];
        for(const curve of path.path.subPaths){
            const points=curve.getPoints(20);if(points.length<2)continue;
            const part=SVGLoader.pointsToStroke(points,{strokeWidth:width,strokeLineCap:['butt','round','square'][cap],strokeLineJoin:['bevel','miter','round'][join],strokeMiterLimit:miter},12,.001);
            if(part){positions.push(...part.attributes.position.array);part.dispose();}
        }
        geometry=new this.T.BufferGeometry();geometry.setAttribute('position',new this.T.Float32BufferAttribute(positions,3));path.strokes.set(key,geometry);return geometry;
    }
    bindExternalImage(id,bitmap){
        if(!Number.isInteger(id)||id<1||id>0xffffffff||!bitmap||!Number.isInteger(bitmap.width)||!Number.isInteger(bitmap.height)||bitmap.width<1||bitmap.height<1||bitmap.width*bitmap.height>1280*720)throw Error('Invalid owned game image');
        let entry=this.externalImages.get(id);
        if(!entry){const texture=new this.T.Texture(bitmap);texture.colorSpace=this.T.NoColorSpace;texture.flipY=false;texture.generateMipmaps=false;texture.minFilter=texture.magFilter=this.T.LinearFilter;entry={texture,bitmap:null};this.externalImages.set(id,entry);}
        const old=entry.bitmap;entry.bitmap=bitmap;entry.texture.image=bitmap;entry.texture.needsUpdate=true;old?.close();this.stats.externalImageFrames++;
    }
    unbindExternalImage(id){const entry=this.externalImages.get(id);if(!entry)return;this.externalImages.delete(id);entry.texture.dispose();entry.bitmap?.close();}
    texture(id,view,offset,smoothing){
        const external=this.externalImages.get(id);if(external)return external.texture;
        const image=this.images.get(id);if(!image)throw Error('Undefined game GPU image');const variant=smoothing?'linear':'nearest';let texture=image.variants.get(variant);
        if(!texture){const T=this.T;texture=new T.DataTexture(image.pixels,image.width,image.height,T.RGBAFormat,T.UnsignedByteType);texture.colorSpace=T.NoColorSpace;texture.flipY=false;texture.generateMipmaps=false;texture.minFilter=texture.magFilter=smoothing?T.LinearFilter:T.NearestFilter;texture.needsUpdate=true;image.variants.set(variant,texture);this.stats.textureUploads++;this.stats.textureUploadBytes+=image.pixels.length;}
        image.epoch=this.sequence;return texture;
    }
    image(id,width,height,pixels){
        let image=this.images.get(id);
        if(image&&image.width===width&&image.height===height&&same(image.pixels,pixels)){image.epoch=this.sequence;return;}
        if(image)for(const texture of image.variants.values())texture.dispose();
        image={width,height,pixels:pixels.slice(),variants:new Map(),epoch:this.sequence};this.images.set(id,image);
    }
    glyph(font,scalar,color){
        const key=[font,scalar,color].join(':');let glyph=this.glyphs.get(key);
        if(glyph){glyph.epoch=this.sequence;return glyph;}
        if(this.glyphs.size>=512){for(const [id,g]of this.glyphs)if(g.epoch!==this.sequence){g.texture.dispose();g.canvas.width=g.canvas.height=0;this.glyphs.delete(id);if(this.glyphs.size<256)break;}if(this.glyphs.size>=512)throw RangeError('Game glyph preparation budget exceeded');}
        const size=Math.max(1,font&255),weight=font&256?700:400,italic=font&512?'italic ':'',family=['"RiceWM Sans", sans-serif','"RiceWM Serif", Georgia, serif','"RiceWM Mono", ui-monospace, monospace','"RiceWM Cursive", cursive'][(font>>10)&3],css=italic+weight+' '+size+'px '+family;
        // Font shaping/raster preparation is cached asset work, never a
        // per-frame CPU game surface or a GPU framebuffer readback.
        this.glyphMeasure??=new OffscreenCanvas(1,1).getContext('2d');const measure=this.glyphMeasure;measure.font=css;const text=String.fromCodePoint(scalar),metrics=measure.measureText(text),left=metrics.actualBoundingBoxLeft??0,right=metrics.actualBoundingBoxRight??metrics.width,ascent=metrics.actualBoundingBoxAscent??size,descent=metrics.actualBoundingBoxDescent??size*.25;
        const width=Math.max(1,Math.ceil(left+right)+4),height=Math.max(1,Math.ceil(ascent+descent)+4),canvas=new OffscreenCanvas(width,height),context=canvas.getContext('2d');context.font=css;context.textBaseline='alphabetic';context.fillStyle=`rgba(${color&255},${color>>>8&255},${color>>>16&255},${(color>>>24)/255})`;context.fillText(text,left+2,ascent+2);
        const texture=new this.T.CanvasTexture(canvas);texture.colorSpace=this.T.NoColorSpace;texture.flipY=false;texture.generateMipmaps=false;texture.minFilter=texture.magFilter=this.T.LinearFilter;
        glyph={texture,canvas,left:left+2,ascent:ascent+2,width,height,epoch:this.sequence};this.glyphs.set(key,glyph);this.stats.fontRasterizations++;return glyph;
    }
    compile(frame){
        // This private host adapter receives immutable scenes only after the
        // existing authoritative native ingress. Do not copy a full atlas or
        // create a second app-facing drawing protocol here.
        const {width,height,displayList}=frame;
        if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>16384||height>16384||!(displayList instanceof Uint8Array)||displayList.length>8*1024*1024-56)throw Error('Invalid native game frame');
        const view=new DataView(displayList.buffer,displayList.byteOffset,displayList.byteLength);this.sequence++;this.draws=0;this.layerIndex=0;this.shadowIndex=0;this.batchIndex=0;this.lastBatch=null;for(const object of this.pool)if(object)object.visible=false;for(const batch of this.batches)batch.hide();this.layerOrder=[];this.currentLayer=this.rootLayer;this.rootLayer.bounds=null;this.logicalWidth=width;this.logicalHeight=height;
        this.camera.right=width;this.camera.bottom=height;this.camera.updateProjectionMatrix();
        let matrix=IDENTITY,alpha=1,clip=0;const stack=[],clips=[],definedPaths=new Set(),definedImages=new Set();
        for(let offset=0;offset<displayList.length;){
            const compact=view.getUint16(offset+2,true),length=compact?compact*4:view.getUint32(offset+4,true),op=view.getUint8(offset);
            const f=at=>view.getFloat32(offset+at,true),u=at=>view.getUint32(offset+at,true),rect=()=>[8,12,16,20].map(f);
            if(op===29){
                const source=this.startLayer(1);stack.push({kind:'shadow',source,parent:this.currentLayer,matrix,alpha,clip,box:rect(),offset:[f(24),f(28)],blur:f(32),color:u(36),affine:length===64?[40,44,48,52,56,60].map(f):IDENTITY});
                this.currentLayer=source;matrix=IDENTITY;alpha=1;clip=0;
            }else if(op===30){
                const scope=stack.pop();if(scope?.kind!=='shadow')throw Error('Game GPU shadow stack mismatch');this.finishShadow(scope);({matrix,alpha,clip}=scope);
            }else if(op===1){
                const previous={matrix,alpha,kind:'transform'};
                if(length===40){
                    if(view.getUint8(offset+36)!==0||view.getUint8(offset+37)!==3)throw Error('Game GPU isolated blend mode not implemented');
                    previous.layer=this.startLayer(f(32)*alpha);previous.parent=this.currentLayer;previous.clip=clip;this.currentLayer=previous.layer;clip=0;alpha=1;
                }
                stack.push(previous);if(length===36)alpha*=f(32);matrix=multiply(matrix,[8,12,16,20,24,28].map(f));
            }else if(op===2){const previous=stack.pop();if(previous?.kind!=='transform')throw Error('Game GPU transform stack mismatch');({matrix,alpha}=previous);if(previous.layer){this.finishLayer(previous.layer,previous.parent,previous.clip);clip=previous.clip;}
            }else if(op===3||op===20){
                if(clip>=254)throw RangeError('Game GPU clip budget exceeded');
                let geometry,transform;
                if(op===3){const rounded=this.roundClip(rect(),Math.max(...[24,28,32,36].map(f)));geometry=rounded.geometry;transform=multiply(matrix,rounded.matrix);}
                else{if(!definedPaths.has(u(8)))throw Error('Undefined game GPU clip path');geometry=this.fill(this.paths.get(u(8)),view.getUint8(offset+36)===1);transform=multiply(matrix,[12,16,20,24,28,32].map(f));}
                this.paint(geometry,transform,0xffffffff,1,clip,{mask:'push'});clips.push({geometry,transform});clip++;stack.push({kind:'clip'});
            }else if(op===4){if(stack.pop()?.kind!=='clip')throw Error('Game GPU clip stack mismatch');const previous=clips.pop();this.paint(previous.geometry,previous.transform,0xffffffff,1,clip,{mask:'pop'});clip--;
            }else if(op===5){this.rect(rect(),matrix,u(24),alpha,clip);
            }else if(op===6||op===25||op===26){
                const countAt=op===6?40:op===25?48:44,count=view.getUint16(offset+countAt,true),stops=[];
                if(count<2||count>64||length!==countAt+4+count*8)throw Error('Invalid game GPU gradient record');
                for(let i=0;i<count;i++)stops.push([f(countAt+4+i*8),u(countAt+8+i*8)]);
                let from=op===6?[f(24),f(28)]:op===25?[f(24),f(28),f(32)]:[f(24),f(28),f(32),f(36)],to=op===6?[f(32),f(36)]:op===25?[f(36),f(40),f(44)]:[];
                if(op===26&&from[3]<from[2]){[from[2],from[3]]=[from[3],from[2]];stops.reverse();for(const stop of stops)stop[0]=1-stop[0];}
                this.rect(rect(),matrix,0xffffffff,alpha,clip,{gradient:{kind:op,rect:rect(),from,to,stops,alpha,extension:op===26?view.getUint8(offset+40):0}});
            }else if(op===14){this.image(u(8),u(12),u(16),new Uint8Array(displayList.buffer,displayList.byteOffset+offset+24,u(20)));definedImages.add(u(8));
            }else if(op===7){
                const id=u(40);if(!definedImages.has(id))throw Error('Undefined game GPU image');const r=rect(),texture=this.texture(id,view,offset,view.getUint8(offset+48)===1),geometry=this.quad([24,28,32,36].map(f));
                this.paint(geometry,multiply(matrix,[r[2],0,0,r[3],r[0],r[1]]),0xffffffff,alpha*f(44),clip,{texture});
            }else if(op===19){this.path(u(8),view,offset,length);definedPaths.add(u(8));
            }else if(op===21||op===22){
                const id=u(8);if(!definedPaths.has(id))throw Error('Undefined game GPU path');const path=this.paths.get(id);path.epoch=this.sequence;
                const geometry=op===21?this.fill(path,view.getUint8(offset+40)===1):this.stroke(path,f(40),view.getUint8(offset+48),view.getUint8(offset+49),f(44));
                this.paint(geometry,multiply(matrix,[12,16,20,24,28,32].map(f)),u(36),alpha,clip);
            }else if(op===8){
                const count=u(24),font=u(8),color=u(20);
                for(let i=0;i<count;i++){const at=28+i*12,glyph=this.glyph(font,u(at),color);this.rect([f(12)+f(at+4)-glyph.left,f(16)+f(at+8)-glyph.ascent,glyph.width,glyph.height],matrix,0xffffffff,alpha,clip,{texture:glyph.texture});}
            }else if(op===15){
                // Hit regions remain authoritative in native input routing.
            }else if(op===9){
                const [x,y,w,h]=rect(),edges=[24,28,32,36].map(f),colors=[40,44,48,52].map(u);
                for(const [i,r]of [[0,[x,y,w,edges[0]]],[1,[x+w-edges[1],y,edges[1],h]],[2,[x,y+h-edges[2],w,edges[2]]],[3,[x,y,edges[3],h]]])this.rect(r,matrix,colors[i],alpha,clip);
            }else if(op===10){
                const [x,y,width,height]=rect(),delta=[f(24),f(28)],blur=f(32),spread=f(36),rgba=u(40),inside=view.getUint8(offset+44)===1;
                if(width>0&&height>0&&(rgba>>>24)>0&&(inside||(width+2*spread>0&&height+2*spread>0))){
                    const pad=blur*4,r=inside?[x,y,width,height]:[x+delta[0]-spread-pad,y+delta[1]-spread-pad,width+2*spread+2*pad,height+2*spread+2*pad];
                    this.rect(r,matrix,0xffffffff,alpha,clip,{shadow:{width,height,offset:delta,blur,spread,rgba,alpha,inside}});
                }
            }else if(op===12){
                const count=view.getUint16(offset+16,true),flags=view.getUint8(offset+18),points=[];
                for(let i=0;i<count;i++)points.push([f(20+i*8),f(24+i*8)]);
                const key=['polygon',flags,f(12),...points.flat()].join(','),g=this.geometry(key,()=>{
                    const p=new this.T.ShapePath();points.forEach(([x,y],i)=>i?p.lineTo(x,y):p.moveTo(x,y));if(flags&2)p.currentPath.closePath();
                    return flags&1?new this.T.ShapeGeometry(p.toShapes(false)):SVGLoader.pointsToStroke(p.currentPath.getPoints(),{strokeWidth:f(12),strokeLineCap:'butt',strokeLineJoin:'miter',strokeMiterLimit:10});
                });this.paint(g,matrix,u(8),alpha,clip);
            }else if(op===11){
                const x=f(8),y=f(12),dx=f(16)-x,dy=f(20)-y,w=f(24),distance=Math.hypot(dx,dy),style=view.getUint8(offset+32);
                if(distance&&w){const c=dx/distance,s=dy/distance,on=style===1?w:style===2?w*3:distance,off=style===1?w:style===2?w*2:0;
                    for(let at=0;at<distance;at+=on+off){const length=Math.min(on,distance-at);this.paint(this.quad(),multiply(matrix,[c*length,s*length,-s*w,c*w,x+c*at+s*w/2,y+s*at-c*w/2]),u(28),alpha,clip);}
                }
            }else if(op===13){
                const r=rect(),filled=view.getUint8(offset+40)===1,from=f(24)*Math.PI/180,to=f(28)*Math.PI/180,stroke=f(32);
                const key=['ellipse',...r.slice(2),from,to,filled,stroke].join(',');const geometry=this.geometry(key,()=>{
                    const p=new this.T.ShapePath();const rx=Math.abs(r[2]/2),ry=Math.abs(r[3]/2);p.moveTo(rx+rx*Math.cos(from),ry+ry*Math.sin(from));p.currentPath.absellipse(rx,ry,rx,ry,from,to,false,0);
                    return filled?new this.T.ShapeGeometry(p.toShapes(false),Math.max(24,Math.min(128,Math.ceil(Math.sqrt(Math.max(rx,ry))*5)))):SVGLoader.pointsToStroke(p.currentPath.getPoints(96),{strokeWidth:stroke,strokeLineCap:'butt',strokeLineJoin:'round',strokeMiterLimit:4},12,.001);
                });this.paint(geometry,multiply(matrix,[1,0,0,1,r[0],r[1]]),u(36),alpha,clip);
            }else throw Error('Game GPU scene opcode '+op+' is not implemented');
            offset+=length;
        }
        if(stack.length||clip)throw Error('Unbalanced game GPU scene');
        for(let i=0;i<this.batchIndex;i++)this.batches[i].finish();
        for(const[id,image]of this.images)if(!definedImages.has(id)){for(const t of image.variants.values())t.dispose();this.images.delete(id);}
        for(const[id,path]of this.paths)if(!definedPaths.has(id)){path.fill?.dispose();path.evenOdd?.dispose();for(const g of path.strokes.values())g.dispose();this.paths.delete(id);}
        if(this.geometries.size>MAX_GEOMETRIES)for(const[key,entry]of this.geometries)if(entry.epoch!==this.sequence){entry.geometry.dispose();this.geometries.delete(key);if(this.geometries.size<=MAX_GEOMETRIES)break;}
        for(const layer of [this.rootLayer,...this.layers])layer.scene.children.sort((a,b)=>a.userData.gameDrawIndex-b.userData.gameDrawIndex);
        for(const layer of this.layers)if(layer.epoch!==this.sequence&&layer.target){layer.target.dispose();layer.target=null;}
        for(let i=0;i<this.shadows.length;i++){const effect=this.shadows[i];if(effect&&effect.epoch!==this.sequence&&!effect.released){effect.blurNode.dispose();effect.released=true;}}
        return {width,height,draws:this.draws};
    }
    async render(frame){
        if(this.closed)throw Error('Game GPU scene closed');
        const compiled=this.compile(frame);this.gpu.beginFrame();const renderer=this.gpu.renderer;
        try{renderer.setClearColor(0x000000,0);
        for(const layer of this.layerOrder){renderer.setRenderTarget(layer.target);renderer.render(layer.scene,layer.camera);}
        renderer.setRenderTarget(null);renderer.render(this.scene,this.camera);await this.gpu.completeFrame();this.stats.frames++;return {...compiled,...this.gpu.inspect(),...this.stats};
        }catch(error){try{await this.gpu.abortFrame();}catch{}throw error;}
    }
    capture(){return this.gpu.capture();}
    inspect(){return {...this.gpu.inspect(),...this.stats,draws:this.draws,quadBatches:this.batchIndex};}
    close(){if(this.closed)return;this.closed=true;for(const batch of this.batches)batch.close();for(const o of this.pool){if(!o)continue;o.userData.baseMaterial.dispose();o.userData.gradientMaterial?.material.dispose();o.userData.groupMaterial?.material.dispose();o.userData.shadowMaterial?.material.dispose();}for(const layer of this.layers)layer.target?.dispose();for(const effect of this.shadows){if(!effect)continue;effect.material.dispose();effect.blurNode._material?.dispose();effect.blurNode.dispose();}for(const p of this.paths.values()){p.fill?.dispose();p.evenOdd?.dispose();for(const g of p.strokes.values())g.dispose();}for(const i of this.images.values())for(const t of i.variants.values())t.dispose();for(const g of this.geometries.values())g.geometry.dispose();for(const g of this.glyphs.values()){g.texture.dispose();g.canvas.width=g.canvas.height=0;}this.glyphMeasure=null;for(const id of [...this.externalImages.keys()])this.unbindExternalImage(id);if(this.ownsGpu)this.gpu.close();}
}
