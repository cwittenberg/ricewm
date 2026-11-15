import {deckCopilotContext} from './copilot-context.mjs';
import {deckDesignCatalog} from './design-presets.mjs';
import {composeDeck} from './copilot-compose.mjs';
import {exposeAppCopilot} from '@ricewm/app';
import {deckCopilotActions,validateDeckCopilot,deckFileWorkflowContract,deckTextEditingContract} from './copilot-contract.mjs';
import { createSensitivityDialog, sensitivityCaption } from '@ricewm/app';
import { createHyperlinkTools } from './hyperlink-tools.mjs';
import { supportedLink, webLink } from './hyperlink-contract.mjs';
import { cellKey } from './table-values.mjs';
import {
    Button, Column, Dialog, DocumentViewport, Icons, Label, Menu, MenuBar, MenuItem, Message, Row, documentFontResource,
    ScrollView, SplitPane, StatusBar, TextEditor, SceneBuilder, SceneSurface, SceneHandleOverlay, State, ToolBar, ZoomControl, EditorWorkspace, CommandRegistry, CommandPalette,
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
import { createInlineShapeEditor } from './inline-shape-editor.mjs';
import { createWorkspaceViews } from './workspace-views.mjs';

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
    let textObjects = [], refreshingScene = false, selectedSlideId = null, documentName = null, documentGeneration = 0;
    let renderedScene = null, renderedFrames = new Map(), renderedPictures = new Map(), renderDiagnostics=[];
    let transitionPreview = null, animationPreview = null, animations = null, renderedTimeline = null;
    let initialBlankRevision = null;
    let textCommitTimer = null, textCommit = null, committingText = false;
    let clipboardTextTarget = null, clipboardTextBusy = false, clipboardScope='canvas';
    let closeDecision = null, closeDialog = null, finishCloseDecision = null, closeApproved = false;
    const idleWaiters = new Set();
    let thumbnailWork = null, notes = null, overview = false;
    let workspace = null, workspaceViews = null, currentMode = 'normal', commandSearch = null, documentTitle = null;
    let viewRequest = 0;
    const viewButtons = new Map();
    const controls = new Map();
    const shortcutControls = new WeakSet();
    const status = new Label({ name: "status-text", state: State.EXPANSIVE, text: port ? "Reading presentation…" : "Presentation engine unavailable." });
    const statusFooter=new Label({name:'deck-operation-status',state:State.EXPANSIVE,visible:false});
    const syncStatusFooter=()=>{statusFooter.text=status.text;statusFooter.setProperty('tooltip',status.text);statusFooter.visible=currentMode!=='normal'&&Boolean(status.text);};
    status.on(Message.CHANGE, event => { if (event.detail?.property === 'text'){status.setProperty('visible', Boolean(status.text));syncStatusFooter();} });
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
    let inline = null, shapeInline = null;
    const inlineEditing=()=>Boolean(inline?.active||shapeInline?.active);
    const rangeEditor=()=>inline?.rich?inline:shapeInline?.rich?shapeInline:null;
    const pendingRanges=()=>Boolean(inline?.hasPending||shapeInline?.hasPending);
    const discardRanges=()=>{inline?.discard();shapeInline?.discard();};
    let tools = null, properties = null, sceneReady = false, presenter = null, mainWindow = null;
    function cellTarget() {
        const active = canvas.selectedTableCell;
        return active ? { id: cellKey(active.object.id, active.row, active.column), slideId: renderedScene.slideId,
            tableObject: active.object.id, row: active.row, column: active.column, index: 0,
            text: active.cell.text?.text || '', editable: active.cell.editable } : null;
    }
    const canvas = createCanvasEditor({ surface, openHyperlink,selectionTheme:()=>app.theme,
        editTextStyle: (style,options) => rangeEditor() ? formatInlineText(style,options) : null,
        editParagraphFormat:format=>rangeEditor()?formatInlineParagraph(format):null,
        apply: commands => perform(async () => {
            const reply = await port.apply(commands); canvas.created(reply); await read(); return reply;
        }),
        beforePointer: () => {clipboardScope='canvas';animationPreview?.cancel(); transitionPreview?.cancel(); return inlineEditing() ? finishCellEditing() : null; },
        renderPreview: port?.renderText ? scene => port.renderText(scene) : null,
        beginTableInput: ({bounds,size,columns,rows}) => inline.beginPending(bounds,size,columns,rows),
        tableInputKey: key => inline.input([key]),
        changed: objects => {
            if (!refreshingScene) animationPreview?.cancel();
            if (!inlineEditing()) clipboardTextTarget = null;
            tools?.setState({ selected: objects });
            if (!refreshingScene && draft === null) {
                const target = cellTarget() || (objects.length === 1 ? textObjects.find(item => item.id === objects[0].id) : null);
                if (target?.id !== selected?.id || target?.text !== selected?.text) showSelected(target || null);
            }
            updateEnabled();
        },
        editText: async (object, keys = [], pointer = null) => {
          try {
            if (closed) return;
            const target = object?.tableCell ? cellTarget() : textObjects.find(item => item.id === object?.id);
            if (!target || (draft !== null && target.id !== selected?.id)) return;
            if (target.id !== selected?.id) showSelected(target);
            if(target.tableObject) {
                const preserve=inline.active && (inline.pending || inline.key===target.id);
                if(!await inline.begin(canvas.selectedTableCell,renderedScene.size,{pointer,preserve,value:target.text,scene:renderedScene})) {
                    canvas.setEditingCell(null);status.text='This cell’s imported formatting cannot be edited in place.';return;
                }
                canvas.setEditingCell(target.id);clipboardTextTarget=inline.editor;updateEnabled();inline.input(keys);installShowShortcuts(inline.overlay);return;
            }
            const preserve=shapeInline.active&&shapeInline.key===target.id;
            if(await shapeInline.begin(object,renderedScene,{pointer,keys,preserve,value:target.text})) {
                if(closed||!shapeInline.active)return;
                canvas.setEditingTextObject(target.id);clipboardTextTarget=shapeInline.editor;
                updateEnabled();installShowShortcuts(shapeInline.overlay);return;
            }
            // Unsupported affine/paint/rich-layout combinations retain their
            // actual source. The existing inspector remains a visible editor.
            showFormatPane('selection');properties.selectTab('text');
            status.text='Edit this text in Text Options; its imported layout is retained.';
            surface.setProperty("focused", false);clipboardTextTarget = editor;
            editor.setProperty("focused", true);for (const key of keys) editor.sendMessage(Message.KEY, key);
          } catch(error) { status.text=`Text editing unavailable: ${String(error.message).slice(0,180)}.`; }
        },
        notice: text => { status.text = text; },
    });
    inline=createInlineTableEditor({surface,viewport,
        renderText:port?.renderText?(scene,options)=>port.renderText(scene,options):null,
        selectionChanged:()=>updateEnabled(),notice:text=>{status.text=text;},
        change:(value,pending)=>{if(!pending)changedText(value);},
        navigate:moveCell,
        exit:finishCellEditing,
    });
    shapeInline=createInlineShapeEditor({surface,viewport,stack:inline.stack,
        onParagraphFormat:format=>formatInlineParagraph(format),
        renderText:port?.renderText ? (scene,options)=>port.renderText(scene,options) : null,
        selectionChanged:()=>updateEnabled(),
        change:changedText,exit:async()=>{
            if(!await flushText())return false;
            canvas.setEditingTextObject(null);return true;
        },notice:text=>{status.text=text;}});
    const selectionHandles=inline.stack.append(new SceneHandleOverlay({name:'selection-handle-inputs',surface,onPointer:(_handle,event)=>canvas.handleOverlayPointer(event)}));
    canvas.setHandleOverlay(selectionHandles);
    viewport.onViewChanged(()=>canvas.setViewScale(viewport.dimensions().scale));
    const stopCanvasTheme=app.on(Message.THEME_CHANGED,()=>canvas.repaint());
    async function finishCellEditing() {
        if(!inlineEditing())return true;
        if(inline.pending)await canvas.flushPendingTextInput();
        if(!await flushText())return false;
        inline.close();shapeInline.close();canvas.setEditingCell(null);canvas.setEditingTextObject(null);surface.setProperty('focused',true);return true;
    }
    function refreshShapeEditing(object,scene) {
        if(!shapeInline.active||shapeInline.refresh(object,scene))return;
        const value=shapeInline.value;shapeInline.close();canvas.setEditingTextObject(null);
        if(draft!==null){editor.value=value;showFormatPane('selection');properties.selectTab('text');editor.setProperty('focused',true);clipboardTextTarget=editor;}
    }
    async function moveCell(backwards) {
        if(inline.pending)await canvas.flushPendingTextInput();
        if(!await flushText())return false;
        const active=canvas.selectedTableCell;if(!active)return false;
        const cells=active.object.table.cells.filter(cell=>!cell.covered);
        const index=cells.findIndex(cell=>cell.row===active.row&&cell.column===active.column);
        let next=cells[index+(backwards?-1:1)];
        if(!next && !backwards && active.object.table.canEditStructure && active.object.table.rows<128) {
            const row=active.object.table.rows;
            await canvas.tableStructure('insert-table-rows',row,1);
            if(closed||uncertain||canvas.selectedTableCell?.object.table.rows<=row)return false;
            next={row,column:0};
        }
        if(!next) {inline.selectAll();inline.editor.setProperty('focused',true);return false;}
        if(!canvas.selectTableCell(active.object.id,next.row,next.column))return false;
        await canvas.beginText();if(inline.active)inline.selectAll();return inline.active;
    }
    const slides = createSlideNavigation({ port, dialogRoot: root, navigate: (index, id) => navigate(index, 0, id),
        formatBackground: async target => {
            const document = documentGeneration;
            await navigate(target.index, 0, target.slide);
            if (closed || document !== documentGeneration || renderedScene?.slideId !== target.slide) return;
            slides.selectOnly(target.slide);
            design.openBackground();
        },
        clipboardScope:()=>{clipboardScope='slides';clipboardTextTarget=null;if(tools)updateEnabled();},
        clipboard:{...(port?.copySlides?{copy:()=>slideClipboardAction('copy'),cut:()=>slideClipboardAction('cut')}:{}),
            ...(port?.pasteSlides?{paste:()=>slideClipboardAction('paste')}:{}),can:slideClipboardAvailable},
        openSlide:async(index,id)=>{await navigate(index,0,id);await switchView('normal');},
        selectionChanged: () => { if (tools) updateEnabled(); },
        mutate: (type, target) => mutateSlide(type, 'title-content', target),
        previewWork: scheduleThumbnailWork });
    const sorterZoomControl=new ZoomControl({name:'sorter-zoom',viewport:slides.sorterZoom,min:50,max:300,compact:true,label:'Sorter zoom percent',visible:false});
    const slideActions = [...slides.toolbar.children];
    for (const control of slideActions) slides.toolbar.remove(control);
    function showFormatPane(kind = 'selection') {
        if (!workspace) return;
        animations.close();
        workspace.find('properties-scroll').visible = kind === 'selection';
        workspace.find('background-properties-scroll').visible = kind === 'background';
        workspace.setInspectorVisible(true);
    }
    const design = createDesignTools({ root, openBackground: () => showFormatPane('background'), closeBackground: () => workspace?.setInspectorVisible(false),
        apply: async commands => {
            let accepted = false;
            await perform(async () => { await port.apply(commands); await read(); accepted = true; });
            if (!accepted) throw Error(status.text || 'Slide design could not be applied');
            return true;
        },
        importTheme:port?.importTheme?options=>perform(async()=>{const result=await port.importTheme(options);if(result)await read();return result;}):null,
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
        onPaneVisibility: visible => {
            if (!workspace) return;
            workspace.find('properties-scroll').visible = !visible;
            workspace.find('background-properties-scroll').visible = false;
            if (visible) workspace.setInspectorVisible(true);
        },
        notice: text => { status.text = text; },
    });
    const layouts = createLayoutTools({ onApply: layout => perform(async () => {
        if (!renderedScene?.slideId) return;
        await port.apply([{ type: 'set-slide-layout', slide: renderedScene.slideId, layout }]); await read();
    }), onInsert: layout => mutateSlide('insert-slide', layout), notice: text => { status.text = text; } });
    const insertLayouts = createLayoutTools({ namePrefix: 'insert-', onInsert: layout => mutateSlide('insert-slide', layout), notice: text => { status.text = text; } });
    const showControls = [action('show-start', 'From beginning', () => perform(() => beginShow(0))), action('show-current', 'From current slide', () => perform(() => beginShow()))];
    const viewControls = [['normal', 'Normal', 40], ['outline', 'Outline', 140], ['sorter', 'Slide Sorter', 41], ['notesPage', 'Notes Page', 140], ['reading', 'Reading View', 82]].map(([mode, text, icon]) => {
        const button = new Button({ name: `view-${mode}`, text, icon, state: State.START_ALIGNED });
        button.on(Message.ACTIVATE, () => switchView(mode)); viewButtons.set(mode, [button]); return button;
    });
    showControls.forEach(control => control.setProperty('state', State.START_ALIGNED));
    for (const control of [editor, ...inline.editors, ...shapeInline.editors, notes.editor]) {
        control.on(Message.FOCUS, () => { clipboardTextTarget = control; updateEnabled(); });
        control.on(Message.TEXT_SELECTION, updateEnabled);
        // TextEditorController applies keyboard selections in windowProc
        // before listeners run; unlike pointer selection it emits no separate
        // TEXT_SELECTION event. Keep ribbon Copy/Cut ready for Ctrl+A and
        // Shift+navigation, and disable them again when selection collapses.
        control.on(Message.KEY, event => { if (event.detail?.pressed) updateEnabled(); });
    }
    surface.on(Message.FOCUS, () => {clipboardScope='canvas';if (!inlineEditing()) clipboardTextTarget = null; });
    hyperlinks = createHyperlinkTools({ root, open: openHyperlink, slides: () => slides.hyperlinkSlides,
        apply: commands => perform(async () => { await port.apply(commands); await read(); }), notice: text => { status.text = text; } });
    tools = createAuthoringTools({paragraphSelection:()=>{
        const owner=rangeEditor();return owner?{enabled:owner.paragraphFormatting,target:owner.rangeTarget,range:owner.range,paragraphs:owner.paragraphs().map(p=>p.paragraphFormat).filter(Boolean)}:null;
    },restoreTextFocus:()=>{const owner=rangeEditor();if(owner?.active)owner.editor.setProperty('focused',true);},textStyleSelection:()=>{
        const styles=rangeEditor()?.styles();return styles?[{text:{editable:true,runs:styles.map(style=>({text:'selected',style}))}}]:null;
    }, hyperlinkControls: hyperlinks.controls, root, canvas, viewControls, openInspector: () => showFormatPane(), shadowsSupported: Boolean(BigInt(app.hostCapabilities || 0) & (1n << 27n)), slideActions, showControls, layoutControls: [layouts.newSlideMenu, layouts.layoutMenu, slides.sectionMenu], insertSlideControl: insertLayouts.newSlideMenu, designControls: design.controls, designGroups:design.groups, transitionControls: transitions.controls, animationControls: animations.controls,
        clipboardActions: {
            ...(port?.copySlides || port?.copySelection || port?.writeClipboardText ? { copy: () => clipboardAction('copy'), cut: () => clipboardAction('cut') } : {}),
            ...(port?.pasteSlides || port?.pasteSelection || port?.readClipboardText ? { paste: () => clipboardAction('paste') } : {}),
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
        const operationBusy=busy&&!committingText||shapeInline?.formatting||inline?.formatting;
        authoring.setProperty('enabled', closeDecision === null && !closeApproved);
        const showing = Boolean(presenter?.active), idleDraft = (draft === null || inlineEditing()) && !showing && currentMode !== 'reading';
        const usable = projection && !uncertain;
        enable("new", idleDraft && typeof port?.documents?.new === "function");
        enable("open", idleDraft && typeof port?.documents?.open === "function");
        enable('open-template', idleDraft && typeof port?.documents?.['open-template'] === 'function');
        enable('exit', !closeDecision && !closeApproved && Boolean(mainWindow));
        for (const name of ["save", "save-as"]) enable(name, !showing &&
            (draft === null || (selected?.editable && fitsEditor(draft))) &&
            (projection?.loaded ?? Boolean(projection?.slideCount)) && usable && typeof port?.documents?.[name] === "function");
        for (const name of ['show-start', 'show-current', 'status-show-current', 'deck-present']) enable(name, idleDraft && usable && sceneReady && projection.slideCount > 0);
        enable("undo", idleDraft && usable && (projection.canUndo || notes?.dirty || inlineEditing()&&draft!==null));
        enable("redo", idleDraft && usable && draft===null && projection.canRedo);
        enable('notes-toggle', !showing && usable && projection?.slide && typeof port?.speakerNotes === 'function' && currentMode !== 'notesPage');
        enable("previous-text", idleDraft && usable && sceneReady && startText > 0);
        enable("next-text", idleDraft && usable && sceneReady && startText + PAGE < textObjects.length);
        enable("apply", usable && sceneReady && draft !== null && selected?.editable && fitsEditor(draft));
        enable("discard", draft !== null);
        enable("refresh", Boolean(port) && !showing);
        slides.setState({ disabled: closed || (operationBusy && !notes?.committing) || !idleDraft || uncertain || Boolean(closeDecision) || closeApproved, loaded: projection?.loaded ?? Boolean(projection?.slideCount), index: slideIndex, count: projection?.slideCount || 0 });
        contextMenu?.setState({ scope: [documentGeneration, renderedScene?.slideId ?? null], ready: Boolean(usable && sceneReady), busy: closed || operationBusy || !idleDraft || uncertain, selected: canvas.selected,
            canUndo: Boolean(projection?.canUndo), canRedo: Boolean(projection?.canRedo), background: renderedScene?.background,
            editingText:Boolean(rangeEditor()?.active),paragraphFormatting:Boolean(rangeEditor()?.paragraphFormatting||canvas.selected.some(o=>o.text?.paragraphFormatEditable)) });
        hyperlinks?.setState({ ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), scene: renderedScene, revision, selected: canvas.selected });
        canvas.setLocked(closed || operationBusy || (!idleDraft && !inlineEditing()) || uncertain || !sceneReady || currentMode === 'notesPage');
        tools?.setState({ scope: [documentGeneration, renderedScene?.slideId ?? null], ready: Boolean(usable && sceneReady), busy: closed || operationBusy || !idleDraft || uncertain,
            theme:renderedScene?.backgroundTheme??null,clipboardReady: Boolean(usable && (sceneReady||projection?.loaded) && !closed && !operationBusy && !uncertain) });
        properties?.setState({ scope: documentGeneration, objects: canvas.selected, ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft),
            busy: closed || operationBusy || !idleDraft || uncertain, scene: renderedScene, editingText: draft !== null });
        design.setState({ current: renderedScene, ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), scope: documentGeneration });
        transitions.setState({ current: renderedScene, selectedSlides: slides.selectedSlides,
            ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft) });
        animations?.setState({ current: renderedScene, timeline: renderedTimeline, selected: canvas.selected,
            ready: Boolean(usable && sceneReady && !closed && !operationBusy && idleDraft), previewing: animationPreview?.active === true });
        layouts.setState({ ready: Boolean(usable && (projection?.loaded ?? projection?.slideCount)), canApply: sceneReady, busy: closed || operationBusy || !idleDraft || uncertain, selectedLayout: renderedScene?.layoutId || null, pageSize: renderedScene?.size });
        insertLayouts.setState({ ready: Boolean(usable && (projection?.loaded ?? projection?.slideCount)), canApply: false, busy: closed || operationBusy || !idleDraft || uncertain, selectedLayout: renderedScene?.layoutId || null, pageSize: renderedScene?.size });
        const editable = Boolean(currentMode !== 'reading' && !showing && !closed && (!busy || committingText || clipboardTextBusy) && !uncertain && sceneReady && selected?.editable && fitsEditor(selected.text));
        editor.setProperty("enabled", editable);
        editor.setProperty("readOnly", !editable);
        inline?.editor.setProperty('enabled',Boolean(inline.active && !closed && !uncertain && (inline.pending || (!operationBusy||clipboardTextBusy||inline.formatting)&&selected?.tableObject && selected.editable)));
        inline?.editor.setProperty('readOnly',Boolean(closed || uncertain));
        shapeInline?.editor.setProperty('enabled',Boolean(shapeInline.active && !closed && !uncertain && (!operationBusy||clipboardTextBusy||shapeInline.formatting) && selected?.editable));
        shapeInline?.editor.setProperty('readOnly',Boolean(closed || uncertain));
        notes?.setState({ enabled: Boolean(currentMode !== 'reading' && !closed && (!busy || notes.committing || clipboardTextBusy) && !uncertain && !showing && !closeApproved && projection?.slide),
            visible: !overview && !showing });
        for (const [mode, buttons] of viewButtons) for (const button of buttons) button.enabled = Boolean(!closed && (!operationBusy || notes?.committing) && !showing && !uncertain && projection?.slideCount && (mode !== 'outline' || port?.outline) && (mode !== 'notesPage' || port?.speakerNotes));
        for (const button of runs.children) button.setProperty("enabled", !closed && !operationBusy && idleDraft && !uncertain && sceneReady);
    }
    function showSelected(target) {
        // A hidden editor can be recreated while native creation/promotion is
        // still pending. Its previous object's focus request must not steal
        // canvas keys before editText finishes replaying the buffered input.
        if (target?.id !== selected?.id || target?.slideId !== selected?.slideId) editor.setProperty('focused', false);
        if(shapeInline?.active&&(target?.id!==shapeInline.key||target?.slideId!==selected?.slideId)){
            shapeInline.close();canvas.setEditingTextObject(null);
        }
        if(inline?.active&&!inline.pending&&!inline.waiting&&target?.id!==inline.key){inline.close();canvas.setEditingCell(null);}
        selected = target;
        const fits = target && fitsEditor(target.text);
        editor.setProperty("value", target ? (fits ? target.text : "This text box exceeds the native editor size limit; its source is preserved.") : "");
        runLabel.text = target ? `${target.tableObject ? `Cell ${target.row + 1}, ${target.column + 1}` : `Text box ${target.index + 1}`}${target.editable && fits ? "" : " · Read only"}` : "Select a text box";
        const table = Boolean(target?.tableObject);
        if(table && inline?.pending) {
            const value=inline.value;
            inline.begin(canvas.selectedTableCell,renderedScene.size,{value,preserve:true,scene:renderedScene});
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
            if(same?.tableObject)inline.refresh(canvas.selectedTableCell,scene.size,scene);
            else if(same&&shapeInline.active)refreshShapeEditing(scene.objects.find(o=>o.id===same.id),scene);
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
            inline.refresh(canvas.selectedTableCell,scene.size,scene);
            if(!preserveDraft && draft===null && inline.value!==target.text)inline.editor.setProperty('value',target.text);
        }
        if(target&&!target.tableObject&&shapeInline.active){
            refreshShapeEditing(scene.objects.find(o=>o.id===target.id),scene);
            if(!preserveDraft&&draft===null&&shapeInline.value!==target.text)shapeInline.editor.value=target.text;
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
        if (documentTitle) documentTitle.text = `${documentName || 'Presentation'}${projection?.dirty || notes?.dirty ? ' — Edited' : ''}`;
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
        if (workspaceViews) await workspaceViews.refresh({ revision, selectedId: selectedSlideId, slides: slides.outlineItems,
            frame: currentMode === 'notesPage' && renderedScene ? geometryFrame(renderedScene, { textFrames: renderedFrames, imageFrames: renderedPictures }).frame : null });
    }
    async function readScene(preserveDraft) {
        try {
            const scene = await loadDeckScene(port, { index: slideIndex, revision, cancelled: () => closed });
            let text = port.renderText ? await port.renderText(scene, { placeholderPrompts: true }) : { frames: new Map(), notices: [] };
            if(port.renderText&&text.diagnostics?.some(item=>item.message))
                text=await port.renderText(scene,{placeholderPrompts:true});
            const pictures = port.renderImages ? await port.renderImages(scene) : { frames: new Map(), notices: [] };
            const timeline = await loadAnimationTimeline(port, { scene, revision, cancelled: () => closed });
            const catalog = scene.animationTimeline?.editable && port.animationCatalog ? await port.animationCatalog(revision) : null;
            if (closed) return;
            animations.setCatalog(catalog?.result.effects || []);
            renderedTimeline = timeline;
            sceneReady = true; renderedScene = scene; renderedFrames = text.frames; renderedPictures = pictures.frames;
            renderDiagnostics=[...(text.diagnostics||[]),...(pictures.diagnostics||[])].slice(0,8);
            refreshingScene = true;
            try { canvas.setScene(scene, text.frames, pictures.frames); } finally { refreshingScene = false; }
            presentText(scene, preserveDraft); surface.setProperty('visible', true);
            const textFailed=(text.diagnostics||[]).some(item=>item.message);
            sceneNotice.text = textFailed ? 'Some text could not be displayed. Try another font or reopen the presentation.' : '';
            sceneNotice.setProperty('visible', textFailed);
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
        // Thumbnail and inspection reads share the same source lane. A user
        // operation claims busy before awaiting the entire lane, cancelling
        // queued reads and draining the one native request already in flight.
        const pending = Promise.resolve(thumbnailWork).catch(()=>{}).then(()=>{
            if(closed||busy||uncertain||draft!==null||closeDecision||closeApproved||presenter?.active)return null;
            return work();
        });
        thumbnailWork = pending;
        return pending.finally(() => { if (thumbnailWork === pending) thumbnailWork = null; });
    }
    async function perform(operation, flushNotes = true, retainPresentation = false, recoverRejected = false, textCommitOperation = false) {
        transitionPreview?.cancel(); animationPreview?.cancel();
        if (closed || !port) return;
        // Toolbar gestures wait for typing already being committed. Only the
        // text commit itself skips this barrier, so it cannot await itself.
        if(inlineEditing() && !textCommitOperation && !await flushText())return;
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
                if(!recoverRejected||error.rejectedBeforeMutation!==true)uncertain = true;
                status.text = `Operation failed: ${String(error?.message || error).slice(0, 240)}. ${uncertain?'Refresh before retrying; your draft is retained.':'Your document is unchanged.'}`;
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
        if (!await finishCellEditing() || draft !== null || uncertain || !port?.outline) return false;
        return await perform(async () => {
            const command = await slides.command(type, target?.slide ?? selectedSlideId, target?.index ?? slideIndex, projection.slideCount, revision, target);
            if (type === 'insert-slide') command.layout = layout;
            const reply = await port.apply([command]);
            const created = reply?.result?.createdSlides;
            if (Array.isArray(created) && created.length) selectedSlideId = created.at(-1);
            else if (target?.slide) { selectedSlideId = target.slide; slideIndex = target.index; }
            selected = null; startText = 0;
            await read();
            if(Array.isArray(created)&&created.length)slides.selectSlides(created);
            if(command.type==='move-slide')slides.selectOnly(command.slide);
            return true;
        }, true, false, true)===true;
    }
    function clipboardEditor() {
        const shown=target=>{
            if(!target?.getProperty('enabled'))return false;
            for(let owner=target;owner;owner=owner.parent)if(!owner.getProperty('visible'))return false;
            return true;
        };
        const candidates=[shapeInline?.active?shapeInline.editor:null,inline?.active?inline.editor:null,notes?.editor,editor];
        const focused=candidates.find(target=>shown(target)&&target.getProperty('focused'));
        if(focused){clipboardTextTarget=focused;return focused;}
        return shown(clipboardTextTarget)?clipboardTextTarget:null;
    }
    function textEditingContext() {
        const control=clipboardEditor();
        const font=control?.getProperty('documentFont');
        const kind=!control?'none':control===shapeInline.editor?'shape':control===inline.editor?'table-cell':control===notes.editor?'speaker-notes':'inspector';
        return {contract:deckTextEditingContract,kind,object:kind==='none'||kind==='speaker-notes'?null:selected?.tableObject||selected?.id||null,
            slide:projection?.slide?.id||null,selection:control?.selection||null,
            font:font?documentFontResource(font.family,font.weight,font.italic):null,
            mode:rangeEditor()&&(kind==='shape'||kind==='table-cell')?'source-ranges':'plain-value',
            range:kind==='shape'||kind==='table-cell'?rangeEditor()?.range||null:null,
            cell:kind==='table-cell'&&selected?.tableObject?{row:selected.row,column:selected.column}:null,
            typingStyle:rangeEditor()&&(kind==='shape'||kind==='table-cell')?rangeEditor().typingStyle:null,
            paragraphFormats:rangeEditor()&&(kind==='shape'||kind==='table-cell')?rangeEditor().paragraphs().map(p=>p.paragraphFormat??null):null,
            pendingText:draft!==null,shapeEditorOpen:shapeInline.active,tableEditorOpen:inline.active};
    }
    function clipboardAvailable(kind) {
        const target = clipboardEditor();
        if (target) {
            const range = target.selection;
            return kind === 'paste' ? Boolean(port?.readClipboardText && !target.getProperty('readOnly'))
                : Boolean(port?.writeClipboardText && range.anchor !== range.caret && (kind !== 'cut' || !target.getProperty('readOnly')));
        }
        if(clipboardScope==='slides')return slideClipboardAvailable(kind);
        return kind === 'paste' ? Boolean(port?.pasteSelection||port?.pasteSlides&&slides.clipboardSupported) : Boolean(port?.copySelection && canvas.selected.length &&
            (kind !== 'cut' || canvas.selected.every(object => object.edit?.canDelete)));
    }
    function slideClipboardAvailable(kind) {
        return slides.clipboardSupported&&(kind==='paste'?Boolean(port?.pasteSlides):Boolean(port?.copySlides&&slides.selectedSlides.length&&slides.selectedSlides.length<=32));
    }
    async function slideClipboardAction(kind) {
        if(closed||uncertain||!slideClipboardAvailable(kind)||!await flushText())return true;
        if((notes.dirty||notes.committing)&&!await notes.flush())return true;
        if(closed||busy||uncertain)return true;
        const ids=[...slides.selectedSlides],titles=slides.clipboardTitles;
        busy=true;updateEnabled();let dispatched=false;
        try {
            await thumbnailWork;if(closed)return true;
            let reply;
            if(kind==='paste') {
                dispatched=true;reply=await port.pasteSlides({after:projection?.slide?.id??null});
                if(reply===null)return false;
            } else {
                dispatched=kind==='cut';reply=await port.copySlides({slides:ids,text:new TextEncoder().encode(titles).length<=16000?titles:'Selected slides',cut:kind==='cut'});
                if(kind==='copy'){status.text=`Copied ${ids.length} slide${ids.length===1?'':'s'}`;return true;}
            }
            if(closed)return true;
            const created=reply?.result?.createdSlides;
            if(created?.length)selectedSlideId=created.at(-1);
            selected=null;startText=0;await read();
            if(created?.length)slides.selectSlides(created);
            clipboardScope='slides';return true;
        } catch(error) {
            if(dispatched&&error.rejectedBeforeMutation!==true)uncertain=true;
            status.text=`Cannot ${kind} slides: ${String(error?.message||error).slice(0,240)}`;return true;
        } finally {
            busy=false;
            if(!closed){updateEnabled();for(const resolve of idleWaiters)resolve();idleWaiters.clear();if(pendingGrant&&!closeDecision)queueMicrotask(openPendingGrant);}
        }
    }
    async function clipboardAction(kind) {
        const target = clipboardEditor();
        if (target) return textClipboardAction(kind, target);
        if(clipboardScope==='slides')return slideClipboardAction(kind);
        if(kind==='paste'&&slideClipboardAvailable(kind)&&await slideClipboardAction(kind))return;
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
        const focusOwner=shapeInline.active?shapeInline:inline.active?inline:null;
        const textFocus=focusOwner?{owner:focusOwner,editor:focusOwner.editor,key:focusOwner.key}:null;
        // File dialogs temporarily own native focus. Release the retained
        // request before disabling the source editor, then restore it after
        // the document operation has enabled the same editor again.
        textFocus?.editor.setProperty('focused',false);
        try { await perform(async () => {
            const result = await port.documents[name]();
            if (closed || !result) return;
            initialBlankRevision = null;
            if (result.name) documentName = result.name;
            if (result.replaced) { documentGeneration++; inline.close();shapeInline.close();canvas.setEditingCell(null);canvas.setEditingTextObject(null);slides.reset(); revision = null; projection = null; selected = null; slideIndex = 0; startText = 0; selectedSlideId = null; uncertain = false; }
            await read();
            if (name === 'new' || name === 'new-odp') canvas.select([]);
            if (result.startShow) await beginShow(0);
        }); } finally {
            if(textFocus&&!closed&&!uncertain&&textFocus.owner.active&&textFocus.owner.key===textFocus.key&&textFocus.owner.editor===textFocus.editor)
                textFocus.editor.setProperty('focused',true);
        }
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
        const owner=rangeEditor(),rangeBatch=owner?.capture();
        if(owner&&!rangeBatch){draft=null;return;}
        committingText = true;
        textCommit = perform(async () => {
            await port.apply(rangeBatch?rangeBatch.commands:[{ type, ...target, text }]);
            if(rangeBatch)owner.acknowledge(rangeBatch);
            // Keep the native editor and its caret intact while refreshing the
            // authoritative slide. Keystrokes during compute stay in the draft.
            await read(true);
            if (!uncertain && draft === text && !pendingRanges()) draft = null;
        },true,false,false,true);
        try { await textCommit; }
        finally {
            textCommit = null; committingText = false;
            if (!closed) { updateEnabled(); scheduleTextCommit(); }
        }
    }
    async function flushText() {
        cancelTextCommit();
        while(shapeInline?.layoutWaiting||inline?.layoutWaiting)await(shapeInline.layoutWaiting||inline.layoutWaiting);
        while (!closed && !uncertain && (draft !== null || textCommit)) {
            if (busy && !committingText) return false;
            if (draft !== null && !fitsEditor(draft)) return false;
            await applyDraft();
        }
        return !closed && !uncertain;
    }
    function formatInlineText(style,{toggle=null}={}) {
        const owner=rangeEditor();if(!owner)return Promise.resolve(false);
        return owner.format(async()=>{
            if(!await flushText()||rangeEditor()!==owner)return false;
            const range=owner.range;
            if(!range)return false;
            if(['bold','italic','underline'].includes(toggle))style={[toggle]:!owner.styles().every(style=>style[toggle]===true)};
            const {align,...characters}=style;
            if(range.start===range.end&&Object.keys(characters).length)owner.setTypingStyle(characters);
            if(range.start!==range.end||align){
                const commandStyle=range.start===range.end?{align}:style;
                await perform(async()=>{
                    await port.apply([{type:'set-text-range-style',...owner.rangeTarget,range,style:commandStyle}]);
                    await read();
                });
                while(owner.layoutWaiting)await owner.layoutWaiting;
            }
            if(owner.active)owner.editor.setProperty('focused',true);
            return !uncertain;
        });
    }
    function formatInlineParagraph(format){
        const owner=rangeEditor();if(!owner?.paragraphFormatting)return Promise.resolve(false);
        return owner.format(async()=>{
            if(!await flushText()||rangeEditor()!==owner)return false;
            const range=owner.range;if(!range)return false;
            await perform(async()=>{await port.apply([{type:'set-paragraph-format',...owner.rangeTarget,range,format}]);await read();});
            while(owner.layoutWaiting)await owner.layoutWaiting;
            if(owner.active)owner.editor.setProperty('focused',true);
            return !uncertain;
        });
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
    documentTitle = new Label({ name: 'deck-document-title', text: 'Presentation', state: State.EXPANSIVE | State.CENTER_ALIGNED });
    const presentButton = new Button({ name: 'deck-present', text: 'Present', icon: 82, accent: true });
    presentButton.on(Message.ACTIVATE, () => perform(() => beginShow())); controls.set('deck-present', presentButton);
    const searchButton = new Button({ name: 'deck-command-search', icon: Icons.Search, accessibleName: 'Search commands', tooltip: 'Search commands' });
    searchButton.on(Message.ACTIVATE, openCommandSearch);
    authoring.append(new Row({ name: 'deck-title-row', text: 'panel:office-ribbon-heading:flush:accessory-height=38:gap=4' }, [quick, documentTitle, sensitivityBadge, presentButton, searchButton]));
    tools.control.setHeader({ leading: [menus], trailing: [] });
    authoring.append(tools.control);
    const textOutline = new Column({ name: 'text-outline', visible: false }, [
        new ScrollView({ name: 'outline-scroll', text: 'viewport:180|0/0' }, [runs]),
        new Row({}, [action('previous-text', 'Previous boxes', () => pageText(Math.max(0, startText - PAGE))), action('next-text', 'More boxes', () => pageText(startText + PAGE))]),
    ]);
    const outlineToggle = new Button({ name: 'toggle-text-outline', text: 'Text boxes on this slide', icon: 74, state: State.START_ALIGNED });
    outlineToggle.on(Message.ACTIVATE, () => textOutline.setProperty('visible', !textOutline.getProperty('visible')));
    const textPanel = new Column({ name: 'text-panel', text: 'panel:tight-stack' }, [
        runLabel, editor,
        new Row({ text: "panel:actions" }, [action("apply", "Apply text", applyDraft), action("discard", "Discard draft", () => { cancelTextCommit(); discardRanges(); draft = null; showSelected(selected); })]),
        action("refresh", "Refresh", () => perform(async () => { uncertain = false; await read(true, true); }, false)),
        outlineToggle, textOutline,
    ]);
    properties = createSelectionProperties({ canvas, tools, textPanel, shadowsSupported: Boolean(BigInt(app.hostCapabilities || 0) & (1n << 27n)) });
    workspaceViews = createWorkspaceViews({ port, notes, navigate, notice: text => { status.text = text; } });
    workspace = new EditorWorkspace({ name: 'slide-workspace', navigation: slides.control,
        document: new Column({ name: 'canvas-panel', text: 'panel:flush:gap=0' }, [status, sceneNotice, viewport, notes.control]),
        inspector: new Column({ name: 'context-pane', text: 'panel:flush:gap=0', state: State.EXPANSIVE }, [new ScrollView({ name: 'properties-scroll' }, [properties.control]), new Column({ name: 'background-properties-scroll',text:'panel:flush:gap=0',state:State.EXPANSIVE, visible: false }, [design.pane]), animations.pane]),
        outline: workspaceViews.outline, notesPage: workspaceViews.notesPage,
        beforeChange: () => !closed && !uncertain && finishCellEditing(),
        onChange: async (mode, previous) => {
            currentMode = mode; overview = mode === 'sorter';syncStatusFooter();slides.setOverview(overview);
            await workspaceViews.change(mode, previous); zoom.visible = mode !== 'notesPage'&&!overview;sorterZoomControl.visible=overview;
            for (const [id, buttons] of viewButtons) for (const button of buttons) button.setProperty('selected', mode === id);
            await perform(() => read());
        },
    });
    authoring.append(workspace);
    workspace.setInspectorVisible(false);
    async function switchView(mode) {
        if (!workspace || closed || uncertain) return false;
        const request = ++viewRequest, document = documentGeneration;
        // Notes can be committing when a native view button is pressed. Keep
        // that intent and finish its draft instead of silently dropping input.
        if ((notes.dirty || notes.committing) && !await notes.flush()) return false;
        if (busy) await waitForIdle();
        if (closed || uncertain || busy || document !== documentGeneration || request !== viewRequest) return false;
        try { return await workspace.setMode(mode); } catch (error) { status.text = String(error.message); return false; }
    }
    async function setOverview(value) {
        return switchView(value ? 'sorter' : 'normal');
    }
    const viewButton = (name, icon, tooltip, callback) => {
        const button = new Button({ name, icon, tooltip, accessibleName: tooltip, text: 'panel:fixed:accessory-height=26:width=26:flush' });
        button.on(Message.ACTIVATE, callback); return button;
    };
    const normalButton = viewButton('normal-view', 40, 'Normal view', () => void setOverview(false));
    const overviewButton = viewButton('slide-overview-toggle', 41, 'Slide overview', () => void setOverview(true));
    viewButtons.get('normal').push(normalButton); viewButtons.get('sorter').push(overviewButton);
    const outlineButton = viewButton('outline-view', 140, 'Outline view', () => switchView('outline'));
    const notesPageButton = viewButton('notes-page-view', 140, 'Notes page', () => switchView('notesPage'));
    const presentCurrent = viewButton('status-show-current', 82, 'Present from current slide', () => void perform(() => beginShow()));
    controls.set('status-show-current',presentCurrent);
    viewButtons.get('outline').push(outlineButton); viewButtons.get('notesPage').push(notesPageButton);
    const inspectorButton = viewButton('format-pane-toggle', 182, 'Format pane', () => workspace.setInspectorVisible(!workspace.inspectorVisible));
    normalButton.setProperty('selected', true);
    const notesButton = new Button({ name: 'notes-toggle', text: 'Notes', icon: 140, tooltip: 'Speaker notes', accessibleName: 'Speaker notes',
        visible: Boolean(port?.speakerNotes), state: State.START_ALIGNED });
    controls.set('notes-toggle', notesButton);
    notesButton.on(Message.ACTIVATE, () => { void perform(async () => {
        if (!projection?.slide || !port.speakerNotes) return;
        const opening = !notes.opened;
        if (opening) notes.bind((await port.speakerNotes({ slide: projection.slide.id }, revision)).result);
        notes.setOpen(opening); notesButton.setProperty('selected', opening);
    }).then(() => { notes.focus(); }); });
    heading.setProperty('state', State.EXPANSIVE);
    authoring.append(new StatusBar({ name: 'deck-status-bar', text: 'panel:status-bar:accessory-height=28:flush:gap=4' }, [heading,statusFooter, notesButton, inspectorButton, normalButton, outlineButton, overviewButton, notesPageButton, presentCurrent, zoom, sorterZoomControl, workspaceViews.notesZoom]));
    function openCommandSearch() {
        if (commandSearch || closed) return;
        const found = new Map();
        function collect(control) {
            if (control.name && [12, 32].includes(control.kind) && control.listeners.has(Message.ACTIVATE) && !control.name.startsWith('ribbon-tab-') && !control.name.startsWith('slide-choice-')) {
                const label = control.getProperty('accessibleName') || control.getProperty('tooltip') || control.text;
                if (label && !label.startsWith('panel:') && !found.has(control.name)) found.set(control.name, { control, label });
            }
            for (const child of control.children) collect(child);
        }
        collect(authoring);
        const records = [...found].map(([name, value], index) => ({ id: `deck-control-${index}`, name, ...value }));
        const commands = new CommandRegistry(records.map(({ id, name, control, label }) => ({ id, label, keywords: name,
            enabled: control.enabled, run: () => { if (!control.enabled) return false; return control.sendMessage(Message.ACTIVATE); } })));
        const subscriptions = records.map(({ id, control }) => control.on(Message.CHANGE, () => commands.update(id, { enabled: control.enabled })));
        let dialog;
        const close = () => { for (const stop of subscriptions) stop(); commands.dispose(); if (dialog?.parent) root.remove(dialog, { destroy: true }); commandSearch = null; surface.setProperty('focused', true); };
        const palette = new CommandPalette({ name: 'deck-palette', commands, onClose: close });
        dialog = new Dialog({ name: 'deck-command-palette', text: 'Search commands', modal: true }, [palette]);
        commandSearch = { close }; root.append(dialog); palette.input.setProperty('focused', true);
    }
    contextMenu = createDeckContextMenu({ owner: viewport, can:name=>['copy','cut','paste'].includes(name)?clipboardAvailable(name):true, notice: text => { status.text = text; }, actions: {
        undo: () => perform(async () => { await port.undo(); await read(); }),
        redo: () => perform(async () => { await port.redo(); await read(); }),
        editLink: () => hyperlinks.edit(), openLink: () => hyperlinks.open(), removeLink: () => hyperlinks.remove(),
        editText: () => canvas.beginText(), addText: () => canvas.beginText(),
        cut:()=>clipboardAction('cut'),selectAll:()=>{const target=clipboardEditor();if(target){target.setSelection(0,Array.from(target.value).length);void restoreClipboardFocus(target);updateEnabled();}},
        bulletFormat:()=>tools.openParagraphFormat('bullet'),numberFormat:()=>tools.openParagraphFormat('number'),listFormat:()=>tools.openParagraphFormat(),
        alignLeft:()=>canvas.style({align:'left'}),alignCenter:()=>canvas.style({align:'center'}),alignRight:()=>canvas.style({align:'right'}),
        decreaseIndent:()=>canvas.paragraphFormat({levelDelta:-1}),increaseIndent:()=>canvas.paragraphFormat({levelDelta:1}),
        ...(port?.copySelection||port?.writeClipboardText ? { copy: () => clipboardAction('copy') } : {}),
        ...(port?.pasteSelection||port?.readClipboardText ? { paste: () => clipboardAction('paste') } : {}),
        ...(canvas.group ? { group: () => canvas.group() } : {}),
        ...(canvas.ungroup ? { ungroup: () => canvas.ungroup() } : {}),
        ...(canvas.reorder ? {
            bringForward: () => canvas.reorder('forward'), bringToFront: () => canvas.reorder('front'),
            sendBackward: () => canvas.reorder('backward'), sendToBack: () => canvas.reorder('back'),
        } : {}),
        duplicate: () => canvas.duplicate(), delete: () => canvas.remove(),
        positionSize: () => tools.position(), rotate: () => { showFormatPane(); properties.selectTab('size'); },
        formatShape: () => { showFormatPane(); tools.selectTab('format'); }, formatPicture: () => { showFormatPane(); properties.selectTab('picture'); tools.selectTab('picture'); },
        fill: color => canvas.fill(color), strokeColor: color => canvas.strokeColor(color), style: style => canvas.style(style),
        slideBackground: () => design.openBackground(),
        backgroundColor: color => perform(async () => { await port.apply([{ type: 'set-slide-background', slide: renderedScene.slideId, color }]); await read(); }),
    } });
    // The page and its surrounding workspace have separate native hit targets.
    // A secondary click in the gutter must not reuse a shape's old context.
    for (const owner of [viewport, viewport.clip]) owner.on(Message.POINTER, event => {
        if (event.target===owner && event.detail?.kind === 'down' && event.detail.button === 3 && sceneReady && !busy && !uncertain) canvas.select([]);
    });
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
    root.on(Message.KEY, event => {
        const key = event.detail;
        if (!key?.pressed) return;
        if (key.modifiers === 8 && (String(key.key || key.text || '').toLowerCase() === 'q' || key.keyCode === 24)) {
            event.preventDefault(); event.stopPropagation(); openCommandSearch();
        } else if (currentMode === 'reading' && (key.key === 'Escape' || key.keyCode === 9)) {
            event.preventDefault(); event.stopPropagation(); void switchView('normal');
        }
    });
    mainWindow = createWindow({ title: "Rice Deck", root });
    function showShortcut(event) {
        const key = event.detail;
        if (key?.pressed) {
            transitionPreview.cancel();
            if (event.target === surface || key.key === 'Escape' || key.keyCode === 9) animationPreview.cancel();
        }
        if (event.target !== surface && event.target !== editor && event.target !== inline.editor && event.target !== shapeInline.editor && canvas.capturePendingText(event)) return;
        if (key?.pressed && (key.key === 'Escape' || key.keyCode === 9) && canvas.formatPainter?.active && !presenter.active) {
            canvas.cancelFormatPainter(); event.preventDefault(); return;
        }
        const textHistory=key?.pressed&&(key.modifiers&4)&&!(key.modifiers&8)&&([52,29].includes(key.keyCode)||['z','y'].includes(key.key?.toLowerCase()));
        if(textHistory&&!event.defaultPrevented&&!closed&&!uncertain&&!presenter.active&&(event.target===shapeInline.editor||event.target===inline.editor)){
            event.preventDefault();
            const document=documentGeneration,redo=key.keyCode===29||key.key?.toLowerCase()==='y'||Boolean(key.modifiers&1);
            void(async()=>{if(await flushText()&&!closed&&!uncertain&&!busy&&document===documentGeneration&&projection?.[redo?'canRedo':'canUndo'])controls.get(redo?'redo':'undo')?.emit(Message.ACTIVATE);})();
            return;
        }
        const saveShortcut=key?.pressed&&(key.modifiers&4)&&!(key.modifiers&8)&&(key.keyCode===39||key.key?.toLowerCase()==='s');
        if(saveShortcut&&!event.defaultPrevented&&!closed&&!uncertain&&!presenter.active&&(!busy||committingText||notes.committing)){
            event.preventDefault();
            const document=documentGeneration,name=key.modifiers&1?'save-as':'save';
            void(async()=>{
                if(!await flushText()||closed||uncertain||document!==documentGeneration)return;
                if((notes.dirty||notes.committing)&&!await notes.flush())return;
                if(!closed&&!busy&&!uncertain&&document===documentGeneration&&controls.get(name)?.enabled)await documentAction(name);
            })().catch(error=>{if(!closed)status.text=`Could not save: ${String(error?.message||error).slice(0,240)}`;});
            return;
        }
        if (event.defaultPrevented || !key?.pressed || presenter.active || (busy && !notes.committing && !committingText) || closed || uncertain) return;
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
        if (draft !== null && !inlineEditing()) return;
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
        if (closed || (busy && !committingText && !clipboardTextBusy && !inlineEditing()) || uncertain || !selected?.editable) return;
        if (!selected.slideId) selected = { ...selected, slideId: projection.slide.id };
        draft = value;
        if (draft === selected.text && !committingText && !pendingRanges()) draft = null;
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
            cancelTextCommit(); discardRanges(); draft = null; showSelected(selected); event.preventDefault();
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
                if(!['outline','scene','speaker-notes','section-outline','section-name','table-cells'].includes(operation))throw Error('Deck queries support outline, scene, speaker-notes, section-outline, section-name and table-cells');
                const method={'speaker-notes':'speakerNotes','section-outline':'sectionOutline','section-name':'sectionName','table-cells':'tableCells'}[operation]||operation;
                if(typeof port?.[method]!=='function')throw Error('This Deck query is unavailable');
                extra=await scheduleThumbnailWork(()=>port[method](args,revision));
            }
            // A targeted read must not duplicate the selected slide, controls and authoring catalog.
            if(query)return {app:'org.ricewm.deck',revision:revision??'0',ready:!closed&&!busy&&!uncertain&&!closeDecision&&!closeApproved&&draft===null&&!notes?.dirty&&!presenter?.active,name:documentName,queryResult:extra?.result||null,outline:(!query.operation||query.operation==='outline')?extra?.result||null:null};
            const speakerNotes=!busy&&projection?.slide&&port?.speakerNotes?await scheduleThumbnailWork(()=>port.speakerNotes({slide:projection.slide.id},revision)):null;
            const queryTable=canvas.selected.find(object=>object.table)||renderedScene?.objects.find(object=>object.table);
            const tableQueries=port?.tableCells&&queryTable&&projection?.slide?{tableCells:{operation:'table-cells',arguments:{slide:projection.slide.id,object:queryTable.id,startCell:0,limit:8}}}:{};
            return deckCopilotContext({app:'org.ricewm.deck',revision:revision??'0',ready:!closed&&!busy&&!uncertain&&!closeDecision&&!closeApproved&&draft===null&&!notes?.dirty&&!presenter?.active,
                textEditing:textEditingContext(),
                name:documentName,fileWorkflow:{...deckFileWorkflowContract,...port?.fileWorkflow},renderDiagnostics,projection,scene:renderedScene,selection:canvas.selected,tableSelection:canvas.selectedTableCell?{object:canvas.selectedTableCell.object.id,range:canvas.selectedTableCell.range,active:{row:canvas.selectedTableCell.row,column:canvas.selectedTableCell.column}}:null,speakerNotes:speakerNotes?.result||null,outline:(!query?.operation||query.operation==='outline')?extra?.result||null:null,queryResult:extra?.result||null,queries:{outline:{operation:'outline',arguments:{startSlide:0,limit:32}},scene:{operation:'scene',arguments:{slide:0,startObject:0,limit:32}},speakerNotes:{operation:'speaker-notes',arguments:{slide:projection?.slide?.id}},...tableQueries,...(projection?.sectionSupport?{sections:{operation:'section-outline',arguments:{startSection:0,limit:32}},sectionName:{operation:'section-name',arguments:{section:'actual GUID from section-outline'}}}:{})},designCatalog:deckDesignCatalog(),actions:deckCopilotActions({slideClipboard:slides.clipboardSupported&&Boolean(port?.copySlides&&port?.pasteSlides)})});
        },
        validate:validateDeckCopilot,
        act:async(operation,data,expectedRevision)=>{
            const validation=validateDeckCopilot(operation,data,expectedRevision);
            if(!validation.valid)return {...validation,applied:false};
            let applied=false,result;
            await perform(async()=>{
                if(revision!==expectedRevision)throw Error('Presentation changed before the AI edit');
                if(['copy-slides','cut-slides','paste-slides'].includes(operation)) {
                    if(!slides.clipboardSupported)throw Object.assign(Error('Slide clipboard transfer is unavailable for this presentation format'),{rejectedBeforeMutation:true});
                    result=operation==='paste-slides'?await port.pasteSlides(data):await port.copySlides({...data,text:'Selected slides',cut:operation==='cut-slides'});
                    if(result===null)throw Object.assign(Error('The clipboard does not contain presentation slides'),{rejectedBeforeMutation:true});
                } else result=operation==='compose'?await composeDeck(port,data,{expectedRevision,onProgress:p=>{status.text=`Creating slides · ${p.completed} of ${p.total}`;},onDraft:async()=>{selectedSlideId=null;slideIndex=0;await read();}}):operation==='apply'?await port.apply(data.commands):operation==='undo'?await port.undo():await port.redo();
                if(['apply','paste-slides'].includes(operation)){canvas.created(result);if(result?.result?.createdSlides?.length)selectedSlideId=result.result.createdSlides.at(-1);}
                await read();if(['apply','paste-slides'].includes(operation)&&result?.result?.createdSlides?.length)slides.selectSlides(result.result.createdSlides);applied=true;
            },true,false,true);
            if(!applied)throw Error(status.text||'Presentation action was not applied');
            return operation==='compose'?{applied,revision,...result}:{applied,revision,projection,result};
        }});
    const stopClose = app.on(Message.APP_CLOSE, () => {
        if (closed) return;
        closed = true; transitionPreview.cancel(); animationPreview.cancel(); animations.close(); transitions.close(); cancelTextCommit(); inline.close();shapeInline.close();canvas.cancel(); finishCloseDecision?.(false);
        for (const resolve of idleWaiters) resolve(); idleWaiters.clear();
        stopCopilot();stopCanvasTheme(); commandSearch?.close(); workspaceViews?.close(); notes.close(); closeAbout(); contextMenu?.destroy(); void presenter?.close(); slides.close(); finishDiscard(false); stopOpen(); stopRequest(); stopClose();
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
