import { checkedTableReply } from './table-contract.mjs';
import { SELECTION_BYTES, checkedSelectionReply } from './selection-contract.mjs';
import { checkedLayoutCatalog } from './layout-catalog.mjs';
import { checkedShapeCatalog } from './shape-catalog.mjs';
import { checkedNotesReply } from './notes-contract.mjs';
import { checkedAnimationTimeline } from './animation-contract.mjs';
import { checkedAnimationCatalog } from './animation-authoring-contract.mjs';
/** Adapter for the production Deck ABI on the SAME approved compute capability
 * used by Sheet. Rust owns all document state. This module owns only transport.
 */
import { BUFFER_BYTES, FILE_BYTES, FORMATS, checkedRequest, checkedResponse, checkedProjection, checkedOutline, checkedMutation, checkedStatus, encode, failure, revision, normalizeError, rejectedRevision } from './engine-contract.mjs';

import { IMAGE_BYTES, checkedImageReply } from './image-contract.mjs';

const EXPORTS = ['deck_abi_version', 'deck_buffer_ptr', 'deck_buffer_capacity', 'deck_request',
    'deck_input_begin', 'deck_input_write', 'deck_output_read',
    'deck_image_input_begin', 'deck_image_input_write', 'deck_image_output_read',
    'deck_selection_input_begin', 'deck_selection_input_write', 'deck_selection_output_read'];
const decoder = new TextDecoder('utf-8', { fatal: true });

