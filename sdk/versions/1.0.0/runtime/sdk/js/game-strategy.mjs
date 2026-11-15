import { GameTileWorld } from './game-world.mjs';
const validId=id=>typeof id==='string'&&/^[A-Za-z][\w.-]{0,63}$/.test(id);
const finite=(n,min,max)=>Number.isFinite(n)&&n>=min&&n<=max;
const cell=p=>({column:Math.floor(p.x),row:Math.floor(p.y)});
const distance=(a,b)=>Math.hypot(a.x-b.x,a.y-b.y);

/** Footprint-aware navigation over shared tile terrain, with bounded A*. */
export class GameNavigation {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameNavigation(world)',methods:'world is GameTileWorld. block(id,{column,row,columns:2,rows:2}) reserves an entire walkable footprint atomically; unblock(id) removes it. walkable(column,row), canPlace(rect), path(from,to,{maximumVisited:8192}) returns 4-neighbour shortest path excluding start, [] at target or null on blocked/unreachable/budget. Dynamic buildings remain separate from authored terrain. Footprints cannot overlap or extend outside map. At most 1024 blockers. Query actual game-unit radii/occupancy before placement; this class supplies terrain/building navigation only.'});
  constructor(world){if(!(world instanceof GameTileWorld))throw TypeError('Navigation needs GameTileWorld');this.world=world;this.blocks=new Map();this.occupied=new Set();}
  cells(rect){
    if(!rect||!['column','row','columns','rows'].every(k=>Number.isInteger(rect[k]))||rect.columns<1||rect.rows<1||rect.columns*rect.rows>4096||rect.column<0||rect.row<0||rect.column+rect.columns>this.world.columns||rect.row+rect.rows>this.world.rows)throw RangeError('Invalid building footprint');
    const cells=[];for(let row=rect.row;row<rect.row+rect.rows;row++)for(let column=rect.column;column<rect.column+rect.columns;column++)cells.push({column,row,index:row*this.world.columns+column});return cells;
  }
  walkable(column,row){return this.world.walkable(column,row)&&!this.occupied.has(row*this.world.columns+column);}
  canPlace(rect){return this.cells(rect).every(c=>this.walkable(c.column,c.row));}
  block(id,rect){if(!validId(id)||this.blocks.has(id)||this.blocks.size>=1024)throw RangeError('Invalid blocker identity/budget');const cells=this.cells(rect);if(!cells.every(c=>this.walkable(c.column,c.row)))return false;this.blocks.set(id,cells);for(const c of cells)this.occupied.add(c.index);return true;}
  unblock(id){const cells=this.blocks.get(id);if(!cells)return false;for(const c of cells)this.occupied.delete(c.index);this.blocks.delete(id);return true;}
  path(from,to,{maximumVisited=8192}={}){
    if(!Number.isInteger(maximumVisited)||maximumVisited<1||maximumVisited>65536||!from||!to)throw RangeError('Invalid navigation request');
    if(!this.walkable(from.column,from.row)||!this.walkable(to.column,to.row))return null;
    const columns=this.world.columns,start=from.row*columns+from.column,end=to.row*columns+to.column;
    if(start===end)return [];
    const score=new Map([[start,0]]),parent=new Map(),closed=new Set(),heap=[];
    const heuristic=index=>Math.abs(index%columns-to.column)+Math.abs(Math.floor(index/columns)-to.row);
    const push=node=>{heap.push(node);let i=heap.length-1;while(i>0){const p=(i-1)>>1;if(heap[p].f<=node.f)break;heap[i]=heap[p];i=p;}heap[i]=node;};
    const pop=()=>{const result=heap[0],last=heap.pop();if(heap.length){let i=0;while(i*2+1<heap.length){let child=i*2+1;if(child+1<heap.length&&heap[child+1].f<heap[child].f)child++;if(last.f<=heap[child].f)break;heap[i]=heap[child];i=child;}heap[i]=last;}return result;};
    push({index:start,f:heuristic(start)});
    while(heap.length&&closed.size<maximumVisited){
      const current=pop().index;if(closed.has(current))continue;closed.add(current);
      if(current===end){const result=[];for(let at=end;at!==start;at=parent.get(at))result.push({column:at%columns,row:Math.floor(at/columns)});return result.reverse();}
      const column=current%columns,row=Math.floor(current/columns);
      for(const [dc,dr]of [[1,0],[0,1],[-1,0],[0,-1]]){const c=column+dc,r=row+dr,next=r*columns+c;if(!this.walkable(c,r)||closed.has(next))continue;const cost=score.get(current)+1;if(cost>=(score.get(next)??Infinity))continue;score.set(next,cost);parent.set(next,current);push({index:next,f:cost+heuristic(next)});}
    }
    return null;
  }
}

