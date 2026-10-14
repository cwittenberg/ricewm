import { openImageEngine } from '@ricewm/app';
import { failure } from './contract-values.mjs';
import { backgroundImageTarget } from './background-render.mjs';

const CACHE_BYTES = 12 * 1024 * 1024;
const MAIN_PIXELS = 6 * 1024 * 1024 / 4;
const THUMB_PIXELS = 256 * 1024 / 4;

/** Pure demand planner: shared assets get one texture at the largest required
 * resolution. Crop affects source resolution, never the stored source bytes. */
export function imageDemands(scene, { thumbnail = false } = {}) {
    const demands = new Map(), scale = (thumbnail ? 256 : 1920) / (scene.size?.width || 1920);
    const background = backgroundImageTarget(scene);
    for (const object of background ? [background, ...scene.objects] : scene.objects) {
        const image = object.image, box = object.bounds;
        if (!image || !box || !object.transform) continue;
        const crop = image.crop, matrix = object.transform;
        const xScale = Math.hypot(matrix[0], matrix[1]), yScale = Math.hypot(matrix[2], matrix[3]);
        const ratio = Math.min(1, Math.max(box.width * xScale * scale / ((1 - crop.left - crop.right) * image.width),
            box.height * yScale * scale / ((1 - crop.top - crop.bottom) * image.height)));
        const previous = demands.get(image.asset);
        if (!previous || ratio > previous.ratio) demands.set(image.asset, { image, ratio });
    }
    const budget = thumbnail ? THUMB_PIXELS : MAIN_PIXELS;
    const values = [...demands.values()];
    const dimensions = ({ image, ratio }, shrink) => ({ maximumWidth: Math.max(1, Math.floor(image.width * ratio * shrink)),
        maximumHeight: Math.max(1, Math.floor(image.height * ratio * shrink)) });
    const pixels = shrink => values.reduce((sum, value) => { const size = dimensions(value, shrink); return sum + size.maximumWidth * size.maximumHeight; }, 0);
    let shrink = 1;
    if (pixels(1) > budget) {
        let lower = 0, upper = 1;
        for (let index = 0; index < 24; index++) { const middle = (lower + upper) / 2; if (pixels(middle) <= budget) lower = middle; else upper = middle; }
        shrink = lower;
    }
    return values.map(value => ({ image: value.image, ...dimensions(value, shrink), maximumPixels: budget }));
}

/** Disposable presentation resource cache. Document state remains in Deck's
 * engine; decode runs in the SDK's separate bounded native Wasm worker. */
export function createDeckImageLayout({ services, signal, openEngine = openImageEngine } = {}) {
    let engine = null, opening = null, closed = false, cacheBytes = 0;
    const cache = new Map();
    const alive = () => { if (closed || signal?.aborted) throw failure('CLOSED', 'Picture renderer is closed'); };
    async function decoder() {
        alive();
        if (!opening) opening = openEngine({ services, path: 'image.wasm', signal }).then(value => {
            if (closed) { void value.close(); throw failure('CLOSED', 'Picture renderer is closed'); }
            engine = value; return value;
        }).catch(error => { opening = null; throw error; });
        return opening;
    }
    function clear() { cache.clear(); cacheBytes = 0; }
    return Object.freeze({
        clear,
        async render(scene, read, options = {}) {
            alive();
            const frames = new Map(), notices = [];
            const resources = new Map();
            for (const demand of imageDemands(scene, options)) {
                const { image, maximumWidth, maximumHeight, maximumPixels } = demand;
                const key = `${image.asset}/${maximumWidth}/${maximumHeight}`;
                try {
                    let resource = cache.get(key);
                    if (resource) { cache.delete(key); cache.set(key, resource); }
                    else {
                        const snapshot = await read(image.asset); alive();
                        if (snapshot.mime !== image.mime || snapshot.width !== image.width || snapshot.height !== image.height ||
                            snapshot.bytes.length !== image.byteLength) throw failure('INVALID_RESPONSE', 'Picture metadata changed');
                        try {
                            resource = await (await decoder()).decode(snapshot.bytes, { maximumWidth, maximumHeight, maximumPixels });
                        } catch (error) {
                            // A trap or transport failure retires the worker.
                            // A later explicit render can open a fresh decoder.
                            const previous = engine; engine = null; opening = null;
                            await Promise.resolve(previous?.close()).catch(() => {});
                            throw error;
                        }
                        alive();
                        if (!(resource.rgba instanceof Uint8Array) || resource.rgba.length !== resource.width * resource.height * 4 ||
                            resource.width > maximumWidth || resource.height > maximumHeight) throw failure('INVALID_RESPONSE', 'Invalid decoded picture');
                        resource = Object.freeze({ ...resource, key });
                        while (cacheBytes + resource.rgba.length > CACHE_BYTES && cache.size) {
                            const oldest = cache.keys().next().value; cacheBytes -= cache.get(oldest).rgba.length; cache.delete(oldest);
                        }
                        if (resource.rgba.length <= CACHE_BYTES) { cache.set(key, resource); cacheBytes += resource.rgba.length; }
                    }
                    resources.set(image.asset, resource);
                } catch (error) {
                    alive();
                    if (error.code === 'INVALID_RESPONSE') throw error;
                    notices.push(`Picture unavailable: ${String(error.message).slice(0, 160)}`);
                }
            }
            const background = backgroundImageTarget(scene);
            for (const object of background ? [background, ...scene.objects] : scene.objects) {
                if (object.image && resources.has(object.image.asset)) frames.set(object.id, resources.get(object.image.asset));
                else if (object.kind === 'image' && !object.image) notices.push('Some picture formats or effects are not supported yet; their source is preserved.');
            }
            return { frames, notices: [...new Set(notices)] };
        },
        async close() { if (closed) return; closed = true; clear(); await engine?.close(); },
    });
}
