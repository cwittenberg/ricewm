import {PRESENTATION_FONT_FACES} from './presentation-font-resources.mjs';

/** Pinned document faces. Theme chrome deliberately uses its own font stack.
 * CSS and Wasm shaping resolve the same requested family; saved names stay intact.
 */
const families = Object.freeze({
    sans: {family:'DejaVu Sans', css:'Rice Document Sans', file:'DejaVuSans'},
    mono: {family:'DejaVu Sans Mono', css:'Rice Document Mono', file:'DejaVuSansMono'},
    serif: {family:'DejaVu Serif', css:'Rice Document Serif', file:'DejaVuSerif'},
    arial: {family:'Liberation Sans', css:'Rice Document Arial', file:'LiberationSans'},
    inter: {family:'Inter', css:'Rice Document Inter', file:'Inter'},
    manrope: {family:'Manrope', css:'Rice Document Manrope', file:'Manrope'},
});
// Literal resource URLs keep these licensed faces in the selected SDK's closure.
// The host uses the same bytes for measured input and canvas painting.
const interFaces = Object.freeze({
    Regular: new URL('../resources/fonts/inter/Inter-Regular.ttf', import.meta.url).href,
    Italic: new URL('../resources/fonts/inter/Inter-Italic.ttf', import.meta.url).href,
    SemiBold: new URL('../resources/fonts/inter/Inter-SemiBold.ttf', import.meta.url).href,
    Bold: new URL('../resources/fonts/inter/Inter-Bold.ttf', import.meta.url).href,
});
const aliases = new Map([
    ...['dejavu sans','ricewm sans','ricewm editor sans','sans-serif','sans'].map(x=>[x,'sans']),
    ...['dejavu sans mono','ricewm mono','ricewm editor mono','monospace','courier','courier new','consolas','liberation mono'].map(x=>[x,'mono']),
    ...['dejavu serif','serif','times','times new roman','liberation serif','georgia'].map(x=>[x,'serif']),
    ...['arial','helvetica','liberation sans'].map(x=>[x,'arial']),
    ['inter','inter'],
    ['manrope','manrope'],
]);
// Reflect the existing OS inventory used by presentation shaping. Do not
// maintain another font catalog or copy its payloads into SDK snapshots.
const presentationFamilies=new Map();
for(const font of PRESENTATION_FONT_FACES){
    const key=font.family.toLowerCase();
    if(!presentationFamilies.has(key))presentationFamilies.set(key,[]);
    presentationFamilies.get(key).push(font);
}
export function resolveDocumentFont(name) {
    const requested = String(name || '').trim(), reference=presentationFamilies.get(requested.toLowerCase());
    if(reference){
        const family=reference[0].family;
        return Object.freeze({family,css:'Rice Document '+family,file:family.replaceAll(' ',''),requested,exact:true,compatible:false,source:'presentation'});
    }
    const key=aliases.get(requested.toLowerCase()) || 'sans', face=families[key];
    return Object.freeze({...face, requested, exact:requested.toLowerCase()===face.family.toLowerCase(),
        compatible:key==='arial'&&['arial','helvetica'].includes(requested.toLowerCase())});
}
export function documentFontResource(name, weight=400, italic=false) {
    const face=resolveDocumentFont(name), bold=Number(weight)>=600;
    if(face.file==='Inter') {
        const style=italic?'Italic':Number(weight)>=700?'Bold':bold?'SemiBold':'Regular';
        return {...face, exact:face.exact&&!(italic&&bold),
            path:`sdk/resources/fonts/inter/Inter-${style}.ttf`,url:interFaces[style],
            weight:italic?400:Number(weight)>=700?700:bold?600:400,style:italic?'italic':'normal'};
    }
    if(face.file==='Manrope')return {...face,exact:face.exact&&!italic&&Number(weight)>=200&&Number(weight)<=800,
        path:'assets/fonts/Manrope-VariableFont_wght.ttf',weight:Math.max(200,Math.min(800,Number(weight))),weightRange:[200,800],style:'normal'};
    if(face.source==='presentation'){
        const faces=presentationFamilies.get(face.family.toLowerCase()),styled=faces.filter(font=>font.italic===Boolean(italic));
        const available=styled.length?styled:faces,wanted=Number(weight);
        const range=font=>font.variableWeight&&(!font.weightRange||wanted>=font.weightRange[0]&&wanted<=font.weightRange[1]);
        const font=available.find(font=>!font.variableWeight&&font.weight===wanted)||available.find(range)
            ||available.reduce((best,font)=>Math.abs(font.weight-wanted)<Math.abs(best.weight-wanted)?font:best);
        return {...face,exact:face.exact&&font.italic===Boolean(italic)&&(font.weight===wanted||range(font)),
            path:font.path,officeFontId:font.id,bytes:font.bytes,sha256:font.parts[0].sha256,
            weight:font.variableWeight?Math.max(font.weightRange?.[0]??1,Math.min(font.weightRange?.[1]??1000,wanted)):font.weight,
            weightRange:font.variableWeight?font.weightRange:null,style:font.italic?'italic':'normal'};
    }
    if(face.file==='LiberationSans') {
        const style=bold?(italic?'BoldItalic':'Bold'):(italic?'Italic':'Regular');
        return {...face,path:`vendor/pdfjs/standard_fonts/LiberationSans-${style}.ttf`,weight:bold?700:400,style:italic?'italic':'normal'};
    }
    // These bundled DejaVu faces currently provide regular/bold. CSS and the
    // shaping engine may synthesize an oblique face; do not call that exact.
    return {...face,exact:face.exact&&!italic,path:`assets/fonts/document/${face.file}${bold?'-Bold':''}.ttf`,weight:bold?700:400,style:'normal'};
}
