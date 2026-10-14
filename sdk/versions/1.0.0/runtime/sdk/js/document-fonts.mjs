/** Pinned document faces. Theme chrome deliberately uses its own font stack.
 * CSS and Wasm shaping resolve the same requested family; saved names stay intact.
 */
const families = Object.freeze({
    sans: {family:'DejaVu Sans', css:'Rice Document Sans', file:'DejaVuSans'},
    mono: {family:'DejaVu Sans Mono', css:'Rice Document Mono', file:'DejaVuSansMono'},
    serif: {family:'DejaVu Serif', css:'Rice Document Serif', file:'DejaVuSerif'},
    arial: {family:'Liberation Sans', css:'Rice Document Arial', file:'LiberationSans'},
});
const aliases = new Map([
    ...['dejavu sans','ricewm sans','ricewm editor sans','sans-serif','sans'].map(x=>[x,'sans']),
    ...['dejavu sans mono','ricewm mono','ricewm editor mono','monospace','courier','courier new','consolas','liberation mono'].map(x=>[x,'mono']),
    ...['dejavu serif','serif','times','times new roman','liberation serif','georgia'].map(x=>[x,'serif']),
    ...['arial','helvetica','liberation sans'].map(x=>[x,'arial']),
]);
export function resolveDocumentFont(name) {
    const requested = String(name || '').trim(), key=aliases.get(requested.toLowerCase()) || 'sans', face=families[key];
    return Object.freeze({...face, requested, exact:requested.toLowerCase()===face.family.toLowerCase(),
        compatible:key==='arial'&&['arial','helvetica'].includes(requested.toLowerCase())});
}
export function documentFontResource(name, weight=400, italic=false) {
    const face=resolveDocumentFont(name), bold=Number(weight)>=600;
    if(face.file==='LiberationSans') {
        const style=bold?(italic?'BoldItalic':'Bold'):(italic?'Italic':'Regular');
        return {...face,path:`vendor/pdfjs/standard_fonts/LiberationSans-${style}.ttf`,weight:bold?700:400,style:italic?'italic':'normal'};
    }
    // These bundled DejaVu faces currently provide regular/bold. CSS and the
    // shaping engine may synthesize an oblique face; do not call that exact.
    return {...face,exact:face.exact&&!italic,path:`assets/fonts/document/${face.file}${bold?'-Bold':''}.ttf`,weight:bold?700:400,style:'normal'};
}
