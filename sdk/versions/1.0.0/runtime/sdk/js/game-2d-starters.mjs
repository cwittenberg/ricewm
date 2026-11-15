import {gameSeededRandom} from './game-chunks.mjs';
import {GamePlatformMap} from './game-2d.mjs';
/** Original small games demonstrating shared helpers, not a universal art style. */
export const DEFAULT_PLATFORM_MAP={schema:'ricewm.platform-map/v1',width:2560,height:480,spawn:{x:40,y:284},goal:{x:2460,y:260,width:36,height:60},platforms:[{x:0,y:320,width:520,height:160},{x:608,y:320,width:1000,height:160},{x:1704,y:320,width:856,height:160},{x:240,y:230,width:142,height:24,oneWay:true},{x:760,y:216,width:160,height:24,oneWay:true},{x:1130,y:240,width:140,height:24,oneWay:true},{x:1850,y:220,width:170,height:24,oneWay:true},{x:2190,y:250,width:140,height:24,oneWay:true}],pickups:Array.from({length:27},(_,i)=>({id:'acorn-'+i,x:130+i*82,y:274,kind:i%7===6?'gem':'coin',points:i%7===6?250:100})).filter(p=>p.x<500||p.x>628&&p.x<1580||p.x>1724),enemies:[{id:'thornling-1',x:420,y:292,width:32,height:28,minX:388,maxX:480,speed:34},{id:'thornling-2',x:970,y:292,width:32,height:28,minX:950,maxX:1070,speed:42},{id:'thornling-3',x:2040,y:292,width:32,height:28,minX:2030,maxX:2140,speed:48},{id:'moth-1',kind:'moth',x:1400,y:230,width:32,height:28,minX:1330,maxX:1470,speed:27,amplitude:12}],decorations:[]};
const sceneryRandom=gameSeededRandom(912738);for(let x=24;x<2520;x+=30+sceneryRandom()*62){if(x>496&&x<630||x>1586&&x<1726)continue;const n=sceneryRandom(),frame=n<.3?'fern':n<.55?'flower':n<.8?'mushroom':'rock';DEFAULT_PLATFORM_MAP.decorations.push({frame,x:Math.round(x),y:322,scale:.2+sceneryRandom()*.27,flipX:sceneryRandom()<.5,rotation:(sceneryRandom()-.5)*.1});}


