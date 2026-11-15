import { GameSpriteAtlas } from './game-sprites.mjs';
import { drawGameText, measureGameText } from './game-text.mjs';
export { drawGameText, measureGameText } from './game-text.mjs';

// Original 5x7 pixel lettering. A tiny shared atlas keeps HUD text independent
// of desktop theme fonts or a font/engine download. Supply drawText for other art.
const glyphs = {
  A:[14,17,17,31,17,17,17],B:[30,17,17,30,17,17,30],C:[14,17,16,16,16,17,14],D:[30,17,17,17,17,17,30],
  E:[31,16,16,30,16,16,31],F:[31,16,16,30,16,16,16],G:[14,17,16,23,17,17,15],H:[17,17,17,31,17,17,17],
  I:[31,4,4,4,4,4,31],J:[7,2,2,2,18,18,12],K:[17,18,20,24,20,18,17],L:[16,16,16,16,16,16,31],
  M:[17,27,21,21,17,17,17],N:[17,25,21,19,17,17,17],O:[14,17,17,17,17,17,14],P:[30,17,17,30,16,16,16],
  Q:[14,17,17,17,21,18,13],R:[30,17,17,30,20,18,17],S:[15,16,16,14,1,1,30],T:[31,4,4,4,4,4,4],
  U:[17,17,17,17,17,17,14],V:[17,17,17,17,17,10,4],W:[17,17,17,21,21,21,10],X:[17,17,10,4,10,17,17],
  Y:[17,17,10,4,4,4,4],Z:[31,1,2,4,8,16,31],0:[14,17,19,21,25,17,14],1:[4,12,4,4,4,4,14],
  2:[14,17,1,2,4,8,31],3:[30,1,1,14,1,1,30],4:[2,6,10,18,31,2,2],5:[31,16,16,30,1,1,30],
  6:[14,16,16,30,17,17,14],7:[31,1,2,4,8,8,8],8:[14,17,17,14,17,17,14],9:[14,17,17,15,1,1,14],
  ' ':[0,0,0,0,0,0,0],'.':[0,0,0,0,0,12,12],',':[0,0,0,0,4,4,8],':':[0,4,4,0,4,4,0],
  '!':[4,4,4,4,4,0,4],'?':[14,17,1,2,4,0,4],'-':[0,0,0,31,0,0,0],'+':[0,4,4,31,4,4,0],
  '/':[1,2,2,4,8,8,16],'(':[2,4,8,8,8,4,2],')':[8,4,2,2,2,4,8],"'":[4,4,8,0,0,0,0],
  '[':[14,8,8,8,8,8,14],']':[14,2,2,2,2,2,14],'=':[0,0,31,0,31,0,0],'%':[17,2,4,8,17,0,0],
  '<':[1,2,4,8,4,2,1],'>':[16,8,4,2,4,8,16],'·':[0,0,0,4,0,0,0],
  '^':[4,10,17,0,0,0,0],
};
let font;
function fontAtlas() {
  if (font) return font;
  const characters = Object.keys(glyphs), width = characters.length * 6, rgba = new Uint8Array(width * 8 * 4), frames = {};
  characters.forEach((char, i) => {
    frames[char] = { x:i*6, y:0, width:5, height:7, pivotX:0, pivotY:0 };
    glyphs[char].forEach((row,y) => { for(let x=0;x<5;x++) if(row & (16>>x)) rgba.set([255,255,255,255], (y*width+i*6+x)*4); });
  });
  return font = new GameSpriteAtlas({width,height:8,rgba,frames});
}
/** Small crisp scene text. Use a distinct image ID if the scene owns that ID. */
export function drawGamePixelText(scene, text, { x=0, y=0, scale=1.5, maxWidth=620, color=0xfff4f2df, imageId=0x7ffffffe } = {}) {
  if (typeof text !== 'string' || text.length > 2048 || ![x,y,scale,maxWidth].every(Number.isFinite) || scale <= 0 || scale > 16 || maxWidth <= 0)
    throw RangeError('Invalid bounded game text');
  // Color tint is drawn as vector runs; the white atlas is used for white text.
  // Other colors use the same tiny glyph masks without retaining tinted atlases.
  const atlas = fontAtlas(), limit = Math.max(1,Math.floor(maxWidth/(6*scale))),lines=[];
  for(const paragraph of text.toUpperCase().split('\n')){
    let line='';
    for(let word of paragraph.split(/\s+/).filter(Boolean)){
      if(line&&line.length+word.length+1>limit){lines.push(line);line='';}
      while(word.length>limit){lines.push(word.slice(0,limit));word=word.slice(limit);}
      if(word)line+=(line?' ':'')+word;
    }
    lines.push(line);
  }
  lines.forEach((line,row)=>Array.from(line).forEach((raw,column)=>{
    const char = glyphs[raw] ? raw : '?', px=x+column*6*scale, py=y+row*9*scale;
    if(color===0xffffffff) atlas.draw(scene,imageId,char,{x:px,y:py,scale});
    else glyphs[char].forEach((bits,gy)=>{let start=-1;for(let gx=0;gx<=5;gx++){const filled=gx<5&&(bits&(16>>gx));if(filled&&start<0)start=gx;if(!filled&&start>=0){scene.rect([px+start*scale,py+gy*scale,(gx-start)*scale,scale],color);start=-1;}}});
  }));
  return {width:Math.max(...lines.map(line=>line.length))*6*scale,height:lines.length*9*scale};
}
drawGamePixelText.controlFactory = Object.freeze({topics:['games'],constructor:'drawGamePixelText(scene,"SCORE 120",{x:16,y:16,scale:2,color:0xffffffff,maxWidth:400})',methods:'Original 5x7 scene pixel font, automatic uppercase and bounded wrapping. No font download. Returns {width,height}. Supply GameHUD drawText for localized fonts or custom atlas lettering; metadata retains original text. White atlas uses imageId 0x7ffffffe by default; choose a distinct ID for other artwork.'});

