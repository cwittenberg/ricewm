/** Pointer gestures are transient; only their completed command edits the file. */
export function drawingPoints(start, point, kind, constrain = false) {
    let [dx, dy] = [point[0] - start[0], point[1] - start[1]];
    if (constrain) {
        if (kind === 'line') {
            const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI / 4;
            const length = Math.hypot(dx, dy); dx = length * Math.cos(angle); dy = length * Math.sin(angle);
            if (Math.abs(dx) < 1e-8) dx = 0;
            if (Math.abs(dy) < 1e-8) dy = 0;
        } else {
            const size = Math.max(Math.abs(dx), Math.abs(dy));
            dx = (dx < 0 ? -1 : 1) * size; dy = (dy < 0 ? -1 : 1) * size;
        }
    }
    return { start: { x: start[0], y: start[1] }, end: { x: start[0] + dx, y: start[1] + dy } };
}
export function drawingBox({ start, end }) {
    return { x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) };
}
export function drawingCommand(slide, kind, points) {
    if (kind === 'line') return { type: 'insert-line', slide, ...points, stroke: { color: '#287AC7', width: 2 } };
    return { type: 'insert-object', slide, kind, box: drawingBox(points),
        text: '', fill: kind === 'text' ? null : '#287AC7', stroke: null };
}
export function drawingPreview(kind, points) {
    const box = drawingBox(points);
    return { id: '$drawing-preview', kind: kind === 'text' ? 'text' : 'shape', geometry: kind === 'text' ? 'rect' : kind,
        bounds: { x: 0, y: 0, width: box.width, height: box.height }, transform: [1, 0, 0, 1, box.x, box.y],
        line: kind === 'line' ? { ...points, editable: true } : null,
        fill: kind === 'line' || kind === 'text' ? null : { color: '#287AC7', opacity: 0.5 },
        stroke: { color: '#287AC7', opacity: 1, width: 2 }, edit: { canTransform: false } };
}
