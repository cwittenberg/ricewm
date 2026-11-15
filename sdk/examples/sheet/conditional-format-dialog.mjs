import {Button,CheckBox,ColorPalette,Column,Dialog,Label,Menu,MenuItem,Message,Row,ScrollView,Stack,State,TextField,WorkbookContracts} from '@ricewm/app';
import {selectionRange} from './editor-actions.mjs';
import {createFormatDialog} from './format-dialog.mjs';

const {CONDITIONAL_RULE_TYPES,CONDITIONAL_OPERATORS,CONDITIONAL_TEXT_OPERATORS,CONDITIONAL_DATE_PERIODS,CONDITIONAL_ICONS,checkedConditionalRule,parseSheetRanges,formatSheetRanges}=WorkbookContracts;
const labels={'cell-value':'Cell Value',formula:'Use a formula',text:'Text',date:'Dates',duplicate:'Duplicate Values',unique:'Unique Values',blanks:'Blanks','not-blanks':'No Blanks',errors:'Errors','no-errors':'No Errors','above-average':'Above Average','below-average':'Below Average',top:'Top Items',bottom:'Bottom Items','color-scale':'Color Scale','data-bar':'Data Bars','icon-set':'Icon Set','icon-rating':'Icon Rating'};
const words=value=>value.replaceAll('-',' ').replace(/^./u,c=>c.toUpperCase());
const copy=value=>JSON.parse(JSON.stringify(value));
const threshold=(kind,value=null)=>({kind,value});
export function defaultConditionalRule(kind='cell-value'){
    const format={fill:'#FFC7CE',font:{color:'#9C0006'}};
    if(kind==='color-scale')return{kind,thresholds:[{threshold:threshold('min'),color:'#F8696B'},{threshold:threshold('percentile',50),color:'#FFEB84'},{threshold:threshold('max'),color:'#63BE7B'}]};
    if(kind==='data-bar')return{kind,minimum:null,maximum:null,positiveColor:'#638EC6',negativeColor:'#FF0000',gradient:true,showValue:true};
    if(kind==='icon-set')return{kind,icons:['ArrowDown','ArrowRight','ArrowUp'].map((icon,index)=>({icon,color:['#D63B32','#E8B630','#379754'][index],threshold:threshold(index?'percent':'min',index?index*100/3:null),strict:false})),showValue:true};
    if(kind==='icon-rating')return{kind,icon:'Star',color:'#E8B630',thresholds:[25,50,75].map(value=>({threshold:threshold('percent',value),strict:false})),showValue:true};
    return{kind,...({
        'cell-value':{operator:'greater-than',first:'0',second:null},formula:{formula:'=A1>0'},text:{operator:'contains',text:'Text'},
        date:{period:'today',first:null,second:null},top:{rank:10,percent:false},bottom:{rank:10,percent:false},
    }[kind]||{}),format,stopIfTrue:false};
}

/** Convert the existing draft-only format editor into a differential-format
 * patch. Missing properties remain missing; imported format metadata is retained
 * by sourceRuleIndex until the native transaction applies this patch. */
export function conditionalFormatChanges(original,command){
    const format=copy(original||{});
    for(const {path,value}of command.changes||[]){
        const flag={'font.b':'bold','font.i':'italic','font.u':'underline','font.strike':'strike'}[path];
        if(flag)(format.font??={})[flag]=value==='true';
        else if(path==='font.color')(format.font??={}).color=value==='none'?null:value;
        else if(path==='fill.color')format.fill=value==='none'?null:value;
        else if(path==='num_fmt')format.numberFormat=value;
        else throw Error('This property cannot be used in a conditional format.');
    }
    for(const border of command.borders||[]){
        const sides=['none','outline','all'].includes(border.edge)?['top','right','bottom','left']:[border.edge];
        for(const side of sides){if(!['top','right','bottom','left'].includes(side))throw Error('Conditional rules support outer cell borders.');
            (format.borders??={})[side]=border.edge==='none'||border.style==='none'?null:{style:border.style,color:border.color};}
    }
    return format;
}

