import { Button, CheckBox, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';

const defaults = Object.freeze({ mode: 'stretch', opacity: 1, scaleX: 1, scaleY: 1, offsetX: 0, offsetY: 0 });
const number = text => /^-?(?:\d+(?:\.\d*)?|\.\d+)$/u.test(text) ? Number(text) : NaN;
const display = value => String(Math.round(value * 1e6) / 1e6);

/** Native controls edit a background resource, never a slide-sized object. */
export function createPictureBackgroundTools({ root, apply, choosePicture, notice = () => {} }) {
    let scene = null, ready = false, dialog = null, dialogSlide = null;
    const control = new MenuItem({ name: 'picture-background', text: 'Picture or texture fill…', icon: 71,
        visible: typeof choosePicture === 'function', enabled: false });
    const close = () => {
        if (dialog) root.remove(dialog, { destroy: true });
        dialog = null; dialogSlide = null;
    };
    const execute = action => { close(); return Promise.resolve().then(action).catch(error => notice(error.message)); };
    control.on(Message.ACTIVATE, () => {
        if (!ready || dialog) return;
        dialogSlide = scene.slideId;
        const source = scene.backgroundPicture, values = source || defaults;
        let allSlides = false;
        const scope = new Menu({ name: 'background-picture-scope', text: 'Current slide', icon: 74,
            state: State.START_ALIGNED | State.TRAILING_ICON, accessibleName: 'Background scope' });
        for (const [all, label] of [[false, 'Current slide'], [true, 'All slides']]) {
            const item = scope.append(new MenuItem({ name: `background-picture-scope-${all ? 'all' : 'current'}`, text: label, checked: !all }));
            item.on(Message.ACTIVATE, () => {
                allSlides = all; scope.text = label;
                for (const choice of scope.children) choice.setProperty('checked', choice === item);
            });
        }
        const error = new Label({ name: 'background-picture-error', text: '' });
        const fields = new Map();
        function field(key, label, wireToUi, uiToWire, minimum, maximum) {
            const input = new TextField({ name: `background-picture-${key}`, value: display(wireToUi(values[key])) });
            fields.set(key, { input, original: values[key], initial: input.getProperty('value'), uiToWire, minimum, maximum });
            return new Row({ text: 'panel:form-control' }, [new Label({ text: label }), input]);
        }
        const transparency = field('opacity', 'Transparency (%)', value => (1 - value) * 100, value => 1 - value / 100, 0, 1);
        const tile = new CheckBox({ name: 'background-picture-tile', text: 'Tile picture as texture', checked: values.mode === 'tile' });
        const tileRows = [
            field('scaleX', 'Width scale (%)', value => value * 100, value => value / 100, 0.01, 100),
            field('scaleY', 'Height scale (%)', value => value * 100, value => value / 100, 0.01, 100),
            field('offsetX', 'Offset X (cm)', value => value * 2.54 / 96, value => value * 96 / 2.54, -1e6, 1e6),
            field('offsetY', 'Offset Y (cm)', value => value * 2.54 / 96, value => value * 96 / 2.54, -1e6, 1e6),
        ];
        const updateTile = () => tileRows.forEach(row => row.setProperty('visible', tile.getProperty('checked')));
        tile.on(Message.ACTIVATE, updateTile); updateTile();
        const settings = () => {
            const result = { mode: tile.getProperty('checked') ? 'tile' : 'stretch' };
            for (const [key, { input, original, initial, uiToWire, minimum, maximum }] of fields) {
                const text = input.getProperty('value').trim();
                const value = result.mode === 'stretch' && key !== 'opacity' || text === initial ? original : uiToWire(number(text));
                if (!Number.isFinite(value) || value < minimum || value > maximum) {
                    error.text = key === 'opacity' ? 'Enter transparency from 0 to 100%.'
                        : key.startsWith('scale') ? 'Enter a tile scale from 1 to 10000%.' : 'Enter an offset within ±26458.333333 cm.';
                    return null;
                }
                result[key] = value;
            }
            return result;
        };
        const target = () => allSlides ? null : dialogSlide;
        const insert = new Button({ name: 'background-picture-insert', text: 'Insert Picture…', icon: 71 });
        insert.on(Message.ACTIVATE, () => {
            if (!ready) return;
            const options = settings(); if (!options) return;
            const slide = target(); void execute(() => choosePicture({ slide, ...options }));
        });
        const applyButton = new Button({ name: 'background-picture-apply', text: 'Apply', enabled: Boolean(source), accent: true });
        const applyAll = new Button({ name: 'background-picture-apply-all', text: 'Apply to All', enabled: Boolean(source) });
        const update = all => {
            if (!ready || !source) return;
            const options = settings(); if (!options) return;
            const slide = all ? null : target();
            void execute(() => apply([{ type: 'set-slide-picture-background', slide, asset: source.image.asset, ...options }]));
        };
        applyButton.on(Message.ACTIVATE, () => update(false)); applyAll.on(Message.ACTIVATE, () => update(true));
        const reset = new Button({ name: 'background-picture-reset', text: 'Reset Background', icon: 21 });
        reset.on(Message.ACTIVATE, () => {
            if (!ready) return;
            const slide = target(); void execute(() => apply([{ type: 'reset-slide-background', slide }]));
        });
        const cancel = new Button({ name: 'background-picture-close', text: 'Close' }); cancel.on(Message.ACTIVATE, close);
        dialog = new Dialog({ name: 'picture-background-dialog', text: 'Picture or texture fill', modal: true }, [
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Apply to' }), scope]),
            new Row({}, [insert]), transparency, tile, ...tileRows,
            new Label({ text: 'Untiled pictures stretch to the slide. Reset restores the inherited background.' }), error,
            new Row({ text: 'panel:actions' }, [reset, cancel, applyAll, applyButton]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    });
    return Object.freeze({ control, close, setState({ current, usable }) {
        scene = current; ready = Boolean(usable && scene?.canSetBackground && typeof choosePicture === 'function');
        control.setProperty('enabled', ready);
        if (dialog && scene?.slideId !== dialogSlide) close();
    } });
}
