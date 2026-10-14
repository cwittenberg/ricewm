import { createPresentationClock } from './presentation-clock.mjs';
import { prepareMorph } from './morph-render.mjs';
import { prepareTransition } from './transition-render.mjs';
import { transitionSettings } from './transition-contract.mjs';

/** Preview on the real editor surface, restoring its selection after completion
 * or any new interaction. No document mutation and no modal preview window. */
export function createTransitionPreview({ paint, restore, failed, clockOptions }) {
    const clock = createPresentationClock(clockOptions);
    let active = false, revision = 0;
    function cancel() {
        revision++; clock.stop();
        if (active) { active = false; restore(); }
    }
    return Object.freeze({ cancel,
        play(previous, next, transition, sources = null) {
            cancel();
            if (!transitionSettings(transition)) return;
            const sample = transition.effect === 'morph' ? prepareMorph(sources?.previous, sources?.next) : prepareTransition(previous, next, transition);
            active = true;
            clock.start({ duration: transition.effect === 'none' ? 0 : transition.durationMs,
                paint: progress => paint(sample(progress)), complete: cancel,
                failed(error) { cancel(); failed(error); },
            });
        },
        get active() { return active; },
        get revision() { return revision; },
    });
}
