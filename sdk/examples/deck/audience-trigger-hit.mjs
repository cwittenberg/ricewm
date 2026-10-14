import { inverseAffine, transformPoint, transformRect, unionRects } from './affine.mjs';
import { linePoints, pointSegmentDistance } from './canvas-geometry.mjs';

const positive = value => Number.isFinite(value) && value > 0;
const matrix = value => Array.isArray(value) && value.length === 6 && value.every(Number.isFinite);
const inside = ([x, y], [left, top, width, height]) =>
    x >= left && y >= top && x <= left + width && y <= top + height;

/** SceneSurface pointer coordinates are already contain-fit frame pixels,
 * including negative/outside coordinates in the letterbox. Frame dimensions
 * can be rounded independently; never use the nominal render ratio for both. */
export function audienceScenePoint(scene, frame, point) {
    if (!positive(scene?.size?.width) || !positive(scene?.size?.height) ||
        !positive(frame?.width) || !positive(frame?.height) ||
        !Number.isFinite(point?.x) || !Number.isFinite(point?.y) ||
        !inside([point.x, point.y], [0, 0, frame.width, frame.height])) return null;
    return { x: point.x / frame.width * scene.size.width, y: point.y / frame.height * scene.size.height };
}

function localRect(object) {
    const box = object.bounds;
    if (!box || ![box.x, box.y, box.width, box.height].every(Number.isFinite) ||
        box.width < 0 || box.height < 0 || !matrix(object.transform)) return null;
    return [box.x, box.y, box.width, box.height];
}

function segment(object) {
    if (object.geometry !== 'line') return null;
    if (!object.line && (!localRect(object) || !matrix(object.transform))) return null;
    const points = linePoints(object);
    return points.length === 2 && points.every(p => p.length === 2 && p.every(Number.isFinite)) ? points : null;
}

/** Pure audience hit policy, not editor selection normalization.
 *
 * targets: Set of eligible source IDs; visibility: Map of resolved LOCAL
 * visibility booleans, absent entries visible. Ancestor visibility applies.
 * Opacity is deliberately irrelevant (transparent visible hotspots work).
 * Source transforms already include ancestor transforms. Geometry is declared
 * local bounds (not contours/alpha); groups use the world AABB union of visible
 * descendant bounds, including gaps. Lines use stroke/2 or lineTolerance in
 * source scene units. Returns the foremost registered ID, preserving children.
 * Invalid/cyclic/orphaned hierarchy cannot become an accidental hotspot.
 */
export function hitAudienceTrigger({ scene, frame, point, targets, visibility = new Map(), lineTolerance = 3 }) {
    const converted = audienceScenePoint(scene, frame, point);
    if (!converted || !(targets instanceof Set) || !(visibility instanceof Map) ||
        !Number.isFinite(lineTolerance) || lineTolerance < 0 ||
        !Array.isArray(scene.objects) || scene.objects.length > 4096) return null;
    const byId = new Map(scene.objects.map(object => [object.id, object]));
    if (byId.size !== scene.objects.length) return null;
    const children = new Map(), roots = [], visible = new Map(), order = [];
    for (const object of scene.objects) {
        if (object.parentId) {
            const siblings = children.get(object.parentId) || [];
            siblings.push(object); children.set(object.parentId, siblings);
        } else roots.push(object);
    }
    function visit(object, ancestorsVisible, depth) {
        if (depth > 64 || visible.has(object.id)) return;
        const value = visibility.get(object.id);
        const shown = ancestorsVisible && (value === undefined || value === true);
        visible.set(object.id, shown); order.push(object);
        if (object.kind === 'group') for (const child of children.get(object.id) || []) visit(child, shown, depth + 1);
    }
    for (const root of roots) visit(root, true, 0);
    const extents = new Map();
    function extent(object) {
        if (extents.has(object.id)) return extents.get(object.id);
        if (!visible.get(object.id)) return null;
        let bounds = null;
        if (object.kind === 'group') {
            for (const child of children.get(object.id) || []) bounds = unionRects(bounds, extent(child));
        } else if (object.geometry === 'line') {
            const points = segment(object);
            if (points) {
                const [[x, y], [endX, endY]] = points;
                bounds = [Math.min(x, endX), Math.min(y, endY), Math.abs(endX - x), Math.abs(endY - y)];
            }
        } else {
            const rect = localRect(object);
            if (rect && inverseAffine(object.transform)) bounds = transformRect(object.transform, rect);
        }
        if (bounds && !bounds.every(Number.isFinite)) bounds = null;
        extents.set(object.id, bounds); return bounds;
    }
    const position = [converted.x, converted.y];
    for (let index = order.length - 1; index >= 0; index--) {
        const object = order[index];
        if (!targets.has(object.id) || !visible.get(object.id)) continue;
        if (object.kind === 'group') {
            const bounds = extent(object);
            if (bounds && inside(position, bounds)) return object.id;
        } else if (object.geometry === 'line') {
            const points = segment(object), width = object.stroke?.width;
            const tolerance = Math.max(lineTolerance, Number.isFinite(width) && width > 0 ? width / 2 : 0);
            if (points && pointSegmentDistance(position, ...points) <= tolerance) return object.id;
        } else {
            const rect = localRect(object), inverted = rect && inverseAffine(object.transform);
            if (inverted && inside(transformPoint(inverted, position), rect)) return object.id;
        }
    }
    return null;
}
