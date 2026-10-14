import { shadowSigma as sigma } from './shadow-render.mjs';
import { Button, CheckBox, ColorMenu, Column, Container, Dialog, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State } from '@ricewm/app';
import { TextField } from '@ricewm/app';
import { DEFAULT_SHADOW, validShadow } from './shadow-contract.mjs';

const presets = [['Top left', 225], ['Top', 270], ['Top right', 315], ['Left', 180], ['Center', null],
    ['Right', 0], ['Bottom left', 135], ['Bottom', 90], ['Bottom right', 45]];
const display = value => String(Math.round(value * 1e6) / 1e6);
const number = value => /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value.trim()) ? Number(value) : NaN;

/** Common shadow choices and precise options share one source-backed command.
 * Options patch only fields actually changed, preserving mixed selections. */
export function createShadowTools({ root, canvas, name = 'shape-shadow', notice = () => {} }) {
    let selection = [], ready = false, pending = false, dialog = null, signature = '', dialogSignature = null;
    const content = new Column({ text: 'panel:fixed:gap=4:flush' });
    const control = new Menu({ name, text: 'Shadow', icon: 570, state: State.START_ALIGNED,
        accessibleName: 'Shadow', tooltip: 'Object shadow', enabled: false }, [
        new ScrollView({ text: 'viewport:344|0/0', state: State.FLAT }, [content]),
    ]);
    const enabled = () => ready && !pending && selection.length > 0 && selection.every(object => object.edit?.canSetShadow) && typeof canvas.shadow === 'function';
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; dialogSignature = null; };
    const update = () => control.setProperty('enabled', Boolean(enabled()));
    function execute(value) {
        if (!enabled()) return;
        const expected = signature;
        pending = true; update(); close();
        Promise.resolve().then(() => { if (signature === expected) return canvas.shadow(value); })
            .catch(error => notice(error.message)).finally(() => { pending = false; update(); });
    }
    const portable = object => ({ ...DEFAULT_SHADOW, rotateWithShape: object.edit?.canTransformShadow !== false });
    const none = content.append(new MenuItem({ name: `${name}-none`, text: 'No shadow' }));
    none.on(Message.ACTIVATE, () => execute(null));
    for (let start = 0; start < presets.length; start += 3) {
        const row = content.append(new Row({ text: 'panel:fixed:gap=4:flush' }));
        for (let index = start; index < start + 3; index++) {
            const [label, angle] = presets[index], radians = (angle || 0) * Math.PI / 180;
            const sample = new SceneSurface({ enabled: false });
            const builder = new SceneBuilder(116, 54).rect([0, 0, 116, 54], 0xffffffff);
            builder.pushDropShadow([36, 10, 40, 28], 0x66000000, {
                offset: angle === null ? [0, 0] : [5 * Math.cos(radians), 5 * Math.sin(radians)], blur: sigma(6),
            }).rect([36, 10, 40, 28], 0xffc27a32).popDropShadow();
            sample.submit(builder.finish());
            const item = row.append(new MenuItem({ name: `${name}-preset-${index}`, text: 'panel:fixed-height:82:width=128:gap=2:flush',
                accessibleName: label, tooltip: label }, [new Container({ text: 'panel:fixed-height:54:flush', state: State.FLAT }, [sample]), new Label({ text: label })]));
            item.on(Message.ACTIVATE, () => execute(object => ({ ...portable(object), opacity: .4, blurRadius: 6,
                distance: angle === null ? 0 : 4, angleDegrees: angle || 0 })));
        }
    }
    const options = content.append(new MenuItem({ name: `${name}-options`, text: 'Shadow Options…', icon: 50 }));
    options.on(Message.ACTIVATE, () => {
        if (!enabled() || dialog) return;
        dialogSignature = signature;
        const values = selection.map(object => object.outerShadow || portable(object));
        const fields = [], rows = [], error = new Label({ name: `${name}-error`, text: '' });
        let colorChanged = false, color = values[0].color;
        const colors = new ColorMenu({ name: `${name}-color`, label: 'Shadow color', icon: 186, allowNone: false });
        colors.setValue(color, { selected: true, mixed: !values.every(value => value.color === color) });
        colors.on(Message.VALUE_CHANGED, event => { colorChanged = true; color = event.detail.value; });
        rows.push(new Row({ text: 'panel:form-control' }, [new Label({ text: 'Color' }), colors]));
        function field(key, label, toUi = value => value, fromUi = value => value) {
            const common = values.every(value => value[key] === values[0][key]);
            const initial = common ? display(toUi(values[0][key])) : '';
            const input = new TextField({ name: `${name}-${key}`, value: initial, placeholder: 'Mixed' });
            fields.push({ key, input, initial, fromUi });
            rows.push(new Row({ text: 'panel:form-control' }, [new Label({ text: label }), input]));
        }
        field('opacity', 'Transparency (%)', value => (1 - value) * 100, value => 1 - value / 100);
        field('blurRadius', 'Blur (pt)', value => value * .75, value => value / .75);
        field('angleDegrees', 'Angle (°)', value => value, value => ((value % 360) + 360) % 360);
        field('distance', 'Distance (pt)', value => value * .75, value => value / .75);
        const canTransform = selection.every(object => object.edit?.canTransformShadow);
        let rotate = null, rotateChanged = false;
        if (canTransform) {
            field('scaleX', 'Width (%)', value => value * 100, value => value / 100);
            field('scaleY', 'Height (%)', value => value * 100, value => value / 100);
            rotate = new CheckBox({ name: `${name}-rotate`, text: 'Rotate with shape', checked: values[0].rotateWithShape,
                state: values.every(value => value.rotateWithShape === values[0].rotateWithShape) ? 0 : State.INDETERMINATE });
            rotate.on(Message.ACTIVATE, () => { rotateChanged = true; rotate.setProperty('state', 0); });
            rows.push(rotate);
        }
        const apply = new Button({ name: `${name}-apply`, text: 'Apply', accent: true });
        const cancel = new Button({ name: `${name}-cancel`, text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        apply.on(Message.ACTIVATE, () => {
            if (!enabled() || signature !== dialogSignature) return;
            const patch = {};
            if (colorChanged) patch.color = color;
            for (const { key, input, initial, fromUi } of fields) {
                const text = input.getProperty('value');
                if (text === initial) continue;
                const value = number(text);
                if (!Number.isFinite(value)) { error.text = 'Enter a valid number for each changed setting.'; return; }
                patch[key] = fromUi(value);
            }
            if (rotateChanged) patch.rotateWithShape = rotate.getProperty('checked');
            if (!Object.keys(patch).length) { close(); return; }
            if (!values.every(value => validShadow({ ...value, ...patch }))) {
                error.text = 'Check transparency, blur, distance and size values.'; return;
            }
            execute(object => ({ ...(object.outerShadow || portable(object)), ...patch }));
        });
        dialog = new Dialog({ name: `${name}-dialog`, text: 'Shadow', modal: true }, [
            ...rows, error, new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    });
    return Object.freeze({ control, close, setState({ selected, usable }) {
        selection = selected; ready = usable;
        signature = JSON.stringify(selection.map(object => [object.id, object.outerShadow, object.edit?.canSetShadow, object.edit?.canTransformShadow]));
        if (dialog && (signature !== dialogSignature || !ready)) close();
        none.setProperty('checked', selection.length > 0 && selection.every(object => !object.outerShadow));
        update();
    } });
}