/** Deterministic small RTS simulation, separate from presentation and art. */
export class GameStrategy {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameStrategy({world,resources:{wood:100},unitTypes:{worker:{hp:30,speed:2,damage:2,range:1.5,cooldown:1,worker:true},guard:{hp:80,speed:2,damage:10,range:1.5,cooldown:0.8}},buildingTypes:{barracks:{columns:2,rows:2,hp:200,seconds:8,cost:{wood:50},trains:["guard"]}}})',methods:'addUnit({id,type,team:0,x:2.5,y:2.5}); select(ids), selectRect({x,y,width,height},team=0); orderMove(ids,{column,row},{formationColumns:4}); orderAttack(ids,targetId); orderHarvest(ids,nodeId); orderBuild(ids,buildingId); stop(ids). addResource({id,kind,x,y,amount}); placeBuilding({id,type,team:0,column,row}) atomically pays cost/reserves footprint; train(buildingId,{id,type}) pays unit cost and spawns at a free perimeter cell only when construction complete. update(delta) advances at most 0.25s in <=1/30s substeps. inspect() returns unit positions/HP/action/windup, building progress, selected ids and resources. Units have bounded continuous movement/animationTime, local separation, terrain/building A*, chase/repath, line-of-sight ranged/melee cooldown damage/death, worker harvesting and timed construction. 256 units/128 buildings/256 resource nodes. Explicit orders; enemy AI, factions, fog-of-war, projectiles, technology trees, formations around complex choke points and full campaign rules remain app-owned. Selection/order API takes exact app-owned IDs; native input maps into it.'});
  constructor({world,unitTypes={},buildingTypes={},resources={}}={}){
    this.navigation=new GameNavigation(world);this.units=new Map();this.buildings=new Map();this.nodes=new Map();this.selection=new Set();this.resources={};this.unitTypes=new Map();this.buildingTypes=new Map();this.time=0;
    const costs=cost=>{if(!cost||typeof cost!=='object'||Array.isArray(cost)||Object.keys(cost).length>32||Object.entries(cost).some(([k,v])=>!validId(k)||!Number.isSafeInteger(v)||v<0||v>1000000))throw RangeError('Invalid strategy resources');return {...cost};};
    this.resources=Object.assign(Object.create(null),costs(resources));
    if(Object.keys(unitTypes).length>64||Object.keys(buildingTypes).length>64)throw RangeError('Strategy type budget exceeded');
    for(const [id,type]of Object.entries(unitTypes)){
      const definition={hp:50,speed:2,damage:5,range:1.4,cooldown:0.8,radius:0.22,worker:false,...type,cost:costs(type.cost??{})};
      if(!validId(id)||!finite(definition.hp,1,100000)||!finite(definition.speed,0.1,8)||!finite(definition.damage,0,10000)||!finite(definition.range,0.5,16)||!finite(definition.cooldown,0.1,60)||!finite(definition.radius,0.05,0.4)||typeof definition.worker!=='boolean')throw RangeError('Invalid strategy unit type');this.unitTypes.set(id,definition);
    }
    for(const [id,type]of Object.entries(buildingTypes)){
      const definition={hp:200,seconds:10,trains:[],...type,cost:costs(type.cost??{})};
      if(!validId(id)||!Number.isInteger(definition.columns)||!Number.isInteger(definition.rows)||!finite(definition.columns,1,16)||!finite(definition.rows,1,16)||!finite(definition.hp,1,100000)||!finite(definition.seconds,0.1,600)||!Array.isArray(definition.trains)||definition.trains.length>64||definition.trains.some(k=>!this.unitTypes.has(k)))throw RangeError('Invalid strategy building type');this.buildingTypes.set(id,definition);
    }
  }
  unique(id){return validId(id)&&!this.units.has(id)&&!this.buildings.has(id)&&!this.nodes.has(id);}
  addUnit({id,type,team=0,x,y}){
    const spec=this.unitTypes.get(type);if(!this.unique(id)||!spec||this.units.size>=256||!Number.isInteger(team)||team<0||team>31||![x,y].every(Number.isFinite)||!this.fits(x,y,spec.radius))return false;
    this.units.set(id,{id,type,team,x,y,hp:spec.hp,order:{kind:'idle'},path:[],cooldown:0,repath:0,animationTime:0,action:'idle',windup:0,work:0});return true;
  }
  fits(x,y,radius,except){
    for(const dx of [-radius,radius])for(const dy of [-radius,radius])if(!this.navigation.walkable(Math.floor(x+dx),Math.floor(y+dy)))return false;
    return ![...this.units.values()].some(u=>u.id!==except&&u.hp>0&&Math.hypot(x-u.x,y-u.y)<radius+this.unitTypes.get(u.type).radius-0.001);
  }
  select(ids){this.selection=new Set(this.owned(ids).map(u=>u.id));return [...this.selection];}
  selectRect({x,y,width,height},team=0){if(![x,y,width,height].every(Number.isFinite)||width<0||height<0)throw RangeError('Invalid selection');return this.select([...this.units.values()].filter(u=>u.hp>0&&u.team===team&&u.x>=x&&u.x<=x+width&&u.y>=y&&u.y<=y+height).map(u=>u.id));}
  owned(ids){if(!Array.isArray(ids)||ids.length>256||ids.some(id=>!validId(id)))throw RangeError('Invalid unit IDs');return [...new Set(ids)].map(id=>this.units.get(id)).filter(u=>u?.hp>0);}
  stop(ids){for(const unit of this.owned(ids)){unit.order={kind:'idle'};unit.path=[];}}
  orderMove(ids,target,{formationColumns=4}={}){
    if(!Number.isInteger(formationColumns)||formationColumns<1||formationColumns>16||!target||!this.navigation.walkable(target.column,target.row))return false;
    const units=this.owned(ids),plans=[];
    for(let i=0;i<units.length;i++){
      const destination={column:target.column+i%formationColumns,row:target.row+Math.floor(i/formationColumns)},path=this.navigation.path(cell(units[i]),destination);
      if(!path)return false;plans.push({unit:units[i],path,destination});
    }
    for(const {unit,path,destination}of plans){unit.order={kind:'move',...destination};unit.path=path;unit.repath=0;}return true;
  }
  orderAttack(ids,targetId){const target=this.units.get(targetId)??this.buildings.get(targetId);if(!target||target.hp<=0)return false;for(const unit of this.owned(ids))if(unit.id!==targetId&&unit.team!==target.team){unit.order={kind:'attack',target:targetId};unit.path=[];unit.repath=0;}return true;}
  addResource({id,kind,x,y,amount}){if(!this.unique(id)||!validId(kind)||this.nodes.size>=256||![x,y].every(Number.isFinite)||!this.navigation.world.tile(Math.floor(x),Math.floor(y))||!Number.isSafeInteger(amount)||amount<1||amount>1000000)return false;this.nodes.set(id,{id,kind,x,y,amount});return true;}
  orderHarvest(ids,target){const node=this.nodes.get(target);if(!node||node.amount<=0)return false;for(const unit of this.owned(ids))if(this.unitTypes.get(unit.type).worker){unit.order={kind:'harvest',target};unit.path=[];unit.repath=0;unit.work=0;}return true;}
  pay(cost){if(Object.entries(cost).some(([k,v])=>(this.resources[k]??0)<v))return false;for(const [k,v]of Object.entries(cost))this.resources[k]-=v;return true;}
  placeBuilding({id,type,team=0,column,row}){
    const spec=this.buildingTypes.get(type);if(!this.unique(id)||!spec||this.buildings.size>=128||!Number.isInteger(team)||team<0||team>31)return false;
    const rect={column,row,columns:spec.columns,rows:spec.rows};if(!this.navigation.canPlace(rect))return false;
    if([...this.units.values()].some(u=>u.hp>0&&u.x+this.unitTypes.get(u.type).radius>column&&u.x-this.unitTypes.get(u.type).radius<column+spec.columns&&u.y+this.unitTypes.get(u.type).radius>row&&u.y-this.unitTypes.get(u.type).radius<row+spec.rows))return false;
    if(!this.pay(spec.cost))return false;this.navigation.block(id,rect);
    this.buildings.set(id,{id,type,team,...rect,x:column+spec.columns/2,y:row+spec.rows/2,hp:spec.hp,progress:0});return true;
  }
  orderBuild(ids,target){const building=this.buildings.get(target);if(!building||building.hp<=0||building.progress>=1)return false;for(const unit of this.owned(ids))if(this.unitTypes.get(unit.type).worker&&unit.team===building.team){unit.order={kind:'build',target};unit.path=[];unit.repath=0;}return true;}
  perimeter(target){const positions=[];for(let r=target.row-1;r<=target.row+target.rows;r++)for(let c=target.column-1;c<=target.column+target.columns;c++)if(c===target.column-1||c===target.column+target.columns||r===target.row-1||r===target.row+target.rows)if(this.navigation.walkable(c,r))positions.push({column:c,row:r});return positions;}
  train(buildingId,{id,type}){
    const building=this.buildings.get(buildingId),spec=this.unitTypes.get(type);if(!building||building.hp<=0||building.progress<1||!this.buildingTypes.get(building.type).trains.includes(type)||!this.unique(id)||!spec||this.units.size>=256)return false;
    const spawn=this.perimeter(building).find(p=>this.fits(p.column+0.5,p.row+0.5,spec.radius));if(!spawn||!this.pay(spec.cost))return false;
    return this.addUnit({id,type,team:building.team,x:spawn.column+0.5,y:spawn.row+0.5});
  }
  lineOfSight(a,b){const steps=Math.ceil(distance(a,b)*4);for(let i=1;i<steps;i++){const c=Math.floor(a.x+(b.x-a.x)*i/steps),r=Math.floor(a.y+(b.y-a.y)*i/steps);if(!this.navigation.walkable(c,r))return false;}return true;}
  update(delta){
    if(!finite(delta,0,0.25))throw RangeError('Strategy update requires 0..0.25 seconds');
    if(delta===0)return;
    const steps=Math.max(1,Math.ceil(delta*30)),dt=delta/steps;
    for(let step=0;step<steps;step++){
      this.time+=dt;
      for(const unit of this.units.values()){
        if(unit.hp<=0)continue;
        const spec=this.unitTypes.get(unit.type);unit.cooldown=Math.max(0,unit.cooldown-dt);unit.repath-=dt;unit.action='idle';unit.windup=0;
        const order=unit.order;
        let target;
        if(order.kind==='attack')target=this.units.get(order.target)??this.buildings.get(order.target);
        if(order.kind==='build')target=this.buildings.get(order.target);
        if(order.kind==='harvest')target=this.nodes.get(order.target);
        if(target&&(target.hp===0||target.amount===0||order.kind==='build'&&target.progress>=1)){unit.order={kind:'idle'};unit.path=[];continue;}
        if(['attack','harvest','build'].includes(order.kind)&&!target){unit.order={kind:'idle'};unit.path=[];continue;}
        const interactionDistance=target?.columns?Math.hypot(Math.max(target.column-unit.x,0,unit.x-(target.column+target.columns)),Math.max(target.row-unit.y,0,unit.y-(target.row+target.rows))):target?distance(unit,target):Infinity;
        const near=interactionDistance<=(order.kind==='attack'?spec.range:1.25);
        // A building target's own footprint must not count as an occluding wall.
        const sight=target?.columns?this.lineOfSight(unit,{x:Math.max(target.column-0.01,Math.min(unit.x,target.column+target.columns+0.01)),y:Math.max(target.row-0.01,Math.min(unit.y,target.row+target.rows+0.01))}):target?this.lineOfSight(unit,target):false;
        if(target&&near&&(order.kind!=='attack'||sight)){
          unit.path=[];unit.action=order.kind;
          if(order.kind==='attack'){
            unit.windup=1-unit.cooldown/spec.cooldown;
            if(unit.cooldown<=0){target.hp=Math.max(0,target.hp-spec.damage);unit.cooldown=spec.cooldown;if(target.hp===0){this.selection.delete(target.id);if(target.columns)this.navigation.unblock(target.id);}}
          }else if(order.kind==='build')target.progress=Math.min(1,target.progress+dt/this.buildingTypes.get(target.type).seconds);
          else{unit.work+=dt;while(unit.work>=1&&target.amount>0){unit.work-=1;target.amount--;this.resources[target.kind]=(this.resources[target.kind]??0)+1;}}
          unit.animationTime+=dt;continue;
        }
        if(target&&unit.repath<=0){
          const destinations=target.columns?this.perimeter(target):[{column:Math.floor(target.x),row:Math.floor(target.y)}];
          destinations.sort((a,b)=>Math.abs(a.column-unit.x)+Math.abs(a.row-unit.y)-Math.abs(b.column-unit.x)-Math.abs(b.row-unit.y));
          unit.path=[];for(const destination of destinations.slice(0,8)){const path=this.navigation.path(cell(unit),destination,{maximumVisited:2048});if(path){unit.path=path;break;}}unit.repath=0.5;
        }
        if(order.kind==='move'&&!unit.path.length){const x=order.column+0.5,y=order.row+0.5;if(Math.hypot(unit.x-x,unit.y-y)>0.02)unit.path=[{column:order.column,row:order.row}];else unit.order={kind:'idle'};}
        if(unit.path.length>1){
          const current=unit.path[0],next=unit.path[1];
          if(Math.hypot(current.column+0.5-unit.x,current.row+0.5-unit.y)<0.7&&this.lineOfSight(unit,{x:next.column+0.5,y:next.row+0.5}))unit.path.shift();
        }
        const waypoint=unit.path[0];if(!waypoint)continue;
        if(unit.avoidUntil>this.time){
          const nx=unit.x+unit.avoidX*spec.speed*dt,ny=unit.y+unit.avoidY*spec.speed*dt;
          if(this.fits(nx,ny,spec.radius,unit.id)){unit.x=nx;unit.y=ny;unit.action='walk';unit.animationTime+=dt;}else unit.avoidUntil=0;
          continue;
        }
        if(!this.navigation.walkable(waypoint.column,waypoint.row)){unit.path=[];if(order.kind==='move'){const path=this.navigation.path(cell(unit),order);if(path)unit.path=path;else unit.order={kind:'idle'};}continue;}
        const dx=waypoint.column+0.5-unit.x,dy=waypoint.row+0.5-unit.y,length=Math.hypot(dx,dy),amount=Math.min(length,spec.speed*dt);
        if(length<0.001){unit.path.shift();continue;}
        const nx=unit.x+dx/length*amount,ny=unit.y+dy/length*amount;
        if(this.fits(nx,ny,spec.radius,unit.id)){unit.x=nx;unit.y=ny;unit.facing=Math.atan2(dy,dx);unit.action='walk';unit.animationTime+=dt;if(amount===length)unit.path.shift();}
        else{
          // Brief deterministic side steps let converging formations yield
          // instead of locking two units at the same waypoint indefinitely.
          for(const side of [1,-1]){const sx=-dy/length*side,sy=dx/length*side;if(this.fits(unit.x+sx*spec.speed*dt,unit.y+sy*spec.speed*dt,spec.radius,unit.id)){unit.avoidX=sx;unit.avoidY=sy;unit.avoidUntil=this.time+0.25;break;}}
        }
      }
    }
  }
  inspect(){return {time:this.time,resources:{...this.resources},selection:[...this.selection],units:[...this.units.values()].map(u=>({...u,order:{...u.order},path:u.path.map(p=>({...p}))})),buildings:[...this.buildings.values()].map(b=>({...b})),nodes:[...this.nodes.values()].map(n=>({...n}))};}
}
