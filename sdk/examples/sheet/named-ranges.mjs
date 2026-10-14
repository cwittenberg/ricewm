import {Button,Column,Dialog,Label,Menu,MenuItem,Message,Row,ScrollBar,Stack,State,Table,TextField,VirtualGrid} from '@ricewm/app';
import {cellAddress,selectionRange} from './editor-actions.mjs';

const PAGE=100;
const quoted=value=>`'${value.replaceAll("'","''")}'`;
export function defaultNamedReference(projection){
    const range=selectionRange(projection),sheet=projection.sheets.find(s=>s.id===projection.activeSheetId);
    if(!range||!sheet)throw Error('Select a worksheet range first.');
    const absolute=(row,column)=>cellAddress(row,column).replace(/^([A-Z]+)(\d+)$/u,'$$$1$$$2');
    const first=absolute(range.firstRow,range.firstColumn),last=absolute(range.lastRow,range.lastColumn);
    return `=${quoted(sheet.name)}!${first}${first===last?'':':'+last}`;
}
export function nameFormulaToken(item,activeSheetId){
    if(typeof item.insertToken!=='string'||!item.insertToken)throw Error(item.insertReason||'This name cannot be inserted in the active sheet.');
    return item.insertToken;
}
export function checkedDefinedNames(response,offset){
    const value=response?.result??response,revision=value?.revision??response?.revision;
    if(!value||!Array.isArray(value.items)||value.items.length>PAGE||value.offset!==offset||!Number.isSafeInteger(value.total)||value.total<offset||value.total>65536||offset+value.items.length>value.total||!Number.isSafeInteger(revision)||revision<0||(!value.items.length&&offset<value.total))throw Error('Invalid defined-name page.');
    const seen=new Set();
    for(const item of value.items){
        if(!item||typeof item.name!=='string'||!item.name.length||Array.from(item.name).length>255||!(item.scopeSheetId===null||typeof item.scopeSheetId==='string'&&item.scopeSheetId.length>0&&item.scopeSheetId.length<=128)||typeof item.scopeLabel!=='string'||item.scopeLabel.length>128||typeof item.formula!=='string'||!item.formula.startsWith('=')||item.formula.length>32768||item.readOnly!==undefined&&typeof item.readOnly!=='boolean')throw Error('Invalid defined name.');
        if(!(item.insertToken===null||typeof item.insertToken==='string'&&item.insertToken.length>0&&item.insertToken.length<=1024)||item.insertReason!==undefined&&item.insertReason!==null&&typeof item.insertReason!=='string')throw Error('Invalid name insertion token.');
        const key=JSON.stringify([item.scopeSheetId,item.name.toUpperCase()]);if(seen.has(key))throw Error('Duplicate defined name.');seen.add(key);
    }
    if(value.nextOffset!==undefined&&value.nextOffset!==null&&value.nextOffset!==offset+value.items.length)throw Error('Invalid name continuation.');
    return {...value,revision,nextOffset:offset+value.items.length<value.total?offset+value.items.length:null};
}

