import { dispatchTargetEvents } from './animation-target-queue.mjs';

/** Source timing, independent of painting and host clocks. A sample is a pure
 * function of the saved tree, playhead and accepted input history, so seeking
 * cannot replay clicks or mutate the document. */
const MAX_NODES = 2048, MAX_DEPTH = 64, MAX_DELAY = 600000;
const fail = message => { const error = new Error(message); error.code = 'INVALID_ANIMATION_TIMELINE'; throw error; };
const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const milliseconds = value => Number.isSafeInteger(value) && value >= 0;
const fixedDuration = node => node.duration.kind === 'ms' ? node.duration.value *
    (node.autoReverse ? 2 : 1) * (node.repeatCount ?? Infinity) : Infinity;
function freezeData(value, depth = 0) {
    if (!value || typeof value !== 'object') return;
    if (depth > MAX_DEPTH) fail('Animation metadata is too deep');
    for (const child of Object.values(value)) freezeData(child, depth + 1);
    Object.freeze(value);
}

/** Compile a fully loaded, playable projection. Unsupported source subtrees
 * must remain in the editor's projection; they cannot be executed as no-ops. */
export function compileAnimationTimeline({ root, mainSequence, nodes, playable }) {
    if (playable === false) fail('Source animation graph is not playable');
    if (!Array.isArray(nodes) || nodes.length > MAX_NODES || (root !== null && !id(root))) fail('Invalid timing tree');
    if (root === null) { if (nodes.length) fail('Rootless timing nodes'); }
    else if (!nodes.length) fail('Missing timing root');
    const list = structuredClone(nodes), byId = new Map();
    for (const node of list) {
        if (!node || !id(node.id) || byId.has(node.id) || !['parallel', 'sequence', 'effect'].includes(node.kind) ||
            !Array.isArray(node.children) || !Array.isArray(node.begin) || node.begin.length > 64 ||
            !node.duration || !['ms', 'auto', 'indefinite'].includes(node.duration.kind) ||
            node.duration.kind === 'ms' && (!milliseconds(node.duration.value) || node.duration.value > MAX_DELAY) ||
            !(node.repeatCount === null || Number.isFinite(node.repeatCount) && node.repeatCount > 0 && node.repeatCount <= 1000) ||
            typeof node.autoReverse !== 'boolean' || !['remove', 'freeze', 'hold'].includes(node.fill) ||
            node.endPolicy !== undefined && !['children', 'duration'].includes(node.endPolicy) ||
            node.unsupported?.length) fail('Unsupported or malformed timing node');
        if (node.kind !== 'effect' && (node.repeatCount !== 1 || node.autoReverse)) fail('Repeated timing containers require an expanded playback projection');
        if (node.kind === 'effect' && (node.children.length || !id(node.target?.object) ||
            node.target.subtarget?.kind !== 'object' || !node.effect || typeof node.effect.kind !== 'string' || !node.effect.kind.length ||
            node.endPolicy === 'children' ||
            node.duration.kind !== 'ms')) fail('Unsupported effect timing or target');
        byId.set(node.id, node);
    }
    const preorder = [], parents = new Map(), seen = new Set();
    function visit(key, parent, depth) {
        const node = byId.get(key);
        if (!node || seen.has(key) || depth > MAX_DEPTH || node.parentId !== parent) fail('Invalid timing ownership or cycle');
        seen.add(key); preorder.push(node); parents.set(key, parent);
        for (const child of node.children) visit(child, key, depth + 1);
    }
    if (root !== null) visit(root, null, 0);
    if (seen.size !== list.length) fail('Orphan timing nodes');
    if (mainSequence !== undefined && mainSequence !== null && (!byId.has(mainSequence) || byId.get(mainSequence).kind !== 'sequence')) fail('Invalid main animation sequence');
    for (const node of preorder) for (const begin of node.begin) {
        if (!begin || !['parent-start', 'node-start', 'node-end', 'click', 'target-click'].includes(begin.event) ||
            !Number.isSafeInteger(begin.offsetMs) || Math.abs(begin.offsetMs) > MAX_DELAY ||
            ['node-start', 'node-end'].includes(begin.event) && (!byId.has(begin.nodeId) || begin.nodeId === node.id) ||
            begin.event === 'target-click' && !id(begin.objectId)) fail('Invalid timing condition');
    }
    const effects = preorder.filter(node => node.kind === 'effect');
    freezeData(list);
    let scheduleCache = null;
    const eventHistory = (events, time) => {
        if (!Array.isArray(events) || events.length > 10000) fail('Invalid animation input history');
        let previous = -1;
        return events.map((event, index) => {
            if (!event || event.type !== 'click' || !milliseconds(event.timeMs) || event.timeMs < previous ||
                !(event.target === null || id(event.target))) fail('Invalid animation input event');
            previous = event.timeMs;
            return { ...event, index };
        }).filter(event => event.timeMs <= time);
    };

    function sample(timeMs, events = []) {
        if (!milliseconds(timeMs)) fail('Invalid animation playhead');
        const history = eventHistory(events, timeMs), historyKey = JSON.stringify(history);
        // Activation/dependency resolution changes only when accepted input
        // history changes, not every paint tick. Retain one bounded schedule;
        // seeking across an input or branching history recomputes it.
        const cached = scheduleCache?.key === historyKey ? scheduleCache : null;
        const starts = cached?.starts ?? new Map(), ends = cached?.ends ?? new Map(), resolving = new Set(), causes = cached?.causes ?? new Map();
        let inputHistory = history, targetClaims = cached?.targetClaims ?? new Map();
        const origin = { time: 0, event: -1, sources: [] };
        function memo(map, key, phase, calculate) {
            if (map.has(key)) return map.get(key);
            const token = `${phase}:${key}`;
            if (resolving.has(token)) fail('Cyclic timing dependency');
            resolving.add(token);
            try { const value = calculate(); map.set(key, value); return value; }
            finally { resolving.delete(token); }
        }
        function eligible(node) {
            const parent = byId.get(parents.get(node.id));
            if (!parent) return origin;
            const base = start(parent.id);
            if (!base) return null;
            const sources = [`begin:${parent.id}`];
            if (parent.kind !== 'sequence') return { ...base, sources };
            const index = parent.children.indexOf(node.id);
            if (!index) return { ...base, sources };
            const prior = end(parent.children[index - 1]);
            return prior && Number.isFinite(prior.time) ? { time: Math.max(base.time, prior.time), event: Math.max(base.event, prior.event),
                sources: [...sources, `end:${parent.children[index - 1]}`] } : null;
        }
        function start(key) {
            return memo(starts, key, 'begin', () => {
                const node = byId.get(key), gate = eligible(node);
                if (!gate) return null;
                const conditions = node.begin.length ? node.begin : [{ event: 'parent-start', offsetMs: 0 }];
                const candidates = conditions.map((condition, conditionIndex) => {
                    let value;
                    const sources = [...gate.sources];
                    if (condition.event === 'parent-start') value = parents.get(key) === null ? origin : start(parents.get(key));
                    else if (condition.event === 'node-start') { value = start(condition.nodeId); sources.push(`begin:${condition.nodeId}`); }
                    else if (condition.event === 'node-end') { value = end(condition.nodeId); sources.push(`end:${condition.nodeId}`); }
                    else {
                        const claim = targetClaims.get(key);
                        const event = condition.event === 'target-click'
                            ? claim?.conditionIndex === conditionIndex ? claim.event : null
                            : inputHistory.find(event => event.index > gate.event && event.timeMs >= gate.time && event.target === null);
                        value = event ? { time: event.timeMs, event: event.index } : null;
                    }
                    return value && Number.isFinite(value.time) ? {
                        time: Math.max(gate.time, value.time + condition.offsetMs), event: Math.max(gate.event, value.event), sources,
                    } : null;
                }).filter(Boolean).sort((a, b) => a.time - b.time || a.event - b.event);
                const chosen = candidates[0] || null;
                if (chosen) causes.set(`begin:${key}`, chosen.sources);
                return chosen;
            });
        }
        function end(key) {
            return memo(ends, key, 'end', () => {
                const node = byId.get(key), began = start(key);
                if (!began) return null;
                const sources = [`begin:${key}`];
                let value, unresolved = false;
                if (node.endPolicy === 'children' || node.duration.kind === 'auto') {
                    const children = node.children.map(end);
                    sources.push(...node.children.map(id => `end:${id}`));
                    unresolved = children.some(value => !value);
                    value = { time: unresolved ? Infinity : Math.max(began.time, ...children.map(value => value.time)),
                        event: Math.max(began.event, ...children.filter(Boolean).map(value => value.event)) };
                } else value = { time: began.time + (node.duration.kind === 'ms' && node.duration.value === 0 ? 0 : fixedDuration(node)), event: began.event };
                // Only explicit ancestor durations can cut a child short. Auto
                // containers derive their end from children and must not create
                // a false parent-end/child-end dependency cycle.
                for (let parent = byId.get(parents.get(key)); parent; parent = byId.get(parents.get(parent.id))) {
                    if (parent.endPolicy === 'children' || parent.duration.kind !== 'ms') continue;
                    const parentStart = start(parent.id);
                    if (parentStart && parentStart.time + fixedDuration(parent) < value.time) {
                        value = { time: parentStart.time + fixedDuration(parent), event: Math.max(value.event, parentStart.event) };
                        sources.push(`begin:${parent.id}`);
                    }
                }
                causes.set(`end:${key}`, sources);
                return unresolved && !Number.isFinite(value.time) ? null : value;
            });
        }
        function subscriptions(at, nextEvent = Infinity) {
            return preorder.flatMap((node, sourceOrder) => {
                if (targetClaims.has(node.id)) return [];
                const gate = eligible(node), began = start(node.id);
                if (!gate || gate.time > at || gate.event >= nextEvent || ancestorDeadline(node) <= at || began && began.time <= at) return [];
                return node.begin.flatMap((condition, conditionIndex) => condition.event === 'target-click' &&
                    (!began || Math.max(gate.time, at + condition.offsetMs) < began.time)
                    ? [{ nodeId: node.id, objectId: condition.objectId, conditionIndex, sourceOrder,
                        registeredAt: gate.time, registeredEvent: gate.event,
                        scheduledAt: Math.max(gate.time, at + condition.offsetMs) }] : []);
            });
        }
        let acceptedTargets = cached?.acceptedTargets ?? [];
        if (!cached) {
            const previous = scheduleCache?.history;
            const append = previous && previous.length <= history.length && previous.every((event, index) =>
                event.timeMs === history[index].timeMs && event.target === history[index].target);
            const replay = dispatchTargetEvents(history, (prefix, claims, event) => {
                inputHistory = prefix; targetClaims = claims;
                starts.clear(); ends.clear(); causes.clear();
                return subscriptions(event.timeMs, event.index);
            }, append ? { offset: previous.length, claims: scheduleCache.targetClaims, accepted: scheduleCache.acceptedTargets } : null);
            targetClaims = replay.claims; acceptedTargets = replay.accepted;
            inputHistory = history; starts.clear(); ends.clear(); causes.clear();
        }
        // Resolve every node even when it has no paint, to reject cycles and
        // invalid dependencies instead of silently ignoring an empty branch.
        if (!cached) for (const node of preorder) { start(node.id); end(node.id); }
        // Resolve simultaneous activation priority from the chosen causal
        // branch, never from memo evaluation order (unused alternatives can
        // eagerly resolve unrelated effects). Independent ties keep source order.
        const activation = cached?.activation ?? new Map(), beginEffects = new Map(effects.map(node => [`begin:${node.id}`, node.id]));
        const groups = new Map();
        if (!cached) for (const node of effects) {
            const began = start(node.id);
            if (began) { const group = groups.get(began.time) || []; group.push(node.id); groups.set(began.time, group); }
        }
        let priority = 0;
        for (const [, group] of [...groups].sort(([a], [b]) => a - b)) {
            const members = new Set(group), dependencies = new Map();
            for (const key of group) {
                const stack = [...(causes.get(`begin:${key}`) || [])], visited = new Set(), required = new Set();
                while (stack.length) {
                    const phase = stack.pop();
                    if (visited.has(phase)) continue;
                    visited.add(phase);
                    const effectId = beginEffects.get(phase);
                    if (effectId !== key && members.has(effectId)) required.add(effectId);
                    stack.push(...(causes.get(phase) || []));
                }
                dependencies.set(key, required);
            }
            while (dependencies.size) {
                const next = group.find(key => dependencies.has(key) && dependencies.get(key).size === 0);
                if (!next) fail('Cyclic animation activation priority');
                activation.set(next, priority++); dependencies.delete(next);
                for (const required of dependencies.values()) required.delete(next);
            }
        }
        function ancestorDeadline(node) {
            let deadline = Infinity;
            for (let parent = byId.get(parents.get(node.id)); parent; parent = byId.get(parents.get(parent.id))) {
                if (parent.endPolicy === 'children' || parent.duration.kind !== 'ms') continue;
                const began = start(parent.id);
                if (began) deadline = Math.min(deadline, began.time + fixedDuration(parent));
            }
            return deadline;
        }
        const fillEnds = cached?.fillEnds ?? new Map();
        function fillEnd(key) {
            if (fillEnds.has(key)) return fillEnds.get(key);
            const node = byId.get(key), parent = byId.get(parents.get(key));
            let lifetime = parent ? fillEnd(parent.id) : Infinity;
            if (node.fill === 'remove') lifetime = Math.min(lifetime, end(key)?.time ?? Infinity);
            else if (node.fill === 'freeze' && parent?.kind === 'sequence') {
                const next = parent.children[parent.children.indexOf(key) + 1];
                const nextStart = next && start(next);
                // A delayed sibling outside an expired active scope never
                // activates and therefore cannot release a frozen layer.
                if (nextStart && nextStart.time < ancestorDeadline(byId.get(next))) lifetime = Math.min(lifetime, nextStart.time);
            }
            fillEnds.set(key, lifetime); return lifetime;
        }
        const states = effects.map(node => {
            const began = start(node.id), ended = end(node.id);
            const startMs = began?.time ?? null, deadline = ancestorDeadline(node);
            const skipped = deadline <= timeMs && (startMs === null || startMs >= deadline);
            const endMs = ended?.time ?? (skipped ? deadline : null);
            const phase = skipped ? 'after' : startMs === null ? 'pending' : timeMs < startMs ? 'before' : timeMs >= endMs ? 'after' : 'active';
            const duration = node.duration.value, cycle = duration * (node.autoReverse ? 2 : 1);
            const elapsed = startMs === null ? 0 : Math.max(0, Math.min(timeMs, endMs) - startMs);
            const completeCycle = cycle > 0 && elapsed > 0 && elapsed % cycle === 0 && phase === 'after';
            const position = cycle === 0 ? 1 : completeCycle ? 1 : (elapsed % cycle) / cycle;
            const progress = skipped || phase === 'pending' || phase === 'before' ? 0 : node.autoReverse ? 1 - Math.abs(2 * position - 1) : position;
            const fillEndMs = fillEnd(node.id);
            const scopeExpired = skipped || (parents.get(node.id) !== null && fillEnd(parents.get(node.id)) <= timeMs);
            // Frozen containers retain individual child layers, not a snapshot
            // of all children: a child's remove fill still clears at its end.
            // Transition fill remains explicitly unqualified for compositors.
            const contributes = !scopeExpired && ['active', 'after'].includes(phase) && timeMs < fillEndMs;
            const ancestors = [];
            for (let parent = byId.get(parents.get(node.id)); parent; parent = byId.get(parents.get(parent.id))) {
                const began = start(parent.id), ended = end(parent.id);
                ancestors.unshift({ id: parent.id, fill: parent.fill, kind: parent.kind,
                    startMs: began?.time ?? null, endMs: ended?.time ?? null,
                    phase: !began ? 'pending' : timeMs < began.time ? 'before' : ended && timeMs >= ended.time ? 'after' : 'active' });
            }
            return { id: node.id, target: node.target, effect: node.effect, visibility: node.visibility, fill: node.fill, phase,
                startMs, endMs, progress, skipped, ancestors, fillEndMs, contributes, scopeExpired,
                elapsedMs: elapsed,
                activationOrder: activation.get(node.id) ?? null,
                iteration: cycle ? Math.max(0, Math.floor(elapsed / cycle) - (completeCycle ? 1 : 0)) : 0 };
        });
        const waiting = preorder.filter(node => {
            const gate = eligible(node);
            return !start(node.id) && gate && gate.time <= timeMs && ancestorDeadline(node) > timeMs &&
                node.begin.some(begin => ['click', 'target-click'].includes(begin.event));
        }).map(node => node.id);
        const boundaries = preorder.flatMap(node => [start(node.id)?.time, end(node.id)?.time, fillEnd(node.id)])
            .filter(value => Number.isFinite(value) && value > timeMs);
        const completion = root === null || mainSequence === null ? origin : end(mainSequence ?? root);
        const waitingTargets = [...new Set(subscriptions(timeMs).map(item => item.objectId))];
        scheduleCache = { key: historyKey, history, targetClaims, acceptedTargets, starts, ends, causes, activation, fillEnds };
        return { timeMs, effects: states, waiting, nextBoundaryMs: boundaries.length ? Math.min(...boundaries) : null,
            waitingTargets, targetEvents: acceptedTargets.map(item => ({ ...item })),
            running: states.some(state => state.phase === 'active'), settled: completion !== null && completion.time <= timeMs };
    }
    return Object.freeze({ sample, nodeCount: list.length, effectCount: effects.length });
}
