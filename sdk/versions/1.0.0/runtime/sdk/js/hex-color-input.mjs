import {Row,TextField,Container} from './controls.mjs';
import {State} from './control.mjs';
import {Message} from './messages.mjs';
import {SceneSurface} from './scene-surface.mjs';
import {SceneBuilder} from './scene-builder.mjs';

/** Inline native hex entry and source-independent color preview. Drafts never
 * mutate a caller's document; completed input uses its ordinary commit callback. */
export class HexColorInput extends Row {
    static family='composite';
    static constructorOptions=Object.freeze(['value','allowNone','showHash','onCommit']);
    static properties=Object.freeze({readOnly:{default:false,reflect:false,validate:Boolean},value:{default:'#000000',reflect:false,validate:value=>HexColorInput.parse(value,{allowNone:true})}});
    static authoring=Object.freeze({constructor:'new HexColorInput({value:"#306BDD",allowNone:false,showHash:false,onCommit:async(value)=>{...}})',methods:'parse(text,{allowNone:false}) accepts six hex digits with an optional # and returns canonical #RRGGBB; optional empty input represents no color. setValue(value,{preserveDraft:false}), setFormValue(value,{preserveDraft:true}), commit(), cancel(). Invalid drafts remain editable; Enter commits, Escape restores. The native input and painted swatch share one value. Replaced targets invalidate pending callbacks; caller owns persistence and undo.'});
    static parse(value,{allowNone=false}={}){
        if(allowNone&&(value===null||typeof value==='string'&&!value.trim()))return null;
        if(typeof value!=='string'||!/^#?[0-9a-f]{6}$/iu.test(value.trim()))throw TypeError('Enter six hexadecimal color digits');
        return '#'+value.trim().replace(/^#/u,'').toUpperCase();
    }
    constructor({value='#000000',allowNone=false,showHash=true,onCommit=null,...options}={}){
        if(typeof allowNone!=='boolean'||typeof showHash!=='boolean'||onCommit!==null&&typeof onCommit!=='function')throw TypeError('Invalid hex color input');
        const normalized=HexColorInput.parse(value,{allowNone});
        super({text:'panel:fixed:flush:gap=4',...options,value:normalized});
        Object.assign(this,{allowNone,showHash,onCommit,dirty:false,pending:false,valueEpoch:0});
        this.preview=new SceneSurface({name:(this.name||'hex-color')+'-swatch',enabled:false});
        this.append(new Container({text:'panel:fixed:width=20:accessory-height=22:flush',state:State.FLAT},[this.preview]));
        this.input=this.append(new TextField({name:(this.name||'hex-color')+'-input',value:this.format(normalized),state:State.EXPANSIVE,accessibleName:options.accessibleName||'Hex color'}));
        this.input.on(Message.VALUE_CHANGED,event=>{event.stopPropagation();this.dirty=true;this.error=null;this.input.setProperty('invalid',false);this.paintDraft();this.emit(Message.CHANGE,{draft:true,value:this.input.value});});
        this.input.on(Message.ACTIVATE,event=>{event.stopPropagation();return this.commit();});
        this.on(Message.KEY,event=>{
            if(event.detail?.pressed===false||event.target!==this.input)return;
            const key=event.detail;
            if(key.key==='Escape'||key.keyCode===9){if(!this.dirty&&!this.error)return;event.preventDefault();event.stopPropagation();this.cancel();this.emit(Message.CHANGE,{draft:true,value:this.value});}
            else if(key.key==='Enter'||[36,104].includes(key.keyCode)){event.preventDefault();event.stopPropagation();return this.commit();}
        });
        this.cancel();this.syncAvailability();
    }
    get value(){return this.getProperty('value');}
    set value(value){this.setValue(value);}
    format(value){return value===null?'':this.showHash?value:value.slice(1);}
    canCommit(){return this.enabled&&this.visible&&!this.pending&&!(this.state&State.BUSY)&&!this.getProperty('readOnly');}
    syncAvailability(){if(this.input){this.input.enabled=this.enabled&&!(this.state&State.BUSY)&&!this.pending;this.input.setProperty('readOnly',this.getProperty('readOnly'));}}
    setProperty(name,value,options){
        if(name==='value'&&this.input){this.setValue(value);return true;}
        const changed=super.setProperty(name,value,options);
        if(['state','enabled','busy','readOnly'].includes(name))this.syncAvailability();return changed;
    }
    setValue(value,{preserveDraft=false}={}){
        value=HexColorInput.parse(value,this);if(!preserveDraft||value!==this.value)this.valueEpoch++;
        super.setProperty('value',value);if(!(preserveDraft&&(this.dirty||this.pending)))this.cancel();
    }
    setFormValue(value,{preserveDraft=true}={}){
        if(preserveDraft&&this.dirty)return;
        try{this.setValue(value,{preserveDraft});}
        catch{this.input.value=String(value??'');this.dirty=true;this.paintDraft();}
    }
    paintDraft(){
        let value;try{value=HexColorInput.parse(this.input.value,this);}catch{}
        const frame=new SceneBuilder(18,18).rect([0,0,18,18],0xff808080);
        if(value===undefined)frame.rect([1,1,16,16],0xffc8c8c8);
        else if(value!==null){const rgb=parseInt(value.slice(1),16);frame.rect([1,1,16,16],(0xff000000|((rgb&255)<<16)|(rgb&0xff00)|(rgb>>>16))>>>0);}
        else{frame.rect([1,1,16,16],0xffffffff);frame.line([1,17],[17,1],1,0xff0000ff);}
        this.preview.submit(frame.finish());
    }
    cancel(){if(!this.input)return;this.dirty=false;this.error=null;this.input.value=this.format(this.value);this.input.setProperty('invalid',false);this.input.setProperty('tooltip',null);this.paintDraft();}
    async commit(){
        if(!this.canCommit())return false;const epoch=this.valueEpoch,draft=this.input.value;
        try{
            const value=HexColorInput.parse(draft,this);if(value===this.value){this.cancel();return true;}
            this.pending=true;this.syncAvailability();const result=await this.onCommit?.(value);
            if(result===false)throw Error('Change was not applied');if(this.destroyed||epoch!==this.valueEpoch)return false;
            super.setProperty('value',value);const newer=this.input.value!==draft;if(!newer)this.cancel();
            this.emit(Message.VALUE_CHANGED,{value});if(newer)this.emit(Message.CHANGE,{draft:true,value:this.input.value});return true;
        }catch(error){if(!this.destroyed&&epoch===this.valueEpoch){this.error=error.message;this.input.setProperty('invalid',true);this.input.setProperty('tooltip',this.error);}return false;}
        finally{this.pending=false;if(!this.destroyed)this.syncAvailability();}
    }
}
