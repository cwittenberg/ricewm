import { supportedLink } from './hyperlink-contract.mjs';
import { canReorderObjects, objectOrderingActions } from './object-ordering.mjs';
import { ColorPalette, ContextMenu, Menu, MenuItem, Message, ScrollView } from '@ricewm/app';
import { selectedTextStyle } from './text-selection-style.mjs';

/** Contextual commands remain injected; the shared SDK owns native positioning. */
export function createDeckContextMenu({ owner, actions = {}, can = () => true, notice = () => {} }) {
    let state = { ready: false, busy: false, selected: [], canUndo: false, canRedo: false, background: null };
    let destroyed = false;
    const entries = [], palettes = [];
    const menu = new ContextMenu({ name: 'deck-context-menu' }).attachTo(owner);
    const selected = () => state.selected;
    const single = () => selected().length === 1;
    const any = () => selected().length > 0;
    const all = flag => any() && selected().every(object => object.edit?.[flag]);
    const empty = () => !any();
    function supported(name) { return typeof actions[name] === 'function'; }
    function enabled(entry) { return !destroyed && state.ready && !state.busy && supported(entry.name) && entry.visible() && entry.available() && can(entry.name, state); }
    function invoke(entry, value) {
        if (!enabled(entry)) return;
        // Native MenuItem activation dismisses the popup before this callback.
        try { Promise.resolve(actions[entry.name](value)).catch(error => notice(String(error?.message || error))); }
        catch (error) { notice(String(error?.message || error)); }
    }
    function item(name, label, icon, visible = any, available = () => true) {
        const control = new MenuItem({ name: `context-${name}`, text: label, icon });
        const entry = { name, control, visible, available };
        control.on(Message.ACTIVATE, () => invoke(entry)); menu.append(control); entries.push(entry);
        return entry;
    }
    function palette(name, label, icon, visible, available, colorValues, allowNone = true) {
        const colors = new ColorPalette({ name: `context-${name}-palette`, allowNone,
            noneLabel: name === 'strokeColor' ? 'No outline' : 'No fill', moreColors: true });
        const scroll = new ScrollView({ name: `context-${name}-scroll`, text: 'viewport:360|0/0' }, [colors]);
        colors.moreButton.on(Message.ACTIVATE, () => { if (colors.editor.getProperty('visible')) scroll.scrollTo(colors.editor); });
        const submenu = new Menu({}, [scroll]);
        const control = new MenuItem({ name: `context-${name}`, text: label, icon }, [submenu]);
        const entry = { name, control, visible, available, colors, colorValues };
        colors.on(Message.VALUE_CHANGED, event => invoke(entry, name === 'style' ? { color: event.detail.value } : event.detail.value));
        menu.append(control); entries.push(entry); palettes.push(entry);
    }
    item('undo', 'Undo', 20, () => true, () => state.canUndo);
    item('redo', 'Redo', 21, () => true, () => state.canRedo);
    item('editText', 'Edit text', 194, () => single() && Boolean(selected()[0].text), () => selected()[0]?.text?.editable === true);
    item('addText', 'Add text', 194, () => single() && selected()[0].kind === 'shape' && !selected()[0].text && selected()[0].geometry !== 'line', () => selected()[0].edit?.canAddText === true);
    item('editLink', 'Insert or edit link…', 80, single, () => selected()[0]?.canEditHyperlink === true);
    item('openLink', 'Open link', 80, () => single() && selected()[0].hyperlinks?.some(supportedLink));
    item('removeLink', 'Remove link…', 25, () => single() && Boolean(selected()[0].hyperlinks?.length), () => selected()[0]?.canEditHyperlink === true);
    item('copy', 'Copy', 23, any);
    item('paste', 'Paste', 24, () => true);
    item('duplicate', 'Duplicate', 23, any, () => all('canTransform'));
    item('delete', 'Delete', 25, any, () => all('canDelete'));
    item('positionSize', 'Position and size…', 189, any, () => single() && all('canTransform'));
    item('rotate', 'Rotate…', 195, any, () => all('canRotate'));
    item('formatShape', 'Format shape…', 182, () => any() && selected().every(object => !object.image), () => all('canStyle'));
    item('formatPicture', 'Format picture…', 71, () => single() && Boolean(selected()[0].image));
    palette('fill', 'Fill', 186, () => any() && selected().every(object => !object.image),
        () => all('canStyle') && selected().every(object => object.geometry !== 'line'), () => selected().map(object => object.fill?.color ?? null));
    palette('strokeColor', 'Outline', 187, any, () => all('canStyle'), () => selected().map(object => object.stroke?.color ?? null));
    palette('style', 'Text color', 188, () => any() && selected().some(object => object.text),
        () => selected().some(object => object.text?.editable), () => {
            const style = selectedTextStyle(selected()); return style.mixed.includes('color') ? [null, '#000000'] : [style.color ?? '#000000'];
        }, false);
    for (const { action, label, icon } of objectOrderingActions) {
        item(action, label, icon, any, () => canReorderObjects(selected()));
    }
    item('group', 'Group', 239, () => selected().length > 1, () => all('canGroup') && selected().every(object => !object.parentId));
    item('ungroup', 'Ungroup', 240, () => single() && selected()[0].kind === 'group', () => selected()[0].group?.canUngroup === true);
    palette('backgroundColor', 'Background color', 186, empty, () => true,
        () => [typeof state.background === 'string' ? state.background : state.background?.color ?? '#FFFFFF'], false);
    item('slideBackground', 'Format background…', 186, empty);
    function update() {
        for (const entry of entries) {
            const visible = supported(entry.name) && entry.visible();
            entry.control.setProperty('visible', visible);
            entry.control.setProperty('enabled', enabled(entry));
            entry.colors?.setProperty('enabled', enabled(entry));
        }
        for (const entry of palettes) {
            const values = entry.colorValues(), first = values[0] ?? (entry.colors.allowNone ? null : '#000000');
            entry.colors.setValue(first, { mixed: values.some(value => value !== values[0]) });
            if (state.themeColors) entry.colors.setThemeColors(state.themeColors);
        }
    }
    update();
    return Object.freeze({ menu,
        setState(next) { if (destroyed) return; state = { ...state, ...next, selected: next.selected ?? state.selected }; update(); },
        destroy() { if (destroyed) return; destroyed = true; menu.detach(); menu.destroy(); },
    });
}
