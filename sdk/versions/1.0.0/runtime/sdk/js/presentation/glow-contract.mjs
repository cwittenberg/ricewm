import { exact } from './contract-values.mjs';

export function validGlow(value) {
    return exact(value, ['color', 'opacity', 'radius']) && /^#[0-9a-f]{6}$/iu.test(value.color) &&
        Number.isFinite(value.opacity) && value.opacity >= 0 && value.opacity <= 1 &&
        Number.isFinite(value.radius) && value.radius >= 0 && value.radius <= 1e6;
}
export const sceneGlow = value => value === undefined || value === null || validGlow(value);
