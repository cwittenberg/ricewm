import {createDesignThemeTools} from './design-theme-tools.mjs';
import {createBackgroundStyles} from './background-styles.mjs';
import { createBackgroundProperties } from './background-properties.mjs';
import { createPictureBackgroundTools } from './picture-background-tools.mjs';
import { Button, Dialog, DropDownList, Label, Menu, MenuItem, Message, PropertyInspector, RadioButton, Row, ScrollView, State } from '@ricewm/app';

// The original 0.8.0 SLIDE_SIZES order and point dimensions, converted only at
// the view boundary. Standard paper labels intentionally retain the reference's
// slide dimensions rather than substituting physical sheet measurements.
// https://github.com/storytold/deckcraft/blob/22db9261f9d1a214c296fd390173130c45d76d66/crates/model/src/defaults.rs
export const SLIDE_SIZE_PRESETS = Object.freeze([
    ['Widescreen (16:9)', 960, 540], ['Standard (4:3)', 720, 540], ['On-screen Show (16:10)', 720, 450],
    ['Letter Paper (8.5x11 in)', 720, 540], ['Ledger Paper (11x17 in)', 960, 720], ['A3 Paper (297x420 mm)', 1008, 756],
    ['A4 Paper (210x297 mm)', 780, 540], ['B4 (ISO) Paper (250x353 mm)', 852, 639], ['B5 (ISO) Paper (176x250 mm)', 598, 448.5],
    ['35mm Slides', 810, 540], ['Overhead', 720, 540], ['Banner', 576, 72], ['Widescreen (13.333x7.5 in)', 960, 540],
    ['Square (1:1)', 540, 540], ['Portrait (9:16)', 540, 960],
].map(Object.freeze));

