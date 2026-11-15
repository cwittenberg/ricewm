import {Button,Column,HexColorInput,ComboBox,Dialog,Gallery,Icons,Label,Menu,MenuItem,Message,PropertyInspector,Row,State,THEME_COLOR_SLOTS,Workspace} from '@ricewm/app';
import {DESIGN_THEMES,DESIGN_COLORS,DESIGN_FONTS,DESIGN_EFFECTS,designVariants,designThemeDefinition} from './design-presets.mjs';

const fontChoices=[...new Set(DESIGN_FONTS.flatMap(f=>[f.major.latin,f.minor.latin]))];
const slotNames=['Text/Background – Dark 1','Text/Background – Light 1','Text/Background – Dark 2','Text/Background – Light 2','Accent 1','Accent 2','Accent 3','Accent 4','Accent 5','Accent 6','Hyperlink','Followed Hyperlink'];
const sameColors=(a,b)=>Array.isArray(a)&&Array.isArray(b)&&a.length===b.length&&a.every((v,i)=>v.toUpperCase()===b[i].toUpperCase());
function themePreview(item){
    const {colors}=item;
    const preview=new Workspace({width:78,height:50,transparent:true,enabled:false,render(view){
        const w=view.width,h=view.height;
        view.panel([0,0,w,h],'#C8C8C8');view.panel([1,1,w-2,h-2],colors[1]);
        const sample=view.slots.get('sample')?.control??new Label({text:'Aa',enabled:false,state:State.FLAT|State.START_ALIGNED,
            documentFont:{family:'Inter',size_pt:14.25,weight:400,italic:false,underline:false},documentPaint:{foreground:colors[0],background:colors[1]}});
        view.place('sample',sample,[8,h*.42-15,w-16,30]);
        for(let i=0;i<6;i++)view.panel([8+i*(w-16)/6,h-11,(w-16)/6-1.5,5],colors[4+i]);
    }});
    return preview;
}
function colorPreview(item){
    return new Workspace({width:220,height:32,transparent:true,enabled:false,render(view){
        for(let i=0;i<8;i++)view.panel([i*12,9,11,14],item.colors[i+2]);
        view.label('name',item.title,[104,2,116,28]);
    }});
}

/** Theme choices are data; all durable edits use the same document port as the
 * ribbon, inspector and agent. Dialog state is only a cancellable draft. */
