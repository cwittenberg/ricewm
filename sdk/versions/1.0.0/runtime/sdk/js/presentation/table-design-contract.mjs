import {exact,failure} from './contract-values.mjs';
import {checkedPresentationTableStyle} from './table-design.mjs';
export const TABLE_DESIGN_COMMANDS=new Set(['set-table-design','set-table-options']);
const optionKeys=new Set(Object.keys(checkedPresentationTableStyle({preset:'none'})).filter(key=>key!=='preset'));
export function checkedTableDesignCommand(command){
    const id=value=>typeof value==='string'&&value.length>0&&value.length<=4096;
    if(!TABLE_DESIGN_COMMANDS.has(command?.type)||!id(command.slide)||!id(command.object))throw failure('INVALID_REQUEST','Invalid table design command');
    if(command.type==='set-table-options'){
        if(!exact(command,['type','slide','object','options'])||!command.options||typeof command.options!=='object'||Array.isArray(command.options)||!Object.keys(command.options).length||Object.entries(command.options).some(([key,value])=>!optionKeys.has(key)||typeof value!=='boolean'))throw failure('INVALID_REQUEST','Invalid table style options');
        return {...command,options:{...command.options}};
    }
    if(!exact(command,['type','slide','object','style']))throw failure('INVALID_REQUEST','Invalid table design command');
    let style;try{style=checkedPresentationTableStyle(command.style);}catch{throw failure('INVALID_REQUEST','Unknown table design preset or option');}
    return {...command,style:Object.fromEntries(Object.keys(command.style).map(key=>[key,style[key]]))};
}
