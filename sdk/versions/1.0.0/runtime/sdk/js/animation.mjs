import { Message } from './messages.mjs';

// One clock for all active animations in this app worker. No timer is retained
// when every animation is paused, hidden, static, or disposed.
const active = new Set();
let timer = null;
const now = () => performance.now();
function schedule() {
    if (timer !== null || !active.size) return;
    timer = setTimeout(() => {
        timer = null;
        const time = now();
        for (const animation of [...active]) if (animation.next <= time) animation.tick(time);
        schedule();
    }, Math.max(0, Math.min(...[...active].map(animation => animation.next)) - now()));
}
function remove(animation) {
    active.delete(animation);
    if (!active.size && timer !== null) { clearTimeout(timer); timer = null; }
}

/** Worker-safe animation owned by a native window; draw may submit SceneBuilder
 * frames to SceneSurface. Return false to settle until invalidate()/play(). */
export class AnimationClock {
    static controlFactory = Object.freeze({
        topics: ['scene', 'media', 'animation'],
        constructor: 'new AnimationClock({window,draw:({time,delta,reducedMotion})=>{},frameRate:30,playing:true,onError})',
        methods: 'play(), pause(), invalidate(), destroy(), inspect(); inspect returns lifecycle state, scheduled, ticks, seconds, lastDelta and error. Repeated play/invalidate calls preserve elapsed time while running; time/delta are seconds. draw returns false to settle. Native window visibility, close and host reduced motion govern scheduling. Bind pointer/keyboard handlers to SceneSurface normally; do not use browser canvas or requestAnimationFrame.',
    });
    constructor({ window, draw, frameRate = 30, playing = true, onError = error => { throw error; } } = {}) {
        if (!window?.onState || !window?.onClose || !window?.application?.on || typeof draw !== 'function' || typeof onError !== 'function') throw TypeError('AnimationClock requires a native window and draw function');
        if (!Number.isInteger(frameRate) || frameRate < 1 || frameRate > 60 || typeof playing !== 'boolean') throw RangeError('Animation frameRate must be 1..60 and playing a boolean');
        this.window = window; this.draw = draw; this.frameRate = frameRate; this.playing = playing; this.onError = onError;
        this.time = 0; this.previous = null; this.next = 0; this.destroyed = false;
        this.ticks = 0; this.lastDelta = 0; this.lastError = null;
        this.observedState = window.state;
        this.stops = [window.onState(state => {
            const previous=this.observedState;this.observedState=state;
            if(previous&&Object.keys(state).length===Object.keys(previous).length&&Object.keys(state).every(key=>state[key]===previous[key]))return;
            if(!this.visible){this.previous=null;remove(this);return;}
            // Geometry/focus notifications must not restart a running clock:
            // repeated host updates used to discard its simulation delta.
            if(this.playing&&active.has(this)&&!this.reducedMotion)return;
            this.invalidate();
        }), window.onClose(() => this.destroy()),
            window.application.on(Message.THEME_CHANGED, () => this.invalidate())];
        this.invalidate();
    }
    get reducedMotion() { return this.window.application.theme?.reducedMotion === true; }
    get visible() { return this.window.state?.visible !== false; }
    invalidate() {
        if (this.destroyed) return;
        // A repaint request is not a simulation restart. Theme notifications,
        // render callbacks and repeated play() calls may arrive every frame.
        if (this.playing && active.has(this) && this.visible && !this.reducedMotion) return;
        this.previous = null;
        remove(this);
        if (!this.visible) return;
        this.next = now(); active.add(this); schedule();
    }
    play() { if (!this.destroyed) { this.playing = true; this.invalidate(); } }
    pause() { this.playing = false; this.previous = null; remove(this); }
    tick(time) {
        if (this.destroyed || !this.visible) { remove(this); return; }
        const delta = this.previous === null ? 0 : Math.max(0, Math.min(.1, (time - this.previous) / 1000));
        this.previous = time; this.time += delta; this.lastDelta = delta; this.ticks++;
        try {
            const keepGoing = this.draw(Object.freeze({ time: this.time, delta, reducedMotion: this.reducedMotion }));
            if (keepGoing?.then) throw TypeError('Animation draw must be synchronous');
            if (keepGoing === false || !this.playing || this.reducedMotion) remove(this);
            else this.next = time + 1000 / this.frameRate;
        } catch (error) { this.lastError = error?.message || String(error); this.pause(); this.onError(error); }
    }
    inspect() {
        return Object.freeze({state:this.destroyed?'destroyed':this.lastError?'failed':!this.visible?'hidden':this.reducedMotion?'reduced-motion':!this.playing?'paused':active.has(this)?'running':'settled',
            playing:this.playing,visible:this.visible,reducedMotion:this.reducedMotion,scheduled:active.has(this),frameRate:this.frameRate,ticks:this.ticks,seconds:this.time,lastDelta:this.lastDelta,error:this.lastError});
    }
    destroy() { if (this.destroyed) return; this.destroyed = true; remove(this); for (const stop of this.stops) stop(); this.stops = []; }
}

/** Orthographic projection for small interactive globes; angles are degrees. */
export function projectGlobePoint(latitude, longitude, { longitude: centerLon = 0, latitude: centerLat = 0, x = 0, y = 0, radius = 1 } = {}) {
    if (![latitude, longitude, centerLon, centerLat, x, y, radius].every(Number.isFinite) || Math.abs(latitude) > 90 || Math.abs(centerLat) > 90 || radius <= 0) throw RangeError('Invalid globe coordinates');
    const radians = Math.PI / 180, lat = latitude * radians, lon = (longitude - centerLon) * radians, center = centerLat * radians;
    const depth = Math.sin(center) * Math.sin(lat) + Math.cos(center) * Math.cos(lat) * Math.cos(lon);
    return Object.freeze({ x: x + radius * Math.cos(lat) * Math.sin(lon), y: y - radius * (Math.cos(center) * Math.sin(lat) - Math.sin(center) * Math.cos(lat) * Math.cos(lon)), depth, visible: depth > 0 });
}
projectGlobePoint.controlFactory = Object.freeze({ topics: ['scene', 'animation'], constructor: 'projectGlobePoint(latitude,longitude,{latitude:0,longitude:0,x:0,y:0,radius:100})', methods: 'Returns {x,y,depth,visible}; use visible for hemisphere culling and x/y for pointer picking.' });
