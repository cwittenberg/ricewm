import { RiceApp } from "./application.mjs";
import { RiceBus, RiceBusBackbone } from "./bus.mjs";
import { Control } from "./control.mjs";
import { createAppConsole, createLogger, DebugSession } from "./debug.mjs";
import { Message } from "./messages.mjs";
import {
    decodeRappMessage,
    encodeCloseResponse,
    encodeSetWindowTitle,
    encodeUiPatch,
    encodeWindowAction,
} from "./rapp.mjs";
import { createServices } from "./services.mjs";
import { encodeScenePatch } from "./scene-wire.mjs";
import {resolveChartTheme} from './chart-theme.mjs';

export class RiceJavaScriptAppCore {
    constructor(definition, {
        backbone = new RiceBusBackbone(), capabilityHost = null, manifest = definition?.manifest,
    } = {}) {
        if (!definition || typeof definition.start !== "function") {
            throw new TypeError("JavaScript app definition must come from defineApp()");
        }
        if (!manifest || typeof manifest !== "object" || Array.isArray(manifest) || typeof manifest.id !== "string" || !manifest.id) {
            throw new TypeError("A host package manifest is required when defineApp omits manifest");
        }
        if (definition.manifest !== undefined && (manifest.id !== definition.manifest?.id || manifest.version !== definition.manifest?.version)) {
            throw new Error("package and entry-module identities do not match");
        }
        this.manifest = Object.freeze({ ...manifest });
        this.definition = Object.freeze({ ...definition, manifest: this.manifest });
        this.backbone = backbone;
        this.capabilityHost = capabilityHost;
        this.instances = new Map();
        this.responses = [];
        this.callHost = null;
        this.capabilities = 0n;
        this.maximumMessageBytes = 0;
        this.closed = false;
        this.dispatching = 0;
        this.emitResponses = null;
        this.onFault = null;
        this.pendingInteractions = new Map();
        this.publishedWindowTitles = new WeakMap();
        this.pendingCloses = new Map();
        this.lastCloseRequests = new Map();
    }

    async initialize({ capabilities = 0n, maximumMessageBytes = 0, callHost = null,
        emitResponses = null, onFault = null } = {}) {
        if (this.closed) throw new Error("JavaScript app core is closed");
        this.capabilities = BigInt(capabilities);
        this.maximumMessageBytes = Number(maximumMessageBytes);
        this.callHost = callHost;
        this.emitResponses = typeof emitResponses === "function" ? emitResponses : null;
        this.onFault = typeof onFault === "function" ? onFault : null;
    }

    serviceBridge(manifest) {
        if (this.capabilityHost) return this.capabilityHost.forApp(manifest);
        return {
            request: (capability, operation, parameters) => {
                if (typeof this.callHost !== "function") throw new Error("RiceWM host bridge is unavailable");
                return this.callHost(capability, operation, parameters);
            },
        };
    }

    collect(app) {
        // flush() synchronously notifies window.onPatch. These frames are
        // returned by collect(), so suppress the asynchronous listener while
        // collecting, including when a title change initiates the collection.
        // Otherwise the same sequence is emitted twice and native UI rejects it.
        this.dispatching += 1;
        try {
        let published = this.publishedWindowTitles.get(app);
        if (!published) {
            published = new Map();
            this.publishedWindowTitles.set(app, published);
        }
        const responses = [];
        for (const window of app.windows.values()) {
            if (published.get(window.id) === window.title) continue;
            responses.push(encodeSetWindowTitle({
                app: app.id,
                window: window.id,
                title: window.title,
            }));
            published.set(window.id, window.title);
        }
        responses.push(...app.flush().map(encodeUiPatch));
        responses.push(...app.drainScenes().map(encodeScenePatch));
        for (const window of app.windows.values()) {
            responses.push(...window.hostRequests.splice(0).map(request => encodeWindowAction({ app: app.id, ...request })));
        }
        return responses;
        } finally {
            this.dispatching -= 1;
        }
    }

    controlMetadata(appId, windowId = 1n) {
        const app = this.instances.get(BigInt(appId)), window = app?.windows.get(BigInt(windowId));
        if (!window || app.closed || window.presentationState.closed) return [];
        const names = [];
        for (const control of app.controls.values()) {
            if (app.controlWindows.get(control.id) !== window || !window.rendered.has(control.id)) continue;
            if (typeof control.name !== 'string' || !/^[A-Za-z][A-Za-z0-9_.:-]{0,127}$/.test(control.name)) continue;
            names.push({target:String(control.id),name:control.name});
            if (names.length >= 4096) break;
        }
        return names;
    }

