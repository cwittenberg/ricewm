import { GAME_FONT } from './game-font-data.mjs';
import { GAME_CONDENSED_FONT } from './game-condensed-data.mjs';
import { GAME_SERIF_FONT } from './game-serif-data.mjs';
import { SceneBuilder } from './scene-builder.mjs';
const resources=new WeakMap();
const outlines=new WeakMap();
function defineGlyph(scene,face,id,path){
  if(!(scene instanceof SceneBuilder)){scene.definePath(id,path);return;}
  let definitions=outlines.get(face);if(!definitions){definitions=new Map();outlines.set(face,definitions);}
  let bytes=definitions.get(id);
  if(!bytes){
    bytes=new SceneBuilder(1,1).definePath(id,path).finish().displayList;
    // Unsupported Unicode characters also have distinct IDs. Keep arbitrary
    // game/user text from growing this outline cache without a bound.
    if(definitions.size>=256)definitions.delete(definitions.keys().next().value);
    definitions.set(id,bytes);
  }
  scene.append(bytes);
}
const glyph=(character,font=GAME_FONT)=>font.glyphs[character]??font.glyphs['?'];
function layout(text,{scale=1.5,maxWidth=620,size=scale*9,font='body'}={}){
  if(typeof text!=='string'||text.length>2048||![scale,maxWidth,size].every(Number.isFinite)||scale<=0||scale>16||size<=0||size>256||maxWidth<=0)throw RangeError('Invalid bounded game typography');
  if(!['body','serif','condensed'].includes(font))throw RangeError('Unknown game font');
  const face=font==='serif'?GAME_SERIF_FONT:font==='condensed'?GAME_CONDENSED_FONT:GAME_FONT;
  const unit=size/face.units,lineHeight=size*1.35,lines=[];
  const measure=value=>Array.from(value).reduce((width,char)=>width+glyph(char,face).advance*unit,0);
  for(const paragraph of text.split('\n')){
    let line='';
    for(const word of paragraph.split(/\s+/).filter(Boolean)){
      const combined=line+word;
      if(line&&measure(combined)>maxWidth){lines.push(line.trimEnd());line='';}
      for(const char of word){if(line&&measure(line+char)>maxWidth){lines.push(line.trimEnd());line='';}line+=char;}
      line+=' ';
    }
    lines.push(line.trimEnd());
  }
  return {face,font,lines,unit,size,lineHeight,width:Math.max(0,...lines.map(measure)),height:lines.length*lineHeight};
}
export function measureGameText(text,options){const {width,height}=layout(text,options);return {width,height};}
measureGameText.controlFactory=Object.freeze({topics:['games'],constructor:'measureGameText("Health 80",{size:13,maxWidth:200})',methods:'Measure the same retained Inter outline lettering used by drawGameText. No font download, compute permission or pixel font. Covers Latin ASCII plus common punctuation; use the existing typography engine with GameHUD.drawText for complex scripts.'});
export function drawGameText(scene,text,{x=0,y=0,color=0xfff4f2df,...options}={}){
  if(![x,y].every(Number.isFinite))throw RangeError('Invalid text position');
  const result=layout(text,options);let defined=resources.get(scene);if(!defined){defined=new Set();resources.set(scene,defined);}
  result.lines.forEach((line,row)=>{let cursor=x;
    for(const char of line){const outline=glyph(char,result.face),id=(result.font==='serif'?0x51000000:result.font==='condensed'?0x52000000:0x50000000)+char.codePointAt(0);
      if(outline.path.length){if(!defined.has(id)){defineGlyph(scene,result.face,id,outline.path);defined.add(id);}scene.fillPath(id,color,{transform:[result.unit,0,0,-result.unit,cursor,y+row*result.lineHeight+result.size*0.98]});}
      cursor+=outline.advance*result.unit;
    }
  });
  return {width:result.width,height:result.height};
}
drawGameText.controlFactory=Object.freeze({topics:['games'],constructor:'drawGameText(scene,"Health 80",{x:16,y:16,size:14,color:0xffffffff,maxWidth:400})',methods:'Smooth scalable native Inter font outlines, case preserved, proportional spacing and bounded wrapping. Native curves stay sharp at window scaling; no raster glyphs, network/compute permission or font download. Returns {width,height}; measureGameText uses identical layout. scale remains compatible (size=scale*9), or supply size directly. Retains the SDK font license. font:condensed uses licensed Rajdhani Medium instrument lettering; font:body uses Inter; font:serif uses licensed Playfair Display outlines for narrative headings. Latin ASCII/common punctuation; use an existing typography-engine drawText callback for complex scripts. drawGamePixelText is an explicit retro alternative.'});
