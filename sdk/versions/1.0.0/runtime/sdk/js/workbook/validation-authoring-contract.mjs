/** Data-validation authoring. Evaluation and formula parsing remain engine-owned. */
export const VALIDATION_KINDS=Object.freeze(['any','whole','decimal','list','date','time','text-length','custom']);
export const VALIDATION_OPERATORS=Object.freeze(['between','not-between','equal','not-equal','greater-than','greater-or-equal','less-than','less-or-equal']);
const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_COMMAND'});};
const object=(value,keys)=>value!==null&&typeof value==='object'&&!Array.isArray(value)&&Object.getPrototypeOf(value)===Object.prototype&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
const text=(value,max)=>typeof value==='string'&&value.length<=max&&!/[\u0000\ud800-\udfff]/u.test(value);
export function checkedDataValidation(value){
    const source=Object.hasOwn(value??{},'sourceRuleIndex');
    if(!object(value,['kind','operator','formula1','formula2','ignoreBlank','inCellDropdown','input','error',...(source?['sourceRuleIndex']:[])]))fail('Invalid data-validation rule fields');
    if(!VALIDATION_KINDS.includes(value.kind)||typeof value.ignoreBlank!=='boolean'||typeof value.inCellDropdown!=='boolean')fail('Invalid data-validation settings');
    const comparison=['whole','decimal','date','time','text-length'].includes(value.kind);
    if(comparison?!VALIDATION_OPERATORS.includes(value.operator):value.operator!==null)fail('Invalid data-validation comparison');
    const formula=(v,required)=>required?text(v,8192)&&v.trim().length>0:v===null;
    if(!formula(value.formula1,value.kind!=='any')||!formula(value.formula2,comparison&&['between','not-between'].includes(value.operator)))fail('Invalid validation formula operands');
    if(value.kind==='list'&&value.formula1.startsWith('"')&&value.formula1.length>255)fail('An inline validation list supports at most 255 characters');
    if(!object(value.input,['show','title','message'])||typeof value.input.show!=='boolean'||!text(value.input.title,32)||!text(value.input.message,255))fail('Invalid validation input prompt');
    if(!object(value.error,['show','style','title','message'])||typeof value.error.show!=='boolean'||!['stop','warning','information'].includes(value.error.style)||!text(value.error.title,32)||!text(value.error.message,225))fail('Invalid validation error alert');
    if(source&&(!Number.isSafeInteger(value.sourceRuleIndex)||value.sourceRuleIndex<0||value.sourceRuleIndex>=1024))fail('Invalid validation source rule');
    return Object.freeze({...value,input:Object.freeze({...value.input}),error:Object.freeze({...value.error})});
}
