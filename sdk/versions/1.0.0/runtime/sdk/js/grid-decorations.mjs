/** Resolved document paint, never conditional rules or workbook values.
 * Fractions use millionths: below a tenth of a pixel of error across the largest
 * native viewport. Authoritative numeric values remain in the workbook. */
export const GRID_DECORATIONS_CAPABILITY=1n<<30n;
export const GRID_DECORATION_SCALE=1_000_000;
export const GRID_DECORATION_SYMBOLS=Object.freeze(['ArrowUp','ArrowRight','ArrowDown','ArrowAngleUp','ArrowAngleDown','Circle','TriangleUp','TriangleDown','TriangleUpFilled','TriangleDownFilled','FlatRectangle','Rhombus','Flag','Check','Cross','Exclamation','Star','Heart','ThumbsUp','ThumbsDown']);
const object=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const exact=(value,keys)=>object(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
const color=value=>typeof value==='string'&&/^#[0-9a-f]{6}$/iu.test(value);
const integer=(value,min,max)=>Number.isSafeInteger(value)&&value>=min&&value<=max;
const invalid=()=>{throw new RangeError('Invalid native grid decorations');};
export function validateGridDecorations(value){
    if(!object(value)||!Object.keys(value).length||Object.keys(value).some(key=>!['data_bar','icon','rating'].includes(key)))invalid();
    if(Object.hasOwn(value,'data_bar')){
        const bar=value.data_bar;
        if(!exact(bar,['value','axis','positive_color','negative_color','gradient','show_value'])||!integer(bar.value,0,GRID_DECORATION_SCALE)||!integer(bar.axis,0,GRID_DECORATION_SCALE)||!color(bar.positive_color)||!color(bar.negative_color)||typeof bar.gradient!=='boolean'||typeof bar.show_value!=='boolean')invalid();
    }
    for(const key of ['icon','rating'])if(Object.hasOwn(value,key)){
        const icon=value[key];
        if(!exact(icon,['kind','color','show_value',...(key==='rating'?['count','total']:[])])||!GRID_DECORATION_SYMBOLS.includes(icon.kind)||!color(icon.color)||typeof icon.show_value!=='boolean')invalid();
        if(key==='rating'&&(!integer(icon.total,1,5)||!integer(icon.count,0,icon.total)))invalid();
    }
    return value;
}