function formatProjection(projection,draft){
    const range=selectionRange(projection),patch=draft.rule.format,preview=draft.formatPreview;
    const font={family:'Arial',size_pt:12,weight:400,italic:false,underline:false,...preview.font},format={...preview};
    for(const [key,value]of Object.entries(patch.font||{})){
        if(key==='bold')font.weight=value?700:400;else if(key==='color'){if(value===null||typeof value==='string')format.fontColor=value;}
        else font[key]=value;
    }
    if(Object.hasOwn(patch,'fill')&&(patch.fill===null||typeof patch.fill==='string'))format.fillColor=patch.fill;
    if(patch.borders)format.borders={...format.borders,...patch.borders};
    return {...projection,selection:{...projection.selection,font,format,numberFormat:patch.numberFormat??preview.numberFormat??'General',
        range:{...range,lastRow:range.firstRow,lastColumn:range.firstColumn}}};
}

export function createConditionalFormatDialog({projection,query,apply,currentRevision,themeColors=[],onClose,initialKind=null}){
    const selection=selectionRange(projection);let page=null,revision=projection.revision,selected=null,offset=0,scope='selection',acting=false,closed=false,styleDialog=null,draft=null;
    const status=new Label({name:'conditional-status',text:''});
    const manager=new Column({name:'conditional-manager',text:'panel:flush:tight-stack'}),editor=new Column({name:'conditional-editor',text:'panel:flush:tight-stack',visible:false});
    const list=new Column({name:'conditional-rules',text:'panel:flush:tight-stack'});
    const body=new Stack({text:'panel:flush'},[manager,editor]);
    const dialog=new Dialog({name:'conditional-format-dialog',text:'Conditional Formatting Rules Manager',modal:true},[new ScrollView({text:'viewport:380|0/0',state:State.EXPANSIVE},[body]),status]);
    const button=(name,text,handler,parent=manager)=>{const control=new Button({name:`conditional-${name}`,text});control.on(Message.ACTIVATE,()=>{if(!acting&&!closed)void handler();});parent.append(control);return control;};
    const fail=error=>{status.text=String(error.message||error).slice(0,300);};
    function close(){if(closed||acting)return;closed=true;onClose();}
    function choose(name,label,value,options,changed){
        const menu=new Menu({name:`conditional-${name}`,text:options.find(([key])=>key===value)?.[1]||String(value),accessibleName:label,state:State.START_ALIGNED|State.TRAILING_ICON});
        for(const [key,text]of options){const item=new MenuItem({name:`conditional-${name}-${key}`,text,checked:key===value});item.on(Message.ACTIVATE,()=>{if(acting)return;menu.text=text;for(const sibling of menu.children)sibling.setProperty('checked',sibling===item);changed(key);});menu.append(item);}
        return new Row({text:'panel:fixed:flush'},[new Label({text:label}),menu]);
    }
    function field(parent,name,label,value,changed){const input=new TextField({name:`conditional-${name}`,value:String(value??''),accessibleName:label});input.on(Message.VALUE_CHANGED,()=>changed(input.getProperty('value')));parent.append(new Row({text:'panel:fixed:flush'},[new Label({text:label}),input]));return input;}
    function toggle(parent,name,label,value,changed){const control=new CheckBox({name:`conditional-${name}`,text:label,checked:Boolean(value)});control.on(Message.ACTIVATE,()=>changed(Boolean(control.getProperty('checked'))));parent.append(control);return control;}
    function color(parent,name,label,value,changed){parent.append(new Label({text:label}));const control=new ColorPalette({name:`conditional-${name}`,value,themeColors,moreColors:true});control.on(Message.VALUE_CHANGED,event=>changed(event.detail.value));parent.append(control);}
    function clear(control){for(const child of [...control.children])control.remove(child,{destroy:true});}
    function setBusy(value){acting=value;body.setProperty('enabled',!value);}
    function updateButtons(){const item=page?.rules.find(rule=>rule.ruleIndex===selected);edit.setProperty('enabled',Boolean(item&&!item.readOnly));remove.setProperty('enabled',Boolean(item));up.setProperty('enabled',Boolean(item?.canMoveUp));down.setProperty('enabled',Boolean(item?.canMoveDown));previous.setProperty('enabled',offset>0);next.setProperty('enabled',page?.nextOffset!==null&&page?.nextOffset!==undefined);}
    async function refresh(reset=false){
        if(reset)offset=0;setBusy(true);status.text='Loading rules…';
        try{const result=await query({action:'conditional-formats',sheetId:projection.activeSheetId,range:scope==='selection'?selection:null,offset,limit:16});if(closed)return;
            page=result.result??result;revision=currentRevision();selected=null;clear(list);
            for(const item of page.rules){const rule=item.rule;const control=new Button({name:`conditional-rule-${item.ruleIndex}`,text:`${item.priority}. ${rule?labels[rule.kind]||words(rule.kind):'Imported rule'} · ${item.rangeLabel}${rule?.stopIfTrue?' · Stop If True':''}`,state:State.START_ALIGNED});
                control.on(Message.ACTIVATE,()=>{if(acting)return;selected=item.ruleIndex;for(const other of list.children)other.setProperty('checked',other===control);updateButtons();});list.append(control);}
            status.text=page.total?`${offset+1}–${offset+page.rules.length} of ${page.total} rules`:'No conditional formatting rules in this selection.';
            updateButtons();
        }catch(error){fail(error);}finally{setBusy(false);}
    }
    async function perform(command){
        if(currentRevision()!==revision)throw Error('Workbook changed. Close and reopen Conditional Formatting.');
        const result=await apply(command,revision);if(!result)throw Error('The conditional-format change could not be applied.');
        return result;
    }
    async function mutate(type,extra={}){setBusy(true);try{await perform({type,sheetId:projection.activeSheetId,ruleIndex:selected,...extra});offset=0;await refresh();}catch(error){fail(error);}finally{setBusy(false);}}
    manager.append(choose('scope','Show formatting rules for',scope,[['selection','Current Selection'],['sheet','This Worksheet']],value=>{scope=value;void refresh(true);}));
    const commands=new Row({text:'panel:fixed:flush'});manager.append(commands);
    button('new','New Rule…',()=>startEditor(),commands);
    const edit=button('edit','Edit Rule…',()=>startEditor(page.rules.find(rule=>rule.ruleIndex===selected)),commands);
    const remove=button('delete','Delete Rule',()=>mutate('delete-conditional-format'),commands);
    const up=button('up','Move Up',()=>mutate('move-conditional-format',{direction:'up'}),commands),down=button('down','Move Down',()=>mutate('move-conditional-format',{direction:'down'}),commands);
    manager.append(list);
    const paging=new Row({text:'panel:fixed:flush'});manager.append(paging);
    const previous=button('previous','Previous',()=>{offset=Math.max(0,offset-16);return refresh();},paging),next=button('next','Next',()=>{offset=page.nextOffset;return refresh();},paging);
    button('close','Close',close,manager);updateButtons();

    function editThreshold(parent,name,label,get,set){
        const host=new Column({text:'panel:flush:fixed:tight-stack'});parent.append(host);
        function render(){clear(host);const value=get()??threshold('min');host.append(choose(`${name}-kind`,label,value.kind,['min','max','number','percent','percentile','formula'].map(kind=>[kind,words(kind)]),kind=>{set(threshold(kind,['min','max'].includes(kind)?null:kind==='formula'?'=A1':0));render();}));
            if(!['min','max'].includes(value.kind))field(host,`${name}-value`,'Value',value.value,text=>set(threshold(get().kind,get().kind==='formula'?text:text.trim()?Number(text):NaN)));}
        render();
    }
    function startEditor(item=null,kind=initialKind||'cell-value'){
        draft={item,rule:copy(item?.rule||defaultConditionalRule(kind)),ranges:formatSheetRanges(item?.ranges||[selection]),formatPreview:item?.formatPreview||{}};showEditor();
    }
    function showManager(){draft=null;editor.setProperty('visible',false);manager.setProperty('visible',true);status.text='';}
    function showEditor(){
        clear(editor);manager.setProperty('visible',false);editor.setProperty('visible',true);status.text='';const rule=draft.rule;
        editor.append(choose('rule-type','Rule type',rule.kind,CONDITIONAL_RULE_TYPES.map(kind=>[kind,labels[kind]]),kind=>{const previous=draft.rule;draft.rule=defaultConditionalRule(kind);if(previous.format&&draft.rule.format)draft.rule.format=previous.format;showEditor();}));
        field(editor,'applies-to','Applies to',draft.ranges,value=>{draft.ranges=value;});
        if(rule.kind==='cell-value'){
            editor.append(choose('operator','Format cells whose value is',rule.operator,CONDITIONAL_OPERATORS.map(value=>[value,words(value)]),value=>{rule.operator=value;rule.second=['between','not-between'].includes(value)?rule.second??'0':null;showEditor();}));
            field(editor,'first','Value or formula',rule.first,value=>{rule.first=value;});if(rule.second!==null)field(editor,'second','And',rule.second,value=>{rule.second=value;});
        }else if(rule.kind==='formula')field(editor,'formula','Format values where this formula is true',rule.formula,value=>{rule.formula=value;});
        else if(rule.kind==='text'){
            editor.append(choose('text-operator','Format cells with text that',rule.operator,CONDITIONAL_TEXT_OPERATORS.map(value=>[value,words(value)]),value=>{rule.operator=value;}));field(editor,'text','Text',rule.text,value=>{rule.text=value;});
        }else if(rule.kind==='date'){
            editor.append(choose('period','Dates occurring',rule.period,CONDITIONAL_DATE_PERIODS.map(value=>[value,words(value)]),value=>{rule.period=value;rule.first=['between','not-between'].includes(value)?rule.first??'0':null;rule.second=rule.first===null?null:rule.second??'0';showEditor();}));
            if(rule.first!==null){field(editor,'first','From date or formula',rule.first,value=>{rule.first=value;});field(editor,'second','To date or formula',rule.second,value=>{rule.second=value;});}
        }else if(['top','bottom'].includes(rule.kind)){
            field(editor,'rank','Number of items',rule.rank,value=>{rule.rank=value.trim()?Number(value):NaN;});toggle(editor,'percent','Percent of items',rule.percent,value=>{rule.percent=value;});
        }
        if(rule.format){
            toggle(editor,'stop-if-true','Stop If True',rule.stopIfTrue,value=>{rule.stopIfTrue=value;});
            button('format','Format…',()=>{
                // A one-cell border preview corresponds to the four differential
                // edges that apply independently to each matching cell.
                styleDialog=createFormatDialog({projection:formatProjection(projection,draft),conditional:true,themeColors,isCurrent:()=>currentRevision()===revision,
                    apply:async command=>{rule.format=conditionalFormatChanges(rule.format,command);status.text='Rule format updated. Choose OK to save the rule.';return true;},
                    onClose:()=>{styleDialog?.parent?.remove(styleDialog,{destroy:true});styleDialog=null;}});dialog.append(styleDialog);
            },editor);
        }else if(rule.kind==='color-scale'){
            editor.append(choose('scale-count','Scale',String(rule.thresholds.length),[['2','2-Color Scale'],['3','3-Color Scale']],value=>{const stops=defaultConditionalRule('color-scale').thresholds;rule.thresholds=Number(value)===2?[stops[0],stops[2]]:stops;showEditor();}));
            rule.thresholds.forEach((stop,index)=>{editThreshold(editor,`scale-${index}`,['Minimum','Midpoint','Maximum'][rule.thresholds.length===2?index*2:index],()=>stop.threshold,value=>{stop.threshold=value;});color(editor,`scale-${index}-color`,'Color',stop.color,value=>{stop.color=value;});});
        }else if(rule.kind==='data-bar'){
            for(const key of ['minimum','maximum']){toggle(editor,`bar-${key}-automatic`,`${words(key)}: Automatic`,rule[key]===null,value=>{rule[key]=value?null:threshold(key==='minimum'?'min':'max');showEditor();});if(rule[key]!==null)editThreshold(editor,`bar-${key}`,words(key),()=>rule[key],value=>{rule[key]=value;});}
            color(editor,'positive-color','Positive bar color',rule.positiveColor,value=>{rule.positiveColor=value;});color(editor,'negative-color','Negative bar color',rule.negativeColor,value=>{rule.negativeColor=value;});toggle(editor,'gradient','Gradient fill',rule.gradient,value=>{rule.gradient=value;});
        }else if(rule.kind==='icon-set'){
            editor.append(new Label({text:'Icons are ordered from lowest to highest threshold.'}));
            editor.append(choose('icon-count','Number of icons',String(rule.icons.length),['3','4','5'].map(value=>[value,value]),value=>{const count=Number(value);rule.icons=Array.from({length:count},(_,index)=>copy(rule.icons[index]||{icon:'Circle',color:'#379754',threshold:threshold('percent',index*100/count),strict:false}));showEditor();}));
            rule.icons.forEach((item,index)=>{editor.append(choose(`icon-${index}`,'Icon',item.icon,CONDITIONAL_ICONS.map(value=>[value,value]),value=>{item.icon=value;}));color(editor,`icon-${index}-color`,'Icon color',item.color,value=>{item.color=value;});editThreshold(editor,`icon-${index}-threshold`,'Threshold',()=>item.threshold,value=>{item.threshold=value;});toggle(editor,`icon-${index}-strict`,'Greater than (exclude equal)',item.strict,value=>{item.strict=value;});});
        }else if(rule.kind==='icon-rating'){
            editor.append(choose('rating-icon','Icon',rule.icon,CONDITIONAL_ICONS.map(value=>[value,value]),value=>{rule.icon=value;}));color(editor,'rating-color','Icon color',rule.color,value=>{rule.color=value;});
            editor.append(choose('rating-count','Number of levels',String(rule.thresholds.length+1),['3','4','5'].map(value=>[value,value]),value=>{rule.thresholds=Array.from({length:Number(value)-1},(_,index)=>({threshold:threshold('percent',(index+1)*100/Number(value)),strict:false}));showEditor();}));
            rule.thresholds.forEach((item,index)=>{editThreshold(editor,`rating-${index}`,`Level ${index+2}`,()=>item.threshold,value=>{item.threshold=value;});toggle(editor,`rating-${index}-strict`,'Greater than (exclude equal)',item.strict,value=>{item.strict=value;});});
        }
        if(Object.hasOwn(rule,'showValue'))toggle(editor,'show-value','Show cell value',rule.showValue,value=>{rule.showValue=value;});
        const actions=new Row({text:'panel:actions'});editor.append(actions);button('cancel-edit','Cancel',showManager,actions);button('save-rule','OK',async()=>{
            let command;try{command={type:draft.item?'update-conditional-format':'add-conditional-format',sheetId:projection.activeSheetId,ranges:parseSheetRanges(draft.ranges),rule:checkedConditionalRule(rule),...(draft.item?{ruleIndex:draft.item.ruleIndex}:{})};}catch(error){fail(error);return;}
            setBusy(true);try{await perform(command);showManager();await refresh(true);}catch(error){fail(error);}finally{setBusy(false);}
        },actions);
    }
    dialog.on(Message.KEY,event=>{if(event.detail?.pressed&&(event.detail.key==='Escape'||event.detail.keyCode===9)&&!styleDialog){event.preventDefault();if(draft)showManager();else close();}});
    dialog.on(Message.DESTROY,()=>{closed=true;});
    if(initialKind)startEditor(null,initialKind);else void refresh();return dialog;
}
