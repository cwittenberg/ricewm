import {exact,failure} from './contract-values.mjs';
import {validNotesText} from './notes-contract.mjs';
import {validTextStyle} from './authoring-contract.mjs';

export const COMPOSE_LAYOUTS=['hero','split','cards','diagram','comparison','closing'];
const text=(v,max)=>typeof v==='string'&&v.trim().length>0&&v.length<=max&&!/[\x00-\x08\x0b-\x1f\x7f]/.test(v);
const color=v=>typeof v==='string'&&/^#[0-9a-f]{6}$/i.test(v);
export function checkedComposition(data){
 const fail=()=>{throw failure('INVALID_REQUEST','Compose requires 1–12 slides with title, 1–4 short points, notes, optional layout and imageTopic (an English Wikipedia article title).');};
 if(!exact(data,['slides','accent'],['slides'])||!Array.isArray(data.slides)||data.slides.length<1||data.slides.length>12||data.accent!==undefined&&!color(data.accent))fail();
 for(const s of data.slides){
  if(!exact(s,['title','points','notes','layout','imageTopic'],['title','points','notes'])||!text(s.title,110)||!Array.isArray(s.points)||!s.points.length||s.points.length>4||!s.points.every(p=>text(p,220))||(!text(s.notes,2000)||!validNotesText(s.notes))||s.layout!==undefined&&!COMPOSE_LAYOUTS.includes(s.layout)||s.imageTopic!==undefined&&(!text(s.imageTopic,120)||/[\/<>]/.test(s.imageTopic)))fail();
 }
 return structuredClone(data);
}

/** Native authoring orchestration, not an alternate document model. All IDs
 * come from checked engine replies; local batches keep the actor operation lock.
 * Existing work is preserved. Only the untouched initial placeholder is reused.
 */
