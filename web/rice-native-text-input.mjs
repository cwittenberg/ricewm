import {nativeClientRect} from './rice-native-window-stack.mjs';
// Browser IME/committed-text ingress for authoritative native text widgets.
// The transient textarea never mirrors document content or owns editing state.
export function focusedNativeTextTarget(snapshot, { terminal = false } = {}) {
    const window = [...(snapshot?.windows || [])].reverse().find(window =>
        window.active && window.layerVisible !== false && !window.minimized);
    if (!window || window.popup) return null;
    if (terminal && window.entrypoint === 'rice-term' && !window.dialog &&
        !window.commands?.some(command => command.type === 'text' && command.caret === true)) {
        const focus = Number(window.focusedWidget || 0);
        const session = focus >= 100000 && (focus - 100000) % 512 === 0
            ? Math.floor((focus - 100000) / 512) : null;
        const grid = window.commands?.find(command => command.type === 'terminal-grid' &&
            (session === null || String(command.session) === String(session)));
        if (!grid?.rect || grid.rect.width <= 0 || grid.rect.height <= 0) return null;
        return { key: [window.app, window.window, 'terminal', grid.session].map(String).join(':'),
            terminal: true,
            x: window.frame.x + nativeClientRect(window.frame).x + grid.rect.x,
            y: window.frame.y + window.frame.titleHeight + grid.rect.y,
            width: grid.rect.width, height: grid.rect.height };
    }
    if (!window.focusedWidget || String(window.focusedWidget) === '0') return null;
    // A scene owns application input, not a native text caret. Older frames can
    // retain a caret request on a previously focused editor during actor updates.
    if (window.commands?.some(command => command.type === 'web-scene' &&
        String(command.surface) === String(window.focusedWidget))) return null;
    // `caret` is a native focused-text eligibility flag, not the blink phase.
    const command = window.commands?.find(command => command.type === 'text' && command.caret === true);
    if (!command?.rect || command.rect.width <= 0 || command.rect.height <= 0) return null;
    return { key: [window.app, window.window, window.focusedWidget].map(String).join(':'),
        x: window.frame.x + nativeClientRect(window.frame).x + command.rect.x,
        y: window.frame.y + window.frame.titleHeight + command.rect.y };
}

