import {Button, Container, Column, Label, Message, Row, ScrollView, State, Table, Tabs,
    createChart, resolveChartTheme} from '@ricewm/app';

const flat = State.ENABLED | State.FLAT;
const label = (text, state=0) => new Label({text,state:State.ENABLED|State.START_ALIGNED|state});
const textCell = value => String(value??'').replace(/[\t\r\n\u0000-\u001f\u007f]/g,' ');
const columns = [
    ['seconds','Time (s)'],['cpu','CPU (%)'],['memoryMiB','Memory (MiB)'],
    ['processes','Processes'],['uploadMs','Upload work (ms)'],['displayMs','Display work (ms)'],
    ['x11Requests','X11 requests'],['instructions','Guest instructions'],
];
export function recordingRows(report) {
    if(report?.schema!=='rise-profiler-report'||!Array.isArray(report.samples)||report.samples.length>300)
        throw Error('This file is not a supported performance recording.');
    return report.samples.map((sample,index)=>{
        const previous=report.samples[index-1];
        const delta=(value,before)=>previous?Math.max(0,Number(value||0)-Number(before||0)):0;
        return {id:String(index),seconds:Number(sample.elapsed_ms||0)/1000,
            cpu:Number(sample.cpu_percent||0),memoryMiB:Number(sample.memory_bytes||0)/1048576,
            processes:Number(sample.process_count||0),
            uploadMs:delta(sample.graphics?.surface_upload_us,previous?.graphics?.surface_upload_us)/1000,
            displayMs:delta(sample.graphics?.scene_callback_us,previous?.graphics?.scene_callback_us)/1000,
            x11Requests:delta(sample.x11?.requests,previous?.x11?.requests),
            instructions:delta(sample.guest_execution?.total_instructions,previous?.guest_execution?.total_instructions),
        };
    });
}

