export const bounded = value => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e6;
export const count = (value, max) => Number.isSafeInteger(value) && value >= 1 && value <= max;
export const index = (value, max) => Number.isSafeInteger(value) && value >= 0 && value < max;
export const paint = value => value === null || value && /^#[a-f0-9]{6}$/iu.test(value.color) && bounded(value.opacity) && value.opacity >= 0 && value.opacity <= 1;
export const border = value => paint(value) && (value === null || bounded(value.width) && value.width >= 0);
export const tableMetadata = value => value === undefined || value === null || value && count(value.rows, 128) && count(value.columns, 64) && value.rows * value.columns <= 4096 &&
    Array.isArray(value.columnWidths) && value.columnWidths.length === value.columns && value.columnWidths.every(v => bounded(v) && v > 0) &&
    Array.isArray(value.rowHeights) && value.rowHeights.length === value.rows && value.rowHeights.every(v => bounded(v) && v > 0) &&
    ['headerRow','bandedRows','canEditCells','canEditStructure'].every(key => typeof value[key] === 'boolean') &&
    Array.isArray(value.unsupported) && value.unsupported.length <= 128 && value.unsupported.every(v => typeof v === 'string' && v.length <= 4096);
export const cellKey = (object, row, column) => `${object}/cell:${row}:${column}`;
/** Derived text requests, never independently editable document objects. */
export function tableTextScene(scene) {
    return { ...scene, objects: scene.objects.flatMap(object => object.table ? (object.table.cells || []).filter(cell => !cell.covered && cell.text).map(cell => ({
        id: cellKey(object.id, cell.row, cell.column), kind: 'text', bounds: cell.bounds, text: cell.text,
    })) : [object]) };
}
