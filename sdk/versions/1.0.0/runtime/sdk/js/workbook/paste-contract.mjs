/** Paste Special options use logical cells and the current internal clipboard. */
export const SHEET_PASTE_MODES=Object.freeze(['all','formulas','values','formats','validation','column-widths','all-except-borders','formulas-and-number-formats','values-and-number-formats']);
export const SHEET_PASTE_OPERATIONS=Object.freeze(['none','add','subtract','multiply','divide']);
export function checkedPasteOptions(value){
    const fail=message=>{throw Object.assign(new TypeError(message),{code:'INVALID_COMMAND'});};
    const keys=['mode','operation','transpose','skipBlanks','link'];
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==5||keys.some(k=>!Object.hasOwn(value,k))||!SHEET_PASTE_MODES.includes(value.mode)||!SHEET_PASTE_OPERATIONS.includes(value.operation)||['transpose','skipBlanks','link'].some(k=>typeof value[k]!=='boolean'))fail('Invalid Paste Special settings');
    const content=['all','formulas','values','all-except-borders','formulas-and-number-formats','values-and-number-formats'].includes(value.mode);
    if(value.operation!=='none'&&(!content||value.link))fail('Paste operations require cell contents and cannot be combined with Paste Link');
    if(value.link&&!['all','formulas','values'].includes(value.mode))fail('Paste Link requires a content paste mode');
    if(value.mode==='column-widths'&&(value.transpose||value.skipBlanks||value.link))fail('Column-width paste cannot transpose or skip cell contents');
    return Object.freeze({...value});
}
