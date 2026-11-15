import { gameSeededRandom } from './game-chunks.mjs';

/** Small modular building meshes, suitable for baking through rasterizeGameModel.
 * Geometry and UVs are generated here; it is not a picture standing in for a mesh. */
export function createGameBuilding({ type = 'cottage', seed = 1, roofStyle = null, palette = {} } = {}) {
  if(['moonwell','observatory','gateway'].includes(type))return createGroveBuilding({type,seed,palette});
  const sizes = { cottage: [3, 3, 1.9], chapel: [3, 4, 2.8], tower: [2, 2, 4.2], keep: [5, 4, 2.5] };
  if (!Object.hasOwn(sizes, type) || !Number.isInteger(seed)) throw RangeError('Invalid procedural building');
  if(roofStyle!==null&&!['pitched','spire','battlements'].includes(roofStyle))throw RangeError('Invalid building roof style');
  if(!palette||typeof palette!=='object'||Object.entries(palette).some(([key,rgb])=>!['wall','roof','wood'].includes(key)||!Array.isArray(rgb)||rgb.length!==3||rgb.some(v=>!Number.isFinite(v)||v<0||v>1)))throw RangeError('Invalid building palette');
  roofStyle??=(type==='tower'||type==='keep'?'battlements':'pitched');
  const [width, depth, height] = sizes[type], triangles = [], random = gameSeededRandom(seed);
  const materials = {
    wall: { color: [0.83, 0.81, 0.72, 1], texture: 'wall' },
    roof: { color: [0.65 + random() * 0.2, 0.7, 0.76, 1], texture: 'roof' },
    wood: { color: [0.5, 0.38, 0.26, 1], texture: 'wood' },
    dark: { color: [0.075, 0.055, 0.035, 1] },
    glass: { color: [1, 0.69, 0.26, 1] },
    iron: { color: [0.24, 0.26, 0.25, 1] },
  };
  for(const [key,rgb]of Object.entries(palette))materials[key].color=[...rgb,1];
  const quad = (points, material, repeat = 1) => {
    const uv = [[0, 1], [repeat, 1], [repeat, 0], [0, 0]];
    for (const ids of [[0, 1, 2], [0, 2, 3]]) triangles.push({ material, corners: ids.map(i => ({ point: [...points[i]], uv: uv[i] })) });
  };
  const box = (x, y, z, w, h, d, material = 'wall') => {
    quad([[x,y,z],[x+w,y,z],[x+w,y+h,z],[x,y+h,z]], material, Math.max(w,h));
    quad([[x+w,y,z+d],[x,y,z+d],[x,y+h,z+d],[x+w,y+h,z+d]], material, Math.max(w,h));
    quad([[x,y,z+d],[x,y,z],[x,y+h,z],[x,y+h,z+d]], material, Math.max(d,h));
    quad([[x+w,y,z],[x+w,y,z+d],[x+w,y+h,z+d],[x+w,y+h,z]], material, Math.max(d,h));
    quad([[x,y+h,z],[x+w,y+h,z],[x+w,y+h,z+d],[x,y+h,z+d]], material, Math.max(w,d));
  };
  const roof = (x, z, w, d, y, rise) => {
    quad([[x,y,z],[x+w/2,y+rise,z],[x+w/2,y+rise,z+d],[x,y,z+d]], 'roof', 3);
    quad([[x+w/2,y+rise,z],[x+w,y,z],[x+w,y,z+d],[x+w/2,y+rise,z+d]], 'roof', 3);
    triangles.push({material:'wall',corners:[{point:[x,y,z],uv:[0,1]},{point:[x+w,y,z],uv:[1,1]},{point:[x+w/2,y+rise,z],uv:[0.5,0]}]});
    triangles.push({material:'wall',corners:[{point:[x,y,z+d],uv:[0,1]},{point:[x+w,y,z+d],uv:[1,1]},{point:[x+w/2,y+rise,z+d],uv:[0.5,0]}]});
    box(x + w / 2 - 0.045, y + rise - 0.02, z - 0.03, 0.09, 0.08, d + 0.06, 'wood');
  };
  const windowFront = (x, y, z, w = 0.4, h = 0.55) => {
    box(x - 0.055, y - 0.055, z - 0.025, w + 0.11, h + 0.11, 0.08, 'wood');
    box(x, y, z - 0.07, w, h, 0.06, 'dark');
    box(x + 0.025, y + 0.025, z - 0.085, w - 0.05, h - 0.05, 0.02, 'glass');
    box(x + w / 2 - 0.02, y, z - 0.1, 0.04, h, 0.04, 'iron');
    box(x, y + h / 2, z - 0.1, w, 0.035, 0.04, 'iron');
    box(x - 0.1, y - 0.12, z - 0.13, w + 0.2, 0.1, 0.2, 'wall');
  };
  const x = -width / 2, z = -depth / 2;
  box(x - 0.1, 0, z - 0.1, width + 0.2, 0.28, depth + 0.2);
  box(x, 0.28, z, width, height - 0.28, depth);
  box(x - 0.025, 0.3, z - 0.025, width + 0.05, 0.1, depth + 0.05, 'wood');
  for (const px of [x - 0.025, -0.06, -x - 0.09]) box(px, 0.28, z - 0.05, 0.12, height - 0.18, 0.1, 'wood');
  for (const pz of [z - 0.025, -z - 0.09]) box(x - 0.05, 0.28, pz, 0.1, height - 0.18, 0.12, 'wood');
  box(x - 0.05, height - 0.1, z - 0.05, width + 0.1, 0.12, depth + 0.1, 'wood');
  if (roofStyle === 'battlements') {
    box(x - 0.13, height, z - 0.13, width + 0.26, 0.23, depth + 0.26);
    for (let i = 0; i < width; i += 0.6) for (const edge of [z - 0.15, -z - 0.12]) box(x + i, height + 0.23, edge, 0.32, 0.38, 0.28);
    for (let i = 0.6; i < depth - 0.2; i += 0.6) for (const edge of [x - 0.15, -x - 0.12]) box(edge, height + 0.23, z + i, 0.28, 0.38, 0.32);
    if (type === 'keep') for (const px of [x - 0.4, -x - 0.5]) {
      box(px, 0, z - 0.4, 0.9, height + 1.3, 0.9);
      roof(px - 0.08, z - 0.48, 1.06, 1.06, height + 1.3, 0.9);
    }
  } else if(roofStyle==='spire') {
    const corners=[[x-0.18,height,z-0.18],[x+width+0.18,height,z-0.18],[x+width+0.18,height,z+depth+0.18],[x-0.18,height,z+depth+0.18]],apex=[0,height+Math.max(width,depth)*1.15,0];
    for(let i=0;i<4;i++)triangles.push({material:'roof',corners:[{point:corners[i],uv:[0,1]},{point:corners[(i+1)%4],uv:[2,1]},{point:apex,uv:[1,0]}]});
  } else {
    roof(x - 0.22, z - 0.22, width + 0.44, depth + 0.44, height, type === 'chapel' ? 1.7 : 1.15);
    box(x + 0.45, height, 0.5, 0.42, 1.4, 0.45);
    box(x + 0.4, height + 1.33, 0.45, 0.52, 0.13, 0.55, 'dark');
    if (type === 'chapel') {
      box(-0.48, height + 0.6, z - 0.25, 0.96, 1.8, 1.1);
      roof(-0.58, z - 0.35, 1.16, 1.3, height + 2.4, 1);
      windowFront(-0.18, height + 1.2, z - 0.28, 0.36, 0.7);
    }
  }
  // Planked door, iron straps, steps, and inset lit windows.
  box(-0.37, 0.25, z - 0.08, 0.74, 1.25, 0.12, 'dark');
  box(-0.3, 0.25, z - 0.15, 0.6, 1.15, 0.1, 'wood');
  for (const y of [0.45, 1.15]) box(-0.3, y, z - 0.18, 0.6, 0.045, 0.04, 'iron');
  for (let i = 0; i < 3; i++) box(-0.52, 0.02 + i * 0.07, z - 0.62 + i * 0.16, 1.04, 0.07, 0.5 - i * 0.14);
  windowFront(x + 0.35, 0.9, z);
  if (width > 2) windowFront(width / 2 - 0.8, 0.9, z);
  if (type === 'tower') windowFront(-0.2, 2.7, z, 0.4, 0.65);
  return { triangles, materials, footprint: { columns: width, rows: depth }, type,
    height: roofStyle==='spire'?height+Math.max(width,depth)*1.15:type === 'chapel' ? height + 3.4 : type === 'keep' ? height + 2.2 : height + 1.5 };
}

