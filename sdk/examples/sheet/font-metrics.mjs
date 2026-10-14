/** Workbook font measurements are bounded metadata. Worksheet coordinates and
 * OOXML width conversion remain in the workbook engine. */
const fail = message => { throw Object.assign(new TypeError(message), {code:'INVALID_FONT_METRICS'}); };
const exact = (value, keys) => value && Object.getPrototypeOf(value) === Object.prototype &&
    Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value,key));
const text = (value, max) => typeof value === 'string' && value.length > 0 &&
    new TextEncoder().encode(value).length <= max && !/[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(value);

export function checkedImportFonts(fonts) {
    if (!Array.isArray(fonts) || fonts.length > 16) fail('Invalid workbook font request');
    const ids = new Set();
    return Object.freeze(fonts.map(font => {
        if (!exact(font,['id','family','sizePt','bold','italic']) || !text(font.id,128) || ids.has(font.id) ||
            !text(font.family,256) || !Number.isFinite(font.sizePt) || font.sizePt < 1 || font.sizePt > 409 ||
            typeof font.bold !== 'boolean' || typeof font.italic !== 'boolean') fail('Invalid workbook font descriptor');
        ids.add(font.id); return Object.freeze({...font});
    }));
}

export function checkedFontMetrics(fonts, metrics) {
    if (!Array.isArray(metrics) || metrics.length !== fonts.length) fail('Incomplete workbook font measurements');
    const wanted = new Set(fonts.map(font=>font.id)), found = new Set();
    return Object.freeze(metrics.map(metric => {
        if (!exact(metric,['id','maxDigitWidthPx']) || !wanted.has(metric.id) || found.has(metric.id) ||
            !Number.isFinite(metric.maxDigitWidthPx) || metric.maxDigitWidthPx <= 0 || metric.maxDigitWidthPx > 4096)
            fail('Invalid workbook digit width');
        found.add(metric.id); return Object.freeze({...metric});
    }));
}

/** Reuse the Office typography worker and packaged fonts. Open lazily, shape
 * individual digits (no pair kerning), and close before committing the import.
 * Cache only measurements, never a second persistent typography worker. */
export function createFontMetricsResolver(openEngine) {
    if (typeof openEngine !== 'function') fail('Font measurement requires a typography engine');
    const cache = new Map();
    let pending = false;
    return async requested => {
        const fonts = checkedImportFonts(requested);
        if (pending) fail('A workbook font measurement is already running');
        pending = true;
        let engine;
        try {
            const metrics = [];
            for (const font of fonts) {
                const key = JSON.stringify([font.family,font.sizePt,font.bold,font.italic]);
                let width = cache.get(key);
                if (width === undefined) {
                    engine ||= await openEngine();
                    const family=engine.resolveDocumentFamily ? engine.resolveDocumentFamily(font.family) : font.family;
                    width = 0;
                    for (const digit of '0123456789') {
                        const frame = await engine.shape({text:digit,families:family,size:font.sizePt*4/3,
                            bold:font.bold,italic:font.italic,width:null});
                        if (!Array.isArray(frame?.glyphs) || !frame.glyphs.length || frame.glyphs.length > 16)
                            fail('Typography did not return digit advances');
                        let advance = 0;
                        for (const glyph of frame.glyphs) {
                            if (!Number.isFinite(glyph.advance) || glyph.advance < 0 || glyph.advance > 4096)
                                fail('Typography returned an invalid digit advance');
                            advance += glyph.advance;
                        }
                        if (advance <= 0 || advance > 4096) fail('Typography returned an unsupported digit width');
                        width = Math.max(width,advance);
                    }
                    if (cache.size >= 16) cache.delete(cache.keys().next().value);
                    cache.set(key,width);
                }
                metrics.push({id:font.id,maxDigitWidthPx:width});
            }
            return checkedFontMetrics(fonts,metrics);
        } finally {
            try { await engine?.close(); } finally { pending = false; }
        }
    };
}
