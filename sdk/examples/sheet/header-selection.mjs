/** Header gestures operate on logical coordinates, including hidden/filtered tracks. */
export function headerAxis(hit) {
    if (!hit || hit.kind) return null;
    if (hit.row === null && Number.isInteger(hit.column)) return 'columns';
    if (hit.column === null && Number.isInteger(hit.row)) return 'rows';
    if (hit.row === null && hit.column === null) return 'all';
    return null;
}

export function headerRange(frame, axis, first, last = first) {
    return {
        firstRow: axis === 'rows' ? Math.min(first, last) : 0,
        lastRow: axis === 'rows' ? Math.max(first, last) : frame.row_count - 1,
        firstColumn: axis === 'columns' ? Math.min(first, last) : 0,
        lastColumn: axis === 'columns' ? Math.max(first, last) : frame.column_count - 1,
    };
}

/** Presentation-only selection damage; no cell values or engine state are changed. */
export function headerPreview(frame, range) {
    const rows = frame.rows.flatMap((track, slot) => track.index >= range.firstRow && track.index <= range.lastRow ? [slot] : []);
    const columns = frame.columns.flatMap((track, slot) => track.index >= range.firstColumn && track.index <= range.lastColumn ? [slot] : []);
    return { ...frame, selection: rows.length && columns.length ? {
        first_row: rows[0], last_row: rows.at(-1), first_column: columns[0], last_column: columns.at(-1),
    } : null };
}
