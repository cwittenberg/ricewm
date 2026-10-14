import { createPictureBackgroundTools } from './picture-background-tools.mjs';
import { Button, ColorPalette, Dialog, Label, Menu, MenuItem, Message, Row, ScrollView, State, TextField } from '@ricewm/app';

/** Presentation-wide page size and source-backed slide backgrounds. */
export function createDesignTools({ root, apply, backgroundPicture, notice = () => {} }) {
    let scene = null, usable = false, dialog = null;
    const size = new Menu({ name: 'slide-size', text: '', icon: 189, state: State.DISPLAY,
        tooltip: 'Slide size', accessibleName: 'Slide size', enabled: false });
    const execute = commands => Promise.resolve(apply(commands)).catch(error => notice(error.message));
    const presets = [
        ['standard', 'Standard (4:3)', 960, 720],
        ['wide', 'Widescreen (16:9)', 1280, 720],
    ];
    for (const [name, label, width, height] of presets) {
        const item = size.append(new MenuItem({ name: `slide-size-${name}`, text: label }));
        item.on(Message.ACTIVATE, () => { if (size.getProperty('enabled')) void execute([{ type: 'set-slide-size', width, height }]); });
    }
    const custom = size.append(new MenuItem({ name: 'slide-size-custom', text: 'Custom slide size…' }));
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    custom.on(Message.ACTIVATE, () => {
        if (!size.getProperty('enabled') || !scene?.size || dialog) return;
        const width = new TextField({ name: 'slide-width', value: String(Math.round(scene.size.width * 2.54 / 96 * 1000) / 1000) });
        const height = new TextField({ name: 'slide-height', value: String(Math.round(scene.size.height * 2.54 / 96 * 1000) / 1000) });
        const error = new Label({ name: 'slide-size-error', text: '' });
        const save = new Button({ name: 'apply-slide-size', text: 'Apply', accent: true });
        const cancel = new Button({ text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        save.on(Message.ACTIVATE, () => {
            const values = [width, height].map(field => field.getProperty('value').trim());
            if (!values.every(value => /^(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value) && Number(value) >= 0.1 && Number(value) <= 142.24)) {
                error.text = 'Enter dimensions from 0.1 to 142.24 cm.'; return;
            }
            close(); void execute([{ type: 'set-slide-size', width: Number(values[0]) * 96 / 2.54, height: Number(values[1]) * 96 / 2.54 }]);
        });
        dialog = new Dialog({ name: 'slide-size-dialog', text: 'Slide size', modal: true }, [
            new Row({}, [new Label({ text: 'Width (cm)' }), width]),
            new Row({}, [new Label({ text: 'Height (cm)' }), height]),
            new Label({ text: 'Applies to every slide. Objects keep their size and position.' }), error,
            new Row({ text: 'panel:actions' }, [cancel, save]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog); width.setProperty('focused', true);
    });
    const pictures = createPictureBackgroundTools({ root, apply, choosePicture: backgroundPicture, notice });
    const colors = new ColorPalette({ name: 'slide-background-palette', allowNone: false, moreColors: true });
    const scroll = new ScrollView({ name: 'slide-background-scroll', text: 'viewport:360|0/0' }, [colors]);
    colors.moreButton.on(Message.ACTIVATE, () => { if (colors.editor.getProperty('visible')) scroll.scrollTo(colors.editor); });
    const all = new Button({ name: 'background-apply-all', text: 'Apply to all slides', enabled: false });
    const background = new Menu({ name: 'format-background', text: '', icon: 186, state: State.DISPLAY,
        tooltip: 'Format background', accessibleName: 'Format background', enabled: false }, [pictures.control, scroll, all]);
    colors.on(Message.VALUE_CHANGED, event => {
        if (background.getProperty('enabled')) void execute([{ type: 'set-slide-background', slide: scene.slideId, color: event.detail.value }]);
    });
    all.on(Message.ACTIVATE, () => {
        if (all.getProperty('enabled')) void execute([{ type: 'set-slide-background', slide: null, color: scene.background.color }]);
    });
    return Object.freeze({ controls: [size, background], setState({ current = scene, ready = usable } = {}) {
        scene = current; usable = ready;
        pictures.setState({ current: scene, usable });
        size.setProperty('enabled', Boolean(usable && scene?.canSetSize));
        background.setProperty('enabled', Boolean(usable && scene?.canSetBackground));
        all.setProperty('enabled', Boolean(usable && scene?.canSetBackground && !scene?.backgroundPicture && !scene?.backgroundGradient && scene?.background?.opacity === 1));
        colors.setValue(scene?.background?.color || '#FFFFFF', { mixed: !scene?.background || Boolean(scene?.backgroundPicture || scene?.backgroundGradient) });
        for (const [name, , width, height] of presets) size.find(`slide-size-${name}`).setProperty('checked', Boolean(scene?.size && Math.abs(scene.size.width / scene.size.height - width / height) < 0.0001));
    } });
}
