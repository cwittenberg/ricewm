import { Control, State } from "./control.mjs";
import { Message, message } from "./messages.mjs";
import { retainedScene } from "./scene-surface.mjs";
import { WindowStateController, WindowSizeController } from "./window-state.mjs";

const MAX_WIDGETS = 65_536;

function frozenPatch(type, values = {}) {
    return Object.freeze({ type, ...values });
}

function retainCompatibleControlIds(previous, replacement) {
    if (
        previous.kind !== replacement.kind ||
        replacement.application !== null || replacement.id !== 0n
    ) return;
    replacement.id = previous.id;
    const count = Math.min(previous.children.length, replacement.children.length);
    for (let index = 0; index < count; index += 1) {
        retainCompatibleControlIds(previous.children[index], replacement.children[index]);
    }
}

export class AppWindow {
    #presentationHolds = 0;

    constructor(application, { id, title = application.manifest.name, root = null, contentSize = null, fitContent = null } = {}) {
        if(contentSize!==null&&(!contentSize||typeof contentSize!=='object'||Array.isArray(contentSize)||Object.keys(contentSize).some(key=>!['width','height'].includes(key))||![contentSize.width,contentSize.height].every(value=>Number.isInteger(value)&&value>=96&&value<=4096)))throw RangeError('Initial contentSize needs width/height 96..4096');
        if(contentSize!==null&&fitContent!==null)throw TypeError('Choose initial contentSize or native fitContent');
        this.application = application;
        this.id = BigInt(id);
        this.titleListeners = new Set();
        this._title = "";
        this.title = title;
        this.root = null;
        this.sequence = 0n;
        this.rendered = new Map();
        this.rootReplacementPending = false;
        this.patchListeners = new Set();
        this.hostRequests = [];
        this.hostRequestListeners = new Set();
        this.contentSizing = new WindowSizeController(request => this.queueHostRequest(request));
        this.presentationState = new WindowStateController((operation, enabled) => this.requestHost(operation, enabled));
        if (root) this.setRoot(root);
        // Startup declares the initial request without waiting for Launch to
        // return its first native tree. The promise reports real host clamping.
        this.initialContentSizeReady=contentSize?this.setContentSize(contentSize.width,contentSize.height):null;
        this.initialContentSizeReady?.catch(error=>this.application.debug?.log?.('error','Initial window sizing failed',{error:error.message}));
        this.fitOptions=fitContent===null?null:this.checkedFitOptions(fitContent);
        this.fitSequence=Promise.resolve();
        this.initialFitReady=null;
        if(this.fitOptions)this.scheduleInitialFit();
    }

    scheduleInitialFit() {
        if(!this.initialFitReady){
            let resolve,reject;
            this.initialFitReady=new Promise((yes,no)=>{resolve=yes;reject=no;});
            // Launch must return the first tree before resize acknowledgements
            // can be dispatched. Start fitting only after initial host geometry.
            const stop=this.onState(state=>{
                if(state.contentWidth<96||state.contentHeight<96)return;
                stop();this.fitContent().then(resolve,reject);
            });
            this.initialFitReady.catch(error=>this.application.debug?.log?.('error','Initial native fitting failed',{error:error.message}));
            this.stopInitialFit=()=>{stop();reject(Error('Window closed before initial native fitting'));};
        }
    }

    get title() { return this._title; }

    set title(value) {
        const next = String(value);
        if (!next.trim()) throw new RangeError("window title must not be empty");
        if (next === this._title) return;
        this._title = next;
        for (const listener of [...this.titleListeners]) listener(next);
    }

    onTitleChange(listener) {
        if (typeof listener !== "function") throw new TypeError("title listener must be a function");
        this.titleListeners.add(listener);
        return () => this.titleListeners.delete(listener);
    }

