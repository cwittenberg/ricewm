import { Control } from './control.mjs';
import { Workspace } from './workspace.mjs';

/** Responsive placement shared by retained token and command collections. */
export function wrapLayout(items, { width, gap = 8 } = {}) {
    if (
        !Array.isArray(items) ||
        !Number.isFinite(width) ||
        width <= 0 ||
        !Number.isFinite(gap) ||
        gap < 0 ||
        gap > 64
    )
        throw TypeError('Invalid wrapping layout');
    let x = 0,
        y = 0,
        rowHeight = 0;
    const bounds = [];
    for (const item of items) {
        if (
            !Number.isFinite(item.width) ||
            !Number.isFinite(item.height) ||
            item.width <= 0 ||
            item.height <= 0
        )
            throw RangeError('Wrap items need positive width/height');
        const w = Math.min(width, item.width);
        if (x && x + w > width) {
            x = 0;
            y += rowHeight + gap;
            rowHeight = 0;
        }
        bounds.push({ x, y, width: w, height: item.height });
        x += w + gap;
        rowHeight = Math.max(rowHeight, item.height);
    }
    return { bounds, height: y + rowHeight };
}
export class WrapPanel extends Workspace {
    static constructorOptions = Object.freeze(['items', 'gap']);
    static authoring = Object.freeze({
        constructor:
            'new WrapPanel({items:[{id:"action",control:new Button({text:"Action"}),width:100,height:32}],gap:8})',
        methods:
            'setItems(items); wraps native controls on actual resize, preserves identities and focused drafts, and reports contentHeight. Omitted controls are removed without destroying caller-owned instances. Explicit item sizes are native pixels.',
    });
    constructor({ items = [], gap = 8, ...options } = {}) {
        let panel;
        super({ ...options, height: options.height ?? 64, render: (view) => panel?.renderItems(view) });
        panel = this;
        this.gap = gap;
        this.items = [];
        this.setItems(items);
    }
    setItems(items) {
        if (
            !Array.isArray(items) ||
            items.length > 256 ||
            items.some(
                (item) =>
                    !item ||
                    typeof item.id !== 'string' ||
                    !/^[A-Za-z0-9_.:-]{1,100}$/.test(item.id) ||
                    !(item.control instanceof Control),
            ) ||
            new Set(items.map((i) => i.id)).size !== items.length ||
            new Set(items.map((i) => i.control)).size !== items.length
        )
            throw TypeError('WrapPanel requires bounded unique item ids and native controls');
        const ids = new Set(items.map((i) => i.id));
        for (const [key, slot] of this.slots)
            if (!ids.has(key)) {
                slot.control.parent?.remove(slot.control);
                this.remove(slot.overlay);
                slot.overlay.destroy();
                this.slots.delete(key);
            }
        this.items = items.map((item) => ({
            ...item,
            width: item.width ?? Math.max(72, Array.from(item.control.text || '').length * 8 + 48),
            height: item.height ?? 32,
        }));
        this.refresh();
        return this;
    }
    renderItems(view) {
        const layout = wrapLayout(this.items, { width: view.width, gap: this.gap });
        for (const [i, item] of this.items.entries()) {
            const b = layout.bounds[i];
            view.place(item.id, item.control, [b.x, b.y, b.width, b.height]);
        }
        this.contentHeight = layout.height;
        this.setProperty('text', `panel:fixed-height:${Math.max(1, this.contentHeight)}:flush`);
    }
}
