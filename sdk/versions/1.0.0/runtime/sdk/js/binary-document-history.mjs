const MAX_REVISION = 18446744073709551615n;
const same = (a, b) => a.length === b.length && a.every((byte, index) => byte === b[index]);

/** One bounded history for imported binary document snapshots. The owner must
 * validate/import a candidate before commit, and restore undoBytes/redoBytes
 * successfully before accepting undo/redo. No caller-owned array is retained.
 */
export class BinaryDocumentHistory {
    #states; #cursor = 0; #serial = 0n; #saved; #revision = 0n; #maximumBytes; #limit;
    static controlFactory = Object.freeze({ topics: ['documents', 'workflow'],
        constructor: 'new BinaryDocumentHistory(bytes,{saved:true,limit:128,maximumBytes:33554432})',
        methods: 'snapshot(), commit(bytes), undoBytes(), redoBytes(), undo(), redo(), markSaved(); revision, stateId, stateIds, dirty, canUndo, canRedo, retainedBytes. Stable stateId/stateIds support owner sidecars through Undo/Redo and bounded eviction. Validate/import before commit; restore the planned bytes before accepting undo/redo. Serial caller owns async work and durable save acknowledgment.' });
    constructor(bytes, { saved = true, limit = 128, maximumBytes = 32 * 1024 * 1024 } = {}) {
        if (typeof saved !== 'boolean' || !Number.isSafeInteger(limit) || limit < 1 || limit > 10000 ||
            !Number.isSafeInteger(maximumBytes) || maximumBytes < 1 || maximumBytes > 128 * 1024 * 1024) throw new TypeError('Invalid binary history options');
        this.#maximumBytes = maximumBytes; this.#limit = limit;
        this.#states = [{ id: 0n, bytes: this.#checked(bytes) }]; this.#saved = saved ? 0n : null;
    }
    #checked(bytes) {
        if (!(bytes instanceof Uint8Array) || !bytes.length || bytes.length > this.#maximumBytes) throw new RangeError('Binary document exceeds its history bound');
        return bytes.slice();
    }
    #advance() { if (this.#revision === MAX_REVISION) throw new RangeError('Binary history revision exhausted'); this.#revision++; }
    get revision() { return String(this.#revision); }
    /** Stable snapshot identities let the owner retain bounded sidecar state
     * across Undo/Redo without confusing a revision with document identity. */
    get stateId() { return String(this.#states[this.#cursor].id); }
    get stateIds() { return Object.freeze(this.#states.map(state=>String(state.id))); }
    get dirty() { return this.#states[this.#cursor].id !== this.#saved; }
    get canUndo() { return this.#cursor > 0; }
    get canRedo() { return this.#cursor + 1 < this.#states.length; }
    get retainedBytes() { return this.#states.reduce((sum, state) => sum + state.bytes.length, 0); }
    snapshot() { return this.#states[this.#cursor].bytes.slice(); }
    commit(bytes) {
        const owned = this.#checked(bytes);
        if (same(owned, this.#states[this.#cursor].bytes)) return false;
        this.#advance();
        this.#states.splice(this.#cursor + 1);
        this.#states.push({ id: ++this.#serial, bytes: owned }); this.#cursor++;
        let retained = this.retainedBytes;
        while (this.#states.length > 1 && (this.#states.length > this.#limit + 1 || retained > this.#maximumBytes)) {
            retained -= this.#states.shift().bytes.length; this.#cursor--;
        }
        return true;
    }
    undoBytes() { return this.canUndo ? this.#states[this.#cursor - 1].bytes.slice() : null; }
    redoBytes() { return this.canRedo ? this.#states[this.#cursor + 1].bytes.slice() : null; }
    undo() { if (!this.canUndo) return false; this.#advance(); this.#cursor--; return true; }
    redo() { if (!this.canRedo) return false; this.#advance(); this.#cursor++; return true; }
    markSaved() { this.#saved = this.#states[this.#cursor].id; }
}
