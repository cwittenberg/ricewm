import {Button,CheckBox,ColorPalette,Column,Container,Dialog,Label,Menu,MenuItem,Message,Row,ScrollView,Stack,State,TextField,VirtualGrid} from '@ricewm/app';
import {selectionRange} from './editor-actions.mjs';
import {checkedSheetCommand} from './command-contract.mjs';
import {borderPreviewFrame,borderPreviewEdge} from './border-preview.mjs';

const categories=[['General','General'],['Number','#,##0.00'],['Currency','$#,##0.00'],['Percentage','0.00%'],['Date','yyyy-mm-dd'],['Time','hh:mm:ss'],['Scientific','0.00E+00'],['Text','@']];
/** Draft-only format editor. OK sends one transaction; Cancel sends none. */
export function createFormatDialog({projection,initialTab='number',themeColors=[],isCurrent,apply,onClose}) {
    const range=selectionRange(projection),font=projection.selection.font||{},format=projection.selection.format||{};
    const changes=new Map(),borders=new Map(),pages=new Map(),tabs=new Map();let closed=false,acting=false;
    const status=new Label({name:'format-status',text:''});
    const change=(path,value)=>changes.set(path,String(value));
    const field=(name,label,value,path)=>{const input=new TextField({name:`format-${name}`,value:String(value??''),accessibleName:label});input.on(Message.VALUE_CHANGED,()=>change(path,input.getProperty('value')));return{input,control:new Row({text:'panel:fixed:flush'},[new Label({text:label}),input])};};
    const choice=(name,label,value,options,path)=>{
        const control=new Menu({name:`format-${name}`,text:value||label,accessibleName:label,state:State.START_ALIGNED|State.TRAILING_ICON});
        for(const [key,text]of options){const item=new MenuItem({name:`format-${name}-${key}`,text,checked:key===value});item.on(Message.ACTIVATE,()=>{change(path,key);control.text=text;for(const sibling of control.children)sibling.setProperty('checked',sibling===item);});control.append(item);}
        return new Row({text:'panel:fixed:flush'},[new Label({text:label}),control]);
    };
    const toggle=(name,label,value,path)=>{const c=new CheckBox({name:`format-${name}`,text:label,checked:Boolean(value)});c.on(Message.ACTIVATE,()=>change(path,c.getProperty('checked')));return c;};
    const palette=(name,value,path,noneLabel)=>{const c=new ColorPalette({name:`format-${name}`,value:value??null,themeColors,allowNone:true,noneLabel,moreColors:true});c.on(Message.VALUE_CHANGED,event=>change(path,event.detail.value??'none'));return c;};
    const number=field('number-code','Format code',projection.selection.numberFormat||'General','num_fmt');
    const category=new Menu({name:'format-number-category',text:'Category',state:State.START_ALIGNED|State.TRAILING_ICON});
    for(const [label,code]of categories){const item=new MenuItem({name:`format-category-${label.toLowerCase()}`,text:label});item.on(Message.ACTIVATE,()=>{number.input.setProperty('value',code);change('num_fmt',code);category.text=label;});category.append(item);}
    pages.set('number',new Column({text:'panel:flush:tight-stack'},[category,number.control]));
    pages.set('alignment',new Column({text:'panel:flush:tight-stack'},[
        choice('horizontal','Horizontal',format.horizontal||'general',[['general','General'],['left','Left'],['center','Center'],['right','Right']],'alignment.horizontal'),
        choice('vertical','Vertical',format.vertical||'bottom',[['top','Top'],['center','Center'],['bottom','Bottom']],'alignment.vertical'),
        toggle('wrap','Wrap text',format.wrapText,'alignment.wrap_text'),
    ]));
    pages.set('font',new Column({text:'panel:flush:tight-stack'},[
        field('font-family','Font',font.family||'Arial','font.name').control,
        field('font-size','Size',font.size_pt||12,'font.size').control,
        new Row({text:'panel:fixed:flush'},[toggle('bold','Bold',font.weight>=600,'font.b'),toggle('italic','Italic',font.italic,'font.i'),toggle('underline','Underline',font.underline,'font.u')]),
        palette('font-color',format.fontColor,'font.color','Automatic'),
    ]));
    let borderStyle='thin',borderColor='#000000';
    const styleMenu=new Menu({name:'format-border-style',text:'Thin',state:State.START_ALIGNED|State.TRAILING_ICON});
    for(const [value,label]of[['thin','──── Thin'],['medium','━━━━ Medium'],['thick','━━━━ Thick'],['double','════ Double'],['dotted','······ Dotted'],['mediumDashed','┄┄┄┄ Dashed'],['mediumDashDot','┄·┄· Dash dot'],['mediumDashDotDot','┄··┄ Dash dot dot'],['slantDashDot','╱·╱· Slant dash dot'],['dashed','┄┄┄┄ Fine dashed'],['dashDot','┄·┄· Fine dash dot'],['dashDotDot','┄··┄ Fine dash dot dot'],['hair','──── Hairline'],['none','No line']]){const item=new MenuItem({name:`format-border-style-${value}`,text:label});item.on(Message.ACTIVATE,()=>{borderStyle=value;styleMenu.text=label;});styleMenu.append(item);}
    const borderPalette=new ColorPalette({name:'format-border-color',value:borderColor,themeColors,moreColors:true});borderPalette.on(Message.VALUE_CHANGED,event=>{borderColor=event.detail.value;});
    const preview=new VirtualGrid({name:'format-border-preview',canvas:'document',accessibleName:'Border preview: click an outer or interior edge',frame:borderPreviewFrame(range,format,[])});
    const edgeButtons=[];
    function stageBorder(edge,style=borderStyle){
        if(edge==='none')borders.clear();borders.delete(edge);borders.set(edge,{edge,style,color:borderColor});
        preview.setProperty('frame',borderPreviewFrame(range,format,[...borders.values()]));
        for(const button of edgeButtons)button.setProperty('checked',borders.has(button.borderEdge)&&borders.get(button.borderEdge).style!=='none');
        status.text=edge==='none'?'Borders will be removed':`${edge[0].toUpperCase()+edge.slice(1)} border selected`;
    }
    preview.on(Message.POINTER,event=>{
        if(event.detail?.kind!=='down'||event.detail.button!==1)return;
        const edge=borderPreviewEdge(preview.getProperty('frame'),event.detail.x,event.detail.y);if(!edge)return;
        const frame=preview.getProperty('frame'),sample=edge==='bottom'?(frame.row_count-1)*frame.column_count:edge==='right'?frame.column_count-1:0;
        const side=edge==='inside-horizontal'?'bottom':edge==='inside-vertical'?'right':edge;
        const previous=frame.styles[sample].paint?.borders?.[side];
        stageBorder(edge,previous?.style===borderStyle&&previous.color===borderColor?'none':borderStyle);event.preventDefault();
    });
    for(const [edge,label]of[['none','None'],['outline','Outline'],['inside','Inside'],['all','All'],['top','Top'],['bottom','Bottom'],['left','Left'],['right','Right']]){
        const button=new Button({name:`format-border-${edge}`,text:label,enabled:edge!=='inside'||range.firstRow!==range.lastRow||range.firstColumn!==range.lastColumn});button.borderEdge=edge;
        button.on(Message.ACTIVATE,()=>stageBorder(edge));edgeButtons.push(button);
    }
    const borderOptions = new Column({text:'panel:flush:width=260:fixed:tight-stack',state:State.FLAT},[styleMenu,borderPalette]);
    const borderEdges = new Column({text:'panel:flush:width=210:fixed:tight-stack',state:State.FLAT},[
        new Row({text:'panel:fixed:flush'},edgeButtons.slice(0,2)),
        new Row({text:'panel:fixed:flush'},edgeButtons.slice(2,4)),
        new Container({text:'panel:fixed-height:96:width=210:flush',state:State.FLAT},[preview]),
        new Row({text:'panel:fixed:flush'},edgeButtons.slice(4,6)),
        new Row({text:'panel:fixed:flush'},edgeButtons.slice(6)),
    ]);
    pages.set('border',new Row({text:'panel:flush:fixed:gap=12'},[borderOptions,borderEdges]));
    pages.set('fill',palette('fill-color',format.fillColor,'fill.color','No fill'));
    const tabRow=new Row({text:'panel:fixed:flush'}),body=new Stack({text:'panel:flush'},[...pages.values()]);
    const selectTab=name=>{for(const [key,page]of pages)page.setProperty('visible',key===name);for(const [key,tab]of tabs)tab.setProperty('checked',key===name);};
    for(const [name,label]of[['number','Number'],['alignment','Alignment'],['font','Font'],['border','Border'],['fill','Fill']]){const tab=new Button({name:`format-tab-${name}`,text:label});tab.on(Message.ACTIVATE,()=>selectTab(name));tabs.set(name,tab);tabRow.append(tab);}
    selectTab(pages.has(initialTab)?initialTab:'number');
    const cancel=new Button({name:'format-cancel',text:'Cancel'}),ok=new Button({name:'format-apply',text:'OK'});
    const dialog=new Dialog({name:'format-cells-dialog',text:'Format Cells',modal:true},[tabRow,new ScrollView({text:'viewport:340|0/0',state:State.EXPANSIVE},[body]),new Container({text:'panel:fixed:flush'},[status,new Row({text:'panel:actions'},[cancel,ok])])]);
    function close(){if(closed||acting)return;closed=true;onClose();}
    async function submit(){
        if(closed||acting)return;if(!changes.size&&!borders.size){close();return;}
        let command;try{if(!isCurrent())throw Error('Workbook changed. Reopen Format Cells.');command=checkedSheetCommand({type:'format-cells',sheetId:projection.activeSheetId,range,...(changes.size?{changes:[...changes].map(([path,value])=>({path,value}))}:{}),...(borders.size?{borders:[...borders.values()]}:{})});}catch(error){status.text=String(error.message||error);return;}
        acting=true;body.setProperty('enabled',false);tabRow.setProperty('enabled',false);ok.setProperty('enabled',false);cancel.setProperty('enabled',false);
        try{if(await apply(command,projection.revision)){acting=false;close();}}catch(error){status.text=String(error.message||error).slice(0,240);}
        finally{acting=false;if(!closed){body.setProperty('enabled',true);tabRow.setProperty('enabled',true);ok.setProperty('enabled',true);cancel.setProperty('enabled',true);}}
    }
    ok.on(Message.ACTIVATE,submit);cancel.on(Message.ACTIVATE,close);
    const bindEscape=control=>{control.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)){event.preventDefault();close();}});for(const child of control.children)bindEscape(child);};bindEscape(dialog);
    dialog.on(Message.DESTROY,()=>{closed=true;});return dialog;
}
