const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
function base64(bytes) {
    let text = '';
    for (let i = 0; i < bytes.length; i += 3) {
        const a = bytes[i],
            b = bytes[i + 1],
            c = bytes[i + 2],
            n = (a << 16) | ((b || 0) << 8) | (c || 0);
        text +=
            alphabet[n >>> 18] +
            alphabet[(n >>> 12) & 63] +
            (b === undefined ? '=' : alphabet[(n >>> 6) & 63]) +
            (c === undefined ? '=' : alphabet[n & 63]);
    }
    return text;
}
function bytes(text) {
    if (
        typeof text !== 'string' ||
        text.length % 4 ||
        /[^A-Za-z0-9+/=]/.test(text) ||
        (text.includes('=') && !/^={1,2}$/.test(text.slice(text.indexOf('='))))
    )
        throw TypeError('Invalid image base64');
    const padding = text.endsWith('==') ? 2 : text.endsWith('=') ? 1 : 0,
        out = new Uint8Array((text.length / 4) * 3 - padding);
    let offset = 0;
    for (let i = 0; i < text.length; i += 4) {
        const n =
            (alphabet.indexOf(text[i]) << 18) |
            (alphabet.indexOf(text[i + 1]) << 12) |
            ((alphabet.indexOf(text[i + 2]) & 63) << 6) |
            (alphabet.indexOf(text[i + 3]) & 63);
        for (const shift of [16, 8, 0]) if (offset < out.length) out[offset++] = (n >>> shift) & 255;
    }
    return out;
}
export function encodeVisualImage({ width, height, rgba } = {}) {
    if (
        !Number.isInteger(width) ||
        !Number.isInteger(height) ||
        width < 1 ||
        height < 1 ||
        width > 8192 ||
        height > 8192 ||
        !(rgba instanceof Uint8Array) ||
        rgba.length !== width * height * 4 ||
        rgba.length > 3 * 1024 * 1024
    )
        throw RangeError('Embedded RGBA image exceeds 3 MiB or has invalid dimensions');
    return { type: 'image', encoding: 'rgba-base64', width, height, data: base64(rgba) };
}
export function decodeVisualImage(value) {
    if (
        value?.type !== 'image' ||
        value.encoding !== 'rgba-base64' ||
        !Number.isInteger(value.width) ||
        !Number.isInteger(value.height) ||
        value.width < 1 ||
        value.height < 1 ||
        value.width > 8192 ||
        value.height > 8192 ||
        typeof value.data !== 'string' ||
        value.data.length > 4 * 1024 * 1024
    )
        throw TypeError('Invalid embedded image');
    const rgba = bytes(value.data);
    if (rgba.length !== value.width * value.height * 4)
        throw RangeError('Image byte size differs from its dimensions');
    return { width: value.width, height: value.height, rgba };
}
/** Share one pixel budget across source-aspect samples without dropping assets. */
export function planImageSamples(requests, { maximumPixels = 262144 } = {}) {
    if (!Array.isArray(requests) || requests.length > 4096 || !Number.isSafeInteger(maximumPixels) || maximumPixels < 1 || maximumPixels > 64 * 1024 * 1024)
        throw RangeError('Invalid image sample budget');
    const ids = new Set(), inputs = requests.map(request => {
        if (!request || typeof request.id !== 'string' || !request.id || request.id.length > 256 || ids.has(request.id) ||
            ![request.width, request.height].every(value => Number.isInteger(value) && value >= 1 && value <= 65535) ||
            !Number.isFinite(request.scale) || request.scale <= 0) throw TypeError('Invalid image sample request');
        ids.add(request.id);
        return { id: request.id, width: request.width, height: request.height, scale: Math.min(1, request.scale) };
    });
    if (inputs.length > maximumPixels) throw RangeError('Image budget cannot represent every resource');
    const size = (request, shrink) => ({ maximumWidth: Math.max(1, Math.floor(request.width * request.scale * shrink)),
        maximumHeight: Math.max(1, Math.floor(request.height * request.scale * shrink)) });
    const pixels = shrink => inputs.reduce((sum, request) => { const value = size(request, shrink); return sum + value.maximumWidth * value.maximumHeight; }, 0);
    let shrink = 1;
    if (pixels(1) > maximumPixels) {
        let lower = 0, upper = 1;
        for (let pass = 0; pass < 32; pass++) { const middle = (lower + upper) / 2; if (pixels(middle) <= maximumPixels) lower = middle; else upper = middle; }
        shrink = lower;
    }
    return Object.freeze(inputs.map(request => Object.freeze({ id: request.id, ...size(request, shrink), maximumPixels })));
}

