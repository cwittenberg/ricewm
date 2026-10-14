import {aggregateChartData} from './chart-data.mjs';

/** Apply presentation defaults beneath explicit ECharts options. Keep callback
 * functions and datasets intact; no serialization or mutation of app options. */
export function themedEChartsOption(option,theme){
    const axis=value=>({
        ...value,axisLabel:{color:theme.muted,...value.axisLabel},
        axisLine:{...value.axisLine,lineStyle:{color:theme.border,...value.axisLine?.lineStyle}},
        splitLine:{...value.splitLine,lineStyle:{color:theme.grid,...value.splitLine?.lineStyle}},
        nameTextStyle:{color:theme.muted,...value.nameTextStyle},
    });
    const result={color:theme.series,...option,animation:false,backgroundColor:theme.panel,
        textStyle:{color:theme.foreground,fontSize:14,...option.textStyle},tooltip:{show:false,renderMode:'richText'},legend:{show:false}};
    for(const key of ['xAxis','yAxis','angleAxis','radiusAxis','parallelAxis'])if(option[key])result[key]=Array.isArray(option[key])?option[key].map(axis):axis(option[key]);
    if(option.radar){
        const radar=value=>({...value,axisName:{color:theme.muted,...value.axisName},axisLine:{...value.axisLine,lineStyle:{color:theme.border,...value.axisLine?.lineStyle}},splitLine:{...value.splitLine,lineStyle:{color:theme.grid,...value.splitLine?.lineStyle}},splitArea:{show:false,...value.splitArea}});
        result.radar=Array.isArray(option.radar)?option.radar.map(radar):radar(option.radar);
    }
    const series=value=>{
        if(value.type==='candlestick')return {...value,itemStyle:{color:theme.positive,color0:theme.negative,borderColor:theme.positive,borderColor0:theme.negative,...value.itemStyle}};
        return value;
    };
    if(option.series)result.series=Array.isArray(option.series)?option.series.map(series):series(option.series);
    return result;
}

