import { workbookFormat } from './workbook-formats.mjs';

/** Parses the existing trusted RiceApp launcher argument protocol. Descriptors
 * convey no authority by themselves: the protected files host checks ownership.
 */
export function checkedWorkbookGrant(value) {
    if (!value || typeof value !== 'object' || Array.isArray(value) ||
        Object.keys(value).some(key => !['handle', 'name', 'writable'].includes(key)) ||
        typeof value.handle !== 'string' || !value.handle || value.handle.length > 256 ||
        typeof value.name !== 'string' || !value.name || value.name.length > 255 ||
        /[\u0000-\u001f/\\]/u.test(value.name) || value.writable !== false) {
        throw TypeError('Opening a workbook requires one read-only file grant');
    }
    workbookFormat(value.name);
    return Object.freeze({ handle: value.handle, name: value.name, writable: false });
}

export function workbookGrantFromOpen(event) {
    if (event?.paths?.length) throw TypeError('Opening a path requires an authorized RiceWM file grant');
    const args = event?.arguments || [];
    if (!Array.isArray(args)) throw TypeError('Invalid workbook launch arguments');
    if (!args.length) return null;
    if (args.length !== 2 || args[0] !== '--rice-file-grants' || typeof args[1] !== 'string' || args[1].length > 4096) {
        throw TypeError('Invalid workbook launch arguments');
    }
    const grants = JSON.parse(args[1]);
    if (!Array.isArray(grants) || grants.length !== 1) throw TypeError('Open one workbook per Rice Sheet window');
    return checkedWorkbookGrant(grants[0]);
}
