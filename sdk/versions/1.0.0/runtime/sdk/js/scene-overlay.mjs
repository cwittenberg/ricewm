import { Stack } from './controls.mjs';
import { SceneSurface } from './scene-surface.mjs';
function checkedBounds(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !['x','y','width','height'].includes(key)) ||
        !['x','y','width','height'].every(key => Number.isFinite(value[key]) && Math.abs(value[key]) <= 16) || value.width <= 0 || value.height <= 0) throw new RangeError('Scene overlay requires bounded normalized scene coordinates');
    return Object.freeze({...value});
}
/** Native child widgets anchored to the exact fitted/zoomed SceneSurface.
 * Append after surface in the same Stack. Bounds are fractions of the full
 * scene, including its offscreen region when panned; the parent clips input. */
export class SceneOverlay extends Stack {
    static constructorOptions = Object.freeze(["surface", "bounds"]);
    constructor({surface,bounds,...properties},children=[]) {
        if (!(surface instanceof SceneSurface)) throw new TypeError('SceneOverlay requires a SceneSurface');
        super(properties,children); this.surface=surface; this.bounds=checkedBounds(bounds);
    }
    setBounds(bounds) { this.bounds=checkedBounds(bounds);this.application?.invalidate(this);return this; }
    toWidget() { return Object.freeze({...super.toWidget(),text:`panel:scene-overlay:${this.surface.id}:${['x','y','width','height'].map(key=>Math.round(this.bounds[key]*1000000)).join(':')}:flush`}); }
}
