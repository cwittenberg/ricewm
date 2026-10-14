export const CHART_TYPES = Object.freeze({
    line:'Trends or ordered observations; multiple measures or a series field give multiple lines.',
    area:'Magnitude over an ordered axis.',
    bar:'Rank or compare categories, especially long labels.',
    column:'Compare a few short categories.',
    stackedBar:'Compare category totals and their additive components; separate positive and negative stacks.',
    stackedColumn:'Additive components over categories or time.',
    stackedArea:'Additive composition over an ordered axis.',
    scatter:'Relationship between two numeric measures.',
    bubble:'Relationship between two numeric measures with a third encoded as area.',
    histogram:'Distribution of one numeric measure.',
    heatmap:'A measure across two categorical dimensions.',
    donut:'Part of a whole with a small number of nonnegative parts.',
    gauge:'A single measurement on a meaningful bounded scale; optional target.',
    bullet:'A compact measurement compared with a target.',
    waterfall:'An explicitly ordered sequence of changes, opening balances and totals.',
    funnel:'Nonnegative values at ordered process stages.',
});
const aliases={'multiline':'line','multi-line':'line','stacked-bar':'stackedBar','stacked-column':'stackedColumn','stacked-area':'stackedArea','speedometer':'gauge','doughnut':'donut','pie':'donut'};
export function chartType(type){const value=aliases[type]||type;if(!Object.hasOwn(CHART_TYPES,value))throw TypeError('Unknown chart type '+type);return value;}
export function chartRows(data){
    if(!Array.isArray(data)||data.length>10000)throw RangeError('Chart data requires at most 10000 rows; aggregate larger data first');
    return Object.freeze(data.map(row=>{
        if(!row||typeof row!=='object'||Array.isArray(row)||Object.keys(row).length>64)throw TypeError('Chart rows must be flat records');
        const copy={};
        for(const [key,value] of Object.entries(row)){
            if(!key||key.length>128||!['string','number','boolean','undefined'].includes(typeof value)&&value!==null||typeof value==='string'&&value.length>1024||typeof value==='number'&&!Number.isFinite(value))throw TypeError('Chart values must be bounded strings, finite numbers, booleans or null');
            Object.defineProperty(copy,key,{value:value??null,enumerable:true});
        }
        return Object.freeze(copy);
    }));
}
const isDate=value=>typeof value==='string'&&/^\d{4}-\d\d-\d\d(?:T.*)?$/.test(value)&&Number.isFinite(Date.parse(value));
function fields(data){
    const names=[...new Set(data.flatMap(Object.keys))],result={numeric:[],time:[],category:[]};
    for(const name of names){
        const values=data.map(row=>row[name]).filter(v=>v!=null);
        if(values.length&&values.every(v=>typeof v==='number'))result.numeric.push(name);
        else if(values.length&&values.every(isDate))result.time.push(name);
        else result.category.push(name);
    }
    return result;
}
/** Returns an executable spec plus reasons, alternatives and caveats. Semantics
 * such as changes, additive components or targets must be supplied by the app. */
