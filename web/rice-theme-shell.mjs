import {THEME_FONT_FAMILIES} from './rice-theme-customization.mjs';

const font={fontFamily:Object.keys(THEME_FONT_FAMILIES),fontSize:{min:8,max:36},fontWeight:{min:100,max:900},italic:'boolean'};
/** Declarative shell geometry. Zero/omitted dimensions preserve engine defaults. */
export const SHELL_APPEARANCE_SCHEMA=Object.freeze({
    dock:{folders:'boolean'},
    menuBar:{mode:['standard','application'],showAppName:'boolean'},
    launcher:{...font,width:{min:36,max:320},icon:'semantic icon name',iconSize:{min:12,max:32}},
    menu:{...font,rootWidth:{min:160,max:600},cascadeWidth:{min:120,max:400},rootColumns:[1,2],
        railWidth:{min:0,max:64},railText:'plain text',railIcon:'semantic icon name',
        headerHeight:{min:0,max:96},headerText:'plain text',headerIcon:'semantic icon name',
        footerHeight:{min:0,max:64},footerText:'plain text'},
});
export function validateShellAppearance(value){
    if(!value||typeof value!=='object'||Array.isArray(value))throw TypeError('Shell appearance must be an object');
    for(const [section,fields] of Object.entries(value)){
        const schema=SHELL_APPEARANCE_SCHEMA[section];
        if(!Object.hasOwn(SHELL_APPEARANCE_SCHEMA,section)||!schema||!fields||typeof fields!=='object'||Array.isArray(fields))throw TypeError('Invalid shell appearance section');
        for(const [key,v] of Object.entries(fields)){
            const rule=schema[key];
            const valid=Array.isArray(rule)?rule.includes(v):rule==='boolean'?typeof v==='boolean':rule==='plain text'?typeof v==='string'&&v.length<=96&&!/[\x00-\x1f\x7f]/.test(v):rule==='semantic icon name'?typeof v==='string'&&/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(v):rule&&Number.isInteger(v)&&v>=rule.min&&v<=rule.max;
            if(!valid)throw TypeError('Invalid shell appearance '+section+'.'+key);
        }
    }
    return structuredClone(value);
}
export function shellThemeFont(style={},fallback={},size){
    return `${style.italic?'italic ':''}${style.fontWeight??fallback.bodyWeight??400} ${size??style.fontSize??fallback.bodyPx??13}px ${THEME_FONT_FAMILIES[style.fontFamily??fallback.family??'system']}`;
}
/** Mirrors the native launcher target; shortcut painting uses the same width. */
export function shellLauncherWidth(shell={},chrome={}){
    shell??={};chrome??={};
    if(shell.launcherVisible===false)return 0;
    if(chrome.shellAppearance?.launcher?.width)return chrome.shellAppearance.launcher.width;
    if(chrome.redmond98)return 59;
    const n=new TextEncoder().encode(shell.launcherText??'RiceWM').length;
    return shell.launcherAppearance==='Icon only'?36:shell.launcherAppearance==='Text only'?Math.max(44,Math.min(200,n*7+20)):Math.max(112,Math.min(200,n*7+44));
}
export function shellLauncherStyled(chrome){return Object.keys(chrome?.shellAppearance?.launcher||{}).length>0||Object.keys(chrome?.surfaceSkins?.startButton||{}).length>0||Object.keys(chrome?.controlColors?.startButton||{}).length>0;}

/** Paint into rectangles emitted by the native engine; no DOM/input overlays. */
export function paintShellThemePart(context,rect,chrome,role,{skins,icons,state='normal',text='',icon,style={},background,foreground}={}){
    context.save();
    try{
        context.beginPath();context.rect(rect.x,rect.y,rect.width,rect.height);context.clip();
        if(background){context.fillStyle=background;context.fillRect(rect.x,rect.y,rect.width,rect.height);}
        skins.paint(context,rect,chrome,role,state);
        const ink=skins.style(chrome,role,state).foreground||foreground||'#ffffff';
        const size=Math.min(style.iconSize??(role==='menuHeader'?32:22),rect.height-8,role==='menuRail'?rect.width-4:rect.width-8);
        let x=rect.x+8;
        if(icon&&size>0){icons?.renderNamed({icon,rect:{x,y:rect.y+(rect.height-size)/2,width:size,height:size},color:[parseInt(ink.slice(1,3),16)||0,parseInt(ink.slice(3,5),16)||0,parseInt(ink.slice(5,7),16)||0,255]});x+=size+6;}
        context.fillStyle=ink;context.font=shellThemeFont(style,chrome.typography);context.textAlign='left';context.textBaseline='middle';
        if(role==='menuRail'){
            context.translate(rect.x+rect.width/2,rect.y+rect.height-8);context.rotate(-Math.PI/2);context.fillText(text,0,0,Math.max(1,rect.height-16-(icon?size+8:0)));
        }else if(text)context.fillText(text,x,rect.y+rect.height/2,Math.max(1,rect.x+rect.width-x-8));
    }finally{context.restore();}
}
