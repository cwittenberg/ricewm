/** Geometry shared by retained visual controls. All points are document units. */
export const finiteCoordinate = (value) => {
    if (!Number.isFinite(value) || Math.abs(value) > 1e9)
        throw new RangeError('Coordinate must be finite and within one billion units');
    return value;
};
export function visualBounds(value) {
    if (!value || !['x', 'y', 'width', 'height'].every((key) => Number.isFinite(value[key])))
        throw new TypeError('Expected x, y, width and height');
    for (const key of ['x', 'y', 'width', 'height']) finiteCoordinate(value[key]);
    if (value.width <= 0 || value.height <= 0) throw new RangeError('Object dimensions must be positive');
    return { x: value.x, y: value.y, width: value.width, height: value.height };
}
export const intersects = (a, b) =>
    a.x <= b.x + b.width && a.x + a.width >= b.x && a.y <= b.y + b.height && a.y + a.height >= b.y;
/** Equal tracks retain the caller's total extent. Input and history stay owned
 * by the caller; bounds match the native document track envelope. */
export function distributeTrackSizes(tracks) {
    if (!Array.isArray(tracks) || !tracks.length || tracks.length > 4096) throw TypeError('Expected 1–4096 track sizes');
    let total = 0;
    for (const size of tracks) {
        if (!Number.isFinite(size) || size < 1 || size > 1e6) throw RangeError('Track sizes must be finite and at least one unit');
        total += size;
    }
    if (total > 1e6) throw RangeError('Track extent exceeds one million units');
    const size = total / tracks.length;
    return Array.from({ length: tracks.length }, () => size);
}
distributeTrackSizes.controlFactory = Object.freeze({topics:['visual','scene','documents'],constructor:'distributeTrackSizes([96,160,224])',methods:'Return a fresh array of equal positive track sizes while preserving the total extent. Accepts 1–4096 finite sizes of at least one unit, with total at most one million. Use for columns, rows, panels or grids. The caller owns mutation, source units, selection and one history transaction.'});
export function unionBounds(items) {
    if (!items.length) return { x: 0, y: 0, width: 1, height: 1 };
    const x = Math.min(...items.map((o) => o.x)),
        y = Math.min(...items.map((o) => o.y));
    return {
        x,
        y,
        width: Math.max(1, ...items.map((o) => o.x + o.width - x)),
        height: Math.max(1, ...items.map((o) => o.y + o.height - y)),
    };
}
export function localPoint(object, point) {
    const angle = (-(object.rotation || 0) * Math.PI) / 180,
        c = Math.cos(angle),
        s = Math.sin(angle),
        cx = object.x + object.width / 2,
        cy = object.y + object.height / 2;
    return {
        x: c * (point.x - cx) - s * (point.y - cy) + object.width / 2,
        y: s * (point.x - cx) + c * (point.y - cy) + object.height / 2,
    };
}
export function containsShape(object, point) {
    const p = localPoint(object, point),
        w = object.width,
        h = object.height;
    if (p.x < 0 || p.y < 0 || p.x > w || p.y > h) return false;
    if (object.shape === 'ellipse')
        return ((p.x - w / 2) / (w / 2)) ** 2 + ((p.y - h / 2) / (h / 2)) ** 2 <= 1;
    if (object.shape === 'diamond')
        return Math.abs((p.x - w / 2) / (w / 2)) + Math.abs((p.y - h / 2) / (h / 2)) <= 1;
    const r = Math.min(
        w / 2,
        h / 2,
        object.shape === 'pill' ? h / 2 : (object.radius ?? (object.shape === 'roundedRect' ? 12 : 0)),
    );
    if (!r) return true;
    const dx = Math.max(r - p.x, 0, p.x - (w - r)),
        dy = Math.max(r - p.y, 0, p.y - (h - r));
    return dx * dx + dy * dy <= r * r;
}
export function rotatedBounds(object) {
    const a = ((object.rotation || 0) * Math.PI) / 180,
        w = Math.abs(Math.cos(a)) * object.width + Math.abs(Math.sin(a)) * object.height,
        h = Math.abs(Math.sin(a)) * object.width + Math.abs(Math.cos(a)) * object.height;
    return {
        x: object.x + (object.width - w) / 2,
        y: object.y + (object.height - h) / 2,
        width: w,
        height: h,
    };
}
/** Four local rectangle corners projected through its source rotation. */
export function visualCorners(object) {
    const {x,y,width,height}=visualBounds(object),rotation=object.rotation??0;
    if(!Number.isFinite(rotation))throw TypeError('Invalid object rotation');
    const a=(rotation%360)*Math.PI/180,c=Math.cos(a),s=Math.sin(a),cx=x+width/2,cy=y+height/2;
    return [[-width/2,-height/2],[width/2,-height/2],[width/2,height/2],[-width/2,height/2]]
        .map(([px,py])=>[cx+c*px-s*py,cy+s*px+c*py]);
}
/** Shared named corner/edge handles for any caller-projected rectangle.
 * Corner order remains NW, NE, SE, SW; edge midpoints follow N, E, S, W. */
