import { exact, failure } from './contract-values.mjs';
import { imageAsset, sceneImage } from './image-contract.mjs';
import { checkedBackgroundFill } from '../background-fills.mjs';
import {checkedPictureFillSettings} from '../picture-fills.mjs';

export const BACKGROUND_COMMANDS = new Set(['set-slide-background-style', 'set-slide-picture-background', 'reset-slide-background', 'set-slide-fill-background', 'set-slide-background-graphics', 'copy-slide-background']);
export const BACKGROUND_IMAGE_KEY = '@slide-background';
export const BACKGROUND_SETTINGS = ['mode', 'opacity', 'scaleX', 'scaleY', 'offsetX', 'offsetY'];
export const BACKGROUND_PICTURE_OPTIONS = ['alignment','mirror'];
export const DEFAULT_BACKGROUND_SETTINGS = Object.freeze({ mode: 'stretch', opacity: 1, scaleX: 1, scaleY: 1, offsetX: 0, offsetY: 0 });
const slide = value => value === null || typeof value === 'string' && value.length > 0 && value.length <= 4096;
const between = (value, min, max) => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
export function backgroundSettings(value) {
    try{checkedPictureFillSettings(value);return true;}catch{return false;}
}
export function checkedBackgroundPictureRequest(value) {
    if (!exact(value, ['slide', ...BACKGROUND_SETTINGS,...BACKGROUND_PICTURE_OPTIONS],['slide',...BACKGROUND_SETTINGS]) || !slide(value.slide) || !backgroundSettings(value)) {
        throw failure('INVALID_REQUEST', 'Invalid picture background settings');
    }
    return { ...value };
}
export function checkedBackgroundCommand(command) {
    if(command?.type==='set-slide-background-style') {
        if(!exact(command,['type','slide','style'])||!slide(command.slide)||!Number.isInteger(command.style)||command.style<1||command.style>12)throw failure('INVALID_REQUEST','Invalid background style');
        return {...command};
    }
    if(command?.type==='set-slide-fill-background') {
        if(!exact(command,['type','slide','fill'])||!slide(command.slide))throw failure('INVALID_REQUEST','Invalid background fill target');
        try{return {...command,fill:checkedBackgroundFill(command.fill)};}catch{throw failure('INVALID_REQUEST','Invalid background fill');}
    }
    if(command?.type==='set-slide-background-graphics') {
        if(!exact(command,['type','slide','hidden'])||!slide(command.slide)||typeof command.hidden!=='boolean')throw failure('INVALID_REQUEST','Invalid background graphics command');
        return {...command};
    }
    if(command?.type==='copy-slide-background') {
        if(!exact(command,['type','slide','source'])||!slide(command.slide)||command.source===null||!slide(command.source))throw failure('INVALID_REQUEST','Invalid background copy command');
        return {...command};
    }
    const valid = command?.type === 'reset-slide-background'
        ? exact(command, ['type', 'slide']) && slide(command.slide)
        : command?.type === 'set-slide-picture-background' && exact(command, ['type', 'slide', 'asset', ...BACKGROUND_SETTINGS,...BACKGROUND_PICTURE_OPTIONS],['type','slide','asset',...BACKGROUND_SETTINGS]) &&
            slide(command.slide) && imageAsset(command.asset) && backgroundSettings(command);
    if (!valid) throw failure('INVALID_REQUEST', 'Invalid slide background command');
    return { ...command };
}
export function backgroundPicture(value) {
    return value === undefined || value === null || exact(value, ['image', ...BACKGROUND_SETTINGS,...BACKGROUND_PICTURE_OPTIONS],['image',...BACKGROUND_SETTINGS]) &&
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
