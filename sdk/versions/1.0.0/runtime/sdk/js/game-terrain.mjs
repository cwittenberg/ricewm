import { GameTileWorld } from './game-world.mjs';
/** Continuous, filtered photographic terrain. Retain the result until map or
 * camera changes, instead of submitting many low-resolution diamond sprites. */
export function rasterizeGameTerrain(world,{textures,camera={},width=640,height=360,resolution=1,tileRepeats=0.25,ambient=0.95,uvOrigin={column:0,row:0}}={}){
  if(!Number.isFinite(uvOrigin?.column)||!Number.isFinite(uvOrigin?.row))throw RangeError('Invalid terrain UV origin');
  if(!(world instanceof GameTileWorld)||!textures||typeof textures!=='object'||![width,height,resolution,tileRepeats,ambient].every(Number.isFinite)||width<1||height<1||resolution<=0||resolution>2||tileRepeats<=0||tileRepeats>8||ambient<0||ambient>1)throw RangeError('Invalid photographic terrain');
  const w=Math.ceil(width*resolution),h=Math.ceil(height*resolution);if(w*h>1048576)throw RangeError('Terrain raster exceeds one megapixel');
  let totalPixels=0;for(const image of Object.values(textures)){if(!Number.isInteger(image?.width)||!Number.isInteger(image?.height)||image.width<1||image.height<1||!(image.rgba instanceof Uint8Array)||image.rgba.length!==image.width*image.height*4)throw RangeError('Invalid terrain texture');totalPixels+=image.width*image.height;}if(totalPixels>1048576)throw RangeError('Terrain texture budget exceeded');
  const p=world.cameraProjection(camera),tw=p.tileWidth??64,th=p.tileHeight??32,rgba=new Uint8Array(w*h*4);
  const sample=(image,u,v)=>{
    const x=((u%1+1)%1)*image.width,y=((v%1+1)%1)*image.height,x0=Math.floor(x),y0=Math.floor(y),fx=x-x0,fy=y-y0;
    const result=[0,0,0];for(let r=0;r<2;r++)for(let c=0;c<2;c++){const weight=(c?fx:1-fx)*(r?fy:1-fy),at=(((y0+r)%image.height)*image.width+(x0+c)%image.width)*4;for(let channel=0;channel<3;channel++)result[channel]+=image.rgba[at+channel]*weight;}return result;
  };
  for(let y=0;y<h;y++)for(let x=0;x<w;x++){
    const dx=((x+0.5)/resolution-p.originX)/tw,dy=((y+0.5)/resolution-p.originY)/th,c=dy+dx,r=dy-dx;
    if(c<0||r<0||c>=world.columns||r>=world.rows)continue;
    const left=Math.floor(c-0.5),top=Math.floor(r-0.5),fx=c-0.5-left,fy=r-0.5-top,color=[0,0,0];
    for(let oy=0;oy<2;oy++)for(let ox=0;ox<2;ox++){
      const column=Math.min(world.columns-1,Math.max(0,left+ox)),row=Math.min(world.rows-1,Math.max(0,top+oy)),tile=world.tile(column,row),image=Object.hasOwn(textures,tile)?textures[tile]:null;if(!image)throw Error('Missing terrain material: '+tile);
      const weight=(ox?fx:1-fx)*(oy?fy:1-fy),value=sample(image,(c+uvOrigin.column)*tileRepeats,(r+uvOrigin.row)*tileRepeats);for(let channel=0;channel<3;channel++)color[channel]+=value[channel]*weight;
    }
    const offset=(y*w+x)*4;for(let channel=0;channel<3;channel++)rgba[offset+channel]=Math.round(color[channel]*ambient);rgba[offset+3]=255;
  }
  return {width:w,height:h,rgba};
}
rasterizeGameTerrain.controlFactory=Object.freeze({topics:['games'],constructor:'rasterizeGameTerrain(world,{textures:{grass:decodedGrass,path:decodedPath},camera,width:640,height:360,resolution:1.5,tileRepeats:0.25,ambient:0.95})',methods:'Continuous isometric photographic diffuse terrain using camera-consistent inverse projection, bilinear image filtering, repeating world UVs and blended material transitions. Output native RGBA; draw with SceneBuilder.image smoothing:true. No visible low-resolution diamond sprite grid. Up to 1 megapixel output/total source textures. Terrain names match GameTileWorld tiles. Cache until map/camera changes. Supply uvOrigin:{column,row} for a rebased chunk snapshot to keep material UVs fixed in global space. Vegetation, props, entities, fog and HUD are separate layers; this does not supply PBR/normal/displacement shading or heightfield geometry.'});
