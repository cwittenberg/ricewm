import { Container, ScrollBar, ScrollView } from './controls.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { SceneSurface, retainedScene } from './scene-surface.mjs';

const bounded = value => Math.max(0, Math.min(1, value));
/** A native clipped page with view-only zoom and independent scroll axes. */
export class DocumentViewport extends Container {
    static constructorOptions = Object.freeze(["surface", "name"]);
    constructor({ surface, name = 'document-viewport', ...options } = {}) {
        if (!(surface instanceof SceneSurface)) throw new TypeError('DocumentViewport requires a SceneSurface');
        super({ ...options, name, text: 'panel:document-workspace:flush', state: State.EXPANSIVE });
        this.surface = surface; this.width = 0; this.height = 0; this.listenersForView = new Set();
        surface.setProperty('presentation', 'page');
        this.clip = new ScrollView({ name: `${name}-clip`, text: 'panel:document-viewport:flush', state: State.EXPANSIVE | State.FLAT }, [surface]);
        this.horizontal = new ScrollBar({ name: `${name}-horizontal`, orientation: 'horizontal', value: .5, accessibleName: 'Scroll page horizontally' });
        this.vertical = new ScrollBar({ name: `${name}-vertical`, value: .5, accessibleName: 'Scroll page vertically' });
        this.verticalTrack = new Container({ text: 'panel:fixed:width=16:flush', state: State.FLAT, visible: false }, [this.vertical]);
        this.horizontalTrack = new Container({ text: 'panel:fixed:accessory-height=16:flush', state: State.FLAT, visible: false }, [this.horizontal]);
        this.append(new Container({ text: 'panel:distribute:gap=0:flush', state: State.EXPANSIVE }, [this.clip, this.verticalTrack]));
        this.append(this.horizontalTrack);
        this.clip.on(Message.SIZE_CHANGED, event => {
            const anchor = this.centerAnchor();
            this.width = event.detail.width; this.height = event.detail.height;
            this.panToAnchor(anchor); this.refresh();
        });
        for (const [bar, property] of [[this.horizontal, 'pagePanX'], [this.vertical, 'pagePanY']]) {
            bar.on(Message.VALUE_CHANGED, () => { surface.setProperty(property, bar.getProperty('value')); this.refresh(); });
            bar.on(Message.KEY, event => {
                if (event.detail?.pressed === false) return;
                const key = event.detail?.keyCode;
                if (key === 110 || key === 115) { surface.setProperty(property, key === 110 ? 0 : 1); this.refresh(); }
                else if ([113,111,114,116,112,117].includes(key)) {
                    const delta = ([113,111,112].includes(key) ? -1 : 1) * ([112,117].includes(key) ? 160 : 24);
                    this.scrollBy(property === 'pagePanX' ? delta : 0, property === 'pagePanY' ? delta : 0);
                }
            });
        }
        surface.on(Message.CHANGE, event => {
            if (['frameSize', 'pageZoom', 'presentation'].includes(event.detail?.property)) this.refresh();
        });
        const wheel = event => {
            if (event.detail?.kind !== 'wheel' || ![4, 5].includes(event.detail.button) || surface.getProperty('presentation') !== 'page') return;
            const direction = event.detail.button === 4 ? -1 : 1;
            if (event.detail.modifiers & 4) this.setZoom(Math.max(10, Math.min(400, Math.round(this.view.percent / 10) * 10 - direction * 10)));
            else this.scrollBy(event.detail.modifiers & 1 ? direction * 48 : 0, event.detail.modifiers & 1 ? 0 : direction * 48);
            event.stopPropagation();
        };
        surface.on(Message.POINTER, wheel); this.clip.on(Message.POINTER, wheel);
        this.refresh();
    }
    dimensions() {
        const frame = retainedScene(this.surface), margin = Math.min(20, Math.floor(this.width / 8), Math.floor(this.height / 8));
        const width = Math.max(0, this.width - margin * 2), height = Math.max(0, this.height - margin * 2);
        const zoom = this.surface.getProperty('pageZoom');
        const scale = frame ? zoom === 'fit' ? Math.min(width / frame.width, height / frame.height) : zoom / 100 : 1;
        return { width, height, pageWidth: (frame?.width || 1) * scale, pageHeight: (frame?.height || 1) * scale, scale };
    }
    centerAnchor() {
        const { width, height, pageWidth, pageHeight } = this.dimensions();
        const axis = (viewport, page, pan) => page > viewport ? (pan * (page - viewport) + viewport / 2) / page : .5;
        return [axis(width, pageWidth, this.surface.getProperty('pagePanX')), axis(height, pageHeight, this.surface.getProperty('pagePanY'))];
    }
    panToAnchor([x, y]) {
        const { width, height, pageWidth, pageHeight } = this.dimensions();
        const axis = (viewport, page, anchor) => page > viewport ? bounded((anchor * page - viewport / 2) / (page - viewport)) : .5;
        this.surface.setProperty('pagePanX', axis(width, pageWidth, x));
        this.surface.setProperty('pagePanY', axis(height, pageHeight, y));
    }
    setZoom(zoom) {
        const anchor = this.centerAnchor();
        this.surface.setProperty('pageZoom', zoom);
        this.panToAnchor(anchor); this.refresh();
    }
    scrollBy(x, y) {
        const size = this.dimensions();
        for (const [delta, extent, property] of [[x, size.pageWidth - size.width, 'pagePanX'], [y, size.pageHeight - size.height, 'pagePanY']]) {
            if (extent > 0) this.surface.setProperty(property, bounded(this.surface.getProperty(property) + delta / extent));
        }
        this.refresh();
    }
    refresh() {
        const size = this.dimensions(), page = this.surface.getProperty('presentation') === 'page';
        this.horizontalTrack.setProperty('visible', page && size.pageWidth > size.width + 1);
        this.verticalTrack.setProperty('visible', page && size.pageHeight > size.height + 1);
        this.horizontal.setProperty('value', this.surface.getProperty('pagePanX'));
        this.vertical.setProperty('value', this.surface.getProperty('pagePanY'));
        this.view = Object.freeze({ zoom: this.surface.getProperty('pageZoom'), percent: Math.round(size.scale * 100),
            ready: Boolean(retainedScene(this.surface)), panX: this.surface.getProperty('pagePanX'), panY: this.surface.getProperty('pagePanY') });
        for (const listener of this.listenersForView) listener(this.view);
    }
    onViewChanged(listener) { this.listenersForView.add(listener); listener(this.view); return () => this.listenersForView.delete(listener); }
    destroy() { this.listenersForView.clear(); return super.destroy(); }
}
