import { ControlMessage, Message, Notification, message } from "./messages.mjs";
import {iconId} from './icons.mjs';

const PROPERTY_NAME = /^[a-z][a-zA-Z0-9]*$/;
const CONTROL_NAME = /^[A-Za-z][A-Za-z0-9_.:-]{0,127}$/;
const MAX_CHILDREN = 4096;
const FUNCTION_PROTOTYPE = Function.prototype;

export const State = Object.freeze({
    ENABLED: 1 << 0,
    FOCUSED: 1 << 1,
    HOVERED: 1 << 2,
    PRESSED: 1 << 3,
    SELECTED: 1 << 4,
    CHECKED: 1 << 5,
    INDETERMINATE: 1 << 6,
    INVALID: 1 << 7,
    BUSY: 1 << 8,
    ACCENT: 1 << 9,
    DESTRUCTIVE: 1 << 10,
    EMPHASIZED: 1 << 11,
    MUTED: 1 << 12,
    END_ALIGNED: 1 << 13,
    FLAT: 1 << 14,
    DISPLAY: 1 << 15,
    START_ALIGNED: 1 << 16,
    KEY_NAVIGATION: 1 << 17,
    SELECT_ALL_ON_FOCUS: 1 << 18,
    TRAILING_ICON: 1 << 19,
    EXPANSIVE: 1 << 20,
    CENTER_ALIGNED: 1 << 21,
    LARGE_ICON: 1 << 22,
    INFORMATION: 1 << 23,
    SUCCESS: 1 << 24,
    WARNING: 1 << 25,
    AMBIENT_MOTION: 1 << 26,
    LIQUID_GLASS: 1 << 27,
    SCENIC_BACKDROP: 1 << 28,
    BRAND_TYPE: 1 << 29,
    ACCENT_GRADIENT: 1 << 30,
    PROMINENT: 0x8000_0000,
});

function boolean(value) {
    return Boolean(value);
}

export function optionalString(value, name) {
    if (value === null || value === undefined || value === "") return null;
    if (typeof value !== "string") throw new TypeError(`${name} must be a string`);
    if (new TextEncoder().encode(value).byteLength > 16 * 1024) {
        throw new RangeError(`${name} exceeds the RiceUI text limit`);
    }
    return value;
}

export function numberInRange(min, max, name) {
    return value => {
        const result = Number(value);
        if (!Number.isFinite(result) || result < min || result > max) {
            throw new RangeError(`${name} must be between ${min} and ${max}`);
        }
        return result;
    };
}

export function oneOf(values, name) {
    const accepted = new Set(values);
    return value => {
        if (!accepted.has(value)) {
            throw new TypeError(`${name} must be one of ${values.join(", ")}`);
        }
        return value;
    };
}

function state(value) {
    const number = Number(value);
    // JavaScript bitwise operators return signed i32 values. Accept both the
    // signed and unsigned representations, then canonicalize for the wire.
    if (!Number.isSafeInteger(number) || number < -0x8000_0000 || number > 0xffff_ffff) {
        throw new RangeError("state must be a 32-bit mask");
    }
    return number >>> 0;
}

function propertiesFor(type) {
    const chain = [];
    for (let current = type; current && current !== FUNCTION_PROTOTYPE;
        current = Object.getPrototypeOf(current)) {
        if (Object.hasOwn(current, "properties")) chain.unshift(current.properties);
    }
    return Object.assign({}, ...chain);
}

function normalizeDescriptor(name, descriptor) {
    if (!PROPERTY_NAME.test(name)) throw new TypeError(`invalid control property ${name}`);
    if (!descriptor || typeof descriptor !== "object") {
        throw new TypeError(`control property ${name} needs a descriptor`);
    }
    return Object.freeze({
        default: descriptor.default,
        validate: typeof descriptor.validate === "function"
            ? descriptor.validate
            : value => value,
        reflect: descriptor.reflect !== false,
        stateBit: descriptor.stateBit === undefined ? null : descriptor.stateBit >>> 0,
    });
}

