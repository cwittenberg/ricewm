import {composeDeck} from './copilot-compose.mjs';
import {exposeAppCopilot} from '@ricewm/app';
import {deckCopilotActions,validateDeckCopilot} from './copilot-contract.mjs';
import { createSensitivityDialog, sensitivityCaption } from '@ricewm/app';
import { createHyperlinkTools } from './hyperlink-tools.mjs';
import { supportedLink, webLink } from './hyperlink-contract.mjs';
import { cellKey } from './table-values.mjs';
import {
    Button, Column, Dialog, DocumentViewport, Label, Menu, MenuBar, MenuItem, Message, Row,
    ScrollView, SplitPane, StatusBar, TextEditor, SceneBuilder, SceneSurface, State, ToolBar, ZoomControl,
} from "@ricewm/app";
import { createAboutDialog } from './about.mjs';
import { createCanvasEditor } from './canvas-editor.mjs';
import { createAuthoringTools } from './authoring-tools.mjs';
import { createSlideNavigation } from './slide-navigation.mjs';
import { loadDeckScene, renderDeckScene } from './scene-loader.mjs';
import { geometryFrame } from './scene-preview.mjs';
import { createTransitionTools } from './transition-tools.mjs';
import { createTransitionPreview } from './transition-preview.mjs';
import { createAnimationTools } from './animation-tools.mjs';
import { createAnimationPreview } from './animation-preview.mjs';
import { loadAnimationTimeline } from './animation-loader.mjs';
import { createPresentationView } from './presentation-view.mjs';
import { createSelectionProperties } from './selection-properties.mjs';
import { createDesignTools } from './design-tools.mjs';
import { createDeckContextMenu } from './context-menu.mjs';
import { createLayoutTools } from './layout-tools.mjs';
import { createSpeakerNotes } from './speaker-notes.mjs';
import { createInlineTableEditor } from './inline-table-editor.mjs';

const PAGE = 32;
const fitsEditor = text => new TextEncoder().encode(text).length <= 16_000;
const ACTION_ICONS = Object.freeze({
    new: 10, open: 11, 'open-template': 104, save: 12, 'save-as': 67, exit: 54,
    undo: 20, redo: 21, 'show-start': 82, 'show-current': 104,
});

