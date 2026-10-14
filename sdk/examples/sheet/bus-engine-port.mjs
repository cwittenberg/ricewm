import {checkedViewport} from './viewport-contract.mjs';
/** Rice Sheet semantic RPC on the existing RiceBus, not another transport.
 *
 * The injected backend remains the only workbook model/calculator. The v2
 * backend does atomic revision-checked commands, recalculation and viewport
 * query in one exchange. It generates snapshots/deltas in Wasm; this module
 * performs no workbook calculation or before/after diff. The legacy v1 backend
 * below is PREVIEW ONLY: it makes additional full viewport reads. This adapter only
 * supports select-sheet commands until that backend contract grows. Batches are
 * ordered, NOT atomic, and are never retried after an ambiguous transport error.
 * Real mutating batches require a future backend atomic apply_commands contract;
 * they must not be added to this development-only ordered selection adapter.
 *
 * RiceBus source is a trusted application manifest ID, not a worker instance ID.
 * Each connection also owns an unguessable session capability. Host integration
 * must call releaseOwner() on caller teardown and close() on service shutdown;
 * the SDK does not currently deliver a peer-disconnected event to a service.
 */
import { copyRiceBusPayload } from '@ricewm/app';
import { checkedEnginePort, checkedProjection } from './engine-port.mjs';
import { checkedSheetCommand, checkedCapabilities } from './command-contract.mjs';
import { checkedQueryRequest, checkedQueryResult } from './query-contract.mjs';

export const SHEET_ENGINE_SERVICE = 'org.ricewm.sheet.engine/v1';
const VERSION = 1;
const MAX_BATCH = 32;
const MAX_PENDING = 32;
const DEFAULT_VIEWPORT = Object.freeze({ top: 0, left: 0, rows: 1, columns: 1 });

function error(code, message) { return Object.assign(new Error(message), { code }); }
function token(value) { return typeof value === 'string' && /^[a-zA-Z0-9-]{16,128}$/.test(value); }
const viewport = checkedViewport;

function commands(value) {
    if (!Array.isArray(value) || !value.length || value.length > MAX_BATCH) {
        throw error('BATCH_LIMIT', 'Sheet command batch must contain 1 to 32 commands');
    }
    return value.map(checkedSheetCommand);
}
function bounded(value) {
    // Use the exact SDK limits, including nested entry accounting, before delivery.
    try { return copyRiceBusPayload(value); }
    catch (cause) { throw error('PAYLOAD_LIMIT', `Sheet RiceBus payload rejected: ${cause.message}`); }
}
function cursor(value) {
    if (value === null || value === undefined) return null;
    if (!value || Object.keys(value).length !== 4 || typeof value.view !== 'string' ||
        !value.view.length || value.view.length > 128 || !Number.isSafeInteger(value.generation) || value.generation < 0 ||
        !Number.isSafeInteger(value.sequence) || value.sequence < 0 ||
        !Number.isSafeInteger(value.revision) || value.revision < 0) {
        throw error('INVALID_CURSOR', 'Expected a bounded view, generation, sequence and revision cursor');
    }
    return { view: value.view, generation: value.generation, sequence: value.sequence, revision: value.revision };
}
function updateCursor(value) {
    // Read only the small control header; viewport reconstruction/delta validation
    // belongs to the shell cache. This is not an engine-side JavaScript diff.
    let update;
    try { update = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(value.viewUpdate)); }
    catch { throw error('INVALID_PROJECTION', 'Invalid encoded Sheet viewport update'); }
    if (update?.version !== 1 || !['snapshot', 'delta', 'batch'].includes(update.kind)) {
        throw error('INVALID_PROJECTION', 'Invalid Sheet viewport update version or kind');
    }
    const updatedRevision = update.kind === 'snapshot' ? update.frame?.revision : update.revision;
    const updatedSheet = update.kind === 'snapshot' ? update.frame?.sheet : update.sheet;
    const active = value.sheets.find(sheet => sheet.id === value.activeSheetId);
    if (updatedRevision !== value.revision || updatedSheet !== active?.gridId ||
        !Number.isSafeInteger(updatedSheet) || updatedSheet < 0 || updatedSheet > 0xffffffff ||
        (update.kind === 'snapshot' && update.frame?.version !== 1)) {
        throw error('INVALID_PROJECTION', 'Viewport update does not match its revision or active sheet');
    }
    return cursor({ view: update.view, generation: update.generation, sequence: update.sequence, revision: updatedRevision });
}
function projection(value) {
    if (value?.frame !== undefined && value?.viewUpdate !== undefined) {
        throw error('INVALID_PROJECTION', 'Provide either a legacy frame or a viewport update');
    }
    for (const field of ['frame', 'viewUpdate']) {
        if (value?.[field] !== undefined && (!(value[field] instanceof Uint8Array) || value[field].byteLength > 65536)) {
            throw error('PAYLOAD_LIMIT', 'Sheet viewport data must be at most 64 KiB of encoded bytes');
        }
    }
    // Do not ask the UI validator to reconstruct a delta in this service: the
    // caller's presentation cache owns that operation, not the engine adapter.
    const metadata = checkedProjection({ ...value, viewUpdate: undefined });
    const allowed = new Set([...Object.keys(metadata), 'frame', 'viewUpdate']);
    if (Object.keys(value).some(key => !allowed.has(key))) {
        throw error('INVALID_PROJECTION', 'Use encoded viewport frames, not per-cell object arrays');
    }
    if (value.viewUpdate !== undefined) updateCursor(value);
    return bounded({ ...metadata,
        ...(value.frame === undefined ? {} : { frame: value.frame }),
        ...(value.viewUpdate === undefined ? {} : { viewUpdate: value.viewUpdate }) });
}

