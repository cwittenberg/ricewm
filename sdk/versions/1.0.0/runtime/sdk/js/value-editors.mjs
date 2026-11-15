import { Row, Button, TextField, Menu, MenuItem, Label, ToolBar } from './controls.mjs';
import { ColorMenu } from './color-palette.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';

const numeric = value => value === null ? null : typeof value === 'number' && Number.isFinite(value) ? value : (() => { throw TypeError('Expected a finite number or a mixed value'); })();
const keyIs = (event, name, code) => event.detail?.pressed !== false && (event.detail?.key === name || event.detail?.keyCode === code);

/** Editable native number with a caller-owned commit transaction. Null means mixed. */
export class NumberInput extends Row {
    static family = 'composite';
    static constructorOptions = Object.freeze(['value', 'minimum', 'maximum', 'step', 'precision', 'unit', 'onCommit']);
    static properties = Object.freeze({ value: { default: null, reflect: false, validate: numeric }, readOnly: { default: false, reflect: false, validate: Boolean } });
    static authoring = Object.freeze({
        constructor: 'new NumberInput({value:12,minimum:0,maximum:100,step:0.5,precision:2,unit:"pt",onCommit:async(value)=>{...}})',
        methods: 'NumberInput.parse(value,{minimum,maximum}) validates a finite number or decimal/scientific text using the same rules as native typing. setValue(value,{preserveDraft:true}), commit(), cancel(), stepBy(direction); preserveDraft is for refreshing the same target. Default setValue resets drafts and invalidates a pending response for a replaced target, even when its value is equal. Enter commits, Escape restores, arrows step. Null displays Mixed. Drag the unit to adjust, with Shift for fine movement; releasing commits once. Cancel, blur, loss of availability or an authoritative value change aborts the drag and restores the prior draft. Busy, hidden, disabled and read-only fields reject mutations. A rejected current-target commit retains the draft and error. The caller owns selection, undo and unit conversion.',
    });
    constructor({ value = null, minimum = -1e6, maximum = 1e6, step = 1, precision = 2, unit = '', onCommit = null, ...options } = {}) {
        if (![minimum, maximum, step].every(Number.isFinite) || minimum > maximum || step <= 0 || !Number.isInteger(precision) || precision < 0 || precision > 8 || typeof unit !== 'string' || unit.length > 16 || onCommit !== null && typeof onCommit !== 'function') throw TypeError('Invalid numeric editor');
        numeric(value);
        if (value !== null && (value < minimum || value > maximum)) throw RangeError('Number exceeds editor bounds');
        super({ text: 'panel:flush:fixed:gap=2', ...options, value });
        Object.assign(this, { minimum, maximum, step, precision, unit, onCommit, dirty: false, pending: false, valueEpoch: 0, resetEpoch: 0 });
        const name = this.name || 'number';
        this.input = this.append(new TextField({ name: `${name}-input`, value: this.format(value), accessibleName: options.accessibleName || name, state: State.EXPANSIVE, readOnly: this.getProperty('readOnly') }));
        this.mixedLabel = this.append(new Label({ text: 'Mixed', visible: value === null }));
        this.scrubber = this.append(new Button({ name: `${name}-scrub`, text: unit || '↔', accessibleName: `Drag to adjust ${options.accessibleName || name}`, tooltip: 'Drag horizontally to adjust; Shift for fine adjustment' }));
        const finishScrub = event => {
            if (!this.scrub) return;
            event.preventDefault();
            if (!this.canEdit() || !this.scrub.moved) { this.abortScrub(); return; }
            this.scrub = null; return this.commit();
        };
        this.scrubber.on(Message.POINTER, event => {
            const d = event.detail || {};
            if (d.kind === 'cancel' || d.kind === 'leave') { if (this.abortScrub()) event.preventDefault(); return; }
            if (!this.canEdit()) { this.abortScrub(); return; }
            if (d.kind === 'down' && d.button === 1 && Number.isFinite(d.x)) {
                try { this.scrub = { x: d.x, value: this.input.value.trim() ? this.read() : this.value ?? Math.max(this.minimum, Math.min(this.maximum, 0)), draft: this.input.value,
                    dirty: this.dirty, error: this.error, invalid: this.input.getProperty('invalid'), tooltip: this.input.getProperty('tooltip'), mixed: this.mixedLabel.visible, moved: false }; } catch { return; }
                event.preventDefault();
            } else if (d.kind === 'move' && this.scrub && Number.isFinite(d.x)) {
                const amount = (d.x - this.scrub.x) * this.step / 4 * (d.modifiers & 1 ? .1 : 1);
                this.scrub.moved ||= d.x !== this.scrub.x;
                this.scrub.x = d.x; this.scrub.value = Math.max(this.minimum, Math.min(this.maximum, this.scrub.value + amount));
                this.input.value = this.format(this.scrub.value); this.dirty = this.scrub.dirty || this.input.value !== this.scrub.draft;
                this.mixedLabel.visible = false; this.emit(Message.CHANGE, { value: this.input.value, draft: true }); event.preventDefault();
            } else if (d.kind === 'up' && d.button === 1 && this.scrub) {
                return finishScrub(event);
            }
        });
        // Native buttons turn their matching primary release into activation.
        // Other hosts can retain a pointer-up; both complete the owned gesture.
        this.scrubber.on(Message.ACTIVATE, finishScrub);
        this.scrubber.on(Message.BLUR, () => this.abortScrub());
        this.down = this.append(new Button({ name: `${name}-decrease`, text: '−', accessibleName: `Decrease ${options.accessibleName || name}` }));
        this.up = this.append(new Button({ name: `${name}-increase`, text: '+', accessibleName: `Increase ${options.accessibleName || name}` }));
        this.input.on(Message.VALUE_CHANGED, () => { this.dirty = true; this.input.setProperty('invalid', false); this.emit(Message.CHANGE, { value: this.input.value, draft: true }); });
        this.input.on(Message.ACTIVATE, () => this.commit());
        this.down.on(Message.ACTIVATE, () => this.stepBy(-1));
        this.up.on(Message.ACTIVATE, () => this.stepBy(1));
        this.setReadOnly();
        this.on(Message.KEY, event => {
            if (event.target === this.input && (keyIs(event, 'Enter', 36) || keyIs(event, 'Enter', 104))) { event.preventDefault(); event.stopPropagation(); return this.commit(); }
            if (keyIs(event, 'Escape', 9)) { event.preventDefault(); event.stopPropagation(); this.cancel(); this.emit(Message.CHANGE, { value: this.value, draft: true }); }
            else if (keyIs(event, 'ArrowUp', 111) || keyIs(event, 'ArrowDown', 116)) {
                event.preventDefault(); event.stopPropagation();
                return this.stepBy(keyIs(event, 'ArrowUp', 111) ? 1 : -1);
            }
        });
    }
    format(value) { return value === null ? '' : String(Number(value.toFixed(this.precision))); }
    get value() { return this.getProperty('value'); }
    set value(value) { this.setValue(value); }
    canEdit() { return Boolean(this.state & State.ENABLED) && !(this.state & State.BUSY) && this.visible && !this.pending && !this.destroyed && !this.getProperty('readOnly'); }
    abortScrub() {
        const scrub = this.scrub;
        if (!scrub) return false;
        this.scrub = null; this.input.value = scrub.draft; this.dirty = scrub.dirty; this.error = scrub.error;
        this.mixedLabel.visible = scrub.mixed; this.input.setProperty('invalid', scrub.invalid); this.input.setProperty('tooltip', scrub.tooltip);
        this.emit(Message.CHANGE, { value: this.input.value, draft: true, canceled: true }); return true;
    }
    setProperty(name, value, options) {
        if (name === 'value' && this.input && value !== null && (value < this.minimum || value > this.maximum)) throw RangeError('Number exceeds editor bounds');
        const changed = super.setProperty(name, value, options);
        if (name === 'value' && this.input && changed) this.valueEpoch++;
        if (name === 'readOnly' && this.input) this.setReadOnly();
        if (name === 'value' && this.input && changed && !this.pending) this.cancel();
        if (this.scrub && ['state', 'enabled', 'busy', 'visible', 'readOnly'].includes(name) && !this.canEdit()) this.abortScrub();
        return changed;
    }
    setValue(value, { preserveDraft = false } = {}) {
        numeric(value);
        if (value !== null && (value < this.minimum || value > this.maximum)) throw RangeError('Number exceeds editor bounds');
        if (this.scrub && (!preserveDraft || !Object.is(this.value, value))) this.abortScrub();
        if (this.pending && !preserveDraft) this.resetEpoch++;
        if (!Object.is(this.value, value) || this.pending && !preserveDraft) this.valueEpoch++;
        super.setProperty('value', value);
        if (this.mixedLabel && !this.scrub) this.mixedLabel.visible = value === null;
        if (this.input && !(preserveDraft && (this.dirty || this.pending || this.input.getProperty('focused')))) this.cancel();
    }
    setFormValue(value, { preserveDraft = true } = {}) {
        if (value === null || typeof value === 'number' && Number.isFinite(value) && value >= this.minimum && value <= this.maximum) this.setValue(value, { preserveDraft: preserveDraft && this.dirty });
        else if (String(value) !== this.input.value) { this.input.value = String(value); this.dirty = true; }
    }
    setReadOnly() { const readOnly = this.getProperty('readOnly'); this.input.setProperty('readOnly', readOnly); for (const control of [this.down, this.up, this.scrubber]) control.enabled = !readOnly; }
    cancel() { this.scrub = null; this.dirty = false; this.error = null; this.input.value = this.format(this.value); this.mixedLabel.visible = this.value === null; this.input.setProperty('invalid', false); this.input.setProperty('tooltip', null); }
    static parse(value, { minimum = -1e6, maximum = 1e6 } = {}) {
        if (![minimum, maximum].every(Number.isFinite) || minimum > maximum) throw TypeError('Invalid numeric bounds');
        if (typeof value === 'string') {
            const text = value.trim();
            if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/u.test(text)) throw TypeError('Enter a number');
            value = Number(text);
        }
        if (typeof value !== 'number' || !Number.isFinite(value) || value < minimum || value > maximum) throw RangeError(`Enter a number between ${minimum} and ${maximum}`);
        return value;
    }
    read() { return NumberInput.parse(this.input.value, this); }
    async commit() {
        if (!this.canEdit() || this.scrub) return false;
        const epoch = this.valueEpoch, reset = this.resetEpoch;
        try {
            const value = this.read();
            if (value === this.value) { this.cancel(); return true; }
            this.pending = true;
            const revision = this.input.value;
            const result = await this.onCommit?.(value);
            if (result === false) throw Error('Change was not applied');
            if (this.destroyed || reset !== this.resetEpoch || epoch !== this.valueEpoch && value !== this.value) return false;
            super.setProperty('value', value);
            if (this.input.value === revision) this.cancel();
            this.emit(Message.VALUE_CHANGED, { value });
            return true;
        } catch (error) {
            if (!this.destroyed && epoch === this.valueEpoch) { this.error = String(error.message); this.input.setProperty('invalid', true); this.input.setProperty('tooltip', this.error); }
            return false;
        } finally { this.pending = false; }
    }
    stepBy(direction) {
        if (!this.canEdit() || this.scrub) return false;
        let value;
        try { value = this.input.value.trim() ? this.read() : this.value ?? Math.max(this.minimum, Math.min(this.maximum, 0)); } catch { return this.commit(); }
        this.input.value = this.format(Math.max(this.minimum, Math.min(this.maximum, value + Math.sign(direction) * this.step)));
        this.dirty = true;
        return this.commit();
    }
}

