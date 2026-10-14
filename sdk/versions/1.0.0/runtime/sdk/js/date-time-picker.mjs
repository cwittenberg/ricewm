import { Button, Column, Container, Label, Row, TextField, Tooltip } from './controls.mjs';
import { Icons } from './icons.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { addDays, addMonths, clockTime, dateObject, dateValue, localToday, parseTime, timeSeconds, timeValue } from './date-time-values.mjs';

const panel = 'panel:fixed:flush:tight-stack:gap=6';
const keyNames = { 9: 'Escape', 23: 'Tab', 36: 'Enter', 104: 'Enter', 110: 'Home', 115: 'End', 111: 'ArrowUp', 116: 'ArrowDown', 113: 'ArrowLeft', 114: 'ArrowRight', 112: 'PageUp', 117: 'PageDown' };
const keyOf = event => event.detail?.pressed === false ? null : event.detail?.key || keyNames[event.detail?.keyCode];
const bool = value => Boolean(value);
// Focusable RiceUI primitives, matching the native control interaction contract.
const focusableKinds = new Set([11, 12, 13, 14, 15, 16, 20, 21, 23, 24, 31, 32, 36, 40, 41, 42, 43, 44, 45, 54, 60, 61, 62, 63, 64]);

/** Shared input contract: drafts stay editable; only valid user choices emit VALUE_CHANGED. */
class TemporalPicker extends Column {
    static family = 'composite';
    static properties = Object.freeze({
        value: { default: '', validate: String, reflect: false },
        readOnly: { default: false, validate: bool },
        required: { default: false, validate: bool },
    });
    get value() { return this.getProperty('value'); }
    set value(value) { this.setValue(value); }
    setValue(value) { return this.setProperty('value', value); }
    setProperty(name, value, options) {
        if (name === 'value' && this.field) this.validateValue(value);
        const result = super.setProperty(name, value, options);
        if (this.field) {
            if (name === 'value') this.refresh();
            if (['enabled', 'readOnly', 'required'].includes(name)) {
                this.field.setProperty('readOnly', this.values.readOnly);
                this.field.setProperty('required', this.values.required);
                this.disclosure.setProperty('enabled', this.values.enabled && !this.values.readOnly);
                this.clearButton?.setProperty('enabled', !this.values.required && !this.values.readOnly);
                this.clearButton?.setProperty('visible', !this.values.required);
                if (!this.values.enabled || this.values.readOnly) this.setExpanded(false);
            }
        }
        return result;
    }
    available() {
        if (this.destroyed || this.values.readOnly) return false;
        for (let owner = this; owner; owner = owner.parent) if (!owner.values.enabled || !owner.values.visible || !(owner.state & State.ENABLED)) return false;
        return true;
    }
    makeInput(label, icon, placeholder) {
        this.label = label;
        this.inputLabel = this.append(new Label({ text: label, visible: !this.standalone }));
        this.field = new TextField({ name: `${this.name}-input`, accessibleName: label, placeholder, readOnly: this.values.readOnly, required: this.values.required, state: State.ENABLED | State.SELECT_ALL_ON_FOCUS });
        this.disclosure = this.action('choose', '', `Choose ${label.toLowerCase()}`, () => this.setExpanded(!this.expanded), icon);
        this.disclosure.setProperty('enabled', this.values.enabled && !this.values.readOnly);
        this.inputRow = this.append(new Row({ text: panel, visible: !this.standalone }, [this.field, this.slot(this.disclosure, 40)]));
        this.feedback = this.append(new Label({ state: State.MUTED }));
        this.field.on(Message.VALUE_CHANGED, () => { this.navigationActive = false; if (this.available()) this.acceptDraft(false); });
        this.field.on(Message.BLUR, () => { if (this.available()) this.acceptDraft(true); });
        this.field.on(Message.KEY, event => {
            const key = keyOf(event);
            if (!this.available()) return;
            if (key === 'Tab' && this.expanded) { this.advanceFocus(event); event.preventDefault(); }
            else if (key === 'Escape' && !this.standalone) { this.refresh(); this.setExpanded(false); event.preventDefault(); }
            else if (key === 'Enter') {
                if (this.expanded && this.navigationActive) this.commit(this.cursor);
                else this.acceptDraft(true);
                event.preventDefault();
            } else if (key === 'ArrowDown' && !this.expanded && (event.detail?.modifiers & 8)) {
                this.setExpanded(true); event.preventDefault();
            } else if (this.expanded && this.navigate?.(key, event)) event.preventDefault();
        });
    }
    action(suffix, text, title, callback, icon = null) {
        const button = new Button({ name: `${this.name}-${suffix}`, text, icon, accessibleName: title, tooltip: title }, [new Tooltip({ text: title })]);
        button.on(Message.ACTIVATE, () => { if (this.available() && button.values.enabled) callback(); });
        button.on(Message.KEY, event => {
            if (!this.standalone && this.available() && keyOf(event) === 'Escape') { this.setExpanded(false); this.focusInput(); event.preventDefault(); }
        });
        return button;
    }
    slot(control, width, height = 36) {
        return new Container({ text: `panel:fixed:flush:width=${width}:accessory-height=${height}`, state: State.FLAT }, [control]);
    }
    focusControl(control) {
        // Only one control in the window may advertise a semantic focus request.
        const root = this.application?.controlWindows.get(this.id)?.root || this;
        const clear = control => { control.setProperty('focused', false); for (const child of control.children) clear(child); };
        clear(root); control.setProperty('focused', true);
        if (control instanceof TextField) control.setSelection(0, Array.from(control.value).length);
    }
    initialFocusTarget() { return this.standalone ? this.cells?.find(cell => cell.date === this.cursor) || this.partFields?.hour || this.field : this.field; }
    focusInput() { this.focusControl(this.initialFocusTarget()); }
    advanceFocus(event) {
        const root = this.application?.controlWindows.get(this.id)?.root || this, controls = [];
        const visit = control => {
            if (!control.values.visible || !(control.state & State.ENABLED)) return;
            if (focusableKinds.has(control.kind) && !control.keyboardAccessory) controls.push(control);
            if (!(control instanceof Tooltip)) for (const child of control.children) visit(child);
        };
        visit(root);
        if (!controls.length) return;
        const index = controls.indexOf(event.target), delta = event.detail?.modifiers & 1 ? -1 : 1;
        this.focusControl(controls[(index + delta + controls.length) % controls.length]);
    }
    setExpanded(expanded) {
        if (expanded && !this.available()) return false;
        this.expanded = this.standalone || Boolean(expanded); this.editor?.setProperty('visible', this.expanded);
        this.disclosure?.setProperty('checked', this.expanded);
        if (this.field) this.field.setProperty('state', State.ENABLED | State.SELECT_ALL_ON_FOCUS | (this.expanded ? State.KEY_NAVIGATION : 0));
        this.navigationActive = false;
        if (this.expanded) { this.prepareEditor(); this.focusInput(); }
        return true;
    }
    error(text) { this.field.setProperty('invalid', true); this.feedback.text = text; this.feedback.setProperty('visible', true); this.feedback.setProperty('state', State.WARNING); }
    acceptDraft(normalize) {
        try {
            const value = this.parseDraft(this.field.value);
            this.validateValue(value);
            if (!value && this.values.required) throw new RangeError(`Choose a ${this.label.toLowerCase()}.`);
            const draft = this.field.value;
            this.commit(value, { close: normalize });
            if (!normalize) this.field.setProperty('value', draft);
            return true;
        } catch (error) {
            if (!normalize && this.incompleteDraft?.(this.field.value)) {
                this.field.setProperty('invalid', false); this.feedback.setProperty('visible', true); this.feedback.setProperty('state', State.MUTED);
                this.feedback.text = this instanceof DatePicker ? 'Keep typing YYYY-MM-DD, or choose a day.' : 'Keep typing a time, such as 09:30 or 9:30 AM.';
            } else this.error(error.message);
            return false;
        }
    }
    commit(value, { close = true } = {}) {
        if (!this.available()) return false;
        this.validateValue(value);
        if (!value && this.values.required) return false;
        const previous = this.value;
        // User input has one semantic event; setters use ordinary CHANGE events.
        this.setProperty('value', value, { notify: false });
        if (close) { this.setExpanded(false); this.focusInput(); }
        if (previous !== this.value) this.emit(Message.VALUE_CHANGED, { value: this.value, previous });
        return previous !== this.value;
    }
    makeFooter(actions) {
        this.clearButton = this.action('clear', 'Clear', `Clear ${this.label.toLowerCase()}`, () => this.commit(''), Icons.Close);
        this.clearButton.setProperty('visible', !this.values.required); actions.push(this.clearButton);
        if (!this.standalone) actions.push(this.action('done', 'Done', 'Close selector', () => { this.setExpanded(false); this.focusInput(); }, Icons.SelectionCheck));
        this.editor.append(new Row({ text: panel }, actions));
    }
}

