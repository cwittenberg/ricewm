// The window's shadow changes with its geometry, not the wallpaper below it.
// Retain the full-resolution caster so video frames do not repeat its blur.
export class RiceNativeWindowShadow {
    constructor(canvasFactory) {
        this.canvasFactory = canvasFactory;
        this.cache = new Map();
        this.bytes = 0;
    }

    clear() {
        this.cache.clear();
        this.bytes = 0;
    }

    paint(context, frame, shapeKey, appendPath, orchard, active = true) {
        const transform = context.getTransform?.();
        // Canvas shadow blur is independent of the current scale. Keep the
        // original painter during transformed animations and app previews.
        if (!transform || transform.a !== 1 || transform.d !== 1 ||
            transform.b !== 0 || transform.c !== 0 ||
            !Number.isInteger(transform.e + frame.x) ||
            !Number.isInteger(transform.f + frame.y)) return false;
        const padding = 64;
        const width = frame.width + padding * 2;
        const height = frame.height + padding * 2;
        const cost = width * height * 4;
        if (cost > 32 * 1024 * 1024) return false;
        const key = JSON.stringify([frame.width, frame.height, shapeKey, orchard, active]);
        let surface = this.cache.get(key);
        if (surface) {
            this.cache.delete(key);
            this.cache.set(key, surface);
        } else {
            surface = this.canvasFactory(width, height);
            const target = surface?.getContext?.('2d');
            if (!target) return false;
            target.translate(padding, padding);
            target.shadowColor = orchard ? (active ? 'rgba(12, 20, 35, .30)' : 'rgba(12, 20, 35, .11)') : 'rgba(0, 0, 0, 0.46)';
            target.shadowBlur = orchard ? (active ? 38 : 18) : 28;
            target.shadowOffsetY = orchard ? 12 : 10;
            target.fillStyle = '#000000';
            target.beginPath();
            appendPath(target);
            target.fill();
            while (this.cache.size && (this.bytes + cost > 32 * 1024 * 1024 || this.cache.size >= 16)) {
                const first = this.cache.keys().next().value;
                const old = this.cache.get(first);
                this.bytes -= old.width * old.height * 4;
                this.cache.delete(first);
            }
            this.cache.set(key, surface);
            this.bytes += cost;
        }
        // The caller's original exterior silhouette clip hides the caster.
        context.drawImage(surface, frame.x - padding, frame.y - padding);
        return true;
    }
}