/** Presentation only. Wasm owns names, validation, scope resolution and history. */
export function createNamedRangesDialog({projection,mode='manage',query,apply,currentRevision=()=>projection.revision,onUse,onClose}){
    let closed=false,acting=false,loading=false,generation=0,pending=null,timer=null,items=[],total=0,base=0,top=0,width=616,height=220,selectedIndex=null,revision=projection.revision,selected=null,editing=null,scope=null;
    const search=new TextField({name:'names-search',value:'',focused:mode!=='define',accessibleName:'Search defined names'});
    const table=new VirtualGrid({name:'names-list',accessibleName:'Defined names: Name, Scope, Refers to'});
    const scroll=new ScrollBar({name:'names-scroll',orientation:'vertical',value:0});
    const status=new Label({name:'names-status',text:''});
    const name=new TextField({name:'defined-name',value:'',accessibleName:'Name'}),formula=new TextField({name:'defined-name-formula',value:defaultNamedReference(projection),accessibleName:'Refers to'});
    const scopeMenu=new Menu({name:'defined-name-scope',text:'Workbook',state:State.START_ALIGNED|State.TRAILING_ICON,accessibleName:'Scope'});
    const scopes=[{id:null,name:'Workbook'},...projection.sheets];
    for(const [index,sheet]of scopes.entries()){
        const item=new MenuItem({name:'defined-name-scope-'+index,text:sheet.name,checked:sheet.id===null});
        item.on(Message.ACTIVATE,()=>{if(closed||acting||editing!=='new')return;scope=sheet.id;scopeMenu.text=sheet.name;for(const other of scopeMenu.children)other.setProperty('checked',other===item);});scopeMenu.append(item);
    }
    const create=new Button({name:'names-new',text:'New…',icon:10}),edit=new Button({name:'names-edit',text:'Edit…',icon:69}),remove=new Button({name:'names-delete',text:'Delete',icon:25});
    const use=new Button({name:'names-use',text:'Insert name'}),save=new Button({name:'names-save',text:'OK'}),back=new Button({name:'names-back',text:'Cancel'}),closeButton=new Button({name:'names-close',text:mode==='use'?'Cancel':'Close'});
    const listPanel=new Column({name:'names-browser',text:'panel:flush:tight-stack'},[search,new Table({columnWidths:[300,200,500]},[new Row({text:'panel:fixed:flush'},['Name','Scope','Refers to'].map(text=>new Label({text})))]),new Row({text:'panel:fixed-height:220:flush'},[table,new Stack({text:'panel:fixed:width=16:flush'},[scroll])]),new Row({text:'panel:actions'},mode==='use'?[use]:[create,edit,remove])]);
    const editor=new Column({name:'names-editor',text:'panel:flush:tight-stack',visible:false},[new Label({text:'Name'}),name,new Label({text:'Scope'}),scopeMenu,new Label({text:'Refers to'}),formula]);
    const editorActions=new Row({name:'names-editor-actions',text:'panel:actions',visible:false},[back,save]);
    const confirmation=new Column({name:'names-delete-confirmation',visible:false,text:'panel:flush:tight-stack'},[new Label({name:'names-delete-question',text:''}),new Row({text:'panel:actions'},[new Button({name:'names-delete-cancel',text:'Cancel'}),new Button({name:'names-delete-confirm',text:'Delete'})])]);
    const body=new Stack({name:'names-body',text:'panel:fixed-height:340:flush'},[listPanel,editor,confirmation]);
    const footer=new Row({name:'names-footer',text:'panel:actions'},[closeButton]);
    const dialog=new Dialog({name:'named-ranges-dialog',text:mode==='use'?'Use in Formula':mode==='define'?'Define Name':'Name Manager',modal:true},[new Column({text:'panel:fixed:width=640:flush:tight-stack'},[body,new Stack({text:'panel:fixed-height:40:flush'},[status]),editorActions,footer])]);
    const readonly=item=>item?.readOnly===true||/^_xlnm\./iu.test(item?.name||'');
    function update(){
        for(const control of [search,name,formula,back,closeButton])control.setProperty('enabled',!closed&&!acting);
        create.setProperty('enabled',!closed&&!acting&&!loading);table.setProperty('enabled',!closed&&!acting&&!loading);edit.setProperty('enabled',!closed&&!acting&&!loading&&Boolean(selected)&&!readonly(selected));remove.setProperty('enabled',edit.getProperty('enabled'));
        use.setProperty('enabled',!closed&&!acting&&!loading&&Boolean(selected?.insertToken)&&!readonly(selected));save.setProperty('enabled',!closed&&!acting&&!loading);scopeMenu.setProperty('enabled',!closed&&!acting&&editing==='new');
        paint();
    }
    function close(){if(closed||acting)return;closed=true;generation++;clearTimeout(timer);pending=null;onClose();}
    function showEditor(item=null){
        if(closed||acting||loading||item&&readonly(item))return;editing=item||'new';scope=item?.scopeSheetId??null;
        name.setProperty('value',item?.name||'');formula.setProperty('value',item?.formula||defaultNamedReference(projection));scopeMenu.text=item?.scopeLabel||'Workbook';
        scopeMenu.children.forEach((choice,index)=>choice.setProperty('checked',scopes[index].id===scope));
        body.text='panel:fixed:flush';footer.setProperty('visible',false);
        dialog.text=item?'Edit Name':'Define Name';editorActions.setProperty('visible',true);listPanel.setProperty('visible',false);editor.setProperty('visible',true);confirmation.setProperty('visible',false);status.text='';search.setProperty('focused',false);name.setProperty('focused',true);update();
    }
    function browse(){if(closed||acting)return;if(mode==='define'){close();return;}editing=null;dialog.text=mode==='use'?'Use in Formula':'Name Manager';editorActions.setProperty('visible',false);body.text='panel:fixed-height:340:flush';footer.setProperty('visible',true);editor.setProperty('visible',false);confirmation.setProperty('visible',false);listPanel.setProperty('visible',true);name.setProperty('focused',false);formula.setProperty('focused',false);status.text='';update();}
    const visibleCount=()=>Math.max(1,Math.ceil(height/24));
    function paint(){
        const shown=items.slice(Math.max(0,top-base),Math.max(0,top-base)+visibleCount()),strings=[],cells=[];
        const columns=[0,.3,.5,1].slice(0,3).map((fraction,index)=>{const start=Math.round(width*fraction),end=Math.round(width*[.3,.5,1][index]);return{index,start,extent:end-start,clip_start:start,clip_end:end,frozen:false};});
        const rows=shown.map((item,slot)=>{for(const [column,text]of [item.name,item.scopeLabel,item.formula].entries()){cells.push([slot,column,strings.length,0]);strings.push(Array.from(text).slice(0,256).join(''));}return{index:top+slot,start:slot*24,extent:24,clip_start:slot*24,clip_end:Math.min(height,(slot+1)*24),frozen:false};});
        const selectedSlot=selectedIndex===null?-1:selectedIndex-top,selection=selectedSlot>=0&&selectedSlot<rows.length?{first_row:selectedSlot,last_row:selectedSlot,first_column:0,last_column:2}:null;
        table.setProperty('frame',{version:1,revision,sheet:0,row_count:Math.max(1,total),column_count:3,width,height,row_header:0,column_header:0,rows,columns,cells,strings,styles:[{alignment:0,role:0}],selection,active:selection?[selectedIndex,0]:null});
        scroll.setProperty('value',top/Math.max(1,total-visibleCount()));
    }
    function navigate(value,choose=false){
        if(closed||acting||loading||!total)return;const target=Math.max(0,Math.min(total-1,value));
        if(choose){selectedIndex=target;selected=items[target-base]||null;}
        top=Math.max(0,Math.min(total-visibleCount(),choose?(target<top?target:target>=top+visibleCount()?target-visibleCount()+1:top):target));
        if(top<base||top+Math.min(visibleCount(),total-top)>base+items.length){void load(top,generation);return;}update();
    }
    table.on(Message.SIZE_CHANGED,event=>{const w=Math.round(event.detail?.width),h=Math.round(event.detail?.height);if(w>0&&h>0&&w<=4096&&h<=800&&(w!==width||h!==height)){width=w;height=h;paint();}});
    table.on(Message.POINTER,event=>{const detail=event.detail;if(closed||acting||loading)return;if(detail?.kind==='wheel'){navigate(top+([4,6].includes(detail.button)?-3:3));event.preventDefault();}else if(detail?.kind==='down'&&detail.button===1){const hit=table.hitTest(detail.x,detail.y);if(hit?.row!==null&&Number.isInteger(hit?.row)){selectedIndex=hit.row;selected=items[selectedIndex-base]||null;search.setProperty('focused',false);table.setProperty('focused',true);status.text=readonly(selected)?'Workbook system name.':mode==='use'&&!selected?.insertToken?selected?.insertReason||'This name is not available in this sheet.':selected?.formula||'';update();}}});
    table.on(Message.KEY,event=>{if(!event.detail?.pressed||acting||loading)return;const key=event.detail.keyCode,current=selectedIndex??top;if(key===9){event.preventDefault();close();}else if(key===36){event.preventDefault();if(mode==='use')insert();else if(selected)showEditor(selected);}else if([111,116,110,115,112,117].includes(key)){event.preventDefault();navigate(key===110?0:key===115?total-1:current+(key===111?-1:key===116?1:key===112?-visibleCount():visibleCount()),true);}});
    const sameName=(item,identity)=>item&&identity&&item.scopeSheetId===identity.scopeSheetId&&item.name.toUpperCase()===identity.name.toUpperCase();
    async function load(offset,token,identity=null){
        if(closed||acting||token!==generation)return;if(loading){pending={offset,token,identity};return;}loading=true;update();
        try{const response=checkedDefinedNames(await query({action:'defined-names',offset,limit:PAGE,literal:search.getProperty('value')}),offset);if(closed||token!==generation)return;if(response.revision!==currentRevision())throw Error('Workbook changed. Reopen Name Manager.');
            if(offset!==0&&response.revision!==revision)throw Error('Name list changed. Search again.');
            revision=response.revision;total=response.total;base=offset;items=[...response.items];let continuation=response.nextOffset;
            while(items.length<Math.min(visibleCount(),total-offset)&&continuation!==null){const more=checkedDefinedNames(await query({action:'defined-names',offset:continuation,limit:PAGE,literal:search.getProperty('value')}),continuation);if(closed||token!==generation)return;if(more.revision!==revision||more.total!==total)throw Error('Name list changed. Search again.');items.push(...more.items.slice(0,PAGE-items.length));continuation=more.nextOffset;}
            if(identity){
                // Only post-mutation relocation scans continuations. Each old
                // page is discarded; normal scrolling never reads the full list.
                while(!items.some(item=>sameName(item,identity))&&continuation!==null){
                    const more=checkedDefinedNames(await query({action:'defined-names',offset:continuation,limit:PAGE,literal:search.getProperty('value')}),continuation);
                    if(closed||token!==generation)return;
                    if(more.revision!==revision||more.total!==total)throw Error('Name list changed. Search again.');
                    base=more.offset;items=[...more.items];continuation=more.nextOffset;
                }
                const index=items.findIndex(item=>sameName(item,identity));selectedIndex=index<0?null:base+index;
                top=index<0?base:Math.max(base,selectedIndex-visibleCount()+1);
            }else top=Math.min(offset,Math.max(0,total-1));
            selected=selectedIndex===null?null:items[selectedIndex-base]||null;status.text=total?`${total} name${total===1?'':'s'}`:'No matching names.';
        }catch(error){if(!closed&&token===generation)status.text=String(error.message||error).slice(0,240);}
        finally{loading=false;if(!closed){update();const next=pending;pending=null;if(next&&next.token===generation)void load(next.offset,next.token,next.identity);}}
    }
    async function mutate(command){
        if(closed||acting||loading)return;if(currentRevision()!==revision){status.text='Workbook changed. Reopen Name Manager.';return;}acting=true;update();
        try{const applied=await apply(command,revision);if(closed)return;if(applied){acting=false;if(mode==='define'){close();return;}const identity=command.type==='delete-defined-name'?null:{name:command.newName??command.name,scopeSheetId:command.newScopeSheetId===undefined?command.scopeSheetId:command.newScopeSheetId};selected=null;selectedIndex=null;if(identity&&!identity.name.toLowerCase().includes(search.getProperty('value').toLowerCase()))search.setProperty('value','');browse();generation++;await load(0,generation,identity);}else status.text='The name was not changed.';}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{acting=false;if(!closed)update();}
    }
    function submit(){
        const value=name.getProperty('value').trim(),ref=formula.getProperty('value').trim();
        if(!value||Array.from(value).length>255){status.text='Enter a name of 1–255 characters.';return;}if(!ref||ref.length>32768){status.text='Enter a cell range or LAMBDA formula.';return;}
        void mutate(editing==='new'?{type:'create-defined-name',name:value,scopeSheetId:scope,formula:ref.startsWith('=')?ref:'='+ref}:{type:'update-defined-name',name:editing.name,scopeSheetId:editing.scopeSheetId,newName:value,newScopeSheetId:editing.scopeSheetId,formula:ref.startsWith('=')?ref:'='+ref});
    }
    function insert(){if(closed||acting||loading||!selected||readonly(selected)||!selected.insertToken)return;if(currentRevision()!==revision){status.text='Workbook changed. Reopen Use in Formula.';return;}onUse?.(selected);close();}
    create.on(Message.ACTIVATE,()=>showEditor());edit.on(Message.ACTIVATE,()=>showEditor(selected));use.on(Message.ACTIVATE,insert);save.on(Message.ACTIVATE,submit);back.on(Message.ACTIVATE,browse);closeButton.on(Message.ACTIVATE,close);
    remove.on(Message.ACTIVATE,()=>{if(!selected||readonly(selected)||closed||acting||loading)return;listPanel.setProperty('visible',false);confirmation.setProperty('visible',true);confirmation.find('names-delete-question').text=`Delete ${selected.name}? Formulas using it may return #NAME?.`;});
    confirmation.find('names-delete-cancel').on(Message.ACTIVATE,browse);confirmation.find('names-delete-confirm').on(Message.ACTIVATE,()=>{if(selected&&!readonly(selected))void mutate({type:'delete-defined-name',name:selected.name,scopeSheetId:selected.scopeSheetId});});
    search.on(Message.VALUE_CHANGED,()=>{selected=null;selectedIndex=null;generation++;clearTimeout(timer);update();table.setProperty('enabled',false);if(Array.from(search.getProperty('value')).length>128){status.text='Search supports up to 128 characters.';return;}timer=setTimeout(()=>void load(0,generation),100);});
    scroll.on(Message.VALUE_CHANGED,event=>{const value=Number(event.detail?.value);if(Number.isFinite(value))navigate(Math.round(Math.max(0,Math.min(1,value))*Math.max(0,total-visibleCount())));});
    for(const control of [dialog,search,name,formula,scopeMenu,closeButton,back,save,create,edit,remove,use])control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();if(editing&&mode!=='define')browse();else close();}});
    dialog.on(Message.DESTROY,()=>{closed=true;generation++;clearTimeout(timer);pending=null;});
    if(mode==='define'){showEditor();revision=currentRevision();}else void load(0,generation);
    update();return dialog;
}
