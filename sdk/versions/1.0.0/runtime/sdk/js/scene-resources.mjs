import { copySceneFrame, MAX_SCENE_BYTES } from './scene-surface.mjs';

/** Give independently produced frames disjoint resource identities before
 * composing them. Prepare once, then retain these copies across animation ticks.
 * Each input remains independently valid and its bytes are never mutated. */
export function remapSceneFrames(frames) {
    if (!Array.isArray(frames) || frames.length > 32) throw new RangeError('Invalid scene composition');
    let total = 0;
    for (const frame of frames) {
        if (!(frame?.displayList instanceof Uint8Array)) throw new TypeError('Expected scene frame');
        total += Math.max(28, frame.displayList.length);
        if (total > MAX_SCENE_BYTES) throw new RangeError('Scene composition exceeds its byte budget');
    }
    let nextImage = 1, nextPath = 1;
    return frames.map(frame => {
        const result = copySceneFrame(frame), view = new DataView(result.displayList.buffer);
        const images = new Map(), paths = new Map();
        for (let offset = 0; offset < view.byteLength;) {
            const op = view.getUint8(offset), compact = view.getUint16(offset + 2, true);
            const length = compact ? compact * 4 : view.getUint32(offset + 4, true);
            const isImage = op === 7 || op === 14;
            if (isImage || [19, 20, 21, 22].includes(op)) {
                const field = op === 7 ? 40 : 8;
                if (length < field + 4) throw new RangeError('Truncated scene resource');
                const id = view.getUint32(offset + field, true), map = isImage ? images : paths;
                if (!id) throw new RangeError('Invalid scene resource identity');
                if (op === 14 || op === 19) {
                    if (map.has(id)) throw new RangeError('Duplicate scene resource');
                    map.set(id, isImage ? nextImage++ : nextPath++);
                }
                if (!map.has(id)) throw new RangeError('Scene resource must be defined before use');
                view.setUint32(offset + field, map.get(id), true);
            }
            offset += length;
        }
        return result;
    });
}

/** Assemble independently retained command ranges, validating the complete
 * resource/stack closure before it can reach a native surface. */
export function composeSceneFrame(width, height, chunks) {
    if (!Array.isArray(chunks) || chunks.length > 32768) throw new RangeError('Invalid scene composition');
    let length = 0;
    for (const chunk of chunks) {
        if (!(chunk instanceof Uint8Array)) throw new TypeError('Expected scene command bytes');
        length += chunk.length;
        if (length > MAX_SCENE_BYTES) throw new RangeError('Scene composition exceeds its byte budget');
    }
    const displayList = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { displayList.set(chunk, offset); offset += chunk.length; }
    return copySceneFrame({ width, height, displayList });
}