    get state() { return this.presentationState.state; }
    onState(listener) { return this.presentationState.onState(listener); }
    setFullscreen(enabled) { return this.presentationState.setFullscreen(enabled); }
    setContentSize(width, height, options) { return this.contentSizing.setContentSize(width, height, options); }
    checkedFitOptions(options={}) {
        if(!options||typeof options!=='object'||Array.isArray(options)||Object.keys(options).some(key=>!['control','width','minHeight','maxHeight'].includes(key)))throw TypeError('fitContent accepts control, width, minHeight and maxHeight');
        const result={control:options.control??this.root,width:options.width,minHeight:options.minHeight??96,maxHeight:options.maxHeight??640};
        if(!(result.control instanceof Control)||result.control.application!==this.application||this.application.controlWindows.get(result.control.id)!==this||result.control.destroyed)throw TypeError('Fit an attached control in this window');
        if(result.width!==undefined&&(!Number.isInteger(result.width)||result.width<96||result.width>4096)||![result.minHeight,result.maxHeight].every(value=>Number.isInteger(value)&&value>=96&&value<=4096)||result.minHeight>result.maxHeight)throw RangeError('Fitting dimensions must be 96..4096 with minHeight <= maxHeight');
        return result;
    }
    fitContent(options=this.fitOptions??{}) {
        let chosen;
        try{chosen=this.checkedFitOptions(options);}catch(error){return Promise.reject(error);}
        if(!this.application.started){
            // An async startup renderer may await this call. Queue the latest
            // fit without waiting for geometry that Launch has yet to publish.
            // Actual, potentially clamped dimensions arrive on initialFitReady.
            this.fitOptions=chosen;
            this.scheduleInitialFit();
            return Promise.resolve(Object.freeze({scheduled:true}));
        }
        const pending=this.fitSequence.catch(()=>{}).then(async()=>{
            const width=chosen.width??this.state?.contentWidth;
            if(!Number.isInteger(width)||width<96||width>4096)throw Error('Native fitting needs initial window geometry or an explicit width');
            const measured=await this.measureControl(chosen.control,{width});
            if(!Number.isInteger(measured.height)||measured.height<0||!Number.isInteger(measured.width)||measured.width<0)throw Error('Native content measurement is invalid');
            const height=Math.max(chosen.minHeight,Math.min(chosen.maxHeight,measured.height));
            const actual=await this.setContentSize(width,height);
            return {...actual,requiredHeight:measured.height,overflow:measured.height>actual.height};
        });
        this.fitSequence=pending;
        // Native verification waits for measurement and its matching resize
        // acknowledgement, just as it waits for asynchronous control handlers.
        this.application.observeHostWork?.(pending.catch(()=>{}),this.id);
        return pending;
    }
    measureControl(control, {width=0}={}) {
        if(!(control instanceof Control)||control.application!==this.application||this.application.controlWindows.get(control.id)!==this||control.destroyed)return Promise.reject(TypeError('Measure an attached control in this window'));
        if(!Number.isInteger(width)||(width!==0&&(width<96||width>4096)))return Promise.reject(RangeError('Measurement width must be 0 or 96..4096'));
        if(typeof this.application.measureHost!=='function')return Promise.reject(Error('Native control measurement is unavailable'));
        const nodes=[];
        const visit=(current,parent=null,index=0,depth=0)=>{
            if(depth>64||nodes.length>=2048)throw RangeError('Measurement tree exceeds 2048 controls or 64 levels');
            const widget=current.toWidget();
            nodes.push({id:String(widget.id),kind:widget.kind,state:widget.state,text:widget.text,icon:widget.icon,parent,index});
            let childIndex=0;for(const child of current.children)if(child.values.visible)visit(child,String(widget.id),childIndex++,depth+1);
        };
        try{visit(control);return Promise.resolve(this.application.measureHost({app:String(this.application.id),window:String(this.id),nodes,width}));}catch(error){return Promise.reject(error);}
    }
    setTitlebarVisible(visible) { this.requestHost('titlebar-visible', visible); }
    requestClose() { this.requestHost('close', true); }

    requestHost(operation, enabled) {
        if (typeof enabled !== 'boolean') throw new TypeError('Window visibility requires a boolean');
        this.queueHostRequest({ operation, enabled });
    }

    queueHostRequest(request) {
        if (this.application.closed || this.presentationState.closed) throw new Error('Window is closed');
        if (this.hostRequests.length >= 16) throw new RangeError('Too many queued window actions');
        this.hostRequests.push({ window: this.id, ...request });
        queueMicrotask(() => {
            if (!this.presentationState.closed && this.hostRequests.length) {
                for (const listener of [...this.hostRequestListeners]) listener();
            }
        });
    }

