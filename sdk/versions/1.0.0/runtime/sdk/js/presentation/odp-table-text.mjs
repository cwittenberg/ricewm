import {ODF,odfChildren,odfAttribute} from './odp-source.mjs';
import {cellKey} from './table-values.mjs';

/** Source cells retain the original native repeated/covered-cell guards. */
export function odpTableTextCells(target,object){
    const tables=odfChildren(target.node,ODF.table,'table');if(tables.length!==1)return new Map();
    const table=tables[0],nodes=[],stack=[...table.children].reverse();
    while(stack.length){
        const node=stack.pop();if(node.namespaceURI!==ODF.table)continue;
        if(node.localName==='table')continue;
        nodes.push(node);stack.push(...[...node.children].reverse());
        if(nodes.length>20000)return new Map();
    }
    const rows=nodes.filter(n=>n.localName==='table-row'),columns=nodes.filter(n=>n.localName==='table-column');
    if(!rows.length||rows.length>128||!columns.length||columns.length>64||rows.length*columns.length>4096)return new Map();
    // Even a later repeated cell changes native table-wide editability.
    if(nodes.some(node=>['number-columns-repeated','number-rows-repeated'].some(name=>{
        const value=odfAttribute(node,ODF.table,name);return value!==undefined&&(!/^[0-9]+$/u.test(value)||Number(value)!==1);
    })))return new Map();
    const result=new Map();
    for(const[rowIndex,row]of rows.entries()){
        const cells=row.children.filter(n=>n.namespaceURI===ODF.table&&['table-cell','covered-table-cell'].includes(n.localName));
        if(cells.length!==columns.length)return new Map();
        for(const[column,cell]of cells.entries()){
            if(cell.localName!=='table-cell')continue;
            result.set(cellKey(object,rowIndex,column),{body:cell,node:target.node,object,cell:{row:rowIndex,column}});
        }
    }
    return result;
}
