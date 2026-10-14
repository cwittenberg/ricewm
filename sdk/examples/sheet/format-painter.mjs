import { checkedSheetCommand } from './command-contract.mjs';

/** Only logical references cross the app boundary; styles stay in Wasm. */
export function formatPainterCommand(source, sheetId, target) {
    const range = { ...target };
    if (range.firstRow === range.lastRow && range.firstColumn === range.lastColumn) {
        range.lastRow += source.range.lastRow - source.range.firstRow;
        range.lastColumn += source.range.lastColumn - source.range.firstColumn;
    }
    return checkedSheetCommand({ type: 'paint-format', sourceSheetId: source.sheetId,
        sourceRange: source.range, sheetId, range });
}