/** Native editable choice with a real dropdown; suggestions never restrict input. */
export class EditableChoice extends Row {
    static family = 'composite';
    static constructorOptions = Object.freeze(['value', 'choices', 'editable', 'onCommit', 'validate', 'parse']);
    static properties = Object.freeze({ editable: { default: true, reflect: false, validate: Boolean }, readOnly: { default: false, reflect: false, validate: Boolean }, value: { default: '', reflect: false, validate: value => {
        if (typeof value !== 'string' || value.length > 256 || /[\u0000-\u001f]/u.test(value)) throw TypeError('Invalid editable choice');
        return value;
    } } });
    static authoring = Object.freeze({ constructor: 'new EditableChoice({value:"Liberation Sans",choices:["Liberation Sans","Manrope"],editable:true,parse?:text=>normalizedString,onCommit:async(value)=>{...}})', methods: 'setValue(value,{preserveDraft:true}), commit(), cancel(); preserveDraft refreshes the same target; default setValue resets a replacement target and suppresses stale pending success or errors. Dropdown suggestions and typed values use the same validation and commit callback. Optional synchronous parse(text) returns a normalized string before validation. editable:false makes the field selection-only; readOnly prevents all changes. Drafts emit CHANGE; only accepted commits emit VALUE_CHANGED. setFormValue(value,{preserveDraft:true}) retains a form draft without accepting it as the committed value. Escape restores an active draft; an additional Escape can reach the enclosing dialog. Empty value can represent mixed selection.' });
    constructor({ value = '', choices = [], editable = true, onCommit = null, validate = null, parse = null, ...options } = {}) {
        if (!Array.isArray(choices) || choices.length > 256 || choices.some(v => typeof v !== 'string' || !v || v.length > 256) || onCommit !== null && typeof onCommit !== 'function' || validate !== null && typeof validate !== 'function' || parse !== null && typeof parse !== 'function') throw TypeError('Invalid choice editor');
        super({ text: 'panel:flush:fixed:gap=0', ...options, value, editable });
        Object.assign(this, { onCommit, validate, parse, dirty: false, pending: false, valueEpoch: 0, resetEpoch: 0 });
        this.input = this.append(new TextField({ name: `${this.name || 'choice'}-input`, value, accessibleName: options.accessibleName || 'Value', state: State.EXPANSIVE, readOnly: this.getProperty('readOnly') }));
        this.menu = new Menu({ name: `${this.name || 'choice'}-choices`, text: '', icon: 74, accessibleName: `${options.accessibleName || 'Value'} suggestions` });
        // A toolbar supplies a compact native disclosure slot. An ordinary
        // Menu's standalone minimum otherwise consumes almost the entire
        // width of a narrow value editor, leaving no room for its text.
        this.append(new ToolBar({ text: 'panel:flush:fixed:gap=0:width=28:accessory-height=28' }, [this.menu]));
        for (const text of [...new Set(choices)]) this.menu.append(new MenuItem({ text })).on(Message.ACTIVATE, event => {
            event.stopPropagation();
            if (!this.canCommit()) return false;
            this.input.value = text; this.dirty = true; return this.commit();
        });
        this.input.on(Message.VALUE_CHANGED, event => {
            event.stopPropagation();
            if (!this.canCommit() || !this.getProperty('editable')) return;
            this.dirty = true; this.input.setProperty('invalid', false); this.emit(Message.CHANGE, { value: this.input.value, draft: true });
        });
        this.input.on(Message.ACTIVATE, event => { event.stopPropagation(); return this.commit(); });
        this.syncAvailability();
        this.on(Message.KEY, event => {
            if (event.target === this.input && (keyIs(event, 'Enter', 36) || keyIs(event, 'Enter', 104))) { event.preventDefault(); event.stopPropagation(); return this.commit(); }
            if (keyIs(event, 'Escape', 9) && (this.dirty || this.error)) { event.preventDefault(); event.stopPropagation(); this.cancel(); this.emit(Message.CHANGE, { value: this.value, draft: true }); }
        });
    }
    get value() { return this.getProperty('value'); }
    set value(value) { this.setValue(value); }
    setProperty(name, value, options) {
        const changed = super.setProperty(name, value, options);
        if (name === 'value' && this.input && changed) this.valueEpoch++;
        if (['readOnly', 'editable', 'enabled', 'busy', 'state'].includes(name) && this.input) this.syncAvailability();
        if (name === 'value' && this.input && changed && !this.pending) this.cancel();
        return changed;
    }
    setValue(value, { preserveDraft = false } = {}) { if (this.pending && !preserveDraft) this.resetEpoch++; if (!Object.is(this.value, value) || this.pending && !preserveDraft) this.valueEpoch++; super.setProperty('value', value); if (!(preserveDraft && (this.dirty || this.pending || this.input.getProperty('focused')))) this.cancel(); }
    setFormValue(value, { preserveDraft = true } = {}) { if (preserveDraft && this.dirty) return; this.setValue(value, { preserveDraft }); }
    canCommit() { return Boolean(this.state & State.ENABLED) && !(this.state & State.BUSY) && !this.pending && !this.getProperty('readOnly'); }
    syncAvailability() {
        const usable = Boolean(this.state & State.ENABLED) && !(this.state & State.BUSY) && !this.pending;
        this.input.enabled = usable;
        this.input.setProperty('readOnly', this.getProperty('readOnly') || !this.getProperty('editable'));
        this.menu.enabled = usable && !this.getProperty('readOnly');
    }
    cancel() { this.dirty = false; this.error = null; this.input.value = this.value; this.input.setProperty('invalid', false); this.input.setProperty('tooltip', null); }
    async commit() {
        if (!this.canCommit()) return false;
        const epoch = this.valueEpoch, reset = this.resetEpoch;
        try {
            const draft = this.input.value, value = this.parse ? this.parse(draft.trim()) : draft.trim();
            this.propertyDescriptors.value.validate(value);
            if (!value) throw TypeError('Enter a value');
            const error = this.validate?.(value); if (error) throw Error(error);
            if (value === this.value) { this.cancel(); return true; }
            this.pending = true; this.syncAvailability();
            const result = await this.onCommit?.(value);
            if (result === false) throw Error('Change was not applied');
            if (this.destroyed || reset !== this.resetEpoch || epoch !== this.valueEpoch && value !== this.value) return false;
            super.setProperty('value', value);
            if (this.input.value === draft) this.cancel();
            this.emit(Message.VALUE_CHANGED, { value });
            return true;
        } catch (error) {
            if (!this.destroyed && epoch === this.valueEpoch) { this.error = String(error.message); this.input.setProperty('invalid', true); this.input.setProperty('tooltip', this.error); }
            return false;
        } finally { this.pending = false; if (!this.destroyed) this.syncAvailability(); }
    }
}

