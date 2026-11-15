import { loadDeckScene } from './scene-loader.mjs';
import { Button, Column, Container, Dialog, Icons, Label, Menu, MenuItem, Message, ReorderController, Row, SceneBuilder, SceneSurface, ScrollView, SectionHeader, SectionedCollectionLayout, Stack, State, TextField, ThumbnailTile, ZoomModel } from '@ricewm/app';
import { geometryFrame } from './scene-preview.mjs';
import { slideContext, sectionContext, emptySlideDoubleClick } from './slide-navigation-context.mjs';
import { createSlideSelection } from './slide-selection.mjs';
import { tableTextScene } from './table-values.mjs';

// Bound native widgets, including title tooltips and selection borders.
const OUTLINE_BATCH = 64, RETAINED_CARDS = 64;
const PREVIEW_BYTES = 7 * 1024 * 1024, VISIBLE_PICTURE_BYTES = 5 * 1024 * 1024;
const PREVIEW_RETRY_DELAYS = [150, 600];
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
export function createSlideNavigation({ port, navigate, mutate, dialogRoot=null, formatBackground=null, clipboard=null, clipboardScope=()=>{}, selectionChanged = () => {}, openSlide = null, previewWork = work => work() }) {
    let closed = false, locked = true, loaded = false, generation = 0, model = null, overview = false, cacheBytes = 0;
    let selectedIndex = 0, scrollOffset = 0, mountedKey = '', previewRequest = null, rendering = null, previewDirty = false, shownId = null, clickedId = null, viewportHeight = 640, bodySize = null, cardSize = null, mountedStart = 0, mountedEnd = 0, cardHeight = 100, pageRatio = 16 / 9, viewportWidth = null, measuredCardWidth = null;
    let zoomPercent = 100;
    let overviewRowWidth = null;
    const collapsedSections=new Set(),headers=new Map();let layout=null,layoutKey='',renameDialog=null;
    function collectionLayout() {
        if(!model)return null;
        const key=`${model.layoutIdentity}:${overview}:${columns()}:${stride()}:${[...collapsedSections].join('|')}`;
        if(key!==layoutKey) {
            const groups=(model.sections||[]).map(section=>({id:section.id,start:section.startSlide,count:section.slideCount,
                collapsed:overview&&section.slideCount>0&&collapsedSections.has(model.slides[section.startSlide]?.id)}));
            layout=new SectionedCollectionLayout({itemCount:model.slides.length,groups,columns:columns(),rowHeight:stride(),headerHeight:overview?40:26});layoutKey=key;
        }
        return layout;
    }
    const exposedIds=(extraHeight=0)=>{
        const geometry=collectionLayout();if(!geometry)return new Set();
        const {start,end}=geometry.range(scrollOffset,viewportHeight+extraHeight);
        return new Set(geometry.rows.slice(start,end).filter(row=>row.type==='items').flatMap(row=>model.slides.slice(row.start,row.end).map(item=>item.id)));
    };
    const sorterZoom = new ZoomModel({min:50,max:300,value:100,ready:false});
    const previewWidth = () => 200 * zoomPercent / 100;
    const columns = () => overview ? Math.max(1, Math.floor(((overviewRowWidth ?? viewportWidth ?? 960) - 32) / (previewWidth()+36))) : 1;
    const height = () => cardHeight;
    const stride = () => height() + (overview ? 8 : 4);
    const overviewCell = index => {
        const itemRow=collectionLayout()?.rowForItem(index),column=itemRow?.type==='items'?index-itemRow.start:0;
        const rowWidth=Math.max(1,overviewRowWidth ?? (viewportWidth || 960)-12),left=24+column*(previewWidth()+44);
        const width=Math.max(1,Math.min(previewWidth()+36,rowWidth-left));
        return{rowWidth,left,width,right:Math.max(0,18-(previewWidth()+36-width))};
    };
    const overviewPosition = index => {
        const cell=overviewCell(index),x=Math.min(999,Math.round(cell.left/cell.rowWidth*1000));
        return `positioned-item:${x}:0:${Math.max(1,Math.min(1000-x,Math.round(cell.width/cell.rowWidth*1000)))}:1000`;
    };
    const sizeCard=(card,size)=>card.control.setSize(overview?Math.max(1,size.width+overviewCell(model.byId.get(card.item.id).index).width-previewWidth()-36):size.width,size.height);
    const retainedCount = () => Math.min(2048, Math.max(RETAINED_CARDS,(Math.ceil(viewportHeight/stride())+8)*columns()));
    const visibleCount = () => Math.min(retainedCount(), (Math.ceil(viewportHeight / stride()) + 1) * columns());
    const buttons = new Map(), cards = [], cache = new Map();
    const previewFailures = new Map(), previewRetries = new Map();
    function clearPreviewRetries() { for (const timer of previewRetries.values()) clearTimeout(timer); previewRetries.clear(); }
    const reorder = new ReorderController({axis:'vertical',
        onPreview(move) {
            for (const card of cards) card.control.setDropPosition(move && (move.before === card.item.id ? 'before' : move.before === null && card.item.index === model.slides.length - 1 ? 'after' : null));
        },
        onCommit(move) { if (available()) return mutate('move-slide', {slide:move.id,index:move.sourceIndex,before:move.before}); },
    });
    const sectionReorder=new ReorderController({axis:'vertical',onPreview(move){for(const [id,header]of headers)header.setDropPosition(move&&(move.before===id?'before':move.before===null&&id===model.sections.at(-1)?.id?'after':null));},
        onCommit(move){if(available()&&overview)return mutate('move-section',{section:move.id,before:move.before});}});
    const selection = createSlideSelection();
    const modeOffsets = new Map([[false, 0], [true, 0]]);
    let notifiedSelection = '[]';
    function selectionUpdated() {
        for (const card of cards) {
            const selected = selection.has(card.item.id);
            card.control.setProperty('selected', selected);
            card.updateBorder();
            card.context?.update();
        }
        const ids = selection.selected, key = JSON.stringify(ids);
        const deletion = buttons.get('delete-slide');
        if (deletion) deletion.text = ids.length > 1 ? 'Delete selected slides' : 'Delete slide';
        const duplicate=buttons.get('duplicate-slide');
        if(duplicate){duplicate.text=ids.length>1?'Duplicate selected slides':'Duplicate slide';duplicate.enabled=available()&&ids.length>0&&ids.length<=32&&(ids.length===1||model?.slideBatchSupport===true);duplicate.setProperty('tooltip',ids.length>32?'Duplicate up to 32 slides per transaction':duplicate.text);}
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
    const background = new SceneSurface({ name: 'slides-empty-area', accessibleName: 'Slide navigator background',
        backgroundColor: 'transparent', state: State.EXPANSIVE, enabled: false });
    background.submit({width:1,height:1,displayList:new Uint8Array()});
    const viewport = new ScrollView({ name: 'slides-scroll', state: State.EXPANSIVE }, [
        new Stack({ name: 'slides-viewport', text: 'panel:flush' }, [background, list])]);
    const emptyContext = slideContext(viewport, { invoke: mutate, available, clipboard, sectionAvailable:()=>model?.sectionSupport===true&&model.slides.length>0 });
    const resetDoubleClick = emptySlideDoubleClick(background, mutate, available);
    const previewNotice = new Label({name:'slide-preview-notice',text:'Slide previews unavailable',enabled:false});
    const retryPreviews = new Button({name:'slide-preview-retry',text:'Retry previews'});
    const previewProblem = new Column({name:'slide-preview-problem',text:'panel:fixed:gap=0',visible:false},[previewNotice,retryPreviews]);
    const control = new Column({ name: 'slide-navigation', text: 'panel:flush:gap=0', visible: Boolean(port?.outline) }, [viewport,previewProblem]);
    function updatePreviewProblem() {
        const visible=exposedIds(),failed=[...previewFailures.keys()].filter(id=>visible.has(id));
        previewProblem.visible=failed.length>0;
        previewNotice.text=failed.length===1?'Slide preview unavailable':`${failed.length} slide previews unavailable`;
        retryPreviews.enabled=available();
    }
    retryPreviews.on(Message.ACTIVATE,()=>{
        if(!available()||!previewRequest)return;
        clearPreviewRetries();previewFailures.clear();updatePreviewProblem();
        previewRequest={...previewRequest,processed:new Set()};previewDirty=true;requestPreviews();
    });
    const dirtyPreviews = () => {
        if(previewRequest)previewRequest={...previewRequest,processed:new Set()};
        previewDirty=true;requestPreviews();
    };
    sorterZoom.onViewChanged(view=>{
        if(view.percent===zoomPercent)return;
        zoomPercent=view.percent;measuredCardWidth=null;
        if(!overview)return;
        cardHeight=Math.ceil(previewWidth()/pageRatio+44);mountedStart=mountedEnd=0;
        mount();viewport.setScrollPosition(`${Math.round(scrollOffset)}/0`);dirtyPreviews();
    });
    const zoomWheel = event => {
        const d=event.detail;
        if(!overview||!available()||d?.kind!=='wheel'||!(d.modifiers&(4|64))||![4,5].includes(d.button))return;
        event.preventDefault();event.stopPropagation();
        sorterZoom.setZoom(zoomPercent*(d.button===4?1.1:1/1.1));
    };
    // Pointer messages belong to their native hit target; unlike keys they
    // do not bubble. Register the same gesture on each owned input surface.
    for (const target of [control, viewport, background]) target.on(Message.POINTER, zoomWheel);
    // Native scene surfaces own wheel input. Keep the navigator's ordinary
    // 96-pixel scroll step while Ctrl+wheel retains its sorter zoom gesture.
    background.on(Message.POINTER,event=>{
        const d=event.detail;
        if(!ownsPointer()||d?.kind!=='wheel'||![4,5].includes(d.button)||d.modifiers&(4|64))return;
        event.preventDefault();event.stopPropagation();
        const maximum=Math.max(0,(collectionLayout()?.totalHeight||0)-viewportHeight);
        scrollOffset=Math.max(0,Math.min(maximum,scrollOffset+(d.button===4?-96:96)));
        mount();viewport.setScrollPosition(`${Math.round(scrollOffset)}/0`);dirtyPreviews();
    });
    for(const target of [control,viewport,background]) {
        target.on(Message.FOCUS,clipboardScope);
        target.on(Message.POINTER,event=>{if(event.detail?.kind==='down'&&available())clipboardScope();});
    }
    function selectionKey(event) {
        if(event.target instanceof TextField)return;
        const detail = event.detail;
        const key=detail?.key||({110:'Home',115:'End',111:'ArrowUp',116:'ArrowDown',113:'ArrowLeft',114:'ArrowRight',112:'PageUp',117:'PageDown'})[detail?.keyCode];
        const direction=({ArrowUp:-1,ArrowLeft:-1,PageUp:-1,ArrowDown:1,ArrowRight:1,PageDown:1})[key];
        if (available() && detail?.pressed && (direction||key==='Home'||key==='End') && cards.some(card => card.button === event.target) && !(detail.modifiers & (4 | 8 | 64))) {
            event.preventDefault(); event.stopPropagation();
            const next=key==='Home'?0:key==='End'?(model?.slides.length||1)-1:selectedIndex+direction;
            const item = model?.slides[next];
            if (item&&next!==selectedIndex) {
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
        const kind=({54:'copy',53:'cut',55:'paste'})[detail.keyCode]||({c:'copy',x:'cut',v:'paste'})[String(detail.key||detail.text||'').toLowerCase()];
        if(kind&&typeof clipboard?.[kind]==='function') {
            event.preventDefault();event.stopPropagation();clipboardScope();
            if(clipboard.can(kind))void clipboard[kind]();return;
        }
        if(detail.keyCode===40||String(detail.key||detail.text||'').toLowerCase()==='d') {
            event.preventDefault();event.stopPropagation();
            const slides=selection.selected;if(slides.length&&slides.length<=32&&(slides.length===1||model?.slideBatchSupport))void mutate('duplicate-slide',{slides});
            return;
        }
        if (detail.keyCode !== 38 && String(detail.key || detail.text || '').toLowerCase() !== 'a') return;
        event.preventDefault(); event.stopPropagation(); selection.all(); selectionUpdated();
    }
    for (const target of [control, viewport, background]) target.on(Message.KEY, selectionKey);
    viewport.on(Message.SIZE_CHANGED, event => {
        const widthChanged = viewportWidth !== event.detail.width;
        if (widthChanged) { viewportWidth = event.detail.width; measuredCardWidth = null; overviewRowWidth = null; }
        if (viewportHeight === event.detail.height && !widthChanged) return;
        viewportHeight = event.detail.height; mount();
        dirtyPreviews();
    });
    viewport.on(Message.SCROLL, ({ detail }) => {
        const offset = Number(String(detail?.position || '').split('/')[0]);
        if (!Number.isFinite(offset) || offset < 0) return;
        scrollOffset = offset;
        mount();
        dirtyPreviews();
    });
    const toolbar = new Row({ name: 'slide-actions', text: 'panel:fixed' }, [
        button('insert-slide', 'New slide', () => mutate('insert-slide')),
        button('duplicate-slide', 'Duplicate slide', () => mutate('duplicate-slide')),
        button('delete-slide', 'Delete slide', () => mutate('delete-slide', { slides: selection.selected })),
        button('move-slide-earlier', 'Move earlier', () => mutate('move-earlier')),
        button('move-slide-later', 'Move later', () => mutate('move-later')),
    ]);
    const sectionMenu=new Menu({name:'slide-section-menu',text:'Section',icon:Icons.ObjectGroup,state:State.START_ALIGNED,accessibleName:'Section',enabled:false});
    const addSection=new MenuItem({name:'slide-section-add',text:'Add Section',enabled:false});
    const removeSections=new MenuItem({name:'slide-section-remove-all',text:'Remove All Sections',enabled:false});
    addSection.on(Message.ACTIVATE,()=>{if(addSection.enabled&&available())void mutate('add-section');});
    removeSections.on(Message.ACTIVATE,()=>{if(removeSections.enabled&&available())void mutate('remove-all-sections');});
    sectionMenu.append(addSection);sectionMenu.append(removeSections);
    function setState({ disabled, loaded: isLoaded, index, count }) {
        loaded = Boolean(isLoaded);
        locked = Boolean(disabled || !port?.outline);
        reorder.setEnabled(available());
        sectionReorder.setEnabled(available()&&overview);
        sectionMenu.enabled=available()&&model?.sectionSupport===true;
        addSection.enabled=sectionMenu.enabled&&count>0;
        removeSections.enabled=sectionMenu.enabled&&Boolean(model?.sections.length);
        sorterZoom.setReady(available()&&overview);
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
        for(const header of headers.values()){header.button.enabled=ownsPointer();header.context?.update();}
        selectionUpdated();
        // A draft can defer background reads without changing the native lock.
        // Retry retained work whenever its owner reports readiness again.
        if (!locked) requestPreviews();
        updatePreviewProblem();
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
            clearPreviewRetries();previewFailures.clear();updatePreviewProblem();
            generation++;
            const slides = [];let sectionSupport=false,slideBatchSupport=false,slideClipboardSupport=false;
            do {
                const result = await outline({ startSlide: slides.length, limit: OUTLINE_BATCH }, revision);
                sectionSupport=result.sectionSupport===true;
                slideBatchSupport=result.slideBatchSupport===true;
                slideClipboardSupport=result.slideClipboardSupport===true;
                if (closed) return null;
                if (result.startSlide !== slides.length || (count && !result.slides.length)) throw Error('Slide outline made no progress');
                slides.push(...result.slides);
                if (result.nextSlide === null) break;
            } while (slides.length < count);
            if (slides.length !== count || new Set(slides.map(item => item.id)).size !== count) throw Error('Slide outline count or identities changed');
            // Keep the previous painted frames until their replacements arrive.
            // Revision tags require a refresh without blanking the whole sidebar.
            if (model?.slides.length !== count) measuredCardWidth = null;
            const sections=[];
            if(sectionSupport&&port.sectionOutline) {
                do {
                    const reply=await port.sectionOutline({startSection:sections.length,limit:32},revision);
                    if(closed)return null;
                    if(reply.revision!==revision||reply.result.startSection!==sections.length||reply.result.nextSection!==null&&!reply.result.sections.length)throw Error('Section outline changed');
                    sections.push(...reply.result.sections);if(reply.result.nextSection===null)break;
                }while(sections.length<4096);
            }
            model = { revision, layoutIdentity:generation, slides, sections, sectionSupport, slideBatchSupport, slideClipboardSupport, byId: new Map(slides.map(item => [item.id, item])) };
            sectionReorder.setItems(sections.map(section=>section.id));
            reorder.setItems(slides.map(item => item.id));
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
        const card = { item, index: item.index };
        const surface = new SceneSurface({ name: `slide-preview-${item.index}`, presentation: 'surface', state: State.EXPANSIVE, enabled: false, visible: false });
        const tile = new ThumbnailTile({ name: `slide-card-${item.index}`, buttonName: `slide-${item.index}`,
            content: surface, number: slideNumber(item), selected: selection.has(item.id), observeSize: false,
            numberWidth: Math.max(16, String(model?.slides.length || 1).length * 7 + 4),
            numberPlacement: overview ? 'below' : 'leading', contentWidth: overview ? previewWidth() : null, aspectRatio: pageRatio,
            padding: overview ? {left:18,right:18,top:8,bottom:0} : {},
            text: `panel:fixed-height:${height()}:flush:gap=0`,
            accessibleName: `${item.index + 1}. ${item.name || 'Untitled'}${item.hidden ? ' (hidden)' : ''}` });
        const choice = tile.button;
        reorder.attach(choice,item.id);
        choice.text = `panel:fixed-height:${height()}:flush`; choice.enabled = ownsPointer();
        choice.on(Message.POINTER, zoomWheel);
        choice.on(Message.FOCUS,clipboardScope);
        // Native Button releases are ACTIVATE without modifiers. Preserve only
        // its matching primary press; keyboard/blur cannot reuse stale modifiers.
        let pressModifiers = 0, previousClick = null, primaryPress = null;
        choice.on(Message.POINTER, ({ detail }) => {
            if (detail.kind === 'down') {
                if(available())clipboardScope();
                primaryPress = detail.button===1?{x:detail.x,y:detail.y}:null;
                if(detail.button!==1)previousClick=null;
                pressModifiers = detail.button === 1 ? detail.modifiers || 0 : 0;
                if (detail.button === 3 && available() && !selection.has(card.item.id)) {
                    selection.choose(card.item.id); selectionUpdated();
                }
            }
            else if (detail.kind === 'move'&&primaryPress&&Math.hypot(detail.x-primaryPress.x,detail.y-primaryPress.y)>6) {primaryPress=null;previousClick=null;}
            else if (detail.kind === 'up') {pressModifiers = 0;primaryPress=null;}
        });
        choice.on(Message.REORDER,({detail})=>{
            if(primaryPress&&Math.hypot(detail.x-primaryPress.x,detail.y-primaryPress.y)>=6){primaryPress=null;previousClick=null;}
            if(detail.phase==='up'){pressModifiers=0;primaryPress=null;}
        });
        choice.on(Message.BLUR, () => { pressModifiers = 0;previousClick=primaryPress=null; });
        choice.on(Message.KEY, event => { pressModifiers = 0;previousClick=primaryPress=null;selectionKey(event); });
        choice.on(Message.ACTIVATE, () => {
            resetDoubleClick();
            const modifiers = pressModifiers; pressModifiers = 0;
            if (!available()) return;
            const click=primaryPress?{...primaryPress,time:performance.now()}:null;
            const double=typeof openSlide==='function'&&overview&&click&&previousClick&&click.time-previousClick.time<=450&&Math.hypot(click.x-previousClick.x,click.y-previousClick.y)<=6&&!(modifiers&(1|4|8|64));
            previousClick=click;
            if(double){previousClick=null;void openSlide(card.item.index,card.item.id);return;}
            const previous = selection.active;
            const additive = Boolean(modifiers & (4 | 64));
            const id = selection.choose(card.item.id, { control: additive, shift: Boolean(modifiers & 1) });
            selectionUpdated();
            if (id && (id !== previous || !additive)) {
                clickedId = id; void navigate(model.byId.get(id).index, id);
            }
        });
        const badge = tile.number;
        badge.setProperty('name', `slide-number-${item.index}`);
        tile.tooltip.setProperty('name', `slide-title-${item.index}`);
        tile.border.setProperty('name', `slide-border-${item.index}`);
        tile.edges.forEach((edge, index) => edge.setProperty('name', `slide-border-${item.index}-${index}`));
        card.updateBorder = () => tile.setSelected(selection.has(card.item.id));
        card.setTitle = title => tile.setTitle(String(title || '').trim().slice(0, 512));
        card.observe = () => { card.stopObserve?.(); card.stopObserve = tile.layoutRoot.on(Message.SIZE_CHANGED, event => {
            if (event.target !== tile.layoutRoot || !(event.detail?.width > 0 && event.detail?.height > 0)) return;
            // Native feedback for the former row arrangement can be queued
            // behind the viewport resize. Do not seed the new layout epoch
            // with that stale width; allow ordinary scrollbar/chrome insets.
            if (viewportWidth !== null && Math.abs(event.detail.width - (overview ? previewWidth()+36 : viewportWidth-12)) > 32) return;
            cardSize = {width:event.detail.width,height:event.detail.height};
            for (const candidate of cards) sizeCard(candidate,cardSize);
            bodySize = tile.previewSize;
            // A scrollbar changes content width without resizing this viewport.
            // Keep the narrowest native measurement for this layout epoch so
            // width -> height -> overflow cannot repeatedly add/remove it.
            // A real viewport resize, mode/count or aspect change starts afresh.
            const availableWidth = overview ? Math.min(previewWidth(), event.detail.width - 36) : event.detail.width - tile.numberWidth - 4;
            measuredCardWidth = Math.min(measuredCardWidth ?? availableWidth, availableWidth);
            const extra = overview ? 36 : Math.max(0, cardHeight - event.detail.height);
            const nextHeight = Math.max(64, Math.min(overview?99992:400, Math.ceil(measuredCardWidth / pageRatio + extra)));
            if (nextHeight !== cardHeight) {
                const rowOffset = scrollOffset / stride(); cardHeight = nextHeight;
                scrollOffset = rowOffset * stride();
                if (model?.slides.length > retainedCount()) mountedKey = '';
                mount();
                if (scrollOffset) viewport.setScrollPosition(`${Math.round(scrollOffset)}/0`);
                dirtyPreviews();
            }
        }); return card.stopObserve; };
        const container = tile;
        const contextTarget = { slide: item.id, index: item.index };
        Object.assign(card, { control: container, button: choice, surface, badge, contextTarget,
            context: slideContext(container, { target: contextTarget, invoke: mutate, clipboard, formatBackground, available: () => available() && !container.destroyed,
                batchAvailable:()=>model?.slideBatchSupport===true,
                sectionAvailable:()=>model?.sectionSupport===true,
                selection: () => selection.has(card.item.id) ? selection.selected : [card.item.id],
                hidden: id => model?.byId.get(id)?.hidden === true }) });
        const cached = cache.get(item.id);
        card.setTitle(cached?.title || (item.nameSpecified ? item.name : ''));
        if (cached) {
            const exposed=exposedIds();
            // A restored card window can exceed the host64-response cap if
            // every cached surface is emitted with its Create batch. Restore
            // only the viewport; offscreen cache hits remain cheap to reveal.
            if (exposed.has(item.id)&&cards.filter(card=>card.surface.visible).length<24) {
                surface.submit(cached.frame); surface.setProperty('visible', true);
            }
        }
        return card;
    }
    async function renameSection(section) {
        if(!available()||renameDialog||!section)return;
        const reply=await port.sectionName({section:section.id},model.revision);
        if(closed||renameDialog||!available())return;
        const root=dialogRoot||control,field=new TextField({name:'section-rename-name',value:reply.result.name}),error=new Label({name:'section-rename-error',text:''});
        const cancel=new Button({name:'section-rename-cancel',text:'Cancel'}),apply=new Button({name:'section-rename-apply',text:'Rename',accent:true});
        const close=()=>{if(renameDialog)root.remove(renameDialog,{destroy:true});renameDialog=null;};
        renameDialog=new Dialog({name:'section-rename-dialog',text:'Rename Section',modal:true},[new Label({text:'Section name:'}),field,error,new Row({text:'panel:actions'},[cancel,apply])]);
        const commit=async()=>{
            if(!apply.enabled)return;const name=field.value.trim();if(!name){error.text='Enter a section name.';return;}
            apply.enabled=false;
            try{if(await mutate('rename-section',{section:section.id,name})===true)close();else if(renameDialog)error.text='The section could not be renamed.';}
            finally{if(renameDialog)apply.enabled=true;}
        };
        cancel.on(Message.ACTIVATE,close);apply.on(Message.ACTIVATE,commit);
        renameDialog.on(Message.KEY,event=>{event.stopPropagation();if(!event.detail?.pressed)return;const key=event.detail.key||({9:'Escape',36:'Enter'})[event.detail.keyCode];if(key==='Escape'){event.preventDefault();close();}if(key==='Enter'){event.preventDefault();void commit();}});
        root.append(renameDialog);field.setProperty('focused',true);
    }
    function makeHeader(section) {
        const header=new SectionHeader({name:`section-${section.id.slice(1,-1)}`,title:section.name,count:section.slideCount,
            height:overview?32:22,collapsible:overview,renameable:overview,
            collapsed:overview&&collapsedSections.has(model.slides[section.startSlide]?.id),
            renameValue:async()=>{const reply=await port.sectionName({section:section.id},model.revision);return reply.result.name;},
            onToggle(value){
                if(!available()||!overview)return;
                const current=model.sections.find(s=>s.id===section.id),first=model.slides[current?.startSlide]?.id;
                if(!first||!current.slideCount)return;
                if(value)collapsedSections.add(first);else collapsedSections.delete(first);
                mount();dirtyPreviews();
            },
            onRename:name=>available()?mutate('rename-section',{section:section.id,name}):false});
        if(overview)sectionReorder.attach(header.button,section.id);
        header.context=sectionContext(header,{section:()=>model.sections.find(s=>s.id===section.id),
            sections:()=>model.sections,available:()=>available()&&!header.destroyed,invoke:mutate,
            rename:()=>renameSection(model.sections.find(s=>s.id===section.id))});
        header.button.on(Message.POINTER,zoomWheel);header.toggleButton.on(Message.POINTER,zoomWheel);
        return header;
    }
    function mount() {
        if (!model) return;
        const geometry=collectionLayout(),perRow=columns(),totalRows=geometry.rows.length;
        const visible=geometry.range(scrollOffset,viewportHeight);
        const keepWindow=mountedEnd>mountedStart&&visible.start>=mountedStart&&visible.end<=mountedEnd;
        const retainedRows=Math.max(1,Math.floor(retainedCount()/perRow));
        const firstRow=totalRows<=retainedRows?0:keepWindow?mountedStart:Math.min(Math.max(0,visible.start-4),Math.max(0,totalRows-retainedRows));
        const lastRow=Math.min(totalRows,firstRow+retainedRows);
        const source=model.byId.get(reorder.sourceId),sourceRow=source?geometry.rowForItem(source.index)?.index:-1;
        const sourceHeader=geometry.rows.find(row=>row.type==='header'&&row.group===sectionReorder.sourceId)?.index??-1;
        const rows=[...new Set([...Array.from({length:lastRow-firstRow},(_,offset)=>firstRow+offset),
            ...[sourceRow,sourceHeader].filter(index=>index>=0&&(index<firstRow||index>=lastRow))])].sort((a,b)=>a-b);
        const key=`${layoutKey}:${rows.join(',')}`;
        if(key!==mountedKey) {
            mountedKey=key;mountedStart=firstRow;mountedEnd=lastRow;
            const previous=new Map(cards.map(card=>[card.item.id,card])),oldHeaders=new Map(headers),previousRows=[...list.children];
            for(const card of cards)card.stopObserve?.();cards.length=0;headers.clear();
            let nextY=0;
            for(const rowIndex of rows) {
                const descriptor=geometry.rows[rowIndex];spacer(descriptor.y-nextY);nextY=descriptor.y+descriptor.height;
                if(descriptor.type==='header') {
                    const section=model.sections.find(section=>section.id===descriptor.group);
                    let header=oldHeaders.get(section.id);oldHeaders.delete(section.id);
                    if(header&&header.overview!==overview){header.destroy();header=null;}
                    if(!header){header=makeHeader(section);header.overview=overview;}
                    header.setSection({title:section.name,count:section.slideCount,collapsible:overview,renameable:overview,
                        collapsed:overview&&section.slideCount>0&&collapsedSections.has(model.slides[section.startSlide]?.id)});
                    const row=new Column({text:`panel:flush:gap=0:accessory-height=${descriptor.height}`});list.append(row);
                    if(header.parent)header.moveTo(row);else row.append(header);
                    headers.set(section.id,header);continue;
                }
                const row=overview?new Stack({text:'panel:overlay:flush',state:State.EXPANSIVE}):new Row({text:'panel:equal',state:State.EXPANSIVE});
                if(overview&&rowIndex===rows.find(index=>geometry.rows[index].type==='items'))row.on(Message.SIZE_CHANGED,({detail})=>{
                    if(row.destroyed||!overview||!(detail?.width>0)||overviewRowWidth===detail.width)return;
                    overviewRowWidth=detail.width;mount();dirtyPreviews();
                });
                list.append(new Column({text:`panel:fixed-height:${descriptor.height}:gap=0:flush`},[row]));
                for(const item of model.slides.slice(descriptor.start,descriptor.end)) {
                    if(rowIndex===sourceRow&&(rowIndex<firstRow||rowIndex>=lastRow)&&item.id!==source?.id)continue;
                    const card=previous.get(item.id)||makeCard(item);previous.delete(item.id);
                    if(card.control.parent)card.control.moveTo(row);else row.append(card.control);cards.push(card);
                }
            }
            for(const child of previousRows)list.remove(child,{destroy:true});
            for(const card of previous.values())card.control.destroy();for(const header of oldHeaders.values())header.destroy();
            const exposed=exposedIds();for(const card of cards)if(!exposed.has(card.item.id))card.surface.visible=false;
            cards[0]?.observe();spacer(geometry.totalHeight-nextY);
        }
        for (const card of cards) {
            const item = model.byId.get(card.item.id);
            card.control.text = overview ? overviewPosition(item.index) : `panel:fixed-height:${height()}:flush:gap=0`;
            card.control.setProperty('state', overview ? State.FLAT : State.EXPANSIVE);
            card.control.setLayout({numberPlacement:overview?'below':'leading',contentWidth:overview?previewWidth():null,aspectRatio:pageRatio,padding:overview?{left:18,right:overviewCell(item.index).right,top:8,bottom:0}:{}});
            if (cardSize) sizeCard(card,cardSize);
            card.button.text = `panel:fixed-height:${height()}:flush`;
            if (card.index !== item.index) {
                card.control.setProperty('name', `slide-card-${item.index}`); card.button.setProperty('name', `slide-${item.index}`);
                card.surface.setProperty('name', `slide-preview-${item.index}`); card.badge.setProperty('name', `slide-number-${item.index}`);
                card.control.tooltip.setProperty('name', `slide-title-${item.index}`); card.control.border.setProperty('name', `slide-border-${item.index}`);
                card.control.edges.forEach((edge, index) => { edge.setProperty('name', `slide-border-${item.index}-${index}`); });
                card.index = item.index;
            }
            card.button.setProperty('accessibleName', `${item.index + 1}. ${item.name || 'Untitled'}${item.hidden ? ' (hidden)' : ''}`);
            card.setTitle(cache.get(item.id)?.revision === model.revision ? cache.get(item.id).title : (item.nameSpecified ? item.name : ''));
            card.item = item; card.updateBorder(); Object.assign(card.contextTarget, { slide: item.id, index: item.index });
            card.control.setNumber(slideNumber(item));
            card.control.setProperty('selected', selection.has(item.id));
            card.button.setProperty('enabled', ownsPointer()); card.context.update();
        }
        updatePreviewProblem();
    }
    function show(index) {
        resetDoubleClick();
        selectedIndex = index;
        const nextId = model?.slides[index]?.id || null;
        selection.activate(nextId); selectionUpdated();
        const changed = nextId !== shownId;
        const row=collectionLayout()?.rowForItem(index);
        const outside=row&&(row.y<scrollOffset||row.y+row.height>scrollOffset+viewportHeight);
        const reveal = changed && outside && clickedId !== nextId;
        clickedId = null;
        if (reveal) scrollOffset = row.y;
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
        const exposed=exposedIds();
        // Current slide first, then a bounded viewport/overscan batch. The native
        // scroll event schedules the next batch; distant slides need no raster or
        // text layout work merely because the selection changed.
        const visibleIds=exposedIds(stride());
        const visible = cards.filter(card => visibleIds.has(card.item.id));
        const exposedCount = exposed.size;
        const desiredWidth = Math.max(1, Math.round(overview ? measuredCardWidth ?? previewWidth() : measuredCardWidth ?? 256));
        const picturePixels = Math.max(1024, Math.min(6 * 1024 * 1024 / 4, Math.floor(VISIBLE_PICTURE_BYTES / Math.max(1, exposedCount) / 4)));
        const qualityFor = card => {
            const active = exposed.has(card.item.id);
            return { thumbnail: true, thumbnailWidth: active ? desiredWidth : Math.min(128, desiredWidth),
                thumbnailMaximumPixels: active ? picturePixels : Math.min(16384, picturePixels) };
        };
        const current = cards.find(card => card.item.id === currentScene?.slideId);
        const queue = [...new Set([...(current ? [current] : []), ...visible])];
        const cancelled = () => {
            if (closed || request !== previewRequest || epoch !== generation || revision !== model?.revision) return true;
            if (background && locked) {
                // A foreground operation owns the port until unlock. Keep the
                // unpainted part of this request pending across that handoff.
                previewDirty = true;
                return true;
            }
            return false;
        };
        let changed=0;
        for (const card of queue) {
            if (cancelled()) return;
            if(request.processed.has(card.item.id))continue;
            const failure=previewFailures.get(card.item.id);
            if(failure?.attempts>PREVIEW_RETRY_DELAYS.length){request.processed.add(card.item.id);continue;}
            const cached = cache.get(card.item.id);
            const quality = qualityFor(card);
            const fresh = cached?.revision === revision && (!cached.quality || cached.quality.thumbnailWidth === quality.thumbnailWidth && cached.quality.thumbnailMaximumPixels === quality.thumbnailMaximumPixels);
            if(fresh&&card.surface.visible){request.processed.add(card.item.id);continue;}
            if(changed>=24){previewDirty=true;break;}
            changed++;
            if (fresh) {
                if (!card.control.destroyed && !card.surface.getProperty('visible')) {
                    card.surface.submit(cached.frame);
                    card.surface.setProperty('visible', true);
                }
                request.processed.add(card.item.id);
                continue;
            }
            try {
                let scene = currentScene?.slideId === card.item.id ? currentScene : null, frames = currentFrames;
                if (!scene) {
                    scene = await loadDeckScene(port, { index: card.item.index, revision, maximumObjects: 512, cancelled });
                    if (cancelled()) return;
                    if (scene.slideId !== card.item.id) throw Error('Preview identity changed');
                    const text = await port.renderText?.(scene);
                    if(!text?.frames?.size&&text?.diagnostics?.some(item=>item.message))throw Error('Slide text could not be rendered');
                    frames = text?.frames || new Map();
                }
                // A frame can be empty or partial without a provider diagnostic.
                // Include table-cell text, whose derived IDs differ from its table.
                const textObjects = tableTextScene(scene).objects.filter(object => object.text?.text);
                const missingText = () => textObjects.some(object => !frames?.has(object.id));
                if(port.renderText&&missingText()) {
                    const text=await port.renderText(scene);
                    frames=text?.frames;
                    if(missingText())throw Error('Slide text could not be rendered completely');
                }
                if (cancelled()) return;
                const images=await port.renderImages?.(scene, quality);
                const pictures = images?.frames || new Map();
                if (cancelled()) return;
                const frame = thumbnailAppearance(geometryFrame(scene, { textFrames: frames, imageFrames: pictures }).frame, card.item.hidden === true);
                const title = scene.objects.find(object => object.placeholder?.role === 'title' && object.text?.text?.trim())?.text.text.trim()
                    || (card.item.nameSpecified ? card.item.name.trim() : '');
                card.setTitle(title);
                cacheBytes -= cache.get(card.item.id)?.frame.displayList.byteLength || 0;
                const hasPictures = Boolean(scene.backgroundPicture || scene.objects.some(object => object.image));
                cache.delete(card.item.id); cache.set(card.item.id, { revision, frame, title, quality: hasPictures ? quality : null }); cacheBytes += frame.displayList.byteLength;
                // Bound retained vector/image bytes as well as widget count.
                while (cache.size > retainedCount() || (cache.size > 1 && cacheBytes > PREVIEW_BYTES)) {
                    const id = [...cache.keys()].find(id => !exposed.has(id)) ?? cache.keys().next().value;
                    cacheBytes -= cache.get(id).frame.displayList.byteLength; cache.delete(id);
                    const evicted = cards.find(candidate => candidate.item.id === id);
                    if (evicted && !evicted.control.destroyed) {
                        evicted.surface.setProperty('visible', false);
                        evicted.surface.submit({ width: 1, height: 1, displayList: new Uint8Array() });
                    }
                }
                if (!card.control.destroyed) { card.surface.submit(frame); card.surface.setProperty('visible', true); }
                previewFailures.delete(card.item.id);
                if(previewRetries.has(card.item.id)){clearTimeout(previewRetries.get(card.item.id));previewRetries.delete(card.item.id);}
                updatePreviewProblem();
                request.processed.add(card.item.id);
            } catch (error) {
                if (cancelled()) return;
                request.processed.add(card.item.id);
                const attempts=(previewFailures.get(card.item.id)?.attempts||0)+1;
                previewFailures.set(card.item.id,{attempts,message:String(error.message||error).slice(0,240)});
                updatePreviewProblem();
                if(attempts<=PREVIEW_RETRY_DELAYS.length&&!previewRetries.has(card.item.id)) {
                    const id=card.item.id;
                    previewRetries.set(id,setTimeout(()=>{
                        previewRetries.delete(id);
                        if(closed||epoch!==generation||revision!==model?.revision||!previewRequest)return;
                        previewRequest.processed.delete(id);previewDirty=true;requestPreviews();
                    },PREVIEW_RETRY_DELAYS[attempts-1]));
                }
            }
        }
    }
    function requestPreviews() {
        if (closed || locked || !previewDirty || !previewRequest || rendering) return;
        previewDirty = false;
        // Set ownership before the callback can synchronously lock the shell.
        let started = false, failed = false;
        rendering = Promise.resolve().then(() => previewWork(() => {
            started = true;
            return drawPreviews(previewRequest, true);
        })).catch(() => { failed = true; }).finally(() => {
            rendering = null;
            if (!started || failed) {
                // Do not spin while the owner is typing or deciding to close.
                // setState or a new viewport request will retry this work.
                previewDirty = true;
            } else requestPreviews();
        });
    }
    async function previews(revision, currentScene, currentFrames) {
        if (currentScene?.size?.width > 0 && currentScene.size.height > 0) {
            const ratio = currentScene.size.width / currentScene.size.height;
            if (ratio !== pageRatio) { pageRatio = ratio; measuredCardWidth = null; }
        }
        previewRequest = { revision, currentScene, currentFrames, generation,processed:new Set() };
        await rendering;
        previewDirty = false;
        await drawPreviews(previewRequest);
    }
    async function command(type, slide, index, count, revision, target = null) {
        if(['add-section','rename-section','remove-section','remove-section-slides','remove-all-sections','move-section','move-section-earlier','move-section-later'].includes(type)) {
            if(!model?.sectionSupport||model.revision!==revision)throw Error('Section authoring is unavailable for this presentation');
            if(type==='add-section'){if(!model.byId.has(slide))throw Error('Select a slide first');return {type,at:slide,name:'Untitled Section'};}
            if(type==='remove-all-sections')return {type};
            const sectionIndex=model.sections.findIndex(section=>section.id===target?.section);if(sectionIndex<0)throw Error('Section changed');
            if(type==='rename-section')return {type,section:target.section,name:target.name};
            if(type==='remove-section')return {type,section:target.section};
            if(type==='remove-section-slides')return {type:'remove-section',section:target.section,slides:true};
            let before=target.before;
            if(type==='move-section-earlier') {if(sectionIndex===0)throw Error('Section is already first');before=model.sections[sectionIndex-1].id;}
            if(type==='move-section-later') {if(sectionIndex===model.sections.length-1)throw Error('Section is already last');before=model.sections[sectionIndex+2]?.id??null;}
            if(before!==null&&!model.sections.some(section=>section.id===before))throw Error('Section order changed');
            return {type:'move-section',section:target.section,before};
        }
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
        if(type==='duplicate-slide') {
            const ids=target?.slides??(selection.has(slide)?selection.selected:[slide]);
            if(!Array.isArray(ids)||!ids.length||new Set(ids).size!==ids.length||ids.some(id=>!model?.byId.has(id)))throw Error('Select existing slides first');
            if(ids.length===1)return {type,slide:ids[0]};
            if(!model.slideBatchSupport)throw Error('Multiple slide duplication is unavailable for this presentation');
            if(ids.length>32)throw Error('Duplicate up to 32 slides per transaction');
            return {type:'duplicate-selected-slides',slides:[...ids]};
        }
        if (type === 'delete-slide') return { type, slide };
        if (type === 'move-slide') {
            if (!model || model.revision !== revision || model.slides[index]?.id !== slide ||
                target?.before !== null && !model.byId.has(target?.before) || target?.before === slide) throw Error('Slide order changed');
            return {type,slide,before:target.before};
        }
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
    return Object.freeze({ control, toolbar, sectionMenu, sorterZoom, setState, locate, show, previews, command, retainArtwork,
        selectOnly(id) { if(!model?.byId.has(id))return false;selection.choose(id);selectionUpdated();return true; },
        selectSlides(ids) { if(!model||!Array.isArray(ids)||!ids.length||ids.some(id=>!model.byId.has(id)))return false;selection.replace(ids);selectionUpdated();for(const card of cards)card.button.setProperty('focused',card.item.id===selection.active);return true; },
        get outlineItems() { return (model?.slides || []).map(item => ({ ...item, name: cache.get(item.id)?.title || item.name })); },
        get hyperlinkSlides() { return (model?.slides || []).map((slide, index) => ({ id: slide.id, label: `${index + 1}. ${slide.name || 'Slide'}` })); },
        get selectedSlides() { return selection.selected; },
        get clipboardSupported(){return model?.slideClipboardSupport===true;},
        get clipboardTitles(){return selection.selected.map(id=>model?.byId.get(id)?.name||'Untitled slide').join('\n');},
        get hiddenSlideIndices() { return model?.slides.filter(item => item.hidden === true).map(item => item.index) ?? []; },
        reset() { clearPreviewRetries();previewFailures.clear();updatePreviewProblem();generation++; layout=null;layoutKey='';collapsedSections.clear();sectionReorder.setItems([]);headers.clear();if(renameDialog)(dialogRoot||control).remove(renameDialog,{destroy:true});renameDialog=null;reorder.setItems([]); selection.reset(); selectionUpdated(); for(const card of cards)card.stopObserve?.(); for(const child of [...list.children])list.remove(child,{destroy:true});cards.length=0; measuredCardWidth = null; model = null; shownId = null; mountedKey = ''; mountedStart = mountedEnd = 0; cache.clear(); cacheBytes = 0; previewRequest = null; scrollOffset = 0; modeOffsets.set(false, 0); modeOffsets.set(true, 0); viewport.setScrollPosition('0/0'); },
        setOverview(value) { value = Boolean(value); if (value === overview) return; reorder.setAxis(value?'horizontal':'vertical');modeOffsets.set(overview, scrollOffset); measuredCardWidth = null; overview = value;sectionReorder.setEnabled(available()&&overview);sorterZoom.setReady(available()&&overview);cardHeight = overview ? Math.ceil(previewWidth()/pageRatio+44) : 100; shownId = null; mountedStart = mountedEnd = 0; bodySize = null; scrollOffset = modeOffsets.get(overview) || 0; viewport.setScrollPosition(`${Math.round(scrollOffset)}/0`); },
        close() { closed = true;clearPreviewRetries();previewFailures.clear();generation++;if(renameDialog)(dialogRoot||control).remove(renameDialog,{destroy:true});renameDialog=null;sectionReorder.dispose();headers.clear();reorder.dispose();sorterZoom.setReady(false);sorterZoom.dispose(); },
    });
}