export class Control {
    static controlAvailability = 'app control';
    static kind = 1;
    static family = "control";
    static properties = Object.freeze({
        name: { default: null, validate: value => optionalString(value, "name") },
        text: { default: null, validate: value => optionalString(value, "text") },
        icon: {
            default: null,
            validate: value => {
                if (value === null || value === undefined) return null;
                return iconId(value);
            },
        },
        iconSelector:{default:null,validate:value=>{if(value==null)return null;if(typeof value!=='string'||value.length>320||!/^[a-zA-Z0-9_.:+/@*-]+$/.test(value))throw TypeError('Invalid iconSelector');return value;}},
        accessibleName: {
            default: null,
            validate: value => optionalString(value, "accessibleName"),
            reflect: false,
        },
        tooltip: {
            default: null,
            validate: value => optionalString(value, "tooltip"),
            reflect: false,
        },
        state: { default: State.ENABLED, validate: state },
        enabled: { default: true, validate: boolean, stateBit: State.ENABLED },
        // Visibility is represented by insertion/removal in the retained tree.
        visible: { default: true, validate: boolean },
        focused: { default: false, validate: boolean, stateBit: State.FOCUSED },
        selected: { default: false, validate: boolean, stateBit: State.SELECTED },
        checked: { default: false, validate: boolean, stateBit: State.CHECKED },
        busy: { default: false, validate: boolean, stateBit: State.BUSY },
        invalid: { default: false, validate: boolean, stateBit: State.INVALID },
        accent: { default: false, validate: boolean, stateBit: State.ACCENT },
        destructive: { default: false, validate: boolean, stateBit: State.DESTRUCTIVE },
    });

    constructor(properties = {}, children = []) {
        if (!properties || typeof properties !== "object" || Array.isArray(properties)) {
            throw new TypeError("control properties must be an object");
        }
        this.parent = null;
        this.application = null;
        this.id = 0n;
        this.destroyed = false;
        this.children = [];
        this.listeners = new Map();
        this.propertyDescriptors = Object.freeze(Object.fromEntries(
            Object.entries(propertiesFor(this.constructor))
                .map(([name, descriptor]) => [name, normalizeDescriptor(name, descriptor)]),
        ));
        this.stateOverrides = new Set();
        this.values = Object.create(null);
        for (const [name, descriptor] of Object.entries(this.propertyDescriptors)) {
            this.values[name] = descriptor.validate(descriptor.default);
        }
        for (const [name, value] of Object.entries(properties)) {
            this.setProperty(name, value, { notify: false, render: false });
        }
        if (this.values.name !== null && !CONTROL_NAME.test(this.values.name)) {
            throw new TypeError("control name is invalid");
        }
        for (const child of children) this.append(child, { notify: false });
    }

    get type() {
        return this.constructor.name;
    }

    get family() {
        return this.constructor.family;
    }

    get kind() {
        return this.constructor.kind;
    }

    get name() {
        return this.values.name;
    }

    get text() {
        return this.values.text;
    }

    set text(value) {
        this.setProperty("text", value);
    }

    get visible() {
        return this.values.visible;
    }

    set visible(value) {
        this.setProperty("visible", value);
    }

    get enabled() {
        return this.values.enabled;
    }

    set enabled(value) {
        this.setProperty("enabled", value);
    }

    get state() {
        let result = this.values.state >>> 0;
        for (const [name, descriptor] of Object.entries(this.propertyDescriptors)) {
            if (descriptor.stateBit === null) continue;
            if (this.values[name]) result |= descriptor.stateBit;
            else if (this.stateOverrides.has(name)) result &= ~descriptor.stateBit;
        }
        return result >>> 0;
    }

    getProperty(name) {
        if (!Object.hasOwn(this.propertyDescriptors, name)) {
            throw new TypeError(`${this.type} has no ${name} property`);
        }
        return this.values[name];
    }

    setProperty(name, value, { notify = true, render = true } = {}) {
        const descriptor = this.propertyDescriptors[name];
        if (!descriptor) throw new TypeError(`${this.type} has no ${name} property`);
        if (descriptor.stateBit !== null) this.stateOverrides.add(name);
        const normalized = descriptor.validate(value);
        const previous = this.values[name];
        if (Object.is(previous, normalized)) return false;
        this.values[name] = normalized;
        if (notify) {
            this.notifyParent(Notification.CHANGED, { property: name, previous, value: normalized });
            this.emit(Message.CHANGE, { property: name, previous, value: normalized });
        }
        if (render && descriptor.reflect) this.application?.invalidate(this);
        return true;
    }

    append(child, { notify = true } = {}) {
        if (!(child instanceof Control)) throw new TypeError("child must be a Control");
        if (child === this || child.contains(this)) throw new TypeError("control cycle rejected");
        if (child.parent) throw new TypeError("control already has a parent");
        if (this.children.length >= MAX_CHILDREN) throw new RangeError("control child limit exceeded");
        child.parent = this;
        this.children.push(child);
        if (this.application) this.application.attach(child, this);
        if (notify) {
            this.sendMessage(message(Message.CHILD_ADDED, this, { child }));
            this.notifyParent(Notification.CHILD_ADDED, { child });
        }
        this.application?.invalidate(this);
        return child;
    }

