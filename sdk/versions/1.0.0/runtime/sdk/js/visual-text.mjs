import { openTypographyEngine } from './typography-engine.mjs';
import { richTextPlainText } from './visual-document.mjs';
import { rgbaColor } from './visual-geometry.mjs';

/** Shared cached native shaping for styled and rotated object labels. */
export async function createVisualTextRenderer({
    services,
    engine: provided,
    maximumBytes = 8 * 1024 * 1024,
    ...options
} = {}) {
    if (!Number.isInteger(maximumBytes) || maximumBytes < 1024 || maximumBytes > 16 * 1024 * 1024)
        throw RangeError('Invalid text cache budget');
    const engine = provided ?? (await openTypographyEngine({ services, ...options })),
        cache = new Map(),
        pending = new Map(),
        failures = new Map();
    let sequence = 0,
        bytes = 0,
        queue = Promise.resolve(),
        closed = false;
    return {
        request(object, foreground) {
            if (closed) throw Error('Visual text renderer is closed');
            const style = object.style || {},
                text = object.richText ? richTextPlainText(object.richText) : object.text || '',
                key = JSON.stringify([
                    object.id,
                    text,
                    object.width,
                    object.height,
                    object.richText,
                    style,
                    foreground,
                ]);
            if (cache.has(key)) return { key, frame: cache.get(key), pending: null };
            if (failures.has(key)) return { key, frame: null, pending: null, error: failures.get(key) };
            if (!pending.has(key)) {
                const snapshot = structuredClone(object),
                    work = queue
                        .catch(() => {})
                        .then(async () => {
                            if (closed) throw Error('Visual text renderer closed');
                            const s = snapshot.style || {},
                                families = s.fontFamily || 'Liberation Sans, sans-serif',
                                size = s.fontSize || 16,
                                color = rgbaColor(s.foreground || foreground),
                                spans = [];
                            let offset = 0;
                            if (snapshot.richText) {
                                const encoder = new TextEncoder();
                                for (const [i, block] of snapshot.richText.entries()) {
                                    if (i) {
                                        spans.push({
                                            start: offset,
                                            end: offset + 1,
                                            families,
                                            size,
                                            bold: false,
                                            italic: false,
                                            underline: false,
                                            color,
                                        });
                                        offset++;
                                    }
                                    for (const run of block.runs) {
                                        const end = offset + encoder.encode(run.text).length;
                                        if (end > offset)
                                            spans.push({
                                                start: offset,
                                                end,
                                                families,
                                                size,
                                                bold: !!run.bold || block.type === 'heading',
                                                italic: !!run.italic,
                                                underline: !!run.underline || !!run.link,
                                                color,
                                            });
                                        offset = end;
                                    }
                                }
                            }
                            const frame = await engine.shape({
                                text,
                                families,
                                size,
                                width: Math.max(1, Math.min(16384, snapshot.width - 16)),
                                bold: !!s.bold,
                                italic: !!s.italic,
                                underline: !!s.underline,
                                alignment: 'center',
                                color,
                                pathBase: 0x10000000 + (sequence++ % 65536) * 8192,
                                spans,
                            });
                            if (closed) throw Error('Visual text renderer closed');
                            if (frame.displayList.length > maximumBytes)
                                throw RangeError('Shaped object text exceeds cache budget');
                            while (
                                cache.size &&
                                (cache.size >= 256 || bytes + frame.displayList.length > maximumBytes)
                            ) {
                                const first = cache.keys().next().value;
                                bytes -= cache.get(first).displayList.length;
                                cache.delete(first);
                            }
                            cache.set(key, frame);
                            bytes += frame.displayList.length;
                            return frame;
                        })
                        .catch((error) => {
                            failures.set(key, error.message);
                            if (failures.size > 256) failures.delete(failures.keys().next().value);
                            throw error;
                        })
                        .finally(() => pending.delete(key));
                pending.set(key, work);
                queue = work;
            }
            return { key, frame: null, pending: pending.get(key) };
        },
        async whenSettled() {
            await queue;
        },
        async close() {
            closed = true;
            cache.clear();
            failures.clear();
            bytes = 0;
            if (!provided) await engine.close();
        },
        get bytes() {
            return bytes;
        },
    };
}
createVisualTextRenderer.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'await createVisualTextRenderer({services})',
    behavior:
        'Bind as ObjectCanvas({textRenderer}). Uses sdk.resources typography, actual Parley/Skrifa shaping, bounded cached glyph paths and serialized compute. Supports rich runs, custom font styles, wrapping and rotated text. close() releases its owned worker.',
});
