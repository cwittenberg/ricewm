import { Button, Column, Dialog, Label, Menu, MenuItem, Message, Row, State, TextField } from '@ricewm/app';

/** Native controls share one source-backed rotation command with the canvas handle. */
export function createRotationTools({ root, canvas, notice = () => {} }) {
    let enabled = false, selection = [], signature = null, dialog = null;
    const menu = new Menu({ name: 'object-rotation', text: '', icon: 195, state: State.DISPLAY,
        tooltip: 'Rotate', accessibleName: 'Rotate', enabled: false });
    const angle = new TextField({ name: 'object-rotation-angle', value: '0', placeholder: 'Mixed',
        state: State.EXPANSIVE, tooltip: 'Clockwise rotation in degrees', accessibleName: 'Rotation in degrees', enabled: false });
    const apply = new Button({ name: 'apply-object-rotation', text: 'Apply', enabled: false });
    const properties = new Column({ name: 'object-rotation-properties', text: 'panel:tight-stack' }, [
        new Label({ text: 'Rotation (°)' }), new Row({}, [angle, apply]),
    ]);
    const run = (degrees, relative = false) => {
        if (!enabled) return;
        return Promise.resolve(canvas.rotate(degrees, relative)).catch(error => notice(error.message));
    };
    const read = field => {
        const value = field.getProperty('value').trim();
        if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/u.test(value) || Math.abs(Number(value)) > 36000) {
            notice('Enter a rotation between −36000° and 36000°.'); return null;
        }
        return Number(value);
    };
    const applyAngle = () => { const degrees = read(angle); if (degrees !== null) void run(degrees); };
    apply.on(Message.ACTIVATE, applyAngle);
    angle.on(Message.ACTIVATE, applyAngle);
    for (const [name, label, degrees, relative] of [
        ['right', 'Rotate right 90°', 90, true], ['left', 'Rotate left 90°', -90, true], ['reset', 'Reset rotation', 0, false],
    ]) {
        const item = menu.append(new MenuItem({ name: `rotate-${name}`, text: label }));
        item.on(Message.ACTIVATE, () => { void run(degrees, relative); });
    }
    const more = menu.append(new MenuItem({ name: 'rotate-angle', text: 'Rotation angle…' }));
    const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
    more.on(Message.ACTIVATE, () => {
        if (!enabled || dialog) return;
        const input = new TextField({ name: 'rotation-dialog-angle', value: angle.getProperty('value'), placeholder: 'Mixed', accessibleName: 'Rotation in degrees' });
        const save = new Button({ name: 'rotation-dialog-apply', text: 'Apply', accent: true });
        const cancel = new Button({ text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        save.on(Message.ACTIVATE, () => { const value = read(input); if (value !== null) { close(); void run(value); } });
        dialog = new Dialog({ name: 'rotation-dialog', text: 'Rotation', modal: true }, [
            new Row({}, [new Label({ text: 'Clockwise angle (°)' }), input]),
            new Row({ text: 'panel:actions' }, [cancel, save]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog); input.setProperty('focused', true);
    });
    return Object.freeze({ controls: [menu], properties, setState({ selected, usable }) {
        selection = selected;
        enabled = Boolean(usable && selection.length && selection.every(object => object.edit?.canRotate));
        for (const control of [menu, angle, apply]) control.setProperty('enabled', enabled);
        properties.setProperty('visible', selection.length > 0 && selection.every(object => object.edit?.canRotate));
        const next = JSON.stringify(selection.map(object => [object.id, object.edit?.rotationDegrees]));
        if (next !== signature) {
            signature = next;
            const degrees = selection[0]?.edit?.rotationDegrees || 0;
            angle.setProperty('value', selection.every(object => (object.edit?.rotationDegrees || 0) === degrees) ? String(Math.round(degrees * 100) / 100) : '');
        }
    } });
}
