// Share the same compact UI face across captions, shell labels and controls.
// The bundled fallback keeps the classic look on hosts without Windows fonts.
export const classicUiFont = 'Tahoma, "MS Sans Serif", "RiceWM Classic UI", "RiceWM CJK", sans-serif';
let fontsReady;
export function ensureClassicUiFonts() {
    if (!globalThis.document?.fonts || typeof globalThis.FontFace !== 'function') return null;
    return fontsReady ||= Promise.all([
        ['Regular', '400'], ['Bold', '700'],
    ].map(async ([face, weight]) => {
        const url = new URL(`assets/fonts/office/LiberationSans-${face}.ttf`, import.meta.url);
        const font = new FontFace('RiceWM Classic UI', `url("${url.href}")`, {weight});
        document.fonts.add(await font.load());
    }));
}

// Integer pixels keep the small caption symbols crisp at the native scale.
export function paintClassicCaptionGlyph(context, button, action, maximized, pressed, ink='#000000') {
    const x = Math.floor(button.x + button.width / 2) + Number(pressed);
    const y = Math.floor(button.y + button.height / 2) + Number(pressed);
    context.fillStyle = ink;
    if (action === 'minimize') {
        context.fillRect(x - 4, y + 3, 6, 2);
    } else if (action === 'maximize') {
        const window = (left, top, width, height) => {
            if(ink==='#000000'){context.fillStyle = '#c0c0c0';context.fillRect(left, top, width, height);}
            context.fillStyle = ink;
            context.fillRect(left, top, width, 2);
            context.fillRect(left, top, 1, height);
            context.fillRect(left + width - 1, top, 1, height);
            context.fillRect(left, top + height - 1, width, 1);
        };
        if (maximized) {
            window(x - 2, y - 4, 7, 6);
            window(x - 4, y - 2, 7, 6);
        } else window(x - 4, y - 4, 9, 8);
    } else if (action === 'close') {
        for (let row = 0; row < 7; row += 1) {
            context.fillRect(x - 4 + row, y - 3 + row, 2, 1);
            context.fillRect(x + 2 - row, y - 3 + row, 2, 1);
        }
    }
}
