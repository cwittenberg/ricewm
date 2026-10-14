import { MAX_SCENE_BYTES } from './scene-surface.mjs';

/** Existing RAPP WebRenderPatch envelope; native widget kind selects the
 * vector-only SceneSurface validator, without creating a second renderer. */
export function encodeScenePatch({ app, window, surface, epoch, width, height, displayList }) {
    if (!(displayList instanceof Uint8Array) || displayList.length > MAX_SCENE_BYTES) throw new RangeError('Scene exceeds wire budget');
    const output = new Uint8Array(56 + displayList.length);
    const view = new DataView(output.buffer);
    output.set([0x52, 0x41, 0x50, 0x50]);
    view.setUint16(4, 1, true);
    view.setUint16(6, 22, true);
    view.setUint32(8, output.length - 12, true);
    for (const [index, value] of [app, window, surface, epoch].entries()) view.setBigUint64(12 + index * 8, BigInt(value), true);
    view.setUint32(44, width, true);
    view.setUint32(48, height, true);
    view.setUint32(52, displayList.length, true);
    output.set(displayList, 56);
    return output;
}
