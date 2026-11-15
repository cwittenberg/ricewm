import { failure, object } from './contract-values.mjs';

let catalog = Object.freeze([]), byId = new Map();
const verbs = { M: 2, L: 2, Q: 4, C: 6, Z: 0 };
export function checkedShapeCatalog(reply) {
    if (reply?.ok !== true || !object(reply.result)) throw failure('INVALID_RESPONSE', 'Invalid shape catalog envelope');
    const shapes = reply.result.shapes, ids = new Set();
    const fail = () => { throw failure('INVALID_RESPONSE', 'Invalid native shape catalog'); };
    if (!Array.isArray(shapes) || !shapes.length || shapes.length > 128) fail();
    for (const shape of shapes) {
        if (!shape || typeof shape.id !== 'string' || !/^[a-zA-Z][a-zA-Z0-9-]{0,63}$/.test(shape.id) || ids.has(shape.id) ||
            !['label', 'category'].every(key => typeof shape[key] === 'string' && shape[key].length > 0 && shape[key].length <= 128) ||
            !Array.isArray(shape.path) || shape.path.length > 512) fail();
        for (const segment of shape.path) {
            if (!Array.isArray(segment) || !Object.hasOwn(verbs, segment[0]) || segment.length !== verbs[segment[0]] + 1 ||
                !segment.slice(1).every(value => Number.isFinite(value) && Math.abs(value) <= 10000)) fail();
        }
        ids.add(shape.id);
    }
    return reply;
}
export function installShapeCatalog(reply) {
    checkedShapeCatalog(reply);
    catalog = Object.freeze(reply.result.shapes.map(shape => Object.freeze({ ...shape,
        path: Object.freeze(shape.path.map(segment => Object.freeze([...segment]))) })));
    byId = new Map(catalog.map(shape => [shape.id, shape]));
    return catalog;
}
export const shapes = () => catalog;
export const shapeById = id => byId.get(id);
