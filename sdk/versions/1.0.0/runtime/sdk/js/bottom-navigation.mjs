import {Control, State} from './control.mjs';
import {Button, Column, Container, Row, Tooltip} from './controls.mjs';
import {Message} from './messages.mjs';
import {Drawer} from './drawer.mjs';

/** Small, stable top-level navigation. Screens remain mounted so moving
 * between destinations preserves editable state and selection. */
export class BottomNavigation extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['items']);
    static properties = Object.freeze({
        value:{default:'',validate:String,reflect:false},
        showLabels:{default:true,validate:value => {
            if (typeof value !== 'boolean') throw TypeError('showLabels requires a boolean'); return value;
        },reflect:false},
    });
    static authoring = Object.freeze({
        constructor:'new BottomNavigation({items:[{id:"reminder",label:"Reminder",icon:Icons.Calendar,content:reminderView},...],value:"reminder",showLabels:true,...properties}). Supply 2 to 5 unique destinations, ideally 3 to 5. icon is a native semantic icon. content is an unparented Control for every item, or omitted for all items when the app handles screen rendering.',
        methods:'Read navigation.value, call navigation.setValue(id), or listen to Message.SELECT with event.detail.value. Programmatic selection and user activation emit SELECT only when the destination changes. setProperty("value",id) is supported.',
        behavior:'With content, the composite is the full flexible body: one screen fills the remaining area and navigation stays below it. Screens retain controls and drafts; focus moves out of hidden content. All labels stay visible for 2 or 3 destinations; showLabels:false hides inactive labels only for 4 or 5. Every action has an accessible name and a native Tooltip. ArrowLeft/ArrowRight/Home/End switch destinations from the bar; Tab/Enter use native controls. Use it for destinations, not submit/delete commands.',
    });
    constructor({items=[],value=items[0]?.id || '',showLabels=true,...properties}={}) {
        if (!Array.isArray(items) || items.length < 2 || items.length > 5 || new Set(items.map(item => item?.id)).size !== items.length) throw TypeError('BottomNavigation requires 2 to 5 unique destinations');
        for (const item of items) if (!item || typeof item.id !== 'string' || !item.id || typeof item.label !== 'string' || !item.label.trim() || item.icon == null) throw TypeError('Each destination needs an id, label and native icon');
        const screens = items.some(item => item.content !== undefined);
        if (screens && (items.some(item => !(item.content instanceof Control) || item.content.parent) || new Set(items.map(item => item.content)).size !== items.length)) throw TypeError('Supply unique unparented content for every destination');
        if (!items.some(item => item.id === value)) throw RangeError('Unknown navigation destination');
        super({text:'panel:fixed:flush:gap=0',state:State.EXPANSIVE|State.FLAT,...properties,value,showLabels});
        this.items = items.map(item => ({...item}));
        this.views = screens ? this.items.map(item => new Container({text:'panel:flush',state:State.EXPANSIVE|State.FLAT},[item.content])) : [];
        if (screens) this.append(new Column({text:'panel:fixed:flush',state:State.EXPANSIVE|State.FLAT},this.views));
        this.actions = this.items.map(item => {
            const button = new Button({name:`${this.name || 'bottom-navigation'}-${item.id}`,icon:item.icon,text:item.label,accessibleName:item.label,state:State.EXPANSIVE},[new Tooltip({text:item.label})]);
            button.on(Message.ACTIVATE, () => { if (this.values.enabled) this.setValue(item.id); });
            button.on(Message.KEY, event => {
                if (!this.values.enabled || event.detail?.pressed === false) return;
                const key = event.detail?.key || ({113:'ArrowLeft',114:'ArrowRight',110:'Home',115:'End'})[event.detail?.keyCode];
                const index = this.items.findIndex(candidate => candidate.id === item.id);
                const next = key === 'Home' ? 0 : key === 'End' ? this.items.length-1 : key === 'ArrowLeft' ? (index+this.items.length-1)%this.items.length : key === 'ArrowRight' ? (index+1)%this.items.length : null;
                if (next === null) return;
                this.setValue(this.items[next].id); this.focusAction(next); event.preventDefault();
            });
            return button;
        });
        this.bar = this.append(new Row({text:'panel:actions:gap=8',state:State.FLAT},this.actions));
        this.refresh();
    }
    get value() { return this.values.value; }
    set value(value) { this.setValue(value); }
    setValue(value) { return this.setProperty('value',value); }
    setProperty(name,value,options) {
        if (name === 'value' && this.items && !this.items.some(item => item.id === value)) throw RangeError('Unknown navigation destination');
        const previous = this.values?.value, result = super.setProperty(name,value,options);
        if (this.actions && ['value','showLabels'].includes(name)) {
            this.refresh();
            if (name === 'value' && previous !== value) {
                // No hidden screen retains a semantic focus request.
                this.focusAction(this.items.findIndex(item => item.id === value));
                this.emit(Message.SELECT,{value});
            }
        }
        return result;
    }
    refresh() {
        this.items.forEach((item,index) => {
            const selected = item.id === this.value, action = this.actions[index];
            action.text = this.values.showLabels || this.items.length <= 3 || selected ? item.label : '';
            action.setProperty('state',State.ENABLED|State.EXPANSIVE|(selected ? State.SELECTED|State.ACCENT : State.FLAT));
            const view = this.views[index];
            if (view?.values.visible && !selected) {
                const closeDrawers = control => {
                    if (control instanceof Drawer) control.setOpen(false,{restoreFocus:false}).catch(() => {});
                    for (const child of control.children) closeDrawers(child);
                };
                closeDrawers(view);
            }
            view?.setProperty('visible',selected);
        });
    }
    focusAction(index) {
        const root = this.application?.controlWindows.get(this.id)?.root || this;
        const clear = control => { control.setProperty('focused',false); for (const child of control.children) clear(child); };
        clear(root); this.actions[index].setProperty('focused',true);
    }
}
