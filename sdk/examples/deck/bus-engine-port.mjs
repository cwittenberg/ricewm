import { checkedTableReply } from './table-contract.mjs';
import { checkedSelectionReply } from './selection-contract.mjs';
import { checkedNotesReply } from './notes-contract.mjs';
import { checkedAnimationTimeline } from './animation-contract.mjs';
import { checkedAnimationCatalog } from './animation-authoring-contract.mjs';
/** Per-window semantic service over the existing SDK RiceBus. Like Sheet, the
 * native actor provides and calls a private service; compute stays host-owned.
 * No bytes, Wasm export names, pointers or filesystem paths cross this service.
 */
import { checkedRequest, checkedResponse, checkedProjection, checkedOutline, checkedMutation, checkedStatus, encode, exact, failure, revision, normalizeError, rejectedRevision } from './engine-contract.mjs';
import { checkedShapeCatalog } from './shape-catalog.mjs';
import { checkedLayoutCatalog } from './layout-catalog.mjs';
import { checkedScene } from './scene-contract.mjs';

const SCHEMA = 'ricewm.office.engine/v1/deck';
const OPERATIONS = new Set(['shape-catalog', 'layout-catalog', 'animation-catalog', 'status', 'project', 'outline', 'scene', 'animation-timeline', 'table-cells', 'speaker-notes', 'apply', 'undo', 'redo', 'export-selection', 'prepare-selection', 'discard-selection']);
function deadline(promise, milliseconds) {
    let timer;
    return Promise.race([promise, new Promise((_, reject) => {
        timer = setTimeout(() => reject(failure('DEADLINE', 'Deck engine request timed out')), milliseconds);
    })]).finally(() => clearTimeout(timer));
}

/** The backend is already opened by this actor with its approved compute grant.
 * Closing/revoking the actor also destroys its compute workers. No global engine
 * broker or independently mutable JavaScript document is installed here.
 */
