/**
 * Stable application/control messages. Names deliberately describe semantics
 * instead of renderer events so a control can retain its procedure when the
 * host input or rendering implementation changes.
 */
export const Message = Object.freeze({
    CREATE: "rice.create",
    CREATED: "rice.created",
    DESTROY: "rice.destroy",
    CHILD_ADDED: "rice.child-added",
    CHILD_REMOVED: "rice.child-removed",
    GET_PROPERTY: "rice.get-property",
    SET_PROPERTY: "rice.set-property",
    GET_TEXT: "rice.get-text",
    SET_TEXT: "rice.set-text",
    ENABLE: "rice.enable",
    SHOW: "rice.show",
    FOCUS: "rice.focus",
    BLUR: "rice.blur",
    ACTIVATE: "rice.activate",
    VALUE_CHANGED: "rice.value-changed",
    SIZE_CHANGED: "rice.size-changed",
    POINTER: "rice.pointer",
    KEY: "rice.key",
    TEXT_SELECTION: "rice.text-selection",
    COMMAND: "rice.command",
    NOTIFY: "rice.notify",
    CHANGE: "rice.change",
    SELECT: "rice.select",
    SCROLL: "rice.scroll",
    TIMER: "rice.timer",
    APP_OPEN: "rice.app-open",
    APP_CLOSE_REQUEST: "rice.app-close-request",
    APP_CLOSE: "rice.app-close",
    BUS_EVENT: "rice.bus-event",
    BUS_CALL: "rice.bus-call",
    THEME_CHANGED: "rice.theme-changed",
});

export const Notification = Object.freeze({
    CLICKED: "clicked",
    CHANGED: "changed",
    SELECTED: "selected",
    FOCUSED: "focused",
    BLURRED: "blurred",
    POINTER: "pointer",
    KEY: "key",
    SCROLLED: "scrolled",
    CHILD_ADDED: "child-added",
    CHILD_REMOVED: "child-removed",
});

export class ControlMessage {
    constructor(type, {
        target = null,
        source = null,
        detail = null,
        bubbles = false,
        cancelable = false,
    } = {}) {
        if (typeof type !== "string" || !type) {
            throw new TypeError("control message type must be a non-empty string");
        }
        this.type = type;
        this.target = target;
        this.currentTarget = null;
        this.source = source || target;
        this.detail = detail;
        this.bubbles = Boolean(bubbles);
        this.cancelable = Boolean(cancelable);
        this.defaultPrevented = false;
        this.propagationStopped = false;
    }

    preventDefault() {
        if (this.cancelable) this.defaultPrevented = true;
    }

    stopPropagation() {
        this.propagationStopped = true;
    }
}

export function message(type, target, detail = null, options = {}) {
    return new ControlMessage(type, { ...options, target, detail });
}
