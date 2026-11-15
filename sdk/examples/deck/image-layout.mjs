import { openImageEngine, planImageSamples, rasterizeVectorGlow } from '@ricewm/app';
import { failure } from './contract-values.mjs';
import { backgroundImageTarget } from './background-render.mjs';
import { glowDemands } from './glow-render.mjs';

const CACHE_BYTES = 12 * 1024 * 1024;
const MAIN_PIXELS = 6 * 1024 * 1024 / 4;
// Keep enough of the common 8 MiB scene for the bounded gradient raster and
// object vectors, including square slides at the maximum decoded picture size.
const GRADIENT_MAIN_PIXELS = 4 * 1024 * 1024 / 4;
const THUMB_PIXELS = 256 * 1024 / 4;

/** Pure demand planner: shared assets get one texture at the largest required
 * resolution. Crop affects source resolution, never the stored source bytes. */
export function imageDemands(scene, { thumbnail = false, thumbnailWidth = 256, thumbnailMaximumPixels = THUMB_PIXELS } = {}) {
    if (thumbnail && (!Number.isFinite(thumbnailWidth) || thumbnailWidth < 1 || thumbnailWidth > 4096 ||
        !Number.isSafeInteger(thumbnailMaximumPixels) || thumbnailMaximumPixels < 1 || thumbnailMaximumPixels > MAIN_PIXELS)) throw RangeError('Invalid thumbnail image demand');
    const demands = new Map(), scale = (thumbnail ? thumbnailWidth : 1920) / (scene.size?.width || 1920);
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
    const glowPixels = glowDemands(scene, { thumbnail, thumbnailWidth }).plans.reduce((sum, { plan }) => sum + plan.width * plan.height, 0);
    const budget = thumbnail ? thumbnailMaximumPixels : scene.backgroundFill?.kind === 'gradient' ? GRADIENT_MAIN_PIXELS : MAIN_PIXELS;
    const plan = planImageSamples([...demands].map(([id, value]) => ({ id, width: value.image.width, height: value.image.height, scale: value.ratio })),
        { maximumPixels: Math.max(1, budget - glowPixels) });
    return plan.map(({id,...bounds}) => ({ image: demands.get(id).image, ...bounds }));
}

/** Disposable presentation resource cache. Document state remains in Deck's
 * engine; decode runs in the SDK's separate bounded native Wasm worker. */
export function createDeckImageLayout({ services, signal, openEngine = openImageEngine } = {}) {
    let engine = null, opening = null, closed = false, cacheBytes = 0;
    const cache = new Map();
    const resourceCost = (key, resource) => resource.rgba.length + (resource.bounds ? key.length * 2 : 0);
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
                            const oldest = cache.keys().next().value; cacheBytes -= resourceCost(oldest, cache.get(oldest)); cache.delete(oldest);
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
            const glows = glowDemands(scene, options); notices.push(...glows.notices);
            for (const { id, plan, sourceTransform, sourceBounds } of glows.plans) {
                try {
                    let resource = cache.get(plan.key);
                    if (resource) { cache.delete(plan.key); cache.set(plan.key, resource); }
                    else {
                        const decoderEngine = await decoder(); alive();
                        try { resource = await rasterizeVectorGlow(plan, { decode: (...args) => decoderEngine.decode(...args), signal }); }
                        catch (error) { const previous = engine; engine = null; opening = null; await Promise.resolve(previous?.close()).catch(() => {}); throw error; }
                        alive();
                        const cost = resource.rgba.length + plan.key.length * 2;
                        while (cacheBytes + cost > CACHE_BYTES && cache.size) {
                            const oldest = cache.keys().next().value, previous = cache.get(oldest);
                            cacheBytes -= resourceCost(oldest, previous); cache.delete(oldest);
                        }
                        if (cost <= CACHE_BYTES) { cache.set(plan.key, resource); cacheBytes += cost; }
                    }
                    frames.set(id, { ...resource, sourceTransform, sourceBounds });
                } catch (error) {
                    alive(); if (error.code === 'INVALID_RESPONSE') throw error;
                    notices.push(`Glow unavailable: ${String(error.message).slice(0, 160)}`);
                }
            }
            return { frames, notices: [...new Set(notices)] };
        },
        async close() { if (closed) return; closed = true; clear(); await engine?.close(); },
    });
}
