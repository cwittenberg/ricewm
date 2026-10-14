import { shapeById } from './shape-catalog.mjs';
import { exact, failure } from './contract-values.mjs';

const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
const color = value => typeof value === 'string' && /^#[0-9a-f]{6}$/iu.test(value);
const finite = value => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e6;
const box = value => exact(value, ['x', 'y', 'width', 'height']) && Object.values(value).every(finite) && value.width > 0 && value.height > 0;
const point = value => exact(value, ['x', 'y']) && finite(value.x) && finite(value.y);
const points = command => point(command.start) && point(command.end) && (command.start.x !== command.end.x || command.start.y !== command.end.y);
const stroke = value => value === null || exact(value, ['color', 'width']) && color(value.color) && finite(value.width) && value.width >= 0;
function style(value) {
    if (!exact(value, ['fontFamily', 'fontSize', 'bold', 'italic', 'underline', 'color', 'align'], []) || !Object.keys(value).length) return false;
    return Object.entries(value).every(([key, current]) => {
        if (['bold', 'italic', 'underline'].includes(key)) return typeof current === 'boolean';
        if (key === 'color') return color(current);
        if (key === 'align') return ['left', 'center', 'right'].includes(current);
        if (key === 'fontFamily') return typeof current === 'string' && current.length > 0 && current.length <= 256;
        return finite(current) && current >= 1 && current <= 512;
    });
}
function textLayout(value) {
    return exact(value, ['left', 'top', 'right', 'bottom', 'vertical', 'wrap'], []) && Object.keys(value).length > 0 &&
        Object.entries(value).every(([key, current]) => key === 'vertical' ? ['top', 'middle', 'bottom'].includes(current)
            : key === 'wrap' ? typeof current === 'boolean' : finite(current) && current >= 0);
}
function paragraphSpacing(value) {
    return exact(value, ['lineHeight', 'spaceBefore', 'spaceAfter'], []) && Object.keys(value).length > 0 &&
        Object.entries(value).every(([key, current]) => current === null || exact(current, ['kind', 'value']) &&
            (key === 'lineHeight' && current.kind === 'normal' && current.value === 1 ||
            ['multiple', 'exact'].includes(current.kind) && typeof current.value === 'number' && Number.isFinite(current.value) &&
            current.value >= 0 && current.value <= (current.kind === 'multiple' ? 100 : 65536)));
}
export function checkedObjectCommand(command) {
    const fail = () => { throw failure('INVALID_REQUEST', 'Invalid presentation authoring command'); };
    if (!id(command?.slide)) fail();
    const base = ['type', 'slide', 'object'];
    if (command.type === 'reorder-objects') {
        if (!exact(command, ['type', 'slide', 'objects', 'direction']) ||
            !Array.isArray(command.objects) || command.objects.length < 1 || command.objects.length > 32 ||
            !command.objects.every(id) || new Set(command.objects).size !== command.objects.length ||
            !['front', 'back', 'forward', 'backward'].includes(command.direction)) fail();
    } else if (['duplicate-objects', 'group-objects'].includes(command.type)) {
        const duplicate = command.type === 'duplicate-objects';
        if (!exact(command, duplicate ? ['type', 'slide', 'objects', 'offset'] : ['type', 'slide', 'objects']) ||
            !Array.isArray(command.objects) || command.objects.length < (duplicate ? 1 : 2) || command.objects.length > 32 ||
            !command.objects.every(id) || new Set(command.objects).size !== command.objects.length ||
            (duplicate && !point(command.offset))) fail();
    } else if (command.type === 'paste-objects') {
        if (!exact(command, ['type', 'slide', 'selection', 'offset']) || !id(command.selection) || !point(command.offset)) fail();
    } else if (command.type === 'insert-freeform') {
        if (!exact(command, ['type', 'slide', 'points', 'closed']) || typeof command.closed !== 'boolean' ||
            !Array.isArray(command.points) || command.points.length < (command.closed ? 3 : 2) || command.points.length > 256 ||
            !command.points.every(point)) fail();
    } else if (command.type === 'insert-line' || command.type === 'set-line-points') {
        const insert = command.type === 'insert-line';
        if (!exact(command, ['type', 'slide', 'start', 'end', insert ? 'stroke' : 'object']) || !points(command) ||
            (insert ? command.stroke === null || !stroke(command.stroke) : !id(command.object))) fail();
    } else if (command.type === 'insert-object') {
        if (!exact(command, ['type', 'slide', 'kind', 'box', 'text', 'fill', 'stroke'], ['type', 'slide', 'kind', 'box']) ||
            !(['rect', 'ellipse', 'line', 'text'].includes(command.kind) || shapeById(command.kind)) || !box(command.box) ||
            (command.text !== undefined && (typeof command.text !== 'string' || command.text.length > 32768)) ||
            (command.fill !== undefined && command.fill !== null && !color(command.fill)) ||
            (command.stroke !== undefined && !stroke(command.stroke))) fail();
    } else {
        if (!id(command.object)) fail();
        const property = { 'set-object-rotation': 'degrees', 'set-object-box': 'box', 'set-object-fill': 'fill', 'set-object-stroke': 'stroke', 'set-text-style': 'style', 'set-text-layout': 'layout', 'set-paragraph-spacing': 'spacing', 'set-object-text': 'text' }[command.type];
        if (command.type === 'delete-object' || command.type === 'ungroup-object') { if (!exact(command, base)) fail(); }
        else if (!property || !exact(command, [...base, property]) ||
            (property === 'box' && !box(command.box)) || (property === 'fill' && command.fill !== null && !color(command.fill)) ||
            (property === 'stroke' && !stroke(command.stroke)) || (property === 'style' && !style(command.style)) ||
            (property === 'degrees' && (!finite(command.degrees) || Math.abs(command.degrees) > 36000)) ||
            (property === 'layout' && !textLayout(command.layout)) ||
            (property === 'spacing' && !paragraphSpacing(command.spacing)) ||
            (property === 'text' && (typeof command.text !== 'string' || command.text.length > 32768 || /[\ud800-\udfff]/u.test(command.text)))) fail();
    }
    return structuredClone(command);
}

export { style as validTextStyle, textLayout as validTextLayout };
