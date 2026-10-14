import { Button, Column, Container, Dialog, Label, Menu, MenuItem, Message, Row, SceneBuilder, SceneSurface, ScrollView, State, TextField, ToolBar } from '@ricewm/app';

const starts = [['click', 'On Click'], ['with-previous', 'With Previous'], ['after-previous', 'After Previous']];
const copy = value => structuredClone(value);
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const milliseconds = text => /^(?:\d+(?:\.\d{1,3})?|\.\d{1,3})$/u.test(text.trim()) && Number(text) <= 600 ? Math.round(Number(text) * 1000) : null;
const targetId = value => typeof value === 'string' ? value : value?.id;
const scope = node => node?.authoringScope ?? node?.parentId;

/** Passive native drawings explain the qualified appear/fade choices without
 * introducing an independent playback model. Preview uses the source timeline. */
function effectPreview(effect) {
    if (!['entrance', 'exit'].includes(effect.category) || !['appear', 'fade'].includes(effect.kind)) return null;
    const builder = new SceneBuilder(112, 44), fade = effect.kind === 'fade', exit = effect.category === 'exit';
    const points = Array.from({ length: 10 }, (_, index) => {
        const angle = -Math.PI / 2 + index * Math.PI / 5, radius = index % 2 ? 7 : 16;
        return [index ? 'L' : 'M', Math.cos(angle) * radius, Math.sin(angle) * radius];
    });
    builder.definePath(1, [...points, ['Z']]);
    const opacities = fade ? (exit ? [1, .5, .15] : [.15, .5, 1]) : [1];
    for (const [index, opacity] of opacities.entries()) {
        builder.pushTransform([1, 0, 0, 1, fade ? 22 + index * 34 : 56, 22], opacity)
            .fillPath(1, exit ? 0xff647bed : 0xff66bb66).popTransform();
    }
    return builder.finish();
}

/** Catalog entries are supplied by the qualified document adapter, never inferred
 * from a format name: {id,label,effect,defaultDurationMs,previewFrame?}.
 * timeline is the complete validated animation-timeline query, not one page.
 * All writes are a captured one-command batch; the owner refreshes setState.
 */
