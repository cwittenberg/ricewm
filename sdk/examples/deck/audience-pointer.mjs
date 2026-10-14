const blockers = ['blank', 'loading', 'transitioning', 'paused', 'ended', 'failed', 'closed'];
const primary = button => button === 1 || button === 0;
const positive = value => Number.isFinite(value) && value > 0;
const generationValid = value => value !== null && value !== undefined &&
    (typeof value === 'object' || typeof value === 'string' || typeof value === 'number' && Number.isFinite(value));

/** Pair native SceneSurface down/up without DOM capture or document mutation.
 * RAPP detail has kind/x/y/button/modifiers, not DOM buttons/pointerId. Native
 * primary is 1; 0 matches the existing surface compatibility convention.
 *
 * Call update on EVERY lifecycle/geometry change, including blanking, then
 * unblanking, even if no pointer event occurs between them. generation is the
 * stable rendered-slide identity (an object or token), not an animation epoch.
 * Frame extents are copied, so in-place resize also cancels the pending press.
 * Drag threshold uses native frame-coordinate units, not browser CSS pixels.
 *
 * Transition clicks may still finish a transition: the caller can omit the
 * transitioning blocker for that route, and MUST separately gate target hit
 * dispatch while transitioning. This helper never navigates or hits objects.
 */
export function createAudiencePointer({ dragThreshold = 5 } = {}) {
    if (!Number.isFinite(dragThreshold) || dragThreshold < 0) throw new RangeError('Invalid audience drag threshold');
    let current = null, down = null;
    const cancel = () => { down = null; };
    function update(state) {
        const valid = state?.active === true && generationValid(state.generation) &&
            positive(state.frame?.width) && positive(state.frame?.height) &&
            blockers.every(key => !state[key]);
        if (!valid) { current = null; cancel(); return false; }
        const next = { generation: state.generation, width: state.frame.width, height: state.frame.height };
        if (!current || current.generation !== next.generation || current.width !== next.width || current.height !== next.height) cancel();
        current = next;
        return true;
    }
    function pointer(detail, state) {
        if (!update(state)) return null;
        if (!detail || !['down', 'move', 'up'].includes(detail.kind)) { cancel(); return null; }
        const modifiers = detail.modifiers ?? 0;
        // Shift/Control/Alt/Super and extra keyboard modifiers aren't normal
        // audience clicks. CapsLock/NumLock and native high event flags may pass.
        if (!Number.isInteger(modifiers) || modifiers < 0 || modifiers > 65535 ||
            modifiers & (1 | 4 | 8 | 32 | 64 | 128) || !primary(detail.button) ||
            !Number.isFinite(detail.x) || !Number.isFinite(detail.y) ||
            detail.x < 0 || detail.y < 0 || detail.x > current.width || detail.y > current.height) {
            cancel(); return null;
        }
        const point = { x: detail.x, y: detail.y };
        if (detail.kind === 'down') { down = point; return null; }
        if (!down) return null;
        if (Math.hypot(point.x - down.x, point.y - down.y) > dragThreshold) { cancel(); return null; }
        if (detail.kind !== 'up') return null;
        cancel(); return point;
    }
    return Object.freeze({ update, pointer, cancel });
}
