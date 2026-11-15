import {openTypographyEngine} from './typography-engine.mjs';
import {SDK_RESOURCE_PROFILES} from './resource-profiles.mjs';
import {createCancellationScope} from './cancellation.mjs';

const validPath=p=>typeof p==='string'&&/^(?!\/)(?!.*(?:^|\/)\.{1,2}(?:\/|$))[A-Za-z0-9._/-]{1,255}$/u.test(p);
const familyKey=name=>String(name).trim().toLowerCase();
const generic=new Map([['sans-serif','Liberation Sans'],['sans','Liberation Sans'],['serif','Liberation Serif']]);
const identity=font=>(font.resource?'sdk:':'package:')+(font.resource||font.path);
const error=(code,message)=>Object.assign(new Error('Typography: '+message),{code});
function families(value){
    if(typeof value!=='string'||!value.trim()||value.length>4096||/[\x00-\x1f\x7f]/u.test(value))throw TypeError('Invalid typography family list');
    const names=[];let token='',quote=null,escape=false;
    for(const c of value){
        if(escape){token+=c;escape=false;continue;}
        if(c==='\\'){escape=true;continue;}
        if(quote){if(c===quote)quote=null;else token+=c;continue;}
        if(c==='"'||c==="'"){quote=c;continue;}
        if(c===','){if(token.trim())names.push(token.trim());token='';}else token+=c;
    }
    if(quote||escape)throw TypeError('Incomplete typography family list');
    if(token.trim())names.push(token.trim());
    if(names.length>32||names.some(name=>name.length>256))throw TypeError('Typography family list exceeds its limit');
    return names.map(name=>generic.get(familyKey(name))||name);
}
function descriptor(font,shared=false){
    if(!font||typeof font.family!=='string'||!font.family.trim()||font.family.length>256)throw TypeError('Font catalog needs real family names');
    const weight=font.weight??400,italic=font.italic??font.style==='italic';
    if(!Number.isFinite(weight)||weight<1||weight>1000||typeof italic!=='boolean')throw TypeError('Invalid font face style');
    if(font.variableWeight!==undefined&&typeof font.variableWeight!=='boolean'||font.weightRange!=null&&(!Array.isArray(font.weightRange)||font.weightRange.length!==2||font.weightRange.some(weight=>!Number.isFinite(weight)||weight<1||weight>1000)||font.weightRange[0]>font.weightRange[1]))throw TypeError('Invalid font variation range');
    if(!shared&&!validPath(font.path))throw TypeError('Invalid package font path');
    const result={family:font.family,weight,italic,variableWeight:Boolean(font.variableWeight),weightRange:font.weightRange??null};
    if(shared){
        if(!Array.isArray(font.parts)||font.parts.length!==1||typeof font.parts[0].resource!=='string')throw TypeError('Shared typography requires a complete selected SDK face');
        Object.assign(result,{resource:font.parts[0].resource,bytes:font.bytes,sha256:font.parts[0].sha256});
    }else result.path=font.path;
    return Object.freeze(result);
}

/** One active worker; actual requested faces come from immutable package/SDK
 * resources. A theme pair can stay resident without exceeding native limits. */