/** Open circular architecture, domes and curved ribs use actual mesh geometry. */
function createGroveBuilding({type,seed,palette}){
  if(!Number.isInteger(seed)||!palette||typeof palette!=='object'||Object.entries(palette).some(([key,rgb])=>!['wall','roof','wood'].includes(key)||!Array.isArray(rgb)||rgb.length!==3||rgb.some(v=>!Number.isFinite(v)||v<0||v>1)))throw RangeError('Invalid procedural building');
  const triangles=[],materials={wall:{color:[0.86,0.91,0.87,1],texture:'wall'},roof:{color:[0.42,0.76,0.71,1],texture:'roof'},wood:{color:[0.66,0.78,0.66,1],texture:'wood'},silver:{color:[0.73,0.86,0.88,1]},water:{color:[0.16,0.65,0.76,1]},dark:{color:[0.06,0.14,0.16,1]},light:{color:[0.78,0.98,0.92,1]}};
  for(const [key,rgb]of Object.entries(palette))materials[key].color=[...rgb,1];
  const quad=(a,b,c,d,material,uvs=[[0,0],[1,0],[1,1],[0,1]])=>{for(const ids of [[0,1,2],[0,2,3]])triangles.push({material,corners:ids.map(i=>({point:[...[a,b,c,d][i]],uv:uvs[i]}))});};
  const lathe=(profile,material,sides=48,cx=0,cz=0)=>{
    for(let j=1;j<profile.length;j++)for(let i=0;i<sides;i++){
      const a=i/sides*Math.PI*2,b=(i+1)/sides*Math.PI*2,[r,y]=profile[j-1],[s,z]=profile[j];
      quad([cx+Math.cos(a)*r,y,cz+Math.sin(a)*r],[cx+Math.cos(a)*s,z,cz+Math.sin(a)*s],[cx+Math.cos(b)*s,z,cz+Math.sin(b)*s],[cx+Math.cos(b)*r,y,cz+Math.sin(b)*r],material,[[i/sides*4,y*.6],[i/sides*4,z*.6],[(i+1)/sides*4,z*.6],[(i+1)/sides*4,y*.6]]);
    }
  };
  const rib=(points,radius,material)=>{
    for(let j=1;j<points.length;j++){
      const a=points[j-1],b=points[j],v=b.map((n,i)=>n-a[i]),length=Math.hypot(...v);if(length<1e-6)continue;
      const n=v.map(x=>x/length),ref=Math.abs(n[1])>.9?[1,0,0]:[0,1,0],u=[n[1]*ref[2]-n[2]*ref[1],n[2]*ref[0]-n[0]*ref[2],n[0]*ref[1]-n[1]*ref[0]],l=Math.hypot(...u);for(let k=0;k<3;k++)u[k]/=l;
      const w=[n[1]*u[2]-n[2]*u[1],n[2]*u[0]-n[0]*u[2],n[0]*u[1]-n[1]*u[0]];
      const at=(p,t)=>p.map((x,k)=>x+radius*(u[k]*Math.cos(t)+w[k]*Math.sin(t)));
      for(let i=0;i<8;i++){const x=i*Math.PI/4,y=(i+1)*Math.PI/4;quad(at(a,x),at(a,y),at(b,y),at(b,x),material);}
    }
  };
  const ring=(radius,y,thickness,material)=>lathe([[radius-thickness,y-thickness],[radius+thickness,y-thickness],[radius+thickness,y+thickness],[radius-thickness,y+thickness],[radius-thickness,y-thickness]],material);
  let height;
  if(type==='moonwell'){
    lathe([[0,0],[1.95,0],[1.95,.15],[1.75,.15],[1.75,.32],[0,.32]],'wall');
    lathe([[1.15,.32],[1.45,.32],[1.45,.7],[1.15,.7],[1.15,.32]],'wall');ring(1.31,.72,.11,'silver');
    lathe([[0,.56],[1.16,.56]],'water');
    for(let i=0;i<6;i++){
      const a=i*Math.PI/3,x=Math.cos(a)*1.62,z=Math.sin(a)*1.62;
      lathe([[.25,.32],[.25,.42],[.22,.46],[.22,.5],[.13,.59],[.115,2.48],[.16,2.54],[.22,2.62],[.24,2.7],[.24,2.83]],'wall',24,x,z);
      for(let flute=0;flute<8;flute++){const f=flute*Math.PI/4;rib([[x+Math.cos(f)*.118,.62,z+Math.sin(f)*.118],[x+Math.cos(f)*.118,2.43,z+Math.sin(f)*.118]],.014,'silver');}
      lathe([[.235,.47],[.235,.51]],'silver',24,x,z);
      const points=[];for(let j=0;j<=16;j++){const t=j/16;points.push([x*(1-t*.7),2.78+Math.sin(t*Math.PI/2)*1.05,z*(1-t*.7)]);}rib(points,.07,'silver');
    }
    ring(.48,3.83,.06,'silver');lathe([[0,1],[.2,1.35],[0,1.75]],'light',8);height=4;
  }else if(type==='observatory'){
    lathe([[0,0],[2.2,0],[2.2,.15],[2,.15],[2,.3],[1.75,.3],[1.75,2.45],[0,2.45]],'wall');
    ring(1.76,.4,.07,'silver');ring(1.78,2.4,.12,'silver');ring(1.79,.69,.035,'silver');ring(1.79,2.18,.035,'silver');
    lathe([[1.77,.3],[1.86,.34],[1.86,.41],[1.79,.46]],'wall');
    const dome=[];for(let i=0;i<=16;i++){const t=i/16*Math.PI/2;dome.push([Math.cos(t)*1.8,2.5+Math.sin(t)*1.45]);}lathe(dome,'roof');
    for(let i=0;i<12;i++){const a=i*Math.PI/6;rib(dome.map(([r,y])=>[Math.cos(a)*r,y+.025,Math.sin(a)*r]),.035,'silver');}
    for(let i=0;i<12;i++){
      const a=i*Math.PI/6,x=Math.cos(a)*1.765,z=Math.sin(a)*1.765;
      rib([[x,.6,z],[x,2.2,z]],.09,'wood');
      const b=a+.16,c=a+.37;quad([Math.cos(b)*1.76,1.05,Math.sin(b)*1.76],[Math.cos(c)*1.76,1.05,Math.sin(c)*1.76],[Math.cos(c)*1.76,1.95,Math.sin(c)*1.76],[Math.cos(b)*1.76,1.95,Math.sin(b)*1.76],'dark');
      const frame=[[b,1.02],[b,1.96],[(b+c)/2,2.1],[c,1.96],[c,1.02],[b,1.02]].map(([angle,y])=>[Math.cos(angle)*1.79,y,Math.sin(angle)*1.79]);rib(frame,.025,'silver');
      const mid=(b+c)/2;rib([[Math.cos(mid)*1.8,1.06,Math.sin(mid)*1.8],[Math.cos(mid)*1.8,2.06,Math.sin(mid)*1.8]],.017,'silver');
    }
    for(const tilt of [-.65,.65]){const points=[];for(let j=0;j<=40;j++){const a=j/40*Math.PI*2;points.push([Math.cos(a)*.5,4.2+Math.sin(a)*.5*Math.cos(tilt),Math.sin(a)*.5*Math.sin(tilt)]);}rib(points,.035,'silver');}
    height=4.8;
  }else{
    for(const x of [-1.4,1.4]){
      lathe([[0,0],[.57,0],[.57,.12],[.51,.17],[.51,.28],[.34,.38],[.28,.48],[.22,2.58],[.3,2.68],[.34,2.76],[.34,2.9]],'wall',32,x,0);
      for(let flute=0;flute<12;flute++){const a=flute*Math.PI/6;rib([[x+Math.cos(a)*.26,.52,Math.sin(a)*.26],[x+Math.cos(a)*.225,2.55,Math.sin(a)*.225]],.023,'silver');}
      lathe([[.32,2.69],[.36,2.73],[.36,2.78]],'silver',32,x,0);
      const points=[];for(let j=0;j<=16;j++){const t=j/16;points.push([x*(1-t),2.8+Math.sin(t*Math.PI/2)*1.8,0]);}rib(points,.17,'wood');
      rib(points.map(([a,b,c])=>[a,b+.12,c-.05]),.045,'silver');
      for(const direction of [-1,1])rib([[x,1.5,0],[x+direction*.32,2.15,0],[x+direction*.65,2.55,0]],.06,'wood');
    }
    lathe([[0,3.6],[.16,3.85],[0,4.1]],'light',8);height=4.8;
  }
  return {triangles,materials,footprint:{columns:type==='observatory'?5:4,rows:type==='gateway'?2:type==='observatory'?5:4},height,type,...(type==='gateway'?{collision:[{column:0,row:0,columns:1,rows:2},{column:3,row:0,columns:1,rows:2}]}:{})};
}
createGameBuilding.controlFactory = Object.freeze({topics:['games'],constructor:'createGameBuilding({type:"cottage",seed:42})',methods:'Generate original textured Y-up triangle meshes and authoritative footprints for cottage, chapel, tower, keep, moonwell, observatory or gateway. Moonwell has an open stepped basin, six columns and curved canopy ribs; observatory has a round drum, ribbed dome and crossed armillary rings; gateway has two separate columns and an open pointed arch. Walls/foundations, pitched roofs or battlements, chimneys, timber beams, lit inset windows, iron-strapped doors and steps are geometry. Returns rasterizeGameModel-compatible {triangles,materials,footprint,height,type}; bind wall/roof/wood decoded textures, or createGameMaterial. Bake once at actual screen resolution; draw with consistent foot pivot/camera/depth sort and apply the same footprint to navigation/fog. roofStyle:pitched|spire|battlements changes real roof geometry; palette:{wall:[r,g,b],roof:[r,g,b],wood:[r,g,b]} controls material colors (0..1) while actual textures are supplied separately. Use different shapes, maps, materials and art direction per requested genre. No interior simulation, PBR shader, automatic placement or claim to arbitrary architectural generation.'});

