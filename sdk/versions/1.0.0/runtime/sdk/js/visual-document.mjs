import { CommandHistory } from './command-history.mjs';
import { visualBounds } from './visual-geometry.mjs';
import { decodeVisualImage } from './visual-resources.mjs';

const MAX_OBJECTS = 20000,
    MAX_CONNECTIONS = 40000;
const id = (value) => typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,127}$/.test(value);
function data(value, depth = 0) {
    if (depth > 32) throw RangeError('Document data nesting exceeds 32');
    if (value === null || typeof value === 'boolean') return value;
    if (typeof value === 'string') {
        if (value.length > 4 * 1024 * 1024) throw RangeError('Document text exceeds budget');
        return value;
    }
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (Array.isArray(value)) {
        if (value.length > 100000) throw RangeError('Document collection exceeds budget');
        return value.map((v) => data(v, depth + 1));
    }
    if (value && Object.getPrototypeOf(value) === Object.prototype) {
        const out = {};
        for (const [key, child] of Object.entries(value)) {
            if (['__proto__', 'prototype', 'constructor'].includes(key))
                throw TypeError('Reserved document key');
            out[key] = data(child, depth + 1);
        }
        return out;
    }
    throw TypeError('Document accepts finite JSON data only');
}
export function validateVisualDocument(input) {
    const value = data(input);
    if (
        value.schema !== 'ricewm.visual-document/v1' ||
        !Array.isArray(value.objects) ||
        !Array.isArray(value.connections) ||
        !Array.isArray(value.tags) ||
        value.objects.length > MAX_OBJECTS ||
        value.connections.length > MAX_CONNECTIONS ||
        value.tags.length > 4096
    )
        throw TypeError('Invalid visual document');
    const objects = new Map(),
        connections = new Set(),
        tags = new Set();
    if (value.resources !== undefined) {
        if (
            !value.resources ||
            typeof value.resources !== 'object' ||
            Array.isArray(value.resources) ||
            Object.keys(value.resources).length > 128
        )
            throw TypeError('Invalid visual resources');
        let total = 0;
        for (const [key, resource] of Object.entries(value.resources)) {
            if (!id(key)) throw TypeError('Invalid resource identity');
            total += decodeVisualImage(resource).rgba.length;
            if (total > 16 * 1024 * 1024) throw RangeError('Embedded resource budget exceeded');
        }
    }
    for (const tag of value.tags) {
        if (
            !id(tag.id) ||
            tags.has(tag.id) ||
            typeof tag.label !== 'string' ||
            !tag.label ||
            tag.label.length > 256 ||
            (tag.color !== undefined && !/^#[0-9a-f]{6}$/i.test(tag.color))
        )
            throw TypeError('Invalid or duplicate tag');
        tags.add(tag.id);
    }
    for (const object of value.objects) {
        if (!id(object.id) || objects.has(object.id)) throw TypeError('Invalid or duplicate object id');
        visualBounds(object);
        if (
            (object.rotation !== undefined &&
                (!Number.isFinite(object.rotation) || Math.abs(object.rotation) > 36000)) ||
            (object.shape !== undefined &&
                !['rect', 'roundedRect', 'pill', 'ellipse', 'diamond', 'path'].includes(object.shape)) ||
            (object.text !== undefined && (typeof object.text !== 'string' || object.text.length > 32768)) ||
            (object.radius !== undefined &&
                (!Number.isFinite(object.radius) || object.radius < 0 || object.radius > 1e6))
        )
            throw TypeError('Invalid object presentation');
        if (
            ['locked', 'pinned', 'collapsed', 'droppable'].some(
                (key) => object[key] !== undefined && typeof object[key] !== 'boolean',
            )
        )
            throw TypeError('Invalid object state');
        if (object.shape === 'path') {
            const arity = { M: 2, L: 2, Q: 4, C: 6, Z: 0 };
            if (
                !Array.isArray(object.path) ||
                !object.path.length ||
                object.path.length > 16384 ||
                object.path.some(
                    (s) =>
                        !Array.isArray(s) ||
                        !Object.hasOwn(arity, s[0]) ||
                        s.length !== arity[s[0]] + 1 ||
                        s.slice(1).some((n) => !Number.isFinite(n) || Math.abs(n) > 1e9),
                )
            )
                throw TypeError('Invalid object path');
            object.pathViewport ??= { width: object.width, height: object.height };
            if (
                ![object.pathViewport.width, object.pathViewport.height].every(
                    (v) => Number.isFinite(v) && v > 0 && v <= 1e9,
                )
            )
                throw TypeError('Invalid custom path viewport');
        }
        if (object.style !== undefined) {
            if (!object.style || typeof object.style !== 'object' || Array.isArray(object.style))
                throw TypeError('Invalid object style');
            for (const key of ['fill', 'stroke', 'foreground'])
                if (object.style[key] !== undefined && !/^#[0-9a-f]{6}$/i.test(object.style[key]))
                    throw TypeError('Invalid object color');
            if (
                object.style.strokeWidth !== undefined &&
                (!Number.isFinite(object.style.strokeWidth) ||
                    object.style.strokeWidth < 0 ||
                    object.style.strokeWidth > 128)
            )
                throw TypeError('Invalid stroke width');
        }
        if (
            object.tags !== undefined &&
            (!Array.isArray(object.tags) ||
                object.tags.some((tag) => !tags.has(tag)) ||
                new Set(object.tags).size !== object.tags.length)
        )
            throw TypeError('Object references an unknown or duplicate tag');
        if (object.parent !== undefined && object.parent !== null && !id(object.parent))
            throw TypeError('Invalid parent');
        if (object.richText !== undefined) validateRichText(object.richText);
        if (
            object.image !== undefined &&
            (!id(object.image) || !Object.hasOwn(value.resources || {}, object.image))
        )
            throw TypeError('Object image resource is missing');
        objects.set(object.id, object);
    }
    const visited = new Set();
    for (const object of value.objects) {
        let current = object;
        const chain = new Set();
        while (current && !visited.has(current.id)) {
            if (chain.has(current.id)) throw TypeError('Cyclic object hierarchy');
            chain.add(current.id);
            if (current.parent && !objects.has(current.parent)) throw TypeError('Unknown parent');
            current = objects.get(current.parent);
        }
        for (const key of chain) visited.add(key);
    }
    for (const edge of value.connections) {
        if (
            !id(edge.id) ||
            connections.has(edge.id) ||
            !objects.has(edge.source) ||
            !objects.has(edge.target) ||
            (edge.routing !== undefined && !['straight', 'curve', 'orthogonal'].includes(edge.routing)) ||
            (edge.color !== undefined && !/^#[0-9a-f]{6}$/i.test(edge.color)) ||
            (edge.arrow !== undefined && typeof edge.arrow !== 'boolean')
        )
            throw TypeError('Invalid connection');
        for (const key of ['sourcePort', 'targetPort'])
            if (edge[key] !== undefined && !['left', 'right', 'top', 'bottom', 'center'].includes(edge[key]))
                throw TypeError('Invalid port');
        if (edge.label !== undefined && (typeof edge.label !== 'string' || edge.label.length > 1024))
            throw TypeError('Invalid connector label');
        connections.add(edge.id);
    }
    return value;
}
export function createVisualDocument({
    objects = [],
    connections = [],
    tags = [],
    metadata = {},
    resources = {},
} = {}) {
    return validateVisualDocument({
        schema: 'ricewm.visual-document/v1',
        objects,
        connections,
        tags,
        metadata,
        resources,
    });
}
export function serializeVisualDocument(document) {
    return JSON.stringify(validateVisualDocument(document));
}
export function parseVisualDocument(source) {
    if (typeof source !== 'string' || source.length > 32 * 1024 * 1024)
        throw RangeError('Visual document text exceeds budget');
    return validateVisualDocument(JSON.parse(source));
}
export function createVisualHistory(value = createVisualDocument(), options = {}) {
    return new CommandHistory({ ...options, value, validate: validateVisualDocument });
}
export function validateRichText(value) {
    if (!Array.isArray(value) || value.length > 4096) throw TypeError('Rich text requires bounded blocks');
    let length = 0;
    for (const block of value) {
        if (
            !block ||
            !['paragraph', 'heading', 'checklist', 'bullet', 'code'].includes(block.type) ||
            !Array.isArray(block.runs) ||
            block.runs.length > 1024 ||
            (block.checked !== undefined && typeof block.checked !== 'boolean')
        )
            throw TypeError('Invalid rich text block');
        for (const run of block.runs) {
            if (
                !run ||
                typeof run.text !== 'string' ||
                (length += run.text.length) > 32768 ||
                Object.keys(run).some((k) => !['text', 'bold', 'italic', 'underline', 'link'].includes(k)) ||
                ['bold', 'italic', 'underline'].some(
                    (k) => run[k] !== undefined && typeof run[k] !== 'boolean',
                ) ||
                (run.link !== undefined &&
                    (typeof run.link !== 'string' || !/^https?:\/\/[^\s]+$/i.test(run.link)))
            )
                throw TypeError('Invalid rich text run');
        }
    }
    return structuredClone(value);
}
export function richTextPlainText(value) {
    return validateRichText(value)
        .map((block) => block.runs.map((run) => run.text).join(''))
        .join('\n');
}
export function encodeVisualSelection(document, ids) {
    const value = validateVisualDocument(document),
        wanted = new Set(ids),
        byId = new Map(value.objects.map((o) => [o.id, o]));
    for (const object of value.objects) {
        let parent = object.parent;
        while (parent) {
            if (wanted.has(parent)) {
                wanted.add(object.id);
                break;
            }
            parent = byId.get(parent)?.parent;
        }
    }
    const objects = value.objects
            .filter((o) => wanted.has(o.id))
            .map((o) => ({ ...o, parent: wanted.has(o.parent) ? o.parent : null })),
        tagIds = new Set(objects.flatMap((o) => o.tags || []));
    const imageIds = new Set(objects.map((o) => o.image).filter(Boolean));
    return JSON.stringify({
        schema: value.schema,
        metadata: {},
        objects,
        tags: value.tags.filter((t) => tagIds.has(t.id)),
        resources: Object.fromEntries(
            Object.entries(value.resources || {}).filter(([key]) => imageIds.has(key)),
        ),
        connections: value.connections.filter((e) => wanted.has(e.source) && wanted.has(e.target)),
    });
}
export function pasteVisualSelection(history, text, { offset = { x: 24, y: 24 }, prefix = 'paste', metadata } = {}) {
    const source = parseVisualDocument(text);
    if (metadata !== undefined) {
        if (!metadata || Object.getPrototypeOf(metadata) !== Object.prototype) throw TypeError('Paste metadata must be a plain object');
        metadata = data(metadata);
    }
    if (!id(prefix) || !Number.isFinite(offset.x) || !Number.isFinite(offset.y))
        throw TypeError('Invalid paste options');
    const ids = new Map(),
        used = new Set(
            [...history.value.objects, ...history.value.connections, ...history.value.tags]
                .map((o) => o.id)
                .concat(Object.keys(history.value.resources || {})),
        );
    let sequence = 0;
    const fresh = () => {
        let key;
        do {
            key = prefix.slice(0, 100) + '-' + ++sequence;
        } while (used.has(key));
        used.add(key);
        return key;
    };
    for (const object of source.objects) ids.set(object.id, fresh());
    history.transact('Paste', (draft) => {
        const resources = new Map();
        draft.resources ??= {};
        for (const [id, resource] of Object.entries(source.resources || {})) {
            const existing = Object.entries(draft.resources).find(
                ([, r]) => JSON.stringify(r) === JSON.stringify(resource),
            );
            let key = existing?.[0];
            if (!key) {
                key = fresh();
                draft.resources[key] = resource;
            }
            resources.set(id, key);
        }
        const tagIds = new Map();
        for (const tag of source.tags) {
            const match = draft.tags.find((t) => t.label === tag.label && t.color === tag.color);
            if (match) tagIds.set(tag.id, match.id);
            else {
                const key = fresh();
                tagIds.set(tag.id, key);
                draft.tags.push({ ...tag, id: key });
            }
        }
        draft.objects.push(
            ...source.objects.map((o) => ({
                ...o,
                id: ids.get(o.id),
                parent: ids.get(o.parent) || null,
                x: o.x + offset.x,
                y: o.y + offset.y,
                ...(o.tags ? { tags: o.tags.map((t) => tagIds.get(t)) } : {}),
                ...(o.image ? { image: resources.get(o.image) } : {}),
                ...(metadata ? { metadata: { ...o.metadata, ...metadata } } : {}),
            })),
        );
        draft.connections.push(
            ...source.connections.map((e) => ({
                ...e,
                id: fresh(),
                source: ids.get(e.source),
                target: ids.get(e.target),
            })),
        );
    });
    return [...ids.values()];
}
/** Document file workflow independent of any application or format engine. */
export class VisualDocumentSession {
    static controlFactory = Object.freeze({
        topics: ['visual', 'scene'],
        constructor: 'new VisualDocumentSession({services,history})',
        methods:
            'open(file), save({file}), importSelection(text), close(); uses granted file handles and durable writes',
    });
    constructor({ services, history = createVisualHistory() } = {}) {
        if (!services?.files) throw TypeError('Document session requires file services');
        this.services = services;
        this.history = history;
        this.file = null;
        this.generation = 0;
        this.closed = false;
        this.saves = Promise.resolve();
    }
    async open(file) {
        if (this.closed) throw Error('Document session closed');
        const generation = ++this.generation,
            revision = this.history.revision;
        await this.saves.catch(() => {});
        const stat = await this.services.files.stat({ handle: file.handle });
        if (!Number.isSafeInteger(stat.size) || stat.size < 1 || stat.size > 32 * 1024 * 1024)
            throw RangeError('Document file exceeds budget');
        const bytes = new Uint8Array(stat.size);
        for (let offset = 0; offset < bytes.length; offset += 65536) {
            const length = Math.min(65536, bytes.length - offset),
                chunk = await this.services.files.read({ handle: file.handle, offset, length });
            if (!(chunk instanceof Uint8Array) || chunk.length !== length)
                throw Error('Incomplete document read');
            bytes.set(chunk, offset);
        }
        const next = parseVisualDocument(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
        if (this.closed || generation !== this.generation || revision !== this.history.revision)
            throw Error('Document changed while opening');
        this.history.replace(next);
        this.file = file;
        return next;
    }
    async save({ file = this.file } = {}) {
        if (this.closed) throw Error('Document session closed');
        if (!file?.handle) throw TypeError('Save requires an authorized file handle');
        const revision = this.history.revision,
            generation = this.generation,
            data = new TextEncoder().encode(serializeVisualDocument(this.history.value));
        const work = this.saves
            .catch(() => {})
            .then(async () => {
                if (this.closed || generation !== this.generation)
                    throw Error('Document session changed before save');
                const written = await this.services.files.write({
                    handle: file.handle,
                    data,
                    requireDurable: true,
                });
                if (written !== data.length) throw Error('Incomplete durable document write');
                if (!this.closed && generation === this.generation && revision === this.history.revision) {
                    this.history.markSaved();
                    this.file = file;
                }
                return {
                    savedRevision: revision,
                    current:
                        !this.closed && revision === this.history.revision && generation === this.generation,
                };
            });
        this.saves = work;
        return work;
    }
    importSelection(text, options) {
        return pasteVisualSelection(this.history, text, options);
    }
    close() {
        this.closed = true;
        this.generation++;
        this.file = null;
    }
}

createVisualDocument.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'createVisualDocument({objects,connections,tags,metadata,resources})',
    behavior:
        'Validated ricewm.visual-document/v1 JSON data. Object ids, geometry, styles, hierarchy, rich blocks and embedded resource references are checked together.',
});
createVisualHistory.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'createVisualHistory(document,options)',
    behavior:
        'Immutable validated object document with atomic undo/redo, revision and dirty identity. VisualDocumentModel adds reusable object, group and tag commands.',
});
encodeVisualSelection.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'encodeVisualSelection(document,ids)',
    behavior:
        'Copies selected objects and descendants, internal connections, referenced tags and embedded resources. Document-wide metadata is excluded.',
});
pasteVisualSelection.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'pasteVisualSelection(history,text,{offset,prefix,metadata?})',
    behavior:
        'Validates the complete payload, maps fresh identities, preserves internal references and resources, merges optional caller-owned metadata (for example the destination page), then commits one undoable transaction.',
});

serializeVisualDocument.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'serializeVisualDocument(document)',
    behavior: 'Validates and serializes portable visual data; never marks a document saved.',
});
parseVisualDocument.controlFactory = Object.freeze({
    topics: ['visual', 'scene'],
    signature: 'parseVisualDocument(text)',
    behavior: 'Bounded JSON import with complete shape/reference validation before active state is replaced.',
});
