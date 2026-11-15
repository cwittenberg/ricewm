import { validGlow } from './presentation/glow-contract.mjs';

const MAX_PIXELS = 256 * 1024, MAX_SEGMENTS = 16384;
const arity = { M: 2, L: 2, Q: 4, C: 6, Z: 0 };
const plans = new WeakSet(), encoder = new TextEncoder();
const number = value => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e12;
const fail = message => { throw new RangeError('Vector glow: ' + message); };

/** Raster demand for a transformed vector silhouette. Radius is expressed in
 * local scene units; Gaussian filtering happens in world space, so nonuniform
 * scaling does not turn the blur into an ellipse. No document state is kept. */
export function vectorGlowPlan({ paths, transform = [1, 0, 0, 1, 0, 0], fill = false, glow } = {},
    { scale = 1, maximumPixels = MAX_PIXELS } = {}) {
    if (!validGlow(glow) || typeof fill !== 'boolean' || !Array.isArray(transform) || transform.length !== 6 || !transform.every(number) ||
        !Number.isFinite(scale) || scale <= 0 || scale > 64 || !Number.isSafeInteger(maximumPixels) || maximumPixels < 1 || maximumPixels > MAX_PIXELS ||
        !Array.isArray(paths) || !paths.length || paths.length > 256) fail('invalid silhouette or raster budget');
    let segments = 0, minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    const [a, b, c, d, e, f] = transform;
    const data = paths.map(path => {
        if (!Array.isArray(path) || !path.length || path[0]?.[0] !== 'M') fail('invalid path');
        return path.map(segment => {
            if (++segments > MAX_SEGMENTS || !Array.isArray(segment) || !Object.hasOwn(arity, segment[0]) ||
                segment.length !== arity[segment[0]] + 1 || !segment.slice(1).every(number)) fail('invalid path segment');
            for (let i = 1; i < segment.length; i += 2) {
                const x = a * segment[i] + c * segment[i + 1] + e, y = b * segment[i] + d * segment[i + 1] + f;
                if (!number(x) || !number(y)) fail('transformed coordinate limit');
                minX = Math.min(minX, x); minY = Math.min(minY, y); maxX = Math.max(maxX, x); maxY = Math.max(maxY, y);
            }
            return segment[0] + segment.slice(1).join(' ');
        }).join(' ');
    }).join(' ');
    if (!Number.isFinite(minX)) fail('empty silhouette');
    if (glow.radius === 0 || glow.opacity === 0) return null;
    // The pinned reference renderer uses a .95r round stroke, a .17r Gaussian,
    // and sqrt(abs(det)) as the world scale. Keep the same geometric semantics.
    const worldScale = Math.sqrt(Math.abs(a * d - b * c));
    if (worldScale < 1e-12) return null;
    const sigmaWorld = glow.radius * .17 * worldScale;
    const halfStroke = glow.radius * .475 * Math.max(Math.hypot(a, c), Math.hypot(b, d));
    const pad = halfStroke + 4 * sigmaWorld + 2 / scale;
    const bounds = [minX - pad, minY - pad, maxX - minX + 2 * pad, maxY - minY + 2 * pad];
    if (!bounds.every(number) || bounds[2] <= 0 || bounds[3] <= 0) fail('invalid visual bounds');
    // Bounded separable convolution: <=32px sigma and <=256 Ki pixels. The
    // full world extent is retained when large effects require downsampling.
    let ratio = Math.min(scale, 32 / Math.max(sigmaWorld, 1e-12), 2048 / bounds[2], 2048 / bounds[3], Math.sqrt(maximumPixels / (bounds[2] * bounds[3])));
    let width = Math.max(1, Math.floor(bounds[2] * ratio)), height = Math.max(1, Math.floor(bounds[3] * ratio));
    if (width * height > maximumPixels) { ratio *= Math.sqrt(maximumPixels / (width * height)); width = Math.max(1, Math.floor(bounds[2] * ratio)); height = Math.max(1, Math.floor(bounds[3] * ratio)); }
    if (width * height > maximumPixels) fail('raster budget too small for aspect ratio');
    // Preserve aspect ratio with independent SVG viewport scaling; convolution
    // uses the actual x/y scale after integer pixel dimensions are chosen.
    const sigma = [sigmaWorld * width / bounds[2], sigmaWorld * height / bounds[3]];
    const source = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${bounds.join(' ')}" preserveAspectRatio="none"><path d="${data}" transform="matrix(${transform.join(' ')})" fill="${fill ? '#fff' : 'none'}" stroke="#fff" stroke-width="${glow.radius * .95}" stroke-linejoin="round"/></svg>`;
    if (source.length > 2 * 1024 * 1024) fail('vector source limit');
    const plan = Object.freeze({ bounds: Object.freeze(bounds), width, height, sigma: Object.freeze(sigma),
        color: glow.color.toUpperCase(), opacity: glow.opacity, source,
        key: `glow:${glow.color.toUpperCase()}/${glow.opacity}/${sigma.join('/')}/${source}` });
    plans.add(plan); return plan;
}

