import { GameSpriteAtlas } from './game-sprites.mjs';
const bounds = r => Array.isArray(r) && r.length===4 && r.every(Number.isFinite) && r[2]>0 && r[3]>0;
const fraction = n => Math.max(0,Math.min(1,Number(n)||0));

/** Artwork is supplied by the game. This renderer contains no genre, lore,
 * franchise assets or fixed icon collection. GameHUD still owns hit semantics. */
export class GameHUDSkin {
  static controlFactory=Object.freeze({topics:['games'],constructor:'new GameHUDSkin({atlas:new GameSpriteAtlas({width,height,rgba,frames}),imageId:22,colors:{selected:0xff85b4d4}})',methods:'art(scene,frame,rect,{opacity}), panel(scene,frame,rect,{sourceInset:24,border:14}), slot(scene,rect,{selected,rare}), resource(scene,frame,rect,value), spell(scene,frame,rect,{cooldown,disabled,focused}). Skin uses app-owned illustrated atlas artwork: nine-slice textured panels, inset item slots, fluid-vessel depletion and radial spell cooldown veils. Keep GameHUD semantic names, hit rectangles, focus and tooltips around the exact same art. No desktop button chrome, placeholder line icons or persistent shortcut legends for cinematic requests. Supply entirely different generated/sourced atlases, motifs, materials, colors:{slotTop,slotBottom,selected,rare,border,highlight,emptyResource,focus} and layouts for each requested narrative; the SDK contains no fixed Ashfall/Diablo/wizard/epic-fantasy skin. Bounds/alpha and slice geometry are validated. Share native image resources per scene; budget atlas pixels and all scene images together before loading.'});
  constructor({atlas,imageId=22,colors={}}={}) {
    if(!(atlas instanceof GameSpriteAtlas)||!Number.isInteger(imageId)||imageId<1||imageId>0xffffffff)throw RangeError('Skin requires an artwork atlas and image identity');
    const defaults={slotTop:0xf00c0e13,slotBottom:0xd91a1720,selected:0xff85b4d4,rare:0xffa578bd,border:0xff3a4549,highlight:0x809ca7aa,emptyResource:0xeb15110e,focus:0xff9cc8e4};
    if(!colors||typeof colors!=='object'||Object.entries(colors).some(([key,n])=>!Object.hasOwn(defaults,key)||!Number.isInteger(n)||n<0||n>0xffffffff))throw RangeError('Invalid HUD skin colors');
    this.colors={...defaults,...colors};this.atlas=atlas;this.imageId=imageId;this.scenes=new WeakSet();this.paths=new WeakMap();
  }
  frame(scene,name){
    const f=this.atlas.frames.get(name);if(!f)throw Error('Missing HUD artwork: '+name);
    if(!this.scenes.has(scene)){scene.defineImage(this.imageId,this.atlas.width,this.atlas.height,this.atlas.rgba);this.scenes.add(scene);}return f;
  }
  path(scene,points){const id=this.paths.get(scene)??0x62000000;this.paths.set(scene,id+1);scene.definePath(id,points);return id;}
  art(scene,name,rect,{opacity=1}={}){
    if(!bounds(rect)||!Number.isFinite(opacity)||opacity<0||opacity>1)throw RangeError('Invalid HUD art bounds');
    const f=this.frame(scene,name);scene.image(this.imageId,rect,{uv:[f.x/this.atlas.width,f.y/this.atlas.height,(f.x+f.width)/this.atlas.width,(f.y+f.height)/this.atlas.height],opacity,smoothing:true});return scene;
  }
  panel(scene,name,rect,{sourceInset=24,border=14}={}){
    if(!bounds(rect)||!Number.isFinite(sourceInset)||sourceInset<=0||!Number.isFinite(border)||border<=0||border*2>=Math.min(rect[2],rect[3]))throw RangeError('Invalid skin panel');
    const f=this.frame(scene,name);if(sourceInset*2>=Math.min(f.width,f.height))throw RangeError('Panel slices exceed source artwork');
    const [x,y,w,h]=rect,sx=[f.x,f.x+sourceInset,f.x+f.width-sourceInset,f.x+f.width],sy=[f.y,f.y+sourceInset,f.y+f.height-sourceInset,f.y+f.height],dx=[x,x+border,x+w-border,x+w],dy=[y,y+border,y+h-border,y+h];
    for(let r=0;r<3;r++)for(let c=0;c<3;c++)scene.image(this.imageId,[dx[c],dy[r],dx[c+1]-dx[c],dy[r+1]-dy[r]],{uv:[sx[c]/this.atlas.width,sy[r]/this.atlas.height,sx[c+1]/this.atlas.width,sy[r+1]/this.atlas.height],smoothing:true});return scene;
  }
  slot(scene,rect,{selected=false,rare=false}={}){
    if(!bounds(rect))throw RangeError('Invalid item slot');const [x,y,w,h]=rect;
    scene.linearGradient(rect,[x,y],[x+w,y+h],[{offset:0,color:this.colors.slotTop},{offset:1,color:this.colors.slotBottom}]);
    const path=this.path(scene,[['M',x+4,y],['L',x+w-4,y],['L',x+w,y+4],['L',x+w,y+h-4],['L',x+w-4,y+h],['L',x+4,y+h],['L',x,y+h-4],['L',x,y+4],['Z']]);
    scene.strokePath(path,selected?1.5:0.65,selected?this.colors.selected:rare?this.colors.rare:this.colors.border);
    scene.line([x+4,y+1],[x+w-4,y+1],0.65,this.colors.highlight);return scene;
  }
  resource(scene,name,rect,value){
    this.art(scene,name,rect);value=fraction(value);const [x,y,w,h]=rect,inner=[x+w*0.19,y+h*0.19,w*0.62,h*0.62];
    if(value<1)scene.pushClip([inner[0],inner[1],inner[2],inner[3]*(1-value)]).ellipse(inner,this.colors.emptyResource).popClip();return scene;
  }
  spell(scene,name,rect,{cooldown=0,disabled=false,focused=false}={}){
    this.art(scene,name,rect,{opacity:disabled?0.7:1});cooldown=fraction(cooldown);const [x,y,w,h]=rect;
    if(cooldown>0){const points=[['M',x+w/2,y+h/2]];for(let i=0,n=Math.max(1,Math.ceil(cooldown*32));i<=n;i++){const a=-Math.PI/2+Math.PI*2*cooldown*i/n;points.push(['L',x+w/2+Math.cos(a)*w*0.72,y+h/2+Math.sin(a)*h*0.72]);}points.push(['Z']);scene.pushClip(rect).fillPath(this.path(scene,points),0xd9080b10).popClip();}
    if(focused){const p=this.path(scene,[['M',x+2,y+2],['L',x+w-2,y+2],['L',x+w-2,y+h-2],['L',x+2,y+h-2],['Z']]);scene.strokePath(p,1.2,this.colors.focus);}return scene;
  }
}
