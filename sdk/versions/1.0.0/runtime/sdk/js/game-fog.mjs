import { GameTileWorld } from './game-world.mjs';
/** Persistent exploration and current, occluded sight. Hidden entities must be
 * omitted from world rendering and selection; the fog image is presentation. */
export class GameFogOfWar {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameFogOfWar(world,{teams:2})',methods:'update([{column,row,radius:8,team:0}],{opaque:(c,r)=>!world.walkable(c,r)}) recomputes current vision with occluded tile rays and retains exploration. state(column,row,team=0) is unexplored|explored|visible. visible/explored return booleans. reset(team?) forgets exploration. Up to 8 teams, 256 sight sources, radius 0..32 and 2 million ray steps per update. Use live units/buildings as sources; update on tile/obstacle changes. Omit hidden enemies from rendering, selection, minimap and targeting; dim explored terrain and retain only explicitly remembered buildings.'});
  constructor(world,{teams=2}={}){if(!(world instanceof GameTileWorld)||!Number.isInteger(teams)||teams<1||teams>8)throw RangeError('Invalid fog world/teams');this.world=world;this.current=Array.from({length:teams},()=>new Uint8Array(world.tiles.length));this.known=Array.from({length:teams},()=>new Uint8Array(world.tiles.length));}
  team(team){if(!Number.isInteger(team)||team<0||team>=this.current.length)throw RangeError('Invalid fog team');return team;}
  index(column,row){return this.world.tile(column,row)===null?-1:row*this.world.columns+column;}
  visible(column,row,team=0){this.team(team);const at=this.index(column,row);return at>=0&&!!this.current[team][at];}
  explored(column,row,team=0){this.team(team);const at=this.index(column,row);return at>=0&&!!this.known[team][at];}
  state(column,row,team=0){return this.visible(column,row,team)?'visible':this.explored(column,row,team)?'explored':'unexplored';}
  reset(team){for(const index of team===undefined?this.current.map((_,i)=>i):[this.team(team)]){this.current[index].fill(0);this.known[index].fill(0);}}
  update(sources,{opaque=(column,row)=>!this.world.walkable(column,row)}={}){
    if(!Array.isArray(sources)||sources.length>256||typeof opaque!=='function')throw RangeError('Invalid fog sources');
    for(const source of sources)if(!source||!Number.isInteger(source.column)||!Number.isInteger(source.row)||this.index(source.column,source.row)<0||!Number.isFinite(source.radius)||source.radius<0||source.radius>32)throw RangeError('Invalid sight source');else this.team(source.team??0);
    const current=this.current.map(array=>new Uint8Array(array.length));let work=0;
    for(const {column,row,radius,team=0}of sources){
      const extent=Math.ceil(radius);
      for(let r=Math.max(0,row-extent);r<=Math.min(this.world.rows-1,row+extent);r++)for(let c=Math.max(0,column-extent);c<=Math.min(this.world.columns-1,column+extent);c++){
        if((c-column)**2+(r-row)**2>radius*radius)continue;
        let x=column,y=row,dx=Math.abs(c-column),dy=-Math.abs(r-row),sx=column<c?1:-1,sy=row<r?1:-1,error=dx+dy,seen=true;
        while(x!==c||y!==r){if(++work>2000000)throw RangeError('Fog visibility work budget exceeded');const twice=2*error;if(twice>=dy){error+=dy;x+=sx;}if(twice<=dx){error+=dx;y+=sy;}if((x!==c||y!==r)&&opaque(x,y)){seen=false;break;}}
        if(seen)current[team][r*this.world.columns+c]=1;
      }
    }
    this.current=current;for(let team=0;team<current.length;team++)for(let i=0;i<current[team].length;i++)if(current[team][i])this.known[team][i]=1;
  }
}
export function rasterizeGameFog(fog,{team=0,camera={},width=640,height=360,resolution=0.5,unexploredAlpha=0.98,exploredAlpha=0.6,color=[12,18,23]}={}){
  if(!(fog instanceof GameFogOfWar)||![width,height,resolution,unexploredAlpha,exploredAlpha].every(Number.isFinite)||width<1||height<1||resolution<=0||resolution>1||unexploredAlpha<0||unexploredAlpha>1||exploredAlpha<0||exploredAlpha>1||!Array.isArray(color)||color.length!==3||color.some(n=>!Number.isInteger(n)||n<0||n>255))throw RangeError('Invalid fog raster');fog.team(team);
  const w=Math.ceil(width*resolution),h=Math.ceil(height*resolution);if(w*h>262144)throw RangeError('Fog raster exceeds pixel budget');
  const rgba=new Uint8Array(w*h*4),p=fog.world.cameraProjection(camera),tw=p.tileWidth??64,th=p.tileHeight??32,alpha=(c,r)=>fog.visible(c,r,team)?0:fog.explored(c,r,team)?exploredAlpha:unexploredAlpha;
  for(let y=0;y<h;y++)for(let x=0;x<w;x++){
    const dx=((x+0.5)/resolution-p.originX)/tw,dy=((y+0.5)/resolution-p.originY)/th,c=dy+dx-0.5,r=dy-dx-0.5,left=Math.floor(c),top=Math.floor(r),fx=c-left,fy=r-top;
    const value=alpha(left,top)*(1-fx)*(1-fy)+alpha(left+1,top)*fx*(1-fy)+alpha(left,top+1)*(1-fx)*fy+alpha(left+1,top+1)*fx*fy,offset=(y*w+x)*4;
    rgba.set(color,offset);rgba[offset+3]=Math.round(value*255);
  }
  return {width:w,height:h,rgba};
}
rasterizeGameFog.controlFactory=Object.freeze({topics:['games'],constructor:'rasterizeGameFog(fog,{team:0,camera,width:640,height:360,resolution:0.5,unexploredAlpha:0.98,exploredAlpha:0.6})',methods:'Smooth bilinear native RGBA fog mask from actual occluded current vision and persistent exploration. Uses the same GameTileWorld camera as world rendering. Draw after world and before HUD with image smoothing enabled. At most 262144 pixels; cache until sight/camera changes. Hiding enemy state in drawing/selection/minimap remains mandatory; a dark overlay alone is not fog of war.'});