export function recommendChart(data,request={}){
    data=chartRows(data);
    const shape=fields(data),spec={...request},warnings=[];
    if(!spec.y)spec.y=shape.numeric.find(name=>name!==spec.x&&name!==spec.size)||(spec.aggregate==='count'?shape.category[0]:undefined);
    const measures=Array.isArray(spec.y)?spec.y:[spec.y];
    if(!spec.x)spec.x=shape.time.find(name=>name!==spec.series)||shape.category.find(name=>name!==spec.series)||shape.numeric.find(name=>!measures.includes(name)&&name!==spec.size);
    if(!spec.series&&shape.category.length>1)warnings.push('Several categorical fields are present; choose series or a filter dimension explicitly.');
    const ys=Array.isArray(spec.y)?spec.y:[spec.y],values=data.flatMap(row=>ys.map(y=>row[y])).filter(v=>typeof v==='number');
    const cardinality=new Set(data.map(row=>row[spec.x]).filter(v=>v!=null)).size;
    const time=spec.xType==='time'||shape.time.includes(spec.x),numericX=spec.xType==='value'||shape.numeric.includes(spec.x);
    const intents=new Set(['compare','trend','composition','distribution','relationship','change','progress','stages']);
    if(spec.intent&&!intents.has(spec.intent))throw TypeError('Unknown chart intent '+spec.intent);
    let type,reason,alternatives;
    if(spec.type&&spec.type!=='auto'){type=chartType(spec.type);reason='The requested chart type takes precedence.';alternatives=[];}
    else if(spec.intent==='change'){type='waterfall';reason='Ordered changes explain how an opening amount reaches a closing amount.';alternatives=['column'];}
    else if(spec.intent==='progress'&&Number.isFinite(spec.min)&&Number.isFinite(spec.max)){type='gauge';reason='A bounded measurement has a meaningful minimum and maximum.';alternatives=['bullet'];}
    else if(spec.intent==='progress'){type='bullet';reason='A target comparison is clearer than inventing a speedometer scale.';alternatives=['bar'];}
    else if(spec.intent==='stages'){type='funnel';reason='Ordered process stages show changes in volume.';alternatives=['bar'];}
    else if(spec.intent==='distribution'||!spec.x&&spec.y){type='histogram';reason='A numeric distribution needs bins rather than one bar per observation.';alternatives=['scatter'];}
    else if(spec.intent==='relationship'||numericX&&!time&&spec.intent!=='trend'){type=spec.size?'bubble':'scatter';reason='Two numeric axes should preserve their numeric spacing.';alternatives=['line'];}
    else if(spec.intent==='composition'&&(spec.series||ys.length>1)){type=time?'stackedArea':'stackedBar';reason='Additive components and their totals can share a stack.';alternatives=[time?'stackedColumn':'column','line'];}
    else if(spec.intent==='composition'&&cardinality<=6&&values.length&&values.every(v=>v>=0)){type='donut';reason='A few nonnegative parts can show a whole.';alternatives=['bar'];}
    else if(time||spec.intent==='trend'){type='line';reason='Ordered observations show change clearly as lines; each series stays separate.';alternatives=['area','column'];}
    else{type='bar';reason='Category comparisons are easiest to read on a shared baseline.';alternatives=['column'];}
    spec.type=type;
    if(!spec.xType)spec.xType=time?'time':numericX?'value':'category';
    if(!data.length)warnings.push('No observations are available; show an empty state.');
    if(!spec.y)warnings.push('No numeric measure was found; select a numeric y field or aggregate count.');
    if(data.some(row=>ys.some(y=>row[y]==null)))warnings.push('Missing measures stay missing; line charts leave gaps.');
    if(type.startsWith('stacked')&&values.some(v=>v<0))warnings.push('Negative components stack separately below zero; do not interpret them as percentages.');
    if(type==='donut'&&cardinality>6)warnings.push('Many slices are difficult to compare; prefer bar.');
    if(type==='waterfall'&&!spec.step)warnings.push('All rows are interpreted as deltas. Supply step for rows marked start or total.');
    if(type==='gauge'&&!(Number.isFinite(spec.min)&&Number.isFinite(spec.max)&&spec.max>spec.min))warnings.push('Supply a meaningful min and max for the gauge.');
    return Object.freeze({type,reason,alternatives:Object.freeze(alternatives),warnings:Object.freeze(warnings),fields:Object.freeze(shape),spec:Object.freeze(spec)});
}

