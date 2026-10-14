import {CHART_KINDS} from './chart-types.mjs';
/** Transport validation mirrors the Wasm chart document model. It never resolves
 * ranges, reads cells, calculates chart values or rewrites formula references. */
const encoder=new TextEncoder();
const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_CHART'});};
export const CHART_LIMITS=Object.freeze({charts:128,series:24,points:4096,projectionBytes:262144,pageBytes:8192});
function object(value,keys) {
    if(!value||Object.getPrototypeOf(value)!==Object.prototype||Object.keys(value).length!==keys.length||!keys.every(key=>Object.hasOwn(value,key)))fail('Invalid chart fields');
}
function integer(value,min=0,max=0xffffffff){if(!Number.isSafeInteger(value)||value<min||value>max)fail('Invalid chart integer');}
function text(value,maximum){if(typeof value!=='string'||/[\u0000-\u0009\u000b-\u001f\u007f-\u009f\ud800-\udfff]/u.test(value)||encoder.encode(value).length>maximum)fail('Invalid chart text');}
function flag(value){if(typeof value!=='boolean')fail('Invalid chart flag');}
function oneOf(value,options){if(!options.includes(value))fail('Invalid chart option');}
function immutable(value){if(value&&typeof value==='object'){Object.values(value).forEach(immutable);Object.freeze(value);}return value;}
function marker(value){object(value,['row','column','dxEmu','dyEmu']);integer(value.row,0,1048575);integer(value.column,0,16383);integer(value.dxEmu,0,100000000);integer(value.dyEmu,0,100000000);}
function anchor(value) {
    if(value?.mode==='oneCell'){object(value,['mode','from','widthEmu','heightEmu']);marker(value.from);integer(value.widthEmu,9525,100000000);integer(value.heightEmu,9525,100000000);}
    else if(value?.mode==='twoCell'){object(value,['mode','from','to']);marker(value.from);marker(value.to);if(!(value.to.row>value.from.row||value.to.row===value.from.row&&value.to.dyEmu>value.from.dyEmu)||!(value.to.column>value.from.column||value.to.column===value.from.column&&value.to.dxEmu>value.from.dxEmu))fail('Chart anchor must have positive dimensions');}
    else if(value?.mode==='absolute'){object(value,['mode','xEmu','yEmu','widthEmu','heightEmu']);integer(value.xEmu,0,27273042316900);integer(value.yEmu,0,27273042316900);integer(value.widthEmu,9525,100000000);integer(value.heightEmu,9525,100000000);}
    else fail('Invalid chart anchor');
}
function reference(value){object(value,['sheetId','firstRow','lastRow','firstColumn','lastColumn']);integer(value.sheetId);integer(value.firstRow,0,1048575);integer(value.lastRow,value.firstRow,1048575);integer(value.firstColumn,0,16383);integer(value.lastColumn,value.firstColumn,16383);if(value.firstRow!==value.lastRow&&value.firstColumn!==value.lastColumn)fail('Chart series must be one-dimensional');return value.lastRow-value.firstRow+value.lastColumn-value.firstColumn+1;}
function color(value){if(value===null)return;if(typeof value==='string'&&/^#[0-9a-fA-F]{6}$/.test(value))return;if(Array.isArray(value)&&value.length===2&&Number.isInteger(value[0])&&value[0]>=0&&value[0]<12&&Number.isFinite(value[1])&&value[1]>=-1&&value[1]<=1)return;fail('Invalid chart color');}
function copy(value,maximum=32768){const json=JSON.stringify(value);if(!json||encoder.encode(json).length>maximum)fail('Chart metadata exceeds its byte budget');return JSON.parse(json);}

export function checkedChart(value,{draft=false}={}) {
    const chart=copy(value);
    object(chart,['id','sheetId','revision','kind','anchor','title','categoryAxisTitle','valueAxisTitle','legend','labels','categories','series','plotVisibleOnly','displayBlanksAs',...(Object.hasOwn(chart,'options')?['options']:[])]);
    integer(chart.id,draft?0:1,draft?0:0xffffffff);integer(chart.revision,0,draft?0:Number.MAX_SAFE_INTEGER);integer(chart.sheetId);anchor(chart.anchor);
    oneOf(chart.kind,CHART_KINDS);for(const title of [chart.title,chart.categoryAxisTitle,chart.valueAxisTitle])text(title,512);
    object(chart.legend,['visible','position']);flag(chart.legend.visible);oneOf(chart.legend.position,['top','right','bottom','left']);
    object(chart.labels,['visible','showValue','showCategory','showSeries']);Object.values(chart.labels).forEach(flag);flag(chart.plotVisibleOnly);oneOf(chart.displayBlanksAs,['gap','zero','span']);if(!['line','stacked-line','percent-stacked-line','area','stacked-area','percent-stacked-area','scatter','combo','radar','filled-radar'].includes(chart.kind)&&chart.displayBlanksAs==='span')fail('Connecting empty cells requires a line chart');
    if(!Array.isArray(chart.series)||!chart.series.length||chart.series.length>CHART_LIMITS.series)fail('Chart series limit');
    const points=reference(chart.categories),ids=new Set();if(points*chart.series.length>CHART_LIMITS.points)fail('Chart point limit');
    for(const series of chart.series){object(series,['id','name','values','color','marker',...(Object.hasOwn(series,'bindings')?['bindings']:[])]);integer(series.id,1);if(ids.has(series.id))fail('Duplicate chart series identity');ids.add(series.id);if(reference(series.values)!==points)fail('Category and series lengths differ');flag(series.marker);color(series.color);
        if(series.name?.kind==='literal'){object(series.name,['kind','text']);text(series.name.text,256);}
        else if(series.name?.kind==='cell'){object(series.name,['kind','reference']);if(reference(series.name.reference)!==1)fail('Series name requires a single cell');}
        else fail('Invalid chart series name');
        if(series.bindings!==undefined){if(!series.bindings||Object.getPrototypeOf(series.bindings)!==Object.prototype||Object.keys(series.bindings).length>9)fail('Invalid chart bindings');for(const[role,range]of Object.entries(series.bindings)){oneOf(role,['x','size','open','high','low','close','volume','labels','hierarchy']);if(role==='hierarchy'){object(range,['sheetId','firstRow','lastRow','firstColumn','lastColumn']);integer(range.sheetId);integer(range.firstRow,0,1048575);integer(range.lastRow,range.firstRow,1048575);integer(range.firstColumn,0,16383);integer(range.lastColumn,range.firstColumn,16383);if((range.lastRow-range.firstRow+1)*(range.lastColumn-range.firstColumn+1)>4096)fail('Hierarchy binding exceeds limit');}else if(reference(range)>4096)fail('Chart binding exceeds limit');}}
    }
    if(chart.options!==undefined)checkedOptions(chart.options,ids);
    return immutable(chart);
}

export function checkedChartBounds(value) {
    object(value,['x','y','width','height']);
    if(!Object.values(value).every(Number.isFinite)||Math.abs(value.x)>Number.MAX_SAFE_INTEGER||Math.abs(value.y)>Number.MAX_SAFE_INTEGER||value.width<=0||value.height<=0||value.width>42000||value.height>42000)fail('Invalid chart bounds');
    return Object.freeze({...value});
}

export function checkedChartList(value,{offset=0,limit=16}={}) {
    integer(offset,0,CHART_LIMITS.charts);integer(limit,1,16);const page=copy(value,65536);object(page,['total','charts','nextOffset']);integer(page.total,0,CHART_LIMITS.charts);
    if(!Array.isArray(page.charts)||page.charts.length>limit||page.charts.length!==Math.min(limit,Math.max(0,page.total-offset)))fail('Invalid chart list page');
    const next=offset+page.charts.length<page.total?offset+page.charts.length:null;if(page.nextOffset!==next)fail('Invalid chart list cursor');
    const ids=new Set();for(const chart of page.charts){object(chart,['id','chartRevision','title','kind','anchor',...(Object.hasOwn(chart,'bounds')?['bounds']:[])]);integer(chart.id,1);integer(chart.chartRevision,0,Number.MAX_SAFE_INTEGER);text(chart.title,512);oneOf(chart.kind,CHART_KINDS);anchor(chart.anchor);if(chart.bounds!==undefined){object(chart.bounds,['x','y','width','height']);if(!Object.values(chart.bounds).every(Number.isFinite)||Math.abs(chart.bounds.x)>Number.MAX_SAFE_INTEGER||Math.abs(chart.bounds.y)>Number.MAX_SAFE_INTEGER||chart.bounds.width<0||chart.bounds.height<0||chart.bounds.width>42000||chart.bounds.height>42000)fail('Invalid chart list bounds');}if(ids.has(chart.id))fail('Duplicate chart identity');ids.add(chart.id);}
    return immutable(page);
}

function checkedOptions(options,seriesIds){
 if(!options||Object.getPrototypeOf(options)!==Object.prototype||Object.keys(options).some(key=>!['groups','axes'].includes(key)))fail('Invalid chart options');
 const axes=options.axes||[],groups=options.groups||[];if(!Array.isArray(axes)||axes.length>4||!Array.isArray(groups)||groups.length>24)fail('Chart option limit');const axisIds=new Set();
 for(const a of axes){object(a,['id','dimension','position',...['title','minimum','maximum','reversed','logBase'].filter(k=>Object.hasOwn(a,k))]);oneOf(a.id,['x','y','x2','y2']);if(axisIds.has(a.id))fail('Duplicate chart axis');axisIds.add(a.id);oneOf(a.dimension,['x','y']);oneOf(a.position,a.dimension==='x'?['bottom','top']:['left','right']);if(!a.id.startsWith(a.dimension))fail('Axis identity mismatch');if(a.title!==undefined)text(a.title,512);if(a.reversed!==undefined)flag(a.reversed);for(const k of ['minimum','maximum','logBase'])if(a[k]!==undefined&&!Number.isFinite(a[k]))fail('Invalid axis number');if(a.minimum!==undefined&&a.maximum!==undefined&&a.minimum>=a.maximum)fail('Invalid chart axis bounds');if(a.logBase!==undefined&&(a.logBase<2||a.logBase>1000||a.minimum!==undefined&&a.minimum<=0||a.maximum!==undefined&&a.maximum<=0))fail('Invalid logarithmic axis');}
 const ids=new Set(),members=new Set();for(const g of groups){object(g,['id','family','seriesIds',...['stacking','xAxis','yAxis','options'].filter(k=>Object.hasOwn(g,k))]);integer(g.id,1);if(ids.has(g.id))fail('Duplicate plot group');ids.add(g.id);oneOf(g.family,['column','bar','line','area','pie','doughnut','scatter','bubble','radar','stock','surface','contour','histogram','boxplot','waterfall','funnel','pareto','treemap','sunburst']);if(g.stacking!==undefined){oneOf(g.stacking,['none','normal','percent']);if(g.stacking!=='none'&&!['column','bar','line','area'].includes(g.family))fail('Chart family does not stack');}if(g.xAxis!==undefined)oneOf(g.xAxis,['x','x2']);if(g.yAxis!==undefined)oneOf(g.yAxis,['y','y2']);if(!Array.isArray(g.seriesIds)||!g.seriesIds.length||g.seriesIds.length>24)fail('Invalid group members');for(const id of g.seriesIds){if(!seriesIds.has(id)||members.has(id))fail('Invalid group series');members.add(id);}if(g.options!==undefined){if(!g.options||Object.getPrototypeOf(g.options)!==Object.prototype)fail('Invalid family options');copy(g.options,4096);}}
 if(groups.length&&members.size!==seriesIds.size)fail('Every series needs a plot group');
}
