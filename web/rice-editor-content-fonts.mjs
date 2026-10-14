// Content coverage is independent of the desktop's UI language. These are
// licensed release assets, not system-font assumptions or downloaded fonts.
const specs = [
    { key: 'mono', family: 'RiceWM Editor Mono', file: 'ricewm-mono.woff2', match: () => true },
    { key: 'cjk', family: 'RiceWM Document CJK', file: 'locales/jp.ttf',
        match: text => /[\u2e80-\u9fff\uf900-\ufaff\uff00-\uffef\u{20000}-\u{3ffff}]/u.test(text) },
    { key: 'emoji', family: 'RiceWM Document Emoji', file: 'NotoEmoji.ttf',
        match: text => /\p{Extended_Pictographic}|\p{Regional_Indicator}|\u20e3/u.test(text) },
];
const documents = new WeakMap();

export const plainEditorFont = size =>
    `400 ${size}px "RiceWM Editor Mono", "RiceWM Document CJK", "RiceWM Document Emoji", "RiceWM Mono", monospace`;

/** Returns null when coverage is ready; callers repaint once pending work ends. */
export function ensureEditorContentFonts(text, document = globalThis.document || globalThis) {
    const Font = document?.defaultView?.FontFace || globalThis.FontFace;
    if (!document?.fonts || !Font) return null;
    let loads = documents.get(document);
    if (!loads) { loads = new Map(); documents.set(document, loads); }
    const pending = [];
    for (const spec of specs) {
        if (!spec.match(text)) continue;
        let load = loads.get(spec.key);
        if (!load) {
            load = { ready: false };
            const face = new Font(spec.family,
                `url(${new URL(`./assets/fonts/${spec.file}`, import.meta.url).href})`);
            load.promise = face.load().then(() => {
                document.fonts.add(face); load.ready = true;
            });
            loads.set(spec.key, load);
        }
        if (!load.ready) pending.push(load.promise);
    }
    return pending.length ? Promise.all(pending) : null;
}
