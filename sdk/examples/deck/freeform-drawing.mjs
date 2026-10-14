/** A disposable click-by-click path. No document mutation until finish. */
export function createFreeformDrawing({ closed, now = () => Date.now() }) {
    let vertices = [], hover = null, lastClick = null;
    const same = (a, b, tolerance) => Math.hypot(a.x - b.x, a.y - b.y) <= tolerance;
    const copy = point => ({ x: point[0], y: point[1] });
    function finish() {
        return vertices.length >= (closed ? 3 : 2) ? { points: vertices.map(value => ({ ...value })), closed } : null;
    }
    return Object.freeze({
        click(point, ratio = 1) {
            const next = copy(point), time = now(), tolerance = 6 / ratio;
            if (closed && vertices.length >= 3 && same(next, vertices[0], tolerance)) return finish();
            if (lastClick && time - lastClick.time < 400 && same(next, lastClick.point, tolerance)) {
                const result = finish(); if (result) return result;
            }
            if (!vertices.length || !same(next, vertices.at(-1), 0.01)) {
                if (vertices.length >= 256) throw new Error('A freeform shape can have up to 256 points. Press Enter to finish.');
                vertices.push(next);
            }
            hover = next; lastClick = { point: next, time }; return null;
        },
        move(point, constrain = false) {
            hover = copy(point);
            if (constrain && vertices.length) {
                const start = vertices.at(-1), dx = hover.x - start.x, dy = hover.y - start.y;
                const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * Math.PI / 4;
                const length = Math.hypot(dx, dy);
                hover = { x: start.x + Math.cos(angle) * length, y: start.y + Math.sin(angle) * length };
            }
        },
        get nextPoint() { return hover ? [hover.x, hover.y] : null; },
        backspace() { vertices.pop(); lastClick = null; },
        finish,
        preview() {
            if (!vertices.length) return null;
            const points = [...vertices, ...(hover ? [hover] : [])];
            const xs = points.map(point => point.x), ys = points.map(point => point.y);
            const x = Math.min(...xs), y = Math.min(...ys), width = Math.max(1, Math.max(...xs) - x), height = Math.max(1, Math.max(...ys) - y);
            const commands = points.map((point, index) => [index ? 'L' : 'M', point.x - x, point.y - y]);
            if (closed && points.length > 2) commands.push(['Z']);
            return { id: '$freeform-preview', kind: 'shape', geometry: null, bounds: { x: 0, y: 0, width, height },
                transform: [1, 0, 0, 1, x, y], paths: [{ commands, fill: closed, stroke: true }],
                fill: closed ? { color: '#DCEBFA', opacity: 0.5 } : null,
                stroke: { color: '#287AC7', opacity: 1, width: 2 }, edit: {} };
        },
    });
}
