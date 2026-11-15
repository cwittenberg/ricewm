import { Button, ColorPalette, Column, Label, Menu, Message, Row, ScrollView, State, PropertyInspector, EditableChoice, NumberInput, TabView, TabPage, CheckBox, ColorMenu, Icons } from '@ricewm/app';
import { DEFAULT_SHADOW } from './shadow-contract.mjs';
import { selectedTextStyle } from './text-selection-style.mjs';
import {deckFontFamilies} from './font-choices.mjs';

/** A selection-aware native inspector. Commands use the same canvas adapter as
 * the ribbon; the inspector never owns a second copy of document state. */
export function createSelectionProperties({ canvas, tools, textPanel, shadowsSupported = false }) {
    let selected = [], usable = false, fontSelection = null, paintSelection = null, geometrySelection = null;
    const title = new Label({ name: 'properties-heading', text: 'Slide' });
    const details = new Label({ name: 'selection-dimensions', text: '' });
    const slideDetails = new Label({ name: 'slide-dimensions', text: '' });
    const dimension = (id, label, group) => ({ id, type: 'number', label, group, unit: '"',
        minimum: ['width', 'height'].includes(id) ? .001 / 2.54 : -10000 / 2.54,
        maximum: 10000 / 2.54, step: .1, precision: 2 });
    const geometrySpecs = [
        dimension('height', 'Height', 'Size'), dimension('width', 'Width', 'Size'),
        { id: 'rotation', type: 'number', label: 'Rotation', group: 'Size', unit: '°', minimum: -36000, maximum: 36000, step: .1, precision: 2 },
        dimension('x', 'Horizontal', 'Position'), dimension('y', 'Vertical', 'Position'),
    ];
    const geometry = new PropertyInspector({ name: 'selection-geometry', properties: geometrySpecs, selection: [], commitMode: 'field',
        read: (id, key) => { const object = selected.find(item => item.id === id); return key === 'rotation' ? object?.edit?.rotationDegrees ?? 0 : object?.edit?.box?.[key] == null ? null : object.edit.box[key] / 96; },
        // Controls display physical units; the canvas and engine share pixels.
        apply: ({ ids, values }) => canvas.setProperties({ ids, values: Object.fromEntries(
            Object.entries(values).map(([key, value]) => [key, key === 'rotation' ? value : value * 96])) }),
    });
    const fonts = new EditableChoice({ name: 'properties-font-family', accessibleName: 'Font family', choices: deckFontFamilies,
        onCommit: fontFamily => usable ? canvas.style({ fontFamily }) : false });
    const fontSize = new NumberInput({ name: 'properties-font-size', accessibleName: 'Font size', minimum: .75, maximum: 384, precision: 2, unit: 'pt',
        onCommit: points => usable ? canvas.style({ fontSize: points * 96 / 72 }) : false });
    const fontPanel = new Column({ name: 'selection-font', text: 'panel:fixed:gap=4', visible: false }, [new Label({ text: 'Font' }), fonts, fontSize]);
    const controls = [];
    const command = (name, label, icon, callback, available) => {
        const button = new Button({ name, text: label, icon, state: State.START_ALIGNED });
        button.on(Message.ACTIVATE, () => { if (button.getProperty('enabled')) void callback(); });
        controls.push({ control: button, available }); return button;
    };
    const palettes = [];
    const palette = (name, label, icon, key, callback) => {
        const colors = new ColorPalette({ name: `${name}-palette`, allowNone: true, columns: 10,
            noneLabel: key === 'stroke' ? 'No outline' : 'No fill', moreColors: true });
        const scroll = new ScrollView({ text: 'viewport:360|0/0' }, [colors]);
        colors.moreButton.on(Message.ACTIVATE, () => { if (colors.editor.getProperty('visible')) scroll.scrollTo(colors.editor); });
        const menu = new Menu({ name, text: '', icon, tooltip: label, accessibleName: label, state: State.DISPLAY }, [scroll]);
        colors.on(Message.VALUE_CHANGED, event => { if (menu.getProperty('enabled')) void callback(event.detail.value); });
        controls.push({ control: menu, available: () => selected.length > 0 && selected.every(object => object.edit?.canStyle && (key !== 'fill' || object.geometry !== 'line')) });
        palettes.push({ colors, key }); return menu;
    };
    const appearance = new Row({ name: 'selection-appearance' }, [
        palette('properties-fill', 'Fill', 186, 'fill', value => canvas.fill(value)),
        palette('properties-outline', 'Outline', 187, 'stroke', value => canvas.strokeColor(value)),
    ]);
    const position = command('properties-position', 'Position and size', 189, tools.position,
        () => selected.length === 1 && selected[0].edit?.canTransform && selected[0].geometry !== 'line');
    const crop = command('properties-crop', 'Crop picture', 193, tools.crop,
        () => selected.length === 1 && selected[0].image?.canCrop);
    const insert = command('properties-insert-picture', 'Insert picture', 71, tools.insertPicture,
        () => selected.length === 1 && selected[0].placeholder?.role === 'picture' && !selected[0].text?.text && selected[0].edit?.canDelete);
    const lineWidth = new NumberInput({ name: 'properties-line-width', accessibleName: 'Line width', unit: 'pt', minimum: 0, maximum: 10000, precision: 2,
        onCommit: value => usable ? canvas.strokeWidth(value / .75) : false });
    const fillOpacity = new NumberInput({ name: 'properties-fill-transparency', accessibleName: 'Fill transparency', unit: '%', minimum: 0, maximum: 100, precision: 1,
        onCommit: value => usable ? canvas.fillOpacity(1 - value / 100) : false });
    const lineOpacity = new NumberInput({ name: 'properties-line-transparency', accessibleName: 'Line transparency', unit: '%', minimum: 0, maximum: 100, precision: 1,
        onCommit: value => usable ? canvas.lineOpacity(1 - value / 100) : false });
    const fieldRow = (text, input) => new Row({ text: 'panel:form-control' }, [new Label({ text }), input]);
    const fillColor = appearance.children[0], lineColor = appearance.children[1];
    appearance.remove(fillColor); appearance.remove(lineColor); appearance.destroy();
    const fillRow = fieldRow('Transparency', fillOpacity), lineRow = fieldRow('Transparency', lineOpacity);
    const shadowEnabled = new CheckBox({ name: 'properties-shadow-enabled', text: 'Shadow' });
    const shadowColor = new ColorMenu({ name: 'properties-shadow-color', label: 'Shadow color', icon: 186, allowNone: false });
    const shadows = new Map();
    const shadowPatch = patch => {
        if (!usable || !shadowsSupported || !selected.length || selected.some(object => !object.edit?.canSetShadow)) return false;
        return canvas.shadow(object => ({ ...DEFAULT_SHADOW, rotateWithShape: object.edit?.canTransformShadow !== false, ...(object.outerShadow ?? { opacity: .4, blurRadius: 6, angleDegrees: 45, distance: 4 }), ...patch }));
    };
    const shadowField = (key, label, unit, min, max, toUi = value => value, fromUi = value => value) => {
        const input = new NumberInput({ name: `properties-shadow-${key}`, accessibleName: `Shadow ${label.toLowerCase()}`, unit, minimum: min, maximum: max, precision: 2,
            onCommit: value => shadowPatch({ [key]: fromUi(value) }) });
        shadows.set(key, { input, toUi }); return fieldRow(label, input);
    };
    shadowEnabled.on(Message.ACTIVATE, () => {
        if (!shadowEnabled.enabled) return;
        if (!shadowEnabled.getProperty('checked')) void canvas.shadow(null);
        else void shadowPatch({});
    });
    shadowColor.on(Message.VALUE_CHANGED, event => void shadowPatch({ color: event.detail.value }));
    const shadowFields = new Column({ name: 'properties-shadow-fields', text: 'panel:tight-stack' }, [fieldRow('Color', shadowColor),
        shadowField('opacity', 'Transparency', '%', 0, 100, value => (1-value)*100, value => 1-value/100),
        shadowField('blurRadius', 'Blur', 'pt', 0, 1000, value => value*.75, value => value/.75),
        shadowField('angleDegrees', 'Angle', '°', 0, 359.99),
        shadowField('distance', 'Distance', 'pt', 0, 1000, value => value*.75, value => value/.75),
    ]);
    const tabs = new TabView({ name: 'format-properties', iconOnly: true, pages: [
        new TabPage({ id: 'fill', title: 'Fill & Line', icon: 186 }, [
            new Label({ text: 'Fill' }), fieldRow('Color', fillColor), fillRow, new Label({ text: 'Line' }), fieldRow('Color', lineColor), fieldRow('Width', lineWidth), lineRow,
        ]),
        new TabPage({ id: 'effects', title: 'Effects', icon: 570 }, [shadowEnabled, shadowFields]),
        new TabPage({ id: 'size', title: 'Size & Properties', icon: 189 }, [details, geometry, position]),
        new TabPage({ id: 'text', title: 'Text Options', icon: Icons.TextBox }, [fontPanel, textPanel]),
        new TabPage({ id: 'picture', title: 'Picture', icon: 71 }, [insert, crop]),
    ] });
    const objectPanel = new Column({ name: 'object-properties', text: 'panel:tight-stack', visible: false }, [tabs]);
    const slidePanel = new Column({ name: 'slide-properties', text: 'panel:tight-stack' }, [slideDetails]);
    const control = new Column({ name: 'selection-properties', text: 'panel:tight-stack' }, [title, slidePanel, objectPanel]);
    let fieldsTarget = null;
    return Object.freeze({ control, selectTab: id => tabs.select(id), setState({ objects = [], ready = false, busy = false, scene = null, editingText = false, scope = null }) {
        selected = objects; usable = ready && !busy;
        const picture = selected.length === 1 && Boolean(selected[0].image || selected[0].placeholder?.role === 'picture' && !selected[0].text?.text);
        const text = !canvas.selectedTableCell && (editingText || !picture && selected.length === 1 && selected[0].text != null);
        title.text = !selected.length ? 'Slide' : selected.length > 1 ? `${selected.length} objects` : picture ? 'Format Picture' : selected[0].table ? 'Table' : selected[0].kind === 'text' ? 'Text Box' : 'Format Shape';
        slidePanel.setProperty('visible', !selected.length);
        objectPanel.setProperty('visible', selected.length > 0);
        textPanel.setProperty('visible', Boolean(text));
        fontPanel.visible = Boolean(text);
        const style = selectedTextStyle(selected);
        if (!busy && !canvas.busy || selected.length) {
            const key = JSON.stringify([scope, scene?.slideId ?? null, selected.map(object => object.id).sort()]);
            const preserveDraft = key === fontSelection;
            fonts.setValue(style.mixed.includes('fontFamily') ? '' : style.fontFamily || '', { preserveDraft });
            fontSize.setValue(style.mixed.includes('fontSize') || !style.fontSize ? null : style.fontSize * 72 / 96, { preserveDraft });
            fontSelection = key;
        }
        fonts.enabled = fontSize.enabled = Boolean(usable && text);
        tabs.setPageState('text', { visible: Boolean(text) });
        tabs.setPageState('picture', { visible: Boolean(picture) });
        tabs.setPageState('effects', { enabled: Boolean(shadowsSupported && selected.length && selected.every(object => object.edit?.canSetShadow)) });
        const fieldTarget = JSON.stringify([scope, scene?.slideId, selected.map(object => object.id).sort()]);
        const preserve = { preserveDraft: fieldTarget === fieldsTarget };
        const common = read => { const value = read(selected[0]); return selected.every(object => read(object) === value) ? value : null; };
        lineWidth.setValue(common(object => object?.stroke?.width == null ? null : object.stroke.width * .75), preserve);
        lineWidth.enabled = Boolean(usable && selected.length && selected.every(object => object.edit?.canStyle));
        fillOpacity.setValue(common(object => object?.fill ? (1-object.fill.opacity)*100 : null), preserve);
        lineOpacity.setValue(common(object => object?.stroke ? (1-object.stroke.opacity)*100 : null), preserve);
        fillOpacity.enabled = Boolean(usable && selected.length && selected.every(object => object.edit?.canSetFillOpacity));
        lineOpacity.enabled = Boolean(usable && selected.length && selected.every(object => object.edit?.canSetLineOpacity));
        fillRow.visible = selected.some(object => object.edit?.canSetFillOpacity);
        lineRow.visible = selected.some(object => object.edit?.canSetLineOpacity);
        const anyShadow = selected.some(object => object.outerShadow), everyShadow = selected.every(object => object.outerShadow);
        shadowEnabled.setProperty('checked', anyShadow);
        shadowEnabled.setProperty('state', anyShadow && !everyShadow ? State.INDETERMINATE : 0);
        shadowEnabled.enabled = Boolean(usable && shadowsSupported && selected.length && selected.every(object => object.edit?.canSetShadow));
        shadowFields.visible = anyShadow; shadowColor.enabled = shadowEnabled.enabled;
        const shadowValue = (object, key) => (object?.outerShadow ?? DEFAULT_SHADOW)[key];
        const color = common(object => shadowValue(object, 'color'));
        shadowColor.setValue(color ?? DEFAULT_SHADOW.color, { mixed: color === null, ...preserve });
        for (const [key, { input, toUi }] of shadows) {
            const value = common(object => shadowValue(object, key));
            input.setValue(value === null ? null : toUi(value), preserve); input.enabled = shadowEnabled.enabled;
        }
        fieldsTarget = fieldTarget;

        for (const spec of geometrySpecs) spec.readOnly = !selected.length || selected.some(object => spec.id === 'rotation' ? !object.edit?.canRotate : !object.edit?.canTransform || object.geometry === 'line');
        const geometryTarget = JSON.stringify([scope, scene?.slideId ?? null, selected.map(object => object.id).sort()]);
        geometry.setSelection(selected.map(object => object.id), { preserveDraft: geometryTarget === geometrySelection });
        geometrySelection = geometryTarget;
        geometry.button.enabled = Boolean(usable && selected.length);
        for (const spec of geometrySpecs) geometry.form.fields.get(spec.id).input.enabled = Boolean(usable && !spec.readOnly);
        crop.setProperty('visible', Boolean(selected.length === 1 && selected[0].image));
        insert.setProperty('visible', picture && !selected[0].image);
        const box = selected.length === 1 ? selected[0].edit?.box : null;
        details.text = box ? `${(box.width / 96).toFixed(2)} × ${(box.height / 96).toFixed(2)} in` : '';
        details.setProperty('visible', Boolean(box));
        slideDetails.text = scene?.size ? `${(scene.size.width / 96).toFixed(2)} × ${(scene.size.height / 96).toFixed(2)} in` : '';
        for (const { control, available } of controls) control.setProperty('enabled', usable && available());
        if (!busy && !canvas.busy || selected.length) {
            const target = JSON.stringify([scope, scene?.slideId ?? null, selected.map(object => object.id).sort()]);
            const preserveDraft = target === paintSelection;
            for (const { colors, key } of palettes) {
                const value = selected[0]?.[key]?.color ?? null;
                if (!preserveDraft) colors.closeEditor();
                colors.setValue(value, { mixed: selected.some(object => (object[key]?.color ?? null) !== value), preserveDraft });
            }
            paintSelection = target;
        }
    } });
}
