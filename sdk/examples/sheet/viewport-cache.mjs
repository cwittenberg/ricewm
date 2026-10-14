/** Bounded presentation cache: one viewport, never the workbook or its formulas.
 * The engine sends snapshots when geometry changes and deltas for stable views.
 * A sequence identifies presentation changes even within one workbook revision.
 */
import { copyGridFrame, validateGridStyle, GRID_FRAME_MAX_BYTES } from '@ricewm/app';

const MAX_BYTES = GRID_FRAME_MAX_BYTES;
const invalid = message => { throw Object.assign(new Error(message), { code: 'INVALID_VIEW_UPDATE' }); };
const resync = () => { throw Object.assign(new Error('Viewport base changed; request a new snapshot'), { code: 'RESYNC_REQUIRED' }); };
const integer = value => Number.isSafeInteger(value) && value >= 0;
const exact = (value, fields) => value && Object.getPrototypeOf(value) === Object.prototype
    && Object.keys(value).length === fields.length && fields.every(field => Object.hasOwn(value, field));
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

export class SheetViewportCache {
    #state = null;
    get current() { return this.#state; }
    clear() { this.#state = null; }

    apply(update) {
        if (!update || typeof update !== 'object' || update.version !== 1
            || typeof update.view !== 'string' || !update.view.length || update.view.length > 128
            || !integer(update.sequence) || !integer(update.generation)) invalid('Invalid viewport update envelope');
        let encoded;
        try { encoded = JSON.stringify(update); } catch { invalid('Viewport update must be plain data'); }
        if (new TextEncoder().encode(encoded).length > MAX_BYTES) invalid('Viewport update exceeds the bounded presentation budget');
        if (update.kind === 'snapshot') return this.#snapshot(update);
        if (update.kind === 'batch') return this.#batch(update);
        if (update.kind !== 'delta') invalid('Unknown viewport update kind');
        return this.#delta(update);
    }

    #snapshot(update) {
        if (!exact(update, ['version', 'kind', 'view', 'generation', 'sequence', 'frame'])) invalid('Invalid viewport snapshot fields');
        const frame = copyGridFrame(update.frame);
        const old = this.#state;
        if (old && (frame.revision < old.frame.revision || update.generation < old.generation
            || (update.generation === old.generation && (update.view !== old.view || update.sequence <= old.sequence)))) resync();
        this.#state = Object.freeze({ view: update.view, generation: update.generation, sequence: update.sequence, frame });
        return Object.freeze({ ...this.#state, fullPaint: true, changedCells: null, selectionChanged: true });
    }

    #batch(update) {
        if (!exact(update, ['version', 'kind', 'view', 'generation', 'baseSequence', 'sequence',
            'baseRevision', 'revision', 'sheet', 'updates']) || !Array.isArray(update.updates)
            || !update.updates.length || update.updates.length > 16) invalid('Invalid viewport batch fields or page count');
        const old = this.#state;
        if (!old || update.view !== old.view || update.generation !== old.generation
            || update.baseSequence !== old.sequence || update.baseRevision !== old.frame.revision
            || update.sheet !== old.frame.sheet || !integer(update.revision) || update.revision < old.frame.revision
            || update.sequence !== old.sequence + update.updates.length) resync();
        // Each page may shrink dictionaries before the next page grows them.
        // Stage those bounded viewport transformations privately; no live cursor,
        // cell or selection becomes observable until every page has validated.
        const staged = new SheetViewportCache();
        staged.#state = old;
        const dirty = new Map();
        for (const page of update.updates) {
            if (!page || page.version !== 1 || page.kind !== 'delta' || page.view !== update.view
                || page.generation !== update.generation || page.sheet !== update.sheet
                || page.revision !== update.revision) invalid('Viewport batch pages must belong to one calculation result');
            const result = staged.apply(page);
            for (const [row, column] of result.changedCells) dirty.set(row * 512 + column, Object.freeze([row, column]));
        }
        if (staged.#state.sequence !== update.sequence || staged.#state.frame.revision !== update.revision) resync();
        const final = staged.#state;
        const selectionChanged = !same(old.frame.selection, final.frame.selection) || !same(old.frame.active, final.frame.active);
        const changedCells = Object.freeze([...dirty].sort(([a], [b]) => a - b).map(([, cell]) => cell));
        this.#state = final;
        return Object.freeze({ ...final, fullPaint: false, changedCells, selectionChanged });
    }

    #delta(update) {
        if (!exact(update, ['version', 'kind', 'view', 'generation', 'sequence', 'baseSequence', 'baseRevision',
            'revision', 'sheet', 'cells', 'selection', 'active'])) invalid('Invalid viewport delta fields');
        const old = this.#state;
        if (!old || update.view !== old.view || update.generation !== old.generation || update.baseSequence !== old.sequence
            || update.baseRevision !== old.frame.revision || update.sheet !== old.frame.sheet) resync();
        if (!integer(update.revision) || update.revision < old.frame.revision
            || update.sequence !== old.sequence + 1) resync();
        if (!Array.isArray(update.cells) || update.cells.length > 4096) invalid('Invalid changed-cell list');
        const frame = old.frame;
        if (!update.cells.length && same(update.selection, frame.selection) && same(update.active, frame.active)) {
            // An offscreen calculation changes the revision cursor only. Reuse
            // immutable visible storage instead of rebuilding/serializing cells.
            const nextFrame = Object.freeze({ ...frame, revision: update.revision });
            this.#state = Object.freeze({ view: old.view, generation: old.generation, sequence: update.sequence, frame: nextFrame });
            return Object.freeze({ ...this.#state, fullPaint: false, changedCells: Object.freeze([]), selectionChanged: false });
        }
        // Build compact dictionaries from the resulting visible values. They are
        // bounded by this viewport; repeated edits cannot grow an unbounded cache.
        const cells = new Map(frame.cells.map(([row, column, text, style]) => [row * 512 + column,
            [row, column, frame.strings[text], frame.styles[style]]]));
        const changedCells = [];
        let previous = -1;
        for (const change of update.cells) {
            if (!Array.isArray(change) || change.length !== 4) invalid('Invalid changed cell');
            const [row, column, text, style] = change;
            if (!integer(row) || row >= frame.rows.length || !integer(column) || column >= frame.columns.length) {
                invalid('Changed cell is outside this viewport');
            }
            const key = row * 512 + column;
            if (key <= previous) invalid('Changed cells must be unique and ordered');
            previous = key;
            if (text === null) {
                if (style !== null) invalid('Deleted display cell must have a null style');
                if (cells.delete(key)) changedCells.push(Object.freeze([row, column]));
            } else {
                if (typeof text !== 'string') invalid('Invalid changed display value');
                try { validateGridStyle(style); } catch { invalid('Invalid changed display value'); }
                const next = [row, column, text, style];
                if (!same(cells.get(key), next)) changedCells.push(Object.freeze([row, column]));
                cells.set(key, next);
            }
        }
        // Native grid damage compares style 0 because it paints empty cells.
        // Keep the engine's reserved fontless default stable during font edits.
        const reserveDefault = exact(frame.styles[0], ['alignment', 'role']) && frame.styles[0].alignment === 0 && frame.styles[0].role === 0;
        const strings = [], styles = reserveDefault ? [frame.styles[0]] : [], stringIds = new Map();
        const styleIds = new Map(styles.map((style, index) => [JSON.stringify(style), index]));
        const intern = (value, values, ids, key = value) => {
            if (!ids.has(key)) { ids.set(key, values.length); values.push(value); }
            return ids.get(key);
        };
        const projectedCells = [...cells].sort(([a], [b]) => a - b).map(([, [row, column, text, style]]) =>
            [row, column, intern(text, strings, stringIds), intern(style, styles, styleIds, JSON.stringify(style))]);
        if (styles.length === 0) styles.push(frame.styles[0]);
        const nextFrame = copyGridFrame({ ...frame, revision: update.revision, strings, styles, cells: projectedCells,
            selection: update.selection, active: update.active });
        const selectionChanged = !same(frame.selection, nextFrame.selection) || !same(frame.active, nextFrame.active);
        this.#state = Object.freeze({ view: old.view, generation: old.generation, sequence: update.sequence, frame: nextFrame });
        return Object.freeze({ ...this.#state, fullPaint: false,
            changedCells: Object.freeze(changedCells), selectionChanged });
    }
}
