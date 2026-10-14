export {createChartLegendDialog} from './chart-legend.mjs';
export {createChartTypeDialog} from './chart-type-gallery.mjs';
import {Button,CheckBox,ColorPalette,Column,ContextMenu,Dialog,Label,Menu,MenuItem,Message,Row,SceneSurface,ScrollView,Stack,State,TextField} from '@ricewm/app';
import {chartHandles,checkedChartProjection,createChartTextRenderer,hitChart,layoutChart,renderChartScene} from './chart-scene.mjs';

const clone = value => JSON.parse(JSON.stringify(value));
function checkedBounds(value) {
    if(!value||['x','y','width','height'].some(key=>!Number.isFinite(value[key]))||Math.abs(value.x)>Number.MAX_SAFE_INTEGER||Math.abs(value.y)>Number.MAX_SAFE_INTEGER||value.width<=0||value.height<=0||value.width>42000||value.height>42000)throw new RangeError('Invalid chart bounds');
    return Object.fromEntries(['x','y','width','height'].map(key=>[key,['width','height'].includes(key)?Math.max(1,Math.round(value[key])):Math.round(value[key])]));
}
function checkedViewport(value) {
    if(!value||!Number.isInteger(value.width)||!Number.isInteger(value.height)||value.width<1||value.height<1||value.width>16384||value.height>16384)throw new RangeError('Invalid chart viewport');
    return {width:value.width,height:value.height};
}
const equalBounds=(a,b)=>['x','y','width','height'].every(key=>a[key]===b[key]);
const keyName=detail=>detail?.key||({9:'Escape',36:'Enter',119:'Delete',111:'ArrowUp',113:'ArrowLeft',114:'ArrowRight',116:'ArrowDown'})[detail?.keyCode];

/** A native positioned surface over the worksheet. All drag motion is a local
 * preview. Only a completed gesture sends a geometry command to the workbook.
 * `shapeText` is the shared Office typography engine's bound shape function. */
