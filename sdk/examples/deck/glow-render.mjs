import { vectorGlowPlan } from '@ricewm/app';
import { shapeById } from './shape-catalog.mjs';
import { inverseAffine, multiplyAffine } from './affine.mjs';

export const glowFrameKey = id => '@shape-glow:' + id;
const rectangle = (x, y, w, h) => [['M', x, y], ['L', x + w, y], ['L', x + w, y + h], ['L', x, y + h], ['Z']];
function silhouette(object) {
    const box = object.bounds, matrix = object.transform;
    if (!box || !matrix || object.kind === 'group' || object.kind === 'table') return null;
    const { x, y, width: w, height: h } = box;
    if (object.paths?.length) {
        const paths = object.paths.filter(path => path.fill || !object.fill);
        return (paths.length ? paths : object.paths).map(path => path.commands);
    }
    if (object.geometry === 'rect') return [rectangle(x, y, w, h)];
    if (object.geometry === 'ellipse') {
        const cx = x + w / 2, cy = y + h / 2, rx = w / 2, ry = h / 2, k = .5522847498307936;
        return [[['M', cx + rx, cy], ['C', cx + rx, cy + k * ry, cx + k * rx, cy + ry, cx, cy + ry],
            ['C', cx - k * rx, cy + ry, cx - rx, cy + k * ry, cx - rx, cy],
            ['C', cx - rx, cy - k * ry, cx - k * rx, cy - ry, cx, cy - ry],
            ['C', cx + k * rx, cy - ry, cx + rx, cy - k * ry, cx + rx, cy], ['Z']]];
    }
    if (object.geometry === 'line') {
        if (!object.line) return [[['M', x, y], ['L', x + w, y + h]]];
        const [a, b, c, d, e, f] = matrix, determinant = a * d - b * c;
        if (Math.abs(determinant) < 1e-12) return null;
        const local = ({ x, y }) => [(d * (x - e) - c * (y - f)) / determinant, (-b * (x - e) + a * (y - f)) / determinant];
        return [[['M', ...local(object.line.start)], ['L', ...local(object.line.end)]]];
    }
    const path = shapeById(object.geometry)?.path;
    return path?.length ? [path.map(([verb, ...values]) => [verb, ...values.map((v, i) => v / 1000 * (i % 2 ? h : w) + (i % 2 ? y : x))])] : null;
}

/** Transient preview resources only. The source-owned scene remains unchanged;
 * every renderer uses the same native geometry and immutable glow metadata. */
export function glowDemands(scene, options = {}) {
    const glowing = scene.objects.filter(object => object.glow && object.glow.radius > 0 && object.glow.opacity > 0);
    const budget = options.thumbnail ? 16 * 1024 : 256 * 1024;
    const plans = [], notices = [];
    const scale = (options.thumbnail ? options.thumbnailWidth ?? 256 : 1920) / (scene.size?.width || 1920);
    // A bounded frame cannot carry an unbounded number of separate textures.
    // Keep unsupported cases visible rather than silently painting a box.
    const maximumPixels = Math.floor(budget / Math.max(1, glowing.length));
    for (const object of glowing) {
        try {
            const paths = silhouette(object);
            if (!paths) throw RangeError('This object has no resolved vector silhouette');
            const plan = vectorGlowPlan({ paths, transform: object.transform, fill: Boolean(object.fill || object.image), glow: object.glow },
                { scale, maximumPixels });
            if (plan) plans.push({ id: glowFrameKey(object.id), plan, sourceTransform: [...object.transform], sourceBounds: { ...object.bounds } });
        } catch (error) { notices.push(`Glow unavailable: ${String(error.message).slice(0, 160)}`); }
    }
    return { plans, notices };
}

export function glowPreviewTransform(object, frame) {
    const inverse = inverseAffine(frame.sourceTransform), before = frame.sourceBounds, after = object.bounds;
    if (!inverse || !object.transform || !before || !after) return null;
    const sx = before.width > 0 ? after.width / before.width : 1, sy = before.height > 0 ? after.height / before.height : 1;
    return multiplyAffine(object.transform, multiplyAffine([sx, 0, 0, sy, after.x - before.x * sx, after.y - before.y * sy], inverse));
}
