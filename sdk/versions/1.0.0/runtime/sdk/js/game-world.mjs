import { isometricPoint, isometricCell, GameSpriteAtlas } from './game-sprites.mjs';
const id = value => typeof value === 'string' && /^[a-zA-Z][\w.-]{0,63}$/.test(value);
const count = value => Number.isSafeInteger(value) && value > 0 && value <= 9999;

/** Bounded tile collision/pathfinding; drawing uses the same camera as picking. */
export class GameTileWorld {
  static controlFactory = Object.freeze({topics:['games'],constructor:'new GameTileWorld({columns:32,rows:32,tiles:Array(1024).fill("grass"),solid:["wall"],projection:{tileWidth:64,tileHeight:32,originX:320,originY:32}})',methods:'At most 65536 tiles. tile(column,row), walkable(column,row), set(column,row,tile), path(from,to,{maximumVisited:8192}) returns a shortest 4-neighbour path excluding start, [] at target, null if blocked/unreachable/budget exhausted. draw(scene,{atlas,imageId:1,camera:{column,row,x:320,y:160},entities:[{column,row,frame,scale:1}],viewport:[0,0,640,290],maximumDraws:2048}) culls and depth-sorts terrain/entities by foot position. Tiles/frames share GameSpriteAtlas; ground frames pivot at 0.5,0, entity feet at 0.5,1. pick(x,y,camera) reverses the exact projection. Put tall walls in entities for correct interleaving. Separate regions/chunks, NPC/combat logic and persistence remain app-owned.'});
  constructor({columns,rows,tiles,solid=[],projection={}}={}) {
    if(!Number.isInteger(columns)||!Number.isInteger(rows)||columns<1||rows<1||columns*rows>65536||!Array.isArray(tiles)||tiles.length!==columns*rows||tiles.some(t=>!id(t))||!Array.isArray(solid)||solid.length>256||solid.some(t=>!id(t)))throw RangeError('Invalid bounded tile world');
    isometricPoint(0,0,0,projection);
    this.columns=columns;this.rows=rows;this.tiles=[...tiles];this.solid=new Set(solid);this.projection={...projection};
  }
  tile(column,row){return Number.isInteger(column)&&Number.isInteger(row)&&column>=0&&row>=0&&column<this.columns&&row<this.rows?this.tiles[row*this.columns+column]:null;}
  walkable(column,row){const tile=this.tile(column,row);return tile!==null&&!this.solid.has(tile);}
  set(column,row,tile){if(this.tile(column,row)===null||!id(tile))throw RangeError('Invalid tile edit');this.tiles[row*this.columns+column]=tile;}
  path(from,to,{maximumVisited=8192}={}){
    if(!Number.isInteger(maximumVisited)||maximumVisited<1||maximumVisited>65536||!from||!to)throw RangeError('Invalid path budget or endpoints');
    if(!this.walkable(from.column,from.row)||!this.walkable(to.column,to.row))return null;
    const start=from.row*this.columns+from.column,end=to.row*this.columns+to.column;
    if(start===end)return [];
    const parents=new Int32Array(this.tiles.length).fill(-1),queue=[start];parents[start]=start;
    for(let cursor=0;cursor<queue.length&&cursor<maximumVisited;cursor++){
      const current=queue[cursor],column=current%this.columns,row=Math.floor(current/this.columns);
      for(const [dc,dr]of [[1,0],[0,1],[-1,0],[0,-1]]){
        const c=column+dc,r=row+dr,next=r*this.columns+c;
        if(!this.walkable(c,r)||parents[next]!==-1)continue;
        parents[next]=current;
        if(next===end){const result=[];for(let at=end;at!==start;at=parents[at])result.push({column:at%this.columns,row:Math.floor(at/this.columns)});return result.reverse();}
        queue.push(next);
      }
    }
    return null;
  }
  cameraProjection(camera={}){
    const {column=0,row=0,x=320,y=160}=camera;
    if(![column,row,x,y].every(Number.isFinite))throw RangeError('Invalid tile camera');
    const origin=isometricPoint(column,row,0,{...this.projection,originX:0,originY:0});
    return {...this.projection,originX:x-origin.x,originY:y-origin.y};
  }
  pick(x,y,camera){return isometricCell(x,y,this.cameraProjection(camera));}
  draw(scene,{atlas,imageId=1,camera={},entities=[],viewport=[0,0,640,290],maximumDraws=2048,smoothing=true}={}){
    if(!(atlas instanceof GameSpriteAtlas)||!Array.isArray(entities)||entities.length>1024||!Number.isInteger(maximumDraws)||maximumDraws<1||maximumDraws>4096||!Array.isArray(viewport)||viewport.length!==4||!viewport.every(Number.isFinite)||viewport[2]<=0||viewport[3]<=0)throw RangeError('Invalid world drawing budget');
    const projection=this.cameraProjection(camera),draws=[];
    const enqueue=(column,row,frame,scale=1,ground=false)=>{
      if(![column,row,scale].every(Number.isFinite)||scale<=0)throw RangeError('Invalid world entity');
      const p=isometricPoint(column,row,0,projection),spec=atlas.frames.get(frame);
      if(!spec)throw Error('Missing world atlas frame: '+frame);
      if(ground)scale=(projection.tileWidth??64)/spec.sourceWidth;
      const y=p.y+(ground?0:(projection.tileHeight??32)/2),w=spec.sourceWidth*scale,h=spec.sourceHeight*scale,left=p.x-w*spec.pivotX,top=y-h*spec.pivotY;
      if(left+w<viewport[0]||left>viewport[0]+viewport[2]||top+h<viewport[1]||top>viewport[1]+viewport[3])return;
      if(draws.length>=maximumDraws)throw RangeError('World exceeded visible draw budget');
      draws.push({x:p.x,y,frame,scale,depth:column+row,ground});
    };
    // Determine a conservative visible tile rectangle using inverse projection.
    const corners=[[viewport[0],viewport[1]],[viewport[0]+viewport[2],viewport[1]],[viewport[0],viewport[1]+viewport[3]],[viewport[0]+viewport[2],viewport[1]+viewport[3]]].map(([x,y])=>isometricCell(x,y,projection));
    const minC=Math.max(0,Math.min(...corners.map(p=>p.column))-2),maxC=Math.min(this.columns-1,Math.max(...corners.map(p=>p.column))+2);
    const minR=Math.max(0,Math.min(...corners.map(p=>p.row))-2),maxR=Math.min(this.rows-1,Math.max(...corners.map(p=>p.row))+2);
    for(let row=minR;row<=maxR;row++)for(let column=minC;column<=maxC;column++)enqueue(column,row,this.tile(column,row),1,true);
    for(const entity of entities)enqueue(entity.column,entity.row,entity.frame,entity.scale);
    draws.sort((a,b)=>a.depth-b.depth||Number(b.ground)-Number(a.ground));
    scene.pushClip(viewport);for(const item of draws)atlas.draw(scene,imageId,item.frame,{...item,smoothing});scene.popClip();
    return draws.length;
  }
}