export function chartSpec(data,request={}){
    const spec={...recommendChart(data,request).spec};
    const allowed=new Set(['type','intent','x','y','series','size','xType','aggregate','min','max','target','step','bins','filterField','smooth','unit','title','subtitle']);
    for(const key of Object.keys(spec))if(!allowed.has(key))throw TypeError('Unknown chart spec field '+key);
    if(!spec.y)throw TypeError('Select a numeric y field');
    const ys=Array.isArray(spec.y)?spec.y:[spec.y];
    if(!ys.length||ys.length>16||ys.some(y=>typeof y!=='string'||!y)||new Set(ys).size!==ys.length)throw TypeError('y requires a field name or up to 16 unique numeric fields');
    if(Array.isArray(spec.y)&&spec.series)throw TypeError('Use wide y fields or a long-form series field, not both');
    for(const field of ['x','series','size','step','filterField'])if(spec[field]!=null&&(typeof spec[field]!=='string'||!spec[field]))throw TypeError(field+' requires a field name');
    if(!['gauge','bullet','histogram'].includes(spec.type)&&!spec.x)throw TypeError('This chart requires an x field');
    if(!['category','time','value'].includes(spec.xType))throw TypeError('xType requires category, time or value');
    spec.aggregate??='sum';
    if(!['sum','mean','count'].includes(spec.aggregate))throw TypeError('aggregate requires sum, mean or count');
    if(spec.type==='gauge'&&!(Number.isFinite(spec.min)&&Number.isFinite(spec.max)&&spec.max>spec.min))throw RangeError('Gauge requires finite min < max');
    if(spec.type==='bullet'&&!Number.isFinite(spec.target))throw RangeError('Bullet requires a finite target');
    for(const key of ['target','min','max'])if(spec[key]!=null&&!Number.isFinite(spec[key]))throw TypeError(key+' must be finite');
    if(spec.bins!=null&&(!Number.isInteger(spec.bins)||spec.bins<2||spec.bins>100))throw RangeError('bins must be 2..100');
    if(spec.smooth!=null&&typeof spec.smooth!=='boolean')throw TypeError('smooth requires a boolean');
    for(const key of ['title','subtitle','unit'])if(spec[key]!=null&&(typeof spec[key]!=='string'||spec[key].length>512))throw TypeError(key+' requires bounded text');
    return Object.freeze({...spec,y:Array.isArray(spec.y)?Object.freeze([...spec.y]):spec.y});
}

export function aggregateChartData(data,spec){
    const ys=Array.isArray(spec.y)?spec.y:[spec.y],groups=new Map(),seriesNames=[];
    for(const row of data)for(const y of ys){
        const name=spec.series?String(row[spec.series]??'(Missing)'):ys.length>1?y:y;
        if(!seriesNames.includes(name))seriesNames.push(name);
        const x=row[spec.x]??null,v=row[y];
        if(spec.aggregate!=='count'&&v!=null&&typeof v!=='number')throw TypeError('Chart measure '+y+' must contain numbers or null');
        if(spec.xType==='value'&&x!=null&&typeof x!=='number')throw TypeError('Numeric x requires numbers');
        if(spec.xType==='time'&&x!=null&&!isDate(x)&&typeof x!=='number')throw TypeError('Time x requires ISO dates or numeric timestamps');
        const key=JSON.stringify([name,x]);
        let group=groups.get(key);
        if(!group){group={name,x,sum:0,count:0,rows:[],size:row[spec.size],step:row[spec.step]};groups.set(key,group);}
        if(v!=null&&typeof v==='number'){group.sum+=v;group.count++;if(!Number.isFinite(group.sum))throw RangeError('Chart aggregate overflow');}
        group.rows.push(row);
    }
    if(seriesNames.length>32||groups.size>4096)throw RangeError('Chart exceeds 32 series or 4096 marks; aggregate first');
    const points=[...groups.values()].map(group=>({...group,value:spec.aggregate==='count'?group.rows.length:!group.count?null:spec.aggregate==='mean'?group.sum/group.count:group.sum}));
    if(spec.xType!=='category')points.sort((a,b)=>(spec.xType==='time'?new Date(a.x).getTime():a.x)-(spec.xType==='time'?new Date(b.x).getTime():b.x));
    const categories=[...new Set(points.map(point=>point.x))];
    return {points,seriesNames,categories};
}
