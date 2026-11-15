import { Column, Label, TextField, CheckBox } from './controls.mjs';
import { State } from './control.mjs';
import { FormModel, FormField } from './forms.mjs';
import { NumberInput } from './value-editors.mjs';
import { Message } from './messages.mjs';

const axes = ['x', 'y', 'width', 'height', 'rotation'];
const equal = (a, b) => axes.every(key => Object.is(a[key], b[key]));
function geometry(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !axes.includes(key))) throw TypeError('Geometry needs x, y, width, height and optional rotation');
    const result = { ...value, rotation: value.rotation ?? 0 };
    if (!axes.every(key => Number.isFinite(result[key])) || result.width <= 0 || result.height <= 0) throw RangeError('Geometry needs finite coordinates and positive dimensions');
    return result;
}

/** Native geometry drafts only. The caller owns units, selection, history and persistence. */
export class GeometryEditor extends Column {
    static family = 'composite';
    static constructorOptions = Object.freeze(['value', 'unit', 'precision', 'minimumSize', 'maximumSize', 'positionLimit', 'allowResize', 'allowRotation', 'allowScale', 'allowAspectLock', 'lockAspect', 'originLabel', 'onCommit']);
    static properties = Object.freeze({
        value: { default: null, reflect: false, validate: value => value === null ? null : geometry(value) },
        readOnly: { default: false, reflect: false, validate: Boolean },
    });
    static authoring = Object.freeze({
        constructor: 'new GeometryEditor({value:{x:0,y:0,width:4,height:2,rotation:0},unit:"in",precision:2,minimumSize:0.0001,maximumSize:1000000,positionLimit:1000000,allowResize:true,allowRotation:true,allowScale:true,allowAspectLock:true,lockAspect:false,originLabel:"From the top-left corner",onCommit:async({value,previous,changed,lockAspect})=>{...}})',
        methods: 'commit(), cancel(), setValue(value), setFormValue(value,{preserveDraft:true}), setDraft(field,text), draft, value, busy; fields exposes named FormFields and lockControl exposes the real checkbox. Scale percentages use dimensions at setValue, initially 100. Width/height and scales update together; locking uses that opening aspect ratio. Unedited axes retain exact numbers despite display rounding. An explicit commit submits one caller-owned transaction; false/rejection retains drafts and errors. Cancel restores accepted geometry. Authoritative replacement invalidates pending validation/results. Use PropertyInspector for mixed multi-selection.',
    });
    constructor({ value, unit = '', precision = 2, minimumSize = .0001, maximumSize = 1e6, positionLimit = 1e6,
        allowResize = true, allowRotation = true, allowScale = true, allowAspectLock = true, lockAspect = false,
        originLabel = 'From the top-left corner', onCommit = null, ...options } = {}) {
        const initial = geometry(value);
        if (typeof unit !== 'string' || unit.length > 16 || !Number.isInteger(precision) || precision < 0 || precision > 8 ||
            !Number.isFinite(minimumSize) || minimumSize <= 0 || !Number.isFinite(maximumSize) || maximumSize < minimumSize ||
            !Number.isFinite(positionLimit) || positionLimit <= 0 || [allowResize, allowRotation, allowScale, allowAspectLock, lockAspect].some(flag => typeof flag !== 'boolean') ||
            typeof originLabel !== 'string' || originLabel.length > 256 || onCommit !== null && typeof onCommit !== 'function') throw TypeError('Invalid geometry editor');
        const format = number => String(Number(number.toFixed(precision)));
        const limits = id => id === 'width' || id === 'height' ? { minimum: minimumSize, maximum: maximumSize }
            : id.startsWith('scale') ? { minimum: .000001, maximum: 1e6 }
            : id === 'rotation' ? { minimum: -36000, maximum: 36000 } : { minimum: -positionLimit, maximum: positionLimit };
        for (const id of axes) NumberInput.parse(initial[id], limits(id));
        const suffix = unit ? ` (${unit})` : '';
        const specs = [
            ...(allowResize ? [['height', `Height${suffix}:`], ['width', `Width${suffix}:`]] : []),
            ...(allowRotation ? [['rotation', 'Rotation (°):']] : []),
            ...(allowResize && allowScale ? [['scaleHeight', 'Scale height (%):'], ['scaleWidth', 'Scale width (%):']] : []),
            ['x', `Horizontal${suffix}:`], ['y', `Vertical${suffix}:`],
        ];
        const values = { ...Object.fromEntries(axes.map(id => [id, format(initial[id])])), scaleWidth: '100', scaleHeight: '100', lockAspect };
        const model = new FormModel({ fields: [
            ...specs.map(([id, label]) => ({ id, label, initial: values[id], validate: text => { try { NumberInput.parse(text === this.seeds[id] ? this.exact[id] : text, limits(id)); return null; } catch (error) { return error.message; } } })),
            { id: 'lockAspect', initial: lockAspect, validate: flag => typeof flag === 'boolean' ? null : 'Choose an aspect ratio setting' },
        ] });
        const fields = new Map(specs.map(([id]) => [id, new FormField({ model, field: id,
            control: new TextField({ name: `geometry-${id}`, state: State.EXPANSIVE }), layout: 'inline', labelWidth: 148, controlWidth: 112 })]));
        const lock = new CheckBox({ name: 'geometry-lock-aspect', text: 'Lock aspect ratio', checked: lockAspect, visible: allowResize && allowAspectLock });
        const status = new Label({ name: 'geometry-status', text: '', visible: false });
        const size = [...fields].filter(([id]) => !['x', 'y'].includes(id)).map(([, field]) => field);
        super({ text: 'panel:fixed:flush:gap=6', ...options }, [
            ...(size.length ? [new Label({ text: 'Size' }), ...size] : []), lock,
            new Label({ text: 'Position' }), ...(originLabel ? [new Label({ text: originLabel })] : []), fields.get('x'), fields.get('y'), status,
        ]);
        Object.assign(this, { model, fields, lockControl: lock, status, format, limits, onCommit, precision, unit,
            allowResize, allowRotation, allowScale, allowAspectLock, accepted: initial, opening: initial,
            exact: { ...initial, scaleWidth: 100, scaleHeight: 100 }, seeds: { ...values }, acceptedLock: lockAspect, epoch: 0, linking: false });
        super.setProperty('value', initial, { notify: false });
        let revision = model.revision;
        this.stopGeometry = model.subscribe(snapshot => {
            const changed = snapshot.revision !== revision; revision = snapshot.revision;
            if (changed && snapshot.lastChangedField !== null && !this.linking) this.reconcile(snapshot.lastChangedField);
            this.refresh();
        });
        lock.on(Message.ACTIVATE, () => { if (this.available() && !this.busy) model.set('lockAspect', lock.getProperty('checked')); });
        this.refresh();
    }
    get value() { return { ...this.accepted }; }
    get busy() { return this.model?.snapshot.busy ?? false; }
    get draft() { return structuredClone(this.model.values); }
    available() {
        if (this.destroyed || this.getProperty('readOnly')) return false;
        for (let owner = this; owner; owner = owner.parent) if (!owner.visible || !(owner.state & State.ENABLED) || owner.state & State.BUSY) return false;
        return true;
    }
    setProperty(name, value, options) {
        if (name === 'value' && this.model) {
            const next = geometry(value);
            if (equal(this.accepted, next)) return false;
            this.setValue(next); return true;
        }
        const changed = super.setProperty(name, value, options);
        if (this.model && ['readOnly', 'visible', 'enabled', 'state', 'busy'].includes(name)) this.refresh();
        return changed;
    }
    publish(id, number) {
        const shown = this.format(number); this.exact[id] = number; this.seeds[id] = shown;
        this.model.set(id, shown);
    }
    number(id) {
        const text = this.model.get(id);
        return text === this.seeds[id] ? this.exact[id] : NumberInput.parse(text, this.limits(id));
    }
    reconcile(id) {
        this.linking = true;
        try {
            const lock = this.allowAspectLock && this.model.get('lockAspect'), ratio = this.opening.width / this.opening.height;
            if (id === 'lockAspect') {
                if (lock && this.allowResize) {
                    const width = this.number('width'); this.publish('height', width / ratio);
                    if (this.allowScale) { this.publish('scaleWidth', width / this.opening.width * 100); this.publish('scaleHeight', width / ratio / this.opening.height * 100); }
                }
            } else if (['width', 'height', 'scaleWidth', 'scaleHeight'].includes(id)) {
                const parsed = this.number(id), width = id === 'width' || id === 'scaleWidth';
                const axis = width ? 'width' : 'height', peer = width ? 'height' : 'width';
                const number = id.startsWith('scale') ? this.opening[axis] * parsed / 100 : parsed;
                if (id.startsWith('scale')) this.publish(axis, number);
                else if (this.allowScale) this.publish(width ? 'scaleWidth' : 'scaleHeight', number / this.opening[axis] * 100);
                if (lock) {
                    const other = width ? number / ratio : number * ratio; this.publish(peer, other);
                    if (this.allowScale) this.publish(width ? 'scaleHeight' : 'scaleWidth', other / this.opening[peer] * 100);
                }
            }
        } catch { /* Keep incomplete/invalid drafts; commit owns validation. */ }
        finally { this.linking = false; }
    }
    refresh() {
        if (!this.model || this.linking) return;
        const enabled = this.available() && !this.busy;
        for (const field of this.fields.values()) {
            field.input.enabled = enabled;
            field.input.setProperty('readOnly', this.getProperty('readOnly'));
        }
        this.lockControl.enabled = enabled; this.lockControl.setProperty('checked', this.model.get('lockAspect'));
        this.emit(Message.CHANGE, { busy: this.busy, values: this.draft });
    }
    setDraft(id, text) {
        if (!this.fields.has(id)) throw TypeError('Unknown geometry field');
        if (!this.available() || this.busy) return false;
        this.model.set(id, String(text)); return true;
    }
    setValue(value) {
        const next = geometry(value); for (const id of axes) NumberInput.parse(next[id], this.limits(id));
        this.epoch++; this.linking = true; this.accepted = next; this.opening = next; this.acceptedLock = false;
        this.exact = { ...next, scaleWidth: 100, scaleHeight: 100 }; this.seeds = { ...Object.fromEntries(axes.map(id => [id, this.format(next[id])])), scaleWidth: '100', scaleHeight: '100', lockAspect: false };
        this.model.reset(this.seeds); this.linking = false;
        super.setProperty('value', next, { notify: false }); this.status.visible = false; this.refresh();
    }
    setFormValue(value, { preserveDraft = true } = {}) { if (!preserveDraft || !equal(geometry(value), this.accepted)) this.setValue(value); }
    cancel() {
        this.epoch++; this.linking = true;
        const value = this.accepted; this.exact = { ...value };
        this.seeds = { ...Object.fromEntries(axes.map(id => [id, this.format(value[id])])), scaleWidth: this.format(value.width / this.opening.width * 100), scaleHeight: this.format(value.height / this.opening.height * 100), lockAspect: this.acceptedLock };
        this.exact.scaleWidth = value.width / this.opening.width * 100; this.exact.scaleHeight = value.height / this.opening.height * 100;
        this.model.reset(this.seeds); this.linking = false; this.status.visible = false; this.refresh();
    }
    async commit() {
        if (!this.available() || this.busy) return false;
        const epoch = this.epoch;
        try {
            return await this.model.submit(async () => {
                if (epoch !== this.epoch || !this.available()) return false;
                const value = Object.fromEntries(axes.map(id => [id, this.fields.has(id) ? this.number(id) : this.accepted[id]]));
                const previous = this.value, changed = axes.filter(id => !Object.is(value[id], previous[id]));
                if (changed.length && await this.onCommit?.({ value, previous, changed, lockAspect: this.model.get('lockAspect') }) === false) throw Error('Geometry was not applied');
                if (epoch !== this.epoch || this.destroyed) return false;
                this.accepted = value; this.acceptedLock = this.model.get('lockAspect');
                super.setProperty('value', value, { notify: false }); this.status.visible = false;
                if (changed.length) this.emit(Message.VALUE_CHANGED, { value: this.value, previous, changed });
                return true;
            });
        } catch (error) {
            if (!this.destroyed && epoch === this.epoch) {
                this.status.text = String(error.message); this.status.visible = true;
                const first = [...this.fields].find(([id]) => this.model.errors[id]); first?.[1].focus();
            }
            return false;
        }
    }
    destroy() { this.stopGeometry?.(); this.model?.destroy(); super.destroy(); }
}
