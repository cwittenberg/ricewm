import { packedRgba } from './rice-native-canvas-primitives.mjs';

// A shadow is a blurred rectangle mask, not a bounding-box approximation of
// arbitrary shape content. Blur is the protocol's Gaussian standard deviation.
const MAX_PIXELS = 2 * 1024 * 1024;
const MAX_CACHE_BYTES = 8 * 1024 * 1024;
const shadows = new Map();
let cacheBytes = 0;

function mask(context, width, height, offset, spread, inset, bounds) {
    const [dx, dy] = offset;
    if (!inset) {
        context.fillRect(dx - spread, dy - spread, width + spread * 2, height + spread * 2);
        return;
    }
    context.beginPath();
    context.rect(...bounds);
    const holeWidth = width - spread * 2, holeHeight = height - spread * 2;
    if (holeWidth > 0 && holeHeight > 0) context.rect(dx + spread, dy + spread, holeWidth, holeHeight);
    context.fill('evenodd');
}

// Gaussian integration of an axis-aligned rectangle is separable. Computing
// this small mask also avoids browser-dependent Canvas shadowBlur conventions
// and Canvas filter support. The error-function approximation is <1.5e-7.
function erf(value) {
    const sign = value < 0 ? -1 : 1, x = Math.abs(value), t = 1 / (1 + 0.3275911 * x);
    return sign * (1 - (((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t) * Math.exp(-x * x));
}
function coverage(position, start, length, sigma) {
    return Math.max(0, Math.min(1, (erf((start + length - position) / (sigma * Math.SQRT2)) -
        erf((start - position) / (sigma * Math.SQRT2))) / 2));
}
function blurMask(painter, bounds, width, height, offset, blur, spread, color, inset, scale) {
    const pixels = painter.createImageData(painter.canvas.width, painter.canvas.height);
    const xs = new Float64Array(pixels.width), ys = new Float64Array(pixels.height);
    const amount = inset ? -spread : spread;
    const left = offset[0] - amount, top = offset[1] - amount;
    const boxWidth = width + amount * 2, boxHeight = height + amount * 2;
    if (boxWidth > 0 && boxHeight > 0) {
        for (let x = 0; x < xs.length; x++) xs[x] = coverage(bounds[0] + (x + 0.5) / scale, left, boxWidth, blur);
        for (let y = 0; y < ys.length; y++) ys[y] = coverage(bounds[1] + (y + 0.5) / scale, top, boxHeight, blur);
    }
    const red = color & 255, green = (color >>> 8) & 255, blue = (color >>> 16) & 255, alpha = color >>> 24;
    for (let y = 0, at = 0; y < ys.length; y++) for (let x = 0; x < xs.length; x++, at += 4) {
        const value = xs[x] * ys[y];
        pixels.data[at] = red; pixels.data[at + 1] = green; pixels.data[at + 2] = blue;
        pixels.data[at + 3] = Math.round(alpha * (inset ? 1 - value : value));
    }
    painter.putImageData(pixels, 0, 0);
}

function sampling(context, width, height) {
    const { a, b, c, d } = context.getTransform();
    // Largest singular value keeps a rotated/sheared mask sharp on both axes.
    const sum = a * a + b * b + c * c + d * d, determinant = a * d - b * c;
    const scale = Math.sqrt((sum + Math.sqrt(Math.max(0, sum * sum - 4 * determinant * determinant))) / 2) || 1;
    return Math.min(scale, 4096 / width, 4096 / height, Math.sqrt(MAX_PIXELS / width / height));
}

/** Paint the native BoxShadow primitive in local coordinates, then transform
 * its mask as a unit so offsets and Gaussian blur follow rotation/shear/zoom.
 * An inset shadow is the inverse spread rectangle clipped to the original box.
 * Outset masks include their interior, as in the native protocol's reference
 * painter; callers place foreground content above the shadow themselves. */
export function renderRiceBoxShadow(context, rect, offset, blur, spread, color, inset) {
    const { x, y, width, height } = rect;
    if (width <= 0 || height <= 0 || (color >>> 24) === 0) return;
    if (!inset && (width + spread * 2 <= 0 || height + spread * 2 <= 0)) return;
    const padding = blur * 4;
    const bounds = inset ? [0, 0, width, height]
        : [offset[0] - spread - padding, offset[1] - spread - padding,
            width + spread * 2 + padding * 2, height + spread * 2 + padding * 2];
    context.save();
    try {
        context.translate(x, y);
        if (inset) { context.beginPath(); context.rect(0, 0, width, height); context.clip(); }
        if (blur === 0) {
            context.fillStyle = packedRgba(color);
            mask(context, width, height, offset, spread, inset, bounds);
            return;
        }
        const scale = sampling(context, bounds[2], bounds[3]);
        const pixelsWide = Math.max(1, Math.floor(bounds[2] * scale));
        const pixelsHigh = Math.max(1, Math.floor(bounds[3] * scale));
        const key = [width, height, ...offset, blur, spread, color, inset, pixelsWide, pixelsHigh].join('/');
        let surface = shadows.get(key);
        if (surface) { shadows.delete(key); shadows.set(key, surface); }
        else {
            surface = typeof globalThis.OffscreenCanvas === 'function'
                ? new OffscreenCanvas(pixelsWide, pixelsHigh) : context.canvas.ownerDocument.createElement('canvas');
            surface.width = pixelsWide; surface.height = pixelsHigh;
            const painter = surface.getContext('2d');
            // Match the allocated raster, using a uniform scale. The outer
            // four-sigma padding is transparent and absorbs subpixel rounding.
            const rasterScale = Math.min(pixelsWide / bounds[2], pixelsHigh / bounds[3]);
            blurMask(painter, bounds, width, height, offset, blur, spread, color, inset, rasterScale);
            surface.shadowBounds = [bounds[0], bounds[1], pixelsWide / rasterScale, pixelsHigh / rasterScale];
            const cost = pixelsWide * pixelsHigh * 4;
            while (shadows.size && (cacheBytes + cost > MAX_CACHE_BYTES || shadows.size >= 16)) {
                const oldest = shadows.keys().next().value, dropped = shadows.get(oldest);
                cacheBytes -= dropped.width * dropped.height * 4; shadows.delete(oldest);
                dropped.width = 0; dropped.height = 0;
            }
            shadows.set(key, surface); cacheBytes += cost;
        }
        context.drawImage(surface, ...surface.shadowBounds);
    } finally { context.restore(); }
}
