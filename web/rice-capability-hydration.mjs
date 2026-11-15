const HASH = /^[a-f0-9]{64}$/;
const ID = /^[a-z0-9][a-z0-9._:-]{0,191}$/;
const MAX_ASSET_BYTES = 256 * 1024 * 1024;
const CACHE = 'ricewm-capability-assets-v1';

function aborted(signal) {
    if (signal?.aborted) throw signal.reason || new DOMException('Loading cancelled', 'AbortError');
}

export async function capabilityDigest(bytes) {
    const input = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
    const digest = await crypto.subtle.digest('SHA-256', input);
    return Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, '0')).join('');
}

/** One loading transaction per capability. Only verified assets reach adapters;
 * installation stays with the existing package, filesystem and runtime owners. */
export class RiceCapabilityHydrator {
    constructor({fetch: fetchAsset = globalThis.fetch?.bind(globalThis), caches = globalThis.caches,
        baseUrl = import.meta.url, onProgress = () => {}, locks = globalThis.navigator?.locks,
        maximumCacheBytes = 512 * 1024 * 1024} = {}) {
        this.fetch = fetchAsset;
        this.locks = locks;
        if (!Number.isSafeInteger(maximumCacheBytes) || maximumCacheBytes < 1) throw new TypeError('Invalid capability cache budget');
        this.maximumCacheBytes = maximumCacheBytes;
        this.caches = caches;
        this.baseUrl = baseUrl;
        this.onProgress = onProgress;
        this.definitions = new Map();
        this.operations = new Map();
        this.states = new Map();
    }

    register(definition, adapter = {}) {
        if (!definition || !ID.test(definition.id) || typeof definition.label !== 'string' ||
            !definition.label.trim() || definition.label.length > 256 ||
            !Array.isArray(definition.assets) || definition.assets.length > 128 ||
            !Array.isArray(definition.dependencies || []) ||
            (definition.dependencies || []).some(id => !ID.test(id) || id === definition.id)) {
            throw new TypeError('Invalid RiceWM capability');
        }
        const assets = definition.assets.map(asset => {
            const url = new URL(asset.url, this.baseUrl);
            if (!HASH.test(asset.sha256) || !Number.isSafeInteger(asset.bytes) ||
                asset.bytes < 1 || asset.bytes > MAX_ASSET_BYTES ||
                !['http:', 'https:'].includes(url.protocol) || url.username || url.password ||
                url.origin !== new URL(this.baseUrl).origin) {
                throw new TypeError('Invalid RiceWM capability asset');
            }
            return Object.freeze({...asset, url: url.href});
        });
        const current = this.definitions.get(definition.id);
        if (current) throw new Error(`Capability already registered: ${definition.id}`);
        const value = Object.freeze({...definition, assets: Object.freeze(assets),
            dependencies: Object.freeze([...(definition.dependencies || [])]), adapter});
        this.definitions.set(value.id, value);
        return value;
    }

    status(id) { return this.states.get(id) || Object.freeze({id, state:'idle'}); }

    async ensure(id, {signal} = {}) {
        aborted(signal);
        // Walk before joining a transaction: a cycle must reject rather than
        // deadlock two promises, including when both roots start concurrently.
        this.#checkDependencies(id, new Set());
        let operation = this.operations.get(id);
        if (!operation) {
            operation = this.#hydrate(this.definitions.get(id));
            this.operations.set(id, operation);
            void operation.finally(() => {
                if (this.operations.get(id) === operation) this.operations.delete(id);
            }).catch(() => {});
        }
        if (!signal) return operation;
        // A cancelled caller cannot interrupt another app sharing this download.
        return new Promise((resolve, reject) => {
            const cancel = () => reject(signal.reason || new DOMException('Loading cancelled', 'AbortError'));
            signal.addEventListener('abort', cancel, {once:true});
            if (signal.aborted) cancel();
            operation.then(resolve, reject).finally(() => signal.removeEventListener('abort', cancel));
        });
    }

