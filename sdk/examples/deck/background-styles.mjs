import { Column, Gallery, Menu, MenuItem, Message, SceneBuilder, SceneSurface, State, paintBackgroundFill } from '@ricewm/app';

/** Design's native 4 × 3 theme-reference menu. Domain commits retain the
 * source bgRef rather than copying the preview's approximate swatch colour. */
export function createBackgroundStyles({apply,notice=()=>{}}) {
    let scene=null,ready=false,pending=false,catalog=[],shown=null;
    const control=new Menu({name:'background-styles',text:'',icon:186,state:State.DISPLAY,
        accessibleName:'Background Styles',tooltip:'Background Styles',enabled:false});
    const gallery=new Gallery({name:'background-style-gallery',text:'panel:menu-grid:columns=4:fixed:width=240:gap=0:flush',
        presentation:'tiles',itemRole:'menuitem',pageSize:12,tileWidth:48,tileHeight:28,showTitles:false,showSearch:false,
        provider:async()=>({items:catalog.map(item=>({id:String(item.style),title:item.label,enabled:item.available,preview:item.preview})),next:null}),
        renderPreview:item=>{const surface=new SceneSurface({name:'background-style-preview-'+item.id,accessibleName:item.title});const b=new SceneBuilder(48,28);if(item.preview)paintBackgroundFill(b,{kind:'solid',...item.preview},[0,0,48,28]);surface.submit(b.finish());return surface;},
        onSelect:async item=>{
            if(!control.enabled||pending||!catalog.find(c=>String(c.style)===item.id)?.available)return false;
            pending=true;refresh();
            try{return await apply([{type:'set-slide-background-style',slide:null,style:Number(item.id)}]);}
            catch(error){notice(error.message||'Background style could not be applied');return false;}
            finally{pending=false;refresh();}
        }});
    gallery.status.visible=false;
    // Popup rows fill the widest command under every theme. A retained column
    // lets the gallery's explicit width constrain its own child layout.
    control.append(new Column({text:'panel:fixed:gap=0:flush'},[gallery]));
    const reset=control.append(new MenuItem({name:'background-style-reset',text:'Reset Slide Background'}));
    reset.on(Message.ACTIVATE,async()=>{
        if(!reset.enabled||pending)return false;
        pending=true;refresh();
        try{return await apply([{type:'reset-slide-background',slide:scene.slideId}]);}
        catch(error){notice(error.message||'Background could not be reset');return false;}
        finally{pending=false;refresh();}
    });
    function refresh(){
        control.enabled=Boolean(ready&&!pending&&scene?.canSetBackground&&catalog.some(c=>c.available));
        gallery.enabled=control.enabled;reset.enabled=Boolean(ready&&!pending&&scene?.canSetBackground);
        gallery.setSelection(scene?.backgroundStyle?String(scene.backgroundStyle):null);
    }
    return {control,gallery,setState({current=null,usable=false}={}){
        scene=current;ready=usable;catalog=Array.isArray(scene?.backgroundStyleCatalog)?scene.backgroundStyleCatalog:[];
        const key=JSON.stringify(catalog);
        if(key!==shown){shown=key;void gallery.search('').catch(error=>notice(error.message));}
        refresh();
    }};
}
