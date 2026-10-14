import { exact, failure } from './contract-values.mjs';
import { imageAsset, sceneImage } from './image-contract.mjs';

export const BACKGROUND_COMMANDS = new Set(['set-slide-picture-background', 'reset-slide-background']);
export const BACKGROUND_IMAGE_KEY = '@slide-background';
export const BACKGROUND_SETTINGS = ['mode', 'opacity', 'scaleX', 'scaleY', 'offsetX', 'offsetY'];
export const DEFAULT_BACKGROUND_SETTINGS = Object.freeze({ mode: 'stretch', opacity: 1, scaleX: 1, scaleY: 1, offsetX: 0, offsetY: 0 });
const slide = value => value === null || typeof value === 'string' && value.length > 0 && value.length <= 4096;
const between = (value, min, max) => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
export function backgroundSettings(value) {
    return value && ['stretch', 'tile'].includes(value.mode) && between(value.opacity, 0, 1) &&
        ['scaleX', 'scaleY'].every(key => between(value[key], .01, 100)) &&
        ['offsetX', 'offsetY'].every(key => between(value[key], -1e6, 1e6));
}
export function checkedBackgroundPictureRequest(value) {
    if (!exact(value, ['slide', ...BACKGROUND_SETTINGS]) || !slide(value.slide) || !backgroundSettings(value)) {
        throw failure('INVALID_REQUEST', 'Invalid picture background settings');
    }
    return { ...value };
}
export function checkedBackgroundCommand(command) {
    const valid = command?.type === 'reset-slide-background'
        ? exact(command, ['type', 'slide']) && slide(command.slide)
        : command?.type === 'set-slide-picture-background' && exact(command, ['type', 'slide', 'asset', ...BACKGROUND_SETTINGS]) &&
            slide(command.slide) && imageAsset(command.asset) && backgroundSettings(command);
    if (!valid) throw failure('INVALID_REQUEST', 'Invalid slide background command');
    return { ...command };
}
export function backgroundPicture(value) {
    return value === undefined || value === null || exact(value, ['image', ...BACKGROUND_SETTINGS]) &&
        value.image !== null && sceneImage(value.image) && backgroundSettings(value);
}
export function backgroundGradient(value) {
    if (value === undefined || value === null) return true;
    const point = p => Array.isArray(p) && p.length === 2 && p.every(n => between(n, -1e12, 1e12));
    if (!Array.isArray(value.stops) || value.stops.length < 2 || value.stops.length > 64 ||
        !value.stops.every((stop, index) => exact(stop, ['offset', 'color', 'opacity']) &&
            between(stop.offset, 0, 1) && between(stop.opacity, 0, 1) && /^#[0-9a-f]{6}$/iu.test(stop.color) &&
            (index === 0 || stop.offset >= value.stops[index - 1].offset))) return false;
    if (value.kind === 'linear') return exact(value, ['kind', 'start', 'end', 'stops']) && point(value.start) && point(value.end) &&
        value.start.some((n, i) => n !== value.end[i]);
    return value.kind === 'radial' && exact(value, ['kind', 'center', 'radius', 'stops']) &&
        point(value.center) && point(value.radius) && value.radius.every(n => n > 0);
}
