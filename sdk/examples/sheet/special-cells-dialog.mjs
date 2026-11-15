import {Button,CheckBox,Column,Dialog,Label,Message,RadioButton,Row,WorkbookContracts} from '@ricewm/app';
import {selectionRange,rangeAddress} from './editor-actions.mjs';
const {checkedSheetSpecialCellsPlan,checkedSheetRange,checkedSheetCommand,sheetSpecialCellKinds}=WorkbookContracts;
const kinds=[['notes','Notes'],['constants','Constants'],['formulas','Formulas'],['blanks','Blanks'],['current-region','Current region'],['current-array','Current array'],['row-differences','Row differences'],['column-differences','Column differences'],['precedents','Precedents'],['dependents','Dependents'],['last-cell','Last cell'],['visible','Visible cells only'],['conditional-formats','Conditional formats'],['data-validation','Data validation']];
const types=[['numbers','Numbers'],['text','Text'],['logicals','Logicals'],['errors','Errors']];

/** Draft-only native predicate authoring. Counts and preview geometry are
 * native query results; acceptance repeats the predicate in one native command. */
export function createSpecialCellsDialog({projection,isCurrent,query,apply,onPreview,onClose,initialKind='constants'}) {
    let kind=initialKind,ruleMode=initialKind.startsWith('same-')?'same':'all',graphMode=initialKind.startsWith('all-')?'all':'direct',mask=types.map(([value])=>value),closed=false,acting=false,loading=false,generation=0,tail=Promise.resolve(),plan=null,request=null,used;
    const family=()=>kind.replace(/^(same-|all-)/u,''),ruleFamily=()=>['conditional-formats','data-validation'].includes(family());
    const activeGeometry=value=>['current-region','current-array','last-cell'].includes(value);
    const referenceKind=value=>['precedents','all-precedents','dependents','all-dependents'].includes(value);
    const scopes=projection.selection.ranges||[selectionRange(projection)],single=scopes.length===1&&scopes[0].firstRow===scopes[0].lastRow&&scopes[0].firstColumn===scopes[0].lastColumn;
    const status=new Label({name:'special-cells-status',text:''}),scope=new Label({name:'special-cells-scope',text:''});
    const options=new Column({text:'panel:fixed:flush'}),values=new Column({text:'panel:fixed:flush'}),radios=[],checks=[];
    const accept=new Button({name:'special-cells-ok',text:'OK',enabled:false}),cancel=new Button({name:'special-cells-cancel',text:'Cancel'});
    const all=new RadioButton({name:'special-cells-match-all',text:'All',group:'special-cells-match',checked:true}),same=new RadioButton({name:'special-cells-match-same',text:'Same',group:'special-cells-match'});
    values.append(all);values.append(same);
    const direct=new RadioButton({name:'special-cells-level-direct',text:'Direct only',group:'special-cells-level',checked:true}),levels=new RadioButton({name:'special-cells-level-all',text:'All levels',group:'special-cells-level'});values.append(direct);values.append(levels);
    const dialog=new Dialog({name:'special-cells-dialog',text:'Go To Special',modal:true},[new Column({text:'panel:fixed:width=420'},[scope,new Row({text:'panel:fixed'},[options,values]),status,new Row({text:'panel:actions'},[cancel,accept])])]);
    const update=()=>{
        for(const [button,value] of radios){button.setProperty('checked',family()===value);button.setProperty('enabled',!acting);}
        for(const [button,value] of checks){button.setProperty('checked',mask.includes(value));button.setProperty('enabled',!acting&&['constants','formulas'].includes(kind));}
        accept.setProperty('enabled',!acting&&!loading&&Boolean(plan?.ranges.length));cancel.setProperty('enabled',!acting);
        for(const [button,value]of [[all,'all'],[same,'same']]){button.setProperty('checked',ruleMode===value);button.setProperty('enabled',!acting&&ruleFamily());}
        for(const [button,value]of [[direct,'direct'],[levels,'all']]){button.setProperty('checked',graphMode===value);button.setProperty('enabled',!acting&&referenceKind(kind));}
    };
    const stop=()=>{closed=true;generation++;};
    const close=()=>{if(!closed&&!acting){stop();onClose();}};
    async function scopeFor(selectedKind){
        if(activeGeometry(selectedKind)||referenceKind(selectedKind))return [{firstRow:0,lastRow:1048575,firstColumn:0,lastColumn:16383}];
        if(!single)return scopes;
        if(!['constants','formulas','blanks','row-differences','column-differences'].includes(selectedKind))return [{firstRow:0,lastRow:1048575,firstColumn:0,lastColumn:16383}];
        if(used===undefined){
            const response=await query({action:'used-range',sheetId:projection.activeSheetId});
            if(response?.revision!==projection.revision||response.result?.sheetId!==projection.activeSheetId)throw Error('Workbook changed. Reopen Go To Special.');
            const {firstRow,lastRow,firstColumn,lastColumn}=response.result;
            if([firstRow,lastRow,firstColumn,lastColumn].every(value=>value===null))used=null;
            else used=checkedSheetRange({firstRow,lastRow,firstColumn,lastColumn});
        }return used?[used]:[];
    }
    function refresh(){
        if(closed||acting)return;
        const token=++generation,selectedKind=kind,selectedTypes=['constants','formulas'].includes(kind)?[...mask]:[];
        loading=true;plan=request=null;status.text='Finding cells…';scope.text=referenceKind(kind)?`Source: ${scopes.length>1?scopes.length+' selected areas':rangeAddress(scopes[0])}; results on this worksheet`:activeGeometry(kind)?(kind==='last-cell'?'Scope: Worksheet':kind==='current-array'?'Scope: Array containing the active cell':'Scope: Region containing the active cell'):single?(!['constants','formulas','blanks','row-differences','column-differences'].includes(kind)?'Scope: Worksheet':'Scope: Used area of worksheet'):`Scope: ${scopes.length>1?scopes.length+' selected areas':rangeAddress(scopes[0])}`;onPreview?.(null);update();
        tail=tail.catch(()=>{}).then(async()=>{
            if(closed||token!==generation)return;
            try{
                if(!isCurrent())throw Error('Workbook changed. Reopen Go To Special.');
                if(['constants','formulas'].includes(selectedKind)&&!selectedTypes.length){status.text='Choose at least one value type.';return;}
                const ranges=await scopeFor(selectedKind);if(closed||token!==generation)return;
                if(!isCurrent())throw Error('Workbook changed. Reopen Go To Special.');
                if(!ranges.length){status.text='No cells were found.';return;}
                const next={action:'special-cells',sheetId:projection.activeSheetId,ranges,kind:selectedKind,types:selectedTypes};
                const response=await query(next);if(closed||token!==generation)return;
                if(!isCurrent())throw Error('Workbook changed. Reopen Go To Special.');
                plan=checkedSheetSpecialCellsPlan(response,next,projection.revision);request=next;
                status.text=plan.ranges.length?`${plan.cellCount} cells in ${plan.ranges.length} areas`:'No cells were found.';onPreview?.(plan);
            }catch(error){if(!closed&&token===generation){plan=request=null;status.text=String(error.message||error).slice(0,240);}}
            finally{if(!closed&&token===generation){loading=false;update();}}
        });
    }
    for(const [value,text] of kinds){
        if(!sheetSpecialCellKinds(projection.capabilities).includes(value))continue;
        const button=new RadioButton({name:'special-cells-kind-'+value,text,group:'special-cells-kind'});radios.push([button,value]);options.append(button);
        button.on(Message.ACTIVATE,()=>{if(!acting){kind=referenceKind(value)?(graphMode==='all'?'all-'+value:value):ruleMode==='same'&&['conditional-formats','data-validation'].includes(value)?'same-'+value:value;refresh();}});
    }
    for(const [button,value]of [[all,'all'],[same,'same']])button.on(Message.ACTIVATE,()=>{if(!acting&&ruleFamily()){ruleMode=value;kind=value==='same'?'same-'+family():family();refresh();}});
    for(const [button,value]of [[direct,'direct'],[levels,'all']])button.on(Message.ACTIVATE,()=>{if(!acting&&referenceKind(kind)){graphMode=value;kind=value==='all'?'all-'+family():family();refresh();}});
    for(const [value,text] of types){
        const button=new CheckBox({name:'special-cells-type-'+value,text,checked:true});checks.push([button,value]);values.append(button);
        button.on(Message.ACTIVATE,()=>{if(!acting){mask=types.map(([value])=>value).filter(value=>value===button.name.slice('special-cells-type-'.length)?Boolean(button.getProperty('checked')):mask.includes(value));refresh();}});
    }
    const submit=async()=>{
        if(closed||acting||loading||!request||!plan?.ranges.length)return;
        if(!isCurrent()){status.text='Workbook changed. Reopen Go To Special.';return;}
        const {action,...data}=request;const command=checkedSheetCommand({type:'select-special-cells',...data});acting=true;update();
        try{if(await apply(command,projection.revision)){acting=false;close();}}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{if(!closed){acting=false;update();}}
    };
    accept.on(Message.ACTIVATE,()=>void submit());cancel.on(Message.ACTIVATE,close);
    dialog.on(Message.KEY,event=>{if(!event.detail?.pressed)return;const key=event.detail.keyCode;if(key===9||event.detail.key==='Escape'){event.preventDefault();close();}else if([36,104].includes(key)||event.detail.key==='Enter'){event.preventDefault();void submit();}});
    dialog.on(Message.DESTROY,stop);refresh();return dialog;
}
