/** Immutable, app-independent format-copy session. The app supplies a bounded
 * style snapshot and an atomic apply callback; document content never lives here. */
export class FormatPainterSession {
    #snapshot = null; #persistent = false; #generation = 0; #activation = -Infinity;
    get active() { return this.#snapshot !== null; }
    get persistent() { return this.active && this.#persistent; }
    get snapshot() { return this.#snapshot; }
    arm(value, { persistent = false } = {}) {
        const encoded = JSON.stringify(value);
        if (!encoded || encoded.length > 65536) throw new RangeError('Format snapshot exceeds 64 KiB');
        const freeze = value => { if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value); } return value; };
        this.#snapshot = freeze(JSON.parse(encoded));
        this.#persistent = Boolean(persistent); this.#generation += 1;
        return this.#snapshot;
    }
    activate(value, now = performance.now()) {
        if (this.active) {
            if (!this.#persistent && now >= this.#activation && now - this.#activation <= 500) this.#persistent = true;
            else this.cancel();
        } else this.arm(value);
        this.#activation = now;
        return this.active;
    }
    cancel() { this.#snapshot = null; this.#persistent = false; this.#generation += 1; this.#activation = -Infinity; }
    async apply(callback) {
        if (!this.active) return false;
        const generation = this.#generation, snapshot = this.#snapshot;
        await callback(snapshot);
        if (generation === this.#generation && !this.#persistent) this.cancel();
        return true;
    }
}
