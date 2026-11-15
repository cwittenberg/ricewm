import {drawGameText} from './game-text.mjs';

export function drawGame2DPanel(scene,rect,{fill=0xf2252017,border=0xffa9c2c8,radius=14,id=0x66110000}={}){
 if(!Array.isArray(rect)||rect.length!==4||!rect.every(Number.isFinite)||rect[2]<=0||rect[3]<=0||!Number.isFinite(radius)||radius<0)throw RangeError('Invalid game panel geometry');
 const [x,y,w,h]=rect,r=Math.min(radius,w/2,h/2);
 scene.definePath(id,[['M',x+r,y],['L',x+w-r,y],['Q',x+w,y,x+w,y+r],['L',x+w,y+h-r],['Q',x+w,y+h,x+w-r,y+h],['L',x+r,y+h],['Q',x,y+h,x,y+h-r],['L',x,y+r],['Q',x,y,x+r,y],['Z']]).fillPath(id,fill).strokePath(id,.8,border);
 return scene;
}
drawGame2DPanel.controlFactory=Object.freeze({topics:['games'],constructor:'drawGame2DPanel(scene,[x,y,width,height],{fill,border,radius:14,id:0x66110000})',methods:'Smooth native rounded path for game-owned card/table/menu composition; no desktop control widget. Use distinct path identities per panel in a frame. Colors and artwork remain game-specific; use GameHUD for actual hit/focus/disabled semantics and GameHUDSkin for textured narrative panels.'});

/** Compact shared sample layout. Game identity/art stays with the app renderer. */
export function layoutGame2DHud(hud,state){
 const {controls,status,help,playing,continuous,ended,instructions,started}=state;
 hud.set('game-status',{type:'label',rect:[16,12,544,26],text:status,scale:1.15,hitTest:false});
 const menu=controls.find(c=>c.name==='help');
 hud.set('help',{type:'button',rect:[594,12,30,30],text:'Menu',shape:'circle',activate:menu.run,draw(scene,{rect,focused,hovered}){const [x,y,w,h]=rect;scene.ellipse(rect,hovered?0xd85b5940:0x8032271d);if(focused)scene.ellipse([x+1,y+1,w-2,h-2],0xffb4d4de,{filled:false,width:1.3});for(let i=0;i<3;i++)scene.line([x+9,y+10+i*5],[x+w-9,y+10+i*5],1,0xffd6e2e7);}});
 const overlay=help||ended||continuous&&!playing&&!started;
 hud.set('game-paused',{type:'label',rect:[280,42,90,22],text:'Paused',visible:continuous&&!playing&&started&&!ended&&!help,scale:1,hitTest:false});
 hud.set('game-shade',{type:'panel',rect:[0,0,640,360],visible:overlay,hitTest:false,draw(scene){scene.rect([0,0,640,360],0x700d1511);}});
 hud.set('game-menu',{type:'panel',rect:[146,66,348,226],visible:overlay,draw(scene){drawGame2DPanel(scene,[146,66,348,226],{id:0x66110010});drawGameText(scene,help?'Field guide':ended?(status.includes('win')?'Trail complete':'Try again'):'Moonwood trail',{x:170,y:85,size:23,font:'serif',maxWidth:300,color:0xffd5e6ec});drawGameText(scene,help?instructions:ended?status:'Follow the woodland trail. Gather acorns, leap over the ravines and outwit the thornlings.',{x:170,y:126,size:12,maxWidth:298,color:0xffb6cdd3});}});
 for(const [i,name]of ['play','restart','step'].entries()){
  const c=controls.find(c=>c.name===name),visible=name==='play'?continuous&&overlay&&!ended:name==='restart'?help||ended:help&&continuous;
  hud.set(name,{type:'button',rect:[170+i*102,230,94,35],visible,enabled:name==='step'?!ended:c.enabled,activate:c.run,text:name==='play'?(help?'Resume':'Begin'):name==='restart'?'Again':'Step',draw(scene,{rect,focused,hovered,pressed,disabled,text}){drawGame2DPanel(scene,rect,{radius:17,fill:disabled?0xa0333029:pressed?0xff695344:hovered?0xff695e43:0xff493e2d,border:focused?0xffe2f0ee:0xffa9c2c8,id:0x66110001+i});drawGameText(scene,text,{x:rect[0]+16,y:rect[1]+10,size:12,maxWidth:rect[2]-24,color:0xffe0edf0});}});
 }
 hud.set('game-back',{type:'button',rect:[268,274,104,25],text:'Return',visible:help,activate:menu.run,shape:'circle',scale:1});
}
layoutGame2DHud.controlFactory=Object.freeze({topics:['games'],constructor:'layoutGame2DHud(hud,state)',methods:'Optional compact sample game HUD for createGameView: small unframed score/status, a round menu affordance, rounded custom-painted menu/intro/result controls, and keyboard instructions only inside Help. Supply your own layout/art for other plots. Gameplay actions use local keys/pointer rather than permanent giant movement-button rows. Menu controls retain real GameHUD hit regions, focus, pressed and disabled feedback.'});
