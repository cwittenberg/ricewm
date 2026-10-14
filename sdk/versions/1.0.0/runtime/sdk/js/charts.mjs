import {Column,Row,Stack,Container,Label,Button,Tooltip} from './controls.mjs';
import {State} from './control.mjs';
import {Message} from './messages.mjs';
import {SceneSurface} from './scene-surface.mjs';
import {SceneOverlay} from './scene-overlay.mjs';
import {SceneBuilder} from './scene-builder.mjs';
import {chartRows,chartSpec,recommendChart,CHART_TYPES} from './chart-data.mjs';
import {ChartDashboard} from './chart-dashboard.mjs';
import {Table} from './data-table.mjs';
import {resolveChartTheme} from './chart-theme.mjs';
import {chartOption,themedEChartsOption} from './chart-options.mjs';
import {paintECharts,echartsMarkMap,configureEChartsPainter} from './chart-native-painter.mjs';
import {chartAnimation,createNativeChartEngine} from './chart-engine.mjs';
export {recommendChart,CHART_TYPES,ChartDashboard,resolveChartTheme};

let enginePromise;
const loadEngine=()=>enginePromise??=import('./vendor/echarts/echarts.mjs');
const clear=control=>{for(const child of [...control.children])control.remove(child,{destroy:true});};
let nextChart=1;
const supportedSeries=new Set(['line','bar','scatter','pie','gauge','funnel','heatmap','radar','boxplot','candlestick','treemap','sunburst','sankey','graph','custom']);
function checkedOption(option){
    if(!option||typeof option!=='object'||Array.isArray(option))throw TypeError('ECharts option requires an object');
    const series=Array.isArray(option.series)?option.series:[option.series];
    if(!series.length||series.length>32||series.some(s=>!s||!supportedSeries.has(s.type)))throw TypeError('Unsupported native ECharts series type');
    if(series.some(s=>s.coordinateSystem==='geo'||s.type==='graph'&&s.layout==='force'||s.data?.length>4096))throw RangeError('Native ECharts needs bounded, deterministic series');
    // Bound the raw configuration before giving it to the engine, and reject
    // DOM/network-dependent features. Formatter/renderItem functions are app code.
    const pending=[option],seen=new Set();let count=0,text=0;
    while(pending.length){
        const value=pending.pop();if(++count>100000)throw RangeError('ECharts option is too complex');
        if(typeof value==='string'){text+=value.length;if(text>1024*1024||value.startsWith('image://'))throw RangeError('Native ECharts options cannot load images');}
        if(value&&typeof value==='object'){
            if(seen.has(value))throw TypeError('Cyclic/shared ECharts option objects are unsupported');seen.add(value);
            for(const [key,item] of Object.entries(value)){if(['image','geo','toolbox','graphic'].includes(key))throw TypeError('Native ECharts option does not support '+key);pending.push(item);}
        }
    }
    return option;
}
/** ECharts in the guest app worker, painted as native RiceDisplay vectors.
 * Loading is lazy; await ready (or createChart) before returning startup. */