    #checkDependencies(id, visiting) {
        const definition = this.definitions.get(id);
        if (!definition) throw new Error(`Capability is unavailable: ${id}`);
        if (visiting.has(id)) throw new Error(`Capability dependency cycle: ${id}`);
        visiting.add(id);
        for (const dependency of definition.dependencies) this.#checkDependencies(dependency, visiting);
        visiting.delete(id);
    }

    #publish(definition, state, detail = {}) {
        const value = Object.freeze({id:definition.id, label:definition.label, state, ...detail});
        this.states.set(definition.id, value);
        // A presentation failure must never change the installation outcome.
        try { this.onProgress(value); } catch { /* Owner can recover its UI separately. */ }
    }

    async #hydrate(definition) {
        try {
            for (const dependency of definition.dependencies) await this.ensure(dependency);
            if (await definition.adapter.isReady?.(definition)) {
                this.states.set(definition.id, Object.freeze({id:definition.id, state:'ready'}));
                return definition.adapter.value?.(definition);
            }
            this.#publish(definition, 'loading', {phase:'Preparing', loaded:0,
                total:definition.assets.reduce((sum, asset) => sum + asset.bytes, 0)});
            const transaction = await definition.adapter.begin?.(definition);
            let loaded = 0;
            for (const asset of definition.assets) {
                const bytes = await this.asset(asset, amount => this.#publish(definition, 'loading', {
                    phase:'Downloading', loaded:loaded + amount,
                    total:definition.assets.reduce((sum, value) => sum + value.bytes, 0),
                }));
                this.#publish(definition, 'loading', {phase:'Attaching', loaded:loaded + asset.bytes,
                    total:definition.assets.reduce((sum, value) => sum + value.bytes, 0)});
                await definition.adapter.attach?.(asset, bytes, transaction);
                loaded += asset.bytes;
            }
            this.#publish(definition, 'loading', {phase:'Preparing', loaded, total:loaded});
            await definition.adapter.commit?.(transaction, definition);
            this.#publish(definition, 'ready', {phase:'Ready', loaded, total:loaded});
            return definition.adapter.value?.(definition);
        } catch (error) {
            this.#publish(definition, 'failed', {phase:'Could not load', error:String(error?.message || error)});
            throw error;
        }
    }

    async asset(asset, onProgress = () => {}) {
        // One immutable download across desktop, terminal and renderer workers.
        // Check the shared cache after acquiring the lock, so concurrent first
        // activations reuse the first worker's verified response.
        if (this.locks?.request) return this.locks.request(`ricewm-capability-asset:${asset.sha256}`,
            () => this.#asset(asset, onProgress));
        return this.#asset(asset, onProgress);
    }

    async #asset(asset, onProgress) {
        let cache;
        try { cache = await this.caches?.open(CACHE); } catch { /* Optional cache. */ }
        // Use the digest as the cache identity, independent of release tokens.
        const cacheKey = new URL(`./.capability-cache/${asset.sha256}`, this.baseUrl).href;
        let cached;
        try { cached = await cache?.match(cacheKey); } catch { /* Optional cache. */ }
        if (cached) {
            try {
                const bytes = new Uint8Array(await cached.arrayBuffer());
                if (bytes.byteLength === asset.bytes && await capabilityDigest(bytes) === asset.sha256) {
                    onProgress(bytes.byteLength);
                    return bytes;
                }
            } catch { /* A damaged browser cache must fall back to the object. */ }
            await cache.delete(cacheKey).catch(() => {});
        }
        const response = await this.fetch(asset.url, {cache:'force-cache'});
        if (!response.ok) throw new Error(`Download failed (${response.status})`);
        const bytes = new Uint8Array(asset.bytes);
        let loaded = 0;
        if (response.body?.getReader) {
            const reader = response.body.getReader();
            try {
                for (;;) {
                    const {done, value} = await reader.read();
                    if (done) break;
                    if (value.byteLength > asset.bytes - loaded) throw new Error('Capability asset exceeds its declared size');
                    bytes.set(value, loaded); loaded += value.byteLength; onProgress(loaded);
                }
            } catch (error) { await reader.cancel(error).catch(() => {}); throw error; }
            finally { reader.releaseLock(); }
        } else {
            const value = new Uint8Array(await response.arrayBuffer());
            if (value.byteLength !== asset.bytes) throw new Error('Capability asset size mismatch');
            bytes.set(value); loaded = value.byteLength; onProgress(loaded);
        }
        if (loaded !== asset.bytes || await capabilityDigest(bytes) !== asset.sha256) {
            throw new Error('Capability asset integrity mismatch');
        }
        try {
            if (cache && asset.bytes <= this.maximumCacheBytes) {
                await cache.put(cacheKey, new Response(bytes, {headers:{'Content-Type':'application/octet-stream',
                    'Content-Length':String(asset.bytes)}}));
                const prune = ()=>this.#pruneCache(cache, cacheKey);
                if (this.locks?.request) await this.locks.request('ricewm-capability-cache-prune',prune);
                else await prune();
            }
        }
        catch { /* Successful verification and attachment do not depend on cache quota. */ }
        return bytes;
    }

    async #pruneCache(cache, preserve) {
        // CacheStorage preserves insertion order. Bound retained generations;
        // active modules keep their own compiled or attached backing in memory.
        const keys = await cache.keys();
        let total = 0;
        const entries = [];
        for (const key of keys) {
            const response = await cache.match(key);
            const declared = Number(response?.headers.get('Content-Length'));
            const bytes = Number.isSafeInteger(declared) && declared > 0 ? declared : MAX_ASSET_BYTES;
            total += bytes; entries.push({key,bytes});
        }
        for (const {key,bytes} of entries) {
            if (total <= this.maximumCacheBytes) break;
            if ((key.url || String(key)) === preserve) continue;
            await cache.delete(key); total -= bytes;
        }
    }
}
