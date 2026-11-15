const copy = (value) => structuredClone(value);
const freeze = (value) => {
    if (value && typeof value === 'object') {
        Object.freeze(value);
        for (const child of Object.values(value)) freeze(child);
    }
    return value;
};
/** Synchronous, atomic document transactions. Drag previews do not enter history. */
export class CommandHistory {
    static controlFactory = Object.freeze({
        topics: ['visual', 'scene'],
        constructor: 'new CommandHistory({value,validate,limit:200,maximumBytes:33554432})',
        methods:
            'transact(label,mutate), undo(), redo(), replace(value), markSaved(), subscribe(listener); immutable value, revision, dirty, canUndo/canRedo',
    });
    constructor({
        value = {},
        validate = (value) => value,
        limit = 200,
        maximumBytes = 32 * 1024 * 1024,
    } = {}) {
        if (
            typeof validate !== 'function' ||
            !Number.isInteger(limit) ||
            limit < 1 ||
            limit > 10000 ||
            !Number.isInteger(maximumBytes) ||
            maximumBytes < 1024 ||
            maximumBytes > 128 * 1024 * 1024
        )
            throw TypeError('Invalid history options');
        this.validate = validate;
        this.limit = limit;
        this.maximumBytes = maximumBytes;
        this.entries = [];
        this.cursor = 0;
        this.revision = 0;
        this.serial = 0;
        this.saved = 0;
        this.listeners = new Set();
        this.value = this.checked(value);
        this.valueBytes = this.checkedBytes;
        this.historyBytes = 0;
        this.identity = 0;
    }
    checked(value) {
        const checked = copy(this.validate(copy(value))),
            bytes = new TextEncoder().encode(JSON.stringify(checked)).length;
        if (bytes > this.maximumBytes) throw RangeError('Document exceeds history byte budget');
        this.checkedBytes = bytes;
        return freeze(checked);
    }
    get dirty() {
        return this.identity !== this.saved;
    }
    get canUndo() {
        return this.cursor > 0;
    }
    get canRedo() {
        return this.cursor < this.entries.length;
    }
    transact(label, mutate) {
        if (this.mutating) throw Error('Nested document transaction');
        if (typeof mutate !== 'function' || typeof label !== 'string' || !label)
            throw TypeError('Transaction requires label and mutation');
        this.mutating = true;
        let next;
        try {
            const draft = copy(this.value),
                result = mutate(draft);
            if (result && typeof result.then === 'function')
                throw TypeError('Transactions must be synchronous');
            next = this.checked(result === undefined ? draft : result);
        } finally {
            this.mutating = false;
        }
        if (JSON.stringify(next) === JSON.stringify(this.value)) return false;
        const entry = {
            label,
            before: this.value,
            after: next,
            beforeId: this.identity,
            afterId: ++this.serial,
            beforeBytes: this.valueBytes,
            afterBytes: this.checkedBytes,
        };
        for (const removed of this.entries.splice(this.cursor))
            this.historyBytes -= removed.beforeBytes + removed.afterBytes;
        this.entries.push(entry);
        this.historyBytes += entry.beforeBytes + entry.afterBytes;
        this.cursor = this.entries.length;
        this.value = next;
        this.valueBytes = entry.afterBytes;
        this.identity = entry.afterId;
        while (
            this.entries.length > this.limit ||
            (this.entries.length > 1 && this.historyBytes > this.maximumBytes)
        ) {
            const removed = this.entries.shift();
            this.historyBytes -= removed.beforeBytes + removed.afterBytes;
            this.cursor--;
        }
        this.changed();
        return true;
    }
    undo() {
        if (!this.canUndo) return false;
        const entry = this.entries[--this.cursor];
        this.value = entry.before;
        this.valueBytes = entry.beforeBytes;
        this.identity = entry.beforeId;
        this.changed();
        return true;
    }
    redo() {
        if (!this.canRedo) return false;
        const entry = this.entries[this.cursor++];
        this.value = entry.after;
        this.valueBytes = entry.afterBytes;
        this.identity = entry.afterId;
        this.changed();
        return true;
    }
    replace(value, { saved = true } = {}) {
        const next = this.checked(value);
        this.entries = [];
        this.cursor = 0;
        this.historyBytes = 0;
        this.value = next;
        this.valueBytes = this.checkedBytes;
        this.identity = ++this.serial;
        if (saved) this.saved = this.identity;
        this.changed();
    }
    markSaved() {
        this.saved = this.identity;
        this.changed(false);
    }
    changed(revision = true) {
        if (revision) this.revision++;
        for (const listener of [...this.listeners])
            listener(this.value, { revision: this.revision, dirty: this.dirty });
    }
    subscribe(listener) {
        if (typeof listener !== 'function') throw TypeError('Expected history listener');
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }
    destroy() {
        this.listeners.clear();
        this.entries = [];
    }
}
