/** One cancellable presentation clock. Callers inject a clock for deterministic
 * seeking/tests; document timing is never derived from the number of frames. */
export function createPresentationClock({
    now = () => performance.now(),
    schedule = callback => setTimeout(callback, 16),
    cancel = token => clearTimeout(token),
} = {}) {
    let generation = 0, token = null, animation = null;
    function stop() {
        generation++;
        if (token !== null) cancel(token);
        token = null; animation = null;
    }
    function start({ duration, paint, complete = () => {}, failed = error => { throw error; }, reducedMotion = false }) {
        if (!Number.isFinite(duration) || duration < 0 || duration > 600000 || typeof paint !== 'function') {
            throw new RangeError('Invalid presentation timing');
        }
        stop();
        const epoch = generation, started = now();
        const current = { started, pausedAt: null, pausedTotal: 0, elapsed: 0 };
        animation = current;
        const tick = () => {
            if (generation !== epoch || animation !== current || current.pausedAt !== null) return;
            token = null;
            // Monotonic progress even if an injected or host clock moves back.
            current.elapsed = Math.max(current.elapsed, now() - started - current.pausedTotal);
            const progress = reducedMotion || duration === 0 ? 1 : Math.min(1, current.elapsed / duration);
            try {
                paint(progress);
                if (generation !== epoch || animation !== current) return;
                if (progress === 1) { animation = null; complete(); }
                else if (current.pausedAt === null) token = schedule(tick);
            } catch (error) {
                if (generation === epoch) stop();
                failed(error);
            }
        };
        current.tick = tick;
        tick();
    }
    return Object.freeze({ start, stop,
        pause() {
            if (!animation || animation.pausedAt !== null) return;
            animation.pausedAt = now();
            if (token !== null) cancel(token);
            token = null;
        },
        resume() {
            if (!animation || animation.pausedAt === null) return;
            animation.pausedTotal += Math.max(0, now() - animation.pausedAt);
            animation.pausedAt = null; animation.tick();
        },
        get running() { return animation !== null; },
        get paused() { return animation?.pausedAt !== null && animation !== null; },
    });
}
