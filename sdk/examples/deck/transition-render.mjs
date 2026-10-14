import { SceneBuilder, remapSceneFrames } from '@ricewm/app';

const directions = Object.freeze({ left: [-1, 0], right: [1, 0], up: [0, -1], down: [0, 1] });

/** Compose native slide frames without re-decoding document resources per tick.
 * Direction describes the movement of the incoming slide, not its starting edge.
 * This is a paint strategy; the presentation clock owns timing and cancellation. */
export function prepareTransition(previous, next, transition) {
    const frames = remapSceneFrames(previous ? [previous, next] : [next]);
    const before = previous ? frames[0] : null, after = frames.at(-1);
    return progress => transitionFrame(before, after, transition, progress);
}

function transitionFrame(previous, next, transition, progress) {
    if (!Number.isFinite(progress)) throw new RangeError('Invalid transition progress');
    const { effect, direction = 'left' } = transition;
    if (!['none', 'fade', 'fade-black', 'push', 'wipe'].includes(effect) || !Object.hasOwn(directions, direction)) {
        throw new RangeError('Unsupported transition paint strategy');
    }
    const t = Math.max(0, Math.min(1, progress));
    if (effect === 'none' || t === 1) return next;
    if (t === 0 && previous) return previous;
    const { width, height } = next;
    const builder = new SceneBuilder(width, height).rect([0, 0, width, height], 0xff000000);
    const paint = (frame, x = 0, y = 0, opacity = 1) => {
        if (!frame || opacity === 0) return;
        builder.pushTransform([width / frame.width, 0, 0, height / frame.height, x, y], opacity)
            .append(frame.displayList).popTransform();
    };
    builder.pushClip([0, 0, width, height]);
    if (effect === 'fade') {
        paint(previous); paint(next, 0, 0, t);
    } else if (effect === 'fade-black') {
        if (t < .5) paint(previous, 0, 0, 1 - t * 2);
        else paint(next, 0, 0, t * 2 - 1);
    } else if (effect === 'push') {
        const [dx, dy] = directions[direction];
        paint(previous, dx * width * t, dy * height * t);
        paint(next, dx * width * (t - 1), dy * height * (t - 1));
    } else {
        paint(previous);
        const rect = direction === 'left' ? [width * (1 - t), 0, width * t, height]
            : direction === 'right' ? [0, 0, width * t, height]
                : direction === 'up' ? [0, height * (1 - t), width, height * t]
                    : [0, 0, width, height * t];
        builder.pushClip(rect); paint(next); builder.popClip();
    }
    builder.popClip();
    return builder.finish();
}
