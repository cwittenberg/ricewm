import { ContainerControl, OverlayControl, State } from "./control.mjs";
import {
    Button, CheckBox, Column, Dialog, GroupBox, Label, List, Menu, MenuBar, MenuItem,
    NotificationBanner, Popover, Row, Spinner, TextField, ToolBar,
} from "./controls.mjs";
import { Message } from "./messages.mjs";
import { EditableChoice } from './value-editors.mjs';

/** Base for standards controls constructed from multiple RiceUI primitives. */
export class CompositeControl extends ContainerControl {
    static family = "composite";
}

export class CommandLink extends Button {
    constructor(properties = {}) {
        super({ ...properties, state: (properties.state || 0) | State.EXPANSIVE });
    }
}

export class SplitButton extends CompositeControl {
    static kind = Row.kind;
    static constructorOptions = Object.freeze(["text", "menu", "presentation", "onActivate"]);
    static authoring = Object.freeze({
        constructor: 'new SplitButton({text:"Save",icon:12,menu:[MenuItem,...] | unparented Menu,presentation:"button"|"ribbon",onActivate:()=>save()})',
        behavior: 'The main native Button invokes the default action once. A separate native Menu opens alternatives with the pointer. Choosing an alternative does not invoke the default action. Ribbon presentation stacks a large icon above the menu caption. The caller owns enabled/busy state and asynchronous command/history semantics. Keyboard disclosure remains an open native-renderer acceptance requirement; do not claim that pointer checks establish keyboard support.',
        methods: 'primary and menu expose the retained native controls; disclosure aliases menu. Message.ACTIVATE on SplitButton reports the primary action; Message.SELECT reports an activated alternative. Parent text/icon/accessibleName/tooltip changes update the real main button and menu caption.',
    });
    constructor({ text = "Action", menu = [], presentation = 'button', onActivate = null, ...properties } = {}) {
        if (!['button', 'ribbon'].includes(presentation)) throw new RangeError('Unknown split button presentation');
        if (onActivate !== null && typeof onActivate !== 'function') throw new TypeError('Split button onActivate must be a function');
        if (!(menu instanceof Menu) && (!Array.isArray(menu) || menu.length > 256 || menu.some(item => !(item instanceof MenuItem) && typeof item !== 'string'))) throw new TypeError('Split button alternatives require MenuItems, labels or a native Menu');
        if (menu instanceof Menu ? menu.parent || menu.destroyed : menu.some(item => item instanceof MenuItem && (item.parent || item.destroyed)) || new Set(menu.filter(item => item instanceof MenuItem)).size !== menu.filter(item => item instanceof MenuItem).length) throw new TypeError('Split button alternatives must be unique unparented controls');
        super({ ...properties, text });
        this.presentation = presentation;
        this.primary = new Button({ name: 'primary', state: presentation === 'ribbon' ? State.FLAT | State.DISPLAY : State.START_ALIGNED });
        this.menu = menu instanceof Menu ? menu : new Menu({ name: 'menu' }, menu.map(item => item instanceof MenuItem ? item : new MenuItem({ text: item })));
        this.disclosure = this.menu;
        // START_ALIGNED|TRAILING_ICON is the native choice-field width mode.
        // Alternatives size to their content, independently of this caption.
        this.menu.setProperty('state', presentation === 'ribbon' ? State.FLAT | State.TRAILING_ICON : State.FLAT);
        this.menu.setProperty('icon', 74);
        const content = presentation === 'ribbon'
            ? new Column({ text: 'panel:flush:fixed:gap=0' }, [new ToolBar({ text: 'panel:flush:fixed:accessory-height=42' }, [this.primary]), new Row({ text: 'panel:flush:fixed:gap=0:accessory-height=24' }, [this.menu])])
            : new Row({ text: 'panel:flush:fixed:gap=0' }, [this.primary, this.menu]);
        this.append(content);
        const sync = () => {
            const label = this.getProperty('text') || 'Action', name = this.getProperty('accessibleName') || label;
            this.primary.text = presentation === 'ribbon' ? '' : label;
            for (const property of ['icon', 'tooltip']) this.primary.setProperty(property, this.getProperty(property));
            this.primary.setProperty('accessibleName', name);
            this.menu.text = presentation === 'ribbon' ? label : '';
            this.menu.setProperty('accessibleName', name + ' alternatives');
            this.menu.setProperty('tooltip', name + ' alternatives');
        };
        const usable = () => Boolean(this.state & State.ENABLED) && !(this.state & State.BUSY);
        const syncEnabled = () => {
            const enabled = usable();
            this.primary.enabled = enabled;
            this.menu.enabled = enabled;
        };
        sync();
        syncEnabled();
        this.on(Message.CHANGE, event => {
            if (event.target !== this) return;
            if (['text', 'icon', 'accessibleName', 'tooltip'].includes(event.detail?.property)) sync();
            if (['enabled', 'busy', 'state'].includes(event.detail?.property)) syncEnabled();
        });
        this.primary.on(Message.ACTIVATE, event => {
            event.stopPropagation();
            if (usable() && this.primary.enabled) return this.emit(Message.ACTIVATE, { ...event.detail, part: 'primary' });
        });
        this.menu.on(Message.ACTIVATE, event => {
            event.stopPropagation();
            if (event.target !== this.menu && this.enabled) this.emit(Message.SELECT, { item: event.target });
        });
        if (onActivate) this.on(Message.ACTIVATE, () => { if (usable()) return onActivate(); });
    }
    // The semantic icon belongs to the retained primary Button. Painting it on
    // the composite row too creates a second, clipped glyph over that button.
    toWidget() { return Object.freeze({ ...super.toWidget(), icon: null, text: 'panel:split-button:flush:fixed:gap=0' }); }
}