    setRoot(root) {
        if (!(root instanceof Control)) throw new TypeError("window root must be a Control");
        if (root.parent) throw new TypeError("window root cannot have a parent");
        if (this.root === root) return root;
        // A root replacement is a transaction boundary. Remember it explicitly
        // instead of inferring it later from widget IDs, so the host always
        // receives one clear followed by the complete replacement tree.
        this.rootReplacementPending ||= this.rendered.size > 0;
        if (this.root) {
            // Declarative applications commonly rebuild an equivalent tree.
            // Preserve compatible positional identities so an asynchronous
            // refresh between pointer-down and pointer-up cannot orphan the
            // host's pressed/focused widget.
            retainCompatibleControlIds(this.root, root);
            this.application.detach(this.root);
        }
        this.root = root;
        this.application.attach(root, null, this);
        this.application.invalidate(root);
        return root;
    }

    onPatch(listener) {
        if (typeof listener !== "function") throw new TypeError("patch listener must be a function");
        this.patchListeners.add(listener);
        return () => this.patchListeners.delete(listener);
    }

    get presentationDeferred() { return this.#presentationHolds > 0; }

    /** Retain the last complete frame during an async view change. Control
     * state and input guards still update immediately; patches and scenes are
     * published together on release. Always release in a finally block. */
    deferPresentation() {
        if (this.presentationState.closed) throw new Error('Window is closed');
        this.#presentationHolds++;
        let released = false;
        return () => {
            if (released) return;
            released = true;
            this.#presentationHolds--;
            if (!this.presentationDeferred && !this.presentationState.closed) this.application.scheduleRender();
        };
    }

    close() {
        this.stopInitialFit?.();this.stopInitialFit=null;
        this.contentSizing.close();
        this.presentationState.close();
        this.hostRequests.length = 0;
        this.hostRequestListeners.clear();
        if (this.root) this.root.destroy();
        this.root = null;
        this.rendered.clear();
        this.rootReplacementPending = false;
        this.patchListeners.clear();
        this.titleListeners.clear();
        this.application.windows.delete(this.id);
        this.application.dirtyWindows.delete(this);
    }
}

export class RiceApp {
    constructor({ manifest, bus = null, services = null, debug = null } = {}) {
        if (!manifest || typeof manifest !== "object") throw new TypeError("app manifest is required");
        this.manifest = Object.freeze({ ...manifest });
        this.bus = bus;
        this.services = services;
        this.debug = debug?.attach?.(this) || debug;
        this.id = 0n;
        this.windows = new Map();
        this.windowListeners = new Set();
        this.controls = new Map();
        this.controlWindows = new Map();
        this.nextWindowId = 1n;
        this.nextWidgetId = 1n;
        this.dirtyWindows = new Set();
        this.renderScheduled = false;
        this.messageQueue = [];
        this.messageScheduled = false;
        this.lifecycle = new Map();
        this.started = false;
        this.pendingScenes = new Map();
        this.sceneListeners = new Set();
        this.maximumSceneFrameBytes = 8 * 1024 * 1024;
        this.hostCapabilities = 0n;
        this.closed = false;
        this.closeGeneration = 0;
    }

    on(type, listener) {
        if (typeof listener !== "function") throw new TypeError("lifecycle listener must be a function");
        const listeners = this.lifecycle.get(type) || new Set();
        listeners.add(listener);
        this.lifecycle.set(type, listeners);
        return () => listeners.delete(listener);
    }

    emit(type, detail = null) {
        let result;
        for (const listener of [...(this.lifecycle.get(type) || [])]) {
            const candidate = listener(detail);
            if (candidate !== undefined) result = candidate;
        }
        return result;
    }

    launch({ appId, arguments: args = [], paths = [] } = {}) {
        this.id = BigInt(appId);
        this.started = true;
        this.closed = false;
        return this.emit(Message.APP_OPEN, Object.freeze({ arguments: [...args], paths: [...paths] }));
    }

