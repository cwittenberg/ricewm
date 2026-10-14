import {Button,Column,Container,Dialog,Label,Message,Row,SceneBuilder,SceneSurface,ScrollView,State} from '@ricewm/app';
import {CHART_CATEGORIES,checkedChartCatalog,chartCategory} from './chart-types.mjs';
const keyName=d=>d?.key||({9:'Escape',36:'Enter',111:'ArrowUp',113:'ArrowLeft',114:'ArrowRight',116:'ArrowDown'})[d?.keyCode];
const BLUE=0xffc77538,ORANGE=0xff248cec,GRAY=0xff909090;
/** Small illustrative vector thumbnails, never worksheet calculation or a
 * substitute for a chart projection. The qualified engine catalog controls
 * which illustrations are reachable. */
export function chartTypePreview(kind,{selected=false}={}){
 const b=new SceneBuilder(132,80),c=chartCategory(kind),stacked=kind.includes('stacked'),percent=kind.includes('percent'),line=points=>{for(let i=1;i<points.length;i++)b.line(points[i-1],points[i],2,BLUE);};
 if(c!=='radar')b.line([16,8],[16,66],1,GRAY).line([16,66],[124,66],1,GRAY);
 if(['pie','hierarchy'].includes(c)&&kind!=='treemap'){let start=-Math.PI/2;for(const [i,f]of[.42,.33,.25].entries()){const end=start+f*Math.PI*2,path=[];for(let n=0;n<=24;n++){const a=start+(end-start)*n/24;path.push([n?'L':'M',68+Math.cos(a)*29,37+Math.sin(a)*29]);}if(kind==='doughnut'||kind==='sunburst'){for(let n=24;n>=0;n--){const a=start+(end-start)*n/24;path.push(['L',68+Math.cos(a)*14,37+Math.sin(a)*14]);}}else path.push(['L',68,37]);path.push(['Z']);b.definePath(i+1,path).fillPath(i+1,[BLUE,ORANGE,0xff69974f][i]);start=end;}}
 else if(c==='bar'){for(let i=0;i<3;i++){const w=[65,86,47][i];b.rect([18,14+i*17,percent?54:stacked?w*.65:w,11],BLUE);if(stacked)b.rect([18+(percent?54:w*.65),14+i*17,percent?48:w*.35,11],ORANGE);}}
 else if(c==='line'||c==='area'||c==='combo'||kind==='pareto'){const pts=[[20,52],[46,36],[72,45],[98,17],[120,25]];if(c==='area'){b.definePath(1,[['M',20,66],...pts.map(p=>['L',...p]),['L',120,66],['Z']]).fillPath(1,BLUE);if(stacked)b.definePath(2,[['M',20,52],['L',46,36],['L',72,45],['L',98,17],['L',120,25],['L',120,11],['L',98,6],['L',72,30],['L',46,19],['L',20,38],['Z']]).fillPath(2,ORANGE);}else{if(c==='combo'||kind==='pareto')for(let i=0;i<4;i++)b.rect([23+i*25,66-[25,35,20,40][i],14,[25,35,20,40][i]],ORANGE);line(pts);if(stacked)line([[20,38],[46,19],[72,30],[98,6],[120,11]]);}}
 else if(c==='scatter'){for(const[i,p]of[[25,50],[40,33],[59,42],[72,17],[91,30],[116,12]].entries()){const r=kind==='bubble'?[5,8,6,4,9,5][i]:3;b.ellipse([p[0]-r,p[1]-r,2*r,2*r],i%2?ORANGE:BLUE);}}
 else if(c==='radar'){
  const point=(i,r)=>[68+Math.cos(i*Math.PI/3-Math.PI/2)*r,39+Math.sin(i*Math.PI/3-Math.PI/2)*r],polygon=points=>[...points.map((p,i)=>[i?'L':'M',...p]),['Z']];
  for(const [i,r]of[10,20,30].entries())b.definePath(20+i,polygon(Array.from({length:6},(_,j)=>point(j,r)))).strokePath(20+i,1,GRAY);
  for(let i=0;i<6;i++)b.line([68,39],point(i,30),1,GRAY);
  for(const [i,radii]of Array.from([[28,19,26,23,15,21],[16,26,18,14,25,27]].entries()).reverse()){
   const pts=radii.map((r,j)=>point(j,r)),color=i?ORANGE:BLUE,id=30+i;b.definePath(id,polygon(pts));
   if(kind==='filled-radar')b.fillPath(id,color);
   b.strokePath(id,2,color);
   if(kind==='radar')for(const p of pts)b.ellipse([p[0]-2,p[1]-2,4,4],color);
  }
 }
 else if(c==='stock'||kind==='boxplot'){for(let i=0;i<4;i++){const x=32+i*25,top=[18,10,22,14][i],h=[28,34,21,32][i];b.line([x,top-6],[x,top+h+6],1,GRAY);b.rect([x-6,top,12,h],i%2?ORANGE:BLUE);b.line([x-9,top+h/2],[x+9,top+h/2],1,GRAY);}}
 else if(c==='funnel'){for(let i=0;i<4;i++){const w=100-i*20;b.rect([68-w/2,10+i*14,w,11],i%2?ORANGE:BLUE);}}
 else if(kind==='treemap'){b.rect([22,10,48,50],BLUE).rect([73,10,45,28],ORANGE).rect([73,41,22,19],0xff69974f).rect([98,41,20,19],0xffa86f80);}
 else if(c==='surface'){for(let row=0;row<4;row++)for(let col=0;col<4;col++){const x=20+col*22+row*3,y=55-row*9-Math.sin(col)*12;b.definePath(1+row*4+col,[['M',x,y],['L',x+22,y-6],['L',x+25,y-15],['L',x+3,y-9],['Z']]).fillPath(1+row*4+col,[BLUE,ORANGE,0xff69974f,0xffa86f80][row]);}}
 else {for(let i=0;i<4;i++){const h=[27,43,32,51][i],x=24+i*25;if(c==='waterfall')b.rect([x,12+i*9,17,18],i%2?ORANGE:BLUE);else{const total=percent?50:h;b.rect([x,66-total,17,stacked?total*.65:total],BLUE);if(stacked)b.rect([x,66-total*.35,17,total*.35],ORANGE);}}}
 if(selected)b.line([1,1],[131,1],2,BLUE).line([131,1],[131,79],2,BLUE).line([131,79],[1,79],2,BLUE).line([1,79],[1,1],2,BLUE);return b.finish();
}
/** Shared native gallery for insertion and Change Chart Type. Selection remains
 * a local draft until OK. Returning false keeps the dialog open. */
