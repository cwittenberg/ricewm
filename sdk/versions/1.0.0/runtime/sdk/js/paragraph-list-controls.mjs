import {Button,Menu,MenuItem,Row} from './controls.mjs';
import {Message} from './messages.mjs';
import {State} from './control.mjs';
import {PARAGRAPH_LIST_STYLES,PARAGRAPH_BULLET_GLYPHS,validParagraphFormatMetadata} from './presentation/paragraph-format.mjs';
import {createParagraphFormatDialog} from './paragraph-format-dialog.mjs';

/** Retained native commands. The caller owns paragraph selection, source
 * edits and history; checked state follows committed or caller-owned drafts. */
export function createParagraphListControls({name='paragraph',root=null,onChange,onError=()=>{},onDialogClose=()=>{}}={}){
    if(typeof onChange!=='function'||typeof onError!=='function'||typeof onDialogClose!=='function'||root!==null&&typeof root.append!=='function')throw TypeError('Paragraph controls need change and error callbacks');
    let enabled=false,paragraphs=[],selectionKey=null,dialog=null,dialogKey=null,pending=false;
    const closeDialog=()=>{const current=dialog;dialog=null;if(current&&!current.destroyed)root.remove(current,{destroy:true});if(current&&enabled)onDialogClose();};
    const configure=kind=>{
        if(kind!==undefined&&!['bullet','number'].includes(kind))throw TypeError('Paragraph dialog kind must be bullet or number');
        if(!enabled||dialog||!root)return false;
        dialogKey=selectionKey;
        dialog=createParagraphFormatDialog({name:`${name}-format`,kind,paragraphs,onClose:closeDialog,onApply:async format=>{
            if(!enabled||selectionKey!==dialogKey)throw Error('The text selection changed. Reopen Bullets and Numbering.');
            pending=true;try{return await onChange(format);}finally{pending=false;}
        }});
        root.append(dialog);return true;
    };
    const invoke=format=>{if(enabled)Promise.resolve().then(()=>onChange(format)).catch(onError);};
    const choices={bullet:[['disc','Solid round bullets'],['circle','Hollow round bullets'],['square','Square bullets']],number:[['decimal','1. 2. 3.'],['lower-alpha','a. b. c.'],['upper-alpha','A. B. C.'],['lower-roman','i. ii. iii.'],['upper-roman','I. II. III.']]};
    const lists=['bullet','number'].map(kind=>{
        const suffix=kind==='bullet'?'bullets':'numbering',label=kind==='bullet'?'Bullets':'Numbering',icon=kind==='bullet'?155:156;
        let last={kind,style:PARAGRAPH_LIST_STYLES[kind][0]};
        const button=new Button({name:`${name}-${suffix}`,text:'',icon,state:State.DISPLAY,accessibleName:label,tooltip:label,enabled:false});
        const menu=new Menu({name:`${name}-${suffix}-styles`,text:'panel:flush:fixed:gap=0:width=20',icon:74,state:State.DISPLAY,accessibleName:label+' styles',tooltip:label+' styles',enabled:false});
        const none=menu.append(new MenuItem({name:`${name}-${suffix}-none`,text:'None'}));none.on(Message.ACTIVATE,()=>invoke({list:{kind:'none'}}));
        const items=choices[kind].map(([style,text])=>{
            const item=menu.append(new MenuItem({name:`${name}-${suffix}-${style}`,text,icon}));
            item.on(Message.ACTIVATE,()=>{last={kind,style};invoke({list:last});});return {style,item};
        });
        if(kind==='bullet')for(const[glyphIndex,glyph]of PARAGRAPH_BULLET_GLYPHS.entries()){
            const item=menu.append(new MenuItem({name:`${name}-bullets-glyph-${glyphIndex}`,text:`${glyph}  Bullet`}));
            item.on(Message.ACTIVATE,()=>{last={kind:'bullet',style:'custom',glyph};invoke({list:last,marker:{fontFamily:'Arial'}});});
            items.push({glyph,item});
        }
        if(root){const item=menu.append(new MenuItem({name:`${name}-${suffix}-format`,text:'Bullets and Numbering…'}));item.on(Message.ACTIVATE,()=>configure(kind));}
        button.on(Message.ACTIVATE,()=>{const list=paragraphs.length&&paragraphs.every(p=>p.list?.kind===kind)?{kind:'none'}:last;invoke({list,...(list.style==='custom'?{marker:{fontFamily:'Arial'}}:{})});});
        const control=new Row({name:`${name}-${suffix}-control`,text:'panel:flush:fixed:gap=0:width=52'},[button,menu]);
        return {control,button,menu,items,none,kind};
    });
    const indent=(delta,label,text)=>{
        const button=new Button({name:`${name}-${delta<0?'decrease':'increase'}-indent`,text:'',icon:delta<0?31:32,state:State.DISPLAY,accessibleName:label,tooltip:label,enabled:false});
        button.on(Message.ACTIVATE,()=>invoke({levelDelta:delta}));return button;
    };
    const decrease=indent(-1,'Decrease list level','←'),increase=indent(1,'Increase list level','→');
    return Object.freeze({controls:Object.freeze([...lists.map(list=>list.control),decrease,increase]),closeDialog,openDialog:configure,setState({enabled:ready=false,paragraphs:source=[],selectionKey:key=null}={}){
        if(typeof ready!=='boolean'||key!==null&&typeof key!=='string'||!Array.isArray(source)||source.length>256||source.some(p=>!validParagraphFormatMetadata(p)))throw TypeError('Invalid selected paragraph formats');
        enabled=ready;paragraphs=structuredClone(source);selectionKey=key;
        if(dialog&&!pending&&(!ready||dialogKey!==key))closeDialog();
        for(const list of lists){
            list.button.enabled=ready;list.menu.enabled=ready;
            list.button.setProperty('checked',source.length>0&&source.every(p=>p.list?.kind===list.kind));
            list.none.setProperty('checked',source.length>0&&source.every(p=>p.list?.kind==='none'));
            for(const{style,glyph,item}of list.items)item.setProperty('checked',source.length>0&&source.every(p=>p.list?.kind===list.kind&&(glyph?p.list.style==='custom'&&p.list.glyph===glyph:p.list.style===style)));
        }
        decrease.enabled=ready&&source.some(p=>p.level>0);increase.enabled=ready&&source.some(p=>p.level<8);
    }});
}
createParagraphListControls.controlFactory=Object.freeze({topics:['documents','forms'],constructor:'createParagraphListControls({name?,root?,onChange:async(format)=>{},onError?,onDialogClose?})',methods:'Append controls to a Ribbon or toolbar. setState({enabled,paragraphs:[paragraphFormat,...],selectionKey?:string|null}) follows caller-owned selection. Native split commands toggle lists; menus include nine bullet glyphs and named list presets. With an app-owned root, Bullets and Numbering opens the native glyph/number/size/color draft dialog. A changed selectionKey closes that draft; openDialog(kind?:"bullet"|"number") opens that same checked draft from context or menu commands and returns false when unavailable; omitted kind follows the selected list. closeDialog() cancels it. onDialogClose restores caller-owned editor focus. Indent emits levelDelta +/-1. Caller owns checked source commands and one Undo transaction. Disabled controls do not edit.'});