/** Presentation-wide page size and source-backed slide backgrounds. */
export function createDesignTools({ root, apply, backgroundPicture, importTheme=null, openBackground = () => {}, closeBackground = () => {}, notice = () => {} }) {
    let scene = null, usable = false, dialog = null, sizeSession = null, documentScope = null;
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
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; sizeSession = null; };
    const updateSizeDialog = () => {
        if (!sizeSession) return;
        const ready = Boolean(usable && scene?.canSetSize && sizeSession.scope === documentScope);
        sizeSession.inspector.enabled = ready && !sizeSession.applying;
        sizeSession.save.enabled = ready && !sizeSession.pending;
        sizeSession.cancel.enabled = !sizeSession.pending;
        for (const control of sizeSession.choices) control.enabled = ready && !sizeSession.pending;
    };
    custom.on(Message.ACTIVATE, () => {
        if (!size.getProperty('enabled') || !scene?.size || dialog) return;
        const original = { width: scene.size.width, height: scene.size.height };
        const displayed = Object.fromEntries(Object.entries(original).map(([id, value]) => [id, Math.round(value / 96 * 100) / 100]));
        const session = { scope: documentScope, pending: false, applying: false };
        const inspector = new PropertyInspector({ name: 'slide-size-properties', selection: ['presentation'],
            properties: ['width', 'height'].map(id => ({ id, label: id === 'width' ? 'Width' : 'Height', type: 'number', unit: 'in',
                precision: 2, step: .01, minimum: 1, maximum: 56 })),
            read: (_, id) => original[id] / 96,
            apply: async ({ values }) => {
                if (sizeSession !== session || session.scope !== documentScope || !scene?.canSetSize || !usable) throw Error('Presentation changed. Reopen Slide size.');
                if (scene.size?.width !== original.width || scene.size?.height !== original.height) throw Error('Slide size changed. Reopen Slide size before applying this draft.');
                // Unchanged dimensions retain exact source pixels; displayed
                // display rounding must never silently resize the other axis.
                const command = { type: 'set-slide-size', width: original.width, height: original.height };
                for (const id of ['width', 'height']) if (Object.hasOwn(values, id) && values[id] !== displayed[id]) command[id] = values[id] * 96;
                if (command.width === original.width && command.height === original.height) return true;
                session.applying = true; updateSizeDialog();
                try { return await apply([command]); }
                finally { session.applying = false; }
            },
        });
        inspector.button.visible = false; inspector.status.visible = false;
        for (const id of ['width', 'height']) {
            const editor = inspector.form.fields.get(id).input;
            editor.setProperty('name', `slide-${id}`); editor.input.setProperty('name', `slide-${id}-input`);
            editor.input.on(Message.KEY, event => {
                if (event.detail?.pressed !== false && (event.detail?.key === 'Escape' || event.detail?.keyCode === 9) && !editor.dirty) {
                    event.preventDefault(); event.stopPropagation(); if (!session.pending) close();
                }
            });
        }
        const error = new Label({ name: 'slide-size-error', text: '' });
        const setDimensions = (width, height) => {
            for (const [id, value] of [['width', width], ['height', height]]) {
                inspector.form.fields.get(id).input.setValue(value);
                inspector.model.set(id, value);
            }
            error.text = '';
        };
        const preset = new DropDownList({ name: 'slide-size-preset', accessibleName: 'Slides sized for',
            items: SLIDE_SIZE_PRESETS.map(([label]) => label), value: '', emptyLabel: 'Custom',
            onCommit: label => {
                const match = SLIDE_SIZE_PRESETS.find(([name]) => name === label);
                if (!match) return true;
                setDimensions(match[1] / 72, match[2] / 72);
                return true;
            } });
        const portrait = new RadioButton({ name: 'slide-size-portrait', text: 'Portrait', checked: displayed.height > displayed.width });
        const landscape = new RadioButton({ name: 'slide-size-landscape', text: 'Landscape', checked: displayed.width >= displayed.height });
        const updateOrientation = () => {
            const width = Number(inspector.model.get('width')), height = Number(inspector.model.get('height'));
            if (!Number.isFinite(width) || !Number.isFinite(height)) return;
            portrait.setProperty('checked', height > width); landscape.setProperty('checked', width >= height);
        };
        for (const [control, vertical] of [[portrait, true], [landscape, false]]) control.on(Message.ACTIVATE, () => {
            if (!control.enabled || session.pending) return false;
            try {
                const value = id => { const field = inspector.form.fields.get(id).input; return field.dirty ? field.read() : field.value; };
                const width = value('width'), height = value('height');
                if (vertical ? width > height : height > width) setDimensions(height, width);
                updateOrientation(); return true;
            } catch (failure) { error.text = failure.message; updateOrientation(); return false; }
        });
        // The reference retains the last chosen preset caption while its
        // dimensions are edited. Only the dimensions are persisted.
        const stop = inspector.model.subscribe(updateOrientation);
        inspector.on(Message.DESTROY, stop);
        const save = new Button({ name: 'apply-slide-size', text: 'Apply', accent: true });
        const cancel = new Button({ text: 'Cancel' }); cancel.on(Message.ACTIVATE, () => { if (cancel.enabled && !session.pending && sizeSession === session) close(); });
        Object.assign(session, { inspector, save, cancel, choices: [preset, portrait, landscape] }); sizeSession = session;
        save.on(Message.ACTIVATE, async () => {
            if (sizeSession !== session || !save.enabled || session.pending) return false;
            session.pending = true; error.text = '';
            const pending = inspector.commit(); updateSizeDialog();
            try {
                const accepted = await pending;
                if (sizeSession === session && (accepted || !inspector.changedIds.size)) close();
                return accepted;
            } catch (failure) {
                if (sizeSession === session) error.text = failure.message || 'Slide size could not be applied';
                return false;
            } finally { session.pending = false; updateSizeDialog(); }
        });
        dialog = new Dialog({ name: 'slide-size-dialog', text: 'Slide size', modal: true }, [
            new Label({ text: 'Slides sized for:' }), preset,
            inspector,
            new Row({}, [new Label({ text: 'Orientation:' }), portrait, landscape]),
            new Label({ text: 'Applies to every slide. Objects keep their size and position.' }), error,
            new Row({ text: 'panel:actions' }, [cancel, save]),
        ]);
        dialog.on(Message.KEY, event => {
            if (event.detail?.pressed !== false && (event.detail?.key === 'Escape' || event.detail?.keyCode === 9)) {
                event.preventDefault(); event.stopPropagation(); if (!session.pending) close();
            }
        });
        root.append(dialog); inspector.form.fields.get('width').input.input.setProperty('focused', true);
    });
    const pictures = createPictureBackgroundTools({ root, apply, choosePicture: backgroundPicture, notice });
    const background = new Button({ name: 'format-background', text: '', icon: 186, state: State.DISPLAY,
        tooltip: 'Format background', accessibleName: 'Format background', enabled: false });
    const styles=createBackgroundStyles({apply,notice});
    const themes=createDesignThemeTools({root,apply,notice,browseTheme:typeof importTheme==='function'?()=>importTheme({slide:scene?.slideId}):null});
    styles.control.text='Background\nStyles';size.text='Slide\nSize';background.text='Format\nBackground';
    const properties = createBackgroundProperties({ apply, choosePicture: backgroundPicture, notice, close: closeBackground });
    // Keep the advanced modal available while all ordinary background edits use
    // the retained task pane, including the canvas context-menu entrypoint.
    pictures.control.text = 'Advanced picture options…';
    properties.body.append(pictures.control);
    const open = () => { if (usable && scene) openBackground(); };
    background.on(Message.ACTIVATE, open);
    return Object.freeze({ controls: [themes.themes,themes.variantGallery,...themes.variantCommands,styles.control,size,background], groups:[...themes.groups,{id:'theme-options',title:'Variants',controls:[...themes.variantCommands,styles.control],primary:4},{id:'customize',title:'Customize',controls:[size,background],primary:2}], pane: properties.control, openBackground: open, setState({ current = scene, ready = usable, scope = documentScope } = {}) {
        if (scope !== documentScope) close();
        documentScope = scope;
        scene = current; usable = ready;
        updateSizeDialog();
        pictures.setState({ current: scene, usable });
        styles.setState({current:scene,usable});
        properties.setState({ current: scene, usable, documentScope });
        themes.setState({current:scene,usable,documentScope});
        size.setProperty('enabled', Boolean(usable && scene?.canSetSize));
        background.setProperty('enabled', Boolean(usable && scene));
        for (const [name, , width, height] of presets) size.find(`slide-size-${name}`).setProperty('checked', Boolean(scene?.size && Math.abs(scene.size.width / scene.size.height - width / height) < 0.0001));
    } });
}
