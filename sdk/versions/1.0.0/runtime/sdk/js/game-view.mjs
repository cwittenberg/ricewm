import { State } from './control.mjs';
import { SceneSurface, retainedScene } from './scene-surface.mjs';
import { SceneBuilder } from './scene-builder.mjs';
import { Message } from './messages.mjs';
import { AnimationClock } from './animation.mjs';
import { GameStepper } from './games.mjs';
import { GameInput } from './game-input.mjs';
import { GameHUD, drawGameText } from './game-hud.mjs';

// Small discrete games may advance their clock without changing any pixels.
// Keep comparisons strictly bounded; scrolling/large image frames never pay
// for a whole-frame diff on the input turn.
function sameSmallFrame(previous, next) {
  const bytes=next.displayList;
  if(!previous||bytes.length>65536||previous.width!==next.width||previous.height!==next.height||previous.displayList.length!==bytes.length)return false;
  const left=new DataView(previous.displayList.buffer,previous.displayList.byteOffset,bytes.length),right=new DataView(bytes.buffer,bytes.byteOffset,bytes.length);
  for(let i=0;i<bytes.length;i+=4)if(left.getUint32(i,true)!==right.getUint32(i,true))return false;
  return true;
}

/** Default game art, entirely inside the scene. Replace layoutHud or individual
 * item.draw callbacks to use the game's own atlas, icons, meters and palette. */
function layoutGameHud(hud, { controls, status, instructions, help, playing, continuous, ended, input, customCommandKeys }) {
  hud.set('game-status', {type:'panel',rect:[12,12,350,32],text:status,scale:1.15});
  const commands=controls.filter(c=>c.command);
  commands.forEach((control,i)=>hud.set(control.name,{
    type:'button',rect:[384+i*52,12,32,32],text:control.text,scale:1,shape:'circle',
    visible:control.visible,enabled:control.enabled,activate:control.run,
    draw(scene,{rect,pressed,hovered,focused,disabled,theme}){
      const [x,y,w,h]=rect,cx=x+w/2,cy=y+h/2,ink=disabled?theme.disabled:theme.text;
      scene.ellipse(rect,pressed?theme.pressed:hovered?theme.hover:theme.panel).ellipse([x+1,y+1,w-2,h-2],focused?theme.accent:theme.muted,{filled:false,width:focused?3:1});
      if(control.name==='play'&&playing)scene.rect([cx-5,cy-7,3,14],ink).rect([cx+2,cy-7,3,14],ink);
      else if(control.name==='play'||control.name==='step'){
        scene.line([cx-4,cy-7],[cx+5,cy],2,ink).line([cx+5,cy],[cx-4,cy+7],2,ink);
        if(control.name==='step')scene.rect([cx+7,cy-7,2,14],ink);else scene.line([cx-4,cy+7],[cx-4,cy-7],2,ink);
      }else if(control.name==='restart'){
        scene.ellipse([cx-7,cy-7,14,14],ink,{filled:false,width:2}).line([cx+6,cy-7],[cx+7,cy-1],3,theme.accent).line([cx+7,cy-1],[cx+1,cy-3],3,theme.accent);
      }else drawGameText(scene,'?',{x:cx-4,y:cy-6,scale:1.6,color:ink,maxWidth:20});
    },
  }));
  let primary=0;
  for(const control of controls.filter(c=>!c.command)){
    const direction={left:[12,300,36,36],right:[92,300,36,36],up:[52,260,36,36],forward:[52,260,36,36],down:[52,300,36,36],back:[52,300,36,36],strafeLeft:[12,260,36,36],strafeRight:[92,260,36,36]},
      arrows={left:'<',right:'>',up:'^',forward:'^',down:'V',back:'V'},
      rect=direction[control.name]||[532-primary++*108,300,96,36];
    hud.set(control.name,{
      type:'button',rect,text:arrows[control.name]||control.text,shape:direction[control.name]?'circle':'cut',
      enabled:control.enabled,visible:control.visible,
      pressed:()=>input.down(control.name),onPress:control.press,onRelease:control.release,activate:control.keyboard,
    });
  }
  hud.set('game-hints',{type:'label',rect:[148,340,480,16],text:customCommandKeys?'USE THE HUD MENU FOR CONTROLS':ended?'R TO PLAY AGAIN · H HELP':continuous?(playing?'ESC PAUSE · R RESTART · H HELP':'P PLAY · N STEP · R RESTART · H HELP'):'ARROWS/WASD · ENTER/SPACE ACTION · H HELP',scale:1});
  hud.set('game-help',{type:'panel',rect:[140,64,360,204],text:instructions+'\n\nTAB SELECTS HUD · ENTER ACTIVATES\nESC RETURNS TO GAME',visible:help,scale:1.5});
  hud.set('game-result',{type:'panel',rect:[180,110,280,118],text:status,visible:ended,scale:1.5});
  hud.set('game-retry',{type:'button',rect:[244,188,152,30],text:customCommandKeys?'PLAY AGAIN':'R · PLAY AGAIN',visible:ended,activate:controls.find(c=>c.name==='restart').run});
}

