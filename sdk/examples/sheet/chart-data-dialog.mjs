import { Button, CheckBox, Column, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';
import { parseSortRange, sortRangeText } from './sort-dialog.mjs';

export function createChartDataDialog({chart,sheets,onApply,onClose,isCurrent=()=>true,seriesNames={}}) {
    if(['scatter','bubble','stock'].includes(chart.kind))return createNumericDataDialog({chart,sheets,onApply,onClose,isCurrent,seriesNames});
    const ranges=[chart.categories,...chart.series.map(s=>s.values),...chart.series.flatMap(s=>s.name.kind==='cell'?[s.name.reference]:[])];
    const local=ranges.filter(r=>r.sheetId===chart.categories.sheetId);
    const initial={firstRow:Math.min(...local.map(r=>r.firstRow)),lastRow:Math.max(...local.map(r=>r.lastRow)),firstColumn:Math.min(...local.map(r=>r.firstColumn)),lastColumn:Math.max(...local.map(r=>r.lastColumn))};
    let source=String(chart.categories.sheetId),closed=false,busy=false;
    const worksheet=new Menu({name:'chart-data-sheet',text:sheets.find(s=>s.id===source)?.name||'Worksheet'});
    for(const sheet of sheets){const item=new MenuItem({text:sheet.name,checked:sheet.id===source});item.on(Message.ACTIVATE,()=>{source=sheet.id;worksheet.text=sheet.name;for(const option of worksheet.children)option.setProperty('checked',option===item);});worksheet.append(item);}
    const range=new TextField({name:'chart-data-range',value:sortRangeText(initial),accessibleName:'Chart data range'});
    const rows=new CheckBox({name:'chart-data-rows',text:'Series in rows',checked:chart.categories.firstRow===chart.categories.lastRow&&chart.categories.firstColumn!==chart.categories.lastColumn});
    const headers=new CheckBox({name:'chart-data-headers',text:'First row or column contains series names',checked:chart.series.some(s=>s.name.kind==='cell')});
    const status=new Label({text:''}),cancel=new Button({text:'Cancel'}),ok=new Button({text:'OK',name:'chart-data-apply'});
    const body=new Column({text:'panel:flush:tight-stack'},[new Row({text:'panel:fixed:flush'},[new Label({text:'Worksheet'}),worksheet]),new Label({text:'Data range'}),range,rows,headers,status]);
    const dialog=new Dialog({name:'chart-data-dialog',text:'Select Data',modal:true},[body,new Row({text:'panel:actions'},[cancel,ok])]);
    function close(){if(closed||busy)return;closed=true;onClose();}
    cancel.on(Message.ACTIVATE,close);
    ok.on(Message.ACTIVATE,async()=>{
        if(closed||busy)return;
        if(!isCurrent()){status.text='Chart changed. Reopen Select Data.';return;}
        try {
            const request={action:'chart-draft',sheetId:source,range:parseSortRange(range.getProperty('value')),kind:chart.kind,seriesByRows:rows.getProperty('checked'),headers:headers.getProperty('checked')};
            busy=true;body.setProperty('enabled',false);ok.setProperty('enabled',false);cancel.setProperty('enabled',false);
            if(await onApply(request)!==false){busy=false;close();}
        }catch(error){status.text=String(error.message||error).slice(0,240);}
        finally{busy=false;if(!closed){body.setProperty('enabled',true);ok.setProperty('enabled',true);cancel.setProperty('enabled',true);}}
    });
    const bind=control=>{control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();close();}});for(const child of control.children)bind(child);};bind(dialog);
    dialog.on(Message.DESTROY,()=>{closed=true;});return dialog;
}


/** Numeric role editing sends only a detached document command. It never
 * samples cells or derives X/size values in the presentation worker. */