export function createChartEditor({projection:initialProjection,bounds:initialBounds,viewport:initialViewport,shapeText,theme={},documentZoom=100,clipInsets={},selected:initialSelected=false,onCommit=async()=>false,onSelection=()=>{},onContext=()=>{},onEdit=()=>{},onDelete=()=>{},onError=()=>{}}) {
    const zoom=value=>{if(!Number.isInteger(value)||value<25||value>400)throw new RangeError('Chart zoom must be 25–400 percent');return value;};
    const insets=value=>{const result={left:0,top:0,right:0,bottom:0,...value};if(Object.keys(result).some(key=>!['left','top','right','bottom'].includes(key))||Object.values(result).some(n=>!Number.isInteger(n)||n<0||n>16384))throw new RangeError('Invalid chart viewport insets');return result;};
    documentZoom=zoom(documentZoom);clipInsets=insets(clipInsets);
    let projection=checkedChartProjection(initialProjection),bounds=checkedBounds(initialBounds),viewport=checkedViewport(initialViewport);
    let selected=Boolean(initialSelected),selectedPart=null,gesture=null,closed=false,busy=false,version=0,frames=new Map(),layout,clip;
    const typography=createChartTextRenderer(shapeText),surface=new SceneSurface({name:`chart-${projection.chartId}-surface`,accessibleName:projection.title||'Chart'});
    const control=new Stack({name:`chart-${projection.chartId}`,text:'positioned-item:0:0:1:1'},[surface]);
    function draw() {
        if(closed)return;
        layout=layoutChart(projection,{width:bounds.width,height:bounds.height,documentZoom,theme});
        const x=Math.max(clipInsets.left,bounds.x),y=Math.max(clipInsets.top,bounds.y),right=Math.min(viewport.width-clipInsets.right,bounds.x+bounds.width),bottom=Math.min(viewport.height-clipInsets.bottom,bounds.y+bounds.height);
        if(right<=x||bottom<=y){control.setProperty('visible',false);clip=null;return;}
        control.setProperty('visible',true);clip={x,y,width:right-x,height:bottom-y};
        const px=Math.min(999,Math.round(x/viewport.width*1000)),py=Math.min(999,Math.round(y/viewport.height*1000));
        const pw=Math.max(1,Math.min(1000-px,Math.round(clip.width/viewport.width*1000))),ph=Math.max(1,Math.min(1000-py,Math.round(clip.height/viewport.height*1000)));
        control.text=`positioned-item:${px}:${py}:${pw}:${ph}`;
        // Clip at scene construction so an imported large chart never needs
        // an oversized offscreen texture or loses its original document scale.
        surface.submit(renderChartScene(layout,{textFrames:frames,selected,selectedPart,
            viewportClip:{x:x-bounds.x,y:y-bounds.y,width:clip.width,height:clip.height}}));
    }
    async function shapeAndDraw(ticket=version) {
        const candidate=layoutChart(projection,{width:bounds.width,height:bounds.height,documentZoom,theme});
        const next=await typography.shape(candidate);
        if(closed||ticket!==version)return false;
        frames=next;draw();return true;
    }
    function select(part) {
        selected=true;selectedPart=part;surface.setProperty('focused',true);draw();onSelection({chartId:projection.chartId,part});
    }
    function cancelGesture() {
        if(!gesture)return false;bounds=gesture.before;gesture=null;surface.setProperty('cursor','default');draw();return true;
    }
    async function commit(before,after,operation) {
        if(closed||busy||equalBounds(before,after))return false;
        busy=true;const ticket=version;
        try {
            const accepted=await onCommit({chartId:projection.chartId,chartRevision:projection.chartRevision,dataRevision:projection.dataRevision,operation,before:{...before},after:{...after}});
            if(!closed&&ticket===version){if(accepted===false)bounds=before;await shapeAndDraw(ticket);}
            return accepted!==false;
        } catch(error) {if(!closed&&ticket===version){bounds=before;draw();onError(error);}return false;}
        finally {busy=false;}
    }
    function handleAt(x,y) {
        return selected?chartHandles(bounds.width,bounds.height).find(handle=>Math.abs(handle.x-x)<=7&&Math.abs(handle.y-y)<=7):null;
    }
    function resize(before,edge,dx,dy) {
        let {x,y,width,height}=before;const minimumWidth=160*documentZoom/100,minimumHeight=120*documentZoom/100;
        if(edge.includes('e'))width=Math.max(minimumWidth,Math.min(16384,width+dx));
        if(edge.includes('s'))height=Math.max(minimumHeight,Math.min(16384,height+dy));
        if(edge.includes('w')){const next=Math.max(minimumWidth,Math.min(16384,width-dx));x+=width-next;width=next;}
        if(edge.includes('n')){const next=Math.max(minimumHeight,Math.min(16384,height-dy));y+=height-next;height=next;}
        return {x:Math.round(x),y:Math.round(y),width:Math.round(width),height:Math.round(height)};
    }
    surface.on(Message.POINTER,event=>{
        const detail=event.detail;if(closed||busy||!detail||!clip)return;
        if(detail.kind==='down'&&detail.button===1) {
            // Native positioned surfaces capture in enclosing-canvas permille,
            // which stays stable while the positioned wrapper itself moves.
            const x=detail.x*viewport.width/1000,y=detail.y*viewport.height/1000;
            const localX=x-bounds.x,localY=y-bounds.y,handle=handleAt(localX,localY),part=hitChart(layout,localX,localY)||{kind:'chart'};
            select(part);gesture={before:{...bounds},x,y,edge:handle?.edge||null,moved:false,part};event.preventDefault();
        } else if(detail.kind==='move'&&gesture) {
            const x=detail.x*viewport.width/1000,y=detail.y*viewport.height/1000,dx=x-gesture.x,dy=y-gesture.y;
            if(!gesture.moved&&Math.hypot(dx,dy)<3)return;
            gesture.moved=true;
            bounds=gesture.edge?resize(gesture.before,gesture.edge,dx,dy):{...gesture.before,
                x:Math.round(Math.max(32-gesture.before.width,Math.min(viewport.width-32,gesture.before.x+dx))),
                y:Math.round(Math.max(24-gesture.before.height,Math.min(viewport.height-24,gesture.before.y+dy)))};
            surface.setProperty('cursor',gesture.edge?'crosshair':'default');draw();event.preventDefault();
        } else if(detail.kind==='up'&&detail.button===1&&gesture) {
            const completed=gesture;gesture=null;surface.setProperty('cursor','default');event.preventDefault();
            if(completed.moved)void commit(completed.before,{...bounds},completed.edge?'resize':'move');
            else if(completed.part?.kind==='legend')onEdit({chartId:projection.chartId,part:completed.part});
        } else if(detail.kind==='up'&&detail.button===3) {
            const x=detail.x+clip.x-bounds.x,y=detail.y+clip.y-bounds.y,part=hitChart(layout,x,y)||{kind:'chart'};
            select(part);onContext({chartId:projection.chartId,part,x:detail.x+clip.x,y:detail.y+clip.y});event.preventDefault();
        } else if(detail.kind==='move'&&!gesture) {
            surface.setProperty('cursor',handleAt(detail.x+clip.x-bounds.x,detail.y+clip.y-bounds.y)?'crosshair':'default');
        }
    });
    surface.on(Message.KEY,event=>{
        const detail=event.detail;if(closed||busy||!detail?.pressed)return;const key=keyName(detail);
        if(key==='Escape'){if(!cancelGesture()){selected=false;selectedPart=null;surface.setProperty('focused',false);draw();onSelection({chartId:projection.chartId,part:null});}event.preventDefault();}
        else if(key==='Enter'){onEdit({chartId:projection.chartId,part:selectedPart||{kind:'chart'}});event.preventDefault();}
        else if(key==='Delete'){onDelete({chartId:projection.chartId,chartRevision:projection.chartRevision});event.preventDefault();}
        else if(selected&&['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(key)&&!(detail.modifiers&12)) {
            cancelGesture();const before={...bounds},step=detail.modifiers&1?10:1;
            bounds={...bounds,x:bounds.x+(key==='ArrowLeft'?-step:key==='ArrowRight'?step:0),y:bounds.y+(key==='ArrowUp'?-step:key==='ArrowDown'?step:0)};
            draw();void commit(before,{...bounds},'move');event.preventDefault();
        }
    });
    control.on(Message.DESTROY,()=>{closed=true;version++;gesture=null;typography.clear();});
    draw();const ready=shapeAndDraw().catch(error=>{onError(error);throw error;});
    return {
        control,surface,ready,
        async present(next) {
            if(closed)return false;cancelGesture();const p=next.projection?checkedChartProjection(next.projection):projection;
            if(p.chartId!==projection.chartId)throw new Error('Cannot replace the chart identity of an editor');
            const b=next.bounds?checkedBounds(next.bounds):bounds,v=next.viewport?checkedViewport(next.viewport):viewport;
            documentZoom=next.documentZoom===undefined?documentZoom:zoom(next.documentZoom);if(next.clipInsets)clipInsets=insets(next.clipInsets);projection=p;bounds=b;viewport=v;if(next.theme)theme=next.theme;version++;draw();return shapeAndDraw(version);
        },
        setSelected(value,part=null){selected=Boolean(value);selectedPart=selected?part:null;if(!selected)cancelGesture();surface.setProperty('focused',selected);draw();},
        cancelGesture,
        get bounds(){return {...bounds};},get selectedPart(){return selectedPart?{...selectedPart}:null;},get pending(){return busy;},get summary(){return {hiddenLegendEntries:layout?.hiddenLegendEntries||0,skippedLabels:layout?.skippedLabels||0};},
        destroy(){control.destroy();},
    };
}

/** Native popup rows carry semantic commands; the host owns anchoring,
 * submenu arrows, keyboard traversal and dismissal. Attach to editor.control. */
export function createChartContextMenu({onAction,can=()=>true}) {
    const actions=[['edit-chart','Format Chart…',243],['select-data','Select Data…',244],['chart-type','Change Chart Type…',241],['delete-chart','Delete Chart',25]];
    return new ContextMenu({name:'chart-context-menu'},actions.map(([name,text,icon])=>{
        const item=new MenuItem({name:`chart-context-${name}`,text,icon,enabled:can(name)});item.on(Message.ACTIVATE,()=>{if(can(name))onAction(name);});return item;
    }));
}

/** Edits a detached model draft. OK submits exactly one complete update-chart
 * transaction; Cancel cannot mutate the workbook. Source ranges stay engine-
 * validated and are edited through the caller's shared range-picker UI. */
export function createChartOptionsDialog({chart,onApply,onClose,onSelectData,isCurrent=()=>true,themeColors=[],seriesColors={},legendItems=[],initialTab='chart',initialPart=null}) {
    const draft=clone(chart),radar=['radar','filled-radar'].includes(chart.kind),seriesColorLabel=chart.kind==='filled-radar'?'Fill color':chart.kind==='radar'?'Line color':'Series color';let closed=false,busy=false;
    const status=new Label({name:'chart-options-status',text:''});
    const field=(name,title,value,change)=>{const input=new TextField({name:`chart-option-${name}`,value,accessibleName:title});input.on(Message.VALUE_CHANGED,()=>change(input.getProperty('value')));return new Row({text:'panel:fixed:flush'},[new Label({text:title}),input]);};
    const choice=(name,title,value,options,change)=>{const menu=new Menu({name:`chart-option-${name}`,text:options.find(([key])=>key===value)?.[1]||title,state:State.START_ALIGNED|State.TRAILING_ICON});for(const [key,text]of options){const item=new MenuItem({name:`chart-option-${name}-${key}`,text,checked:key===value});item.on(Message.ACTIVATE,()=>{change(key);menu.text=text;for(const child of menu.children)child.setProperty('checked',child===item);});menu.append(item);}return new Row({text:'panel:fixed:flush'},[new Label({text:title}),menu]);};
    const toggle=(name,title,value,change)=>{const item=new CheckBox({name:`chart-option-${name}`,text:title,checked:value});item.on(Message.ACTIVATE,()=>change(item.getProperty('checked')));return item;};
    const categoryDirection=()=>draft.options?.axes?.find(axis=>axis.id==='x')?.reversed??false;
    const reverseCategories=value=>{draft.options ||= {groups:[],axes:[]};draft.options.axes ||= [];let axis=draft.options.axes.find(axis=>axis.id==='x');if(!axis){axis={id:'x',dimension:'x',position:'bottom',title:'',reversed:false};draft.options.axes.push(axis);}axis.reversed=value;};
    const body=new Column({text:'panel:flush:tight-stack'},[
        field('title','Chart title',draft.title,value=>{draft.title=value;}),
        new Row({text:'panel:fixed:flush'},[new Label({text:'Chart type'}),new Label({name:'chart-option-type',text:draft.kind.split('-').map(word=>word[0].toUpperCase()+word.slice(1)).join(' ')})]),
        ...(radar?[toggle('reverse-categories','Reverse category order',categoryDirection(),reverseCategories)]:[]),
        field('category-title',radar?'Category axis':'Horizontal axis',draft.categoryAxisTitle,value=>{draft.categoryAxisTitle=value;}),
        field('value-title',radar?'Value axis':'Vertical axis',draft.valueAxisTitle,value=>{draft.valueAxisTitle=value;}),
        toggle('legend','Show legend',draft.legend.visible,value=>{draft.legend.visible=value;}),
        choice('legend-position','Legend position',draft.legend.position,[['top','Top'],['right','Right'],['bottom','Bottom'],['left','Left']],value=>{draft.legend.position=value;}),
        toggle('labels','Show data labels',draft.labels.visible,value=>{draft.labels.visible=value;}),
        new Row({text:'panel:fixed:flush'},[
            toggle('label-value','Value',draft.labels.showValue,value=>{draft.labels.showValue=value;}),
            toggle('label-category','Category',draft.labels.showCategory,value=>{draft.labels.showCategory=value;}),
            toggle('label-series','Series',draft.labels.showSeries,value=>{draft.labels.showSeries=value;}),
        ]),
        toggle('visible-only','Plot visible rows only',draft.plotVisibleOnly,value=>{draft.plotVisibleOnly=value;}),
        choice('blanks','Empty cells',draft.displayBlanksAs,[['gap','Gaps'],['zero','Zero'],['span','Connect line points']],value=>{draft.displayBlanksAs=value;}),
    ]);
    const familyFields=[];
    const family=['pie','doughnut','scatter','bubble'].includes(draft.kind)?draft.kind:null;
    const existingGroup=family?draft.options?.groups?.find(group=>group.family===family):null;
    function changeFamily(key,value){
        draft.options ||= {groups:[],axes:[]};draft.options.groups ||= [];
        let group=draft.options.groups.find(group=>group.family===family);
        if(!group){group={id:1,family,stacking:'none',seriesIds:draft.series.map(series=>series.id),xAxis:'x',yAxis:'y',options:{}};draft.options.groups.push(group);}
        group.options ||= {};group.options[key]=value;
    }
    function numericOption(key,title,fallback,min,max){let value=String(existingGroup?.options?.[key]??fallback);body.append(field(key,title,value,next=>{value=next;}));familyFields.push(()=>{if(!/^\d+$/.test(value)||Number(value)<min||Number(value)>max)throw Error(`${title} must be ${min}–${max}.`);if(Number(value)!==(existingGroup?.options?.[key]??fallback))changeFamily(key,Number(value));});}
    if(family==='pie'||family==='doughnut')numericOption('firstSliceAngle','First slice angle (°)',0,0,360);
    if(family==='doughnut')numericOption('holeSize','Hole size (%)',50,10,90);
    if(family==='bubble')numericOption('bubbleScale','Bubble scale (%)',100,1,300);
    if(family==='scatter')body.append(toggle('lines','Connect points with lines',existingGroup?.options?.lines??false,value=>changeFamily('lines',value)));
    const seriesPage=new Column({text:'panel:flush:tight-stack',visible:false});
    const seriesMenu=new Menu({name:'chart-option-series',text:'Series',state:State.START_ALIGNED|State.TRAILING_ICON});
    const seriesStatus=new Label({name:'chart-option-series-color-label',text:seriesColorLabel});
    const seriesPalette=new ColorPalette({name:'chart-option-series-color',value:null,themeColors,allowNone:true,noneLabel:'Automatic',moreColors:true});
    const seriesMarker=new CheckBox({name:'chart-option-series-marker',text:'Show markers',visible:chart.kind!=='filled-radar'});let seriesId=draft.series?.[0]?.id;
    const currentSeries=()=>draft.series?.find(series=>series.id===seriesId);
    function showSeries(){const series=currentSeries();if(!series)return;
        const index=draft.series.indexOf(series);seriesMenu.text=series.name.kind==='literal'?series.name.text||`Series ${index+1}`:`Series ${index+1}`;
        const resolved=typeof series.color==='string'?series.color:seriesColors instanceof Map?seriesColors.get(series.id):seriesColors[series.id];
        seriesPalette.setValue(resolved||null,{mixed:Array.isArray(series.color)&&!resolved});
        seriesStatus.text=Array.isArray(series.color)?`${seriesColorLabel} · Theme ${series.color[0]+1}`:seriesColorLabel;
        seriesMarker.setProperty('checked',series.marker);for(const item of seriesMenu.children)item.setProperty('checked',item.seriesId===seriesId);
    }
    function rebuildSeries(){for(const item of [...seriesMenu.children])seriesMenu.remove(item,{destroy:true});if(!currentSeries())seriesId=draft.series?.[0]?.id;
        for(const [index,series]of (draft.series||[]).entries()){const item=new MenuItem({name:`chart-option-series-${series.id}`,text:series.name.kind==='literal'?series.name.text||`Series ${index+1}`:`Series ${index+1}`});item.seriesId=series.id;item.on(Message.ACTIVATE,()=>{seriesId=series.id;showSeries();});seriesMenu.append(item);}showSeries();
    }
    seriesPalette.on(Message.VALUE_CHANGED,event=>{const series=currentSeries();if(series){series.color=event.detail.value;seriesStatus.text=seriesColorLabel;}});
    seriesMarker.on(Message.ACTIVATE,()=>{const series=currentSeries();if(series)series.marker=seriesMarker.getProperty('checked');});
    if(draft.series?.length){seriesPage.append(seriesMenu);seriesPage.append(seriesStatus);seriesPage.append(seriesPalette);seriesPage.append(seriesMarker);rebuildSeries();}
    if(onSelectData){const data=new Button({name:'chart-option-data',text:'Select Data…'});data.on(Message.ACTIVATE,()=>{if(!closed&&!busy)onSelectData(clone(draft),next=>{if(!closed&&!busy){draft.categories=clone(next.categories);draft.series=clone(next.series);rebuildSeries();}});});seriesPage.append(data);}
    const slices=(family==='pie'||family==='doughnut')?legendItems.filter(item=>Number.isInteger(item.sourceIndex)&&item.sourceIndex>=0&&item.sourceIndex<4096):[];
    const slicePage=new Column({text:'panel:flush:tight-stack',visible:false}),sliceMenu=new Menu({name:'chart-option-slice',text:'Category',state:State.START_ALIGNED|State.TRAILING_ICON});
    const slicePalette=new ColorPalette({name:'chart-option-slice-color',value:null,themeColors,allowNone:true,noneLabel:'Automatic',moreColors:true}),slicePrevious=new Button({name:'chart-option-slice-previous',text:'Previous'}),sliceNext=new Button({name:'chart-option-slice-next',text:'Next'}),sliceStatus=new Label({name:'chart-option-slice-page'});
    let sliceIndex=Math.max(0,slices.findIndex(item=>item.sourceIndex===initialPart?.sourceIndex)),sliceOffset=Math.floor(sliceIndex/32)*32;
    function showSlice(){const item=slices[sliceIndex];if(!item)return;sliceMenu.text=item.text||'(Blank)';const override=draft.options?.groups?.find(group=>group.family===family)?.options?.pointColors?.find(point=>point.sourceIndex===item.sourceIndex);slicePalette.setValue(typeof override?.color==='string'?override.color:override?item.color:null,{mixed:Array.isArray(override?.color)});for(const option of sliceMenu.children)option.setProperty('checked',option.sliceIndex===sliceIndex);}
    function pageSlices(){for(const child of [...sliceMenu.children])sliceMenu.remove(child,{destroy:true});for(let i=sliceOffset;i<Math.min(slices.length,sliceOffset+32);i++){const item=new MenuItem({name:`chart-option-slice-${slices[i].sourceIndex}`,text:slices[i].text||'(Blank)'});item.sliceIndex=i;item.on(Message.ACTIVATE,()=>{sliceIndex=i;showSlice();});sliceMenu.append(item);}sliceStatus.text=`${sliceOffset+1}–${Math.min(slices.length,sliceOffset+32)} of ${slices.length}`;slicePrevious.setProperty('enabled',sliceOffset>0);sliceNext.setProperty('enabled',sliceOffset+32<slices.length);showSlice();}
    slicePrevious.on(Message.ACTIVATE,()=>{sliceOffset=Math.max(0,sliceOffset-32);sliceIndex=sliceOffset;pageSlices();});sliceNext.on(Message.ACTIVATE,()=>{if(sliceOffset+32<slices.length){sliceOffset+=32;sliceIndex=sliceOffset;pageSlices();}});
    slicePalette.on(Message.VALUE_CHANGED,event=>{const item=slices[sliceIndex];if(!item)return;const prior=draft.options?.groups?.find(group=>group.family===family)?.options?.pointColors||[],points=prior.filter(point=>point.sourceIndex!==item.sourceIndex);if(event.detail.value!==null){if(points.length>=32){status.text='At most 32 custom category colors. Reset a color to add another.';showSlice();return;}points.push({sourceIndex:item.sourceIndex,color:event.detail.value});}changeFamily('pointColors',points);status.text='';});
    if(slices.length){slicePage.append(sliceMenu);slicePage.append(new Row({text:'panel:fixed:flush'},[slicePrevious,sliceStatus,sliceNext]));slicePage.append(new Label({text:'Category color'}));slicePage.append(slicePalette);pageSlices();}
    const tabs=new Row({text:'panel:fixed:flush'}),pages=new Stack({text:'panel:flush'},[body,seriesPage,slicePage]);

    const chartTab=new Button({name:'chart-options-tab-chart',text:'Chart'}),seriesTab=new Button({name:'chart-options-tab-series',text:'Series',enabled:Boolean(draft.series?.length)}),sliceTab=new Button({name:'chart-options-tab-slices',text:'Categories'});
    const chooseTab=name=>{const slice=name==='slices'&&Boolean(slices.length),series=!slice&&name==='series'&&Boolean(draft.series?.length);body.setProperty('visible',!series&&!slice);seriesPage.setProperty('visible',series);slicePage.setProperty('visible',slice);chartTab.setProperty('checked',!series&&!slice);seriesTab.setProperty('checked',series);sliceTab.setProperty('checked',slice);};
    chartTab.on(Message.ACTIVATE,()=>chooseTab('chart'));seriesTab.on(Message.ACTIVATE,()=>chooseTab('series'));tabs.append(chartTab);tabs.append(seriesTab);if(slices.length){tabs.append(sliceTab);sliceTab.on(Message.ACTIVATE,()=>chooseTab('slices'));}chooseTab(initialTab);
    const cancel=new Button({name:'chart-options-cancel',text:'Cancel'}),ok=new Button({name:'chart-options-apply',text:'OK'});
    const dialog=new Dialog({name:'chart-options-dialog',text:'Format Chart',modal:true},[
        tabs,new ScrollView({text:'viewport:340|0/0',state:State.EXPANSIVE},[pages]),status,new Row({text:'panel:actions'},[cancel,ok]),
    ]);
    const close=()=>{if(closed||busy)return;closed=true;onClose();};
    cancel.on(Message.ACTIVATE,close);
    ok.on(Message.ACTIVATE,async()=>{
        if(closed||busy)return;
        if(!isCurrent()){status.text='Chart changed. Reopen Format Chart.';return;}
        if(!['line','stacked-line','percent-stacked-line','area','stacked-area','percent-stacked-area','scatter','combo','radar','filled-radar'].includes(draft.kind)&&draft.displayBlanksAs==='span'){status.text='Connecting empty cells is available for line charts.';return;}
        if([draft.title,draft.categoryAxisTitle,draft.valueAxisTitle].some(text=>new TextEncoder().encode(text).length>512)){status.text='Chart titles must be at most 512 bytes.';return;}
        try{for(const validate of familyFields)validate();}catch(error){status.text=error.message;return;}
        busy=true;for(const item of [pages,tabs,cancel,ok])item.setProperty('enabled',false);
        try{const accepted=await onApply({type:'update-chart',chart:clone(draft)});if(accepted!==false){busy=false;close();}}
        catch(error){status.text=String(error.message||error).slice(0,240);}
        finally{busy=false;if(!closed)for(const item of [pages,tabs,cancel,ok])item.setProperty('enabled',true);}
    });
    const bind=control=>{control.on(Message.KEY,event=>{if(event.detail?.pressed&&keyName(event.detail)==='Escape'){event.preventDefault();close();}});for(const child of control.children)bind(child);};bind(dialog);
    dialog.on(Message.DESTROY,()=>{closed=true;});return dialog;
}
