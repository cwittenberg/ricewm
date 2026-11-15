import {escapeXmlAttribute} from '../xml-source.mjs';
import {THEME_COLOR_SLOTS,checkedPresentationTheme,checkedThemeFonts,checkedThemeFormat} from './theme-contract.mjs';

const A='http://schemas.openxmlformats.org/drawingml/2006/main';
const esc=escapeXmlAttribute,emu=points=>Math.round(points*12700);
const freeze=value=>{for(const child of Object.values(value))if(child&&typeof child==='object')freeze(child);return Object.freeze(value);};
export const DEFAULT_THEME_FORMAT=freeze({name:'Rice',fills:[{kind:'solid'},{kind:'linear',startLum:110000,endLum:92000,startSaturation:103000},{kind:'linear',startLum:105000,endLum:80000,startSaturation:103000}],lines:[.75,1,1.5],effects:[{},{},{shadow:{blur:6,distance:3,opacity:.4,alignment:'t'}}],backgrounds:[{kind:'solid'},{kind:'solid',tint:95000,saturation:170000},{kind:'linear',startLum:102000,endLum:85000,startSaturation:103000}]});
const placeholder=transforms=>`<a:schemeClr val="phClr">${transforms??''}</a:schemeClr>`;
function fillXml(fill){
    if(fill.kind==='solid')return `<a:solidFill>${placeholder((fill.tint===undefined?'':`<a:tint val="${fill.tint}"/>`)+(fill.saturation===undefined?'':`<a:satMod val="${fill.saturation}"/>`))}</a:solidFill>`;
    return `<a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0">${placeholder(`<a:lumMod val="${fill.startLum}"/>`+(fill.startSaturation===undefined?'':`<a:satMod val="${fill.startSaturation}"/>`))}</a:gs><a:gs pos="100000">${placeholder(`<a:lumMod val="${fill.endLum}"/>`)}</a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/><a:tileRect/></a:gradFill>`;
}
export function presentationFormatSchemeXml(input,namespace=A){
    const format=checkedThemeFormat(input);
    return `<a:fmtScheme xmlns:a="${esc(namespace)}" name="${esc(format.name)}"><a:fillStyleLst>${format.fills.map(fillXml).join('')}</a:fillStyleLst><a:lnStyleLst>${format.lines.map(width=>`<a:ln w="${emu(width)}" cap="flat" cmpd="sng" algn="ctr"><a:solidFill>${placeholder()}</a:solidFill><a:prstDash val="solid"/><a:miter lim="800000"/></a:ln>`).join('')}</a:lnStyleLst><a:effectStyleLst>${format.effects.map(effect=>{
        let xml='';
        if(effect.shadow){const s=effect.shadow;xml+=`<a:outerShdw blurRad="${emu(s.blur)}" dist="${emu(s.distance)}" dir="5400000" sx="100000" sy="100000" kx="0" ky="0" algn="${s.alignment}" rotWithShape="0"><a:srgbClr val="000000"><a:alpha val="${Math.round(s.opacity*100000)}"/></a:srgbClr></a:outerShdw>`;}
        if(effect.glow){const g=effect.glow;xml+=`<a:glow rad="${emu(g.radius)}">${placeholder(`<a:alpha val="${Math.round(g.opacity*100000)}"/>`)}</a:glow>`;}
        return `<a:effectStyle>${xml?'<a:effectLst>'+xml+'</a:effectLst>':'<a:effectLst/>'}</a:effectStyle>`;
    }).join('')}</a:effectStyleLst><a:bgFillStyleLst>${format.backgrounds.map(fillXml).join('')}</a:bgFillStyleLst></a:fmtScheme>`;
}
export function presentationFontSchemeXml(input,namespace=A){
    const fonts=checkedThemeFonts(input);
    const set=(name,font)=>`<a:${name}Font><a:latin typeface="${esc(font.latin)}"/><a:ea typeface="${esc(font.ea)}"/><a:cs typeface="${esc(font.cs)}"/>${font.scripts.map(s=>`<a:font script="${s.script}" typeface="${esc(s.typeface)}"/>`).join('')}</a:${name}Font>`;
    return `<a:fontScheme xmlns:a="${esc(namespace)}" name="${esc(fonts.name)}">${set('major',fonts.major)}${set('minor',fonts.minor)}</a:fontScheme>`;
}
export function presentationColorSchemeXml(name,colors,namespace=A){
    if(typeof name!=='string'||!name.trim()||name.length>256||!Array.isArray(colors)||colors.length!==12||!colors.every(c=>typeof c==='string'&&/^#[0-9a-f]{6}$/iu.test(c)))throw TypeError('Invalid presentation color scheme');
    return `<a:clrScheme xmlns:a="${esc(namespace)}" name="${esc(name)}">${THEME_COLOR_SLOTS.map((slot,i)=>`<a:${slot}><a:srgbClr val="${colors[i].slice(1).toUpperCase()}"/></a:${slot}>`).join('')}</a:clrScheme>`;
}
export function presentationThemeXml(input,namespace=A){
    const theme=checkedPresentationTheme(input);
    return `<a:theme xmlns:a="${esc(namespace)}" name="${esc(theme.name)}"><a:themeElements>${presentationColorSchemeXml(theme.colorsName,theme.colors,namespace)}${presentationFontSchemeXml(theme.fonts,namespace)}${presentationFormatSchemeXml(theme.format,namespace)}</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>`;
}
