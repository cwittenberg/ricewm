import { exact, failure } from './contract-values.mjs';

export const SELECTION_BYTES = 4 * 1024 * 1024;
export const CLIPBOARD_TEXT_BYTES = 6 * 1024 * 1024;
export const SELECTION_MIME = 'application/vnd.ricewm.deck-selection+zip';
const SCHEMA = 'ricewm.deck.selection/v1';
const identity = value => typeof value === 'string' && value.length > 0 && new TextEncoder().encode(value).length <= 1024 && !/[\ud800-\udfff]/u.test(value);
const count = value => Number.isSafeInteger(value) && value >= 1 && value <= 32;
export const selectionToken = value => typeof value === 'string' && /^selection:sha256:[0-9a-f]{64}$/.test(value);
export function selectionArguments(args) {
    if (!exact(args, ['slide', 'objects']) || !identity(args.slide) || !Array.isArray(args.objects) || !count(args.objects.length) ||
        !args.objects.every(identity) || new Set(args.objects).size !== args.objects.length) throw failure('INVALID_REQUEST', 'Choose 1 to 32 distinct presentation objects');
    return { slide: args.slide, objects: [...args.objects] };
}
export function checkedSelectionReply(reply, operation, expected) {
    const value = reply?.result;
    const valid = operation === 'export-selection'
        ? exact(value, ['length', 'objectCount', 'mime', 'version']) && count(value.objectCount) && value.mime === SELECTION_MIME && value.version === 1 && Number.isSafeInteger(value.length) && value.length > 0 && value.length <= SELECTION_BYTES
        : operation === 'prepare-selection' && exact(value, ['selection', 'objectCount']) && selectionToken(value.selection) && count(value.objectCount);
    if (reply?.ok !== true || reply.revision !== expected || !valid) throw failure('INVALID_RESPONSE', 'Invalid native selection transfer');
    return reply;
}
function base64(bytes) {
    let binary = '';
    for (let offset = 0; offset < bytes.length; offset += 32768) binary += String.fromCharCode(...bytes.subarray(offset, offset + 32768));
    return btoa(binary);
}
export function encodeSelectionClipboard(bytes) {
    if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > SELECTION_BYTES) throw failure('CLIPBOARD_LIMIT', 'Selection exceeds the 4 MiB clipboard limit');
    return JSON.stringify({ schema: SCHEMA, mime: SELECTION_MIME, version: 1, data: base64(bytes) });
}
export function decodeSelectionClipboard(text) {
    const invalid = () => failure('INVALID_CLIPBOARD', 'The clipboard does not contain a supported Deck selection');
    if (typeof text !== 'string' || !text.length || text.length > CLIPBOARD_TEXT_BYTES) throw invalid();
    let value;
    try { value = JSON.parse(text); } catch { throw invalid(); }
    if (!exact(value, ['schema', 'mime', 'version', 'data']) || value.schema !== SCHEMA || value.mime !== SELECTION_MIME || value.version !== 1 ||
        typeof value.data !== 'string' || !value.data.length || value.data.length > 4 * Math.ceil(SELECTION_BYTES / 3) ||
        value.data.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(value.data)) throw invalid();
    let binary;
    try { binary = atob(value.data); } catch { throw invalid(); }
    if (!binary.length || binary.length > SELECTION_BYTES || btoa(binary) !== value.data) throw invalid();
    return Uint8Array.from(binary, character => character.charCodeAt(0));
}
