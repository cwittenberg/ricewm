import { Column, Row, Button, Label, TextField, ToolBar } from './controls.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { ObjectCanvas } from './object-canvas.mjs';
import { WrapPanel } from './wrap-panel.mjs';

/** Standard editor commands with native buttons; no document-domain logic. */
export class ObjectToolbar extends WrapPanel {
    static family = 'composite';
    static constructorOptions = Object.freeze(['canvas', 'services']);
    static authoring = Object.freeze({
        constructor: 'new ObjectToolbar({canvas,services})',
        behavior:
            'Native Select/shape/Connect tools, undo/redo, cut/copy/paste, delete, fit and tree/grid layout. Uses the same ObjectCanvas commands as pointer and keyboard input. Clipboard services require their exact grants.',
    });
    constructor({ canvas, services, ...options } = {}) {
        if (!(canvas instanceof ObjectCanvas)) throw TypeError('ObjectToolbar needs an ObjectCanvas');
        const actions = [
            ['Select', () => canvas.setTool('select')],
            ['Rectangle', () => canvas.setTool('rect')],
            ['Ellipse', () => canvas.setTool('ellipse')],
            ['Pill', () => canvas.setTool('pill')],
            ['Diamond', () => canvas.setTool('diamond')],
            ['Connect', () => canvas.setTool('connect')],
            ['Undo', () => canvas.undo()],
            ['Redo', () => canvas.redo()],
            ['Cut', () => canvas.cut(services)],
            ['Copy', () => canvas.copy(services)],
            ['Paste', () => canvas.paste(services)],
            ['Delete', () => canvas.deleteSelection()],
            ['Fit', () => canvas.fit()],
            ['Tree', () => canvas.setLayout({ type: 'tree' })],
            ['Grid', () => canvas.setLayout({ type: 'grid' })],
        ];
        const items = actions.map(([text, run]) => {
            const button = new Button({ text, name: 'object-' + text.toLowerCase(), accessibleName: text });
            button.on(Message.ACTIVATE, run);
            return { id: 'command-' + text.toLowerCase(), control: button };
        });
        super({ ...options, items, gap: 4 });
        this.canvas = canvas;
    }
}
export class ObjectInspector extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['canvas']);
    static authoring = Object.freeze({
        constructor: 'new ObjectInspector({canvas})',
        behavior:
            'Native selected-object title, position, size, rotation, fill and outline editors. Apply validates all fields atomically; Cancel restores fields. Focused drafts survive selection-independent repaint. Each application uses the same shared object properties.',
    });
    constructor({ canvas, ...options } = {}) {
        if (!(canvas instanceof ObjectCanvas)) throw TypeError('ObjectInspector needs an ObjectCanvas');
        super({ ...options, text: 'panel:fixed:gap=8:padding=12' });
        this.canvas = canvas;
        this.fields = new Map();
        this.selected = null;
        this.heading = new Label({ text: 'Object', state: State.EMPHASIZED });
        this.append(this.heading);
        for (const [key, title] of [
            ['text', 'Text'],
            ['x', 'X'],
            ['y', 'Y'],
            ['width', 'Width'],
            ['height', 'Height'],
            ['rotation', 'Rotation'],
            ['fill', 'Fill #RRGGBB'],
            ['stroke', 'Outline #RRGGBB'],
        ]) {
            const field = new TextField({ name: 'object-property-' + key, accessibleName: title });
            this.fields.set(key, field);
            this.append(new Row({}, [new Label({ text: title }), field]));
        }
        const apply = new Button({ text: 'Apply', name: 'object-property-apply' }),
            cancel = new Button({ text: 'Cancel', name: 'object-property-cancel' });
        apply.on(Message.ACTIVATE, () => this.apply());
        cancel.on(Message.ACTIVATE, () => this.refresh(true));
        this.append(new Row({}, [apply, cancel]));
        this.stopSelection = canvas.onSelectionChanged(() => this.refresh(true));
        this.stopHistory = canvas.history.subscribe(() => this.refresh());
        this.refresh(true);
    }
    refresh(force = false) {
        const id = this.canvas.selection.length === 1 ? this.canvas.selection[0] : null,
            object = this.canvas.byId.get(id);
        this.heading.text = object ? 'Object ' + object.id : 'Select one object';
        const changed = id !== this.selected;
        this.selected = id;
        for (const [key, field] of this.fields) {
            field.setProperty('enabled', Boolean(object && !object.locked && !this.canvas.readOnly));
            if (!force && !changed && field.getProperty('focused')) continue;
            field.value = object
                ? String(
                      key === 'fill' || key === 'stroke'
                          ? object.style?.[key] || ''
                          : (object[key] ?? (key === 'rotation' ? 0 : '')),
                  )
                : '';
        }
    }
    apply() {
        if (!this.selected || this.canvas.readOnly) return false;
        const patch = {};
        for (const [key, field] of this.fields) {
            if (['x', 'y', 'width', 'height', 'rotation'].includes(key)) {
                if (!field.value.trim()) throw TypeError('A geometry field is empty');
                patch[key] = Number(field.value);
                if (!Number.isFinite(patch[key])) throw TypeError('Geometry must be numeric');
            } else if (key === 'text') patch.text = field.value;
        }
        const id = this.selected;
        return this.canvas.history.transact('Edit object properties', (d) => {
            const object = d.objects.find((o) => o.id === id);
            if (!object || object.locked) throw Error('Object changed');
            Object.assign(object, patch);
            object.style = { ...object.style };
            for (const key of ['fill', 'stroke']) {
                const value = this.fields.get(key).value.trim();
                if (value) object.style[key] = value;
                else delete object.style[key];
            }
        });
    }
    destroy() {
        this.stopSelection?.();
        this.stopHistory?.();
        super.destroy();
    }
}
