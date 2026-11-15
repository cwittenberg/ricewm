/** Keep an exact native TerminalGrid press bound to its owner until release.
 * The outer shell already captures the physical window. This supplies the
 * missing widget capture without allowing an actor to choose its own target. */
export class NativeTerminalPointer {
    constructor(tracker) { this.tracker = tracker; this.active = new Map(); }
    valid(key, target) {
        const nodes = this.tracker.windows.get(key), seen = new Set(); let node = nodes?.get(String(target));
        if (node?.kind !== 63) return false;
        while (node) { if (!(node.state & 1) || seen.has(node)) return false; seen.add(node); if (node.parent === '0') return true; node = nodes.get(node.parent); }
        return false;
    }
    observe(key) { const capture = this.active.get(key), nodes = this.tracker.windows.get(key); if (capture && (!this.valid(key, capture.target) || [...nodes.values()].some(node => node.kind === 50 || node.kind === 51))) this.active.delete(key); }
    begin(bridge, app, window, x, y, button) {
        const key = `${app}:${window}`; this.active.delete(key); if (button !== 1) return;
        const target = bridge.hitTest(app, window, x, y); if (!target || !this.valid(key, target)) return;
        const size = bridge.renderSizes.get(key), snapshot = size && bridge.render(app, window, size.width, size.height);
        if (!snapshot || snapshot.dialog || snapshot.popup) return;
        const rect = snapshot.commands.find(command => command.type === 'terminal-grid' && x >= command.rect.x && y >= command.rect.y && x < command.rect.x + command.rect.width && y < command.rect.y + command.rect.height && bridge.hitTest(app, window, command.rect.x + command.rect.width / 2, command.rect.y + command.rect.height / 2) === target)?.rect;
        if (rect) this.active.set(key, {target, rect});
    }
    sample(app, window, kind, x, y, button) {
        const key = `${app}:${window}`, capture = this.active.get(key);
        if (!capture || ![0, 2, 4].includes(kind)) return null;
        if (!this.valid(key, capture.target) || ![x,y].every(Number.isFinite)) { this.active.delete(key); return null; }
        if (kind === 2 && button !== 1) return null;
        if (kind !== 0) this.active.delete(key);
        return {target: capture.target, kind, x: Math.trunc(x - capture.rect.x), y: Math.trunc(y - capture.rect.y), button: kind === 0 ? 0 : 1};
    }
    cancel(app, window) { this.active.delete(`${app}:${window}`); }
    removeApp(app) { for (const key of this.active.keys()) if (key.startsWith(`${app}:`)) this.active.delete(key); }
}
