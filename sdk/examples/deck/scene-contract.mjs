import { sceneHyperlinks } from './hyperlink-contract.mjs';
import { tableMetadata } from './table-values.mjs';
import { animationTimelineHeader } from './animation-contract.mjs';
import { sceneImage } from './image-contract.mjs';
import { checkedStatus } from './status-contract.mjs';
import { failure } from './contract-values.mjs';
import { validSpacing } from './paragraph-layout.mjs';
import { backgroundPicture, backgroundGradient } from './background-contract.mjs';
import { sceneTransition } from './transition-contract.mjs';
import { sceneShadow } from './shadow-contract.mjs';

const finite = value => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e12;
const text = value => typeof value === 'string' && value.length <= 4096;
const codes = value => Array.isArray(value) && value.length <= 128 && value.every(text);
const color = value => value === null || (value && /^#[0-9A-Fa-f]{6}$/.test(value.color) &&
    finite(value.opacity) && value.opacity >= 0 && value.opacity <= 1);
const nullable = (value, check) => value === null || check(value);
const style = value => value && (value.opacity === undefined || value.opacity === null || finite(value.opacity) && value.opacity >= 0 && value.opacity <= 1) && nullable(value.fontFamily, text) && nullable(value.fontSize, value => finite(value) && value > 0) &&
    ['bold', 'italic', 'underline'].every(key => nullable(value[key], value => typeof value === 'boolean')) &&
    nullable(value.color, value => /^#[0-9A-Fa-f]{6}$/.test(value)) && nullable(value.align, value => ['left', 'center', 'right'].includes(value));
const edit = value => value && ['canRotate', 'canAddText', 'canResize', 'canGroup', 'canReorder', 'canSetShadow', 'canTransformShadow'].every(key => value[key] === undefined || typeof value[key] === 'boolean') && ['canTransform', 'canStyle', 'canDelete'].every(key => typeof value[key] === 'boolean') &&
    nullable(value.rotationDegrees, finite) && nullable(value.box, value => value && ['x', 'y', 'width', 'height'].every(key => finite(value[key]))) &&
    (!value.canTransform || value.box !== null);
const textLayout = value => value === undefined || value === null || value &&
    ['left', 'top', 'right', 'bottom'].every(key => finite(value[key]) && Math.abs(value[key]) <= 1_000_000) &&
    ['top', 'middle', 'bottom'].includes(value.vertical) && typeof value.wrap === 'boolean' &&
    (value.useParagraphSpacing === undefined || typeof value.useParagraphSpacing === 'boolean') &&
    (value.horizontalOverflow === undefined || ['overflow', 'clip'].includes(value.horizontalOverflow)) &&
    (value.verticalOverflow === undefined || ['overflow', 'clip', 'ellipsis'].includes(value.verticalOverflow));
const line = value => value === undefined || value === null || value && typeof value.editable === 'boolean' &&
    ['start', 'end'].every(key => value[key] && finite(value[key].x) && finite(value[key].y));
const sourceText = value => value === null || value && typeof value.text === 'string' && value.text.length <= 65536 && style(value.style) &&
    (value.paragraphs === undefined || Array.isArray(value.paragraphs) && value.paragraphs.length <= 256 && value.paragraphs.every(p => p &&
        Number.isSafeInteger(p.start) && p.start >= 0 && Number.isSafeInteger(p.end) && p.end >= p.start && p.end <= 262144 && style(p.style) &&
        ['lineHeight', 'spaceBefore', 'spaceAfter'].every(key => validSpacing(p[key], key === 'lineHeight')))) &&
    textLayout(value.layout) && typeof value.mixed === 'boolean' && typeof value.editable === 'boolean' && codes(value.unsupported) && Array.isArray(value.runs) && value.runs.length <= 16384 &&
    value.runs.every(run => text(run.id) && typeof run.text === 'string' && run.text.length <= 65536 && style(run.style));
const size = value => value && finite(value.width) && value.width > 0 && finite(value.height) && value.height > 0;
const placeholder = value => value === undefined || value === null || value && text(value.role) &&
    Number.isSafeInteger(value.index) && value.index >= 0 && value.index <= 0xffffffff && text(value.prompt);

const paths = value => {
    if (value === undefined) return true;
    if (!Array.isArray(value) || value.length > 256) return false;
    let count = 0;
    const arity = { M: 2, L: 2, Q: 4, C: 6, Z: 0 };
    return value.every(path => path && typeof path.fill === 'boolean' && typeof path.stroke === 'boolean' &&
        Array.isArray(path.commands) && path.commands.length > 0 && (count += path.commands.length) <= 16384 && path.commands.every(command =>
            Array.isArray(command) && Object.hasOwn(arity, command[0]) && command.length === arity[command[0]] + 1 && command.slice(1).every(finite)));
};

const group = object => (object.parentId === undefined || object.parentId === null ||
    text(object.parentId) && object.parentId.length > 0 && object.parentId !== object.id) &&
    (object.group === undefined || object.group === null || object.kind === 'group' &&
        typeof object.group.canUngroup === 'boolean' && Array.isArray(object.group.children) &&
        object.group.children.length <= 100000 && object.group.children.every(id => text(id) && id && id !== object.id) &&
        new Set(object.group.children).size === object.group.children.length);

export function checkedScene(response) {
    checkedStatus(response);
    const scene = response.result.scene;
    const invalid = () => { throw failure('INVALID_RESPONSE', 'Invalid bounded slide geometry'); };
    if (!scene || !text(scene.slideId) || !Number.isSafeInteger(scene.index) || scene.index < 0 ||
        scene.index >= response.result.slideCount || scene.units !== 'px96' ||
        (scene.size !== null && !size(scene.size)) || (scene.background !== undefined && !color(scene.background)) ||
        !backgroundPicture(scene.backgroundPicture) ||
        !backgroundGradient(scene.backgroundGradient) ||
        !sceneTransition(scene.transition) ||
        !animationTimelineHeader(scene.animationTimeline) ||
        ['canSetBackground', 'canSetSize', 'canSetTransition', 'canMorphTransition', 'canDisableTransitionClick', 'hasAnimationTiming'].some(key => scene[key] !== undefined && typeof scene[key] !== 'boolean') ||
        (scene.layoutId !== undefined && scene.layoutId !== null && !text(scene.layoutId)) ||
        !Number.isSafeInteger(scene.totalObjects) || scene.totalObjects < 0 ||
        scene.totalObjects > 100000 || !Array.isArray(scene.objects) || scene.objects.length > 64 ||
        !codes(scene.unsupported) || (scene.nextObject !== null && (!Number.isSafeInteger(scene.nextObject) ||
        scene.nextObject < 1 || scene.nextObject >= scene.totalObjects))) invalid();
    const ids = new Set();
    for (const object of scene.objects) {
        if (!object || !text(object.id) || !object.id || ids.has(object.id) ||
            !sceneHyperlinks(object) || !['shape', 'text', 'image', 'group', 'table'].includes(object.kind) || !group(object) || !tableMetadata(object.table) || !paths(object.paths) ||
            (object.morphSource !== undefined && object.morphSource !== null && (!text(object.morphSource.sourceId) || !text(object.morphSource.sourcePart) || !nullable(object.morphSource.name,text) || !nullable(object.morphSource.creationId,text))) ||
            !(object.geometry === null || typeof object.geometry === 'string' && /^[a-zA-Z][a-zA-Z0-9-]{0,63}$/.test(object.geometry)) || !line(object.line) ||
            (object.transform !== null && (!Array.isArray(object.transform) || object.transform.length !== 6 || !object.transform.every(finite))) ||
            (object.bounds !== null && (!finite(object.bounds.x) || !finite(object.bounds.y) ||
                !finite(object.bounds.width) || object.bounds.width < 0 || !finite(object.bounds.height) || object.bounds.height < 0)) ||
            !color(object.fill) || !color(object.stroke) || !sceneShadow(object.outerShadow) || (object.stroke !== null && (!finite(object.stroke.width) || object.stroke.width < 0)) ||
            !placeholder(object.placeholder) || !Array.isArray(object.textRunIds) || object.textRunIds.length > 16384 || !object.textRunIds.every(text) ||
            (object.image !== undefined && !sceneImage(object.image)) || !edit(object.edit) || !sourceText(object.text) || typeof object.supported !== 'boolean' || !codes(object.unsupported)) invalid();
        ids.add(object.id);
    }
    return response;
}

export { sourceText as validSceneText };
