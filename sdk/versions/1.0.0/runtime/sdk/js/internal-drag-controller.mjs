import {desktopEventTopics} from './desktop.mjs';
import {Message} from './messages.mjs';

/** Native containment dragging. Tokens identify app data, never filesystem authority. */
export class InternalDragController {
    static controlFactory = Object.freeze({topics: ['visual', 'workflow', 'documents'], constructor: 'new InternalDragController({window,control,bus,services,onPreview,onDrop,onError})', methods: 'refresh([{control,token,source,destination}]), cancel(), close(). One same-window controller; actual native hits, captured source, 6px threshold, no nearest target. Declare drops.register/unregister and desktopEventTopics(appId).dropEvent. onDrop owns and validates the mutation; rejected background/file/outside drops never commit.'});
    constructor({window, control, bus, services, onPreview = () => {}, onDrop, onError = () => {}}) {
        if (!window?.application || control?.application !== window.application || window.application.controlWindows.get(control.id) !== window || !bus || !services?.drops || typeof onDrop !== 'function') throw TypeError('Internal dragging needs a live app-owned control and drop services');
        Object.assign(this, {window, control, bus, services, onPreview, onDrop, onError}); this.records = new Map(); this.active = null; this.closed = false; this.pending = false; this.generation = 0;
        this.stop = bus.subscribe(desktopEventTopics(window.application.manifest.id).dropEvent, event => this.receive(event.payload));
        this.stops = [control.on(Message.DESTROY, () => void this.close().catch(onError)), window.root.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) this.cancel(); })];
        this.queue = Promise.resolve();
    }
    refresh(records) {
        if (this.closed || !Array.isArray(records) || records.length > 4352) throw TypeError('Invalid internal drag records');
        this.cancel(); this.records = new Map();
        const serialized = records.map(record => {
            if (record.control?.application !== this.window.application || this.window.application.controlWindows.get(record.control.id) !== this.window || record.control.destroyed || typeof record.token !== 'string' || this.records.has(record.token)) throw TypeError('Drag records must belong to the same window');
            this.records.set(record.token, record); return {control: String(record.control.id), token: record.token, source: record.source === true, destination: record.destination === true};
        });
        const generation = ++this.generation;
        this.queue = this.queue.catch(() => {}).then(() => { if (this.closed || generation !== this.generation) return; return this.services.drops.register({kind: 'internal', window: String(this.window.id), control: String(this.control.id), records: serialized}); });
        return this.queue;
    }
    receive(data) {
        if (this.closed || data?.kind !== 'internal' || data.window !== String(this.window.id) || data.control !== String(this.control.id) || typeof data.gesture !== 'string') return;
        if (data.phase === 'start') { this.cancel(); if (!this.pending && this.records.get(data.source)?.source) this.active = {gesture: data.gesture, source: data.source}; return; }
        if (!this.active || this.active.gesture !== data.gesture || this.active.source !== data.source) return;
        if (data.phase === 'cancel') { this.cancel(); return; }
        const source = this.records.get(data.source), target = this.records.get(data.target);
        if (!source || source.control.destroyed) { this.cancel(); return; }
        const preview = data.moved && target?.destination && !target.control.destroyed ? {source: data.source, target: data.target} : null;
        this.onPreview(preview);
        if (data.phase !== 'drop') return;
        this.active = null; this.onPreview(null);
        if (!preview || this.pending) return;
        this.pending = true;
        return Promise.resolve().then(() => this.onDrop(preview)).catch(this.onError).finally(() => { this.pending = false; });
    }
    cancel() { this.active = null; this.onPreview(null); }
    async close() {
        if (this.closed) return; this.closed = true; this.generation++; this.cancel(); this.stop(); for (const stop of this.stops) stop();
        await this.queue.catch(() => {}); await this.services.drops.unregister({window: String(this.window.id), control: String(this.control.id)});
    }
}
