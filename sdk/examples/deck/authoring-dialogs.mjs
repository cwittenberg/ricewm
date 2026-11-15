import { Button, Dialog, GeometryEditor, Label, Message, Row, TextField } from '@ricewm/app';
import { canResize } from './canvas-geometry.mjs';

export function createAuthoringDialogs({root, canvas, notice, selected, context = () => null}) {
    let dialog = null, positionSession = null;
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
    function linePositionDialog() {
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
    function positionDialog() {
        const targets = selected(), object = targets[0];
        if (targets.length !== 1 || !object?.edit?.canTransform || !object.edit.box || dialog) return;
        if (object.geometry === 'line') return linePositionDialog();
        const original = { ...object.edit.box, rotation: object.edit.rotationDegrees || 0 };
        const scope = JSON.stringify(context());
        const session = { id: object.id, scope, pending: false };
        const close = () => {
            if (positionSession !== session) return;
            root.remove(session.dialog, { destroy: true }); dialog = null; positionSession = null;
        };
        const current = () => {
            const objects = selected();
            if (JSON.stringify(context()) !== scope || objects.length !== 1 || objects[0].id !== object.id)
                throw Error('Selection changed. Reopen Size and Position.');
            const next = { ...objects[0].edit?.box, rotation: objects[0].edit?.rotationDegrees || 0 };
            if (Object.keys(original).some(key => next[key] !== original[key]))
                throw Error('Object geometry changed. Reopen Size and Position before applying this draft.');
        };
        const editor = new GeometryEditor({ name: 'object-geometry',
            value: { ...Object.fromEntries(['x', 'y', 'width', 'height'].map(key => [key, original[key] / 96])), rotation: original.rotation },
            unit: 'in', allowResize: canResize(object), allowRotation: Boolean(object.edit.canRotate),
            originLabel: 'From the top-left corner of the slide',
            onCommit: async ({ value, changed }) => {
                current();
                const values = Object.fromEntries(changed.map(key => [key, key === 'rotation' ? value[key] : value[key] * 96]));
                if (!await canvas.setProperties({ ids: [object.id], values })) throw Error('The editor is busy. Try again.');
                return true;
            },
        });
        const apply = new Button({ name: 'apply-object-position', text: 'OK', accent: true });
        const cancel = new Button({ name: 'cancel-object-position', text: 'Cancel' });
        cancel.on(Message.ACTIVATE, () => { if (!session.pending) close(); });
        const applyDraft = async () => {
            if (positionSession !== session || session.pending) return false;
            session.pending = true; apply.enabled = false; cancel.enabled = false;
            try {
                const accepted = await editor.commit();
                if (accepted) close();
                return accepted;
            } finally {
                session.pending = false;
                if (positionSession === session) { apply.enabled = true; cancel.enabled = true; }
            }
        };
        apply.on(Message.ACTIVATE, applyDraft);
        dialog = new Dialog({ name: 'object-position-dialog', text: 'Size and Position', modal: true }, [
            editor, new Row({ text: 'panel:actions' }, [cancel, apply]),
        ]);
        Object.assign(session, { dialog, editor }); positionSession = session;
        dialog.on(Message.KEY, event => {
            if (event.detail?.pressed === false) return;
            if (event.detail?.key === 'Escape' || event.detail?.keyCode === 9) {
                event.preventDefault(); event.stopPropagation(); if (!session.pending) close();
            } else if (event.detail?.key === 'Enter' || event.detail?.keyCode === 36) {
                event.preventDefault(); event.stopPropagation(); return applyDraft();
            }
        });
        root.append(dialog);
    }
    return { crop: cropDialog, position: positionDialog, update() {
        const session = positionSession;
        if (!session) return;
        const objects = selected();
        if (JSON.stringify(context()) !== session.scope || objects.length !== 1 || objects[0].id !== session.id) {
            root.remove(session.dialog, { destroy: true }); dialog = null; positionSession = null;
        }
    } };
}
