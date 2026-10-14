import {Control, State} from './control.mjs';
import {Column, Container, Dialog, Label, Row, ScrollView, Tooltip} from './controls.mjs';
import {Message} from './messages.mjs';
import {IconButton} from './icon-button.mjs';
import {Icons} from './icons.mjs';

const focusable = new Set([11,12,13,14,15,16,20,21,23,24,31,32,36,40,41,42,43,44,45,54,60,61,62,63,64]);
function focus(root, target) {
    const clear = control => { control.setProperty('focused', false); for (const child of control.children) clear(child); };
    if (!target || target.destroyed) return;
    for (let control = target; control; control = control.parent) if (!control.values.visible || !control.values.enabled) return;
    clear(root); target.setProperty('focused', true);
}
function firstInput(control) {
    if (!control.values.visible || !control.values.enabled) return null;
    const preferred=control.initialFocusTarget?.();
    if(preferred instanceof Control&&control.contains(preferred)){
        let available=true;
        for(let candidate=preferred;candidate&&candidate!==control;candidate=candidate.parent)if(!candidate.values.visible||!candidate.values.enabled)available=false;
        if(available)return preferred;
    }
    if (focusable.has(control.kind)) return control;
    for (const child of control.children) { const found = firstInput(child); if (found) return found; }
    return null;
}

/** Owns a primary view and an on-demand panel. Expanding drawers add client
 * space before revealing the panel. Fixed/limited windows use a native dialog.
 * The same controls survive every transition, including their editable drafts. */
