import {Button,CheckBox,Column,Dialog,Label,Menu,MenuItem,Message,Row,ScrollView,Stack,State,Table,TextField} from '@ricewm/app';
import {cellAddress,rangeAddress,selectionRange} from './editor-actions.mjs';

function destination(value){
    const match=/^\$?([A-Z]{1,3})\$?([1-9][0-9]{0,6})$/iu.exec(value.trim());
    if(!match)throw Error('Enter a destination cell such as D2.');
    let column=0;for(const letter of match[1].toUpperCase())column=column*26+letter.charCodeAt(0)-64;
    const row=Number(match[2])-1;if(column>16384||row>=1048576)throw Error('Destination is outside the worksheet.');
    return{row,column:column-1};
}
/** Bounded previews and all data transformations belong to the Wasm engine. */
export function createDataToolsDialog({kind,projection,query,apply,isCurrent,onClose}){
    const split=kind==='text-to-columns',range=selectionRange(projection),sheetId=projection.activeSheetId;
    if(!['remove-duplicates','text-to-columns'].includes(kind)||!range)throw Error('Select worksheet data first.');
    if(split&&range.firstColumn!==range.lastColumn)throw Error('Select one column to split.');
    let closed=false,acting=false,preview=null,page=0,generation=0,previewTimer=null,previewInFlight=false,pendingPreview=null,delimiter=',',qualifier='"',mode='general';
    const controls=[],status=new Label({name:'data-tool-status',text:''});
    const cancel=new Button({name:'data-tool-cancel',text:'Cancel'}),commit=new Button({name:'data-tool-apply',text:split?'Split columns':'Remove duplicates',enabled:false});
    const content=new Column({text:'panel:flush:tight-stack'}),sample=new Table({name:'data-tool-sample',columnWidths:[]});
    const sampleScroll=new ScrollView({name:'data-tool-sample-scroll',text:'viewport:160|0/0',visible:split},[sample]);
    const header=new CheckBox({name:'dedup-header',text:'My data has headers',checked:true});
    const caseSensitive=new CheckBox({name:'dedup-case-sensitive',text:'Case sensitive',checked:false});
    const collapse=new CheckBox({name:'split-collapse',text:'Treat consecutive delimiters as one',checked:false});
    const overwrite=new CheckBox({name:'split-overwrite',text:'Replace occupied destination cells',checked:false,visible:split});
    const target=new TextField({name:'split-destination',value:cellAddress(range.firstRow,range.firstColumn),accessibleName:'Destination cell'});
    const custom=new TextField({name:'split-custom-delimiter',value:'',enabled:false,accessibleName:'Custom delimiter'});
    const chosen=new Set(range.lastColumn-range.firstColumn<256?Array.from({length:range.lastColumn-range.firstColumn+1},(_,i)=>range.firstColumn+i):[]);
    const columns=new Column({name:'dedup-columns',text:'panel:flush:tight-stack'}),previous=new Button({name:'dedup-previous',text:'Previous'}),next=new Button({name:'dedup-next',text:'Next'}),pageLabel=new Label({name:'dedup-page',text:''});
    const invalidate=()=>{generation++;clearTimeout(previewTimer);preview=null;status.text='Checking selection…';for(const child of [...sample.children])sample.remove(child,{destroy:true});overwrite.setProperty('checked',false);update();previewTimer=setTimeout(()=>refreshPreview(generation),80);};
    function update(){
        for(const control of controls)control.setProperty('enabled',!acting&&!closed);
        for(const control of columns.children)control.setProperty('enabled',!acting&&!closed);
        custom.setProperty('enabled',!acting&&!closed&&delimiter===null);
        previous.setProperty('enabled',!acting&&!closed&&page>0);next.setProperty('enabled',!acting&&!closed&&range.firstColumn+(page+1)*256<=range.lastColumn);
        overwrite.setProperty('enabled',!acting&&!closed&&Boolean(preview?.conflicts));
        commit.setProperty('enabled',!acting&&!closed&&Boolean(preview)&&(!split||!preview.conflicts||overwrite.getProperty('checked')));
    }
    const choice=(name,label,items,select)=>{
        const menu=new Menu({name,text:items[0][1],accessibleName:label,state:State.START_ALIGNED|State.TRAILING_ICON});
        for(const [index,[value,text]]of items.entries()){const item=new MenuItem({name:name+'-'+index,text,checked:value===items[0][0]});item.on(Message.ACTIVATE,()=>{if(acting||closed)return;menu.text=text;for(const sibling of menu.children)sibling.setProperty('checked',sibling===item);select(value);invalidate();});menu.append(item);}
        controls.push(menu);return new Row({text:'panel:fixed:flush'},[new Label({text:label}),menu]);
    };
    function showColumns(){
        for(const child of [...columns.children])columns.remove(child,{destroy:true});
        const first=range.firstColumn+page*256,last=Math.min(range.lastColumn,first+255);
        for(let column=first;column<=last;column++){
            const box=new CheckBox({name:'dedup-column-'+column,text:'Column '+cellAddress(0,column).slice(0,-1),checked:chosen.has(column)});
            box.on(Message.ACTIVATE,()=>{if(acting||closed)return;if(box.getProperty('checked')){if(chosen.size===256){box.setProperty('checked',false);status.text='Choose up to 256 comparison columns.';return;}chosen.add(column);}else chosen.delete(column);invalidate();});columns.append(box);
        }
        pageLabel.text=`${first-range.firstColumn+1}–${last-range.firstColumn+1} of ${range.lastColumn-range.firstColumn+1} columns`;update();
    }
    if(split){
        content.append(choice('split-delimiter','Separator',[[',','Comma'],['\t','Tab'],[';','Semicolon'],[' ','Space'],[null,'Custom']],value=>delimiter=value));
        content.append(custom);content.append(collapse);
        content.append(choice('split-qualifier','Text qualifier',[['"','Double quote'],["'",'Single quote'],[null,'None']],value=>qualifier=value));
        content.append(choice('split-mode','Column data', [['general','General (numbers and text)'],['text','Text (preserve leading zeros)']],value=>mode=value));
        content.append(new Row({text:'panel:fixed:flush'},[new Label({text:'Destination'}),target]));content.append(overwrite);
        controls.push(custom,collapse,target,overwrite);
        for(const control of [custom,target])control.on(Message.VALUE_CHANGED,invalidate);collapse.on(Message.ACTIVATE,invalidate);overwrite.on(Message.ACTIVATE,update);
    }else{
        content.append(header);content.append(caseSensitive);content.append(new Label({text:'Keep the first matching row. Compare these columns:'}));
        content.append(new ScrollView({text:'viewport:180|0/0'},[columns]));if(range.lastColumn-range.firstColumn>=256)content.append(new Row({text:'panel:fixed:flush'},[previous,pageLabel,next]));
        controls.push(header,caseSensitive,previous,next);header.on(Message.ACTIVATE,invalidate);caseSensitive.on(Message.ACTIVATE,invalidate);
        previous.on(Message.ACTIVATE,()=>{if(!acting&&!closed&&page>0){page--;showColumns();}});next.on(Message.ACTIVATE,()=>{if(!acting&&!closed&&range.firstColumn+(page+1)*256<=range.lastColumn){page++;showColumns();}});showColumns();
    }
    function request(){
        if(!isCurrent())throw Error('Worksheet changed. Reopen this tool.');
        if(!split){if(!chosen.size)throw Error('Choose at least one comparison column.');return{sheetId,range,header:header.getProperty('checked'),columns:[...chosen].sort((a,b)=>a-b),caseSensitive:caseSensitive.getProperty('checked')};}
        const separator=delimiter??custom.getProperty('value');if(Array.from(separator).length!==1)throw Error('Enter one separator character.');
        return{sheetId,range,delimiter:separator,textQualifier:qualifier,collapseDelimiters:collapse.getProperty('checked'),destination:destination(target.getProperty('value')),mode};
    }
    async function refreshPreview(token){
        if(acting||closed||token!==generation)return;
        // The document port deliberately serializes worker calls. Keep only
        // the newest requested preview while an earlier generation completes.
        if(previewInFlight){pendingPreview=token;return;}
        previewInFlight=true;
        try{
            const options=request();const response=await query({action:'preview-'+kind,...options});if(closed||token!==generation)return;
            if(response.revision!==projection.revision||!isCurrent())throw Error('Worksheet changed. Reopen this tool.');
            preview=response.result;
            if(split){
                for(const child of [...sample.children])sample.remove(child,{destroy:true});
                const count=Math.max(1,Math.min(10,preview.columnCount));sample.setProperty('columnWidths',count===1?[]:Array(count).fill(Math.floor(1000/count)));if(count===1)sample.text='';
                for(const row of preview.sample)sample.append(new Row({text:'panel:fixed:flush'},Array.from({length:count},(_,index)=>new Label({text:row[index]??''}))));
                sampleScroll.setProperty('visible',true);
                status.text=`${preview.rowCount} rows × ${preview.columnCount} columns → ${rangeAddress(preview.outputRange)}. ${preview.conflicts} occupied destination cells.${preview.truncated?' Preview shows the first 10 rows and columns.':''}`;
            }else status.text=`${preview.removedRows} duplicate rows will be removed; ${preview.keptRows} rows kept.`;
        }catch(error){if(!closed&&token===generation){preview=null;status.text=String(error.message||error);}}finally{
            previewInFlight=false;if(!closed&&token===generation)update();
            const next=pendingPreview;pendingPreview=null;
            if(next!==null&&!closed&&!acting&&next===generation)void refreshPreview(next);
        }
    }
    commit.on(Message.ACTIVATE,async()=>{
        if(acting||closed||!preview||split&&preview.conflicts&&!overwrite.getProperty('checked'))return;
        try{const options=request();acting=true;update();const applied=await apply({type:kind,...options,...(split?{overwrite:overwrite.getProperty('checked')}:{})},projection.revision);if(applied&&!closed){acting=false;closed=true;onClose();}else if(!closed)status.text='The operation could not be applied. Preview again.';}
        catch(error){if(!closed)status.text=String(error.message||error);}finally{acting=false;if(!closed)update();}
    });
    const close=()=>{if(!acting&&!closed){closed=true;generation++;clearTimeout(previewTimer);onClose();}};cancel.on(Message.ACTIVATE,close);controls.push(cancel,commit);
    const dialog=new Dialog({name:'data-tool-dialog',text:split?'Text to Columns':'Remove Duplicates',modal:true},[new Column({text:split?'panel:flush:tight-stack:width=560':'panel:flush:tight-stack'},[new Label({text:'Selected range: '+rangeAddress(range)}),content,...(split?[new Stack({text:'panel:fixed-height:160:flush'},[sampleScroll]),new Stack({text:'panel:fixed-height:48:flush'},[status])]:[status]),new Row({text:'panel:actions'},[cancel,commit])])]);
    dialog.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9))close();});dialog.on(Message.DESTROY,()=>{closed=true;generation++;clearTimeout(previewTimer);});invalidate();return dialog;
}