export async function recordingView({report,app,services,onLive}) {
    const rows=recordingRows(report),charts=[],palette=resolveChartTheme(app.theme||{});
    const duration=(Number(report.capture?.elapsed_ms||0)/1000).toFixed(1);
    const target=report.target?.pid?`${report.target.command||'Process'} · PID ${report.target.pid}`:'Everything in this session';
    const status=label('Select a point to inspect a moment in the recording.');
    const table=new Table({name:'recording-samples',rowKey:'id',data:rows,height:360,
        selectionMode:'row',columns:columns.map(([key,title])=>({key,title,width:key==='seconds'?100:150,align:'right'}))});
    const tablePage=new Column({text:'panel:fixed:flush:gap=12',state:flat},[
        label('Recorded samples',State.EMPHASIZED),table]);
    const selected=label('',State.EMPHASIZED);
    const chart = async (id,title,y,unit,height=250)=>{
        const measures={cpu:'CPU',memoryMiB:'Memory',uploadMs:'Upload',displayMs:'Compositor'};
        const chartRows=rows.map(row=>({...row,'Time (s)':row.seconds,CPU:row.cpu,Memory:row.memoryMiB,Upload:row.uploadMs,Compositor:row.displayMs}));
        const result=await createChart({id,name:id,data:chartRows,spec:{type:'line',x:'Time (s)',xType:'value',y:Array.isArray(y)?y.map(key=>measures[key]):measures[y],title,unit},
            height,animation:false,theme:palette,text:'panel:fixed:flush:gap=4',state:flat});
        result.surface.setProperty('backgroundColor',palette.panel);
        result.on(Message.SELECT,event=>{
            const index=event.detail?.dataIndex??event.detail?.point?.dataIndex;
            const point=rows[index];
            if(point)selected.text=`${point.seconds.toFixed(1)} s · CPU ${point.cpu.toFixed(0)}% · Memory ${point.memoryMiB.toFixed(1)} MiB`;
        });
        charts.push(result);return result;
    };
    let content;
    if(rows.length){
        const cpu=await chart('recording-cpu','CPU over time','cpu','%');
        const memory=await chart('recording-memory','Memory over time','memoryMiB','MiB');
        const paint=await chart('recording-display','Display work per sample',['uploadMs','displayMs'],'ms',240);
        const peakCpu=rows.reduce((peak,row)=>row.cpu>peak.cpu?row:peak,rows[0]);
        const peakMemory=Math.max(...rows.map(row=>row.memoryMiB));
        const pair=new Column({text:'panel:fixed:flush:gap=16',state:flat});
        const responsive=new Row({text:'panel:equal',state:flat},[cpu,memory]);pair.append(responsive);
        content=new Column({text:'panel:fixed:flush:gap=16',state:flat},[
            label(`Peak CPU ${peakCpu.cpu.toFixed(0)}% at ${peakCpu.seconds.toFixed(1)} s · Peak memory ${peakMemory.toFixed(1)} MiB`,State.EMPHASIZED),
            pair,paint,selected]);
        content.setCompact=compact=>{
            if(pair.compact===compact)return;pair.compact=compact;
            const old=pair.children[0];for(const child of [...old.children])old.remove(child);
            pair.remove(old,{destroy:true});pair.append(new (compact?Column:Row)({text:compact?'panel:fixed:flush:gap=16':'panel:equal',state:flat},[cpu,memory]));
        };
    }else content=new Column({text:'panel:fixed:flush:gap=12',state:flat},[
        label('No samples were recorded',State.EMPHASIZED),label('Return to Profiler and make a slightly longer recording.')]);
    const windows=Array.isArray(report.wine_execution_profiles)?report.wine_execution_profiles:[];
    const abi=new Column({text:'panel:fixed:flush:gap=12',state:flat},[
        label('Win32 / ABI execution',State.EMPHASIZED),
        label(windows.length?'Execution profiles from this recording.':'No Win32 execution was observed in this recording.'),
    ]);
    for(const execution of windows){
        const effectiveness=execution.aot_effectiveness||{};
        abi.append(label(execution.windows_image||execution.program||`PID ${execution.pid}`,State.EMPHASIZED));
        abi.append(label(`Execution ${execution.execution_ordinal} · ${(Number(effectiveness.optimizer_attribution_share_ppm||0)/10000).toFixed(2)}% optimizer attribution · ${Number(execution.mapped_regions||0)} mapped regions`));
        if(execution.hot_regions?.length)abi.append(new Table({rowKey:'rip',height:240,
            data:execution.hot_regions.map(region=>({...region,share:(Number(region.share_ppm||0)/10000).toFixed(2)+'%'})),
            columns:[{key:'module_path',title:'Module',width:340},{key:'module_offset',title:'Offset',width:140},{key:'share',title:'Samples',width:100}]}));
    }
    const body=new Column({text:'panel:fixed:flush:gap=16',state:flat},[content]);
    const tabs=new Tabs({name:'recording-tabs'},['Overview','Samples',...(windows.length?['Win32 / ABI']:[])].map((name,index)=>{
        const button=new Button({text:name,state:State.ENABLED|(index===0?State.SELECTED:0)});
        button.on(Message.ACTIVATE,()=>{
            for(const tab of tabs.children)tab.setProperty('state',State.ENABLED|(tab===button?State.SELECTED:0));
            const old=body.children[0];body.remove(old);
            body.append(name==='Overview'?content:name==='Samples'?tablePage:abi);
        });return button;
    }));
    const exportData=async(sheet,button)=>{
        button.enabled=false;
        try{
            const data=[columns.map(([,title])=>title),...rows.map(row=>columns.map(([key])=>row[key]))];
            if(sheet){
                const csv=data.map(row=>row.map(value=>'"'+textCell(value).replaceAll('"','""')+'"').join(',')).join('\r\n');
                const name=`recording-${Date.now()}.csv`;
                await services.files.write({path:name,data:new TextEncoder().encode(csv)});
                await services.applications.launch({id:'riceapp-org.ricewm.sheet.desktop',paths:[`/root/.local/share/ricewm/app-data/dev.ricewm.performance-sdk/${name}`]});
                status.text='Opened in Rice Sheet.';
            }else{
                await services.clipboard.writeText(data.map(row=>row.map(textCell).join('\t')).join('\n'));
                status.text='Copied recorded samples.';
            }
        }catch(error){status.text=String(error?.message||error);}finally{button.enabled=true;}
    };
    const sheet=new Button({text:'Open in Rice Sheet',enabled:rows.length>0});sheet.on(Message.ACTIVATE,()=>void exportData(true,sheet));
    const copy=new Button({text:'Copy data',enabled:rows.length>0});copy.on(Message.ACTIVATE,()=>void exportData(false,copy));
    const live=new Button({text:'Live monitor'});live.on(Message.ACTIVATE,onLive);
    const contentRoot=new Column({text:'panel:padding=20:gap=12',state:flat},[
        new Column({text:'panel:fixed:flush:gap=4',state:flat},[
            label('Recording overview',State.EMPHASIZED),label(`${target} · ${duration} s · ${rows.length} samples`)]),
        new Row({text:'panel:fixed:flush:gap=12',state:flat},[sheet,copy,live]),
        tabs,new ScrollView({state:State.EXPANSIVE|State.FLAT},[body]),status,
    ]);
    const root=new Container({text:'panel:flush'},[contentRoot]);
    const stopTheme=app.on(Message.THEME_CHANGED,theme=>{
        const palette=resolveChartTheme(theme);
        for(const chart of charts){chart.surface.setProperty('backgroundColor',palette.panel);chart.setTheme(palette);}
    });
    return {root,resize:width=>content.setCompact?.(width<820),destroy(){stopTheme();for(const chart of charts)chart.destroy();}};
}
