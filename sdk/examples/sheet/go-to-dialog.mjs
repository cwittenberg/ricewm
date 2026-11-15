import {Button,Column,Dialog,Label,Message,Row,ScrollView,TextField,WorkbookContracts} from '@ricewm/app';
const {sheetNavigationCommand,formatSheetNavigationReference,checkedSheetNavigationPage}=WorkbookContracts;
export function createGoToDialog({projection,recent=[],isCurrent,apply,onClose,query,onSpecial}){
    let closed=false,acting=false,loading=false,generation=0,timer=null,tail=Promise.resolve(),offset=0,nextOffset=null,previousOffsets=[];
    const reference=new TextField({name:'go-to-reference',value:'',focused:true,accessibleName:'Reference'}),status=new Label({name:'go-to-status',text:''});
    const accept=new Button({name:'go-to-ok',text:'OK'}),cancel=new Button({name:'go-to-cancel',text:'Cancel'}),history=new Column({name:'go-to-recent',text:'panel:flush:tight-stack'});
    const special=typeof onSpecial==='function'?new Button({name:'go-to-special-button',text:'Special…'}):null;
    const named=new Column({name:'go-to-destinations',text:'panel:flush:tight-stack'}),search=new TextField({name:'go-to-search',value:'',accessibleName:'Search names and tables'});
    const previous=new Button({name:'go-to-previous',text:'Previous',enabled:false}),next=new Button({name:'go-to-next',text:'Next',enabled:false}),pageLabel=new Label({name:'go-to-page',text:''});
    const list=new Column({text:'panel:flush:tight-stack'},[...(typeof query==='function'?[new Label({text:'Names and tables'}),search,named]:[]),new Label({text:'Recent destinations'}),history]);
    const dialog=new Dialog({name:'go-to-dialog',text:'Go To',modal:true},[new Column({text:'panel:fixed:width=420'},[new ScrollView({name:'go-to-destination-scroll',text:'viewport:200|0/0'},[list]),...(typeof query==='function'?[new Row({text:'panel:fixed:flush'},[previous,pageLabel,next])]:[]),new Label({text:'Reference:'}),reference,status,new Row({text:'panel:actions'},[...(special?[special]:[]),cancel,accept])])]);
    const stop=()=>{closed=true;generation++;clearTimeout(timer);timer=null;};
    const close=()=>{if(!closed&&!acting){stop();onClose();}};
    const update=()=>{reference.setProperty('enabled',!acting);search.setProperty('enabled',!acting);cancel.setProperty('enabled',!acting);special?.setProperty('enabled',!acting&&!loading);for(const item of [accept,...history.children,...named.children])item.setProperty('enabled',!acting&&!loading);previous.setProperty('enabled',!acting&&!loading&&previousOffsets.length>0);next.setProperty('enabled',!acting&&!loading&&nextOffset!==null);};
    const submit=async()=>{
        if(closed||acting||loading)return;let command;
        try{if(!isCurrent())throw Error('Workbook changed. Reopen Go To.');command=sheetNavigationCommand(reference.getProperty('value'),projection);}catch(error){status.text=String(error.message||error).slice(0,240);return;}
        acting=true;update();
        try{if(await apply(command,projection.revision)){acting=false;close();}else status.text='The destination was not selected.';}
        catch(error){if(!closed)status.text=String(error.message||error).slice(0,240);}
        finally{if(!closed){acting=false;update();}}
    };
    for(const command of recent.slice(0,10)){
        let text;try{text=formatSheetNavigationReference(command,projection.sheets);}catch{continue;}
        const item=new Button({name:'go-to-recent-'+history.children.length,text});item.on(Message.ACTIVATE,()=>{if(!acting&&!loading){reference.setProperty('value',text);void submit();}});history.append(item);
    }
    const load=(target,{reset=false,debounce=false}={})=>{
        if(closed||acting||typeof query!=='function')return;const token=++generation;clearTimeout(timer);timer=null;
        if(reset)previousOffsets=[];loading=true;nextOffset=null;pageLabel.text='Loading destinations…';for(const child of [...named.children])named.remove(child,{destroy:true});update();
        const request={action:'navigation-destinations',sheetId:projection.activeSheetId,offset:target,limit:16,literal:search.getProperty('value')};
        const run=()=>{timer=null;tail=tail.catch(()=>{}).then(async()=>{
            if(closed||token!==generation)return;
            try{
                if(!isCurrent())throw Error('Workbook changed. Reopen Go To.');
                const response=await query(request);if(closed||token!==generation)return;
                if(!isCurrent())throw Error('Workbook changed. Reopen Go To.');
                const page=checkedSheetNavigationPage(response,request,projection.revision);
                for(const child of [...named.children])named.remove(child,{destroy:true});
                for(const entry of page.items){const item=new Button({name:'go-to-destination-'+named.children.length,text:entry.label});item.on(Message.ACTIVATE,()=>{if(!acting&&!loading){reference.setProperty('value',entry.reference);void submit();}});named.append(item);}
                offset=page.offset;nextOffset=page.nextOffset;pageLabel.text=page.items.length?`${page.items.length} destinations`:'No destinations on this page';status.text='';
            }catch(error){if(!closed&&token===generation){status.text=String(error.message||error).slice(0,240);nextOffset=null;}}
            finally{if(!closed&&token===generation){loading=false;update();}}
        });};
        if(debounce)timer=setTimeout(run,160);else run();
    };
    search.on(Message.VALUE_CHANGED,()=>load(0,{reset:true,debounce:true}));
    previous.on(Message.ACTIVATE,()=>{if(!acting&&!loading&&previousOffsets.length)load(previousOffsets.pop());});
    next.on(Message.ACTIVATE,()=>{if(!acting&&!loading&&nextOffset!==null){previousOffsets.push(offset);load(nextOffset);}});
    accept.on(Message.ACTIVATE,()=>void submit());cancel.on(Message.ACTIVATE,close);
    special?.on(Message.ACTIVATE,()=>{if(!closed&&!acting&&!loading){stop();onSpecial();}});
    dialog.on(Message.KEY,event=>{if(!event.detail?.pressed)return;const key=event.detail.keyCode;if(key===9||event.detail.key==='Escape'){event.preventDefault();close();}else if([36,104].includes(key)||event.detail.key==='Enter'){event.preventDefault();void submit();}});
    dialog.on(Message.DESTROY,stop);if(typeof query==='function')load(0);return dialog;
}