/** Native game viewport with app-painted HUD, local held input and owned clock. */
export function createGameView({
  createWindow, instructions = '', render, update = () => {}, reset,
  actions = [], continuous = true, pointer = () => {}, drag = () => {}, hover = () => {},
  hud = new GameHUD(), layoutHud = layoutGameHud, commandKeys = {},
  animate = () => false, inspectState = () => ({}),
}) {
  if (!(hud instanceof GameHUD) || typeof render !== 'function' || typeof reset !== 'function' || typeof layoutHud !== 'function' || typeof animate !== 'function' || typeof hover !== 'function' || typeof inspectState !== 'function' || !Array.isArray(actions) || actions.length>32)
    throw TypeError('Game view needs render, reset and bounded HUD actions');
  if(!commandKeys||typeof commandKeys!=='object'||Array.isArray(commandKeys)||Object.keys(commandKeys).some(name=>!['play','restart','help','step'].includes(name)))throw RangeError('Invalid game command keys');
  const bindings={play:33,restart:27,help:43,step:57,...commandKeys},codes=Object.values(bindings).filter(code=>code!==null);
  if(codes.some(code=>!Number.isInteger(code)||code<1||code>255||code===9)||new Set(codes).size!==codes.length)throw RangeError('Invalid or conflicting game command keys');
  let width=640,height=360,playing=false,closed=false,ended=false,help=false,status='Ready',dragging=null,focused=null,animating=false,started=false;
  let simulationSteps=0,simulationSeconds=0,submittedFrames=0;
  const stepper=new GameStepper(),input=new GameInput(Object.fromEntries(actions.map(a=>[a.name,a.keys??(a.keyCode?[a.keyCode]:[])])));
  const surface=new SceneSurface({name:'game',accessibleName:instructions,backgroundColor:'#0b1018',state:State.ENABLED|State.EXPANSIVE|State.KEY_NAVIGATION,focused:true});
  const window=createWindow({root:surface});
  window.application?.registerGameSurface?.(surface);
  // A running clock already has its next frame scheduled. Invalidating every
  // mouse move would reset delta and freeze simulation while aiming.
  const refresh=()=>{if(!closed&&(!continuous||!playing||clock.reducedMotion)&&!animating)clock.invalidate();};
  const clear=()=>{dragging=null;hud.clearInput();input.clear();hover(null);};
  const pause=()=>{playing=false;clear();stepper.reset();clock.pause();if(animating)animate(0,{reducedMotion:true});animating=false;};
  const play=()=>{if(ended)return;help=false;clear();playing=true;started=true;stepper.reset();clock.play();refresh();};
  const toggle=()=>{if(playing)pause();else play();refresh();};
  const restart=()=>{pause();ended=false;help=false;started=false;reset();refresh();};
  const toggleHelp=()=>{help=!help;if(help)pause();else clear();refresh();};
  const advance=delta=>stepper.advance(delta,step=>{
    simulationSteps++;simulationSeconds+=step;
    const result=update(step,{input,playing,reducedMotion:clock.reducedMotion});input.endFrame();
    if(result===false){pause();ended=true;return false;}
  });
  const step=()=>{
    if(help)return;started=true;
    // Preserve held keys for reduced-motion combinations such as Right+N.
    playing=false;dragging=null;stepper.reset();clock.pause();if(!ended)advance(0.1);refresh();
  };
  function invoke(action){action.run?.();refresh();}
  const controls=[
    {name:'play',command:true,text:()=>playing?'PAUSE':'PLAY',visible:continuous,enabled:()=>!ended,run:toggle},
    {name:'step',command:true,text:'STEP',visible:()=>continuous&&(!playing||clock.reducedMotion),enabled:()=>!ended&&!help,run:step},
    {name:'restart',command:true,text:'RESET',run:restart},
    {name:'help',command:true,text:'?',run:toggleHelp},
    ...actions.map(action=>({
      ...action,
      enabled:()=>!help&&(typeof action.enabled==='function'?action.enabled():action.enabled)!==false,
      run(){if(!help)invoke(action);},
      press(){if(!help&&input.set(action.name,true,'hud'))invoke(action);},
      release(){input.set(action.name,false,'hud');refresh();},
      keyboard(source){if(!help&&source==='keyboard')invoke(action);},
    })),
  ];
  const hudState=()=>({controls,status,instructions,help,playing,continuous,ended,input,started,customCommandKeys:Object.keys(commandKeys).length>0});
  function layout(){layoutHud(hud,hudState());}
  const clock=new AnimationClock({
    window,playing:false,
    draw({delta,reducedMotion}){
      if(playing&&!reducedMotion)advance(delta);
      if(animating)animating=animate(delta,{reducedMotion})===true;
      // SceneSurface natively fits the retained frame and converts pointer
      // coordinates back into that frame. Keep the frame logical size stable:
      // a second resize transform would double-map input during frame changes.
      const scene=new SceneBuilder(640,360).pushClip([0,0,640,360]);
      render(scene,{input,hud,playing,ended,reducedMotion});layout();hud.draw(scene);
      const frame=scene.popClip().finish();
      if(!sameSmallFrame(retainedScene(surface),frame)){surface.submit(frame);submittedFrames++;}
      return (continuous&&playing||animating)&&!reducedMotion;
    },
    onError(error){animating=false;pause();status=error.message;},
  });
  surface.inspectControl=({start=0,limit=128}={})=>{
    layout();const objects=hud.inspect();
    // Read only on inspection, never in the frame loop. Keep user model data
    // explicit and bounded; diagnostics must not become a second save file.
    const state=inspectState(),serialized=JSON.stringify(state);
    if(!state||typeof state!=='object'||Array.isArray(state)||state.then||!serialized||serialized.length>8192)throw TypeError('inspectState must return a synchronous JSON object of at most 8192 characters');
    return {kind:'game',clock:clock.inspect(),simulation:{steps:simulationSteps,seconds:simulationSeconds,submittedFrames},state:JSON.parse(serialized),rendering:surface.gameRenderingStats?.()??null,surface:String(surface.id),width:640,height:360,viewport:{width,height},status,playing,ended,help,animating,total:objects.length,start,selection:hud.focus?[hud.focus]:[],
      input:actions.filter(a=>input.down(a.name)).map(a=>a.name),
      objects:objects.slice(start,start+limit).map(item=>({...item,screenBounds:{...item.bounds}})),
    };
  };
  surface.on(Message.SIZE_CHANGED,event=>{
    const size=event.detail;if(size?.width>0&&size?.height>0){clear();width=Math.min(16384,Math.max(1,Math.round(size.width)));height=Math.min(16384,Math.max(1,Math.round(size.height)));refresh();}
  });
  surface.on(Message.POINTER,event=>{
    const detail=event.detail||{},point={x:detail.x,y:detail.y},kind=detail.kind;
    if(!Number.isFinite(point.x)||!Number.isFinite(point.y)||point.x<0||point.y<0||point.x>=640||point.y>=360){hud.clearInput();dragging=null;hover(null);refresh();return;}
    const consumed=hud.pointer({...detail,...point});
    if(kind==='cancel'||kind==='leave'){dragging=null;hover(null);refresh();return;}
    if(kind==='down'){
      surface.setProperty('focused',true);
      if(!help&&!consumed&&point.x>=0&&point.y>=0&&point.x<640&&point.y<360){dragging=point;pointer({...point,button:detail.button??1,input});}
      else dragging=null;
    }else if(kind==='move'&&dragging){if(!consumed)drag({...point,dx:point.x-dragging.x,dy:point.y-dragging.y,input});dragging=point;}
    else if(kind==='move'&&!help)hover(consumed?null:{...point,input});
    else if(kind==='up')dragging=null;
    refresh();
  });
  const commandCodes=new Set();
  surface.on(Message.KEY,event=>{
    const detail=event.detail||{};
    // Always deliver releases before HUD routing, including keys held while Tab
    // moves into the HUD. This avoids a stuck movement after menu navigation.
    if(!detail.pressed){input.key(detail);hud.key(detail);commandCodes.delete(detail.keyCode);refresh();return;}
    if(detail.modifiers&(4|8|64))return;
    if(hud.key(detail)){input.clear();event.preventDefault();refresh();return;}
    const commands={9:()=>{if(help){help=false;clear();}else if(playing)pause();else if(continuous)play();}};for(const [name,run]of Object.entries({play:toggle,restart,help:toggleHelp,step}))if(bindings[name]!==null)commands[bindings[name]]=run;const command=commands[detail.keyCode];
    if(command){if(!commandCodes.has(detail.keyCode)){commandCodes.add(detail.keyCode);command();}event.preventDefault();refresh();return;}
    if(help){event.preventDefault();return;}
    const names=input.key(detail);
    for(const name of names)invoke(actions.find(a=>a.name===name));
    if(names.length||actions.some(a=>(a.keys??[a.keyCode]).includes(detail.keyCode)))event.preventDefault();
  });
  surface.on(Message.BLUR,()=>{commandCodes.clear();pause();refresh();});
  const stopState=window.onState(state=>{
    const lostFocus=focused===true&&state.focused===false;
    if(typeof state.focused==='boolean')focused=state.focused;
    if(state.visible===false||lostFocus){commandCodes.clear();pause();}
  });
  window.onClose(()=>{closed=true;playing=false;animating=false;clear();commandCodes.clear();stopState();clock.destroy();});
  layout();
  return{surface,window,input,hud,refresh,play,animate(){if(closed||help)return;const was=animating;animating=true;if(!playing&&!was)clock.play();},pause:()=>{pause();refresh();},restart,step,get playing(){return playing;},get status(){return status;},stop(){pause();ended=true;refresh();},setStatus(text){text=String(text).slice(0,2048);if(status!==text){status=text;refresh();}}};
}
createGameView.controlFactory=Object.freeze({
  topics:['games'],
  constructor:'createGameView({createWindow,instructions,render:(scene,{input,hud})=>{},update:(delta,{input})=>{},reset:()=>{},actions:[{name:"left",text:"LEFT",keys:[113,38],run:()=>{}}],continuous:true,inspectState:()=>({}),animate:(delta,{reducedMotion})=>false,hover:point=>{},pointer:({x,y,button,input})=>{},drag:({dx,dy,x,y,input})=>{},hud:new GameHUD(),layoutHud:(hud,state)=>{}})',
  methods:'Uses a host-selected WebGPU-first renderer Strategy through the existing native compositor, with the original CPU renderer only when usable WebGPU is unavailable. Game code keeps the same SceneBuilder, input, HUD and lifecycle API. Consecutive rectangles/sprites are GPU-instanced; immutable textures and path geometry are retained. A GPU-capable adapter is selected once; unsupported shaders or quota errors fail only this owned game, rather than silently switching to CPU. Renderer diagnostics report the actual backend, fallback reason and readbacks. Unchanged frames up to 64 KiB skip native submission while simulation continues. One native SceneSurface fills the window client; all HUD buttons/status/help are game-painted vectors/sprites, with no office button rows or native labels. 640x360 logical coordinates fit proportionally with letterboxing; pointer and HUD use the same transform. Returns {surface,window,input,hud,refresh(),animate(),play(),pause(),restart(),step(),stop(),setStatus(text),playing,status}. Omit layoutHud for the built-in in-scene HUD; override it to place custom GameHUD items, art and meters. update(delta,{input}) reads held actions and combinations, without OS repeat. action.run fires once per press (also HUD); use it for jump/fire/turn moves; held movement belongs in update. Defaults: P play/pause, Escape pause/resume, R restart, N fixed Step, H help. commandKeys:{play:33,restart:null,help:43,step:57} remaps X11 command keys or disables them with null, reserving Escape. For FPS reload on R, disable restart binding and expose restart through the game menu; Tab/arrows/Enter navigate HUD. Start paused. Step preserves held input for reduced-motion play. Clear on blur/hide/pause/restart/close; no global key capture. continuous:false redraws on input and finite requested transitions only. Pass animate:(delta,{reducedMotion})=>boolean and call view.animate() after a discrete change; return true only while finite work remains. Rules commit in the model before presentation. Pause/hide settles requested presentation with reducedMotion:true; reduced motion settles without a loop. Hidden/closed views stop clocks. Pass inspectState:()=>({activeY:model.active.y,score:model.score}) to expose a small synchronous JSON model projection (max 8192 characters) for outcome checks. surface.inspectControl() exposes state, clock lifecycle/ticks/delta/error, simulation steps/seconds/submittedFrames, and current game HUD status and transformed hit regions to sdk.inspect; sdk.interact click {name:"game",object:"restart"} uses those real regions. render/update/animate/hover are synchronous. hover receives a native viewport point or null on leave/HUD occlusion; use actual animated geometry for card lift feedback.',
});
