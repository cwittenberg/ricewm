import { failure, object } from './contract-values.mjs';

export const FORMATS = Object.freeze(['pptx', 'potx', 'ppsx', 'odp', 'otp']);
const semanticRejection = new WeakSet();
export const normalizeError = cause => cause instanceof Error ? cause : failure('ENGINE_UNAVAILABLE', 'Deck transport failed');
export function rejectedRevision(cause, minimum) {
    if (!semanticRejection.has(cause) || cause.rejectedBeforeMutation !== true) return null;
    try {
        const value = revision(cause.revision);
        return BigInt(value) >= BigInt(minimum) ? value : null;
    } catch { return null; }
}
export function revision(value) {
    if (typeof value !== 'string' || !/^(0|[1-9][0-9]{0,19})$/.test(value) || BigInt(value) > 18446744073709551615n) {
        throw failure('INVALID_REVISION', 'Expected a canonical decimal u64 revision');
    }
    return value;
}
export function integer(value, maximum = 0xffffffff) { return Number.isSafeInteger(value) && value >= 0 && value <= maximum; }
export function text(value, maximum) {
    return typeof value === 'string' && value.length <= maximum && !/[\ud800-\udfff]/u.test(value);
}
export function checkedResponse(value) {
    if (!object(value) || typeof value.ok !== 'boolean') throw failure('INVALID_RESPONSE', 'Invalid Deck response envelope');
    if (value.revision !== undefined) revision(value.revision);
    if (!value.ok) {
        const error = value.error;
        if (!object(error) || typeof error.code !== 'string' || !/^[A-Z_]{1,64}$/.test(error.code) || !text(error.message, 1024) ||
            (value.rejectedBeforeMutation !== undefined && typeof value.rejectedBeforeMutation !== 'boolean')) {
            throw failure('INVALID_RESPONSE', 'Invalid Deck error envelope');
        }
        const cause = failure(error.code, error.message);
        if (value.revision !== undefined) cause.revision = value.revision;
        cause.rejectedBeforeMutation = value.rejectedBeforeMutation === true;
        semanticRejection.add(cause);
        throw cause;
    }
    if (!object(value.result)) throw failure('INVALID_RESPONSE', 'Missing Deck operation result');
    return value;
}
export function checkedStatus(value) {
    checkedResponse(value);
    revision(value.revision);
    const data = value.result;
    if (!FORMATS.includes(data.format) || !text(data.mime, 256) || !data.mime || !integer(data.slideCount, 4096) ||
        !['dirty', 'canUndo', 'canRedo'].every(key => typeof data[key] === 'boolean')) {
        throw failure('INVALID_RESPONSE', 'Invalid Deck document status');
    }
    return value;
}
