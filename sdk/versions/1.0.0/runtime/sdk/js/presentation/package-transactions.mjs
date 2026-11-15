import {TABLE_RANGE_COMMANDS} from './table-range-contract.mjs';
import {TABLE_MERGE_COMMANDS} from './table-merge-contract.mjs';
import {TABLE_DESIGN_COMMANDS} from './table-design-contract.mjs';
import {inspectPptxTableDesign,editPptxTableDesign,editPptxTableOptions,initializeCreatedPptxTableDesign} from './table-design-package.mjs';
import {readPptxTableDesignProjection} from './table-design-projection.mjs';
import {inspectPptxTableMerges,inspectOdpTableMerges,editPptxTableMerges,editOdpTableMerges} from './table-merge-package.mjs';
import {admitNativeTableMerge} from './table-merge-admission.mjs';
import { BinaryDocumentHistory } from '../binary-document-history.mjs';
import { inspectPptxPackageSlides, editPptxPackageReferences, preparePptxPackageStructure } from './slide-package.mjs';
import { SECTION_COMMANDS, checkedSectionReply, checkedSectionNameReply } from './section-contract.mjs';
import { checkedRequest, checkedResponse, checkedMutation, encode, failure, rejectedRevision, revision } from './engine-contract.mjs';
import { checkedSelectionReply } from './selection-contract.mjs';
import {createPptxSlideClipboard,inspectPptxSlideClipboard,insertPptxSlideClipboard} from './slide-transfer.mjs';
import {SLIDE_CLIPBOARD_MIME,SLIDE_CLIPBOARD_BYTES,checkedSlideClipboardReply} from './slide-transfer-contract.mjs';
import {OBJECT_OPACITY_COMMANDS,inspectPptxObjectOpacity,inspectPptxBackgroundKind,editPptxObjectOpacity} from './object-opacity.mjs';
import {PACKAGE_BACKGROUND_COMMANDS,inspectPptxBackground,editPptxBackground,inspectPptxPictureBackground,pictureSourceSettings,editPptxPictureOptions} from './background-package.mjs';
import {pictureFillTiles} from '../picture-fills.mjs';
import {THEME_COMMANDS,THEME_BYTES,checkedThemeReply} from './theme-contract.mjs';
import {inspectPresentationThemePackage,importPptxTheme} from './theme-import.mjs';
import {inspectPptxTheme,inspectPptxThemeScope,editPptxTheme} from './theme-package.mjs';
import {inspectPptxObjectGlow} from './glow-package.mjs';
import {TEXT_RANGE_COMMANDS,textRangeTargetKey} from './text-range-contract.mjs';
import {nativeRangeObject,nativeRangeText} from './text-range-admission.mjs';
import {cellKey} from './table-values.mjs';
import {editPptxTextRanges,inspectPptxTextRanges} from './text-range-package.mjs';
import {editOdpTextRanges,inspectOdpTextRanges} from './odp-text-range-package.mjs';
import {readOdpSource,odpSlideIds} from './odp-source.mjs';

/** Per-document owner for native edits and package-reference edits. Each apply
 * is evaluated in a spare native worker. The committed worker, revision and
 * unified history change only after the whole candidate imports and projects.
 * Source adapters apply only to understood unsigned packages. Both formats
 * retain their native signature/source guards in every disposable candidate.
 */