export async function openTypographyFontBank({services,path='typography.wasm',fontManifest='fonts-manifest.json',sharedFonts=SDK_RESOURCE_PROFILES.profiles.typography.fonts,signal}={}){
    if(!validPath(path)||!validPath(fontManifest)||!services?.assets||!services?.compute||!Array.isArray(sharedFonts)||sharedFonts.length>256)throw TypeError('Invalid typography font bank');
    sharedFonts=structuredClone(sharedFonts);
    const lifetime=createCancellationScope();let closed=false,pending=false,engine=null,preferred=[],closing=null;
    const alive=()=>{if(closed||signal?.aborted)throw error('CLOSED','font bank closed');};
    const close=()=>{
        if(closing)return closing;closed=true;lifetime.abort();signal?.removeEventListener('abort',abort);
        closing=Promise.resolve(engine?.close());return closing;
    };
    const abort=()=>{void close().catch(()=>{});};signal?.addEventListener('abort',abort,{once:true});
    try{
        alive();const info=await services.assets.stat({path:fontManifest});alive();
        if(!Number.isSafeInteger(info?.size)||info.size<1||info.size>65536)throw TypeError('Invalid font manifest size');
        const data=await services.assets.read({path:fontManifest,offset:0,length:info.size});alive();
        if(!(data instanceof Uint8Array)||data.length!==info.size)throw TypeError('Incomplete font manifest');
        const manifest=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(data));
        if(manifest?.version!==1||!Array.isArray(manifest.fonts)||!manifest.fonts.length&&!sharedFonts.length||manifest.fonts.length>64)throw TypeError('Invalid font manifest');
        // A selected SDK family owns its complete face set; do not mix font
        // generations with same-named package faces and different metrics.
        const catalog=new Map();
        for(const font of sharedFonts){const face=descriptor(font,true),key=familyKey(face.family);if(!catalog.has(key))catalog.set(key,[]);catalog.get(key).push(face);}
        const shared=new Set(catalog.keys());
        for(const font of manifest.fonts){const face=descriptor(font),key=familyKey(face.family);if(shared.has(key))continue;if(!catalog.has(key))catalog.set(key,[]);catalog.get(key).push(face);}
        const choose=(name,bold=false,italic=false)=>{
            const faces=catalog.get(familyKey(name));if(!faces?.length)return null;
            const style=faces.filter(face=>face.italic===Boolean(italic)),weight=bold?700:400,available=style.length?style:faces;
            return available.find(face=>face.weight===weight&&!face.variableWeight)||available.find(face=>face.variableWeight&&(!face.weightRange||weight>=face.weightRange[0]&&weight<=face.weightRange[1]))||available.reduce((best,face)=>Math.abs(face.weight-weight)<Math.abs(best.weight-weight)?face:best);
        };
        const selectedFonts=query=>{
            const required=new Map(),add=face=>{if(face)required.set(identity(face),face);};
            // Script fallbacks are requested by actual text, not by opening an
            // Office document or selecting a theme. Explicit family selections
            // below still load their face even for Latin-only text.
            add(choose('Liberation Sans'));
            if(/[\p{Script=Han}\p{Script=Hiragana}\p{Script=Katakana}\p{Script=Hangul}]/u.test(query.text||''))add(choose('Noto Sans JP'));
            if(/\p{Script=Arabic}/u.test(query.text||''))add(choose('Noto Sans Arabic'));
            // Use the existing OS symbol face only for text that needs these
            // ranges. Ordinary paragraphs retain the same sixteen-face budget.
            if(/[\u2190-\u2bff]/u.test(query.text||''))add(choose('DejaVu Sans'));
            if(!required.size)add(catalog.values().next().value?.[0]);
            if(query.spans!==undefined&&(!Array.isArray(query.spans)||query.spans.length>256))throw TypeError('Invalid typography spans');
            for(const style of [query,...(query.spans||[])])for(const name of families(style.families??query.families??'Liberation Sans'))add(choose(name,style.bold??query.bold,style.italic??query.italic));
            if(required.size>16)throw error('FONT_SET_LIMIT','this text requires more than sixteen native font faces');
            // Preferred theme families describe document choices. They are not
            // a request to download every face or unused heading/body family.
            if(!required.size)throw error('FONT_UNAVAILABLE','no matching or fallback face is available');
            return [...required.values()];
        };
        return Object.freeze({
            version:1,
            get fonts(){return engine?.fonts||[];},
            setPreferredFamilies(names){alive();if(!Array.isArray(names)||names.length>16||names.some(name=>typeof name!=='string'||!name.trim()||name.length>256))throw TypeError('Invalid preferred font families');preferred=[...new Set(names)];},
            resolveDocumentFamily(requested){return choose(requested)?.family||choose(generic.get(familyKey(requested)))?.family||choose('Liberation Sans')?.family||catalog.values().next().value?.[0]?.family;},
            async shape(request={}){
                alive();if(pending)throw error('BUSY','font operation already pending');pending=true;
                try{
                    const query=structuredClone(request),fonts=selectedFonts(query),needed=new Set(fonts.map(identity));
                    if(!engine||[...needed].some(key=>!engine.fonts.some(font=>identity(font)===key))){
                        const previous=engine;engine=null;if(previous)await previous.close();alive();
                        engine=await openTypographyEngine({services,path,fonts,signal:lifetime.signal});alive();
                    }
                    const frame=await engine.shape(query);alive();
                    return {...frame,fontFaces:engine.fonts};
                }finally{pending=false;}
            },
            close,
        });
    }catch(failure){await close().catch(()=>{});throw failure;}
}
