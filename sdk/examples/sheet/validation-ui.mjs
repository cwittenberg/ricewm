import {Button,Column,Dialog,Label,Message,Popover,Row,ScrollBar,ScrollView,Stack,TextField,VirtualGrid} from '@ricewm/app';

import {checkedValidationList} from './validation-contract.mjs';
export {checkedCellValidation,checkedSelectionValidation,checkedValidationList} from './validation-contract.mjs';

/** Native decision UI only; all rule evaluation remains in the Wasm engine. */
export function createValidationDialog({alerts,onDecision}){
    if(!alerts.length)throw Error('Validation dialog needs an alert.');
    const blocked=alerts.some(alert=>alert.style==='stop'||alert.style==='unsupported');
    const primary=alerts.find(alert=>alert.style==='stop'||alert.style==='unsupported')||alerts[0];
    let settled=false;
    const finish=decision=>{if(settled)return;settled=true;onDecision(decision);};
    const edit=new Button({name:'validation-edit',text:'Edit Value',focused:true});
    const cancel=new Button({name:'validation-cancel',text:'Cancel'});
    const accept=new Button({name:'validation-accept',text:'Keep Value'});
    const severity=new Label({name:'validation-severity',text:'',icon:blocked?53:primary.style==='warning'?52:51,accessibleName:blocked?'Error':primary.style==='warning'?'Warning':'Information'});
    const messages=alerts.map((alert,index)=>new Label({name:'validation-message-'+index,text:(index&&alert.title?alert.title+'\n':'')+(alert.message||'This value does not meet the cell’s validation rule.')}));
    cancel.on(Message.ACTIVATE,()=>finish('cancel'));
    edit.on(Message.ACTIVATE,()=>finish('retry'));
    accept.on(Message.ACTIVATE,()=>{if(!blocked)finish('accept');});
    const dialog=new Dialog({name:'validation-dialog',text:primary.title||(blocked?'Invalid value':'Check this value'),modal:true},[
        new Column({text:'panel:fixed:flush:width=420'},[
            new Row({text:'panel:fixed:flush'},[new Stack({text:'panel:fixed:width=48:flush'},[severity]),new ScrollView({text:'viewport:104|0/0'},[new Column({text:'panel:flush:tight-stack'},messages)])]),
            ...(!blocked?[new Label({text:'Keep this value anyway?'})]:[]),
            new Row({text:'panel:actions'},blocked?[cancel,edit]:[cancel,edit,accept]),
        ]),
    ]);
    for(const control of [dialog,cancel,edit,accept])control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();finish('cancel');}});
    dialog.on(Message.DESTROY,()=>finish('cancel'));
    return dialog;
}