    controlState(appId, windowId = 1n) {
        const app=BigInt(appId),window=BigInt(windowId);
        return {controls:this.controlMetadata(app,window),pendingInteractions:this.pendingInteractions.get(app)?.get(window)?.size||0};
    }

    async launch(current) {
        if (this.instances.has(current.app)) throw new Error(`app ${current.app} is already running`);
        const permissions = this.manifest.permissions?.bus || {};
        const bus = new RiceBus({ owner: this.manifest.id, backbone: this.backbone, permissions });
        const services = createServices(
            this.serviceBridge(this.manifest),
            this.manifest.permissions,
        );
        const debug = new DebugSession({
            appId: this.manifest.id,
            sink: record => services.debug.log(record),
        });
        const app = new RiceApp({
            manifest: this.manifest,
            bus,
            services,
            debug,
        });
        app.id=current.app;
        const work=new Map();this.pendingInteractions.set(current.app,work);
        app.observeHostWork=(candidate,window)=>{
            if(!candidate||typeof candidate.then!=="function")return;
            const pending=work.get(window)||new Set();work.set(window,pending);
            const promise=Promise.resolve(candidate);if(pending.has(promise))return;
            pending.add(promise);
            promise.then(()=>pending.delete(promise),error=>{
                pending.delete(promise);
                if(this.onFault)this.onFault(error);else queueMicrotask(()=>{throw error;});
            });
        };
        app.measureHost=parameters=>{
            if(this.closed||app.closed)throw Error('Application is closed');
            if(typeof this.callHost!=='function')throw Error('Native control measurement is unavailable');
            return this.callHost('native-ui','measure',parameters);
        };
        this.instances.set(current.app, app);
        app.windowListeners.add(window => this.bindWindow(app, window));
        app.hostCapabilities = this.capabilities;
        app.maximumSceneFrameBytes = this.maximumMessageBytes > 0 ? Math.min(this.maximumMessageBytes, 8 * 1024 * 1024) : 8 * 1024 * 1024;
        const context = Object.freeze({
            instanceId:String(current.app),
            app,
            bus,
            services: app.services,
            debug,
            log: createLogger(debug, this.manifest.id),
            console: createAppConsole(debug),
            arguments: Object.freeze([...current.arguments]),
            entrypoint: current.entrypoint,
            createWindow: options => app.createWindow(options),
        });
        try {
            const root = await this.definition.start(context);
            if (root instanceof Control) app.createWindow({ root });
            if (app.windows.size === 0) throw new Error("app start() must create a window or return a root control");
            for (const window of app.windows.values()) {
                // start() may create a window before yielding. Its scheduled
                // render can then run at the await boundary above, before an
                // outbound patch listener exists. Rebase every initial window
                // so Launch always carries a complete retained tree.
                window.rendered.clear();
                window.sequence = 0n;
                app.dirtyWindows.add(window);
            }
            app.launch({ appId: current.app, arguments: current.arguments });
            app.sceneListeners.add(() => {
                if (!this.dispatching && this.emitResponses) {
                    const frames = app.drainScenes().map(encodeScenePatch);
                    if (frames.length) Promise.resolve(this.emitResponses(frames)).catch(() => {});
                }
            });
            return this.collect(app);
        } catch (error) {
            bus.close();
            app.close({ reason: "launch-failed" });
            this.instances.delete(current.app);
            this.pendingInteractions.delete(current.app);
            throw error;
        }
    }

    bindWindow(app, window) {
        window.onPatch(envelope => {
            if (app.started && !this.dispatching && this.emitResponses) {
                Promise.resolve(this.emitResponses([encodeUiPatch(envelope), ...this.collect(app)])).catch(() => {});
            }
        });
        const collect = () => {
            if (app.started && !this.dispatching && this.emitResponses) {
                Promise.resolve(this.emitResponses(this.collect(app))).catch(() => {});
            }
        };
        window.onTitleChange(collect);
        window.hostRequestListeners.add(collect);
    }

