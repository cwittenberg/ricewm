import { openWasmSheetBackend } from './wasm-engine.mjs';
import { connectSheetEnginePort, exposeSheetEngineService } from './bus-engine-port.mjs';
import { checkedWorkbookGrant } from './file-open.mjs';
import { openTypographyEngine } from '@ricewm/app';
import { createFontMetricsResolver } from './font-metrics.mjs';

import { workbookFormat, workbookFileFilters } from './workbook-formats.mjs';
/** Owns one document. Replacements are prepared in a spare worker and swapped
 * only after import and an initial projection succeed. The UI never owns cells.
 */
export async function openSheetDocumentPort(context) {
    let current, closed = false, busy = false, dirty = false, handle = null, sourceHandle = null, imported = false;
    let pendingOperation = null, closeCheck = null, closeApproved = false;
    const { services, bus } = context;
    const resolveFontMetrics = createFontMetricsResolver(()=>openTypographyEngine({services}));
    async function exclusive(operation) {
        if (closed) throw Error('Document is closed');
        if (busy || closeCheck || closeApproved) throw Object.assign(Error('A workbook operation or close request is already running'), {code:'BUSY'});
        busy = true;
        const pending = Promise.resolve().then(operation);
        pendingOperation = pending;
        try { return await pending; } finally { busy = false; if (pendingOperation === pending) pendingOperation = null; }
    }
    async function prepare(bytes = null, name = 'Untitled') {
        const format = bytes ? workbookFormat(name) : 'xlsx';
        let backend, provider, port;
        try {
            backend = await openWasmSheetBackend({ services, path: 'engine.wasm', create: { name }, resolveFontMetrics });
            if (bytes) {
                if (format === 'xlsx') await backend.importXlsx(bytes, { name });
                else await backend.importWorkbook(bytes, { name, format });
            }
            const service = `org.ricewm.sheet/engine/${crypto.randomUUID()}`;
            provider = exposeSheetEngineService({ bus, service, maximumSessions: 1, openBackend: () => backend });
            port = await connectSheetEnginePort({ bus, service });
            await port.readViewport({top:0,left:0,rows:40,columns:20}, null);
            if (closed) throw Error('Document closed while opening');
            const sensitivity = (await backend.sensitivity()).labels;
            return { backend, port, format, sensitivity, async close() { try { await port.close(); } finally { await provider.close(); } } };
        } catch (error) {
            await port?.close().catch(() => {});
            await provider?.close().catch(() => {});
            await backend?.close().catch(() => {});
            throw error;
        }
    }
    async function discardAllowed() {
        return !dirty || await services.dialogs.confirm({title:'Unsaved workbook',text:'Discard the unsaved changes to this workbook?'}) === true;
    }
    async function replace(bytes, name, source = null) {
        const replacement = await prepare(bytes, name);
        const previous = current;
        current = replacement; dirty = false; handle = null; sourceHandle = source?.handle || null; imported = Boolean(source);
        await previous.close().catch(error => context.log?.warn('Previous workbook cleanup failed', {message:String(error.message)}));
        return { replaced:true };
    }
    async function save(asNew, requestedFormat = asNew ? null : current.format) {
        if (closed) throw Error('Document is closed');
        if (requestedFormat !== null && !current.backend.fileFormats.includes(requestedFormat)) throw Error('This workbook engine does not support that file format');
        // Imported workbooks can contain features outside current codec coverage.
        // Keep the source grant read-only and require a separately chosen output.
        if (imported && !handle && await services.dialogs.confirm({title:'Save a workbook copy',
            text:'This development build supports a limited set of workbook features. Save a separate copy and keep your original workbook.'}) !== true) return null;
        let destination = asNew ? null : handle;
        if (!destination) {
            const formats = requestedFormat === null ? current.backend.fileFormats : [requestedFormat];
            const choices = await services.dialogs.saveFile({title:`Save workbook as ${formats.map(format => format.toUpperCase()).join(' or ')}`,
                filters:workbookFileFilters(formats),suggestedName:`Workbook.${requestedFormat || current.format}`,
                ...(sourceHandle ? {excludeHandles:[sourceHandle]} : {})});
            destination = choices?.[0] || null;
            if (!destination) return null;
        }
        if (closed) throw Error('Document is closed');
        const destinationFormat = workbookFormat(destination.name);
        if (!current.backend.fileFormats.includes(destinationFormat) || requestedFormat !== null && destinationFormat !== requestedFormat) {
            throw Error(`Choose a file ending in .${requestedFormat || current.format} for this format; no workbook bytes were written`);
        }
        if (['csv','tsv'].includes(destinationFormat) && await services.dialogs.confirm({
            title: `Save ${destinationFormat.toUpperCase()}`,
            text: 'Save displayed values from the active sheet? Text files do not include formulas, formatting, charts or other sheets. Use XLSX or ODS to keep those features.',
        }) !== true) return null;
        const bytes = destinationFormat === 'xlsx' ? await current.backend.exportXlsx() : await current.backend.exportWorkbook({format:destinationFormat});
        if (closed) throw Error('Document is closed');
        const written = await services.files.write({handle:destination.handle,data:bytes});
        if (written !== bytes.byteLength) throw Error('Workbook write was incomplete');
        handle = destination; current.format = destinationFormat; dirty = false;
        return { saved:true, name:destination.name };
    }
    current = await prepare();
    return Object.freeze({
        version:1,
        readViewport: (...args) => exclusive(() => current.port.readViewport(...args)),
        query: (data,options) => exclusive(async () => {
            const result = await current.port.query(data,options);
            if (['activate-filter', 'clear-filter'].includes(data.action) && result.result?.changed !== false) dirty = true;
            return result;
        }),
        requestClose() {
            if (closed || closeApproved) return Promise.resolve(true);
            if (closeCheck) return closeCheck;
            const pending = pendingOperation;
            closeCheck = (async () => {
                try {
                    if (pending) await pending;
                    if (closed) return true;
                    const allowed = await discardAllowed();
                    if (allowed === true) closeApproved = true;
                    return allowed === true;
                } catch { return false; }
                finally { closeCheck = null; }
            })();
            return closeCheck;
        },
        command: (command, options) => exclusive(async () => {
            const previousRevision = current.backend.revision;
            const result = await current.port.command(command, options);
            if (['edit-cell','fill-range','paste-range','clear-range','format-range','format-cells','apply-cell-style','paint-format','create-chart','update-chart','delete-chart','move-chart','sort-range','merge-cells','unmerge-cells','remove-duplicates','text-to-columns','create-defined-name','update-defined-name','delete-defined-name','undo','redo',
                'create-sheet','rename-sheet','delete-sheet','duplicate-sheet','move-sheet','toggle-autofilter','fill-down-adjacent',
                'insert-rows','delete-rows','insert-columns','delete-columns'].includes(command.type)) dirty = true;
            if (['resize-rows','resize-columns'].includes(command.type) && result.revision !== previousRevision) dirty = true;
            if (['undo','redo'].includes(command.type)) current.sensitivity = (await current.backend.sensitivity()).labels;
            return result;
        }),
        documents: Object.freeze({
            get sensitivity() { return current.sensitivity; },
            changeSensitivity: change => exclusive(async()=>{
                const result=await current.backend.sensitivity(change);
                current.sensitivity=result.labels;
                if(result.changed)dirty=true;
                return result;
            }),
            get format() { return current.format; },
            get formats() { return current.backend.fileFormats; },
            'open-grant': grant => exclusive(async () => {
                const file = checkedWorkbookGrant(grant);
                if (!current.backend.fileFormats.includes(workbookFormat(file.name))) throw Error('This engine cannot open that workbook format');
                if (!await discardAllowed()) return null;
                const bytes = await services.files.read({ handle: file.handle });
                return replace(bytes, file.name, file);
            }),
            new: () => exclusive(async () => { if (!await discardAllowed()) return null; return replace(null, 'Untitled'); }),
            open: () => exclusive(async () => {
                if (!await discardAllowed()) return null;
                const choices = await services.dialogs.openFile({title:'Open workbook',filters:workbookFileFilters(current.backend.fileFormats),multiple:false});
                if (!choices?.[0]) return null;
                const file = checkedWorkbookGrant(choices[0]);
                if (!current.backend.fileFormats.includes(workbookFormat(file.name))) throw Error('This engine cannot open that workbook format');
                const bytes = await services.files.read({handle:file.handle});
                return replace(bytes, file.name || 'Imported workbook', file);
            }),
            save: () => exclusive(() => save(false)),
            'save-as': () => exclusive(() => save(true)),
            'save-as-xlsx': current.backend.fileFormats.includes('xlsx') ? () => exclusive(() => save(true, 'xlsx')) : undefined,
            'save-as-ods': current.backend.fileFormats.includes('ods') ? () => exclusive(() => save(true, 'ods')) : undefined,
            'save-as-csv': current.backend.fileFormats.includes('csv') ? () => exclusive(() => save(true, 'csv')) : undefined,
            'save-as-tsv': current.backend.fileFormats.includes('tsv') ? () => exclusive(() => save(true, 'tsv')) : undefined,
        }),
        async close() { if (closed) return; closed = true; await current?.close(); },
    });
}
