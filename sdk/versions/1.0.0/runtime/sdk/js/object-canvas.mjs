import { Stack, Label, TextField } from './controls.mjs';
import { SceneSurface, MAX_SCENE_BYTES } from './scene-surface.mjs';
import { SceneOverlay } from './scene-overlay.mjs';
import { SceneBuilder } from './scene-builder.mjs';
import { State } from './control.mjs';
import { Message } from './messages.mjs';
import { resolveChartTheme } from './chart-theme.mjs';
import {
    SpatialIndex,
    containsShape,
    rotatedBounds,
    unionBounds,
    shapePath,
    routeConnector,
    rgbaColor,
    visualCorners,
    rectangleHandles,
} from './visual-geometry.mjs';
import {
    createVisualHistory,
    createVisualDocument,
    encodeVisualSelection,
    pasteVisualSelection,
} from './visual-document.mjs';
import { ObjectGestureController } from './object-gestures.mjs';
import { ConnectorRouteCache } from './connector-route-cache.mjs';
import { layoutObjects } from './visual-layout.mjs';
import { VisualResourceStore, decodeVisualImage, encodeVisualImage } from './visual-resources.mjs';
import { richTextPlainText } from './visual-document.mjs';

/** General retained object editor. Application data lives in the supplied history. */
export class ObjectCanvas extends Stack {
    static family = 'composite';
    static constructorOptions = Object.freeze([
        'history',
        'document',
        'width',
        'height',
        'grid',
        'colors',
        'onSelection',
        'onActivate',
        'onContextMenu',
        'maximumLabels',
        'minimap',
        'readOnly',
        'textRenderer',
        'routeConnection',
    ]);
    static authoring = Object.freeze({
        constructor:
            'new ObjectCanvas({history:createVisualHistory(document),width:1000,height:680,grid:0,minimap:true,onSelection(ids),onActivate(object),onContextMenu(detail)})',
        methods:
            'select(ids), selection, panBy(dx,dy), zoomAt(zoom,screenPoint), fit(ids?), setFilter(predicate), setLayout(options), beginEdit(id), connect(source,target,options), copy(services), cut(services), paste(services), undo(), redo(), inspect({start,limit}), destroy()',
        interaction:
            'Drag objects or empty space to pan; Shift drag empty space selects a rectangle. Ctrl/Meta click toggles selection. Each selected object has eight round corner/edge handles: grabbing one resizes that object while preserving selection. The handle beyond its local top rotates the unlocked selection around that object center, including positions. Ctrl wheel zooms at pointer. Arrows move selection (Shift:10 units); Ctrl/Meta C/X/V copies/cuts/pastes with granted clipboard services; Ctrl/Meta Z/Y undo/redo; F2 edits text, Delete removes, Escape cancels, Home fits, Tab navigates objects, Space then arrows moves a keyboard-picked selection and Enter drops. Active theme colors by default; object style overrides are semantic data. One gesture equals one undo step.',
        inspection:
            'Native control metadata includes bounded semantic objects with ids, screen bounds, selected state and available actions; inspect({start,limit}) pages all objects. Labels are culled, pooled and limited; low zoom shows shapes without allocating a widget per object.',
    });
    constructor({
        history,
        document,
        width = 1000,
        height = 680,
        grid = 0,
        colors = {},
        onSelection,
        onActivate,
        onContextMenu,
        maximumLabels = 160,
        minimap = true,
        readOnly = false,
        textRenderer = null,
        routeConnection = routeConnector,
        ...options
    } = {}) {
        if (
            ![width, height].every((v) => Number.isInteger(v) && v >= 32 && v <= 4096) ||
            !Number.isInteger(maximumLabels) ||
            maximumLabels < 0 ||
            maximumLabels > 256
        )
            throw TypeError('Invalid object canvas dimensions');
        super({ ...options, text: 'panel:flush', state: State.ENABLED | State.EXPANSIVE | State.FLAT });
        this.history = history ?? createVisualHistory(document ?? createVisualDocument());
        this.ownsHistory = !history;
        this.width = width;
        this.height = height;
        this.view = { x: 0, y: 0, zoom: 1 };
        this.selection = [];
        this.preview = new Map();
        this.labels = [];
        this.labelLimit = maximumLabels;
        this.showMinimap = minimap;
        this.readOnly = readOnly;
        this.filter = () => true;
        this.colors = resolveChartTheme({}, colors);
        this.colorOverrides = colors;
        this.onSelection = onSelection;
        this.onActivate = onActivate;
        this.onContextMenu = onContextMenu;
        this.index = new SpatialIndex();
        this.selectionListeners = new Set();
        this.resources = new VisualResourceStore();
        this.tool = 'select';
        this.textRenderer = textRenderer;
        this.routeCache = new ConnectorRouteCache({ route: routeConnection });
        this.awaitingText = new Set();
        this.warnings = new Map();
        this.surface = new SceneSurface({
            name: (options.name || 'object-canvas') + '-surface',
            state: State.ENABLED | State.EXPANSIVE | State.FLAT | State.KEY_NAVIGATION,
        });
        this.append(this.surface);
        this.gestures = new ObjectGestureController({
            grid,
            onPreview: (objects) => {
                this.preview = new Map(objects.map((o) => [o.id, o]));
                this.refresh();
            },
            onCancel: () => {
                this.preview.clear();
                this.refresh();
            },
            acceptDrop: (objects, target) => {
                const ids = new Set(objects.map((o) => o.id));
                let current = target;
                while (current) {
                    if (ids.has(current.id)) return false;
                    current = this.byId.get(current.parent);
                }
                return Boolean(target.droppable);
            },
            onCommit: (objects, { mode, dropTarget }) => {
                this.preview.clear();
                this.history.transact(
                    mode === 'move'
                        ? 'Move objects'
                        : mode === 'resize'
                          ? 'Resize objects'
                          : 'Rotate objects',
                    (draft) => {
                        for (const next of objects) {
                            const object = draft.objects.find((o) => o.id === next.id);
                            Object.assign(object, {
                                x: next.x,
                                y: next.y,
                                width: next.width,
                                height: next.height,
                                ...(next.rotation !== undefined ? { rotation: next.rotation } : {}),
                            });
                            if (dropTarget && mode === 'move') object.parent = dropTarget.id;
                        }
                    },
                );
            },
        });
        this.surface.on(Message.SIZE_CHANGED, (event) => {
            const { width, height } = event.detail || {};
            if (width > 0 && height > 0 && (width !== this.width || height !== this.height)) {
                this.width = Math.min(4096, width);
                this.height = Math.min(4096, height);
                this.refresh();
            }
        });
        this.surface.on(Message.POINTER, (event) => this.pointer(event));
        this.surface.on(Message.KEY, (event) => this.key(event));
        this.surface.on(Message.BLUR, () => this.cancelGesture());
        this.on(Message.CREATED, (event) => {
            if (event.target !== this) return;
            this.setTheme(this.application.theme || {});
            this.stopTheme = this.application.on(Message.THEME_CHANGED, (theme) => this.setTheme(theme));
        });
        this.stopHistory = this.history.subscribe(() => {
            if (this.indexedValue !== this.history.value) this.reindex();
            this.selection = this.selection.filter((id) => this.byId.has(id));
            this.refresh();
        });
        this.reindex();
        this.refresh();
    }
    setTheme(theme) {
        this.colors = resolveChartTheme(theme, this.colorOverrides);
        this.refresh();
    }
    reindex() {
        this.indexedValue = this.history.value;
        this.byId = new Map(this.history.value.objects.map((o) => [o.id, o]));
        this.index.clear();
        this.hidden = new Set();
        for (const o of this.byId.values()) {
            let parent = this.byId.get(o.parent);
            while (parent) {
                if (parent.collapsed) {
                    this.hidden.add(o.id);
                    break;
                }
                parent = this.byId.get(parent.parent);
            }
            if (!this.hidden.has(o.id) && this.filter(o)) this.index.set(o.id, rotatedBounds(o));
        }
        this.resources.clear();
        for (const [id, record] of Object.entries(this.history.value.resources || {}))
            this.resources.set(id, decodeVisualImage(record));
    }
    setFilter(predicate) {
        if (typeof predicate !== 'function') throw TypeError('Expected object predicate');
        this.filter = predicate;
        this.reindex();
        this.refresh();
    }
    screenToWorld(point) {
        return { x: this.view.x + point.x / this.view.zoom, y: this.view.y + point.y / this.view.zoom };
    }
    worldToScreen(point) {
        return { x: (point.x - this.view.x) * this.view.zoom, y: (point.y - this.view.y) * this.view.zoom };
    }
    screenBounds(object) {
        const r = rotatedBounds(object),
            p = this.worldToScreen(r);
        return { ...p, width: r.width * this.view.zoom, height: r.height * this.view.zoom };
    }
    hitTest(point) {
        const ids = new Set(this.index.query({ x: point.x, y: point.y, width: 0.001, height: 0.001 }));
        return this.history.value.objects
            .filter((o) => ids.has(o.id))
            .reverse()
            .find((o) => containsShape(this.preview.get(o.id) || o, point));
    }
    select(ids) {
        if (!Array.isArray(ids) || ids.some((id) => !this.byId.has(id)))
            throw TypeError('Unknown selected object');
        this.selection = [...new Set(ids)];
        this.onSelection?.([...this.selection]);
        for (const listener of this.selectionListeners) listener([...this.selection]);
        this.refresh();
        return this;
    }
    onSelectionChanged(listener) {
        if (typeof listener !== 'function') throw TypeError('Expected selection listener');
        this.selectionListeners.add(listener);
        return () => this.selectionListeners.delete(listener);
    }
    setTool(tool) {
        if (!['select', 'rect', 'roundedRect', 'ellipse', 'pill', 'diamond', 'connect'].includes(tool))
            throw TypeError('Unknown object tool');
        this.cancelGesture();
        this.tool = tool;
        this.surface.setProperty('cursor', tool === 'select' ? 'default' : 'crosshair');
        this.refresh();
    }
    addObject(descriptor = {}) {
        if (this.readOnly) return false;
        let id = descriptor.id ?? 'object-' + (this.history.revision + 1),
            n = 0;
        while (this.byId.has(id)) id = 'object-' + this.history.revision + '-' + ++n;
        this.history.transact('Add object', (d) => {
            d.objects.push({
                x: 0,
                y: 0,
                width: 160,
                height: 64,
                shape: 'roundedRect',
                text: '',
                ...descriptor,
                id,
            });
        });
        this.select([id]);
        return id;
    }
    addImage(id, image) {
        if (this.readOnly) return false;
        const record = encodeVisualImage(image);
        this.history.transact('Add image resource', (d) => {
            d.resources ??= {};
            d.resources[id] = record;
        });
        return id;
    }
    panBy(dx, dy) {
        if (![dx, dy].every(Number.isFinite)) throw TypeError('Invalid pan');
        this.view.x -= dx / this.view.zoom;
        this.view.y -= dy / this.view.zoom;
        this.refresh();
    }
    zoomAt(zoom, point = { x: this.width / 2, y: this.height / 2 }) {
        if (!Number.isFinite(zoom) || zoom < 0.02 || zoom > 16 || ![point.x, point.y].every(Number.isFinite))
            throw RangeError('Zoom must be .02..16');
        const anchor = this.screenToWorld(point);
        this.view = { zoom, x: anchor.x - point.x / zoom, y: anchor.y - point.y / zoom };
        this.refresh();
    }
    fit(ids) {
        const objects = ids
                ? ids.map((id) => this.byId.get(id)).filter(Boolean)
                : this.history.value.objects.filter((o) => this.index.entries.has(o.id)),
            bounds = unionBounds(objects.map(rotatedBounds)),
            zoom = Math.max(
                0.02,
                Math.min(2, (this.width - 80) / bounds.width, (this.height - 80) / bounds.height),
            );
        this.view = {
            zoom,
            x: bounds.x + bounds.width / 2 - this.width / zoom / 2,
            y: bounds.y + bounds.height / 2 - this.height / zoom / 2,
        };
        this.refresh();
    }
    setLayout(options = {}) {
        if (this.readOnly) return false;
        const positions = layoutObjects(this.history.value.objects, {
            connections: this.history.value.connections,
            ...options,
        });
        return this.history.transact('Arrange objects', (draft) => {
            for (const object of draft.objects) {
                const position = positions.find((p) => p.id === object.id);
                object.x = position.x;
                object.y = position.y;
            }
        });
    }
    connect(source, target, options = {}) {
        if (this.readOnly) return false;
        if (!this.byId.has(source) || !this.byId.has(target)) throw TypeError('Unknown connection endpoints');
        let id = options.id ?? 'connection-' + (this.history.revision + 1),
            n = 0;
        while (this.history.value.connections.some((e) => e.id === id))
            id = 'connection-' + this.history.revision + '-' + ++n;
        this.history.transact('Connect objects', (draft) => {
            draft.connections.push({ ...options, id, source, target });
        });
        return id;
    }
    deleteSelection() {
        if (this.readOnly) return false;
        const selected = new Set(this.selection);
        for (const o of this.byId.values()) {
            let parent = o.parent;
            while (parent) {
                if (selected.has(parent)) {
                    selected.add(o.id);
                    break;
                }
                parent = this.byId.get(parent)?.parent;
            }
        }
        this.history.transact('Delete objects', (draft) => {
            draft.objects = draft.objects.filter((o) => !selected.has(o.id) || o.locked);
            const remaining = new Set(draft.objects.map((o) => o.id));
            for (const o of draft.objects) if (o.parent && !remaining.has(o.parent)) o.parent = null;
            draft.connections = draft.connections.filter(
                (e) => remaining.has(e.source) && remaining.has(e.target),
            );
        });
        this.select([]);
    }
    async copy(services = this.application?.services) {
        if (!services?.clipboard) throw Error('Clipboard services unavailable');
        const text = encodeVisualSelection(this.history.value, this.selection);
        await services.clipboard.writeText({ text });
        return { objects: this.selection.length };
    }
    async cut(services) {
        if (this.readOnly) return false;
        const revision = this.history.revision,
            ids = [...this.selection];
        await this.copy(services);
        if (
            this.destroyed ||
            this.history.revision !== revision ||
            JSON.stringify(this.selection) !== JSON.stringify(ids)
        )
            throw Error('Selection changed while copying');
        this.deleteSelection();
    }
    async paste(services = this.application?.services, options = {}) {
        if (this.readOnly) return false;
        if (!services?.clipboard) throw Error('Clipboard services unavailable');
        const revision = this.history.revision,
            reply = await services.clipboard.readText({}),
            text = typeof reply === 'string' ? reply : reply?.text;
        if (this.destroyed || this.history.revision !== revision)
            throw Error('Document changed while reading clipboard');
        const ids = pasteVisualSelection(this.history, text, options);
        this.select(ids);
        return ids;
    }
    undo() {
        if (this.readOnly) return false;
        this.cancelGesture();
        return this.history.undo();
    }
    redo() {
        if (this.readOnly) return false;
        this.cancelGesture();
        return this.history.redo();
    }
    cancelGesture() {
        this.pan = null;
        this.marquee = null;
        this.keyboardMove = null;
        this.connecting = null;
        this.drawing = null;
        this.gestures.cancel();
        this.preview.clear();
        this.refresh();
    }
    pointer(event) {
        const d = event.detail || {},
            screen = { x: d.x, y: d.y },
            point = this.screenToWorld(screen),
            primary = [0, 1].includes(d.button);
        if (d.kind === 'wheel') {
            const direction = d.button === 4 ? 1 : -1;
            if (d.modifiers & 4)
                this.zoomAt(
                    Math.max(0.02, Math.min(16, this.view.zoom * (direction > 0 ? 1.15 : 1 / 1.15))),
                    screen,
                );
            else this.panBy(d.modifiers & 1 ? direction * 48 : 0, d.modifiers & 1 ? 0 : direction * 48);
            event.stopPropagation();
            return;
        }
        if (['cancel', 'leave'].includes(d.kind)) {
            this.cancelGesture();
            return;
        }
        if (d.kind === 'down' && d.button === 3) {
            this.onContextMenu?.({ point, object: this.hitTest(point) });
            return;
        }
        if (d.kind === 'down' && primary)
            this.surface.setProperty('state', this.surface.state | State.KEY_NAVIGATION);
        if (!this.readOnly && this.tool !== 'select') {
            if (d.kind === 'down' && primary) {
                if (this.tool === 'connect') {
                    const source = this.hitTest(point);
                    if (source) this.connecting = { source: source.id, point };
                } else this.drawing = { start: point, end: point, shape: this.tool };
                this.refresh();
                return;
            }
            if (d.kind === 'move') {
                if (this.drawing) this.drawing.end = point;
                if (this.connecting) this.connecting.point = point;
                this.refresh();
                return;
            }
            if (d.kind === 'up') {
                if (this.drawing) {
                    const { start, end, shape } = this.drawing;
                    this.drawing = null;
                    this.addObject({
                        x: Math.min(start.x, end.x),
                        y: Math.min(start.y, end.y),
                        width: Math.max(16, Math.abs(end.x - start.x)),
                        height: Math.max(16, Math.abs(end.y - start.y)),
                        shape,
                    });
                }
                if (this.connecting) {
                    const target = this.hitTest(point),
                        source = this.connecting.source;
                    this.connecting = null;
                    if (target && target.id !== source) this.connect(source, target.id);
                }
                this.setTool('select');
                return;
            }
        }
        if (d.kind === 'down' && primary) {
            if (
                this.minimapRect &&
                screen.x >= this.minimapRect.x &&
                screen.y >= this.minimapRect.y &&
                screen.x < this.minimapRect.x + this.minimapRect.width &&
                screen.y < this.minimapRect.y + this.minimapRect.height
            ) {
                const m = this.minimapRect;
                this.view.x = m.bounds.x + (screen.x - m.x) / m.scale - this.width / this.view.zoom / 2;
                this.view.y = m.bounds.y + (screen.y - m.y) / m.scale - this.height / this.view.zoom / 2;
                this.refresh();
                return;
            }
            const handle = this.handles?.slice().reverse().find((h) => Math.hypot(screen.x - h.x, screen.y - h.y) < 9);
            if (handle && !this.readOnly) {
                this.gestures.begin({
                    point,
                    objects: this.selection.map((id) => this.byId.get(id)),
                    mode: handle.mode,
                    handle: handle.handle,
                    ...(handle.mode === 'resize' ? { targetId: handle.object } : { pivot: handle.pivot }),
                });
                return;
            }
            const object = this.hitTest(point);
            if (!object) {
                if (d.modifiers & 1) {
                    this.marquee = { start: point, end: point };
                } else {
                    this.pan = { screen, view: { ...this.view } };
                    if (!(d.modifiers & 4)) this.select([]);
                }
                return;
            }
            if (d.modifiers & 4)
                this.select(
                    this.selection.includes(object.id)
                        ? this.selection.filter((id) => id !== object.id)
                        : [...this.selection, object.id],
                );
            else if (!this.selection.includes(object.id)) this.select([object.id]);
            if (!this.readOnly && !object.locked) {
                const ids = new Set(this.selection);
                for (const o of this.byId.values()) {
                    let parent = o.parent;
                    while (parent) {
                        if (ids.has(parent)) {
                            ids.add(o.id);
                            break;
                        }
                        parent = this.byId.get(parent)?.parent;
                    }
                }
                this.gestures.begin({ point, objects: [...ids].map((id) => this.byId.get(id)) });
            }
        } else if (d.kind === 'move') {
            if (this.pan) {
                this.view = {
                    ...this.pan.view,
                    x: this.pan.view.x - (screen.x - this.pan.screen.x) / this.view.zoom,
                    y: this.pan.view.y - (screen.y - this.pan.screen.y) / this.view.zoom,
                };
                this.refresh();
            } else if (this.marquee) {
                this.marquee.end = point;
                this.refresh();
            } else
                this.gestures.update(point, {
                    scale: this.view.zoom,
                    constrain: Boolean(d.modifiers & 1),
                    dropTarget: this.hitTest(point),
                });
        } else if (d.kind === 'up') {
            if (this.marquee) {
                const { start, end } = this.marquee;
                this.marquee = null;
                this.select(
                    this.index.query({
                        x: Math.min(start.x, end.x),
                        y: Math.min(start.y, end.y),
                        width: Math.max(0.001, Math.abs(end.x - start.x)),
                        height: Math.max(0.001, Math.abs(end.y - start.y)),
                    }),
                );
            }
            this.pan = null;
            this.gestures.finish();
            this.refresh();
        }
    }
    key(event) {
        const d = event.detail || {};
        if (d.pressed === false) return;
        const code = d.keyCode,
            ctrl = Boolean(d.modifiers & (4 | 64)),
            shift = Boolean(d.modifiers & 1);
        if (code === 9) {
            this.cancelGesture();
            this.endEdit(false);
            this.surface.setProperty('state', this.surface.state & ~State.KEY_NAVIGATION);
            event.preventDefault();
            return;
        }
        if (ctrl && [54, 53, 55].includes(code)) {
            const work = code === 54 ? this.copy() : code === 53 ? this.cut() : this.paste();
            this.application?.observeHostWork?.(
                Promise.resolve(work),
                this.application.controlWindows.get(this.id)?.id,
            );
            return work;
        }
        if (ctrl && [52, 29].includes(code)) {
            code === 29 || shift ? this.redo() : this.undo();
            event.preventDefault();
            return;
        }
        if (code === 110) {
            this.fit();
            event.preventDefault();
            return;
        }
        if (code === 23) {
            const objects = this.history.value.objects.filter((o) => this.index.entries.has(o.id)),
                i = objects.findIndex((o) => o.id === this.selection.at(-1)),
                next = objects[(i + (shift ? -1 : 1) + objects.length) % objects.length];
            if (next) {
                this.select([next.id]);
                const b = this.screenBounds(next);
                if (b.x < 0 || b.y < 0 || b.x + b.width > this.width || b.y + b.height > this.height) {
                    this.view.x = next.x + next.width / 2 - this.width / this.view.zoom / 2;
                    this.view.y = next.y + next.height / 2 - this.height / this.view.zoom / 2;
                    this.refresh();
                }
            }
            event.preventDefault();
            return;
        }
        if (this.readOnly) return;
        if (ctrl && code === 38) {
            this.select([...this.index.entries.keys()]);
            event.preventDefault();
            return;
        }
        if (code === 118) {
            const center = this.screenToWorld({ x: this.width / 2, y: this.height / 2 });
            this.addObject({
                ...center,
                shape: ['select', 'connect'].includes(this.tool) ? 'roundedRect' : this.tool,
            });
            event.preventDefault();
            return;
        }
        if (code === 54 && !ctrl && this.selection.length === 1) {
            this.connecting = { source: this.selection[0], point: null };
            event.preventDefault();
            return;
        }
        if (code === 68 && this.selection.length === 1) {
            this.beginEdit(this.selection[0]);
            event.preventDefault();
            return;
        }
        if (code === 119) {
            this.deleteSelection();
            event.preventDefault();
            return;
        }
        if (code === 65 && this.selection.length) {
            this.keyboardMove = { point: { x: 0, y: 0 } };
            this.gestures.begin({
                point: { x: 0, y: 0 },
                objects: this.selection.map((id) => this.byId.get(id)),
            });
            event.preventDefault();
            return;
        }
        if ([36, 104].includes(code)) {
            if (this.keyboardMove) {
                this.gestures.finish();
                this.keyboardMove = null;
            } else if (this.connecting && this.selection.length === 1) {
                const source = this.connecting.source;
                this.connecting = null;
                if (this.selection[0] !== source) this.connect(source, this.selection[0]);
            } else if (this.selection.length === 1) this.onActivate?.(this.byId.get(this.selection[0]));
            event.preventDefault();
            return;
        }
        if ([111, 113, 114, 116].includes(code) && this.selection.length) {
            const delta = shift ? 10 : 1,
                dx = code === 113 ? -delta : code === 114 ? delta : 0,
                dy = code === 111 ? -delta : code === 116 ? delta : 0;
            if (this.keyboardMove) {
                this.keyboardMove.point.x += dx;
                this.keyboardMove.point.y += dy;
                this.gestures.update(this.keyboardMove.point, { scale: 100 });
            } else
                this.history.transact('Move objects', (draft) => {
                    for (const o of draft.objects)
                        if (this.selection.includes(o.id) && !o.locked) {
                            o.x += dx;
                            o.y += dy;
                        }
                });
            event.preventDefault();
        }
    }
    beginEdit(id) {
        if (this.readOnly) return false;
        const object = this.byId.get(id);
        if (!object || object.locked) throw TypeError('Object cannot be edited');
        this.endEdit(false);
        const input = new TextField({
                name: 'object-text-editor',
                value: object.text || '',
                accessibleName: 'Object text',
            }),
            overlay = new SceneOverlay(
                { surface: this.surface, bounds: { x: 0, y: 0, width: 1, height: 1 } },
                [input],
            );
        this.edit = { id, input, overlay, revision: this.history.revision };
        input.on(Message.KEY, (e) => {
            if (e.detail?.pressed === false) return;
            if ([36, 104].includes(e.detail.keyCode)) {
                this.endEdit(true);
                e.preventDefault();
            } else if (e.detail.keyCode === 9) {
                this.endEdit(false);
                e.preventDefault();
            }
        });
        this.append(overlay);
        this.refresh();
        input.setProperty('focused', true);
        return true;
    }
    endEdit(commit = true) {
        const edit = this.edit;
        if (!edit) return false;
        this.edit = null;
        const text = edit.input.value;
        this.remove(edit.overlay);
        edit.overlay.destroy();
        if (commit) {
            if (edit.revision !== this.history.revision) throw Error('Object changed while editing');
            this.history.transact('Edit object text', (draft) => {
                const o = draft.objects.find((o) => o.id === edit.id);
                if (o) o.text = text;
            });
        }
        this.surface.setProperty('focused', true);
        this.refresh();
        return true;
    }
    inspect({ start = 0, limit = 200 } = {}) {
        if (!Number.isInteger(start) || start < 0 || !Number.isInteger(limit) || limit < 1 || limit > 500)
            throw RangeError('Object inspection needs start >= 0 and limit 1..500');
        const objects = this.history.value.objects.slice(start, start + limit).map((o) => ({
            id: o.id,
            text: o.text || '',
            bounds: { x: o.x, y: o.y, width: o.width, height: o.height },
            screenBounds: this.screenBounds(o),
            selected: this.selection.includes(o.id),
            visible: this.index.entries.has(o.id),
            locked: !!o.locked,
            parent: o.parent ?? null,
            tags: o.tags || [],
            actions:
                this.readOnly || o.locked
                    ? ['select']
                    : ['select', 'move', 'resize', 'rotate', 'edit', 'copy', 'delete'],
        }));
        return {
            revision: this.history.revision,
            total: this.byId.size,
            start,
            objects,
            selection: [...this.selection],
            connections: this.history.value.connections.length,
            view: { ...this.view },
            performance: this.performance,
            warnings: [...this.warnings.values()].slice(-16),
        };
    }
    inspectControl(query) {
        const result = this.inspect(query || { limit: 100 });
        if (!query) {
            const visible = this.index.query({
                    x: this.view.x,
                    y: this.view.y,
                    width: this.width / this.view.zoom,
                    height: this.height / this.view.zoom,
                }),
                ids = [...new Set([...this.selection, ...visible])].slice(0, 100);
            result.objects = ids.map((id) => {
                const o = this.byId.get(id);
                return {
                    id,
                    text: o.text || '',
                    bounds: { x: o.x, y: o.y, width: o.width, height: o.height },
                    screenBounds: this.screenBounds(o),
                    selected: this.selection.includes(id),
                    visible: this.index.entries.has(id),
                    locked: !!o.locked,
                    actions:
                        this.readOnly || o.locked
                            ? ['select']
                            : ['select', 'move', 'resize', 'rotate', 'edit', 'copy', 'delete'],
                };
            });
            result.visiblePage = true;
        }
        return { ...result, surface: String(this.surface.id), width: this.width, height: this.height };
    }
    refresh() {
        if (this.destroyed || this.rendering || !this.surface) return;
        this.rendering = true;
        try {
            const start = globalThis.performance?.now?.() ?? Date.now(),
                b = new SceneBuilder(this.width, this.height, {
                    maximumBytes: Math.min(
                        MAX_SCENE_BYTES,
                        (this.application?.maximumSceneFrameBytes || 1048576) - 56,
                    ),
                }),
                z = this.view.zoom,
                c = this.colors,
                imageIds = new Map();
            let path = 1,
                labelCount = 0;
            b.rect([0, 0, this.width, this.height], rgbaColor(c.background));
            const viewport = {
                    x: this.view.x,
                    y: this.view.y,
                    width: this.width / z,
                    height: this.height / z,
                },
                visible = new Set(this.index.query(viewport));
            for (const o of this.preview.values()) visible.add(o.id);
            const objects = this.history.value.objects
                    .filter((o) => visible.has(o.id))
                    .map((o) => this.preview.get(o.id) || o),
                imageCount = new Set(objects.map((o) => o.image).filter(Boolean)).size,
                imageBudget = Math.max(
                    1,
                    Math.floor(
                        Math.min(4 * 1024 * 1024, (this.application?.maximumSceneFrameBytes || 1048576) / 4) /
                            4 /
                            Math.max(1, imageCount),
                    ),
                ),
                lookup = (id) => this.preview.get(id) || this.byId.get(id),
                simple = z < 0.3 || objects.length * 400 > b.maximumBytes / 2;
            const placeLabel = (text, p, style = {}) => {
                if (labelCount >= this.labelLimit) return;
                let slot = this.labels[labelCount];
                if (!slot) {
                    const label = new Label({
                            name: 'object-label-' + labelCount,
                            enabled: false,
                            state: State.FLAT | State.CENTER_ALIGNED,
                        }),
                        overlay = new SceneOverlay(
                            { surface: this.surface, bounds: { x: 0, y: 0, width: 1, height: 1 } },
                            [label],
                        );
                    this.append(overlay);
                    slot = { label, overlay };
                    this.labels.push(slot);
                }
                const caption = Array.from(text);
                slot.label.text = caption.length > 1024 ? caption.slice(0, 1023).join('') + '…' : text;
                let foreground = style.foreground || c.foreground;
                if (!style.foreground && style.fill) {
                    const rgb = style.fill
                            .slice(1)
                            .match(/../g)
                            .map((v) => parseInt(v, 16) / 255),
                        linear = rgb.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
                    foreground =
                        linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722 > 0.179
                            ? '#000000'
                            : '#ffffff';
                }
                slot.label.setProperty('documentFont', {
                    family: style.fontFamily || 'sans-serif',
                    size_pt: Math.max(1, Math.min(409, Math.round((style.fontSize || 16) * z * 0.75))),
                    weight: style.bold ? 700 : 400,
                    italic: !!style.italic,
                    underline: !!style.underline,
                });
                slot.label.setProperty('documentPaint', { foreground, background: c.background });
                slot.overlay.setBounds({
                    x: p.x / this.width,
                    y: p.y / this.height,
                    width: p.width / this.width,
                    height: p.height / this.height,
                });
                slot.overlay.visible = true;
                labelCount++;
            };
            b.pushClip([0, 0, this.width, this.height]);
            b.pushTransform([z, 0, 0, z, -this.view.x * z, -this.view.y * z]);
            const postReserve = this.showMinimap ? 64000 : 4096,
                minimumObjectBytes = simple ? 28 : 72,
                edgeLimit =
                    b.maximumBytes -
                    postReserve -
                    objects.length * minimumObjectBytes -
                    (imageCount ? Math.min(262144, b.maximumBytes / 4) : 0),
                simpleEdges = simple || this.history.value.connections.length * 512 > edgeLimit / 2;
            this.warnings.delete('connection-budget');
            let renderedConnections = 0,
                omittedConnections = 0;
            for (const [edgeIndex, edge] of this.history.value.connections.entries()) {
                if (b.bytes + 512 > edgeLimit) {
                    omittedConnections = this.history.value.connections.length - edgeIndex;
                    this.warnings.set(
                        'connection-budget',
                        'Some connections exceed the current vector frame budget. Zoom in to inspect a smaller area.',
                    );
                    break;
                }
                if (!this.index.entries.has(edge.source) || !this.index.entries.has(edge.target)) continue;
                const source = lookup(edge.source),
                    target = lookup(edge.target),
                    bounds = unionBounds([source, target]);
                if (
                    !visible.has(edge.source) &&
                    !visible.has(edge.target) &&
                    !(
                        bounds.x < viewport.x + viewport.width &&
                        bounds.x + bounds.width > viewport.x &&
                        bounds.y < viewport.y + viewport.height &&
                        bounds.y + bounds.height > viewport.y
                    )
                )
                    continue;
                if (simpleEdges) {
                    b.line(
                        [source.x + source.width / 2, source.y + source.height / 2],
                        [target.x + target.width / 2, target.y + target.height / 2],
                        1 / z,
                        rgbaColor(edge.color || c.border),
                    );
                    renderedConnections++;
                    continue;
                }
                const route = this.routeCache.route(source, target, {
                        ...edge,
                        obstacles: edge.routing === 'orthogonal' ? objects : [],
                    }),
                    id = path++;
                b.definePath(id, route.path).strokePath(id, 1.5 / z, rgbaColor(edge.color || c.border));
                if (edge.label && z >= 0.3 && objects.length < this.labelLimit) {
                    const points = route.points,
                        a = points[Math.floor((points.length - 1) / 2)],
                        e = points[Math.ceil((points.length - 1) / 2)],
                        p = this.worldToScreen({ x: (a.x + e.x) / 2, y: (a.y + e.y) / 2 });
                    placeLabel(edge.label, { x: p.x - 80, y: p.y - 26, width: 160, height: 24 });
                }
                renderedConnections++;
                if (edge.arrow !== false) {
                    const p = route.points.at(-1),
                        q = route.points.at(-2),
                        a = Math.atan2(p.y - q.y, p.x - q.x),
                        size = 8 / z,
                        arrow = path++;
                    b.definePath(arrow, [
                        ['M', p.x, p.y],
                        ['L', p.x - Math.cos(a - 0.45) * size, p.y - Math.sin(a - 0.45) * size],
                        ['L', p.x - Math.cos(a + 0.45) * size, p.y - Math.sin(a + 0.45) * size],
                        ['Z'],
                    ]).fillPath(arrow, rgbaColor(edge.color || c.border));
                }
            }
            for (const [objectIndex, object] of objects.entries()) {
                const reserve = postReserve + (objects.length - objectIndex - 1) * minimumObjectBytes,
                    simpleObject =
                        simple ||
                        b.bytes + reserve + (object.shape === 'path' ? object.path.length * 32 + 128 : 512) >
                            b.maximumBytes;
                const style = object.style || {},
                    color = rgbaColor(style.fill || c.panel),
                    stroke = rgbaColor(style.stroke || c.border);
                if (object.rotation && !simple) {
                    const a = (object.rotation * Math.PI) / 180,
                        x = object.x + object.width / 2,
                        y = object.y + object.height / 2;
                    b.pushTransform([
                        Math.cos(a),
                        Math.sin(a),
                        -Math.sin(a),
                        Math.cos(a),
                        x - Math.cos(a) * x + Math.sin(a) * y,
                        y - Math.sin(a) * x - Math.cos(a) * y,
                    ]);
                }
                if (simpleObject) b.rect([object.x, object.y, object.width, object.height], color);
                else if (object.shape === 'ellipse')
                    b.ellipse([object.x, object.y, object.width, object.height], color).ellipse(
                        [object.x, object.y, object.width, object.height],
                        stroke,
                        { filled: false, width: (style.strokeWidth ?? 1) / z },
                    );
                else {
                    const id = path++;
                    b.definePath(id, shapePath(object))
                        .fillPath(id, color)
                        .strokePath(id, (style.strokeWidth ?? 1) / z, stroke);
                }
                if (object.image) {
                    const resource = this.resources.sample(
                        object.image,
                        Math.max(1, object.width * z),
                        Math.max(1, object.height * z),
                        { maximumPixels: imageBudget },
                    );
                    if (
                        resource &&
                        b.bytes + reserve + (imageIds.has(object.image) ? 52 : resource.rgba.length + 76) <
                            b.maximumBytes
                    ) {
                        let resourceId = imageIds.get(object.image);
                        if (!resourceId) {
                            resourceId = imageIds.size + 1;
                            imageIds.set(object.image, resourceId);
                            b.defineImage(resourceId, resource.width, resource.height, resource.rgba);
                        }
                        b.image(resourceId, [object.x, object.y, object.width, object.height]);
                    }
                }
                let vectorText = false;
                if (z >= 0.3 && this.textRenderer && (object.text || object.richText)) {
                    const request = this.textRenderer.request(object, c.foreground);
                    if (request.error) this.warnings.set(request.key, request.error);
                    if (request.frame) {
                        const frame = request.frame;
                        if (b.bytes + reserve + frame.displayList.length + 256 < b.maximumBytes) {
                            b.pushClip([object.x, object.y, object.width, object.height])
                                .pushTransform([
                                    1,
                                    0,
                                    0,
                                    1,
                                    object.x + 8,
                                    object.y + Math.max(4, (object.height - frame.height) / 2),
                                ])
                                .append(frame.displayList)
                                .popTransform()
                                .popClip();
                            vectorText = true;
                        } else
                            this.warnings.set(
                                request.key,
                                'Text omitted from vector frame because its byte budget is full',
                            );
                    }
                    if (request.pending && !this.awaitingText.has(request.key)) {
                        this.awaitingText.add(request.key);
                        const work = request.pending
                            .then(
                                () => {
                                    if (!this.destroyed) this.refresh();
                                },
                                (error) => {
                                    this.warnings.set(request.key, error.message);
                                },
                            )
                            .finally(() => this.awaitingText.delete(request.key));
                        this.application?.observeHostWork?.(
                            work,
                            this.application.controlWindows.get(this.id)?.id,
                        );
                    }
                }
                if (object.rotation && !simple) b.popTransform();
                const objectText = object.richText ? richTextPlainText(object.richText) : object.text;
                if (
                    z >= 0.3 &&
                    !vectorText &&
                    objectText &&
                    labelCount < this.labelLimit &&
                    (object.width * z) / this.width <= 16 &&
                    (object.height * z) / this.height <= 16
                ) {
                    const p = object.rotation
                        ? this.screenBounds(object)
                        : {
                              ...this.worldToScreen(object),
                              width: object.width * z,
                              height: object.height * z,
                          };
                    placeLabel(objectText, p, style);
                }
            }
            if (this.drawing) {
                const { start, end, shape } = this.drawing,
                    id = path++;
                b.definePath(
                    id,
                    shapePath({
                        x: Math.min(start.x, end.x),
                        y: Math.min(start.y, end.y),
                        width: Math.max(1, Math.abs(start.x - end.x)),
                        height: Math.max(1, Math.abs(start.y - end.y)),
                        shape,
                    }),
                )
                    .fillPath(id, rgbaColor(c.accent, 0.12))
                    .strokePath(id, 2 / z, rgbaColor(c.accent));
            }
            if (this.connecting?.point) {
                const source = this.byId.get(this.connecting.source);
                if (source)
                    b.line(
                        [source.x + source.width / 2, source.y + source.height / 2],
                        [this.connecting.point.x, this.connecting.point.y],
                        2 / z,
                        rgbaColor(c.accent),
                    );
            }
            b.popTransform();
            this.handles = [];
            const selectedObjects = this.selection.map(lookup).filter(Boolean);
            for (const o of selectedObjects) {
                const corners=visualCorners(o).map(([x,y])=>{const p=this.worldToScreen({x,y});return[p.x,p.y];});
                for(let n=0;n<4;n++)b.line(corners[n],corners[(n+1)%4],2,rgbaColor(c.accent));
                if (!this.readOnly && !o.locked) {
                    for (const {handle,point:[x,y]} of rectangleHandles(corners)) {
                        this.handles.push({ mode: 'resize', handle, object: o.id, x, y });
                        b.ellipse([x-4.5,y-4.5,9,9],0xffffffff)
                            .ellipse([x-4.5,y-4.5,9,9],rgbaColor(c.accent),{filled:false,width:1});
                    }
                    const top=corners[0].map((v,i)=>(v+corners[1][i])/2),center=corners[0].map((v,i)=>(v+corners[2][i])/2),distance=Math.hypot(top[0]-center[0],top[1]-center[1]);
                    if(distance>1e-9) {
                        const rotate=top.map((v,i)=>v+(v-center[i])*24/distance);
                        this.handles.push({mode:'rotate',handle:'se',object:o.id,pivot:{x:o.x+o.width/2,y:o.y+o.height/2},x:rotate[0],y:rotate[1]});
                        b.line(top,rotate,1,rgbaColor(c.accent)).ellipse([rotate[0]-4,rotate[1]-4,8,8],rgbaColor(c.accent));
                    }
                }
            }
            if (this.marquee) {
                const a = this.worldToScreen(this.marquee.start),
                    q = this.worldToScreen(this.marquee.end),
                    id = path++;
                b.definePath(
                    id,
                    shapePath({
                        x: Math.min(a.x, q.x),
                        y: Math.min(a.y, q.y),
                        width: Math.max(1, Math.abs(a.x - q.x)),
                        height: Math.max(1, Math.abs(a.y - q.y)),
                    }),
                )
                    .fillPath(id, rgbaColor(c.accent, 0.12))
                    .strokePath(id, 1, rgbaColor(c.accent));
            }
            this.minimapRect = null;
            if (this.showMinimap && this.byId.size) {
                const bounds = unionBounds(this.history.value.objects),
                    w = 160,
                    h = 100,
                    x = this.width - w - 16,
                    y = this.height - h - 16,
                    scale = Math.min((w - 12) / bounds.width, (h - 12) / bounds.height);
                if (x >= 0 && y >= 0) {
                    b.pushClip([x, y, w, h]);
                    b.rect([x, y, w, h], rgbaColor(c.panel, 0.95));
                    for (const [i, o] of this.history.value.objects.entries())
                        if (i % Math.max(1, Math.ceil(this.byId.size / 2000)) === 0)
                            b.rect(
                                [
                                    x + 6 + (o.x - bounds.x) * scale,
                                    y + 6 + (o.y - bounds.y) * scale,
                                    Math.max(1, o.width * scale),
                                    Math.max(1, o.height * scale),
                                ],
                                rgbaColor(this.selection.includes(o.id) ? c.accent : c.border),
                            );
                    const id = path++;
                    b.definePath(
                        id,
                        shapePath({
                            x: x + 6 + (this.view.x - bounds.x) * scale,
                            y: y + 6 + (this.view.y - bounds.y) * scale,
                            width: (this.width / z) * scale,
                            height: (this.height / z) * scale,
                        }),
                    ).strokePath(id, 1, rgbaColor(c.accent));
                    b.popClip();
                    this.minimapRect = { x: x + 6, y: y + 6, width: w - 12, height: h - 12, scale, bounds };
                }
            }
            b.popClip();
            for (let i = labelCount; i < this.labels.length; i++) this.labels[i].overlay.visible = false;
            if (this.edit) {
                const o = lookup(this.edit.id);
                if (o) {
                    const r = this.screenBounds(o);
                    this.edit.overlay.setBounds({
                        x: r.x / this.width,
                        y: r.y / this.height,
                        width: r.width / this.width,
                        height: r.height / this.height,
                    });
                }
            }
            while (this.warnings.size > 256) this.warnings.delete(this.warnings.keys().next().value);
            const frame = b.finish();
            this.surface.submit(frame);
            this.performance = {
                objects: this.byId.size,
                visible: objects.length,
                simplified: simple || simpleEdges,
                renderedConnections,
                omittedConnections,
                labels: labelCount,
                frameBytes: frame.displayList.length,
                renderMs: (globalThis.performance?.now?.() ?? Date.now()) - start,
            };
        } finally {
            this.rendering = false;
        }
    }
    destroy() {
        this.endEdit(false);
        this.gestures.destroy();
        this.stopHistory?.();
        this.stopTheme?.();
        this.index.clear();
        this.selectionListeners.clear();
        this.resources.clear();
        this.labels = [];
        this.routeCache.clear();
        if (this.ownsHistory) this.history.destroy();
        super.destroy();
    }
}
