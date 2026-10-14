/** Bounded semantic requests only; parsing and comparison execute in Wasm. */
export function checkedDataToolOptions(value,kind,{preview=false,fail=message=>{throw TypeError(message);}}={}) {
    const plain=v=>v!==null&&typeof v==='object'&&!Array.isArray(v)&&Object.getPrototypeOf(v)===Object.prototype;
    const exact=(v,keys)=>plain(v)&&Object.keys(v).length===keys.length&&keys.every(k=>Object.hasOwn(v,k));
    const integer=(v,max)=>Number.isSafeInteger(v)&&v>=0&&v<max;
    const keys=['sheetId','range',...(kind==='remove-duplicates'?['header','columns','caseSensitive']:['delimiter','textQualifier','collapseDelimiters','destination','mode',...(preview?[]:['overwrite'])])];
    if(!['remove-duplicates','text-to-columns'].includes(kind)||!exact(value,keys)||typeof value.sheetId!=='string'||!value.sheetId.length||value.sheetId.length>128)fail('Invalid data tool request');
    const r=value.range;
    if(!exact(r,['firstRow','lastRow','firstColumn','lastColumn'])||!integer(r.firstRow,1048576)||!integer(r.lastRow,1048576)||!integer(r.firstColumn,16384)||!integer(r.lastColumn,16384)||r.firstRow>r.lastRow||r.firstColumn>r.lastColumn||(r.lastRow-r.firstRow+1)*(r.lastColumn-r.firstColumn+1)>10000)fail('Choose a rectangle of at most 10000 cells');
    const range=Object.freeze({...r});
    if(kind==='remove-duplicates'){
        if(typeof value.header!=='boolean'||typeof value.caseSensitive!=='boolean'||!Array.isArray(value.columns)||value.columns.length<1||value.columns.length>256||new Set(value.columns).size!==value.columns.length||Array.from(value.columns).some(c=>!integer(c,16384)||c<r.firstColumn||c>r.lastColumn)||r.lastRow-r.firstRow+1<=Number(value.header))fail('Choose data rows and distinct comparison columns inside the selection');
        return Object.freeze({sheetId:value.sheetId,range,header:value.header,columns:Object.freeze([...value.columns]),caseSensitive:value.caseSensitive});
    }
    if(r.firstColumn!==r.lastColumn||typeof value.delimiter!=='string'||Array.from(value.delimiter).length!==1||/[\ud800-\udfff\r\n\0]/u.test(value.delimiter)||!['"',"'",null].includes(value.textQualifier)||value.delimiter===value.textQualifier||typeof value.collapseDelimiters!=='boolean'||!exact(value.destination,['row','column'])||!integer(value.destination.row,1048576)||!integer(value.destination.column,16384)||value.destination.row+r.lastRow-r.firstRow>=1048576||!['general','text'].includes(value.mode)||!preview&&typeof value.overwrite!=='boolean')fail('Invalid Text to Columns options');
    return Object.freeze({sheetId:value.sheetId,range,delimiter:value.delimiter,textQualifier:value.textQualifier,collapseDelimiters:value.collapseDelimiters,destination:Object.freeze({...value.destination}),mode:value.mode,...(preview?{}:{overwrite:value.overwrite})});
}