    remove(child, { destroy = false } = {}) {
        const index = this.children.indexOf(child);
        if (index < 0) return false;
        this.children.splice(index, 1);
        if (destroy) child.destroy();
        else this.application?.detach(child);
        child.parent = null;
        this.sendMessage(message(Message.CHILD_REMOVED, this, { child }));
        this.notifyParent(Notification.CHILD_REMOVED, { child });
        this.application?.invalidate(this);
        return true;
    }

    contains(candidate) {
        for (let current = candidate; current; current = current.parent) {
            if (current === this) return true;
        }
        return false;
    }

    find(name) {
        if (this.name === name) return this;
        for (const child of this.children) {
            const found = child.find(name);
            if (found) return found;
        }
        return null;
    }

    on(type, listener) {
        if (typeof type !== "string" || !type || typeof listener !== "function") {
            throw new TypeError("control listener is invalid");
        }
        const listeners = this.listeners.get(type) || new Set();
        listeners.add(listener);
        this.listeners.set(type, listeners);
        if (type === Message.SIZE_CHANGED) this.application?.invalidate(this);
        return () => {
            listeners.delete(listener);
            if (listeners.size === 0) this.listeners.delete(type);
            if (type === Message.SIZE_CHANGED) this.application?.invalidate(this);
        };
    }

    emit(type, detail = null, options = {}) {
        return this.sendMessage(message(type, this, detail, options));
    }

    sendMessage(input, detail = null, options = {}) {
        const current = input instanceof ControlMessage
            ? input
            : message(input, this, detail, options);
        if (!current.target) current.target = this;
        current.currentTarget = this;
        this.application?.debug?.recordControlMessage(this, current);
        let result = this.windowProc(current);
        if(this.application?.hostEventWindow!==undefined)this.application.observeHostWork?.(result,this.application.hostEventWindow);
        for (const listener of [...(this.listeners.get(current.type) || [])]) {
            const candidate = listener(current);
            if(this.application?.hostEventWindow!==undefined)this.application.observeHostWork?.(candidate,this.application.hostEventWindow);
            if (candidate !== undefined) result = candidate;
            if (current.propagationStopped) break;
        }
        if (current.bubbles && !current.propagationStopped && this.parent) {
            const parentResult = this.parent.sendMessage(current);
            if (parentResult !== undefined) result = parentResult;
        }
        return result;
    }

    postMessage(input, detail = null, options = {}) {
        const current = input instanceof ControlMessage
            ? input
            : message(input, this, detail, options);
        if (this.application) return this.application.postControlMessage(this, current);
        return Promise.resolve().then(() => this.sendMessage(current));
    }

    notifyParent(code, detail = null) {
        if (!this.parent) return undefined;
        return this.parent.sendMessage(new ControlMessage(Message.NOTIFY, {
            target: this.parent,
            source: this,
            detail: { code, value: detail },
            bubbles: false,
            cancelable: true,
        }));
    }

    windowProc(current) {
        return this.defaultProc(current);
    }

    defaultProc(current) {
        switch (current.type) {
            case Message.GET_PROPERTY:
                return this.getProperty(current.detail?.name);
            case Message.SET_PROPERTY:
                return this.setProperty(current.detail?.name, current.detail?.value);
            case Message.GET_TEXT:
                return this.text;
            case Message.SET_TEXT:
                return this.setProperty("text", current.detail?.value);
            case Message.ENABLE:
                return this.setProperty("enabled", current.detail?.enabled);
            case Message.SHOW:
                return this.setProperty("visible", current.detail?.visible);
            case Message.FOCUS:
                this.setProperty("focused", true);
                this.notifyParent(Notification.FOCUSED, current.detail);
                return true;
            case Message.BLUR:
                this.setProperty("focused", false);
                this.notifyParent(Notification.BLURRED, current.detail);
                return true;
            case Message.ACTIVATE:
                if (!this.getProperty("enabled") || !this.getProperty("visible")) return false;
                this.notifyParent(Notification.CLICKED, current.detail);
                return true;
            case Message.VALUE_CHANGED:
                if (Object.hasOwn(this.propertyDescriptors, "value")) {
                    this.setProperty("value", current.detail?.value);
                }
                this.notifyParent(Notification.CHANGED, current.detail);
                return true;
            default:
                return undefined;
        }
    }

    dispatchHostEvent(event) {
        const mapping = {
            activate: Message.ACTIVATE,
            "value-changed": Message.VALUE_CHANGED,
            pointer: Message.POINTER,
            key: Message.KEY,
            focus: event?.focused === false ? Message.BLUR : Message.FOCUS,
            "text-selection": Message.TEXT_SELECTION,
            "size-changed": Message.SIZE_CHANGED,
        };
        const type = mapping[event?.type];
        if (!type) return false;
        return this.sendMessage(message(type, this, event, {
            cancelable: true,
            bubbles: type === Message.KEY,
        }));
    }

