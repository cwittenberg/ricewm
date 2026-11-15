import { Button, Label, Column, Row, TextField } from './controls.mjs';
import { Workspace } from './workspace.mjs';
import { Message, ControlMessage } from './messages.mjs';
import { State } from './control.mjs';

const color = (value) => {
    if (value !== null && (typeof value !== 'string' || !/^#[0-9a-f]{6}$/i.test(value)))
        throw TypeError('Chip color must be null or #RRGGBB');
    return value?.toLowerCase() ?? null;
};
const properties = Object.freeze({
    removable: { default: false, validate: (value) => Boolean(value) },
    color: { default: null, validate: color },
    foreground: { default: null, validate: color },
    radius: {
        default: 65535,
        validate: (value) => {
            if (!Number.isInteger(value) || value < 0 || value > 65535)
                throw RangeError('Invalid chip radius');
            return value;
        },
    },
});
function widget(control, prototype) {
    const base = prototype.toWidget.call(control);
    return Object.freeze({
        ...base,
        text: `visual-chip:${control.getProperty('radius')}:${control.getProperty('color')?.slice(1) || '-'}:${control.getProperty('foreground')?.slice(1) || '-'}:${control.getProperty('removable') ? 1 : 0}:${control.text || ''}`,
    });
}
/** Native semantic action with independently colored rounded geometry. */
export class Chip extends Button {
    static family = 'composite';
    static properties = properties;
    static authoring = Object.freeze({
        constructor:
            'new Chip({text,color:null|"#RRGGBB",foreground:null|"#RRGGBB",selected:false,enabled:true,icon,accessibleName})',
        behavior:
            'Native rounded hit geometry, theme typography, hover/focus/pressed/disabled feedback and Enter/Space activation. Explicit colors represent data; null uses active theme. Listen to Message.ACTIVATE. Use ChipGroup for selection/removal and TokenInput for editing.',
    });
    toWidget() {
        return widget(this, Button.prototype);
    }
    windowProc(event) {
        if (event.type === Message.VALUE_CHANGED && event.detail?.value === 'remove') {
            this.emit(Message.COMMAND, { command: 'remove' });
            return true;
        }
        return super.windowProc(event);
    }
}
export class Badge extends Label {
    static properties = properties;
    toWidget() {
        return widget(this, Label.prototype);
    }
}

/** Wrapping selection/removal control, independent of a tag database. */
export class ChipGroup extends Workspace {
    static constructorOptions = Object.freeze([
        'items',
        'value',
        'multiple',
        'removable',
        'gap',
        'onChange',
        'onRemove',
    ]);
    static authoring = Object.freeze({
        constructor:
            'new ChipGroup({items:[{id,label,color?,disabled?,width?}],value:[],multiple:true,removable:false,onChange(ids),onRemove(id)})',
        methods:
            'setItems(items), setValue(ids); native Left/Right/Home/End focus, Enter/Space select, Delete/Backspace remove. Stable item ids preserve focus; wraps on native resize.',
    });
    constructor({
        items = [],
        value = [],
        multiple = true,
        removable = false,
        gap = 8,
        onChange,
        onRemove,
        ...options
    } = {}) {
        let group;
        super({ ...options, height: options.height ?? 96, render: (view) => group?.renderChips(view) });
        group = this;
        if (
            typeof multiple !== 'boolean' ||
            typeof removable !== 'boolean' ||
            !Number.isFinite(gap) ||
            gap < 0 ||
            gap > 64
        )
            throw TypeError('Invalid chip group options');
        this.multiple = multiple;
        this.removable = removable;
        this.gap = gap;
        this.onChange = onChange;
        this.onRemove = onRemove;
        this.buttons = new Map();
        this.value = [];
        this.setItems(items);
        this.setValue(value);
    }
    setItems(items) {
        if (
            !Array.isArray(items) ||
            items.length > 512 ||
            items.some(
                (item) =>
                    !item ||
                    typeof item.id !== 'string' ||
                    !/^[A-Za-z0-9_.:-]{1,100}$/.test(item.id) ||
                    typeof item.label !== 'string' ||
                    item.label.length > 256 ||
                    (item.color !== undefined && color(item.color) === undefined) ||
                    (item.width !== undefined &&
                        (!Number.isFinite(item.width) || item.width < 32 || item.width > 4096)),
            ) ||
            new Set(items.map((i) => i.id)).size !== items.length
        )
            throw TypeError('Invalid chip items');
        this.items = items.map((item) => ({ ...item }));
        const live = new Set(items.map((i) => i.id));
        for (const [key, button] of this.buttons)
            if (!live.has(key)) {
                const slot = this.slots.get('chip-' + key);
                if (slot) {
                    this.remove(slot.overlay);
                    slot.overlay.destroy();
                    this.slots.delete('chip-' + key);
                }
                this.buttons.delete(key);
            }
        this.value = this.value.filter((id) => live.has(id));
        this.refresh();
        return this;
    }
    setValue(ids) {
        if (
            !Array.isArray(ids) ||
            ids.some((id) => !this.items.some((i) => i.id === id)) ||
            new Set(ids).size !== ids.length ||
            (!this.multiple && ids.length > 1)
        )
            throw TypeError('Invalid chip selection');
        this.value = [...ids];
        this.refresh();
        return this;
    }
    changed() {
        this.onChange?.([...this.value]);
        this.sendMessage(
            new ControlMessage(Message.VALUE_CHANGED, { target: this, detail: { value: [...this.value] } }),
        );
        this.refresh();
    }
    select(id) {
        const item = this.items.find((i) => i.id === id);
        if (!item || item.disabled) return;
        this.value = this.multiple
            ? this.value.includes(id)
                ? this.value.filter((v) => v !== id)
                : [...this.value, id]
            : [id];
        this.changed();
    }
    removeItem(id) {
        if (!this.removable || this.items.find((i) => i.id === id)?.disabled) return;
        const index = this.items.findIndex((i) => i.id === id);
        this.onRemove?.(id);
        this.setItems(this.items.filter((i) => i.id !== id));
        this.changed();
        this.focusItem(Math.min(index, this.items.length - 1));
    }
    focusItem(index) {
        const item = this.items[index];
        if (item) this.buttons.get(item.id)?.setProperty('focused', true);
    }
    renderChips(view) {
        let x = 0,
            y = 0;
        for (const [index, item] of this.items.entries()) {
            const width = Math.min(
                    view.width,
                    Math.max(
                        32,
                        item.width ?? Array.from(item.label).length * 8 + 32 + (this.removable ? 20 : 0),
                    ),
                ),
                height = 32;
            if (x && x + width > view.width) {
                x = 0;
                y += height + this.gap;
            }
            let chip = this.buttons.get(item.id);
            if (!chip) {
                chip = new Chip({ name: 'chip-' + item.id, accessibleName: item.label });
                chip.on(Message.ACTIVATE, () => this.select(item.id));
                chip.on(Message.COMMAND, (event) => {
                    if (event.detail?.command === 'remove') this.removeItem(item.id);
                });
                chip.on(Message.KEY, (event) => {
                    const key = event.detail;
                    if (key?.pressed === false) return;
                    const items = this.items.filter((i) => !i.disabled),
                        i = items.findIndex((v) => v.id === item.id),
                        code = key.keyCode;
                    if ([113, 114, 110, 115].includes(code)) {
                        const next =
                            code === 110
                                ? 0
                                : code === 115
                                  ? items.length - 1
                                  : (i + (code === 113 ? -1 : 1) + items.length) % items.length;
                        this.buttons.get(items[next]?.id)?.setProperty('focused', true);
                        event.preventDefault();
                    } else if ([22, 119].includes(code) && this.removable) {
                        this.removeItem(item.id);
                        event.preventDefault();
                    }
                });
                this.buttons.set(item.id, chip);
            }
            chip.text = item.label + (this.removable ? '  ×' : '');
            chip.setProperty('removable', this.removable);
            chip.setProperty('color', item.color ?? null);
            chip.setProperty('selected', this.value.includes(item.id));
            chip.setProperty('enabled', !item.disabled);
            view.place('chip-' + item.id, chip, [x, y, width, height]);
            x += width + this.gap;
        }
        this.contentHeight = y + 32;
        this.setProperty('text',this.contentHeight<=64?`panel:fixed:flush:accessory-height=${this.contentHeight}`:`panel:fixed-height:${this.contentHeight}:flush`);
    }
}
export class TokenInput extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['items', 'onChange', 'onCreate']);
    static authoring = Object.freeze({
        constructor: 'new TokenInput({items:[{id,label,color?}],onChange(items),onCreate(text)})',
        behavior:
            'Native input, wrapping removable chips, Enter/Add creates a token; empty Backspace removes the last; Escape clears the draft. onCreate can synchronously return a token descriptor. No document or application assumptions.',
    });
    constructor({ items = [], onChange, onCreate, ...options } = {}) {
        super({ ...options, text: 'panel:fixed:gap=8' });
        this.items = items.map((i) => ({ ...i }));
        this.sequence = 0;
        this.onChange = onChange;
        this.onCreate = onCreate;
        this.group = new ChipGroup({
            items: this.items,
            removable: true,
            height: 96,
            onRemove: (id) => {
                this.items = this.items.filter((i) => i.id !== id);
                this.onChange?.(structuredClone(this.items));
            },
        });
        this.field = new TextField({ name: 'token-draft', accessibleName: 'New token' });
        this.addButton = new Button({ text: 'Add', name: 'token-add' });
        this.addButton.on(Message.ACTIVATE, () => this.commit());
        this.field.on(Message.KEY, (event) => {
            const key = event.detail;
            if (key?.pressed === false) return;
            if ([36, 104].includes(key.keyCode)) {
                this.commit();
                event.preventDefault();
            } else if (key.keyCode === 9) {
                this.field.value = '';
                event.preventDefault();
            } else if (key.keyCode === 22 && !this.field.value) {
                const last = this.items.at(-1);
                if (last) this.group.removeItem(last.id);
            }
        });
        this.append(this.group);
        this.append(new Row({}, [this.field, this.addButton]));
    }
    commit() {
        const text = this.field.value.trim();
        if (!text) return false;
        let item = this.onCreate?.(text);
        if (item && typeof item.then === 'function') throw TypeError('Token creation must be synchronous');
        if (item === false) return false;
        if (!item) {
            let id;
            do {
                id = 'token-' + ++this.sequence;
            } while (this.items.some((i) => i.id === id));
            item = { id, label: text };
        }
        const next = [...this.items, item];
        this.group.setItems(next);
        this.items = next;
        this.field.value = '';
        this.onChange?.(structuredClone(next));
        return true;
    }
}
