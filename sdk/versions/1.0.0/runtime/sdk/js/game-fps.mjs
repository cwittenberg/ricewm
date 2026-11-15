// Deterministic first-person motion and bounded, world-space impact simulation.
const finite = (n, label) => { if (!Number.isFinite(n)) throw TypeError(label + ' must be finite'); return n; };
const vector = (v) => { if (!Array.isArray(v) || v.length !== 3 || v.some(n => !Number.isFinite(n))) throw TypeError('Expected a finite 3D vector'); return [...v]; };
const dt = (n) => { finite(n, 'delta'); if (n < 0 || n > .25) throw RangeError('delta must be between 0 and .25 seconds'); return n; };
const spring = (s, target, frequency, delta) => {
  const x = s.value - target, j = s.velocity + frequency * x, decay = Math.exp(-frequency * delta);
  s.value = target + (x + j * delta) * decay;
  s.velocity = (s.velocity - frequency * j * delta) * decay;
};
export class GameFPSMotion {
  constructor({response = 14, stride = 1.65} = {}) {
    finite(response, 'response'); finite(stride, 'stride');
    if (response <= 0 || response > 100 || stride <= 0 || stride > 10) throw RangeError('Invalid first-person motion bounds');
    this.response = response; this.stride = stride; this.reset();
  }
  reset() { this.velocity = [0, 0]; this.distance = 0; this.steps = 0; this.bob = 0; this.recoil = {value:0,velocity:0}; this.kick = {value:0,velocity:0}; this.landing = {value:0,velocity:0}; this.sway = [0,0]; }
  move(delta, {x = 0, z = 0, grounded = true} = {}) {
    dt(delta); finite(x, 'x velocity'); finite(z, 'z velocity');
    if (Math.hypot(x,z) > 100) throw RangeError('Movement speed exceeds bound');
    const rate = this.response * (grounded ? 1 : .22), decay = Math.exp(-rate * delta), previous = [...this.velocity];
    this.velocity = previous.map((v,i) => [x,z][i] + (v-[x,z][i])*decay);
    // Exact displacement of the exponentially approached velocity, independent of update rate.
    return previous.map((v,i) => [x,z][i]*delta + (v-[x,z][i])*(1-decay)/rate);
  }
  contact({x = false, z = false} = {}) { if(x)this.velocity[0]=0; if(z)this.velocity[1]=0; }
  travel(x, z, delta, {grounded = true} = {}) {
    finite(x,'travel x'); finite(z,'travel z'); dt(delta);
    const distance = Math.hypot(x,z); if(distance > 30) throw RangeError('Travel exceeds update bound');
    if(grounded)this.distance += distance;
    this.steps = Math.floor(this.distance/(this.stride*.5));
    const weight = grounded ? Math.min(1,distance/Math.max(.001,delta)/3.1) : 0;
    this.bob += (weight-this.bob)*(1-Math.exp(-14*delta));
  }
  shoot({pitch = .025, kick = .075} = {}) {
    finite(pitch,'pitch impulse'); finite(kick,'kick impulse');
    if(pitch<0||pitch>.3||kick<0||kick>.5)throw RangeError('Shot impulse exceeds bound');
    this.recoil.value=Math.min(.25,this.recoil.value+pitch); this.recoil.velocity+=pitch*8;
    this.kick.value=Math.min(.2,this.kick.value+kick); this.kick.velocity+=kick*5;
  }
  land(speed) { finite(speed,'landing speed'); if(speed<0||speed>100)throw RangeError('Invalid landing speed'); this.landing.velocity-=Math.min(1.3,speed*.09); }
  look(x,y) { finite(x,'look x'); finite(y,'look y'); this.sway[0]=Math.max(-.035,Math.min(.035,this.sway[0]-x*.015)); this.sway[1]=Math.max(-.025,Math.min(.025,this.sway[1]+y*.015)); }
  update(delta) { dt(delta); spring(this.recoil,0,19,delta);spring(this.kick,0,23,delta);spring(this.landing,0,17,delta);this.sway=this.sway.map(v=>v*Math.exp(-11*delta)); }
  sample({yaw = 0} = {}) {
    finite(yaw,'view yaw');
    const phase=this.distance/this.stride*Math.PI*2, b=this.bob;
    return {pitch:this.recoil.value, kick:this.kick.value, eye:Math.sin(phase*2)*.011*b+this.landing.value, x:Math.sin(phase)*.008*b+this.sway[0], y:Math.cos(phase*2)*.008*b+this.sway[1]+this.landing.value*.6, roll:-(this.velocity[0]*Math.cos(yaw)-this.velocity[1]*Math.sin(yaw))*.003, footsteps:this.steps};
  }
}
GameFPSMotion.controlFactory=Object.freeze({topics:['games'],constructor:'new GameFPSMotion({response:14,stride:1.65})',methods:'move(delta,{x,z,grounded}) returns smoothed horizontal displacement; apply authoritative collision, contact({x,z}) cancels blocked velocity, travel(actualX,actualZ,delta,{grounded}) drives distance-based footsteps/bob. shoot({pitch,kick}), land(speed), look(dx,dy), update(delta), sample({yaw:0}), reset(). sample.pitch must affect BOTH camera and hit ray; use kick/sway for viewmodel. Fixed updates <=.25s. No collision, audio, damage or camera authority is invented.'});

