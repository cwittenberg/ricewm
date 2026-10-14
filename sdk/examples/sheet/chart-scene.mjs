import {SceneBuilder} from '@ricewm/app';
import {CHART_KINDS} from './chart-types.mjs';
import {validateChartGeometry,geometryPlot,paintGeometry,hitGeometry} from './chart-geometry.mjs';

const encoder = new TextEncoder();
const MAX_BYTES = 256 * 1024, MAX_POINTS = 4096, MAX_LABEL_BYTES = 64 * 1024;
const own = (value, keys) => {
    if (!value || Object.getPrototypeOf(value) !== Object.prototype || Object.keys(value).some(key => !keys.includes(key))) throw new TypeError('Invalid chart projection object');
};
const integer = (value, low = 0, high = Number.MAX_SAFE_INTEGER) => { if (!Number.isSafeInteger(value) || value < low || value > high) throw new RangeError('Invalid chart integer'); };
const finite = value => { if (!Number.isFinite(value)) throw new RangeError('Invalid chart number'); };
const unit = value => { finite(value); if (value < 0 || value > 1) throw new RangeError('Invalid normalized chart coordinate'); };
const boolean = value => { if (typeof value !== 'boolean') throw new TypeError('Invalid chart flag'); };
const freeze = value => { if (value && typeof value === 'object') { for (const child of Object.values(value)) freeze(child); Object.freeze(value); } return value; };

/** Validate the bounded result of chart-projection, never worksheet cell data.
 * Geometry and formula values have already been computed by the Wasm engine. */
