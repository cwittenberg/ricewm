import {Stack,Column,Label,Button,Tooltip} from './controls.mjs';
import {State} from './control.mjs';
import {Message} from './messages.mjs';
import {SceneSurface} from './scene-surface.mjs';
import {SceneOverlay} from './scene-overlay.mjs';
import {SceneBuilder} from './scene-builder.mjs';
import {resolveChartTheme} from './chart-theme.mjs';

const rgba=(color,opacity=1)=>{const n=parseInt(color.slice(1),16);return ((Math.round(opacity*255)<<24)|((n&255)<<16)|(n&0xff00)|(n>>>16))>>>0;};
/** Reconciled native controls over a painted workspace. Stable keys retain
 * input identity/focus, hit testing, tool selectors and keyboard activation.
 * All geometry uses the same scene coordinate system. No DOM or compute grant. */
export class Workspace extends Stack {
    static family='composite';
    static constructorOptions=Object.freeze(['width','height','colors','render']);
    static authoring=Object.freeze({
        constructor:'new Workspace({width:1040,height:680,render(view){...}}). Surfaces follow the active theme; native controls keep host typography, input chrome, focus and editing behavior. References specify composition, not a replacement theme. Override surface colors only when explicitly requested. Workspace never skins native inputs or buttons with document fonts/paint. Import Workspace, State, Message and native inputs from @ricewm/app. No extra permissions, fonts, assets or npm.',
        rendering:'render(view) runs at startup, native resize and theme change. view.width/height and view.colors are current. Paint via panel([x,y,w,h],color,opacity), rule([x,y],[x,y]), then label(key,text,bounds,{emphasized,muted,display}), action(key,text,bounds,handler,{icon,title,selected}), place(key,nativeControl,bounds). Decorative circle() is not an input; use native CheckBox/Toggle for state choices. Call view.refresh() after model changes. Bounds are pixels, not CSS; keep targets at least 32px high. render is synchronous. Keys are unique, stable across renders. Omitted keys are hidden while preserving their drafts. Do not replace a focused editor.',
        layout:'For task/mail/catalog products allocate a 200–240px navigation rail, flexible central list and 280–320px inspector at wide widths. At narrow widths show one focused pane with named navigation/back actions; never shrink all panes. Place scrolling lists using a native ScrollView in place(). Preserve drafts/selection; contextual titles identify the selected list or record. Flat actions have actual native tooltips and focus/activation. A Workspace is presentation, not a task model or persistence layer.',
        methods:'refresh(), destroy(). SIZE_CHANGED updates actual width/height. All actions/inputs remain visible to sdk.inspect, sdk.interact and keyboard navigation. Test empty/populated, selection, resize, invalid input and save/reopen. Exact custom type or illustrations may still use scene/typography engines.',
    });
    constructor({width=1040,height=680,colors={},render,...properties}={}) {
        if(![width,height].every(v=>Number.isInteger(v)&&v>=32&&v<=4096)||typeof render!=='function')throw TypeError('Workspace needs bounded dimensions and a render function');
        super({text:'panel:flush',state:State.ENABLED|State.FLAT|State.EXPANSIVE,...properties});
        this.width=width;this.height=height;this.overrides=colors;this.colors=resolveChartTheme({},colors);this.renderer=render;this.slots=new Map();
        this.surface=new SceneSurface({name:'workspace-surface',state:State.ENABLED|State.FLAT|State.EXPANSIVE});this.append(this.surface);
        this.surface.on(Message.SIZE_CHANGED,event=>{const {width,height}=event.detail||{};if(width>0&&height>0&&(width!==this.width||height!==this.height)){this.width=width;this.height=height;this.refresh();}});
        this.on(Message.CREATED,event=>{if(event.target!==this)return;this.setTheme(this.application.theme||{});this.stopTheme=this.application.on(Message.THEME_CHANGED,palette=>this.setTheme(palette));});
        this.refresh();
    }
    setTheme(theme){this.colors=resolveChartTheme(theme,this.overrides);this.refresh();}
    refresh(){
        if(this.destroyed||this.rendering)return;
        this.rendering=true;
        try{
            this.used=new Set();this.builder=new SceneBuilder(this.width,this.height);this.panel([0,0,this.width,this.height],this.colors.background);this.renderer(this);
            this.surface.submit(this.builder.finish());
            for(const [key,slot]of this.slots)if(!this.used.has(key)){slot.overlay.visible=false;}
            if(this.slots.size>512)throw RangeError('Workspace has too many retained slots');
        }finally{this.rendering=false;}
    }
    panel(bounds,color=this.colors.panel,opacity=1){if(!Number.isFinite(opacity)||opacity<0||opacity>1)throw RangeError('Workspace opacity requires 0..1');this.builder.rect(bounds,rgba(color,opacity));return this;}
    rule(from,to,color=this.colors.border){this.builder.line(from,to,1,rgba(color));return this;}
    circle(bounds,{checked=false}={}){this.builder.ellipse(bounds,rgba(this.colors.accent),{filled:checked,width:1.3});return this;}
    place(key,control,bounds){
        if(typeof key!=='string'||!key||this.used.has(key))throw TypeError('Workspace keys must be unique');
        if(!Array.isArray(bounds)||bounds.length!==4||!bounds.every(Number.isFinite)||bounds[2]<=0||bounds[3]<=0)throw RangeError('Workspace bounds require x,y,width,height');
        let slot=this.slots.get(key);
        if(slot&&slot.control!==control)throw TypeError('Retain the same native control for a Workspace key');
        const [x,y,width,height]=bounds,normalized={x:x/this.width,y:y/this.height,width:width/this.width,height:height/this.height};
        if(!slot){const overlay=new SceneOverlay({surface:this.surface,bounds:normalized,state:State.ENABLED|State.FLAT},[control instanceof Label?new Column({text:'panel:fixed:flush:tight-stack:multiline',state:State.ENABLED|State.FLAT},[control]):control]);this.append(overlay);slot={overlay,control};this.slots.set(key,slot);}
        else{slot.overlay.setBounds(normalized);slot.overlay.visible=true;}
        // A layout owns geometry, not the native control's theme or editor.
        // Document canvas styling is an explicit per-control opt-in.
        this.used.add(key);return control;
    }
    label(key,text,bounds,{emphasized=false,muted=false,display=false}={}){
        const label=this.slots.get(key)?.control||new Label({name:key});label.text=text;
        label.setProperty('state',State.FLAT|State.START_ALIGNED|(emphasized?State.EMPHASIZED:0)|(muted?State.MUTED:0)|(display?State.DISPLAY:0));
        return this.place(key,label,bounds);
    }
    action(key,text,bounds,handler,{icon=null,title=text,selected=false}={}){
        let button=this.slots.get(key)?.control;
        if(!button){button=new Button({name:key,state:State.ENABLED|State.FLAT|State.START_ALIGNED},[new Tooltip({text:title||key})]);button.on(Message.ACTIVATE,()=>button.workspaceHandler?.());}
        button.workspaceHandler=handler;button.text=text;button.setProperty('icon',icon);button.setProperty('accessibleName',title||key);button.setProperty('selected',selected);button.children[0].text=title||key;
        return this.place(key,button,bounds);
    }
    destroy(){this.stopTheme?.();this.slots.clear();super.destroy();}
}