function kernel(sigma) {
    if (sigma < .01) return Float64Array.of(1);
    const radius = Math.ceil(sigma * 4), weights = new Float64Array(radius * 2 + 1);
    let total = 0;
    for (let i = -radius; i <= radius; i++) total += weights[i + radius] = Math.exp(-.5 * (i / sigma) ** 2);
    for (let i = 0; i < weights.length; i++) weights[i] /= total;
    return weights;
}
/** Decode vector coverage through an approved caller-owned image engine and
 * blur only the alpha plane. RGB remains straight alpha for SceneBuilder.
 * The returned image contains the effect alone, with no crisp foreground. */
export async function rasterizeVectorGlow(plan, { decode, signal } = {}) {
    if (!plans.has(plan) || typeof decode !== 'function') fail('use a valid plan and an approved decoder');
    const alive = () => { if (signal?.aborted) throw Object.assign(new Error('Vector glow closed'), { code: 'CLOSED' }); };
    alive();
    const mask = await decode(encoder.encode(plan.source), { maximumWidth: plan.width, maximumHeight: plan.height, maximumPixels: plan.width * plan.height });
    alive();
    const { width, height } = plan, count = width * height;
    if (mask.width !== width || mask.height !== height || !(mask.rgba instanceof Uint8Array) || mask.rgba.length !== count * 4) throw Object.assign(new Error('Invalid vector coverage'), { code: 'INVALID_RESPONSE' });
    const horizontal = new Float32Array(count), rgba = new Uint8Array(count * 4);
    const kx = kernel(plan.sigma[0]), ky = kernel(plan.sigma[1]), rx = (kx.length - 1) / 2, ry = (ky.length - 1) / 2;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
        let sum = 0;
        for (let i = Math.max(-rx, -x); i <= Math.min(rx, width - 1 - x); i++) sum += mask.rgba[(y * width + x + i) * 4 + 3] * kx[i + rx];
        horizontal[y * width + x] = sum;
    }
    const color = Number.parseInt(plan.color.slice(1), 16), red = color >>> 16, green = color >>> 8 & 255, blue = color & 255;
    for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
        let sum = 0;
        for (let i = Math.max(-ry, -y); i <= Math.min(ry, height - 1 - y); i++) sum += horizontal[(y + i) * width + x] * ky[i + ry];
        const at = (y * width + x) * 4, alpha = Math.round(sum * plan.opacity);
        if (alpha) { rgba[at] = red; rgba[at + 1] = green; rgba[at + 2] = blue; rgba[at + 3] = alpha; }
    }
    alive();
    return Object.freeze({ width, height, rgba, bounds: plan.bounds, key: plan.key });
}
vectorGlowPlan.controlFactory = Object.freeze({ topics: ['drawing'], constructor: 'vectorGlowPlan({paths,transform,fill,glow},{scale,maximumPixels})',
    methods: 'Bounded transformed silhouette demand for a whole-shape glow. Paths use M/L/Q/C/Z segments; glow has color, opacity, radius. Returns null for an invisible effect. No document authority or external resources.' });
rasterizeVectorGlow.controlFactory = Object.freeze({ topics: ['drawing'], constructor: 'await rasterizeVectorGlow(plan,{decode:engine.decode,signal})',
    methods: 'Uses an approved caller-owned image engine to rasterize self-contained vector coverage, then performs bounded world-space Gaussian alpha filtering. Returns straight-alpha RGBA and world bounds for SceneBuilder.defineImage/image; paint before the foreground. No crisp silhouette or box approximation.' });
