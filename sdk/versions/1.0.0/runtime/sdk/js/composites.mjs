import { ContainerControl, OverlayControl, State } from "./control.mjs";
import {
    Button, CheckBox, Column, Dialog, GroupBox, Label, List, Menu, MenuBar, MenuItem,
    NotificationBanner, Popover, Row, Spinner, TextField,
} from "./controls.mjs";
import { Message } from "./messages.mjs";

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
    static constructorOptions = Object.freeze(["text", "menu"]);
    constructor({ text = "Action", menu = [] , ...properties } = {}) {
        const primary = new Button({ name: "primary", text });
        const disclosure = new Button({ name: "disclosure", text: "More" });
        const popup = new Menu({ name: "menu", open: false }, menu.map(item =>
            item instanceof MenuItem ? item : new MenuItem({ text: String(item) })));
        super(properties, [primary, disclosure, popup]);
        disclosure.on(Message.ACTIVATE, () => popup.setProperty("open", !popup.getProperty("open")));
    }
}

export class DropDownList extends CompositeControl {
    static constructorOptions = Object.freeze(["items", "value"]);
    constructor({ items = [], value = null, ...properties } = {}) {
        const field = new TextField({ name: "value", value: value ?? "", readOnly: true });
        const menu = new Menu({ name: "menu", open: false }, items.map(item => {
            const option = new MenuItem({ text: String(item) });
            option.on(Message.ACTIVATE, () => { field.setProperty("value", option.text); menu.setProperty("open", false); });
            return option;
        }));
        const open = new Button({ name: "open", text: "Choose" });
        super(properties, [new Row({}, [field, open]), menu]);
        open.on(Message.ACTIVATE, () => menu.setProperty("open", !menu.getProperty("open")));
    }
}

export class ComboBox extends CompositeControl {
    static constructorOptions = Object.freeze(["items", "value"]);
    constructor({ items = [], value = "", ...properties } = {}) {
        super(properties, [
            new TextField({ name: "value", value }),
            new Menu({ name: "suggestions" }, items.map(item => new MenuItem({ text: String(item) }))),
        ]);
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
