/** Editable colour identities and DrawingML colour arithmetic. Resolved RGB is
 * preview data; a source reference keeps theme changes live after persistence. */
export const SCHEME_COLOR_SLOTS=Object.freeze(['dk1','lt1','dk2','lt2','accent1','accent2','accent3','accent4','accent5','accent6','hlink','folHlink','bg1','tx1','bg2','tx2','phClr']);
const names=new Set(['tint','shade','lumMod','lumOff','satMod','satOff','hueMod','hueOff','alpha','alphaMod','alphaOff','comp','inv','gray','redMod','greenMod','blueMod']);
const noValue=new Set(['comp','inv','gray']);
const exact=(v,keys,optional=[])=>v&&typeof v==='object'&&!Array.isArray(v)&&keys.every(k=>Object.hasOwn(v,k))&&Object.keys(v).every(k=>keys.includes(k)||optional.includes(k));
const integer=v=>Number.isInteger(v)&&v>=-2147483648&&v<=2147483647;
const hex=v=>typeof v==='string'&&/^#[a-f0-9]{6}$/iu.test(v);
const clamp=v=>Math.max(0,Math.min(1,v));
export function checkedColorReference(value) {
    if(!value||!Array.isArray(value.transforms)||value.transforms.length>64)throw TypeError('Invalid color reference');
    if(value.transforms.some(t=>!names.has(t?.kind)||!exact(t,noValue.has(t.kind)?['kind']:['kind','value'])||!noValue.has(t.kind)&&!integer(t.value)))throw TypeError('Invalid color transform');
    let valid=false;
    if(['srgb','scheme','preset'].includes(value.kind))valid=exact(value,['kind','value','transforms'])&&(value.kind==='scheme'?SCHEME_COLOR_SLOTS.includes(value.value):value.kind==='srgb'?hex(value.value):typeof value.value==='string'&&/^[A-Za-z]{1,64}$/u.test(value.value));
    else if(['scrgb','hsl'].includes(value.kind))valid=exact(value,['kind','value','transforms'])&&Array.isArray(value.value)&&value.value.length===3&&value.value.every(integer);
    else if(value.kind==='system')valid=exact(value,['kind','name','lastColor','transforms'])&&typeof value.name==='string'&&/^[A-Za-z]{1,64}$/u.test(value.name)&&hex(value.lastColor);
    if(!valid)throw TypeError('Invalid color reference');
    const result=structuredClone(value);if(value.kind==='srgb')result.value=value.value.toUpperCase();if(value.kind==='system')result.lastColor=value.lastColor.toUpperCase();return result;
}
export function checkedResolvedPaint(value) {
    if(!exact(value,['color','opacity'],['source'])||!hex(value.color)||typeof value.opacity!=='number'||!Number.isFinite(value.opacity)||value.opacity<0||value.opacity>1)throw TypeError('Invalid resolved color');
    return {color:value.color.toUpperCase(),opacity:value.opacity,...(Object.hasOwn(value,'source')?{source:checkedColorReference(value.source)}:{})};
}
const toLinear=v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4;
const fromLinear=v=>v<=.0031308?12.92*v:1.055*v**(1/2.4)-.055;
export function colorHsl(color) {
    const [r,g,b]=[1,3,5].map(i=>parseInt(color.slice(i,i+2),16)/255),hi=Math.max(r,g,b),lo=Math.min(r,g,b),d=hi-lo,l=(hi+lo)/2;
    if(d<1e-12)return [0,0,l];
    const h=hi===r?((g-b)/d+6)%6:hi===g?(b-r)/d+2:(r-g)/d+4;
    return [h/6,d/(1-Math.abs(2*l-1)),l];
}
const hslRgb=([h,s,l])=>{
    h=((h%1)+1)%1*6;s=clamp(s);l=clamp(l);const c=(1-Math.abs(2*l-1))*s,x=c*(1-Math.abs(h%2-1)),m=l-c/2;
    return (h<1?[c,x,0]:h<2?[x,c,0]:h<3?[0,c,x]:h<4?[0,x,c]:h<5?[x,0,c]:[c,0,x]).map(v=>v+m);
};
const colorHex=channels=>'#'+channels.map(v=>Math.round(clamp(v)*255).toString(16).padStart(2,'0')).join('').toUpperCase();
export function applyColorTransforms(paint,transforms) {
    let {color,opacity}=checkedResolvedPaint(paint);
    checkedColorReference({kind:'srgb',value:color,transforms});
    for(const t of transforms){
        let rgb=[1,3,5].map(i=>parseInt(color.slice(i,i+2),16)/255);const p=t.value/100000;
        if(t.kind==='alpha')opacity=clamp(p);
        else if(t.kind==='alphaMod')opacity=clamp(opacity*p);
        else if(t.kind==='alphaOff')opacity=clamp(opacity+p);
        else if(t.kind==='shade'||t.kind==='tint')rgb=rgb.map(v=>fromLinear(clamp(toLinear(v)*clamp(p)+(t.kind==='tint'?1-clamp(p):0))));
        else if(t.kind==='inv')rgb=rgb.map(v=>1-v);
        else if(t.kind==='gray')rgb=Array(3).fill(.299*rgb[0]+.587*rgb[1]+.114*rgb[2]);
        else if(['redMod','greenMod','blueMod'].includes(t.kind))rgb[['redMod','greenMod','blueMod'].indexOf(t.kind)]*=p;
        else{
            const hsl=colorHsl(color),index=t.kind.startsWith('hue')||t.kind==='comp'?0:t.kind.startsWith('sat')?1:2;
            hsl[index]=t.kind==='comp'?hsl[index]+.5:t.kind==='hueOff'?hsl[index]+t.value/21600000:t.kind.endsWith('Off')?hsl[index]+p:hsl[index]*p;
            rgb=hslRgb(hsl);
        }
        color=colorHex(rgb);
    }
    return {color,opacity};
}
const presets={black:'#000000',white:'#FFFFFF',red:'#FF0000',green:'#008000',lime:'#00FF00',blue:'#0000FF',yellow:'#FFFF00',cyan:'#00FFFF',aqua:'#00FFFF',magenta:'#FF00FF',fuchsia:'#FF00FF',gray:'#808080',grey:'#808080',silver:'#C0C0C0',maroon:'#800000',navy:'#000080',olive:'#808000',purple:'#800080',teal:'#008080',orange:'#FFA500',pink:'#FFC0CB',brown:'#A52A2A',gold:'#FFD700',ltGray:'#D3D3D3',lightGray:'#D3D3D3',dkGray:'#A9A9A9',darkGray:'#A9A9A9'};
export function resolveColorReference(raw,resolveScheme=()=>null) {
    const source=checkedColorReference(raw);let base;
    if(source.kind==='srgb')base={color:source.value,opacity:1};
    else if(source.kind==='system')base={color:source.lastColor,opacity:1};
    else if(source.kind==='scheme')base=resolveScheme(source.value);
    else if(source.kind==='preset')base=presets[source.value]?{color:presets[source.value],opacity:1}:null;
    else if(source.kind==='scrgb')base={color:colorHex(source.value.map(v=>fromLinear(clamp(v/100000)))),opacity:1};
    else base={color:colorHex(hslRgb([source.value[0]/21600000,source.value[1]/100000,source.value[2]/100000])),opacity:1};
    if(!base)throw TypeError('Unresolved color reference');
    return {...applyColorTransforms(base,source.transforms),source};
}
/** Changing transparency overrides alpha only; RGB/theme transforms stay live. */
export function colorWithOpacity(raw,opacity) {
    const paint=checkedResolvedPaint(raw);if(typeof opacity!=='number'||!Number.isFinite(opacity)||opacity<0||opacity>1)throw TypeError('Invalid color opacity');
    // DrawingML alpha is an integer percentage with 1/100000 resolution.
    // Canonicalize before persistence so a source refresh acknowledges the edit.
    opacity=Math.round(opacity*100000)/100000;
    if(paint.source&&paint.opacity!==opacity)paint.source=checkedColorReference({...paint.source,transforms:[...paint.source.transforms.filter(t=>!t.kind.startsWith('alpha')),{kind:'alpha',value:Math.round(opacity*100000)}]});
    return {...paint,opacity};
}
export function transformedColor(raw,transforms) {
    const paint=checkedResolvedPaint(raw),resolved=applyColorTransforms(paint,transforms);
    return {...resolved,...(paint.source?{source:checkedColorReference({...paint.source,transforms:[...paint.source.transforms,...transforms]})}:{})};
}
export function paintFromColorChoice(previous,detail) {
    if(detail?.choice?.data?.paint)return colorWithOpacity(checkedResolvedPaint(detail.choice.data.paint),previous.opacity);
    return checkedResolvedPaint({color:detail.value,opacity:previous.opacity});
}
export function themeColorPaint(theme,slot) {
    const entry=theme?.colors?.find(c=>c.slot===slot);if(!entry)return null;
    return {color:entry.color,opacity:entry.opacity,source:{kind:'scheme',value:slot,transforms:[]}};
}
export function themeColorSwatches(theme) {
    const columns=['bg1','tx1','bg2','tx2','accent1','accent2','accent3','accent4','accent5','accent6'];
    if(!columns.every(slot=>themeColorPaint(theme,slot)))return [];
    const result=[];
    for(let row=0;row<6;row++)for(const slot of columns){
        const base=themeColorPaint(theme,slot),l=colorHsl(base.color)[2];let mods=[],label=slot;
        if(row){
            let factor,offset;
            if(l<.2){factor=[.1,.25,.5,.75,.9][row-1];offset=1-factor;label='Lighter '+Math.round(offset*100)+'%';}
            else if(l>.8){factor=[.95,.85,.75,.65,.5][row-1];label='Darker '+Math.round((1-factor)*100)+'%';}
            else if(row<=3){factor=[.2,.4,.6][row-1];offset=1-factor;label='Lighter '+Math.round(offset*100)+'%';}
            else{factor=row===4?.75:.5;label='Darker '+Math.round((1-factor)*100)+'%';}
            mods=[{kind:'lumMod',value:Math.round(factor*100000)},...(offset===undefined?[]:[{kind:'lumOff',value:Math.round(offset*100000)}])];
        }
        const paint=transformedColor(base,mods);result.push({key:`theme:${slot}:${row}`,color:paint.color,label:row?`${slot}, ${label}`:slot,data:{paint}});
    }
    return result;
}
export function paintSelectionKey(paint,swatches) {
    if(!paint?.source)return null;
    const identity=s=>JSON.stringify({...s,transforms:s.transforms.filter(t=>!t.kind.startsWith('alpha'))});
    return swatches.find(s=>s.data?.paint?.source&&identity(s.data.paint.source)===identity(paint.source))?.key??null;
}
export const PRESENTATION_STANDARD_COLORS=Object.freeze(['#C00000','#FF0000','#FFC000','#FFFF00','#92D050','#00B050','#00B0F0','#0070C0','#002060','#7030A0']);
themeColorSwatches.controlFactory=Object.freeze({topics:['drawing','documents'],constructor:'themeColorSwatches({colors:[{slot:"bg1",color:"#FFFFFF",opacity:1},...]})',methods:'Build the ten-column, six-row theme palette with editable color references in caller-owned choice data. Resolve schemes from the actual document; this helper never changes a document. colorWithOpacity preserves RGB/theme transforms while changing alpha.'});
