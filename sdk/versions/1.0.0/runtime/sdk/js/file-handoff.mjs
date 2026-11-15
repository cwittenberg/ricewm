import {FileStore} from './file-store.mjs';
/** Save real format bytes before asking the desktop to open the completed file. */
export class FileHandoff {
    static controlFactory = Object.freeze({
        topics: ['integrations', 'product', 'visual', 'tables'],
        constructor: 'new FileHandoff({services})',
        methods:
            'handlers(mimeType), open({handle|path,mimeType,applicationId?}), status(id), wait(id,{timeoutMs?}), exportAndOpen({data,name,mimeType,file?,applicationId?}). file is an authorized {handle} or app-relative {path}; omitted file uses the native Save dialog. Cancellation returns {cancelled:true}; errors remain errors.',
        behavior:
            'Declare applications.handlers/open/status and files.saveDialog/write as needed. handlers returns installed MIME-compatible applications with real ids and defaults. open sends a protected RiceBus file-open request through the MIME registry. Omit applicationId to respect the user default; an explicit id must be a discovered compatible file handler. Cross-app open is asynchronous: queued means requested, opened means the host dispatched the file to its handler, not that document parsing succeeded. wait observes the actual host receipt. Exports require actual bytes from a format encoder; renaming CSV to XLSX does not convert it. No command strings, executable launch, package installation or default-handler changes are accepted.',
    });
    constructor({ services, store = null } = {}) {
        if (!services?.applications) throw TypeError('FileHandoff requires app services');
        this.services = services;
        this.store = store || (['beginWrite','writeChunk','commitWrite','abortWrite','releaseVersion'].every(key=>services.files?.[key]?.granted)?new FileStore({services}):null);
    }
    handlers(mimeType) {
        return this.services.applications.handlers({ mimeType });
    }
    open(options) {
        return this.services.applications.open(options);
    }
    status(id) {
        return this.services.applications.status({ id });
    }
    async wait(id, { timeoutMs = 30000 } = {}) {
        if (!Number.isInteger(timeoutMs) || timeoutMs < 0 || timeoutMs > 120000)
            throw RangeError('Invalid handoff deadline');
        const deadline = Date.now() + timeoutMs;
        for (;;) {
            const result = await this.status(id);
            if (result.status === 'opened') return result;
            if (['failed', 'cancelled'].includes(result.status))
                throw Object.assign(Error(result.error || 'File handoff failed'), {
                    code: 'HANDOFF_FAILED',
                    handoff: result,
                });
            if (Date.now() >= deadline) return result;
            await new Promise((resolve) => setTimeout(resolve, 100));
        }
    }
    async exportAndOpen({ data, name, mimeType, file, applicationId } = {}) {
        if (typeof name !== 'string' || !name || name.length > 255 || /[/\\\0\r\n]/u.test(name))
            throw TypeError('Export needs a filename');
        if (
            typeof mimeType !== 'string' ||
            !/^[a-z0-9][a-z0-9!#$&^_.+-]{0,126}\/[a-z0-9][a-z0-9!#$&^_.+-]{0,126}$/.test(mimeType)
        )
            throw TypeError('Export needs a concrete MIME type');
        const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
        if (!(bytes instanceof Uint8Array) || bytes.length > (this.store?64:4) * 1024 * 1024)
            throw RangeError(this.store?'Export requires at most 64 MiB of encoded bytes':'Exports above 4 MiB require granted transactional file services or an explicit FileStore');
        if (!file) {
            const picked = await this.services.files.saveDialog({ title: 'Export', suggestedName: name });
            file = Array.isArray(picked) ? picked[0] : picked;
            if (!file) return { cancelled: true };
        }
        const target = file.handle ? { handle: file.handle } : { path: file.path };
        let written;
        if(this.store){const result=await this.store.write(target,bytes,{requireDurable:true});if(result?.durable!==true)throw Error('Export durability was not acknowledged');written=result.size;if(result.version)await this.store.releaseVersion(result.version).catch(()=>{});}
        else written = await this.services.files.write({ ...target, data: bytes, requireDurable: true });
        if (written !== bytes.length) throw Error('Incomplete export write');
        const handoff = await this.open({ ...target, mimeType, ...(applicationId ? { applicationId } : {}) });
        return { cancelled: false, saved: true, file, bytes: written, handoff };
    }
}