function valueOf(data,spec){
    if(spec.aggregate==='count')return data.length||null;
    const values=data.map(row=>row[spec.y]).filter(value=>value!=null);
    if(values.some(value=>typeof value!=='number'))throw TypeError('Chart measure must be numeric');
    if(!values.length)return null;
    const sum=values.reduce((sum,value)=>sum+value,0);
    if(!Number.isFinite(sum))throw RangeError('Chart aggregate overflow');
    return spec.aggregate==='count'?values.length:spec.aggregate==='mean'?sum/values.length:sum;
}
export function chartOption(data,spec,theme,{hidden=new Set(),range=null}={}){
    const color=(name,index)=>theme.seriesOverrides[name]||theme.seriesOverrides[index]||theme.series[index%theme.series.length];
    const option={animation:false,backgroundColor:theme.panel,color:theme.series,
        textStyle:{color:theme.foreground,fontSize:14},aria:{enabled:false},tooltip:{show:false},
        grid:{left:72,right:24,top:26,bottom:48},legend:{show:false},
    };
    const pointSeries=[],legend=[];
    const axis=(type='value')=>({type,splitNumber:4,axisLabel:{color:theme.muted,fontSize:14,hideOverlap:true,rotate:0},axisLine:{lineStyle:{color:theme.border}},axisTick:{show:false},splitLine:{show:type!=='category',lineStyle:{color:theme.grid}},nameTextStyle:{color:theme.muted}});
    const type=spec.type;
    if(['histogram','gauge','bullet','bubble','heatmap','donut','funnel','waterfall'].includes(type)&&Array.isArray(spec.y))throw TypeError(type+' requires one y field');
    if(type==='gauge'||type==='bullet'){
        if(Array.isArray(spec.y)||spec.series)throw TypeError('Gauge and bullet need one measure');
        const value=valueOf(data,spec),point={name:spec.y,x:spec.y,value,rows:data};
        if(value===null){option.series=[];return {option,pointSeries,legend,empty:true};}
        if(type==='gauge'){
            option.series=[{type:'gauge',min:spec.min,max:spec.max,splitNumber:5,startAngle:210,endAngle:-30,radius:'90%',center:['50%','60%'],
                axisLine:{lineStyle:{width:16,color:[[.65,theme.positive],[.85,theme.accent],[1,theme.negative]]}},
                progress:{show:true,width:16,itemStyle:{color:theme.accent}},pointer:{show:true,width:4,itemStyle:{color:theme.foreground}},
                axisTick:{show:false},splitLine:{length:8,distance:4,lineStyle:{color:theme.border,width:1}},axisLabel:{distance:-8,color:theme.muted,fontSize:14},
                anchor:{show:true,size:10,itemStyle:{color:theme.foreground}},title:{show:false},detail:{valueAnimation:true,offsetCenter:[0,'38%'],fontSize:24,color:theme.foreground,formatter:value=>`${new Intl.NumberFormat(undefined,{maximumFractionDigits:1}).format(value)}${spec.unit||''}`},data:[{value,name:spec.y}]}];
            if(spec.target!=null){option.series.push({type:'gauge',min:spec.min,max:spec.max,startAngle:210,endAngle:-30,radius:'90%',center:['50%','60%'],axisLine:{show:false},axisTick:{show:false},splitLine:{show:false},axisLabel:{show:false},detail:{show:false},title:{show:false},pointer:{show:true,width:2,length:'90%',itemStyle:{color:theme.muted}},data:[{value:spec.target}]});}
        }else{
            const max=spec.max??Math.max(value,spec.target,1)*1.15,min=spec.min??Math.min(0,value,spec.target);
            if(max<=min)throw RangeError('Bullet requires min < max');
            option.xAxis={...axis(),min,max};option.yAxis={...axis('category'),data:[spec.y],axisLabel:{show:false}};
            option.series=[{type:'bar',data:[value],barWidth:24,itemStyle:{color:theme.accent,borderRadius:3},showBackground:true,backgroundStyle:{color:theme.grid},markLine:{symbol:'none',label:{show:false},lineStyle:{color:theme.foreground,width:3,type:'solid'},data:[{xAxis:spec.target}]}}];
        }
        pointSeries.push([point]);return {option,pointSeries,legend,empty:false};
    }
    if(type==='histogram'){
        const values=data.map(row=>row[spec.y]).filter(v=>v!=null);
        if(values.some(v=>typeof v!=='number'))throw TypeError('Histogram needs numeric values');
        if(!values.length){option.series=[];return {option,pointSeries,legend,empty:true};}
        const lo=Math.min(...values),hi=Math.max(...values),bins=spec.bins??Math.min(40,Math.max(2,Math.ceil(Math.sqrt(values.length)))),span=hi===lo?Math.max(1,Math.abs(lo)*.01):(hi-lo),step=span/bins;
        const points=Array.from({length:bins},(_,i)=>({name:spec.y,x:lo+i*step,value:0,rows:[],range:[lo+i*step,lo+(i+1)*step],inclusiveEnd:i===bins-1}));
        for(const row of data){if(row[spec.y]==null)continue;const index=Math.min(bins-1,Math.floor((row[spec.y]-lo)/step));points[index].value++;points[index].rows.push(row);}
        option.xAxis={...axis('category'),data:points.map(p=>`${Number(p.range[0].toPrecision(3))}`)};option.yAxis=axis();
        option.series=[{type:'bar',barCategoryGap:'2%',itemStyle:{color:theme.accent},data:points.map(p=>p.value)}];pointSeries.push(points);
        return {option,pointSeries,legend,empty:false};
    }
    let points,seriesNames,categories;
    if(type==='scatter'||type==='bubble'){
        seriesNames=[...new Set(data.map(row=>String(spec.series?row[spec.series]??'(Missing)':spec.y)))];
        points=data.filter(row=>row[spec.x]!=null&&row[spec.y]!=null).map(row=>({name:String(spec.series?row[spec.series]??'(Missing)':spec.y),x:row[spec.x],value:row[spec.y],size:row[spec.size],rows:[row]}));
        categories=[];
        if(points.length>4096||seriesNames.length>32)throw RangeError('Chart exceeds 32 series or 4096 marks; aggregate first');
        if(points.some(p=>typeof p.x!=='number'||typeof p.value!=='number'))throw TypeError('Scatter needs numeric x and y');
    }else ({points,seriesNames,categories}=aggregateChartData(data,spec));
    for(const [index,name] of seriesNames.entries())legend.push({name,color:color(name,index)});
    const names=seriesNames.filter(name=>!hidden.has(name)),horizontal=['bar','stackedBar'].includes(type),stacked=type.startsWith('stacked');
    const isCartesian=['line','area','bar','column','stackedBar','stackedColumn','stackedArea','waterfall'].includes(type);
    if(isCartesian){
        const category={...axis(spec.xType),min:null,max:null,...(spec.xType==='category'?{data:categories.map(x=>x==null?'(Missing)':String(x))}:{}),boundaryGap:!['line','area','stackedArea'].includes(type)};
        const measure=axis();
        option.xAxis=horizontal?measure:category;option.yAxis=horizontal?category:measure;
        if(range){const ordered=horizontal?option.yAxis:option.xAxis;ordered.min=range[0];ordered.max=range[1];}
        option.series=[];
        if(type==='waterfall'){
            if(names.length>1)throw TypeError('Waterfall requires one ordered measure');
            if(spec.xType!=='category')throw TypeError('Waterfall requires a category x axis');
            option.xAxis.axisLabel={...option.xAxis.axisLabel,interval:0};
            let total=0;const ordered=points.filter(p=>p.name===names[0]);
            for(const point of ordered){
                if(point.value==null)throw TypeError('Waterfall steps cannot be missing');
                const step=point.step??'delta';if(!['start','delta','total'].includes(step))throw TypeError('Waterfall step requires start, delta or total');
                const before=total;total=step==='delta'?total+point.value:point.value;
                if(!Number.isFinite(total))throw RangeError('Waterfall total overflow');
                // Floating intervals need custom bars when a change crosses zero.
                point.interval=step==='delta'?[before,total]:[0,total];
            }
            option.series=[{type:'custom',renderItem:(params,api)=>{
                const point=ordered[params.dataIndex],p0=api.coord([params.dataIndex,point.interval[0]]),p1=api.coord([params.dataIndex,point.interval[1]]),width=api.size([1,0])[0]*.6;
                const bar={type:'rect',shape:{x:p0[0]-width/2,y:Math.min(p0[1],p1[1]),width,height:Math.max(1,Math.abs(p1[1]-p0[1])),r:2},enterFrom:{shape:{y:p0[1],height:0}},transition:['shape'],style:{fill:point.step==='start'||point.step==='total'?theme.accent:point.value>=0?theme.positive:theme.negative}};
                if(!params.dataIndex||point.step==='start')return bar;
                const previous=api.coord([params.dataIndex-1,ordered[params.dataIndex-1].interval[1]]);
                return {type:'group',children:[{type:'line',shape:{x1:previous[0]+width/2,y1:previous[1],x2:p0[0]-width/2,y2:previous[1]},style:{stroke:theme.border,lineWidth:1}},bar]};
            },data:ordered.map((p,i)=>[i,p.interval[0],p.interval[1]]),encode:{x:0,y:[1,2]}}];pointSeries.push(ordered);
        }else for(const name of names){
            const index=seriesNames.indexOf(name),seriesPoints=categories.map(x=>points.find(p=>p.name===name&&p.x===x)||{name,x,value:null,rows:[]}),line=['line','area','stackedArea'].includes(type);
            const item={name,type:line?'line':'bar',...(stacked?{stack:'total',stackStrategy:'samesign'}:{}),itemStyle:{color:color(name,index),...(line?{}:{borderRadius:stacked?0:3})},
                ...(line?{showSymbol:true,symbolSize:7,smooth:spec.smooth===true,connectNulls:false,lineStyle:{width:2.5,color:color(name,index)},...(type!=='line'?{areaStyle:{opacity:stacked?.65:.18,color:color(name,index)}}:{})}:{barMaxWidth:48}),
                data:seriesPoints.map(p=>spec.xType==='category'?p.value==null?'-':p.value:horizontal?[p.value==null?'-':p.value,p.x]:[p.x,p.value==null?'-':p.value]),emphasis:{focus:'none'}};
            option.series.push(item);pointSeries.push(seriesPoints);
        }
    }else if(type==='scatter'||type==='bubble'){
        option.xAxis={...axis('value'),min:null,max:null};option.yAxis=axis();if(range){option.xAxis.min=range[0];option.xAxis.max=range[1];}
        option.series=names.map((name,index)=>{
            const selected=points.filter(p=>p.name===name&&p.value!=null&&p.x!=null);
            if(selected.some(p=>typeof p.x!=='number'||type==='bubble'&&(!Number.isFinite(p.size)||p.size<0)))throw TypeError('Scatter needs numeric x; bubble also needs nonnegative size');
            pointSeries.push(selected);const maxSize=Math.max(1,...selected.map(p=>p.size||0));
            return {name,type:'scatter',symbolSize:type==='bubble'?value=>Math.max(4,36*Math.sqrt(value[2]/maxSize)):9,itemStyle:{color:color(name,seriesNames.indexOf(name)),opacity:.8},data:selected.map(p=>[p.x,p.value,p.size||0])};
        });
    }else if(type==='heatmap'){
        if(!spec.series||Array.isArray(spec.y))throw TypeError('Heatmap requires x, series (second dimension) and one y measure');
        option.xAxis={...axis('category'),data:categories.map(String),splitArea:{show:false}};option.yAxis={...axis('category'),data:names,splitArea:{show:false}};
        const selected=points.filter(p=>names.includes(p.name)&&p.value!=null),values=selected.map(p=>p.value);
        option.visualMap={show:false,min:Math.min(...values,0),max:Math.max(...values,1),inRange:{color:[theme.grid,theme.accent]}};
        option.series=[{type:'heatmap',data:selected.map(p=>[categories.indexOf(p.x),names.indexOf(p.name),p.value]),itemStyle:{borderWidth:2,borderColor:theme.panel},label:{show:false}}];pointSeries.push(selected);
    }else if(type==='donut'||type==='funnel'){
        if(seriesNames.length>1)throw TypeError('Donut and funnel require one measure without series');
        const selected=points.filter(p=>p.value!=null);
        if(selected.some(p=>p.value<0))throw RangeError('Donut and funnel require nonnegative values');
        legend.length=0;selected.forEach((p,i)=>legend.push({name:String(p.x),color:color(String(p.x),i)}));
        const visible=selected.filter(p=>!hidden.has(String(p.x)));
        option.series=[{type:type==='donut'?'pie':'funnel',...(type==='donut'?{radius:['48%','76%'],center:['50%','50%']}:{sort:'none',left:'18%',width:'64%',top:16,bottom:16,gap:3}),
            label:{show:false},itemStyle:{borderColor:theme.panel,borderWidth:3,borderRadius:4},data:visible.map(p=>({name:String(p.x),value:p.value,itemStyle:{color:color(String(p.x),selected.indexOf(p))}}))}];pointSeries.push(visible);
    }
    return {option,pointSeries,legend,empty:!pointSeries.some(points=>points.some(p=>p.value!=null))};
}
