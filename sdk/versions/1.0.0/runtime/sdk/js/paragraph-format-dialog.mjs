import {Button,Column,Dialog,Label,Menu,MenuItem,Row,TextField} from './controls.mjs';
import {Message} from './messages.mjs';
import {PARAGRAPH_BULLET_GLYPHS,PARAGRAPH_MARKER_COLORS,validBulletGlyph,validParagraphFormatMetadata,paragraphListLabels} from './presentation/paragraph-format.mjs';

const numberStyles=[['decimal','1. 2. 3.'],['decimal-parenthesis','1) 2) 3)'],['upper-roman','I. II. III.'],['lower-roman','i. ii. iii.'],['upper-alpha','A. B. C.'],['lower-alpha-parenthesis','a) b) c)'],['lower-alpha','a. b. c.']];
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);

/** A native draft. Only Apply/None invokes the caller's source transaction. */
export function createParagraphFormatDialog({name='paragraph-format',paragraphs=[],kind,onApply,onClose}={}){
    if(!Array.isArray(paragraphs)||paragraphs.length>256||paragraphs.some(p=>!validParagraphFormatMetadata(p))||
        typeof onApply!=='function'||typeof onClose!=='function'||kind!==undefined&&!['bullet','number'].includes(kind))throw TypeError('Invalid paragraph formatting dialog');
    const initial=paragraphs[0]||{level:0,list:{kind:'none'},marginLeft:0,marginRight:0,indent:0};
    let mode=kind??(initial.list?.kind==='number'?'number':'bullet'),pending=false,listTouched=false,colorTouched=false;
    let chosenColor=initial.marker?.color??null,numberStyle=initial.list?.kind==='number'?initial.list.style:'decimal';
    const glyphValue=initial.list?.kind==='bullet'?paragraphListLabels([{paragraphFormat:initial}])[0]:'•';
    const glyph=new TextField({name:`${name}-glyph`,value:glyphValue,accessibleName:'Custom bullet character'});
    const size=new TextField({name:`${name}-size`,value:initial.marker?.sizePercent===undefined?'100':String(initial.marker.sizePercent),accessibleName:'Bullet or number size as a percentage of text'});
    const start=new TextField({name:`${name}-start`,value:String(initial.startAt??1),accessibleName:'Start numbering at'});
    const error=new Label({name:`${name}-error`,text:''}),controls=[],glyphButtons=[],numberButtons=[];
    const color=new Menu({name:`${name}-color`,text:'Automatic',accessibleName:'Bullet or number color'});
    const colors=[[null,'Automatic'],...PARAGRAPH_MARKER_COLORS.map(slot=>[slot,slot==='tx1'?'Text':`Accent ${slot.slice(-1)}`])];
    const colorItems=colors.map(([value,label])=>{
        const item=color.append(new MenuItem({name:`${name}-color-${value??'automatic'}`,text:label}));
        item.on(Message.ACTIVATE,()=>{chosenColor=value;colorTouched=true;update();});return {item,value};
    });
    const bulletPanel=new Column({name:`${name}-bullet-panel`}),numberPanel=new Column({name:`${name}-number-panel`});
    const rowGroups=(choices,panel,width,create)=>{
        for(let i=0;i<choices.length;i+=width)panel.append(new Row({text:'panel:flush:gap=4'},choices.slice(i,i+width).map(create)));
    };
    rowGroups(PARAGRAPH_BULLET_GLYPHS,bulletPanel,5,(value)=>{
        const index=PARAGRAPH_BULLET_GLYPHS.indexOf(value),button=new Button({name:`${name}-glyph-${index}`,text:value,accessibleName:`Use ${value} as the bullet`,tooltip:`Bullet: ${value}`});
        button.on(Message.ACTIVATE,()=>{glyph.value=value;listTouched=true;update();});glyphButtons.push({value,button});controls.push(button);
        return new Row({text:'panel:flush:fixed:gap=0:width=44'},[button]);
    });
    bulletPanel.append(new Row({text:'panel:form-control'},[new Label({text:'Custom character:'}),glyph]));
    rowGroups(numberStyles,numberPanel,4,([value,label])=>{
        const button=new Button({name:`${name}-number-${value}`,text:label,accessibleName:`Numbering: ${label}`});
        button.on(Message.ACTIVATE,()=>{numberStyle=value;listTouched=true;update();});numberButtons.push({value,button});controls.push(button);return button;
    });
    numberPanel.append(new Row({text:'panel:form-control'},[new Label({text:'Start at:'}),start]));
    const tabs=['bullet','number'].map(value=>{
        const button=new Button({name:`${name}-${value}-tab`,text:value==='bullet'?'Bulleted':'Numbered'});
        button.on(Message.ACTIVATE,()=>{mode=value;update();});controls.push(button);return {value,button};
    });
    function update(){
        bulletPanel.visible=mode==='bullet';numberPanel.visible=mode==='number';
        for(const{value,button}of tabs)button.setProperty('checked',mode===value);
        for(const{value,button}of glyphButtons)button.setProperty('checked',glyph.value===value);
        for(const{value,button}of numberButtons)button.setProperty('checked',numberStyle===value);
        for(const{value,item}of colorItems)item.setProperty('checked',chosenColor===value);
        color.text=colors.find(([value])=>chosenColor===value)?.[1]??`Custom ${chosenColor}`;
    }
    glyph.on(Message.VALUE_CHANGED,()=>{listTouched=true;update();});
    const initialSize=size.value,initialStart=start.value;
    async function apply(remove=false){
        if(pending)return;
        const format={};
        if(remove)format.list={kind:'none'};
        else{
            const next=mode==='bullet'?{kind:'bullet',style:'custom',glyph:glyph.value}:{kind:'number',style:numberStyle};
            if(mode==='bullet'&&!validBulletGlyph(glyph.value)){error.text='Choose a bullet or enter one visible character.';glyph.setProperty('focused',true);return;}
            const initialGlyph=initial.list?.kind==='bullet'?glyphValue:null;
            const listChanged=listTouched||initial.list?.kind!==mode||mode==='number'&&initial.list?.style!==numberStyle||mode==='bullet'&&glyph.value!==initialGlyph||paragraphs.some(p=>!same(p.list,initial.list));
            if(listChanged)format.list=next;
            const marker={};
            if(size.value!==initialSize){
                const value=size.value.trim().replace(/%$/u,'').trim();
                if(!/^(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value)||!Number.isFinite(Number(value))||Number(value)<25||Number(value)>400){error.text='Enter a size from 25% to 400% of the text.';size.setProperty('focused',true);return;}
                marker.sizePercent=Number(value);
            }
            if(colorTouched)marker.color=chosenColor;
            if(mode==='bullet'&&listChanged)marker.fontFamily='Arial';
            if(Object.keys(marker).length)format.marker=marker;
            if(mode==='number'&&(start.value!==initialStart||listTouched||initial.list?.kind!=='number')){
                if(!/^\d{1,5}$/u.test(start.value.trim())||Number(start.value)<1||Number(start.value)>32767){error.text='Enter a starting number from 1 to 32767.';start.setProperty('focused',true);return;}
                format.startAt=Number(start.value);
            }
        }
        pending=true;for(const c of controls)c.enabled=false;
        try{if(Object.keys(format).length){const accepted=await onApply(format);if(accepted===false)throw Error('Formatting was not applied. Try again.');}onClose();}
        catch(cause){error.text=String(cause.message||cause).slice(0,400);}
        finally{pending=false;for(const c of controls)if(!c.destroyed)c.enabled=true;}
    }
    const none=new Button({name:`${name}-none`,text:'None'}),cancel=new Button({name:`${name}-cancel`,text:'Cancel'}),ok=new Button({name:`${name}-apply`,text:'OK',accent:true});
    none.on(Message.ACTIVATE,()=>apply(true));cancel.on(Message.ACTIVATE,()=>{if(!pending)onClose();});ok.on(Message.ACTIVATE,()=>apply());
    controls.push(glyph,size,start,color,none,cancel,ok);
    const dialog=new Dialog({name,text:'Bullets and Numbering',modal:true},[new Column({text:'panel:clamp:560'},[
        new Row({text:'panel:flush:gap=4'},tabs.map(t=>t.button)),bulletPanel,numberPanel,
        new Row({text:'panel:form-control'},[new Label({text:'Size (% of text):'}),size]),
        new Row({text:'panel:form-control'},[new Label({text:'Color:'}),color]),error,
        new Row({text:'panel:actions'},[none,cancel,ok]),
    ])]);
    dialog.on(Message.KEY,event=>{
        if(event.detail?.pressed===false)return;
        if(event.detail?.key==='Escape'||event.detail?.keyCode===9){event.preventDefault();event.stopPropagation();if(!pending)onClose();}
        else if(event.detail?.key==='Enter'||event.detail?.keyCode===36){event.preventDefault();event.stopPropagation();return apply();}
    });
    update();return dialog;
}
createParagraphFormatDialog.controlFactory=Object.freeze({topics:['documents','forms'],constructor:'createParagraphFormatDialog({name?,paragraphs:[paragraphFormat,...],kind?:"bullet"|"number",onApply:async(format)=>boolean|void,onClose:()=>{}})',methods:'Returns a native modal Dialog with nine glyph presets, one custom Unicode character, seven number styles, start number, marker size 25..400 percent and Automatic/Text/Accent 1..6 colors. Append to an app-owned root. Initial source properties survive unless changed. Cancel/Escape calls onClose without edits; OK/None waits for one caller-owned source transaction. Rejections retain the draft and error. Caller removes the dialog onClose and owns source selection, focus and Undo.'});
