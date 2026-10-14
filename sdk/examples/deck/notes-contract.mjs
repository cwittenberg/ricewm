import { exact, failure } from './contract-values.mjs';

export const MAX_SPEAKER_NOTES_BYTES = 16 * 1024;
const identifier = value => typeof value === 'string' && value.length > 0 && value.length <= 4096 && !/[\ud800-\udfff]/u.test(value);
export const validNotesText = value => typeof value === 'string' &&
    !/[\u0000-\u0008\u000b-\u001f\ufffe\uffff\ud800-\udfff]/u.test(value) &&
    new TextEncoder().encode(value).length <= MAX_SPEAKER_NOTES_BYTES;

export function notesArguments(value) {
    if (!exact(value, ['slide']) || !identifier(value.slide)) throw failure('INVALID_REQUEST', 'Invalid speaker notes slide');
    return { slide: value.slide };
}

export function checkedNotesCommand(value) {
    if (!exact(value, ['type', 'slide', 'text']) || value.type !== 'set-speaker-notes' ||
        !identifier(value.slide) || !validNotesText(value.text)) throw failure('INVALID_REQUEST', 'Invalid speaker notes text');
    return { ...value };
}

export function checkedNotesReply(reply, args, expected) {
    const value = reply?.result;
    if (reply?.revision !== expected || !exact(value, ['slide', 'text', 'editable', 'reason']) ||
        value.slide !== args.slide || !validNotesText(value.text) || typeof value.editable !== 'boolean' ||
        !(value.reason === null || typeof value.reason === 'string' && value.reason.length > 0 && value.reason.length <= 1024) ||
        (value.editable && value.reason !== null) || (!value.editable && !value.reason)) {
        throw failure('INVALID_RESPONSE', 'Invalid speaker notes reply');
    }
    return reply;
}
