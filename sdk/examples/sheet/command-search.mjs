import { Button, MenuItem, Dialog, CommandRegistry, CommandPalette, Message } from '@ricewm/app';

/** Search invokes the existing native controls. It neither maintains a second
 * feature list nor sends unchecked workbook operations. */
export function createSheetCommandSearch({root,canOpen,onOpen,onClose}) {
    let opened = null;
    const close = () => {
        if (!opened) return;
        const current=opened;opened=null;
        if(current.dialog.parent)current.dialog.parent.remove(current.dialog,{destroy:true});
        current.commands.dispose();onClose();
    };
    return Object.freeze({close,open() {
        if(opened || !canOpen())return false;
        const controls=new Map();
        function collect(control){
            if(control.name&&[Button.kind,MenuItem.kind].includes(control.kind)&&control.listeners.has(Message.ACTIVATE)
                &&!control.name.startsWith('ribbon-tab-')&&control.name!=='search-commands'){
                const label=control.getProperty('accessibleName')||control.getProperty('tooltip')||control.text;
                if(label&&!label.startsWith('panel:')&&!controls.has(control.name))controls.set(control.name,{control,label});
            }
            for(const child of control.children)collect(child);
        }
        collect(root);
        const commands=new CommandRegistry([...controls].map(([id,{control,label}])=>({id,label,
            enabled:control.enabled,keywords:id,disabledReason:control.enabled?null:'Unavailable for the current selection or workbook',
            run:()=>{close();if(!control.enabled||control.destroyed)return false;control.sendMessage(Message.ACTIVATE);return true;}})));
        const palette=new CommandPalette({name:'sheet-command-search',commands,onClose:close});
        const dialog=new Dialog({name:'sheet-command-palette',text:'Search commands',modal:true},[palette]);
        opened={commands,dialog};onOpen();root.append(dialog);palette.input.setProperty('focused',true);
        return true;
    }});
}
