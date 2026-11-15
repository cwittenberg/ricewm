import { tableTextScene } from './table-values.mjs';
import { checkedObjectCommand } from './authoring-contract.mjs';
import { encodeSelectionClipboard, decodeSelectionClipboard, selectionArguments } from './selection-contract.mjs';
import { installShapeCatalog } from './shape-catalog.mjs';
import { installLayoutCatalog } from './layout-catalog.mjs';
import { openWasmDeckBackend } from './wasm-engine.mjs';
import { connectDeckEnginePort } from './bus-engine-port.mjs';
import { BUFFER_BYTES, FILE_BYTES, FORMATS, failure } from './engine-contract.mjs';
import {Clipboard} from '../../versions/1.0.0/runtime/sdk/js/clipboard.mjs';
import {FileStore} from '../../versions/1.0.0/runtime/sdk/js/file-store.mjs';
import {SLIDE_CLIPBOARD_MIME,SLIDE_CLIPBOARD_BYTES,slideClipboardArguments} from '../../versions/1.0.0/runtime/sdk/js/presentation/slide-transfer-contract.mjs';

import { IMAGE_BYTES } from './image-contract.mjs';
import { checkedBackgroundPictureRequest } from './background-contract.mjs';

const MIMES = Object.freeze({
    pptx: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    potx: 'application/vnd.openxmlformats-officedocument.presentationml.template',
    ppsx: 'application/vnd.openxmlformats-officedocument.presentationml.slideshow',
    odp: 'application/vnd.oasis.opendocument.presentation',
    otp: 'application/vnd.oasis.opendocument.presentation-template',
});
const FORMAT_LABELS = Object.freeze({
    pptx: 'PowerPoint presentation (.pptx)', odp: 'OpenDocument presentation (.odp)',
    potx: 'PowerPoint template (.potx)', otp: 'OpenDocument template (.otp)',
    ppsx: 'PowerPoint show (.ppsx)',
});
const saveFilters = preferred => [preferred, ...Object.keys(FORMAT_LABELS).filter(format => format !== preferred)]
    .map(format => ({ name: FORMAT_LABELS[format], extensions: [format] }));
const extension = name => String(name || '').split('.').pop().toLowerCase();
const template = format => ['potx', 'otp'].includes(format);
const filename = (name, format) => `${String(name || 'Presentation').replace(/\.[^.]*$/u, '')}.${format}`;
const idle = () => ({ ok: true, revision: '0', result: { loaded: false, slide: null, nextRun: null, slideCount: 0, dirty: false, canUndo: false, canRedo: false } });

/** Owns a native window's document resources. A replacement opens in a spare
 * compute worker and is projected successfully before the old worker closes.
 * FileStore owns bounded atomic writes and version comparisons. The native
 * engine still imposes its 8 MiB format limit. Legacy clients retain their
 * existing 4 MiB whole-file path; a failed atomic save never falls back to it.
 */
