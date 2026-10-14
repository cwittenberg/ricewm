import { compileAnimationTimeline } from './animation-timeline.mjs';
import { prepareAnimationFrame } from './animation-render.mjs';
import { createAnimationPlayback } from './animation-playback.mjs';

/** Rehearsal on the editor surface. Preview supplies ordinary main-sequence
 * clicks when the preceding group has finished; it never fabricates target
 * triggers or changes saved timing. Any edit/selection/navigation cancels it. */
export function createAnimationPreview({ paint, restore, failed, changed = () => {},
    now, schedule = (callback, delay) => setTimeout(callback, delay), cancel = token => clearTimeout(token),
}) {
    let active = false, generation = 0, queuedClick = null, draw = null;
    function stop() {
        generation++; player.stop();
        if (queuedClick !== null) cancel(queuedClick);
        queuedClick = null; draw = null;
        if (active) { active = false; restore(); changed(false); }
    }
    const player = createAnimationPlayback({ now, schedule, cancel,
        paint(sample) {
            if (!active) return;
            const epoch = generation;
            paint(draw(sample));
            if (!active || epoch !== generation || sample.settled || sample.running || sample.nextBoundaryMs !== null || queuedClick !== null) return;
            if (sample.waiting.length) queuedClick = schedule(() => {
                if (!active || epoch !== generation) return;
                queuedClick = null;
                const before = player.snapshot;
                const after = player.click();
                // A slide with only unavailable target triggers must not spin
                // generating ineffective navigation clicks indefinitely.
                if (after && before.effects.every((value, index) => value.startMs === after.effects[index].startMs) &&
                    before.settled === after.settled && before.waiting.join('\0') === after.waiting.join('\0')) stop();
            }, 0);
        },
        settled: stop,
        failed(error) { stop(); failed(error); },
    });
    return Object.freeze({ cancel: stop, get active() { return active; }, get revision() { return generation; },
        play(rendered, projection) {
            stop();
            try {
                const timeline = compileAnimationTimeline(projection);
                draw = prepareAnimationFrame(rendered, projection);
                active = true;
                const epoch = generation;
                changed(true);
                if (active && epoch === generation) player.start(timeline);
            } catch (error) { stop(); failed(error); }
        },
    });
}