function createNumericDataDialog({chart,sheets,onApply,onClose,isCurrent,seriesNames}){
 const draft=JSON.parse(JSON.stringify(chart)),stock=chart.kind==='stock',roles=stock?['category','open','high','low','close']:chart.kind==='bubble'?['x','y','size']:['x','y'],sharedRole=stock?'category':'x',labels={x:'X values (all series)',y:'Y values',size:'Bubble sizes',category:'Categories',open:'Open (optional)',high:'High',low:'Low',close:'Close'},fields=new Map(),controls=new Map();let current=draft.series[0]?.id,closed=false,busy=false;
 if(!current||draft.series.length>24)throw new TypeError('Invalid numeric chart series');
 const resolvedName=series=>series.name.kind==='literal'?series.name.text:seriesNames instanceof Map?seriesNames.get(series.id)||`Series ${draft.series.indexOf(series)+1}`:seriesNames[series.id]||`Series ${draft.series.indexOf(series)+1}`;
 for(const series of draft.series){const record={name:resolvedName(series),nameChanged:false,roles:{}};for(const role of roles){const range=['y','close'].includes(role)?series.values:role===sharedRole?draft.categories:series.bindings?.[role];record.roles[role]={sheetId:String(range?.sheetId??series.values.sheetId),range:range?sortRangeText(range):''};}fields.set(series.id,record);}
 const chooser=new Menu({name:'chart-data-series',text:'Series',state:State.START_ALIGNED|State.TRAILING_ICON}),name=new TextField({name:'chart-data-series-name',value:'',accessibleName:'Series name'}),status=new Label({name:'chart-data-status',text:''}),cancel=new Button({name:'chart-data-cancel',text:'Cancel'}),ok=new Button({name:'chart-data-apply',text:'OK'});
 const body=new Column({text:'panel:flush:tight-stack'},[new Row({text:'panel:fixed:flush'},[new Label({text:'Series'}),chooser]),new Row({text:'panel:fixed:flush'},[new Label({text:'Name'}),name])]);
 function present(){const record=fields.get(current);chooser.text=record.name;name.setProperty('value',record.name);for(const option of chooser.children)option.setProperty('checked',option.seriesId===current);for(const role of roles){const control=controls.get(role),value=record.roles[role];control.range.setProperty('value',value.range);control.sheet.text=sheets.find(s=>String(s.id)===value.sheetId)?.name||'Worksheet';for(const option of control.sheet.children)option.setProperty('checked',option.sheetId===value.sheetId);}}
 name.on(Message.VALUE_CHANGED,()=>{const record=fields.get(current);record.name=name.getProperty('value');record.nameChanged=true;chooser.text=record.name;const item=chooser.children.find(child=>child.seriesId===current);if(item)item.text=record.name;});
 for(const role of roles){const sheet=new Menu({name:`chart-data-${role}-sheet`,text:'Worksheet',state:State.START_ALIGNED|State.TRAILING_ICON}),range=new TextField({name:`chart-data-${role}-range`,value:'',accessibleName:labels[role]});for(const source of sheets){const item=new MenuItem({name:`chart-data-${role}-sheet-${source.id}`,text:source.name});item.sheetId=String(source.id);item.on(Message.ACTIVATE,()=>{if(role===sharedRole)for(const record of fields.values())record.roles[sharedRole].sheetId=item.sheetId;else fields.get(current).roles[role].sheetId=item.sheetId;present();});sheet.append(item);}range.on(Message.VALUE_CHANGED,()=>{if(role===sharedRole)for(const record of fields.values())record.roles[sharedRole].range=range.getProperty('value');else fields.get(current).roles[role].range=range.getProperty('value');});controls.set(role,{sheet,range});body.append(new Row({text:'panel:fixed:flush'},[new Label({text:labels[role]}),sheet,range]));}
 for(const series of draft.series){const item=new MenuItem({name:`chart-data-series-${series.id}`,text:resolvedName(series)});item.seriesId=series.id;item.on(Message.ACTIVATE,()=>{current=series.id;present();});chooser.append(item);}present();body.append(status);
 const dialog=new Dialog({name:'chart-data-dialog',text:'Select Data',modal:true},[body,new Row({text:'panel:actions'},[cancel,ok])]);function close(){if(!closed&&!busy){closed=true;onClose();}}cancel.on(Message.ACTIVATE,close);
 const count=r=>r.lastRow-r.firstRow+r.lastColumn-r.firstColumn+1;
 function parsed(role){if(!/^\d+$/.test(role.sheetId)||!Number.isSafeInteger(Number(role.sheetId))||Number(role.sheetId)>0xffffffff||!sheets.some(s=>String(s.id)===role.sheetId))throw Error('Choose a worksheet.');const range=parseSortRange(role.range);if(range.firstRow!==range.lastRow&&range.firstColumn!==range.lastColumn)throw Error('Each data role needs a single row or column.');return{sheetId:Number(role.sheetId),...range};}
 ok.on(Message.ACTIVATE,async()=>{if(closed||busy)return;if(!isCurrent()){status.text='Chart changed. Reopen Select Data.';return;}try{let length=null;for(const series of draft.series){const record=fields.get(series.id),references=Object.fromEntries(roles.map(role=>[role,stock&&role==='open'&&!record.roles.open.range.trim()?null:parsed(record.roles[role])])),n=count(references[stock?'close':'y']);if(roles.some(role=>references[role]&&count(references[role])!==n)||length!==null&&length!==n||n*draft.series.length>4096)throw Error('All data roles must have matching lengths within 4096 total points.');length=n;series.values=references[stock?'close':'y'];if(stock){if(!references.open&&draft.options?.groups?.find(group=>group.family==='stock')?.options?.style==='open-high-low-close')throw Error('Open values are required by the Open-High-Low-Close style.');series.bindings={...series.bindings,high:references.high,low:references.low};if(references.open)series.bindings.open=references.open;else delete series.bindings.open;}else series.bindings={...series.bindings,x:references.x,...(references.size?{size:references.size}:{})};if(series===draft.series[0])draft.categories=references[sharedRole];if(record.nameChanged){if(new TextEncoder().encode(record.name).length>256||/[\u0000-\u001f]/u.test(record.name))throw Error('Series names must be plain text up to 256 bytes.');series.name={kind:'literal',text:record.name};}}busy=true;body.setProperty('enabled',false);ok.setProperty('enabled',false);cancel.setProperty('enabled',false);if(await onApply({type:'update-chart',chart:JSON.parse(JSON.stringify(draft))})!==false){busy=false;close();}}catch(error){status.text=String(error.message||error).slice(0,240);}finally{busy=false;if(!closed){body.setProperty('enabled',true);ok.setProperty('enabled',true);cancel.setProperty('enabled',true);}}});
 const bind=control=>{control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();close();}});for(const child of control.children)bind(child);};bind(dialog);dialog.on(Message.DESTROY,()=>{closed=true;});return dialog;
}