/** Bounded decoded resources shared by visual editors and previews. */
export class VisualResourceStore {
    constructor({ maximumBytes = 16 * 1024 * 1024 } = {}) {
        if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 4 || maximumBytes > 64 * 1024 * 1024)
            throw RangeError('Invalid resource budget');
        this.maximumBytes = maximumBytes;
        this.entries = new Map();
        this.samples = new Map();
        this.sampleBytes = 0;
        this.bytes = 0;
        this.revision = 0;
    }
    set(id, image) {
        if (typeof id !== 'string' || !/^[A-Za-z0-9_.:-]{1,128}$/.test(id))
            throw TypeError('Invalid resource id');
        if (
            !image ||
            !Number.isInteger(image.width) ||
            !Number.isInteger(image.height) ||
            image.width < 1 ||
            image.height < 1 ||
            image.width > 8192 ||
            image.height > 8192 ||
            !(image.rgba instanceof Uint8Array) ||
            image.rgba.length !== image.width * image.height * 4 ||
            image.rgba.length > 3 * 1024 * 1024
        )
            throw RangeError('Invalid decoded visual image');
        const record = { width: image.width, height: image.height, rgba: image.rgba.slice() },
            previous = this.entries.get(id);
        if (this.bytes - (previous?.rgba.length || 0) + record.rgba.length > this.maximumBytes)
            throw RangeError('Visual resource budget exceeded');
        this.bytes += record.rgba.length - (previous?.rgba.length || 0);
        this.entries.set(id, record);
        this.revision++;
        return this;
    }
    get(id) {
        return this.entries.get(id) || null;
    }
    sample(id, width, height, { maximumPixels = 262144 } = {}) {
        const source = this.get(id);
        if (!source) return null;
        if (
            ![width, height, maximumPixels].every(Number.isFinite) ||
            width <= 0 ||
            height <= 0 ||
            maximumPixels < 1
        )
            throw RangeError('Invalid texture sample size');
        const [{ maximumWidth: w, maximumHeight: h }] = planImageSamples([{ id, width: source.width, height: source.height,
            scale: Math.max(Number.MIN_VALUE, Math.min(width / source.width, height / source.height)) }],
            { maximumPixels: Math.min(64 * 1024 * 1024, Math.floor(maximumPixels)) });
        if (w === source.width && h === source.height) return source;
        const key = `${id}:${this.revision}:${w}:${h}`;
        if (this.samples.has(key)) return this.samples.get(key);
        const rgba = new Uint8Array(w * h * 4);
        for (let y = 0; y < h; y++)
            for (let x = 0; x < w; x++) {
                const offset =
                    (Math.floor((y * source.height) / h) * source.width +
                        Math.floor((x * source.width) / w)) *
                    4;
                rgba.set(source.rgba.subarray(offset, offset + 4), (y * w + x) * 4);
            }
        const result = { width: w, height: h, rgba };
        while (this.samples.size && this.sampleBytes + rgba.length > 4 * 1024 * 1024) {
            const first = this.samples.keys().next().value;
            this.sampleBytes -= this.samples.get(first).rgba.length;
            this.samples.delete(first);
        }
        this.samples.set(key, result);
        this.sampleBytes += rgba.length;
        return result;
    }
    delete(id) {
        const item = this.entries.get(id);
        if (!item) return false;
        this.bytes -= item.rgba.length;
        this.revision++;
        return this.entries.delete(id);
    }
    clear() {
        this.entries.clear();
        this.samples.clear();
        this.sampleBytes = 0;
        this.bytes = 0;
        this.revision++;
    }
}

encodeVisualImage.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'encodeVisualImage({width,height,rgba})',
    behavior:
        'Validated portable RGBA resource for visual documents. At most 3 MiB decoded pixels; the image remains complete through structured clipboard and serialization.',
});
decodeVisualImage.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'decodeVisualImage(resource)',
    behavior: 'Validates dimensions and base64 length; returns owned RGBA bytes.',
});
VisualResourceStore.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    constructor: 'new VisualResourceStore({maximumBytes:16777216})',
    methods:
        'set(id,{width,height,rgba}), get(id), sample(id,width,height,{maximumPixels}), delete(id), clear(); bounded decoded and thumbnail caches',
});
planImageSamples.controlFactory = Object.freeze({
    topics: ['visual', 'scene', 'media', 'documents'],
    signature: 'planImageSamples([{id,width,height,scale}],{maximumPixels:262144})',
    behavior: 'Plans immutable integer decode bounds for a collection under one aggregate pixel budget. scale is the desired source-pixel ratio; sources are never enlarged. Preserves source aspect and at least one pixel per asset, rejecting duplicate IDs or an impossible budget. Callers own crop/transform demand, viewport quality and resource lifetime. VisualResourceStore.sample uses this same planner.',
});
