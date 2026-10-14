import { Column, Container, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State, Tooltip } from '@ricewm/app';
import { layouts, layoutById } from './layout-catalog.mjs';

/** A miniature document page, drawn exclusively from native placeholder boxes. */
export function layoutThumbnail(layout, pageSize = { width: 16, height: 9 }) {
    if (![pageSize.width, pageSize.height].every(value => Number.isFinite(value) && value > 0)) throw new TypeError('Invalid layout page size');
    const scale = 1600 / Math.max(pageSize.width, pageSize.height);
    const width = Math.max(1, Math.round(pageSize.width * scale)), height = Math.max(1, Math.round(pageSize.height * scale));
    const builder = new SceneBuilder(width, height).rect([0, 0, width, height], 0xffffffff);
    const ink = 0xff929292, light = 0xffb5b5b5, unit = Math.max(1, Math.min(width, height) / 180);
    const dashed = (x1, y1, x2, y2) => {
        const length = Math.hypot(x2 - x1, y2 - y1), count = Math.max(1, Math.ceil(length / (unit * 7)));
        for (let index = 0; index < count; index++) {
            const start = index / count, end = Math.min(1, (index + 0.55) / count);
            builder.line([x1 + (x2 - x1) * start, y1 + (y2 - y1) * start],
                [x1 + (x2 - x1) * end, y1 + (y2 - y1) * end], unit, ink);
        }
    };
    for (const { role, box } of layout.placeholders) {
        const x = box.x * width, y = box.y * height, w = box.width * width, h = box.height * height;
        dashed(x, y, x + w, y); dashed(x + w, y, x + w, y + h);
        dashed(x + w, y + h, x, y + h); dashed(x, y + h, x, y);
        if (role === 'picture') {
            builder.line([x + w * .2, y + h * .75], [x + w * .42, y + h * .3], unit * 2, light)
                .line([x + w * .42, y + h * .3], [x + w * .62, y + h * .62], unit * 2, light)
                .line([x + w * .62, y + h * .62], [x + w * .77, y + h * .4], unit * 2, light)
                .line([x + w * .77, y + h * .4], [x + w * .9, y + h * .75], unit * 2, light);
        } else {
            const count = role === 'body' ? 3 : 1;
            for (let line = 0; line < count; line++) {
                const top = y + h * (count === 1 ? .5 : .24 + line * .22);
                builder.line([x + w * .15, top], [x + w * (line % 2 ? .7 : .85), top], role === 'title' ? unit * 4 : unit * 2, light);
            }
        }
    }
    return builder.finish();
}

/** The same native thumbnail chooser backs new-slide and apply-layout actions. */
export function createLayoutTools({ onApply, onInsert, notice = () => {}, namePrefix = '' }) {
    let ready = false, busy = false, canApply = true, selectedLayout = null, current = null;
    let pageSize = { width: 16, height: 9 };
    const previews = [], choices = [];
    const makeMenu = (name, title, icon, callback) => {
        name = namePrefix + name;
        const content = new Column({ name: `${name}-choices`, text: 'panel:fixed:gap=4:flush' });
        const scroll = new ScrollView({ name: `${name}-scroll`, text: 'viewport:420|0/0', state: State.FLAT }, [content]);
        const menu = new Menu({ name, text: title, icon, state: State.START_ALIGNED, enabled: false, tooltip: title }, [scroll]);
        return { menu, content, callback, name };
    };
    const apply = makeMenu('slide-layout', 'Layout', 41, onApply);
    const insert = makeMenu('new-slide-layout', 'New slide', 10, onInsert);
    function update() {
        for (const record of [apply, insert]) record.menu.setProperty('enabled', ready && !busy && (record !== apply || canApply) && layouts().length > 0 && typeof record.callback === 'function');
        for (const { action, layout, record } of choices) {
            action.setProperty('enabled', record.menu.getProperty('enabled'));
            action.setProperty('selected', record === apply && layout.id === selectedLayout);
        }
    }
    function refresh() {
        if (current === layouts()) { update(); return; }
        current = layouts(); previews.length = 0; choices.length = 0;
        for (const record of [apply, insert]) {
            for (const child of [...record.content.children]) record.content.remove(child, { destroy: true });
            if (!current.length) record.content.append(new Label({ text: 'Open a presentation to load layouts' }));
            for (let start = 0; start < current.length; start += 3) {
                const row = new Row({ text: 'panel:fixed:gap=4:flush' });
                for (const layout of current.slice(start, start + 3)) {
                    const preview = new SceneSurface({ name: `${record.name}-preview-${layout.id}`, presentation: 'page', enabled: false });
                    preview.submit(layoutThumbnail(layout, pageSize)); previews.push({ preview, layout });
                    const action = new MenuItem({ name: `${record.name}-${layout.id}`, text: 'panel:fixed-height:132:width=144:gap=2:flush',
                        accessibleName: layout.name, tooltip: layout.name }, [
                        new Container({ text: 'panel:fixed-height:84:flush', state: State.FLAT }, [preview]),
                        new Label({ text: layout.name }), new Tooltip({ text: layout.name }),
                    ]);
                    action.on(Message.ACTIVATE, () => {
                        if (!action.getProperty('enabled') || !layoutById(layout.id)) return;
                        try { Promise.resolve(record.callback(layout.id)).catch(error => notice(error.message || 'Could not change slide layout')); }
                        catch (error) { notice(error.message || 'Could not change slide layout'); }
                    });
                    row.append(action); choices.push({ action, layout, record });
                }
                record.content.append(row);
            }
        }
        update();
    }
    refresh();
    return Object.freeze({ layoutMenu: apply.menu, newSlideMenu: insert.menu, refresh,
        setState(state = {}) {
            ready = state.ready ?? ready; busy = state.busy ?? busy;
            canApply = state.canApply ?? canApply;
            if (Object.hasOwn(state, 'selectedLayout')) selectedLayout = state.selectedLayout;
            if (state.pageSize && (state.pageSize.width !== pageSize.width || state.pageSize.height !== pageSize.height)) {
                layoutThumbnail({ placeholders: [] }, state.pageSize);
                pageSize = { ...state.pageSize };
                for (const { preview, layout } of previews) preview.submit(layoutThumbnail(layout, pageSize));
            }
            refresh();
        },
    });
}