    createWindow(options = {}) {
        if (this.windows.size >= 32) throw new RangeError('application window limit exceeded');
        const id = options.id === undefined ? this.nextWindowId++ : BigInt(options.id);
        if (id <= 0n || this.windows.has(id)) throw new RangeError("window ID must be unique and nonzero");
        const window = new AppWindow(this, { ...options, id });
        this.windows.set(id, window);
        for (const listener of [...this.windowListeners]) listener(window);
        return window;
    }

    attach(control, parent = null, window = null) {
        if (!(control instanceof Control)) throw new TypeError("only controls can be attached");
        const owner = window || (parent && this.controlWindows.get(parent.id));
        if (!owner) throw new TypeError("attached control has no owning window");
        const attachOne = current => {
            if (this.controls.size >= MAX_WIDGETS) throw new RangeError("application widget limit exceeded");
            if (current.application && current.application !== this) {
                throw new TypeError("control belongs to another application");
            }
            current.application = this;
            if (current.id === 0n) current.id = this.nextWidgetId++;
            if (this.controls.has(current.id) && this.controls.get(current.id) !== current) {
                throw new TypeError("duplicate widget ID");
            }
            this.controls.set(current.id, current);
            this.controlWindows.set(current.id, owner);
            current.sendMessage(Message.CREATE);
            for (const child of current.children) attachOne(child);
            current.sendMessage(Message.CREATED);
        };
        attachOne(control);
        this.invalidate(control);
        return control;
    }

    detach(control) {
        const owner = this.controlWindows.get(control.id);
        const detachOne = current => {
            this.pendingScenes.delete(current);
            for (const child of current.children) detachOne(child);
            this.controls.delete(current.id);
            this.controlWindows.delete(current.id);
            current.application = null;
            current.id = 0n;
        };
        detachOne(control);
        if (owner) {
            this.dirtyWindows.add(owner);
            this.scheduleRender();
        }
    }

    invalidate(control) {
        const window = this.controlWindows.get(control.id);
        if (!window) return;
        this.dirtyWindows.add(window);
        this.scheduleRender();
    }

    scheduleRender() {
        if (this.renderScheduled) return;
        this.renderScheduled = true;
        queueMicrotask(() => {
            this.renderScheduled = false;
            this.flush();
        });
    }

    flush() {
        const envelopes = [];
        for (const window of [...this.dirtyWindows]) {
            if (window.presentationDeferred) continue;
            this.dirtyWindows.delete(window);
            const envelope = this.renderWindow(window);
            if (envelope.patches.length) {
                envelopes.push(envelope);
                for (const listener of [...window.patchListeners]) listener(envelope);
            }
        }
        if (this.pendingScenes.size) for (const listener of [...this.sceneListeners]) listener();
        return envelopes;
    }

    queueScene(control, frame) {
        if (frame.displayList.length + 56 > this.maximumSceneFrameBytes) throw new RangeError('Scene exceeds negotiated host message budget');
        let bytes = frame.displayList.length;
        for (const current of this.controls.values()) {
            if (current !== control) bytes += retainedScene(current)?.displayList.length || 0;
        }
        if (bytes > 16 * 1024 * 1024) throw new RangeError('Application retained scene budget exceeded');
        this.pendingScenes.set(control, frame);
        this.scheduleRender();
    }

    drainScenes() {
        const frames = [];
        for (const [control, frame] of this.pendingScenes) {
            const window = this.controlWindows.get(control.id);
            if (control.application === this && !window?.presentationDeferred && window?.rendered.has(control.id)) {
                frames.push({ app: this.id, window: window.id, surface: control.id, ...frame });
                this.pendingScenes.delete(control);
            }
        }
        return frames;
    }

