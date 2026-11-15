/** Native SceneSurface viewport and the exact transform used for its glyphs.
 * Caret maps remain in the shaper's source coordinates. */
export function shapedTextPresentation(value) {
    if(value===null)return null;
    if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(k=>!['width','height','transform','surface','epoch','caretColor'].includes(k))||
        !['width','height'].every(k=>Number.isInteger(value[k])&&value[k]>=1&&value[k]<=16384)||!Array.isArray(value.transform)||value.transform.length!==6||
        value.transform.some((n,i)=>!Number.isFinite(n)||Math.abs(n)>(i<4?1024:1000000)))throw RangeError('Invalid shaped text presentation');
    const [a,b,c,d]=value.transform,det=a*d-b*c;
    if(!Number.isFinite(det)||Math.abs(det)<1e-12)throw RangeError('Shaped text presentation must be invertible');
    const bound=value.surface!==undefined||value.epoch!==undefined;
    if(bound&&![value.surface,value.epoch].every(v=>typeof v==='string'&&/^[1-9][0-9]{0,19}$/u.test(v)&&BigInt(v)<=0xffffffffffffffffn))throw RangeError('Invalid shaped glyph surface identity');
    if(value.caretColor!==undefined&&(typeof value.caretColor!=='string'||!/^#[0-9a-fA-F]{6}$/u.test(value.caretColor)))throw RangeError('Invalid shaped caret color');
    return Object.freeze({width:value.width,height:value.height,transform:Object.freeze([...value.transform]),...(bound?{surface:value.surface,epoch:value.epoch}:{}),...(value.caretColor?{caretColor:value.caretColor}:{})});
}
