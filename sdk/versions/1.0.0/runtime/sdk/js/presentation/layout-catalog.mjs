import { failure, object } from './contract-values.mjs';

let catalog = Object.freeze([]), byId = new Map();
const roles = new Set(['title', 'subtitle', 'body', 'caption', 'picture']);
/** Native catalog is the single source of placeholder geometry for both
 * thumbnails and document commands; the UI never invents a second layout. */
export function checkedLayoutCatalog(reply) {
    const fail = () => { throw failure('INVALID_RESPONSE', 'Invalid native slide layout catalog'); };
    if (reply?.ok !== true || !object(reply.result) || !Array.isArray(reply.result.layouts) ||
        !reply.result.layouts.length || reply.result.layouts.length > 64) fail();
    const ids = new Set();
    for (const layout of reply.result.layouts) {
        if (!object(layout) || !/^[a-z][a-z0-9-]{0,63}$/u.test(layout.id || '') || ids.has(layout.id) ||
            typeof layout.name !== 'string' || !layout.name.trim() || layout.name.length > 128 ||
            !Array.isArray(layout.placeholders) || layout.placeholders.length > 16) fail();
        ids.add(layout.id);
        const indices = new Set();
        for (const item of layout.placeholders) {
            const box = item?.box;
            if (!object(item) || !roles.has(item.role) || !Number.isSafeInteger(item.index) || item.index < 0 ||
                indices.has(item.index) || !object(box) || !['x', 'y', 'width', 'height'].every(key => Number.isFinite(box[key])) ||
                box.x < 0 || box.y < 0 || box.width <= 0 || box.height <= 0 || box.x + box.width > 1 || box.y + box.height > 1) fail();
            indices.add(item.index);
        }
    }
    return reply;
}
export function installLayoutCatalog(reply) {
    checkedLayoutCatalog(reply);
    catalog = Object.freeze(reply.result.layouts.map(layout => Object.freeze({ id: layout.id, name: layout.name,
        placeholders: Object.freeze(layout.placeholders.map(item => Object.freeze({ role: item.role, index: item.index,
            box: Object.freeze({ ...item.box }) }))) })));
    byId = new Map(catalog.map(layout => [layout.id, layout]));
    return catalog;
}
export const layouts = () => catalog;
export const layoutById = id => byId.get(id);