export function createGameMaterial({ type = 'roof', seed = 1, size = 128, baseColor = null } = {}) {
  if (!['roof','wood','stone'].includes(type) || !Number.isInteger(seed) || !Number.isInteger(size) || size < 16 || size > 256) throw RangeError('Invalid procedural material');
  if(baseColor!==null&&(!Array.isArray(baseColor)||baseColor.length!==3||baseColor.some(n=>!Number.isInteger(n)||n<0||n>255)))throw RangeError('Invalid material palette');
  const random = gameSeededRandom(seed), rgba = new Uint8Array(size * size * 4);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    let base, shade = 0.88 + random() * 0.24;
    if (type === 'roof') {
      const row = Math.floor(y / 12), seam = (x + row % 2 * 10) % 20;
      base = [78, 86, 82]; shade *= y % 12 < 2 || seam < 1 ? 0.38 : 0.76 + (y % 12) / 42;
    } else if (type === 'wood') {
      base = [151, 119, 77]; shade *= 0.8 + 0.17 * Math.sin(x * 0.6 + Math.sin(y * 0.08) * 2) + 0.05 * Math.sin(x * 2.4 + y * 0.07);
      if (x % 24 < 1) shade *= 0.45;
    } else {
      base = [164, 155, 132]; const row = Math.floor(y / 18);
      if (y % 18 < 2 || (x + row % 2 * 16) % 32 < 2) shade *= 0.45;
    }
    const at = (y * size + x) * 4;
    for (let c = 0; c < 3; c++) rgba[at+c] = Math.min(255, Math.round((baseColor??base)[c] * shade));
    rgba[at+3] = 255;
  }
  return { width: size, height: size, rgba };
}
createGameMaterial.controlFactory = Object.freeze({topics:['games'],constructor:'createGameMaterial({type:"roof",seed:42,size:128})',methods:'Deterministic original shingle, wood-grain or masonry diffuse RGBA texture, 16..256 pixels square. Use as rasterizeGameModel textures roof/wood/wall. This is procedurally generated diffuse material, not a photographed or PBR material. baseColor:[r,g,b] (0..255) changes the procedural pigment while preserving real shingle/grain/masonry patterns. Imported photographic textures may be bound instead.'});

