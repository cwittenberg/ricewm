import { Column, Label, MenuItem, Row } from './controls.mjs';
import { Message } from './messages.mjs';

function extent(value, maximum, name) {
    if (!Number.isSafeInteger(value) || value < 1 || value > maximum) throw new RangeError(`${name} must be an integer from 1 through ${maximum}`);
    return value;
}

/** Native popup grid. Hover/focus only previews; VALUE_CHANGED commits a size.
 * Use as a Menu child. The native menu owns Enter, Escape and dismissal, and
 * panel:menu-grid opts the nested cells into two-dimensional key navigation. */
export class TableSizePicker extends Column {
    static constructorOptions = Object.freeze(["columns", "rows", "cellSize"]);
    constructor({ columns = 10, rows = 8, cellSize = 24, ...properties } = {}) {
        extent(columns, 32, 'columns'); extent(rows, 32, 'rows'); extent(cellSize, 48, 'cellSize');
        if (columns * rows > 256 || cellSize < 20) throw new RangeError('Table picker supports at most 256 cells of 20–48 pixels');
        super({ name: 'table-size-picker', text: 'panel:fixed:flush:gap=4', ...properties });
        this.columns = columns; this.rows = rows; this.preview = null; this.cells = [];
        this.caption = this.append(new Label({ name: `${this.name}-caption`, text: 'Insert table' }));
        this.grid = this.append(new Column({ name: `${this.name}-grid`, text: `panel:menu-grid:columns=${columns}:fixed:flush:gap=0` }));
        for (let row = 1; row <= rows; row++) {
            const line = this.grid.append(new Row({ text: 'panel:fixed:flush:gap=0' }));
            for (let column = 1; column <= columns; column++) {
                const label = `${column} column${column === 1 ? '' : 's'} × ${row} row${row === 1 ? '' : 's'}`;
                const cell = line.append(new MenuItem({ name: `${this.name}-${column}-${row}`,
                    text: `panel:menu-grid-cell:fixed:accessory-height=${cellSize}:width=${cellSize}:flush`, accessibleName: label, tooltip: label }));
                const preview = () => { if (this.available()) this.setPreview({ rows: row, columns: column }); };
                cell.on(Message.POINTER, event => { if (['move', 'down'].includes(event.detail?.kind)) preview(); });
                cell.on(Message.FOCUS, preview);
                cell.on(Message.ACTIVATE, () => {
                    if (!this.available()) return;
                    this.setPreview({ rows: row, columns: column });
                    this.emit(Message.VALUE_CHANGED, { value: Object.freeze({ rows: row, columns: column }) });
                });
                this.cells.push({ cell, row, column });
            }
        }
    }
    available() {
        for (let owner = this; owner; owner = owner.parent) if (!owner.getProperty('enabled') || !owner.getProperty('visible')) return false;
        return !this.destroyed;
    }
    setPreview(value = null) {
        if (value !== null) {
            extent(value.rows, this.rows, 'rows'); extent(value.columns, this.columns, 'columns');
            value = Object.freeze({ rows: value.rows, columns: value.columns });
        }
        if (this.preview?.rows === value?.rows && this.preview?.columns === value?.columns) return;
        this.preview = value;
        this.caption.text = value ? `${value.columns} × ${value.rows} Table` : 'Insert table';
        for (const { cell, row, column } of this.cells) cell.setProperty('selected', Boolean(value && row <= value.rows && column <= value.columns));
        this.emit(Message.SELECT, { value });
    }
}
