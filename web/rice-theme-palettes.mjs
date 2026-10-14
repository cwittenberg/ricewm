const groups = {
    taskbar: ['taskbar.bar','taskbar.ridge','taskbar.start','taskbar.task','taskbar.tray','taskbar.text','accent'],
    menu: ['menu.background','menu.alternate','menu.separator','menu.hover','menu.selected','menu.text'],
    frame: ['frame.active','frame.inactive','frame.border','frame.body','frame.text','frame.control','frame.close','frame.pressed'],
};
const colorKeys = new Set([...Object.values(groups).flat(),'icon','icon.hover']);
const idPattern = /^[a-z0-9][a-z0-9-]{0,47}$/;
export function validateThemePalettes(bundle) {
    if (bundle.palettes === undefined) {
        if (bundle.defaultPalette !== undefined) throw new TypeError('Default palette is missing');
        return;
    }
    if (!Array.isArray(bundle.palettes) || !bundle.palettes.length || bundle.palettes.length > 16) throw new TypeError('A theme supports 1–16 palettes');
    const ids = new Set();
    for (const palette of bundle.palettes) {
        if (!palette || !idPattern.test(palette.id) || ids.has(palette.id)) throw new TypeError('Invalid or duplicate palette ID');
        ids.add(palette.id);
        if (typeof palette.name !== 'string' || !palette.name.trim() || palette.name.length > 64 || /[\x00-\x1f]/.test(palette.name)) throw new TypeError('Invalid palette name');
        if (!palette.appearance || typeof palette.appearance !== 'object' || Array.isArray(palette.appearance)) throw new TypeError('Palette appearance must contain colors');
        for (const [key,value] of Object.entries(palette.appearance)) {
            if (!colorKeys.has(key) || typeof value !== 'string' || !/^#?[a-f\d]{6}$/i.test(value)) throw new TypeError('Invalid palette color: '+key);
        }
        if (palette.terminalPalette !== undefined) {
            const terminal = palette.terminalPalette;
            const rgb = value => Number.isInteger(value) && value >= 0 && value <= 0xffffff;
            if (!terminal || !['background','foreground','selection','selectionForeground'].every(key => rgb(terminal[key])) ||
                !Array.isArray(terminal.ansi) || terminal.ansi.length !== 16 || !terminal.ansi.every(rgb) ||
                (terminal.imageAccent !== undefined && !rgb(terminal.imageAccent))) throw new TypeError('Invalid terminal palette');
        }
        if (palette.wallpaper !== undefined && !bundle.wallpapers?.some(w=>w.id===palette.wallpaper)) throw new TypeError('Palette wallpaper is missing');
    }
    if (bundle.defaultPalette !== undefined && !ids.has(bundle.defaultPalette)) throw new TypeError('Default palette is missing');
}
export function resolveThemePalette(source, requested) {
    const theme = structuredClone(source), bundle = theme.bundle || {};
    validateThemePalettes(bundle);
    if (!bundle.palettes?.length) return theme;
    const palette = bundle.palettes.find(p=>p.id===requested) || bundle.palettes.find(p=>p.id===bundle.defaultPalette) || bundle.palettes[0];
    const color = key => palette.appearance[key] === undefined ? undefined : parseInt(palette.appearance[key].replace(/^#/,''),16);
    for (const [group,keys] of Object.entries(groups)) theme[group] = keys.map((key,i)=>color(key) ?? theme[group][i]);
    theme.icon = color('icon') ?? theme.icon;
    theme.iconHover = color('icon.hover') ?? theme.iconHover;
    theme.paletteId = palette.id;
    if (palette.terminalPalette) theme.terminalTheme = structuredClone(palette.terminalPalette);
    if (bundle.surface && color('accent') !== undefined) bundle.surface.edge = '#'+color('accent').toString(16).padStart(6,'0');
    if (palette.wallpaper) {
        bundle.defaultWallpaper = palette.wallpaper;
        const wallpaper = bundle.wallpapers.find(w=>w.id===palette.wallpaper);
        bundle.background = {type:wallpaper.type || 'image',source:wallpaper.source,mode:wallpaper.mode || 'Crop'};
    }
    return theme;
}
