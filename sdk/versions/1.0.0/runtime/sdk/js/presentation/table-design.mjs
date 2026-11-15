import {resolveColorReference,checkedResolvedPaint} from '../color-references.mjs';

const families=[['light1','Light Style 1'],['light2','Light Style 2'],['medium2','Medium Style 2'],['medium4','Medium Style 4'],['dark1','Dark Style 1']];
const inline=['none','grid','medium2-tx1','medium2-accent1','medium2-accent2','medium2-accent3','medium2-accent4','medium2-accent5'];
const catalog=Object.freeze([
    {id:'none',label:'No Style, No Grid',family:'none',slot:'accent1'},
    {id:'grid',label:'No Style, Table Grid',family:'grid',slot:'accent1'},
    ...families.flatMap(([family,label])=>['tx1','accent1','accent2','accent3','accent4','accent5','accent6'].map((slot,index)=>({
        id:family+'-'+slot,label:label+(index?' - Accent '+index:''),family,slot,
    }))),
].map(style=>Object.freeze({...style,inline:inline.includes(style.id)})));
const styles=new Map(catalog.map(style=>[style.id,style]));
const optionDefaults=Object.freeze({headerRow:true,firstColumn:false,totalRow:false,lastColumn:false,bandedRows:true,bandedColumns:false});
const keys=new Set(['preset',...Object.keys(optionDefaults)]);
const fail=message=>{throw TypeError('Presentation table design: '+message);};
const size=(value,maximum)=>Number.isSafeInteger(value)&&value>=1&&value<=maximum;

/** The authored catalog is shared by document commands, galleries and agents. */
export function presentationTableStyles({inlineOnly=false}={}) {
    if(typeof inlineOnly!=='boolean')fail('invalid gallery selection');
    return inlineOnly?Object.freeze(inline.map(id=>styles.get(id))):catalog;
}
export function checkedPresentationTableStyle(value) {
    if(!value||typeof value!=='object'||Array.isArray(value)||!Object.hasOwn(value,'preset')||!styles.has(value.preset)||Object.keys(value).some(key=>!keys.has(key)))fail('unknown style or option');
    const result={preset:value.preset};
    for(const [key,fallback]of Object.entries(optionDefaults)){
        const flag=Object.hasOwn(value,key)?value[key]:fallback;
        if(typeof flag!=='boolean')fail('style options must be booleans');
        result[key]=flag;
    }
    return Object.freeze(result);
}
const scheme=(slot,transforms=[])=>({kind:'scheme',value:slot,transforms});
const fixed=color=>({kind:'srgb',value:color,transforms:[]});
function resolve(source,resolver) {
    if(typeof resolver!=='function')fail('a document theme resolver is required');
    return resolveColorReference(source,slot=>{
        const paint=resolver(slot);
        return paint?checkedResolvedPaint(paint):null;
    });
}

/** Built-in cell appearance before direct cell/run overrides. Lengths use
 * browser document units (96 dpi); colour sources retain their theme identity.
 * Header/total/column emphasis and both band directions follow the source
 * style family. Applying a table style must not erase direct formatting. */
function cellStyle(value,{row,column,rows,columns,rowSpan=1,columnSpan=1},resolveScheme,fileDefined) {
    const style=checkedPresentationTableStyle(value),entry=styles.get(style.preset);
    if(!size(rows,128)||!size(columns,64)||rows*columns>4096||!Number.isSafeInteger(row)||row<0||row>=rows||!Number.isSafeInteger(column)||column<0||column>=columns||!size(rowSpan,rows)||!size(columnSpan,columns)||row+rowSpan>rows||column+columnSpan>columns)fail('cell lies outside the bounded table');
    const header=style.headerRow&&row===0,total=style.totalRow&&row+(fileDefined?rowSpan:1)===rows&&(!fileDefined||rows>1);
    const first=style.firstColumn&&column===0,last=style.lastColumn&&column+(fileDefined?columnSpan:1)===columns&&(!fileDefined||columns>1);
    const band=style.bandedRows&&!header&&!total&&(row-Number(style.headerRow))%2===0;
    const columnBand=style.bandedColumns&&(!fileDefined||!first&&!last)&&(column-(fileDefined?Number(style.firstColumn):0))%2===0,emphasis=header||total||first||last;
    const accent=scheme(fileDefined&&entry.slot==='tx1'?'dk1':entry.slot),white=fileDefined?scheme('lt1'):fixed('#FFFFFF');
    const slot=fileDefined&&entry.slot==='tx1'?'dk1':entry.slot,tint=value=>scheme(slot,[{kind:'tint',value}]);
    let fill=null,text=fileDefined&&entry.family!=='none'?scheme('dk1'):null,bold=false,line=null,width=0;
    switch(entry.family){
    case 'none':break;
    case 'grid':line=scheme(fileDefined?'dk1':'tx1');width=.75;break;
    case 'light1':fill=band||columnBand?tint(20000):null;bold=emphasis;if(header){line=accent;width=1;}break;
    case 'light2':fill=header?accent:null;text=header?white:text;bold=header||total||first;line=accent;width=1;break;
    case 'dark1':fill=header?(fileDefined?scheme('dk1'):fixed('#000000')):scheme(slot,[{kind:'shade',value:band||columnBand?60000:75000}]);text=white;bold=header||total||first;line=white;width=1;break;
    case 'medium4':fill=tint(header||band||columnBand?40000:20000);bold=emphasis;line=accent;width=.75;break;
    case 'medium2':fill=emphasis?accent:tint(band||columnBand?40000:20000);text=emphasis?white:text;bold=emphasis;line=white;width=1;break;
    }
    return {fill:fill?resolve(fill,resolveScheme):null,text:text?resolve(text,resolveScheme):null,bold,
        border:line?{...resolve(line,resolveScheme),width:width*96/72}:null};
}
export function presentationTableCellStyle(value,where,resolveScheme){return cellStyle(value,where,resolveScheme,false);}
/** Appearance of the explicit standard definitions written by the source codec.
 * Edge row/column bands, merged anchors and scheme light/dark colours follow
 * that definition cascade; caller-owned direct cell/run properties still win. */
export function presentationTableDefinitionCellStyle(value,where,resolveScheme){return cellStyle(value,where,resolveScheme,true);}

for(const [name,operation,description]of [
    ['presentationTableStyles',presentationTableStyles,'Return the immutable 37-style catalog or the separate eight-entry inline gallery, preserving authored order, labels and family identities.'],
    ['checkedPresentationTableStyle',checkedPresentationTableStyle,'Validate a known preset and six boolean options: headerRow, firstColumn, totalRow, lastColumn, bandedRows and bandedColumns. Omitted options use the new-table defaults.'],
    ['presentationTableCellStyle',presentationTableCellStyle,'Resolve bounded theme-relative cell appearance before direct overrides. Preserve returned colour sources for persistence; border width uses 96 dpi document units. This helper does not authorize edits.'],
    ['presentationTableDefinitionCellStyle',presentationTableDefinitionCellStyle,'Resolve the explicit source definition cascade before direct overrides, including edge-band exclusions, source light/dark colors and merged rowSpan/columnSpan anchors. This helper does not authorize edits.'],
])operation.controlFactory=Object.freeze({topics:['tables','documents','visual'],constructor:name+'(...)',methods:description});
