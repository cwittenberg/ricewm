import {Button,CheckBox,Column,Dialog,Label,Menu,MenuItem,Message,Row,ScrollView,Stack,State,TextField,WorkbookContracts} from '@ricewm/app';
import {selectionRange} from './editor-actions.mjs';
const {VALIDATION_KINDS,VALIDATION_OPERATORS,checkedDataValidation,formatSheetRanges,parseSheetRanges}=WorkbookContracts;
const labels={'any':'Any value','whole':'Whole number','decimal':'Decimal','list':'List','date':'Date','time':'Time','text-length':'Text length','custom':'Custom'};
const operators={'between':'between','not-between':'not between','equal':'equal to','not-equal':'not equal to','greater-than':'greater than','greater-or-equal':'greater than or equal to','less-than':'less than','less-or-equal':'less than or equal to'};
export function defaultDataValidation(kind='any'){
    const comparison=['whole','decimal','date','time','text-length'].includes(kind);
    return {kind,operator:comparison?'between':null,formula1:kind==='any'?null:kind==='list'?'"Yes,No"':kind==='custom'?'=A1>0':'0',formula2:comparison?'100':null,ignoreBlank:true,inCellDropdown:true,input:{show:true,title:'',message:''},error:{show:true,style:'stop',title:'',message:''}};
}
/** Settings, Input Message and Error Alert are draft-only until OK. */
export function createDataValidationDialog({projection,query,apply,currentRevision,onClose}){
    const selection=selectionRange(projection);if(!selection)throw Error('Select cells first.');
    let revision=projection.revision,draft=defaultDataValidation(),closed=false,acting=true,tab='settings',mixed=false,clearRequested=false;
    const tabs=new Row({text:'panel:fixed:flush'}),content=new Column({name:'validation-authoring-content',text:'panel:flush:tight-stack'}),status=new Label({name:'validation-authoring-status',text:'Loading validation settings…'});
    const target=new TextField({name:'validation-target',value:formatSheetRanges([selection]),accessibleName:'Applies to'});
    const ok=new Button({name:'validation-authoring-ok',text:'OK'}),cancel=new Button({name:'validation-authoring-cancel',text:'Cancel'}),clear=new Button({name:'validation-authoring-clear',text:'Clear All'});
    const body=new Column({text:'panel:fixed:width=520'},[tabs,new Row({text:'panel:fixed:flush'},[new Label({text:'Applies to'}),target]),new ScrollView({text:'viewport:300|0/0'},[content]),status,new Row({text:'panel:actions'},[clear,cancel,ok])]);
    const dialog=new Dialog({name:'validation-authoring-dialog',text:'Data Validation',modal:true},[body]);
    function close(){if(closed||acting)return;closed=true;onClose();}
    function report(error){status.text=String(error.message||error).slice(0,240);}
    function busy(value){acting=value;for(const c of [tabs,target,content,clear,cancel,ok])c.setProperty('enabled',!value);}
    function field(name,label,value,change){const control=new TextField({name:'validation-'+name,value:String(value??''),accessibleName:label});control.on(Message.VALUE_CHANGED,()=>{clearRequested=false;change(control.getProperty('value'));});content.append(new Row({text:'panel:fixed:flush'},[new Label({text:label}),control]));return control;}
    function toggle(name,label,value,change){const c=new CheckBox({name:'validation-'+name,text:label,checked:value});c.on(Message.ACTIVATE,()=>{clearRequested=false;change(Boolean(c.getProperty('checked')));});content.append(c);}
    function choice(name,label,value,options,change){const control=new Button({name:'validation-'+name,text:options.find(([v])=>v===value)?.[1]||String(value)}),menu=new Menu({name:'validation-'+name+'-menu'});for(const [v,text]of options){const item=new MenuItem({name:`validation-${name}-${v}`,text,checked:v===value});item.on(Message.ACTIVATE,()=>{clearRequested=false;change(v);render();});menu.append(item);}control.append(menu);content.append(new Row({text:'panel:fixed:flush'},[new Label({text:label}),control]));}
    function render(){
        for(const c of [...content.children])content.remove(c,{destroy:true});
        for(const b of tabs.children)b.setProperty('checked',b.name==='validation-tab-'+tab);
        if(tab==='settings'){
            choice('kind','Allow',draft.kind,VALIDATION_KINDS.map(k=>[k,labels[k]]),kind=>{const prior=draft;draft={...defaultDataValidation(kind),ignoreBlank:prior.ignoreBlank,input:prior.input,error:prior.error,...(Object.hasOwn(prior,'sourceRuleIndex')?{sourceRuleIndex:prior.sourceRuleIndex}:{})};mixed=false;});
            toggle('ignore-blank','Ignore blank',draft.ignoreBlank,v=>{draft.ignoreBlank=v;});
            if(draft.kind==='list'){
                toggle('in-cell-dropdown','In-cell dropdown',draft.inCellDropdown,v=>{draft.inCellDropdown=v;});
                const f=draft.formula1,value=f.startsWith('"')&&f.endsWith('"')?f.slice(1,-1).replaceAll('""','"'):f.startsWith('=')?f:'='+f;
                field('source','Source',value,v=>{draft.formula1=v.startsWith('=')?v:'"'+v.replaceAll('"','""')+'"';});
                content.append(new Label({text:'Enter comma-separated values or a formula beginning with =.'}));
            }else if(draft.kind==='custom')field('formula','Formula',draft.formula1,v=>{draft.formula1=v;});
            else if(draft.kind!=='any'){
                choice('operator','Data',draft.operator,VALIDATION_OPERATORS.map(v=>[v,operators[v]]),v=>{draft.operator=v;draft.formula2=['between','not-between'].includes(v)?draft.formula2??'100':null;});
                field('formula1',['between','not-between'].includes(draft.operator)?'Minimum':'Value',draft.formula1,v=>{draft.formula1=v;});
                if(draft.formula2!==null)field('formula2','Maximum',draft.formula2,v=>{draft.formula2=v;});
                if(['date','time'].includes(draft.kind))content.append(new Label({text:'Use an Excel date/time serial or a formula, such as =DATE(2026,1,1).'}));
            }
            if(mixed)content.append(new Label({text:'This selection contains different validation settings. OK replaces the settings only in the specified cells.'}));
        }else if(tab==='input'){
            toggle('input-show','Show input message when cell is selected',draft.input.show,v=>{draft.input.show=v;});
            field('input-title','Title',draft.input.title,v=>{draft.input.title=v;});field('input-message','Input message',draft.input.message,v=>{draft.input.message=v;});
        }else{
            toggle('error-show','Show error alert after invalid data is entered',draft.error.show,v=>{draft.error.show=v;});
            choice('error-style','Style',draft.error.style,[['stop','Stop'],['warning','Warning'],['information','Information']],v=>{draft.error.style=v;});
            field('error-title','Title',draft.error.title,v=>{draft.error.title=v;});field('error-message','Error message',draft.error.message,v=>{draft.error.message=v;});
        }
    }
    for(const [value,text]of [['settings','Settings'],['input','Input Message'],['error','Error Alert']]){const b=new Button({name:'validation-tab-'+value,text,state:State.START_ALIGNED});b.on(Message.ACTIVATE,()=>{if(!acting){tab=value;render();}});tabs.append(b);}
    ok.on(Message.ACTIVATE,async()=>{
        if(acting||closed)return;
        let command;try{if(currentRevision()!==revision)throw Error('Workbook changed. Close and reopen Data Validation.');command={type:clearRequested?'clear-data-validations':'set-data-validation',sheetId:projection.activeSheetId,ranges:parseSheetRanges(target.getProperty('value')),...(clearRequested?{}:{rule:checkedDataValidation(draft)})};}catch(error){report(error);return;}
        busy(true);try{const done=await apply(command,revision);if(done&&!closed){busy(false);close();}else status.text='The settings were not changed.';}catch(error){report(error);}finally{if(!closed)busy(false);}
    });
    clear.on(Message.ACTIVATE,()=>{if(acting)return;draft=defaultDataValidation();draft.input.show=false;draft.error.show=false;mixed=false;clearRequested=true;render();status.text='Validation will be cleared when you choose OK.';});
    cancel.on(Message.ACTIVATE,close);dialog.on(Message.KEY,e=>{if(e.detail?.pressed&&(e.detail.key==='Escape'||e.detail.keyCode===9)){e.preventDefault();close();}});dialog.on(Message.DESTROY,()=>{closed=true;});
    busy(true);render();
    void(async()=>{try{const response=await query({action:'data-validations',sheetId:projection.activeSheetId,range:selection,offset:0,limit:2});if(closed)return;if(currentRevision()!==revision)throw Error('Workbook changed. Close and reopen Data Validation.');const page=response.result??response;mixed=page.total>1;if(page.total===1&&page.rules[0]?.rule&&!page.rules[0].readOnly)draft=structuredClone(page.rules[0].rule);else if(page.total===1)throw Error('This imported rule can be cleared or replaced through the worksheet commands, but cannot be edited in this dialog.');status.text=page.promptsDisabled?'Input messages are disabled for this worksheet.':mixed?'Multiple validation settings selected.':'Changes apply to the specified cells; existing values are kept.';render();busy(false);}catch(error){if(!closed){report(error);cancel.setProperty('enabled',true);acting=false;ok.setProperty('enabled',false);}}})();
    return dialog;
}
