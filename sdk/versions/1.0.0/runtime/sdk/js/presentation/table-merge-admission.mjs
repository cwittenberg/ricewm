import {nativeRangeObject} from './text-range-admission.mjs';
import {failure} from './contract-values.mjs';
import {cellRangeBetween} from '../cell-range.mjs';

/** Read original native permissions and compare the complete source grid. This
 * adds no native capability and never changes canEditStructure on merged tables. */
export async function admitNativeTableMerge(backend, slide, command, source, alive = () => {}, {after = false,projectCell = null} = {}) {
    const reject = message => { throw failure('UNSUPPORTED_OPERATION', message); };
    const {target, editable} = await nativeRangeObject(backend, slide, command.object, alive);
    const table = target?.table;
    if (!editable || !table?.canEditCells || table.rows !== source.rows || table.columns !== source.columns) reject('Select a source-backed editable table');
    let range;
    if (after) range = {firstRow: 0, lastRow: source.rows - 1, firstColumn: 0, lastColumn: source.columns - 1};
    else if (command.type === 'merge-table-cells' || command.type === 'set-table-range-style') range = cellRangeBetween(command.from, command.to, {rows: source.rows, columns: source.columns, merges: source.merges});
    else {
        const cell = source.cells[command.row * source.columns + command.column];
        if (!cell || cell.covered || cell.rowSpan === 1 && cell.columnSpan === 1) reject('Select a merged anchor cell to split');
        range = {firstRow: cell.row, lastRow: cell.row + cell.rowSpan - 1, firstColumn: cell.column, lastColumn: cell.column + cell.columnSpan - 1};
    }
    let startCell = 0, limit = 32;
    while (startCell < source.cells.length) {
        let reply;
        try { reply = await backend.exchange('table-cells', {slide: command.slide, object: command.object, startCell, limit}, backend.revision); }
        catch (error) { alive(); if (error.code !== 'RESPONSE_TOO_LARGE' || limit === 1) throw error; limit = Math.max(1, Math.floor(limit / 2)); continue; }
        alive();
        if (reply.result.totalCells !== source.cells.length || !reply.result.cells.length) reject('Native and source table dimensions differ');
        for (const [offset, cell] of reply.result.cells.entries()) {
            const original = source.cells[startCell + offset];
            if (!original || ['row', 'column', 'rowSpan', 'columnSpan', 'covered'].some(key => original[key] !== cell[key]) ||
                (cell.text?.text || '') !== original.text) reject('Native and source table cells differ');
            const selected = cell.row >= range.firstRow && cell.row <= range.lastRow && cell.column >= range.firstColumn && cell.column <= range.lastColumn;
            if (selected && !cell.covered && (!cell.editable || !cell.text?.editable)) reject('Selected cell text retains its native read-only guard');
            if(projectCell)projectCell(cell);
        }
        startCell += reply.result.cells.length;
        if (reply.result.nextCell !== (startCell < source.cells.length ? startCell : null)) reject('Native cell projection is incomplete');
    }
    return range;
}
