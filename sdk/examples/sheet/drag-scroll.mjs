/** Scroll requests use logical viewport offsets; native geometry owns hidden,
 * filtered, frozen and variably sized tracks. No cell geometry is inferred. */
export function dragScrollViewport(frame, viewport, point, axis = null) {
    if (!frame || !point || !Number.isFinite(point.x) || !Number.isFinite(point.y) || axis === 'all') return null;
    const changes = {};
    for (const [name, coordinate, start, header, edge, count] of [
        ['rows', 'y', 'top', frame.column_header, frame.height, frame.row_count],
        ['columns', 'x', 'left', frame.row_header, frame.width, frame.column_count],
    ]) {
        if (axis && axis !== name) continue;
        const tracks = frame[name];
        if (!tracks.some(track => !track.frozen)) continue;
        const position = point[coordinate];
        const distance = position < header ? position - header : position >= edge ? position - edge + 1 : 0;
        if (!distance) continue;
        const step = Math.sign(distance) * Math.min(32, Math.ceil(Math.abs(distance) / 24));
        const frozenEnd = tracks.filter(track => track.frozen).at(-1)?.index;
        const maximum = Math.max(0, count - (viewport.pixelWidth ? 1 : Math.max(1, viewport[name] || 1)));
        const minimum = Math.min(maximum, frozenEnd === undefined ? 0 : frozenEnd + 1);
        const next = Math.max(minimum, Math.min(maximum, viewport[start] + step));
        if (next !== viewport[start]) changes[start] = next;
    }
    return Object.keys(changes).length ? {...changes, followSelection: false} : null;
}

/** Clamp a captured pointer to native visible track clips before hit testing.
 * The actual hit test retains merge anchors and logical track identities. */
export function dragScrollHitPoint(frame, point, axis = null) {
    if (!frame || !point || !Number.isFinite(point.x) || !Number.isFinite(point.y)) return null;
    const result = {...point};
    for (const [name, coordinate, header] of [['rows', 'y', frame.column_header], ['columns', 'x', frame.row_header]]) {
        if (axis && axis !== name) continue;
        const tracks = frame[name];
        if (!tracks.length) return null;
        const leading = point[coordinate] < header ? tracks.filter(track => !track.frozen) : tracks;
        const first = Math.min(...(leading.length ? leading : tracks).map(track => track.clip_start));
        const last = Math.max(...tracks.map(track => track.clip_end)) - 1;
        result[coordinate] = Math.max(header + first, Math.min(header + last, point[coordinate]));
    }
    return result;
}

/** One timer and at most one asynchronous viewport read per drag. stop() joins
 * the outstanding step so release never commits an endpoint from an old view. */
export function createDragAutoScroll({canScroll, step, onError, schedule = callback => setTimeout(callback, 80), cancel = clearTimeout}) {
    let timer = null, flight = null, stopped = false;
    const arm = () => {
        if (!stopped && timer === null && flight === null && canScroll()) timer = schedule(run);
    };
    const run = () => {
        timer = null;
        if (stopped || !canScroll()) return;
        flight = Promise.resolve().then(step).catch(error => { stopped = true; onError(error); }).finally(() => { flight = null; arm(); });
    };
    return {
        update() {
            if (stopped) return;
            if (!canScroll() && timer !== null) { cancel(timer); timer = null; }
            arm();
        },
        stop() {
            stopped = true;
            if (timer !== null) { cancel(timer); timer = null; }
            return flight || Promise.resolve();
        },
    };
}
