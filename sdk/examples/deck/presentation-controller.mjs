import { loadDeckScene, renderDeckScene } from './scene-loader.mjs';
import { loadAnimationTimeline } from './animation-loader.mjs';

/** Read-only presentation state. The document remains authoritative; navigation
 * coalesces rapid input and decodes one bounded slide at a time. No worker is
 * shared concurrently with authoring and no late frame survives stop/restart. */
export function createPresentationController({ port, changed }) {
    let active = false, generation = 0, revision = null, count = 0;
    let index = 0, cursor = 0, requested = null, frame = null, blank = null, error = null, loading = false, ended = false;
    let task = null;
    let hidden = new Set();
    const state = () => Object.freeze({ active, loading, index, count, frame, blank, error, ended });
    const notify = () => {
        try { changed(state()); }
        catch (cause) {
            error = `Slide rendering failed: ${String(cause?.message || cause).slice(0, 200)}`;
            frame = null; loading = false;
            changed(state());
        }
    };
    function pump() {
        if (task) return task;
        const request = generation;
        task = Promise.resolve().then(async () => {
            while (active && generation === request && requested !== null) {
                const next = requested; requested = null;
                loading = true; error = null; notify();
                try {
                    const cancelled = () => !active || generation !== request;
                    const scene = await loadDeckScene(port, { index: next, revision, cancelled });
                    if (requested !== null && requested !== next) continue;
                    const rendered = await renderDeckScene(port, scene, { cancelled });
                    if (cancelled()) return;
                    // A newer request can reuse the serialized worker but must
                    // not briefly flash this now-unwanted frame on the audience.
                    if (requested !== null && requested !== next) continue;
                    const animationTimeline = await loadAnimationTimeline(port, { scene, revision, cancelled });
                    if (cancelled()) return;
                    if (requested !== null && requested !== next) continue;
                    if (animationTimeline) rendered.animationTimeline = animationTimeline;
                    requested = null; index = next; frame = rendered;
                } catch (cause) {
                    if (!active || generation !== request) return;
                    error = String(cause?.message || cause).slice(0, 240); frame = null;
                }
                loading = false; notify();
            }
        }).finally(() => {
            task = null;
            if (active && requested !== null) void pump();
        });
        return task;
    }
    function go(next) {
        if (!active || !Number.isSafeInteger(next)) return Promise.resolve();
        blank = null; ended = next >= count; cursor = Math.min(count, Math.max(0, next));
        if (ended) { notify(); return Promise.resolve(); }
        next = Math.max(0, next);
        requested = next; notify(); return pump();
    }
    function visibleSlide(next, direction) {
        while (next >= 0 && next < count && hidden.has(next)) next += direction;
        return next;
    }
    function previous() {
        const next = visibleSlide(cursor - 1, -1);
        return next < 0 ? Promise.resolve() : go(next);
    }
    return Object.freeze({
        get state() { return state(); },
        async start(options) {
            if (active || task) throw Error('A slide show is already active');
            if (!Number.isSafeInteger(options.count) || options.count < 1 || options.count > 4096 ||
                !Number.isSafeInteger(options.index) || options.index < 0 || options.index >= options.count ||
                typeof options.revision !== 'string' || !/^(0|[1-9]\d*)$/u.test(options.revision) ||
                options.hiddenSlides !== undefined && (!Array.isArray(options.hiddenSlides) || options.hiddenSlides.length > options.count ||
                    options.hiddenSlides.some(value => !Number.isSafeInteger(value) || value < 0 || value >= options.count))) throw Error('Invalid slide-show snapshot');
            generation++; active = true; count = options.count; revision = options.revision; index = options.index;
            hidden = new Set(options.hiddenSlides ?? []);
            frame = null; blank = null; ended = false; error = null;
            return go(visibleSlide(index, 1));
        },
        // An explicit slide-number jump may reveal a hidden slide; sequential
        // and automatic navigation always follow the visible-slide sequence.
        go,
        next() { return go(visibleSlide(cursor + 1, 1)); },
        previous,
        first() { return go(visibleSlide(0, 1)); },
        last() { const next = visibleSlide(count - 1, -1); return go(next < 0 ? count : next); },
        toggleBlank(color) {
            if (!active || !['black', 'white'].includes(color)) return;
            blank = blank === color ? null : color; notify();
        },
        async stop() {
            active = false; generation++; requested = null; frame = null; blank = null; loading = false; ended = false; error = null;
            notify(); await task;
        },
    });
}