/** Calendar plus editable ISO date. Calendar browsing never changes the value. */
export class DatePicker extends TemporalPicker {
    static authoring = Object.freeze({"usage":"Use DatePicker for civil date inputs; use Calendar inside a drawer/dialog when a primary summary already shows the date. Value is YYYY-MM-DD or empty. Constructor options: label, value, min, max, locale, firstDayOfWeek (locale or 0–6), expanded, required, readOnly, enabled. It supplies a calendar, month/year jump, Today/Clear, localized weekdays and real date validation.","events":"Import DatePicker, TimeSelector and Message from @ricewm/app. Listen to Message.VALUE_CHANGED for valid user changes, event.detail.value. Read picker.value; call picker.setValue(value) for programmatic updates. Invalid/incomplete typing retains the last valid value. Values have no timestamp or timezone conversion.","keyboard":"Calendar arrows browse days/weeks, PageUp/PageDown browse months, Enter chooses and Escape closes without choosing. Time Up/Down changes the focused hour/minute/second; Tab advances to the next field and Shift+Tab goes back, skipping arrow buttons. Place expanded selectors in a ScrollView for short windows. Exercise typing, keyboard navigation and limits through sdk.interact, then inspect and visually review the real native app."});
    static constructorOptions = Object.freeze(["name", "label", "value", "min", "max", "locale", "firstDayOfWeek", "expanded", "presentation", "now"]);
    static properties = Object.freeze({ value: { default: '', validate: dateValue, reflect: false } });
    constructor({ name = 'date-picker', label = 'Date', value = '', min = '', max = '', locale = undefined,
        firstDayOfWeek = 'locale', expanded = false, presentation = 'field', now = () => new Date(), ...properties } = {}) {
        if (!['field','calendar'].includes(presentation)) throw TypeError('Invalid date picker presentation');
        dateValue(min); dateValue(max);
        if (min && max && min > max) throw new RangeError('Minimum date must precede maximum date');
        if (typeof now !== 'function') throw new TypeError('now must be a function');
        const monthFormat = new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric', timeZone: 'UTC' });
        if (firstDayOfWeek === 'locale') {
            const language = new Intl.Locale(monthFormat.resolvedOptions().locale);
            firstDayOfWeek = (language.getWeekInfo?.() || language.weekInfo || { firstDay: 1 }).firstDay % 7;
        }
        if (!Number.isInteger(firstDayOfWeek) || firstDayOfWeek < 0 || firstDayOfWeek > 6) throw new RangeError('firstDayOfWeek must be locale or 0 (Sunday) through 6');
        super({ name, text: panel, state:State.FLAT, ...properties });
        Object.assign(this, { min, max, now, firstDayOfWeek, monthFormat, standalone:presentation === 'calendar' });
        this.dayFormat = new Intl.DateTimeFormat(locale, { dateStyle: 'full', timeZone: 'UTC' });
        this.weekFormat = new Intl.DateTimeFormat(locale, { weekday: 'narrow', timeZone: 'UTC' });
        this.makeInput(label, Icons.Calendar, 'YYYY-MM-DD');
        this.editor = this.append(new Column({ text: panel, visible: false, state:State.FLAT }));
        this.previous = this.action('previous-month', '', 'Previous month', () => this.moveMonth(-1), 31);
        this.next = this.action('next-month', '', 'Next month', () => this.moveMonth(1), 32);
        this.month = new TextField({ name: `${name}-month`, accessibleName: 'Month and year, YYYY-MM', placeholder: 'YYYY-MM', state: State.ENABLED | State.SELECT_ALL_ON_FOCUS });
        this.monthTitle = this.action('month-heading', '', 'Change month and year', () => {
            this.monthTitle.setProperty('visible',false); this.month.setProperty('visible',true); this.focusControl(this.month);
        });
        this.month.setProperty('visible',false);
        this.editor.append(new Row({ text: panel }, [this.slot(this.previous, 36), this.monthTitle, this.month, this.slot(this.next, 36)]));
        const jump = () => {
            if (!this.available()) return;
            try {
                const date = dateValue(`${this.month.value.trim()}-01`);
                this.cursor = this.clamp(date); this.renderCalendar(); this.month.setProperty('invalid', false); this.month.setProperty('visible',false); this.monthTitle.setProperty('visible',true);
            } catch { this.month.setProperty('invalid', true); }
        };
        this.month.on(Message.BLUR, jump);
        this.month.on(Message.KEY, event => { if (keyOf(event) === 'Enter') { jump(); this.focusInput(); } else if (keyOf(event) === 'Escape') { if (this.month.values.visible) { this.month.setProperty('visible',false); this.monthTitle.setProperty('visible',true); this.focusInput(); event.preventDefault(); } else if (!this.standalone) { this.setExpanded(false); this.focusInput(); event.preventDefault(); } } });
        const headings = this.editor.append(new Row({ text: 'panel:fixed:flush:gap=4', state:State.FLAT }));
        for (let day = 0; day < 7; day++) headings.append(this.slot(new Label({ text: this.weekFormat.format(dateObject(addDays('2023-01-01', (day + firstDayOfWeek) % 7))), state: State.CENTER_ALIGNED | State.MUTED }), 48, 28));
        this.cells = [];
        for (let row = 0; row < 6; row++) {
            const line = this.editor.append(new Row({ text: 'panel:fixed:flush:gap=4', state:State.FLAT }));
            for (let col = 0; col < 7; col++) {
                const index = row * 7 + col;
                const cell = this.action(`day-${index}`, '', 'Choose day', () => this.commit(cell.date));
                cell.on(Message.FOCUS, () => { if (cell.date) { this.cursor = cell.date; this.navigationActive = true; } });
                cell.on(Message.KEY, event => {
                    if (this.available() && this.navigate(keyOf(event))) { this.focusInput(); event.preventDefault(); }
                });
                this.cells.push(cell); line.append(this.slot(cell, 48, 40));
            }
        }
        this.todayButton = this.action('today', 'Today', 'Choose today', () => this.commit(localToday(this.now())), Icons.Calendar);
        this.makeFooter([this.todayButton]);
        this.setValue(value); this.setExpanded(expanded);
    }
    validateValue(value) {
        dateValue(value);
        if (value && (this.min && value < this.min || this.max && value > this.max)) throw new RangeError(`Choose a date ${this.min ? `on or after ${this.min}` : ''}${this.min && this.max ? ' and ' : ''}${this.max ? `on or before ${this.max}` : ''}.`);
    }
    parseDraft(value) { return dateValue(value.trim()); }
    incompleteDraft(value) { return value.length < 10 && /^\d{0,4}(?:-\d{0,2}(?:-\d{0,2})?)?$/u.test(value); }
    clamp(value) {
        if (Number(value.split('-')[0]) > 9999) return this.max || '9999-12-31';
        return value < (this.min || '0001-01-01') ? this.min || '0001-01-01' : value > (this.max || '9999-12-31') ? this.max || '9999-12-31' : value;
    }
    refresh() {
        this.field.value = this.value; this.field.setProperty('invalid', false);
        this.feedback.setProperty('state', State.MUTED);
        this.feedback.text = ''; this.feedback.setProperty('visible',false);
        if (this.expanded) this.prepareEditor();
    }
    prepareEditor() { this.cursor = this.clamp(this.value || localToday(this.now())); this.navigationActive = true; this.renderCalendar(); }
    moveMonth(delta) { this.cursor = this.clamp(addMonths(this.cursor, delta)); this.navigationActive = true; this.renderCalendar(); }
    renderCalendar() {
        const first = `${this.cursor.slice(0, 7)}-01`, weekday = dateObject(first).getUTCDay();
        const offset = (weekday - this.firstDayOfWeek + 7) % 7;
        this.monthTitle.text = this.monthFormat.format(dateObject(first)); this.month.value = first.slice(0, 7);
        this.month.setProperty('invalid', false);
        const today = localToday(this.now());
        this.cells.forEach((cell, index) => {
            const date = addDays(first, index - offset), inMonth = date.slice(0, 7) === first.slice(0, 7);
            const available = date >= (this.min || '0001-01-01') && date <= (this.max || '9999-12-31');
            const realDate = /^\d{4}-/u.test(date) && !date.startsWith('0000');
            cell.date = realDate ? date : null; cell.text = realDate ? String(Number(date.slice(-2))) : '';
            cell.setProperty('enabled', available && realDate && !this.values.readOnly);
            cell.setProperty('state', State.ENABLED | (date === this.value || date === this.cursor ? 0 : State.FLAT) | (inMonth ? 0 : State.MUTED) | (date === today ? State.EMPHASIZED : 0));
            cell.setProperty('selected', date === this.cursor); cell.setProperty('checked', date === this.value);
            cell.setProperty('accent', date === this.value);
            const title = available && realDate ? `${this.dayFormat.format(dateObject(date))}${date === today ? ', today' : ''}${date === this.value ? ', selected' : ''}` : 'Date unavailable';
            cell.setProperty('accessibleName', title); cell.setProperty('tooltip', title); cell.children[0].text = title;
        });
        this.previous.setProperty('enabled', this.clamp(addMonths(first, -1)).slice(0, 7) !== first.slice(0, 7));
        this.next.setProperty('enabled', this.clamp(addMonths(first, 1)).slice(0, 7) !== first.slice(0, 7));
        this.todayButton.setProperty('enabled', today === this.clamp(today));
    }
    navigate(key) {
        if (!this.expanded) return false;
        const delta = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[key];
        if (delta) this.cursor = this.clamp(addDays(this.cursor, delta));
        else if (key === 'PageUp' || key === 'PageDown') this.cursor = this.clamp(addMonths(this.cursor, key === 'PageUp' ? -1 : 1));
        else if (key === 'Home' || key === 'End') {
            const offset = (dateObject(this.cursor).getUTCDay() - this.firstDayOfWeek + 7) % 7;
            this.cursor = this.clamp(addDays(this.cursor, key === 'Home' ? -offset : 6 - offset));
        } else return false;
        this.navigationActive = true; this.renderCalendar();
        // The highlighted calendar cell and its accessible name already convey the date.
        return true;
    }
}

