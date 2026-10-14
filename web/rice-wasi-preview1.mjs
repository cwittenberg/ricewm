const WASI_SUCCESS = 0;
const WASI_BADF = 8;
const WASI_SPIPE = 29;
const WASI_NOTCAPABLE = 76;

function defaultRandomFill(bytes) {
    if (globalThis.crypto?.getRandomValues) {
        globalThis.crypto.getRandomValues(bytes);
        return;
    }
    // The only current caller uses randomness to seed in-guest hash tables.
    // Keep non-browser test hosts functional without granting a new capability.
    let state = (Date.now() ^ 0x9e37_79b9) >>> 0;
    for (let index = 0; index < bytes.length; index += 1) {
        state ^= state << 13;
        state ^= state >>> 17;
        state ^= state << 5;
        bytes[index] = state & 0xff;
    }
}

function checkedRange(memory, pointer, length) {
    const start = pointer >>> 0;
    const end = start + (length >>> 0);
    if (
        !(memory instanceof WebAssembly.Memory) ||
        !Number.isSafeInteger(end) ||
        end < start ||
        end > memory.buffer.byteLength
    ) {
        throw new RangeError("WASI memory access is out of bounds");
    }
    return start;
}

/**
 * Minimal, capability-denying WASI Preview-1 adapter for rice_apps.wasm.
 *
 * It exists solely to support the Rust/C runtimes linked into the guest.
 * Bowl networking, storage, image resources, and rendering use explicit
 * bounded RAPP capabilities; this adapter does not expose a host filesystem.
 */
export function createRiceWasiPreview1({
    now = () => Date.now(),
    log = () => {},
    randomFill = defaultRandomFill,
    isolatedRenderer = false,
} = {}) {
    let memory = null;
    // Only the dedicated renderer worker opts in. The in-process fallback
    // retains an empty environment and the shared-desktop execution ceiling.
    const rendererEnvironment = isolatedRenderer === true
        ? new TextEncoder().encode("RICE_BOWL_ISOLATED_RENDERER=1\0")
        : null;
    const view = () => new DataView(memory.buffer);
    const writeU32 = (pointer, value) => {
        checkedRange(memory, pointer, 4);
        view().setUint32(pointer, value >>> 0, true);
    };
    const writeU64 = (pointer, value) => {
        checkedRange(memory, pointer, 8);
        view().setBigUint64(pointer, BigInt(value), true);
    };
    const imports = {
        environ_sizes_get(count, bytes) {
            writeU32(count, rendererEnvironment ? 1 : 0);
            writeU32(bytes, rendererEnvironment?.length || 0);
            return WASI_SUCCESS;
        },
        environ_get(pointers, buffer) {
            if (rendererEnvironment) {
                checkedRange(memory, buffer, rendererEnvironment.length);
                writeU32(pointers, buffer);
                new Uint8Array(memory.buffer, buffer, rendererEnvironment.length)
                    .set(rendererEnvironment);
            }
            return WASI_SUCCESS;
        },
        clock_time_get(clock, precision, output) {
            void clock;
            void precision;
            writeU64(output, BigInt(Math.floor(now())) * 1_000_000n);
            return WASI_SUCCESS;
        },
        random_get(output, length) {
            const start = checkedRange(memory, output, length);
            const bytes = new Uint8Array(memory.buffer, start, length >>> 0);
            randomFill(bytes);
            return WASI_SUCCESS;
        },
        sched_yield() {
            // The renderer is configured for sequential style resolution. A
            // cooperative yield therefore has no host scheduling work to do.
            return WASI_SUCCESS;
        },
        fd_write(fd, iovs, count, written) {
            checkedRange(memory, iovs, count * 8);
            const data = new Uint8Array(memory.buffer);
            const descriptor = view();
            const chunks = [];
            let total = 0;
            for (let index = 0; index < count; index += 1) {
                const offset = iovs + index * 8;
                const pointer = descriptor.getUint32(offset, true);
                const length = descriptor.getUint32(offset + 4, true);
                checkedRange(memory, pointer, length);
                chunks.push(data.slice(pointer, pointer + length));
                total += length;
            }
            writeU32(written, total);
            if (fd === 1 || fd === 2) {
                const merged = new Uint8Array(total);
                let offset = 0;
                for (const chunk of chunks) {
                    merged.set(chunk, offset);
                    offset += chunk.byteLength;
                }
                log(new TextDecoder().decode(merged), fd);
                return WASI_SUCCESS;
            }
            return WASI_BADF;
        },
        fd_close() {
            return WASI_BADF;
        },
        fd_fdstat_get(fd, output) {
            checkedRange(memory, output, 24);
            new Uint8Array(memory.buffer, output, 24).fill(0);
            if (fd > 2) return WASI_BADF;
            new DataView(memory.buffer).setUint8(output, 2);
            return WASI_SUCCESS;
        },
        fd_fdstat_set_flags() {
            return WASI_BADF;
        },
        fd_filestat_get() {
            return WASI_BADF;
        },
        fd_read(fd, iovs, count, read) {
            void fd;
            void iovs;
            void count;
            writeU32(read, 0);
            return WASI_BADF;
        },
        fd_prestat_get() {
            return WASI_BADF;
        },
        fd_prestat_dir_name() {
            return WASI_BADF;
        },
        fd_readdir() {
            return WASI_BADF;
        },
        fd_seek(fd, offset, whence, output) {
            void fd;
            void offset;
            void whence;
            writeU64(output, 0n);
            return WASI_SPIPE;
        },
        path_open() {
            return WASI_NOTCAPABLE;
        },
        path_filestat_get() {
            return WASI_NOTCAPABLE;
        },
        path_remove_directory() {
            return WASI_NOTCAPABLE;
        },
        path_rename() {
            return WASI_NOTCAPABLE;
        },
        path_unlink_file() {
            return WASI_NOTCAPABLE;
        },
        poll_oneoff() {
            return WASI_NOTCAPABLE;
        },
        proc_exit(code) {
            throw new Error(`rice_apps.wasm requested WASI exit ${code}`);
        },
    };
    return Object.freeze({
        imports: Object.freeze(imports),
        attach(nextMemory) {
            if (!(nextMemory instanceof WebAssembly.Memory)) {
                throw new TypeError("WASI adapter requires exported Wasm memory");
            }
            memory = nextMemory;
        },
    });
}