/** Seeded volumetric foliage: real branches and folded leaf meshes, not flat tree cutouts. */
export function createGameTree({type='oak',seed=1}={}){
  if(!['oak','pine','birch'].includes(type)||!Number.isInteger(seed))throw RangeError('Invalid procedural tree');
  const random=gameSeededRandom(seed),triangles=[],materials={bark:{color:type==='birch'?[.86,.85,.76,1]:[.42,.31,.2,1],texture:'bark'}};
  for(let i=0;i<16;i++)materials['leaf'+i]={color:type==='pine'?[.12+i*.009,.25+i*.017,.17+i*.009,1]:[.18+i*.016,.32+i*.019,.1+i*.009,1]};
  let windJoint=-1;const triangle=(points,material,leaf=null)=>triangles.push({material,wind:{joint:windJoint,...(leaf?{center:[...leaf.center],mode:leaf.mode}:{})},corners:points.map((point,i)=>({point,uv:[[0,0],[1,0],[.5,1]][i]}))});
  const branch=(a,b,r1,r2)=>{
    const n=b.map((v,i)=>v-a[i]),length=Math.hypot(...n);for(let k=0;k<3;k++)n[k]/=length;
    let u=[n[1],-n[0],0];if(Math.hypot(...u)<.01)u=[1,0,0];const l=Math.hypot(...u);for(let k=0;k<3;k++)u[k]/=l;
    const v=[n[1]*u[2]-n[2]*u[1],n[2]*u[0]-n[0]*u[2],n[0]*u[1]-n[1]*u[0]],at=(p,r,t)=>p.map((x,k)=>k===1&&p[1]===0?0:x+r*(u[k]*Math.cos(t)+v[k]*Math.sin(t)));
    for(let i=0;i<8;i++){const x=i*Math.PI/4,y=(i+1)*Math.PI/4,aa=at(a,r1,x),ab=at(a,r1,y),ba=at(b,r2,x),bb=at(b,r2,y);triangle([aa,ab,bb],'bark');triangle([aa,bb,ba],'bark');}
  };
  const height=(type==='pine'?7:type==='birch'?6.2:5.8)*(.85+random()*.3),lean=[(random()-.5)*.8,0,(random()-.5)*.8],trunk=type==='oak'?.25:.15;
  const bend=[lean[0]*.4,height*.38,lean[2]*.4];branch([0,0,0],bend,trunk,trunk*.68);branch(bend,[lean[0],height*.85,lean[2]],trunk*.68,.04);
  for(let i=0;i<7;i++){
    windJoint=i;const angle=i*2.399+random()*1.1,y=(type==='pine'?1.3+i*.65:2.3+i*.34)*height/(type==='pine'?7:type==='birch'?6.2:5.8),spread=type==='pine'?1.9-i*.19:type==='birch'?1.05+random()*.35:1.45+random()*.35;
    const center=[lean[0]+Math.cos(angle)*spread*(.6+random()*.4),y+.5+(random()-.5)*.4,lean[2]+Math.sin(angle)*spread*(.6+random()*.4)],fork=[center[0]*.6,y+.1,center[2]*.6];branch([lean[0]*.6,y-.7,lean[2]*.6],fork,.08,.045);branch(fork,center,.045,.018);
    branch(fork,[center[0]+.25,center[1]-.15,center[2]-.25],.035,.012);
    for(let j=0;j<220;j++){
      const a=random()*Math.PI*2,z=random()*2-1,r=Math.sqrt(Math.max(0,1-z*z))*Math.cbrt(random()),radius=type==='pine'?Math.max(.45,1.3-i*.1):type==='birch'?.9:1.15;
      const c=[center[0]+Math.cos(a)*r*radius,center[1]+z*(type==='pine'?.5:1.0),center[2]+Math.sin(a)*r*radius],yaw=random()*Math.PI*2,pitch=(random()-.5)*1.8,size=type==='pine'?.125:.16;
      const u=[Math.cos(yaw)*size,Math.sin(pitch)*size*.5,Math.sin(yaw)*size],v=[-Math.sin(yaw)*size*.58,Math.cos(pitch)*size*.6,Math.cos(yaw)*size*.58];
      const points=[c.map((x,k)=>x-u[k]),c.map((x,k)=>x+v[k]),c.map((x,k)=>x+u[k]),c.map((x,k)=>x-v[k])],ridge=[c[0],c[1]+size*.2,c[2]],material='leaf'+Math.floor(random()*16);
      for(let k=0;k<4;k++)triangle([points[k],points[(k+1)%4],ridge],material,{center:c,mode:j%16});
    }
  }
  return {type,triangles,materials,height,wind:{joints:7,leafModes:16},footprint:{columns:2,rows:2}};
}
createGameTree.controlFactory=Object.freeze({topics:['games'],constructor:'createGameTree({type:"oak",seed:42})',methods:'Original deterministic oak, pine or birch meshes with tapered 3D trunks, branches and 1540 folded leaf meshes distributed through seven canopy volumes. Bind a bark texture, light using rasterizeGameModel and bake camera-matched views. Returns {triangles,materials,height,footprint}; approximately 6300 triangles, bounded geometry. Leaves have real surface normals and depth; these are not flat photographed tree sprites. Add ground shadows and depth sorting in the world, use fog visibility/exploration, and avoid oversized per-frame raster work. Fixed-camera baked views do not provide dynamic shadow maps or GPU wind simulation.'});

