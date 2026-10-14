import { Button, Container, Label, Row, Slider, Tooltip } from './controls.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';

export const ZOOM_ICONS_CAPABILITY = 1n << 23n;
export const ZOOM_IN_ICON = 516;
export const ZOOM_OUT_ICON = 517;

// Resolve at render time, after the host handshake and app attachment. Older
// displays cannot decode new icon IDs, so use unambiguous text instead.
class ZoomActionButton extends Button {
    toWidget() {
        const widget = super.toWidget();
        if (![ZOOM_IN_ICON, ZOOM_OUT_ICON].includes(widget.icon)
            || ((this.application?.hostCapabilities ?? 0n) & ZOOM_ICONS_CAPABILITY)) return widget;
        return Object.freeze({ ...widget, icon: null, text: widget.icon === ZOOM_IN_ICON ? '+' : '−' });
    }
}


/** Shared view zoom control; changes page scale without modifying document data. */
export class ZoomControl extends Row {
    static constructorOptions = Object.freeze(["viewport", "name", "compact", "min", "max", "showFit", "fitLabel"]);
    constructor({ viewport, name = 'document-zoom', compact = false, min = 10, max = 400, showFit = true, fitLabel = 'Fit slide to window', ...options } = {}) {
        if (!viewport?.setZoom || !viewport?.onViewChanged) throw new TypeError('ZoomControl requires a DocumentViewport');
        if (!Number.isSafeInteger(min) || !Number.isSafeInteger(max) || min < 10 || max > 400 || min >= max) throw new RangeError('Invalid zoom bounds');
        super({ ...options, name, text: compact ? 'panel:fixed:accessory-height=26:gap=2:flush' : 'panel:fixed:gap=4:flush' });
        this.viewport = viewport;
        const action = (suffix, icon, title, callback) => {
            const button = new ZoomActionButton({ name: `${name}-${suffix}`, text: compact ? 'panel:fixed:accessory-height=26:width=26:flush' : '', icon, tooltip: title, accessibleName: title }, [new Tooltip({ text: title })]);
            button.on(Message.ACTIVATE, () => { if (button.getProperty('enabled')) callback(); });
            return button;
        };
        this.slider = new Slider({ name: `${name}-slider`, min, max, step: 1, value: 100, accessibleName: 'Page zoom percent' });
        this.percentage = new Label({ name: `${name}-percent`, text: 'Fit' });
        this.minus = action('out', ZOOM_OUT_ICON, 'Zoom out', () => viewport.setZoom(Math.max(min, Math.min(max, Math.round(viewport.view.percent / 10) * 10 - 10))));
        this.plus = action('in', ZOOM_IN_ICON, 'Zoom in', () => viewport.setZoom(Math.max(min, Math.min(max, Math.round(viewport.view.percent / 10) * 10 + 10))));
        this.fit = action('fit', 87, fitLabel, () => viewport.setZoom('fit'));
        this.slider.on(Message.VALUE_CHANGED, () => viewport.setZoom(Math.round(this.slider.getProperty('value'))));
        this.slider.on(Message.KEY, event => {
            if (event.detail?.pressed === false) return;
            const key = event.detail?.keyCode;
            if (key === 110 || key === 115) viewport.setZoom(key === 110 ? min : max);
            else if ([113,111,114,116,112,117].includes(key)) {
                const delta = ([113,116,117].includes(key) ? -1 : 1) * ([112,117].includes(key) ? 50 : 10);
                viewport.setZoom(Math.max(min, Math.min(max, viewport.view.percent + delta)));
            }
        });
        this.append(this.minus);
        this.append(new Container({ text: `panel:fixed:accessory-height=${compact ? 26 : 24}:width=112:flush`, state: State.FLAT }, [this.slider]));
        this.append(this.plus); this.append(this.percentage); if (showFit) this.append(this.fit);
        this.unsubscribe = viewport.onViewChanged(view => {
            this.slider.setProperty('value', Math.max(min, Math.min(max, view.percent)));
            this.percentage.text = `${view.percent}%`;
            this.fit.setProperty('checked', view.zoom === 'fit');
            for (const control of [this.slider, this.fit]) control.setProperty('enabled', view.ready);
            this.minus.setProperty('enabled', view.ready && view.percent > min);
            this.plus.setProperty('enabled', view.ready && view.percent < max);
        });
    }
    destroy() { this.unsubscribe?.(); return super.destroy(); }
}
