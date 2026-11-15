import { routeConnector } from './visual-geometry.mjs';

const geometry = (o) => [
    o.id,
    o.x,
    o.y,
    o.width,
    o.height,
    o.rotation || 0,
    o.shape || 'rect',
    o.radius || 0,
    o.path || null,
];

/** Retain document-space routes across selection, paint and viewport changes.
 * Geometry and obstacle changes invalidate routes independently of paint.
 * The bounded cache owns immutable copies; caller previews never mutate it. */
export class ConnectorRouteCache {
    static controlFactory = Object.freeze({
        topics: ['visual', 'scene'],
        constructor: 'new ConnectorRouteCache({maximumEntries:2048,route:routeConnector})',
        methods:
            'route(source,target,options), clear(), inspect(). Reuses document-space connector geometry across zoom, pan, selection and color changes. Invalidates changed endpoints, ports, routing and orthogonal obstacles. Custom route callbacks must be pure.',
    });
    constructor({ maximumEntries = 2048, route = routeConnector } = {}) {
        if (
            !Number.isSafeInteger(maximumEntries) ||
            maximumEntries < 1 ||
            maximumEntries > 8192 ||
            typeof route !== 'function'
        )
            throw TypeError('Invalid connector route cache');
        this.maximumEntries = maximumEntries;
        this.compute = route;
        this.entries = new Map();
        this.hits = 0;
        this.misses = 0;
    }
    route(source, target, options = {}) {
        const key = JSON.stringify([
            geometry(source),
            geometry(target),
            options.routing || 'curve',
            options.sourcePort || 'right',
            options.targetPort || 'left',
            options.clearance ?? 16,
            options.routing === 'orthogonal'
                ? (options.obstacles || [])
                      .filter((o) => o.id !== source.id && o.id !== target.id)
                      .slice(0, 128)
                      .map(geometry)
                : [],
        ]);
        if (this.entries.has(key)) {
            const value = this.entries.get(key);
            this.entries.delete(key);
            this.entries.set(key, value);
            this.hits++;
            return value;
        }
        const route = this.compute(source, target, options);
        if (
            !route ||
            !Array.isArray(route.points) ||
            !Array.isArray(route.path) ||
            route.points.length > 4096 ||
            route.path.length > 16384
        )
            throw TypeError('Invalid connector route');
        const value = Object.freeze({
            points: Object.freeze(
                route.points.map((p) => {
                    if (![p.x, p.y].every(Number.isFinite))
                        throw TypeError('Invalid connector point');
                    return Object.freeze({ x: p.x, y: p.y });
                }),
            ),
            path: Object.freeze(route.path.map((p) => Object.freeze([...p]))),
        });
        this.entries.set(key, value);
        this.misses++;
        if (this.entries.size > this.maximumEntries)
            this.entries.delete(this.entries.keys().next().value);
        return value;
    }
    clear() {
        this.entries.clear();
    }
    inspect() {
        return { entries: this.entries.size, hits: this.hits, misses: this.misses };
    }
}