function revision(value) {
    if (!Number.isSafeInteger(value) || value < 0) throw error('INVALID_REVISION', 'Invalid Sheet revision');
    return value;
}
function deadline(work, milliseconds) {
    let timer;
    return Promise.race([
        Promise.resolve(work),
        new Promise((_, reject) => { timer = setTimeout(() => reject(error('DEADLINE', 'Sheet engine request exceeded its RiceBus deadline')), milliseconds); }),
    ]).finally(() => clearTimeout(timer));
}
function envelope(session, workbook, operation, extra = {}) {
    return bounded({ version: VERSION, session, workbook, operation, ...extra });
}

/** Expose a bounded service around real, host-authorized per-document backends.
 * openBackend({source, documentToken}) -> either:
 * - {version:2, revision, exchange(request), close()}: production-intended seam.
 *   exchange receives {operation,expectedRevision,viewport,cursor,commands?}.
 *   Reads use a minimum revision; command batches compare exact revision and
 *   commit atomically in the engine. Known conflicts reject with code
 *   REVISION_CONFLICT, rejectedBeforeMutation:true and authoritative revision.
 * - the existing v1 engine port: preview-only selection compatibility adapter.
 * documentToken is an opaque host-authorized document reference, never a path
 * authorization, workbook byte array, or caller-supplied source identity.
 */