export function createDesignThemeTools({root,apply,browseTheme=null,notice=()=>{}}){
    let scene=null,ready=false,scope=null,pending=false,variants=[],variantKey=null,dialog=null,session=null;
    const enabled=()=>Boolean(ready&&!pending&&scene?.canSetTheme);
    const componentEnabled=()=>enabled()&&Boolean(scene?.designTheme);
    const refresh=()=>{
        themes.enabled=enabled();variantGallery.enabled=componentEnabled();browse.enabled=enabled()&&typeof browseTheme==='function';
        for(const menu of [colorsMenu,fontsMenu,effectsMenu])menu.enabled=componentEnabled();
        if(session){session.inspector.enabled=componentEnabled()&&!session.pending;session.save.enabled=session.inspector.enabled;session.cancel.enabled=!session.pending;}
    };
    async function commit(command){
        if(!enabled())return false;pending=true;refresh();
        try{return await apply([{...command,slide:scene.slideId}]);}
        catch(error){notice(error.message||'Theme could not be changed');return false;}
        finally{pending=false;refresh();}
    }
    function gallery({name,width,count,provider,preview,onSelect,tileWidth=84,tileHeight=56,itemRole='button'}){
        const control=new Gallery({name,text:`panel:fixed-height:64:fixed:width=${width}:gap=0:padding=0:flush`,presentation:'tiles',itemRole,pageSize:count,
            tileWidth,tileHeight,gap:1,chrome:false,previewPadding:tileHeight===56?3:0,showSearch:false,showTitles:false,provider,renderPreview:preview,onSelect,enabled:false});
        control.status.visible=false;return control;
    }
    const themes=gallery({name:'design-theme-gallery',width:679,count:8,provider:async()=>({items:DESIGN_THEMES.map(t=>({...t,title:t.name})),next:null}),preview:themePreview,
        onSelect:item=>commit({type:'set-presentation-theme',theme:designThemeDefinition(DESIGN_THEMES.find(t=>t.id===item.id))})});
    const variantGallery=gallery({name:'design-variant-gallery',width:339,count:4,provider:async()=>({items:variants.map((v,i)=>({...v,id:String(i),title:v.name})),next:null}),preview:themePreview,
        onSelect:item=>commit({type:'set-theme-colors',name:item.name,colors:Object.fromEntries(THEME_COLOR_SLOTS.map((slot,i)=>[slot,item.colors[i]]))})});
    const browse=new Button({name:'browse-theme',text:'Browse for Themes…',icon:Icons.Appearance,state:State.START_ALIGNED,enabled:false,visible:typeof browseTheme==='function'});
    browse.on(Message.ACTIVATE,async()=>{
        if(!browse.enabled)return false;pending=true;refresh();
        try{return await browseTheme();}catch(error){notice(error.message||'Theme file could not be opened');return false;}finally{pending=false;refresh();}
    });
    function menu(name,label,icon){return new Menu({name,text:label,icon,tooltip:label,accessibleName:label,enabled:false,state:State.DISPLAY});}
    const colorsMenu=menu('theme-colors','Colors',Icons.Appearance),fontsMenu=menu('theme-fonts','Fonts',Icons.TextColor),effectsMenu=menu('theme-effects','Effects',Icons.ShapeShadow);
    const colorRows=gallery({name:'theme-color-scheme-gallery',width:240,count:16,tileWidth:220,tileHeight:28,itemRole:'menuitem',
        provider:async()=>({items:DESIGN_COLORS.map((c,i)=>({...c,id:String(i),title:c.name})),next:null}),preview:colorPreview,
        onSelect:item=>commit({type:'set-theme-colors',name:item.name,colors:Object.fromEntries(THEME_COLOR_SLOTS.map((slot,i)=>[slot,item.colors[i]]))})});
    colorRows.text='panel:fixed:width=240:menu-grid:columns=1:gap=0:padding=0:flush';colorRows.items.gap=0;
    colorsMenu.append(new Column({text:'panel:fixed:gap=0:flush'},[colorRows]));
    const customColors=colorsMenu.append(new MenuItem({name:'customize-theme-colors',text:'Customize Colors…'}));
    const fontItems=[];
    for(const [i,fonts]of DESIGN_FONTS.entries()){
        const item=fontsMenu.append(new MenuItem({name:'theme-font-pair-'+i,text:fonts.name+'\n   '+fonts.major.latin+' / '+fonts.minor.latin}));
        item.on(Message.ACTIVATE,()=>componentEnabled()?commit({type:'set-theme-fonts',fonts}):false);fontItems.push(item);
    }
    const customFonts=fontsMenu.append(new MenuItem({name:'customize-theme-fonts',text:'Customize Fonts…'}));
    const effectItems=[];
    for(const [i,format]of DESIGN_EFFECTS.entries()){
        const item=effectsMenu.append(new MenuItem({name:'theme-effect-'+i,text:format.name}));item.on(Message.ACTIVATE,()=>componentEnabled()?commit({type:'set-theme-format',format}):false);effectItems.push(item);
    }
    const closeDialog=()=>{if(dialog)root.remove(dialog,{destroy:true});dialog=null;session=null;};
    function openDialog(kind){
        if(!componentEnabled()||dialog)return false;
        const original=structuredClone(scene.designTheme),target=scene.slideId,documentScope=scope;
        const color=kind==='colors',values=color?{name:original.colorsName,...Object.fromEntries(THEME_COLOR_SLOTS.map((slot,i)=>[slot,original.colors[i]]))}:{name:original.fontName,major:original.majorFont,minor:original.minorFont};
        const title=color?'Create New Theme Colors':'Create New Theme Fonts',error=new Label({name:'theme-dialog-error',text:''});
        const checkName=value=>typeof value==='string'&&value.trim()&&value.length<=256&&!/[\x00-\x1f\x7f]/u.test(value)?null:'Enter a name';
        const properties=[{id:'name',label:'Name:',type:'text',layout:'inline',labelWidth:color?188:110,controlWidth:color?316:240,validate:checkName},...(color?THEME_COLOR_SLOTS.map((slot,i)=>({id:slot,label:slotNames[i],type:'custom',layout:'inline',labelWidth:188,controlWidth:116,initial:values[slot],createEditor:()=>new HexColorInput({name:'theme-color-'+slot,value:values[slot],showHash:false}),validate:v=>{try{HexColorInput.parse(v);return null;}catch(error){return error.message;}}})):
            ['major','minor'].map((id,i)=>({id,label:i?'Body:':'Heading:',type:'custom',layout:'inline',labelWidth:110,controlWidth:240,initial:values[id],validate:checkName,createEditor:()=>new ComboBox({name:'theme-'+id+'-font',items:fontChoices,value:values[id],validate:v=>checkName(v),onCommit:()=>true})})))];
        const colorLeft=['dk1','lt1','dk2','lt2','hlink','folHlink'];
        const rows=color?[...colorLeft.map((slot,i)=>[slot,'accent'+(i+1)]),['name']]:[['major'],['minor'],['name']];
        const currentSession={scope:documentScope,master:original.master,original,pending:false};
        const inspector=new PropertyInspector({name:'theme-dialog-properties',selection:['theme'],properties,rows,read:(_,id)=>values[id],apply:async({values:changes})=>{
            if(session!==currentSession||scope!==documentScope||scene?.designTheme?.master!==original.master||JSON.stringify(scene.designTheme)!==JSON.stringify(original))throw Error('The theme changed. Reopen this dialog before applying the draft.');
            const command=color?{type:'set-theme-colors',slide:target,name:changes.name??values.name,colors:Object.fromEntries(THEME_COLOR_SLOTS.filter(slot=>Object.hasOwn(changes,slot)).map(slot=>[slot,HexColorInput.parse(changes[slot])]))}:
                {type:'set-theme-fonts',slide:target,name:changes.name??values.name,...Object.fromEntries(['major','minor'].filter(id=>Object.hasOwn(changes,id)).map(id=>[id,changes[id]]))};
            // A font scheme can be renamed while keeping both explicit faces.
            if(!color&&!Object.hasOwn(command,'major')&&!Object.hasOwn(command,'minor')){command.major=values.major;command.minor=values.minor;}
            if(!color){
                const preset=DESIGN_FONTS.find(f=>f.name.toLowerCase()===command.name.toLowerCase());
                if(preset){const fonts=structuredClone(preset);fonts.name=command.name;fonts.major.latin=changes.major??values.major;fonts.minor.latin=changes.minor??values.minor;return await apply([{type:'set-theme-fonts',slide:target,fonts}]);}
            }
            return await apply([command]);
        }});
        inspector.form.text='panel:fixed:flush:gap=4';
        const samples=[];
        if(!color){
            for(const [id,size]of [['major',20],['minor',14]]){
                const sample=new Label({name:'theme-'+id+'-sample',text:(id==='major'?'Heading sample  ':'Body sample  ')+values[id],documentFont:{family:'Inter',size_pt:size*.75,weight:400,italic:false,underline:false}});
                const update=()=>{sample.text=(id==='major'?'Heading sample  ':'Body sample  ')+inspector.model.get(id);};
                const stop=inspector.model.subscribe(update);sample.on(Message.DESTROY,stop);samples.push(sample);
            }
            const nameField=inspector.form.fields.get('name');inspector.form.remove(nameField);
            for(const sample of samples)inspector.form.append(sample);inspector.form.append(nameField);
        }
        inspector.button.visible=false;inspector.status.visible=false;
        const cancel=new Button({name:'theme-dialog-cancel',text:'Cancel'}),save=new Button({name:'theme-dialog-save',text:'Save',accent:true});
        cancel.on(Message.ACTIVATE,()=>{if(!currentSession.pending)closeDialog();});
        save.on(Message.ACTIVATE,async()=>{
            if(session!==currentSession||currentSession.pending||!save.enabled)return false;
            currentSession.pending=true;error.text='';const work=inspector.commit();save.enabled=cancel.enabled=inspector.enabled=false;
            try{const result=await work;if(session===currentSession&&(result||!inspector.changedIds.size))closeDialog();return result;}
            catch(failure){if(session===currentSession)error.text=failure.message;return false;}
            finally{if(session===currentSession){currentSession.pending=false;refresh();}}
        });
        dialog=new Dialog({name:'theme-'+kind+'-dialog',text:title,modal:true},[new Column({text:`panel:fixed:width=${color?680:420}:flush:gap=6`},[
            inspector,new Label({text:color?'Applies to this presentation’s theme colours.':'Heading and body fonts for this presentation.'}),error,new Row({text:'panel:actions'},[cancel,save])])]);
        dialog.on(Message.KEY,event=>{if(event.defaultPrevented||event.detail?.pressed===false)return;if(event.detail?.key==='Escape'||event.detail?.keyCode===9){event.preventDefault();event.stopPropagation();if(!currentSession.pending)closeDialog();}});
        Object.assign(currentSession,{inspector,save,cancel});session=currentSession;root.append(dialog);inspector.form.fields.get(color?'dk1':'major').focus();return true;
    }
    customColors.on(Message.ACTIVATE,()=>openDialog('colors'));customFonts.on(Message.ACTIVATE,()=>openDialog('fonts'));
    void themes.search('').catch(error=>notice(error.message));void colorRows.search('').catch(error=>notice(error.message));
    return {themes,variantGallery,colorsMenu,fontsMenu,effectsMenu,browse,
        groups:[{id:'themes',title:'Themes',controls:[themes,browse],primary:1,rowBreak:1},{id:'variants',title:'Variants',controls:[variantGallery],primary:1}],
        variantCommands:[colorsMenu,fontsMenu,effectsMenu],
        close:closeDialog,
        setState({current=null,usable=false,documentScope=null}={}){
            const master=current?.designTheme?.master;
            if(session&&(documentScope!==scope||master!==session.master))closeDialog();
            scene=current;ready=usable;scope=documentScope;
            const data=scene?.designTheme;
            themes.setSelection(DESIGN_THEMES.find(t=>t.name===data?.name||t.id==='rice'&&data?.name==='DeckCraft')?.id??null);
            const next=data?designVariants(data):[],key=JSON.stringify(next);variants=next;
            if(key!==variantKey){variantKey=key;void variantGallery.search('').catch(error=>notice(error.message));}
            variantGallery.setSelection(data?String(variants.findIndex(v=>sameColors(v.colors,data.colors))):null);
            colorRows.setSelection(data?String(DESIGN_COLORS.findIndex(v=>v.name===data.colorsName&&sameColors(v.colors,data.colors))):null);
            for(const [i,item]of fontItems.entries())item.checked=Boolean(data&&data.fontName===DESIGN_FONTS[i].name&&data.majorFont===DESIGN_FONTS[i].major.latin&&data.minorFont===DESIGN_FONTS[i].minor.latin);
            for(const [i,item]of effectItems.entries())item.checked=data?.formatName===DESIGN_EFFECTS[i].name;
            colorRows.enabled=Boolean(usable&&!pending&&data);refresh();
        },
    };
}