export function checkedChartProjection(input) {
    const json = JSON.stringify(input);
    if (!json || encoder.encode(json).length > MAX_BYTES) throw new RangeError('Chart projection exceeds its byte budget');
    const chart = JSON.parse(json); let labelBytes = 0;
    const text = (value, limit = 1024) => {
        if (typeof value !== 'string' || /[\u0000-\u0009\u000b-\u001f\u007f-\u009f\ud800-\udfff]/u.test(value)) throw new TypeError('Invalid chart label');
        const bytes = encoder.encode(value).length; labelBytes += bytes;
        if (bytes > limit || labelBytes > MAX_LABEL_BYTES) throw new RangeError('Chart labels exceed their byte budget');
    };
    own(chart, ['chartId','chartRevision','dataRevision','kind','title','categoryAxisTitle','valueAxisTitle','legend','categories','axis','zeroPosition','series','geometry']);
    integer(chart.chartId, 1, 0xffffffff); integer(chart.chartRevision); integer(chart.dataRevision);
    if (!CHART_KINDS.includes(chart.kind)) throw new TypeError('Unsupported chart kind');
    text(chart.title, 512); text(chart.categoryAxisTitle, 512); text(chart.valueAxisTitle, 512);
    own(chart.legend, ['visible','position']); boolean(chart.legend.visible);
    if (!['top','right','bottom','left'].includes(chart.legend.position)) throw new TypeError('Invalid legend position');
    own(chart.axis, ['minimum','maximum','ticks']); finite(chart.axis.minimum); finite(chart.axis.maximum);
    if (chart.axis.minimum >= chart.axis.maximum || !Array.isArray(chart.axis.ticks) || chart.axis.ticks.length > 12) throw new RangeError('Invalid chart axis');
    for (const tick of chart.axis.ticks) { own(tick, ['value','position']); finite(tick.value); unit(tick.position); }
    unit(chart.zeroPosition);
    if (!Array.isArray(chart.categories) || chart.categories.length > MAX_POINTS || !Array.isArray(chart.series) || chart.series.length < 1 || chart.series.length > 24 || chart.categories.length * chart.series.length > MAX_POINTS) throw new RangeError('Chart exceeds its point budget');
    const categories = new Set(); let previous = -1;
    for (const category of chart.categories) {
        own(category, ['sourceIndex','label','position']); integer(category.sourceIndex, 0, 0xffffffff); text(category.label); unit(category.position);
        if (categories.has(category.sourceIndex) || category.position < previous) throw new RangeError('Duplicate or unordered chart category');
        categories.add(category.sourceIndex); previous = category.position;
    }
    const seriesIds = new Set(); let count = 0;
    for (const series of chart.series) {
        own(series, ['id','name','color','marker','points']); integer(series.id, 1, 0xffffffff); text(series.name); boolean(series.marker);
        if (seriesIds.has(series.id) || !/^#[0-9a-fA-F]{6}$/.test(series.color) || !Array.isArray(series.points) || (count += series.points.length) > MAX_POINTS) throw new RangeError('Invalid chart series');
        seriesIds.add(series.id); const sources = new Set(); let lastX = -1;
        for (const point of series.points) {
            own(point, ['sourceIndex','value','x','y','segmentStart','bar','label']); integer(point.sourceIndex, 0, 0xffffffff); finite(point.value); unit(point.x); unit(point.y); boolean(point.segmentStart);
            if (!categories.has(point.sourceIndex) || sources.has(point.sourceIndex) || point.x < lastX) throw new RangeError('Invalid chart point source');
            sources.add(point.sourceIndex); lastX = point.x;
            if (point.label !== undefined) text(point.label);
            if (point.bar !== undefined) {
                own(point.bar, ['x','y','width','height']); for (const n of Object.values(point.bar)) unit(n);
                if (Object.keys(point.bar).length !== 4 || point.bar.x + point.bar.width > 1.000001 || point.bar.y + point.bar.height > 1.000001 || chart.kind !== 'clustered-column') throw new RangeError('Invalid chart bar');
            } else if (chart.kind === 'clustered-column') throw new TypeError('Column chart point requires bar geometry');
        }
    }
    if(chart.geometry!==undefined)validateChartGeometry(chart.geometry,{seriesIds,text});
    else if(!['clustered-column','line'].includes(chart.kind))throw new TypeError('Chart kind requires projected geometry');
    return freeze(chart);
}


export function chartLegendItems(projection){return projection.geometry?.legendItems||projection.series.map(series=>({text:series.name,color:series.color,seriesId:series.id}));}

export function chartColor(hex) {
    if (!/^#[0-9a-fA-F]{6}$/.test(hex)) throw new TypeError('Chart color must be #RRGGBB');
    return (0xff000000 | parseInt(hex.slice(5,7),16) << 16 | parseInt(hex.slice(3,5),16) << 8 | parseInt(hex.slice(1,3),16)) >>> 0;
}
const inside = (rect, x, y, pad = 0) => x >= rect[0] - pad && y >= rect[1] - pad && x <= rect[0] + rect[2] + pad && y <= rect[1] + rect[3] + pad;
const overlaps = (a,b) => a[0] < b[0]+b[2] && a[0]+a[2] > b[0] && a[1] < b[1]+b[3] && a[1]+a[3] > b[1];
const tickText = value => value === 0 ? '0' : Number(value.toPrecision(6)).toString();
const palette = theme => Object.fromEntries(Object.entries({background:'#ffffff',foreground:'#202020',grid:'#dedede',axis:'#666666',selection:'#2878d4',...theme}).map(([key,value]) => [key, chartColor(value)]));

/** Layout changes only presentation; normalized plot geometry stays authoritative. */
export function layoutChart(projection, {width = 480, height = 300, documentZoom = 100, theme = {}} = {}) {
    integer(width, 1, 42000); integer(height, 1, 42000); integer(documentZoom,25,400);
    const scale=documentZoom/100,physicalWidth=width,physicalHeight=height,legendItems=chartLegendItems(projection);
    width/=scale;height/=scale;
    if(width<80||height<60)return {projection,width:physicalWidth,height:physicalHeight,scale,colors:palette(theme),plot:geometryPlot([0,0,physicalWidth,physicalHeight],projection.geometry),labels:[],hits:[],swatches:[],skippedLabels:projection.series.reduce((n,s)=>n+s.points.filter(p=>p.label).length,0),hiddenLegendEntries:projection.legend.visible?legendItems.length:0,compact:true};
    const colors = palette(theme), labels = [], hits = [], swatches = [];let geometrySkipped=0;
    const titleHeight = projection.title ? 32 : 12;
    const sideLegend = projection.legend.visible && ['left','right'].includes(projection.legend.position);
    const legendWidth = sideLegend ? Math.min(144, Math.max(64, width * .25)) : 0;
    const legendColumns = Math.max(1, Math.floor((width - 24) / 140));
    const legendRows = projection.legend.visible ? Math.ceil(legendItems.length / (sideLegend ? 1 : legendColumns)) : 0;
    const legendHeight = projection.legend.visible && !sideLegend ? Math.min(Math.floor(height*.3), legendRows * 20 + 8) : 0;
    const plot = [56 + (projection.valueAxisTitle ? 20 : 0) + (projection.legend.visible && projection.legend.position === 'left' ? legendWidth : 0),
        titleHeight + (projection.legend.visible && projection.legend.position === 'top' ? legendHeight : 0), 0, 0];
    plot[2] = Math.max(24, width - plot[0] - 16 - (projection.legend.visible && projection.legend.position === 'right' ? legendWidth : 0));
    plot[3] = Math.max(24, height - plot[1] - 36 - (projection.categoryAxisTitle ? 20 : 0) - (projection.legend.visible && projection.legend.position === 'bottom' ? legendHeight : 0));
    if(projection.geometry?.axes.some(axis=>axis.position==='right'))plot[2]=Math.max(24,plot[2]-52);
    if(projection.geometry?.axes.some(axis=>axis.position==='top')||projection.geometry?.primitives.some(primitive=>primitive.type==='label'&&primitive.role==='value-axis-title'&&primitive.y<0)){plot[1]+=24;plot[3]=Math.max(24,plot[3]-24);}
    // Tiny charts retain their plot and clip peripheral labels instead of scaling the document font.
    plot[0] = Math.min(plot[0], width - 40); plot[1] = Math.min(plot[1], height - 48);
    plot[2] = Math.min(plot[2], width - plot[0] - 8); plot[3] = Math.min(plot[3], height - plot[1] - 24);
    function label(id, text, rect, {size = 12, bold = false, align = 'center', rotation = 0, part = null} = {}) {
        if (!text || rect[2] <= 0 || rect[3] <= 0) return;
        const scalars=Array.from(text.replaceAll('\n',' ')),limit=Math.max(2,Math.min(160,Math.floor((rotation?rect[3]:rect[2])/(size*.45))));
        const displayText=scalars.length>limit?scalars.slice(0,limit-1).join('')+'…':scalars.join('');
        labels.push({id,text:displayText,fullText:text,rect,size,bold,align,rotation,color:colors.foreground,pathBase:1 + labels.length * 8192});
        if (part) hits.push({part, rect});
    }
    label('title', projection.title, [12, 6, width-24, 24], {size:16,bold:true,part:{kind:'title'}});
    if(!projection.geometry) {
    for (const [i,tick] of projection.axis.ticks.entries()) label(`tick-${i}`,tickText(tick.value),[plot[0]-52,plot[1]+tick.position*plot[3]-8,46,16],{align:'end'});
    const stride = Math.max(1, Math.ceil(projection.categories.length / Math.max(1, Math.floor(plot[2] / 64))));
    const categoryWidth = Math.min(100, Math.max(40, plot[2] / Math.max(1,Math.ceil(projection.categories.length / stride))));
    for (let i=0;i<projection.categories.length;i+=stride) {
        const category=projection.categories[i];
        label(`category-${category.sourceIndex}`,category.label,[plot[0]+category.position*plot[2]-categoryWidth/2,plot[1]+plot[3]+6,categoryWidth,18]);
    }
    hits.push({part:{kind:'category-axis'},rect:[plot[0],plot[1]+plot[3],plot[2],32]},{part:{kind:'value-axis'},rect:[Math.max(0,plot[0]-56),plot[1],56,plot[3]]});
    label('category-axis-title',projection.categoryAxisTitle,[plot[0],plot[1]+plot[3]+26,plot[2],18],{part:{kind:'category-axis'}});
    label('value-axis-title',projection.valueAxisTitle,[6,plot[1],18,plot[3]],{rotation:-90,part:{kind:'value-axis'}});
    } else {
        const fitted=geometryPlot(plot,projection.geometry);plot.splice(0,4,...fitted);
        for(const axis of projection.geometry.axes){
            const horizontal=axis.dimension==='x',low=axis.position==='bottom'||axis.position==='left';
            const tickStride=axis.kind==='category'?Math.max(1,Math.ceil(axis.ticks.length/Math.max(1,Math.floor((horizontal?plot[2]:plot[3])/(horizontal?64:20))))):1;
            for(const[i,tick]of axis.ticks.entries()){if(i%tickStride)continue;const rect=horizontal?[plot[0]+tick.position*plot[2]-40,axis.position==='bottom'?plot[1]+plot[3]+5:plot[1]-22,80,18]:[axis.position==='left'?plot[0]-52:plot[0]+plot[2]+6,plot[1]+tick.position*plot[3]-8,46,16];
                label(`axis-${axis.id}-${i}`,tick.label,rect,{align:horizontal?'center':low?'end':'start'});}
            const rect=horizontal?[plot[0],axis.position==='bottom'?plot[1]+plot[3]+25:plot[1]-42,plot[2],18]:[axis.position==='left'?6:plot[0]+plot[2]+42,plot[1],18,plot[3]];
            label(`axis-title-${axis.id}`,axis.title,rect,{rotation:horizontal?0:-90,part:{kind:'axis',axisId:axis.id}});
        }
        const occupied=[];for(const[i,p]of projection.geometry.primitives.entries())if(p.type==='label'){const ww=Math.min(140,Math.max(28,Array.from(p.text).length*7)),rect=[plot[0]+p.x*plot[2]-(p.align==='center'?ww/2:p.align==='end'?ww:0),plot[1]+p.y*plot[3]-8,ww,16];if(labels.length>=512||occupied.some(previous=>overlaps(previous,rect))){geometrySkipped++;continue;}occupied.push(rect);label(`geometry-${i}`,p.text,rect,{align:p.align,part:p.seriesId?{kind:p.sourceIndex===undefined?'series':'point',seriesId:p.seriesId,...(p.sourceIndex===undefined?{}:{sourceIndex:p.sourceIndex})}:{kind:p.role.startsWith('category')?'category-axis':'value-axis'}});}
    }
    let hiddenLegendEntries=0;
    if (projection.legend.visible) {
        const left = projection.legend.position === 'left' ? 8 : projection.legend.position === 'right' ? width-legendWidth+4 : 12;
        const top = projection.legend.position === 'bottom' ? height-legendHeight+4 : projection.legend.position === 'top' ? titleHeight+4 : plot[1];
        const cols = sideLegend ? 1 : legendColumns, itemWidth = sideLegend ? legendWidth-8 : (width-24)/cols;
        const maxRows = sideLegend ? Math.max(1,Math.floor((height-top-8)/20)) : Math.max(1,Math.floor((legendHeight-8)/20));
        const capacity=cols*maxRows,shown=legendItems.length>capacity?Math.max(0,capacity-1):legendItems.length;
        hiddenLegendEntries=legendItems.length-shown;
        for (const [i,item] of legendItems.entries()) {
            if(i>=shown)break;
            const x=left+(i%cols)*itemWidth,y=top+Math.floor(i/cols)*20,part={kind:item.sourceIndex===undefined?'series':'point',seriesId:item.seriesId,...(item.sourceIndex===undefined?{}:{sourceIndex:item.sourceIndex})};
            swatches.push({rect:[x,y+5,12,8],color:chartColor(item.color)});
            label(`legend-${item.sourceIndex===undefined?item.seriesId:`${item.seriesId}-${item.sourceIndex}`}`,item.text,[x+18,y,itemWidth-22,18],{align:'start',part});
            hits.push({part,rect:[x,y,itemWidth,20]});
        }
        if(hiddenLegendEntries){const x=left+(shown%cols)*itemWidth,y=top+Math.floor(shown/cols)*20;
            label('legend-overflow',`+${hiddenLegendEntries} more ${projection.geometry?.legendItems?'items':'series'}`,[x,y,itemWidth-4,18],{align:'start',part:{kind:'legend'}});}
    }
    let skippedLabels = geometrySkipped; const dataLabels = [];
    for (const series of projection.series) for (const point of series.points) {
        if (!point.label) continue;
        const boxWidth = Math.min(104,Math.max(28,Array.from(point.label).length*7));
        const x=plot[0]+(point.bar?point.bar.x+point.bar.width/2:point.x)*plot[2],y=plot[1]+(point.bar?(point.value<0?point.bar.y+point.bar.height:point.bar.y):point.y)*plot[3];
        const labelY=point.bar&&point.value<0?Math.min(plot[1]+plot[3]-16,y+4):Math.max(plot[1],y-20);
        const rect=[Math.max(plot[0],Math.min(plot[0]+plot[2]-boxWidth,x-boxWidth/2)),labelY,Math.min(boxWidth,plot[2]),16];
        if (labels.length >= 512 || dataLabels.some(previous => overlaps(previous,rect))) { skippedLabels++;continue; }
        dataLabels.push(rect);label(`point-${series.id}-${point.sourceIndex}`,point.label,rect,{part:{kind:'point',seriesId:series.id,sourceIndex:point.sourceIndex}});
    }
    const scaled=rect=>rect.map(value=>value*scale);
    return {projection,width:physicalWidth,height:physicalHeight,scale,colors,plot:scaled(plot),
        labels:labels.map(label=>({...label,rect:scaled(label.rect),size:label.size*scale})),
        hits:hits.map(hit=>({...hit,rect:scaled(hit.rect)})),swatches:swatches.map(swatch=>({...swatch,rect:scaled(swatch.rect)})),skippedLabels,hiddenLegendEntries};
}

export function hitChart(layout,x,y) {
    if (!Number.isFinite(x)||!Number.isFinite(y)||!inside([0,0,layout.width,layout.height],x,y)) return null;
    // Labels and legend are first-class parts; plot hit testing retains source indices.
    for (const hit of [...layout.hits].reverse()) if(inside(hit.rect,x,y)) return hit.part;
    const [px,py,pw,ph]=layout.plot;
    if(inside(layout.plot,x,y)) {
        if(layout.projection.geometry)return hitGeometry(layout.projection.geometry,layout.plot,x,y)||{kind:'plot'};
        for(const series of [...layout.projection.series].reverse()) for(let i=series.points.length-1;i>=0;i--) {
            const p=series.points[i],xx=px+p.x*pw,yy=py+p.y*ph;
            if(p.bar ? inside([px+p.bar.x*pw,py+p.bar.y*ph,p.bar.width*pw,p.bar.height*ph],x,y,2) : Math.hypot(x-xx,y-yy)<=7) return {kind:'point',seriesId:series.id,sourceIndex:p.sourceIndex};
            if(!p.segmentStart&&i>0&&!p.bar) {
                const previous=series.points[i-1],ax=px+previous.x*pw,ay=py+previous.y*ph,dx=xx-ax,dy=yy-ay;
                const t=Math.max(0,Math.min(1,((x-ax)*dx+(y-ay)*dy)/(dx*dx+dy*dy||1)));
                if(Math.hypot(x-ax-t*dx,y-ay-t*dy)<=5)return {kind:'series',seriesId:series.id};
            }
        }
        return {kind:'plot'};
    }
    return {kind:'chart'};
}

/** Render only native vector commands. Text frames must come from the shared
 * typography engine, whose glyph outlines use each label's reserved pathBase. */
export function renderChartScene(layout,{textFrames = new Map(),selected = false,selectedPart = null,viewportClip=null} = {}) {
    const {width,height,plot,projection,colors,scale=1}=layout,[px,py,pw,ph]=plot;
    const builder=new SceneBuilder(viewportClip?.width||width,viewportClip?.height||height,{maximumBytes:8*1024*1024-56});
    if(viewportClip)builder.pushTransform([1,0,0,1,-viewportClip.x,-viewportClip.y]);
    builder.rect([0,0,width,height],colors.background);
    if(!projection.geometry){
    for(const tick of projection.axis.ticks)builder.line([px,py+tick.position*ph],[px+pw,py+tick.position*ph],scale,colors.grid);
    builder.line([px,py],[px,py+ph],scale,colors.axis).line([px,py+projection.zeroPosition*ph],[px+pw,py+projection.zeroPosition*ph],scale,colors.axis);
    }else for(const axis of projection.geometry.axes){const horizontal=axis.dimension==='x',position=axis.position==='top'?py:axis.position==='bottom'?py+ph:axis.position==='left'?px:px+pw;
        for(const tick of axis.ticks)if(axis.kind==='value')horizontal?builder.line([px+tick.position*pw,py],[px+tick.position*pw,py+ph],scale,colors.grid):builder.line([px,py+tick.position*ph],[px+pw,py+tick.position*ph],scale,colors.grid);
        horizontal?builder.line([px,position],[px+pw,position],scale,colors.axis):builder.line([position,py],[position,py+ph],scale,colors.axis);}
    builder.pushClip(plot);
    if(projection.geometry)paintGeometry(builder,projection.geometry,plot,{color:chartColor,scale,selection:selectedPart,selectionColor:colors.selection});
    else {
    for(const [seriesIndex,series]of projection.series.entries()) {
        const color=chartColor(series.color),path=[];
        for(const point of series.points) {
            const x=px+point.x*pw,y=py+point.y*ph;
            if(point.bar)builder.rect([px+point.bar.x*pw,py+point.bar.y*ph,point.bar.width*pw,point.bar.height*ph],color);
            else path.push([point.segmentStart||!path.length?'M':'L',x,y]);
        }
        if(path.length)builder.definePath(0x80000000+seriesIndex,path).strokePath(0x80000000+seriesIndex,(selectedPart?.seriesId===series.id?3:2)*scale,color,{join:1});
        for(const point of series.points)if(!point.bar&&(series.marker||series.points.length===1))builder.ellipse([px+point.x*pw-3*scale,py+point.y*ph-3*scale,6*scale,6*scale],color);
        if(selectedPart?.kind==='point'&&selectedPart.seriesId===series.id) {
            const point=series.points.find(p=>p.sourceIndex===selectedPart.sourceIndex);
            if(point)builder.ellipse([px+point.x*pw-5,py+point.y*ph-5,10,10],colors.selection,{filled:false,width:2});
        }
    }
    }
    builder.popClip();
    for(const swatch of layout.swatches)builder.rect(swatch.rect,swatch.color);
    for(const label of layout.labels) {
        const frame=textFrames.get(label.id);if(!frame)continue;
        const [x,y,w,h]=label.rect;
        builder.pushClip(label.rect);
        if(label.rotation===-90)builder.pushTransform([0,-1,1,0,x+(w-frame.height)/2,y+(h+frame.width)/2]);
        else builder.pushTransform([1,0,0,1,x+(label.align==='center'?(w-frame.width)/2:label.align==='end'?w-frame.width:0),y+(h-frame.height)/2]);
        builder.append(frame.displayList).popTransform().popClip();
    }
    if(selected) {
        const c=colors.selection;
        builder.line([1,1],[width-1,1],1,c).line([width-1,1],[width-1,height-1],1,c).line([width-1,height-1],[1,height-1],1,c).line([1,height-1],[1,1],1,c);
        for(const [x,y]of chartHandles(width,height).map(handle=>[handle.x,handle.y]))builder.rect([x-3,y-3,6,6],c);
    }
    if(viewportClip)builder.popTransform();
    return builder.finish();
}

export function chartHandles(width,height) {
    return [['nw',3,3],['n',width/2,3],['ne',width-3,3],['e',width-3,height/2],['se',width-3,height-3],['s',width/2,height-3],['sw',3,height-3],['w',3,height/2]].map(([edge,x,y])=>({edge,x,y}));
}

// Shared typography engines accept one job at a time. Sharing the same callback
// across charts also shares this queue; no chart can race another engine call.
const shapingQueues=new WeakMap();
function queuedShape(shapeText,request) {
    const pending=(shapingQueues.get(shapeText)||Promise.resolve()).catch(()=>{}).then(()=>shapeText(request));
    shapingQueues.set(shapeText,pending.then(()=>{},()=>{}));return pending;
}
export function createChartTextRenderer(shapeText,{maximumBytes=4*1024*1024}={}) {
    if(typeof shapeText!=='function')throw new TypeError('Charts require the shared native typography shaper');
    integer(maximumBytes,1024,8*1024*1024-56);
    const cache=new Map();let bytes=0;
    return {
        async shape(layout) {
            const frames=new Map();let total=0;
            for(const label of layout.labels) {
                const key=JSON.stringify([label.id,label.text,label.size,label.bold,label.color,label.pathBase]);let frame=cache.get(key);
                if(!frame) {
                    frame=await queuedShape(shapeText,{text:label.text,families:'Liberation Sans, sans-serif',size:label.size,width:null,bold:label.bold,italic:false,alignment:'start',color:label.color,pathBase:label.pathBase,underline:false,spans:[]});
                    if(!frame||!Number.isFinite(frame.width)||!Number.isFinite(frame.height)||frame.width<0||frame.height<0||!(frame.displayList instanceof Uint8Array)||frame.displayList.length>maximumBytes)throw new RangeError('Invalid chart typography frame');
                    frame={width:frame.width,height:frame.height,displayList:frame.displayList.slice()};
                    while(bytes+frame.displayList.length>maximumBytes&&cache.size){const first=cache.keys().next().value;bytes-=cache.get(first).displayList.length;cache.delete(first);}
                    cache.set(key,frame);bytes+=frame.displayList.length;
                } else {cache.delete(key);cache.set(key,frame);}
                if((total+=frame.displayList.length)>maximumBytes)throw new RangeError('Chart text exceeds its scene budget');
                frames.set(label.id,frame);
            }
            return frames;
        },
        clear(){cache.clear();bytes=0;},
        get byteLength(){return bytes;},
    };
}
