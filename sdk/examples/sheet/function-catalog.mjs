import { FUNCTION_CATALOG_ROWS, FUNCTION_CATALOG_SOURCE } from './function-catalog-data.mjs';

export { FUNCTION_CATALOG_SOURCE };
export const FUNCTION_PAGE_SIZE = 6;
export const FUNCTION_CATALOG = Object.freeze(FUNCTION_CATALOG_ROWS.map(([name, category, signature, description]) =>
    Object.freeze({ name, category, signature, description })));
export const FUNCTION_CATEGORIES = Object.freeze([...new Set(FUNCTION_CATALOG.map(item=>item.category))].sort());
const searchText = FUNCTION_CATALOG.map(item => `${item.name} ${item.category} ${item.description}`.toLowerCase());

/** Searches immutable help metadata only. No worksheet data crosses this UI boundary. */
export function functionCatalogMatches(literal = '', category = null) {
    if (typeof literal !== 'string' || literal.length > 128) throw TypeError('Invalid function search');
    if(category!==null&&!FUNCTION_CATEGORIES.includes(category))throw TypeError('Invalid function category');
    const query = literal.trim().toLowerCase(), words = query.split(/\s+/u).filter(Boolean);
    const priority = item => item.name.toLowerCase() === query ? 0 : item.name.toLowerCase().startsWith(query) ? 1 : 2;
    return Object.freeze(FUNCTION_CATALOG.filter((item, index) => (category===null||item.category===category)&&words.every(word => searchText[index].includes(word)))
        .sort((first, second) => priority(first) - priority(second)));
}
/** Kept for existing metadata consumers; the native picker uses the complete
 * matching collection and the host's scrolling instead of pagination. */
export function functionCatalogPage(literal = '', offset = 0) {
    if (!Number.isSafeInteger(offset) || offset < 0) throw TypeError('Invalid function search');
    const matches = functionCatalogMatches(literal), entries = matches.slice(offset, offset + FUNCTION_PAGE_SIZE);
    return Object.freeze({ entries: Object.freeze(entries), offset, total: matches.length,
        next: offset + entries.length < matches.length ? offset + entries.length : null });
}
export function shortFunctionDescription(entry) {
    const sentence=entry.description.replace(/\s+/gu,' ').trim().split(/(?<=[.!?])\s+(?=[A-Z])/u)[0];
    if(sentence.length<=120)return sentence;
    const prefix=sentence.slice(0,117),space=prefix.lastIndexOf(' ');return prefix.slice(0,space>70?space:117)+'…';
}