export function createAnimationTools({ root, apply, preview, selectObject = () => {}, onPaneVisibility = () => {}, notice = () => {}, catalog = [] }) {
    function normalizeCatalog(catalog) {
      if (!Array.isArray(catalog) || catalog.length > 128) throw new TypeError('Animation catalog must be a bounded array');
      const entries = catalog.map(entry => {
        if (typeof entry.id !== 'string' || !entry.id || typeof entry.label !== 'string' || !entry.label || !entry.effect ||
            !Number.isInteger(entry.defaultDurationMs) || entry.defaultDurationMs < 0 || entry.defaultDurationMs > 600000) throw new TypeError('Invalid animation catalog entry');
        return { ...entry, effect: copy(entry.effect) };
    });
      if (new Set(entries.map(entry => entry.id)).size !== entries.length) throw new TypeError('Duplicate animation catalog entry');
      return entries;
    }
    let entries = normalizeCatalog(catalog);
    let scene = null, timeline = null, selected = [], selectedEffect = null, ready = false, pending = false, previewing = false;
    let paneOpen = false, dialog = null, draftSource = null, generation = 0, nextRow = 0;
    const rowControls = new Map(), choices = [];
    const timelineNodes = () => timeline && scene && timeline.slide === scene.slideId ? (timeline.nodes || []) : [];
    const effects = () => timelineNodes().filter(node => node.kind === 'effect');
    const currentEffect = () => effects().find(node => node.id === selectedEffect) || null;
    const available = () => ready && !pending && !previewing && timeline?.slide === scene?.slideId && scene?.animationTimeline?.version === 1 &&
        timeline.nextNode === null && timeline.totalNodes === timeline.nodes?.length;
    const writable = () => available() && scene.animationTimeline.editable === true && typeof apply === 'function';
    const editable = () => writable() && currentEffect()?.editable === true;
    const canAdd = () => writable() && selected.length === 1 && !!targetId(selected[0]) && entries.length > 0;
    const effectLabel = node => entries.find(entry => same(entry.effect, node.effect))?.label || node.effect?.kind || 'Unsupported effect';
    function targetLabel(node) {
        const objects = scene?.objects || [], index = objects.findIndex(object => object.id === node.target?.object);
        const object = objects[index];
        const name = object?.name || object?.morphSource?.name || object?.text?.text?.trim().split('\n')[0];
        if (name) return name.slice(0, 80);
        const kind = object?.geometry || object?.kind || 'Object';
        return `${kind[0].toUpperCase()}${kind.slice(1).replaceAll('-', ' ')}${index >= 0 ? ` ${index + 1}` : ''}`;
    }

    const gallery = new Menu({ name: 'animation-effects', text: 'Effects', icon: 133, state: State.START_ALIGNED, tooltip: 'Replace the selected animation effect', enabled: false });
    const add = new Menu({ name: 'animation-add', text: 'Add Animation', icon: 76, state: State.START_ALIGNED, enabled: false });
    const play = typeof preview === 'function' ? new Button({ name: 'animation-preview', text: 'Preview', icon: 82, state: State.START_ALIGNED, enabled: false }) : null;
    const toggle = new Button({ name: 'animation-pane-toggle', text: 'Animation Pane', icon: 40, state: State.START_ALIGNED, enabled: false });
    const controls = [gallery, add, ...(play ? [play] : []), toggle];
    const heading = new Label({ name: 'animation-pane-heading', text: 'Animations' });
    const closePane = new Button({ name: 'animation-pane-close', icon: 54, tooltip: 'Close Animation Pane', accessibleName: 'Close Animation Pane' });
    const rows = new Column({ name: 'animation-rows', text: 'panel:fixed:gap=2:flush' });
    const scroll = new ScrollView({ name: 'animation-list', state: State.EXPANSIVE }, [rows]);
    const status = new Label({ name: 'animation-pane-status', text: '' });
    const timing = new Button({ name: 'animation-timing', text: 'Timing', state: State.START_ALIGNED, tooltip: 'Animation timing', accessibleName: 'Animation timing' });
    const remove = new Button({ name: 'animation-remove', icon: 77, tooltip: 'Remove selected animation', accessibleName: 'Remove selected animation' });
    const earlier = new Button({ name: 'animation-earlier', icon: 73, tooltip: 'Move animation earlier', accessibleName: 'Move animation earlier' });
    const later = new Button({ name: 'animation-later', icon: 74, tooltip: 'Move animation later', accessibleName: 'Move animation later' });
    const pane = new Column({ name: 'animation-pane', visible: false, state: State.EXPANSIVE }, [
        new Row({}, [heading, closePane]), scroll, status,
        new ToolBar({ text: 'panel:fixed:flush:gap=4:accessory-height=28', name: 'animation-pane-actions' }, [timing, earlier, later, remove]),
    ]);
    function dismissDialog() {
        if (dialog) root.remove(dialog, { destroy: true });
        dialog = null; draftSource = null;
    }
    function visibility(value) {
        if (!value) generation++;
        paneOpen = value; pane.setProperty('visible', value); toggle.setProperty('checked', value);
        if (!value) dismissDialog();
        onPaneVisibility(value);
    }
    function choose(id, selectTarget = true) {
        if (selectedEffect !== id) dismissDialog();
        selectedEffect = id;
        const node = currentEffect();
        update();
        if (node && selectTarget) selectObject(node.target.object);
    }
    function updateRows() {
        const nodes = effects(), ids = new Set(nodes.map(node => node.id));
        for (const [id, control] of rowControls) if (!ids.has(id)) { rows.remove(control, { destroy: true }); rowControls.delete(id); }
        for (const [index, node] of nodes.entries()) {
            let control = rowControls.get(node.id);
            if (!control) {
                control = new Button({ name: `animation-row-${nextRow++}`, icon: 133, state: State.START_ALIGNED });
                control.on(Message.ACTIVATE, () => { if (available()) choose(node.id); });
                rowControls.set(node.id, control);
            }
            const start = starts.find(([id]) => id === node.start)?.[1] || 'Source timing';
            control.text = `${index + 1}. ${effectLabel(node)} · ${targetLabel(node)}`;
            const details = `${start} · ${(node.durationMs ?? 0) / 1000} s · Delay ${(node.delayMs ?? 0) / 1000} s${node.editable ? '' : ' · Read only'}${node.unsupported?.length ? `: ${node.unsupported.join('; ')}` : ''}`;
            control.setProperty('tooltip', details);
            control.setProperty('accessibleName', `${control.text} · ${details}`);
            control.setProperty('selected', node.id === selectedEffect);
            control.setProperty('enabled', available());
        }
        // Preserve every control and scroll state for ordinary source refreshes.
        // Reparent only when the document actually changes effect order.
        const ordered = nodes.map(node => rowControls.get(node.id));
        if (ordered.some((control, index) => rows.children[index] !== control)) {
            for (const control of [...rows.children]) rows.remove(control);
            for (const control of ordered) rows.append(control);
        }
    }
    function update() {
        const node = currentEffect(), all = effects(), index = all.findIndex(value => value.id === selectedEffect);
        gallery.setProperty('enabled', editable() && entries.length > 0);
        gallery.text = node ? effectLabel(node) : 'Effects';
        add.setProperty('enabled', canAdd()); toggle.setProperty('enabled', !!scene);
        timing.setProperty('enabled', editable()); remove.setProperty('enabled', editable());
        earlier.setProperty('enabled', editable() && index > 0 && all[index - 1].editable === true && scope(all[index - 1]) === scope(node));
        later.setProperty('enabled', editable() && index >= 0 && index + 1 < all.length && all[index + 1].editable === true && scope(all[index + 1]) === scope(node));
        if (play) { play.text = previewing ? 'Stop' : 'Preview'; play.setProperty('enabled', previewing || available() && scene.animationTimeline.playable === true && all.length > 0); }
        for (const choice of choices) {
            choice.control.setProperty('enabled', choice.add ? canAdd() : editable());
            choice.control.setProperty('checked', !choice.add && same(choice.entry.effect, node?.effect));
        }
        const unsupportedNodes = timelineNodes().filter(value => value.kind === 'unsupported');
        const reasons = [...new Set([...(scene?.animationTimeline?.unsupported || []), ...unsupportedNodes.flatMap(value => value.unsupported || [])])];
        const unsupportedCount = unsupportedNodes.filter(value => value.role === 'effect').length;
        const unsupportedText = unsupportedCount ? `${unsupportedCount} unsupported imported animation${unsupportedCount === 1 ? ' is' : 's are'} read only.` : 'Imported animation timing is read only.';
        status.setProperty('tooltip', reasons.length ? reasons.slice(0, 8).join('; ') : null);
        status.text = !timeline || timeline.slide !== scene?.slideId ? 'Loading animations…' : unsupportedNodes.length || reasons.length ? unsupportedText :
            node?.editable === false ? `Read only: ${node.unsupported?.join('; ') || 'Unsupported source timing'}` :
                all.length ? node ? `${starts.find(([id]) => id === node.start)?.[1] || 'Source timing'} · ${(node.durationMs ?? 0) / 1000} s` : 'Select an effect to edit it.' : 'Select an object, then Add Animation.';
        updateRows();
    }
    async function run(command, after = () => {}) {
        if (!writable()) return;
        const sourceGeneration = generation;
        pending = true; dismissDialog(); update();
        try {
            const result = await apply([copy(command)]);
            if (sourceGeneration === generation && scene?.slideId === command.slide) after(result);
        } catch (error) { notice(error?.message || String(error)); }
        finally { pending = false; update(); }
    }
    function populateCatalog() {
      choices.length = 0;
      for (const [isAdd, menu] of [[false, gallery], [true, add]]) {
        for (const child of [...menu.children]) menu.remove(child, { destroy: true });
        const content = new Column({ text: 'panel:fixed:gap=4:flush' });
        menu.append(new ScrollView({ name: isAdd ? 'animation-add-gallery' : 'animation-effect-gallery', text: `viewport:${Math.min(360, Math.ceil(entries.length / 2) * 88 + 8)}|0/0`, state: State.FLAT }, [content]));
        for (let index = 0; index < entries.length; index += 2) {
            const row = new Row({ text: 'panel:fixed:gap=4:flush' });
            for (const [offset, entry] of entries.slice(index, index + 2).entries()) {
                const children = [];
                const previewFrame = entry.previewFrame || effectPreview(entry.effect);
                if (previewFrame) { const sample = new SceneSurface({ enabled: false }); sample.submit(previewFrame); children.push(new Container({ text: 'panel:fixed-height:48:flush', state: State.FLAT }, [sample])); }
                children.push(new Label({ text: entry.label }));
                const control = new MenuItem({ name: `animation-${isAdd ? 'add' : 'effect'}-${index + offset}`, text: `panel:fixed-height:${previewFrame ? 84 : 40}:width=168:gap=2:flush`, accessibleName: entry.label, tooltip: entry.label }, children);
                control.on(Message.ACTIVATE, () => {
                    if (isAdd ? !canAdd() : !editable()) return;
                    const slide = scene.slideId;
                    if (!isAdd) {
                        const node = currentEffect();
                        if (!same(node.effect, entry.effect)) void run({ type: 'set-object-animation', slide, animation: node.id, patch: { effect: entry.effect } });
                    } else {
                        const object = targetId(selected[0]), previousIds = new Set(effects().map(node => node.id));
                        void run({ type: 'add-object-animation', slide, object, effect: entry.effect, start: 'click', durationMs: entry.defaultDurationMs, delayMs: 0, after: null }, result => {
                            const created = (result?.result ?? result)?.createdAnimations;
                            const id = created?.find(value => value.slide === slide && value.object === object)?.animation ||
                                effects().find(node => !previousIds.has(node.id) && node.target?.object === object)?.id;
                            if (id) selectedEffect = id;
                            visibility(true);
                        });
                    }
                });
                choices.push({ control, entry, add: isAdd }); row.append(control);
            }
            content.append(row);
        }
      }
    }
    populateCatalog();
    remove.on(Message.ACTIVATE, () => { if (editable()) void run({ type: 'remove-object-animation', slide: scene.slideId, animation: selectedEffect }); });
    function reorder(delta) {
        if (!(delta < 0 ? earlier : later).getProperty('enabled')) return;
        const all = effects(), index = all.findIndex(node => node.id === selectedEffect);
        const next = all[index + 2];
        const before = delta < 0 ? all[index - 1].id : scope(next) === scope(all[index]) ? next.id : null;
        void run({ type: 'reorder-object-animation', slide: scene.slideId, animation: selectedEffect, before });
    }
    earlier.on(Message.ACTIVATE, () => reorder(-1)); later.on(Message.ACTIVATE, () => reorder(1));
    toggle.on(Message.ACTIVATE, () => { if (scene) visibility(!paneOpen); }); closePane.on(Message.ACTIVATE, () => visibility(false));
    play?.on(Message.ACTIVATE, () => {
        if (!play.getProperty('enabled')) return;
        const intent = { slide: scene.slideId, stop: previewing };
        Promise.resolve().then(() => preview(intent)).catch(error => notice(error.message));
    });
    timing.on(Message.ACTIVATE, () => {
        if (!editable() || dialog) return;
        const node = currentEffect(); draftSource = copy(node);
        let start = node.start;
        const mode = new Menu({ name: 'animation-start', text: starts.find(([id]) => id === start)?.[1] || 'Unsupported', state: State.START_ALIGNED });
        for (const [id, label] of starts) {
            const option = mode.append(new MenuItem({ name: `animation-start-${id}`, text: label, checked: id === start }));
            option.on(Message.ACTIVATE, () => { start = id; mode.text = label; for (const child of mode.children) child.setProperty('checked', child === option); });
        }
        const duration = new TextField({ name: 'animation-duration', value: String(node.durationMs / 1000) });
        const delay = new TextField({ name: 'animation-delay', value: String(node.delayMs / 1000) });
        const error = new Label({ name: 'animation-timing-error', text: '' });
        const save = new Button({ name: 'animation-timing-apply', text: 'Apply', accent: true });
        const cancel = new Button({ name: 'animation-timing-cancel', text: 'Cancel' }); cancel.on(Message.ACTIVATE, dismissDialog);
        save.on(Message.ACTIVATE, () => {
            if (!editable() || !draftSource || !same(currentEffect(), draftSource)) { dismissDialog(); return; }
            const durationMs = milliseconds(duration.getProperty('value')), delayMs = milliseconds(delay.getProperty('value'));
            if (durationMs === null || delayMs === null) { error.text = 'Enter seconds from 0 to 600, with at most three decimal places.'; return; }
            const patch = {};
            for (const [key, value] of Object.entries({ start, durationMs, delayMs })) if (value !== draftSource[key]) patch[key] = value;
            if (!Object.keys(patch).length) { dismissDialog(); return; }
            void run({ type: 'set-object-animation', slide: scene.slideId, animation: selectedEffect, patch });
        });
        dialog = new Dialog({ name: 'animation-timing-dialog', text: 'Animation timing', modal: true }, [
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Start' }), mode]),
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Duration (s)' }), duration]),
            new Row({ text: 'panel:form-control' }, [new Label({ text: 'Delay (s)' }), delay]), error,
            new Row({ text: 'panel:actions' }, [cancel, save]),
        ]);
        dialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) dismissDialog(); });
        root.append(dialog);
    });
    update();
    return Object.freeze({ controls, pane, setCatalog(catalog) {
        const next = normalizeCatalog(catalog);
        if (same(next, entries)) return;
        entries = next; generation++; dismissDialog(); populateCatalog(); update();
    }, close() { dismissDialog(); visibility(false); }, setState({ current = scene, timeline: nextTimeline = timeline, selected: nextSelected = selected, ready: nextReady = ready, previewing: nextPreview = previewing } = {}) {
        const oldSlide = scene?.slideId, oldSelection = selected.map(targetId).join('\0');
        scene = current; timeline = nextTimeline; selected = nextSelected || []; ready = nextReady; previewing = nextPreview;
        if (scene?.slideId !== oldSlide) { generation++; selectedEffect = null; dismissDialog(); }
        if (!effects().some(node => node.id === selectedEffect)) selectedEffect = null;
        const selectedIds = selected.map(targetId);
        if (selectedIds.join('\0') !== oldSelection) generation++;
        if (selectedIds.join('\0') !== oldSelection && !selectedIds.includes(currentEffect()?.target?.object)) selectedEffect = null;
        if (!selectedEffect && selectedIds.length === 1) {
            const matches = effects().filter(node => node.target?.object === selectedIds[0]);
            if (matches.length === 1) selectedEffect = matches[0].id;
        }
        if (dialog && (!editable() || !same(currentEffect(), draftSource))) dismissDialog();
        update();
    } });
}