export function exposeSheetEngineService({ bus, service = SHEET_ENGINE_SERVICE, openBackend,
    maximumSessions = 32, deadlineMs = 4000 } = {}) {
    if (!bus || typeof bus.expose !== 'function' || typeof openBackend !== 'function' ||
        !Number.isInteger(maximumSessions) || maximumSessions < 1 || maximumSessions > 128 ||
        !Number.isInteger(deadlineMs) || deadlineMs < 1 || deadlineMs > 4000) {
        throw new TypeError('Invalid Rice Sheet engine service configuration');
    }
    const sessions = new Map();
    let stopped = false;
    let stopPromise;
    function dispose(entry) {
        if (entry.disposal) return entry.disposal;
        entry.closed = true;
        sessions.delete(entry.session);
        // Wait for an in-flight backend call to finish before its teardown. A
        // deadline retires its capability immediately; late replies cannot revive it.
        entry.disposal = Promise.resolve(entry.work).catch(() => {}).then(async () => {
            if (entry.backend) await entry.backend.close();
        });
        entry.disposal.catch(() => {});
        return entry.disposal;
    }
    async function request({ source, request: input }) {
        let entry;
        let mutationStarted = false;
        try {
            if (stopped) throw error('CLOSED', 'Sheet engine service is closed');
            const req = bounded(input);
            if (typeof source !== 'string' || !source || req?.version !== VERSION || !token(req.session)) {
                throw error('INVALID_REQUEST', 'Invalid Sheet engine request envelope');
            }
            if (req.operation === 'open') {
                if (sessions.has(req.session)) throw error('SESSION_EXISTS', 'Sheet session already exists');
                if (sessions.size >= maximumSessions) throw error('SESSION_LIMIT', 'Sheet engine session limit reached');
                if (req.documentToken !== null && (typeof req.documentToken !== 'string' || !req.documentToken.length || req.documentToken.length > 512)) {
                    throw error('INVALID_REQUEST', 'Expected an authorized document token or null');
                }
                entry = { source, session: req.session, workbook: crypto.randomUUID(), closed: false, busy: true, backend: null };
                sessions.set(entry.session, entry);
                entry.work = (async () => {
                    const backend = await openBackend({ source, documentToken: req.documentToken });
                    // Retain malformed backend handles for best-effort cleanup too.
                    entry.backend = backend;
                    if (backend?.version === 2) {
                        revision(backend.revision);
                        if (typeof backend.exchange !== 'function' || typeof backend.close !== 'function') {
                            throw error('BACKEND_UNAVAILABLE', 'Invalid Sheet v2 backend contract');
                        }
                    } else checkedEnginePort(backend);
                    if (!backend) throw error('BACKEND_UNAVAILABLE', 'Sheet backend is unavailable');
                    if (entry.closed) throw error('CLOSED', 'Sheet session was closed while opening');
                    const initial = backend.version === 2 ? null : projection(await backend.readViewport(DEFAULT_VIEWPORT));
                    entry.capabilities = checkedCapabilities(backend.version === 2 ? backend.capabilities : initial.capabilities);
                    const initialRevision = backend.version === 2 ? backend.revision : initial.revision;
                    return envelope(entry.session, entry.workbook, 'opened', { revision: initialRevision });
                })();
            } else {
                entry = sessions.get(req.session);
                if (!entry || entry.source !== source || entry.closed) throw error('SESSION_UNAVAILABLE', 'Sheet session is unavailable to this caller');
                if (req.operation === 'close') {
                    // Token-only release also cleans up an open whose reply was lost.
                    if (req.workbook !== null && req.workbook !== entry.workbook) throw error('WORKBOOK_MISMATCH', 'Sheet workbook handle mismatch');
                    await deadline(dispose(entry), deadlineMs);
                    return envelope(req.session, req.workbook, 'closed');
                }
                if (req.workbook !== entry.workbook) throw error('WORKBOOK_MISMATCH', 'Sheet workbook handle mismatch');
                if (entry.busy) throw error('BUSY', 'A Sheet session request is already in flight');
                if (!['viewport', 'commands', 'query'].includes(req.operation)) throw error('UNSUPPORTED_OPERATION', 'Unsupported Sheet engine operation');
                const expected = revision(req.expectedRevision);
                const query = req.operation === 'query' ? checkedQueryRequest(req.data) : null;
                if (query && (entry.backend.version !== 2 || typeof entry.backend.query !== 'function')) {
                    throw Object.assign(error('UNSUPPORTED_QUERY', 'This backend does not support workbook queries'),
                        { rejectedBeforeMutation: true, revision: expected });
                }
                const region = query ? null : viewport(req.viewport);
                const batch = req.operation === 'commands' ? commands(req.commands) : null;
                if (batch && batch.some(command => entry.backend.version === 1 ? command.type !== 'select-sheet'
                    : !entry.capabilities.includes(command.type))) {
                    throw error('UNSUPPORTED_COMMAND', 'Backend does not advertise this Sheet command');
                }
                const viewCursor = cursor(req.cursor);
                if (viewCursor && viewCursor.revision > expected) throw error('INVALID_CURSOR', 'Viewport cursor is ahead of caller revision');
                entry.busy = true;
                entry.work = (async () => {
                    if (query) {
                        const result = checkedQueryResult(await entry.backend.query(query));
                        if (result.revision < expected) throw error('STALE_REPLY', 'Query workbook revision moved backwards');
                        if (entry.closed) throw error('CLOSED', 'Sheet session is closed');
                        return envelope(entry.session, entry.workbook, 'data-result', result);
                    }
                    if (entry.backend.version === 2) {
                        // Exactly one engine entry. The engine owns atomic compare,
                        // commands/recalculation, indexed query and changed results.
                        mutationStarted = Boolean(batch);
                        const current = projection(await entry.backend.exchange({
                            operation: req.operation, expectedRevision: expected, viewport: region,
                            cursor: viewCursor, ...(batch ? { commands: batch } : {}),
                        }));
                        if (current.revision < expected) throw error('STALE_REPLY', 'Backend revision moved backwards');
                        if (entry.closed) throw error('CLOSED', 'Sheet session is closed');
                        entry.capabilities = checkedCapabilities(current.capabilities);
                        return envelope(entry.session, entry.workbook, 'result', { revision: current.revision, projection: current });
                    }
                    let current = projection(await entry.backend.readViewport(region));
                    if (current.revision < expected) throw error('STALE_REPLY', 'Backend revision moved backwards');
                    if (batch && current.revision !== expected) {
                        throw Object.assign(error('REVISION_CONFLICT', 'Workbook changed since the caller last read it'),
                            { revision: current.revision, rejectedBeforeMutation: true });
                    }
                    if (batch) {
                        for (const command of batch) {
                            if (entry.closed) throw error('CLOSED', 'Sheet session is closed');
                            mutationStarted = true;
                            await entry.backend.command(command);
                        }
                        if (entry.closed) throw error('CLOSED', 'Sheet session is closed');
                        const next = projection(await entry.backend.readViewport(region));
                        if (next.revision < current.revision) throw error('STALE_REPLY', 'Backend revision moved backwards');
                        current = next;
                    }
                    if (entry.closed) throw error('CLOSED', 'Sheet session is closed');
                    // The preview v1 adapter cannot expose capabilities its
                    // non-atomic command path deliberately rejects.
                    current = projection({ ...current, capabilities: current.capabilities.filter(type => type === 'select-sheet') });
                    return envelope(entry.session, entry.workbook, 'result', { revision: current.revision, projection: current });
                })();
            }
            try { return await deadline(entry.work, deadlineMs); }
            catch (cause) {
                // A command might have partially applied, or opening might have
                // completed after timeout. Retire the capability; never retry it.
                if (entry.backend?.version === 2 && cause.rejectedBeforeMutation === true &&
                    Number.isSafeInteger(cause.revision) && cause.revision >= input.expectedRevision) {
                    mutationStarted = false;
                }
                if (input.operation === 'open' || mutationStarted || cause.code === 'DEADLINE') dispose(entry);
                if (mutationStarted && cause.code !== 'DEADLINE') {
                    throw error(entry.backend?.version === 2 ? 'UNKNOWN_OUTCOME' : 'PARTIAL_BATCH',
                        `Sheet batch failed; its effects are not confirmed: ${cause.message}`);
                }
                throw cause;
            } finally { entry.busy = false; }
        } catch (cause) {
            // Structured protocol errors survive the real transport's error
            // serialization, which otherwise only guarantees a message field.
            return { version: VERSION, operation: 'error', code: String(cause.code || 'BACKEND_FAILURE').slice(0,128),
                message: String(cause.message || 'Sheet backend failed').slice(0,1024),
                ...(cause.rejectedBeforeMutation === true ? { rejectedBeforeMutation: true, revision: cause.revision } : {}) };
        }
    }
    const unexpose = bus.expose(service, request);
    return Object.freeze({
        get sessionCount() { return sessions.size; },
        async releaseOwner(source) {
            const owned = [...sessions.values()].filter(entry => entry.source === source);
            await deadline(Promise.all(owned.map(dispose)), deadlineMs);
        },
        close() {
            if (stopPromise) return stopPromise;
            stopped = true;
            unexpose();
            stopPromise = deadline(Promise.all([...sessions.values()].map(dispose)), deadlineMs);
            return stopPromise;
        },
    });
}