/** Transactional stacks and equipment; UI and combat read the same definitions. */
export class GameInventory {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameInventory({items:{sword:{name:"Iron sword",slot:"weapon",stats:{attack:4},stack:1},potion:{name:"Healing draught",stack:20}},capacity:24})',methods:'add(id,quantity=1), remove(id,quantity=1), equip(id), unequip(slot) return boolean without partial mutation on failure. quantity(id), stats() sums equipped stat modifiers, entries() returns [{id,quantity,equipped,...definition}]. capacity is distinct stacks 1..128; item.stack 1..9999. Consumable effects, loot tables/rarity, vendors, gold and saves are app-owned. Reset with a new instance; do not invent equipped bonuses in HUD text.'});
  constructor({items,capacity=24}={}){
    if(!items||typeof items!=='object'||Array.isArray(items)||Object.keys(items).length>512||!Number.isInteger(capacity)||capacity<1||capacity>128)throw RangeError('Invalid inventory');
    this.items=new Map();this.stacks=new Map();this.equipment=new Map();this.capacity=capacity;
    for(const [key,value]of Object.entries(items)){
      if(!id(key)||!value||typeof value.name!=='string'||!value.name||value.name.length>128||!count(value.stack??1)||(value.slot!==undefined&&!id(value.slot)))throw RangeError('Invalid inventory definition');
      const stats=value.stats??{};
      if(typeof stats!=='object'||Array.isArray(stats)||Object.keys(stats).length>16||Object.entries(stats).some(([k,v])=>!id(k)||!Number.isFinite(v)||Math.abs(v)>9999))throw RangeError('Invalid item stats');
      this.items.set(key,{...value,stats:{...stats},stack:value.stack??1});
    }
  }
  quantity(key){return this.stacks.get(key)??0;}
  add(key,quantity=1){const item=this.items.get(key);if(!item||!count(quantity)||this.quantity(key)+quantity>item.stack||!this.stacks.has(key)&&this.stacks.size>=this.capacity)return false;this.stacks.set(key,this.quantity(key)+quantity);return true;}
  remove(key,quantity=1){if(!count(quantity)||this.quantity(key)<quantity)return false;const remaining=this.quantity(key)-quantity;if(remaining)this.stacks.set(key,remaining);else{this.stacks.delete(key);for(const [slot,equipped]of this.equipment)if(equipped===key)this.equipment.delete(slot);}return true;}
  equip(key){const item=this.items.get(key);if(!item?.slot||!this.quantity(key))return false;this.equipment.set(item.slot,key);return true;}
  unequip(slot){return this.equipment.delete(slot);}
  stats(){const result={};for(const key of this.equipment.values())for(const [stat,value]of Object.entries(this.items.get(key).stats))result[stat]=(Object.hasOwn(result,stat)?result[stat]:0)+value;return result;}
  entries(){return [...this.stacks].map(([key,quantity])=>({...this.items.get(key),stats:{...this.items.get(key).stats},id:key,quantity,equipped:[...this.equipment.values()].includes(key)}));}
}

