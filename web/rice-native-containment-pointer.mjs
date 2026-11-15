/** Same-window, exact-hit containment gestures. Paths and mutations stay in the app.
 * Reuses accepted native ownership metadata and the outer window's physical capture. */
export class NativeContainmentPointer {
    constructor(tracker) { this.tracker = tracker; this.registrations = new Map(); this.active = new Map(); this.suppressed = new Set(); }
    register(key, owner, records, emit) {
        const old = this.registrations.get(key);
        if (old?.owner === owner && old.signature === JSON.stringify(records)) return;
        if (old) this.unregister(key, old.owner);
        this.registrations.set(key, {owner, records: new Map(records.map(record => [record.control, record])), emit, signature: JSON.stringify(records)});
    }
    unregister(key, owner) {
        if (this.registrations.get(key)?.owner !== owner) return;
        this.cancel(key); this.registrations.delete(key);
    }
    cancel(key) {
        const active = this.active.get(key); this.active.delete(key);
        if (active?.moved) this.suppressed.add(key);
        if (active) active.registration.emit({phase: 'cancel', gesture: active.gesture, source: active.source.token, target: null});
    }
    valid(key, control) {
        const nodes = this.tracker.windows.get(key); let node = nodes?.get(control), seen = new Set();
        if (!node || !(node.state & 1)) return false;
        while (node && node.parent !== '0') {
            if (seen.has(node.parent)) return false; seen.add(node.parent);
            node = nodes.get(node.parent); if (!node || !(node.state & 1)) return false;
        }
        return true;
    }
    observe(key) {
        const active = this.active.get(key), nodes = this.tracker.windows.get(key);
        if (active && (!this.valid(key, active.source.control) || [...nodes.values()].some(node => node.kind === 50 || node.kind === 51))) this.cancel(key);
    }
    pointer(bridge, app, window, kind, x, y, button) {
        const key = `${app}:${window}`, registration = this.registrations.get(key);
        if (kind === 1) {
            this.cancel(key); this.suppressed.delete(key);
            if (button !== 1 || !registration) return false;
            const source = registration.records.get(String(bridge.hitTest(app, window, x, y)));
            if (!source?.source || !this.valid(key, source.control)) return false;
            if (this.tracker.windows.get(key)?.get(source.control)?.kind === 44) {
                const size = bridge.renderSizes.get(key), snapshot = size && bridge.render(app, window, size.width, size.height);
                const label = snapshot?.commands.find(command => command.type === 'text' && command.rect && String(bridge.hitTest(app, window, command.rect.x + Math.min(4, command.rect.width / 2), command.rect.y + command.rect.height / 2)) === source.control);
                if (!label) return false;
                const nodes = this.tracker.windows.get(key), node = nodes.get(source.control);
                const expandable = !!(node.state & 64) || [...nodes.values()].some(candidate => candidate.parent === source.control);
                const icons = snapshot.commands.filter(command => command.type === 'icon' && command.rect && command.rect.x < label.rect.x && String(bridge.hitTest(app, window, command.rect.x + command.rect.width / 2, command.rect.y + command.rect.height / 2)) === source.control).sort((a, b) => a.rect.x - b.rect.x);
                const contentIcon = icons.length > (expandable ? 1 : 0) ? icons.at(-1) : null;
                if (x < (contentIcon?.rect.x ?? label.rect.x)) return false;
            }
            const active = {registration, source, x, y, moved: false, gesture: crypto.randomUUID()}; this.active.set(key, active);
            registration.emit({phase: 'start', gesture: active.gesture, source: source.token, target: null, x, y}); return false;
        }
        if (kind === 4) { this.cancel(key); return false; }
        if (kind === 2 && this.suppressed.delete(key)) return true;
        const active = this.active.get(key); if (!active || ![0, 2].includes(kind)) return false;
        if (kind === 2 && button !== 1 || ![x, y].every(Number.isFinite) || !this.valid(key, active.source.control)) { this.cancel(key); return false; }
        active.moved ||= Math.hypot(x - active.x, y - active.y) >= 6;
        const target = registration.records.get(String(bridge.hitTest(app, window, x, y)));
        const accepted = target?.destination && target.control !== active.source.control && this.valid(key, target.control) ? target.token : null;
        if (kind === 2) this.active.delete(key);
        registration.emit({phase: kind === 2 ? 'drop' : 'move', gesture: active.gesture, source: active.source.token, target: active.moved ? accepted : null, moved: active.moved, x, y});
        return active.moved;
    }
    consumesRelease(app, window) { const key = `${app}:${window}`; return this.suppressed.has(key) || this.active.get(key)?.moved === true; }
    removeApp(app) { for (const [key, registration] of this.registrations) if (key.startsWith(`${app}:`)) { this.unregister(key, registration.owner); this.suppressed.delete(key); } }
}
