import {exact,failure} from './contract-values.mjs';
import {checkedTableCommand} from './table-contract.mjs';
export const TABLE_RANGE_COMMANDS=new Set(['set-table-range-style']);
export function checkedTableRangeCommand(command){
    if(!exact(command,['type','slide','object','from','to','style'])||!TABLE_RANGE_COMMANDS.has(command.type))throw failure('INVALID_REQUEST','Invalid table range style command');
    for(const point of [command.from,command.to]){
        if(!exact(point,['row','column']))throw failure('INVALID_REQUEST','Invalid table range endpoint');
        checkedTableCommand({type:'set-table-cell-style',slide:command.slide,object:command.object,...point,style:command.style});
    }
    return structuredClone(command);
}