/** Connect one shell instance. All requests are sequential and bounded; no
 * command is retried. Reads refresh to the latest revision; commands use exact
 * revisions and may be refreshed after a known pre-mutation conflict. A transport/protocol error poisons the connection because
 * a timed-out command may have applied. Close is idempotent and release-only.
 */
export async function connectSheetEnginePort({ bus, service = SHEET_ENGINE_SERVICE, documentToken = null } = {}) {
    if (!bus || typeof bus.call !== 'function') throw new TypeError('RiceBus client is required');
    const session = crypto.randomUUID();
    let workbook = null;
    let currentRevision;
    let currentDataRevision = 0;
    let closing = false;
    let poisoned = false;
    let tail = Promise.resolve();
    let closePromise;
    let pending = 0;
    let lastViewport = DEFAULT_VIEWPORT;
    let lastCursor = null;
    async function rpc(operation, extra = {}) {
        const reply = await bus.call(service, envelope(session, workbook, operation, extra));
        if (reply?.version !== VERSION) throw error('PROTOCOL', 'Invalid Sheet protocol version');
        if (reply.operation === 'error') throw Object.assign(error(reply.code, reply.message), { rejectedBeforeMutation: reply.rejectedBeforeMutation === true, revision: reply.revision });
        if (reply.session !== session || !token(reply.workbook) || (workbook !== null && reply.workbook !== workbook)) {
            throw error('PROTOCOL', 'Sheet reply belongs to a different session or workbook');
        }
        return reply;
    }
    async function release() {
        // Best effort is not reported as success if the bus or backend rejects it.
        const reply = await bus.call(service, envelope(session, workbook, 'close'));
        if (reply?.version !== VERSION || (reply.operation !== 'closed' && !(reply.operation === 'error' && reply.code === 'SESSION_UNAVAILABLE'))) {
            throw error(reply?.code || 'PROTOCOL', reply?.message || 'Sheet session release failed');
        }
        if (reply.operation === 'closed' && (reply.session !== session || reply.workbook !== workbook)) {
            throw error('PROTOCOL', 'Sheet release reply belongs to another session');
        }
    }
    try {
        const opened = await rpc('open', { documentToken });
        if (opened.operation !== 'opened') throw error('PROTOCOL', 'Expected Sheet open response');
        workbook = opened.workbook;
        currentRevision = revision(opened.revision);
    } catch (cause) {
        await release().catch(() => {});
        throw cause;
    }
    function enqueue(operation) {
        if (closing || poisoned) return Promise.reject(error('CLOSED', 'Sheet engine port is closed or requires reconnection'));
        if (pending >= MAX_PENDING) return Promise.reject(error('QUEUE_LIMIT', 'Too many queued Sheet requests'));
        pending += 1;
        const task = tail.then(async () => {
            if (closing || poisoned) throw error('CLOSED', 'Sheet engine port is closed or requires reconnection');
            try { return await operation(); }
            catch (cause) {
                if (!(cause.rejectedBeforeMutation === true && Number.isSafeInteger(cause.revision)
                    && cause.revision >= currentRevision)) poisoned = true;
                throw cause;
            }
        }).finally(() => { pending -= 1; });
        tail = task.catch(() => {});
        return task;
    }
    async function execute(operation, region, batch, viewCursor = null) {
        const expected = currentRevision;
        const reply = await rpc(operation, { expectedRevision: expected, viewport: region, cursor: viewCursor, ...(batch ? { commands: batch } : {}) });
        if (closing) throw error('CLOSED', 'Late Sheet reply discarded after close');
        if (reply.operation !== 'result' || revision(reply.revision) < expected) {
            throw error('STALE_REPLY', 'Sheet response has an unexpected revision');
        }
        if (reply.projection?.revision !== reply.revision) throw error('STALE_REPLY', 'Sheet projection revision does not match its response');
        const value = projection(reply.projection);
        const nextCursor = value.viewUpdate === undefined ? null : updateCursor(value);
        if (nextCursor && lastCursor && (nextCursor.generation < lastCursor.generation ||
            (nextCursor.generation === lastCursor.generation && nextCursor.view !== lastCursor.view))) {
            throw error('STALE_REPLY', 'Sheet viewport generation regressed or changed identity');
        }
        currentRevision = reply.revision;
        lastViewport = region;
        lastCursor = nextCursor;
        return value;
    }
    return Object.freeze({
        version: VERSION,
        readViewport(region, viewCursor = null) {
            let checked, checkedCursor;
            try { checked = viewport(region); checkedCursor = cursor(viewCursor); } catch (cause) { return Promise.reject(cause); }
            return enqueue(() => execute('viewport', checked, null, checkedCursor));
        },
        command(command, options) { return this.commandBatch([command], options); },
        commandBatch(batch, options = {}) {
            let checked;
            let expected, region;
            try {
                checked = commands(batch);
                if (!options || typeof options !== 'object' || Array.isArray(options) || Object.keys(options).some(key => !['expectedRevision','viewport'].includes(key))) throw error('INVALID_REQUEST', 'Invalid command revision options');
                expected = options.expectedRevision;
                if (options.viewport !== undefined) region = viewport(options.viewport);
                if (expected !== undefined && (!Number.isSafeInteger(expected) || expected < 0)) throw error('INVALID_REQUEST', 'Invalid expected command revision');
            } catch (cause) { return Promise.reject(cause); }
            return enqueue(() => {
                if (expected !== undefined && expected !== currentRevision) throw Object.assign(error('STALE_REVISION', 'Workbook changed before this operation could run'), { revision: currentRevision, rejectedBeforeMutation: true });
                return execute('commands', region || lastViewport, checked, lastCursor);
            });
        },
        query(data,options={}) {
            let checked,requestedRevision;
            try {
                checked = checkedQueryRequest(data); bounded(checked);
                if(!options||typeof options!=='object'||Array.isArray(options)||Object.keys(options).some(k=>k!=='expectedRevision'))throw error('INVALID_REQUEST','Invalid query revision options');
                requestedRevision=options.expectedRevision;
                if(requestedRevision!==undefined&&(!Number.isSafeInteger(requestedRevision)||requestedRevision<0))throw error('INVALID_REQUEST','Invalid expected query revision');
            } catch (cause) { return Promise.reject(cause); }
            return enqueue(async () => {
                if(requestedRevision!==undefined&&requestedRevision!==currentRevision)throw Object.assign(error('STALE_REVISION','Workbook changed before this query could run'),{revision:currentRevision,rejectedBeforeMutation:true});
                const expected = currentRevision;
                let reply;
                try { reply = await rpc('query', { expectedRevision: expected, data: checked }); }
                catch (cause) {
                    if (cause.rejectedBeforeMutation === true && Number.isSafeInteger(cause.revision) && cause.revision >= currentRevision) currentRevision = cause.revision;
                    throw cause;
                }
                if (closing) throw error('CLOSED', 'Late Sheet query discarded after close');
                if (reply.operation !== 'data-result') throw error('PROTOCOL', 'Expected Sheet query result');
                const result = checkedQueryResult({ revision: reply.revision, dataRevision: reply.dataRevision, result: reply.result });
                if (result.revision < expected || result.dataRevision < currentDataRevision) throw error('STALE_REPLY', 'Query revision moved backwards');
                currentRevision = result.revision;
                currentDataRevision = result.dataRevision;
                return result;
            });
        },
        close() {
            if (closePromise) return closePromise;
            closing = true;
            closePromise = tail.then(release);
            return closePromise;
        },
    });
}