export function connectDeckEnginePort({ bus, backend, deadlineMs = 4500 } = {}) {
    if (!bus || typeof bus.expose !== 'function' || typeof bus.call !== 'function' || typeof bus.owner !== 'string' ||
        backend?.version !== 1 || typeof backend.exchange !== 'function' || typeof backend.close !== 'function' ||
        !Number.isInteger(deadlineMs) || deadlineMs < 1 || deadlineMs > 4500) throw failure('INVALID_REQUEST', 'Invalid Deck service configuration');
    let currentRevision = revision(backend.revision), closed = false, busy = false, serving = false, closePromise;
    const session = crypto.randomUUID();
    const service = `org.ricewm.deck/engine/${crypto.randomUUID()}`;
    const owner = bus.owner;
    let unexpose;
    function close() {
        if (closePromise) return closePromise;
        closed = true;
        unexpose?.();
        closePromise = Promise.resolve().then(() => backend.close());
        return closePromise;
    }
    unexpose = bus.expose(service, async ({ source, request }) => {
        // A caller cannot manufacture the trusted bus source or reuse another
        // window's random session even when their manifest IDs are identical.
        if (closed || source !== owner || request?.session !== session) throw failure('SESSION_UNAVAILABLE', 'Deck session unavailable');
        if (!exact(request, ['schema', 'session', 'operation', 'expectedRevision', 'arguments'], ['schema', 'session', 'operation', 'arguments']) ||
            request.schema !== SCHEMA || !OPERATIONS.has(request.operation)) throw failure('INVALID_REQUEST', 'Invalid Deck semantic request');
        const checked = checkedRequest(request.operation, request.expectedRevision, request.arguments);
        if (serving) throw failure('BUSY', 'Deck service is already handling a request');
        serving = true;
        try {
            const value = await deadline(backend.exchange(checked.operation, checked.arguments, checked.expectedRevision), deadlineMs);
            if (closed) throw failure('CLOSED', 'Deck session was closed during the request');
            checkedResponse(value);
            encode(value);
            return { schema: SCHEMA, session, ...value };
        } catch (thrown) {
            const cause = normalizeError(thrown);
            const recovered = rejectedRevision(cause, currentRevision);
            if (recovered === null) void close().catch(() => {});
            // Error properties do not survive throwing across worker RPC. Return
            // the explicitly bounded error envelope to retain revision evidence.
            const code = typeof cause.code === 'string' && /^[A-Z_]{1,64}$/.test(cause.code) ? cause.code : 'ENGINE_UNAVAILABLE';
            const result = { schema: SCHEMA, session, ok: false,
                error: { code, message: String(cause.message || 'Deck engine unavailable').slice(0, 1024) },
                rejectedBeforeMutation: recovered !== null };
            if (recovered !== null) result.revision = recovered;
            return result;
        } finally { serving = false; }
    });
    async function request(operation, args = {}, expected) {
        if (closed) throw failure('CLOSED', 'Deck port is closed');
        if (busy) throw failure('BUSY', 'A Deck command is already in flight');
        const checked = checkedRequest(operation, expected, args);
        busy = true;
        try {
            const reply = await deadline(bus.call(service, { schema: SCHEMA, session, ...checked }), deadlineMs + 250);
            if (closed && reply?.ok !== false) throw failure('CLOSED', 'Deck port closed during the request');
            if (reply?.schema !== SCHEMA || reply?.session !== session) throw failure('INVALID_RESPONSE', 'Deck reply belongs to another session');
            checkedResponse(reply);
            if (operation === 'shape-catalog') checkedShapeCatalog(reply);
            else if (operation === 'layout-catalog') checkedLayoutCatalog(reply);
            else if (operation === 'project') checkedProjection(reply);
            else if (operation === 'outline') checkedOutline(reply, checked.arguments);
            else if (operation === 'apply') checkedMutation(reply, checked.arguments.commands);
            else if (operation === 'scene') checkedScene(reply);
            else if (operation === 'animation-timeline') checkedAnimationTimeline(reply, checked.arguments, checked.expectedRevision);
            else if (operation === 'animation-catalog') checkedAnimationCatalog(reply, checked.expectedRevision);
            else if (operation === 'table-cells') checkedTableReply(reply, checked.arguments, checked.expectedRevision);
            else if (operation === 'speaker-notes') checkedNotesReply(reply, checked.arguments, checked.expectedRevision);
            else if (['export-selection', 'prepare-selection'].includes(operation)) checkedSelectionReply(reply, operation, checked.expectedRevision);
            else checkedStatus(reply);
            const next = revision(reply.revision);
            if (BigInt(next) < BigInt(currentRevision)) throw failure('INVALID_RESPONSE', 'Stale Deck reply');
            currentRevision = next;
            return reply;
        } catch (cause) {
            const recovered = rejectedRevision(cause, currentRevision);
            if (recovered !== null) currentRevision = recovered;
            else void close().catch(() => {});
            throw normalizeError(cause);
        } finally { busy = false; }
    }
    return Object.freeze({
        version: 1,
        get revision() { return currentRevision; },
        project: args => request('project', args),
        outline: (args, expected = currentRevision) => request('outline', args, expected),
        scene: (args, expected = currentRevision) => request('scene', args, expected),
        animationTimeline: (args, expected = currentRevision) => request('animation-timeline', args, expected),
        animationCatalog: (expected = currentRevision) => request('animation-catalog', {}, expected),
        tableCells: (args, expected = currentRevision) => request('table-cells', args, expected),
        speakerNotes: (args, expected = currentRevision) => request('speaker-notes', args, expected),
        shapeCatalog: () => request('shape-catalog'),
        layoutCatalog: () => request('layout-catalog'),
        status: () => request('status'),
        exportSelection: (args, expected = currentRevision) => request('export-selection', args, expected),
        prepareSelection: (expected = currentRevision) => request('prepare-selection', {}, expected),
        discardSelection: (expected = currentRevision) => request('discard-selection', {}, expected),
        apply: (commands, expected = currentRevision) => request('apply', { commands }, expected),
        undo: (expected = currentRevision) => request('undo', {}, expected),
        redo: (expected = currentRevision) => request('redo', {}, expected),
        close,
    });
}
