import {exact,failure} from './contract-values.mjs';

export const THEME_COLOR_SLOTS=Object.freeze(['dk1','lt1','dk2','lt2','accent1','accent2','accent3','accent4','accent5','accent6','hlink','folHlink']);
export const THEME_BYTES=8*1024*1024;
export const themeAsset=value=>typeof value==='string'&&/^theme:sha256:[0-9a-f]{64}$/u.test(value);
export const THEME_COMMANDS=new Set(['set-presentation-theme','set-theme-colors','set-theme-fonts','set-theme-format','set-theme-source']);
const invalid=message=>{throw failure('INVALID_REQUEST','Presentation theme: '+message);};
const name=value=>typeof value==='string'&&value.trim().length>0&&value.length<=256&&!/[\x00-\x1f\x7f]/u.test(value);
const number=(v,min,max)=>Number.isFinite(v)&&v>=min&&v<=max;
const transform=(v,min,max)=>Number.isInteger(v)&&number(v,min,max);
export function checkedThemeFontSet(value){
    if(!exact(value,['latin','ea','cs','scripts'],['latin'])||!name(value.latin)||
        ['ea','cs'].some(k=>value[k]!==undefined&&(typeof value[k]!=='string'||value[k].length>256||/[\x00-\x1f\x7f]/u.test(value[k])))||
        value.scripts!==undefined&&(!Array.isArray(value.scripts)||value.scripts.length>64||value.scripts.some(v=>!exact(v,['script','typeface'])||!/^[A-Za-z]{4}$/u.test(v.script)||!name(v.typeface))||new Set(value.scripts.map(v=>v.script.toLowerCase())).size!==value.scripts.length))invalid('invalid font set');
    return {latin:value.latin.trim(),ea:value.ea??'',cs:value.cs??'',scripts:(value.scripts??[]).map(v=>({...v,typeface:v.typeface.trim()}))};
}
export function checkedThemeFonts(value){
    if(!exact(value,['name','major','minor'])||!name(value.name))invalid('invalid font scheme');
    return {name:value.name.trim(),major:checkedThemeFontSet(value.major),minor:checkedThemeFontSet(value.minor)};
}
export function checkedThemeFormat(value){
    if(!exact(value,['name','fills','lines','effects','backgrounds'])||!name(value.name))invalid('invalid style matrix');
    const fills=items=>{
        if(!Array.isArray(items)||items.length!==3)invalid('three style fills required');
        return items.map(fill=>{
            if(fill?.kind==='solid'&&exact(fill,['kind','tint','saturation'],['kind'])&&
                (fill.tint===undefined||transform(fill.tint,0,100000))&&(fill.saturation===undefined||transform(fill.saturation,0,1000000)))return {...fill};
            if(fill?.kind==='linear'&&exact(fill,['kind','startLum','endLum','startSaturation'],['kind','startLum','endLum'])&&
                transform(fill.startLum,0,1000000)&&transform(fill.endLum,0,1000000)&&(fill.startSaturation===undefined||transform(fill.startSaturation,0,1000000)))return {...fill};
            invalid('unsupported style fill');
        });
    };
    if(!Array.isArray(value.lines)||value.lines.length!==3||!value.lines.every(v=>number(v,.01,20))||!Array.isArray(value.effects)||value.effects.length!==3)invalid('three lines and effects required');
    const effects=value.effects.map(effect=>{
        if(!exact(effect,['shadow','glow'],[]))invalid('invalid style effect');
        if(effect.shadow!==undefined){const s=effect.shadow;if(!exact(s,['blur','distance','opacity','alignment'])||!number(s.blur,0,192)||!number(s.distance,0,192)||!number(s.opacity,0,1)||!['t','b','ctr'].includes(s.alignment))invalid('invalid style shadow');}
        if(effect.glow!==undefined){const g=effect.glow;if(!exact(g,['radius','opacity'])||!number(g.radius,0,192)||!number(g.opacity,0,1))invalid('invalid style glow');}
        return structuredClone(effect);
    });
    return {name:value.name.trim(),fills:fills(value.fills),lines:[...value.lines],effects,backgrounds:fills(value.backgrounds)};
}
export function checkedPresentationTheme(value){
    if(!exact(value,['name','colorsName','colors','fonts','format'])||!name(value.name)||!name(value.colorsName)||!Array.isArray(value.colors)||value.colors.length!==12||!value.colors.every(v=>typeof v==='string'&&/^#[0-9a-f]{6}$/iu.test(v)))invalid('invalid theme definition');
    return {name:value.name.trim(),colorsName:value.colorsName.trim(),colors:value.colors.map(v=>v.toUpperCase()),fonts:checkedThemeFonts(value.fonts),format:checkedThemeFormat(value.format)};
}
export function checkedThemeCommand(command){
    if(!THEME_COMMANDS.has(command?.type)||typeof command.slide!=='string'||!command.slide||command.slide.length>4096)invalid('invalid source slide');
    if(command.type==='set-theme-source'){if(!exact(command,['type','slide','asset'])||!themeAsset(command.asset))invalid('invalid prepared theme resource');return {...command};}
    if(command.type==='set-presentation-theme'){
        if(!exact(command,['type','slide','theme']))invalid('invalid theme command');return {...command,theme:checkedPresentationTheme(command.theme)};
    }
    if(command.type==='set-theme-format'){
        if(!exact(command,['type','slide','format']))invalid('invalid format command');return {...command,format:checkedThemeFormat(command.format)};
    }
    if(command.type==='set-theme-colors'){
        if(!exact(command,['type','slide','name','colors'],['type','slide','colors'])||command.name!==undefined&&!name(command.name)||!command.colors||typeof command.colors!=='object'||Array.isArray(command.colors)||!Object.keys(command.colors).length&&command.name===undefined||Object.entries(command.colors).some(([k,v])=>!THEME_COLOR_SLOTS.includes(k)||typeof v!=='string'||!/^#[0-9a-f]{6}$/iu.test(v)))invalid('invalid color scheme');
        return {...command,...(command.name!==undefined?{name:command.name.trim()}:{}),colors:Object.fromEntries(Object.entries(command.colors).map(([k,v])=>[k,v.toUpperCase()]))};
    }
    if(!exact(command,['type','slide','name','major','minor','fonts'],['type','slide'])||command.name!==undefined&&!name(command.name)||['major','minor'].some(k=>command[k]!==undefined&&!name(command[k]))||command.fonts===undefined&&command.major===undefined&&command.minor===undefined||command.fonts!==undefined&&['name','major','minor'].some(k=>command[k]!==undefined))invalid('invalid font command');
    return command.fonts?{...command,fonts:checkedThemeFonts(command.fonts)}:{...command,...Object.fromEntries(['name','major','minor'].filter(k=>command[k]!==undefined).map(k=>[k,command[k].trim()]))};
}

/** Build variants from an explicit stable anchor when the caller owns presets.
 * Otherwise the supplied source scheme is the anchor. Colors remain opaque RGB. */
export function presentationThemeVariants({name:themeName,colorsName,colors},anchor=null){
    const base=anchor??{name:colorsName,colors};
    if(!name(themeName)||!name(base.name)||!Array.isArray(base.colors)||base.colors.length!==12||!base.colors.every(v=>/^#[0-9a-f]{6}$/iu.test(v)))invalid('invalid variant anchor');
    const original=base.colors.map(v=>v.toUpperCase()),rotate=by=>original.map((c,i)=>i>=4&&i<10?original[4+(i-4+by)%6]:c),dark=[...original];
    [dark[0],dark[1]]=[dark[1],dark[0]];[dark[2],dark[3]]=[dark[3],dark[2]];
    return [original,rotate(1),rotate(3),dark].map((values,i)=>({name:base.name+(i?' '+(i+1):''),colors:values}));
}

export function checkedThemeReply(reply,expected){
    if(expected!==undefined&&reply?.revision!==expected||!themeAsset(reply?.result?.themeAsset)||!name(reply.result.themeName)||!Number.isSafeInteger(reply.result.themeBytes)||reply.result.themeBytes<1||reply.result.themeBytes>THEME_BYTES)throw failure('INVALID_RESPONSE','Invalid prepared theme snapshot');return reply;
}
