import { visualBounds, intersects } from './visual-geometry.mjs';

/** Deterministic layout shared by tree, graph, board and timeline views.
 * Pinned objects retain their positions. Returns geometry; never mutates input. */
export function layoutObjects(
    objects,
    {
        connections = [],
        type = 'tree',
        direction = 'right',
        gap = 40,
        layerGap = 100,
        columns = 4,
        groupBy = 'group',
        start = 0,
        end = 1,
        width = 1200,
    } = {},
) {
    if (
        !Array.isArray(objects) ||
        objects.length > 20000 ||
        !['tree', 'hierarchical', 'graph', 'grid', 'board', 'timeline'].includes(type) ||
        !['right', 'down'].includes(direction) ||
        ![gap, layerGap, width].every((v) => Number.isFinite(v) && v > 0) ||
        !Number.isInteger(columns) ||
        columns < 1 ||
        columns > 100
    )
        throw TypeError('Invalid object layout');
    const result = objects.map((o) => ({ ...o, ...visualBounds(o) })),
        byId = new Map(result.map((o) => [o.id, o]));
    if (byId.size !== result.length) throw TypeError('Layout object ids must be unique');
    const pinned = result.filter((o) => o.pinned),
        placed = [...pinned];
    const place = (o, x, y) => {
        if (o.pinned) return;
        if (direction === 'down' && ['tree', 'hierarchical', 'graph'].includes(type)) [x, y] = [y, x];
        o.x = x;
        o.y = y;
        let attempts = 0;
        while (
            placed.some((p) =>
                intersects(
                    { x: o.x - gap / 2, y: o.y - gap / 2, width: o.width + gap, height: o.height + gap },
                    p,
                ),
            )
        ) {
            o.y += o.height + gap;
            if (++attempts > result.length + 1) throw Error('Could not resolve layout collisions');
        }
        placed.push(o);
    };
    if (type === 'grid') {
        let y = 0;
        for (let i = 0; i < result.length; i += columns) {
            const row = result.slice(i, i + columns);
            let x = 0;
            for (const o of row) {
                place(o, x, y);
                x += o.width + gap;
            }
            y += Math.max(...row.map((o) => o.height)) + gap;
        }
        return result;
    }
    if (type === 'board') {
        const groups = new Map();
        for (const o of result) {
            const key = String(o.metadata?.[groupBy] ?? o[groupBy] ?? '');
            if (!groups.has(key)) groups.set(key, []);
            groups.get(key).push(o);
        }
        let x = 0;
        for (const group of groups.values()) {
            let y = 0;
            for (const o of group) {
                place(o, x, y);
                y += o.height + gap;
            }
            x += Math.max(...group.map((o) => o.width)) + layerGap;
        }
        return result;
    }
    if (type === 'timeline') {
        if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start)
            throw RangeError('Timeline needs a finite increasing range');
        for (const o of result) {
            const time = o.metadata?.[groupBy] ?? o[groupBy];
            if (!Number.isFinite(time)) throw TypeError('Timeline item needs a numeric time');
            place(o, ((time - start) / (end - start)) * width, 0);
        }
        return result;
    }
    const children = new Map(),
        incoming = new Map(result.map((o) => [o.id, 0]));
    const link = (a, b) => {
        if (!byId.has(a) || !byId.has(b) || a === b) return;
        if (!children.has(a)) children.set(a, new Set());
        if (!children.get(a).has(b)) {
            children.get(a).add(b);
            incoming.set(b, incoming.get(b) + 1);
        }
    };
    for (const o of result) if (o.parent) link(o.parent, o.id);
    for (const e of connections) link(e.source, e.target);
    const levels = new Map(),
        queue = result.filter((o) => incoming.get(o.id) === 0).map((o) => o.id);
    for (const key of queue) levels.set(key, 0);
    let cursor = 0;
    while (cursor < queue.length) {
        const key = queue[cursor++];
        for (const child of children.get(key) || []) {
            levels.set(child, Math.max(levels.get(child) || 0, levels.get(key) + 1));
            incoming.set(child, incoming.get(child) - 1);
            if (incoming.get(child) === 0) queue.push(child);
        }
    }
    // Cyclic graph components receive a stable finite layer instead of looping.
    for (const o of result) if (!queue.includes(o.id) && incoming.get(o.id) > 0) levels.set(o.id, 0);
    const layers = new Map();
    for (const o of result) {
        const level = levels.get(o.id) || 0;
        if (!layers.has(level)) layers.set(level, []);
        layers.get(level).push(o);
    }
    let x = 0;
    for (const [, layer] of [...layers].sort((a, b) => a[0] - b[0])) {
        let y = 0;
        for (const o of layer) {
            place(o, x, y);
            y += (direction === 'down' ? o.width : o.height) + gap;
        }
        x += Math.max(...layer.map((o) => (direction === 'down' ? o.height : o.width))) + layerGap;
    }
    return result;
}
layoutObjects.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature:
        'layoutObjects(objects,{type:"tree"|"hierarchical"|"graph"|"grid"|"board"|"timeline",connections,direction,gap,layerGap,groupBy,start,end,width})',
    behavior:
        'Pure deterministic layout. Pinned positions retained; finite cycle handling and collision separation. Apply returned geometry in one history transaction.',
});
