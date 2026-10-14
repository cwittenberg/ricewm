import {checkedViewport} from './viewport-contract.mjs';
/** Package-local workbook bridge. The Wasm engine owns workbook state, formula
 * evaluation and changed-result detection. This adapter only validates control
 * messages and encodes engine-produced presentation objects for RiceBus.
 */
import { checkedCapabilities, checkedSheetCommand } from './command-contract.mjs';
import { checkedQueryRequest, checkedQueryResult } from './query-contract.mjs';
import { checkedImportFonts, checkedFontMetrics } from './font-metrics.mjs';

const BUFFER_BYTES = 65536;
const OUTPUT_BYTES = 262144;
const EXPORTS = ['rs_sheet_input_ptr', 'rs_sheet_output_ptr', 'rs_sheet_exchange'];
const FILE_EXPORTS = ['rs_sheet_file_write', 'rs_sheet_file_read'];
const MAX_FILE_BYTES = 8 * 1024 * 1024;
const encoder = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const error = (code, message) => Object.assign(new Error(message), { code });
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
const integer = value => Number.isSafeInteger(value) && value >= 0;
function exact(value, allowed, required = allowed) {
    return object(value) && Object.keys(value).every(key => allowed.includes(key)) && required.every(key => Object.hasOwn(value, key));
}
function revision(value) {
    if (!integer(value)) throw error('INVALID_RESPONSE', 'Invalid workbook revision');
    return value;
}
function encode(value, limit = BUFFER_BYTES) {
    const bytes = encoder.encode(JSON.stringify(value));
    if (bytes.byteLength > limit) throw error('PAYLOAD_LIMIT', limit === BUFFER_BYTES ? 'Workbook exchange exceeds 64 KiB' : 'Workbook presentation exceeds its negotiated byte budget');
    return bytes;
}
function createRequest(value) {
    if (!exact(value, ['name', 'locale', 'timezone', 'epochMs', 'randomSeed'], [])) throw error('INVALID_REQUEST', 'Invalid workbook create options');
    const result = { operation: 'create' };
    for (const key of ['name', 'locale', 'timezone']) {
        if (value[key] !== undefined) {
            if (typeof value[key] !== 'string' || !value[key].length || value[key].length > 256 || /[\ud800-\udfff]/u.test(value[key])) {
                throw error('INVALID_REQUEST', `Invalid workbook ${key}`);
            }
            result[key] = value[key];
        }
    }
    result.epochMs = value.epochMs ?? Date.now();
    if (!integer(result.epochMs)) throw error('INVALID_REQUEST', 'Invalid workbook epoch');
    let seed = value.randomSeed;
    if (seed === undefined) {
        if (typeof globalThis.crypto?.getRandomValues !== 'function') throw error('ENTROPY_UNAVAILABLE', 'Workbook creation requires host entropy');
        seed = globalThis.crypto.getRandomValues(new Uint8Array(32));
    }
    if (!(seed instanceof Uint8Array) && !Array.isArray(seed)) throw error('INVALID_REQUEST', 'Expected a 32-byte workbook random seed');
    if (seed.length !== 32 || [...seed].some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) {
        throw error('INVALID_REQUEST', 'Expected a 32-byte workbook random seed');
    }
    result.randomSeed = [...seed];
    return result;
}
function exchangeRequest(value) {
    const isCommands = value?.operation === 'commands';
    if (!['viewport', 'commands'].includes(value?.operation) || !exact(value,
        ['operation', 'expectedRevision', 'viewport', 'cursor', ...(isCommands ? ['commands'] : [])],
        ['operation', 'expectedRevision', 'viewport', ...(isCommands ? ['commands'] : [])])) {
        throw error('INVALID_REQUEST', 'Invalid workbook exchange operation');
    }
    revision(value.expectedRevision);
    const view = checkedViewport(value.viewport);
    let cursor = null;
    if (value.cursor != null) {
        const c = value.cursor;
        if (!exact(c, ['view', 'generation', 'sequence', 'revision']) || typeof c.view !== 'string' ||
            !c.view.length || c.view.length > 128 || ![c.generation, c.sequence, c.revision].every(integer)) {
            throw error('INVALID_REQUEST', 'Invalid workbook viewport cursor');
        }
        cursor = { ...c };
    }
    const request = { operation: value.operation, expectedRevision: value.expectedRevision, viewport: { ...view }, cursor };
    if (isCommands) {
        if (!Array.isArray(value.commands) || !value.commands.length || value.commands.length > 32) throw error('BATCH_LIMIT', 'Invalid workbook command batch');
        request.commands = value.commands.map(checkedSheetCommand);
    }
    return request;
}

