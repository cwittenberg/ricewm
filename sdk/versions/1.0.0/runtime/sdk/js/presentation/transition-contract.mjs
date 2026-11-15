import { exact, failure } from './contract-values.mjs';

export const TRANSITION_FIELDS = Object.freeze(['effect', 'direction', 'durationMs', 'advanceOnClick', 'advanceAfterMs']);
const time = value => Number.isSafeInteger(value) && value >= 0 && value <= 600000;
export function transitionSettings(value) {
    return value && ['none', 'fade', 'fade-black', 'push', 'wipe', 'morph'].includes(value.effect) &&
        ['left', 'right', 'up', 'down'].includes(value.direction) && time(value.durationMs) &&
        typeof value.advanceOnClick === 'boolean' && (value.advanceAfterMs === null || time(value.advanceAfterMs));
}
export function sceneTransition(value) {
    return value === undefined || value === null || exact(value, TRANSITION_FIELDS) && transitionSettings(value);
}
export function checkedTransitionCommand(command) {
    if (command?.type === 'set-selected-slide-transitions') {
        if (!exact(command, ['type', 'slides', ...TRANSITION_FIELDS]) || !Array.isArray(command.slides) || !command.slides.length || command.slides.length > 4096 ||
            command.slides.some(id => typeof id !== 'string' || !id.length || id.length > 4096) ||
            new Set(command.slides).size !== command.slides.length || !transitionSettings(command)) throw failure('INVALID_REQUEST', 'Invalid selected slide transition settings');
        return { ...command, slides: [...command.slides] };
    }
    if (!exact(command, ['type', 'slide', ...TRANSITION_FIELDS]) || command.type !== 'set-slide-transition' ||
        !(command.slide === null || typeof command.slide === 'string' && command.slide.length > 0 && command.slide.length <= 4096) ||
        !transitionSettings(command)) throw failure('INVALID_REQUEST', 'Invalid slide transition settings');
    return { ...command };
}
