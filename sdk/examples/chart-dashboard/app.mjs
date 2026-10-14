import {defineApp,createChart,recommendChart,ChartDashboard,Button,Column,Row,Label,ScrollView,State,Message,Stack,SceneSurface,SceneOverlay,SceneBuilder,resolveChartTheme} from '@ricewm/app';

export const rows=Array.from({length:6},(_,month)=>['North','South','West'].flatMap((region,index)=>['Online','Retail'].map((channel,c)=>({
    date:`2026-${String(month+1).padStart(2,'0')}-01`,region,channel,
    revenue:14+month*5+index*9+c*6,forecast:18+month*5+index*8+c*4,
})))).flat();
export const regionSpec={type:'stackedBar',x:'region',y:'revenue',series:'channel',title:'Revenue by region',unit:'k'};
export const trendSpec={...recommendChart(rows,{x:'date',y:['revenue','forecast'],intent:'trend'}).spec,title:'Actual and forecast',unit:'k'};
export const ledger=[{stage:'Opening',amount:120,kind:'start'},{stage:'New sales',amount:68,kind:'delta'},{stage:'Expansion',amount:42,kind:'delta'},{stage:'Costs',amount:-37,kind:'delta'},{stage:'Closing',amount:193,kind:'total'}];

export async function dashboardView(){
    const dashboard=new ChartDashboard(rows);
    const regions=await createChart({dashboard,id:'regions',spec:regionSpec,height:220});
    const trend=await createChart({dashboard,id:'trend',spec:trendSpec,height:220,colors:{series:{forecast:'#d79a45'}}});
    const target=await createChart({dashboard,id:'target',spec:{type:'gauge',y:'revenue',min:0,max:1800,target:1400,title:'Revenue target',unit:'k'},height:220});
    const waterfall=await createChart({data:ledger,id:'bridge',spec:{type:'waterfall',x:'stage',y:'amount',step:'kind',title:'Revenue bridge',unit:'k'},height:220});
    const summary=new Label({name:'dashboard-summary',state:State.EMPHASIZED});
    const update=()=>{const selected=dashboard.rows();summary.text=`${selected.length} observations · Revenue ${selected.reduce((sum,row)=>sum+row.revenue,0)}k`;};
    dashboard.subscribe(update);update();
    const reset=new Button({name:'dashboard-reset',text:'Reset filters'});reset.on(Message.ACTIVATE,()=>dashboard.clear());
    let nextPeriod=false;
    const refresh=new Button({name:'dashboard-update',text:'Update data'});
    refresh.on(Message.ACTIVATE,()=>{
        nextPeriod=!nextPeriod;refresh.text=nextPeriod?'Restore data':'Update data';
        dashboard.setData(nextPeriod?rows.map(row=>({...row,revenue:Math.round(row.revenue*1.18),forecast:Math.round(row.forecast*1.12)})):rows);
        waterfall.setData(nextPeriod?ledger.map(row=>({...row,amount:Math.round(row.amount*1.18)})):ledger);
    });
    const body=new Column({text:'panel:fixed:padding=12:gap=12',state:State.FLAT},[
        new Row({text:'panel:equal',state:State.FLAT},[regions,trend]),
        new Row({text:'panel:equal',state:State.FLAT},[target,waterfall]),
    ]);
    const content=new Column({text:'panel:flush:gap=0',state:State.EXPANSIVE|State.FLAT},[
        new Row({text:'panel:padding=12:gap=12'},[summary,refresh,reset]),
        new ScrollView({name:'dashboard-scroll',state:State.EXPANSIVE|State.FLAT},[body]),
    ]);
    // A full native scene backdrop keeps transparent layout panels readable in
    // themes that deliberately use a different window-chrome material.
    const surface=new SceneSurface({name:'dashboard-backdrop',state:State.EXPANSIVE|State.FLAT});
    const root=new Stack({text:'panel:flush',state:State.EXPANSIVE|State.FLAT},[surface,new SceneOverlay({surface,bounds:{x:0,y:0,width:1,height:1},state:State.FLAT},[content])]);
    let width=1100,height=850,palette=resolveChartTheme();
    const paint=()=>{const n=parseInt(palette.background.slice(1),16),color=(0xff000000|((n&255)<<16)|(n&0xff00)|(n>>>16))>>>0;surface.submit(new SceneBuilder(width,height).rect([0,0,width,height],color).finish());};
    surface.on(Message.SIZE_CHANGED,event=>{if(event.detail?.width>0&&event.detail?.height>0){width=event.detail.width;height=event.detail.height;paint();}});
    root.on(Message.CREATED,event=>{if(event.target===root){palette=resolveChartTheme(root.application.theme||{});paint();root.application.on(Message.THEME_CHANGED,value=>{palette=resolveChartTheme(value);paint();});}});
    paint();return root;
}
export default defineApp({async start({createWindow}){createWindow({root:await dashboardView()});}});
