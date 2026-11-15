import { gameSeededRandom } from './game-chunks.mjs';

const point = p => Array.isArray(p) && p.length === 3 && p.every(Number.isFinite) && p.every(n => Math.abs(n) <= 2147483647);
const ink = (rgb, alpha) => ((Math.round(Math.max(0, Math.min(1, alpha)) * 255) << 24) | (rgb[2] << 16) | (rgb[1] << 8) | rgb[0]) >>> 0;

/** Deterministic world-space spell presentation with one impact event per cast.
 * The application owns target checks, costs and applying damage on that event. */
export class GameEffects {
  static controlFactory = Object.freeze({
    topics: ['games'], constructor: 'new GameEffects({seed:42,maximum:32})',
    methods: 'emit({type:"firebolt"|"lightning"|"nova"|"frost"|"moonbeam",from:[column,row,height],to:[column,row,height],target:"actorId",color:[255,140,45],duration:0.35,life:1.2,power:1}) returns effect ID; update(delta), drainImpacts(), draw(scene,{project:([column,row,height])=>({x,y})}), clear(), inspect(). Native layered light gradients, moving fire/trails, seeded branching lightning, expanding impact shockwaves, sparks and dissipating smoke. Frost launches clustered icy lances and grows/melts impact crystals. Moonbeam descends through orbiting rings, with expanding glyph circles and rising star motes; these have separate animation geometry, not just a palette swap. Impact receipts {id,type,target,point} occur exactly once when travel finishes; apply the already-paid GameCharacter cast receipt to authoritative actors there, never repeat damage every rendered frame. Use the same world/camera projection as terrain/actors; pass visible:effect=>known(effect.to) to omit effects in unrevealed terrain. Up to 32 active effects/128 pending impacts; bounded life 0.1..4 seconds and delta <=0.25. Effects update only while gameplay advances and stop with its lifecycle. Native vector/radial-gradient effects, not a GPU particle/PBR engine.',
  });
  constructor({ seed = 1, maximum = 32 } = {}) {
    if (!Number.isInteger(seed) || !Number.isInteger(maximum) || maximum < 1 || maximum > 32) throw RangeError('Invalid effects budget');
    this.seed = seed; this.maximum = maximum; this.effects = []; this.impacts = []; this.serial = 0;
  }
  emit({ type = 'firebolt', from, to, target = '', color = type === 'frost' ? [137,211,255] : type === 'moonbeam' ? [118,247,198] : type === 'lightning' ? [127,180,255] : [255,137,39], duration = 0.35, life = 1.25, power = 1 } = {}) {
    if (!['firebolt','lightning','nova','frost','moonbeam'].includes(type) || !point(from) || !point(to) ||
        typeof target !== 'string' || target.length > 96 || !Array.isArray(color) || color.length !== 3 || color.some(n => !Number.isInteger(n) || n < 0 || n > 255) ||
        ![duration,life,power].every(Number.isFinite) || duration < 0.01 || duration > 2 || life <= duration || life > 4 || power < 0.25 || power > 3) throw RangeError('Invalid spell effect');
    if (this.effects.length >= this.maximum || this.effects.length + this.impacts.length >= 128) return null;
    const id = ++this.serial, random = gameSeededRandom((this.seed ^ id) | 0);
    const particles = Array.from({length:24}, () => ({ angle: random()*Math.PI*2, speed: 16+random()*68, lift: 10+random()*50, size: 0.6+random()*1.7 }));
    this.effects.push({ id,type,from:[...from],to:[...to],target,color:[...color],duration,life,power,age:0,impacted:false,particles });
    return id;
  }
  update(delta) {
    if (!Number.isFinite(delta) || delta < 0 || delta > 0.25) throw RangeError('Effects tick requires 0..0.25 seconds');
    for (const effect of this.effects) {
      effect.age += delta;
      if (!effect.impacted && effect.age >= effect.duration) {
        effect.impacted = true;
        this.impacts.push({id:effect.id,type:effect.type,target:effect.target,point:[...effect.to]});
      }
    }
    this.effects = this.effects.filter(e => e.age < e.life);
  }
  drainImpacts() { return this.impacts.splice(0); }
  clear() { this.effects = []; this.impacts = []; }
  inspect() { return this.effects.map(({particles,...effect}) => ({...effect,from:[...effect.from],to:[...effect.to],color:[...effect.color]})); }
  draw(scene, { project, visible = () => true } = {}) {
    if (typeof project !== 'function' || typeof visible !== 'function') throw TypeError('Effects require the world projection');
    const at = p => { const q = project(p); if (!q || !Number.isFinite(q.x) || !Number.isFinite(q.y)) throw RangeError('Invalid projected effect'); return q; };
    let count = 0;
    const glow = (x,y,r,rgb,alpha) => {
      scene.radialGradient([x-r,y-r,r*2,r*2],[x,y,0],[x,y,r], [{offset:0,color:ink(rgb,alpha)},{offset:0.25,color:ink(rgb,alpha*0.65)},{offset:1,color:ink(rgb,0)}]); count++;
    };
    for (const e of this.effects) {
      if (!visible(e)) continue;
      const fraction = Math.min(1,e.age/e.duration), fade = Math.max(0,1-(e.age-e.duration)/(e.life-e.duration));
      const from = at(e.from), to = at(e.to), size = e.power;
      if (!e.impacted) {
        const x = from.x+(to.x-from.x)*fraction, y = from.y+(to.y-from.y)*fraction;
        if (e.type === 'lightning') {
          const random = gameSeededRandom((e.id ^ Math.floor(e.age*30)) | 0), points = [from];
          for (let i=1;i<10;i++) points.push({x:from.x+(to.x-from.x)*i/10+(random()-0.5)*22,y:from.y+(to.y-from.y)*i/10+(random()-0.5)*22});
          points.push(to);
          for(let i=1;i<points.length;i++) {
            const a=[points[i-1].x,points[i-1].y],b=[points[i].x,points[i].y];
            scene.line(a,b,7*size,ink(e.color,0.14)).line(a,b,2.5*size,ink(e.color,0.85)).line(a,b,0.8*size,0xfff2f6ff);
            if(i%3===0) scene.line(a,[a[0]+(random()-0.5)*30,a[1]+20],0.8,ink(e.color,0.65));
          }
          glow(to.x,to.y,34*size,e.color,0.55);
        } else if(e.type==='frost') {
          const angle=Math.atan2(to.y-from.y,to.x-from.x),dx=Math.cos(angle),dy=Math.sin(angle);
          for(let i=0;i<6;i++){const t=Math.max(0,fraction-i*.04),px=from.x+(to.x-from.x)*t,py=from.y+(to.y-from.y)*t;glow(px,py,(13-i)*size,e.color,.32-i*.04);}
          for(let i=0;i<5;i++){
            const side=(i-2)*4*size,px=x-dy*side-dx*Math.abs(side),py=y+dx*side-dy*Math.abs(side);
            scene.line([px-dx*12*size,py-dy*12*size],[px+dx*13*size,py+dy*13*size],3*size,ink(e.color,.8));
            scene.line([px,py],[px+dx*16*size,py+dy*16*size],.8,0xfff5ffff);
          }
        } else if(e.type==='moonbeam') {
          const strength=Math.sin(fraction*Math.PI/2);
          scene.linearGradient([to.x-9*size,to.y-150*size,18*size,150*size],[to.x,to.y-150*size],[to.x,to.y],[{offset:0,color:ink(e.color,0)},{offset:1,color:ink(e.color,.5*strength)}]);
          glow(to.x,to.y,45*size,e.color,.45*strength);
          for(let i=0;i<5;i++){
            const a=e.age*7+i*Math.PI*.4,r=(19+i*4)*size,py=to.y-(i*23+fraction*20)*size;
            scene.ellipse([to.x-r,py-r*.25,r*2,r*.5],ink(e.color,.65*strength),{filled:false,width:1});
            const px=to.x+Math.cos(a)*r,sy=py+Math.sin(a)*r*.25;glow(px,sy,5*size,e.color,.8*strength);
          }
        } else if(e.type==='firebolt') {
          for(let i=6;i>=0;i--) {
            const t=Math.max(0,fraction-i*0.035),tx=from.x+(to.x-from.x)*t,ty=from.y+(to.y-from.y)*t;
            glow(tx,ty,(10-i)*size,e.color,0.5-i*0.055);
          }
          glow(x,y,21*size,e.color,0.65); scene.ellipse([x-3*size,y-3*size,6*size,6*size],0xffd9f5ff);
        } else glow(from.x,from.y,30*size,e.color,Math.min(0.6,fraction));
      } else {
        const t=e.age-e.duration, r=(12+t*76)*size;
        if(e.type==='frost'){
          const rise=Math.min(1,t*9),melt=Math.max(0,1-t/(e.life-e.duration));
          glow(to.x,to.y,65*size,e.color,.5*melt);
          for(let i=0;i<12;i++){
            const a=i*Math.PI/6,px=to.x+Math.cos(a)*(12+i%3*8)*size,py=to.y+Math.sin(a)*(7+i%3*4)*size,h=(20+i%4*8)*size*rise*melt;
            scene.line([px-4*size,py],[px,py-h],3*size,ink(e.color,.85*melt));scene.line([px,py-h],[px+4*size,py],1.4*size,ink([226,252,255],melt));
          }
          scene.ellipse([to.x-r,to.y-r*.4,r*2,r*.8],ink(e.color,.7*melt),{filled:false,width:1.2});count+=26;continue;
        }
        if(e.type==='moonbeam'){
          glow(to.x,to.y,90*size*fade,e.color,.55*fade);
          for(let i=0;i<3;i++){const radius=(24+t*(40+i*18))*size;scene.ellipse([to.x-radius,to.y-radius*.42,radius*2,radius*.84],ink(e.color,fade*(.7-i*.15)),{filled:false,width:1.7});}
          for(let i=0;i<16;i++){
            const a=i*Math.PI/8+t*2,px=to.x+Math.cos(a)*r*.65,py=to.y+Math.sin(a)*r*.28-t*28*size;
            glow(px,py,4*size,e.color,fade);scene.line([px-2,py],[px+2,py],.7,ink([234,255,222],fade));scene.line([px,py-2],[px,py+2],.7,ink([234,255,222],fade));
          }
          count+=35;continue;
        }
        glow(to.x,to.y,75*size*fade,e.color,0.7*fade);
        scene.ellipse([to.x-r,to.y-r*0.44,r*2,r*0.88],ink(e.color,0.65*fade),{filled:false,width:Math.max(0.2,3*fade)});
        scene.ellipse([to.x-r*0.8,to.y-r*0.35,r*1.6,r*0.7],ink([255,237,180],0.5*fade),{filled:false,width:Math.max(0.2,fade)});
        for(const p of e.particles) {
          const x=to.x+Math.cos(p.angle)*p.speed*t*size,y=to.y+Math.sin(p.angle)*p.speed*t*0.5*size-p.lift*t+35*t*t;
          if(t>0.14)glow(x,y-8*t,7*size,[81,72,65],0.12*fade);
          scene.line([x,y],[x-Math.cos(p.angle)*6*fade,y-p.lift*0.025],p.size,ink(e.color,fade));
        }
        count+=e.particles.length+2;
      }
    }
    return count;
  }
}
