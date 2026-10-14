// Browser IME/committed-text ingress for authoritative native text widgets.
// The transient textarea never mirrors document content or owns editing state.
export function focusedNativeTextTarget(snapshot) {
    const window = [...(snapshot?.windows || [])].reverse().find(window =>
        window.active && window.layerVisible !== false && !window.minimized);
    if (!window || window.popup || !window.focusedWidget || String(window.focusedWidget) === '0') return null;
    // A scene owns application input, not a native text caret. Older frames can
    // retain a caret request on a previously focused editor during actor updates.
    if (window.commands?.some(command => command.type === 'web-scene' &&
        String(command.surface) === String(window.focusedWidget))) return null;
    // `caret` is a native focused-text eligibility flag, not the blink phase.
    const command = window.commands?.find(command => command.type === 'text' && command.caret === true);
    if (!command?.rect || command.rect.width <= 0 || command.rect.height <= 0) return null;
    return { key: [window.app, window.window, window.focusedWidget].map(String).join(':'),
        x: window.frame.x + command.rect.x,
        y: window.frame.y + window.frame.titleHeight + command.rect.y };
}

export function createRiceNativeTextInput({ document, container, canvas, snapshot, enabled,
    commit, rejected = () => {}, compositionChanged = () => {}, maximumBytes = 4096 }) {
    if (!document || !container || !canvas || typeof snapshot !== 'function' ||
        typeof enabled !== 'function' || typeof commit !== 'function') throw new TypeError('Native text input host is incomplete');
    if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 4096) throw new RangeError('Native text commit limit');
    const controller = new AbortController();
    const encoder = new TextEncoder();
    let element = null, owner = null, composing = false, compositionOwner = null;
    let pendingComposition = null, compositionTimer = null, disposed = false, waitingForScene = false;
    let windowBlurred = false, resumeOwner = null;
    const listen = (target, type, handler, options = {}) => target.addEventListener(type, handler,
        { ...options, signal: controller.signal });
    const current = () => enabled() && container.isConnected ? focusedNativeTextTarget(snapshot()) : null;
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
        element.tabIndex = -1;
        Object.assign(element.style, { position: 'fixed', width: '1px', height: '1px', opacity: '0',
            padding: '0', border: '0', pointerEvents: 'none', resize: 'none', overflow: 'hidden' });
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
            clear();
        });
        element.addEventListener('input', () => { if (!composing) clear(); });
        element.addEventListener('keydown', () => { if (!composing) pendingComposition = null; });
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
        const focus = document.activeElement;
        const ownsFocus = focus === canvas || focus === element;
        if (!target || !ownsFocus || waitingForScene) {
            release({ restoreFocus: ownsFocus && !waitingForScene }); return;
        }
        if (owner !== target.key) { release(); owner = target.key; create(); }
        const bounds = canvas.getBoundingClientRect();
        element.style.left = `${bounds.left + target.x * bounds.width / canvas.width}px`;
        element.style.top = `${bounds.top + target.y * bounds.height / canvas.height}px`;
        if (document.activeElement !== element) element.focus({ preventScroll: true });
    }
    listen(document, 'pointerdown', event => {
        if (event.target === element) return;
        waitingForScene = true; release();
    }, { capture: true });
    listen(document, 'focusin', event => {
        // The synchronous clipboard fallback restores this same live input.
        // Removing it here would leave focus on BODY when copying finishes.
        if (event.target?.dataset?.riceClipboardTransfer === 'true') return;
        if (event.target !== canvas && event.target !== element) {
            if (event.target !== document.body && event.target !== document.documentElement) resumeOwner = null;
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
