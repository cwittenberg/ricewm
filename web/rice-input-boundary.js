(function installRiceInputBoundary(global) {
    "use strict";

    const VERSION = "rice.input-boundary/v1";
    const keycodes = Object.freeze({
        Escape: 9, Digit1: 10, Digit2: 11, Digit3: 12, Digit4: 13,
        Digit5: 14, Digit6: 15, Digit7: 16, Digit8: 17, Digit9: 18,
        Digit0: 19, Minus: 20, Equal: 21, Backspace: 22, Tab: 23,
        KeyQ: 24, KeyW: 25, KeyE: 26, KeyR: 27, KeyT: 28, KeyY: 29,
        KeyU: 30, KeyI: 31, KeyO: 32, KeyP: 33, BracketLeft: 34,
        BracketRight: 35, Enter: 36, ControlLeft: 37, KeyA: 38,
        KeyS: 39, KeyD: 40, KeyF: 41, KeyG: 42, KeyH: 43, KeyJ: 44,
        KeyK: 45, KeyL: 46, Semicolon: 47, Quote: 48, Backquote: 49,
        ShiftLeft: 50, Backslash: 51, KeyZ: 52, KeyX: 53, KeyC: 54,
        KeyV: 55, KeyB: 56, KeyN: 57, KeyM: 58, Comma: 59,
        Period: 60, Slash: 61, ShiftRight: 62, NumpadMultiply: 63,
        AltLeft: 64, Space: 65, CapsLock: 66, F1: 67, F2: 68, F3: 69,
        F4: 70, F5: 71, F6: 72, F7: 73, F8: 74, F9: 75, F10: 76,
        NumLock: 77, ScrollLock: 78, Numpad7: 79, Numpad8: 80,
        Numpad9: 81, NumpadSubtract: 82, Numpad4: 83, Numpad5: 84,
        Numpad6: 85, NumpadAdd: 86, Numpad1: 87, Numpad2: 88,
        Numpad3: 89, Numpad0: 90, NumpadDecimal: 91,
        IntlBackslash: 94, F11: 95, F12: 96, NumpadEnter: 104,
        ControlRight: 105, NumpadDivide: 106, PrintScreen: 107,
        AltRight: 108, Home: 110, ArrowUp: 111, PageUp: 112,
        ArrowLeft: 113, ArrowRight: 114, End: 115, ArrowDown: 116,
        PageDown: 117, Insert: 118, Delete: 119, Pause: 127,
        MetaLeft: 133, MetaRight: 134, ContextMenu: 135,
    });
    const shiftedKeys = Object.freeze({
        Digit1: "!", Digit2: "@", Digit3: "#", Digit4: "$",
        Digit5: "%", Digit6: "^", Digit7: "&", Digit8: "*",
        Digit9: "(", Digit0: ")", Minus: "_", Equal: "+",
        KeyQ: "Q", KeyW: "W", KeyE: "E", KeyR: "R", KeyT: "T",
        KeyY: "Y", KeyU: "U", KeyI: "I", KeyO: "O", KeyP: "P",
        BracketLeft: "{", BracketRight: "}", KeyA: "A", KeyS: "S",
        KeyD: "D", KeyF: "F", KeyG: "G", KeyH: "H", KeyJ: "J",
        KeyK: "K", KeyL: "L", Semicolon: ":", Quote: "\"",
        Backquote: "~", Backslash: "|", KeyZ: "Z", KeyX: "X",
        KeyC: "C", KeyV: "V", KeyB: "B", KeyN: "N", KeyM: "M",
        Comma: "<", Period: ">", Slash: "?", IntlBackslash: ">",
    });
    const modifierMasks = Object.freeze({
        ShiftLeft: 0x0001, ShiftRight: 0x0001,
        ControlLeft: 0x0004, ControlRight: 0x0004,
        AltLeft: 0x0008, AltRight: 0x0008,
        MetaLeft: 0x0040, MetaRight: 0x0040,
    });

    function boundedCommittedText(value, maximumBytes = 4096, multiline = false) {
        const limit = Math.max(0, Number(maximumBytes) || 0);
        const encoder = new TextEncoder();
        let bytes = 0;
        let output = "";
        for (const character of String(value || "")) {
            const scalar = character.codePointAt(0);
            if ((scalar <= 0x1f || (scalar >= 0x7f && scalar <= 0x9f))
                && !(multiline && [9, 10, 13].includes(scalar))) {
                continue;
            }
            const encodedBytes = encoder.encode(character).byteLength;
            if (bytes + encodedBytes > limit) break;
            output += character;
            bytes += encodedBytes;
        }
        return output;
    }

    function keyCommittedText(event, pressed, compositionActive = false) {
        if (!pressed || compositionActive || event?.isComposing) return "";
        const key = typeof event?.key === "string" ? event.key : "";
        if (Array.from(key).length !== 1) return "";
        const altGraph = Boolean(event?.getModifierState?.("AltGraph"));
        if (
            event?.metaKey ||
            (event?.ctrlKey && !altGraph) ||
            (event?.altKey && !altGraph)
        ) return "";
        return boundedCommittedText(key);
    }

    function eventState(event) {
        let value = 0;
        if (event?.shiftKey) value |= 0x0001;
        if (event?.getModifierState?.("CapsLock")) value |= 0x0002;
        if (event?.ctrlKey) value |= 0x0004;
        if (event?.altKey) value |= 0x0008;
        if (event?.metaKey) value |= 0x0040;
        if (event?.getModifierState?.("AltGraph")) value |= 0x0080;
        if (Number(event?.buttons) & 1) value |= 0x0100;
        if (Number(event?.buttons) & 4) value |= 0x0200;
        if (Number(event?.buttons) & 2) value |= 0x0400;
        return value;
    }

    function domButton(event) {
        return event?.button === 0 ? 1
            : event?.button === 1 ? 2
                : event?.button === 2 ? 3 : 0;
    }

    function buttonMask(button) {
        return button >= 1 && button <= 5 ? 0x0080 << button : 0;
    }

    function needsSyntheticShift(event, inputState) {
        if (inputState & 0x0001) return false;
        if (inputState & (0x0004 | 0x0008 | 0x0040)) return false;
        if (shiftedKeys[event?.code] !== event?.key) return false;
        return !(String(event?.code || "").startsWith("Key") &&
            (inputState & 0x0002));
    }

    function canvasPoint(event, canvas, mapPoint = null, rounding = "floor") {
        if (typeof mapPoint === "function") {
            const mapped = mapPoint(event);
            const x = Number(mapped?.x);
            const y = Number(mapped?.y);
            const round = rounding === "round" ? Math.round : Math.floor;
            return {
                x: round(Number.isFinite(x) ? x : Number(event?.clientX) || 0),
                y: round(Number.isFinite(y) ? y : Number(event?.clientY) || 0),
            };
        }
        const bounds = canvas.getBoundingClientRect();
        const width = Math.max(1, Number(canvas.width) || 1);
        const height = Math.max(1, Number(canvas.height) || 1);
        const scaleX = bounds.width > 0 ? width / bounds.width : 1;
        const scaleY = bounds.height > 0 ? height / bounds.height : 1;
        return {
            x: Math.max(0, Math.min(width - 1,
                Math.floor((Number(event?.clientX) - bounds.left) * scaleX))),
            y: Math.max(0, Math.min(height - 1,
                Math.floor((Number(event?.clientY) - bounds.top) * scaleY))),
        };
    }

    function pointerButton(event, pressed, point) {
        const button = domButton(event);
        if (!button) return null;
        let state = eventState(event);
        state = pressed ? state & ~buttonMask(button) : state | buttonMask(button);
        return Object.freeze({
            input: "pointer-button", pressed: Boolean(pressed), button,
            x: point.x, y: point.y, state,
        });
    }

    function doubleClick(event, point) {
        if (event?.button !== 0) return null;
        return Object.freeze({
            input: "pointer-button", pressed: true, button: 6,
            x: point.x, y: point.y, state: eventState(event),
        });
    }

    function wheel(event, point, ticks = 1) {
        const state = eventState(event);
        const horizontal = Math.abs(Number(event?.deltaX) || 0) > Math.abs(Number(event?.deltaY) || 0);
        const delta = horizontal ? Number(event.deltaX) : Number(event?.deltaY);
        if (!delta) return Object.freeze([]);
        // Preserve Alt-wheel desktop gestures while ordinary horizontal wheels
        // use standard X11 buttons 6/7 instead of disappearing at this boundary.
        const button = horizontal && !(state & 8) ? (delta < 0 ? 6 : 7) : (delta < 0 ? 4 : 5);
        const count = Math.max(1, Math.min(32,
            Math.trunc(Number.isFinite(Number(ticks)) ? Number(ticks) : 1)));
        const events = [];
        for (let index = 0; index < count; index++) {
            events.push(
                Object.freeze({ input: "pointer-button", pressed: true, button,
                    x: point.x, y: point.y, state, scrollDeltaX: index===0 ? (Number(event.deltaX)||0) * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 600 : 1) : 0, scrollDeltaY: index===0 ? (Number(event.deltaY)||0) * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 600 : 1) : 0, ...((state & 8) ? {scrollDelta: index===0 ? delta * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? 600 : 1) : 0} : {}) }),
                Object.freeze({ input: "pointer-button", pressed: false, button,
                    x: point.x, y: point.y, state: state | buttonMask(button) }),
            );
        }
        return Object.freeze(events);
    }

    // Chromium may coalesce several evdev REL_WHEEL notches into one DOM
    // WheelEvent while its main thread is painting. Browser RiceWM keeps its
    // historical one-DOM-event behavior; the desktop adapter opts into this
    // bounded magnitude decoder for physical device fidelity.
    function domWheelTicks(event, { pixelStep = 120, lineStep = 3 } = {}) {
        const delta = Math.abs(Number(event?.deltaY));
        if (!Number.isFinite(delta) || delta === 0) return 0;
        const mode = Number(event?.deltaMode) || 0;
        const step = mode === 1 ? lineStep : mode === 2 ? 1 : pixelStep;
        return Math.max(1, Math.min(32, Math.round(delta / step)));
    }

    function keyEvents(event, pressed, compositionActive = false) {
        const keycode = keycodes[event?.code];
        if (!keycode) return Object.freeze([]);
        const state = eventState(event);
        const text = keyCommittedText(event, pressed, compositionActive);
        const composition = compositionActive || event?.isComposing ? {composing:true} : {};
        if (needsSyntheticShift(event, state)) {
            return Object.freeze(pressed ? [
                Object.freeze({ input: "key", pressed: true,
                    keycode: keycodes.ShiftLeft, state }),
                Object.freeze({ input: "key", pressed: true, keycode,
                    state: state | 0x0001, text, ...composition }),
            ] : [
                Object.freeze({ input: "key", pressed: false, keycode,
                    state: state | 0x0001, text: "", ...composition }),
                Object.freeze({ input: "key", pressed: false,
                    keycode: keycodes.ShiftLeft, state: state | 0x0001,
                    text: "" }),
            ]);
        }
        const mask = modifierMasks[event.code] || 0;
        return Object.freeze([Object.freeze({
            input: "key", pressed: Boolean(pressed), keycode,
            state: pressed ? state & ~mask : state | mask,
            text,
            ...composition,
        })]);
    }

    // Window-manager shortcuts are owned by the shared compositor even while
    // a Rice or native X11 application has keyboard focus. Keep this policy in
    // the target-neutral boundary so browser and desktop cannot diverge.
    function isGlobalShortcutEvent(event) {
        if (event?.input !== "key") return false;
        const keycode = Number(event.keycode) >>> 0;
        const state = Number(event.state) & 0xffff;
        const alt = (state & 0x0008) !== 0;
        const control = (state & 0x0004) !== 0;
        const shift = (state & 0x0001) !== 0;
        return keycode === 133 || keycode === 134 ||
            ((state & 0x40) !== 0 && [37, 105, 64, 108].includes(keycode)) ||
            (alt && !control && keycode === 27) ||
            (alt && !control && (keycode === 70 || keycode === 75 || keycode === 76)) ||
            (alt && !control && (
                (shift && (keycode === 111 || keycode === 113 || keycode === 114)) ||
                (!shift && (keycode === 112 || keycode === 117))
            )) ||
            (alt && control && (keycode === 113 || keycode === 114));
    }

    class RiceDomInputNormalizer {
        constructor({ canvas, point = null, rounding = "floor",
            wheelTicks = null, horizontalWheel = false } = {}) {
            if (!canvas) throw new TypeError("Rice DOM input canvas is required");
            this.canvas = canvas;
            this.mapPoint = point;
            this.rounding = rounding;
            this.horizontalWheel = horizontalWheel;
            this.wheelTicks = typeof wheelTicks === "function" ? wheelTicks : null;
            this.heldButtons = new Map();
            this.heldKeys = new Map();
            this.lastPoint = { x: 0, y: 0 };
            this.lastState = 0;
        }
        point(event) {
            this.lastPoint = canvasPoint(
                event, this.canvas, this.mapPoint, this.rounding,
            );
            this.lastState = eventState(event);
            return this.lastPoint;
        }
        motion(event) {
            const point = this.point(event);
            return Object.freeze({
                input: "pointer-motion", ...point, state: eventState(event),
            });
        }
        pointer(event, pressed) {
            const point = this.point(event);
            const normalized = pointerButton(event, pressed, point);
            if (!normalized) return Object.freeze({ events: [], stale: false });
            const prior = this.heldButtons.get(normalized.button);
            if (!pressed && !prior) {
                return Object.freeze({ events: [], stale: false });
            }
            const events = [];
            if (pressed && prior) {
                events.push(Object.freeze({
                    input: "pointer-button", pressed: false,
                    button: normalized.button, ...prior.point,
                    state: prior.state | buttonMask(normalized.button),
                    recovered: true,
                }));
            }
            events.push(normalized);
            if (pressed) {
                this.heldButtons.set(normalized.button, {
                    point: { x: normalized.x, y: normalized.y },
                    state: normalized.state,
                });
            } else this.heldButtons.delete(normalized.button);
            return Object.freeze({ events: Object.freeze(events), stale: Boolean(prior && pressed) });
        }
        doubleClick(event) {
            const normalized = doubleClick(event, this.point(event));
            return normalized ? Object.freeze([normalized]) : Object.freeze([]);
        }
        wheel(event) {
            const point=this.point(event);
            if(this.horizontalWheel&&!event.altKey&&Number.isFinite(event.deltaX)&&Number.isFinite(event.deltaY)&&Math.abs(event.deltaX)>Math.abs(event.deltaY)) {
                const scale=event.deltaMode===1?16:event.deltaMode===2?600:1;
                return Object.freeze([Object.freeze({input:'pointer-axis',...point,state:eventState(event),scrollDeltaX:event.deltaX*scale,scrollDeltaY:event.deltaY*scale})]);
            }
            return wheel(event, point, this.wheelTicks?.(event) ?? 1);
        }
        key(event, pressed, compositionActive = false) {
            const normalized = keyEvents(event, pressed, compositionActive);
            for (const input of normalized) {
                if (input.input !== "key") continue;
                if (input.pressed) {
                    this.heldKeys.set(input.keycode,
                        modifierMasks[event?.code] || 0);
                } else this.heldKeys.delete(input.keycode);
            }
            this.lastState = eventState(event);
            return normalized;
        }
        cancelPointer() {
            const events = [];
            for (const [button, held] of this.heldButtons) {
                events.push(Object.freeze({
                    input: "pointer-button", pressed: false, button,
                    ...this.lastPoint,
                    state: this.lastState | held.state | buttonMask(button),
                    recovered: true,
                }));
            }
            this.heldButtons.clear();
            return Object.freeze(events);
        }
        cancelAll() {
            const events = [...this.cancelPointer()];
            for (const [keycode, mask] of this.heldKeys) {
                events.push(Object.freeze({
                    input: "key", pressed: false, keycode,
                    state: this.lastState | mask, text: "", recovered: true,
                }));
            }
            this.heldKeys.clear();
            this.lastState = 0;
            return Object.freeze(events);
        }
    }

    class RiceInputOwnershipArbiter {
        constructor() {
            this.x11Buttons = new Set();
            this.shellKeys = new Set();
        }
        reset() {
            const hadCapture = this.x11Buttons.size > 0 || this.shellKeys.size > 0;
            this.x11Buttons.clear();
            this.shellKeys.clear();
            return hadCapture;
        }
        classifyKeyboard({ event = null, modalDialogOpen = false,
            shellKeyboardCaptured = false, foregroundOwner = "x11",
            activeNativeWindow = false, nativeOwnsKeyboard = null,
            globalKey = false } = {}) {
            const keycode = Number(event?.keycode) >>> 0;
            // Global compositor accelerators remain global while an
            // override-redirect shell menu is visible. In particular X11
            // keycode 27 is R (Alt+R), not the DOM Escape value.
            if (globalKey) return "x11";
            const shell = Boolean(shellKeyboardCaptured) ||
                (keycode > 0 && this.shellKeys.has(keycode));
            if (shell) {
                if (keycode > 0) {
                    if (event?.pressed) this.shellKeys.add(keycode);
                    else this.shellKeys.delete(keycode);
                }
                return "shell";
            }
            const native = typeof nativeOwnsKeyboard === "boolean"
                ? nativeOwnsKeyboard
                : modalDialogOpen || (foregroundOwner === "native" && activeNativeWindow);
            return native ? "native" : "x11";
        }
        classifyPointer({ event, layer, foregroundOwner = "x11",
            nativeOwnsGesture = false, nativeAcceptsPointer = false, nativeOwnsOverlay = false,
            x11Captured = null } = {}) {
            const captured = x11Captured === null
                ? this.x11Buttons.size > 0
                : Boolean(x11Captured);
            if (captured) return "x11";
            if (nativeOwnsOverlay && layer !== "shell") return "native";
            const x11Window = layer === "window" || layer === "managed-frame";
            if (x11Window && foregroundOwner !== "native" && !nativeOwnsGesture) {
                return "x11";
            }
            if (nativeOwnsGesture || nativeAcceptsPointer) return "native";
            return "x11";
        }
        noteX11(event) {
            if (event?.input !== "pointer-button") return false;
            const button = Number(event.button);
            const stale = event.pressed && this.x11Buttons.has(button);
            if (event.pressed) this.x11Buttons.add(button);
            else this.x11Buttons.delete(button);
            return stale;
        }
    }

    global.RiceInputBoundary = Object.freeze({
        version: VERSION,
        keycodes,
        shiftedKeys,
        modifierMasks,
        boundedCommittedText,
        keyCommittedText,
        eventState,
        domButton,
        buttonMask,
        needsSyntheticShift,
        canvasPoint,
        pointerButton,
        doubleClick,
        wheel,
        domWheelTicks,
        keyEvents,
        isGlobalShortcutEvent,
        RiceDomInputNormalizer,
        RiceInputOwnershipArbiter,
    });
})(globalThis);
