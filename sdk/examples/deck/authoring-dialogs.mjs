import { Button, Dialog, Label, Message, Row, TextField } from '@ricewm/app';
import { canResize } from './canvas-geometry.mjs';

export function createAuthoringDialogs({root, canvas, notice, selected}) {
    let dialog = null;
    function cropDialog() {
        const object = selected()[0];
        if (!object?.image?.canCrop || dialog) return;
        const fields = new Map();
        const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
        const apply = new Button({ name: 'apply-picture-crop', text: 'Apply crop', accent: true });
        const cancel = new Button({ name: 'cancel-picture-crop', text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        const error = new Label({ name: 'crop-error', text: '' });
        dialog = new Dialog({ name: 'picture-crop-dialog', text: 'Crop picture', modal: true }, [
            new Label({ text: 'Trim each edge as a percentage of the original picture. The picture frame stays in place.' }),
            ...['left', 'top', 'right', 'bottom'].map(key => {
                const field = new TextField({ name: `crop-${key}`, value: String(Math.round(object.image.crop[key] * 10000) / 100) });
                fields.set(key, field); return new Row({}, [new Label({ text: `${key[0].toUpperCase() + key.slice(1)} (%)` }), field]);
            }), error, new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        apply.on(Message.ACTIVATE, () => {
            const crop = Object.fromEntries([...fields].map(([key, field]) => [key, Number(field.getProperty('value')) / 100]));
            if (!Object.values(crop).every(value => Number.isFinite(value) && value >= 0 && value < 1) || crop.left + crop.right >= 1 || crop.top + crop.bottom >= 1) {
                error.text = 'Use values from 0 to below 100; opposite edges must total less than 100%.'; return;
            }
            close(); void canvas.crop(crop);
        });
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    }
    function positionDialog() {
        const object = selected()[0];
        if (!object?.edit?.canTransform || !object.edit.box || dialog) return;
        const fields = new Map();
        const close = () => { if (dialog) root.remove(dialog, { destroy: true }); dialog = null; };
        const apply = new Button({ name: 'apply-object-position', text: 'Apply', accent: true });
        const cancel = new Button({ text: 'Cancel' }); cancel.on(Message.ACTIVATE, close);
        dialog = new Dialog({ name: 'object-position-dialog', text: canResize(object) ? 'Position and size' : 'Position', modal: true }, [
            ...[['X', 'x'], ['Y', 'y'], ...(canResize(object) ? [['Width', 'width'], ['Height', 'height']] : [])].map(([label, key]) => {
                const field = new TextField({ name: `object-${key}`, value: (object.edit.box[key] * 2.54 / 96).toFixed(2) });
                fields.set(key, field); return new Row({}, [new Label({ text: `${label} (cm)` }), field]);
            }), new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        apply.on(Message.ACTIVATE, () => {
            const box = { ...object.edit.box, ...Object.fromEntries([...fields].map(([key, field]) => {
                const value = field.getProperty('value');
                return [key, value === (object.edit.box[key] * 2.54 / 96).toFixed(2) ? object.edit.box[key] : Number(value) * 96 / 2.54];
            })) };
            if (!Object.values(box).every(Number.isFinite) || box.width <= 0 || box.height <= 0) { notice('Enter valid dimensions greater than zero.'); return; }
            close(); void canvas.resize(box);
        });
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) close(); });
        root.append(dialog);
    }
    return { crop: cropDialog, position: positionDialog };
}
