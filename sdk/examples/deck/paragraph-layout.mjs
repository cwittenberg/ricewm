// Source paragraph boundaries use UTF-8 offsets, matching native glyph clusters.
const encoder = new TextEncoder(), decoder = new TextDecoder('utf-8', { fatal: true });
export const validSpacing = (value, allowNormal = false) => value === null || value &&
    (allowNormal && value.kind === 'normal' && value.value === 1 ||
    ['multiple', 'exact'].includes(value.kind) && Number.isFinite(value.value) && value.value >= 0 &&
    value.value <= (value.kind === 'multiple' ? 100 : 65536));

export function sourceParagraphs(source, runs, resolveStyle) {
    if (!Array.isArray(source.paragraphs) || !source.paragraphs.length || source.paragraphs.length > 256) {
        throw new RangeError('Invalid source paragraph count');
    }
    const bytes = encoder.encode(source.text);
    let at = 0;
    const intervals = runs.map(run => {
        const start = at; at += encoder.encode(run.text).length;
        return { start, end: at, style: run.style };
    });
    if (at !== bytes.length) throw new TypeError('Source runs do not cover paragraphs');
    at = 0;
    return source.paragraphs.map((paragraph, index) => {
        const { start, end } = paragraph;
        if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start !== at || end < start || end > bytes.length ||
            !['lineHeight', 'spaceBefore', 'spaceAfter'].every(key => validSpacing(paragraph[key], key === 'lineHeight'))) {
            throw new TypeError('Invalid source paragraph layout');
        }
        const text = decoder.decode(bytes.subarray(start, end));
        if (index + 1 < source.paragraphs.length ? bytes[end] !== 10 : end !== bytes.length) {
            throw new TypeError('Source paragraphs do not cover text');
        }
        at = end + 1;
        const style = resolveStyle(paragraph.style), spans = [];
        for (const interval of intervals) {
            const left = Math.max(start, interval.start), right = Math.min(end, interval.end);
            if (left >= right) continue;
            const { alignment, ...paint } = interval.style;
            spans.push({ start: left - start, end: right - start, ...paint });
        }
        return { text, spans, style, offset: start, lineHeight: paragraph.lineHeight,
            spaceBefore: index === 0 && source.layout?.useParagraphSpacing === false ? null : paragraph.spaceBefore,
            spaceAfter: index + 1 === source.paragraphs.length && source.layout?.useParagraphSpacing === false ? null : paragraph.spaceAfter };
    });
}

export function paragraphSpace(spacing, fontSize) {
    if (!spacing) return 0;
    if (!validSpacing(spacing)) throw new TypeError('Invalid paragraph spacing');
    if (spacing.kind === 'exact') return spacing.value;
    if (!Number.isFinite(fontSize) || fontSize < 0 || fontSize > 4096) throw new TypeError('Paragraph font metrics unavailable');
    return spacing.value * fontSize;
}