/** Editable clock time with labelled hour/minute steppers and optional AM/PM. */
export class TimePicker extends TemporalPicker {
    static authoring = Object.freeze({"usage":"Use TimeSelector (alias TimePicker) for wall-clock time inputs; use TimeEditor inside an already disclosed drawer/dialog to avoid a duplicate full-time field. Value is HH:MM, or HH:MM:SS with showSeconds:true. Constructor options: label, value, min, max, locale, hourCycle (locale,12,24), minuteStep (1–60, default 1), showSeconds, quickTimes, expanded, required, readOnly, enabled. It supplies a clock icon, AM/PM, Now and full-height icon steppers beside each input. Minutes default to one-minute increments. Set minuteStep only when larger increments are requested; labels disclose the exact amount. Secondary editor controls stay collapsed until requested.","events":"Import DatePicker, TimeSelector and Message from @ricewm/app. Listen to Message.VALUE_CHANGED for valid user changes, event.detail.value. Read picker.value; call picker.setValue(value) for programmatic updates. Invalid/incomplete typing retains the last valid value. Values have no timestamp or timezone conversion.","keyboard":"Calendar arrows browse days/weeks, PageUp/PageDown browse months, Enter chooses and Escape closes without choosing. Time Up/Down changes the focused hour/minute/second; Tab advances to the next field and Shift+Tab goes back, skipping arrow buttons. Place expanded selectors in a ScrollView for short windows. Exercise typing, keyboard navigation and limits through sdk.interact, then inspect and visually review the real native app."});
    static constructorOptions = Object.freeze(["name", "label", "value", "min", "max", "locale", "hourCycle", "minuteStep", "showSeconds", "expanded", "quickTimes", "presentation", "now"]);
    static properties = Object.freeze({ value: { default: '', validate: timeValue, reflect: false } });
    constructor({ name = 'time-picker', label = 'Time', value = '', min = '', max = '', locale = undefined,
        hourCycle = 'locale', minuteStep = 1, showSeconds = false, expanded = false,
        quickTimes = ['09:00', '12:00', '17:00'], presentation = 'field', now = () => new Date(), ...properties } = {}) {
        if (!['field','editor'].includes(presentation)) throw TypeError('Invalid time picker presentation');
        timeValue(min); timeValue(max);
        if (!showSeconds && [min, max].some(value => value && timeSeconds(value) % 60)) throw new RangeError('Enable showSeconds for bounds with seconds');
        if (min && max && timeSeconds(min) > timeSeconds(max)) throw new RangeError('Minimum time must precede maximum time');
        if (!Number.isInteger(minuteStep) || minuteStep < 1 || minuteStep > 60) throw new RangeError('minuteStep must be 1 through 60');
        if (!Array.isArray(quickTimes) || quickTimes.length > 8) throw new RangeError('Provide up to eight quick times');
        quickTimes.forEach(timeValue);
        if (typeof now !== 'function') throw new TypeError('now must be a function');
        const resolvedCycle = new Intl.DateTimeFormat(locale, { hour: 'numeric' }).resolvedOptions().hourCycle;
        if (hourCycle === 'locale') hourCycle = ['h11', 'h12'].includes(resolvedCycle) ? 12 : 24;
        if (![12, 24].includes(hourCycle)) throw new RangeError('hourCycle must be locale, 12 or 24');
        super({ name, text: panel, state:State.FLAT, ...properties });
        Object.assign(this, { min, max, hourCycle, minuteStep, showSeconds: Boolean(showSeconds), now, standalone:presentation === 'editor' });
        this.makeInput(label, Icons.Clock, hourCycle === 12 ? '9:30 AM' : '09:30');
        this.editor = this.append(new Column({ text: panel, visible: false, state:State.FLAT }));
        this.partFields = {};
        const parts = this.editor.append(new Row({ text: panel }));
        for (const [part, amount] of [['hour', 3600], ['minute', minuteStep * 60], ...(showSeconds ? [['second', 1]] : [])]) {
            const title = part[0].toUpperCase() + part.slice(1);
            const field = new TextField({ name: `${name}-${part}`, accessibleName: title, state: State.ENABLED | State.SELECT_ALL_ON_FOCUS | State.KEY_NAVIGATION });
            const increment = part === 'minute' && minuteStep !== 1 ? ` by ${minuteStep} minutes` : '';
            const down = this.action(`${part}-down`, '', `Decrease ${part}${increment}`, () => this.adjust(-amount), Icons.ChevronDown);
            const up = this.action(`${part}-up`, '', `Increase ${part}${increment}`, () => this.adjust(amount), Icons.ChevronUp);
            down.keyboardAccessory = true; up.keyboardAccessory = true;
            parts.append(new Column({ text: panel }, [new Label({ text: title }), new Row({ text: 'panel:fixed:flush:gap=4' }, [this.slot(down, 36), this.slot(field, 44), this.slot(up, 36)])]));
            const edit = () => {
                if (!this.available()) return;
                const number = /^\d{1,2}$/u.test(field.value) ? Number(field.value) : NaN;
                const valid = Number.isInteger(number) && number >= (part === 'hour' && hourCycle === 12 ? 1 : 0) && number <= (part === 'hour' ? hourCycle === 12 ? 12 : 23 : 59);
                field.setProperty('invalid', !valid);
                if (!valid) return;
                const values = this.cursor.split(':').map(Number);
                values[['hour', 'minute', 'second'].indexOf(part)] = part === 'hour' && hourCycle === 12 ? number % 12 + (values[0] >= 12 ? 12 : 0) : number;
                const draft = field.value;
                try {
                    this.commit(clockTime(values[0] * 3600 + values[1] * 60 + (values[2] || 0), this.showSeconds), { close: false });
                    field.value = draft;
                } catch (error) { field.setProperty('invalid', true); this.feedback.text = error.message; this.feedback.setProperty('visible', true); }
            };
            field.on(Message.VALUE_CHANGED, edit);
            field.on(Message.KEY, event => {
                if (!this.available()) return;
                const key = keyOf(event);
                if (key === 'Tab') { this.advanceFocus(event); event.preventDefault(); }
                else if (['ArrowUp', 'ArrowDown'].includes(key)) { this.adjust(key === 'ArrowUp' ? amount : -amount); event.preventDefault(); }
                else if (this.moveCaret(field, key, event)) event.preventDefault();
                else if (key === 'Enter') { edit(); if (!this.standalone) { this.setExpanded(false); this.focusInput(); } }
                else if (key === 'Escape' && !this.standalone) { this.setExpanded(false); this.focusInput(); event.preventDefault(); }
            });
            this.partFields[part] = field;
        }
        if (hourCycle === 12) {
            this.period = this.action('period', 'AM', 'Switch AM or PM', () => this.chooseClock((timeSeconds(this.cursor) + 43200) % 86400)); parts.append(this.slot(this.period, 48));
        }
        this.quickButtons = quickTimes.map((time, index) => {
            const button = this.action(`quick-${index}`, this.display(time), `Choose ${this.display(time)}`, () => this.commit(this.normalizeTime(time)));
            button.quickTime = time; return button;
        });
        if (this.quickButtons.length) this.editor.append(new Row({ text: panel }, this.quickButtons));
        this.nowButton = this.action('now', 'Now', 'Choose current time', () => this.commit(this.currentTime()), Icons.Clock);
        this.makeFooter([this.nowButton]);
        this.setValue(value); this.setExpanded(expanded);
    }
    normalizeTime(value) {
        timeValue(value);
        if (!value) return '';
        if (!this.showSeconds && timeSeconds(value) % 60) throw new RangeError('Seconds are not enabled');
        return clockTime(timeSeconds(value), this.showSeconds);
    }
    setProperty(name, value, options) { if (name === 'value' && this.field) value = this.normalizeTime(value); return super.setProperty(name, value, options); }
    validateValue(value) {
        timeValue(value);
        if (value && (this.min && timeSeconds(value) < timeSeconds(this.min) || this.max && timeSeconds(value) > timeSeconds(this.max))) throw new RangeError(`Choose a time ${this.min ? `at or after ${this.display(this.min)}` : ''}${this.min && this.max ? ' and ' : ''}${this.max ? `at or before ${this.display(this.max)}` : ''}.`);
    }
    parseDraft(value) { return parseTime(value, this.showSeconds); }
    incompleteDraft(value) {
        return /^\d{0,2}(?::\d{0,2}){0,2}\s*[ap]?$/iu.test(value.trim())
            && (value.trim().length < 5 || /[:ap]$/iu.test(value.trim()));
    }
    display(value) {
        if (!value) return '';
        const [h, ...rest] = value.split(':');
        return this.hourCycle === 12 ? `${Number(h) % 12 || 12}:${rest.join(':')} ${Number(h) >= 12 ? 'PM' : 'AM'}` : value;
    }
    currentTime() { const date = this.now(); return clockTime(date.getHours() * 3600 + date.getMinutes() * 60 + (this.showSeconds ? date.getSeconds() : 0), this.showSeconds); }
    clamp(seconds) { return Math.max(this.min ? timeSeconds(this.min) : 0, Math.min(this.max ? timeSeconds(this.max) : this.showSeconds ? 86399 : 86340, seconds)); }
    refresh() {
        this.field.value = this.display(this.value); this.field.setProperty('invalid', false);
        this.feedback.setProperty('state', State.MUTED);
        this.feedback.text = ''; this.feedback.setProperty('visible', false); // Show feedback only for an incomplete or invalid draft.
        if (this.expanded) this.prepareEditor();
    }
    prepareEditor() {
        this.cursor = this.value || clockTime(this.clamp(timeSeconds(this.currentTime())), this.showSeconds);
        const [hour, minute, second] = this.cursor.split(':').map(Number);
        for (const [part, number] of Object.entries({ hour: this.hourCycle === 12 ? hour % 12 || 12 : hour, minute, second })) if (this.partFields[part]) { this.partFields[part].value = String(number).padStart(2, '0'); this.partFields[part].setProperty('invalid', false); }
        if (this.period) this.period.text = hour >= 12 ? 'PM' : 'AM';
        this.quickButtons.forEach(button => { try { this.validateValue(this.normalizeTime(button.quickTime)); button.setProperty('enabled', true); } catch { button.setProperty('enabled', false); } });
        this.nowButton.setProperty('enabled', this.clamp(timeSeconds(this.currentTime())) === timeSeconds(this.currentTime()));
    }
    chooseClock(seconds) {
        const value = clockTime(this.clamp(seconds), this.showSeconds);
        return this.commit(value, { close: false });
    }
    adjust(amount) { if (this.available()) this.chooseClock(timeSeconds(this.cursor || this.value || this.currentTime()) + amount); }
    navigate(key, event) {
        if (!['ArrowUp', 'ArrowDown'].includes(key)) return this.moveCaret(this.field, key, event);
        this.adjust((key === 'ArrowUp' ? 1 : -1) * this.minuteStep * 60); return true;
    }
    moveCaret(field, key, event) {
        if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(key)) return false;
        const length = Array.from(field.value).length, selection = field.selection || { anchor: length, caret: length };
        const caret = key === 'Home' ? 0 : key === 'End' ? length : Math.max(0, Math.min(length, selection.caret + (key === 'ArrowLeft' ? -1 : 1)));
        field.setSelection(event.detail?.modifiers & 1 ? selection.anchor : caret, caret); return true;
    }
}