/** Quest progress is event-driven, independent of NPC/HUD presentation. */
export class GameQuestLog {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameQuestLog([{id:"warden",title:"Break the seal",requires:[],objectives:[{event:"kill",target:"warden",count:1}]}])',methods:'accept(id), record(event,target,quantity=1), ready(id), complete(id), entries(). Prerequisite quests must be completed before acceptance; only active matching objectives advance, counters clamp to required count. complete returns true exactly once; grant app-owned reward only on true. At most 128 quests/16 objectives each. entries exposes status locked|available|active|complete, progress and objective data for journal/HUD. NPC dialogue/branches, rewards and authored story remain app-owned.'});
  constructor(quests=[]){
    if(!Array.isArray(quests)||quests.length>128)throw RangeError('Invalid quests');this.quests=new Map();
    for(const quest of quests){
      if(!id(quest?.id)||this.quests.has(quest.id)||typeof quest.title!=='string'||quest.title.length>128||!Array.isArray(quest.objectives)||!quest.objectives.length||quest.objectives.length>16||!Array.isArray(quest.requires??[])||(quest.requires??[]).length>128||(quest.requires??[]).some(r=>!id(r))||quest.objectives.some(o=>!id(o.event)||!id(o.target)||!count(o.count??1)))throw RangeError('Invalid quest definition');
      this.quests.set(quest.id,{...quest,requires:[...(quest.requires??[])],objectives:quest.objectives.map(o=>({...o,count:o.count??1})),progress:quest.objectives.map(()=>0),status:'available'});
    }
    const checked=new Set();
    const visit=(key,trail=new Set())=>{if(trail.has(key))throw Error('Quest prerequisite cycle');if(checked.has(key))return;const quest=this.quests.get(key);if(!quest)throw Error('Missing prerequisite quest');const next=new Set(trail).add(key);for(const r of quest.requires)visit(r,next);checked.add(key);};
    for(const key of this.quests.keys())visit(key);
  }
  available(quest){return quest.requires.every(key=>this.quests.get(key).status==='complete');}
  accept(key){const quest=this.quests.get(key);if(!quest||quest.status!=='available'||!this.available(quest))return false;quest.status='active';return true;}
  record(event,target,quantity=1){if(!id(event)||!id(target)||!count(quantity))throw RangeError('Invalid quest event');for(const quest of this.quests.values())if(quest.status==='active')quest.objectives.forEach((o,i)=>{if(o.event===event&&o.target===target)quest.progress[i]=Math.min(o.count,quest.progress[i]+quantity);});}
  ready(key){const quest=this.quests.get(key);return !!quest&&quest.status==='active'&&quest.objectives.every((o,i)=>quest.progress[i]>=o.count);}
  complete(key){if(!this.ready(key))return false;this.quests.get(key).status='complete';return true;}
  entries(){return [...this.quests.values()].map(q=>({...q,requires:[...q.requires],objectives:q.objectives.map(o=>({...o})),progress:[...q.progress],status:q.status==='available'&&!this.available(q)?'locked':q.status}));}
}
