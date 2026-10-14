import {defineApp,Column,Row,Label,Button,Table,ChartDashboard,createChart,Message,State} from '@ricewm/app';
export const rows=Array.from({length:48},(_,i)=>({id:`order-${i+1}`,customer:`Customer ${i+1}`,region:['North','South','West'][i%3],sales:i%7===0?-20:50+i,status:i%7===0?'Refund':'Paid'}));
export const reportSpec={type:'bar',x:'region',y:'sales',title:'Sales by region',unit:' USD'};
export default defineApp({start:async({createWindow})=>{
 const dashboard=new ChartDashboard(rows),summary=new Label({name:'report-summary',text:'48 records'}),selection=new Label({name:'report-selection',text:'No selection'}),detail=new Label({name:'report-detail',text:'Select an order and press Enter to open its details.'});
 const table=new Table({name:'orders',dashboard,id:'orders',rowKey:'id',filterField:'region',height:270,columns:[{key:'id',title:'Order',width:160},{key:'customer',title:'Customer',width:230},{key:'region',title:'Region',width:140},{key:'sales',title:'Sales (USD)',width:160,align:'right',format:v=>v.toFixed(2)},{key:'status',title:'Status',width:140}],conditionalFormats:[{field:'sales',op:'lt',value:0,style:{foreground:'#d14d5b',bold:true}},{field:'status',op:'eq',value:'Refund',scope:'row',style:{italic:true}}]});
 const chart=await createChart({dashboard,id:'regions',spec:reportSpec,height:210});
 dashboard.subscribe(()=>{summary.text=`${dashboard.rows().length} records in report`;});
 table.on(Message.SELECT,e=>{selection.text=e.detail.selection?`${e.detail.selection.kind}: ${e.detail.rowIds.join(', ')}`:'No selection';});
 table.on(Message.ACTIVATE,e=>{detail.text=`Order details: ${e.detail.rowId} · ${e.detail.row.customer} · ${e.detail.row.status}`;});
 const button=(name,caption,action)=>{const b=new Button({name,text:caption});b.on(Message.ACTIVATE,action);return b;};
 createWindow({title:'Sales Report',root:new Column({text:'panel:padding=12:gap=8',state:State.FLAT},[new Label({text:'Sales report',state:State.EMPHASIZED}),summary,new Row({text:'panel:fixed:flush'},[button('reset-report','Reset report',()=>{dashboard.clear();table.clearSelection();}),button('sort-sales','Sort sales',()=>table.sort('sales','desc')),button('sort-order','Order sequence',()=>table.sort('id','asc'))]),table,selection,detail,chart])});
}});
