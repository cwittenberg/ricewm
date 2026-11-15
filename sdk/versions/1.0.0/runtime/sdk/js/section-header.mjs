import {Button,Column,Container,Label,Row,Stack,TextField} from './controls.mjs';
import {State} from './control.mjs';
import {Message} from './messages.mjs';
import {ReorderButton} from './reorder-controller.mjs';
import {Icons} from './icons.mjs';

/** Native collection header. View state and document transactions stay with
 * the caller; collapse never commits a document edit. Rename keeps its draft
 * on rejection and commits only trimmed, nonempty names.
 */
export class SectionHeader extends Column {
    static family='composite';
    static constructorOptions=Object.freeze(['title','count','collapsed','collapsible','renameable','height','onToggle','onRename','renameValue']);
    static authoring=Object.freeze({topics:['navigation','documents','visual'],constructor:'new SectionHeader({title:"Overview",count:8,collapsed:false,collapsible:true,renameable:true,height:32,onToggle:collapsed=>{},onRename:async name=>{...}})',methods:'setSection({title,count,collapsed,collapsible,renameable}), beginRename(name?), cancelRename(), commitRename(), setDropPosition(null|"before"|"after"); button is a ReorderButton for whole-group drag, toggleButton owns collapse, field owns rename. Double-click/Enter begin renaming only when renameable and onRename exists. Enter/blur commits after focus; Escape restores the title. False/rejected commits retain drafts. Caller owns accepted names, errors, collapse, stable group IDs, ordering and Undo/Redo. Set collapsible:false for static sidebar headers.'});
    constructor({title='',count=0,collapsed=false,collapsible=true,renameable=false,height=32,onToggle=null,onRename=null,renameValue=null,...options}={}) {
        if(!Number.isInteger(height)||height<20||height>100||[onToggle,onRename,renameValue].some(callback=>callback!==null&&typeof callback!=='function'))throw TypeError('Invalid section header');
        super({text:`panel:flush:gap=0:accessory-height=${height}`,...options});
        Object.assign(this,{onToggle,onRename,renameValue,editing:false,pending:false,epoch:0,focusSeen:false});
        this.toggleButton=new Button({name:`${this.name}-toggle`,text:'panel:flush',icon:Icons.ChevronDown,state:State.FLAT,accessibleName:'Collapse section'});
        this.label=new Label({name:`${this.name}-title`,text:'',enabled:false,state:State.EXPANSIVE,documentFont:{family:'sans-serif',size_pt:height>=32?9.75:9,weight:height>=32?700:400,italic:false,underline:false}});
        this.countLabel=new Label({name:`${this.name}-count`,text:'',enabled:false,documentFont:{family:'sans-serif',size_pt:9,weight:400,italic:false,underline:false}});
        this.field=new TextField({name:`${this.name}-rename`,value:'',visible:false,state:State.EXPANSIVE});
        this.button=new ReorderButton({name:`${this.name}-body`,text:'panel:flush',state:State.FLAT|State.EXPANSIVE},[new Container({text:'panel:flush:content-card',state:State.EXPANSIVE},[new Row({text:'panel:flush:gap=6'},[this.label,this.countLabel])])]);
        this.row=new Row({text:`panel:flush:gap=0:accessory-height=${height}`},[
            new Container({text:'panel:flush:width=28',state:State.FLAT},[this.toggleButton]),this.button,this.field]);
        this.marker=new Container({text:'positioned-item:0:0:1000:60',state:State.PRESSED|State.ACCENT,enabled:false,visible:false});
        this.append(new Stack({text:'panel:flush',state:State.EXPANSIVE},[this.row,this.marker]));
        this.setSection({title,count,collapsed,collapsible,renameable});
        this.toggleButton.on(Message.ACTIVATE,()=>{if(this.ready()&&this.collapsible&&this.count>0)this.onToggle?.(!this.collapsed);});
        let press=null,previous=null;
        this.button.on(Message.POINTER,({detail:d})=>{
            if(d?.kind==='down')press=d.button===1?{x:d.x,y:d.y,time:performance.now()}:null;
            if(d?.kind==='move'&&press&&Math.hypot(d.x-press.x,d.y-press.y)>6){press=previous=null;}
        });
        this.button.on(Message.BLUR,()=>{press=previous=null;});
        this.button.on(Message.ACTIVATE,()=>{
            const click=press;press=null;if(!click||!this.ready())return;
            if(previous&&click.time-previous.time<=450&&Math.hypot(click.x-previous.x,click.y-previous.y)<=6){previous=null;this.beginRename();}else previous=click;
        });
        this.button.on(Message.KEY,event=>{if(event.detail?.pressed&&['Enter','F2'].includes(event.detail.key)&&this.beginRename()){event.preventDefault();event.stopPropagation();}});
        this.field.on(Message.FOCUS,()=>{if(this.editing)this.focusSeen=true;});
        this.field.on(Message.BLUR,()=>{if(this.editing&&this.focusSeen)void this.commitRename();});
        this.field.on(Message.KEY,event=>{
            if(!event.detail?.pressed)return;
            // Native text editing already owns these keys. Do not let its
            // Select All/Delete/arrows become collection navigation commands.
            event.stopPropagation();
            const key=event.detail.key||({9:'Escape',36:'Enter'})[event.detail.keyCode];
            if(key==='Escape'){event.preventDefault();event.stopPropagation();this.cancelRename();}
            if(key==='Enter'){event.preventDefault();event.stopPropagation();void this.commitRename();}
        });
    }
    ready() {for(let p=this;p;p=p.parent)if(p.destroyed||!p.visible||!p.enabled)return false;return !this.pending;}
    setSection({title=this.title,count=this.count,collapsed=this.collapsed,collapsible=this.collapsible,renameable=this.renameable}={}) {
        if(typeof title!=='string'||title.length>65536||!Number.isInteger(count)||count<0||count>65536||![collapsed,collapsible,renameable].every(v=>typeof v==='boolean'))throw TypeError('Invalid section state');
        if(this.editing&&title!==this.title)this.cancelRename();
        Object.assign(this,{title,count,collapsed,collapsible,renameable});
        this.label.text=title;this.countLabel.text=`(${count})`;this.toggleButton.icon=collapsed?Icons.SubmenuRight:Icons.ChevronDown;
        this.toggleButton.enabled=collapsible&&count>0;this.toggleButton.setProperty('accessibleName',`${collapsed?'Expand':'Collapse'} ${title}`);
        this.button.setProperty('accessibleName',`${title}, ${count} items`);
    }
    beginRename(name) {
        if(name===undefined&&this.renameValue&&this.ready()&&this.renameable&&!this.editing) {
            const epoch=++this.epoch;this.pending=true;
            return Promise.resolve().then(()=>this.renameValue()).then(value=>{
                if(this.destroyed||epoch!==this.epoch)return false;
                this.pending=false;return this.beginRename(value);
            }).catch(error=>{if(!this.destroyed&&epoch===this.epoch)this.button.setProperty('tooltip',String(error?.message||error));return false;})
                .finally(()=>{if(!this.destroyed&&epoch===this.epoch)this.pending=false;});
        }
        name??=this.title;
        if(!this.ready()||!this.renameable||!this.onRename||this.editing||typeof name!=='string')return false;
        this.epoch++;this.editing=true;this.focusSeen=false;this.field.setProperty('value',name);this.field.visible=true;this.button.visible=false;this.field.setProperty('focused',true);return true;
    }
    cancelRename() {this.epoch++;this.editing=false;this.pending=false;this.focusSeen=false;this.field.enabled=true;this.field.visible=false;this.button.visible=true;}
    async commitRename() {
        if(!this.editing||this.pending)return false;
        const name=this.field.getProperty('value').trim();
        if(!name){this.cancelRename();return false;}
        if(name===this.title){this.cancelRename();return false;}
        const epoch=this.epoch;this.pending=true;this.field.enabled=false;
        try {
            if(await this.onRename(name)===false)return false;
            if(this.destroyed||epoch!==this.epoch)return false;
            this.cancelRename();this.setSection({title:name});return true;
        } catch(error) {if(!this.destroyed&&epoch===this.epoch)this.field.setProperty('tooltip',String(error?.message||error));return false;}
        finally {if(!this.destroyed&&epoch===this.epoch){this.pending=false;this.field.enabled=true;}}
    }
    setDropPosition(position) {if(![null,'before','after'].includes(position))throw TypeError('Invalid section insertion');this.marker.visible=position!==null;this.marker.text=`positioned-item:0:${position==='after'?940:0}:1000:60`;}
}
