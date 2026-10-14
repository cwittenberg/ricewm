import {createRiceWasiPreview1} from './rice-wasi-preview1.mjs';

let compiled;
async function decoderModule() {
    if (!compiled) compiled = (async () => {
        const base = new URL(import.meta.url);
        const bundled = /\/(?:ricewm-runtime\.min\.mjs|runtime-worker-dependencies\.min\.js)$/.test(base.pathname);
        const url = new URL(bundled ? './ricewm/rice_xz.wasm' : './rice_xz.wasm', base);
        url.search = base.search;
        const response = await fetch(url);
        if (!response.ok) throw new Error('XZ decoder is unavailable');
        return WebAssembly.compile(await response.arrayBuffer());
    })().catch(error => { compiled = null; throw error; });
    return compiled;
}

/** Decode original archive bytes with bounded memory and cooperative cancellation. */
export async function decompressRiceXz(bytes, {
    maximumBytes = 256 * 1024 * 1024, signal, module,
} = {}) {
    if (!Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 256 * 1024 * 1024)
        throw new RangeError('Invalid XZ output limit');
    bytes = new Uint8Array(bytes);
    if (bytes.length > 256 * 1024 * 1024) throw new RangeError('XZ input exceeds size limit');
    const checkCancelled = () => { if (signal?.aborted) throw Object.assign(new Error('Archive operation was cancelled'), {code:'ECANCELED'}); };
    checkCancelled();
    const wasi = createRiceWasiPreview1();
    const instance = await WebAssembly.instantiate(module || await decoderModule(), {wasi_snapshot_preview1:wasi.imports});
    const api = instance.exports;
    wasi.attach(api.memory); api._initialize?.();
    const input = api.malloc(Math.max(1, bytes.length)), output = api.malloc(65536);
    let opened = false;
    try {
        if (!input || !output || !(opened = Boolean(api.rice_xz_open()))) throw new Error('XZ decoder memory limit exceeded');
        new Uint8Array(api.memory.buffer, input, bytes.length).set(bytes);
        const chunks = []; let offset = 0, size = 0;
        while (true) {
            checkCancelled();
            const status = api.rice_xz_step(input + offset, bytes.length - offset, output, 65536);
            const consumed = api.rice_xz_consumed(), produced = api.rice_xz_produced();
            offset += consumed; size += produced;
            if (size > maximumBytes) throw Object.assign(new Error('Expanded archive exceeds size limit'), {code:'EFBIG'});
            if (produced) chunks.push(new Uint8Array(api.memory.buffer, output, produced).slice());
            if (status === 1) {
                if (offset !== bytes.length) throw new Error('XZ archive has trailing data');
                break;
            }
            if (status !== 0 || (!consumed && !produced)) throw new Error(`Invalid or unsupported XZ archive (decoder ${status})`);
            await new Promise(resolve => setTimeout(resolve, 0));
        }
        const result = new Uint8Array(size); let cursor = 0;
        for (const chunk of chunks) { result.set(chunk, cursor); cursor += chunk.length; }
        return result;
    } finally {
        if (opened) api.rice_xz_close();
        if (input) api.free(input);
        if (output) api.free(output);
    }
}
