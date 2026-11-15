import {exact, failure} from './contract-values.mjs';
export const TABLE_MERGE_COMMANDS = new Set(['merge-table-cells', 'split-table-cell']);
const index = (value, maximum) => Number.isSafeInteger(value) && value >= 0 && value < maximum;
const point = value => exact(value, ['row', 'column']) && index(value.row, 128) && index(value.column, 64);
export function checkedTableMergeCommand(command) {
    const bad = () => { throw failure('INVALID_REQUEST', 'Invalid table merge or split command'); };
    const id = value => typeof value === 'string' && value.length > 0 && value.length <= 4096;
    if (!id(command?.slide) || !id(command.object)) bad();
    if (command.type === 'merge-table-cells') {
        if (!exact(command, ['type', 'slide', 'object', 'from', 'to']) || !point(command.from) || !point(command.to)) bad();
    } else if (command.type === 'split-table-cell') {
        if (!exact(command, ['type', 'slide', 'object', 'row', 'column']) || !index(command.row, 128) || !index(command.column, 64)) bad();
    } else bad();
    return structuredClone(command);
}
