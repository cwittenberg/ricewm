const keys = ['firstRow', 'lastRow', 'firstColumn', 'lastColumn'];
const object = value => value && typeof value === 'object' && !Array.isArray(value);
const exact = (value, fields) => object(value) && Object.keys(value).length === fields.length && fields.every(key => Object.hasOwn(value, key));
const integer = (value, maximum) => Number.isSafeInteger(value) && value >= 0 && value < maximum;
function dimensions(value) {
    if (!object(value) || Object.keys(value).some(key => !['rows', 'columns', 'merges'].includes(key)) ||
        !Number.isSafeInteger(value.rows) || value.rows < 1 || value.rows > 1048576 ||
        !Number.isSafeInteger(value.columns) || value.columns < 1 || value.columns > 16384 ||
        value.merges !== undefined && (!Array.isArray(value.merges) || value.merges.length > 4096)) throw RangeError('Cell ranges require bounded row and column counts');
    return value;
}
function checked(range, size) {
    if (!exact(range, keys) || !integer(range.firstRow, size.rows) || !integer(range.lastRow, size.rows) ||
        range.firstRow > range.lastRow || !integer(range.firstColumn, size.columns) || !integer(range.lastColumn, size.columns) ||
        range.firstColumn > range.lastColumn) throw RangeError('Cell range is outside its rows or columns');
    return Object.fromEntries(keys.map(key => [key, range[key]]));
}
/** Include every merged rectangle touched by a range, including newly reached
 * merges. The caller owns selection, document capabilities and persistence. */
export function expandCellRange(range, size) {
    dimensions(size);
    const result = checked(range, size), merges = (size.merges || []).map(merge => checked(merge, size));
    for (let pass = 0; pass <= merges.length; pass++) {
        let changed = false;
        for (const merge of merges) {
            if (merge.firstRow > result.lastRow || merge.lastRow < result.firstRow ||
                merge.firstColumn > result.lastColumn || merge.lastColumn < result.firstColumn) continue;
            for (const key of keys) {
                const next = key.startsWith('first') ? Math.min(result[key], merge[key]) : Math.max(result[key], merge[key]);
                changed ||= result[key] !== next; result[key] = next;
            }
        }
        if (!changed) return Object.freeze(result);
    }
    throw RangeError('Merged range expansion did not converge');
}
export function cellRangeBetween(anchor, focus, size) {
    dimensions(size);
    for (const point of [anchor, focus]) if (!exact(point, ['row', 'column']) ||
        !integer(point.row, size.rows) || !integer(point.column, size.columns)) throw RangeError('Cell range endpoints are outside the grid');
    return expandCellRange({firstRow: Math.min(anchor.row, focus.row), lastRow: Math.max(anchor.row, focus.row),
        firstColumn: Math.min(anchor.column, focus.column), lastColumn: Math.max(anchor.column, focus.column)}, size);
}
expandCellRange.controlFactory = Object.freeze({topics: ['visual', 'scene', 'documents', 'tables'], constructor: 'expandCellRange({firstRow,lastRow,firstColumn,lastColumn},{rows,columns,merges:[]})',
    methods: 'Return a new immutable zero-based inclusive rectangle expanded until every intersecting merge is fully included. Accepts 1–1048576 rows, 1–16384 columns and up to 4096 bounded merge rectangles. Does not mutate inputs, select widgets, grant editability or write documents.'});
cellRangeBetween.controlFactory = Object.freeze({topics: ['visual', 'scene', 'documents', 'tables'], constructor: 'cellRangeBetween({row,column},{row,column},{rows,columns,merges:[]})',
    methods: 'Normalize either drag direction or Shift-selection endpoints, then include whole intersecting merges through expandCellRange. Result and all indices use zero-based inclusive bounds. The caller retains anchor/focus, capability checks and one transaction per document edit.'});
