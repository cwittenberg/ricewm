import { checkedObjectCommand } from './authoring-contract.mjs';
import { checkedShadowCommand } from './shadow-contract.mjs';
const textKeys = ['fontFamily', 'fontSize', 'bold', 'italic', 'underline', 'color', 'align'];
const layoutKeys = ['left', 'top', 'right', 'bottom', 'vertical', 'wrap'];
const pick = (value, keys) => Object.fromEntries(keys.filter(key => value?.[key] != null).map(key => [key, value[key]]));
const opaque = paint => paint == null || paint.opacity == null || paint.opacity === 1;
const spacingKeys = ['lineHeight', 'spaceBefore', 'spaceAfter'];
const paragraphSpacing = paragraph => {
    if (!paragraph.lineHeight) throw new Error('The source paragraph line spacing is unresolved; Format Painter cannot copy it yet.');
    return Object.fromEntries(spacingKeys.map(key => [key,
        paragraph[key] ? { ...paragraph[key] } : { kind: 'exact', value: 0 }]));
};
const sameSpacing = (left, right) => spacingKeys.every(key => left[key].kind === right[key].kind && left[key].value === right[key].value);
/** Read-only scene projection becomes a detached formatting snapshot. */
export function captureObjectFormat(object) {
    if (!object || object.group || object.image || !object.edit?.canStyle) throw new Error('Select one editable shape or text box to copy its formatting.');
    if (!opaque(object.fill) || !opaque(object.stroke) || object.unsupported?.some(reason => /FILL|STROKE|COLOR|EFFECT|GEOMETRY/.test(reason))) {
        throw new Error('Format Painter cannot copy this shape’s unsupported paint or effects yet.');
    }
    const snapshot = { fill: object.geometry === 'line' ? undefined : object.fill?.color ?? null,
        stroke: object.stroke ? { color: object.stroke.color, width: object.stroke.width } : null };
    if (object.edit.canSetShadow && object.outerShadow !== undefined) {
        snapshot.shadow = object.outerShadow ? { ...object.outerShadow } : null;
    }
    if (object.text) {
        const missingLayout = object.text.layout === null && object.text.unsupported?.includes('ODF_UNRESOLVED_VERTICAL_ALIGNMENT');
        if (object.text.mixed || !opaque(object.text.style) || object.text.unsupported?.some(reason => reason !== 'ODF_SCRIPT_SPECIFIC_TEXT_STYLE' && !(missingLayout && reason === 'ODF_UNRESOLVED_VERTICAL_ALIGNMENT'))) throw new Error('Format Painter currently requires uniform, supported text formatting.');
        snapshot.limitation = [object.text.unsupported?.includes('ODF_SCRIPT_SPECIFIC_TEXT_STYLE') ? 'Uses the source’s Western font settings for all scripts.' : '',
            missingLayout ? 'Text box layout unavailable; target layout is unchanged.' : '',
            !object.text.paragraphs?.length ? 'Paragraph spacing unavailable; target paragraph spacing is unchanged.' : ''].filter(Boolean).join(' ');
        if (object.text.paragraphs?.length) {
            snapshot.spacing = paragraphSpacing(object.text.paragraphs[0]);
            if (object.text.paragraphs.some(paragraph => !sameSpacing(snapshot.spacing, paragraphSpacing(paragraph)))) {
                throw new Error('Format Painter currently requires the same spacing for every paragraph.');
            }
            snapshot.useParagraphSpacing = object.text.layout?.useParagraphSpacing;
        }
        snapshot.style = pick(object.text.style, textKeys);
        snapshot.layout = pick(object.text.layout, layoutKeys);
    }
    return snapshot;
}
/** One command batch is one native undo transaction. No geometry/text commands. */
export function paintObjectFormat(snapshot, target, slide) {
    if (!target || !target.edit?.canStyle || target.group || target.image) throw new Error('Choose an editable shape or text box as the Format Painter target.');
    const base = { slide, object: target.id }, commands = [];
    if ('fill' in snapshot && target.geometry !== 'line') commands.push({ type: 'set-object-fill', ...base, fill: snapshot.fill });
    commands.push({ type: 'set-object-stroke', ...base, stroke: snapshot.stroke });
    if ('shadow' in snapshot) {
        if (!target.edit.canSetShadow) throw new Error('This object’s effects cannot receive the copied shadow formatting.');
        commands.push({ type: 'set-object-shadow', ...base, shadow: snapshot.shadow });
    }
    const addText = !target.text && target.edit?.canAddText && snapshot.style && Object.keys(snapshot.style).length > 0;
    if (snapshot.spacing && (target.text?.editable || addText)) {
        const nonzeroEdges = snapshot.spacing.spaceBefore.value !== 0 || snapshot.spacing.spaceAfter.value !== 0;
        if (nonzeroEdges && (typeof snapshot.useParagraphSpacing !== 'boolean' || snapshot.useParagraphSpacing !== target.text?.layout?.useParagraphSpacing)) {
            throw new Error('These text boxes use different paragraph spacing at their edges; Format Painter cannot copy that setting yet.');
        }
    }
    if (addText) commands.push({ type: 'set-object-text', ...base, text: '' });
    if (snapshot.style && Object.keys(snapshot.style).length && (target.text?.editable || addText)) commands.push({ type: 'set-text-style', ...base, style: snapshot.style });
    if (snapshot.layout && Object.keys(snapshot.layout).length && (target.text?.editable && target.text.layout || addText)) commands.push({ type: 'set-text-layout', ...base, layout: snapshot.layout });
    if (snapshot.spacing && (target.text?.editable || addText)) commands.push({ type: 'set-paragraph-spacing', ...base, spacing: snapshot.spacing });
    return commands.map(command => command.type === 'set-object-shadow' ? checkedShadowCommand(command) : checkedObjectCommand(command));
}
