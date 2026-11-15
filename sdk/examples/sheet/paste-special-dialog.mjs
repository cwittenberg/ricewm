import {Button,CheckBox,Column,Dialog,Label,Message,RadioButton,Row,WorkbookContracts} from '@ricewm/app';
import {selectionRange} from './editor-actions.mjs';
const {SHEET_PASTE_MODES,SHEET_PASTE_OPERATIONS,checkedSheetCommand}=WorkbookContracts;
const labels={'all':'All','formulas':'Formulas','values':'Values','formats':'Formats','validation':'Validation','column-widths':'Column widths','all-except-borders':'All except borders','formulas-and-number-formats':'Formulas and number formats','values-and-number-formats':'Values and number formats'};
/** One draft maps to one native transaction, including repeated destinations. */
export function createPasteSpecialDialog({projection,isCurrent,apply,onClose}){
    const target=selectionRange(projection);if(!target)throw Error('Select a destination first.');
    const options={mode:'all',operation:'none',transpose:false,skipBlanks:false,link:false};let acting=false,closed=false;
    const modeRow=new Row({text:'panel:fixed:flush'}),left=new Column({text:'panel:flush:tight-stack'}),right=new Column({text:'panel:flush:tight-stack'});modeRow.append(left);modeRow.append(right);
    const modes=[],operations=[];
    const cutting=projection.clipboard?.mode==='cut';
    const status=new Label({name:'paste-special-status',text:cutting?'Cut moves the source once with Paste All. Press Escape in the sheet to cancel Cut.':'A single destination expands to the copied size. A larger selection repeats the copied range.'});
    const skip=new CheckBox({name:'paste-special-skip-blanks',text:'Skip blanks'}),transpose=new CheckBox({name:'paste-special-transpose',text:'Transpose'}),link=new CheckBox({name:'paste-special-link',text:'Paste link'});
    const accept=new Button({name:'paste-special-ok',text:'OK'}),cancel=new Button({name:'paste-special-cancel',text:'Cancel'});
    const operationRow=new Row({text:'panel:fixed:flush'});
    const body=new Column({text:'panel:fixed:width=560'},[new Label({text:'Paste'}),modeRow,new Label({text:'Operation'}),operationRow,new Row({text:'panel:fixed:flush'},[skip,transpose,link]),status,new Row({text:'panel:actions'},[cancel,accept])]);
    const dialog=new Dialog({name:'sheet-paste-special-dialog',text:'Paste Special',modal:true},[body]);
    function refresh(){
        const content=['all','values','formulas','all-except-borders','formulas-and-number-formats','values-and-number-formats'].includes(options.mode),linked=['all','formulas','values'].includes(options.mode);
        if(!content||options.link)options.operation='none';if(!linked)options.link=false;
        if(options.mode==='column-widths'){options.transpose=false;options.skipBlanks=false;}
        for(const [button,value]of modes){button.setProperty('checked',options.mode===value);button.setProperty('enabled',!acting&&(!cutting||value==='all'));}
        for(const [button,value]of operations){button.setProperty('checked',options.operation===value);button.setProperty('enabled',!acting&&content&&!options.link&&(!cutting||value==='none'));}
        for(const [button,value]of [[skip,'skipBlanks'],[transpose,'transpose'],[link,'link']]){button.setProperty('checked',options[value]);button.setProperty('enabled',!acting&&!cutting&&(button===link?linked:options.mode!=='column-widths'));}
        modeRow.setProperty('enabled',!acting);accept.setProperty('enabled',!acting);cancel.setProperty('enabled',!acting);
    }
    for(const [index,value]of SHEET_PASTE_MODES.entries()){
        const button=new RadioButton({name:'paste-special-mode-'+value,text:labels[value],group:'paste-mode'});button.on(Message.ACTIVATE,()=>{if(!acting){options.mode=value;refresh();}});modes.push([button,value]);(index<5?left:right).append(button);
    }
    for(const value of SHEET_PASTE_OPERATIONS){const button=new RadioButton({name:'paste-special-operation-'+value,text:value[0].toUpperCase()+value.slice(1),group:'paste-operation'});button.on(Message.ACTIVATE,()=>{if(!acting){options.operation=value;refresh();}});operations.push([button,value]);operationRow.append(button);}
    for(const [button,value]of [[skip,'skipBlanks'],[transpose,'transpose'],[link,'link']])button.on(Message.ACTIVATE,()=>{if(!acting){options[value]=Boolean(button.getProperty('checked'));refresh();}});
    const close=()=>{if(!closed&&!acting){closed=true;onClose();}};
    accept.on(Message.ACTIVATE,async()=>{
        if(acting||closed)return;let command;
        try{if(!isCurrent())throw Error('Workbook changed. Close and reopen Paste Special.');command=checkedSheetCommand({type:'paste-special',sheetId:projection.activeSheetId,target,options,...(projection.clipboard?{clipboardGeneration:projection.clipboard.generation}:{})});}catch(error){status.text=String(error.message||error).slice(0,240);return;}
        acting=true;refresh();try{const done=await apply(command,projection.revision);if(done&&!closed){acting=false;close();}else status.text='The clipboard was not pasted.';}catch(error){status.text=String(error.message||error).slice(0,240);}finally{if(!closed){acting=false;refresh();}}
    });
    cancel.on(Message.ACTIVATE,close);dialog.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();close();}});dialog.on(Message.DESTROY,()=>{closed=true;});refresh();return dialog;
}
