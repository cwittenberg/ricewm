import {Button,Column,Dialog,Label,Message,Row,ScrollView,TextField,WorkbookContracts} from '@ricewm/app';
import {checkedSheetCommand} from './command-contract.mjs';
import {selectionRange} from './editor-actions.mjs';
const {formatSheetRanges,parseSheetRanges}=WorkbookContracts;

export function createTableDialog({projection,currentRevision,apply,onClose}){
    const range=selectionRange(projection);if(!range)throw Error('Select the table range first.');
    let closed=false,saving=false;
    const address=new TextField({name:'table-create-range',value:formatSheetRanges([range]),accessibleName:'Table range'});
    const name=new TextField({name:'table-create-name',value:'',accessibleName:'Table name'});
    const status=new Label({name:'table-create-status',text:'The first row is used as column headings.'});
    const ok=new Button({name:'table-create-ok',text:'OK'}),cancel=new Button({name:'table-create-cancel',text:'Cancel'});
    const dialog=new Dialog({name:'table-create-dialog',text:'Create Table',modal:true},[new Column({text:'panel:fixed:width=440'},[
        new Label({text:'Where is the data for your table?'}),address,new Label({text:'Table name (optional)'}),name,status,new Row({text:'panel:actions'},[cancel,ok]),
    ])]);
    const close=()=>{if(closed||saving)return;closed=true;onClose();};
    ok.on(Message.ACTIVATE,async()=>{
        if(closed||saving)return;
        try{
            if(currentRevision()!==projection.revision)throw Error('Workbook changed. Close and reopen Create Table.');
            const ranges=parseSheetRanges(address.getProperty('value'));if(ranges.length!==1)throw Error('Choose one rectangular table range.');
            const value=name.getProperty('value');const command=checkedSheetCommand({type:'create-table',sheetId:projection.activeSheetId,range:ranges[0],name:value||null,hasHeaders:true});
            saving=true;for(const c of [ok,cancel,address,name])c.setProperty('enabled',false);
            if(await apply(command,projection.revision)){saving=false;close();}
        }catch(error){status.text=String(error.message||error).slice(0,240);}
        finally{if(!closed){saving=false;for(const c of [ok,cancel,address,name])c.setProperty('enabled',true);}}
    });
    cancel.on(Message.ACTIVATE,close);dialog.on(Message.KEY,e=>{if(e.detail?.pressed&&(e.detail.key==='Escape'||e.detail.keyCode===9)){e.preventDefault();close();}});dialog.on(Message.DESTROY,()=>{closed=true;});
    return dialog;
}