    async dispatch(input, { windowId = null } = {}) {
        if (this.closed) throw new Error("JavaScript app core is closed");
        this.dispatching += 1;
        try {
        const current = decodeRappMessage(input);
        if (current.type === "host-hello") {
            this.capabilities = current.capabilities;
            this.maximumMessageBytes = current.maxMessageBytes;
            return [];
        }
        if (current.type === "launch") return await this.launch(current);
        const app = current.app && this.instances.get(current.app);
        if (!app) throw new Error(`unknown JavaScript app instance ${current.app}`);
        if (current.type === "open") {
            app.emit(Message.APP_OPEN, Object.freeze({ arguments: [], paths: current.paths }));
            return this.collect(app);
        }
        if (current.type === "ui-event") {
            if (current.event.type === "grid-resync") {
                if (!(this.capabilities & (1n << 10n))) throw new Error("Unnegotiated grid resync");
                const window = app.windows.get(current.window);
                if (!window) return [];
                // Keep the envelope sequence monotonic. Native consumed queued
                // presentation envelopes while preserving its last valid tree.
                window.rootReplacementPending = true;
                app.dirtyWindows.add(window);
                return this.collect(app);
            }
            app.dispatchUiEvent(current);
            return this.collect(app);
        }
        if (current.type === "close-request") {
            const existing = this.pendingCloses.get(current.app);
            if (existing?.request === current.request) {
                return [encodeCloseResponse({ request: current.request, app: current.app, decision: "pending" })];
            }
            // The host allocates monotonically increasing close request IDs.
            // Remember one watermark per app instead of retaining an unbounded
            // set of completed requests, and never run a stale handler twice.
            if (current.request <= (this.lastCloseRequests.get(current.app) || 0n)) return [];
            this.lastCloseRequests.set(current.app, current.request);
            if (existing) {
                return [encodeCloseResponse({ request: current.request, app: current.app, decision: "cancelled" })];
            }
            const decision = app.requestClose({ reason: current.reason, window: windowId === null ? null : String(windowId), lastWindow: app.windows.size === 1 });
            if (decision && typeof decision.then === "function") {
                if (!this.emitResponses) {
                    // There is no transport for a later final decision. Keep
                    // the app open and consume settlement without approving it.
                    Promise.resolve(decision).catch(() => {});
                    return [encodeCloseResponse({ request: current.request, app: current.app, decision: "cancelled" }), ...this.collect(app)];
                }
                const pending = { app, request: current.request };
                this.pendingCloses.set(current.app, pending);
                Promise.resolve(decision).then(
                    result => this.finishClose(pending, result === "approved" ? "approved" : "cancelled"),
                    () => this.finishClose(pending, "cancelled"),
                ).catch(() => {});
                return [encodeCloseResponse({ request: current.request, app: current.app, decision: "pending" }), ...this.collect(app)];
            }
            return [encodeCloseResponse({
                request: current.request,
                app: current.app,
                decision,
            }), ...this.collect(app)];
        }
        if (current.type === "close") {
            this.pendingCloses.delete(current.app);
            // A compositor window closes independently. Its sibling windows
            // retain their shared application state, bus and capability lifetime.
            if (windowId !== null) {
                app.windows.get(BigInt(windowId))?.close();
                if (app.windows.size) return this.collect(app);
            }
            this.lastCloseRequests.delete(current.app);
            app.close({ reason: current.reason });
            app.bus.close();
            this.instances.delete(current.app);
            this.pendingInteractions.delete(current.app);
            return [];
        }
        if (current.type === 'window-size-result') {
            app.windows.get(current.window)?.contentSizing.receive(current);
            return this.collect(app);
        }
        if (current.type === 'window-state') {
            app.windows.get(current.window)?.presentationState.observe(current.state);
            // Preserve the existing opaque provider channel for consumers
            // that already decode these native events themselves.
            app.emit('rice.provider-response', Object.freeze({ type: 'provider', kind: current.kind, app: current.app, bytes: current.bytes }));
            return this.collect(app);
        }
        if (current.type === 'desktop-theme') {
            const palette = resolveChartTheme(current.palette);
            app.theme = palette;
            app.emit(Message.THEME_CHANGED, palette);
            return this.collect(app);
        }
        app.emit("rice.provider-response", current);
        return this.collect(app);
        } finally {
            this.dispatching -= 1;
        }
    }

    finishClose(pending, decision) {
        const { app, request } = pending;
        if (this.pendingCloses.get(app.id) !== pending) return;
        this.pendingCloses.delete(app.id);
        if (this.closed || app.closed || this.instances.get(app.id) !== app) return;
        if (!this.emitResponses) return;
        // Production worker response ordering holds these frames until the
        // initial pending response is published, even for an already-resolved
        // Promise. UI updates precede the final decision that can close it.
        try {
            Promise.resolve(this.emitResponses([
                ...this.collect(app), encodeCloseResponse({ request, app: app.id, decision }),
            ])).catch(() => {});
        } catch { /* Failed delivery leaves the host's close request pending. */ }
    }

    async close() {
        if (this.closed) return;
        this.closed = true;
        this.pendingCloses.clear();
        this.lastCloseRequests.clear();
        for (const app of this.instances.values()) {
            app.close({ reason: "host-shutdown" });
            app.bus.close();
        }
        this.instances.clear();
        this.pendingInteractions.clear();
    }
}