    renderWindow(window) {
        const desired = new Map();
        const visit = (control, parent = null, index = 0, depth = 0, ancestorEnabled = true) => {
            if (!control.values.visible) return;
            const widget = control.toWidget();
            const enabled = ancestorEnabled && Boolean(widget.state & State.ENABLED);
            const effectiveState = enabled ? widget.state : (widget.state & ~State.ENABLED) >>> 0;
            const observeSize = (this.hostCapabilities & 8192n) !== 0n && Boolean(control.listeners.get(Message.SIZE_CHANGED)?.size);
            const editSelection = control.nativeSelectionRequest?.() ?? null;
            const documentFont = control.nativeDocumentFont?.() ?? null;
            const iconSelector=control.values.iconSelector??null;if(iconSelector&&!(BigInt(this.hostCapabilities??0)&(1n<<28n)))throw Error('Native host does not support icon selectors');
            desired.set(control.id, Object.freeze({ ...widget, state: effectiveState, parent, index, depth, observeSize, editSelection, documentFont, iconSelector }));
            let visibleIndex = 0;
            for (const child of control.children) {
                if (!child.values.visible) continue;
                visit(child, control.id, visibleIndex++, depth + 1, enabled);
            }
        };
        if (window.root) visit(window.root);

        const patches = [];
        const replacesWholeTree = window.rootReplacementPending;
        if (replacesWholeTree) {
            patches.push(frozenPatch("clear"));
        } else {
            const removals = [...window.rendered.values()]
                .filter(widget => !desired.has(widget.id))
                .sort((left, right) => right.depth - left.depth);
            for (const widget of removals) patches.push(frozenPatch("remove", { id: widget.id }));
        }

        for (const widget of desired.values()) {
            const previous = replacesWholeTree
                ? null
                : window.rendered.get(widget.id);
            if (!previous) {
                const scene = retainedScene(this.controls.get(widget.id));
                if (scene) this.queueScene(this.controls.get(widget.id), scene);
                patches.push(frozenPatch("create", {
                    id: widget.id,
                    parent: widget.parent,
                    index: widget.index,
                    kind: widget.kind,
                    state: widget.state,
                    text: widget.text,
                    icon: widget.icon,
                }));
                if (widget.observeSize) patches.push(frozenPatch("observe-size", { id: widget.id, enabled: true }));
                if(widget.iconSelector)patches.push(frozenPatch('icon-selector',{id:widget.id,selector:widget.iconSelector}));
                if (widget.documentFont) patches.push(frozenPatch("document-font", { id: widget.id, font: widget.documentFont }));
                if (widget.editSelection) patches.push(frozenPatch("edit-selection", { id: widget.id, anchor: widget.editSelection.anchor, caret: widget.editSelection.caret }));
                continue;
            }
            if (previous.observeSize !== widget.observeSize) patches.push(frozenPatch("observe-size", { id: widget.id, enabled: widget.observeSize }));
            const control = this.controls.get(widget.id);
            const damage = previous.text !== widget.text ? (control?.nativeGridDamage?.(previous.text) ?? null) : null;
            const update = { id: widget.id };
            let changed = false;
            for (const property of ["state", "text", "icon"]) {
                if (property === "text" && damage !== null) continue;
                if (!Object.is(previous[property], widget[property])) {
                    update[property] = widget[property];
                    changed = true;
                }
            }
            if (changed) patches.push(frozenPatch("update", update));
            if(widget.iconSelector!==previous.iconSelector)patches.push(frozenPatch('icon-selector',{id:widget.id,selector:widget.iconSelector}));
            if (widget.documentFont !== previous.documentFont) patches.push(frozenPatch("document-font", { id: widget.id, font: widget.documentFont }));
            if (widget.editSelection && previous.editSelection?.revision !== widget.editSelection.revision) patches.push(frozenPatch("edit-selection", { id: widget.id, anchor: widget.editSelection.anchor, caret: widget.editSelection.caret }));
            if (damage !== null) patches.push(frozenPatch("grid-damage", { id: widget.id, damage }));
            if (previous.parent !== widget.parent || previous.index !== widget.index) {
                patches.push(frozenPatch("move", {
                    id: widget.id,
                    parent: widget.parent,
                    index: widget.index,
                }));
            }
        }
        for (const widget of desired.values()) this.controls.get(widget.id)?.commitPresentation?.();
        window.rendered = desired;
        window.rootReplacementPending = false;
        if (patches.length) window.sequence += 1n;
        return Object.freeze({
            app: this.id,
            window: window.id,
            sequence: window.sequence,
            patches: Object.freeze(patches),
        });
    }

