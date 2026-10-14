import { Button, ColorPalette, Column, Label, Menu, Message, Row, ScrollView, State } from '@ricewm/app';

/** A selection-aware native inspector. Commands use the same canvas adapter as
 * the ribbon; the inspector never owns a second copy of document state. */
export function createSelectionProperties({ canvas, tools, textPanel }) {
    let selected = [], usable = false;
    const title = new Label({ name: 'properties-heading', text: 'Slide' });
    const details = new Label({ name: 'selection-dimensions', text: '' });
    const slideDetails = new Label({ name: 'slide-dimensions', text: '' });
    const controls = [];
    const command = (name, label, icon, callback, available) => {
        const button = new Button({ name, text: label, icon, state: State.START_ALIGNED });
        button.on(Message.ACTIVATE, () => { if (button.getProperty('enabled')) void callback(); });
        controls.push({ control: button, available }); return button;
    };
    const palettes = [];
    const palette = (name, label, icon, key, callback) => {
        const colors = new ColorPalette({ name: `${name}-palette`, allowNone: true,
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
    const objectPanel = new Column({ name: 'object-properties', text: 'panel:tight-stack', visible: false }, [
        details, appearance, position, tools.rotationProperties, insert, crop,
    ]);
    const slidePanel = new Column({ name: 'slide-properties', text: 'panel:tight-stack' }, [slideDetails]);
    const control = new Column({ name: 'selection-properties', text: 'panel:tight-stack' }, [title, slidePanel, objectPanel, textPanel]);
    return Object.freeze({ control, setState({ objects = [], ready = false, scene = null, editingText = false }) {
        selected = objects; usable = ready;
        const picture = selected.length === 1 && Boolean(selected[0].image || selected[0].placeholder?.role === 'picture' && !selected[0].text?.text);
        const text = !canvas.selectedTableCell && (editingText || !picture && selected.length === 1 && selected[0].text != null);
        title.text = !selected.length ? 'Slide' : selected.length > 1 ? `${selected.length} objects` : picture ? 'Format Picture' : selected[0].table ? 'Table' : selected[0].kind === 'text' ? 'Text Box' : 'Format Shape';
        slidePanel.setProperty('visible', !selected.length);
        objectPanel.setProperty('visible', selected.length > 0);
        textPanel.setProperty('visible', Boolean(text));
        crop.setProperty('visible', Boolean(selected.length === 1 && selected[0].image));
        insert.setProperty('visible', picture && !selected[0].image);
        const box = selected.length === 1 ? selected[0].edit?.box : null;
        details.text = box ? `${(box.width * 2.54 / 96).toFixed(2)} × ${(box.height * 2.54 / 96).toFixed(2)} cm` : '';
        details.setProperty('visible', Boolean(box));
        slideDetails.text = scene?.size ? `${(scene.size.width * 2.54 / 96).toFixed(2)} × ${(scene.size.height * 2.54 / 96).toFixed(2)} cm` : '';
        for (const { control, available } of controls) control.setProperty('enabled', usable && available());
        for (const { colors, key } of palettes) {
            const value = selected[0]?.[key]?.color ?? null;
            colors.setValue(value, { mixed: selected.some(object => (object[key]?.color ?? null) !== value) });
        }
    } });
}
