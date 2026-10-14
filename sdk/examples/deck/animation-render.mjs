import { SceneBuilder, composeSceneFrame } from '@ricewm/app';
import { geometryFrame } from './scene-preview.mjs';

const identity = [1, 0, 0, 1, 0, 0];
const unsupported = message => { const error = new Error(message); error.code = 'UNSUPPORTED_OBJECT_ANIMATION'; throw error; };
const sameEffect = (a, b) => a?.category === b?.category && a?.kind === b?.kind && (a?.direction ?? null) === (b?.direction ?? null);

/** Prepare complete native object scopes once. The caller supplies samples from
 * the source timing scheduler; geometry, glyphs, images and shadows are neither
 * relaid out nor replaced with screenshots during animation ticks.
 *
 * First profile: whole objects/groups and group members, appear/fade entrance
 * and exit. Recursive retained ranges keep enclosing group shadows intact.
 * Source fill lifetimes must be resolved by the scheduler, not reconstructed
 * from effective active-end timestamps by this paint adapter.
 */
export function prepareAnimationFrame(rendered, projection) {
    if (!rendered?.scene || projection?.playable !== true || !Array.isArray(projection.nodes) || projection.nodes.length > 2048) unsupported('Animation source is not qualified for playback.');
    const source = structuredClone(projection.nodes);
    for (const node of source) {
        if (node.kind === 'unsupported' || node.unsupported?.length) unsupported('Unsupported source timing must be preserved without guessed playback.');
        if (node.fill === 'transition') unsupported('Transition fill needs a source-specific playback profile.');
    }
    const effects = source.filter(node => node.kind === 'effect');
    const projected = geometryFrame(rendered.scene, { textFrames: rendered.textFrames, imageFrames: rendered.imageFrames, captureLayers: true });
    const frame = projected.frame;
    const { width, height, displayList } = frame;
    const layers = (projected.layers.tree || projected.layers.objects).map(layer => ({ ...layer, children: [] }));
    const byId = new Map(layers.map(layer => [layer.id, layer])), roots = [];
    for (const layer of layers) {
        const parent = byId.get(layer.parentId);
        if (parent) parent.children.push(layer); else roots.push(layer);
    }
    const ids = new Set(), initialVisibility = new Map();
    for (const node of effects) {
        if (!node.id || ids.has(node.id)) unsupported('Animation identities must be unique.');
        ids.add(node.id);
        if (node.target?.subtarget?.kind !== 'object') unsupported('Text subtargets need a separate native animation profile.');
        if (typeof node.target.initialVisibility !== 'boolean' || initialVisibility.has(node.target.object) && initialVisibility.get(node.target.object) !== node.target.initialVisibility) {
            unsupported('Animation targets need a consistent source-projected initial visibility.');
        }
        initialVisibility.set(node.target.object, node.target.initialVisibility);
        if (!byId.has(node.target.object)) unsupported('The animation target has no retained native drawing scope.');
        if (!['entrance', 'exit'].includes(node.effect?.category) || !['appear', 'fade'].includes(node.effect?.kind) || node.effect.direction != null) {
            unsupported('This native animation compositor supports appear and fade entrance/exit effects only.');
        }
        if (node.repeatCount !== 1 || node.autoReverse) unsupported('Repeating or reversed property animations need a separate rendering profile.');
        if (typeof node.visibility?.value !== 'boolean' || !Number.isSafeInteger(node.visibility.offsetMs) || node.visibility.offsetMs < 0 || node.visibility.offsetMs > 600000) {
            unsupported('Animation visibility must come from a recognized source setter.');
        }
    }
    const prefix = displayList.subarray(0, projected.layers.backgroundEnd);
    const suffix = displayList.subarray(projected.layers.objects.at(-1)?.end ?? projected.layers.backgroundEnd);
    const pop = new SceneBuilder(width, height).popTransform().commands[0];
    function channels(sample) {
        if (!sample || !Number.isSafeInteger(sample.timeMs) || sample.timeMs < 0 || !Array.isArray(sample.effects) || sample.effects.length !== effects.length) {
            unsupported('Animation sample does not match the prepared source.');
        }
        const states = new Map();
        for (const [index, state] of sample.effects.entries()) {
            const expected = effects[index];
            if (state.id !== expected.id || state.target?.object !== expected.target.object || state.target?.subtarget?.kind !== 'object' ||
                state.target.initialVisibility !== expected.target.initialVisibility ||
                !sameEffect(state.effect, expected.effect) || !Number.isFinite(state.progress) || state.progress < 0 || state.progress > 1 ||
                !['pending', 'before', 'active', 'after'].includes(state.phase) ||
                typeof state.contributes !== 'boolean' || typeof state.scopeExpired !== 'boolean' ||
                !Number.isFinite(state.elapsedMs) || state.elapsedMs < 0 ||
                state.visibility?.value !== expected.visibility.value || state.visibility?.offsetMs !== expected.visibility.offsetMs ||
                state.contributes && (!Number.isFinite(state.startMs) || !['active', 'after'].includes(state.phase) || state.scopeExpired ||
                    !Number.isSafeInteger(state.activationOrder) || state.activationOrder < 0)) {
                unsupported('Animation sample needs matching source identities and resolved fill lifetimes.');
            }
            const values = states.get(expected.target.object) || [];
            values.push({ ...state, order: index }); states.set(expected.target.object, values);
        }
        const opacities = new Map(), visibilities = new Map();
        for (const [object, values] of states) {
            let visibility = initialVisibility.get(object), opacity = 1;
            const contributing = values.filter(state => state.contributes);
            // Recognized fade and visibility behaviors replace their own
            // channels. A later active layer overrides an earlier one just
            // as a retained layer does; removal reveals the surviving layer.
            // This is not multiplicative opacity composition.
            contributing.sort((a, b) => a.startMs - b.startMs || a.activationOrder - b.activationOrder || a.order - b.order);
            for (const state of contributing) {
                if (state.elapsedMs >= state.visibility.offsetMs) visibility = state.visibility.value;
                if (state.effect.kind === 'fade') opacity = state.effect.category === 'entrance' ? state.progress : 1 - state.progress;
            }
            opacities.set(object, visibility ? opacity : 0);
            visibilities.set(object, visibility);
        }
        return { opacities, visibilities };
    }
    const appearance = sample => channels(sample).opacities;
    const render = sample => {
        const opacities = appearance(sample);
        if (layers.every(layer => (opacities.get(layer.id) ?? 1) === 1)) return frame;
        const chunks = [prefix];
        function paintLayer(layer) {
            const opacity = opacities.get(layer.id) ?? 1;
            if (opacity === 0) return;
            if (opacity !== 1) chunks.push(new SceneBuilder(width, height).pushTransform(identity, opacity).commands[0]);
            let cursor = layer.start;
            for (const child of layer.children) {
                chunks.push(displayList.subarray(cursor, child.start));
                paintLayer(child); cursor = child.end;
            }
            chunks.push(displayList.subarray(cursor, layer.end));
            if (opacity !== 1) chunks.push(pop);
        }
        for (const layer of roots) paintLayer(layer);
        chunks.push(suffix);
        return composeSceneFrame(width, height, chunks);
    };
    // Detached local opacity values: parent/child multiplication remains a
    // property of retained scene scopes, never flattened into each child.
    Object.defineProperty(render, 'appearance', { value: appearance });
    // Visibility is a separate input channel: a transparent visible shape is
    // still a trigger, whereas an invisible shape (or ancestor) is not.
    Object.defineProperty(render, 'visibility', { value: sample => channels(sample).visibilities });
    return render;
}
