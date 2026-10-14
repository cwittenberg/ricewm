import { exact, failure } from './contract-values.mjs';
import { text, checkedResponse, revision } from './status-contract.mjs';

export const ANIMATION_COMMANDS = new Set(['add-object-animation', 'set-object-animation', 'remove-object-animation', 'reorder-object-animation']);
const id = value => typeof value === 'string' && value.length > 0 && text(value, 4096);
const nullableId = value => value === null || id(value);
const time = value => Number.isSafeInteger(value) && value >= 0 && value <= 600000;
const effect = value => exact(value, ['category', 'kind', 'direction']) &&
    ['entrance', 'exit'].includes(value.category) && ['appear', 'fade'].includes(value.kind) && value.direction === null;
const fields = ['effect', 'start', 'durationMs', 'delayMs'];
function settings(value) {
    return Object.entries(value).every(([key, value]) => key === 'effect' ? effect(value) : key === 'start' ?
        ['click', 'with-previous', 'after-previous'].includes(value) : ['durationMs', 'delayMs'].includes(key) && time(value));
}

/** This checks wire shape, not document editability. Native source adapters own
 * graph/target validation and atomic rejection of unsupported source edits. */
export function checkedAnimationCommand(command) {
    const invalid = () => { throw failure('INVALID_REQUEST', 'Invalid object animation command'); };
    if (!command || !id(command.slide)) invalid();
    if (command.type === 'add-object-animation') {
        if (!exact(command, ['type', 'slide', 'object', ...fields, 'after']) || !id(command.object) || !nullableId(command.after) ||
            !settings(Object.fromEntries(fields.map(key => [key, command[key]])))) invalid();
    } else if (command.type === 'set-object-animation') {
        if (!exact(command, ['type', 'slide', 'animation', 'patch']) || !id(command.animation) ||
            !exact(command.patch, fields, []) || !Object.keys(command.patch).length || !settings(command.patch)) invalid();
    } else if (command.type === 'remove-object-animation') {
        if (!exact(command, ['type', 'slide', 'animation']) || !id(command.animation)) invalid();
    } else if (command.type === 'reorder-object-animation') {
        if (!exact(command, ['type', 'slide', 'animation', 'before']) || !id(command.animation) || !nullableId(command.before)) invalid();
    } else invalid();
    return structuredClone(command);
}

export function checkedCreatedAnimations(result, commands) {
    const added = commands.filter(command => command.type === 'add-object-animation');
    const created = result.createdAnimations;
    if (created === undefined && !added.length) return;
    if (!Array.isArray(created) || created.length !== added.length || created.some((value, index) =>
        !exact(value, ['slide', 'animation', 'object']) || !id(value.animation) ||
        value.slide !== added[index].slide || value.object !== added[index].object) ||
        new Set(created.map(value => JSON.stringify([value.slide, value.animation]))).size !== created.length) {
        throw failure('INVALID_RESPONSE', 'Invalid created animation identities');
    }
}

export function checkedAnimationCatalog(response, expected) {
    checkedResponse(response); revision(response.revision);
    const entries = response.result.effects;
    if (expected !== undefined && response.revision !== expected || !Array.isArray(entries) || entries.length > 128 ||
        entries.some(entry => !exact(entry, ['id', 'label', 'effect', 'defaultDurationMs']) || !id(entry.id) ||
            !text(entry.label, 128) || !entry.label || !effect(entry.effect) || !time(entry.defaultDurationMs)) ||
        new Set(entries.map(entry => entry.id)).size !== entries.length) {
        throw failure('INVALID_RESPONSE', 'Invalid animation effect catalog');
    }
    return response;
}