/** Small damped spring system for procedural tree deformation. Geometry is
 * always sampled from the immutable rest mesh, so motion cannot accumulate drift. */
export class GameTreeWind {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameTreeWind({seed:42,position:[12,8],direction:[1,0.35],speed:1.3,period:3.2})',methods:'setWind({direction:[x,z],speed}); update(delta); deform(createGameTree({type,seed})); inspect(). A shared deterministic world-space gust field drives damped trunk/branch springs and sixteen faster leaf angular modes. update accepts 0..0.25 seconds and uses <=1/120-second semi-implicit substeps. deform returns a new triangle mesh: root anchored, height-weighted trunk bend, seven branch responses and leaf rotation around each leaf centre; recompute lighting via rasterizeGameModel. No whole-image translation, cumulative vertex drift, arbitrary rigid-body collisions, GPU wind or shadow maps. Stop the wind and springs settle. For a bounded fixed-camera probe, pre-bake the actual deformed mesh poses into a loop after settling; retain geometry/lighting and label baked animation honestly. The live simulation can also deform a mesh for a suitable renderer. Lifecycle belongs to the game; advance only while playing and clear/recreate on reset.'});
  constructor({seed=1,position=[0,0],direction=[1,.35],speed=1.3,period=3.2}={}){
    if(!Number.isInteger(seed)||!Array.isArray(position)||position.length!==2||position.some(v=>!Number.isFinite(v)||Math.abs(v)>2147483647)||!Number.isFinite(period)||period<1||period>30)throw RangeError('Invalid tree wind field');
    this.seed=seed;this.position=[...position];this.period=period;this.time=0;this.trunk={x:0,z:0,vx:0,vz:0};this.branches=Array.from({length:7},()=>({x:0,z:0,vx:0,vz:0}));this.leaves=Array.from({length:16},()=>({x:0,z:0,vx:0,vz:0}));this.setWind({direction,speed});
  }
  setWind({direction=this.direction,speed=this.speed}={}){
    if(!Array.isArray(direction)||direction.length!==2||direction.some(v=>!Number.isFinite(v))||!Math.hypot(...direction)||!Number.isFinite(speed)||speed<0||speed>12)throw RangeError('Invalid tree wind');
    const n=Math.hypot(...direction);this.direction=direction.map(v=>v/n);this.speed=speed;return this;
  }
  update(delta){
    if(!Number.isFinite(delta)||delta<0||delta>.25)throw RangeError('Tree wind requires 0..0.25 seconds');
    const count=Math.ceil(delta*120),step=count?delta/count:0;
    const spring=(state,fx,fz,k,damping,mass,limit)=>{state.vx+=(fx-k*state.x-damping*state.vx)/mass*step;state.vz+=(fz-k*state.z-damping*state.vz)/mass*step;state.x+=state.vx*step;state.z+=state.vz*step;for(const [p,v]of [['x','vx'],['z','vz']])if(Math.abs(state[p])>limit){state[p]=Math.sign(state[p])*limit;state[v]*=.2;}};
    for(let n=0;n<count;n++){
      this.time+=step;const t=this.time/this.period*Math.PI*2,phase=this.position[0]*.11+this.position[1]*.07,seed=(this.seed%997)*.013;
      const gust=1+.28*Math.sin(t+phase)+.12*Math.sin(t*3+phase*.7),pressure=this.speed*this.speed*gust,fx=this.direction[0]*pressure,fz=this.direction[1]*pressure;
      spring(this.trunk,fx*.3,fz*.3,12,5,3,.4);
      for(let i=0;i<7;i++){const response=1+.16*Math.sin(t*3+i*.9+seed);spring(this.branches[i],fx*.35*response,fz*.35*response,7+i*.9,3.5+i*.13,1.2+i*.08,.6);}
      for(let i=0;i<16;i++){const flutter=Math.sin(t*5+i*.87+seed)+.25*Math.sin(t*3+i*.4);spring(this.leaves[i],fz*1.5*flutter,fx*1.5*flutter,35,3,.3,.4);}
    }
    return this;
  }
  deform(mesh){
    if(!Array.isArray(mesh?.triangles)||mesh.triangles.length>16384||!Number.isFinite(mesh.height)||mesh.height<=0||mesh.wind?.joints!==7)throw RangeError('Tree wind requires a procedural rest tree');
    const bend=(point,joint)=>{const h=Math.max(0,Math.min(1,point[1]/mesh.height)),trunk=h*h,branch=joint>=0?this.branches[joint]:null,weight=Math.min(1,Math.hypot(point[0],point[2])/1.2)*h;
      return [point[0]+this.trunk.x*trunk+(branch?.x??0)*weight,point[1]-.05*(this.trunk.x*this.trunk.x+this.trunk.z*this.trunk.z)*trunk,point[2]+this.trunk.z*trunk+(branch?.z??0)*weight];};
    const triangles=mesh.triangles.map(triangle=>{const binding=triangle.wind;if(!Number.isInteger(binding?.joint)||binding.joint< -1||binding.joint>6)throw RangeError('Missing tree wind binding');
      const center=binding.center?bend(binding.center,binding.joint):null,leaf=center?this.leaves[binding.mode]:null;if(center&&!leaf)throw RangeError('Invalid leaf wind mode');
      const corners=triangle.corners.map(corner=>{let point;
        if(center){const dx=corner.point[0]-binding.center[0],dy=corner.point[1]-binding.center[1],dz=corner.point[2]-binding.center[2],cx=Math.cos(leaf.x),sx=Math.sin(leaf.x),cz=Math.cos(leaf.z),sz=Math.sin(leaf.z),yy=dy*cx-dz*sx,zz=dy*sx+dz*cx;point=[center[0]+dx*cz-yy*sz,center[1]+dx*sz+yy*cz,center[2]+zz];}
        else point=bend(corner.point,binding.joint);
        return {...corner,point};
      });return {...triangle,corners};
    });
    return {...mesh,triangles};
  }
  inspect(){const copy=s=>({...s});return {time:this.time,speed:this.speed,direction:[...this.direction],trunk:copy(this.trunk),branches:this.branches.map(copy),leaves:this.leaves.map(copy)};}
}