export async function attachDeckPackageTransactions({ backend, reopen, signal,newDocument=false } = {}) {
    const initialStatus = await backend.exchange('status');
    if (!['pptx','potx','ppsx','odp','otp'].includes(initialStatus.result.format)) return backend;
    const initial = await backend.exportBytes();
    const powerpoint=['pptx','potx','ppsx'].includes(initialStatus.result.format);
    let references=null;
    if(powerpoint) {
        try { references = inspectPptxPackageSlides(initial.bytes); }
        catch { return backend; } // Unknown/signed metadata retains the native guard.
    }
    const history = new BinaryDocumentHistory(initial.bytes, { saved: !newDocument&&!initialStatus.result.dirty });
    let current = backend, staged = null, closed = false, busy = false, closing;
    let image = null, selection = null, selectionPrepared = null, savedSnapshot = null;
    let sectionRows = null, opacityStates = new Map(), backgroundKinds = new Map(), backgroundStates = new Map();
    const pictureStates=new Map(),pictureParts=new Map(),pictureResources=new Map();
    const themeStates=new Map(),glowStates=new Map(),textRangeStates=new Map(),tableRangePermissions=new Map();let incomingTheme=null;
    const tableMergeStates=new Map();
    const tableDesignStates=new Map();
    let incomingSlides=null,outgoingSlides=null,slideOrigin=null;
    const slideStates=new Map([[history.stateId,new Map((references?.slides||[]).map(slide=>[slide.id,crypto.randomUUID()]))]]);
    const token=async bytes=>'slides:sha256:'+Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),byte=>byte.toString(16).padStart(2,'0')).join('');
    const clipboardStatus=(status,resource,extra={})=>({...status,result:{...status.result,selection:resource.token,selectedSlideCount:resource.count,clipboardMime:SLIDE_CLIPBOARD_MIME,clipboardVersion:1,...extra}});
    const alive = () => { if (closed || signal?.aborted) throw failure('CLOSED','Deck document is closed'); };
    const expected = value => {
        if (value !== undefined && revision(value) !== history.revision) reject('REVISION_CONFLICT','The presentation changed before this operation');
    };
    function reject(code, message) {
        checkedResponse({ ok: false, revision: history.revision, rejectedBeforeMutation: true,
            error: { code: /^[A-Z_]{1,64}$/u.test(code || '') ? code : 'INVALID_REQUEST', message: String(message).slice(0,1024) } });
    }
    function response(value) {
        const result = { ...value.result };
        if (Object.hasOwn(result,'dirty')) result.dirty = history.dirty;
        if (Object.hasOwn(result,'canUndo')) result.canUndo = history.canUndo;
        if (Object.hasOwn(result,'canRedo')) result.canRedo = history.canRedo;
        if (Object.hasOwn(result,'format') && Object.hasOwn(result,'slideCount')) {result.sectionSupport = Boolean(references);result.slideBatchSupport=true;result.slideClipboardSupport=powerpoint;}
        return { ...value, revision: history.revision, result };
    }
    async function textRanges(slide){
        if(!textRangeStates.has(slide)){
            try{const ranges=powerpoint?inspectPptxTextRanges(history.snapshot(),slide):await inspectOdpTextRanges(history.snapshot(),slide);alive();textRangeStates.set(slide,ranges);}
            catch{alive();textRangeStates.set(slide,new Map());}
        }
        return textRangeStates.get(slide);
    }
    async function tableDesign(slide,object){
        if(!powerpoint)return null;
        const key=JSON.stringify([slide,object]);
        if(tableDesignStates.has(key)){const value=tableDesignStates.get(key);tableDesignStates.delete(key);tableDesignStates.set(key,value);return value;}
        let value=null;
        try{
            const bytes=history.snapshot(),source=inspectPptxTableDesign(bytes,slide,object);
            const projection=source.design?readPptxTableDesignProjection(bytes,slide,object):null;
            value={source,projection,canSetDesign:false};
            const index=references.slides.findIndex(row=>row.id===slide),grid=inspectPptxTableMerges(bytes,slide,object);
            if(index>=0){await admitNativeTableMerge(current,index,{slide,object},grid,alive,{after:true,projectCell:projection?cell=>projection.cell(cell):null});value.canSetDesign=true;}
        }catch{alive();/* Native/source permission remains conservative. Known read-only styles may still be projected. */}
        if(tableDesignStates.size>=8)tableDesignStates.delete(tableDesignStates.keys().next().value);
        tableDesignStates.set(key,value);return value;
    }
    function rangeCapability(text,source,editable){
        text.rangeEditable=Boolean(editable&&text.editable&&source&&!source.fields&&source.text===text.text);
        if(text.rangeEditable&&text.text===''&&!text.paragraphs?.length)text.paragraphs=[{start:0,end:0,style:{...text.style},lineHeight:null,spaceBefore:null,spaceAfter:null}];
        const formats=source?.paragraphs;
        text.paragraphFormatEditable=Boolean(powerpoint&&text.rangeEditable&&formats?.length===text.paragraphs?.length&&formats.every((p,i)=>p.start===text.paragraphs[i].start&&p.end===text.paragraphs[i].end));
        if(powerpoint&&formats?.length===text.paragraphs?.length&&formats.every((p,i)=>p.start===text.paragraphs[i].start&&p.end===text.paragraphs[i].end))
            text.paragraphs=text.paragraphs.map((p,i)=>({...p,paragraphFormat:structuredClone(formats[i].paragraphFormat)}));
    }
    function close() {
        if (closing) return closing;
        closed = true; signal?.removeEventListener('abort', abort);
        image = selection = selectionPrepared = null;
        incomingSlides=outgoingSlides=slideOrigin=null;slideStates.clear();
        pictureStates.clear();pictureParts.clear();pictureResources.clear();tableDesignStates.clear();
        themeStates.clear();glowStates.clear();textRangeStates.clear();tableRangePermissions.clear();tableMergeStates.clear();incomingTheme=null;
        closing = Promise.allSettled([current.close(), staged?.close()]).then(() => {});
        return closing;
    }
    function abort() { void close(); }
    signal?.addEventListener('abort', abort, { once: true });
    async function run(work) {
        alive(); if (busy) throw failure('BUSY','A Deck transaction is already in progress');
        busy = true;
        try { return await work(); }
        catch (cause) {
            if (closed || signal?.aborted) throw failure('CLOSED','Deck document closed during the operation');
            // The committed worker was not touched by candidate failures. A
            // genuine transport failure of that worker must still close the owner.
            try { await current.exchange('status'); alive(); }
            catch { void close(); throw cause; }
            if (rejectedRevision(cause,history.revision) !== null) throw cause;
            reject(cause?.code,cause?.message || 'The candidate could not be applied');
        } finally { busy = false; }
    }
    async function openStage(bytes) {
        const next = await reopen(bytes, initialStatus.result.format);
        if (closed || signal?.aborted) { await next.close(); alive(); }
        staged = next;
        // Import validation must not aggregate many large runs into the 64 KiB
        // semantic buffer. The editor pages subsequent runs independently.
        await next.exchange('project', { slide:0,startRun:0,limit:1 }); alive();
        return next;
    }
    async function dropStage() { const old = staged; staged = null; await old?.close(); }
    async function resources(group) {
        let picture=image;
        const background=group.find(command=>command.type==='set-slide-picture-background');
        if (background && background.asset!==picture?.asset) {
            // This resource was projected by the committed native document.
            // Transfer through its existing checked image channel; a fresh
            // candidate has not yet hydrated that document's resource index.
            const source=pictureResources.get(background.asset)??await current.imageBytes(background.asset,current.revision); alive();
            picture={bytes:source.bytes,asset:background.asset};
        }
        if (picture) {
            const prepared = await staged.prepareImage(picture.bytes,staged.revision); alive();
            if (prepared.result.asset !== picture.asset) throw failure('INVALID_RESPONSE','Candidate picture identity changed');
        }
        if (selection && selectionPrepared) {
            await staged.uploadSelection(selection); alive();
            const prepared = await staged.exchange('prepare-selection',{},staged.revision); alive();
            if (prepared.result.selection !== selectionPrepared) throw failure('INVALID_RESPONSE','Candidate selection identity changed');
        }
    }
    async function commitStage(change, status, bytes,{createdSlides=[],restore=false}={}) {
        const nextReferences = powerpoint?inspectPptxPackageSlides(bytes):null;
        const oldParts=new Map((references?.slides||[]).map(slide=>[slide.id,slide.relationship]));
        const previous=slideStates.get(history.stateId),created=new Set(createdSlides);
        const generations=restore?null:new Map((nextReferences?.slides||[]).map(slide=>[slide.id,
            !created.has(slide.id)&&oldParts.get(slide.id)===slide.relationship&&previous?.has(slide.id)?previous.get(slide.id):crypto.randomUUID()]));
        // A semantic reply must fit before ownership changes, including the
        // longest facade revision/booleans. Transport rejection cannot follow a
        // successful but unreportable document commit.
        encode({...status,revision:'18446744073709551615',result:{...status.result,sectionSupport:false,slideBatchSupport:true,slideClipboardSupport:false,dirty:false,canUndo:false,canRedo:false}});
        alive(); const old = current, next = staged;
        // Everything below is synchronous until ownership and history agree.
        change(); current = next; references = nextReferences; sectionRows = null; opacityStates.clear(); backgroundKinds.clear(); backgroundStates.clear(); staged = null; savedSnapshot = null;
        pictureStates.clear();pictureParts.clear();pictureResources.clear();themeStates.clear();glowStates.clear();textRangeStates.clear();tableRangePermissions.clear();tableMergeStates.clear();tableDesignStates.clear();incomingTheme=null;
        image = selection = selectionPrepared = null;
        incomingSlides=outgoingSlides=null;
        if(!restore)slideStates.set(history.stateId,generations);
        const retained=new Set(history.stateIds);for(const key of slideStates.keys())if(!retained.has(key))slideStates.delete(key);
        await old.close().catch(() => {});
        alive(); return response(status);
    }
    async function apply(commands) {
        const guid = () => `{${crypto.randomUUID().toUpperCase()}}`;
        commands = commands.map(command => command.type === 'add-section'
            ? {...command,id:command.id??guid(),defaultId:command.defaultId??guid()} : command);
        const packageCommand = command => TABLE_DESIGN_COMMANDS.has(command.type)||TABLE_RANGE_COMMANDS.has(command.type)||TABLE_MERGE_COMMANDS.has(command.type)||TEXT_RANGE_COMMANDS.has(command.type)||THEME_COMMANDS.has(command.type)||PACKAGE_BACKGROUND_COMMANDS.has(command.type)||OBJECT_OPACITY_COMMANDS.has(command.type)||command.type==='paste-slides'||command.type==='duplicate-selected-slides'||powerpoint&&(command.type === 'insert-table'||command.type === 'move-slide' || SECTION_COMMANDS.has(command.type));
        let bytes = history.snapshot();
        const created = { created:[],createdSlides:[],createdAnimations:[] };
        async function flush() {
            if(staged){bytes=(await staged.exportBytes()).bytes;alive();await dropStage();}
        }
        async function nativeGroup(group) {
            const structural=group.some(command=>['insert-slide','duplicate-slide','delete-slide','delete-selected-slides'].includes(command.type));
            let adapter;
            if(structural&&powerpoint){await flush();adapter=preparePptxPackageStructure(bytes);await openStage(adapter.bytes);}
            else if(!staged)await openStage(bytes);
            await resources(group);
            const reply=await staged.exchange('apply',{commands:group},staged.revision);alive();
            checkedMutation(reply,group);
            for(const key of Object.keys(created))if(Array.isArray(reply.result[key]))created[key].push(...reply.result[key]);
            if(adapter){await flush();bytes=adapter.restore(bytes,group,reply.result.createdSlides??[]).bytes;}
            return reply;
        }
        try {
            for (let offset=0; offset<commands.length;) {
                const command=commands[offset];
                if(powerpoint&&command.type==='insert-table'){
                    const reply=await nativeGroup([command]),rows=reply.result.created;
                    if(!Array.isArray(rows)||rows.length!==1||rows[0].slide!==command.slide||typeof rows[0].object!=='string'||!rows[0].object)reject('INVALID_RESPONSE','Native table creation did not return one matching object');
                    await flush();bytes=initializeCreatedPptxTableDesign(bytes,rows[0]).bytes;alive();offset++;continue;
                }
                if(TABLE_DESIGN_COMMANDS.has(command.type)){
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','These source-backed table definitions require a PowerPoint presentation');
                    await flush();await openStage(bytes);
                    const index=inspectPptxPackageSlides(bytes).slides.findIndex(row=>row.id===command.slide);
                    if(index<0)reject('INVALID_REQUEST','Unknown table design slide');
                    const source=inspectPptxTableMerges(bytes,command.slide,command.object);
                    await admitNativeTableMerge(staged,index,command,source,alive,{after:true});
                    await dropStage();bytes=(command.type==='set-table-design'?editPptxTableDesign:editPptxTableOptions)(bytes,command).bytes;alive();await openStage(bytes);
                    const candidate=inspectPptxTableMerges(bytes,command.slide,command.object);
                    const projection=readPptxTableDesignProjection(bytes,command.slide,command.object);
                    if(!projection&&command.type==='set-table-design')reject('INVALID_RESPONSE','Candidate table definition was not recognized');
                    await admitNativeTableMerge(staged,index,command,candidate,alive,{after:true,projectCell:projection?cell=>projection.cell(cell):null});
                    offset++;continue;
                }
                if(TABLE_RANGE_COMMANDS.has(command.type)) {
                    await flush();await openStage(bytes);
                    const ids=powerpoint?inspectPptxPackageSlides(bytes).slides.map(item=>item.id):await odpSlideIds(readOdpSource(bytes)),slide=ids.indexOf(command.slide);
                    if(slide<0)reject('INVALID_REQUEST','Unknown table range slide');
                    const inspect=powerpoint?inspectPptxTableMerges:inspectOdpTableMerges;
                    const source=await inspect(bytes,command.slide,command.object);alive();
                    const range=await admitNativeTableMerge(staged,slide,command,source,alive);
                    const cells=source.cells.filter(cell=>!cell.covered&&cell.row>=range.firstRow&&cell.row<=range.lastRow&&cell.column>=range.firstColumn&&cell.column<=range.lastColumn);
                    // Native batches keep their existing 32-command/64 KiB bounds;
                    // the document owner commits all chunks as one transaction.
                    for(let start=0;start<cells.length;start+=32)await nativeGroup(cells.slice(start,start+32).map(({row,column})=>({type:'set-table-cell-style',slide:command.slide,object:command.object,row,column,style:command.style})));
                    offset++;continue;
                }
                if(TABLE_MERGE_COMMANDS.has(command.type)) {
                    await flush();await openStage(bytes);
                    const ids=powerpoint?inspectPptxPackageSlides(bytes).slides.map(item=>item.id):await odpSlideIds(readOdpSource(bytes)),slide=ids.indexOf(command.slide);
                    if(slide<0)reject('INVALID_REQUEST','Unknown table merge slide');
                    const inspect=powerpoint?inspectPptxTableMerges:inspectOdpTableMerges,edit=powerpoint?editPptxTableMerges:editOdpTableMerges;
                    const source=await inspect(bytes,command.slide,command.object);alive();
                    await admitNativeTableMerge(staged,slide,command,source,alive);
                    await dropStage();bytes=(await edit(bytes,[command])).bytes;alive();
                    await openStage(bytes);
                    const candidate=await inspect(bytes,command.slide,command.object);alive();
                    await admitNativeTableMerge(staged,slide,command,candidate,alive,{after:true});
                    offset++;continue;
                }
                if(TEXT_RANGE_COMMANDS.has(command.type)) {
                    if(command.type==='set-paragraph-format'&&!powerpoint)reject('UNSUPPORTED_OPERATION','Paragraph list source editing requires a PowerPoint presentation');
                    // Validate every operation against the actual candidate at
                    // that step. Range edits and ordinary native edits share a
                    // single staged import, atomic commit and Undo history.
                    await flush();await openStage(bytes);
                    const ids=powerpoint?inspectPptxPackageSlides(bytes).slides.map(item=>item.id):await odpSlideIds(readOdpSource(bytes)),slide=ids.indexOf(command.slide);
                    if(slide<0)reject('INVALID_REQUEST','Unknown text range slide');
                    const target=await nativeRangeText(staged,slide,command,alive);
                    const ranges=powerpoint?inspectPptxTextRanges(bytes,command.slide):await inspectOdpTextRanges(bytes,command.slide),source=ranges.get(textRangeTargetKey(command));
                    alive();
                    if(!source||source.fields||source.text!==target.text)reject('UNSUPPORTED_OPERATION','The source range does not match the native text');
                    await dropStage();bytes=(powerpoint?editPptxTextRanges(bytes,[command]):await editOdpTextRanges(bytes,[command])).bytes;alive();offset++;continue;
                }
                if(THEME_COMMANDS.has(command.type)){
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','Theme source editing requires a PowerPoint presentation');
                    await flush();await openStage(bytes);
                    const slides=inspectPptxPackageSlides(bytes).slides,slide=slides.findIndex(s=>s.id===command.slide);
                    if(slide<0)reject('INVALID_REQUEST','Unknown theme slide');
                    const page=await staged.exchange('scene',{slide,startObject:0,limit:1},staged.revision);alive();
                    if(!page.result.scene.canSetBackground)reject('UNSUPPORTED_OPERATION','The source presentation is not editable');
                    await dropStage();
                    if(command.type==='set-theme-source'){
                        if(!incomingTheme||incomingTheme.revision!==history.revision||incomingTheme.asset!==command.asset)reject('INVALID_RESOURCE','Prepare the matching theme file for this document revision');
                        bytes=importPptxTheme(bytes,incomingTheme.bytes,command.slide).bytes;
                    }else bytes=editPptxTheme(bytes,[command]).bytes;
                    await openStage(bytes);await staged.exchange('scene',{slide,startObject:0,limit:1},staged.revision);alive();
                    offset++;continue;
                }
                if(command.type==='set-slide-picture-background'&&(Object.hasOwn(command,'alignment')||Object.hasOwn(command,'mirror'))){
                    const {alignment,mirror,...native}=command;
                    if(!powerpoint&&((alignment??'tl')!=='tl'||(mirror??'none')!=='none'))reject('UNSUPPORTED_OPERATION','Picture alignment and mirroring require PowerPoint source');
                    // Retain native resource attachment, original source guards,
                    // decoder and conservative tile budget before adding options.
                    await nativeGroup([native]);await flush();
                    if(powerpoint)bytes=editPptxPictureOptions(bytes,command).bytes;
                    offset++;continue;
                }
                if(PACKAGE_BACKGROUND_COMMANDS.has(command.type)) {
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','These background fills are unavailable for this presentation format');
                    await flush();await openStage(bytes);
                    const slides=inspectPptxPackageSlides(bytes).slides;
                    const targets=command.slide===null?slides.map((s,i)=>i):[slides.findIndex(s=>s.id===command.slide)];
                    for(const slide of targets){
                        if(slide<0)reject('INVALID_REQUEST','Unknown slide');
                        const page=await staged.exchange('scene',{slide,startObject:0,limit:1},staged.revision);alive();
                        if(!page.result.scene.canSetBackground)reject('UNSUPPORTED_OPERATION','The slide background is not editable');
                    }
                    await dropStage();bytes=editPptxBackground(bytes,[command]).bytes;
                    if(command.type==='set-slide-background-style'){
                        // Picture styles keep their theme relationship. Accept
                        // them only after the same native decoder and tile
                        // bounds used for explicit picture backgrounds pass.
                        await openStage(bytes);const decoded=new Map();
                        for(const index of targets){
                            const record=inspectPptxPictureBackground(bytes,slides[index].id);if(!record)continue;
                            let resource=decoded.get(record.part);
                            if(!resource){const prepared=await staged.prepareImage(record.bytes,staged.revision);alive();resource=prepared.result;decoded.set(record.part,resource);}
                            const page=await staged.exchange('scene',{slide:index,startObject:0,limit:1},staged.revision);alive();
                            if(page.result.scene.size)pictureFillTiles(page.result.scene.size,resource,pictureSourceSettings(record));
                        }
                    }
                    offset++;continue;
                }
                if(OBJECT_OPACITY_COMMANDS.has(command.type)) {
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','Object transparency editing is unavailable for this presentation format');
                    const group=[];
                    while(offset<commands.length&&OBJECT_OPACITY_COMMANDS.has(commands[offset].type))group.push(commands[offset++]);
                    await flush();await openStage(bytes);
                    const slides=inspectPptxPackageSlides(bytes).slides;
                    for(const edit of group){
                        const slide=slides.findIndex(item=>item.id===edit.slide);
                        if(slide<0)reject('INVALID_REQUEST','Unknown slide');
                        let startObject=0,object=null;
                        do{
                            const page=await staged.exchange('scene',{slide,startObject,limit:64},staged.revision);alive();
                            object=page.result.scene.objects.find(item=>item.id===edit.object);
                            startObject=page.result.scene.nextObject;
                        }while(!object&&startObject!==null);
                        const paint=edit.type==='set-object-fill-opacity'?'fill':'stroke';
                        if(!object?.edit?.canStyle||!object[paint])reject('UNSUPPORTED_OPERATION','Select an editable solid paint');
                    }
                    await dropStage();bytes=editPptxObjectOpacity(bytes,group).bytes;continue;
                }
                if(command.type==='paste-slides') {
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','Slide clipboard transfer is unavailable for this presentation format');
                    if(!incomingSlides||incomingSlides.revision!==history.revision||incomingSlides.token!==command.selection||incomingSlides.count!==command.count)
                        reject('INVALID_CLIPBOARD','Prepare the matching slide clipboard before paste');
                    await flush();
                    const generations=slideStates.get(history.stateId),preservedSlides=slideOrigin?[...slideOrigin.generations].filter(([id,generation])=>generations?.get(id)===generation&&!created.createdSlides.includes(id)).map(([id])=>id):[];
                    const candidate=insertPptxSlideClipboard(bytes,incomingSlides.bytes,{after:command.after,identity:slideOrigin?.identity??null,preservedSlides});
                    bytes=candidate.bytes;created.createdSlides.push(...candidate.createdSlides);offset++;continue;
                }
                if(command.type==='duplicate-selected-slides') {
                    await flush();
                    let order;
                    if(powerpoint)order=inspectPptxPackageSlides(bytes).slides.map(s=>s.id);
                    else {
                        await openStage(bytes);order=[];let startSlide=0;
                        do{const page=await staged.exchange('outline',{startSlide,limit:64},staged.revision);alive();order.push(...page.result.slides.map(s=>s.id));startSlide=page.result.nextSlide;}while(startSlide!==null);
                    }
                    const selected=new Set(command.slides),sources=order.filter(id=>selected.has(id));
                    if(sources.length!==selected.size)reject('INVALID_REQUEST','Unknown slide in duplicate selection');
                    const before=order[order.indexOf(sources.at(-1))+1]??null,firstCreated=created.createdSlides.length;
                    await nativeGroup(sources.map(slide=>({type:'duplicate-slide',slide})));
                    const copies=created.createdSlides.slice(firstCreated),moves=copies.map(slide=>({type:'move-slide',slide,before}));
                    if(powerpoint)bytes=editPptxPackageReferences(bytes,moves).bytes;
                    else await nativeGroup(moves);
                    offset++;continue;
                }
                if(command.type==='remove-section'&&command.slides) {
                    await flush();
                    const section=inspectPptxPackageSlides(bytes).sections.find(s=>s.id.toUpperCase()===command.section.toUpperCase());
                    if(!section)reject('INVALID_REQUEST','Unknown section');
                    if(section.slides.length)await nativeGroup([{type:'delete-selected-slides',slides:[...section.slides]}]);
                    bytes=editPptxPackageReferences(bytes,[command]).bytes;offset++;continue;
                }
                if (packageCommand(commands[offset])) {
                    const moves = [];
                    while (offset<commands.length && packageCommand(commands[offset])&&!THEME_COMMANDS.has(commands[offset].type)&&!PACKAGE_BACKGROUND_COMMANDS.has(commands[offset].type)&&!OBJECT_OPACITY_COMMANDS.has(commands[offset].type)&&!['duplicate-selected-slides','paste-slides'].includes(commands[offset].type)&&!(commands[offset].type==='remove-section'&&commands[offset].slides)) moves.push(commands[offset++]);
                    await flush();
                    bytes = editPptxPackageReferences(bytes,moves).bytes;
                } else {
                    const group = [];
                    while (offset<commands.length && !packageCommand(commands[offset])) {
                        if (commands[offset].type==='set-slide-picture-background') {
                            if (!group.length) group.push(commands[offset++]);
                            break;
                        }
                        group.push(commands[offset++]);
                    }
                    await nativeGroup(group);
                }
            }
            if (!staged) await openStage(bytes);
            const snapshot = await staged.exportBytes(); alive();
            const status = await staged.exchange('status'); alive();
            const reply = { ...status,result:{...status.result,...created},revision:history.revision };
            checkedMutation(reply,commands);
            return await commitStage(() => history.commit(snapshot.bytes),reply,snapshot.bytes,{createdSlides:created.createdSlides});
        } finally { await dropStage(); }
    }
    async function historyStep(operation) {
        const bytes = operation === 'undo' ? history.undoBytes() : history.redoBytes();
        if (!bytes) return response(await current.exchange('status'));
        try {
            await openStage(bytes);
            const status = await staged.exchange('status'); alive();
            return await commitStage(() => history[operation](),status,bytes,{restore:true});
        } finally { await dropStage(); }
    }
    return Object.freeze({
        version:1,
        get revision() { return history.revision; },
        exchange(operation,args={},expectedRevision) {
            const checked = checkedRequest(operation,expectedRevision,args);
            return run(async () => {
                expected(checked.expectedRevision);
                if (operation==='apply') return apply(checked.arguments.commands);
                if (operation==='undo'||operation==='redo') return historyStep(operation);
                if(['export-slides','prepare-slides','discard-slides','confirm-slide-copy'].includes(operation)) {
                    if(!powerpoint)reject('UNSUPPORTED_OPERATION','Slide clipboard transfer is unavailable for this presentation format');
                    const status=response(await current.exchange('status'));alive();
                    if(operation==='discard-slides'){incomingSlides=outgoingSlides=null;return status;}
                    if(operation==='confirm-slide-copy') {
                        if(!outgoingSlides||outgoingSlides.revision!==history.revision||outgoingSlides.token!==checked.arguments.selection)reject('INVALID_CLIPBOARD','The copied slide snapshot is no longer available');
                        slideOrigin=outgoingSlides.origin;return status;
                    }
                    if(operation==='prepare-slides') {
                        if(!incomingSlides||incomingSlides.revision!==history.revision)reject('INVALID_CLIPBOARD','Upload a slide clipboard for the current revision');
                        return checkedSlideClipboardReply(clipboardStatus(status,incomingSlides),operation,history.revision);
                    }
                    const identity=crypto.randomUUID(),capsule=createPptxSlideClipboard(history.snapshot(),checked.arguments.slides,{identity});
                    const resource={bytes:capsule.bytes,count:capsule.slideCount,token:await token(capsule.bytes),revision:history.revision,origin:{identity,generations:new Map(slideStates.get(history.stateId))}};alive();
                    const reply=checkedSlideClipboardReply(clipboardStatus(status,resource,{length:resource.bytes.length}),operation,history.revision);encode(reply);
                    outgoingSlides=resource;return reply;
                }
                if (operation==='section-outline' || operation==='section-name') {
                    if(!references)reject('UNSUPPORTED_OPERATION','Sections are unavailable for this presentation format');
                    const status=response(await current.exchange('status')); alive();
                    if(operation==='section-name') {
                        const section=references.sections.find(s=>s.id.toUpperCase()===checked.arguments.section.toUpperCase());
                        if(!section)reject('INVALID_REQUEST','Unknown section');
                        if(new TextEncoder().encode(section.name).length>48000)reject('PAYLOAD_LIMIT','Section name exceeds the bounded editor query');
                        const reply=checkedSectionNameReply({...status,result:{...status.result,section:checked.arguments.section,name:section.name}},checked.arguments);
                        encode(reply);return reply;
                    }
                    const startSection=Math.min(checked.arguments.startSection,references.sections.length);
                    if(!sectionRows){let startSlide=0;sectionRows=references.sections.map((section,index)=>{
                        let name='',count=0;for(const character of section.name){if(count===256){count++;break;}name+=character;count++;}
                        const row={index,id:section.id,name,nameTruncated:count>256,startSlide,slideCount:section.slides.length};
                        startSlide+=row.slideCount;return row;
                    });}
                    const rows=sectionRows;
                    const sections=rows.slice(startSection,startSection+checked.arguments.limit),next=startSection+sections.length;
                    return checkedSectionReply({...status,result:{...status.result,sectionCount:rows.length,startSection,sections,nextSection:next<rows.length?next:null}},checked.arguments);
                }
                if(operation==='prepare-theme'){
                    expected(checked.expectedRevision);
                    if(!incomingTheme||incomingTheme.revision!==history.revision)reject('INVALID_RESOURCE','No theme file prepared for this revision');
                    const status=response(await current.exchange('status'));return checkedThemeReply({...status,result:{...status.result,themeAsset:incomingTheme.asset,themeName:incomingTheme.name,themeBytes:incomingTheme.bytes.length}},checked.expectedRevision);
                }
                if(operation==='discard-theme'){incomingTheme=null;return response(await current.exchange('status'));}
                if (operation==='mark-saved') {
                    if (savedSnapshot!==history.revision) reject('NO_SAVE_SNAPSHOT','No matching save snapshot');
                    history.markSaved(); return response(await current.exchange('status'));
                }
                const reply = await current.exchange(operation,checked.arguments,checked.expectedRevision===undefined?undefined:current.revision); alive();
                if(operation==='scene'&&!powerpoint){
                    const scene=reply.result.scene;
                    const ranges=await textRanges(scene.slideId);
                    for(const object of scene.objects)if(object.text)rangeCapability(object.text,ranges.get(object.id),scene.canSetBackground);
                }
                if(operation==='scene'&&powerpoint) {
                    const scene=reply.result.scene;
                    const ranges=await textRanges(scene.slideId);
                    if(!themeStates.has(scene.slideId)){
                        try{const value=inspectPptxTheme(history.snapshot(),scene.slideId);themeStates.set(scene.slideId,{supported:true,value});}
                        catch{try{inspectPptxThemeScope(history.snapshot(),scene.slideId);themeStates.set(scene.slideId,{supported:true,value:null});}catch{themeStates.set(scene.slideId,{supported:false,value:null});}}
                    }
                    const themeState=themeStates.get(scene.slideId);scene.canSetTheme=Boolean(scene.canSetBackground&&themeState.supported);
                    if(themeState.value){const {fonts,theme,...compact}=themeState.value;scene.designTheme=compact;}
                    if(!pictureStates.has(scene.slideId)){
                        let picture=null;
                        try{
                            const record=inspectPptxPictureBackground(history.snapshot(),scene.slideId);
                            if(record){
                                let resource=pictureParts.get(record.part);
                                if(!resource){
                                    const prepared=await current.prepareImage(record.bytes,current.revision);alive();
                                    if(prepared.result.mime!==record.mime||prepared.result.length!==record.bytes.length)throw failure('INVALID_RESPONSE','Background image identity changed');
                                    resource={...prepared.result,bytes:record.bytes};pictureParts.set(record.part,resource);pictureResources.set(resource.asset,resource);
                                }
                                const settings=pictureSourceSettings({...record,bytes:resource.bytes});
                                if(scene.size)pictureFillTiles(scene.size,resource,settings);
                                picture={...settings,image:{asset:resource.asset,mime:resource.mime,width:resource.width,height:resource.height,byteLength:resource.length,crop:{left:0,top:0,right:0,bottom:0},canCrop:false,canReplace:false}};
                            }
                        }catch(error){alive();/* Unsupported source or a rejected decoder stays opaque. */}
                        pictureStates.set(scene.slideId,picture);
                    }
                    const picture=pictureStates.get(scene.slideId);
                    if(picture){scene.backgroundPicture=picture;scene.unsupported=scene.unsupported.filter(code=>!['UNRESOLVED_SLIDE_BACKGROUND','UNSUPPORTED_BACKGROUND_IMAGE_RESOURCE_OR_SETTINGS'].includes(code));}
                    scene.canSetPictureFillOptions=Boolean(scene.canSetBackground);
                    if(!backgroundKinds.has(scene.slideId)){try{backgroundKinds.set(scene.slideId,inspectPptxBackgroundKind(history.snapshot(),scene.slideId));}catch{backgroundKinds.set(scene.slideId,'unknown');}}
                    scene.backgroundKind=backgroundKinds.get(scene.slideId);
                    if(!backgroundStates.has(scene.slideId)){try{backgroundStates.set(scene.slideId,inspectPptxBackground(history.snapshot(),scene.slideId));}catch{backgroundStates.set(scene.slideId,null);}}
                    const backgroundState=backgroundStates.get(scene.slideId);
                    if(backgroundState){
                        scene.backgroundKind=backgroundState.kind;
                        scene.canSetBackgroundFill=Boolean(scene.canSetBackground);scene.canSetBackgroundGraphics=Boolean(scene.canSetBackground);
                        scene.hideBackgroundGraphics=backgroundState.hidden;
                        scene.backgroundFill=backgroundState.fill??backgroundState.effectiveFill;
                        if(backgroundState.theme)scene.backgroundTheme=backgroundState.theme;
                        scene.backgroundStyle=backgroundState.style;scene.backgroundStyleCatalog=backgroundState.styles;scene.canSetBackgroundStyle=Boolean(scene.canSetBackground&&backgroundState.styles.some(s=>s.available));
                        if(scene.backgroundFill?.kind==='solid')scene.background={color:scene.backgroundFill.color,opacity:scene.backgroundFill.opacity};
                        if(scene.backgroundFill)scene.unsupported=scene.unsupported.filter(code=>code!=='UNRESOLVED_SLIDE_BACKGROUND');
                        if(!scene.backgroundFill && backgroundState.kind==='solid' && scene.background)scene.backgroundFill={kind:'solid',...scene.background};
                        if(!scene.backgroundFill && backgroundState.kind==='gradient' && scene.backgroundGradient){
                            const g=scene.backgroundGradient;
                            scene.backgroundFill={kind:'gradient',gradient:{kind:g.kind,angle:g.kind==='linear'?Math.atan2(g.end[1]-g.start[1],g.end[0]-g.start[0])*180/Math.PI:0,stops:g.stops}};
                        }
                    }
                    let states=opacityStates.get(scene.slideId);
                    if(!states){try{states=inspectPptxObjectOpacity(history.snapshot(),scene.slideId);}catch{states=new Map();}opacityStates.set(scene.slideId,states);}
                    if(!glowStates.has(scene.slideId)){try{glowStates.set(scene.slideId,inspectPptxObjectGlow(history.snapshot(),scene.slideId));}catch{glowStates.set(scene.slideId,new Map());}}
                    for(const object of scene.objects) {
                        if(object.text)rangeCapability(object.text,ranges.get(object.id),scene.canSetBackground);
                        const state=states.get(object.id);
                        if(object.edit.canStyle&&object.fill&&state?.fill)object.edit.canSetFillOpacity=true;
                        if(object.edit.canStyle&&object.stroke&&state?.line)object.edit.canSetLineOpacity=true;
                        const glow=glowStates.get(scene.slideId).get(object.id);
                        if(glow){object.glow={...glow};object.unsupported=object.unsupported.filter(code=>code!=='UNSUPPORTED_SHADOW_EFFECT_COMBINATION');}
                    }
                }
                if(operation==='table-cells'){
                    const {slide,object}=checked.arguments,ranges=await textRanges(slide),key=JSON.stringify([slide,object]);
                    const design=await tableDesign(slide,object);
                    if(design?.projection)reply.result.cells=reply.result.cells.map(cell=>design.projection.cell(cell));
                    if(!tableRangePermissions.has(key)){
                        const ids=powerpoint?references.slides.map(item=>item.id):await odpSlideIds(readOdpSource(history.snapshot())),index=ids.indexOf(slide);
                        const original=index<0?null:await nativeRangeObject(current,index,object,alive);
                        tableRangePermissions.set(key,Boolean(original?.editable&&original.target?.table?.canEditCells));
                    }
                    for(const cell of reply.result.cells)if(cell.text)rangeCapability(cell.text,ranges.get(cellKey(object,cell.row,cell.column)),tableRangePermissions.get(key)&&cell.editable&&!cell.covered);
                    while(true){try{encode(response(reply));break;}catch(error){
                        if(error.code!=='PAYLOAD_LIMIT')throw error;
                        if(reply.result.cells.length<=1)throw failure('RESPONSE_TOO_LARGE','Enriched cell exceeds its response budget');
                        reply.result.cells.pop();reply.result.nextCell=checked.arguments.startCell+reply.result.cells.length;
                    }}
                }
                if(operation==='scene'){
                    const scene=reply.result.scene;
                    for(const object of scene.objects)if(object.table){
                        const design=await tableDesign(scene.slideId,object.id);
                        if(design){object.table.design=design.source.design;object.table.designOptions=design.source.options;object.table.canSetDesign=Boolean(scene.canSetBackground&&object.table.canEditCells&&design.canSetDesign);}
                        const key=JSON.stringify([scene.slideId,object.id]);
                        tableRangePermissions.set(key,Boolean(scene.canSetBackground&&object.table.canEditCells));
                        if(!tableMergeStates.has(key)){
                            try{const source=await(powerpoint?inspectPptxTableMerges:inspectOdpTableMerges)(history.snapshot(),scene.slideId,object.id);alive();tableMergeStates.set(key,source.rows===object.table.rows&&source.columns===object.table.columns);}
                            catch{alive();tableMergeStates.set(key,false);}
                        }
                        object.table.canMergeCells=Boolean(scene.canSetBackground&&object.table.canEditCells&&tableMergeStates.get(key));
                    }
                    // The same semantic transport budget still applies after
                    // enriching native projections. Continue at the first
                    // omitted object instead of losing the rest of a slide.
                    while(true){try{encode(response(reply));break;}catch(error){
                        if(error.code!=='PAYLOAD_LIMIT')throw error;
                        if(scene.objects.length<=1){delete scene.designTheme;delete scene.canSetTheme;delete scene.backgroundStyle;delete scene.backgroundStyleCatalog;delete scene.canSetBackgroundStyle;delete scene.backgroundKind;delete scene.backgroundFill;delete scene.backgroundTheme;delete scene.canSetBackgroundFill;delete scene.canSetBackgroundGraphics;delete scene.hideBackgroundGraphics;for(const object of scene.objects){delete object.edit.canSetFillOpacity;delete object.edit.canSetLineOpacity;}encode(response(reply));break;}
                        scene.objects.pop();scene.nextObject=(checked.arguments.startObject||0)+scene.objects.length;
                    }}
                }
                if (operation==='begin-save') savedSnapshot=history.revision;
                if (operation==='discard-image') image=null;
                if (operation==='prepare-selection') selectionPrepared=reply.result.selection;
                if (operation==='discard-selection') selection=selectionPrepared=null;
                return response(reply);
            });
        },
        prepareImage(bytes,revision=history.revision) {
            const owned=bytes instanceof Uint8Array?bytes.slice():bytes;
            return run(async () => {
                expected(revision); const reply=await current.prepareImage(owned,current.revision); alive();
                image={bytes:owned,asset:reply.result.asset}; return response(reply);
            });
        },
        uploadTheme(bytes,revision=history.revision){
            const owned=bytes instanceof Uint8Array&&bytes.length>0&&bytes.length<=THEME_BYTES?bytes.slice():null;
            return run(async()=>{
                expected(revision);if(!powerpoint)reject('UNSUPPORTED_OPERATION','Theme-file import requires a PowerPoint presentation');incomingTheme=null;
                if(!owned)reject('FILE_LIMIT','Theme source exceeds the 8 MiB file profile');
                const info=inspectPresentationThemePackage(owned),asset='theme:sha256:'+Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',owned)),b=>b.toString(16).padStart(2,'0')).join('');alive();
                incomingTheme={bytes:owned,asset,name:info.name,revision:history.revision};
            });
        },
        uploadSelection(bytes) {
            const owned=bytes instanceof Uint8Array?bytes.slice():bytes;
            return run(async () => { await current.uploadSelection(owned); alive(); selection=owned; selectionPrepared=null; });
        },
        uploadSlides(bytes,revision=history.revision) {
            const owned=bytes instanceof Uint8Array&&bytes.length<=SLIDE_CLIPBOARD_BYTES?bytes.slice():null;
            return run(async()=>{
                expected(revision);if(!powerpoint)reject('UNSUPPORTED_OPERATION','Slide clipboard transfer is unavailable for this presentation format');
                incomingSlides=null;
                const info=inspectPptxSlideClipboard(owned),selection=await token(owned);alive();
                incomingSlides={bytes:owned,count:info.slideCount,token:selection,revision:history.revision};
            });
        },
        slideClipboardBytes(snapshot) {
            return run(async()=>{
                checkedSlideClipboardReply(snapshot,'export-slides',history.revision);
                if(!outgoingSlides||outgoingSlides.revision!==history.revision||outgoingSlides.token!==snapshot.result.selection||outgoingSlides.bytes.length!==snapshot.result.length||outgoingSlides.count!==snapshot.result.selectedSlideCount)
                    reject('INVALID_CLIPBOARD','The exported slide snapshot is no longer available');
                return outgoingSlides.bytes.slice();
            });
        },
        selectionBytes(snapshot) {
            return run(async () => {
                checkedSelectionReply(snapshot,'export-selection',history.revision);
                return current.selectionBytes({...snapshot,revision:current.revision});
            });
        },
        imageBytes(asset,revision=history.revision) {
            return run(async () => { expected(revision); const value=pictureResources.get(asset)??await current.imageBytes(asset,current.revision); alive(); return Object.freeze({...value,bytes:value.bytes.slice(),revision:history.revision}); });
        },
        exportBytes(revision=history.revision) {
            return run(async () => {
                expected(revision); const snapshot=await current.exportBytes(current.revision); alive(); savedSnapshot=history.revision;
                return Object.freeze({...snapshot,revision:history.revision});
            });
        },
        close,
    });
}