export function createDeckDocumentPort({ app, services, bus, log, createTextLayout, createImageLayout, lifetime = new AbortController() } = {}) {
    const typography = createTextLayout?.({ services, signal: lifetime.signal });
    const pictures = createImageLayout?.({ services, signal: lifetime.signal });
    const slideClipboard=typeof services.clipboard?.readFormats==='function'&&typeof services.clipboard?.writeFormats==='function'?new Clipboard({services}):null;
    const transactionalFiles=['version','releaseVersion','beginWrite','writeChunk','commitWrite','abortWrite']
        .every(name=>typeof services.files?.[name]==='function'&&services.files[name].granted!==false);
    const fileStore=transactionalFiles?new FileStore({services,maximumBytes:FILE_BYTES}):null;
    let current = null, closed = false, busy = false, destination = null;
    let typographyWork = Promise.resolve(), typographyPending = 0, typographyBytes = 0;
    function renderText(scene, options) {
        alive();
        if (typographyPending >= 8) throw failure('BUSY', 'Text layout queue is full');
        if (!Array.isArray(scene?.objects) || scene.objects.length > 8192) throw failure('INVALID_REQUEST', 'Invalid text scene');
        // Typography owns a separate worker from the source document. Retain a
        // bounded immutable projection so typing, previews and a source read
        // share one font lane without rejecting each other's document work.
        const source = tableTextScene(scene);
        const snapshot = structuredClone({ slideId: source.slideId, designTheme: source.designTheme,
            objects: source.objects.map(({id,bounds,text,placeholder}) => ({id,bounds,text,placeholder})) });
        const bytes = new TextEncoder().encode(JSON.stringify(snapshot)).byteLength;
        if (bytes > 8 * 1024 * 1024 || typographyBytes + bytes > 32 * 1024 * 1024) throw failure('BUSY', 'Text layout source budget is full');
        const captured = options ? structuredClone(options) : undefined;
        typographyPending++; typographyBytes += bytes;
        const pending = typographyWork.catch(() => {}).then(() => {
            alive(); return typography ? typography.render(snapshot, captured) : { frames: new Map(), notices: ['Text rendering unavailable.'] };
        });
        typographyWork = pending;
        return pending.finally(() => { typographyPending--; typographyBytes -= bytes; });
    }
    let destinationVersion;
    const releaseFileVersion=async version=>{if(version&&fileStore)await fileStore.releaseVersion(version).catch(error=>log?.warn('Deck file version cleanup failed',{message:String(error.message)}));};
    async function setDestination(file,version){const previous=destinationVersion;destination=file;destinationVersion=version;if(previous!==version)await releaseFileVersion(previous);}
    let confirmDiscard = options => services.dialogs.confirm(options);
    const alive = () => { if (closed) throw failure('CLOSED', 'Deck document is closed'); };
    async function run(work) {
        alive();
        if (busy) throw failure('BUSY', 'A Deck document operation is in progress');
        busy = true;
        try { return await work(); } finally { busy = false; }
    }
    async function load(file, { editTemplate = false } = {}) {
        if (!file || typeof file.handle !== 'string' || !file.handle || typeof file.name !== 'string') throw failure('INVALID_FILE', 'Expected an authorized file descriptor');
        let inputVersion,adoptedVersion=false;
        try {
        const meta = await services.files.stat({ handle: file.handle });
        alive();
        if (meta?.kind !== 1 || !Number.isSafeInteger(meta.size) || meta.size < 1 || meta.size > FILE_BYTES) {
            throw failure('FILE_LIMIT', 'This Deck engine profile opens presentations up to 8 MiB');
        }
        let bytes;
        if(fileStore){
            inputVersion=(await fileStore.version({handle:file.handle})).version;alive();
            bytes=await fileStore.read({handle:file.handle,version:inputVersion},{signal:lifetime.signal});alive();
            if(!bytes.length)throw failure('READ_FAILED','Presentation is empty');
        }else{
        bytes = new Uint8Array(meta.size);
        for (let offset = 0; offset < bytes.length; offset += BUFFER_BYTES) {
            const length = Math.min(BUFFER_BYTES, bytes.length - offset);
            const chunk = await services.files.read({ handle: file.handle, offset, length });
            alive();
            if (!(chunk instanceof Uint8Array) || chunk.length !== length) throw failure('READ_FAILED', 'Presentation changed or could not be read completely');
            bytes.set(chunk, offset);
        }
        }
        let backend, port;
        try {
            const expected = extension(file.name);
            backend = await openWasmDeckBackend({ services, bytes, format: FORMATS.includes(expected) ? expected : undefined, signal: lifetime.signal });
            alive();
            port = connectDeckEnginePort({ bus, backend });
            // Prove a usable initial projection, including an empty deck.
            await port.project({ slide: 0, startRun: 0, limit: 32 });
            let status = await port.status();
            const sourceFormat = status.result.format;
            if (template(sourceFormat) && !editTemplate) {
                // A warmed editor already owns document/text/image workers.
                // Reuse its one spare slot; the previous document stays alive
                // while this temporary source is replaced by the validated copy.
                await port.close(); port = null; backend = null; alive();
                backend = await openWasmDeckBackend({ services, bytes, format: sourceFormat, copy: true, signal: lifetime.signal });
                port = connectDeckEnginePort({ bus, backend });
                await port.project({ slide: 0, startRun: 0, limit: 32 }); status = await port.status();
            }
            alive();
            const previous = current;
            current = { backend, port, format: status.result.format, name: filename(file.name, status.result.format) };
            const writable=file.writable===true&&!(template(sourceFormat)&&!editTemplate);
            await setDestination(writable?file:null,writable?inputVersion:undefined);adoptedVersion=writable;
            pictures?.clear();
            await previous?.port.close().catch(error => log?.warn('Previous Deck document cleanup failed', { message: String(error.message) }));
            return { replaced: true, name: current.name, fromTemplate: template(sourceFormat) && !editTemplate,
                startShow: sourceFormat === 'ppsx' };
        } catch (cause) {
            await port?.close().catch(() => {});
            await backend?.close().catch(() => {});
            throw cause;
        }
        }finally{if(!adoptedVersion)await releaseFileVersion(inputVersion);}
    }
    async function newDocument(format = 'pptx') {
        if (!await allowReplace()) return null;
        let backend, port;
        try {
            backend = await openWasmDeckBackend({ services, format, newDocument: true, initialLayout: 'title-slide', signal: lifetime.signal });
            alive(); port = connectDeckEnginePort({ bus, backend });
            await port.project({ slide: 0, startRun: 0, limit: 32 }); alive();
            alive();
            const previous = current;
            current = { backend, port, format, name: `Presentation.${format}` }; await setDestination(null); pictures?.clear();
            await previous?.port.close().catch(error => log?.warn('Previous Deck document cleanup failed', { message: String(error.message) }));
            return { replaced: true, name: current.name };
        } catch (error) { await port?.close().catch(() => {}); await backend?.close().catch(() => {}); throw error; }
    }
    async function allowReplace() {
        if (!current || !(await current.port.status()).result.dirty) return true;
        return await confirmDiscard({ title: 'Unsaved presentation', text: 'Discard the unsaved changes to this presentation?' }) === true;
    }
    async function save(asNew, variant = null) {
        if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
        const preferred = variant === 'template' ? (['odp', 'otp'].includes(current.format) ? 'otp' : 'potx')
            : variant === 'show' ? 'ppsx' : variant === 'presentation' ? (['odp', 'otp'].includes(current.format) ? 'odp' : 'pptx') : current.format;
        let target = asNew || variant ? null : destination;
        if (!target) {
            const files = await services.dialogs.saveFile({ title: 'Save presentation', filters: saveFilters(preferred),
                suggestedName: filename(current.name, preferred) });
            alive();
            target = files?.[0];
            if (!target) return null;
        }
        const targetFormat = extension(target.name);
        if (typeof target.handle !== 'string' || !target.handle || target.writable !== true || !FORMATS.includes(targetFormat)) {
            throw failure('FORMAT_MISMATCH', 'Choose a PowerPoint or OpenDocument presentation, template, or show file type');
        }
        let snapshot = await current.backend.exportBytes(current.port.revision);
        alive();
        let converted = null;
        try {
        if (targetFormat !== current.format) {
            converted = await openWasmDeckBackend({ services, bytes: snapshot.bytes, format: current.format,
                copy: true, targetFormat, signal: lifetime.signal });
            // Verify the converted package through its ordinary import path
            // before writing any destination or replacing the current document.
            await converted.exchange('project', { slide: 0, startRun: 0, limit: 32 });
            snapshot = await converted.exportBytes(converted.revision); alive();
        }
        if (snapshot.bytes.length > (fileStore?FILE_BYTES:4*1024*1024)) throw failure('SAVE_LIMIT', fileStore?'This presentation exceeds the engine profile limit of 8 MiB':'Saving above 4 MiB requires the transactional file services in the current Deck release');
        let written,nextVersion;
        if(fileStore){
            const result=await fileStore.write({handle:target.handle},snapshot.bytes,{signal:lifetime.signal,
                requireDurable:true,...(target===destination&&destinationVersion?{expectedVersion:destinationVersion}:{})});
            if(result?.durable!==true)throw failure('WRITE_FAILED','Storage did not acknowledge a durable presentation save');
            written=result.size;nextVersion=result.version;
        }else written = await services.files.write({ handle: target.handle, data: snapshot.bytes, requireDurable: true });
        alive();
        if (written !== snapshot.bytes.length) throw failure('WRITE_FAILED', 'Presentation write was incomplete');
        // The file has committed even if a later mark-saved acknowledgement
        // fails. Retain its new comparison token so a retry can succeed.
        await setDestination(target,nextVersion);
        await (converted || current.backend).exchange('mark-saved', {}, snapshot.revision);
        alive();
        const replaced = Boolean(converted);
        if (converted) {
            const previous = current;
            current = { backend: converted, port: connectDeckEnginePort({ bus, backend: converted }), format: targetFormat, name: target.name };
            converted = null; pictures?.clear();
            await previous.port.close().catch(error => log?.warn('Previous Deck document cleanup failed', { message: String(error.message) }));
        }
        current.name = target.name;
        return { saved: true, replaced, name: target.name };
        } finally { await converted?.close().catch(() => {}); }
    }
    async function choosePicture(title, commands) {
        if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
        const session = current;
        const files = await services.dialogs.openFile({ title, filters: ['image/png', 'image/jpeg'], multiple: false });
        alive();
        const file = files?.[0]; if (!file) return null;
        const meta = await services.files.stat({ handle: file.handle }); alive();
        if (meta?.kind !== 1 || !Number.isSafeInteger(meta.size) || meta.size < 1 || meta.size > IMAGE_BYTES) throw failure('IMAGE_LIMIT', 'Choose a PNG or JPEG up to 4 MiB');
        const bytes = new Uint8Array(meta.size);
        for (let offset = 0; offset < bytes.length; offset += BUFFER_BYTES) {
            const length = Math.min(BUFFER_BYTES, bytes.length - offset);
            const chunk = await services.files.read({ handle: file.handle, offset, length }); alive();
            if (!(chunk instanceof Uint8Array) || chunk.length !== length) throw failure('READ_FAILED', 'Picture changed or could not be read completely');
            bytes.set(chunk, offset);
        }
        try {
            const prepared = await session.backend.prepareImage(bytes, session.port.revision); alive();
            return await session.port.apply(commands(prepared.result));
        } finally {
            await session.backend.exchange('discard-image', {}, session.backend.revision).catch(() => {});
        }
    }
    return Object.freeze({
        fileWorkflow:Object.freeze({maximumPresentationBytes:FILE_BYTES,maximumSaveBytes:fileStore?FILE_BYTES:4*1024*1024,atomic:Boolean(fileStore),conflictProtection:Boolean(fileStore),durableRequired:true}),
        version: 1,
        readClipboardText: typeof services.clipboard?.readText === 'function' ? () => run(async () => {
            const text = await services.clipboard.readText(); alive();
            if (typeof text !== 'string') throw failure('INVALID_CLIPBOARD', 'Clipboard does not contain text');
            return text;
        }) : undefined,
        writeClipboardText: typeof services.clipboard?.writeText === 'function' ? text => run(async () => {
            if (typeof text !== 'string' || new TextEncoder().encode(text).length > 16_000) throw failure('INVALID_CLIPBOARD', 'Selected text exceeds the editor limit');
            await services.clipboard.writeText(text); alive(); return true;
        }) : undefined,
        setConfirmDiscard(handler) {
            if (typeof handler !== 'function') throw new TypeError('Expected a native confirmation handler');
            confirmDiscard = handler;
        },
        copySlides: slideClipboard?({slides,text='',cut=false}={})=>run(async()=>{
            if(!current)throw failure('NO_DOCUMENT','Open a presentation first');
            const session=current,request=slideClipboardArguments({slides}),revision=session.port.revision;
            let dispatched=false;
            if(typeof text!=='string'||new TextEncoder().encode(text).length>16000||typeof cut!=='boolean')throw failure('INVALID_REQUEST','Invalid slide clipboard text');
            try {
                const snapshot=await session.port.exportSlides(request,revision);alive();
                const bytes=await session.backend.slideClipboardBytes(snapshot);alive();
                const plain=new TextEncoder().encode(text);
                if(bytes.length+plain.length>SLIDE_CLIPBOARD_BYTES)throw failure('CLIPBOARD_LIMIT','Slides and their titles exceed the 4 MiB clipboard limit');
                await slideClipboard.write([{mimeType:SLIDE_CLIPBOARD_MIME,data:bytes},{mimeType:'text/plain',data:plain}]);alive();
                await session.port.confirmSlideCopy({selection:snapshot.result.selection},revision);alive();
                if(cut){dispatched=true;return await session.port.apply([{type:'delete-selected-slides',slides:request.slides}],revision);}
                return {copied:true,slideCount:snapshot.result.selectedSlideCount,revision:snapshot.revision};
            } catch(cause) {
                const error=cause instanceof Error?cause:failure('CLIPBOARD_UNAVAILABLE','Slide copy failed');
                if(!dispatched)error.rejectedBeforeMutation=true;throw error;
            } finally {await session.port.discardSlides().catch(()=>{});}
        }):undefined,
        pasteSlides: slideClipboard?({after=null}={})=>run(async()=>{
            if(!current)throw failure('NO_DOCUMENT','Open a presentation first');
            const session=current,revision=session.port.revision;
            let dispatched=false;
            try {
                const value=await slideClipboard.read([SLIDE_CLIPBOARD_MIME]);alive();
                if(value===null)return null;
                if(value?.mimeType!==SLIDE_CLIPBOARD_MIME||!(value.data instanceof Uint8Array)||!value.data.length||value.data.length>SLIDE_CLIPBOARD_BYTES)
                    throw failure('INVALID_CLIPBOARD','Invalid slide clipboard data');
                await session.backend.uploadSlides(value.data,revision);alive();
                const prepared=await session.port.prepareSlides(revision);alive();
                dispatched=true;return await session.port.apply([{type:'paste-slides',after,selection:prepared.result.selection,count:prepared.result.selectedSlideCount}],revision);
            } catch(cause) {
                const error=cause instanceof Error?cause:failure('CLIPBOARD_UNAVAILABLE','Slide paste failed');
                if(!dispatched)error.rejectedBeforeMutation=true;throw error;
            } finally {await session.port.discardSlides().catch(()=>{});}
        }):undefined,
        copySelection: args => run(async () => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            const session = current;
            if (typeof services.clipboard?.writeText !== 'function') throw failure('CLIPBOARD_UNAVAILABLE', 'Clipboard writing is unavailable');
            const request = selectionArguments(args);
            try {
                const snapshot = await session.port.exportSelection(request); alive();
                const bytes = await session.backend.selectionBytes(snapshot); alive();
                await services.clipboard.writeText(encodeSelectionClipboard(bytes)); alive();
                return { copied: true, objectCount: snapshot.result.objectCount, revision: snapshot.revision };
            } finally { await session.port.discardSelection().catch(() => {}); }
        }),
        pasteSelection: ({ slide, offset = { x: 20, y: 20 } } = {}) => run(async () => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            const session = current;
            let command, bytes;
            try {
                if (typeof services.clipboard?.readText !== 'function') throw failure('CLIPBOARD_UNAVAILABLE', 'Clipboard reading is unavailable');
                command = checkedObjectCommand({ type: 'paste-objects', slide, selection: 'selection:sha256:' + '0'.repeat(64), offset });
                const text = await services.clipboard.readText(); alive();
                bytes = decodeSelectionClipboard(text);
            } catch (cause) {
                const error = cause instanceof Error ? cause : failure('CLIPBOARD_UNAVAILABLE', 'Clipboard reading failed');
                error.rejectedBeforeMutation = true;
                throw error;
            }
            try {
                await session.backend.uploadSelection(bytes); alive();
                const prepared = await session.port.prepareSelection(); alive();
                return await session.port.apply([{ ...command, selection: prepared.result.selection }]);
            } finally { await session.port.discardSelection().catch(() => {}); }
        }),
        // Image discovery uses the same origin-scoped host network capability as other apps.
        // It can run alongside local authoring; only preparePicture locks the engine.
        findPicture: async topic => {
            const reply = await services.network.fetch({url:'https://en.wikipedia.org/api/rest_v1/page/summary/'+encodeURIComponent(topic.replaceAll(' ','_')),timeoutMs:8000});
            if(reply.status!==200)throw failure('IMAGE_UNAVAILABLE','No Wikipedia picture found for '+topic);
            const summary=JSON.parse(new TextDecoder().decode(reply.body));
            const value=summary.thumbnail?.source;
            if(!value)throw failure('IMAGE_UNAVAILABLE','This Wikipedia article has no picture: '+topic);
            const url=new URL(value);if(!['https://upload.wikimedia.org','https://thumb.wikimedia.org'].includes(url.origin))throw failure('IMAGE_UNAVAILABLE','Picture origin is unavailable');
            url.search='';url.pathname=url.pathname.replace(/\/\d+px-/,'/960px-');
            const image=await services.network.fetch({url:url.href,timeoutMs:8000});
            if(image.status!==200||!['image/png','image/jpeg'].includes((image.headers['content-type']||'').split(';')[0])||image.body.length>IMAGE_BYTES)throw failure('IMAGE_UNAVAILABLE','Picture could not be downloaded within the image limits');
            return {bytes:image.body,source:summary.content_urls?.desktop?.page||'https://en.wikipedia.org/wiki/'+encodeURIComponent(topic.replaceAll(' ','_'))};
        },
        preparePicture: bytes => run(async()=>{
            if(!current)throw failure('NO_DOCUMENT','Open a presentation first');
            const prepared=await current.backend.prepareImage(bytes,current.port.revision);return prepared.result;
        }),
        renderImages: (scene, options) => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return pictures ? pictures.render(scene, asset => current.backend.imageBytes(asset, current.port.revision), options)
                : { frames: new Map(), notices: [] };
        }),
        picture: ({ slide, size, replace = null, placeholder = null }) => run(() => choosePicture(replace ? 'Replace picture' : 'Insert picture', ({ asset, width, height }) => {
            const frame = placeholder?.box;
            const ratio = frame ? Math.min(frame.width / width, frame.height / height) : Math.min(1, size.width * 0.7 / width, size.height * 0.7 / height);
            const command = replace ? { type: 'replace-image', slide, object: replace, asset }
                : { type: 'insert-image', slide, asset, box: { x: frame ? frame.x + (frame.width - width * ratio) / 2 : (size.width - width * ratio) / 2,
                    y: frame ? frame.y + (frame.height - height * ratio) / 2 : (size.height - height * ratio) / 2, width: width * ratio, height: height * ratio } };
            return [command, ...(!replace && placeholder ? [{ type: 'delete-object', slide, object: placeholder.id }] : [])];
        })),
        importTheme: ({slide}={}) => run(async()=>{
            if(!current)throw failure('NO_DOCUMENT','Open a presentation first');
            if(typeof slide!=='string'||!slide||typeof current.backend.uploadTheme!=='function')throw failure('UNSUPPORTED_OPERATION','The source theme is not editable');
            const owner=current,revision=owner.port.revision,files=await services.dialogs.openFile({title:'Browse for Themes',filters:[{name:'Presentation themes',extensions:['thmx','pptx','potx','ppsx']}],multiple:false});alive();
            if(!files?.length)return null;
            if(current!==owner)throw failure('REVISION_CONFLICT','The presentation changed while choosing a theme');
            const file=files[0],meta=await services.files.stat({handle:file.handle});alive();
            if(meta?.kind!==1||!Number.isSafeInteger(meta.size)||meta.size<1||meta.size>FILE_BYTES)throw failure('FILE_LIMIT','Theme files use the presentation profile limit of 8 MiB');
            const bytes=new Uint8Array(meta.size);
            for(let offset=0;offset<bytes.length;offset+=BUFFER_BYTES){const length=Math.min(BUFFER_BYTES,bytes.length-offset),chunk=await services.files.read({handle:file.handle,offset,length});alive();if(!(chunk instanceof Uint8Array)||chunk.length!==length)throw failure('READ_FAILED','Theme file changed or could not be read completely');bytes.set(chunk,offset);}
            if(current!==owner)throw failure('REVISION_CONFLICT','The presentation changed while reading a theme');
            await owner.backend.uploadTheme(bytes,revision);alive();
            try{const prepared=await owner.port.prepareTheme(revision);alive();return await owner.port.apply([{type:'set-theme-source',slide,asset:prepared.result.themeAsset}],revision);}
            finally{if(current===owner&&!closed)await owner.port.discardTheme(owner.port.revision).catch(()=>{});}
        }),
        backgroundPicture: options => {
            const settings = checkedBackgroundPictureRequest(options);
            return run(() => choosePicture('Insert background picture', ({ asset }) => [{ type: 'set-slide-picture-background', ...settings, asset }]));
        },
        renderText,
        project: args => run(async () => {
            if (!current) return idle();
            const reply = await current.port.project(args);
            return { ...reply, result: { ...reply.result, loaded: true } };
        }),
        outline: (args, expected) => run(() => current ? current.port.outline(args, expected)
            : { ...idle(), result: { ...idle().result, startSlide: 0, slides: [], nextSlide: null, anchorIndex: null } }),
        sectionOutline: (args, expected) => run(() => current ? current.port.sectionOutline(args, expected)
            : { ...idle(), result: { ...idle().result, sectionCount: 0, startSection: 0, sections: [], nextSection: null } }),
        sectionName: (args, expected) => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.sectionName(args, expected);
        }),
        tableCells: (args, expected) => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.tableCells(args, expected);
        }),
        scene: (args, expected) => run(async () => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            if (!current.shapes) current.shapes = installShapeCatalog(await current.port.shapeCatalog());
            if (!current.layouts) current.layouts = installLayoutCatalog(await current.port.layoutCatalog());
            const reply = await current.port.scene(args, expected);
            if (app && !(BigInt(app.hostCapabilities || 0) & (1n << 27n)) &&
                reply.result.scene.objects.some(object => object.outerShadow)) {
                throw failure('HOST_UPDATE_REQUIRED', 'Reload the desktop and reopen Deck to display this presentation’s shadows.');
            }
            const objects = [];
            for (const object of reply.result.scene.objects) {
                if (!object.table) { objects.push(object); continue; }
                const cells = []; let startCell = 0;
                do {
                    let page, limit = 32;
                    for (;;) {
                        try { page = await current.port.tableCells({ slide: reply.result.scene.slideId, object: object.id, startCell, limit }, reply.revision); break; }
                        catch (error) { if (error.code !== 'RESPONSE_TOO_LARGE' || limit === 1) throw error; limit = Math.max(1, Math.floor(limit / 2)); }
                    }
                    if (page.result.totalCells !== object.table.rows * object.table.columns ||
                        page.result.cells.some((cell, offset) => cell.row * object.table.columns + cell.column !== startCell + offset)) {
                        throw failure('INVALID_RESPONSE', 'Table cells do not match their native table');
                    }
                    cells.push(...page.result.cells); startCell = page.result.nextCell;
                } while (startCell !== null);
                if (cells.length !== object.table.rows * object.table.columns) throw failure('INVALID_RESPONSE', 'Incomplete table cells');
                objects.push({ ...object, table: { ...object.table, cells } });
            }
            return { ...reply, result: { ...reply.result, scene: { ...reply.result.scene, objects } } };
        }),
        apply: commands => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.apply(commands);
        }),
        speakerNotes: (args, expected) => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.speakerNotes(args, expected);
        }),
        animationTimeline: (args, expected) => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.animationTimeline(args, expected);
        }),
        animationCatalog: expected => run(() => {
            if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first');
            return current.port.animationCatalog(expected);
        }),
        undo: () => run(() => { if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first'); return current.port.undo(); }),
        redo: () => run(() => { if (!current) throw failure('NO_DOCUMENT', 'Open a presentation first'); return current.port.redo(); }),
        documents: Object.freeze({
            new: () => run(() => newDocument()),
            'new-odp': () => run(() => newDocument('odp')),
            open: () => run(async () => {
                if (!await allowReplace()) return null;
                alive();
                const files = await services.dialogs.openFile({ title: 'Open presentation', filters: Object.values(MIMES), multiple: false });
                alive();
                return files?.[0] ? load(files[0]) : null;
            }),
            'open-template': () => run(async () => {
                if (!await allowReplace()) return null;
                const files = await services.dialogs.openFile({ title: 'Edit presentation template', filters: [MIMES.potx, MIMES.otp], multiple: false });
                alive(); return files?.[0] ? load(files[0], { editTemplate: true }) : null;
            }),
            // Only host-minted launch descriptors reach this method; paths are
            // never turned into authority by this application.
            openGranted: file => run(async () => {
                if (!await allowReplace()) return null;
                alive(); return load(file);
            }),
            save: () => run(() => save(false)),
            'save-as': () => run(() => save(true)),
            'save-template': () => run(() => save(true, 'template')),
            'save-show': () => run(() => save(true, 'show')),
            'save-presentation': () => run(() => save(true, 'presentation')),
        }),
        async close() {
            if (closed) return;
            closed = true; lifetime.abort();
            await Promise.allSettled([current?.port.close(), typography?.close(), pictures?.close(),releaseFileVersion(destinationVersion)]); current = null;destinationVersion=undefined;destination=null;
        },
    });
}