export function game2DStarterSource(template,{map=DEFAULT_PLATFORM_MAP,artwork}={}){
 if(template==='game-board')return `import {defineApp,createGameView,GameHUD,GameTransitions,gameGridCell,drawGameText,drawGame2DPanel,layoutGame2DHud} from '@ricewm/app';
export default defineApp({start({createWindow}){
 const motion=new GameTransitions(),lines=[[0,1,2],[3,4,5],[6,7,8],[0,3,6],[1,4,7],[2,5,8],[0,4,8],[2,4,6]];
 let board,turn,winner,selected,winning,view;
 function reset(){board=Array(9).fill('');turn='X';winner='';selected=0;winning=null;motion.clear();}
 const status=()=>winner?(winner==='Draw'?'Draw':winner+' wins'):turn+' to move';
 function move(i){if(winner||board[i])return;board[i]=turn;const x=220+i%3*100,y=95+Math.floor(i/3)*94;
  motion.set('piece-'+i,{x:turn==='X'?80:560,y:40,scale:.4,angle:turn==='X'?-.8:.8}).to('piece-'+i,{x,y,scale:1,angle:0},{duration:.38,ease:'out'});
  winning=lines.find(line=>line.every(n=>board[n]===turn))||null;winner=winning?turn:board.every(Boolean)?'Draw':'';if(!winner)turn=turn==='X'?'O':'X';
  if(winning)motion.set('win',{progress:0}).to('win',{progress:1},{duration:.6,delay:.2});view.setStatus(status());view.animate();
 }
 reset();view=createGameView({createWindow,continuous:false,hud:new GameHUD({theme:{text:0xffdce8ec}}),layoutHud:layoutGame2DHud,
  instructions:'Two players share the board. Arrows select a square; Enter or Space places a stone. Three in a row wins. R starts another round. H opens this guide.',
  animate(dt,options){return motion.advance(dt,options);},reset(){reset();view.setStatus(status());},pointer({x,y}){const c=gameGridCell(x,y,{x:170,y:48,width:300,height:282,columns:3,rows:3});if(c){selected=c.index;move(selected);}},
  actions:[{name:'left',keys:[113,38],run(){selected=(selected+8)%9;}},{name:'right',keys:[114,40],run(){selected=(selected+1)%9;}},{name:'up',keys:[111,25],run(){selected=(selected+6)%9;}},{name:'down',keys:[116,39],run(){selected=(selected+3)%9;}},{name:'place',keys:[36,65],run(){move(selected);}}],
  render(scene){scene.linearGradient([0,0,640,360],[0,0],[640,360],[{offset:0,color:0xff302b22},{offset:1,color:0xff111b16}]);
   for(let i=0;i<32;i++)scene.line([0,10+i*12],[640,3+i*12],.5,0x142f7082);
   drawGameText(scene,'THE QUIET',{x:30,y:132,size:17,font:'serif',maxWidth:125,color:0xff96afb9});drawGameText(scene,'GROVE',{x:30,y:155,size:25,font:'serif',maxWidth:125,color:0xffdce8ec});
   drawGameText(scene,'Two minds.',{x:492,y:156,size:12,maxWidth:126,color:0xff96afb9});drawGameText(scene,'One clearing.',{x:492,y:175,size:12,maxWidth:126,color:0xff96afb9});
   scene.ellipse([158,42,326,296],0x70070907);drawGame2DPanel(scene,[166,44,308,290],{radius:24,fill:0xff314432,border:0xff8baca6});
   for(let i=1;i<3;i++){scene.line([170+i*100,58],[170+i*100,320],1,0x8093b3b2);scene.line([180,48+i*94],[460,48+i*94],1,0x8093b3b2);}
   const sx=220+selected%3*100,sy=95+Math.floor(selected/3)*94;scene.ellipse([sx-34,sy-34,68,68],0x257dc2d3).ellipse([sx-34,sy-34,68,68],0x8098d2df,{filled:false,width:1});
   board.forEach((mark,i)=>{if(!mark)return;const t=motion.get('piece-'+i);scene.ellipse([t.x-27,t.y+20,58,15],0x60050907);scene.pushTransform([Math.cos(t.angle)*t.scale,Math.sin(t.angle)*t.scale,-Math.sin(t.angle)*t.scale,Math.cos(t.angle)*t.scale,t.x,t.y]);
    scene.ellipse([-29,-27,58,60],mark==='X'?0xff596875:0xff42514e).ellipse([-29,-31,58,58],mark==='X'?0xffc1dce8:0xff7eaaa9).ellipse([-24,-26,48,48],mark==='X'?0xffdcecf0:0xffa2cbc5).ellipse([-22,-24,44,44],0x405f7d81,{filled:false,width:1});
    if(mark==='X')scene.line([-11,-13],[11,9],4,0xff4a727b).line([11,-13],[-11,9],4,0xff4a727b);else scene.ellipse([-13,-15,26,26],0xff355f60,{filled:false,width:4});scene.popTransform();
   });
   if(winning){const p=motion.get('win').progress,a=winning[0],b=winning[2],x=220+a%3*100,y=95+Math.floor(a/3)*94;scene.line([x,y],[x+(b%3-a%3)*100*p,y+(Math.floor(b/3)-Math.floor(a/3))*94*p],3,0xffa5e8ef);}
  }});view.setStatus(status());
}});
`;
 if(template==='game-platform')return `import {defineApp,createGameView,GameHUD,GamePlatformMap,GamePlatformSession,GameTransitions,createGameArtworkWindow,drawGameParallax,drawGameText,layoutGame2DHud} from '@ricewm/app';
// Editable map data: use the same platforms and placements for gameplay and art.
const mapData=${JSON.stringify(template==='game-platform'?new GamePlatformMap(map).toJSON():map)};
export default defineApp({start({createWindow,services}){
 return createGameArtworkWindow({createWindow,services,artwork:${JSON.stringify(artwork??null)},ready({atlas,createWindow}){
 const map=new GamePlatformMap(mapData),game=new GamePlatformSession(map),feedback=new GameTransitions();let view,particles=[],feedbackSequence=0;
 const status=()=>game.status==='won'?'You win! '+game.score+' points':game.status==='lost'?(game.health?'You fell! ':'Defeated! ')+game.score+' points':'Treasures '+game.collected.size+'/'+map.data.pickups.length+'  ·  '+game.score+' points  ·  Health '+game.health;
 const layers=[{frame:'sky',depth:.035,x:-600,y:-8,scale:2.9,speed:0},{frame:'forest-far',depth:.17,y:88,scale:2.4,speed:-.7,mirror:true},{frame:'forest-near',depth:.43,y:80,scale:2.6,mirror:true}];
 function advance(dt,input){const events=game.step(dt,input);for(const e of events)if(e.type==='pickup'||e.type==='stomp'){while(particles.length>=12)feedback.delete(particles.shift().id);const id='feedback-'+feedbackSequence++;feedback.set(id,{rise:0,alpha:1}).to(id,{rise:38,alpha:0},{duration:.65,ease:'out'});particles.push({...e,id});}
  feedback.advance(dt);particles=particles.filter(p=>{if(feedback.get(p.id).alpha>0)return true;feedback.delete(p.id);return false;});view.setStatus(status());return game.status==='playing';}
 view=createGameView({createWindow,hud:new GameHUD({theme:{text:0xffe8f1ed}}),layoutHud:layoutGame2DHud,
  instructions:'A/D or Left/Right runs. Hold Space/W/Up to jump higher; release for a short hop. Stomp thornlings from above. Collect acorns and crystals; reach the pennant. P or Escape pauses. N steps while paused; hold movement/jump with N for reduced motion. R restarts.',
  actions:[{name:'left',keys:[113,38]},{name:'right',keys:[114,40]},{name:'jump',keys:[65,111,25]}],
  reset(){game.reset();feedback.clear();particles=[];feedbackSequence=0;view.setStatus(status());},update(dt,{input}){return advance(dt,{axis:input.axis('left','right'),jumpPressed:input.takePressed('jump'),jumpHeld:input.down('jump')});},
  render(scene){const s=game.snapshot(),camera=s.cameraX;scene.rect([0,0,640,360],0xff302b18);drawGameParallax(scene,atlas,layers,{cameraX:camera,time:s.time,imageId:70});
   const art=(frame,x,y,scale=1,extra={})=>atlas.draw(scene,70,frame,{x:x-camera,y,scale,...extra});
   for(const p of map.data.platforms){if(p.x+p.width<camera-40||p.x>camera+680)continue;scene.linearGradient([p.x-camera,p.y+10,p.width,p.height],[0,p.y],[0,p.y+p.height],[{offset:0,color:0xff28392b},{offset:1,color:0xff151e19}]);for(let x=p.x+38;x<p.x+p.width+45;){const variation=((Math.round(x)*2654435761)>>>0)/4294967296,scale=.72+variation*.2;scene.pushClip([p.x-camera,p.y-12,p.width,p.height+60]);art('ledge',x,p.y-2+57*scale,scale,{flipX:variation>.5});scene.popClip();x+=66+variation*10;}}
   for(const d of map.data.decorations)if(Math.abs(d.x-camera-320)<390)art(d.frame,d.x,d.y,d.scale,{flipX:!!d.flipX,rotation:(d.rotation||0)+(['fern','flower'].includes(d.frame)?Math.sin(s.time*1.8+d.x)*.025:0)});
   for(const p of map.data.pickups)if(!game.collected.has(p.id)&&Math.abs(p.x-camera-320)<360){const bob=Math.sin(s.time*3+p.x)*3;art(p.kind==='gem'?'gem':p.kind==='health'?'flower':'coin',p.x+10,p.y+23+bob,.27);}
   for(const e of s.enemies)if(e.alive&&Math.abs(e.x-camera-320)<380){scene.ellipse([e.x-camera-5,e.y+e.height-3,e.width+10,8],0x550b100c);art(atlas.sample(e.kind==='moth'?'moth':'crawler',s.time),e.x+e.width/2,e.y+e.height+3,.42,{flipX:e.direction>0});}
   art('banner',map.data.goal.x+16,map.data.goal.y+64,.63,{rotation:Math.sin(s.time*2)*.025});
   const p=s.player;scene.ellipse([p.x-camera-5,320-4,34,8],0x550c160f);const frame=!p.grounded?'jump':Math.abs(p.vx)>8?atlas.sample('run',s.animationTime):'idle';if(!s.invulnerable||Math.floor(s.time*12)%2)art(frame,p.x+12,p.y+40,.56,{flipX:s.facing<0});
   for(const f of particles){const t=feedback.get(f.id);scene.pushTransform([1,0,0,1,0,0],t.alpha);drawGameText(scene,'+'+f.points,{x:f.x-camera,y:f.y-t.rise,size:13,maxWidth:90,color:0xffb4eef6});scene.popTransform();}
   drawGameParallax(scene,atlas,[{frame:'foreground',depth:.83,y:282,scale:1.45,mirror:true}],{cameraX:camera,time:s.time,imageId:70});
   // Subtle route progress; no permanent movement-button or shortcut strip.
   scene.line([20,348],[620,348],1,0x509bb9ad).line([20,348],[20+600*p.x/(map.data.width-p.width),348],2,0xff97d1ca);
  }});const inspect=view.surface.inspectControl;view.surface.inspectControl=o=>({...inspect(o),platform:game.snapshot()});view.setStatus(status());
 }});
}});
`;
 if(template==='game-card')return `import {defineApp,createGameView,GameHUD,GameTransitions,GameDeck,gameSeededRandom,createGameArtworkWindow,drawGame2DPanel,drawGameText,layoutGame2DHud} from '@ricewm/app';
export default defineApp({start({createWindow,services}){
 return createGameArtworkWindow({createWindow,services,artwork:${JSON.stringify(artwork??null)},ready({atlas,createWindow}){
 const motion=new GameTransitions(),symbols=['coin','gem','mushroom','fern','moth-a','flower'],names=['Acorn','Crystal','Mushrooms','Fern','Moon moth','Bluebells'];let cards,shown,matched,moves,score,selected,resolving,wait,view,hovered=-1,deal=0;
 const target=i=>({x:44+i%6*92,y:64+Math.floor(i/6)*138});
 const status=()=>matched.size===12?'You win! '+score+' points in '+moves+' turns':matched.size/2+' / 6 pairs  ·  '+score+' points  ·  '+moves+' turns';
 function reset(){cards=new GameDeck(symbols.flatMap((frame,pair)=>[0,1].map(copy=>({id:pair+'-'+copy,pair,frame,name:names[pair]}))),{random:gameSeededRandom(548613+deal++)}).draw(12);shown=[];matched=new Set();moves=0;score=0;selected=0;hovered=-1;resolving=false;wait=0;motion.clear();cards.forEach((c,i)=>motion.set(c.id,{x:566,y:202,flip:0,lift:0}).to(c.id,{...target(i),flip:0,lift:0},{duration:.42,delay:i*.035,ease:'out'}));}
 function choose(i){if(resolving||matched.has(i)||shown.includes(i))return;selected=i;shown.push(i);const t=motion.get(cards[i].id);motion.to(cards[i].id,{...t,...target(i),flip:1,lift:-3},{duration:.26});
  if(shown.length===2){moves++;const [a,b]=shown;if(cards[a].pair===cards[b].pair){matched.add(a);matched.add(b);score+=200;shown=[];}else{resolving=true;wait=0;}}
  view.setStatus(status());view.animate();
 }
 function animate(dt,options){if(resolving){wait+=options.reducedMotion?2:dt;if(wait>=.95){for(const i of shown)motion.to(cards[i].id,{...target(i),flip:0,lift:0},{duration:.26});shown=[];resolving=false;}}motion.advance(dt,options);return resolving||motion.active;}
 reset();view=createGameView({createWindow,continuous:false,hud:new GameHUD({theme:{text:0xffdce8ec}}),layoutHud:layoutGame2DHud,animate,
  instructions:'Reveal two illustrated cards to find a pair. A match earns 200 points. Arrow keys select a card, Enter or Space reveals it; mouse selection works too. R deals again. H opens this guide. Reduced motion reveals and settles immediately.',
  reset(){reset();view.setStatus(status());view.animate();},hover(point){
   if(resolving||cards.some((c,i)=>Math.abs(motion.get(c.id).x-target(i).x)>1||Math.abs(motion.get(c.id).y-target(i).y)>1))return;
   const next=point?cards.findIndex((c,i)=>{const t=motion.get(c.id);return point.x>=t.x&&point.x<t.x+82&&point.y>=t.y+t.lift&&point.y<t.y+t.lift+122;}):-1;if(next===hovered)return;const previous=hovered;hovered=next;
   for(const i of [previous,next])if(i>=0)motion.to(cards[i].id,{...target(i),flip:shown.includes(i)||matched.has(i)?1:0,lift:i===hovered?-8:matched.has(i)?-3:0},{duration:.18,ease:'out'});view.animate();
  },pointer({x,y}){for(let i=cards.length-1;i>=0;i--){const t=motion.get(cards[i].id),half=41*Math.max(.025,Math.abs(Math.cos(t.flip*Math.PI)));if(x>=t.x+41-half&&x<t.x+41+half&&y>=t.y+t.lift&&y<t.y+t.lift+122){choose(i);break;}}},
  actions:[{name:'left',keys:[113,38],run(){selected=(selected+11)%12;}},{name:'right',keys:[114,40],run(){selected=(selected+1)%12;}},{name:'up',keys:[111,25],run(){selected=(selected+6)%12;}},{name:'down',keys:[116,39],run(){selected=(selected+6)%12;}},{name:'reveal',keys:[36,65],run(){choose(selected);}}],
  render(scene){scene.linearGradient([0,0,640,360],[0,0],[640,360],[{offset:0,color:0xff473a22},{offset:1,color:0xff17221a}]);for(let i=0;i<22;i++)scene.line([0,i*18],[640,i*18-25],.5,0x10319492);
   cards.forEach((c,i)=>{const t=motion.get(c.id),x=t.x,y=t.y+t.lift,cx=x+41,flip=Math.max(.025,Math.abs(Math.cos(t.flip*Math.PI))),face=t.flip>=.5;scene.ellipse([x+1,y+114,84,13],0x60060d09);scene.pushTransform([flip,0,0,1,cx*(1-flip),0]);
    drawGame2DPanel(scene,[x,y,82,122],{radius:10,fill:face?0xffd8e3e7:0xff364c3b,border:matched.has(i)?0xff99e6ed:i===selected?0xffe7efef:0xff8fa9a9,id:0x66200000+i});
    drawGame2DPanel(scene,[x+5,y+5,72,112],{radius:7,fill:face?0xffe6eef1:0xff29402c,border:face?0x7045726b:0x80779f98,id:0x66200100+i});
    if(face){atlas.draw(scene,70,c.frame,{x:x+41,y:y+88,scale:.58});drawGameText(scene,matched.has(i)?'Paired':c.name,{x:x+10,y:y+102,size:10,maxWidth:64,color:0xff466657});}
    else{atlas.draw(scene,70,'fern',{x:x+41,y:y+98,scale:.55,opacity:1});scene.ellipse([x+13,y+32,56,66],0x807da79b,{filled:false,width:.7});scene.line([x+21,y+93],[x+61,y+93],.7,0xff89aaa4);drawGameText(scene,'GROVE',{x:x+18,y:y+16,size:10,font:'serif',maxWidth:52,color:0xffbbd5d2});}
    if(i===selected)scene.line([x+24,y+128],[x+58,y+128],1.5,0xffbee7e7);scene.popTransform();
   });
  }});const inspect=view.surface.inspectControl;view.surface.inspectControl=o=>({...inspect(o),cards:cards.map((c,i)=>({index:i,faceUp:shown.includes(i)||matched.has(i),matched:matched.has(i),...(shown.includes(i)||matched.has(i)?{frame:c.frame}:{})})),resolving,score,moves,selected});view.setStatus(status());view.animate();
 }});
}});
`;
 return null;
}
