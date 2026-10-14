import {Button,Tooltip} from './controls.mjs';
import {State} from './control.mjs';
/** Compact native action with a real hover tooltip and an explicit name.
 * Keep text on primary or unfamiliar actions; use icon-only for familiar ones. */
export class IconButton extends Button {
    static constructorOptions = Object.freeze(["title", "icon", "text"]);
    constructor({title,icon,text='',...properties}={}){
        if(typeof title!=='string'||!title.trim()||title.length>128)throw new TypeError('IconButton requires a clear action title');
        if(icon===undefined||icon===null)throw new TypeError('IconButton requires a native icon; discover one with sdk.icons');
        const state=properties.state??(State.ENABLED|(text?0:State.FLAT));
        super({...properties,text,icon,accessibleName:properties.accessibleName||title,
            tooltip:properties.tooltip||title,state:text?state|State.START_ALIGNED:state});
        this.append(new Tooltip({text:properties.tooltip||title}));
    }
}