export class GameImpactEffects {
  constructor({maximumParticles=96,maximumStains=64,random=Math.random}={}) {
    if(!Number.isInteger(maximumParticles)||maximumParticles<1||maximumParticles>256||!Number.isInteger(maximumStains)||maximumStains<1||maximumStains>256||typeof random!=='function')throw RangeError('Invalid impact effect bounds');
    this.maximumParticles=maximumParticles;this.maximumStains=maximumStains;this.random=random;this.reset();
  }
  reset(){this.particles=[];this.stains=[];this.sequence=0;this.impacts=0;}
  rand(){const n=this.random();if(!Number.isFinite(n)||n<0||n>=1)throw RangeError('random must return [0,1)');return n;}
  stain(point,normal,size,kind='blood'){vector(point);vector(normal);finite(size,'stain size');if(!['blood','scorch'].includes(kind)||size<=0||size>3||Math.hypot(...normal)<.5)throw RangeError('Invalid impact stain');const n=Math.hypot(...normal);this.stains.push({id:++this.sequence,point:[...point],normal:normal.map(v=>v/n),size,kind});if(this.stains.length>this.maximumStains)this.stains.shift();}
  emit({point,direction,kind='blood',lethal=false}={}){
    point=vector(point);direction=vector(direction);if(!['blood','spark'].includes(kind)||Math.hypot(...direction)<1e-6)throw RangeError('Invalid impact direction or kind');const length=Math.hypot(...direction);direction=direction.map(v=>v/length);this.impacts++;
    const count=kind==='blood'?(lethal?24:14):8;
    for(let i=0;i<count;i++){const speed=.6+this.rand()*2.7;this.particles.push({id:++this.sequence,point:[...point],velocity:direction.map((v,j)=>v*speed+(this.rand()-.5)*2.1+(j===1?.8:0)),size:kind==='blood'?.008+this.rand()*.021:.006,age:0,lifetime:kind==='blood'?2.3:.35,kind,fragment:kind==='blood'&&lethal&&i<4});if(this.particles.length>this.maximumParticles)this.particles.shift();}
    if(this.particles.length>this.maximumParticles)this.particles.splice(0,this.particles.length-this.maximumParticles);
  }
  update(delta,{ground=()=>0,sweep=()=>null}={}){
    dt(delta);if(typeof ground!=='function'||typeof sweep!=='function')throw TypeError('Impact collision callbacks required');
    // Substeps keep droplets from tunnelling through thin walls; host collision remains app-owned.
    const steps=Math.max(1,Math.ceil(delta/.016)),h=delta/steps;
    for(let i=0;i<steps;i++)this.particles=this.particles.filter(p=>{
      p.age+=h;if(p.age>p.lifetime)return false;const next=p.point.map((v,j)=>v+p.velocity[j]*h);p.velocity[1]-=9.81*h;
      const hit=sweep([...p.point],next),floor=finite(ground(next[0],next[2]),'ground height');
      if(hit||next[1]<=floor+.012){if(p.kind==='blood'){const point=hit?vector(hit.point):[next[0],floor+.012,next[2]],normal=hit?vector(hit.normal):[0,1,0];this.stain(point.map((v,j)=>v+normal[j]*.003),normal,p.fragment?.09:.025+p.size*2);}return false;}
      p.point=next;return true;
    });
  }
}
GameImpactEffects.controlFactory=Object.freeze({topics:['games'],constructor:'new GameImpactEffects({maximumParticles:96,maximumStains:64,random:Math.random})',methods:'emit({point,direction,kind:"blood"|"spark",lethal:false}) ONLY after a real occlusion-tested hit. update(delta,{ground(x,z),sweep(from,to)}) advances gravity and collision; sweep returns {point,normal} or null. Blood collisions leave bounded persistent stains; stain(point,normal,size,"blood"|"scorch") adds an authored surface mark. particles/stains contain world-space data for game-owned rendering; reset() clears them. No textures, damage, ragdoll or mesh intersection is implied. Maximum 256 particles and 256 stains; fixed deltas <=.25s.'});