export class Drawer extends Container {
    static family = 'composite';
    static constructorOptions = Object.freeze(['anchor','extent','title','variant','actions']);
    static authoring = Object.freeze({
        constructor:'new Drawer({anchor:"bottom",title:"Schedule",actions:[done]}, [primaryView,panelView]). Both children must be unparented Controls. Starts closed. anchor accepts right|left|bottom|top; extent defaults to "auto" or accepts 96..2048 pixels; variant defaults to "expanding" or accepts "temporary".',
        methods:'await drawer.setOpen(true|false, {opener:button}); await drawer.setExtent(pixels|"auto") resizes an open panel before changing its allocation; auto is the default and measures native content/header/actions using the current theme; drawer.open and drawer.mode report settled state. Expanding mode grows the owning primary window before showing the panel and subtracts only its added space on close. Temporary mode and fixed/small-window fallback use a native Dialog with Close/Escape.',
        behavior:'Preserves panel controls and drafts, restores opener focus, and serializes rapid toggles. Omit extent to fit native content automatically. Switching mutually exclusive panel controls and calling setOpen(true) refits an open automatic drawer. Explicit extents remain available for deliberately fixed panels. The panel has a distinct theme surface and joining/ending seams, including its footer. An ordinary panel heading, a subtle unframed close icon and Escape make every anchor discoverable. A drawer has no titlebar or minimize/maximize controls of its own. Panel scrolls independently; optional actions:[button,...] pins confirmation actions outside scrolling. Window growth follows the anchor; left/top preserve the opposite frame edge where the work area permits. Message.SELECT reports settled {open,mode,anchor}. Only one expanding Drawer per window; group mutually exclusive editors in its panel. Attach as the root or flexible main body. Use a scrollable panel for content taller than the viewport.',
    });
    constructor({anchor='right', extent='auto', title='Options', variant='expanding', actions=[], ...properties}={}, children=[]) {
        if (!['left','right','top','bottom'].includes(anchor) || !['expanding','temporary'].includes(variant)) throw TypeError('Invalid drawer anchor or variant');
        if (extent !== 'auto' && (!Number.isInteger(extent) || extent < 96 || extent > 2048)) throw RangeError('Drawer extent must be 96 to 2048 pixels');
        if (children.length !== 2 || children.some(child => !(child instanceof Control) || child.parent) || children[0] === children[1]) throw TypeError('Drawer needs unique unparented primary and panel controls');
        if (!Array.isArray(actions) || actions.some(action => !(action instanceof Control) || action.parent || children.includes(action)) || new Set(actions).size !== actions.length) throw TypeError('Drawer actions must be unique unparented controls');
        super({text:'panel:fixed:flush', state:State.EXPANSIVE|State.FLAT, ...properties});
        Object.assign(this, {anchor,extent:extent==='auto'?280:extent,automatic:extent==='auto',title,variant,open:false,mode:null,opener:null,added:0,addedSize:{width:0,height:0},transition:Promise.resolve()});
        this.horizontal = ['left','right'].includes(anchor);
        this.primary = new Container({text:'panel:fixed:flush',state:State.EXPANSIVE|State.FLAT},[children[0]]);
        this.primary.on(Message.SIZE_CHANGED, event => {
            if (!this.open && event.detail?.width > 0 && event.detail?.height > 0) this.primarySize = {width:event.detail.width,height:event.detail.height};
        });
        this.panelContent = children[1];
        this.closeButton = new IconButton({name:`${this.name || 'drawer'}-close`,icon:Icons.Close,title:`Close ${title}`,state:State.ENABLED|State.FLAT|State.MUTED});
        this.closeButton.on(Message.ACTIVATE, () => { this.setOpen(false).catch(() => {}); });
        this.heading = new Label({text:title,state:State.EXPANSIVE});
        this.header = new Row({text:'panel:fixed:flush:accessory-height=40:gap=8',state:State.FLAT},[this.heading,
            new Container({text:'panel:fixed:flush:width=40:accessory-height=40',state:State.FLAT},[this.closeButton])]);
        this.footer = new Row({text:'panel:actions:flush:gap=8',state:State.FLAT,visible:actions.some(action => action.values.visible)},actions);
        for (const action of actions) action.on(Message.CHANGE,event => {
            if (event.detail?.property === 'visible') this.footer.setProperty('visible',actions.some(candidate => candidate.values.visible));
        });
        this.panel = new Column({text:'panel:fixed:padding=12:gap=8:drawer-surface',visible:false,state:State.EXPANSIVE|State.FLAT},[
            this.header, new ScrollView({text:'panel:padding=8',state:State.EXPANSIVE|State.FLAT},[this.panelContent]), this.footer,
        ]);
        this.body = new (this.horizontal ? Row : Column)({text:'panel:fixed:flush:gap=0',state:State.EXPANSIVE|State.FLAT},
            ['left','top'].includes(anchor) ? [this.panel,this.primary] : [this.primary,this.panel]);
        this.append(this.body);
        this.on(Message.KEY, event => {
            if (!event.defaultPrevented && this.open && event.detail?.pressed !== false && (event.detail?.key === 'Escape' || event.detail?.keyCode === 9)) {
                this.setOpen(false).catch(() => {}); event.preventDefault();
            }
        });
    }
    get title() { return this._title; }
    set title(value) { this.setTitle(value); }
    setTitle(value) {
        if (typeof value !== 'string' || !value.trim() || value.length > 120) throw TypeError('Drawer title needs 1..120 characters');
        const changed = this._title !== value; this._title = value;
        if (this.heading) this.heading.text = value;
        if (this.closeButton) {
            const caption = `Close ${value}`;
            this.closeButton.setProperty('accessibleName',caption); this.closeButton.setProperty('tooltip',caption);
            for (const child of this.closeButton.children) if (child instanceof Tooltip) child.text = caption;
        }
        if (this.dialog) { this.dialog.text = `panel:drawer-dialog:${this.anchor}:${value}`; this.dialog.setProperty('accessibleName',value); }
        return changed;
    }
    setProperty(name,value,options) {
        // A composite caption must never replace native layout directives.
        // Preserve the familiar .text setter as a title alias for plain text.
        if (name === 'text' && typeof value === 'string' && !value.startsWith('panel:')) return this.setTitle(value);
        return super.setProperty(name,value,options);
    }
    panelLayout(extent, expanding) {
        const dimension = this.horizontal ? `panel:fixed:width=${extent}` : `panel:height:${extent}`;
        return `${dimension}${!expanding&&this.automatic&&this.horizontal?`:accessory-height=${this.panelSize.height}`:''}:padding=12:gap=8:drawer-surface${expanding?`:drawer-edge=${this.anchor}`:''}`;
    }
    setOpen(open, {opener=null,restoreFocus=true}={}) {
        if (typeof open !== 'boolean') return Promise.reject(TypeError('Drawer open requires a boolean'));
        const task = this.transition.then(() => this.changeOpen(open, opener, restoreFocus));
        this.transition = task.catch(() => {});
        return task;
    }
    async measurePanel(window) {
        const [content,header,footer] = await Promise.all([
            window.measureControl(this.panelContent),window.measureControl(this.header),
            this.footer.values.visible?window.measureControl(this.footer):Promise.resolve({width:0,height:0}),
        ]);
        const width=Math.max(96,Math.min(2048,Math.max(content.width+40,header.width+24,footer.width+24)));
        const wrapped=await window.measureControl(this.panelContent,{width:Math.max(96,width-40)});
        const height=Math.max(96,Math.min(2048,wrapped.height+header.height+footer.height+40+(this.footer.values.visible?16:8)));
        this.panelSize={width,height};this.extent=this.horizontal?width:height;
        return this.panelSize;
    }
    desiredSize(width,height) {
        if(this.automatic){
            // Native primary geometry can occupy less than the closed client
            // area. Consume that existing space instead of adding a blank strip.
            return this.horizontal?[Math.max(width,Math.min(width,this.primarySize?.width||width)+this.extent),Math.max(height,this.panelSize.height)]
                :[Math.max(width,this.panelSize.width),Math.max(height,Math.min(height,this.primarySize?.height||height)+this.extent)];
        }
        return this.horizontal?[width+this.extent,height]:[width,height+this.extent];
    }
    async refitOpen(window) {
        const state=window.state,base={width:state.contentWidth-this.addedSize.width,height:state.contentHeight-this.addedSize.height};
        await this.measurePanel(window);
        if(this.mode==='temporary'){this.panel.text=this.panelLayout(this.extent,false);return;}
        const desired=this.desiredSize(base.width,base.height);
        const actual=await window.setContentSize(...desired,{anchor:this.anchor});
        if(actual.width<desired[0]||actual.height<desired[1]){
            await window.setContentSize(state.contentWidth,state.contentHeight,{anchor:this.anchor});
            const opener=this.opener;await this.changeOpen(false,null,false);await this.changeOpen(true,opener,false);return;
        }
        this.addedSize={width:actual.width-base.width,height:actual.height-base.height};
        this.added=this.horizontal?this.addedSize.width:this.addedSize.height;
        this.panel.text=this.panelLayout(this.extent,true);
    }
    setExtent(extent) {
        if (extent !== 'auto' && (!Number.isInteger(extent) || extent < 96 || extent > 2048)) return Promise.reject(RangeError('Drawer extent must be 96 to 2048 pixels'));
        const task = this.transition.then(async () => {
            if (this.destroyed) throw Error('Drawer is destroyed');
            if(extent==='auto'){this.automatic=true;if(this.open)await this.refitOpen(this.application.controlWindows.get(this.id));return this.extent;}
            this.automatic=false;
            if (this.open && this.mode === 'expanding') {
                const window = this.application.controlWindows.get(this.id), state = window.state;
                const delta = extent-this.extent;
                const desired = [state.contentWidth+(this.horizontal?delta:0),state.contentHeight+(this.horizontal?0:delta)];
                const actual = await window.setContentSize(...desired,{anchor:this.anchor});
                if (actual.width < desired[0] || actual.height < desired[1]) {
                    await window.setContentSize(state.contentWidth,state.contentHeight,{anchor:this.anchor});
                    throw Error('The work area cannot fit this drawer extent');
                }
                this.addedSize.width+=actual.width-state.contentWidth;this.addedSize.height+=actual.height-state.contentHeight;
                this.added = this.horizontal ? this.addedSize.width : this.addedSize.height;
                this.panel.text = this.panelLayout(extent,true);
            }
            if (this.open && this.mode === 'temporary') this.panel.text = this.panelLayout(extent,false);
            this.extent = extent;
            return extent;
        });
        this.transition = task.catch(() => {});
        return task;
    }
    async changeOpen(open, opener, restoreFocus) {
        const window = this.application?.controlWindows.get(this.id);
        if (!window || this.destroyed) throw Error('Attach the drawer to a live window first');
        if (open === this.open) {
            if(open){
                if(this.automatic)await this.refitOpen(window);
                if(opener)this.opener=opener;
                // Switching editor content in an open drawer must move input
                // focus into that editor, rather than leave it on its launcher.
                focus(window.root,firstInput(this.panelContent)||this.closeButton);
            }
            return {open:this.open,mode:this.mode};
        }
        if (open) {
            this.opener = opener || [...this.application.controls.values()].find(control => control.values.focused && window.root.contains(control));
            if(this.automatic)await this.measurePanel(window);
            const state = window.state, width = state?.contentWidth, height = state?.contentHeight;
            const primaryWidth = Math.min(width,this.primarySize?.width||width), primaryHeight = Math.min(height,this.primarySize?.height||height);
            let expanded = false; this.fallbackReason = null;
            if (this.variant === 'expanding' && width && height && !state.fullscreen && !state.maximized) {
                const desired = this.desiredSize(width,height);
                try {
                    const actual = await window.setContentSize(...desired,{anchor:this.anchor});
                    expanded = actual.width >= desired[0] && actual.height >= desired[1];
                    if (!expanded) { this.fallbackReason = 'The work area cannot fit the expanded panel'; await window.setContentSize(width,height,{anchor:this.anchor}); }
                    else {this.addedSize={width:actual.width-width,height:actual.height-height};this.added=this.horizontal?this.addedSize.width:this.addedSize.height;}
                } catch (error) { this.fallbackReason = error.message; }
            }
            if (this.destroyed || window.presentationState.closed) throw Error('Drawer window closed');
            if (expanded) {
                this.primary.text = this.horizontal ? `panel:fixed:flush:width=${primaryWidth}` : `panel:height:${primaryHeight}:flush`;
                this.panel.text = this.panelLayout(this.extent,true);
                this.panel.setProperty('visible',true); this.mode = 'expanding';
            } else {
                this.body.remove(this.panel); this.panel.setProperty('visible',true); this.panel.text = this.panelLayout(this.extent,false);
                this.dialog = new Dialog({text:`panel:drawer-dialog:${this.anchor}:${this.title}`,accessibleName:this.title,modal:true,state:State.EXPANSIVE},[this.panel]);
                this.append(this.dialog); this.mode = 'temporary';
            }
            this.open = true; focus(window.root,firstInput(this.panelContent) || this.closeButton);
        } else {
            this.panel.setProperty('visible',false);
            this.heading.setProperty('visible',true);
            if (this.dialog) {
                this.panel.parent.remove(this.panel); this.remove(this.dialog,{destroy:true}); this.dialog = null;
                this.body.append(this.panel);
                if (['left','top'].includes(this.anchor)) this.body.children.unshift(this.body.children.pop());
            }
            this.primary.text = 'panel:fixed:flush';
            this.open = false; this.mode = null;
            if (this.addedSize.width||this.addedSize.height) {
                const state = window.state, amount = this.addedSize; this.added = 0;this.addedSize={width:0,height:0};
                // Preserve user resizing that occurred while the drawer was open.
                try { await window.setContentSize(Math.max(96,state.contentWidth-amount.width), Math.max(96,state.contentHeight-amount.height),{anchor:this.anchor}); } catch {}
            }
            if (restoreFocus) focus(window.root,this.opener); this.opener = null;
        }
        this.emit(Message.SELECT,{open:this.open,mode:this.mode,anchor:this.anchor});
        return {open:this.open,mode:this.mode};
    }
}