export function createChartTypeDialog({catalog,currentKind,onSelect,onClose}){
 const types=checkedChartCatalog(catalog).filter(row=>row.authoring),categories=CHART_CATEGORIES.filter(c=>types.some(t=>t.category===c.id));let selected=types.find(t=>t.kind===currentKind)||types[0],category=selected?.category,closed=false,busy=false;const tiles=new Map(),categoryButtons=new Map();
 const status=new Label({name:'chart-type-status',text:types.length?'':'No chart types are available.'}),categoryList=new Column({text:'panel:width=116:flush:tight-stack'}),variants=new Column({text:'panel:flush:tight-stack'}),ok=new Button({name:'chart-type-apply',text:'OK',enabled:!!selected}),cancel=new Button({name:'chart-type-cancel',text:'Cancel'});
 function choose(type){if(closed||busy)return;selected=type;for(const [kind,tile]of tiles){tile.button.setProperty('checked',kind===type.kind);tile.surface.submit(chartTypePreview(kind,{selected:kind===type.kind}));}status.text='';}
 function family(id){if(closed||busy)return;category=id;for(const [key,button]of categoryButtons)button.setProperty('checked',key===id);for(const child of [...variants.children])variants.remove(child,{destroy:true});tiles.clear();const members=types.filter(t=>t.category===id);if(selected?.category!==id)selected=members[0];for(let i=0;i<members.length;i+=2){const row=new Row({text:'panel:fixed:flush'});for(const type of members.slice(i,i+2)){const surface=new SceneSurface({name:`chart-type-preview-${type.kind}`,accessibleName:type.label}),button=new Button({name:`chart-type-${type.kind}`,text:type.label,checked:type===selected,state:State.KEY_NAVIGATION}),tile=new Column({text:'panel:fixed:width=156:flush:tight-stack'},[new Container({text:'panel:fixed-height:80:flush'},[surface]),button]);surface.submit(chartTypePreview(type.kind,{selected:type===selected}));const pick=e=>{if(e.detail?.kind==='up'&&e.detail?.button===1){choose(type);button.setProperty('focused',true);}};surface.on(Message.POINTER,pick);button.on(Message.POINTER,pick);button.on(Message.ACTIVATE,()=>choose(type));bindKeys(surface);bindKeys(button);tiles.set(type.kind,{surface,button});row.append(tile);}variants.append(row);} }
 async function apply(){if(closed||busy||!selected)return;busy=true;ok.setProperty('enabled',false);cancel.setProperty('enabled',false);try{if(await onSelect(selected.kind)!==false){busy=false;close();}}catch(error){status.text=String(error?.message||error).slice(0,160);}finally{busy=false;if(!closed){ok.setProperty('enabled',true);cancel.setProperty('enabled',true);}}}
 function close(){if(closed||busy)return;closed=true;onClose();}
 function bindKeys(control){control.on(Message.KEY,e=>{if(!e.detail?.pressed||busy||closed)return;const key=keyName(e.detail);if(key==='Escape'){e.preventDefault();close();}else if(key==='Enter'){e.preventDefault();void apply();}else if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(key)){e.preventDefault();const members=types.filter(t=>t.category===category),i=members.indexOf(selected),delta={ArrowLeft:-1,ArrowRight:1,ArrowUp:-2,ArrowDown:2}[key];if(members.length){choose(members[Math.max(0,Math.min(members.length-1,i+delta))]);tiles.get(selected.kind)?.button.setProperty('focused',true);}}});}
 for(const c of categories){const button=new Button({name:`chart-category-${c.id}`,text:c.label,icon:c.icon,checked:c.id===category,state:State.START_ALIGNED});button.on(Message.ACTIVATE,()=>family(c.id));button.on(Message.KEY,e=>{if(e.detail?.pressed&&keyName(e.detail)==='Escape'){e.preventDefault();close();}});categoryButtons.set(c.id,button);categoryList.append(button);}
 const dialog=new Dialog({name:'chart-type-dialog',text:'Chart Type',modal:true},[new Row({text:'panel:flush'},[new ScrollView({text:'viewport:336|0/0'},[categoryList]),new ScrollView({text:'viewport:336|0/0',state:State.EXPANSIVE},[variants])]),status,new Row({text:'panel:actions'},[cancel,ok])]);
 cancel.on(Message.ACTIVATE,close);ok.on(Message.ACTIVATE,()=>void apply());bindKeys(dialog);dialog.on(Message.DESTROY,()=>{closed=true;});if(category)family(category);return dialog;
}
