import { loadDeckScene } from './scene-loader.mjs';
import { Button, Column, Container, Label, Message, Row, SceneBuilder, SceneSurface, ScrollView, Stack, State, Tooltip } from '@ricewm/app';
import { geometryFrame } from './scene-preview.mjs';
import { slideContext, emptySlideDoubleClick } from './slide-navigation-context.mjs';
import { createSlideSelection } from './slide-selection.mjs';

// Bound native widgets, including title tooltips and selection borders.
const OUTLINE_BATCH = 64, RETAINED_CARDS = 72;
const slideNumber = item => item.hidden ? [...String(item.index + 1)].map(digit => `${digit}\u0338`).join('') : String(item.index + 1);

/** Presentation-only opacity: badges and selection chrome stay outside this
 * isolated native paint group; overlapping artwork is composited exactly once. */
export function thumbnailAppearance(frame, hidden) {
    return hidden ? new SceneBuilder(frame.width, frame.height)
        .rect([0, 0, frame.width, frame.height], 0xff808080)
        .pushTransform([1, 0, 0, 1, 0, 0], 0.5).append(frame.displayList).popTransform().finish() : frame;
}

/** Bounded retained view/command adapter. Stable identities and all history stay
 * in the engine; thumbnail surfaces share the editor's native vector renderer. */
