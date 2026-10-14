import { createPresentationClock } from './presentation-clock.mjs';
import { createAnimationPlayback } from './animation-playback.mjs';
import { transitionSettings } from './transition-contract.mjs';

/** Read-only playback ownership shared by rehearsal and the audience view.
 * Frames/resources are prepared once. A new slide, stop or failure invalidates
 * every old callback; automatic advance starts after its entering effect ends. */
export function createSlidePlayback({ paint, render, prepareTransition, prepareMorph, prepareAnimations, advance, failed,
    now = () => performance.now(), schedule = (callback, delay) => setTimeout(callback, delay), cancel = token => clearTimeout(token),
    reducedMotion = () => false,
}) {
    let previousRendered = null;
    let epoch = 0, frame = null, finalFrame = null, active = false, paused = false;
    let autoTimer = null, autoDelay = null, autoDeadline = null, showingTransition = false;
    let animation = null, animationPending = false;
    let animationPaintRevision = 0;
    const clock = createPresentationClock({ now, schedule: callback => schedule(callback, 16), cancel });
    const animations = createAnimationPlayback({ now, schedule, cancel,
        paint(sample) {
            if (!active || !animation) return;
            const generation = epoch, adapter = animation, revision = ++animationPaintRevision;
            frame = adapter.sample(sample); paint(frame);
            if (!active || generation !== epoch || animation !== adapter || revision !== animationPaintRevision) return;
            if (adapter.appearance) previousRendered = { ...previousRendered, objectOpacity: adapter.appearance(sample) };
        },
        settled() { animationPending = false; queueAdvance(epoch); },
        failed: error => report(error, epoch),
    });
    function clearTimer() { if (autoTimer !== null) cancel(autoTimer); autoTimer = null; }
    function report(error, generation) {
        if (generation !== epoch) return;
        clock.stop(); animations.stop(); clearTimer(); showingTransition = false; autoDelay = null;
        failed(error);
    }
    function queueAdvance(generation) {
        if (!active || paused || showingTransition || animationPending || autoDelay === null || generation !== epoch) return;
        clearTimer(); autoDeadline = now() + autoDelay;
        autoTimer = schedule(() => {
            autoTimer = null;
            if (!active || paused || generation !== epoch) return;
            autoDelay = 0;
            Promise.resolve().then(() => {
                if (active && !paused && generation === epoch) { autoDelay = null; return advance(); }
            }).catch(error => report(error, generation));
        }, autoDelay);
    }
    function complete(generation) {
        if (generation !== epoch || !active) return;
        showingTransition = false; frame = finalFrame;
        if (animation) animations.start(animation.timeline, { paused });
        else queueAdvance(generation);
    }
    function stop() {
        epoch++; clock.stop(); animations.stop(); clearTimer(); active = false; paused = false;
        animation = null; animationPending = false;
        frame = null; finalFrame = null; previousRendered = null; autoDelay = null; autoDeadline = null; showingTransition = false;
    }
    return Object.freeze({
        show(rendered, { automatic = true } = {}) {
            const previous = frame, source = previousRendered;
            stop(); active = true; previousRendered = rendered;
            const generation = epoch, transition = rendered.scene.transition;
            try {
                animation = prepareAnimations?.(rendered) ?? null;
                animationPending = Boolean(animation);
                const initial = animation?.timeline.sample(0);
                finalFrame = animation ? animation.sample(initial) : render(rendered);
                if (animation?.appearance) previousRendered = { ...rendered, objectOpacity: animation.appearance(initial) };
                const valid = transitionSettings(transition);
                autoDelay = automatic && valid && (!rendered.scene.hasAnimationTiming || animation) ? transition.advanceAfterMs : null;
                if (valid && transition.effect !== 'none' && transition.durationMs > 0 && !reducedMotion()) {
                    const sample = transition.effect === 'morph' ? prepareMorph(source, previousRendered) : prepareTransition(previous, finalFrame, transition);
                    showingTransition = true;
                    clock.start({ duration: transition.durationMs,
                        paint(progress) { frame = sample(progress); paint(frame); },
                        complete: () => complete(generation), failed: error => report(error, generation),
                    });
                } else { frame = finalFrame; paint(frame); complete(generation); }
            } catch (error) { report(error, generation); }
        },
        pause() {
            if (!active || paused) return;
            paused = true; clock.pause(); animations.pause();
            if (autoTimer !== null) { autoDelay = Math.max(0, autoDeadline - now()); clearTimer(); }
        },
        resume() {
            if (!active || !paused) return;
            paused = false;
            if (frame) paint(frame);
            clock.resume(); animations.resume(); queueAdvance(epoch);
        },
        finishTransition() {
            if (!showingTransition) return false;
            clock.stop();
            try { frame = finalFrame; paint(frame); complete(epoch); }
            catch (error) { report(error, epoch); }
            return true;
        },
        advanceAnimation() {
            if (!active || paused || showingTransition || !animations.active || animations.snapshot?.settled) return false;
            animations.click(); return true;
        },
        triggerAnimation(objectId) {
            if (!active || paused || showingTransition) return false;
            return animations.trigger(objectId);
        },
        get animationTargets() {
            return !active || paused || showingTransition ? [] : animations.eligibleTargets;
        },
        get interactionState() {
            if (!active || paused || showingTransition || !animation) return null;
            const sample = animations.sampleNow();
            return sample ? { frame, targets: new Set(sample.waitingTargets),
                visibility: animation.visibility?.(sample) ?? new Map() } : null;
        },
        repaint() { if (frame) paint(frame); },
        stop,
        get transitioning() { return showingTransition; },
    });
}