/** Conventional names share the native EditableChoice implementation. */
export class ComboBox extends EditableChoice {
    static constructorOptions = Object.freeze(['items', 'value', 'onCommit', 'validate', 'parse']);
    static authoring = Object.freeze({ constructor: 'new ComboBox({items:["Inter","Manrope"],value:"Inter",onCommit:async(value)=>{...}})', methods: 'Typed values and native alternatives share validation and one committed VALUE_CHANGED event. Optional synchronous parse(text) returns a normalized string before validation. setValue(value,{preserveDraft:true}), commit(), cancel(); input and menu expose the actual controls. Caller owns persistence and undo.' });
    constructor({ items = [], value = '', ...options } = {}) {
        super({ ...options, choices: items, value: value ?? '' });
    }
}

export class DropDownList extends EditableChoice {
    static constructorOptions = Object.freeze(['items', 'value', 'onCommit', 'validate', 'emptyLabel']);
    static authoring = Object.freeze({ constructor: 'new DropDownList({items:["Left","Center","Right"],value:"Left",emptyLabel:"Choose alignment",onCommit:async(value)=>{...}})', methods: 'Selection-only native choice field. Choose from items; the field cannot accept typed replacements. setValue(value), commit(), cancel(); input and menu expose the retained controls. Empty value represents no selection; emptyLabel paints its caption without adding a selectable item or changing the value. VALUE_CHANGED reports only accepted commits. Read-only, disabled and busy states block changes.' });
    constructor({ items = [], value = '', validate = null, emptyLabel = '', ...options } = {}) {
        if (!Array.isArray(items) || items.some(item => typeof item !== 'string')) throw TypeError('Dropdown items must be labels');
        if (validate !== null && typeof validate !== 'function') throw TypeError('Invalid dropdown validator');
        if (typeof emptyLabel !== 'string' || emptyLabel.length > 256 || /[\u0000-\u001f]/u.test(emptyLabel)) throw TypeError('Invalid dropdown empty caption');
        const allowed = Object.freeze([...items]);
        if (value != null && value !== '' && !allowed.includes(value)) throw RangeError('Dropdown value is not in items');
        super({ ...options, choices: allowed, value: value ?? '', editable: false,
            validate: candidate => allowed.includes(candidate) ? validate?.(candidate) : 'Choose one of the available items' });
        this.items = allowed;
        this.input.text = emptyLabel;
    }
    setProperty(name, value, options) {
        if (name === 'editable' && value) throw TypeError('DropDownList is selection-only');
        if (name === 'value' && this.items && value !== '' && !this.items.includes(value)) throw RangeError('Dropdown value is not in items');
        return super.setProperty(name, value, options);
    }
    setValue(value, options) {
        if (value !== '' && !this.items.includes(value)) throw RangeError('Dropdown value is not in items');
        return super.setValue(value, options);
    }
}

export class Link extends Button {
    constructor(properties = {}) { super({ ...properties, state: (properties.state || 0) | State.FLAT }); }
}
export class SearchBox extends TextField {
    constructor(properties = {}) { super({ placeholder: "Search", ...properties, state: (properties.state || 0) | State.TRAILING_ICON }); }
}
export class SpinControl extends CompositeControl {
    static constructorOptions = Object.freeze(["value", "step"]);
    constructor({ value = 0, step = 1, ...properties } = {}) {
        const field = new TextField({ name: "value", value: String(value) });
        const down = new Button({ name: "down", text: "−" });
        const up = new Button({ name: "up", text: "+" });
        super(properties, [field, down, up]);
        const change = delta => field.setProperty("value", String((Number(field.getProperty("value")) || 0) + delta));
        down.on(Message.ACTIVATE, () => change(-Number(step)));
        up.on(Message.ACTIVATE, () => change(Number(step)));
    }
}
export class ListBox extends List {}
export class Submenu extends Menu {}
/** A command on click, with native menu alternatives after a held press.
 * Down Arrow opens the alternatives without a pointer. Menu ownership and
 * cancellation are checked by the native host, including late timer delivery.
 */