/** Only bounded table identities/headings cross into this draft editor. */
export function createTablePropertiesDialog({projection,currentRevision,query,apply,onClose}){
    let closed=false,loading=false,saving=false,loaded=false,selected=null,tableOffset=0,tableNext=null,columnOffset=0,columnNext=null;
    const tableHistory=[],columnHistory=[],draft=new Map();
    const choices=new Column({name:'table-choices',text:'panel:flush:tight-stack'}),fields=new Column({name:'table-column-fields',text:'panel:flush:tight-stack'});
    const name=new TextField({name:'table-properties-name',value:'',accessibleName:'Table name'}),status=new Label({name:'table-properties-status',text:'Loading tables…'});
    const tablesPrevious=new Button({name:'table-previous',text:'Previous tables'}),tablesNext=new Button({name:'table-next',text:'More tables'});
    const columnsPrevious=new Button({name:'table-columns-previous',text:'Previous columns'}),columnsNext=new Button({name:'table-columns-next',text:'More columns'});
    const ok=new Button({name:'table-properties-ok',text:'OK'}),cancel=new Button({name:'table-properties-cancel',text:'Cancel'});
    const dialog=new Dialog({name:'table-properties-dialog',text:'Table Properties',modal:true},[new Column({text:'panel:fixed:width=520'},[
        new ScrollView({text:'viewport:110|0/0'},[choices]),new Row({text:'panel:fixed:flush'},[tablesPrevious,tablesNext]),
        new Row({text:'panel:fixed:flush'},[new Label({text:'Table name'}),name]),
        new ScrollView({text:'viewport:240|0/0'},[fields]),new Row({text:'panel:fixed:flush'},[columnsPrevious,columnsNext]),status,new Row({text:'panel:actions'},[cancel,ok]),
    ])]);
    function enabled(){const ready=!loading&&!saving;name.setProperty('enabled',ready&&loaded&&Boolean(selected));ok.setProperty('enabled',ready&&loaded&&Boolean(selected));cancel.setProperty('enabled',!saving);choices.setProperty('enabled',ready);fields.setProperty('enabled',ready);tablesPrevious.setProperty('enabled',ready&&tableHistory.length>0);tablesNext.setProperty('enabled',ready&&tableNext!==null);columnsPrevious.setProperty('enabled',ready&&columnHistory.length>0);columnsNext.setProperty('enabled',ready&&columnNext!==null);}
    function current(){if(currentRevision()!==projection.revision)throw Error('Workbook changed. Close and reopen Table Properties.');}
    function page(response){current();if(response?.revision!==projection.revision||!response.result)throw Error('Table information is out of date. Reopen Table Properties.');return response.result;}
    function clear(container){for(const c of [...container.children])container.remove(c,{destroy:true});}
    function report(error){status.text=String(error.message||error).slice(0,240);}
    function close(){if(closed||saving)return;closed=true;onClose();}
    async function columns(offset){
        const result=page(await query({action:'table-columns',table:selected.table,offset,limit:32}));if(closed)return;
        if(result.table!==selected.table||result.name!==selected.name||!Array.isArray(result.columns)||result.columns.length>32||result.nextOffset!==null&&(!Number.isInteger(result.nextOffset)||result.nextOffset<=offset||result.nextOffset>16384))throw Error('Invalid table column page.');
        if(!Number.isInteger(result.total)||result.total<offset+result.columns.length||result.total>16384||result.offset!==offset||result.nextOffset!==null&&result.nextOffset!==offset+result.columns.length)throw Error('Invalid table column count.');
        const ids=new Set();for(const c of result.columns){if(!Number.isInteger(c.id)||c.id<1||c.id>0xffffffff||ids.has(c.id)||typeof c.name!=='string'||!c.name.length||[...c.name].length>255)throw Error('Invalid table column identity.');ids.add(c.id);}
        columnOffset=offset;columnNext=result.nextOffset;clear(fields);
        for(const [i,c]of result.columns.entries()){
            if(!Number.isInteger(c.id)||c.id<1||typeof c.name!=='string')throw Error('Invalid table column identity.');
            const input=new TextField({name:'table-column-'+c.id,value:draft.get(c.id)??c.name,accessibleName:'Column '+(offset+i+1)+' heading'});
            input.on(Message.VALUE_CHANGED,()=>{const value=input.getProperty('value');if(value===c.name)draft.delete(c.id);else draft.set(c.id,value);});
            fields.append(new Row({text:'panel:fixed:flush'},[new Label({text:String(offset+i+1)}),input]));
        }
        loaded=true;
        status.text=`${selected.name}: ${result.total} columns. Changes are applied together when you choose OK.`;
    }
    async function choose(table){selected=table;draft.clear();columnHistory.length=0;columnOffset=0;name.setProperty('value',table.name);await columns(0);}
    async function tables(offset){
        const result=page(await query({action:'tables',sheetId:projection.activeSheetId,offset,limit:16,literal:''}));if(closed)return;
        if(!Array.isArray(result.tables)||result.tables.length>16||result.nextOffset!==null&&(!Number.isInteger(result.nextOffset)||result.nextOffset<=offset||result.nextOffset>256))throw Error('Invalid table page.');
        if(!Number.isInteger(result.total)||result.total<offset+result.tables.length||result.total>256||result.offset!==offset||result.nextOffset!==null&&result.nextOffset!==offset+result.tables.length)throw Error('Invalid table count.');
        const keys=new Set();for(const t of result.tables){if(typeof t.table!=='string'||!t.table.length||[...t.table].length>255||keys.has(t.table)||typeof t.name!=='string'||!t.name.length||[...t.name].length>255)throw Error('Invalid table identity.');keys.add(t.table);}
        tableOffset=offset;tableNext=result.nextOffset;clear(choices);
        for(const [i,t]of result.tables.entries()){
            if(typeof t.table!=='string'||typeof t.name!=='string')throw Error('Invalid table identity.');
            const b=new Button({name:'table-select-'+i,text:t.name});b.on(Message.ACTIVATE,()=>run(()=>choose(t)));choices.append(b);
        }
        if(!result.tables.length){selected=null;columnNext=null;columnHistory.length=0;clear(fields);name.setProperty('value','');status.text='No tables on this worksheet.';return;}
        const range=selectionRange(projection);const active=result.tables.find(t=>t.range&&range&&t.range.firstRow<=range.firstRow&&t.range.lastRow>=range.firstRow&&t.range.firstColumn<=range.firstColumn&&t.range.lastColumn>=range.firstColumn);
        await choose(active??result.tables[0]);
    }
    async function run(work){if(loading||saving||closed)return;loading=true;loaded=false;enabled();try{current();await work();}catch(error){if(!closed)report(error);}finally{loading=false;if(!closed)enabled();}}
    tablesNext.on(Message.ACTIVATE,()=>{if(tableNext!==null)return run(async()=>{tableHistory.push(tableOffset);await tables(tableNext);});});
    tablesPrevious.on(Message.ACTIVATE,()=>{if(tableHistory.length)return run(()=>tables(tableHistory.pop()));});
    columnsNext.on(Message.ACTIVATE,()=>{if(columnNext!==null)return run(async()=>{columnHistory.push(columnOffset);await columns(columnNext);});});
    columnsPrevious.on(Message.ACTIVATE,()=>{if(columnHistory.length)return run(()=>columns(columnHistory.pop()));});
    ok.on(Message.ACTIVATE,async()=>{
        if(closed||loading||saving||!loaded||!selected)return;
        try{current();const value=name.getProperty('value');const command=checkedSheetCommand({type:'rename-table',table:selected.table,name:value===selected.name?null:value,columns:[...draft].map(([id,name])=>({id,name}))});
            saving=true;enabled();if(await apply(command,projection.revision)){saving=false;close();}
        }catch(error){report(error);}finally{if(!closed){saving=false;enabled();}}
    });
    cancel.on(Message.ACTIVATE,close);dialog.on(Message.KEY,e=>{if(e.detail?.pressed&&(e.detail.key==='Escape'||e.detail.keyCode===9)){e.preventDefault();close();}});dialog.on(Message.DESTROY,()=>{closed=true;});
    enabled();void run(()=>tables(0));return dialog;
}
