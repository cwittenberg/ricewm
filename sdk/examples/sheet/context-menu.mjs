import { ContextMenu, Menu, MenuItem, Message } from '@ricewm/app';
import { sheetIcons } from './ribbon.mjs';

/** Native menu actions carry commands, never worksheet contents. */
export function createSheetContextMenu({x,y,can,filtered,onAction,onClose}) {
    let closed=false;
    const actions=[],branches=[];
    const invoke=name=>{if(closed||!can(name))return;onAction(name);};
    const item=(name,text)=>{
        const control=new MenuItem({name:`context-${name}`,text,icon:sheetIcons[name]??null,enabled:can(name),...(['merge-cells','unmerge-cells'].includes(name)?{visible:can(name)}:{}),...(name==='toggle-filter'?{checked:filtered}:{})});
        control.on(Message.ACTIVATE,()=>invoke(name));actions.push([control,name]);return control;
    };
    const submenu=(name,text,entries)=>{const control=new MenuItem({name:`context-${name}-submenu`,text,icon:sheetIcons[name]??null,enabled:entries.some(([action])=>can(action))},[new Menu({name:`context-${name}-items`},entries.map(([name,text])=>item(name,text)))]);branches.push([control,entries]);return control;};
    const dialog=new ContextMenu({name:'sheet-context-menu'},[
            item('cut','Cut'),item('copy','Copy'),item('paste','Paste'),
            submenu('paste-special','Paste Special',[['paste-values','Values'],['paste-formulas','Formulas']]),
            submenu('insert','Insert',[['insert-rows','Rows Above'],['insert-columns','Columns Before']]),
            submenu('delete','Delete',[['delete-rows','Selected Rows…'],['delete-columns','Selected Columns…']]),
            item('clear','Clear Contents'),
            submenu('filter-values','Filter',[['toggle-filter','AutoFilter'],['filter-values','Filter Values…'],['clear-column-filter','Clear Column Filter']]),
            submenu('sort','Sort',[['sort-ascending','Smallest to Largest'],['sort-descending','Largest to Smallest'],['sort','Custom Sort…']]),
            item('format-cells','Format Cells…'),item('merge-cells','Merge Cells'),item('unmerge-cells','Unmerge Cells'),
    ]);
    // Native RiceWM owns hover expansion, submenu arrows, focus, dismissal and
    // keyboard navigation. This registration does not create a second dialog.
    dialog.on(Message.DESTROY,()=>{closed=true;});
    dialog.refresh=next=>{can=next.can;filtered=next.filtered;for(const [control,name]of actions){control.setProperty("enabled",can(name));if(["merge-cells","unmerge-cells"].includes(name))control.setProperty("visible",can(name));if(name==="toggle-filter")control.setProperty("checked",filtered);}for(const [control,entries]of branches)control.setProperty("enabled",entries.some(([name])=>can(name)));};
    return dialog;
}