export class Chart extends Column {
    static family='composite';
    static properties=Object.freeze({animation:{default:true,validate:chartAnimation,reflect:false}});
    static constructorOptions=Object.freeze(['data','spec','dashboard','id','colors','theme','height','option','animation']);
    static authoring=Object.freeze({
        constructor:'await createChart({data:[{month:"2026-01-01",actual:30,forecast:40}],spec:{x:"month",y:["actual","forecast"],type:"line",title:"Revenue"},height:260,colors:{series:{actual:"#168b70"}},...properties}). Or new Chart(options), ui.chart(options), or createControl("Chart",options), then await chart.ready. No chart Wasm, DOM, npm, compute grant or external CDN is required. Uses pinned Apache ECharts 6.1.0.',
        selection:'When type is unspecified, call recommendChart(data,{intent,x,y,series,size,min,max,target}) and inspect type, reason, alternatives, warnings and executable spec. Chart also chooses automatically. Use explicit type for user requests. Multiline: y:["actual","forecast"] or y:"amount",series:"channel". Stacked bars: type:"stackedBar",x:"region",y:"amount",series:"channel". Missing values stay gaps; stacking is additive with separate signs.',
        interaction:'Hover gives a native details caption and ECharts highlight. Click or Enter emits Message.SELECT with {point,seriesIndex,dataIndex}; arrows move through marks, Escape clears this chart filter. Native legend toggles series; Show data exposes keyboard-selectable rows. +/− zoom controls and setRange([min,max]) use numeric/time axes. resetView restores the full extent. Native ECharts animations are enabled by default for entry, data changes and linked filters. animation:false disables them; animation:{duration:600,updateDuration:360,easing:"cubicOut",frameRate:30} customizes them. Await chart.whenSettled() before reading terminal geometry.',
        linked:'const dashboard=new ChartDashboard(rows); await createChart({dashboard,id:"regions",spec:{x:"region",y:"sales"}}); await createChart({dashboard,id:"trend",spec:{x:"date",y:"sales",series:"region"}}). Clicking a mark re-aggregates other charts from shared rows, excluding its own filter to retain selection context. Ctrl-click adds values; repeat click clears. Filters from different sources intersect. dashboard.clear() resets all. Use filterField for a different source dimension; histogram clicks select numeric bin ranges. Dispose charts on closure.',
        methods:'setAnimation(booleanOrOptions), whenSettled(), setData(rows), setSpec(spec), setColors(overrides), setTheme(activePalette), setRange([min,max]|null), resetView(), selectMark(index,{additive:false}), setOption(echartsOption), getDataTable(), inspect(), destroy(). For advanced radar, boxplot, candlestick, treemap, sunburst, sankey and fixed-layout graph charts use option/setOption with normal ECharts options; native output supports paths, solid strokes, local linear fills, unrotated native text and ECharts geometry animation. No images, HTML, maps, force simulation or DOM toolbox. Raw-option SELECT is app-owned; set dashboard filters in your handler.',
        theme:'Native host theme snapshots arrive automatically at launch and on live changes, without a theme-edit permission or bus grants. Chart axes, grids, paint and native typography follow that theme. colors:{series:{measureOrSeries:"#RRGGBB"},accent,positive,negative,panel,...} overrides only supplied colors and persists over theme changes. Use semantic positive/negative for changes. Do not hardcode a light background into ordinary apps.',
    });
    constructor({data=[],spec={},dashboard=null,id=null,colors={},theme={},height=260,option=null,animation=true,...properties}={}){
        if(dashboard!==null&&!(dashboard instanceof ChartDashboard))throw TypeError('dashboard must be a ChartDashboard');
        if(!Number.isInteger(height)||height<160||height>1200)throw RangeError('Chart height requires 160..1200');
        if(id!==null&&(typeof id!=='string'||!id||id.length>128))throw TypeError('Chart id requires bounded text');
        super({text:'panel:fixed:padding=12:gap=8',state:State.FLAT|State.EXPANSIVE,...properties,animation});
        this.chartId=id||properties.name||`chart-${nextChart++}`;
        this.dashboard=dashboard;this.data=chartRows(dashboard?dashboard.rows({except:this.chartId}):data);
        this.spec=option?null:chartSpec(this.data,!this.data.length&&!Object.keys(spec).length?{type:'bar',x:'category',y:'value',xType:'category'}:spec);this.rawOption=option?checkedOption(option):null;
        this.activeTheme=theme;this.colors=colors;this.theme=resolveChartTheme(theme,colors);
        this.hidden=new Set();this.range=null;this.focusIndex=-1;this.hovered=null;this.width=520;this.height=height;this.engine=null;this.markMap={items:[],hit:()=>null};
        this.heading=new Label({text:this.spec?.title||'',visible:!!this.spec?.title,state:State.EMPHASIZED});
        this.subtitle=new Label({text:this.spec?.subtitle||'',visible:!!this.spec?.subtitle,state:State.MUTED});
        this.surface=new SceneSurface({name:`${this.chartId}-plot`,accessibleName:this.spec?.title||'Interactive chart',state:State.EXPANSIVE|State.KEY_NAVIGATION,cursor:'crosshair'});
        this.plot=new Stack({text:`panel:fixed-height:${height}:flush`,state:State.FLAT},[this.surface]);
        this.legend=new Column({text:'panel:fixed:flush:gap=4',visible:false,state:State.FLAT});
        this.details=new Label({name:`${this.chartId}-details`,text:'Loading chart…',state:State.MUTED});
        this.filterLabel=new Label({name:`${this.chartId}-filters`,text:'',visible:false,state:State.MUTED});
        this.reset=new Button({name:`${this.chartId}-reset`,text:'Reset selection',visible:false});
        this.reset.on(Message.ACTIVATE,()=>{this.dashboard?.setFilter(this.chartId,null);this.emit(Message.SELECT,{point:null});});
        this.tableToggle=new Button({name:`${this.chartId}-table`,text:'Show data'});
        this.table=new Column({text:'panel:fixed:flush:gap=4',visible:false,state:State.FLAT});
        this.tableToggle.on(Message.ACTIVATE,()=>{this.table.visible=!this.table.visible;this.tableToggle.text=this.table.visible?'Hide data':'Show data';if(this.table.visible)this.renderTable();});
        this.zoomIn=new Button({name:`${this.chartId}-zoom-in`,text:'+',accessibleName:'Zoom into chart'},[new Tooltip({text:'Zoom in'})]);
        this.zoomOut=new Button({name:`${this.chartId}-zoom-out`,text:'−',accessibleName:'Zoom out of chart'},[new Tooltip({text:'Zoom out'})]);
        this.zoomReset=new Button({name:`${this.chartId}-zoom-reset`,text:'Reset zoom'});
        this.zoomIn.on(Message.ACTIVATE,()=>this.zoom(.6));this.zoomOut.on(Message.ACTIVATE,()=>this.zoom(1/.6));this.zoomReset.on(Message.ACTIVATE,()=>this.resetView());
        this.zoomActions=new Row({text:'panel:fixed:flush',visible:false},[this.zoomOut,this.zoomIn,this.zoomReset]);
        this.append(this.heading);this.append(this.subtitle);this.append(this.plot);this.append(this.legend);this.append(this.details);this.append(this.filterLabel);
        this.append(new Row({text:'panel:fixed:flush'},[this.reset,this.tableToggle,this.zoomActions]));this.append(this.table);
        this.surface.submit(new SceneBuilder(this.width,height).rect([0,0,this.width,height],0).finish());
        this.surface.on(Message.SIZE_CHANGED,event=>{
            const width=Math.max(160,Math.min(4096,Math.round(event.detail?.width||this.width)));
            const first=!this.nativeLayoutReady;this.nativeLayoutReady=true;
            if(first)this.optionSignature=null;
            if(first||width!==this.width){this.width=width;this.render({animate:first});}
        });
        this.surface.on(Message.POINTER,event=>this.pointer(event));this.surface.on(Message.KEY,event=>this.key(event));
        this.on(Message.CHANGE,event=>{if(event.target===this&&event.detail?.property==='animation')this.render({animate:false});});
        this.on(Message.CREATED,event=>{
            if(event.target!==this)return;
            this.stopTheme?.();this.setTheme(this.application.theme||this.activeTheme);
            this.stopTheme=this.application.on(Message.THEME_CHANGED,palette=>this.setTheme(palette));
        });
        if(dashboard)this.stopDashboard=dashboard.subscribe(()=>{
            this.data=chartRows(dashboard.rows({except:this.chartId}));this.render();
        });
        this.ready=loadEngine().then(ec=>{
            if(this.destroyed)return this;
            configureEChartsPainter(ec.color.parse);
            this.driver=createNativeChartEngine(ec,{width:this.width,height:this.height,present:()=>{if(this.engine&&!this.destroyed)this.paint();},onError:error=>{this.animationError=error.message;this.details.text='Chart rendering failed: '+error.message;this.application?.debug?.log?.('error','Chart rendering failed',{error:error.message});}});
            this.engine=this.driver.engine;
            this.render();return this;
        });
    }
    setData(data){if(this.dashboard)throw Error('Use dashboard.setData for a linked chart');if(!this.spec)throw Error('Use setOption to update a raw ECharts chart');const next=chartRows(data);chartSpec(next,this.spec);this.data=next;this.render();return this;}
    setSpec(spec){const next=chartSpec(this.data,spec);this.spec=next;this.rawOption=null;this.hidden.clear();this.range=null;this.focusIndex=-1;this.render();return this;}
    setColors(colors){const theme=resolveChartTheme(this.activeTheme,colors);this.colors=colors;this.theme=theme;this.render();return this;}
    setTheme(active){const theme=resolveChartTheme(active,this.colors);this.activeTheme=active;this.theme=theme;this.render({animate:false});return this;}
    setOption(option){this.rawOption=checkedOption(option);this.spec=null;this.hidden.clear();this.range=null;this.render();return this;}
    setRange(range){
        if(!this.spec)throw TypeError('Use setOption/dataZoom for a raw ECharts chart');
        if(range!==null&&(!Array.isArray(range)||range.length!==2||!range.every(Number.isFinite)||range[0]>=range[1]))throw RangeError('Chart range requires finite min < max');
        if(range&&this.spec?.xType==='category')throw TypeError('Zoom needs a numeric or time x axis');
        this.range=range?Object.freeze([...range]):null;this.render();return this;
    }
    resetView(){return this.setRange(null);}
    zoom(factor){
        if(!this.spec||this.spec.xType==='category')return;
        const values=this.data.map(row=>this.spec.xType==='time'?new Date(row[this.spec.x]).getTime():row[this.spec.x]).filter(Number.isFinite);
        if(!values.length)return;
        const lo=this.range?.[0]??Math.min(...values),hi=this.range?.[1]??Math.max(...values),middle=(lo+hi)/2,span=Math.max(1,hi-lo)*factor;
        return this.setRange([middle-span/2,middle+span/2]);
    }
    setAnimation(value){this.setProperty('animation',value);return this;}
    whenSettled(){return this.driver?.settled||this.ready.then(()=>this.driver?.settled);}
    render({animate=true}={}){
        if(!this.engine||this.destroyed)return;
        const built=this.spec?chartOption(this.data,this.spec,this.theme,{hidden:this.hidden,range:this.range}):{option:this.rawOption,pointSeries:[],legend:[],empty:false};
        const config=this.values.animation,enabled=animate&&config.enabled&&!this.theme.reducedMotion;
        if(!enabled)this.driver.finish();
        if(this.engine.getWidth()!==this.width||this.engine.getHeight()!==this.height)this.engine.resize({width:this.width,height:this.height});
        const option=themedEChartsOption(built.option,this.theme);
        Object.assign(option,{animation:enabled,animationDuration:config.duration,animationDurationUpdate:config.updateDuration,animationEasing:config.easing,animationEasingUpdate:config.easing,stateAnimation:{duration:enabled?120:0}});
        const signature=JSON.stringify(this.spec?{type:this.spec.type,xType:this.spec.xType,x:this.spec.x,y:this.spec.y,series:this.spec.series}:{series:(Array.isArray(this.rawOption.series)?this.rawOption.series:[this.rawOption.series]).filter(Boolean).map(series=>series.type),components:['xAxis','yAxis','radar','polar','parallel','geo','calendar'].filter(key=>this.rawOption[key]!=null)});
        const reset=signature!==this.optionSignature;this.optionSignature=signature;
        this.engine.setOption(option,{notMerge:reset,replaceMerge:reset?undefined:['series'],lazyUpdate:false});
        this.markMap=echartsMarkMap(this.engine,built.pointSeries);this.focusIndex=-1;this.hovered=null;
        this.paint();this.renderLegend(built.legend);
        this.heading.text=this.spec?.title||'';this.heading.visible=!!this.heading.text;
        this.subtitle.text=this.spec?.subtitle||'';this.subtitle.visible=!!this.subtitle.text;
        this.defaultCaption=this.dataCaption(built);this.details.text=this.defaultCaption;
        this.zoomActions.visible=!!this.spec&&this.spec.xType!=='category'&&['line','area','stackedArea','scatter','bubble'].includes(this.spec.type);
        const filters=this.dashboard?.filters||{},own=filters[this.chartId];this.reset.visible=!!own;
        const text=Object.values(filters).map(f=>`${f.field}: ${f.values?f.values.map(String).join(', '):f.range.map(v=>Number(v.toPrecision(4))).join(' – ')}`).join(' · ');
        this.filterLabel.text=text;this.filterLabel.visible=!!text;
        if(this.table.visible)this.renderTable();
        const settled=this.driver.request(config.frameRate);
        const window=this.application?.controlWindows.get(this.id);
        if(window&&this.driver.active)this.application.observeHostWork?.(settled,window.id);
    }
    paint(){
        const {frame,labels}=paintECharts(this.engine,this.width,this.height,this.theme);
        this.surface.submit(frame);
        // Keep existing native label identities whenever possible. Hover paint
        // should not churn the retained widget tree or keyboard focus.
        const current=this.plot.children.slice(1);
        labels.forEach((label,index)=>{
            const bounds={x:label.x/this.width,y:label.y/this.height,width:label.width/this.width,height:label.height/this.height};
            const caption=current[index]?.children[0],state=State.FLAT|State.CENTER_ALIGNED|(label.bold?State.EMPHASIZED:0);
            const nativeLabel=caption||new Label({text:label.text,state});
            nativeLabel.text=label.text;nativeLabel.setProperty('state',state);
            const font={family:'sans-serif',size_pt:Math.round(label.fontSize*.75),weight:label.bold?700:400,italic:false,underline:false};
            const paint={foreground:label.foreground,background:this.theme.panel};
            const inkSignature=JSON.stringify([font,paint]);
            if(nativeLabel.chartInkSignature!==inkSignature||JSON.stringify(nativeLabel.getProperty('documentFont'))!==JSON.stringify(font)||JSON.stringify(nativeLabel.getProperty('documentPaint'))!==JSON.stringify(paint)){nativeLabel.chartInkSignature=inkSignature;nativeLabel.setProperty('documentFont',font);nativeLabel.setProperty('documentPaint',paint);}
            if(caption){if(['x','y','width','height'].some(key=>Math.abs(current[index].bounds[key]-bounds[key])>1e-6))current[index].setBounds(bounds);}
            else this.plot.append(new SceneOverlay({surface:this.surface,bounds,text:'panel:flush',state:State.FLAT},[nativeLabel]));
        });
        for(const child of current.slice(labels.length))this.plot.remove(child,{destroy:true});
    }
    renderLegend(items){
        const signature=JSON.stringify(items.map(item=>[item.name,item.color,this.hidden.has(item.name)]));if(signature===this.legendSignature)return;this.legendSignature=signature;
        clear(this.legend);this.legend.visible=items.length>1;
        // Wrap deliberately into rows, keeping long legend lists scrollable by
        // the owning app's viewport rather than silently hiding categories.
        const columns=Math.max(1,Math.min(4,Math.floor(this.width/160)));
        for(let i=0;i<items.length;i+=columns){
            const row=new Row({text:'panel:fixed:flush:gap=8',state:State.FLAT});
            for(const [index,item] of items.slice(i,i+columns).entries()){
                const button=new Button({name:`${this.chartId}-legend-${i+index}`,text:item.name,checked:!this.hidden.has(item.name),accessibleName:`Toggle ${item.name}`,state:State.FLAT|State.EXPANSIVE},[new Tooltip({text:`Show or hide ${item.name}`})]);
                button.on(Message.ACTIVATE,()=>{if(this.hidden.has(item.name))this.hidden.delete(item.name);else this.hidden.add(item.name);this.render();});
                const swatch=new SceneSurface({accessibleName:`${item.name} color`,backgroundColor:'transparent'});
                const rgb=parseInt(item.color.slice(1),16),packed=(0xff000000|((rgb&255)<<16)|(rgb&0xff00)|(rgb>>>16))>>>0;
                swatch.submit(new SceneBuilder(12,12).ellipse([1,1,10,10],packed).finish());
                row.append(new Row({text:'panel:fixed:flush:gap=8',state:State.FLAT|State.EXPANSIVE},[new Container({text:'panel:fixed:width=16:flush',state:State.FLAT},[swatch]),button]));
            }
            this.legend.append(row);
        }
    }
    pointer(event){
        const d=event.detail;if(!this.engine||!this.values.enabled||!d)return;
        if(d.kind==='wheel'&&(d.modifiers&4)){this.zoom(d.button===4?.8:1.25);event.preventDefault();return;}
        if(d.kind==='cancel'){this.hover(null);return;}
        if(!Number.isFinite(d.x)||!Number.isFinite(d.y))return;
        const item=this.markMap.hit(d.x,d.y);
        if(d.kind==='move')this.hover(item);
        if(d.kind==='up'&&d.button===1&&item){this.focusIndex=this.markMap.items.indexOf(item);this.surface.setProperty('focused',true);this.selectMark(this.focusIndex,{additive:!!(d.modifiers&4)});event.preventDefault();}
    }
    hover(item){
        if(item===this.hovered)return;
        if(this.hovered)this.engine.dispatchAction({type:'downplay',seriesIndex:this.hovered.seriesIndex,dataIndex:this.hovered.dataIndex});
        this.hovered=item;
        if(item){this.engine.dispatchAction({type:'highlight',seriesIndex:item.seriesIndex,dataIndex:item.dataIndex});this.details.text=this.describe(item.point);}
        else this.details.text=this.defaultCaption||'';
        this.paint();
        this.driver.request(this.values.animation.frameRate);
    }
    describe(point){const value=typeof point.value==='number'?new Intl.NumberFormat(undefined,{maximumFractionDigits:4}).format(point.value):JSON.stringify(point.value);return `${point.name} · ${point.range?point.range.map(v=>Number(v.toPrecision(4))).join(' – '):point.x??''}: ${value}${this.spec?.unit||''}`;}
    dataCaption(built){
        if(built.empty)return 'No data for the current filters.';
        const format=value=>new Intl.NumberFormat(undefined,{maximumFractionDigits:1}).format(value)+(this.spec?.unit||'');
        const groups=built.pointSeries||[],points=groups.flat().filter(point=>point.value!=null);
        if(this.spec?.type==='gauge'||this.spec?.type==='bullet')return this.spec.target!=null?`Target ${format(this.spec.target)}`:`Range ${format(this.spec.min)}–${format(this.spec.max)}`;
        if(this.spec?.type==='waterfall'&&points.length)return `${points[0].x} ${format(points[0].interval?.[1]??points[0].value)} → ${points.at(-1).x} ${format(points.at(-1).interval?.[1]??points.at(-1).value)}`;
        if(['line','area','stackedArea'].includes(this.spec?.type))return groups.map(group=>group.filter(point=>point.value!=null).at(-1)).filter(Boolean).map(point=>`${point.name} ${format(point.value)}`).join(' · ');
        if(this.spec?.aggregate==='sum'&&['bar','column','stackedBar','stackedColumn','donut','funnel'].includes(this.spec.type))return 'Total '+format(points.reduce((total,point)=>total+point.value,0));
        return `${this.data.length} observations`;
    }
    key(event){
        const d=event.detail;if(!this.values.enabled||d?.pressed===false)return;
        const key=d?.key||({113:'ArrowLeft',114:'ArrowRight',111:'ArrowUp',116:'ArrowDown',110:'Home',115:'End',36:'Enter',9:'Escape'}[d?.keyCode]);
        if(['ArrowLeft','ArrowRight','ArrowUp','ArrowDown','Home','End'].includes(key)&&this.markMap.items.length){
            this.focusIndex=key==='Home'?0:key==='End'?this.markMap.items.length-1:Math.max(0,Math.min(this.markMap.items.length-1,this.focusIndex+(['ArrowLeft','ArrowUp'].includes(key)?-1:1)));
            this.hover(this.markMap.items[this.focusIndex]);event.preventDefault();
        }else if(key==='Enter'&&this.focusIndex>=0){this.selectMark(this.focusIndex,{additive:!!(d.modifiers&4)});event.preventDefault();}
        else if(key==='Escape'){this.dashboard?.setFilter(this.chartId,null);this.hover(null);event.preventDefault();}
    }
    selectMark(index,{additive=false}={}){
        const item=this.markMap.items[index];if(!item)throw RangeError('Unknown chart mark');
        const point=item.point;
        if(this.dashboard&&this.spec&&(!['gauge','bullet'].includes(this.spec.type)||this.spec.filterField)){
            const field=this.spec.filterField|| (point.range?this.spec.y:this.spec.x);
            if(point.range)this.dashboard.setFilter(this.chartId,{field,range:point.range,inclusiveEnd:point.inclusiveEnd});
            else if(field){const values=[...new Set(point.rows.map(row=>row[field]??null))];if(values.length===1)this.dashboard.toggle(this.chartId,field,values[0],{additive});else this.dashboard.setFilter(this.chartId,{field,values});}
        }
        this.details.text=this.describe(point);
        this.emit(Message.SELECT,{point:Object.freeze({...point,rows:Object.freeze([...point.rows])}),seriesIndex:item.seriesIndex,dataIndex:item.dataIndex});return this;
    }
    getDataTable(){return this.markMap.items.map((item,index)=>Object.freeze({index,series:item.point.name,x:item.point.x,value:item.point.value,range:item.point.range??null}));}
    renderTable(){
        clear(this.table);const rows=this.getDataTable();
        const dataTable=new Table({name:`${this.chartId}-data`,rowKey:'index',data:rows,height:220,selectionMode:'row',columns:[{key:'series',title:'Series',width:180},{key:'x',title:'Category / X',width:180},{key:'value',title:'Value',width:160,align:'right'},{key:'range',title:'Range',width:180,format:v=>v?.join(' – ')??''}]});
        dataTable.on(Message.SELECT,event=>{const row=event.detail?.rows?.[0];if(row)this.selectMark(row.index);});
        this.table.append(dataTable);
    }
    inspect(){return {engine:'Apache ECharts',version:'6.1.0',type:this.spec?.type||'option',width:this.width,height:this.height,theme:this.theme,range:this.range,filters:this.dashboard?.filters||{},marks:this.getDataTable(),hiddenSeries:[...this.hidden],animation:{...this.values.animation,active:this.driver?.active||false,frames:this.driver?.frames||0,reducedMotion:this.theme.reducedMotion===true,error:this.animationError||null}};}
    destroy(){this.stopTheme?.();this.stopDashboard?.();this.driver?.dispose();this.engine=null;return super.destroy();}
}
export async function createChart(options){const chart=new Chart(options);try{await chart.ready;return chart;}catch(error){chart.destroy();throw error;}}
createChart.controlFactory=Object.freeze({options:Chart.constructorOptions,usage:Chart.authoring.constructor});