/** One backend owns one approved compute instance. There is no arbitrary export
 * or pointer access through exchange(). A transport/protocol failure retires the
 * instance because a mutation may already have committed; it is never retried.
 */
export async function openWasmSheetBackend({ services, path, create = {}, signal, timeoutMs = 3000, resolveFontMetrics } = {}) {
    const compute = services?.compute;
    if (!compute || ['open', 'invoke', 'cancel', 'close'].some(key => typeof compute[key] !== 'function') ||
        typeof path !== 'string' || !/^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}\.wasm$/.test(path) ||
        !Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 4000 ||
        resolveFontMetrics !== undefined && typeof resolveFontMetrics !== 'function') throw error('INVALID_REQUEST', 'Invalid workbook compute configuration');
    const initial = createRequest(create);
    let handle, input, output, closed = false, pending = null, closePromise;
    let currentRevision = 0, currentDataRevision = 0, capabilities = Object.freeze([]), config, hasFileTransfer = false, fileFormats = Object.freeze([]);
    const alive = () => { if (closed || signal?.aborted) throw error('CLOSED', 'Workbook backend is closed'); };
    const invoke = (name, args = [], extra = {}) => compute.invoke({ abi: 1, handle, name, args, timeoutMs, ...extra });
    async function release(cancel = false) {
        const owned = handle;
        handle = undefined;
        if (!owned) return;
        try { await compute[cancel ? 'cancel' : 'close']({ handle: owned }); }
        catch { /* Timeout/revocation may already have destroyed this instance. */ }
    }
    async function raw(request) {
        const data = encode(request);
        const outputLimit = request.viewport?.frameBytes === OUTPUT_BYTES ? OUTPUT_BYTES - data.byteLength : BUFFER_BYTES;
        if (input < output + outputLimit && output < input + BUFFER_BYTES) throw error('INVALID_ABI','Workbook buffers overlap');
        const result = await invoke(EXPORTS[2], [data.byteLength], {
            write: { offset: input, data }, read: { offset: output, lengthFromResult: true, maxLength: outputLimit },
        });
        if (!Number.isInteger(result?.value) || result.value < 1 || result.value > outputLimit ||
            !(result.data instanceof Uint8Array) || result.data.byteLength !== result.value) throw error('INVALID_RESPONSE', 'Invalid workbook output byte count');
        let response;
        try { response = JSON.parse(decoder.decode(result.data)); }
        catch { throw error('INVALID_RESPONSE', 'Workbook output is not valid UTF-8 JSON'); }
        if (!object(response) || typeof response.ok !== 'boolean') throw error('INVALID_RESPONSE', 'Invalid workbook response envelope');
        if (!response.ok) {
            if (typeof response.code !== 'string' || !/^[A-Z_]{1,64}$/.test(response.code) ||
                typeof response.message !== 'string' || response.message.length > 1024 ||
                (response.rejectedBeforeMutation !== undefined && typeof response.rejectedBeforeMutation !== 'boolean')) {
                throw error('INVALID_RESPONSE', 'Invalid workbook failure envelope');
            }
            const failure = error(response.code, response.message);
            if (response.revision !== undefined) failure.revision = revision(response.revision);
            if (response.rejectedBeforeMutation === true) failure.rejectedBeforeMutation = true;
            throw failure;
        }
        return response;
    }
    function close() {
        if (closePromise) return closePromise;
        closed = true;
        signal?.removeEventListener('abort', abort);
        const busy = pending;
        closePromise = (async () => {
            if (busy) { await release(true); await busy.catch(() => {}); }
            else if (handle) {
                try { await raw({ operation: 'close' }); }
                finally { await release(); }
            }
        })();
        return closePromise;
    }
    function abort() { close().catch(() => {}); }
    function fileReady() {
        alive();
        if (pending) throw error('BUSY', 'Workbook already has an exchange in progress');
        if (!hasFileTransfer) throw error('UNSUPPORTED_FILE_API', 'Workbook engine does not expose bounded file transfer');
    }
    async function performOperation(work, cancelFile = false) {
        pending = (async () => {
            try { return await work(); }
            catch (cause) {
                if (!closed && cause.rejectedBeforeMutation === true && integer(cause.revision) && cause.revision >= currentRevision) {
                    let cleaned = !cancelFile;
                    if (cancelFile) try {
                        const cancelled = await raw({ operation: 'file-cancel' });
                        alive();
                        if (!exact(cancelled, ['ok'])) throw error('INVALID_RESPONSE', 'Invalid file cancellation response');
                        cleaned = true;
                    } catch { /* A failed cleanup makes transfer state uncertain. */ }
                    if (cleaned) { currentRevision = cause.revision; throw cause; }
                }
                closed = true;
                signal?.removeEventListener('abort', abort);
                await release(true);
                throw cause;
            }
        })();
        try { return await pending; }
        finally { pending = null; }
    }
    function checkedFormat(format = 'xlsx') {
        if (!['xlsx', 'ods', 'csv', 'tsv'].includes(format) || !fileFormats.includes(format)) throw error('UNSUPPORTED_FORMAT', 'This workbook engine does not support the requested file format');
        return format;
    }
    async function fontMeasurements(operation, expectedRevision) {
        const request = await raw({operation,expectedRevision});
        alive();
        if (!exact(request,['ok','fonts'])) throw error('INVALID_RESPONSE','Invalid workbook font request');
        const fonts = checkedImportFonts(request.fonts);
        try {
            const metrics = checkedFontMetrics(fonts,await resolveFontMetrics(fonts));
            alive(); return metrics;
        } catch (cause) {
            // Only a read-only font query has run. A font service failure must
            // not retire an otherwise usable open workbook during Save.
            throw Object.assign(error('FONT_METRICS',String(cause?.message || cause).slice(0,1024)),
                {rejectedBeforeMutation:true,revision:expectedRevision});
        }
    }
    async function importWorkbook(bytes, options = {}) {
        fileReady();
        if (!(bytes instanceof Uint8Array) || (!bytes.length && !['csv','tsv'].includes(options.format)) || bytes.length > MAX_FILE_BYTES ||
            !exact(options, ['name', 'format'], [])) throw error('INVALID_REQUEST', 'Expected a workbook byte array of 1 byte to 8 MiB');
        const format = checkedFormat(options.format);
        const name = options.name ?? `Imported workbook.${format}`;
        if (typeof name !== 'string' || !name.length || encoder.encode(name).length > 512 ||
            /[\u0000-\u001f\u007f-\u009f\ud800-\udfff]/u.test(name)) throw error('INVALID_REQUEST', 'Invalid imported workbook name');
        // Capture caller ownership once: asynchronous chunks cannot observe
        // later edits or detachment of the supplied buffer.
        const owned = bytes.slice();
        const expectedRevision = currentRevision;
        return performOperation(async () => {
            const begun = await raw({ operation: 'import-begin', size: owned.length, name, ...(options.format === undefined ? {} : {format}) });
            alive();
            if (!exact(begun, ['ok', 'size']) || begun.size !== owned.length) throw error('INVALID_RESPONSE', 'Invalid workbook import acknowledgement');
            for (let offset = 0; offset < owned.length; offset += BUFFER_BYTES) {
                const data = owned.slice(offset, offset + BUFFER_BYTES);
                const written = await invoke(FILE_EXPORTS[0], [offset, data.length], { write: { offset: input, data } });
                alive();
                if (written?.value !== data.length) throw error('INVALID_RESPONSE', 'Incomplete workbook import chunk');
            }
            let fontMetrics;
            if (format === 'xlsx' && config?.importFontMetrics === true && resolveFontMetrics) {
                fontMetrics = await fontMeasurements('import-fonts',expectedRevision);
            }
            const committed = await raw({ operation: 'import-commit', expectedRevision, ...(fontMetrics ? {fontMetrics} : {}) });
            alive();
            if (!exact(committed, ['ok', 'revision']) || revision(committed.revision) <= expectedRevision) {
                throw error('INVALID_RESPONSE', 'Invalid workbook import revision');
            }
            currentRevision = committed.revision;
            return Object.freeze({ revision: currentRevision });
        }, true);
    }
    async function exportWorkbook(options = {}) {
        fileReady();
        if (!exact(options, ['format'], [])) throw error('INVALID_REQUEST', 'Invalid workbook export options');
        const format = checkedFormat(options.format);
        const expectedRevision = currentRevision;
        return performOperation(async () => {
            const fontMetrics = format === 'xlsx' && config?.exportFontMetrics === true && resolveFontMetrics
                ? await fontMeasurements('export-fonts',expectedRevision) : undefined;
            const begun = await raw({ operation: 'export-begin', expectedRevision, ...(options.format === undefined ? {} : {format}),
                ...(fontMetrics ? {fontMetrics} : {}) });
            alive();
            if (!exact(begun, ['ok', 'size', 'revision']) || !integer(begun.size) || !begun.size ||
                begun.size > MAX_FILE_BYTES || begun.revision !== expectedRevision) {
                throw error('INVALID_RESPONSE', 'Invalid workbook export acknowledgement');
            }
            const bytes = new Uint8Array(begun.size);
            for (let offset = 0; offset < bytes.length; offset += BUFFER_BYTES) {
                const length = Math.min(BUFFER_BYTES, bytes.length - offset);
                const chunk = await invoke(FILE_EXPORTS[1], [offset, length], {
                    read: { offset: output, lengthFromResult: true, maxLength: length },
                });
                alive();
                if (chunk?.value !== length || !(chunk.data instanceof Uint8Array) || chunk.data.length !== length) {
                    throw error('INVALID_RESPONSE', 'Incomplete workbook export chunk');
                }
                bytes.set(chunk.data, offset);
            }
            const cancelled = await raw({ operation: 'file-cancel' });
            alive();
            if (!exact(cancelled, ['ok'])) throw error('INVALID_RESPONSE', 'Invalid workbook export cleanup acknowledgement');
            return bytes;
        }, true);
    }
    signal?.addEventListener('abort', abort, { once: true });
    try {
        alive();
        const opened = await compute.open({ abi: 1, path, timeoutMs });
        handle = opened?.handle;
        alive();
        if (typeof handle !== 'string' || opened.abi !== 1 || !Array.isArray(opened.exports) || EXPORTS.some(name => !opened.exports.includes(name))) {
            throw error('INVALID_ABI', 'Workbook module lacks its fixed buffer ABI');
        }
        hasFileTransfer = FILE_EXPORTS.every(name => opened.exports.includes(name));
        for (const name of EXPORTS.slice(0, 2)) {
            pending = invoke(name);
            const pointer = (await pending)?.value;
            pending = null;
            alive();
            if (!integer(pointer) || pointer > 0xffffffff - BUFFER_BYTES) throw error('INVALID_ABI', 'Invalid workbook buffer pointer');
            if (name === EXPORTS[0]) input = pointer; else output = pointer;
        }
        if (Math.abs(input - output) < BUFFER_BYTES) throw error('INVALID_ABI', 'Workbook buffers overlap');
        pending = raw(initial);
        const created = await pending;
        pending = null;
        alive();
        currentRevision = revision(created.revision);
        if (!Array.isArray(created.capabilities)) throw error('INVALID_RESPONSE', 'Missing workbook command capabilities');
        capabilities = checkedCapabilities(created.capabilities);
        if (created.config !== undefined && !object(created.config)) throw error('INVALID_RESPONSE', 'Invalid workbook configuration');
        config = created.config === undefined ? undefined : Object.freeze({ ...created.config });
        if (config?.importFontMetrics !== undefined && typeof config.importFontMetrics !== 'boolean')
            throw error('INVALID_RESPONSE','Invalid workbook font metric capability');
        if (config?.exportFontMetrics !== undefined && typeof config.exportFontMetrics !== 'boolean')
            throw error('INVALID_RESPONSE','Invalid workbook font metric capability');
        const advertisedFormats = config?.fileFormats ?? (hasFileTransfer ? ['xlsx'] : []);
        if (!Array.isArray(advertisedFormats) || advertisedFormats.length > 4 || new Set(advertisedFormats).size !== advertisedFormats.length ||
            advertisedFormats.some(format => !['xlsx', 'ods', 'csv', 'tsv'].includes(format)) || !hasFileTransfer && advertisedFormats.length) {
            throw error('INVALID_RESPONSE', 'Invalid workbook file format capabilities');
        }
        fileFormats = Object.freeze([...advertisedFormats]);
    } catch (cause) {
        closed = true;
        signal?.removeEventListener('abort', abort);
        await release(true);
        throw cause;
    }
    return Object.freeze({
        version: 2,
        get revision() { return currentRevision; },
        get capabilities() { return capabilities; },
        get config() { return config; },
        /** Import into a fresh backend before replacing an open document. A trap
         * retires this compute instance; another live backend remains untouched.
         * Bytes use bounded compute calls, never the RiceBus control channel.
         */
        get fileFormats() { return fileFormats; },
        importWorkbook,
        exportWorkbook,
        async importXlsx(bytes, options = {}) {
            if (!exact(options, ['name'], [])) throw error('INVALID_REQUEST', 'Invalid XLSX import options');
            return importWorkbook(bytes, options);
        },
        async exportXlsx() { return exportWorkbook(); },
        async sensitivity(change = null) {
            alive();
            if (pending) throw error('BUSY','Workbook already has an exchange in progress');
            if (change && (!exact(change,['currentId','siteId','id']) || !Object.values(change).every(v=>typeof v==='string'&&v.length<=38))) throw error('INVALID_REQUEST','Invalid embedded label selection');
            const request={operation:'sensitivity',expectedRevision:currentRevision,...(change?{sensitivity:change}:{})};
            return performOperation(async()=>{
                const response=await raw(request);alive();
                if (!Array.isArray(response.labels) || revision(response.revision)<currentRevision) throw error('INVALID_RESPONSE','Invalid sensitivity response');
                currentRevision=response.revision;
                return response;
            });
        },
        async query(data) {
            alive();
            if (pending) throw error('BUSY', 'Workbook already has an exchange in progress');
            const request = { operation: 'data', expectedRevision: currentRevision, data: checkedQueryRequest(data) };
            encode(request);
            return performOperation(async () => {
                const response = await raw(request);
                alive();
                if (!exact(response, ['ok', 'revision', 'dataRevision', 'result'])) throw error('INVALID_RESPONSE', 'Invalid workbook query response');
                const result = checkedQueryResult({ revision: response.revision, dataRevision: response.dataRevision, result: response.result });
                if (result.revision < currentRevision || result.dataRevision < currentDataRevision) throw error('INVALID_RESPONSE', 'Workbook query revision regressed');
                currentRevision = result.revision;
                currentDataRevision = result.dataRevision;
                return result;
            });
        },
        async exchange(value) {
            alive();
            if (pending) throw error('BUSY', 'Workbook already has an exchange in progress');
            const request = exchangeRequest(value);
            // Reject oversize input before entering the engine or poisoning it.
            encode(request);
            return performOperation(async () => {
                    const response = await raw(request);
                    alive();
                    const result = response.projection;
                    if (!object(result) || revision(result.revision) < Math.max(currentRevision, request.expectedRevision)) {
                        throw error('INVALID_RESPONSE', 'Stale or missing workbook projection');
                    }
                    if (!Array.isArray(result.capabilities)) throw error('INVALID_RESPONSE', 'Missing workbook command capabilities');
                    const nextCapabilities = checkedCapabilities(result.capabilities);
                    if (result.frame !== undefined && result.viewUpdate !== undefined) throw error('INVALID_RESPONSE', 'Conflicting workbook presentation encodings');
                    const projection = { ...result };
                    for (const field of ['frame', 'viewUpdate']) {
                        if (result[field] !== undefined) {
                            if (!object(result[field])) throw error('INVALID_RESPONSE', 'Expected an engine-produced presentation object');
                            projection[field] = encode(result[field], request.viewport.frameBytes || BUFFER_BYTES);
                        }
                    }
                    currentRevision = result.revision;
                    capabilities = nextCapabilities;
                    return projection;
            });
        },
        close,
    });
}