    toWidget() {
        return Object.freeze({
            id: this.id,
            kind: this.kind,
            state: this.state,
            text: this.text,
            icon: this.values.icon,
            parent: this.parent?.id || null,
            visible: this.values.visible,
        });
    }

    destroy() {
        if (this.destroyed) return false;
        this.sendMessage(Message.DESTROY);
        for (const child of [...this.children]) child.destroy();
        this.children.length = 0;
        this.listeners.clear();
        if (this.parent) {
            const index = this.parent.children.indexOf(this);
            if (index >= 0) this.parent.children.splice(index, 1);
        }
        this.application?.detach(this);
        this.application = null;
        this.parent = null;
        this.destroyed = true;
        return true;
    }
}

export class ContainerControl extends Control {
    static family = "container";
}

export class ContentControl extends Control {
    static family = "content";
}

export class CommandControl extends ContentControl {
    static family = "command";

    windowProc(current) {
        if (current.type === Message.KEY && ["Enter", " "].includes(current.detail?.key)) {
            return this.sendMessage(Message.ACTIVATE, { source: "keyboard" });
        }
        return super.windowProc(current);
    }
}

export class InputControl extends ContentControl {
    get value() { return this.getProperty('value'); }
    set value(value) { this.setProperty('value',value); }
    static family = "input";
    static properties = Object.freeze({
        value: { default: "", validate: value => String(value ?? "") },
        readOnly: { default: false, validate: boolean, reflect: false },
        required: { default: false, validate: boolean, reflect: false },
        placeholder: {
            default: null,
            validate: value => optionalString(value, "placeholder"),
            reflect: false,
        },
    });

    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED && this.values.readOnly) return false;
        return super.windowProc(current);
    }

    toWidget() {
        const widget = super.toWidget();
        return Object.freeze({
            ...widget,
            // Native editable text is the document, not a paint-only hint.
            // This ABI has no separate placeholder channel; serializing the
            // hint here makes it part of the user's input on the first key.
            text: this.values.value || this.text || '',
        });
    }
}

export class SelectionControl extends InputControl {
    static family = "selection";
    static properties = Object.freeze({
        multiple: { default: false, validate: boolean, reflect: false },
    });
}

export class CollectionControl extends ContainerControl {
    static family = "collection";
    static properties = Object.freeze({
        selection: {
            default: null,
            validate: value => value === null ? null : String(value),
            reflect: false,
        },
    });
}

export class OverlayControl extends ContainerControl {
    static family = "overlay";
}

export class SurfaceControl extends ContainerControl {
    static family = "surface";
}

/** Discover the loaded SDK, including extension exports, without invoking a
 * constructor. Composite classes own constructorOptions and authoring metadata;
 * ordinary properties come from the same descriptors used by setProperty. */
export function describeControlLibrary(namespace) {
    const schemas = {}, constructors = {}, helpers = {};
    for (const [name, type] of Object.entries(namespace).sort(([a], [b]) => a.localeCompare(b))) {
        if (typeof type !== 'function') continue;
        if (type !== Control && !(type.prototype instanceof Control)) {
            if (type.controlFactory) helpers[name] = type.controlFactory;
            continue;
        }
        const fields = new Set(), options = new Set();
        for (let current = type; current && current !== Function.prototype; current = Object.getPrototypeOf(current)) {
            for (const key of Object.keys(current.properties || {})) fields.add(key);
            for (const key of current.constructorOptions || []) options.add(key);
        }
        schemas[name] = [...fields].sort();
        constructors[name] = { kind: type.kind, family: type.family,
            constructorOptions: [...options].filter(key => !fields.has(key)).sort(), availability: type.controlAvailability,
            ...(type.name !== name ? { aliasOf: type.name } : {}),
            ...(type.authoring ? { guidance: type.authoring } : {}) };
    }
    const factories = Object.fromEntries(Object.entries(namespace.CONTROL_CLASSES || {}).map(([name, type]) =>
        [name, { constructor: type.name, ui: name[0].toLowerCase() + name.slice(1), availability: type.controlAvailability }]));
    return { schemas, constructors, factories, helpers,
        contract: 'Import constructors from @ricewm/app. schemas lists inherited setProperty fields; constructorOptions lists additional constructor-only options, including those inherited by aliases. Primitive controls accept (properties, children). ui factory names and createControl names are separate from exported constructor names. Consult guidance and the SDK reference for values, events and methods. Base classes support custom composites. Host-owned controls require trusted integration and do not grant generated apps additional authority.' };
}