export function rectangleHandles(corners) {
    if(!Array.isArray(corners)||corners.length!==4||corners.some(p=>!Array.isArray(p)||p.length!==2||!p.every(Number.isFinite)))throw TypeError('Expected four finite rectangle corners');
    const midpoint=(a,b)=>a.map((v,i)=>v/2+b[i]/2);
    return [['nw',corners[0]],['ne',corners[1]],['se',corners[2]],['sw',corners[3]],
        ['n',midpoint(corners[0],corners[1])],['e',midpoint(corners[1],corners[2])],
        ['s',midpoint(corners[2],corners[3])],['w',midpoint(corners[3],corners[0])]]
        .map(([handle,point])=>({handle,point:[...point]}));
}
visualCorners.controlFactory=Object.freeze({topics:['visual','scene'],constructor:'visualCorners({x,y,width,height,rotation?})',methods:'Return four source-rotated document-space corners without changing the object. Project them through the caller viewport before painting or hit testing.'});
rectangleHandles.controlFactory=Object.freeze({topics:['visual','scene'],constructor:'rectangleHandles([[x0,y0],[x1,y1],[x2,y2],[x3,y3]])',methods:'Return eight named corner/edge handle points in the caller coordinate space. Corners come first (nw,ne,se,sw), then n,e,s,w. Input arrays remain caller owned; results are independent copies. Use the same points for painting and pointer hit testing.'});
export function roundedRectPath({ x, y, width: w, height: h }, radius = 12) {
    const r = Math.max(0, Math.min(radius, w / 2, h / 2));
    return [
        ['M', x + r, y],
        ['L', x + w - r, y],
        ['Q', x + w, y, x + w, y + r],
        ['L', x + w, y + h - r],
        ['Q', x + w, y + h, x + w - r, y + h],
        ['L', x + r, y + h],
        ['Q', x, y + h, x, y + h - r],
        ['L', x, y + r],
        ['Q', x, y, x + r, y],
        ['Z'],
    ];
}
export function shapePath(object) {
    const { x, y, width: w, height: h } = object;
    if (object.shape === 'path' && Array.isArray(object.path)) {
        const base = object.pathViewport || object;
        return object.path.map((segment) => [
            segment[0],
            ...segment.slice(1).map((n, i) => (i % 2 ? y + (n * h) / base.height : x + (n * w) / base.width)),
        ]);
    }
    if (object.shape === 'diamond')
        return [
            ['M', x + w / 2, y],
            ['L', x + w, y + h / 2],
            ['L', x + w / 2, y + h],
            ['L', x, y + h / 2],
            ['Z'],
        ];
    return roundedRectPath(
        object,
        object.shape === 'pill' ? h / 2 : (object.radius ?? (object.shape === 'roundedRect' ? 12 : 0)),
    );
}
export function rgbaColor(color, opacity = 1) {
    if (
        typeof color !== 'string' ||
        !/^#[0-9a-f]{6}$/i.test(color) ||
        !Number.isFinite(opacity) ||
        opacity < 0 ||
        opacity > 1
    )
        throw TypeError('Expected #RRGGBB color and opacity 0..1');
    const n = parseInt(color.slice(1), 16);
    return ((Math.round(opacity * 255) << 24) | ((n & 255) << 16) | (n & 0xff00) | (n >>> 16)) >>> 0;
}
/** Bounded grid index; oversized objects use a separate overflow set. */
export class SpatialIndex {
    constructor({ cellSize = 256 } = {}) {
        if (!Number.isFinite(cellSize) || cellSize < 1) throw RangeError('Invalid spatial cell size');
        this.cellSize = cellSize;
        this.cells = new Map();
        this.entries = new Map();
        this.overflow = new Set();
    }
    keys(bounds) {
        const s = this.cellSize,
            x = Math.floor(bounds.x / s),
            y = Math.floor(bounds.y / s),
            right = Math.floor((bounds.x + bounds.width) / s),
            bottom = Math.floor((bounds.y + bounds.height) / s);
        if ((right - x + 1) * (bottom - y + 1) > 1024) return null;
        const out = [];
        for (let i = x; i <= right; i++) for (let j = y; j <= bottom; j++) out.push(`${i},${j}`);
        return out;
    }
    set(id, bounds) {
        this.delete(id);
        bounds = visualBounds(bounds);
        const keys = this.keys(bounds);
        this.entries.set(id, { bounds, keys });
        if (!keys) this.overflow.add(id);
        else
            for (const key of keys) {
                if (!this.cells.has(key)) this.cells.set(key, new Set());
                this.cells.get(key).add(id);
            }
        return this;
    }
    delete(id) {
        const entry = this.entries.get(id);
        if (!entry) return false;
        for (const key of entry.keys || []) {
            const cell = this.cells.get(key);
            cell?.delete(id);
            if (!cell?.size) this.cells.delete(key);
        }
        this.overflow.delete(id);
        return this.entries.delete(id);
    }
    query(bounds) {
        visualBounds(bounds);
        const keys = this.keys(bounds),
            ids = keys
                ? new Set([...this.overflow, ...keys.flatMap((key) => [...(this.cells.get(key) || [])])])
                : new Set(this.entries.keys());
        return [...ids].filter((id) => intersects(this.entries.get(id).bounds, bounds));
    }
    clear() {
        this.cells.clear();
        this.entries.clear();
        this.overflow.clear();
    }
}
export function connectionPort(object, side = 'right') {
    if (!['left', 'right', 'top', 'bottom', 'center'].includes(side))
        throw TypeError('Invalid connection port');
    const x = side === 'left' ? 0 : side === 'right' ? object.width : object.width / 2,
        y = side === 'top' ? 0 : side === 'bottom' ? object.height : object.height / 2,
        a = ((object.rotation || 0) * Math.PI) / 180,
        dx = x - object.width / 2,
        dy = y - object.height / 2;
    return {
        x: object.x + object.width / 2 + dx * Math.cos(a) - dy * Math.sin(a),
        y: object.y + object.height / 2 + dx * Math.sin(a) + dy * Math.cos(a),
    };
}
export function routeConnector(
    source,
    target,
    { routing = 'curve', sourcePort = 'right', targetPort = 'left', obstacles = [], clearance = 16 } = {},
) {
    if (!['straight', 'curve', 'orthogonal'].includes(routing)) throw TypeError('Invalid connector routing');
    const a = connectionPort(source, sourcePort),
        b = connectionPort(target, targetPort);
    if (routing === 'straight')
        return {
            points: [a, b],
            path: [
                ['M', a.x, a.y],
                ['L', b.x, b.y],
            ],
        };
    if (routing === 'curve') {
        const distance = Math.max(32, Math.abs(b.x - a.x) / 2),
            offset = (side, d) =>
                side === 'left' ? [-d, 0] : side === 'right' ? [d, 0] : side === 'top' ? [0, -d] : [0, d],
            u = offset(sourcePort, distance),
            v = offset(targetPort, distance);
        return {
            points: [a, b],
            path: [
                ['M', a.x, a.y],
                ['C', a.x + u[0], a.y + u[1], b.x + v[0], b.y + v[1], b.x, b.y],
            ],
        };
    }
    // Orthogonal visibility graph. Inflated obstacles keep edges off object faces.
    const boxes = obstacles
        .filter((o) => o.id !== source.id && o.id !== target.id)
        .slice(0, 128)
        .map((o) => ({
            x: o.x - clearance,
            y: o.y - clearance,
            width: o.width + clearance * 2,
            height: o.height + clearance * 2,
        }));
    const xs = [...new Set([a.x, b.x, ...boxes.flatMap((o) => [o.x, o.x + o.width])])],
        ys = [...new Set([a.y, b.y, ...boxes.flatMap((o) => [o.y, o.y + o.height])])];
    const blocked = (p, q) =>
        boxes.some((o) =>
            p.x === q.x
                ? p.x > o.x &&
                  p.x < o.x + o.width &&
                  Math.max(p.y, q.y) > o.y &&
                  Math.min(p.y, q.y) < o.y + o.height
                : p.y > o.y &&
                  p.y < o.y + o.height &&
                  Math.max(p.x, q.x) > o.x &&
                  Math.min(p.x, q.x) < o.x + o.width,
        );
    // Bound work independently of the document size.
    const candidates = [
        [a, { x: (a.x + b.x) / 2, y: a.y }, { x: (a.x + b.x) / 2, y: b.y }, b],
        ...ys.map((y) => [a, { x: a.x, y }, { x: b.x, y }, b]),
        ...xs.map((x) => [a, { x, y: a.y }, { x, y: b.y }, b]),
    ];
    const valid = candidates.filter((points) => points.slice(1).every((p, i) => !blocked(points[i], p))),
        length = (points) =>
            points
                .slice(1)
                .reduce((s, p, i) => s + Math.abs(p.x - points[i].x) + Math.abs(p.y - points[i].y), 0);
    const points = valid.sort((p, q) => length(p) - length(q))[0];
    if (!points)
        return {
            points: [a, b],
            path: [
                ['M', a.x, a.y],
                ['L', b.x, b.y],
            ],
            unrouted: true,
        };
    return { points, path: points.map((p, i) => [i ? 'L' : 'M', p.x, p.y]) };
}

SpatialIndex.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    constructor: 'new SpatialIndex({cellSize:256})',
    methods:
        'set(id,bounds), delete(id), query(bounds), clear(); bounded uniform-grid queries with oversized-object fallback',
});
routeConnector.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'routeConnector(source,target,{routing,sourcePort,targetPort,obstacles,clearance})',
    behavior:
        'Shared endpoint, curve and orthogonal routing. Returns points/path and an explicit unrouted flag when a bounded obstacle route cannot be found.',
});