export function createRiceNativeTextInput({ document, container, canvas, snapshot, enabled,
    commit, rejected = () => {}, compositionChanged = () => {}, maximumBytes = 4096,
    mobileTerminal = false }) {
    if (!document || !container || !canvas || typeof snapshot !== 'function' ||
        typeof enabled !== 'function' || typeof commit !== 'function') throw new TypeError('Native text input host is incomplete');
    if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 4096) throw new RangeError('Native text commit limit');
    const controller = new AbortController();
    const encoder = new TextEncoder();
    let element = null, owner = null, composing = false, compositionOwner = null;
    let pendingComposition = null, compositionTimer = null, disposed = false, waitingForScene = false;
    let windowBlurred = false, resumeOwner = null;
    let nativeTouchFocus = false;
    const listen = (target, type, handler, options = {}) => target.addEventListener(type, handler,
        { ...options, signal: controller.signal });
    const current = () => enabled() && container.isConnected
        ? focusedNativeTextTarget(snapshot(), { terminal: mobileTerminal }) : null;
    function showKeyboard() {
        // Android exposes this API; iOS uses focus in the original tap handler.
        if (!current()?.terminal || document.activeElement !== element) return;
        try { document.defaultView.navigator.virtualKeyboard?.show(); } catch {}
    }
    function key(keycode) {
        commit({ input: 'key', pressed: true, keycode, state: 0, text: '' });
        commit({ input: 'key', pressed: false, keycode, state: 0, text: '' });
    }
    function setComposing(value) { composing = value; compositionChanged(value); }
    function clear() { if (element) { element.value = ''; element.setSelectionRange(0, 0); } }
    function release({ restoreFocus = false, preserveResume = false } = {}) {
        if (!preserveResume) resumeOwner = null;
        const previous = element, focused = previous && document.activeElement === previous;
        element = null; owner = null; compositionOwner = null; pendingComposition = null;
        clearTimeout(compositionTimer); compositionTimer = null; setComposing(false);
        if (previous) { previous.value = ''; previous.remove(); }
        if (focused && restoreFocus && enabled() && canvas.isConnected) canvas.focus({ preventScroll: true });
    }
    function valid(event) {
        return !disposed && element && event.target === element && document.activeElement === element &&
            !waitingForScene && current()?.key === owner;
    }
    function send(value) {
        const text = String(value || '');
        let bytes = 0;
        for (const character of text) {
            const scalar = character.codePointAt(0);
            if ((scalar >= 0xd800 && scalar <= 0xdfff) || scalar <= 0x1f || (scalar >= 0x7f && scalar <= 0x9f)) {
                rejected('Native text input rejected an invalid committed character.'); return;
            }
            bytes += encoder.encode(character).length;
            if (bytes > maximumBytes) {
                rejected(`Native text input exceeds the ${maximumBytes}-byte commit limit; nothing was inserted.`); return;
            }
        }
        if (text) commit({ input: 'text', state: 0, text });
    }
    function create() {
        element = document.createElement('textarea');
        element.dataset.riceNativeTextInput = 'true';
        element.setAttribute('aria-label', 'Native application text input');
        element.autocomplete = 'off'; element.autocapitalize = 'off'; element.spellcheck = false;
        element.setAttribute('autocorrect', 'off');
        element.setAttribute('inputmode', 'text');
        element.setAttribute('enterkeyhint', 'enter');
        element.tabIndex = -1;
        Object.assign(element.style, { position: 'fixed', width: '1px', height: '1px', opacity: '0',
            padding: '0', border: '0', pointerEvents: 'none', resize: 'none', overflow: 'hidden', fontSize: '16px' });
        // These element listeners are collected with the element on release.
        element.addEventListener('compositionstart', event => {
            event.stopPropagation();
            if (!valid(event)) return;
            pendingComposition = null; compositionOwner = owner; setComposing(true);
        });
        element.addEventListener('compositionend', event => {
            event.stopPropagation();
            if (!valid(event) || compositionOwner !== owner) return;
            setComposing(false); compositionOwner = null;
            send(event.data); pendingComposition = String(event.data || ''); clear();
            clearTimeout(compositionTimer);
            compositionTimer = setTimeout(() => { pendingComposition = null; compositionTimer = null; }, 0);
        });
        element.addEventListener('beforeinput', event => {
            event.stopPropagation();
            if (!valid(event)) { event.preventDefault(); clear(); return; }
            if (composing || event.isComposing || event.inputType === 'insertCompositionText') return;
            event.preventDefault();
            if (event.inputType === 'insertFromComposition' ||
                (pendingComposition !== null && event.data === pendingComposition)) { clear(); return; }
            if (event.inputType === 'insertText' || event.inputType === 'insertReplacementText') send(event.data);
            else if (current()?.terminal) {
                if (event.inputType === 'insertLineBreak' || event.inputType === 'insertParagraph') key(36);
                else if (event.inputType === 'deleteContentBackward') key(22);
                else if (event.inputType === 'deleteContentForward') key(119);
            }
            clear();
        });
        element.addEventListener('input', () => { if (!composing) clear(); });
        element.addEventListener('keydown', event => {
            if (!composing) pendingComposition = null;
            // Some phone keyboards provide a key name without a physical
            // code, including Backspace on an empty textarea. Mapped hardware
            // keys were already consumed by the native input handler.
            if (!valid(event) || event.defaultPrevented || composing || event.isComposing ||
                event.keyCode === 229 || !current()?.terminal) return;
            const keycode = {Enter:36,Backspace:22,Delete:119}[event.key];
            if (keycode) { event.preventDefault(); key(keycode); }
        });
        element.addEventListener('blur', () => { if (!disposed) queueMicrotask(() => sync()); });
        container.append(element);
    }
    function sync({ scene = false } = {}) {
        if (disposed) return;
        if (scene) waitingForScene = false;
        const target = current();
        if (windowBlurred) {
            if (target?.key !== resumeOwner) resumeOwner = null;
            release({ preserveResume: true }); return;
        }
        // Keep the same browser input connected while a terminal touch is
        // awaiting its native scene. Recreating it during the tap can leave
        // WebKit's keyboard process pointing at the detached input.
        if (waitingForScene && mobileTerminal && nativeTouchFocus && target?.terminal &&
            owner === target.key) return;
        const focus = document.activeElement;
        const ownsFocus = focus === canvas || focus === element ||
            (mobileTerminal && nativeTouchFocus && target?.terminal &&
                (focus === document.body || focus === document.documentElement));
        if (!target || !ownsFocus || waitingForScene) {
            release({ restoreFocus: ownsFocus && !waitingForScene }); return;
        }
        if (owner !== target.key) { release(); owner = target.key; create(); }
        const bounds = canvas.getBoundingClientRect();
        element.style.left = `${bounds.left + target.x * bounds.width / canvas.width}px`;
        element.style.top = `${bounds.top + target.y * bounds.height / canvas.height}px`;
        if (document.activeElement !== element) {
            element.focus({ preventScroll: true });
            showKeyboard();
        }
    }
    listen(document, 'pointerdown', event => {
        if (event.target === element) return;
        nativeTouchFocus = Boolean(event.isTrusted && event.pointerType === 'touch' &&
            (event.target === canvas || event.target === container));
        waitingForScene = true;
        if (mobileTerminal && nativeTouchFocus && current()?.terminal && current()?.key === owner) return;
        release();
    }, { capture: true });
    // A native scene arrives asynchronously, after iOS's keyboard activation
    // opportunity. A direct tap inside the current terminal must focus the
    // ingress synchronously, after the canvas's pointer handler has run.
    function focusTerminalTouch(event, point) {
        if (!mobileTerminal || !event.isTrusted) return;
        // RiceWM captures touch pointers on the desktop wrapper.
        if (event.target !== canvas && event.target !== container) return;
        const target = current(), bounds = canvas.getBoundingClientRect();
        if (!target?.terminal || bounds.width <= 0 || bounds.height <= 0) return;
        const x = (point.clientX - bounds.left) * canvas.width / bounds.width;
        const y = (point.clientY - bounds.top) * canvas.height / bounds.height;
        if (x < target.x || y < target.y || x >= target.x + target.width || y >= target.y + target.height) return;
        // WebKit's compatibility mouse focus follows pointerup and otherwise
        // takes focus back from the textarea. Native pointer input was already
        // delivered; cancel only that browser default at the end of the touch.
        event.preventDefault();
        sync({ scene: true });
        showKeyboard();
    }
    listen(container, 'pointerup', event => {
        if (event.pointerType === 'touch' && event.button === 0) focusTerminalTouch(event, event);
    });
    listen(container, 'touchend', event => {
        if (event.touches.length === 0 && event.changedTouches.length === 1)
            focusTerminalTouch(event, event.changedTouches[0]);
    }, { passive: false });
    listen(document, 'focusin', event => {
        // The synchronous clipboard fallback restores this same live input.
        // Removing it here would leave focus on BODY when copying finishes.
        if (event.target?.dataset?.riceClipboardTransfer === 'true') return;
        if (event.target !== canvas && event.target !== element) {
            if (event.target !== document.body && event.target !== document.documentElement) resumeOwner = null;
            if (event.target !== document.body && event.target !== document.documentElement) nativeTouchFocus = false;
            release({ preserveResume: windowBlurred });
        }
        else if (event.target === canvas && !waitingForScene) queueMicrotask(() => sync());
    });
    listen(document.defaultView, 'blur', () => {
        const resume = element && document.activeElement === element ? owner : null;
        windowBlurred = true; waitingForScene = true; release(); resumeOwner = resume;
    });
    listen(document.defaultView, 'focus', () => {
        windowBlurred = false;
        const resume = resumeOwner; resumeOwner = null;
        if (resume && current()?.key === resume &&
            [canvas, document.body, document.documentElement].includes(document.activeElement)) {
            waitingForScene = false; canvas.focus({ preventScroll: true }); sync();
        }
    });
    return Object.freeze({ sync, release, owns: target => Boolean(element && target === element),
        get composing() { return composing; },
        destroy() { if (disposed) return; disposed = true; controller.abort(); release(); } });
}