/** Retained RiceUI shell. The injected document port owns files and engine state. */
export function startDeckShell({ app, createWindow, log, services, bus, instanceId }, documentPort = null) {
    const port = documentPort;
    if (port && (port.version !== 1 || ["project", "apply", "undo", "redo", "close"].some(key => typeof port[key] !== "function"))) {
        throw new TypeError("Rice Deck requires document port version 1");
    }
    let aboutDialog = null;
    const closeAbout = () => {
        if (aboutDialog) root.remove(aboutDialog, { destroy: true });
        aboutDialog = null;
    };
    const about = new MenuItem({ name: 'about', text: 'About Rice Deck', icon: 104 });
    about.on(Message.ACTIVATE, () => {
        if (!aboutDialog) { aboutDialog = createAboutDialog(closeAbout); root.append(aboutDialog); }
    });
    const sensitivityBadge = new Button({name:'sensitivity-badge',icon:119,state:State.FLAT,visible:false});
    const sensitivityItem = new MenuItem({name:'sensitivity',text:'Sensitivity…',icon:119,visible:false});
    sensitivityItem.on(Message.ACTIVATE, () => {
        if (busy || uncertain || draft !== null || aboutDialog || !projection?.sensitivity?.length) return;
        const expected = revision;
        aboutDialog = createSensitivityDialog({labels:projection.sensitivity,onClose:closeAbout,onApply:async change=>{
            if (revision !== expected) throw Error('Presentation changed. Reopen Sensitivity.');
            let applied = false;
            await perform(async()=>{await port.apply([{type:'change-sensitivity',change}]);await read();applied=true;});
            if (!applied) throw Error(status.text || 'Sensitivity could not be changed');
        }});
        root.append(aboutDialog);
    });
    sensitivityBadge.on(Message.ACTIVATE,()=>sensitivityItem.sendMessage(Message.ACTIVATE));
    let closed = false, busy = false, uncertain = false;
    let projection = null, revision = null, slideIndex = 0, startText = 0;
    let selected = null, draft = null, pendingGrant = null, discardDialog = null, resolveDiscard = null;
    let textObjects = [], refreshingScene = false, selectedSlideId = null, documentName = null;
    let renderedScene = null, renderedFrames = new Map(), renderedPictures = new Map();
    let transitionPreview = null, animationPreview = null, animations = null, renderedTimeline = null;
    let initialBlankRevision = null;
    let textCommitTimer = null, textCommit = null, committingText = false;
    let clipboardTextTarget = null, clipboardTextBusy = false;
    let closeDecision = null, closeDialog = null, finishCloseDecision = null, closeApproved = false;
    const idleWaiters = new Set();
    let thumbnailWork = null, notes = null, overview = false;
    const controls = new Map();
    const shortcutControls = new WeakSet();
    const status = new Label({ name: "status-text", state: State.EXPANSIVE, text: port ? "Reading presentation…" : "Presentation engine unavailable." });
    status.on(Message.CHANGE, event => { if (event.detail?.property === 'text') status.setProperty('visible', Boolean(status.text)); });
    const heading = new Label({ name: "slide-heading", text: "No presentation open" });
    const runLabel = new Label({ name: "run-heading", text: "Select a text box" });
    const runs = new Column({ name: "text-object-list" });
    const editor = new TextEditor({ name: "run-editor", value: "", enabled: false, readOnly: true, rows: 4 });
    const root = new Column({ name: "deck-root", text: 'panel:flush' });
    const authoring = new Column({ name: 'authoring-view', text: 'panel:flush:gap=0', state: State.EXPANSIVE });
    const surface = new SceneSurface({ name: 'slide-surface', presentation: 'page', visible: false, state: State.EXPANSIVE });
    const viewport = new DocumentViewport({ name: 'document-workspace', surface });
    const zoom = new ZoomControl({ name: 'slide-zoom', viewport, compact: true });
    const sceneNotice = new Label({ name: 'scene-notice', visible: false });
    const geometryVisible = Boolean(port?.scene);
    notes = createSpeakerNotes({
        save: command => perform(async () => {
            const previous = revision;
            await port.apply([command]);
            // Notes have no audience artwork. Refresh document status without
            // replacing canvas or thumbnail frames on each typing pause.
            const response = await port.project({ slide: slideIndex, startRun: 0, limit: PAGE });
            present(response);
            slides.retainArtwork(previous, response.revision);
            return true;
        }, false),
        changed: () => { updateEnabled(); updateTitle(); },
        notice: text => { status.text = text; },
    });
    let contextMenu = null, hyperlinks = null;
    async function openHyperlink(link, audience = false) {
        if (closed || !supportedLink(link)) return;
        try {
            if (link.target.kind === 'slide') {
                if (!audience) await navigate(link.slideIndex, 0, link.target.value);
            } else {
                if (!webLink(link.target.value) || typeof services?.host?.openExternal !== 'function') throw Error('Web links cannot be opened by this host.');
                await services.host.openExternal({ url: link.target.value });
            }
        } catch (error) { status.text = `Could not open link: ${String(error.message).slice(0,200)}`; if (audience) throw error; }
    }
    let inline = null;
    let tools = null, properties = null, sceneReady = false, presenter = null, mainWindow = null;
    function cellTarget() {
        const active = canvas.selectedTableCell;
        return active ? { id: cellKey(active.object.id, active.row, active.column), slideId: renderedScene.slideId,
            tableObject: active.object.id, row: active.row, column: active.column, index: 0,
            text: active.cell.text?.text || '', editable: active.cell.editable } : null;
    }
    const canvas = createCanvasEditor({ surface, openHyperlink,
        apply: commands => perform(async () => {
            const reply = await port.apply(commands); canvas.created(reply); await read(); return reply;
        }),
        beforePointer: () => { animationPreview?.cancel(); transitionPreview?.cancel(); return inline?.active ? finishCellEditing() : null; },
        renderPreview: port?.renderText ? scene => port.renderText(scene) : null,
        beginTableInput: ({bounds,size,columns,rows}) => inline.beginPending(bounds,size,columns,rows),
        tableInputKey: key => inline.input([key]),
        changed: objects => {
            if (!refreshingScene) animationPreview?.cancel();
            if (!inline?.active) clipboardTextTarget = null;
            tools?.setState({ selected: objects });
            if (!refreshingScene && draft === null) {
                const target = cellTarget() || (objects.length === 1 ? textObjects.find(item => item.id === objects[0].id) : null);
                if (target?.id !== selected?.id || target?.text !== selected?.text) showSelected(target || null);
            }
            updateEnabled();
        },
        editText: (object, keys = [], pointer = null) => {
            if (closed) return;
            const target = object?.tableCell ? cellTarget() : textObjects.find(item => item.id === object?.id);
            if (!target || (draft !== null && target.id !== selected?.id)) return;
            if (target.id !== selected?.id) showSelected(target);
            if(target.tableObject) {
                const preserve=inline.active && (inline.pending || inline.key===target.id);
                if(!inline.begin(canvas.selectedTableCell,renderedScene.size,{pointer,preserve,value:target.text})) {
                    canvas.setEditingCell(null);status.text='This cell’s imported formatting cannot be edited in place.';return;
                }
                canvas.setEditingCell(target.id);updateEnabled();inline.input(keys);installShowShortcuts(inline.overlay);return;
            }
            surface.setProperty("focused", false);
            clipboardTextTarget = editor;
            editor.setProperty("focused", true);
            for (const key of keys) editor.sendMessage(Message.KEY, key);
        },
        notice: text => { status.text = text; },
    });
    inline=createInlineTableEditor({surface,viewport,
        change:(value,pending)=>{if(!pending)changedText(value);},
        navigate:moveCell,
        exit:finishCellEditing,
    });
    viewport.onViewChanged(view=>canvas.setViewScale(view.percent/100));
    async function finishCellEditing() {
        if(!inline.active)return true;
        if(inline.pending)await canvas.flushPendingTextInput();
        if(!await flushText())return false;
        inline.close();canvas.setEditingCell(null);surface.setProperty('focused',true);return true;
    }
    async function moveCell(backwards) {
        if(inline.pending)await canvas.flushPendingTextInput();
        if(!await flushText())return;
        const active=canvas.selectedTableCell;if(!active)return;
        const cells=active.object.table.cells.filter(cell=>!cell.covered);
        const index=cells.findIndex(cell=>cell.row===active.row&&cell.column===active.column);
        let next=cells[index+(backwards?-1:1)];
        if(!next && !backwards && active.object.table.canEditStructure && active.object.table.rows<128) {
            const row=active.object.table.rows;
            await canvas.tableStructure('insert-table-rows',row,1);
            next={row,column:0};
        }
        if(!next) {inline.editor.setProperty('focused',true);return;}
        canvas.selectTableCell(active.object.id,next.row,next.column);canvas.beginText();
    }
    const slides = createSlideNavigation({ port, navigate: (index, id) => navigate(index, 0, id),
        selectionChanged: () => { if (tools) updateEnabled(); },
        mutate: (type, target) => mutateSlide(type, 'title-content', target),
        previewWork: scheduleThumbnailWork });
    const slideActions = [...slides.toolbar.children];
    for (const control of slideActions) slides.toolbar.remove(control);
    const design = createDesignTools({ root,
        apply: commands => perform(async () => { await port.apply(commands); await read(); }),
        backgroundPicture: port?.backgroundPicture ? options => perform(async () => {
            const reply = await port.backgroundPicture(options);
            if (reply) await read();
            return reply;
        }) : undefined,
        notice: text => { status.text = text; },
    });
    transitionPreview = createTransitionPreview({ paint: frame => surface.submit(frame), restore: () => canvas.repaint(),
        failed: error => { status.text = `Transition preview failed: ${String(error?.message || error).slice(0, 200)}`; } });
    root.on(Message.POINTER, event => { if (event.detail?.kind === 'down') transitionPreview.cancel(); });
    const transitions = createTransitionTools({ root,
        apply: commands => perform(async () => {
            const previous = revision, response = await port.apply(commands);
            if (response?.revision) slides.retainArtwork(previous, response.revision);
            await read();
        }),
        preview: () => perform(async () => {
            if (!renderedScene?.transition) return;
            const previewRevision = transitionPreview.revision;
            const current = geometryFrame(renderedScene, { textFrames: renderedFrames, imageFrames: renderedPictures }).frame;
            let previous = null, previousSource = null;
            if (slideIndex > 0) {
                const scene = await loadDeckScene(port, { index: slideIndex - 1, revision, cancelled: () => closed });
                const rendered = await renderDeckScene(port, scene, { cancelled: () => closed });
                previousSource = { ...rendered, scene };
                previous = geometryFrame(scene, { textFrames: rendered.textFrames, imageFrames: rendered.imageFrames }).frame;
            }
            if (!closed && previewRevision === transitionPreview.revision) transitionPreview.play(previous, current, renderedScene.transition, { previous: previousSource, next: { scene: renderedScene, textFrames: renderedFrames, imageFrames: renderedPictures } });
        }), notice: text => { status.text = text; },
    });
    animationPreview = createAnimationPreview({
        paint: frame => surface.submit(frame), restore: () => canvas.repaint(),
        changed: () => updateEnabled(),
        failed: error => { status.text = `Animation preview failed: ${String(error?.message || error).slice(0, 200)}`; },
    });
    animations = createAnimationTools({ root,
        apply: commands => perform(async () => { const reply = await port.apply(commands); await read(); return reply; }),
        preview: intent => {
            if (intent.stop) { animationPreview.cancel(); return; }
            return perform(async () => {
                if (closed || intent.slide !== renderedScene?.slideId || !renderedTimeline) return;
                // Audience typography excludes placeholder prompts and editor guides.
                const scene = renderedScene, timeline = renderedTimeline, previewRevision = animationPreview.revision;
                let rendered;
                try { rendered = await renderDeckScene(port, scene, { cancelled: () => closed || previewRevision !== animationPreview.revision }); }
                catch (error) {
                    if (error.name === 'AbortError' && (closed || previewRevision !== animationPreview.revision)) return;
                    throw error;
                }
                if (!closed && previewRevision === animationPreview.revision && renderedScene === scene && renderedTimeline === timeline)
                    animationPreview.play({ ...rendered, scene }, timeline);
            });
        },
        selectObject: id => { animationPreview.cancel(); canvas.select([id]); },
        onPaneVisibility: visible => properties?.control.parent?.setProperty('visible', !visible),
        notice: text => { status.text = text; },
    });
    const layouts = createLayoutTools({ onApply: layout => perform(async () => {
        if (!renderedScene?.slideId) return;
        await port.apply([{ type: 'set-slide-layout', slide: renderedScene.slideId, layout }]); await read();
    }), onInsert: layout => mutateSlide('insert-slide', layout), notice: text => { status.text = text; } });
    const insertLayouts = createLayoutTools({ namePrefix: 'insert-', onInsert: layout => mutateSlide('insert-slide', layout), notice: text => { status.text = text; } });
    const showControls = [action('show-start', 'From beginning', () => perform(() => beginShow(0))), action('show-current', 'From current slide', () => perform(() => beginShow()))];
    showControls.forEach(control => control.setProperty('state', State.START_ALIGNED));
    for (const control of [editor, inline.editor, notes.editor]) {
        control.on(Message.FOCUS, () => { clipboardTextTarget = control; updateEnabled(); });
        control.on(Message.TEXT_SELECTION, updateEnabled);
        // TextEditorController applies keyboard selections in windowProc
        // before listeners run; unlike pointer selection it emits no separate
        // TEXT_SELECTION event. Keep ribbon Copy/Cut ready for Ctrl+A and
        // Shift+navigation, and disable them again when selection collapses.
        control.on(Message.KEY, event => { if (event.detail?.pressed) updateEnabled(); });
    }
    surface.on(Message.FOCUS, () => { if (!inline.active) clipboardTextTarget = null; });
    hyperlinks = createHyperlinkTools({ root, open: openHyperlink, slides: () => slides.hyperlinkSlides,
        apply: commands => perform(async () => { await port.apply(commands); await read(); }), notice: text => { status.text = text; } });
    tools = createAuthoringTools({ hyperlinkControls: hyperlinks.controls, root, canvas, shadowsSupported: Boolean(BigInt(app.hostCapabilities || 0) & (1n << 27n)), slideActions, showControls, layoutControls: [layouts.newSlideMenu, layouts.layoutMenu], insertSlideControl: insertLayouts.newSlideMenu, designControls: design.controls, transitionControls: transitions.controls, animationControls: animations.controls,
        clipboardActions: {
            ...(port?.copySelection || port?.writeClipboardText ? { copy: () => clipboardAction('copy'), cut: () => clipboardAction('cut') } : {}),
            ...(port?.pasteSelection || port?.readClipboardText ? { paste: () => clipboardAction('paste') } : {}),
            can: clipboardAvailable,
        }, picture: port?.picture ? insertPicture : undefined, notice: text => { status.text = text; } });
    function finishDiscard(accepted) {
        const resolve = resolveDiscard;
        resolveDiscard = null;
        if (discardDialog) root.remove(discardDialog, { destroy: true });
        discardDialog = null;
        resolve?.(accepted);
    }
    port?.setConfirmDiscard?.(() => {
        if (closed || discardDialog) return Promise.resolve(false);
        // An untouched automatically created page can yield to an opened file.
        // It remains unsaved: closing it still uses the normal save question.
        if (initialBlankRevision !== null && revision === initialBlankRevision && draft === null && !notes.dirty) return Promise.resolve(true);
        return new Promise(resolve => {
            resolveDiscard = resolve;
            const cancel = new Button({ name: "discard-document-cancel", text: "Cancel" });
            const discard = new Button({ name: "discard-document-confirm", text: "Discard changes" });
            cancel.on(Message.ACTIVATE, () => finishDiscard(false));
            discard.on(Message.ACTIVATE, () => finishDiscard(true));
            discardDialog = new Dialog({ name: "discard-document-dialog", text: "Unsaved presentation", modal: true }, [
                new Label({ text: "Discard unsaved changes and open another presentation?" }),
                new Row({ text: "panel:actions" }, [cancel, discard]),
            ]);
            discardDialog.on(Message.KEY, event => {
                if (event.detail?.pressed && (event.detail.key === "Escape" || event.detail.keyCode === 9)) {
                    finishDiscard(false); event.preventDefault();
                }
            });
            root.append(discardDialog);
        });
    });
    function action(name, text, callback, menu = false) {
        const options = { name, text, enabled: false, ...(ACTION_ICONS[name] ? { icon: ACTION_ICONS[name] } : {}) };
        const item = menu ? new MenuItem(options) : new Button(options);
        item.on(Message.ACTIVATE, () => { if (item.getProperty("enabled")) void callback(); });
        controls.set(name, item);
        return item;
    }
    function enable(name, enabled) { controls.get(name)?.setProperty("enabled", Boolean(!closed && (!busy || notes?.committing || committingText) && enabled)); }
    function updateEnabled() {
        const labelText = sensitivityCaption(projection?.sensitivity);
        sensitivityBadge.text = sensitivityCaption(projection?.sensitivity,{compact:true});
        sensitivityBadge.setProperty('accessibleName',labelText);
        sensitivityBadge.setProperty('tooltip',labelText);
        sensitivityBadge.setProperty('visible',Boolean(labelText));
        sensitivityItem.setProperty('visible',Boolean(labelText));
        sensitivityItem.setProperty('enabled',!busy&&!uncertain&&draft===null&&!presenter?.active);
        sensitivityBadge.setProperty('enabled',sensitivityItem.getProperty('enabled'));
        const operationBusy=busy&&!committingText;
        authoring.setProperty('enabled', closeDecision === null && !closeApproved);
        const showing = Boolean(presenter?.active), idleDraft = (draft === null || inline?.active) && !showing;
        const usable = projection && !uncertain;
        enable("new", idleDraft && typeof port?.documents?.new === "function");
        enable("open", idleDraft && typeof port?.documents?.open === "function");
        enable('open-template', idleDraft && typeof port?.documents?.['open-template'] === 'function');
        enable('exit', !closeDecision && !closeApproved && Boolean(mainWindow));
        for (const name of ["save", "save-as"]) enable(name, !showing &&
            (draft === null || (selected?.editable && fitsEditor(draft))) &&
            (projection?.loaded ?? Boolean(projection?.slideCount)) && usable && typeof port?.documents?.[name] === "function");
        for (const name of ['show-start', 'show-current', 'status-show-start', 'status-show-current']) enable(name, idleDraft && usable && sceneReady && projection.slideCount > 0);
        enable("undo", idleDraft && usable && (projection.canUndo || notes?.dirty));
        enable("redo", idleDraft && usable && projection.canRedo);
        enable("previous-text", idleDraft && usable && sceneReady && startText > 0);
        enable("next-text", idleDraft && usable && sceneReady && startText + PAGE < textObjects.length);
        enable("apply", usable && sceneReady && draft !== null && selected?.editable && fitsEditor(draft));
        enable("discard", draft !== null);
        enable("refresh", Boolean(port) && !showing);
        slides.setState({ disabled: closed || (operationBusy && !notes?.committing) || !idleDraft || uncertain || Boolean(closeDecision) || closeApproved, loaded: projection?.loaded ?? Boolean(projection?.slideCount), index: slideIndex, count: projection?.slideCount || 0 });
        contextMenu?.setState({ ready: Boolean(usable && sceneReady), busy: closed || operationBusy || !idleDraft || uncertain, selected: canvas.selected,
            canUndo: Boolean(projection?.canUndo), canRedo: Boolean(projection?.canRedo), background: renderedScene?.background });
        hyperlinks?.setState({ ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), scene: renderedScene, revision, selected: canvas.selected });
        canvas.setLocked(closed || operationBusy || (!idleDraft && !inline?.active) || uncertain || !sceneReady);
        tools?.setState({ ready: Boolean(usable && sceneReady), busy: closed || operationBusy || !idleDraft || uncertain,
            clipboardReady: Boolean(usable && sceneReady && !closed && !operationBusy && !uncertain) });
        properties?.setState({ objects: canvas.selected, ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), scene: renderedScene, editingText: draft !== null });
        design.setState({ current: renderedScene, ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft) });
        transitions.setState({ current: renderedScene, selectedSlides: slides.selectedSlides,
            ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft) });
        animations?.setState({ current: renderedScene, timeline: renderedTimeline, selected: canvas.selected,
            ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), previewing: animationPreview?.active === true });
        layouts.setState({ ready: Boolean(usable && (projection?.loaded ?? projection?.slideCount)), canApply: sceneReady, busy: closed || operationBusy || !idleDraft || uncertain, selectedLayout: renderedScene?.layoutId || null, pageSize: renderedScene?.size });
        insertLayouts.setState({ ready: Boolean(usable && (projection?.loaded ?? projection?.slideCount)), canApply: false, busy: closed || operationBusy || !idleDraft || uncertain, selectedLayout: renderedScene?.layoutId || null, pageSize: renderedScene?.size });
        const editable = Boolean(!showing && !closed && (!busy || committingText || clipboardTextBusy) && !uncertain && sceneReady && selected?.editable && fitsEditor(selected.text));
        editor.setProperty("enabled", editable);
        editor.setProperty("readOnly", !editable);
        inline?.editor.setProperty('enabled',Boolean(inline.active && !closed && !uncertain && (inline.pending || selected?.tableObject && selected.editable)));
        inline?.editor.setProperty('readOnly',Boolean(closed || uncertain));
        notes?.setState({ enabled: Boolean(!closed && (!busy || notes.committing || clipboardTextBusy) && !uncertain && !showing && !closeApproved && projection?.slide),
            visible: !overview && !showing });
        for (const button of runs.children) button.setProperty("enabled", !closed && !operationBusy && idleDraft && !uncertain && sceneReady);
    }
    function showSelected(target) {
        // A hidden editor can be recreated while native creation/promotion is
        // still pending. Its previous object's focus request must not steal
        // canvas keys before editText finishes replaying the buffered input.
        if (target?.id !== selected?.id || target?.slideId !== selected?.slideId) editor.setProperty('focused', false);
        selected = target;
        const fits = target && fitsEditor(target.text);
        editor.setProperty("value", target ? (fits ? target.text : "This text box exceeds the native editor size limit; its source is preserved.") : "");
        runLabel.text = target ? `${target.tableObject ? `Cell ${target.row + 1}, ${target.column + 1}` : `Text box ${target.index + 1}`}${target.editable && fits ? "" : " · Read only"}` : "Select a text box";
        const table = Boolean(target?.tableObject);
        if(table && inline?.pending) {
            const value=inline.value;
            inline.begin(canvas.selectedTableCell,renderedScene.size,{value,preserve:true});
            canvas.setEditingCell(target.id);
            draft=value!==target.text?value:null;
            scheduleTextCommit();
        } else if(!table && inline?.active) {inline.close();canvas.setEditingCell(null);}
        root.find('toggle-text-outline')?.setProperty('visible', !table);
        if (table) root.find('text-outline')?.setProperty('visible', false);
        for (const button of runs.children) button.setProperty("checked", button.name === `object-${target?.index}`);
        updateEnabled();
    }
    function renderTextList() {
        for (const child of [...runs.children]) runs.remove(child, { destroy: true });
        for (const target of textObjects.slice(startText, startText + PAGE)) {
            const button = new Button({ name: `object-${target.index}`, text: `Text box ${target.index + 1}` });
            button.on(Message.ACTIVATE, () => {
                if (!busy && !closed && !uncertain && draft === null && sceneReady) {
                    showSelected(target); canvas.select([target.id]);
                }
            });
            runs.append(button);
        }
    }
    function presentText(scene, preserveDraft) {
        const old = selected;
        textObjects = scene.objects.filter(object => object.text !== null).map((object, index) => ({
            id: object.id, slideId: scene.slideId, index, text: object.text.text,
            editable: object.text.editable === true,
        }));
        const cell = cellTarget();
        const same = cell?.id === old?.id && cell?.slideId === old?.slideId ? cell : textObjects.find(item => item.id === old?.id && item.slideId === old?.slideId);
        if (preserveDraft && draft !== null) {
            selected = same || old;
            if(same?.tableObject)inline.refresh(canvas.selectedTableCell,scene.size);
            if (!same) {
                uncertain = true;
                status.text = "The selected text box is no longer available. Your draft has been retained; discard it and refresh to select another box.";
            }
            startText = Math.min(startText, Math.max(0, Math.floor((textObjects.length - 1) / PAGE) * PAGE));
            renderTextList(); updateEnabled(); return;
        }
        draft = null;
        const active = canvas.selected;
        const target = cell || (active.length ? (active.length === 1 ? textObjects.find(item => item.id === active[0].id) : null)
            : same || textObjects.find(item => item.editable));
        if (target) startText = Math.floor(target.index / PAGE) * PAGE;
        else startText = Math.min(startText, Math.max(0, Math.floor((textObjects.length - 1) / PAGE) * PAGE));
        renderTextList(); showSelected(target || null);
        if(target?.tableObject && inline.active) {
            inline.refresh(canvas.selectedTableCell,scene.size);
            if(!preserveDraft && draft===null && inline.value!==target.text)inline.editor.setProperty('value',target.text);
        }
        if (target && !active.length) canvas.select([target.id]);
    }
    function pageText(start) {
        if (busy || closed || uncertain || draft !== null || !sceneReady) return;
        startText = start; renderTextList();
        const target = textObjects[startText];
        showSelected(target || null); canvas.select(target ? [target.id] : []);
    }
    function present(response) {
        if (!response || typeof response.revision !== "string" || !/^(0|[1-9]\d*)$/u.test(response.revision) || !response.result) throw new TypeError("Invalid Deck projection");
        if (revision !== null && BigInt(response.revision) < BigInt(revision)) throw new TypeError("Stale Deck projection");
        const next = response.result;
        if (!Number.isSafeInteger(next.slideCount) || next.slideCount < 0 || (next.slide && (!Array.isArray(next.slide.text) || next.slide.text.length > PAGE))) throw new TypeError("Invalid Deck outline");
        projection = next; revision = response.revision;
        updateTitle();
        heading.text = next.slide ? `Slide ${next.slide.index + 1} of ${next.slideCount}` : next.loaded ? "No slides · Choose New slide to begin" : "No presentation open";
        status.text = ''; 
        updateEnabled();
    }
    function updateTitle() {
        if (mainWindow) mainWindow.title = documentName ? `${projection?.dirty || notes?.dirty ? '* ' : ''}${documentName} — Rice Deck` : 'Rice Deck';
    }
    async function read(preserveDraft = false, preserveNotesDraft = false) {
        let response = await port.project({ slide: port.outline ? 0 : slideIndex, startRun: 0, limit: PAGE });
        if (closed) return;
        if (port.outline) {
            const target = await slides.locate({ revision: response.revision, id: selectedSlideId, index: slideIndex, count: response.result.slideCount });
            if (closed) return;
            slideIndex = target.index; selectedSlideId = target.id;
            if (slideIndex) response = await port.project({ slide: slideIndex, startRun: 0, limit: PAGE });
        }
        present(response);
        selectedSlideId = projection.slide?.id || null;
        if (port.outline) slides.show(slideIndex, projection.slideCount);
        transitionPreview.cancel(); animationPreview.cancel(); renderedTimeline = null; renderedScene = null; renderedFrames = new Map(); renderedPictures = new Map();
        if (geometryVisible && projection.slideCount) await readScene(preserveDraft);
        else {
            sceneReady = false; surface.setProperty("visible", false);
            textObjects = []; renderTextList();
            if (draft === null) showSelected(null);
            sceneNotice.text = projection.loaded ? 'This presentation has no slides. Choose New slide to begin.' : '';
            sceneNotice.setProperty('visible', Boolean(projection.loaded));
        }
        if (port.outline && projection.slideCount) await slides.previews(revision, renderedScene, renderedFrames);
        if (notes.opened && projection.slide && port.speakerNotes) {
            const reply = await port.speakerNotes({ slide: projection.slide.id }, revision);
            if (!closed) notes.bind(reply.result, { preserveDraft: preserveNotesDraft });
        } else notes.bind(null);
        installShowShortcuts(authoring);
    }
    async function readScene(preserveDraft) {
        try {
            const scene = await loadDeckScene(port, { index: slideIndex, revision, cancelled: () => closed });
            const text = port.renderText ? await port.renderText(scene, { placeholderPrompts: true }) : { frames: new Map(), notices: [] };
            const pictures = port.renderImages ? await port.renderImages(scene) : { frames: new Map(), notices: [] };
            const timeline = await loadAnimationTimeline(port, { scene, revision, cancelled: () => closed });
            const catalog = scene.animationTimeline?.editable && port.animationCatalog ? await port.animationCatalog(revision) : null;
            if (closed) return;
            animations.setCatalog(catalog?.result.effects || []);
            renderedTimeline = timeline;
            sceneReady = true; renderedScene = scene; renderedFrames = text.frames; renderedPictures = pictures.frames;
            refreshingScene = true;
            try { canvas.setScene(scene, text.frames, pictures.frames); } finally { refreshingScene = false; }
            presentText(scene, preserveDraft); surface.setProperty('visible', true);
            sceneNotice.text = '';
            sceneNotice.setProperty('visible', false);
        } catch (error) {
            sceneReady = false; surface.setProperty('visible', false);
            if (error.code && error.rejectedBeforeMutation !== true) uncertain = true;
            sceneNotice.text = `Slide unavailable: ${String(error.message).slice(0, 180)}.`;
            sceneNotice.setProperty('visible', true);
        }
    }
    function scheduleThumbnailWork(work) {
        if (closed || busy || uncertain || draft !== null || closeDecision || closeApproved || presenter?.active) return;
        // Thumbnail reads do not disable authoring. A user operation takes the
        // normal lock, cancelling the preview at its next await, then waits for
        // the one in-flight port request before it accesses the document.
        const pending = Promise.resolve().then(work);
        thumbnailWork = pending;
        return pending.finally(() => { if (thumbnailWork === pending) thumbnailWork = null; });
    }
    async function perform(operation, flushNotes = true, retainPresentation = false) {
        transitionPreview?.cancel(); animationPreview?.cancel();
        if (closed || !port) return;
        if(inline?.active && !committingText && !await flushText())return;
        if (busy) {
            if (!flushNotes || !notes?.committing || !await notes.flush()) return;
        }
        if (flushNotes && notes?.dirty && !await notes.flush()) return;
        if (closed || busy) return;
        // Navigation keeps the last complete slide/ribbon visible while its
        // real disabled controls continue rejecting input in the SDK.
        const releasePresentation = retainPresentation ? mainWindow?.deferPresentation() : null;
        busy = true;
        try { updateEnabled(); await thumbnailWork; if (!closed) return await operation(); }
        catch (error) {
            if (!closed) {
                uncertain = true;
                status.text = `Operation failed: ${String(error?.message || error).slice(0, 240)}. Refresh before retrying; your draft is retained.`;
            }
        } finally {
            try {
                if (!closed) {
                    busy = false; updateEnabled();
                    for (const resolve of idleWaiters) resolve();
                    idleWaiters.clear();
                    if (pendingGrant && !closeDecision) queueMicrotask(openPendingGrant);
                }
            } finally { releasePresentation?.(); }
        }
    }
    async function navigate(slide, start, id = null) {
        if (uncertain || !await finishCellEditing() || draft !== null) return;
        const previous = [slideIndex, startText, selectedSlideId];
        await perform(async () => {
            slideIndex = slide; startText = start; selectedSlideId = id;
            try { await read(); } catch (error) { [slideIndex, startText, selectedSlideId] = previous; throw error; }
        }, true, true);
    }
    async function insertPicture(replace = null) {
        if (!await finishCellEditing() || draft !== null || uncertain || !sceneReady || !renderedScene?.size) return;
        await perform(async () => {
            const target = canvas.selected.length === 1 ? canvas.selected[0] : null;
            const placeholder = !replace && target?.placeholder?.role === 'picture' && !target.text?.text && target.edit?.canDelete && target.edit?.box
                ? { id: target.id, box: { ...target.edit.box } } : null;
            const reply = await port.picture({ slide: renderedScene.slideId, size: renderedScene.size, replace, ...(placeholder ? { placeholder } : {}) });
            if (reply) { canvas.created(reply); await read(); }
        });
    }
    async function mutateSlide(type, layout = 'title-content', target = null) {
        if (!await finishCellEditing() || draft !== null || uncertain || !port?.outline) return;
        await perform(async () => {
            const command = await slides.command(type, target?.slide ?? selectedSlideId, target?.index ?? slideIndex, projection.slideCount, revision, target);
            if (type === 'insert-slide') command.layout = layout;
            const reply = await port.apply([command]);
            const created = reply?.result?.createdSlides;
            if (Array.isArray(created) && created.length) selectedSlideId = created[0];
            else if (target?.slide) { selectedSlideId = target.slide; slideIndex = target.index; }
            selected = null; startText = 0;
            await read();
        });
    }
    function clipboardEditor() {
        const target = inline?.active ? inline.editor : clipboardTextTarget;
        return target?.getProperty('visible') && target.getProperty('enabled') ? target : null;
    }
    function clipboardAvailable(kind) {
        const target = clipboardEditor();
        if (target) {
            const range = target.selection;
            return kind === 'paste' ? Boolean(port?.readClipboardText && !target.getProperty('readOnly'))
                : Boolean(port?.writeClipboardText && range.anchor !== range.caret && (kind !== 'cut' || !target.getProperty('readOnly')));
        }
        return kind === 'paste' ? Boolean(port?.pasteSelection) : Boolean(port?.copySelection && canvas.selected.length &&
            (kind !== 'cut' || canvas.selected.every(object => object.edit?.canDelete)));
    }
    async function clipboardAction(kind) {
        const target = clipboardEditor();
        if (target) return textClipboardAction(kind, target);
        if (closed || uncertain || !sceneReady || !(await flushText())) return;
        if ((notes.dirty || notes.committing) && !await notes.flush()) return;
        if (closed || busy || uncertain || !sceneReady) return;
        const objects = canvas.selected.map(object => object.id);
        if (kind !== 'paste' && !objects.length || kind === 'cut' && canvas.selected.some(object => !object.edit?.canDelete)) return;
        let mutationStarted = false;
        busy = true; updateEnabled();
        try {
            await thumbnailWork; if (closed) return;
            if (kind !== 'paste') {
                const slide = renderedScene.slideId, copiedRevision = revision;
                await port.copySelection({ slide, objects });
                if (kind === 'cut' && !closed && revision === copiedRevision && renderedScene.slideId === slide) {
                    mutationStarted = true;
                    await port.apply(objects.map(object => ({ type: 'delete-object', slide, object })));
                    canvas.select([]); await read();
                }
            }
            else {
                mutationStarted = true;
                const reply = await port.pasteSelection({ slide: renderedScene.slideId, offset: { x: 16, y: 16 } });
                if (closed) return;
                canvas.created(reply); await read();
            }
        } catch (error) {
            if (mutationStarted && error.rejectedBeforeMutation !== true) uncertain = true;
            status.text = `Cannot ${kind}: ${String(error?.message || error).slice(0, 240)}`;
        } finally {
            busy = false;
            if (!closed) {
                updateEnabled(); for (const resolve of idleWaiters) resolve(); idleWaiters.clear();
                if (pendingGrant && !closeDecision) queueMicrotask(openPendingGrant);
            }
        }
    }
    async function restoreClipboardFocus(target) {
        const value = target.getProperty('value'), range = target.selection;
        // Pointer focus on a ribbon command does not clear the editor's
        // retained FOCUSED bit. Commit a real release edge through the actor's
        // scheduled patch queue before requesting it again; direct flushes can
        // bypass worker transport and a same-value assignment is a no-op.
        target.setProperty('focused', false);
        await Promise.resolve();
        const current = target.selection;
        if (!closed && clipboardEditor() === target && target.getProperty('value') === value &&
            current.anchor === range.anchor && current.caret === range.caret) target.sendMessage(Message.FOCUS);
    }
    async function textClipboardAction(kind, target) {
        if (committingText) await flushText();
        if (notes?.committing) await notes.flush();
        if (closed || busy || uncertain || !clipboardAvailable(kind)) return;
        const value = target.getProperty('value'), range = target.selection;
        const start = Math.min(range.anchor, range.caret), end = Math.max(range.anchor, range.caret), chars = Array.from(value);
        busy = true; clipboardTextBusy = true; updateEnabled();
        try {
            const inserted = kind === 'paste' ? await port.readClipboardText() : '';
            if (kind !== 'paste') await port.writeClipboardText(chars.slice(start, end).join(''));
            if (closed) return;
            const current = target.selection;
            if (clipboardEditor() !== target || target.getProperty('value') !== value || current.anchor !== range.anchor || current.caret !== range.caret)
                throw Error('The text selection changed while accessing the clipboard. Try again.');
            if (kind === 'copy') { await restoreClipboardFocus(target); return; }
            if (!fitsEditor([...chars.slice(0, start), inserted, ...chars.slice(end)].join(''))) throw Error('Clipboard text exceeds the cell editor limit');
            // Use the native editor's ordinary selection replacement and draft
            // events; never rebuild the document or flatten other cells.
            target.setProperty('enabled', true);
            target.sendMessage(Message.KEY, { pressed: true, keyCode: kind === 'cut' ? 22 : 0, modifiers: 0, text: inserted });
            await restoreClipboardFocus(target);
        } catch (error) { if (!closed) status.text = `Cannot ${kind}: ${String(error?.message || error).slice(0, 240)}`; }
        finally {
            busy = false; clipboardTextBusy = false;
            if (!closed) {
                updateEnabled(); scheduleTextCommit(); for (const resolve of idleWaiters) resolve(); idleWaiters.clear();
                if (pendingGrant && !closeDecision) queueMicrotask(openPendingGrant);
            }
        }
    }
    async function documentAction(name) {
        if(inline.pending)await canvas.flushPendingTextInput();
        if (!(await flushText())) return;
        await perform(async () => {
            const result = await port.documents[name]();
            if (closed || !result) return;
            initialBlankRevision = null;
            if (result.name) documentName = result.name;
            if (result.replaced) { inline.close();canvas.setEditingCell(null);slides.reset(); revision = null; projection = null; selected = null; slideIndex = 0; startText = 0; selectedSlideId = null; uncertain = false; }
            await read();
            if (name === 'new' || name === 'new-odp') canvas.select([]);
            if (result.startShow) await beginShow(0);
        });
    }
    async function beginShow(index = slideIndex) {
        if (!presenter || !await finishCellEditing() || draft !== null || uncertain || !sceneReady || !projection?.slideCount) return;
        const start = () => presenter.start({ index, revision, count: projection.slideCount, hiddenSlides: slides.hiddenSlideIndices });
        if (busy) await start(); else await perform(start);
    }
    function cancelTextCommit() {
        if (textCommitTimer !== null) clearTimeout(textCommitTimer);
        textCommitTimer = null;
    }
    function scheduleTextCommit() {
        cancelTextCommit();
        if (draft !== null && !closed && !closeDecision && !closeApproved && !uncertain && fitsEditor(draft)) {
            textCommitTimer = setTimeout(() => { textCommitTimer = null; void applyDraft(); }, 150);
        }
    }
    async function applyDraft() {
        cancelTextCommit();
        if (textCommit) { await textCommit; return; }
        if (draft === null || !selected || uncertain || closed || !fitsEditor(draft)) return;
        if (busy) { scheduleTextCommit(); return; }
        const text = draft, target = selected.tableObject ? { slide: selected.slideId, object: selected.tableObject, row: selected.row, column: selected.column } : { slide: selected.slideId, object: selected.id };
        const type = selected.tableObject ? "set-table-cell-text" : "set-object-text";
        committingText = true;
        textCommit = perform(async () => {
            await port.apply([{ type, ...target, text }]);
            // Keep the native editor and its caret intact while refreshing the
            // authoritative slide. Keystrokes during compute stay in the draft.
            await read(true);
            if (!uncertain && draft === text) draft = null;
        });
        try { await textCommit; }
        finally {
            textCommit = null; committingText = false;
            if (!closed) { updateEnabled(); scheduleTextCommit(); }
        }
    }
    async function flushText() {
        cancelTextCommit();
        while (!closed && !uncertain && (draft !== null || textCommit)) {
            if (busy && !committingText) return false;
            if (draft !== null && !fitsEditor(draft)) return false;
            await applyDraft();
        }
        return !closed && !uncertain;
    }
    const menus = new MenuBar({ name: 'deck-file-menu', text: 'panel:office-ribbon-heading:flush:fixed' }, [
        new Menu({ name: 'file-menu', text: "File", icon: 2 }, [action("new", "New", () => documentAction("new"), true),
            new MenuItem({ name: 'open-submenu', text: 'Open', icon: 11 }, [new Menu({ name: 'open-menu', text: 'Open', icon: 11 }, [
                action("open", "Open…", () => documentAction("open"), true),
                action('open-template', 'Edit template…', () => documentAction('open-template'), true),
            ])]),
            action("save", "Save", () => documentAction("save"), true), action("save-as", "Save As…", () => documentAction("save-as"), true),
            sensitivityItem]),
        new Menu({ text: "Edit", icon: 69 }, [action("undo", "Undo", () => perform(async () => { await port.undo(); await read(); }), true), action("redo", "Redo", () => perform(async () => { await port.redo(); await read(); }), true)]),
        new Menu({ name: 'help-menu', text: 'Help', icon: 51 }, [about]),
    ]);
    for (const menu of [...menus.children].slice(1)) {
        menus.remove(menu);
        menus.children[0].append(new MenuItem({ text: menu.text, icon: menu.getProperty('icon') }, [menu]));
    }
    menus.children[0].append(action('exit', 'Exit', () => mainWindow.requestClose(), true));
    const quick = new ToolBar({ name: 'deck-quick-access', text: 'panel:office-ribbon-heading:flush:fixed:gap=2:accessory-height=28' });
    // Reuse the authoritative menu actions so capability/dirty-state handling
    // stays identical for keyboard, menu and quick-access activation.
    for (const [name, label, icon] of [['save', 'Save (Ctrl+S)', 12], ['undo', 'Undo (Ctrl+Z)', 20], ['redo', 'Redo (Ctrl+Y)', 21]]) {
        const source = controls.get(name), button = new Button({ name: `quick-${name}`, icon, tooltip: label, accessibleName: label, enabled: source.getProperty('enabled') });
        source.on(Message.CHANGE, event => { if (event.detail?.property === 'enabled') button.setProperty('enabled', source.getProperty('enabled')); });
        button.on(Message.ACTIVATE, () => { if (button.getProperty('enabled')) source.sendMessage(Message.ACTIVATE); });
        quick.append(button);
    }
    tools.control.setHeader({ leading: [menus], trailing: [sensitivityBadge, quick] });
    authoring.append(tools.control);
    const textOutline = new Column({ name: 'text-outline', visible: false }, [
        new ScrollView({ name: 'outline-scroll', text: 'viewport:180|0/0' }, [runs]),
        new Row({}, [action('previous-text', 'Previous boxes', () => pageText(Math.max(0, startText - PAGE))), action('next-text', 'More boxes', () => pageText(startText + PAGE))]),
    ]);
    const outlineToggle = new Button({ name: 'toggle-text-outline', text: 'Text boxes on this slide', icon: 74, state: State.START_ALIGNED });
    outlineToggle.on(Message.ACTIVATE, () => textOutline.setProperty('visible', !textOutline.getProperty('visible')));
    const textPanel = new Column({ name: 'text-panel', text: 'panel:tight-stack' }, [
        runLabel, editor,
        new Row({ text: "panel:actions" }, [action("apply", "Apply text", applyDraft), action("discard", "Discard draft", () => { cancelTextCommit(); draft = null; showSelected(selected); })]),
        action("refresh", "Refresh", () => perform(async () => { uncertain = false; await read(true, true); }, false)),
        outlineToggle, textOutline,
    ]);
    properties = createSelectionProperties({ canvas, tools, textPanel });
    const navigationHost = new Column({ name: 'navigation-host', text: 'panel:flush:gap=0' }, [slides.control]);
    const overviewHost = new Column({ name: 'slide-overview', text: 'panel:flush:gap=4', state: State.EXPANSIVE, visible: false });
    const workspace = new SplitPane({ name: 'slide-workspace', text: 'split:260:trailing:min-width=220:max-width=340', state: State.EXPANSIVE }, [
        new SplitPane({ name: 'slides-canvas', text: 'split:180:min-width=180:max-width=260', state: State.EXPANSIVE }, [
            navigationHost, new Column({ name: 'canvas-panel', text: 'panel:flush:gap=0' }, [status, sceneNotice,
                viewport, notes.control]),
        ]), new Column({ name: 'context-pane', text: 'panel:flush:gap=0', state: State.EXPANSIVE }, [
            new ScrollView({ name: 'properties-scroll' }, [properties.control]), animations.pane]),
    ]);
    authoring.append(workspace); authoring.append(overviewHost);
    async function setOverview(value) {
        if (closed || uncertain || !(await flushText())) return;
        await perform(async () => {
            overview = value;
            slides.control.parent.remove(slides.control);
            (overview ? overviewHost : navigationHost).append(slides.control);
            overviewHost.setProperty('visible', overview); workspace.setProperty('visible', !overview);
            overviewButton.setProperty('selected', overview); normalButton.setProperty('selected', !overview);
            slides.setOverview(overview); await read();
        });
    }
    const viewButton = (name, icon, tooltip, callback) => {
        const button = new Button({ name, icon, tooltip, accessibleName: tooltip, text: 'panel:fixed:accessory-height=26:width=26:flush' });
        button.on(Message.ACTIVATE, callback); return button;
    };
    const normalButton = viewButton('normal-view', 40, 'Normal view', () => void setOverview(false));
    const overviewButton = viewButton('slide-overview-toggle', 41, 'Slide overview', () => void setOverview(true));
    normalButton.setProperty('selected', true);
    const playBeginning = viewButton('status-show-start', 82, 'Play from beginning', () => void perform(() => beginShow(0)));
    const playCurrent = viewButton('status-show-current', 104, 'Play from current slide', () => void perform(() => beginShow()));
    controls.set('status-show-start', playBeginning); controls.set('status-show-current', playCurrent);
    const notesButton = new Button({ name: 'notes-toggle', text: 'Notes', icon: 140, tooltip: 'Speaker notes', accessibleName: 'Speaker notes',
        visible: Boolean(port?.speakerNotes), state: State.START_ALIGNED });
    notesButton.on(Message.ACTIVATE, () => { void perform(async () => {
        if (!projection?.slide || !port.speakerNotes) return;
        const opening = !notes.opened;
        if (opening) notes.bind((await port.speakerNotes({ slide: projection.slide.id }, revision)).result);
        notes.setOpen(opening); notesButton.setProperty('selected', opening);
    }).then(() => { notes.focus(); }); });
    heading.setProperty('state', State.EXPANSIVE);
    authoring.append(new StatusBar({ name: 'deck-status-bar', text: 'panel:status-bar:accessory-height=30:flush:gap=4' }, [heading, notesButton, normalButton, overviewButton, zoom, playBeginning, playCurrent]));
    contextMenu = createDeckContextMenu({ owner: viewport, notice: text => { status.text = text; }, actions: {
        undo: () => perform(async () => { await port.undo(); await read(); }),
        redo: () => perform(async () => { await port.redo(); await read(); }),
        editLink: () => hyperlinks.edit(), openLink: () => hyperlinks.open(), removeLink: () => hyperlinks.remove(),
        editText: () => canvas.beginText(), addText: () => canvas.beginText(),
        ...(port?.copySelection ? { copy: () => clipboardAction('copy') } : {}),
        ...(port?.pasteSelection ? { paste: () => clipboardAction('paste') } : {}),
        ...(canvas.group ? { group: () => canvas.group() } : {}),
        ...(canvas.ungroup ? { ungroup: () => canvas.ungroup() } : {}),
        ...(canvas.reorder ? {
            bringForward: () => canvas.reorder('forward'), bringToFront: () => canvas.reorder('front'),
            sendBackward: () => canvas.reorder('backward'), sendToBack: () => canvas.reorder('back'),
        } : {}),
        duplicate: () => canvas.duplicate(), delete: () => canvas.remove(),
        positionSize: () => tools.position(), rotate: () => tools.selectTab('format'),
        formatShape: () => tools.selectTab('format'), formatPicture: () => tools.selectTab('picture'),
        fill: color => canvas.fill(color), strokeColor: color => canvas.strokeColor(color), style: style => canvas.style(style),
        slideBackground: () => tools.selectTab('design'),
        backgroundColor: color => perform(async () => { await port.apply([{ type: 'set-slide-background', slide: renderedScene.slideId, color }]); await read(); }),
    } });
    presenter = createPresentationView({ port, openHyperlink: link => openHyperlink(link, true), getWindow: () => mainWindow,
        changed(active) {
            authoring.setProperty('visible', !active);
            if (active) { animationPreview.cancel(); canvas.cancel(); surface.submit(new SceneBuilder(1, 1).finish()); editor.setProperty('focused', false); surface.setProperty('focused', false); }
            updateEnabled();
        },
        async exited(index) {
            slideIndex = index; selectedSlideId = null;
            await perform(() => read()); surface.setProperty('focused', true);
        },
    });
    root.append(authoring); root.append(presenter.control);
    mainWindow = createWindow({ title: "Rice Deck", root });
    function showShortcut(event) {
        const key = event.detail;
        if (key?.pressed) {
            transitionPreview.cancel();
            if (event.target === surface || key.key === 'Escape' || key.keyCode === 9) animationPreview.cancel();
        }
        if (event.target !== surface && event.target !== editor && event.target !== inline.editor && canvas.capturePendingText(event)) return;
        if (key?.pressed && (key.key === 'Escape' || key.keyCode === 9) && canvas.formatPainter?.active && !presenter.active) {
            canvas.cancelFormatPainter(); event.preventDefault(); return;
        }
        if (event.defaultPrevented || !key?.pressed || presenter.active || (busy && !notes.committing) || closed || uncertain) return;
        const pageDirection = ({ PageUp: -1, PageDown: 1 })[key.key] || ({ 112: -1, 117: 1 })[key.keyCode];
        if (!(key.modifiers & (4 | 8 | 64)) && pageDirection) {
            const next = slideIndex + pageDirection;
            event.preventDefault();
            if (next >= 0 && next < (projection?.slideCount || 0)) void (async () => {
                if (await flushText()) await navigate(next, 0);
            })();
            return;
        }
        if (!(key.modifiers & 12) && (key.key === 'F5' || key.keyCode === 71) && draft === null) {
            event.preventDefault(); void perform(() => beginShow(key.modifiers & 1 ? slideIndex : 0)); return;
        }
        if (!(key.modifiers & 4) || key.modifiers & 8) return;
        const letter = ({ 54: 'c', 53: 'x', 55: 'v', 40: 'd', 39: 's', 52: 'z', 29: 'y', 56: 'b', 31: 'i', 30: 'u', 32: 'o', 57: 'n', 45: 'k' })[key.keyCode] || key.key?.toLowerCase();
        if (letter === 's') {
            event.preventDefault();
            void (async () => {
                if (draft !== null) await applyDraft();
                const name = key.modifiers & 1 ? 'save-as' : 'save';
                if (draft === null && controls.get(name)?.getProperty('enabled')) await documentAction(name);
            })();
            return;
        }
        if (draft !== null && !inline.active) return;
        if(event.target===inline.editor && ['z','y'].includes(letter))return;
        if (event.target === surface && ((['c', 'x'].includes(letter) && port?.copySelection) || (letter === 'v' && port?.pasteSelection))) {
            event.preventDefault(); void clipboardAction(letter === 'c' ? 'copy' : letter === 'x' ? 'cut' : 'paste'); return;
        }
        if (letter === 'k') { event.preventDefault(); hyperlinks.edit(); return; }
        if (letter === 'd' && event.target === surface) { event.preventDefault(); void canvas.duplicate(); return; }
        const property = { b: 'bold', i: 'italic', u: 'underline' }[letter];
        if (event.target === notes.editor && property) return;
        if (property) { event.preventDefault(); void tools.toggleTextStyle(property); return; }
        const name = { z: key.modifiers & 1 ? 'redo' : 'undo', y: 'redo', o: 'open', n: 'new' }[letter];
        if (name && controls.get(name)?.getProperty('enabled')) {
            event.preventDefault(); controls.get(name).emit(Message.ACTIVATE);
        }
    }
    function installShowShortcuts(control) {
        if (!shortcutControls.has(control)) {
            control.on(Message.KEY, showShortcut);
            control.on(Message.CHILD_ADDED, event => installShowShortcuts(event.detail.child));
            shortcutControls.add(control);
        }
        for (const child of control.children) installShowShortcuts(child);
    }
    installShowShortcuts(authoring);
    function changedText(value) {
        if (closed || (busy && !committingText && !clipboardTextBusy && !inline.active) || uncertain || !selected?.editable) return;
        if (!selected.slideId) selected = { ...selected, slideId: projection.slide.id };
        draft = value;
        if (draft === selected.text && !committingText) draft = null;
        updateEnabled(); scheduleTextCommit();
    }
    editor.on(Message.VALUE_CHANGED, event => changedText(String(event.detail?.value ?? editor.getProperty('value'))));
    editor.on(Message.KEY, event => {
        if (selected?.tableObject && event.detail?.pressed && (event.detail.key === 'Tab' || event.detail.keyCode === 23)) {
            const backwards = Boolean(event.detail.modifiers & 1);
            event.preventDefault();
            void flushText().then(ok => { if (ok) canvas.nextTableCell(backwards); }); return;
        }
        if (event.detail?.pressed && (event.detail.key === "Escape" || event.detail.keyCode === 9) && !busy) {
            cancelTextCommit(); draft = null; showSelected(selected); event.preventDefault();
        }
    });
    async function openPendingGrant() {
        if (closed || busy || closeDecision || !pendingGrant) return;
        const { file, extra } = pendingGrant;
        pendingGrant = null;
        await presenter?.stop();
        if (draft !== null) {
            status.text = "Finish or discard the text draft before opening another presentation.";
            return;
        }
        await perform(async () => {
            const result = await port.documents.openGranted(file);
            if (closed || !result) return;
            initialBlankRevision = null;
            if (result.name) documentName = result.name;
            slides.reset(); revision = null; projection = null; selected = null; slideIndex = 0; startText = 0; selectedSlideId = null; uncertain = false;
            await read();
            if (result.startShow) await beginShow(0);
            if (!closed && extra) status.text += " Only the first selected file was opened; open other presentations separately.";
        });
    }
    const stopOpen = app.on(Message.APP_OPEN, event => {
        const args = event.arguments || [];
        const index = args.indexOf("--rice-file-grants");
        if (index !== -1) {
            try {
                const files = JSON.parse(args[index + 1]);
                if (!Array.isArray(files) || files.length < 1 || files.length > 128 || files.some(file =>
                    !file || typeof file.handle !== "string" || !file.handle || typeof file.name !== "string" || file.writable !== false)) {
                    throw new TypeError("Invalid launch file descriptors");
                }
                if (typeof port?.documents?.openGranted !== "function") throw new Error("Launch-file opening is unavailable");
                if (pendingGrant) throw new Error("A launch-file request is already pending");
                pendingGrant = { file: files[0], extra: files.length > 1 };
                queueMicrotask(openPendingGrant);
            } catch (error) { status.text = `Cannot open launch file: ${String(error.message).slice(0, 240)}`; }
        } else if (event.paths?.length || args.length) {
            status.text = "Use File → Open to grant access to a presentation. Raw launch paths do not grant file access.";
        }
    });
    function waitForIdle() {
        return busy ? new Promise(resolve => idleWaiters.add(resolve)) : Promise.resolve(thumbnailWork);
    }
    const stopRequest = app.on(Message.APP_CLOSE_REQUEST, () => {
        if (closeApproved) return true;
        if (closeDecision) return closeDecision;
        if (!busy && draft === null && !notes.dirty && !projection?.dirty && !uncertain) return true;
        cancelTextCommit();
        const message = new Label({ name: 'close-save-message', text: `Save changes to ${documentName || 'this presentation'} before closing?` });
        const error = new Label({ name: 'close-save-error', text: '' });
        const save = new Button({ name: 'close-save', text: 'Save', icon: 12, state: State.EXPANSIVE, accent: true, enabled: typeof port?.documents?.save === 'function' });
        const discard = new Button({ name: 'close-discard', text: "Don't Save", icon: 25, state: State.EXPANSIVE });
        const cancel = new Button({ name: 'close-cancel', text: 'Cancel', icon: 54, state: State.EXPANSIVE });
        let choosing = false, settled = false;
        const pending = new Promise(resolve => {
            finishCloseDecision = approved => {
                if (settled) return;
                settled = true;
                closeApproved = approved;
                if (closeDialog) root.remove(closeDialog, { destroy: true });
                closeDialog = null; closeDecision = null; finishCloseDecision = null;
                if (!closed) {
                    updateEnabled();
                    if (!approved) { scheduleTextCommit(); if (pendingGrant) queueMicrotask(openPendingGrant); }
                }
                resolve(approved);
            };
        });
        const choose = async action => {
            if (choosing || settled) return;
            choosing = true; save.setProperty('enabled', false); discard.setProperty('enabled', false);
            error.text = busy ? 'Finishing the current operation…' : '';
            try {
                await waitForIdle();
                if (settled || closed) return;
                // The document operation resolves before canvas creation hands
                // buffered keystrokes to the editor. Save includes that handoff.
                await canvas.flushPendingTextInput();
                if (settled || closed) return;
                if (action === 'discard') { cancelTextCommit(); finishCloseDecision?.(true); return; }
                if (!(await flushText())) {
                    error.text = 'The latest text could not be applied. Cancel and review the document before saving.';
                    return;
                }
                if (settled || closed) return;
                const result = await perform(async () => {
                    const result = await port.documents.save();
                    if (result?.saved && !closed) { if (result.name) documentName = result.name; await read(); }
                    return result;
                });
                if (settled || closed) return;
                if (result?.saved && draft === null && !projection?.dirty && !uncertain) finishCloseDecision?.(true);
                else if (!uncertain && !result) finishCloseDecision?.(false); // Save chooser cancelled.
                else error.text = uncertain ? status.text : 'The presentation was not saved. You can cancel closing or choose Don’t Save.';
            } catch (cause) {
                if (!settled && !closed) error.text = `Could not save: ${String(cause?.message || cause).slice(0, 240)}`;
            } finally {
                if (!settled && !closed) {
                    choosing = false; save.setProperty('enabled', typeof port?.documents?.save === 'function'); discard.setProperty('enabled', true);
                }
            }
        };
        save.on(Message.ACTIVATE, () => { void choose('save'); });
        discard.on(Message.ACTIVATE, () => { void choose('discard'); });
        cancel.on(Message.ACTIVATE, () => finishCloseDecision?.(false));
        closeDialog = new Dialog({ name: 'close-save-dialog', text: 'Unsaved presentation', modal: true }, [message, error,
            new Row({ text: 'panel:fixed-height:64:gap=8:flush' }, [save, discard, cancel])]);
        closeDialog.on(Message.KEY, event => { if (event.detail?.pressed && event.detail.keyCode === 9) { event.preventDefault(); finishCloseDecision?.(false); } });
        closeDecision = pending; root.append(closeDialog); updateEnabled();
        return pending;
    });
    const stopCopilot=exposeAppCopilot({bus,instanceId,
        context:async query=>{
            if(!busy&&draft!==null)await applyDraft();
            if(!busy&&notes?.dirty)await notes.flush();
            let extra=null;
            if(query&&!busy){
                const operation=query.operation||'outline',args=query.operation?query.arguments||{}:query;
                if(!['outline','scene','speaker-notes'].includes(operation))throw Error('Deck queries support outline, scene and speaker-notes');
                const method=operation==='speaker-notes'?'speakerNotes':operation;
                if(typeof port?.[method]!=='function')throw Error('This Deck query is unavailable');
                extra=await port[method](args,revision);
            }
            // A targeted read must not duplicate the selected slide, controls and authoring catalog.
            if(query)return {app:'org.ricewm.deck',revision:revision??'0',ready:!closed&&!busy&&!uncertain&&!closeDecision&&!closeApproved&&draft===null&&!notes?.dirty&&!presenter?.active,name:documentName,queryResult:extra?.result||null,outline:(!query.operation||query.operation==='outline')?extra?.result||null:null};
            const speakerNotes=!busy&&projection?.slide&&port?.speakerNotes?await port.speakerNotes({slide:projection.slide.id},revision):null;
            return {app:'org.ricewm.deck',revision:revision??'0',ready:!closed&&!busy&&!uncertain&&!closeDecision&&!closeApproved&&draft===null&&!notes?.dirty&&!presenter?.active,
                name:documentName,projection,scene:renderedScene,selection:canvas.selected,speakerNotes:speakerNotes?.result||null,outline:(!query?.operation||query.operation==='outline')?extra?.result||null:null,queryResult:extra?.result||null,queries:{outline:{operation:'outline',arguments:{startSlide:0,limit:32}},scene:{operation:'scene',arguments:{slide:0,startObject:0,limit:32}},speakerNotes:{operation:'speaker-notes',arguments:{slide:projection?.slide?.id}}},actions:deckCopilotActions()};
        },
        validate:validateDeckCopilot,
        act:async(operation,data,expectedRevision)=>{
            const validation=validateDeckCopilot(operation,data,expectedRevision);
            if(!validation.valid)return {...validation,applied:false};
            let applied=false,result;
            await perform(async()=>{
                if(revision!==expectedRevision)throw Error('Presentation changed before the AI edit');
                result=operation==='compose'?await composeDeck(port,data,{expectedRevision,onProgress:p=>{status.text=`Creating slides · ${p.completed} of ${p.total}`;},onDraft:async()=>{selectedSlideId=null;slideIndex=0;await read();}}):operation==='apply'?await port.apply(data.commands):operation==='undo'?await port.undo():await port.redo();
                if(operation==='apply')canvas.created(result);
                await read();applied=true;
            });
            if(!applied)throw Error(status.text||'Presentation action was not applied');
            return operation==='compose'?{applied,revision,...result}:{applied,revision,projection,result};
        }});
    const stopClose = app.on(Message.APP_CLOSE, () => {
        if (closed) return;
        closed = true; transitionPreview.cancel(); animationPreview.cancel(); animations.close(); transitions.close(); cancelTextCommit(); canvas.cancel(); finishCloseDecision?.(false);
        for (const resolve of idleWaiters) resolve(); idleWaiters.clear();
        stopCopilot(); notes.close(); closeAbout(); contextMenu?.destroy(); void presenter?.close(); slides.close(); finishDiscard(false); stopOpen(); stopRequest(); stopClose();
        try { Promise.resolve(port?.close()).catch(() => log?.warn("Deck engine cleanup failed")); }
        catch { log?.warn("Deck engine cleanup failed"); }
    });
    updateEnabled();
    if (port) void perform(async () => {
        await read();
        if (closed || pendingGrant || projection?.loaded || projection?.slideCount || typeof port.documents?.new !== 'function') return;
        const result = await port.documents.new();
        if (closed || !result) return;
        documentName = result.name || 'Presentation.pptx';
        await read();
        initialBlankRevision = revision;
        canvas.select([]);
    });
}
