/** Conditional-rule authoring data shared by the native UI and automation.
 * Evaluation, rule identities, workbook state and history stay in the engine. */
export const CONDITIONAL_RULE_FIELDS = Object.freeze(Object.fromEntries(Object.entries({
    'cell-value':['operator','first','second'], formula:['formula'], text:['operator','text'],
    date:['period','first','second'], duplicate:[], unique:[], blanks:[], 'not-blanks':[], errors:[], 'no-errors':[],
    'above-average':[], 'below-average':[], top:['rank','percent'], bottom:['rank','percent'],
    'color-scale':['thresholds'], 'data-bar':['minimum','maximum','positiveColor','negativeColor','gradient','showValue'],
    'icon-set':['icons','showValue'], 'icon-rating':['icon','color','thresholds','showValue'],
}).map(([kind,fields])=>[kind,Object.freeze(fields)])));
export const CONDITIONAL_RULE_TYPES=Object.freeze(Object.keys(CONDITIONAL_RULE_FIELDS));
export const CONDITIONAL_OPERATORS=Object.freeze(['equal','not-equal','greater-than','greater-or-equal','less-than','less-or-equal','between','not-between']);
export const CONDITIONAL_TEXT_OPERATORS=Object.freeze(['contains','not-contains','begins-with','ends-with','equals']);
export const CONDITIONAL_DATE_PERIODS=Object.freeze(['between','not-between','yesterday','today','tomorrow','last-7-days','next-7-days','last-week','this-week','next-week','last-month','this-month','next-month','last-year','this-year','next-year']);
export {GRID_DECORATION_SYMBOLS as CONDITIONAL_ICONS} from '../grid-decorations.mjs';
import {GRID_DECORATION_SYMBOLS as CONDITIONAL_ICONS} from '../grid-decorations.mjs';
const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_CONDITIONAL_FORMAT'});};
const plain=value=>value!==null&&typeof value==='object'&&!Array.isArray(value)&&Object.getPrototypeOf(value)===Object.prototype;
const exact=(value,fields)=>plain(value)&&Object.keys(value).length===fields.length&&fields.every(key=>Object.hasOwn(value,key));
const color=value=>value===null||typeof value==='string'&&/^#[0-9a-f]{6}$/iu.test(value)
    ||exact(value,['theme','tint'])&&integer(value.theme,0,11)&&typeof value.tint==='number'&&Number.isFinite(value.tint)&&value.tint>=-1&&value.tint<=1;
