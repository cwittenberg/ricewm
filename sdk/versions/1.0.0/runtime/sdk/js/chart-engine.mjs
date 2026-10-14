/** ECharts' supported painter extension targets RiceDisplay instead of a DOM
 * canvas/SVG node. Geometry and animation stay in upstream ECharts/zrender. */
const registered = new WeakSet();
const pending = new Set();
let timer = null;

function schedule() {
    if (timer !== null || !pending.size) return;
    const delay = Math.max(0, Math.min(...[...pending].map(driver => driver.nextFrame - Date.now())));
    timer = setTimeout(() => {
        timer = null;
        const now = Date.now();
        for (const driver of [...pending]) if (driver.nextFrame <= now) driver.tick();
        schedule();
    }, delay);
}

class RiceChartPainter {
    // Prevent browser event listeners and zrender's own render loop. The shared
    // native clock flushes the normal animation/update cycle at the chosen rate.
    ssrOnly = true;
    constructor(root, storage, options) { this.root = root; this.storage = storage; this.width = options.width; this.height = options.height; }
    getType() { return 'ricewm-native'; }
    getWidth() { return this.width; }
    getHeight() { return this.height; }
    getViewportRoot() { return this.root; }
    getViewportRootOffset() { return {offsetLeft:0,offsetTop:0}; }
    resize(width, height) { this.width = width; this.height = height; }
    refresh() { this.present?.(); }
    refreshHover() { this.present?.(); }
    clear() {}
    dispose() { this.present = null; }
}

const easings = new Set(['linear', 'cubicOut', 'cubicInOut', 'quarticOut', 'exponentialOut']);
export function chartAnimation(value = true) {
    if (value === false) return Object.freeze({enabled:false,duration:0,updateDuration:0,easing:'cubicOut',frameRate:30});
    if (value === true) value = {};
    if (!value || typeof value !== 'object' || Array.isArray(value) || Object.keys(value).some(key => !['enabled','duration','updateDuration','easing','frameRate'].includes(key))) throw TypeError('Chart animation requires a boolean or duration/updateDuration/easing/frameRate options');
    const config = {enabled:true,duration:600,updateDuration:360,easing:'cubicOut',frameRate:30,...value};
    if (typeof config.enabled !== 'boolean' || ![config.duration,config.updateDuration].every(n => Number.isInteger(n) && n >= 0 && n <= 2000) || ![24,30,60].includes(config.frameRate) || !easings.has(config.easing)) throw RangeError('Chart animation requires durations 0..2000, frameRate 24|30|60 and a supported easing');
    return Object.freeze(config);
}

function finishElements(zr) {
    const seen = new Set();
    const finish = element => {
        if (!element || seen.has(element)) return;
        seen.add(element); element.stopAnimation?.(null, true);
        finish(element.getClipPath?.());
    };
    for (const root of zr.storage.getRoots()) { finish(root); root.traverse?.(finish); }
    zr.animation.clear(); zr.animation.stop();
}

export function createNativeChartEngine(ec, {width,height,present,onError}) {
    if (!registered.has(ec)) { ec.zrender.registerPainter('ricewm-native',RiceChartPainter); registered.add(ec); }
    // ECharts associates instances with a target identity. This small object is
    // not a DOM node and offers no navigation, events or browser capability.
    const attributes = new Map();
    const target = {getAttribute:key => attributes.get(key),setAttribute:(key,value) => attributes.set(key,value),removeAttribute:key => attributes.delete(key)};
    const engine = ec.init(target,null,{renderer:'ricewm-native',width,height});
    const zr = engine.getZr(); zr.animation.stop();
    let resolve,settled = Promise.resolve(),disposed = false;
    const driver = {
        engine,frames:0,nextFrame:0,frameRate:30,active:false,
        get settled() { return settled; },
        request(frameRate=30) {
            if (disposed) return settled;
            this.frameRate=frameRate;
            if (!resolve) settled = new Promise(done => { resolve=done; });
            this.active=!zr.animation.isFinished();this.nextFrame=Date.now();pending.add(this);
            // Element updates can wake zrender's default RAF. Native rendering
            // has one owner; stop that loop after each upstream mutation.
            zr.animation.stop();schedule();return settled;
        },
        tick() {
            if (disposed) return;
            try {
                zr.animation.update();zr.flush();zr.animation.stop();
                this.frames++;
                if (zr.animation.isFinished()) {
                    // Ensure the exact terminal geometry reaches the native
                    // surface, even when the last upstream frame was unchanged.
                    present();this.active=false;pending.delete(this);
                    const done=resolve;resolve=null;done?.();
                } else { this.active=true;this.nextFrame=Date.now()+Math.round(1000/this.frameRate); }
            } catch(error) {
                pending.delete(this);this.active=false;finishElements(zr);
                const done=resolve;resolve=null;done?.();onError?.(error);
            }
        },
        finish() {
            if(disposed)return;
            finishElements(zr);pending.delete(this);present();this.active=false;
            const done=resolve;resolve=null;done?.();
        },
        dispose() {
            if(disposed)return;disposed=true;pending.delete(this);engine.dispose();this.active=false;
            const done=resolve;resolve=null;done?.();
            if(!pending.size&&timer!==null){clearTimeout(timer);timer=null;}
        },
    };
    zr.painter.present = present;
    return driver;
}
