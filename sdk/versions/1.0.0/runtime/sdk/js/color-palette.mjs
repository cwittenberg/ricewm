import { Control, State } from './control.mjs';
import { Button, Column, Label, Menu, MenuItem, Row, ScrollView, TextField, Tooltip } from './controls.mjs';
import { Message } from './messages.mjs';

const STANDARD_COLORS = Object.freeze(['#000000', '#FFFFFF', '#808080', '#C0C0C0', '#C00000', '#FF0000', '#FFC000', '#FFFF00', '#92D050', '#00B050', '#00B0F0', '#0070C0', '#002060', '#7030A0', '#A52A2A', '#FF69B4']);
function color(value) {
    if (typeof value !== 'string' || !/^#[0-9a-f]{6}$/iu.test(value)) throw new TypeError('Color must be #RRGGBB');
    return value.toUpperCase();
}
function colors(values, maximum) {
    if (!Array.isArray(values) || values.length > maximum) throw new RangeError(`Palette section supports at most ${maximum} colors`);
    return values.map(item => {
        const result = { color: color(typeof item === 'string' ? item : item?.color), label: typeof item === 'string' ? item : item.label };
        if (result.label != null && (typeof result.label !== 'string' || result.label.length > 160)) throw new TypeError('Color labels must be short strings');
        return result;
    });
}
function hueOf(hex) {
    const [r, g, b] = [1, 3, 5].map(offset => Number.parseInt(hex.slice(offset, offset + 2), 16) / 255);
    const high = Math.max(r, g, b), low = Math.min(r, g, b), delta = high - low;
    if (!delta) return 0;
    const hue = high === r ? (g - b) / delta : high === g ? (b - r) / delta + 2 : (r - g) / delta + 4;
    return Math.round(((hue * 60) % 360 + 360) % 360);
}
/** Existing native ScrollBar HSV protocol, deliberately separate from numeric ScrollBar. */
class NativeColorAxis extends Control {
    static kind = 23;
    constructor(axis, hex) {
        super({ accessibleName: axis === 'hue' ? 'Hue' : 'Saturation and brightness' });
        this.axis = axis; this.setColor(hex, hueOf(hex));
        this.append(new Tooltip({ text: this.getProperty('accessibleName') }));
    }
    setColor(hex, hue) { this.setProperty('text', `color-${this.axis}:${hex}@${hue}`); }
    windowProc(current) {
        if (current.type === Message.VALUE_CHANGED) {
            if (!this.getProperty('enabled') || !this.getProperty('visible')) return false;
            const match = /^(#[0-9a-f]{6})@(\d{1,3})$/iu.exec(current.detail?.value ?? '');
            if (!match || Number(match[2]) > 360) return false;
            const value = color(match[1]), hue = Number(match[2]);
            this.setColor(value, hue);
            this.emit(Message.CHANGE, { color: value, hue });
            return true;
        }
        return super.windowProc(current);
    }
}

/** Native palette; only committed changes emit VALUE_CHANGED. */
export class ColorPalette extends Column {
    static constructorOptions = Object.freeze(["value", "themeColors", "standardColors", "recentColors", "mixed", "allowNone", "noneLabel", "moreColors", "onCommit"]);
    constructor({ value = '#000000', themeColors = [], standardColors = STANDARD_COLORS,
        recentColors = [], mixed = false, allowNone = false, noneLabel = 'No fill', moreColors = true, onCommit = null, ...properties } = {}) {
        if (typeof noneLabel !== 'string' || !noneLabel.trim() || noneLabel.length > 80) throw new TypeError('No-color label must contain 1–80 characters');
        if (onCommit !== null && typeof onCommit !== 'function') throw new TypeError('Palette onCommit must be a function');
        const theme = colors(themeColors, 40), standard = colors(standardColors, 40), recent = colors(recentColors, 10);
        if (value === null && !allowNone) throw new TypeError('No fill requires allowNone');
        const initial = value === null ? null : color(value);
        super({ text: 'panel:flush:fixed', ...properties });
        this.mixed = Boolean(mixed); this.allowNone = Boolean(allowNone); this.selectedColor = initial; this.draft = initial || '#000000';
        this.swatches = new Set(); this.sections = new Map(); this.recentColors = [...new Set(recent.map(item => item.color))];
        this.moreOpen = false; this.onColorCommit = onCommit;
        if (this.allowNone) {
            this.noFill = this.append(new MenuItem({ name: 'no-fill', text: noneLabel, checked: initial === null }));
            this.noFill.on(Message.ACTIVATE, () => this.commit(null));
        }
        this.makeSection('theme', 'Theme colors', theme);
        this.makeSection('standard', 'Standard colors', standard);
        this.makeSection('recent', 'Recent colors', this.recentColors.map(color => ({ color })));
        if (moreColors) this.makeEditor();
        this.updateSelection();
    }
    get value() { return this.selectedColor; }
    setValue(value, { mixed = false } = {}) {
        const normalized = value === null && this.allowNone ? null : color(value);
        this.selectedColor = normalized; this.mixed = Boolean(mixed);
        if (this.moreOpen) this.setDraft(normalized || '#000000');
        this.updateSelection();
    }
    setThemeColors(values) {
        const validated = colors(values, 40);
        this.fillSection(this.sections.get('theme'), validated); this.updateSelection();
    }
    makeSection(id, title, values) {
        const section = this.append(new Column({ name: `palette-${id}`, text: 'panel:flush:fixed', visible: values.length > 0 }, [new Label({ text: title, state: State.MUTED })]));
        this.sections.set(id, section); this.fillSection(section, values);
    }
    fillSection(section, values) {
        for (const row of section.children.slice(1)) {
            for (const swatch of row.children) this.swatches.delete(swatch);
            section.remove(row, { destroy: true });
        }
        for (let start = 0; start < values.length; start += 8) {
            const row = section.append(new Row({ text: 'panel:flush:fixed' }));
            for (const entry of values.slice(start, start + 8)) {
                const name = entry.label ? `${entry.label} (${entry.color})` : entry.color;
                const swatch = new MenuItem({ text: `color-swatch:${entry.color}`, accessibleName: name }, [new Tooltip({ text: name })]);
                swatch.paletteColor = entry.color;
                swatch.on(Message.ACTIVATE, () => this.commit(entry.color));
                this.swatches.add(swatch); row.append(swatch);
            }
        }
        section.setProperty('visible', values.length > 0);
    }
    updateSelection() {
        for (const swatch of this.swatches) swatch.setProperty('checked', !this.mixed && swatch.paletteColor === this.value);
        this.noFill?.setProperty('checked', !this.mixed && this.value === null);
    }
    makeEditor() {
        this.moreButton = this.append(new Button({ name: 'more-colors', text: 'More colors…' }));
        this.plane = new NativeColorAxis('plane', this.draft); this.hue = new NativeColorAxis('hue', this.draft);
        this.hex = new TextField({ name: 'color-hex', value: this.draft, accessibleName: 'Hex color' });
        this.applyButton = new MenuItem({ name: 'apply-color', text: 'Apply' });
        this.cancelButton = new MenuItem({ name: 'cancel-color', text: 'Cancel' });
        this.editor = this.append(new Column({ name: 'custom-color', text: 'panel:flush:fixed', visible: false }, [
            this.plane, this.hue, new Row({ text: 'panel:flush:fixed' }, [new Label({ text: 'Hex' }), this.hex]),
            new Row({ text: 'panel:flush:fixed' }, [this.applyButton, this.cancelButton]),
        ]));
        this.moreButton.on(Message.ACTIVATE, () => {
            if (!this.getProperty('enabled')) return;
            this.moreOpen = true; this.setDraft(this.value || '#000000'); this.editor.setProperty('visible', true);
        });
        for (const axis of [this.plane, this.hue]) axis.on(Message.CHANGE, event => {
            if (event.detail?.color && this.moreOpen) this.setDraft(event.detail.color, event.detail.hue);
        });
        this.hex.on(Message.VALUE_CHANGED, () => {
            if (!this.moreOpen) return;
            const text = this.hex.getProperty('value');
            const valid = /^#[0-9a-f]{6}$/iu.test(text);
            this.hex.setProperty('invalid', !valid); this.applyButton.setProperty('enabled', valid);
            if (valid) this.setDraft(color(text));
        });
        this.applyButton.on(Message.ACTIVATE, () => {
            if (this.moreOpen && this.applyButton.getProperty('enabled')) this.commit(this.draft);
        });
        this.cancelButton.on(Message.ACTIVATE, () => this.closeEditor());
    }
    setDraft(value, hue = hueOf(value)) {
        this.draft = value; this.plane.setColor(value, hue); this.hue.setColor(value, hue);
        this.hex.setProperty('value', value); this.hex.setProperty('invalid', false); this.applyButton.setProperty('enabled', true);
    }
    closeEditor() { this.moreOpen = false; this.editor?.setProperty('visible', false); }
    commit(value) {
        if (!this.getProperty('enabled') || !this.getProperty('visible')) return false;
        const normalized = value === null && this.allowNone ? null : color(value);
        this.closeEditor();
        if (normalized === this.value && !this.mixed) { this.onColorCommit?.(normalized); return false; }
        this.selectedColor = normalized; this.mixed = false;
        if (normalized !== null) {
            this.recentColors = [normalized, ...this.recentColors.filter(item => item !== normalized)].slice(0, 10);
            this.fillSection(this.sections.get('recent'), this.recentColors.map(color => ({ color })));
        }
        this.updateSelection(); this.onColorCommit?.(normalized); this.emit(Message.VALUE_CHANGED, { value: normalized }); return true;
    }
}


/** Icon menu with a native current-color underline and an independent reusable last choice.
 * Selection refreshes call setValue; only a palette commit changes lastColor. */
export class ColorMenu extends Menu {
    static constructorOptions = Object.freeze(["name", "label", "icon", "value", "allowNone", "noneLabel"]);
    constructor({ name = 'color-menu', label = 'Color', icon = null, value = '#000000', allowNone = false,
        noneLabel = 'No color', ...properties } = {}) {
        if (typeof label !== 'string' || !label.trim() || label.length > 80 || /[\r\n|]/u.test(label)) throw new TypeError('Color menu needs a short label');
        const initial = value === null && allowNone ? null : color(value);
        super({ name, icon, state: State.DISPLAY, tooltip: label, accessibleName: label, ...properties });
        this.label = label; this.noneLabel = noneLabel; this.lastColor = initial;
        this.currentColor = initial; this.mixed = false; this.hasSelection = false;
        this.lastAction = this.append(new MenuItem({ name: `${name}-last-color`, icon }));
        this.palette = new ColorPalette({ name: `${name}-palette`, value: initial, allowNone, noneLabel, moreColors: true,
            onCommit: chosen => {
                this.lastColor = chosen; this.currentColor = chosen; this.hasSelection = true; this.mixed = false; this.updateColorIndicator();
            } });
        this.scroll = this.append(new ScrollView({ name: `${name}-scroll`, text: 'viewport:360|0/0' }, [this.palette]));
        this.palette.moreButton.on(Message.ACTIVATE, () => { if (this.palette.editor.getProperty('visible')) this.scroll.scrollTo(this.palette.editor); });
        this.palette.on(Message.VALUE_CHANGED, event => this.emit(Message.VALUE_CHANGED, { value: event.detail.value }));
        this.lastAction.on(Message.ACTIVATE, () => {
            if (this.getProperty('enabled')) this.palette.commit(this.lastColor);
        });
        this.setProperty('enabled', this.getProperty('enabled'));
        this.updateColorIndicator();
    }
    setProperty(name, value, options) {
        const result = super.setProperty(name, value, options);
        if (name === 'enabled') {
            this.palette?.setProperty('enabled', Boolean(value)); this.lastAction?.setProperty('enabled', Boolean(value));
        }
        return result;
    }
    setValue(value, { mixed = false, selected = true } = {}) {
        const normalized = value === null && this.palette.allowNone ? null : color(value);
        this.currentColor = normalized; this.mixed = Boolean(mixed); this.hasSelection = Boolean(selected);
        this.palette.setValue(normalized, { mixed }); this.updateColorIndicator();
    }
    updateColorIndicator() {
        const shown = this.hasSelection ? this.currentColor : this.lastColor;
        const token = this.hasSelection && this.mixed ? 'mixed' : shown ?? 'none';
        this.text = `panel:color-control:${token}|${this.label}`;
        const description = token === 'mixed' ? 'Mixed colors' : shown ?? this.noneLabel;
        this.setProperty('tooltip', `${this.label}: ${description}`);
        this.setProperty('accessibleName', `${this.label}: ${description}`);
        const last = this.lastColor ?? this.noneLabel;
        this.lastAction.text = `panel:color-control:${this.lastColor ?? 'none'}|Apply last color (${last})`;
        this.lastAction.setProperty('accessibleName', `Apply last color (${last})`);
        for (const tooltip of this.children.filter(child => child.kind === Tooltip.kind)) tooltip.text = `${this.label}: ${description}`;
    }
}
