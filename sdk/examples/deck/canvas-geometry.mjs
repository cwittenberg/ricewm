/** Derived geometry only. All persistent edits go back to the native document. */
export function transformPoint(matrix, x, y) {
    return [matrix[0] * x + matrix[2] * y + matrix[4], matrix[1] * x + matrix[3] * y + matrix[5]];
}
export function objectCorners(object) {
    if (object.geometry === 'line') return linePoints(object);
    if (!object.transform || !object.bounds) return [];
    const { width: w, height: h } = object.bounds;
    return [[0, 0], [w, 0], [w, h], [0, h]].map(([x, y]) => transformPoint(object.transform, x, y));
}
export function linePoints(object) {
    if (object.line) return [object.line.start, object.line.end].map(({ x, y }) => [x, y]);
    if (!object.transform || !object.bounds) return [];
    const { x, y, width, height } = object.bounds;
    return [[x, y], [x + width, y + height]].map(([px, py]) => transformPoint(object.transform, px, py));
}
export function translatedCommand(scene, object, dx, dy) {
    if (object.geometry === 'line' && object.line?.editable) {
        const [start, end] = linePoints(object).map(([x, y]) => ({ x: x + dx, y: y + dy }));
        return { type: 'set-line-points', slide: scene.slideId, object: object.id, start, end };
    }
    return { type: 'set-object-box', slide: scene.slideId, object: object.id,
        box: { ...object.edit.box, x: object.edit.box.x + dx, y: object.edit.box.y + dy } };
}
export function pointSegmentDistance(point, start, end) {
    const dx = end[0] - start[0], dy = end[1] - start[1], square = dx * dx + dy * dy;
    const t = square ? Math.max(0, Math.min(1, ((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / square)) : 0;
    return Math.hypot(point[0] - start[0] - t * dx, point[1] - start[1] - t * dy);
}
export function visualBounds(object) {
    const points = objectCorners(object);
    if (!points.length) return null;
    const x = Math.min(...points.map(point => point[0])), y = Math.min(...points.map(point => point[1]));
    return { x, y, width: Math.max(...points.map(point => point[0])) - x, height: Math.max(...points.map(point => point[1])) - y };
}
/** A flat scene retains the native composite hierarchy through parentId. */
export function outermostObject(object, byId) {
    const seen = new Set();
    while (object?.parentId && !seen.has(object.id) && seen.size < 64) {
        seen.add(object.id);
        const parent = byId.get(object.parentId);
        if (!parent || parent.kind !== 'group' || seen.has(parent.id)) break;
        object = parent;
    }
    return object;
}
export function normalizedSelection(scene, ids) {
    const byId = new Map((scene?.objects || []).map(object => [object.id, object]));
    return [...new Set(ids.map(id => outermostObject(byId.get(id), byId)?.id).filter(Boolean))].slice(0, 32);
}
export const canResize = object => object.geometry === 'line' ? Boolean(object.line?.editable)
    : Boolean(object.edit?.canResize ?? object.edit?.canTransform);
export function objectAt(scene, x, y, tolerance = 5) {
    const byId = new Map(scene.objects.map(object => [object.id, object]));
    for (let index = scene.objects.length - 1; index >= 0; index--) {
        const object = scene.objects[index], m = object.transform, box = object.bounds;
        if (object.geometry === 'line') {
            const points = linePoints(object);
            if (points.length === 2 && pointSegmentDistance([x, y], ...points) <= Math.max(tolerance, (object.stroke?.width || 0) / 2)) return outermostObject(object, byId);
            continue;
        }
        if (!m || !box) continue;
        const det = m[0] * m[3] - m[1] * m[2];
        if (Math.abs(det) < 1e-12) continue;
        const dx = x - m[4], dy = y - m[5];
        const localX = (m[3] * dx - m[2] * dy) / det, localY = (-m[1] * dx + m[0] * dy) / det;
        if (localX >= 0 && localY >= 0 && localX <= box.width && localY <= box.height) return outermostObject(object, byId);
    }
    return null;
}
export function alignmentCommands(scene, objects, direction, reference = 'auto') {
    if (!scene?.size || !objects.length || objects.some(object => !object.edit?.canTransform)) return [];
    if (!['auto', 'slide', 'selection'].includes(reference) || !['left', 'center', 'right', 'top', 'middle', 'bottom', 'distribute-horizontal', 'distribute-vertical'].includes(direction)) return [];
    const toSlide = reference === 'slide' || reference === 'auto' && objects.length === 1;
    const bounds = objects.map(visualBounds);
    if (bounds.some(value => !value)) return [];
    const horizontal = ['left', 'center', 'right', 'distribute-horizontal'].includes(direction);
    const axis = horizontal ? 'x' : 'y', extent = horizontal ? 'width' : 'height';
    const targets = new Map();
    if (direction.startsWith('distribute-')) {
        if (objects.length < (toSlide ? 2 : 3)) return [];
        const indices = objects.map((_, index) => index).sort((a, b) => bounds[a][axis] - bounds[b][axis]);
        const first = toSlide ? 0 : bounds[indices[0]][axis], last = bounds[indices.at(-1)];
        const end = toSlide ? scene.size[extent] : last[axis] + last[extent];
        const spacing = (end - first - bounds.reduce((sum, value) => sum + value[extent], 0)) / (objects.length - 1);
        let cursor = first;
        for (const index of indices) { targets.set(index, cursor); cursor += bounds[index][extent] + spacing; }
    } else {
        const start = toSlide ? 0 : Math.min(...bounds.map(value => value[axis]));
        const end = toSlide ? scene.size[extent] : Math.max(...bounds.map(value => value[axis] + value[extent]));
        const mode = ['left', 'top'].includes(direction) ? 0 : ['right', 'bottom'].includes(direction) ? 1 : 0.5;
        bounds.forEach((value, index) => targets.set(index, start + mode * (end - start - value[extent])));
    }
    return objects.map((object, index) => translatedCommand(scene, object,
        horizontal ? targets.get(index) - bounds[index][axis] : 0,
        horizontal ? 0 : targets.get(index) - bounds[index][axis]));
}

export function previewBox(object, box) {
    const [a, b, c, d] = object.transform;
    if (object.table) {
        const sx = box.width / object.bounds.width, sy = box.height / object.bounds.height;
        return { ...object, transform: [a * sx, b * sx, c * sy, d * sy,
            box.x + box.width / 2 - (a * box.width + c * box.height) / 2,
            box.y + box.height / 2 - (b * box.width + d * box.height) / 2] };
    }
    return { ...object, bounds: { x: 0, y: 0, width: box.width, height: box.height },
        transform: [a, b, c, d, box.x + box.width / 2 - (a * box.width + c * box.height) / 2,
            box.y + box.height / 2 - (b * box.width + d * box.height) / 2] };
}

/** Resize in the object's local axes, keeping the opposite corner stationary. */
export function resizedBox(object, dx, dy, corner = 2) {
    const box = object.edit.box, [a, b, c, d] = object.transform, det = a * d - b * c;
    if (Math.abs(det) < 1e-12) return { ...box };
    const sx = [1, 2].includes(corner) ? 1 : -1, sy = corner >= 2 ? 1 : -1;
    const width = Math.max(4, box.width + sx * (d * dx - c * dy) / det);
    const height = Math.max(4, box.height + sy * (-b * dx + a * dy) / det);
    const dw = width - box.width, dh = height - box.height;
    return { ...box, width, height, x: box.x + (a * sx * dw + c * sy * dh - dw) / 2,
        y: box.y + (b * sx * dw + d * sy * dh - dh) / 2 };
}

export const normalizedDegrees = value => (value % 360 + 360) % 360;
/** Preserve flips and center while changing the source rotation angle. */
export function previewRotation(object, degrees) {
    const delta = (degrees - (object.edit.rotationDegrees || 0)) * Math.PI / 180;
    const cos = Math.cos(delta), sin = Math.sin(delta), [a, b, c, d] = object.transform;
    return previewBox({ ...object, transform: [cos*a-sin*b, sin*a+cos*b, cos*c-sin*d, sin*c+cos*d, 0, 0] }, object.edit.box);
}
export function rotationHandle(object, ratio = 1) {
    if (!object.edit?.canRotate || object.geometry === 'line') return null;
    const corners = objectCorners(object);
    if (corners.length !== 4) return null;
    const center = [(corners[0][0]+corners[2][0])/2, (corners[0][1]+corners[2][1])/2];
    const top = [(corners[0][0]+corners[1][0])/2, (corners[0][1]+corners[1][1])/2];
    const length = Math.hypot(top[0]-center[0], top[1]-center[1]);
    if (length < 1e-8) return null;
    return { center, top, point: [top[0]+(top[0]-center[0])*28/ratio/length, top[1]+(top[1]-center[1])*28/ratio/length] };
}
export function draggedRotation(object, start, point, snap = false) {
    const handle = rotationHandle(object);
    if (!handle) return object.edit.rotationDegrees || 0;
    const [cx, cy] = handle.center;
    const delta = Math.atan2(point[1]-cy, point[0]-cx) - Math.atan2(start[1]-cy, start[0]-cx);
    const degrees = (object.edit.rotationDegrees || 0) + delta * 180 / Math.PI;
    return normalizedDegrees(snap ? Math.round(degrees / 15) * 15 : degrees);
}

function multiply(left, right) {
    const [a,b,c,d,e,f] = left, [g,h,i,j,k,l] = right;
    return [a*g+c*h,b*g+d*h,a*i+c*j,b*i+d*j,a*k+c*l+e,b*k+d*l+f];
}
function inverse([a,b,c,d,e,f]) {
    const det = a*d-b*c;
    return Math.abs(det) < 1e-12 ? null : [d/det,-b/det,-c/det,a/det,(c*f-d*e)/det,(b*e-a*f)/det];
}
/** Apply each edited group's world-space delta to descendant world geometry.
 * Only display objects change; the native composite remains the source of truth. */
export function previewObjects(objects, { boxes = new Map(), lines = new Map(), rotations = new Map() } = {}) {
    const byId = new Map(objects.map(object => [object.id, object])), deltas = new Map();
    const direct = objects.map(object => {
        const next = rotations.has(object.id) ? previewRotation(object, rotations.get(object.id))
            : lines.has(object.id) ? { ...object, line: lines.get(object.id) }
            : boxes.has(object.id) ? previewBox(object, boxes.get(object.id)) : object;
        if (object.kind === 'group' && next !== object && object.transform) {
            const inverted = inverse(object.transform);
            if (inverted) deltas.set(object.id, multiply(next.transform, inverted));
        }
        return next;
    });
    if (!deltas.size) return direct;
    return direct.map(object => {
        let next = object, parentId = object.parentId;
        const seen = new Set([object.id]);
        while (parentId && !seen.has(parentId) && seen.size < 64) {
            seen.add(parentId);
            const delta = deltas.get(parentId);
            if (delta) {
                next = { ...next, transform: next.transform ? multiply(delta, next.transform) : null };
                if (next.line) next.line = { ...next.line, ...Object.fromEntries(['start','end'].map(key => {
                    const [x,y] = transformPoint(delta, next.line[key].x, next.line[key].y);
                    return [key,{x,y}];
                })) };
            }
            parentId = byId.get(parentId)?.parentId;
        }
        return next;
    });
}