    dispatchUiEvent({ window, target, event }) {
        const control = this.controls.get(BigInt(target));
        const owner = control && this.controlWindows.get(control.id);
        if (!control || !owner || owner.id !== BigInt(window)) return false;
        const interactive = ["activate", "value-changed", "pointer", "key", "text-selection"].includes(event?.type)
            || (event?.type === "focus" && event.focused !== false);
        if (interactive) {
            for (let ancestor = control; ancestor; ancestor = ancestor.parent) {
                if (!ancestor.values.visible || !(ancestor.state & State.ENABLED)) return false;
            }
        }
        if(event?.type==='focus'&&event.focused!==false){
            for(const candidate of this.controls.values())if(candidate!==control&&this.controlWindows.get(candidate.id)===owner&&candidate.values.focused)candidate.setProperty('focused',false);
        }
        const previous=this.hostEventWindow;this.hostEventWindow=owner.id;
        try{return control.dispatchHostEvent(event);}finally{this.hostEventWindow=previous;}
    }

    postControlMessage(target, current) {
        return new Promise((resolve, reject) => {
            this.messageQueue.push({ target, current, resolve, reject });
            if (!this.messageScheduled) {
                this.messageScheduled = true;
                queueMicrotask(() => this.drainMessages());
            }
        });
    }

    drainMessages() {
        this.messageScheduled = false;
        while (this.messageQueue.length) {
            const { target, current, resolve, reject } = this.messageQueue.shift();
            try { resolve(target.sendMessage(current)); } catch (error) { reject(error); }
        }
    }

    /** Synchronous listeners retain their existing decision semantics. An
     * asynchronous APP_CLOSE_REQUEST listener must resolve true or "approved"
     * to approve; false, rejection or any other async result cancels. The core
     * sends Pending while awaiting this Promise; the host owns actual closure.
     */
    requestClose(detail = null) {
        if (this.closed) return "cancelled";
        const generation = this.closeGeneration;
        const pending = [];
        let result, veto = false, failed = false;
        // Unlike ordinary lifecycle emit(), a later listener must not silently
        // discard an earlier asynchronous save/confirmation decision.
        for (const listener of [...(this.lifecycle.get(Message.APP_CLOSE_REQUEST) || [])]) {
            try {
                const candidate = listener(detail);
                if (candidate !== undefined) result = candidate;
                veto ||= candidate === false || candidate === "cancelled" || candidate === "pending";
                if (candidate && typeof candidate.then === "function") {
                    // Attach rejection handling immediately, including if a
                    // subsequent listener throws. Invalid async results veto.
                    pending.push(Promise.resolve(candidate).then(
                        value => value === true || value === "approved",
                        () => false,
                    ));
                }
            } catch { failed = true; }
        }
        if (failed || this.closed) return "cancelled";
        if (!pending.length) {
            // Preserve existing synchronous listener behavior.
            return result === false ? "cancelled" : result === "pending" ? "pending" : "approved";
        }
        return Promise.all(pending).then(decisions =>
            !this.closed && this.closeGeneration === generation && !veto && decisions.every(Boolean)
                ? "approved" : "cancelled");
    }

    controlSnapshot() {
        return Object.freeze([...this.controls.values()].slice(0, MAX_WIDGETS).map(control => Object.freeze({
            id: control.id.toString(),
            parent: control.parent?.id.toString() || null,
            type: control.type,
            family: control.family,
            name: control.name,
            state: control.state,
            text: control.text,
            children: control.children.length,
        })));
    }

    close(detail = null) {
        this.closed = true;
        this.closeGeneration += 1;
        this.emit(Message.APP_CLOSE, detail);
        for (const window of [...this.windows.values()]) window.close();
        this.pendingScenes.clear();
        this.sceneListeners.clear();
        this.started = false;
    }
}

export function defineApp(definition) {
    if (!definition || typeof definition !== "object") throw new TypeError("app definition is required");
    if (typeof definition.start !== "function") throw new TypeError("defineApp requires start");
    if (definition.manifest !== undefined && (!definition.manifest || typeof definition.manifest !== "object" || Array.isArray(definition.manifest))) {
        throw new TypeError("An explicit app manifest must be an object");
    }
    return Object.freeze({ ...definition });
}