/** Search and scroll retain at most 32 labels. Wasm resolves the selected value. */
export function createValidationList({cell,revision,anchor,query,apply,isCurrent,onClose}){
    let closed=false,acting=false,loading=false,generation=0,pending=null,timer=null,items=[],base=0,total=0,top=0,selected=null,width=288,height=24,navigating=false;
    const search=new TextField({name:'validation-list-search',value:'',accessibleName:'Search allowed values',focused:true});
    const table=new VirtualGrid({name:'validation-list-values',accessibleName:'Allowed values'}),scroll=new ScrollBar({orientation:'vertical',value:0});
    const status=new Label({name:'validation-list-status',text:'Loading…'}),cancel=new Button({name:'validation-list-cancel',text:'Cancel'});
    const scrollHolder=new Stack({name:'validation-list-scroll-holder',text:'panel:fixed:width=16:flush',visible:false},[scroll]);
    const listRow=new Row({name:'validation-list-viewport',text:'panel:fixed-height:24:flush'},[table,scrollHolder]);
    const popup=anchor&&Number.isInteger(anchor.x)&&Number.isInteger(anchor.y),Surface=popup?Popover:Dialog;
    const dialog=new Surface({name:'validation-list-dialog',text:popup?`panel:anchored-popup:${anchor.x}:${anchor.y}:320:152`:'Choose a value',...(popup?{}:{modal:true})},[
        new Column({text:'panel:fixed:flush:tight-stack'},[new Row({text:'panel:fixed:flush'},[new Label({name:'validation-list-search-label',text:'Search'}),search]),listRow,status,new Row({text:'panel:actions'},[cancel])]),
    ]);
    const count=()=>Math.min(32,Math.max(1,Math.ceil(height/24)));
    function update(){
        search.setProperty('enabled',!closed&&!acting);table.setProperty('enabled',!closed&&!acting&&!loading);scroll.setProperty('enabled',!closed&&!acting&&!loading);
        const viewportHeight=Math.max(1,Math.min(8,total))*24;
        listRow.text=`panel:fixed-height:${viewportHeight}:flush`;
        if(popup)dialog.text=`panel:anchored-popup:${anchor.x}:${anchor.y}:320:${viewportHeight+128}`;
        scrollHolder.setProperty('visible',total>8);
        const shown=items.slice(Math.max(0,top-base),Math.max(0,top-base)+count()),rows=shown.map((_,slot)=>({index:top+slot,start:slot*24,extent:24,clip_start:slot*24,clip_end:Math.min(height,(slot+1)*24),frozen:false}));
        const slot=selected===null?-1:selected-top,selection=slot>=0&&slot<rows.length?{first_row:slot,last_row:slot,first_column:0,last_column:0}:null;
        table.setProperty('frame',{version:1,revision,sheet:0,row_count:Math.max(1,total),column_count:1,width,height,row_header:0,column_header:0,rows,columns:[{index:0,start:0,extent:width,clip_start:0,clip_end:width,frozen:false}],cells:shown.map((_,slot)=>[slot,0,slot,0]),strings:shown.map(item=>item.label),styles:[{alignment:0,role:0}],selection,active:selection?[selected,0]:null});
        scroll.setProperty('value',top/Math.max(1,total-count()));
    }
    const retire=()=>{closed=true;generation++;pending=null;clearTimeout(timer);};
    function close(){if(closed||acting)return;retire();onClose();}
    async function load(offset,token){
        if(closed||acting||token!==generation)return;
        if(loading){pending={offset,token};return;}loading=true;update();
        try{
            if(!isCurrent())throw Error('Workbook changed. Reopen the list.');
            const response=await query({action:'validation-list',...cell,offset,limit:32,literal:search.getProperty('value')},{expectedRevision:revision});
            if(closed||token!==generation)return;
            const page=checkedValidationList(response,cell,revision,offset);
            if(!isCurrent())throw Error('Workbook changed. Reopen the list.');
            items=page.items;base=offset;total=page.total;top=offset;
            if(selected===null||selected>=total)selected=items.length?offset:null;
            height=Math.max(1,Math.min(8,total))*24;
            status.text=total?`${total} allowed value${total===1?'':'s'}`:'No matching values.';
        }catch(error){if(!closed&&token===generation){items=[];total=0;selected=null;status.text=String(error.message||error).slice(0,240);}}
        finally{loading=false;if(!closed){update();const next=pending;pending=null;if(next)void load(next.offset,next.token);}}
    }
    function navigate(index,choose=true){
        if(closed||acting||loading||!total)return;
        const target=Math.max(0,Math.min(total-1,index));if(choose)selected=target;
        top=Math.max(0,Math.min(Math.max(0,total-count()),choose?(target<top?target:target>=top+count()?target-count()+1:top):target));
        if(top<base||top+Math.min(count(),total-top)>base+items.length)void load(top,generation);else update();
    }
    async function select(index){
        if(closed||acting||loading)return;const item=items[index-base];if(!item)return;
        if(!isCurrent()){status.text='Workbook changed. Reopen the list.';return;}acting=true;update();
        try{if(await apply({type:'select-validation-value',...cell,itemIndex:item.index},revision)){acting=false;close();}else if(!closed)status.text='The value was not changed.';}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}finally{acting=false;if(!closed)update();}
    }
    table.on(Message.SIZE_CHANGED,event=>{const w=Math.round(event.detail?.width),h=Math.round(event.detail?.height);if(w>0&&h>0&&w<=4096&&h<=768){width=w;height=h;update();}});
    table.on(Message.POINTER,event=>{const d=event.detail;if(d?.kind==='wheel'){event.preventDefault();navigate(top+([4,6].includes(d.button)?-3:3),false);}else if(d?.kind==='down'&&d.button===1){const hit=table.hitTest(d.x,d.y);if(Number.isInteger(hit?.row)){event.preventDefault();void select(hit.row);}}});
    const key=event=>{const d=event.detail;if(!d?.pressed)return;if(d.keyCode===9||d.key==='Escape'){event.preventDefault();close();return;}if(acting||loading)return;
        if(d.keyCode===36){event.preventDefault();if(selected!==null)void select(selected);}
        else if([111,116,110,115,112,117].includes(d.keyCode)){event.preventDefault();search.setProperty('focused',false);table.setProperty('focused',true);const current=selected??top;
            const target=d.keyCode===110?0:d.keyCode===115?total-1:d.keyCode===116&&!navigating?current:current+(d.keyCode===111?-1:d.keyCode===116?1:d.keyCode===112?-count():count());navigating=true;navigate(target);}
    };
    table.on(Message.KEY,key);search.on(Message.KEY,key);dialog.on(Message.KEY,key);cancel.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.keyCode===9||event.detail.key==='Escape')){event.preventDefault();close();}});
    search.on(Message.VALUE_CHANGED,()=>{generation++;clearTimeout(timer);items=[];total=0;top=0;base=0;selected=null;navigating=false;status.text='Searching…';update();const token=generation;if(Array.from(search.getProperty('value')).length>128){status.text='Search supports up to 128 characters.';return;}timer=setTimeout(()=>void load(0,token),80);});
    scroll.on(Message.VALUE_CHANGED,event=>{const value=Number(event.detail?.value);if(Number.isFinite(value))navigate(Math.round(Math.max(0,Math.min(1,value))*Math.max(0,total-count())),false);});
    cancel.on(Message.ACTIVATE,close);dialog.on(Message.POINTER,event=>{if(event.detail?.kind==='cancel')close();});dialog.on(Message.DESTROY,retire);
    void load(0,generation);return dialog;
}
