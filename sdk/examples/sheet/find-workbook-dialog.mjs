import {Button,CheckBox,Column,Dialog,Label,Menu,MenuItem,Message,Row,ScrollView,TextField,Table,WorkbookContracts} from '@ricewm/app';
const {checkedSheetFindResultSort,checkedSheetFindResultsPage,SHEET_FIND_RESULT_SORT,checkedSheetFindOptions,checkedSheetFindPage,checkedSheetFindPreferences,checkedSheetReplaceRequest,checkedSheetReplacePreview,checkedSheetReplacementText,checkedSheetFindHistory,rememberSheetFindText,sheetFindRepeatDirection,SHEET_FIND_DEFAULTS}=WorkbookContracts;
const label=value=>Array.from(String(value).replace(/[\p{Cc}]/gu,' ')).slice(0,200).join('');
export function createWorkbookFindDialog({projection,current,isCurrent,apply,query,onClose,preferences=SHEET_FIND_DEFAULTS,onPreferencesChanged,canReplace=false,initialTab='find',replacementText='',onReplacementChanged,optionsExpanded=false,onOptionsExpandedChanged,findHistory=[],replacementHistory=[],onHistoryChanged,onSearch,canSort=false,resultSort=SHEET_FIND_RESULT_SORT,onResultSortChanged}){
    const initial=checkedSheetFindPreferences(preferences);
    if(typeof optionsExpanded!=='boolean')throw TypeError('Search Options visibility must be boolean');
    let acceptedSort=checkedSheetFindResultSort(resultSort),updatingResults=false;
    let expanded=optionsExpanded,recentFind=checkedSheetFindHistory(findHistory),recentReplace=checkedSheetFindHistory(replacementHistory,{replacement:true});
    let closed=false,acting=false,querying=false,generation=0,within=initial.within,order=initial.order,lookIn=initial.lookIn,next=null,history=[],pageAfter=null,pageOptions=null,pageRevision=null;
    let replacing=canReplace&&initialTab==='replace';if(replacing)lookIn='formulas';
    const text=new TextField({name:'workbook-find-text',value:initial.text,focused:true,accessibleName:'Find what'}),status=new Label({name:'workbook-find-status',text:''});
    const findRecent=new Menu({name:'workbook-find-recent',text:'',icon:74,accessibleName:'Recent searches',tooltip:'Recent searches'}),replaceRecent=new Menu({name:'workbook-find-replacement-recent',text:'',icon:74,accessibleName:'Recent replacements',tooltip:'Recent replacements'});
    const optionsToggle=new Button({name:'workbook-find-options-toggle',text:expanded?'Options <<':'Options >>',accessibleName:expanded?'Hide search options':'Show search options'});
    const matchCase=new CheckBox({name:'workbook-find-case',text:'Match case',checked:initial.matchCase}),whole=new CheckBox({name:'workbook-find-whole',text:'Match entire cell contents',checked:initial.wholeCell});
    const withinMenu=new Menu({name:'workbook-find-within',text:{sheet:'Sheet',workbook:'Workbook',selection:'Selection'}[within]}),orderMenu=new Menu({name:'workbook-find-order',text:order==='rows'?'By Rows':'By Columns'}),lookMenu=new Menu({name:'workbook-find-look-in',text:lookIn==='formulas'?'Formulas':'Values'});
    const previous=new Button({name:'workbook-find-previous',text:'Find Previous'}),findNext=new Button({name:'workbook-find-next',text:'Find Next'}),findAll=new Button({name:'workbook-find-all',text:'Find All'}),closeButton=new Button({name:'workbook-find-close',text:'Close'});
    const findTab=new Button({name:'workbook-find-tab-find',text:'Find'}),replaceTab=new Button({name:'workbook-find-tab-replace',text:'Replace',enabled:canReplace});
    const replacement=new TextField({name:'workbook-find-replacement',value:checkedSheetReplacementText(replacementText),accessibleName:'Replace with'});
    const replacementRow=new Column({name:'workbook-find-replacement-row',text:'panel:flush',visible:replacing},[new Label({text:'Replace with:'}),new Row({text:'panel:fixed:flush'},[new Column({text:'panel:fixed:width=540:flush'},[replacement]),replaceRecent])]);
    const replaceButton=new Button({name:'workbook-find-replace',text:'Replace',visible:replacing}),replaceAll=new Button({name:'workbook-find-replace-all',text:'Replace All',visible:replacing});
    const resultTable=canSort?new Table({name:'workbook-find-table',id:'workbook-find-table',data:[],rowKey:'id',rowHeaders:false,selectionMode:'single-row',sortMode:'external',sortTrigger:'single',height:180,rowHeight:26,
        columns:[{key:'book',title:'Book',width:90},{key:'sheet',title:'Sheet',width:90},{key:'cell',title:'Cell',width:70},{key:'value',title:'Value',width:130},{key:'formula',title:'Formula',width:170}]}):null;
    const results=new Column({name:'workbook-find-results',text:'panel:flush:tight-stack'},resultTable?[resultTable]:[]),scroll=new ScrollView({name:'workbook-find-result-scroll',text:'viewport:180|0/0',visible:false},[results]);
    const pagePrevious=new Button({name:'workbook-find-page-previous',text:'Previous page',enabled:false}),pageNext=new Button({name:'workbook-find-page-next',text:'Next page',enabled:false});
    const sortDefault=new Button({name:'workbook-find-sort-default',text:'Search order',visible:canSort});
    const paging=new Row({name:'workbook-find-paging',text:'panel:fixed:flush',visible:false},[pagePrevious,pageNext,sortDefault]);
    const controls=[text,findRecent,replaceRecent,optionsToggle,withinMenu,orderMenu,lookMenu,matchCase,whole,previous,findNext,findAll,findTab,replacement,replaceButton,replaceAll,...(resultTable?[resultTable,sortDefault]:[])];
    const optionsPanel=new Column({name:'workbook-find-options',text:'panel:flush',visible:expanded},[
        new Row({text:'panel:fixed:flush'},[new Label({text:'Within:'}),withinMenu,new Label({text:'Search:'}),orderMenu,new Label({text:'Look in:'}),lookMenu]),
        new Row({text:'panel:fixed:flush'},[matchCase,whole]),new Label({text:'Use * for any text, ? for one character, and ~ to find a literal *, ? or ~.'})]);
    const dialog=new Dialog({name:'workbook-find-dialog',text:canReplace?'Find and Replace':'Find',modal:true},[new Column({text:'panel:fixed:width=620'},[
        ...(canReplace?[new Row({text:'panel:tabs'},[findTab,replaceTab])]:[]),new Label({text:'Find what:'}),new Row({text:'panel:fixed:flush'},[new Column({text:'panel:fixed:width=540:flush'},[text]),findRecent]),replacementRow,
        new Row({text:'panel:actions'},[optionsToggle]),optionsPanel,scroll,paging,status,new Row({text:'panel:actions'},[replaceAll,replaceButton,findAll,previous,findNext,closeButton])])]);
    const check=()=>{if(closed||!isCurrent())throw Error('Workbook changed. Reopen Find.');return current();};
    const options=()=>checkedSheetFindOptions({sheetId:projection.activeSheetId,within,range:within==='selection'?projection.selection.range:null,text:text.getProperty('value'),matchCase:matchCase.getProperty('checked'),wholeCell:whole.getProperty('checked'),lookIn,order});
    const update=()=>{for(const control of [...controls,...results.children])control.setProperty('enabled',!acting&&!querying);replaceTab.setProperty('enabled',canReplace&&!acting&&!querying);lookMenu.setProperty('enabled',!replacing&&!acting&&!querying);findAll.setProperty('enabled',!acting&&!querying&&typeof query==='function');for(const control of [replaceButton,replaceAll])control.setProperty('enabled',canReplace&&!acting&&!querying&&typeof query==='function');findRecent.setProperty('enabled',!acting&&!querying&&recentFind.length>0);replaceRecent.setProperty('enabled',!acting&&!querying&&recentReplace.length>0);closeButton.setProperty('enabled',!acting);pagePrevious.setProperty('enabled',!acting&&!querying&&history.length>0);pageNext.setProperty('enabled',!acting&&!querying&&next!==null);};
    const clearResultRows=()=>{
        if(resultTable){updatingResults=true;try{resultTable.setData([]);}finally{updatingResults=false;}}
        else for(const child of [...results.children])results.remove(child,{destroy:true});
    };
    const selectResult=async(item)=>{
        if(closed||acting||querying||updatingResults)return;acting=true;update();
        try{const view=check();if(view.revision!==pageRevision)throw Error('Find results changed. Choose Find All again.');
            if(!(view.activeVisible&&view.active?.sheetId===item.sheetId&&view.active.row===item.row&&view.active.column===item.column))await apply({type:'select-cell',sheetId:item.sheetId,row:item.row,column:item.column},view.revision);
            if(!closed){pageRevision=current().revision;status.text='Match selected.';}
        }catch(error){if(!closed)status.text=label(error.message||error);}finally{if(!closed){acting=false;update();}}
    };
    if(resultTable){
        resultTable.on(Message.SELECT,event=>{if(event.detail.rows.length===1)void selectResult(event.detail.rows[0].target);});
        resultTable.on(Message.ACTIVATE,event=>{if(event.detail.row?.target)void selectResult(event.detail.row.target);});
        resultTable.on(Message.SORT_REQUEST,event=>{const {key,direction}=event.detail;void page(null,'sort',key===null?SHEET_FIND_RESULT_SORT:{column:key,direction:direction==='desc'?'descending':'ascending'});});
    }
    const clear=()=>{generation++;next=null;history=[];pageAfter=null;pageOptions=null;pageRevision=null;clearResultRows();scroll.setProperty('visible',false);paging.setProperty('visible',false);status.text='';try{onPreferencesChanged?.(checkedSheetFindPreferences({within,text:text.getProperty('value'),matchCase:matchCase.getProperty('checked'),wholeCell:whole.getProperty('checked'),lookIn,order}));}catch{}update();};
    const selectHistory=(field,value)=>{
        if(closed||acting||querying)return;
        field.setProperty('value',value);field.setProperty('focused',true);field.setSelection?.(0,Array.from(value).length);
        if(field===replacement)onReplacementChanged?.(value);
        clear();
    };
    const rebuildHistories=()=>{
        for(const [menu,values,field]of [[findRecent,recentFind,text],[replaceRecent,recentReplace,replacement]]){
            for(const child of [...menu.children])menu.remove(child,{destroy:true});
            values.forEach((value,index)=>{const item=new MenuItem({name:`${menu.name}-${index}`,text:value===''?'(empty replacement)':label(value)});item.on(Message.ACTIVATE,()=>selectHistory(field,value));menu.append(item);});
        }
    };
    const rememberInputs=(search,replacementValue)=>{
        recentFind=rememberSheetFindText(recentFind,search.text);
        if(replacementValue!==undefined)recentReplace=rememberSheetFindText(recentReplace,replacementValue,{replacement:true});
        rebuildHistories();onHistoryChanged?.({find:recentFind,replacement:recentReplace});onSearch?.(search);
    };
    const toggleOptions=()=>{
        if(closed||acting||querying)return;expanded=!expanded;optionsPanel.setProperty('visible',expanded);
        optionsToggle.text=expanded?'Options <<':'Options >>';optionsToggle.setProperty('accessibleName',expanded?'Hide search options':'Show search options');
        if(!expanded){for(const control of [withinMenu,orderMenu,lookMenu,matchCase,whole])control.setProperty('focused',false);text.setProperty('focused',true);}
        onOptionsExpandedChanged?.(expanded);
    };
    optionsToggle.on(Message.ACTIVATE,toggleOptions);
    const close=()=>{if(!closed&&!acting){closed=true;generation++;onClose();}};
    const submit=async(direction)=>{
        if(closed||acting||querying)return;let command,revision;
        try{const view=check();revision=view.revision;command={type:'find-next',options:options(),direction,after:view.active};}catch(error){status.text=label(error.message||error);return;}
        rememberInputs(command.options);acting=true;update();
        try{await apply(command,revision);if(!closed){if(pageOptions)pageRevision=current().revision;status.text='Match selected.';}}
        catch(error){if(!closed)status.text=label(error.message||error);}
        finally{if(!closed){acting=false;update();}}
    };
    const replace=async(all)=>{
        if(closed||acting||querying||!canReplace||!replacing||typeof query!=='function')return;let request,revision;
        try{const view=check();revision=view.revision;request=checkedSheetReplaceRequest({options:options(),replacement:replacement.getProperty('value'),target:all?null:view.active});}catch(error){status.text=label(error.message||error);return;}
        rememberInputs(request.options,request.replacement);acting=true;update();status.text='Replacing…';
        try{
            const response=await query({action:'preview-replace',...request},revision);if(closed)return;
            if(check().revision!==revision)throw Error('Workbook changed. Repeat Replace.');
            const counts=checkedSheetReplacePreview(response,request,revision);
            if(counts.changedCells)await apply({type:'replace-text',...request},revision);
            if(closed)return;clear();const message=`${counts.changedCells} ${counts.changedCells===1?'cell':'cells'} replaced.`;status.text=message;
            if(!all){try{const view=check();await apply({type:'find-next',options:request.options,after:request.target,direction:'forward'},view.revision);if(!closed)status.text=message;}catch(error){if(!closed)status.text=`${message} ${label(error.message||error)}`;}}
        }catch(error){if(!closed)status.text=label(error.message||error);}
        finally{if(!closed){acting=false;update();}}
    };
    const page=async(after=null,mode='restart',requestedSort=acceptedSort)=>{
        if(closed||acting||querying||typeof query!=='function')return;let request,revision;const token=++generation,previousNext=next;
        try{const view=check();revision=view.revision;if(mode!=='restart'&&revision!==pageRevision)throw Error('Selection changed. Choose Find All again.');request={action:canSort?'find-results':'find-matches',options:mode==='restart'?options():pageOptions,after,limit:16,...(canSort?{sort:checkedSheetFindResultSort(requestedSort)}:{})};}
        catch(error){status.text=label(error.message||error);return;}
        rememberInputs(request.options);querying=true;update();status.text='Searching…';
        try{const response=await query(request,revision);if(closed||generation!==token)return;check();const result=canSort?checkedSheetFindResultsPage(response,request,revision):checkedSheetFindPage(response,request,revision);
            if(mode==='restart'||mode==='sort')history=[];else if(mode==='next')history.push(pageAfter);else history.pop();
            pageAfter=after;pageOptions=request.options;pageRevision=revision;next=result.next;
            if(resultTable){
                const names=new Map(result.sheets.map(sheet=>[sheet.id,sheet.name])),preview=value=>label(value.text)+(value.truncated?'…':'');
                const data=result.items.map(item=>({id:JSON.stringify([item.sheetId,item.row,item.column]),target:item,book:preview(result.book),sheet:preview(names.get(item.sheetId)),cell:item.cell,value:preview(item.value),formula:preview(item.formula)}));
                updatingResults=true;try{resultTable.setData(data);resultTable.setSortState(result.sort.column==='order'?null:result.sort.column,result.sort.column==='order'?null:result.sort.direction==='descending'?'desc':'asc');}finally{updatingResults=false;}
                acceptedSort=result.sort;onResultSortChanged?.(acceptedSort);
            }else{
            for(const child of [...results.children])results.remove(child,{destroy:true});
            for(const [index,item]of result.items.entries()){
                let name=projection.sheets.find(sheet=>sheet.id===item.sheetId)?.name||item.sheetId;
                let column='',n=item.column+1;while(n){column=String.fromCharCode(65+(n-1)%26)+column;n=Math.floor((n-1)/26);}
                const row=new Button({name:'workbook-find-result-'+index,text:`${label(name)}!${column}${item.row+1} · ${label(item.preview)}${item.truncated?'…':''}`});
                row.on(Message.ACTIVATE,()=>void selectResult(item));results.append(row);
            }
            }
            scroll.setProperty('visible',true);paging.setProperty('visible',true);status.text=result.total?`${result.total} cells found.`:"We couldn't find what you were looking for.";
        }catch(error){if(!closed&&generation===token){next=mode==='sort'&&current().revision===pageRevision?previousNext:null;status.text=label(error.message||error);}}
        finally{if(!closed&&generation===token){querying=false;update();}}
    };
    for(const [menu,choices,set]of [[withinMenu,[['sheet','Sheet'],['workbook','Workbook'],['selection','Selection']],value=>{within=value;}],[orderMenu,[['rows','By Rows'],['columns','By Columns']],value=>{order=value;}],[lookMenu,[['formulas','Formulas'],['values','Values']],value=>{lookIn=value;}]]){
        for(const [value,title]of choices){const item=new MenuItem({name:`${menu.name}-${value}`,text:title});item.on(Message.ACTIVATE,()=>{if(!acting&&!querying){set(value);menu.text=title;clear();}});menu.append(item);}
    }
    const setTab=value=>{if(acting||querying)return;replacing=value;if(replacing){lookIn='formulas';lookMenu.text='Formulas';}replacementRow.setProperty('visible',replacing);for(const control of [replaceButton,replaceAll])control.setProperty('visible',replacing);clear();};
    findTab.on(Message.ACTIVATE,()=>setTab(false));replaceTab.on(Message.ACTIVATE,()=>{if(canReplace)setTab(true);});
    replacement.on(Message.VALUE_CHANGED,()=>{try{onReplacementChanged?.(checkedSheetReplacementText(replacement.getProperty('value')));}catch{}clear();});
    replaceButton.on(Message.ACTIVATE,()=>void replace(false));replaceAll.on(Message.ACTIVATE,()=>void replace(true));
    for(const control of [matchCase,whole])control.on(Message.ACTIVATE,clear);text.on(Message.VALUE_CHANGED,clear);
    previous.on(Message.ACTIVATE,()=>void submit('backward'));findNext.on(Message.ACTIVATE,()=>void submit('forward'));findAll.on(Message.ACTIVATE,()=>void page());closeButton.on(Message.ACTIVATE,close);
    sortDefault.on(Message.ACTIVATE,()=>void page(null,'sort',SHEET_FIND_RESULT_SORT));
    pagePrevious.on(Message.ACTIVATE,()=>{if(history.length)void page(history.at(-1),'previous');});pageNext.on(Message.ACTIVATE,()=>{if(next)void page(next,'next');});
    dialog.on(Message.KEY,event=>{if(!event.detail?.pressed)return;const repeat=sheetFindRepeatDirection(event.detail);if(repeat){event.preventDefault();void submit(repeat);}else if(event.detail.modifiers===8&&(event.detail.keyCode===32||event.detail.key==='o'||event.detail.key==='O')){event.preventDefault();toggleOptions();}else if(event.detail.modifiers&4&&event.detail.keyCode===43&&canReplace){event.preventDefault();setTab(true);}else if(event.detail.modifiers&4&&event.detail.keyCode===41){event.preventDefault();setTab(false);}else if(event.detail.keyCode===9||event.detail.key==='Escape'){event.preventDefault();close();}else if(([36,104].includes(event.detail.keyCode)||event.detail.key==='Enter')&&[dialog,text,replacement].includes(event.target)){event.preventDefault();void submit(event.detail.modifiers&1?'backward':'forward');}});
    dialog.on(Message.DESTROY,()=>{closed=true;generation++;});rebuildHistories();update();return dialog;
}
