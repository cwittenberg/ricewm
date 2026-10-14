export function riceSdkThemePalette(theme) {
    const menu=theme?.menu,frame=theme?.frame;
    let accent=theme?.taskbar?.[6];
    const valid=color=>Number.isInteger(color)&&color>=0&&color<=0xffffff;
    if(!Array.isArray(menu)||menu.length!==6||![...menu,accent].every(valid))return null;
    let [background,panel,border,,,foreground]=menu;
    // Match native_ui_theme.rs: glass popup colors are separate from the
    // application's content surfaces, which use the frame palette.
    if(theme.menuMaterial==='glass') {
        if(!Array.isArray(frame)||![frame[2],frame[3],frame[4],frame[5]].every(valid))return null;
        background=frame[3];panel=frame[5];foreground=frame[4];border=frame[2];
    } else if(theme.chromeProfile==='redmond-98') {
        background=0xc0c0c0;panel=0xffffff;foreground=0;accent=0x000080;border=0x808080;
    }
    const blend=(a,b)=>[16,8,0].reduce((color,shift)=>color|Math.floor((((a>>>shift)&255)*66+((b>>>shift)&255)*34)/100)<<shift,0);
    return {background,panel,foreground,muted:blend(foreground,background),border,accent};
}

