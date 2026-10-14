/** Native presentation transport only. Damage slots come from the engine; no
 * formula execution or worksheet diff belongs here. Legacy hosts get snapshots. */
import { validateGridFrame, gridMergeValues, GRID_FRAME_MAX_BYTES } from './grid-frame.mjs';
export const GRID_DAMAGE_CAPABILITY = 1n << 10n;
const geometry = frame => JSON.stringify([frame.sheet, frame.row_count, frame.column_count, frame.width,
    frame.height, frame.row_header, frame.column_header,
    ...[frame.rows, frame.columns].map(tracks => tracks.map(t => [t.index, t.start, t.extent, t.clip_start, t.clip_end, Number(t.frozen)])),
    (frame.filter_columns || []).map(f => [f.column, Number(f.active)]),
    ...(frame.fill_endpoint ? [frame.fill_endpoint] : []),
    ...(frame.filter_header_row !== undefined ? [frame.filter_header_row] : []),
    ...(frame.zoom_percent !== undefined ? [frame.zoom_percent] : []),
    ...(frame.canvas !== undefined ? [frame.canvas] : []),
    ...(frame.merges?.length ? [frame.merges.map(gridMergeValues)] : [])]);
export function gridGeometryKey(frame) {
    let hash = 2166136261;
    for (const byte of new TextEncoder().encode(geometry(frame))) hash = Math.imul(hash ^ byte, 16777619) >>> 0;
    return hash;
}
export function encodeGridDamage(previous, next, slots, maximumBytes = GRID_FRAME_MAX_BYTES) {
    if (![64 * 1024, GRID_FRAME_MAX_BYTES].includes(maximumBytes)) throw new RangeError("invalid grid damage budget");
    validateGridFrame(previous); validateGridFrame(next);
    if (next.revision < previous.revision || geometry(previous) !== geometry(next)
        || JSON.stringify(previous.styles[0]) !== JSON.stringify(next.styles[0])) return null;
    if (!Array.isArray(slots) || slots.length > 4096) throw new RangeError('invalid native grid damage slots');
    const values = new Map(next.cells.map(cell => [cell[0] * 512 + cell[1], cell]));
    let last = -1;
    const cells = slots.map(slot => {
        if (!Array.isArray(slot) || slot.length !== 2 || !Number.isSafeInteger(slot[0]) || !Number.isSafeInteger(slot[1])
            || slot[0] < 0 || slot[0] >= next.rows.length || slot[1] < 0 || slot[1] >= next.columns.length
            || slot[0] * 512 + slot[1] <= last) throw new RangeError('invalid native grid damage slot');
        last = slot[0] * 512 + slot[1];
        const cell = values.get(last);
        return [slot[0], slot[1], cell ? next.strings[cell[2]] : null, cell ? next.styles[cell[3]] : null];
    });
    const result = JSON.stringify({ version: 1, sheet: next.sheet, base_revision: previous.revision,
        revision: next.revision, base_geometry: gridGeometryKey(previous), cells, selection: next.selection, active: next.active,
        ...(previous.references || next.references ? { references: next.references || [] } : {}) });
    return new TextEncoder().encode(result).length <= maximumBytes ? result : null;
}
