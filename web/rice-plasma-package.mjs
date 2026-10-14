/** KPackage ingestion: preserve upstream bytes; describe configuration for the shared editor. */
import {extractArchive} from './rice-archive.mjs';
const decoder=new TextDecoder('utf-8',{fatal:true});
const text=bytes=>decoder.decode(new Uint8Array(bytes));
const xmlText=value=>value.replace(/&(?:lt|gt|amp|quot|apos);/g,s=>({'&lt;':'<','&gt;':'>','&amp;':'&','&quot;':'"','&apos;':"'"})[s]);
export function plasmaConfiguration(xml='') {
    if(xml.length>256*1024||/<!DOCTYPE|<!ENTITY/i.test(xml))throw Error('Unsupported widget configuration document');
    // XML comments are not configuration entries (some packages retain old defaults here).
    xml=xml.replace(/<!--[\s\S]*?-->/g,'');
    const fields=[];
    let entryCount=0;
    for(const match of xml.matchAll(/<entry\b([^>]*)>([\s\S]*?)<\/entry>/g)){
        const attrs=Object.fromEntries([...match[1].matchAll(/([\w]+)\s*=\s*["']([^"']*)["']/g)].map(m=>[m[1],xmlText(m[2])]));
        const name=attrs.name,type=['Bool','Int','UInt','LongLong','ULongLong','Double','Enum','String','StringList','Color','Font','Path','PathList','Url','UrlList','Rect','Size','Point','DateTime'].find(type=>type.toLowerCase()===attrs.type?.toLowerCase())||attrs.type;if(!name||['__proto__','constructor','prototype'].includes(name)||++entryCount>256)throw Error('Invalid widget configuration entries');
        const read=tag=>xmlText(match[2].match(new RegExp('<'+tag+'(?:\\s[^>]*)?>([\\s\\S]*?)</'+tag+'>'))?.[1]||'').trim();
        const choices=[...match[2].matchAll(/<choice\b[^>]*name=["']([^"']+)["'][^>]*(?:\/>|>([\s\S]*?)<\/choice>)/g)].map(m=>({name:xmlText(m[1]),label:xmlText(m[2]?.match(/<label>([\s\S]*?)<\/label>/)?.[1]||m[1])}));
        const raw=read('default');let value=raw;
        if(type==='Bool')value=raw==='true'||raw==='1';
        else if(['Int','UInt','LongLong','ULongLong','Double','Enum'].includes(type))value=Number(raw)||0;
        else if(type==='StringList')value=raw?raw.split(','):[];
        if(type==='Enum'&&choices.some(c=>c.name===raw))value=choices.findIndex(c=>c.name===raw);
        const title=name.replace(/_/g,' ').replace(/([a-z])([A-Z])/g,'$1 $2').replace(/\bPct$/,'(%)').replace(/\bPx$/,'(px)').replace(/^./,c=>c.toUpperCase());
        // KCoreConfigSkeleton::addItem replaces the name lookup with the last
        // declaration. Match that lookup while leaving the original XML untouched.
        const previous=fields.findIndex(field=>field.name===name);
        if(previous!==-1)fields.splice(previous,1);
        fields.push({name,type,...(choices.length?{choices}:{}),label:read('label')||title,description:read('whatsthis'),default:value,...(read('min')?{min:Number(read('min'))}:{}),...(read('max')?{max:Number(read('max'))}:{} )});
    }
    return fields;
}
export function plasmaSettings(input={},fields=[]) {
    if(!input||typeof input!=='object'||Array.isArray(input))throw Error('Invalid widget preferences');
    const result={};
    for(const [name,value] of Object.entries(input)){
        const field=fields.find(f=>f.name===name);if(!field)throw Error('Unknown widget preference: '+name);
        const valid=field.type==='Bool'?typeof value==='boolean':typeof field.default==='number'?Number.isFinite(value)&&(field.min===undefined||value>=field.min)&&(field.max===undefined||value<=field.max):field.type==='StringList'?Array.isArray(value)&&value.length<=128&&value.every(v=>typeof v==='string'&&v.length<4096):typeof value==='string'&&value.length<=16384;
        if(!valid||(['Int','UInt','LongLong','ULongLong','Enum'].includes(field.type)&&!Number.isSafeInteger(value))||(['UInt','ULongLong'].includes(field.type)&&value<0)||(field.choices&&(value<0||value>=field.choices.length)))throw Error('Invalid widget preference: '+field.label);result[name]=value;
    }
    return result;
}
// Plasma 5 packages use desktop-entry metadata. Adapt its descriptor without
// generating or changing any file inside the original package.
export function plasmaDesktopMetadata(source) {
    const values=Object.create(null);let inDesktop=false;
    for(const line of source.split(/\r?\n/)){
        const value=line.trim();if(!value||value.startsWith('#'))continue;
        if(value.startsWith('[')){inDesktop=value==='[Desktop Entry]';continue;}
        if(!inDesktop)continue;
        const at=value.indexOf('=');if(at<1)continue;
        const key=value.slice(0,at).trim();
        values[key]=value.slice(at+1).replace(/\\([sntr\\])/g,(_,c)=>({s:' ',n:'\n',t:'\t',r:'\r','\\':'\\'})[c]);
    }
    if(values['X-Plasma-API']!=='declarativeappletscript'||
        !(values['X-KDE-ServiceTypes']||values.ServiceTypes||'').split(/[;,]/).includes('Plasma/Applet'))throw Error('This package is not a Plasma widget');
    return {KPackageStructure:'Plasma/Applet','X-Plasma-MainScript':values['X-Plasma-MainScript'],'X-Plasma-RootPath':values['X-Plasma-RootPath'],'X-KPackage-Dependencies':values['X-KPackage-Dependencies'],KPlugin:{Id:values['X-KDE-PluginInfo-Name'],Name:values.Name,Description:values.Comment,Version:values['X-KDE-PluginInfo-Version']}};
}
export function describePlasmaPackage(files,{source=null,allowDependencies=false}={}) {
    const metadataFiles=files.filter(f=>/(?:^|\/)metadata\.(?:json|desktop)$/.test(f.path));
    const candidates=[]; const missingRoots=[];
    for(const file of metadataFiles){
        const prefix=file.path.slice(0,file.path.lastIndexOf('/')+1);
        if(file.path.endsWith('.desktop')&&files.some(other=>other.path===prefix+'metadata.json'))continue;
        if(file.bytes.length>256*1024)throw Error('Widget metadata exceeds size limit');
        const metadata=file.path.endsWith('.json')?JSON.parse(text(file.bytes)):plasmaDesktopMetadata(text(file.bytes));
        const script=metadata['X-Plasma-MainScript']||'ui/main.qml';
        if(typeof script!=='string'||!script.startsWith('ui/')||!script.endsWith('.qml')||script.includes('\\')||script.split('/').some(p=>!p||p==='.'||p==='..'))throw Error('Unsafe widget entry point: '+script);
        const entry='contents/'+script;
        if(files.some(other=>other.path===prefix+entry))candidates.push({prefix,metadata,entry});
        else if(metadata['X-Plasma-RootPath']){missingRoots.push(String(metadata['X-Plasma-RootPath']));if(allowDependencies)candidates.push({prefix,metadata,entry});}
    }
    if(candidates.length!==1){
        if(!candidates.length&&missingRoots.length)throw Error('Widget requires a shared package dependency: '+missingRoots.join(', '));
        throw Error(candidates.length?'Choose an archive containing one Plasma widget':'Widget archive has no declared QML entry point');
    }
    const {prefix,metadata,entry}=candidates[0];
    const structure=metadata.KPackageStructure;
    const implicitPlasma=structure===undefined && (metadata['X-Plasma-API']==='declarativeappletscript'||
        /^\d+(?:\.\d+){0,2}$/.test(String(metadata['X-Plasma-API-Minimum-Version']||'')));
    if(structure!=='Plasma/Applet'&&!implicitPlasma)throw Error('This package is not a Plasma widget');
    const plugin=metadata.KPlugin;if(!plugin||!/^[a-zA-Z0-9][a-zA-Z0-9._:@+-]{0,127}$/.test(plugin.Id)||!plugin.Name)throw Error('Widget package metadata is incomplete');
    const rootPackage=metadata['X-Plasma-RootPath'];
    if(rootPackage&&!/^[a-zA-Z0-9][a-zA-Z0-9._:@+-]{0,127}$/.test(rootPackage))throw Error('Invalid shared widget package ID');
    let declared=metadata['X-KPackage-Dependencies'];
    const legacy=files.find(f=>f.path===prefix+'metadata.desktop');
    if(rootPackage&&!declared&&legacy){
        const companion=plasmaDesktopMetadata(text(legacy.bytes));
        if(companion.KPlugin.Id!==plugin.Id)throw Error('Widget metadata identities disagree');
        declared=companion['X-KPackage-Dependencies'];
    }
    const dependencyUrls=Array.isArray(declared)?declared:typeof declared==='string'?declared.split(/[;,]/).filter(Boolean):[];
    if(dependencyUrls.length>8)throw Error('Too many widget dependencies');
    const dependencies=dependencyUrls.map(url=>{
        const match=typeof url==='string'&&url.match(/^kns:\/\/plasmoids\.knsrc\/(?:api\.kde-look\.org|api\.opendesktop\.org)\/([1-9][0-9]*)$/);
        if(!match)throw Error('Unsupported widget dependency: '+url);
        return {provider:'opendesktop',id:match[1]};
    });
    if(rootPackage&&!dependencies.length)throw Error('Shared widget package has no supported dependency declaration: '+rootPackage);
    const content=files.filter(f=>f.path.startsWith(prefix)).map(f=>({path:f.path.slice(prefix.length),bytes:new Uint8Array(f.bytes),...(Number.isInteger(f.mode)?{mode:f.mode&0o777}:{})}));
    let bytes=0;for(const file of content){if(file.path.startsWith('/')||file.path.includes('\\')||file.path.split('/').some(p=>!p||p==='.'||p==='..'))throw Error('Unsafe package path');bytes+=file.bytes.length;}
    if(bytes>32*1024*1024||content.length>2048)throw Error('Widget package exceeds the installation limit');
    const configuration=content.find(f=>f.path==='contents/config/main.xml');
    return {descriptor:{schema:'plasma.applet/v1',id:plugin.Id,name:String(plugin.Name).slice(0,96),description:String(plugin.Description||'').slice(0,512),version:String(plugin.Version||''),size:[320,240],entry,fields:plasmaConfiguration(configuration?text(configuration.bytes):''),source,metadata,...(rootPackage?{rootPackage}:{}),...(dependencies.length?{dependencies}:{})},files:content};
}
export async function unpackPlasmaArchive(bytes,filename,options={}) {
    if(bytes.length>32*1024*1024)throw Error('Widget download exceeds 32 MiB');
    const archive='/'+String(filename).replace(/[^a-zA-Z0-9._-]/g,'_'),files=new Map([[archive,new Uint8Array(bytes)]]),modes=new Map(),dirs=new Set(['/','/out']);
    const filesystem={mergedFileBytes:p=>files.get(p),lstatKind:p=>dirs.has(p)?2:files.has(p)?1:0,mkdir:p=>{dirs.add(p);return 0;},putFile:(p,b,mode)=>{files.set(p,new Uint8Array(b));modes.set(p,mode);return 0;}};
    await extractArchive(filesystem,{archive,destination:'/out',onProgress:()=>{}});
    return describePlasmaPackage([...files].filter(([p])=>p.startsWith('/out/')).map(([path,bytes])=>({path:path.slice(5),bytes,mode:modes.get(path)})),options);
}
