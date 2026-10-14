import { exact, failure } from './contract-values.mjs';

export const SHADOW_FIELDS = Object.freeze(['color', 'opacity', 'blurRadius', 'angleDegrees', 'distance',
    'scaleX', 'scaleY', 'skewXDegrees', 'skewYDegrees', 'alignment', 'rotateWithShape']);
export const SHADOW_ALIGNMENTS = Object.freeze(['tl', 't', 'tr', 'l', 'ctr', 'r', 'bl', 'b', 'br']);
export const DEFAULT_SHADOW = Object.freeze({ color: '#000000', opacity: 1, blurRadius: 0,
    angleDegrees: 0, distance: 0, scaleX: 1, scaleY: 1, skewXDegrees: 0, skewYDegrees: 0,
    alignment: 'b', rotateWithShape: true });
const bounded = (value, low, high) => typeof value === 'number' && Number.isFinite(value) && value >= low && value <= high;
export function validShadow(value) {
    return exact(value, SHADOW_FIELDS) && /^#[0-9a-f]{6}$/iu.test(value.color) &&
        bounded(value.opacity, 0, 1) && bounded(value.blurRadius, 0, 1e6) && bounded(value.distance, 0, 1e6) &&
        bounded(value.angleDegrees, 0, 360) && value.angleDegrees < 360 &&
        ['scaleX', 'scaleY'].every(key => bounded(value[key], -100, 100)) &&
        ['skewXDegrees', 'skewYDegrees'].every(key => bounded(value[key], -90, 90) && Math.abs(value[key]) < 90) &&
        SHADOW_ALIGNMENTS.includes(value.alignment) && typeof value.rotateWithShape === 'boolean';
}
export const sceneShadow = value => value === undefined || value === null || validShadow(value);
export function checkedShadowCommand(command) {
    const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
    if (!exact(command, ['type', 'slide', 'object', 'shadow']) || command.type !== 'set-object-shadow' ||
        !id(command.slide) || !id(command.object) || !(command.shadow === null || validShadow(command.shadow))) {
        throw failure('INVALID_REQUEST', 'Invalid object shadow settings');
    }
    return { ...command, shadow: command.shadow === null ? null : { ...command.shadow } };
}