export function createSlideNavigation({ port, navigate, mutate, selectionChanged = () => {}, previewWork = work => work() }) {
    let closed = false, locked = true, loaded = false, generation = 0, model = null, overview = false, cacheBytes = 0;
    let selectedIndex = 0, scrollOffset = 0, mountedKey = '', previewRequest = null, rendering = null, previewDirty = false, shownId = null, clickedId = null, viewportHeight = 640, bodySize = null, mountedStart = 0, mountedEnd = 0, cardHeight = 100, pageRatio = 16 / 9, viewportWidth = null, measuredCardWidth = null;
    const columns = () => overview ? 3 : 1;
    const height = () => cardHeight;
    const stride = () => height() + 4;
    const visibleCount = () => Math.min(24, (Math.ceil(viewportHeight / stride()) + 1) * columns());
    const buttons = new Map(), cards = [], cache = new Map();
    const selection = createSlideSelection();
    let notifiedSelection = '[]';
    function selectionUpdated() {
        for (const card of cards) {
            const selected = selection.has(card.item.id);
            card.badge.setProperty('selected', card.item.index === selectedIndex);
            card.control.setProperty('selected', selected);
            card.badgeBox.setProperty('selected', card.item.index === selectedIndex);
            card.updateBorder();
            card.context?.update();
        }
        const ids = selection.selected, key = JSON.stringify(ids);
        const deletion = buttons.get('delete-slide');
        if (deletion) deletion.text = ids.length > 1 ? 'Delete selected slides' : 'Delete slide';
        if (key !== notifiedSelection) { notifiedSelection = key; selectionChanged(ids); }
    }
    const list = new Column({ name: 'slide-list', text: 'panel:fixed:gap=0:flush' });
    function button(name, text, callback) {
        const control = new Button({ name, text, enabled: false });
        control.on(Message.ACTIVATE, () => { if (!closed && !locked && control.getProperty('enabled')) void callback(); });
        buttons.set(name, control); return control;
    }
    const available = () => !closed && !locked && loaded;
    // Keep native hit ownership while a document operation is pending.
    // Activation guards and disabled menu commands enforce the lock; removing
    // passive card targets would route right-clicks to an unrelated root menu.
    const ownsPointer = () => !closed && loaded;
    const background = new Button({ name: 'slides-empty-area', text: 'panel:flush', accessibleName: 'Slide navigator background', state: State.FLAT, enabled: false });
    const viewport = new ScrollView({ name: 'slides-scroll', state: State.EXPANSIVE }, [
        new Stack({ name: 'slides-viewport', text: 'panel:flush' }, [background, list])]);
    const emptyContext = slideContext(viewport, { invoke: mutate, available });
    const resetDoubleClick = emptySlideDoubleClick(background, mutate, available);
    const control = new Column({ name: 'slide-navigation', text: 'panel:flush:gap=0', visible: Boolean(port?.outline) }, [viewport]);
    function selectionKey(event) {
        const detail = event.detail;
        const direction = ({ ArrowUp: -1, ArrowDown: 1 })[detail?.key] || ({ 111: -1, 116: 1 })[detail?.keyCode];
        if (available() && detail?.pressed && direction && cards.some(card => card.button === event.target) && !(detail.modifiers & (4 | 8 | 64))) {
            event.preventDefault(); event.stopPropagation();
            const item = model?.slides[selectedIndex + direction];
            if (item) {
                selection.choose(item.id, { shift: Boolean(detail.modifiers & 1) }); selectionUpdated();
                void navigate(item.index, item.id);
            }
            return;
        }
        if (available() && detail?.pressed && (detail.keyCode === 119 || detail.key === 'Delete') && !(detail.modifiers & (4 | 8 | 64))) {
            event.preventDefault(); event.stopPropagation();
            if (selection.selected.length) void mutate('delete-slide', { slides: selection.selected });
            return;
        }
        if (!available() || !detail?.pressed || !(detail.modifiers & (4 | 64)) || detail.modifiers & 8) return;
        if (detail.keyCode !== 38 && String(detail.key || detail.text || '').toLowerCase() !== 'a') return;
        event.preventDefault(); event.stopPropagation(); selection.all(); selectionUpdated();
    }
    for (const target of [control, viewport, background]) target.on(Message.KEY, selectionKey);
    viewport.on(Message.SIZE_CHANGED, event => {
        const widthChanged = viewportWidth !== event.detail.width;
        if (widthChanged) { viewportWidth = event.detail.width; measuredCardWidth = null; }
        if (viewportHeight === event.detail.height && !widthChanged) return;
        viewportHeight = event.detail.height; mount();
        previewDirty = true; requestPreviews();
    });
    viewport.on(Message.SCROLL, ({ detail }) => {
        const offset = Number(String(detail?.position || '').split('/')[0]);
        if (!Number.isFinite(offset) || offset < 0) return;
        scrollOffset = offset;
        mount();
        previewDirty = true; requestPreviews();
    });
    const toolbar = new Row({ name: 'slide-actions', text: 'panel:fixed' }, [
        button('insert-slide', 'New slide', () => mutate('insert-slide')),
        button('duplicate-slide', 'Duplicate slide', () => mutate('duplicate-slide')),
        button('delete-slide', 'Delete slide', () => mutate('delete-slide', { slides: selection.selected })),
        button('move-slide-earlier', 'Move earlier', () => mutate('move-earlier')),
        button('move-slide-later', 'Move later', () => mutate('move-later')),
    ]);
    function setState({ disabled, loaded: isLoaded, index, count }) {
        const wasLocked = locked;
        loaded = Boolean(isLoaded);
        locked = Boolean(disabled || !port?.outline);
        background.setProperty('enabled', ownsPointer()); emptyContext.update();
        if (!available()) resetDoubleClick();
        for (const [name, item] of buttons) {
            let enabled = loaded;
            if (name !== 'insert-slide') enabled &&= count > 0;
            if (name === 'move-slide-earlier') enabled &&= index > 0;
            if (name === 'move-slide-later') enabled &&= index + 1 < count;
            item.setProperty('enabled', !closed && !locked && enabled);
        }
        if (selectedIndex !== index) selection.activate(model?.slides[index]?.id);
        selectedIndex = index;
        for (const card of cards) { card.button.setProperty('enabled', ownsPointer()); card.context.update(); }
        selectionUpdated();
        if (wasLocked && !locked) requestPreviews();
    }
    async function outline(args, revision) {
        const reply = await port.outline(args, revision);
        if (reply.revision !== revision) throw Error('Slide outline revision changed');
        const result = reply.result;
        if (!result || !Array.isArray(result.slides) || result.slides.length > args.limit ||
            !Number.isSafeInteger(result.startSlide) || result.startSlide < 0 ||
            result.slides.some((item, offset) => item.index !== result.startSlide + offset || typeof item.id !== 'string' || !item.id) ||
            (result.nextSlide !== null && result.nextSlide !== result.startSlide + result.slides.length)) throw Error('Invalid slide outline page');
        return result;
    }
    async function locate({ revision, id, index, count }) {
        if (!Number.isSafeInteger(count) || count < 0 || count > 4096) throw Error('Invalid slide count');
        if (!model || model.revision !== revision || model.slides.length !== count) {
            generation++;
            const slides = [];
            do {
                const result = await outline({ startSlide: slides.length, limit: OUTLINE_BATCH }, revision);
                if (closed) return null;
                if (result.startSlide !== slides.length || (count && !result.slides.length)) throw Error('Slide outline made no progress');
                slides.push(...result.slides);
                if (result.nextSlide === null) break;
            } while (slides.length < count);
            if (slides.length !== count || new Set(slides.map(item => item.id)).size !== count) throw Error('Slide outline count or identities changed');
            // Keep the previous painted frames until their replacements arrive.
            // Revision tags require a refresh without blanking the whole sidebar.
            if (model?.slides.length !== count) measuredCardWidth = null;
            model = { revision, slides, byId: new Map(slides.map(item => [item.id, item])) };
            selection.reconcile(slides.map(item => item.id));
            selectionUpdated();
        }
        const anchored = id ? model.slides.findIndex(item => item.id === id) : -1;
        const target = anchored >= 0 ? anchored : Math.min(index, Math.max(0, count - 1));
        return { index: target, id: model.slides[target]?.id || null };
    }
    function spacer(pixels) {
        // Native document-height is bounded per widget; a long continuous deck
        // uses several inert spacers, never thousands of live thumbnail widgets.
        while (pixels > 0) {
            const part = Math.min(100000, pixels);
            list.append(new Container({ text: `panel:document-height:${part}`, enabled: false, state: State.FLAT }));
            pixels -= part;
        }
    }
    function makeCard(item) {
        const card = { item };
        const choice = new Button({ name: `slide-${item.index}`, text: `panel:fixed-height:${height()}:flush`,
            accessibleName: `${item.index + 1}. ${item.name || 'Untitled'}${item.hidden ? ' (hidden)' : ''}`, tooltip: null,
            enabled: ownsPointer(), state: State.FLAT | State.EXPANSIVE });
        // Native Button releases are ACTIVATE without modifiers. Preserve only
        // its matching primary press; keyboard/blur cannot reuse stale modifiers.
        let pressModifiers = 0;
        choice.on(Message.POINTER, ({ detail }) => {
            if (detail.kind === 'down') {
                pressModifiers = detail.button === 1 ? detail.modifiers || 0 : 0;
                if (detail.button === 3 && available() && !selection.has(card.item.id)) {
                    selection.choose(card.item.id); selectionUpdated();
                }
            }
            else if (detail.kind === 'up') pressModifiers = 0;
        });
        choice.on(Message.BLUR, () => { pressModifiers = 0; });
        choice.on(Message.KEY, event => { pressModifiers = 0; selectionKey(event); });
        choice.on(Message.ACTIVATE, () => {
            resetDoubleClick();
            const modifiers = pressModifiers; pressModifiers = 0;
            if (!available()) return;
            const previous = selection.active;
            const additive = Boolean(modifiers & (4 | 64));
            const id = selection.choose(card.item.id, { control: additive, shift: Boolean(modifiers & 1) });
            selectionUpdated();
            if (id && (id !== previous || !additive)) {
                clickedId = id; void navigate(model.byId.get(id).index, id);
            }
        });
        const surface = new SceneSurface({ name: `slide-preview-${item.index}`, presentation: 'surface', state: State.EXPANSIVE, enabled: false, visible: false });
        const badge = new Label({ name: `slide-number-${item.index}`, text: slideNumber(item), enabled: false, selected: item.index === selectedIndex, state: State.CENTER_ALIGNED | State.EMPHASIZED });
        const badgeBox = new Container({ name: `slide-number-backing-${item.index}`, text: 'positioned-item:0:0:180:160', enabled: false, selected: item.index === selectedIndex }, [badge]);
        const tooltip = new Tooltip({ name: `slide-title-${item.index}`, text: '', visible: false });
        // PRESSED paints theme selection color without Stack's SELECTED-child filtering.
        const edges = Array.from({ length: 4 }, (_, edge) => new Container({
            name: `slide-border-${item.index}-${edge}`, enabled: false, state: State.FLAT }));
        const border = new Stack({ name: `slide-border-${item.index}`, text: 'panel:overlay:flush',
            enabled: false }, edges);
        card.updateBorder = () => {
            const selected = selection.has(card.item.id);
            for (const edge of edges) edge.setProperty('state', selected ? State.PRESSED : State.FLAT);
        };
        card.updateBorder();
        card.setTitle = title => {
            title = String(title || '').trim();
            choice.setProperty('tooltip', title || null);
            tooltip.text = title; tooltip.setProperty('visible', Boolean(title));
        };
        const body = new Stack({ text: 'panel:flush', state: State.EXPANSIVE }, [surface,
            new Stack({ text: 'panel:overlay:flush', enabled: false }, [badgeBox]), border]);

        const positionBadge = () => {
            const frame = card.frame;
            if (!bodySize?.width || !bodySize.height) return;
            // Thumbnail rows fit the page aspect directly; editor workspace
            // gutters are intentionally absent from these small previews.
            const bx = Math.ceil(3000 / bodySize.width), by = Math.ceil(3000 / bodySize.height);
            [[0, 0, 1000, by], [0, 1000 - by, 1000, by], [0, 0, bx, 1000], [1000 - bx, 0, bx, 1000]]
                .forEach((rect, index) => { edges[index].text = `positioned-item:${rect.join(':')}`; });
            if (!frame) return;
            const scale = Math.min(bodySize.width / frame.width, bodySize.height / frame.height);
            const pageSize = { width: Math.floor(frame.width * scale), height: Math.floor(frame.height * scale) };
            const x = Math.ceil(Math.max(0, (bodySize.width - pageSize.width) / 2 + 2) * 1000 / bodySize.width);
            const y = Math.ceil(Math.max(0, (bodySize.height - pageSize.height) / 2 + 2) * 1000 / bodySize.height);
            const width = Math.min(1000 - x, Math.ceil(Math.min(Math.max(18, String(card.item.index + 1).length * 8 + 8), pageSize.width) * 1000 / bodySize.width));
            const height = Math.min(1000 - y, Math.ceil(Math.min(18, pageSize.height) * 1000 / bodySize.height));
            badgeBox.text = `positioned-item:${x}:${y}:${width}:${height}`;
        };
        card.positionBadge = positionBadge;
        card.observe = () => body.on(Message.SIZE_CHANGED, event => {
            bodySize = event.detail;
            // A scrollbar changes content width without resizing this viewport.
            // Keep the narrowest native measurement for this layout epoch so
            // width -> height -> overflow cannot repeatedly add/remove it.
            // A real viewport resize, mode/count or aspect change starts afresh.
            measuredCardWidth = Math.min(measuredCardWidth ?? bodySize.width, bodySize.width);
            const nextHeight = Math.max(64, Math.min(400, Math.round(measuredCardWidth / pageRatio)));
            if (nextHeight !== cardHeight) {
                const rowOffset = scrollOffset / stride(); cardHeight = nextHeight;
                scrollOffset = rowOffset * stride();
                if (model?.slides.length > RETAINED_CARDS) mountedKey = '';
                mount();
                if (scrollOffset) viewport.setScrollPosition(`${Math.round(scrollOffset)}/0`);
                previewDirty = true; requestPreviews();
            }
            for (const candidate of cards) candidate.positionBadge();
        });
        choice.append(body); choice.append(tooltip);
        const container = new Container({ name: `slide-card-${item.index}`, selected: selection.has(item.id),
            text: `panel:fixed-height:${height()}:flush:gap=0`, state: State.EXPANSIVE }, [choice]);
        const contextTarget = { slide: item.id, index: item.index };
        Object.assign(card, { control: container, button: choice, surface, badge, badgeBox, contextTarget,
            context: slideContext(container, { target: contextTarget, invoke: mutate, available: () => available() && !container.destroyed,
                selection: () => selection.has(card.item.id) ? selection.selected : [card.item.id],
                hidden: id => model?.byId.get(id)?.hidden === true }) });
        const cached = cache.get(item.id);
        card.setTitle(cached?.title || (item.nameSpecified ? item.name : ''));
        if (cached) {
            card.frame = cached.frame;
            const first = Math.floor(scrollOffset / stride()) * columns();
            // A restored card window can exceed the host64-response cap if
            // every cached surface is emitted with its Create batch. Restore
            // only the viewport; offscreen cache hits remain cheap to reveal.
            if (item.index >= first && item.index < first + visibleCount()) {
                surface.submit(cached.frame); surface.setProperty('visible', true);
            }
        }
        positionBadge();
        return card;
    }
    function mount() {
        if (!model) return;
        const perRow = columns(), totalRows = Math.ceil(model.slides.length / perRow);
        const visibleRow = Math.floor(scrollOffset / stride());
        const visibleRows = Math.ceil(viewportHeight / stride());
        const keepWindow = mountedEnd > mountedStart && visibleRow >= mountedStart &&
            Math.min(totalRows, visibleRow + visibleRows + 1) <= mountedEnd;
        const firstRow = model.slides.length <= RETAINED_CARDS ? 0 : keepWindow ? mountedStart
            : Math.min(Math.max(0, visibleRow - 4), Math.max(0, totalRows - RETAINED_CARDS / perRow));
        const lastRow = Math.min(totalRows, firstRow + RETAINED_CARDS / perRow);
        const items = model.slides.slice(firstRow * perRow, lastRow * perRow);
        const key = `${overview}:${items.map(item => item.id).join('|')}`;
        if (key !== mountedKey) {
            mountedKey = key; mountedStart = firstRow; mountedEnd = lastRow;
            for (const child of [...list.children]) list.remove(child, { destroy: true });
            cards.length = 0;
            spacer(firstRow * stride());
            for (let rowIndex = firstRow; rowIndex < lastRow; rowIndex++) {
                const row = new Row({ text: `panel:fixed-height:${stride()}:equal-fixed:gap=8:flush` });
                list.append(row);
                for (const item of model.slides.slice(rowIndex * perRow, (rowIndex + 1) * perRow)) {
                    const card = makeCard(item); row.append(card.control); cards.push(card);
                }
                // An incomplete overview row keeps the same card widths.
                if (overview) while (row.children.length < 3) row.append(new Container({ state: State.FLAT | State.EXPANSIVE, enabled: false, text: 'panel:flush' }));
            }
            cards[0]?.observe();
            spacer((totalRows - lastRow) * stride());
        }
        for (const row of list.children) if (row instanceof Row) row.text = `panel:fixed-height:${stride()}:equal-fixed:gap=8:flush`;
        for (const card of cards) {
            card.control.text = `panel:fixed-height:${height()}:flush:gap=0`;
            card.button.text = `panel:fixed-height:${height()}:flush`;
            const item = model.byId.get(card.item.id);
            card.button.setProperty('accessibleName', `${item.index + 1}. ${item.name || 'Untitled'}${item.hidden ? ' (hidden)' : ''}`);
            card.setTitle(cache.get(item.id)?.revision === model.revision ? cache.get(item.id).title : (item.nameSpecified ? item.name : ''));
            card.item = item; card.updateBorder(); Object.assign(card.contextTarget, { slide: item.id, index: item.index });
            card.badge.text = slideNumber(item);
            card.badge.setProperty('selected', item.index === selectedIndex);
            card.control.setProperty('selected', selection.has(item.id));
            card.badgeBox.setProperty('selected', item.index === selectedIndex);
            card.positionBadge();
            card.button.setProperty('enabled', ownsPointer()); card.context.update();
        }
    }
    function show(index) {
        resetDoubleClick();
        selectedIndex = index;
        const nextId = model?.slides[index]?.id || null;
        selection.activate(nextId); selectionUpdated();
        const changed = nextId !== shownId;
        const row = Math.floor(index / columns());
        const outside = row * stride() < scrollOffset || (row + 1) * stride() > scrollOffset + viewportHeight;
        const reveal = changed && outside && clickedId !== nextId;
        clickedId = null;
        if (reveal) scrollOffset = row * stride();
        shownId = nextId;
        const focused = cards.some(card => card.button.getProperty('focused'));
        mount();
        const active = cards.find(card => card.item.id === nextId);
        if (focused && active) {
            for (const card of cards) card.button.setProperty('focused', card === active);
        }
        if (reveal && active?.control.id) viewport.scrollTo(active.control);
    }

    async function drawPreviews(request, background = false) {
        const { revision, currentScene, currentFrames, generation: epoch } = request;
        const first = Math.floor(scrollOffset / stride()) * columns();
        // Current slide first, then a bounded viewport/overscan batch. The native
        // scroll event schedules the next batch; distant slides need no raster or
        // text layout work merely because the selection changed.
        const visible = cards.filter(card => card.item.index >= first && card.item.index < first + visibleCount());
        const current = cards.find(card => card.item.id === currentScene?.slideId);
        const queue = [...new Set([...(current ? [current] : []), ...visible])];
        const cancelled = () => closed || epoch !== generation || revision !== model?.revision || (background && locked);
        for (const card of queue) {
            if (cancelled()) return;
            const cached = cache.get(card.item.id);
            if (cached?.revision === revision) {
                if (!card.control.destroyed && !card.surface.getProperty('visible')) {
                    card.frame = cached.frame; card.surface.submit(cached.frame);
                    card.surface.setProperty('visible', true); card.positionBadge();
                }
                continue;
            }
            try {
                let scene = currentScene?.slideId === card.item.id ? currentScene : null, frames = currentFrames;
                if (!scene) {
                    scene = await loadDeckScene(port, { index: card.item.index, revision, maximumObjects: 512, cancelled });
                    if (cancelled()) return;
                    if (scene.slideId !== card.item.id) throw Error('Preview identity changed');
                    frames = (await port.renderText?.(scene))?.frames || new Map();
                }
                if (cancelled()) return;
                const pictures = (await port.renderImages?.(scene, { thumbnail: true }))?.frames || new Map();
                if (cancelled()) return;
                const frame = thumbnailAppearance(geometryFrame(scene, { textFrames: frames, imageFrames: pictures }).frame, card.item.hidden === true);
                const title = scene.objects.find(object => object.placeholder?.role === 'title' && object.text?.text?.trim())?.text.text.trim()
                    || (card.item.nameSpecified ? card.item.name.trim() : '');
                card.setTitle(title);
                cacheBytes -= cache.get(card.item.id)?.frame.displayList.byteLength || 0;
                cache.delete(card.item.id); cache.set(card.item.id, { revision, frame, title }); cacheBytes += frame.displayList.byteLength;
                // Bound retained vector/image bytes as well as widget count.
                while (cache.size > RETAINED_CARDS || (cache.size > 1 && cacheBytes > 7 * 1024 * 1024)) {
                    const id = cache.keys().next().value;
                    cacheBytes -= cache.get(id).frame.displayList.byteLength; cache.delete(id);
                    const evicted = cards.find(candidate => candidate.item.id === id);
                    if (evicted && !evicted.control.destroyed) {
                        evicted.surface.setProperty('visible', false);
                        evicted.surface.submit({ width: 1, height: 1, displayList: new Uint8Array() });
                    }
                }
                if (!card.control.destroyed) { card.frame = frame; card.surface.submit(frame); card.surface.setProperty('visible', true); card.positionBadge(); }
            } catch {
                if (cancelled()) return;
                // Keep only an actual title; a missing preview is not a slide title.
            }
        }
    }
    function requestPreviews() {
        if (closed || locked || !previewDirty || !previewRequest || rendering) return;
        previewDirty = false;
        // Set ownership before the callback can synchronously lock the shell.
        rendering = Promise.resolve().then(() => previewWork(() => drawPreviews(previewRequest, true)))
            .catch(() => {}).finally(() => { rendering = null; requestPreviews(); });
    }
    async function previews(revision, currentScene, currentFrames) {
        if (currentScene?.size?.width > 0 && currentScene.size.height > 0) {
            const ratio = currentScene.size.width / currentScene.size.height;
            if (ratio !== pageRatio) { pageRatio = ratio; measuredCardWidth = null; }
        }
        previewRequest = { revision, currentScene, currentFrames, generation };
        await rendering;
        previewDirty = false;
        await drawPreviews(previewRequest);
    }
    async function command(type, slide, index, count, revision, target = null) {
        if (type === 'delete-slide' || type === 'delete-selected-slides' || type === 'set-slides-hidden') {
            const ids = target?.slides ?? (slide && !selection.has(slide) ? [slide] : selection.selected);
            if (!Array.isArray(ids) || !ids.length || ids.length > 4096 || new Set(ids).size !== ids.length || ids.some(id => !model?.byId.has(id))) throw Error('Select existing slides first');
            if (type === 'set-slides-hidden') {
                if (typeof target?.hidden !== 'boolean') throw Error('Choose whether to hide slides');
                return { type, slides: [...ids], hidden: target.hidden };
            }
            return { type: 'delete-selected-slides', slides: [...ids] };
        }
        if (type === 'insert-slide') return { type, after: slide || null };
        if (!slide) throw Error('Select a slide first');
        if (type === 'duplicate-slide' || type === 'delete-slide') return { type, slide };
        const beforeIndex = type === 'move-earlier' ? index - 1 : index + 2;
        if (beforeIndex < 0 || index >= count || (type === 'move-later' && index + 1 >= count)) throw Error('Slide is already at this edge');
        const before = beforeIndex >= count ? null : (await outline({ startSlide: beforeIndex, limit: 1 }, revision)).slides[0]?.id;
        if (before === undefined) throw Error('Slide order changed');
        return { type: 'move-slide', slide, before };
    }
    // Only callers applying commands known to leave slide identities and
    // audience artwork unchanged may carry painted previews to a new revision.
    function retainArtwork(previous, next) {
        if (!model || model.revision !== previous || !/^(0|[1-9]\d*)$/u.test(next) || BigInt(next) < BigInt(previous)) return false;
        generation++;
        model.revision = next;
        for (const entry of cache.values()) if (entry.revision === previous) entry.revision = next;
        if (previewRequest?.revision === previous) previewRequest = { ...previewRequest, revision: next, generation };
        return true;
    }
    return Object.freeze({ control, toolbar, setState, locate, show, previews, command, retainArtwork,
        get hyperlinkSlides() { return (model?.slides || []).map((slide, index) => ({ id: slide.id, label: `${index + 1}. ${slide.name || 'Slide'}` })); },
        get selectedSlides() { return selection.selected; },
        get hiddenSlideIndices() { return model?.slides.filter(item => item.hidden === true).map(item => item.index) ?? []; },
        reset() { generation++; selection.reset(); selectionUpdated(); measuredCardWidth = null; model = null; shownId = null; mountedKey = ''; mountedStart = mountedEnd = 0; cache.clear(); cacheBytes = 0; previewRequest = null; scrollOffset = 0; viewport.setScrollPosition('0/0'); },
        setOverview(value) { measuredCardWidth = null; overview = Boolean(value); cardHeight = overview ? 180 : 100; shownId = null; mountedStart = mountedEnd = 0; bodySize = null; scrollOffset = 0; viewport.setScrollPosition('0/0'); },
        close() { closed = true; generation++; },
    });
}
