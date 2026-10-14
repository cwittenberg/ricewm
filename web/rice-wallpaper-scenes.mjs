/** App-aware wallpaper views. Coordinates are normalized focal points, zoom is cover-relative. */
export const DEFAULT_WALLPAPER_VIEW = Object.freeze({x:0.5,y:0.5,zoom:1});
export function wallpaperView(value={}) {
    if(!value||typeof value!=='object'||Array.isArray(value))throw new TypeError('Invalid wallpaper view');
    const view={x:value.x??0.5,y:value.y??0.5,zoom:value.zoom??1};
    for(const [key,min,max] of [['x',0,1],['y',0,1],['zoom',1,6]]) if(!Number.isFinite(view[key])||view[key]<min||view[key]>max)throw new TypeError(`Invalid wallpaper ${key}`);
    return view;
}
export function wallpaperFlight(value={}) {
    if(!value||typeof value!=='object'||Array.isArray(value))throw new TypeError('Invalid wallpaper flight');
    if(value.enabled!==undefined&&typeof value.enabled!=='boolean')throw new TypeError('Wallpaper flight must be on or off');
    if(value.views!==undefined&&(!value.views||typeof value.views!=='object'||Array.isArray(value.views)))throw new TypeError('Wallpaper app views must be a map');
    const duration=value.duration??1200;
    if(!Number.isInteger(duration)||duration<0||duration>5000)throw new TypeError('Wallpaper duration must be 0–5000 ms');
    const views=Object.create(null);
    if(Object.keys(value.views||{}).length>64)throw new TypeError('Too many app wallpaper views');
    for(const [app,view] of Object.entries(value.views||{})) {
        if(['__proto__','prototype','constructor'].includes(app)||!/^[a-zA-Z0-9_.:@+\/-]{1,128}$/.test(app))throw new TypeError('Invalid wallpaper app');
        views[app]={...wallpaperView(view)};
        if(view.wallpaper!==undefined){if(!/^[a-zA-Z0-9_-]{1,64}$/.test(view.wallpaper))throw new TypeError('Invalid assigned wallpaper');views[app].wallpaper=view.wallpaper;}
    }
    return {enabled:value.enabled===true,duration,views};
}
export const WALLPAPER_ATTRIBUTION_LICENSES=Object.freeze({'CC-BY-3.0':'https://creativecommons.org/licenses/by/3.0/','CC-BY-4.0':'https://creativecommons.org/licenses/by/4.0/'});
export const MAX_VIDEO_WALLPAPER_BYTES=16*1024*1024;
export function wallpaperMedia(item) {
    if(!item||!/^[a-zA-Z0-9_-]{1,64}$/.test(item.id)||typeof item.source!=='string'||item.source.length>2048||/[\x00-\x1f]/.test(item.source))throw new TypeError('Invalid wallpaper');
    if(item.source.startsWith('/')&&!item.source.startsWith('//')){if(item.source.split('/').some(p=>p==='..'||p==='.'))throw new TypeError('Invalid wallpaper path');}
    else {const url=new URL(item.source);if(url.protocol!=='https:'||url.username||url.password)throw new TypeError('Use HTTPS or an absolute file path');}
    if(!['image','video'].includes(item.type||'image'))throw new TypeError('Choose image or video');
    let attribution;
    if(item.attribution){const a=item.attribution,u=new URL(a.source);if(u.protocol!=='https:'||u.username||u.password||u.href.length>2048)throw new TypeError('Invalid wallpaper attribution');attribution={source:u.href,author:String(a.author||'').slice(0,180),license:String(a.license||'').slice(0,80)};
        if(a.licenseUrl!==undefined){const url=new URL(a.licenseUrl);if(url.protocol!=='https:'||url.username||url.password||url.href.length>2048)throw new TypeError('Invalid wallpaper license URL');attribution.licenseUrl=url.href;}
        for(const [key,max] of [['credit',360],['changes',240]])if(a[key]!==undefined){if(typeof a[key]!=='string'||!a[key].trim()||a[key].length>max||/[\x00-\x1f]/.test(a[key]))throw new TypeError('Invalid wallpaper '+key);attribution[key]=a[key];}
        const licenseUrl=WALLPAPER_ATTRIBUTION_LICENSES[a.license];if(licenseUrl){if(a.licenseUrl!==licenseUrl||typeof a.credit!=='string'||!a.credit.trim()||a.credit.length>360||typeof a.changes!=='string'||!a.changes.trim()||a.changes.length>240||/[\x00-\x1f]/.test(a.credit+a.changes)||!attribution.author)throw new TypeError('CC BY requires creator credit, canonical license and change notice');Object.assign(attribution,{credit:a.credit,licenseUrl,changes:a.changes});}
    }
    const metadata={};
    for(const key of ['poster','preview'])if(item[key]!==undefined){const value=item[key];if(typeof value!=='string'||value.length>2048||/[\x00-\x1f]/.test(value))throw new TypeError('Invalid wallpaper '+key);if(value.startsWith('/')&&!value.startsWith('//')){if(value.split('/').some(part=>part==='..'||part==='.'))throw new TypeError('Invalid wallpaper '+key);}else {const url=new URL(value);if(url.protocol!=='https:'||url.username||url.password)throw new TypeError('Invalid wallpaper '+key);}metadata[key]=value;}
    for(const key of ['sha256','posterSha256','previewSha256'])if(item[key]!==undefined){if(!/^[a-f0-9]{64}$/.test(item[key]))throw new TypeError('Invalid wallpaper integrity');metadata[key]=item[key];}
    for(const [key,max] of [['bytes',MAX_VIDEO_WALLPAPER_BYTES],['previewBytes',2*1024*1024],['durationMillis',30050],['width',3840],['height',2160]])if(item[key]!==undefined){if(!Number.isInteger(item[key])||item[key]<1||item[key]>max)throw new TypeError('Invalid wallpaper '+key);metadata[key]=item[key];}
    if(item.frameRate!==undefined){if(!Number.isFinite(item.frameRate)||item.frameRate<1||item.frameRate>30)throw new TypeError('Invalid wallpaper frame rate');metadata.frameRate=item.frameRate;}
    return {id:item.id,name:String(item.name||item.id).slice(0,96),source:item.source,type:item.type||'image',mode:['Crop','Fit','Stretch','Tile','Center'].includes(item.mode)?item.mode:'Crop',...metadata,...(attribution?{attribution}:{})};
}
export function wallpaperCatalog(theme,raw) {
    const custom=parseWallpaperPreferences(raw)?.themes[theme?.file]?.media||[];
    return [...(theme?.bundle?.wallpapers||[]),...custom.filter(item=>!theme?.bundle?.wallpapers?.some(base=>base.id===item.id))];
}
export function parseWallpaperPreferences(raw) {
    try {
        if(typeof raw!=='string'||raw.length>16000)return null;
        const value=JSON.parse(raw);if(!value||value.version!==1||!value.themes||Array.isArray(value.themes)||typeof value.themes!=='object')return null;
        const themes=Object.create(null);if(Object.keys(value.themes).length>64)return null;
        for(const [file,record] of Object.entries(value.themes)){
            if(!/^[a-zA-Z0-9_.-]{1,128}$/.test(file)||!record||typeof record!=='object')return null;
            const wallpapers=Object.create(null);if(Object.keys(record.wallpapers||{}).length>32)return null;
            for(const [id,flight] of Object.entries(record.wallpapers||{})){if(!/^[a-zA-Z0-9_-]{1,64}$/.test(id))return null;wallpapers[id]=wallpaperFlight(flight);}
            if(record.selected!==undefined&&!/^[a-zA-Z0-9_-]{1,64}$/.test(record.selected))return null;
            if(!Array.isArray(record.media||[])||(record.media||[]).length>32)return null;
            const media=(record.media||[]).map(wallpaperMedia);if(new Set(media.map(m=>m.id)).size!==media.length)return null;
            if(record.appViewsEnabled!==undefined&&typeof record.appViewsEnabled!=='boolean')return null;
            themes[file]={...(record.appViewsEnabled===undefined?{}:{appViewsEnabled:record.appViewsEnabled}),...(record.selected?{selected:record.selected}:{}),wallpapers,media,suspended:record.suspended===true};
        }
        return {version:1,themes,manual:value.manual===true};
    }catch{return null;}
}
export function selectWallpaperScene(theme,raw,app='desktop',reduced=false,workspaceWallpaper=null) {
    if(workspaceWallpaper?.path)return {flying:false,source:workspaceWallpaper.path,type:'image',mode:workspaceWallpaper.mode||'Crop',crop:DEFAULT_WALLPAPER_VIEW,duration:0,id:'workspace-wallpaper'};
    const parsed=parseWallpaperPreferences(raw);if(parsed?.manual)return null;
    const preferences=parsed?.themes[theme?.file]||{};
    if(preferences.suspended)return null;
    const wallpapers=wallpaperCatalog(theme,raw);
    const base=wallpapers.find(item=>item.id===(preferences.selected||theme?.bundle?.defaultWallpaper));
    if(!base)return null;
    const flight=wallpaperFlight(preferences.wallpapers?.[base.id]||base.flight||{});
    // Disabled flight still allows explicit app-to-wallpaper associations; its crops stay static.
    const appViewsEnabled=preferences.appViewsEnabled??theme?.bundle?.appViewsEnabled??true;
    if(!appViewsEnabled)return {flying:false,source:base.source,type:base.type||'image',mode:base.mode||'Crop',crop:DEFAULT_WALLPAPER_VIEW,duration:0,id:base.id};
    const key=app==='bowl'?'rice-bowl':app;
    const requested=flight.views[key]||flight.views.desktop||DEFAULT_WALLPAPER_VIEW;
    const target=wallpapers.find(item=>item.id===requested.wallpaper)||base;
    const crop=flight.enabled?wallpaperView(requested):DEFAULT_WALLPAPER_VIEW;
    return {flying:flight.enabled,source:target.source,type:target.type||'image',mode:target.mode||'Crop',crop,duration:reduced?0:flight.enabled?flight.duration:0,id:target.id};
}
export class RiceWallpaperScenes {
    constructor({load,release=()=>{},redraw=()=>{},error=()=>{}}) {Object.assign(this,{load,release,redraw,error});this.current=null;this.generation=0;this.prepareGeneration=0;this.closed=false;this.prepared=null;}
    /** Decode before committing settings; update consumes this exact live session. */
    async prepare(scene) {
        if(this.closed)throw Error('Wallpaper scenes are closed');
        const key=(scene.type||'image')+':'+scene.source,generation=++this.prepareGeneration;
        if(this.prepared){this.prepared.consumed=true;this.#release(this.prepared.media,this);this.prepared=null;}
        if(this.current?.sourceKey===key&&this.currentProvider===this)return {cancel(){}};
        const media=await this.load(scene);
        if(this.closed||generation!==this.prepareGeneration){this.#release(media,this);throw Error('Wallpaper selection was superseded');}
        const prepared={key,media,consumed:false};this.prepared=prepared;
        return {cancel:()=>{if(!prepared.consumed){prepared.consumed=true;if(this.prepared===prepared)this.prepared=null;this.#release(media,this);}}};
    }
    #release(media,provider) {
        try{Promise.resolve(provider.release?.(media)).catch(error=>this.error(error));}
        catch(error){this.error(error);}
    }
    update(scene,provider=this) {
        if(this.closed)return;
        if(typeof provider?.load!=='function')throw TypeError('Wallpaper media provider is unavailable');
        this.desired=scene;
        if(this.failed){this.failed=false;this.pending=null;}
        if(!scene){this.generation++;this.pending=null;if(this.current){this.#release(this.current,this.currentProvider);this.current=null;this.currentProvider=null;}return;}
        const sourceKey=scene.type+':'+scene.source;
        if(this.current?.sourceKey===sourceKey&&this.currentProvider===provider){this.current={...this.current,...scene};if(this.pending&&(this.pending!==sourceKey||this.pendingProvider!==provider)){this.generation++;this.pending=null;}return;}
        if(this.pending===sourceKey&&this.pendingProvider===provider)return;
        this.pending=sourceKey;this.pendingProvider=provider;const generation=++this.generation;
        Promise.resolve().then(()=>{
            const prepared=this.prepared;
            if(provider===this&&prepared?.key===sourceKey){prepared.consumed=true;this.prepared=null;return prepared.media;}
            return provider.load(scene);
        }).then(media=>{
            if(this.closed||generation!==this.generation){this.#release(media,provider);return;}
            const previous=this.current,previousProvider=this.currentProvider;this.current={...media,...this.desired,sourceKey};this.currentProvider=provider;this.pending=null;
            if(previous)this.#release(previous,previousProvider);this.redraw();
        }).catch(error=>{if(generation===this.generation){this.pending=null;this.error(error);/* Retry only after another scene selection. */this.pending=sourceKey;this.failed=true;}});
    }
    close(){this.closed=true;this.generation++;this.prepareGeneration++;if(this.prepared){this.prepared.consumed=true;this.#release(this.prepared.media,this);}this.prepared=null;if(this.current)this.#release(this.current,this.currentProvider);this.current=null;this.currentProvider=null;this.pendingProvider=null;}
}
