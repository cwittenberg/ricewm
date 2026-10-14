/** Clock ownership for the semantic animation timeline. Painting is injected so
 * Preview, audience playback and offline sampling use the same source timing.
 * Completion reports the main sequence only; interactive branches remain live. */
export function createAnimationPlayback({ paint, settled = () => {}, failed = error => { throw error; },
    now = () => performance.now(), schedule = (callback, delay) => setTimeout(callback, delay),
    cancel = token => clearTimeout(token),
}) {
    let generation = 0, token = null, current = null;
    const validTime = value => Number.isSafeInteger(value) && value >= 0;
    function clearTimer() { if (token !== null) cancel(token); token = null; }
    function stop() { generation++; clearTimer(); current = null; }
    function time(state) {
        if (!state.paused) state.timeMs = Math.max(state.timeMs, state.originTime + Math.floor(Math.max(0, now() - state.originClock)));
        return state.timeMs;
    }
    function update(state, epoch) {
        if (current !== state || generation !== epoch) return null;
        const revision = ++state.revision;
        const ownsUpdate = () => current === state && generation === epoch && state.revision === revision;
        clearTimer();
        try {
            const sample = state.timeline.sample(time(state), state.events);
            state.sample = sample;
            paint(sample);
            if (!ownsUpdate()) return sample;
            const completed = sample.settled && !state.settled;
            state.settled = sample.settled;
            if (completed) settled(sample);
            if (!ownsUpdate() || state.paused) return sample;
            // Idle click waits consume no frames. A finite parent deadline or
            // delayed effect still wakes the scheduler even with no active leaf.
            const boundary = sample.nextBoundaryMs === null ? null : Math.max(1, sample.nextBoundaryMs - sample.timeMs);
            const delay = sample.running ? Math.min(16, boundary ?? 16) : boundary;
            if (delay !== null) token = schedule(() => {
                if (!ownsUpdate() || state.paused) return;
                token = null; update(state, epoch);
            }, delay);
            return sample;
        } catch (error) {
            if (ownsUpdate()) { stop(); failed(error); }
            return null;
        }
    }
    return Object.freeze({
        start(timeline, { timeMs = 0, events = [], paused = false } = {}) {
            if (!timeline || typeof timeline.sample !== 'function' || !validTime(timeMs) || !Array.isArray(events)) throw new TypeError('Invalid animation playback');
            const history = structuredClone(events);
            // Validate before replacing the current show; malformed replacement
            // input must not tear down a still-valid presentation.
            timeline.sample(timeMs, history);
            stop();
            const state = { timeline, events: history, timeMs, originTime: timeMs, originClock: now(),
                paused: Boolean(paused), sample: null, settled: false, revision: 0 };
            current = state;
            return update(state, generation);
        },
        click(target = null) {
            if (target !== null && (typeof target !== 'string' || !target.length || target.length > 4096)) throw new TypeError('Invalid animation target');
            if (!current || current.paused) return null;
            const state = current, timeMs = time(state);
            // A new input after a backward seek branches the rehearsal history.
            state.events = state.events.filter(event => event.timeMs <= timeMs);
            state.events.push({ type: 'click', timeMs, target });
            return update(state, generation);
        },
        trigger(target) {
            if (typeof target !== 'string' || !target.length || target.length > 4096) throw new TypeError('Invalid animation target');
            if (!current || current.paused) return false;
            const state = current, timeMs = time(state), epoch = generation;
            const events = state.events.filter(event => event.timeMs <= timeMs);
            events.push({ type: 'click', timeMs, target });
            try {
                // Consumption is a scheduling decision. A delayed start may
                // leave every painted pixel unchanged while owning this click.
                const sample = state.timeline.sample(timeMs, events);
                if (!sample.targetEvents?.some(event => event.index === events.length - 1)) return false;
                state.events = events;
                update(state, epoch);
                return true;
            } catch (error) {
                if (current === state && generation === epoch) { stop(); failed(error); }
                return false;
            }
        },
        seek(timeMs) {
            if (!validTime(timeMs)) throw new RangeError('Invalid animation playhead');
            if (!current) return null;
            current.timeMs = timeMs; current.originTime = timeMs; current.originClock = now();
            return update(current, generation);
        },
        pause() {
            if (!current || current.paused) return;
            time(current); current.paused = true;
            update(current, generation);
        },
        resume() {
            if (!current || !current.paused) return;
            current.originTime = current.timeMs; current.originClock = now(); current.paused = false;
            update(current, generation);
        },
        stop,
        sampleNow() { return current ? current.timeline.sample(time(current), current.events) : null; },
        get active() { return current !== null; },
        get paused() { return current?.paused ?? false; },
        get snapshot() { return current?.sample ?? null; },
        get eligibleTargets() {
            if (!current || current.paused) return [];
            return [...current.timeline.sample(time(current), current.events).waitingTargets];
        },
    });
}