export async function openWasmDeckBackend({ services, path = 'engine.wasm', bytes, format, newDocument = false, copy = false, targetFormat, signal, timeoutMs = 4000 } = {}) {
    const compute = services?.compute;
    if (!compute || ['open', 'invoke', 'cancel', 'close'].some(key => typeof compute[key] !== 'function') ||
        typeof path !== 'string' || !/^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}\.wasm$/.test(path) ||
        typeof newDocument !== 'boolean' || typeof copy !== 'boolean' || (newDocument && copy) ||
        (targetFormat !== undefined && (!copy || !FORMATS.includes(targetFormat))) ||
        (newDocument ? bytes !== undefined || !["pptx", "odp"].includes(format) : !(bytes instanceof Uint8Array) || !bytes.length || bytes.length > FILE_BYTES) ||
        (format !== undefined && !FORMATS.includes(format)) || !Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 4000) {
        throw failure('INVALID_REQUEST', 'Expected approved Deck compute and a presentation of 1 byte to 8 MiB');
    }
    // Capture ownership before the first await; caller changes cannot affect import.
    const source = bytes?.slice();
    let handle, pointer, pending = null, closed = false, closePromise, currentRevision = '0';
    const alive = () => { if (closed || signal?.aborted) throw failure('CLOSED', 'Deck engine is closed'); };
    const invoke = (name, args = [], extra = {}) => compute.invoke({ abi: 1, handle, name, args, timeoutMs, ...extra });
    async function release() {
        const owned = handle; handle = undefined;
        if (owned) try { await compute.cancel({ handle: owned }); } catch { /* Revocation already destroys workers. */ }
    }
    function stop() {
        if (closePromise) return closePromise;
        closed = true;
        signal?.removeEventListener('abort', abort);
        closePromise = release();
        return closePromise;
    }
    function abort() { stop().catch(() => {}); }
    async function raw(request) {
        const data = encode(request);
        const output = await invoke('deck_request', [data.length], {
            write: { offset: pointer, data }, read: { offset: pointer, lengthFromResult: true, maxLength: BUFFER_BYTES },
        });
        alive();
        if (!Number.isInteger(output?.value) || output.value < 1 || output.value > BUFFER_BYTES ||
            !(output.data instanceof Uint8Array) || output.data.length !== output.value) throw failure('INVALID_RESPONSE', 'Invalid Deck response length');
        let value;
        try { value = JSON.parse(decoder.decode(output.data)); }
        catch { throw failure('INVALID_RESPONSE', 'Deck response is not valid UTF-8 JSON'); }
        return checkedResponse(value);
    }
    function ready() {
        alive();
        if (pending) throw failure('BUSY', 'A Deck operation is already in progress');
    }
    async function execute(work) {
        ready();
        pending = (async () => {
            try { return await work(); }
            catch (cause) {
                const recovered = rejectedRevision(cause, currentRevision);
                if (!closed && recovered !== null) currentRevision = recovered;
                else void stop().catch(() => {}); // Never wait for cleanup to report an ambiguous failure.
                throw normalizeError(cause);
            }
        })();
        try { return await pending; }
        finally { pending = null; }
    }
    function accept(value) {
        const next = revision(value.revision);
        if (BigInt(next) < BigInt(currentRevision)) throw failure('INVALID_RESPONSE', 'Deck revision moved backwards');
        currentRevision = next;
        return value;
    }
    signal?.addEventListener('abort', abort, { once: true });
    try {
        alive();
        const opened = await compute.open({ abi: 1, path, timeoutMs });
        handle = opened?.handle;
        alive();
        if (typeof handle !== 'string' || opened.abi !== 1 || !Array.isArray(opened.exports) || EXPORTS.some(name => !opened.exports.includes(name))) {
            throw failure('INVALID_ABI', 'Missing Deck ABI exports');
        }
        if ((await invoke('deck_abi_version')).value !== 1 || (await invoke('deck_buffer_capacity')).value !== BUFFER_BYTES) {
            throw failure('INVALID_ABI', 'Unsupported Deck ABI version or staging size');
        }
        alive();
        pointer = (await invoke('deck_buffer_ptr')).value;
        if (!Number.isSafeInteger(pointer) || pointer < 0 || pointer > 0xffffffff - BUFFER_BYTES) throw failure('INVALID_ABI', 'Invalid Deck staging pointer');
        if (!newDocument) {
        if ((await invoke('deck_input_begin', [source.length])).value !== 0) throw failure('INPUT_REJECTED', 'Deck rejected the source size');
        for (let offset = 0; offset < source.length; offset += BUFFER_BYTES) {
            alive();
            const data = source.slice(offset, offset + BUFFER_BYTES);
            if ((await invoke('deck_input_write', [offset, data.length], { write: { offset: pointer, data } })).value !== 0) {
                throw failure('INPUT_REJECTED', 'Deck rejected a presentation chunk');
            }
        }
        }
        accept(checkedStatus(await raw({ operation: newDocument ? 'new' : copy ? 'open-copy' : 'open',
            arguments: { ...(format === undefined ? {} : { format }), ...(targetFormat === undefined ? {} : { targetFormat }) } })));
    } catch (cause) {
        void stop().catch(() => {});
        // An abort can precede compute.open's reply; reclaim that late handle too.
        void release().catch(() => {});
        throw normalizeError(cause);
    }
    return Object.freeze({
        version: 1,
        get revision() { return currentRevision; },
        async exchange(operation, args = {}, expectedRevision) {
            ready();
            const request = checkedRequest(operation, expectedRevision, args);
            return execute(async () => {
                const result = await raw(request);
                if (operation === 'shape-catalog') checkedShapeCatalog(result);
                else if (operation === 'layout-catalog') checkedLayoutCatalog(result);
                else if (operation === 'project') checkedProjection(result);
                else if (operation === 'outline') checkedOutline(result, request.arguments);
                else if (operation === 'table-cells') checkedTableReply(result, request.arguments, request.expectedRevision);
                else if (operation === 'speaker-notes') checkedNotesReply(result, request.arguments, request.expectedRevision);
                else if (operation === 'animation-timeline') checkedAnimationTimeline(result, request.arguments, request.expectedRevision);
                else if (operation === 'animation-catalog') checkedAnimationCatalog(result, request.expectedRevision);
                else if (operation === 'apply') checkedMutation(result, request.arguments.commands);
                else if (['image', 'prepare-image'].includes(operation)) checkedImageReply(result, request.expectedRevision, request.arguments.asset);
                else if (['export-selection', 'prepare-selection'].includes(operation)) checkedSelectionReply(result, operation, request.expectedRevision);
                else if (operation !== 'begin-save') checkedStatus(result);
                return accept(result);
            });
        },
        // Bulk resources use the same approved compute channel as file import
        // and export. RiceBus carries commands, never megabyte JSON arrays.
        async uploadSelection(bytes) {
            ready();
            if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > SELECTION_BYTES) throw failure('CLIPBOARD_LIMIT', 'Selection exceeds the 4 MiB clipboard limit');
            const source = bytes.slice();
            return execute(async () => {
                if ((await invoke('deck_selection_input_begin', [source.length])).value !== 0) throw failure('INPUT_REJECTED', 'Selection upload rejected');
                for (let offset = 0; offset < source.length; offset += BUFFER_BYTES) {
                    alive();
                    const data = source.slice(offset, offset + BUFFER_BYTES);
                    if ((await invoke('deck_selection_input_write', [offset, data.length], { write: { offset: pointer, data } })).value !== 0) throw failure('INPUT_REJECTED', 'Selection chunk rejected');
                }
            });
        },
        async selectionBytes(snapshot) {
            ready();
            const length = checkedSelectionReply(snapshot, 'export-selection', currentRevision).result.length;
            return execute(async () => {
                const output = new Uint8Array(length);
                for (let offset = 0; offset < output.length; offset += BUFFER_BYTES) {
                    const count = Math.min(BUFFER_BYTES, output.length - offset);
                    const chunk = await invoke('deck_selection_output_read', [offset, count], { read: { offset: pointer, lengthFromResult: true, maxLength: count } });
                    alive();
                    if (chunk?.value !== count || !(chunk.data instanceof Uint8Array) || chunk.data.length !== count) throw failure('INVALID_RESPONSE', 'Incomplete native selection snapshot');
                    output.set(chunk.data, offset);
                }
                return output;
            });
        },
        async prepareImage(bytes, expectedRevision = currentRevision) {
            ready();
            if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > IMAGE_BYTES) throw failure('IMAGE_LIMIT', 'Choose a PNG or JPEG up to 4 MiB');
            const source = bytes.slice(), request = checkedRequest('prepare-image', expectedRevision);
            return execute(async () => {
                if ((await invoke('deck_image_input_begin', [source.length])).value !== 0) throw failure('INPUT_REJECTED', 'Picture upload rejected');
                for (let offset = 0; offset < source.length; offset += BUFFER_BYTES) {
                    alive();
                    const data = source.slice(offset, offset + BUFFER_BYTES);
                    if ((await invoke('deck_image_input_write', [offset, data.length], { write: { offset: pointer, data } })).value !== 0) throw failure('INPUT_REJECTED', 'Picture chunk rejected');
                }
                return accept(checkedImageReply(await raw(request), expectedRevision));
            });
        },
        async imageBytes(asset, expectedRevision = currentRevision) {
            ready();
            const request = checkedRequest('image', expectedRevision, { asset });
            return execute(async () => {
                const response = accept(checkedImageReply(await raw(request), expectedRevision, asset));
                const output = new Uint8Array(response.result.length);
                for (let offset = 0; offset < output.length; offset += BUFFER_BYTES) {
                    const count = Math.min(BUFFER_BYTES, output.length - offset);
                    const chunk = await invoke('deck_image_output_read', [offset, count], { read: { offset: pointer, lengthFromResult: true, maxLength: count } });
                    alive();
                    if (chunk?.value !== count || !(chunk.data instanceof Uint8Array) || chunk.data.length !== count) throw failure('INVALID_RESPONSE', 'Incomplete picture snapshot');
                    output.set(chunk.data, offset);
                }
                return Object.freeze({ ...response.result, bytes: output, revision: response.revision });
            });
        },
        async exportBytes(expectedRevision = currentRevision) {
            ready();
            const request = checkedRequest('begin-save', expectedRevision);
            return execute(async () => {
                const response = accept(await raw(request));
                const length = response.result.length;
                if (response.revision !== expectedRevision || !Number.isSafeInteger(length) || length < 1 || length > FILE_BYTES) {
                    throw failure('INVALID_RESPONSE', 'Invalid Deck save snapshot');
                }
                const output = new Uint8Array(length);
                for (let offset = 0; offset < length; offset += BUFFER_BYTES) {
                    const count = Math.min(BUFFER_BYTES, length - offset);
                    const chunk = await invoke('deck_output_read', [offset, count], { read: { offset: pointer, lengthFromResult: true, maxLength: count } });
                    alive();
                    if (chunk?.value !== count || !(chunk.data instanceof Uint8Array) || chunk.data.length !== count) throw failure('INVALID_RESPONSE', 'Incomplete Deck save chunk');
                    output.set(chunk.data, offset);
                }
                return Object.freeze({ bytes: output, revision: response.revision });
            });
        },
        close: stop,
    });
}
