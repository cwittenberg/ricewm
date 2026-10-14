import {Button,Column,Container,ContextMenu,Dialog,Label,MenuItem,Message,Row,ScrollView,State,TextField,Tooltip,VirtualGrid,validateGridStyle} from '@ricewm/app';
import {selectionRange} from './editor-actions.mjs';
const PAGE=18;
export function checkedCellStyles(value) {
    const styles=value?.styles;
    if(!Array.isArray(styles)||styles.length>32)throw Error('Invalid cell style page');
    const names=new Set();
    return styles.map(style=>{
        if(typeof style.name!=='string'||!style.name||style.name.length>256||names.has(style.name))throw Error('Invalid cell style name');names.add(style.name);
        if(style.builtin!==undefined&&typeof style.builtin!=='boolean')throw Error('Invalid cell style identity');
        validateGridStyle({alignment:0,role:0,font:style.font,...(style.paint?{paint:style.paint}:{})});
        return style;
    });
}
export function stylePreviewFrame(style,index) {
    const width=174,height=36,track=extent=>({index:0,start:0,extent,clip_start:0,clip_end:extent,frozen:false});
    return {version:1,revision:0,sheet:index,row_count:1,column_count:1,width,height,row_header:0,column_header:0,
        rows:[track(height)],columns:[track(width)],cells:[[0,0,0,0]],strings:[style.name],
        styles:[{alignment:0,role:0,font:{...style.font,size_pt:Math.min(18,style.font.size_pt)},...(style.paint?{paint:style.paint}:{})}],selection:null,active:null};
}
/** Page/search requests stay in Wasm; Rice's ordinary grid renders the previews. */
export function createStyleGallery({projection,initialPage,query,isCurrent,apply,canDelete=false,onClose}) {
    let closed=false,acting=false,loading=false,offset=0,columns=2,page=[],total=0,nextOffset=null,epoch=0,searchTimer=null;
    const search=new TextField({name:'style-search',value:'',accessibleName:'Search cell styles',focused:true});
    const tiles=new Column({name:'style-tiles',text:'panel:flush:tight-stack'}),status=new Label({name:'style-status',text:''});
    const previous=new Button({name:'styles-previous',text:'Back'}),next=new Button({name:'styles-next',text:'Next'}),cancel=new Button({name:'styles-cancel',text:'Cancel'});
    async function choose(style,remove=false){
        if(remove&&(!canDelete||style.builtin!==false))return;
        if(closed||acting||loading)return;
        if(!isCurrent()){status.text='Workbook changed. Reopen Cell Styles.';return;}
        acting=true;tiles.setProperty('enabled',false);search.setProperty('enabled',false);
        try{if(await apply(remove?{type:'delete-cell-style',name:style.name}:{type:'apply-cell-style',sheetId:projection.activeSheetId,range:selectionRange(projection),name:style.name},projection.revision)){acting=false;close();}else{status.text=remove?'Could not delete this style.':'Could not apply this style.';}}
        catch(error){status.text=String(error.message||error).slice(0,240);}
        finally{acting=false;if(!closed){tiles.setProperty('enabled',true);search.setProperty('enabled',true);}}
    }
    function render(){
        for(const child of [...tiles.children])tiles.remove(child,{destroy:true});
        for(let i=0;i<page.length;i+=columns){const row=new Row({text:'panel:fixed:flush'});for(const style of page.slice(i,i+columns)){
            const tile=new VirtualGrid({name:`style-preview-${offset+page.indexOf(style)}`,canvas:'document',accessibleName:`Apply ${style.name} cell style`,frame:stylePreviewFrame(style,offset+page.indexOf(style)),state:State.ENABLED,tooltip:style.name});
            tile.on(Message.POINTER,event=>{if(event.detail?.kind==='up'&&event.detail.button===1){event.preventDefault();void choose(style);}});
            tile.on(Message.KEY,event=>{if(!event.detail?.pressed)return;if(event.detail.keyCode===9){event.preventDefault();close();}else if([36,65].includes(event.detail.keyCode)){event.preventDefault();void choose(style);}});
            const container=new Container({text:'panel:fixed:width=180:accessory-height=40:flush'},[tile,new Tooltip({text:style.name})]);
            if(canDelete&&style.builtin===false){
                const remove=new MenuItem({name:`style-delete-${offset+page.indexOf(style)}`,text:'Delete Style',icon:25});
                remove.on(Message.ACTIVATE,()=>void choose(style,true));
                new ContextMenu({name:`style-context-${offset+page.indexOf(style)}`},[remove]).attachTo(container);
            }
            row.append(container);
        }tiles.append(row);}
        status.text=total?`${offset+1}–${offset+page.length} of ${total} styles`:'No matching styles';
        previous.setProperty('enabled',!loading&&offset>0);next.setProperty('enabled',!loading&&nextOffset!==null);
    }
    function accept(value,start){
        const styles=checkedCellStyles(value);
        if(!Number.isSafeInteger(value.total)||value.total<0||value.total>65536||start>value.total||styles.length>Math.min(PAGE,value.total-start)||
            !(value.nextOffset===null||Number.isSafeInteger(value.nextOffset)&&value.nextOffset===start+styles.length&&value.nextOffset<value.total))throw Error('Invalid cell style pagination');
        page=styles;total=value.total;offset=start;nextOffset=value.nextOffset;render();
    }
    async function load(start){
        if(closed||acting)return;const mine=++epoch,literal=search.getProperty('value');
        if(Array.from(literal).length>128){status.text='Search supports up to 128 characters';return;}
        loading=true;tiles.setProperty('enabled',false);previous.setProperty('enabled',false);next.setProperty('enabled',false);
        try{const result=await query({offset:start,limit:PAGE,literal});if(!closed&&mine===epoch)accept(result,start);}
        catch(error){if(!closed&&mine===epoch)status.text=String(error.message||error).slice(0,240);}
        finally{if(!closed&&mine===epoch){loading=false;tiles.setProperty('enabled',true);previous.setProperty('enabled',offset>0);next.setProperty('enabled',nextOffset!==null);}}
    }
    const dialog=new Dialog({name:'cell-styles-dialog',text:'Cell Styles',modal:true},[search,new ScrollView({text:'viewport:300|0/0',state:State.EXPANSIVE},[tiles]),new Container({text:'panel:fixed:flush'},[status,new Row({text:'panel:actions'},[previous,next,cancel])])]);
    function close(){if(closed||acting)return;closed=true;clearTimeout(searchTimer);epoch++;onClose();}
    const bindEscape=control=>{control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();close();}});for(const child of control.children)bindEscape(child);};bindEscape(dialog);
    tiles.on(Message.SIZE_CHANGED,event=>{const width=event.detail?.width;if(Number.isFinite(width)){const count=Math.max(1,Math.min(3,Math.floor(width/180)));if(count!==columns){columns=count;render();}}});
    search.on(Message.VALUE_CHANGED,()=>{clearTimeout(searchTimer);epoch++;tiles.setProperty('enabled',false);searchTimer=setTimeout(()=>void load(0),150);});previous.on(Message.ACTIVATE,()=>void load(Math.max(0,offset-PAGE)));next.on(Message.ACTIVATE,()=>{if(nextOffset!==null)void load(nextOffset);});cancel.on(Message.ACTIVATE,close);
    dialog.on(Message.DESTROY,()=>{closed=true;clearTimeout(searchTimer);epoch++;});
    accept(initialPage,0);return dialog;
}
