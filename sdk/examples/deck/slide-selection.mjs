/** Stable document identities, independent of native cards and preview lifetime. */
export function createSlideSelection() {
    let order = [], selected = new Set(), active = null, anchor = null;
    const ordered = () => order.filter(id => selected.has(id));
    function activate(id) {
        if (!order.includes(id)) return;
        if (selected.size <= 1 && !selected.has(id)) { selected = new Set([id]); anchor = id; }
        else selected.add(id);
        active = id;
        anchor ||= id;
    }
    function reconcile(ids) {
        if (!Array.isArray(ids) || ids.some(id => typeof id !== 'string' || !id) || new Set(ids).size !== ids.length) throw Error('Invalid slide identities');
        const previousIndex = order.indexOf(active);
        order = [...ids];
        selected = new Set(ordered());
        if (!order.includes(active)) {
            active = order[Math.max(0, Math.min(previousIndex, order.length - 1))] || null;
            if (active) selected.add(active);
        }
        if (!order.includes(anchor)) anchor = active;
    }
    function choose(id, { control = false, shift = false } = {}) {
        if (!order.includes(id)) return active;
        if (shift) {
            anchor ||= active || id;
            const a = order.indexOf(anchor), b = order.indexOf(id);
            const range = order.slice(Math.min(a, b), Math.max(a, b) + 1);
            selected = new Set(control ? [...selected, ...range] : range);
            active = id;
        } else if (control) {
            if (selected.has(id)) {
                if (selected.size > 1) {
                    selected.delete(id);
                    if (active === id) {
                        const index = order.indexOf(id);
                        active = ordered().sort((a, b) => Math.abs(order.indexOf(a) - index) - Math.abs(order.indexOf(b) - index) || order.indexOf(a) - order.indexOf(b))[0];
                    }
                }
            } else { selected.add(id); active = id; }
            anchor = id;
        } else { selected = new Set([id]); active = anchor = id; }
        return active;
    }
    return Object.freeze({ reconcile, activate, choose,
        all() { selected = new Set(order); active ||= order[0] || null; anchor ||= active; },
        reset() { order = []; selected.clear(); active = anchor = null; },
        has: id => selected.has(id),
        get active() { return active; }, get anchor() { return anchor; }, get selected() { return ordered(); },
    });
}