const copyColor=value=>typeof value==='string'||value===null?value:Object.freeze({...value});
const text=(value,max=16384)=>typeof value==='string'&&value.length>0&&!/[\u0000\ud800-\udfff]/u.test(value)&&new TextEncoder().encode(value).length<=max;
const integer=(value,min,max)=>Number.isSafeInteger(value)&&value>=min&&value<=max;
const borderStyles=new Set(['thin','medium','thick','double','dotted','dashed','hair','dashDot','dashDotDot','mediumDashed','mediumDashDot','mediumDashDotDot','slantDashDot']);
export function checkedConditionalFormat(value) {
    const allowed=['font','fill','borders','numberFormat','sourceRuleIndex'];
    if(!plain(value)||Object.keys(value).some(key=>!allowed.includes(key)))fail('Invalid differential cell format');
    const result={};
    if(Object.hasOwn(value,'sourceRuleIndex')){if(!integer(value.sourceRuleIndex,0,4095))fail('Invalid source conditional format');result.sourceRuleIndex=value.sourceRuleIndex;}
    if(Object.hasOwn(value,'font')){
        if(!plain(value.font)||Object.keys(value.font).some(key=>!['bold','italic','underline','strike','color'].includes(key)))fail('Invalid differential font');
        for(const [key,item] of Object.entries(value.font))if(key==='color'?item!==null&&!color(item):typeof item!=='boolean')fail('Invalid differential font property');
        result.font=Object.freeze(Object.fromEntries(Object.entries(value.font).map(([key,item])=>[key,key==='color'?copyColor(item):item])));
    }
    if(Object.hasOwn(value,'fill')){if(value.fill!==null&&!color(value.fill))fail('Invalid conditional fill color');result.fill=copyColor(value.fill);}
    if(Object.hasOwn(value,'numberFormat')){if(value.numberFormat!==null&&!text(value.numberFormat,256))fail('Invalid conditional number format');result.numberFormat=value.numberFormat;}
    if(Object.hasOwn(value,'borders')){
        if(!plain(value.borders)||Object.keys(value.borders).some(key=>!['top','right','bottom','left'].includes(key)))fail('Invalid conditional borders');
        result.borders=Object.freeze(Object.fromEntries(Object.entries(value.borders).map(([side,item])=>{
            if(item===null)return [side,null];
            if(!exact(item,['style','color'])||!borderStyles.has(item.style)||!color(item.color))fail('Invalid conditional border edge');
            return [side,Object.freeze({...item,color:copyColor(item.color)})];
        })));
    }
    return Object.freeze(result);
}
export function checkedConditionalThreshold(value) {
    if(!exact(value,['kind','value']))fail('Invalid conditional threshold');
    if(['min','max'].includes(value.kind)){if(value.value!==null)fail('Minimum/maximum thresholds require null value');}
    else if(value.kind==='formula'){if(!text(value.value))fail('Invalid conditional threshold formula');}
    else if(['number','percent','percentile'].includes(value.kind)){
        if(!Number.isFinite(value.value)||typeof value.value!=='number'||value.kind!=='number'&&(value.value<0||value.value>100))fail('Invalid conditional threshold number');
    }else fail('Unknown conditional threshold type');
    return Object.freeze({...value});
}
export function checkedConditionalRule(value) {
    const fields=plain(value)&&typeof value.kind==='string'&&Object.hasOwn(CONDITIONAL_RULE_FIELDS,value.kind)&&CONDITIONAL_RULE_FIELDS[value.kind];
    const decorated=fields&&!['color-scale','data-bar','icon-set','icon-rating'].includes(value.kind);
    if(!fields||!exact(value,['kind',...fields,...(decorated?['format','stopIfTrue']:[])]))fail('Unknown conditional rule or unexpected fields');
    const result={...value};
    if(decorated){result.format=checkedConditionalFormat(value.format);if(typeof value.stopIfTrue!=='boolean')fail('Stop If True requires a boolean');}
    if(value.kind==='cell-value'){
        if(!CONDITIONAL_OPERATORS.includes(value.operator)||!text(value.first))fail('Invalid conditional cell comparison');
        if(['between','not-between'].includes(value.operator)?!text(value.second):value.second!==null)fail('Invalid second comparison operand');
    }
    if(value.kind==='formula'&&!text(value.formula))fail('Invalid conditional formula');
    if(value.kind==='text'&&(!CONDITIONAL_TEXT_OPERATORS.includes(value.operator)||!text(value.text)))fail('Invalid conditional text rule');
    if(value.kind==='date'){
        if(!CONDITIONAL_DATE_PERIODS.includes(value.period))fail('Unknown conditional date period');
        if(['between','not-between'].includes(value.period)?!text(value.first)||!text(value.second):value.first!==null||value.second!==null)fail('Invalid conditional date bounds');
    }
    if(['top','bottom'].includes(value.kind)&&(!integer(value.rank,1,value.percent?100:1000)||typeof value.percent!=='boolean'))fail('Invalid top/bottom rule');
    if(value.kind==='color-scale'){
        if(!Array.isArray(value.thresholds)||![2,3].includes(value.thresholds.length))fail('A color scale needs two or three thresholds');
        result.thresholds=Object.freeze(value.thresholds.map(item=>{
            if(!exact(item,['threshold','color'])||!color(item.color))fail('Invalid color-scale stop');
            return Object.freeze({threshold:checkedConditionalThreshold(item.threshold),color:copyColor(item.color)});
        }));
    }
    if(value.kind==='data-bar'){
        if(!color(value.positiveColor)||!color(value.negativeColor)||typeof value.gradient!=='boolean')fail('Invalid data-bar appearance');
        result.positiveColor=copyColor(value.positiveColor);result.negativeColor=copyColor(value.negativeColor);
        for(const key of ['minimum','maximum'])result[key]=value[key]===null?null:checkedConditionalThreshold(value[key]);
    }
    if(value.kind==='icon-set'){
        if(!Array.isArray(value.icons)||value.icons.length<3||value.icons.length>5)fail('An icon set needs three to five ordered icons');
        result.icons=Object.freeze(value.icons.map(item=>{
            if(!exact(item,['icon','color','threshold','strict'])||!CONDITIONAL_ICONS.includes(item.icon)||!color(item.color)||typeof item.strict!=='boolean')fail('Invalid conditional icon');
            return Object.freeze({...item,color:copyColor(item.color),threshold:checkedConditionalThreshold(item.threshold)});
        }));
    }
    if(value.kind==='icon-rating'){
        if(!CONDITIONAL_ICONS.includes(value.icon)||!color(value.color)||!Array.isArray(value.thresholds)||value.thresholds.length<2||value.thresholds.length>4)fail('Invalid conditional icon rating');
        result.color=copyColor(value.color);
        result.thresholds=Object.freeze(value.thresholds.map(item=>{
            if(!exact(item,['threshold','strict'])||typeof item.strict!=='boolean')fail('Invalid conditional rating threshold');
            return Object.freeze({threshold:checkedConditionalThreshold(item.threshold),strict:item.strict});
        }));
    }
    if(['data-bar','icon-set','icon-rating'].includes(value.kind)&&typeof value.showValue!=='boolean')fail('Show Value requires a boolean');
    return Object.freeze(result);
}
