import { SurfaceControl, State } from './control.mjs';
import { Message } from './messages.mjs';

/** A catalogued host editor, never application HTML, script or a URL. The
 * native control owns geometry; the host owns the pinned browser implementation.
 * Documents remain in that editor's original model and codecs. */
export class BrowserEditorSurface extends SurfaceControl {
    static kind = 60;
    static acceptsChildren = false;
    static controlAvailability = 'app control; requires editors service grants';
    static constructorOptions = Object.freeze(['adapter']);
    static authoring = Object.freeze({
        constructor: 'new BrowserEditorSurface({adapter:"drawio",name:"drawing"})',
        methods: 'Attach to a window, then openBrowserEditor({services,surface}). Only catalogued host adapters are accepted. No application DOM, script, navigation or renderer URL is exposed.',
    });
    constructor({ adapter = 'drawio', ...options } = {}) {
        if (adapter !== 'drawio') throw TypeError('Unknown managed browser editor');
        super({ state: State.EXPANSIVE | State.ENABLED, ...options });
        this.adapter = adapter;
    }
    inspectControl() { return { adapter: this.adapter, state: this.editorState ?? null }; }
}

export async function openBrowserEditor({ services, surface } = {}) {
    const window = surface?.application?.controlWindows.get(surface.id);
    if (!(surface instanceof BrowserEditorSurface) || !window || surface.destroyed) {
        throw TypeError('Managed editor needs an attached, owned BrowserEditorSurface');
    }
    let closed = false, handle, stopWindow, closing, computeOpenSettled;
    const pending = new Map(), completed = new Map(), computeHandles = new Set();
    const close = () => {
        if (closing) return closing;
        closed = true; stopWindow?.();
        for (const task of pending.values()) task.reject(Error('Editor owner closed'));
        pending.clear(); completed.clear();
        const owned = handle;
        closing = (async () => {
            const failures = [];
            const cancelOwned = async () => {
                const results = await Promise.allSettled([...computeHandles].map(async handle => {
                    await services.compute.cancel({ handle }); computeHandles.delete(handle);
                }));
                for (const result of results) if (result.status === 'rejected') failures.push(result.reason);
            };
            // Stop active invocations first. An open still awaiting its handle
            // is captured by the second pass after its capability settles.
            await cancelOwned(); await computeOpenSettled; await cancelOwned();
            try { if (owned) await services.editors.close({ handle: owned }); }
            finally { handle = null; }
            if (failures.length) throw Error('Editor compute cleanup failed: ' + String(failures[0]?.message || failures[0]));
        })();
        return closing;
    };
    stopWindow = window.onClose(() => { void close().catch(() => {}); });
    try {
        const opened = await services.editors.open({ adapter: surface.adapter, target: {
            app: String(window.application.id), window: String(window.id), surface: String(surface.id),
        } });
        handle = opened.handle;
        if (closed || surface.destroyed) { await services.editors.close({ handle }); throw Error('Editor owner closed'); }
        const pump = async () => {
            while (!closed) {
                const batch = await services.editors.events({ handle });
                if (closed) break;
                if (!Array.isArray(batch) || batch.length > 32) throw Error('Invalid editor event batch');
                for (const event of batch) {
                    if (closed) break;
                    if (event.type === 'state' || event.type === 'ready') {
                        surface.editorState = event.value;
                        surface.emit(Message.CHANGE, { property: 'editorState', value: event.value,
                            ready: event.type === 'ready' });
                    } else if (event.type === 'result') {
                        const task = pending.get(event.id);
                        if (task) {
                            pending.delete(event.id);
                            event.ok ? task.resolve(event.value) : task.reject(Error(event.error));
                        } else {
                            if (completed.size >= 8) throw Error('Editor result queue exceeded');
                            completed.set(event.id, event);
                        }
                    } else if (event.type === 'capability') {
                        let value, error, settled;
                        const openingCompute = event.service === 'compute' && event.operation === 'open';
                        if (openingCompute) computeOpenSettled = new Promise(resolve => { settled = resolve; });
                        try {
                            const method = services[event.service]?.[event.operation];
                            if (event.service === 'editors' || typeof method !== 'function' || method.granted !== true) {
                                throw Error('Editor was not granted ' + event.service + '.' + event.operation);
                            }
                            if (event.service === 'compute' && event.operation === 'open' && services.compute?.cancel?.granted !== true)
                                throw Error('Editor compute needs the compute.cancel cleanup grant');
                            value = await method(event.parameters);
                            if (event.service === 'compute') {
                                if (event.operation === 'open' && typeof value?.handle === 'string') computeHandles.add(value.handle);
                                else if (event.operation === 'cancel' || event.operation === 'close') computeHandles.delete(event.parameters?.handle);
                            }
                        } catch (cause) { error = String(cause.message || cause).slice(0, 4096); }
                        finally { if (openingCompute) { settled(); computeOpenSettled = null; } }
                        if (closed) break;
                        await services.editors.respond({ handle, id: event.id, ok: error === undefined,
                            ...(error === undefined ? { value } : { error }) });
                    } else if (event.type === 'fault') throw Error(event.error);
                }
            }
        };
        void pump().catch(error => {
            if (!closed) surface.emit(Message.CHANGE, { property: 'editorError', value: String(error.message) });
            void close().catch(() => {});
        });
        return Object.freeze({ adapter: surface.adapter,
            invoke: async (operation, parameters = {}) => {
                if (closed || surface.destroyed) throw Error('Editor owner is closed');
                const accepted = await services.editors.invoke({ handle, operation, parameters });
                if (closed) throw Error('Editor owner is closed');
                if (completed.has(accepted.request)) {
                    const result = completed.get(accepted.request); completed.delete(accepted.request);
                    if (!result.ok) throw Error(result.error);
                    return result.value;
                }
                return new Promise((resolve, reject) => pending.set(accepted.request, { resolve, reject }));
            },
            inspect: () => surface.editorState ?? null, close,
        });
    } catch (error) { await close(); throw error; }
}
openBrowserEditor.controlFactory = Object.freeze({ topics: ['documents'],
    constructor: 'await openBrowserEditor({services,surface})',
    methods: 'Requires editors.open/invoke/events/respond/close and each file/clipboard/dialog service used by the adapter. Renderer compute.open also requires compute.cancel for cleanup. Preserves host-owned original document codecs. Close cancels renderer-opened compute, drains late opens and releases the renderer and pending work. Invoke only fixed adapter operations, never script or navigation.' });
