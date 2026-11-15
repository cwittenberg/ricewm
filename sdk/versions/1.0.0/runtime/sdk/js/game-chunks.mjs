import { GameTileWorld } from './game-world.mjs';
const coordinate=value=>Number.isSafeInteger(value)&&Math.abs(value)<=2147483647;
const tile=value=>typeof value==='string'&&/^[A-Za-z][\w.-]{0,63}$/.test(value);
export function gameSeededRandom(seed=1){if(!Number.isInteger(seed))throw RangeError('Random seed must be an integer');let state=seed>>>0;return()=>{state=(state+0x6d2b79f5)>>>0;let value=Math.imul(state^(state>>>15),state|1);value^=value+Math.imul(value^(value>>>7),value|61);return((value^(value>>>14))>>>0)/4294967296;};}
gameSeededRandom.controlFactory=Object.freeze({topics:['games'],constructor:'gameSeededRandom(12345)',methods:'Returns a deterministic [0,1) random callback for chunk generation, encounter/loot tables and replay. Not cryptographic randomness or an account/security primitive.'});

/** Huge procedural worlds with a bounded resident working set and explicit,
 * app-owned persistence. No fabricated claim of infinitely loaded memory. */
export class GameChunkWorld {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameChunkWorld({seed:1,size:32,maximumChunks:25,generate:async({chunkX,chunkY,size,random})=>({tiles:Array.from({length:size*size},()=>random()<0.2?"forest":"grass"),state:{}}),store:{load:async key=>savedChunkOrNull,save:async(key,record)=>{}}})',methods:'await ensureAt(globalColumn,globalRow,{radius:1}) loads/generates the surrounding chunks and evicts nonrequired LRU chunks only after dirty state saves successfully. tile(globalColumn,globalRow), set(globalColumn,globalRow,tile), setChunkState(chunkX,chunkY,json), chunkState(chunkX,chunkY), await save(), snapshot({left,top,columns,rows,solid,projection}). snapshot returns {world:GameTileWorld,origin:{column,row}} for the bounded active simulation/render/nav/fog window. Keep authoritative actor/quest/loot coordinates global and convert by origin; persist/unload distant actors and rebase active simulation when crossing window boundaries. Generation is deterministic per seed/coordinate, including negative coordinates; unloaded tiles return null, never invented empty terrain. Dirty eviction without an explicit persistence store fails. Max 81 resident chunks, size 8..64; logical coordinates bounded to signed 32-bit. Radius 0..4. Streaming errors retain resident/dirty state and must be repaired, never silently regenerate saved progress. This enables huge explorable worlds within bounded memory; it is not an infinite simultaneously simulated world.'});
  constructor({seed=1,size=32,maximumChunks=25,generate,store=null}={}){
    if(!Number.isInteger(seed)||!Number.isInteger(size)||size<8||size>64||!Number.isInteger(maximumChunks)||maximumChunks<1||maximumChunks>81||typeof generate!=='function'||store!==null&&(typeof store.load!=='function'||typeof store.save!=='function'))throw RangeError('Invalid chunk world');
    this.seed=seed;this.size=size;this.maximumChunks=maximumChunks;this.generate=generate;this.store=store;this.chunks=new Map();this.queue=Promise.resolve();this.serial=0;
  }
  address(column,row){if(!coordinate(column)||!coordinate(row))throw RangeError('World coordinates exceed supported range');const chunkX=Math.floor(column/this.size),chunkY=Math.floor(row/this.size);return {chunkX,chunkY,key:chunkX+','+chunkY,index:(row-chunkY*this.size)*this.size+column-chunkX*this.size};}
  record(value){if(!value||!Array.isArray(value.tiles)||value.tiles.length!==this.size*this.size||value.tiles.some(t=>!tile(t)))throw Error('Invalid generated/saved chunk');const state=structuredClone(value.state??{}),encoded=JSON.stringify(state);if(encoded===undefined||encoded.length>262144)throw RangeError('Chunk state exceeds budget');return {tiles:[...value.tiles],state};}
  async ensureAt(column,row,{radius=1}={}){
    const center=this.address(column,row);if(!Number.isInteger(radius)||radius<0||radius>4||(radius*2+1)**2>this.maximumChunks)throw RangeError('Chunk radius exceeds resident budget');
    const run=async()=>{
      const required=new Set(),pending=new Map();
      for(let y=center.chunkY-radius;y<=center.chunkY+radius;y++)for(let x=center.chunkX-radius;x<=center.chunkX+radius;x++){
        const key=x+','+y;required.add(key);if(this.chunks.has(key))continue;
        const saved=this.store?await this.store.load(key):null;
        const random=gameSeededRandom((this.seed^Math.imul(x,0x9e3779b1)^Math.imul(y,0x85ebca6b))|0);
        const value=saved===null||saved===undefined?await this.generate({chunkX:x,chunkY:y,size:this.size,seed:this.seed,random}):saved;
        pending.set(key,{...this.record(value),dirty:false,revision:0,used:++this.serial});
      }
      const candidates=[...this.chunks].filter(([key])=>!required.has(key)).sort((a,b)=>a[1].used-b[1].used),evict=[];
      let size=this.chunks.size+pending.size;
      for(const [key,chunk]of candidates){if(size<=this.maximumChunks)break;if(chunk.dirty){if(!this.store)throw Error('Dirty chunk eviction requires a persistence store');const revision=chunk.revision;await this.store.save(key,this.record(chunk));if(chunk.revision!==revision)throw Error('Chunk changed while saving; retry streaming');}evict.push(key);size--;}
      if(size>this.maximumChunks)throw Error('Required chunks exceed resident capacity');
      for(const key of evict)this.chunks.delete(key);for(const [key,value]of pending)this.chunks.set(key,value);for(const key of required)this.chunks.get(key).used=++this.serial;
      return {resident:this.chunks.size,required:[...required],origin:{column:(center.chunkX-radius)*this.size,row:(center.chunkY-radius)*this.size}};
    };
    const result=this.queue.then(run,run);this.queue=result.catch(()=>{});return result;
  }
  tile(column,row){const address=this.address(column,row),chunk=this.chunks.get(address.key);if(!chunk)return null;chunk.used=++this.serial;return chunk.tiles[address.index];}
  set(column,row,value){if(!tile(value))throw RangeError('Invalid world tile');const address=this.address(column,row),chunk=this.chunks.get(address.key);if(!chunk)throw Error('Load chunk before editing');chunk.tiles[address.index]=value;chunk.dirty=true;chunk.revision++;}
  chunkState(chunkX,chunkY){if(!coordinate(chunkX)||!coordinate(chunkY))throw RangeError('Invalid chunk coordinates');const chunk=this.chunks.get(chunkX+','+chunkY);if(!chunk)throw Error('Chunk is not loaded');return structuredClone(chunk.state);}
  setChunkState(chunkX,chunkY,state){if(!coordinate(chunkX)||!coordinate(chunkY))throw RangeError('Invalid chunk coordinates');const chunk=this.chunks.get(chunkX+','+chunkY);if(!chunk)throw Error('Chunk is not loaded');chunk.state=this.record({tiles:chunk.tiles,state}).state;chunk.dirty=true;chunk.revision++;}
  async save(){
    const run=async()=>{if(!this.store&&[...this.chunks.values()].some(c=>c.dirty))throw Error('Dirty chunks require a persistence store');for(const [key,chunk]of this.chunks)if(chunk.dirty){const revision=chunk.revision;await this.store.save(key,this.record(chunk));if(chunk.revision!==revision)throw Error('Chunk changed while saving; retry checkpoint');chunk.dirty=false;}};
    const result=this.queue.then(run,run);this.queue=result.catch(()=>{});return result;
  }
  snapshot({left,top,columns,rows,solid=[],projection={}}={}){
    if(!coordinate(left)||!coordinate(top)||!Number.isInteger(columns)||!Number.isInteger(rows)||columns<1||rows<1||columns*rows>65536)throw RangeError('Invalid active world snapshot');
    const tiles=[];for(let row=0;row<rows;row++)for(let column=0;column<columns;column++){const value=this.tile(left+column,top+row);if(value===null)throw Error('Active world snapshot requires loaded chunks');tiles.push(value);}
    return {world:new GameTileWorld({columns,rows,tiles,solid,projection}),origin:{column:left,row:top}};
  }
}
