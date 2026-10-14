/** Bounded native presentation request; dimensions describe the host grid, never
 * worksheet storage. Old count-only callers remain supported. */
export function checkedViewport(value) {
    const base=['top','left','rows','columns'],optional=['pixelWidth','pixelHeight','followSelection','frameBytes','zoomPercent'];
    const fail=()=>{throw Object.assign(new TypeError('Invalid bounded workbook viewport'),{code:'VIEWPORT_LIMIT'});};
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>![...base,...optional].includes(k))||!base.every(k=>Number.isSafeInteger(value[k])))fail();
    const {top,left,rows,columns}=value;
    const pixels=value.pixelWidth!==undefined||value.pixelHeight!==undefined;
    if(top<0||left<0||rows<1||columns<1||(!pixels&&rows*columns>4096)||top>=1048576||left>=16384||(!pixels&&(top+rows>1048576||left+columns>16384)))fail();
    if(pixels&&(!Number.isSafeInteger(value.pixelWidth)||value.pixelWidth<49||value.pixelWidth>65536||!Number.isSafeInteger(value.pixelHeight)||value.pixelHeight<25||value.pixelHeight>65536||rows>256||columns>128))fail();
    if(value.frameBytes!==undefined&&(![65536,262144].includes(value.frameBytes)||value.frameBytes===262144&&!pixels))fail();
    if(value.zoomPercent!==undefined&&(!Number.isSafeInteger(value.zoomPercent)||value.zoomPercent<25||value.zoomPercent>400))fail();
    if(value.followSelection!==undefined&&typeof value.followSelection!=='boolean')fail();
    return {top,left,rows,columns,...(value.zoomPercent===undefined?{}:{zoomPercent:value.zoomPercent}),...(value.frameBytes===undefined?{}:{frameBytes:value.frameBytes}),...(pixels?{pixelWidth:value.pixelWidth,pixelHeight:value.pixelHeight}:{}),...(value.followSelection===undefined?{}:{followSelection:value.followSelection})};
}
export function checkedScroll(value) {
    const keys=['topRow','leftColumn','totalRows','totalColumns','visibleRows','visibleColumns','lastRow','lastColumn','pixelWidth','pixelHeight'];
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==keys.length||!keys.every(k=>Number.isSafeInteger(value[k])&&value[k]>=0)||value.totalRows!==1048576||value.totalColumns!==16384||value.topRow>=value.totalRows||value.leftColumn>=value.totalColumns||value.lastRow>=value.totalRows||value.lastColumn>=value.totalColumns||value.visibleRows>256||value.visibleColumns>128||value.pixelWidth>65536||value.pixelHeight>65536)throw new TypeError('Invalid engine scroll bounds');
    return Object.freeze({...value});
}