const inside = (rect,x,y) => x>=rect[0]&&y>=rect[1]&&x<rect[0]+rect[2]&&y<rect[1]+rect[3];
const hits = (item,x,y) => inside(item.rect,x,y)&&(item.shape!=='circle'||((x-item.rect[0])/item.rect[2]*2-1)**2+((y-item.rect[1])/item.rect[3]*2-1)**2<=1);
const value = item => typeof item==='function'?item():item;
const defaults = Object.freeze({panel:0xe6282116,face:0xe63e3021,hover:0xf2614930,pressed:0xff493118,accent:0xff7bddb6,text:0xffffffff,muted:0xffb7b3a5,disabled:0xff59534a});

/** Scene-painted HUD with real pointer hit regions and keyboard focus. */
export class GameHUD {
  static controlFactory = Object.freeze({
    topics:['games'],constructor:'new GameHUD({theme:{accent:0xff7bddb6},drawText:drawGameText})',
    methods:'set(id,{type:"button"|"label"|"meter"|"panel",rect:[x,y,w,h],text,visible:true,enabled:true,hitTest:true,shape:"cut"|"circle",value:0.5,activate,onPress,onRelease,draw:(scene,state)=>{}}), remove(id), draw(scene), pointer({kind,x,y,button:1}), key({keyCode,pressed,modifiers}), clearInput(), inspect(). Custom draw receives hovered/pressed/focused/disabled, rect, text, value and theme: render sprites, icons or vectors in the same scene. Buttons capture locally, cancel on leaving their bounds and activate on release inside. Tab/Shift+Tab, arrows, Enter/Space navigate/activate; Escape exits HUD focus. Disabled/hidden targets cannot activate. Meters clamp 0..1; optional fill is the native color used for health/mana/other resource bars. hitTest:false makes decorative items such as crosshairs pass world pointer input through. Bounded to 128 elements. All geometry uses game logical coordinates.',
  });
  constructor({theme={},drawText=drawGameText}={}) {
    if(typeof drawText!=='function')throw TypeError('HUD needs a scene text renderer');
    this.theme={...defaults,...theme};this.drawText=drawText;this.items=new Map();this.hover=null;this.focus=null;this.capture=null;this.keyDown=new Set();
  }
  set(id,item) {
    if(item?.hitTest!==undefined&&typeof item.hitTest!=='boolean')throw TypeError('HUD hitTest must be boolean');
    if(!/^[A-Za-z][A-Za-z0-9_.:-]{0,63}$/.test(id)||!item||!['button','label','meter','panel'].includes(item.type)||!Array.isArray(item.rect)||item.rect.length!==4||!item.rect.every(Number.isFinite)||item.rect.slice(2).some(v=>v<=0||v>16384)||(!this.items.has(id)&&this.items.size>=128))throw RangeError('Invalid bounded HUD element');
    if(this.capture===id&&(value(item.visible)===false||value(item.enabled)===false))this.cancel();
    if(this.focus===id&&(value(item.visible)===false||value(item.enabled)===false))this.focus=null;
    this.items.set(id,{...item,rect:[...item.rect]});return this;
  }
  remove(id){if(this.capture===id)this.cancel();this.items.delete(id);if(this.focus===id)this.focus=null;return this;}
  enabled(id){const item=this.items.get(id);return !!item&&value(item.visible)!==false&&value(item.enabled)!==false&&item.type==='button';}
  hit(x,y){return [...this.items].reverse().find(([,item])=>value(item.visible)!==false&&item.hitTest!==false&&hits(item,x,y))?.[0]??null;}
  cancel(){const id=this.capture;this.capture=null;if(id)this.items.get(id)?.onRelease?.();}
  clearInput(){this.cancel();this.hover=null;this.focus=null;this.keyDown.clear();}
  pointer({kind,x,y,button=1}) {
    const hit=this.hit(x,y), target=hit&&this.items.get(hit);
    if(kind==='cancel'||kind==='leave'){const handled=!!this.capture;this.clearInput();return handled;}
    if(kind==='move'){
      this.hover=hit;
      if(this.capture&&!hits(this.items.get(this.capture),x,y))this.cancel();
      return !!target||!!this.capture;
    }
    if(button!==1)return !!target;
    if(kind==='down'){
      this.focus=null;this.cancel();
      if(this.enabled(hit)){this.capture=hit;target.onPress?.();}
      return !!target;
    }
    if(kind==='up'){
      const captured=this.capture;this.cancel();
      if(captured===hit&&this.enabled(hit))target.activate?.('pointer');
      return !!captured||!!target;
    }
    return false;
  }
  key({keyCode,pressed,modifiers=0}) {
    if(!pressed){this.keyDown.delete(keyCode);return !!this.focus;}
    if(modifiers&(4|8|64))return false;
    const repeat=this.keyDown.has(keyCode);this.keyDown.add(keyCode);
    const ids=[...this.items.keys()].filter(id=>this.enabled(id));
    if(keyCode===23||this.focus&&[111,113,114,116].includes(keyCode)){
      if(!ids.length)return false;
      if(!repeat){const backwards=keyCode===23?!!(modifiers&1):[111,113].includes(keyCode);const current=ids.indexOf(this.focus);this.focus=ids[(current+(backwards?-1:1)+ids.length)%ids.length];}
      return true;
    }
    if(!this.enabled(this.focus)){this.focus=null;return false;}
    if(keyCode===9){this.focus=null;return true;}
    if([36,65,104].includes(keyCode)){if(!repeat)this.items.get(this.focus).activate?.('keyboard');return true;}
    return false;
  }
  inspect(){return [...this.items].filter(([,i])=>value(i.visible)!==false).map(([id,i])=>({id,text:String(value(i.text)||''),bounds:{x:i.rect[0],y:i.rect[1],width:i.rect[2],height:i.rect[3]},visible:true,enabled:value(i.enabled)!==false,selected:this.focus===id,pressed:this.capture===id,actions:i.type==='button'&&value(i.enabled)!==false?['activate']:[]}));}
  draw(scene) {
    let artPath=0x60000000;
    for(const [id,item]of this.items){
      if(value(item.visible)===false)continue;
      const rect=item.rect,text=String(value(item.text)||'').slice(0,2048),disabled=value(item.enabled)===false,pressed=this.capture===id||!!value(item.pressed),focused=this.focus===id,hovered=this.hover===id,theme=this.theme;
      const state={id,rect,text,disabled,pressed,focused,hovered,value:Math.max(0,Math.min(1,Number(value(item.value))||0)),theme};
      if(item.draw){item.draw(scene,state);continue;}
      const [x,y,w,h]=rect;
      if(item.type!=='label'){
        const color=disabled?theme.disabled:pressed?theme.pressed:hovered?theme.hover:item.type==='button'?theme.face:theme.panel;
        if(item.shape==='circle')scene.ellipse(rect,color).ellipse([x+2,y+2,w-4,h-4],focused?theme.accent:theme.muted,{filled:false,width:focused?2:1});
        else{
          const radius=Math.min(5,w/4,h/4),path=artPath++;
          scene.definePath(path,[['M',x+radius,y],['L',x+w-radius,y],['Q',x+w,y,x+w,y+radius],['L',x+w,y+h-radius],['Q',x+w,y+h,x+w-radius,y+h],['L',x+radius,y+h],['Q',x,y+h,x,y+h-radius],['L',x,y+radius],['Q',x,y,x+radius,y],['Z']]).fillPath(path,color).strokePath(path,focused?1.6:0.7,focused?theme.accent:hovered?theme.muted:0x707b7984);
          scene.line([x+radius,y+1],[x+w-radius,y+1],0.6,0x308f9fae);
        }
        if(item.type==='meter')scene.rect([x+4,y+4,Math.max(0,(w-8)*state.value),h-8],item.fill??theme.accent);
      }
      if(text){const scale=item.scale??1.25,center=item.type==='button',metrics=this.drawText===drawGameText?measureGameText(text,{scale,maxWidth:w-16}):{width:text.length*6*scale,height:9*scale};this.drawText(scene,text,{x:center?x+Math.max(8,(w-metrics.width)/2):x+8,y:center?y+Math.max(3,(h-metrics.height)/2):y+7,scale,maxWidth:w-16,color:disabled?theme.muted:theme.text});}
    }
  }
}