export class HoldMenuButton extends Menu {
    static properties = Object.freeze({
        holdDelay: { default: 450, reflect: false, validate(value) {
            if (!Number.isInteger(value) || value < 250 || value > 1500) throw new RangeError("holdDelay must be 250 through 1500 milliseconds");
            return value;
        } },
    });
    constructor(properties = {}, children = []) {
        super({ ...properties, state: (properties.state ?? State.ENABLED) | State.KEY_NAVIGATION }, children);
        this.holdTimer = null;
    }
    cancelHold() {
        if (this.holdTimer !== null) clearTimeout(this.holdTimer);
        this.holdTimer = null;
    }
    windowProc(current) {
        if (current.type === Message.POINTER) {
            const { kind, button } = current.detail || {};
            if (kind === "down" && button === 1 && this.values.enabled) {
                this.cancelHold();
                this.setProperty("selected", false);
                this.holdTimer = setTimeout(() => {
                    this.holdTimer = null;
                    if (!this.destroyed && this.values.enabled) this.setProperty("selected", true);
                }, this.values.holdDelay);
            } else if (["up", "leave"].includes(kind)) {
                this.cancelHold(); this.setProperty("selected", false);
            }
        } else if ([Message.ACTIVATE, Message.BLUR, Message.DESTROY].includes(current.type)
            || (current.type === Message.NOTIFY && current.detail?.code === "clicked")
            || (current.type === Message.KEY && (current.detail?.key === "Escape" || current.detail?.keyCode === 9))) {
            this.cancelHold(); this.setProperty("selected", false);
        }
        if (current.type === Message.KEY && current.detail?.pressed !== false
            && (["Enter", " "].includes(current.detail?.key) || [36, 65, 104].includes(current.detail?.keyCode))) {
            this.cancelHold();
            if (this.values.enabled) return this.sendMessage(Message.ACTIVATE, { source: "keyboard" });
        }
        return super.windowProc(current);
    }
}
/** A native menu bound to a container, positioned by trusted pointer/focus geometry. */
export class ContextMenu extends Menu {
    attachTo(owner) {
        if (!(owner instanceof ContainerControl) || owner.destroyed) throw new TypeError("Context menu owner must be a live container");
        if (this.destroyed) throw new Error("Context menu is destroyed");
        if (this.contextOwner === owner) return this;
        if (this.parent && this.parent !== this.contextBar) throw new Error("Context menu already belongs to another control");
        if (owner.children.some(child => child !== this.contextBar && child instanceof MenuBar && child.text === "context")) {
            throw new Error("Container already has a context menu");
        }
        this.detach();
        const bar = new MenuBar({ text: "context" }, [this]);
        owner.append(bar);
        this.contextOwner = owner; this.contextBar = bar;
        return this;
    }
    detach() {
        const bar = this.contextBar;
        this.contextOwner = null; this.contextBar = null;
        if (bar && !bar.destroyed) {
            if (this.parent === bar) bar.remove(this);
            bar.parent?.remove(bar, { destroy: true });
            if (!bar.destroyed) bar.destroy();
        }
        return this;
    }
}
export class StatusBar extends Row {}
export class Infotip extends Popover {}
export class Balloon extends NotificationBanner {}

export class InPlaceMessage extends NotificationBanner {}
export class ErrorMessage extends NotificationBanner {
    constructor(properties = {}, children = []) { super({ ...properties, state: (properties.state || 0) | State.INVALID | State.DESTRUCTIVE }, children); }
}
export class WarningMessage extends NotificationBanner {
    constructor(properties = {}, children = []) { super({ ...properties, state: (properties.state || 0) | State.WARNING }, children); }
}
export class ProgressFeedback extends GroupBox {
    constructor(properties = {}, children = []) { super(properties, children.length ? children : [new Spinner(), new Label({ text: properties.text || "Working…" })]); }
}
export class EmptyState extends GroupBox {}

export class ConfirmationDialog extends Dialog {
    static constructorOptions = Object.freeze(["text", "confirmText", "cancelText"]);
    constructor({ text = "Are you sure?", confirmText = "Confirm", cancelText = "Cancel", ...properties } = {}) {
        super(properties, [new Label({ text }), new Row({}, [
            new Button({ name: "cancel", text: cancelText }),
            new Button({ name: "confirm", text: confirmText, accent: true }),
        ])]);
    }
}
export class WarningDialog extends ConfirmationDialog {
    constructor(properties = {}) { super({ ...properties, state: (properties.state || 0) | State.WARNING }); }
}
export class ErrorDialog extends ConfirmationDialog {
    constructor(properties = {}) { super({ ...properties, state: (properties.state || 0) | State.INVALID | State.DESTRUCTIVE }); }
}
export class PropertyWindow extends Dialog {}
export class SettingsPage extends Column {}
export class OpenDialog extends Dialog {
    async show(services, options = {}) { return services.files.openDialog(options); }
}
export class SaveDialog extends Dialog {
    async show(services, options = {}) { return services.files.saveDialog(options); }
}

export const COMPOSITE_CONTROLS = Object.freeze({
    CommandLink, SplitButton, DropDownList, ComboBox, Link, SearchBox, SpinControl,
    ListBox, Submenu, HoldMenuButton, ContextMenu, StatusBar, Infotip, Balloon, InPlaceMessage,
    ErrorMessage, WarningMessage, ProgressFeedback, EmptyState, ConfirmationDialog,
    WarningDialog, ErrorDialog, PropertyWindow, SettingsPage, OpenDialog, SaveDialog,
});
