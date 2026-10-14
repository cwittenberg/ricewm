/** Shared source-backed arrangement commands; icons are RiceUI layer actions. */
export const objectOrderingActions = Object.freeze([
    ['forward', 'bringForward', 'Bring Forward', 196],
    ['front', 'bringToFront', 'Bring to Front', 196],
    ['backward', 'sendBackward', 'Send Backward', 197],
    ['back', 'sendToBack', 'Send to Back', 197],
].map(([direction, action, label, icon]) => Object.freeze({ direction, action, label, icon })));

export function canReorderObjects(objects) {
    return Array.isArray(objects) && objects.length > 0 && objects.length <= 32 &&
        new Set(objects.map(object => object.id)).size === objects.length &&
        objects.every(object => object.edit?.canReorder === true &&
            (object.parentId ?? null) === (objects[0].parentId ?? null));
}
