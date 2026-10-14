import {documentFontMetricsFromBytes} from './rice-document-font-metrics.mjs';
import {resolveDocumentFont, documentFontResource} from '../sdk/versions/1.0.0/runtime/sdk/js/document-fonts.mjs';
// Shared by document text painting and measured native single-line input.
export function documentFontFamily(name) {
    const face=resolveDocumentFont(name);
    return `"${face.css}", "RiceWM CJK", ${face.file.includes('Mono')?'monospace':face.file.includes('Serif')?'serif':'sans-serif'}`;
}
export const documentCanvasFont = command => `${command.font.italic ? 'italic ' : ''}${command.font.weight} ${Math.max(1,Number(command.sizePx)||12)}px ${documentFontFamily(command.font.family)}`;
export function applyDocumentFont(context, command) {
    context.font=documentCanvasFont(command);
    // Preserve fractional OpenType advances used by the shared Wasm shaper.
    // Chromium's auto mode otherwise rounds each advance to a device pixel.
    context.textRendering='geometricPrecision';
}
// Grid text and its inline editor share baselines without changing the full
// cell rectangle used for clipping and native pointer ownership.
export function documentTextBlockGeometry(command, lineCount = 1) {
    // Native document records carry the unzoomed line box scaled once. Rounding
    // each already-zoomed line accumulates overflow in multiline cells even
    // when their physical row height scales correctly. Legacy/chrome commands
    // retain their existing metrics; the wire ABI is unchanged.
    const scaledLine = (command.documentText || command.documentEditor)
        && Number.isFinite(command.lineHeightPx) && command.lineHeightPx > 0;
    const lineHeight = scaledLine ? command.lineHeightPx
        : Math.max(command.font || command.documentText ? 1 : 10,
            Math.round((Number(command.sizePx) || 12) * 1.25));
    const contentHeight = Math.max(1, lineCount) * lineHeight;
    const {y, height} = command.rect;
    const top = y + (command.vertical === 'top' ? 0
        : command.vertical === 'bottom' ? height - contentHeight
        : (height - contentHeight) / 2);
    return {lineHeight, contentHeight, top, baseline: top + lineHeight / 2};
}
export function singleLineGeometry(context, command) {
    context.textRendering='geometricPrecision';
    const text = String(command.text || ''), characters = Array.from(text);
    const caret = Number.isInteger(command.caretOffset) ? Math.max(0,Math.min(characters.length,command.caretOffset)) : characters.length;
    const advance = context.measureText(characters.slice(0,caret).join('')).width;
    const available = Math.max(1,command.rect.width-4);
    const scroll = command.caret ? Math.max(0,advance-available+1) : 0;
    return {text,characters,caret,advance,available,scroll,textX:command.rect.x+2-scroll,
        ...documentTextBlockGeometry(command)};
}
export function documentTextHit(context, command, x) {
    context.save();
    try {
        applyDocumentFont(context,command); context.direction='ltr';
        const {characters,textX} = singleLineGeometry(context,command), target=x-textX;
        if (target<=0) return 0;
        let low=0, high=characters.length;
        while(low<high) {
            const middle=Math.floor((low+high)/2);
            const before=context.measureText(characters.slice(0,middle).join('')).width;
            const after=context.measureText(characters.slice(0,middle+1).join('')).width;
            if(target<(before+after)/2) high=middle; else low=middle+1;
        }
        return low;
    } finally { context.restore(); }
}
const environments = new WeakMap(),fontMetrics=new Map();
export function documentFontMetrics(command) { const face=documentFontResource(command.font.family,command.font.weight,command.font.italic);return fontMetrics.get(face.path)||null; }
/** Offscreen measurement uses the same bundled faces as the desktop canvas. */
export function ensureDocumentTextFonts(command, environment=globalThis.document||globalThis) {
    const Font = environment?.defaultView?.FontFace || globalThis.FontFace;
    if(!environment?.fonts || !Font) return null;
    const face=documentFontResource(command.font.family,command.font.weight,command.font.italic);
    const key=`${face.css}:${face.weight}:${face.style}`;
    let loads=environments.get(environment); if(!loads){loads=new Map();environments.set(environment,loads);}
    let load=loads.get(key);
    if(!load){
        load={ready:false};
        load.promise=fetch(new URL(`./${face.path}`,import.meta.url)).then(response=>{
            if(!response.ok)throw new Error('Document font resource failed');return response.arrayBuffer();
        }).then(async bytes=>{
            const metrics=documentFontMetricsFromBytes(bytes);
            const font=new Font(face.css,bytes,{weight:String(face.weight),style:face.style});
            await font.load();environment.fonts.add(font);fontMetrics.set(face.path,metrics);load.ready=true;
        });loads.set(key,load);
    }
    return load.ready?null:load.promise;
}
