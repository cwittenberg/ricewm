import {textRangeIndex} from '../text-ranges.mjs';
import {SCHEME_COLOR_SLOTS} from '../color-references.mjs';

export const PARAGRAPH_LIST_STYLES=Object.freeze({
    bullet:Object.freeze(['disc','circle','square']),
    number:Object.freeze(['decimal','lower-alpha','upper-alpha','lower-roman','upper-roman','decimal-parenthesis','lower-alpha-parenthesis']),
});
export const PARAGRAPH_BULLET_GLYPHS=Object.freeze(['•','○','■','□','◆','➢','✓','–','★']);
export const PARAGRAPH_MARKER_COLORS=Object.freeze(['tx1','accent1','accent2','accent3','accent4','accent5','accent6']);
export const validBulletGlyph=value=>typeof value==='string'&&Array.from(value).length===1&&!/[\s\u0000-\u001f\u007f-\u009f\ud800-\udfff\ufffe\uffff]/u.test(value);
const markerColor=value=>value===null||typeof value==='string'&&(/^#[0-9a-f]{6}$/iu.test(value)||SCHEME_COLOR_SLOTS.includes(value)&&value!=='phClr');
export function validParagraphMarker(value,{metadata=false}={}){
    if(!value||typeof value!=='object'||Array.isArray(value)||!Object.keys(value).length||
        Object.keys(value).some(key=>!['fontFamily','color','sizePercent',...(metadata?['sizePoints','resolvedColor']:[])].includes(key)))return false;
    return (value.fontFamily===undefined||value.fontFamily===null||typeof value.fontFamily==='string'&&value.fontFamily.length>0&&new TextEncoder().encode(value.fontFamily).length<=128&&!/[\u0000-\u001f\u007f]/u.test(value.fontFamily))&&
        (value.color===undefined||markerColor(value.color))&&
        (value.sizePercent===undefined||Number.isFinite(value.sizePercent)&&value.sizePercent>=25&&value.sizePercent<=400)&&
        (value.sizePoints===undefined||Number.isFinite(value.sizePoints)&&value.sizePoints>0&&value.sizePoints<=4096)&&
        !(value.sizePercent!==undefined&&value.sizePoints!==undefined)&&
        (value.resolvedColor===undefined||value.resolvedColor&&/^#[0-9a-f]{6}$/iu.test(value.resolvedColor.color)&&Number.isFinite(value.resolvedColor.opacity)&&value.resolvedColor.opacity>=0&&value.resolvedColor.opacity<=1);
}
export function validParagraphList(list){
    return list!==null&&typeof list==='object'&&!Array.isArray(list)&&
        (list.kind==='none'&&Object.keys(list).length===1||
        list.kind==='bullet'&&list.style==='custom'&&Object.keys(list).length===3&&validBulletGlyph(list.glyph)||
        Object.keys(list).length===2&&PARAGRAPH_LIST_STYLES[list.kind]?.includes(list.style));
}
export function validParagraphFormat(format){
    return format&&typeof format==='object'&&!Array.isArray(format)&&Object.keys(format).length>0&&
        Object.keys(format).every(key=>['list','level','levelDelta','marker','startAt'].includes(key))&&
        (format.list===undefined||validParagraphList(format.list))&&
        (format.marker===undefined||validParagraphMarker(format.marker)&&format.list?.kind!=='none')&&
        (format.startAt===undefined||Number.isInteger(format.startAt)&&format.startAt>=1&&format.startAt<=32767&&(!format.list||format.list.kind==='number'))&&
        (format.level===undefined||Number.isInteger(format.level)&&format.level>=0&&format.level<=8)&&
        (format.levelDelta===undefined||[-1,1].includes(format.levelDelta))&&
        !(format.level!==undefined&&format.levelDelta!==undefined);
}
export function validParagraphFormatMetadata(value){
    return value&&Number.isInteger(value.level)&&value.level>=0&&value.level<=8&&
        ['marginLeft','marginRight','indent'].every(key=>Number.isFinite(value[key])&&Math.abs(value[key])<=1_000_000)&&
        (value.list===null||validParagraphList(value.list))&&
        (value.startAt===undefined||Number.isInteger(value.startAt)&&value.startAt>=1&&value.startAt<=32767)&&
        (value.marker===undefined||validParagraphMarker(value.marker,{metadata:true}));
}

/** Paragraph ranges are source UTF-8 offsets. A nonempty selection ending at
 * the next paragraph's start does not select that next paragraph. */
export function textParagraphsInRange(text,paragraphs,range){
    const index=textRangeIndex(text);index.scalar(range.start);index.scalar(range.end);
    if(range.start>range.end||!Array.isArray(paragraphs)||paragraphs.length>256)throw RangeError('Invalid paragraph selection');
    return paragraphs.filter(row=>range.start===range.end?range.start>=row.start&&range.start<=row.end:range.start<=row.end&&range.end>row.start);
}
const alphabet=n=>{let label='';while(n>0){n--;label=String.fromCharCode(97+n%26)+label;n=Math.floor(n/26);}return label;};
const roman=n=>{let label='';for(const[value,letters]of [[1000,'m'],[900,'cm'],[500,'d'],[400,'cd'],[100,'c'],[90,'xc'],[50,'l'],[40,'xl'],[10,'x'],[9,'ix'],[5,'v'],[4,'iv'],[1,'i']])while(n>=value){label+=letters;n-=value;}return label;};
export function paragraphListLabels(paragraphs){
    if(!Array.isArray(paragraphs)||paragraphs.length>256)throw RangeError('Invalid list paragraph count');
    const counters=Array(9).fill(null);
    return paragraphs.map(row=>{
        const format=row.paragraphFormat,list=format?.list,level=format?.level??0;
        if(!list||list.kind==='none'){counters.fill(null);return null;}
        if(!validParagraphFormatMetadata(format))throw TypeError('Invalid paragraph list metadata');
        counters.fill(null,level+1);
        if(list.kind==='bullet'){counters[level]=null;return list.style==='custom'?list.glyph:{disc:'•',circle:'○',square:'▪'}[list.style];}
        const key=list.style,start=format.startAt??1,previous=counters[level];
        const n=previous?.key===key&&format.startAt===undefined?previous.value+1:start;
        if(!Number.isInteger(n)||n<1||n>32767+paragraphs.length)throw RangeError('List number exceeds its source bounds');
        counters[level]={key,value:n};
        const label=key.startsWith('decimal')?String(n):key.includes('alpha')?alphabet(n):roman(n);
        return (key.startsWith('upper')?label.toUpperCase():label)+(key.endsWith('parenthesis')?')':'.');
    });
}
textParagraphsInRange.controlFactory=Object.freeze({topics:['documents'],constructor:'textParagraphsInRange(text,paragraphs,{start,end})',methods:'Select source paragraphs with checked UTF-8 grapheme offsets. A selection ending at the next paragraph start excludes that next paragraph. Does not own source, edits or history.'});
paragraphListLabels.controlFactory=Object.freeze({topics:['documents','visual'],constructor:'paragraphListLabels(paragraphs)',methods:'Derive bounded bullet and automatic numbering labels from paragraphFormat source metadata. Nested levels have independent counters and resume the parent number. Labels are paint metadata, never added to editable text or copied content.'});