// Explicit selector spelling for app authors; existing TimePicker imports stay valid.
export { TimePicker as TimeSelector };

/** Calendar-only surface for drawers and dialogs; no duplicate date field or Done action. */
export class Calendar extends DatePicker {
    static authoring = Object.freeze({...DatePicker.authoring, usage:'Standalone Calendar for a drawer/dialog. Same civil date value/events and bounds as DatePicker. No duplicate date input, summary or Done. Month heading opens a month/year jump. Today/Clear are the only footer actions. Give it at least 360 pixels width and 400 pixels height; retain comfortable targets and use a scrollable drawer. Close your drawer on selection if appropriate.'});
    constructor(options={}) { super({...options,presentation:'calendar'}); }
}
/** Time editor for an already disclosed drawer or dialog. The owner supplies one Done/Apply action. */
export class TimeEditor extends TimePicker {
    static authoring = Object.freeze({...TimePicker.authoring, usage:'Standalone TimeEditor for a drawer/dialog: labelled hour/minute fields with steppers, optional seconds/AM-PM, Now/Clear and optional quick times. Same values/events/bounds as TimePicker. No duplicate full-time input or internal Done. Owner supplies one Done/Apply action; do not close on every keystroke. minuteStep defaults to 1.'});
    constructor(options={}) { super({...options,presentation:'editor'}); }
}
