/** A service cancellation scope usable inside the restricted Rice app realm.
 * This is the signal subset used by SDK compute adapters, not a DOM AbortSignal;
 * it deliberately has no dependency on forbidden EventTarget prototype methods.
 */
export function createCancellationScope() {
    let aborted = false, reason;
    const listeners = new Map();
    const signal = Object.freeze({
        get aborted() { return aborted; },
        get reason() { return reason; },
        addEventListener(type, listener, options = {}) {
            if (type !== 'abort' || typeof listener !== 'function' || aborted) return;
            if (!listeners.has(listener)) listeners.set(listener, options?.once === true);
        },
        removeEventListener(type, listener) { if (type === 'abort') listeners.delete(listener); },
        throwIfAborted() { if (aborted) throw reason; },
    });
    return Object.freeze({
        signal,
        abort(value = new Error('Operation canceled')) {
            if (aborted) return;
            aborted = true; reason = value;
            const event = Object.freeze({ type: 'abort', target: signal });
            for (const [listener] of [...listeners]) {
                if (!listeners.has(listener)) continue;
                listeners.delete(listener);
                try { listener.call(signal, event); } catch { /* One observer cannot prevent other owners cleaning up. */ }
            }
        },
    });
}