export async function composeDeck(port,spec,{expectedRevision,onDraft=async()=>{},onProgress=()=>{}}={}){
 const data=checkedComposition(spec),start=Date.now(),outline=await port.outline({startSlide:0,limit:64},expectedRevision);
 if(outline.revision!==expectedRevision)throw failure('REVISION_MISMATCH','Presentation changed before composition');
 let last=outline.result.slides.at(-1)?.id||null;
 if(outline.result.nextSlide!==null){const end=await port.outline({startSlide:outline.result.slideCount-1,limit:1},expectedRevision);last=end.result.slides[0].id;}
 let first=null,size={width:960,height:540},emptyObjects=[];
 if(outline.result.slideCount){const scene=(await port.scene({slide:0,startObject:0,limit:64},expectedRevision)).result.scene;size=scene.size;
  if(outline.result.slideCount===1&&scene.totalObjects===scene.objects.length&&scene.objects.every(o=>o.placeholder&&!o.text?.text&&!o.image)&&!(await port.speakerNotes({slide:scene.slideId},expectedRevision)).result.text){first=scene.slideId;emptyObjects=scene.objects.map(o=>o.id);}
 }
 const ids=[];if(first)ids.push(first);
 const creations=Array.from({length:data.slides.length-ids.length},()=>({type:'insert-slide',after:last,layout:'blank'}));
 if(creations.length)ids.push(...(await port.apply(creations)).result.createdSlides);
 const ordered=ids.map(slide=>({type:'move-slide',slide,before:null}));
 if(first)ordered.unshift(...emptyObjects.map(object=>({type:'delete-object',slide:first,object})));
 await port.apply(ordered);
 const sx=size.width/960,sy=size.height/540,accent=data.accent||'#E7AA52',dark='#112D32',ink='#19383E',paper='#F5F2E9';
 const media=Promise.all(data.slides.map(s=>s.imageTopic&&port.findPicture?port.findPicture(s.imageTopic).catch(error=>({error:String(error.message).slice(0,160)})):null));
 const scenes=[];
 for(const [i,s] of data.slides.entries()){
  const slide=ids[i],layout=s.layout||COMPOSE_LAYOUTS[i%COMPOSE_LAYOUTS.length],night=['hero','closing'].includes(layout),fg=night?'#FFFFFF':ink,bg=night?dark:paper;
  const inserts=[],styles=[];
  const box=(x,y,width,height)=>({x:x*sx,y:y*sy,width:width*sx,height:height*sy});
  const shape=(kind,x,y,w,h,fill)=>{inserts.push({type:'insert-object',slide,kind,box:box(x,y,w,h),fill,stroke:null});styles.push(null);};
  const label=(value,x,y,w,h,fontSize=26,bold=false,clr=fg)=>{const style={fontFamily:'Manrope',fontSize:fontSize*sy,bold,color:clr,align:'left'};if(!validTextStyle(style))throw failure('INVALID_REQUEST','Slide dimensions are outside typography limits');inserts.push({type:'insert-object',slide,kind:'text',box:box(x,y,w,h),text:value,fill:null,stroke:null});styles.push(style);};
  shape('rect',48,38,50,5,accent);label(String(i+1).padStart(2,'0'),850,30,64,30,14,false,night?accent:ink);
  label(s.title,48,68,layout==='hero'||layout==='split'?460:850,layout==='hero'?140:90,layout==='hero'?48:36,true);
  if(layout==='cards')s.points.forEach((p,n)=>{const w=820/s.points.length;shape('rect',48+n*w,190,w-16,s.imageTopic?155:240,'#FFFFFF');label(String(n+1).padStart(2,'0'),65+n*w,205,w-44,48,36,true,accent);label(p,65+n*w,267,w-48,s.imageTopic?70:145,s.imageTopic?18:22);});
  else if(layout==='diagram')s.points.forEach((p,n)=>{const w=830/s.points.length;if(n)shape('rect',36+n*w,250,40,4,accent);shape('ellipse',50+n*w,190,65,65,accent);label(String(n+1),67+n*w,200,35,35,26,true,dark);label(p,48+n*w,283,w-34,s.imageTopic?70:155,s.imageTopic?20:22);});
  else if(layout==='comparison'){s.points.forEach((p,n)=>{const x=48+n%2*440,y=180+Math.floor(n/2)*(s.imageTopic?88:135);shape('rect',x,y,416,s.imageTopic?76:116,n%2?'#DFE8E4':'#FFFFFF');shape('rect',x,y,5,s.imageTopic?76:116,accent);label(p,x+20,y+12,374,s.imageTopic?62:80,s.imageTopic?20:23);});}
  else if(layout==='closing'){label(s.points.join('\n\n'),48,190,s.imageTopic?630:710,285,s.imageTopic?26:30,true);shape('ellipse',810,340,78,78,accent);}
  else {s.points.forEach((p,n)=>{shape('ellipse',52,230+n*66,8,8,accent);label(p,77,213+n*66,layout==='hero'||layout==='split'?418:780,61,24);});shape('rect',535,68,377,410,night?'#21464A':'#DFE8E4');label(s.imageTopic||'KEY IDEAS',565,225,320,180,32,true,night?'#FFFFFF':ink);}
  const base=[{type:'set-slide-background',slide,color:bg},{type:'set-slide-transition',slide,effect:i%3===0?'fade':'push',direction:'left',durationMs:450,advanceOnClick:true,advanceAfterMs:null},{type:'set-speaker-notes',slide,text:s.notes}];
  const objects=[];
  for(let n=0;n<inserts.length;n+=24){const r=await port.apply([...(n===0?base:[]),...inserts.slice(n,n+24)]);objects.push(...r.result.created);}
  // The engine returns per-object {slide,object} identities in insertion order.
  const commands=styles.flatMap((style,n)=>style?[{type:'set-text-style',slide,object:typeof objects[n]==='string'?objects[n]:objects[n].object,style},{type:'set-text-layout',slide,object:typeof objects[n]==='string'?objects[n]:objects[n].object,layout:{left:0,top:0,right:0,bottom:0,vertical:'top',wrap:true}}]:[]);
  for(let n=0;n<commands.length;n+=32)await port.apply(commands.slice(n,n+32));
  scenes.push({slide,index:outline.result.slideCount-(first?1:0)+i,title:s.title,layout});onProgress({phase:'slides',completed:i+1,total:ids.length});
 }
 const nativeDraftMs=Date.now()-start;await onDraft({slides:scenes,draftMs:nativeDraftMs});
 const draftAt=Date.now(),draftMs=draftAt-start;
 const pictures=await media,warnings=[];let pictureCount=0;
 for(const [i,p] of pictures.entries()){
  if(!p)continue;if(p.error){warnings.push({slide:ids[i],message:p.error});continue;}
  let prepared;try{prepared=await port.preparePicture(p.bytes);}catch(error){if(error.rejectedBeforeMutation!==true)throw error;warnings.push({slide:ids[i],message:String(error.message).slice(0,160)});continue;}
  const layout=scenes[i].layout;
  const frame=['hero','split'].includes(layout)?{x:535*sx,y:68*sy,width:377*sx,height:410*sy}:{x:715*sx,y:362*sy,width:180*sx,height:122*sy};
  const inserted=await port.apply([{type:'insert-image',slide:ids[i],asset:prepared.asset,box:frame}]);
  const ratio=prepared.width/prepared.height,target=frame.width/frame.height;
  const crop=ratio>target?{left:(1-target/ratio)/2,right:(1-target/ratio)/2,top:0,bottom:0}:{left:0,right:0,top:(1-ratio/target)/2,bottom:(1-ratio/target)/2};
  await port.apply([{type:'set-image-crop',slide:ids[i],object:inserted.result.created[0].object,crop}]);pictureCount++;
  const source=p.source;await port.apply([{type:'set-speaker-notes',slide:ids[i],text:data.slides[i].notes+'\n\nPicture source: '+source}]);
 }
 return {slides:scenes,pictureCount,warnings,draftAt,completedAt:Date.now(),draftMs,totalMs:Date.now()-start};
}
