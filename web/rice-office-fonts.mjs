import {OFFICE_FONT_FACES} from './rice-office-font-manifest.mjs';

const documents = new WeakMap();
export const officeFace = id => OFFICE_FONT_FACES[id];
export function officeFamilyFace(name) {
    const family = /mono|courier|consolas/i.test(name) ? 8 : /serif|times|cambria|georgia/i.test(name) && !/sans/i.test(name) ? 4 : 0;
    return officeFace(family);
}
export function officeCanvasFont(face, size, bold = false) {
    return `${face.italic ? 'italic ' : ''}${face.bold || (face.variable_weight && bold) ? 700 : 400} ${size}px "${face.css_family}"`;
}
/** Office-only fonts, registered once per document. Failed loads do not loop. */
export function ensureOfficeFonts(layout, document = globalThis.document || globalThis) {
    if (!layout) return null;
    const Font = document?.defaultView?.FontFace || globalThis.FontFace;
    if (!document?.fonts || !Font) return null;
    let loads = documents.get(document);
    if (!loads) { loads = new Map(); documents.set(document, loads); }
    const ids = new Set(layout.runs.map(run => run.face));
    for (const marker of layout.markers) ids.add(officeFamilyFace(marker.font).id);
    const pending = [];
    for (const id of ids) {
        const spec = officeFace(id); if (!spec) continue;
        let load = loads.get(id);
        if (!load) {
            load = { done: false };
            loads.set(id, load);
            const face = new Font(spec.css_family, `url(${new URL(spec.web_path, import.meta.url).href})`, {
                weight: spec.variable_weight ? '100 900' : spec.bold ? '700' : '400',
                style: spec.italic ? 'italic' : 'normal',
            });
            load.promise = face.load().then(() => { document.fonts.add(face); }).finally(() => { load.done = true; });
        }
        if (!load.done) pending.push(load.promise);
    }
    return pending.length ? Promise.all(pending) : null;
}
