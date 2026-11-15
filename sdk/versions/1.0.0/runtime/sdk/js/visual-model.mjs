import { CommandHistory } from './command-history.mjs';
import {
    createVisualDocument,
    validateVisualDocument,
    encodeVisualSelection,
    pasteVisualSelection,
} from './visual-document.mjs';
import { unionBounds } from './visual-geometry.mjs';

/** Shared document commands for object editors, independent of their views. */
export class VisualDocumentModel extends CommandHistory {
    static controlFactory = Object.freeze({
        topics: ['visual', 'scene'],
        constructor: 'new VisualDocumentModel({value:createVisualDocument(),limit:200})',
        methods:
            'addObject(descriptor), updateObjects(ids,patch), removeObjects(ids), duplicate(ids), reparent(ids,parent), group(ids), ungroup(id), setCollapsed(id,collapsed), addTag({label,color}), updateTag(id,patch), removeTag(id), query({text,tags}); inherited atomic history and immutable value',
    });
    constructor({ value = createVisualDocument(), ...options } = {}) {
        super({ ...options, value, validate: validateVisualDocument });
        this.sequence = 0;
    }
    fresh(prefix) {
        const ids = new Set(
            [...this.value.objects, ...this.value.connections, ...this.value.tags].map((o) => o.id),
        );
        let id;
        do {
            id = prefix + '-' + ++this.sequence;
        } while (ids.has(id));
        return id;
    }
    addObject(descriptor = {}) {
        const id = descriptor.id ?? this.fresh('object');
        this.transact('Add object', (d) => {
            d.objects.push({
                x: 0,
                y: 0,
                width: 160,
                height: 64,
                shape: 'roundedRect',
                text: '',
                ...descriptor,
                id,
            });
        });
        return id;
    }
    updateObjects(ids, patch) {
        if (
            !Array.isArray(ids) ||
            !patch ||
            typeof patch !== 'object' ||
            Array.isArray(patch) ||
            Object.hasOwn(patch, 'id')
        )
            throw TypeError('Object update needs ids and a patch without identity');
        const wanted = new Set(ids);
        if (ids.some((id) => !this.value.objects.some((o) => o.id === id))) throw TypeError('Unknown object');
        return this.transact('Update objects', (d) => {
            for (const o of d.objects)
                if (wanted.has(o.id) && !o.locked) Object.assign(o, structuredClone(patch));
        });
    }
    removeObjects(ids) {
        const wanted = new Set(ids),
            byId = new Map(this.value.objects.map((o) => [o.id, o]));
        for (const o of byId.values()) {
            let parent = o.parent;
            while (parent) {
                if (wanted.has(parent)) {
                    wanted.add(o.id);
                    break;
                }
                parent = byId.get(parent)?.parent;
            }
        }
        return this.transact('Remove objects', (d) => {
            d.objects = d.objects.filter((o) => !wanted.has(o.id) || o.locked);
            const remaining = new Set(d.objects.map((o) => o.id));
            for (const o of d.objects) if (o.parent && !remaining.has(o.parent)) o.parent = null;
            d.connections = d.connections.filter((e) => remaining.has(e.source) && remaining.has(e.target));
        });
    }
    duplicate(ids, options = {}) {
        return pasteVisualSelection(this, encodeVisualSelection(this.value, ids), {
            ...options,
            prefix: 'duplicate',
        });
    }
    reparent(ids, parent) {
        return this.updateObjects(ids, { parent });
    }
    group(ids) {
        const selected = this.value.objects.filter((o) => ids.includes(o.id));
        if (selected.length < 2 || selected.some((o) => o.locked))
            throw TypeError('Group at least two unlocked objects');
        const id = this.fresh('group'),
            bounds = unionBounds(selected);
        this.transact('Group objects', (d) => {
            d.objects.unshift({
                id,
                ...bounds,
                shape: 'roundedRect',
                droppable: true,
                text: '',
                metadata: { role: 'group' },
            });
            for (const o of d.objects) if (ids.includes(o.id)) o.parent = id;
        });
        return id;
    }
    ungroup(id) {
        const group = this.value.objects.find((o) => o.id === id);
        if (!group || group.locked) throw TypeError('Group unavailable');
        return this.transact('Ungroup objects', (d) => {
            d.objects = d.objects.filter((o) => o.id !== id);
            for (const o of d.objects) if (o.parent === id) o.parent = group.parent || null;
            d.connections = d.connections.filter((e) => e.source !== id && e.target !== id);
        });
    }
    setCollapsed(id, collapsed) {
        if (typeof collapsed !== 'boolean') throw TypeError('Collapse state must be boolean');
        return this.updateObjects([id], { collapsed });
    }
    addTag({ id = this.fresh('tag'), label, color } = {}) {
        this.transact('Add tag', (d) => {
            d.tags.push({ id, label, ...(color ? { color } : {}) });
        });
        return id;
    }
    updateTag(id, patch) {
        if (!patch || Object.hasOwn(patch, 'id')) throw TypeError('Tag identity cannot change');
        return this.transact('Edit tag', (d) => {
            const tag = d.tags.find((t) => t.id === id);
            if (!tag) throw TypeError('Unknown tag');
            Object.assign(tag, patch);
        });
    }
    removeTag(id) {
        return this.transact('Remove tag', (d) => {
            d.tags = d.tags.filter((t) => t.id !== id);
            for (const o of d.objects) if (o.tags) o.tags = o.tags.filter((t) => t !== id);
        });
    }
    query({ text = '', tags = [], where = () => true } = {}) {
        if (typeof text !== 'string' || !Array.isArray(tags) || typeof where !== 'function')
            throw TypeError('Invalid object query');
        const query = text.toLocaleLowerCase();
        return this.value.objects.filter(
            (o) =>
                (!query || (o.text || '').toLocaleLowerCase().includes(query)) &&
                tags.every((t) => o.tags?.includes(t)) &&
                where(o),
        );
    }
}
