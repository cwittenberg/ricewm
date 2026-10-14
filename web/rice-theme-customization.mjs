/** Shared data-only theme geometry and artwork contract for SDK and Herdr. */
export const WINDOW_DECORATION_SCHEMA = Object.freeze({
    controlBoxPosition: ['Theme Default','Left','Right'], controlLayout:['grouped','split'],
    titlebarEnd:['Straight','Inclined','Rounded'], buttonStyle:['Default','Wedged','Pill','Chamfered','Borderless'],
    windowShape:['Default','Wedged'], decorationArtwork:['Theme Default','On','Off'],
    compactTitlebar:'boolean', compactTitlebarFullWidthBackdrop:'boolean',
    ricewmMenuBorder:{min:0,max:100}, frameBorderWidth:{min:0,max:4},
    captionWidth:{min:12,max:96}, captionHeight:{min:12,max:64}, captionGap:{min:0,max:16},
    captionRadius:{min:0,max:32}, glyphWidth:{min:3,max:32}, glyphHeight:{min:3,max:32},
    glyphStrokeWidth:{min:0.5,max:4}, glyphStyle:['Theme Default','geometric'],
    captionBackground:'color', captionHoverBackground:'color', captionPressedBackground:'color',
    captionCloseBackground:'color', captionCloseHoverBackground:'color', captionClosePressedBackground:'color',
    captionForeground:'color', captionHoverForeground:'color', captionPressedForeground:'color',
    captionCloseForeground:'color', captionCloseHoverForeground:'color', captionClosePressedForeground:'color',
});
export function validateWindowDecorations(value) {
    if(!value || typeof value!=='object' || Array.isArray(value))throw TypeError('Window decorations must be an object');
    for(const [key,v] of Object.entries(value)) {
        const rule=WINDOW_DECORATION_SCHEMA[key];
        if(!rule)throw TypeError('Unknown window decoration '+key);
        const valid=Array.isArray(rule)?rule.includes(v):rule==='boolean'?typeof v==='boolean':rule==='color'?typeof v==='string'&&/^(#[a-f0-9]{6}|transparent)$/i.test(v):Number.isFinite(v)&&v>=rule.min&&v<=rule.max;
        if(!valid)throw TypeError('Invalid window decoration '+key);
    }
    return structuredClone(value);
}
export function validateThemeIcons(value={}) {
    if(!value || typeof value!=='object' || Array.isArray(value) || Object.keys(value).length>256 || JSON.stringify(value).length>48*1024)throw TypeError('Theme icons must be a map of up to 256 icons and 48 KiB');
    const safe=/^[a-z0-9]+(?:-[a-z0-9]+)*$/;
    for(const [name,icon] of Object.entries(value)) {
        if(!safe.test(name)&&!/^app:[A-Za-z0-9][A-Za-z0-9_.:+/@-]{0,254}$/.test(name)&&!/^mime:[a-z0-9][a-z0-9.+-]*\/(?:[a-z0-9][a-z0-9.+-]*|\*)$/.test(name))throw TypeError('Invalid theme icon name');
        if(typeof icon==='string'){if(!safe.test(icon))throw TypeError('Invalid theme icon alias');continue;}
        if(!icon || typeof icon!=='object' || Array.isArray(icon) || Object.keys(icon).some(k=>!['svg','png','monochrome','notice'].includes(k)) || (typeof (icon.svg??icon.png)!=='string') || (icon.svg!==undefined&&icon.png!==undefined) || (icon.svg??icon.png).length>16*1024 || (icon.monochrome!==undefined&&typeof icon.monochrome!=='boolean') || (icon.notice!==undefined&&(typeof icon.notice!=='string'||icon.notice.length>16*1024)))throw TypeError('Invalid theme icon artwork');
        if(icon.png!==undefined) {
            if(!/^[A-Za-z0-9+/]+={0,2}$/.test(icon.png))throw TypeError('Invalid theme PNG');
            const bytes=Uint8Array.from(atob(icon.png),c=>c.charCodeAt(0));
            if(bytes.length<33 || ![137,80,78,71,13,10,26,10].every((v,i)=>bytes[i]===v))throw TypeError('Invalid theme PNG');
            const view=new DataView(bytes.buffer),w=view.getUint32(16),h=view.getUint32(20);
            if(view.getUint32(8)!==13 || String.fromCharCode(...bytes.slice(12,16))!=='IHDR' || !w||!h||w>512||h>512)throw TypeError('Theme PNG dimensions exceed 512 pixels');
            continue;
        }
        const svg=icon.svg;
        if(!/^\s*<svg[\s/>]/i.test(svg)||/<!(?:DOCTYPE|ENTITY)|\bon\w+\s*=|\b(?:href|src)\s*=|url\s*\(|@import|javascript:/i.test(svg.replace(/url\(\s*(['"]?)#[A-Za-z_][\w.-]*\1\s*\)/gi,'')))throw TypeError('Theme icon must be self-contained SVG');
        const allowed=new Set(['svg','g','path','rect','circle','ellipse','line','polyline','polygon','defs','lineargradient','radialgradient','stop','clippath','title','desc']);
        for(const match of svg.matchAll(/<\/?\s*([\w:-]+)/g))if(!allowed.has(match[1].toLowerCase()))throw TypeError('Unsupported theme SVG element');
    }
    for(const name of Object.keys(value)){const seen=new Set();let current=name;while(typeof value[current]==='string'){if(seen.has(current))throw TypeError('Theme icon alias cycle');seen.add(current);current=value[current];}}
    return structuredClone(value);
}
export function themeColor(value,fallback) {
    if(value==='transparent')return [0,0,0,0];
    if(!/^#[a-f0-9]{6}(?:[a-f0-9]{2})?$/i.test(value||''))return fallback;
    const n=parseInt(value.slice(1),16);return value.length===9?[n>>>24,(n>>>16)&255,(n>>>8)&255,n&255]:[n>>>16,(n>>>8)&255,n&255,255];
}

export const THEME_FONT_FAMILIES = Object.freeze({system:'"RiceWM Sans", "RiceWM CJK", sans-serif',monospace:'"RiceWM Mono", "RiceWM CJK", monospace',serif:'Georgia, "RiceWM CJK", serif',classic:'"RiceWM Classic UI", "RiceWM CJK", sans-serif',manrope:'"Manrope", "RiceWM Sans", sans-serif'});
export const TYPOGRAPHY_SCHEMA = Object.freeze({family:Object.keys(THEME_FONT_FAMILIES),titleFamily:Object.keys(THEME_FONT_FAMILIES),bodyPx:{min:8,max:32},smallPx:{min:8,max:28},titlePx:{min:8,max:36},displayPx:{min:12,max:72},bodyWeight:{min:100,max:900},titleWeight:{min:100,max:900},lineHeightPercent:{min:100,max:200}});
export const CONTROL_METRICS_SCHEMA = Object.freeze({titlebarHeight:{min:20,max:72},controlHeight:{min:16,max:72},menuRowHeight:{min:16,max:72},scrollbarWidth:{min:4,max:32},focusWidth:{min:0,max:6},borderWidth:{min:0,max:6},cornerRadius:{min:0,max:32},controlPaddingX:{min:0,max:32},controlPaddingY:{min:0,max:24}});
export const SURFACE_ROLES=Object.freeze(['button','tab','menu','menuItem','dock','notification','separator','runningIndicator','input','checkbox','switch','slider','scrollbar','window']);
export const CONTROL_STATES=Object.freeze(['normal','hover','pressed','focused','disabled','selected']);
const isObject=value=>value&&typeof value==='object'&&!Array.isArray(value);
export function validateThemeTokens(value,schema,label) {
    if(!isObject(value))throw TypeError(label+' must be an object');
    for(const [key,v] of Object.entries(value)) {const rule=schema[key];if(!rule||!(Array.isArray(rule)?rule.includes(v):Number.isInteger(v)&&v>=rule.min&&v<=rule.max))throw TypeError('Invalid '+label+' '+key);}
    return structuredClone(value);
}
const validColor=value=>typeof value==='string'&&/^(#[a-f0-9]{6}(?:[a-f0-9]{2})?|transparent)$/i.test(value);
export function validateControlColors(value) {
    if(!isObject(value))throw TypeError('Control colors must be an object');
    for(const [role,states] of Object.entries(value)) {
        if(!SURFACE_ROLES.includes(role)||!isObject(states))throw TypeError('Invalid control color role');
        for(const [state,colors] of Object.entries(states))if(!CONTROL_STATES.includes(state)||!isObject(colors)||Object.entries(colors).some(([key,v])=>!['background','foreground','border','focus'].includes(key)||!validColor(v)))throw TypeError('Invalid control state colors');
    }
    return structuredClone(value);
}
export function validateSurfaceSkins(value) {
    if(!isObject(value)||JSON.stringify(value).length>48*1024)throw TypeError('Surface skins must be a map within 48 KiB');
    for(const [role,states] of Object.entries(value)) {
        if(!SURFACE_ROLES.includes(role)||!isObject(states))throw TypeError('Invalid surface skin role');
        for(const [state,skin] of Object.entries(states)) {
            if(!CONTROL_STATES.includes(state)||!isObject(skin)||Object.keys(skin).some(key=>!['artwork','slice','imageMode','gradient','background','border','shadow','radius','texture','foreground'].includes(key)))throw TypeError('Invalid surface skin state');
            for(const key of ['background','foreground'])if(skin[key]!==undefined&&!validColor(skin[key]))throw TypeError('Invalid skin color');
            if(skin.artwork!==undefined&&skin.artwork!==null){if(!isObject(skin.artwork))throw TypeError('Skin artwork needs SVG or PNG');validateThemeIcons({'surface-skin':skin.artwork});}
            if(skin.imageMode!==undefined&&!['stretch','tile','contain'].includes(skin.imageMode))throw TypeError('Invalid skin image mode');
            if(skin.slice!==undefined&&(!(skin.artwork===undefined?states.normal?.artwork:skin.artwork)||!Array.isArray(skin.slice)||skin.slice.length!==4||skin.slice.some(v=>!Number.isInteger(v)||v<0||v>128)))throw TypeError('Skin slice needs four source pixel margins');
            if(skin.radius!==undefined&&(!Number.isInteger(skin.radius)||skin.radius<0||skin.radius>64))throw TypeError('Invalid skin radius');
            if(skin.gradient!==undefined){const g=skin.gradient;if(!isObject(g)||Object.keys(g).some(k=>!['angle','stops'].includes(k))||!Number.isFinite(g.angle??90)||Math.abs(g.angle??90)>360||!Array.isArray(g.stops)||g.stops.length<2||g.stops.length>8||g.stops.some((s,i)=>!isObject(s)||Object.keys(s).some(k=>!['at','color'].includes(k))||!Number.isFinite(s.at)||s.at<0||s.at>1||!validColor(s.color)||(i&&s.at<g.stops[i-1].at)))throw TypeError('Invalid skin gradient');}
            for(const key of ['border','shadow'])if(skin[key]!==undefined){const v=skin[key],rules=key==='border'?{width:[0,8]}:{blur:[0,32],x:[-32,32],y:[-32,32]};if(!isObject(v)||!validColor(v.color)||Object.keys(v).some(k=>k!=='color'&&(!rules[k]||!Number.isFinite(v[k])||v[k]<rules[k][0]||v[k]>rules[k][1])))throw TypeError('Invalid skin '+key);}
            if(skin.texture!==undefined){const t=skin.texture;if(!isObject(t)||Object.keys(t).some(k=>!['kind','color','spacing','opacity'].includes(k))||!['dots','lines','grid'].includes(t.kind)||!validColor(t.color)||!Number.isInteger(t.spacing)||t.spacing<2||t.spacing>32||!Number.isFinite(t.opacity??1)||(t.opacity??1)<0||(t.opacity??1)>1)throw TypeError('Invalid skin texture');}
        }
    }
    return structuredClone(value);
}
export function themeStateStyle(map,role,state='normal') {return {...map?.[role]?.normal,...map?.[role]?.[state]};}
export function desktopThemeFont(typography={},role='body',size) {const title=role==='title';return `${typography[title?'titleWeight':'bodyWeight']??(title?600:400)} ${size??typography[title?'titlePx':'bodyPx']??(title?13:14)}px ${THEME_FONT_FAMILIES[typography[title?'titleFamily':'family']??typography.family??'system']}`;}