/** Native palette and explicit mixed state with the ordinary form value contract. */
export class ColorInput extends Row {
    static family = 'composite';
    static constructorOptions = Object.freeze(['value', 'allowNone', 'themeColors', 'standardColors', 'columns', 'selectionKey']);
    static properties = Object.freeze({ readOnly: { default: false, reflect: false, validate: Boolean }, value: { default: null, reflect: false, validate: value => {
        if (value !== null && (typeof value !== 'string' || !/^#[0-9a-f]{6}$/iu.test(value))) throw TypeError('Expected #RRGGBB or no color');
        return value?.toUpperCase() ?? null;
    } } });
    static authoring = Object.freeze({ constructor: 'new ColorInput({value:"#306BDD",allowNone:true})', methods: 'setValue(value,{mixed:false,preserveDraft:true}); preserveDraft retains custom-color fields only for the same caller-owned target. A default setValue resets and closes the custom editor. Native palette emits VALUE_CHANGED only for committed colors, forwarding optional choice metadata. themeColors, standardColors and columns configure the shared palette; setValue accepts selectionKey to distinguish equal RGB identities. Null can mean no fill; mixed remains a separate state.' });
    constructor({ value = null, allowNone = true, themeColors=[], standardColors, columns=8, selectionKey=null, ...options } = {}) {
        super({ text: 'panel:flush:fixed', ...options, value });
        this.palette = this.append(new ColorMenu({ name: `${this.name || 'color'}-palette`, label: options.accessibleName || 'Color', value: value ?? '#000000', allowNone, themeColors, standardColors, columns, selectionKey }));
        this.caption = this.append(new Label({ text: value || 'No color', state: State.EXPANSIVE }));
        this.palette.enabled = !this.getProperty('readOnly');
        this.palette.on(Message.VALUE_CHANGED, event => {
            event.stopPropagation();
            if (!this.enabled || this.getProperty('readOnly')) return;
            this.setValue(event.detail.value,{selectionKey:event.detail.choice?.key??null});
            return this.emit(Message.VALUE_CHANGED, structuredClone(event.detail));
        });
    }
    get value() { return this.getProperty('value'); }
    set value(value) { this.setValue(value); }
    setProperty(name, value, options) {
        const changed = super.setProperty(name, value, options);
        if (name === 'readOnly' && this.palette) this.palette.enabled = !value;
        if (name === 'value' && this.palette) this.sync();
        return changed;
    }
    sync({ preserveDraft = false } = {}) {
        if (!preserveDraft) this.palette.palette.closeEditor();
        this.palette.setValue(this.value, { mixed: this.mixed, preserveDraft, selectionKey:this.selectionKey??null }); this.caption.text = this.mixed ? 'Mixed' : this.value || 'No color';
    }
    setValue(value, { mixed = false, preserveDraft = false, selectionKey=null } = {}) { this.selectionKey=selectionKey; this.mixed = Boolean(mixed); super.setProperty('value', value); this.sync({ preserveDraft }); }
}
